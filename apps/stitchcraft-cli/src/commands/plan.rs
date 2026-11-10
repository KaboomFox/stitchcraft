//! `stitch plan DESIGN -o FILE.pes`: plans a design for a machine and writes the machine file, and a
//! picture of it and a report when asked. `stitch export` is the same command, by the name a VectorCraft
//! user looks for.
//!
//! **Designs.** A design is an SVG file, or a VectorCraft document, which its extension tells apart
//! ([`Source`], `REQ-VC-008`). Both are read into the engine's design model by their adapters, and all
//! that follows is the same for both: one way through the engine, so a document sews as its SVG would.
//!
//! **Pictures.** A PNG or JPEG file is a picture, which VectorCraft's command line traces in the machine's
//! threads ([`trace`], ADR-0015). From then on the document it saves is the design: read with `--auto`, so
//! that StitchCraft chooses every shape's stitches, written with `--traced` beside the other files, and
//! kept in a bug-report bundle in the picture's place. With `--subject`, the picture's subject is cut out
//! first and VectorCraft traces the cut-out ([`subject`], ADR-0019): what was kept is said before the rest,
//! `--cutout` writes the cut-out with the other files, and a report and a bundle record its marks and models.
//!
//! The engine does the planning (`stitchcraft_engine::plan`): it fits the plan to the machine and checks
//! it, so what comes back is written as it is. This command reads the design, prints what was said about
//! the design — element by element, and what the file format adds, such as DST's cuts before long jumps
//! (`SC-I0605`) — and writes the files. Nothing is written when anything said is an error, an element left
//! out included, except the report, which says why (`REQ-CLI-003`). The machine file, the picture and the
//! report are written all or none, and when one cannot be written, what was said about the design is
//! printed before it (`REQ-CLI-006`).
//!
//! What the summary, the report and the picture describe is the machine file as written, read back as a
//! machine reads it, in the design's colours (`REQ-CLI-008`): a DST machine cuts the thread before a long
//! jump, which the plan does not hold, and a PES file has no record for a trim at its very end.
//!
//! [`sew`] is the part a bug-report bundle records and replays (`super::bug_report`), so a bundle
//! reproduces exactly what this command does. When it finds a bug in StitchCraft (`SC-E0009`, or a panic),
//! this command writes a bundle next to the machine file it was asked for, and nothing else
//! (`REQ-CLI-002`). Everything it prints and writes is made before any file is written, and the bundle says
//! whether the panic came while planning or while drawing the picture, which a replay then draws too.

use std::cell::Cell;
use std::fmt::Write as _;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::Path;

use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use stitchcraft_core::{Budget, Code, Diagnostic, Severity};
use stitchcraft_engine::design::{Design, Reading};
use stitchcraft_formats::{Encoded, read_back};
use stitchcraft_plan::profiles;
use stitchcraft_plan::{FormatId, MachineProfile, StitchPlan, TrimSupport};
use stitchcraft_render::{Image, Settings, Style};

use super::bug_report::{self, Phase, Run};
use super::{
    Entries, Outcome, Status, asked_profile, bundle_free, describe_file, describe_plan, describe_threads, distinct, hex, output_format,
    render_diagnostics, write_files,
};
use crate::cli::{PlanArgs, SubjectArgs};
use crate::files::{self, Scratch, Unwritten};
use crate::subject::{self, Provider, Stopped, Subject};
use crate::trace;

/// What a design file is, which decides how it is read: a VectorCraft document by its extension, in any
/// case ([`stitchcraft_vectorcraft::EXTENSIONS`]), and an SVG file otherwise.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Source {
    /// An SVG file, read by `stitchcraft-svg`.
    Svg,
    /// A VectorCraft document, read by `stitchcraft-vectorcraft`.
    VectorCraft,
}

impl Source {
    /// What the design at `path` is, by its extension.
    pub fn of(path: &Path) -> Source {
        let extension = path.extension().and_then(|extension| extension.to_str()).unwrap_or_default();
        let document = stitchcraft_vectorcraft::EXTENSIONS.iter().any(|known| extension.eq_ignore_ascii_case(known));
        if document { Source::VectorCraft } else { Source::Svg }
    }

    /// The part of the design file `bytes` its reader reads, which reads as the file does: a VectorCraft
    /// document's JSON without the images and other data after it ([`stitchcraft_vectorcraft::read_part`]),
    /// and an SVG file whole. A bug-report bundle keeps it.
    pub fn read_part(self, bytes: &[u8]) -> &[u8] {
        match self {
            Source::Svg => bytes,
            Source::VectorCraft => stitchcraft_vectorcraft::read_part(bytes),
        }
    }

    /// The machine the design file `bytes` keeps for itself: a VectorCraft document's, which StitchCraft's
    /// Design Settings filter keeps with it (`REQ-VC-013`). An SVG file keeps none.
    pub fn machine(self, bytes: &[u8]) -> Option<&'static MachineProfile> {
        match self {
            Source::Svg => None,
            Source::VectorCraft => stitchcraft_vectorcraft::machine(bytes, &Budget::DEFAULT),
        }
    }

    /// The design in `bytes`, read as this kind of file as `reading` asks, with what its reader said; or why
    /// it can't be read.
    pub fn read(self, bytes: &[u8], reading: Reading) -> Result<(Design, Vec<Diagnostic>), Diagnostic> {
        match self {
            Source::Svg => stitchcraft_svg::read_with(bytes, &Budget::DEFAULT, reading).map(|svg| (svg.design, svg.warnings)),
            Source::VectorCraft => stitchcraft_vectorcraft::read_with(bytes, &Budget::DEFAULT, reading).map(|read| (read.design, read.warnings)),
        }
    }
}

/// A design planned for a machine and written in a format: what `stitch plan` writes, and what a
/// bug-report bundle records and replays.
#[derive(Debug)]
pub struct Sewn {
    /// Everything said: the reader's warnings, then the engine's diagnostics, then the format's notes.
    pub diagnostics: Vec<Diagnostic>,
    /// The plan, when the engine returned one.
    pub plan: Option<StitchPlan>,
    /// The machine file, when the plan could be written in the format.
    pub file: Option<Vec<u8>>,
    /// The machine file read back, with the plan's threads ([`read_back`]): what it sews.
    pub back: Option<StitchPlan>,
}

impl Sewn {
    /// Whether StitchCraft found a bug in itself: a failed plan check (`SC-E0009`).
    pub fn found_a_bug(&self) -> bool {
        self.diagnostics.iter().any(|d| d.code == Code::InternalCheckFailed)
    }
}

/// The design file `design`, of the kind `source`, read as `reading` asks, planned for `profile` and written in
/// `format`, with `name` as the design name machines show, and the file read back.
pub fn sew(source: Source, design: &[u8], reading: Reading, profile: &MachineProfile, format: FormatId, name: &str) -> Sewn {
    let (design, warnings) = match source.read(design, reading) {
        Ok(read) => read,
        Err(unreadable) => return Sewn { diagnostics: vec![unreadable], plan: None, file: None, back: None },
    };
    // Planned for the format it is written in, whose limits the file then keeps.
    let outcome = stitchcraft_engine::plan(&design, &profile.writing(format), &Budget::DEFAULT);
    let mut diagnostics = warnings;
    diagnostics.extend(outcome.diagnostics);
    let Some(plan) = outcome.plan else { return Sewn { diagnostics, plan: None, file: None, back: None } };
    match stitchcraft_formats::encode(&plan, format, name) {
        Ok(Encoded { bytes, notes }) => {
            diagnostics.extend(notes);
            let back = read_back(&bytes, &plan).map_err(|bug| diagnostics.push(bug)).ok();
            Sewn { diagnostics, plan: Some(plan), file: Some(bytes), back }
        }
        Err(e) => {
            diagnostics.push(e.diagnostic());
            Sewn { diagnostics, plan: Some(plan), file: None, back: None }
        }
    }
}

/// The picture `stitch plan --preview` draws of the file read back, `back`, on a machine that cuts as
/// `trims` says, the profile's: the realistic style at the default scale.
pub fn picture(back: &StitchPlan, trims: TrimSupport) -> Result<Image, Diagnostic> {
    let settings = Settings::new(Style::Realistic, Settings::DEFAULT_SCALE)
        .ok_or_else(|| Diagnostic::new(Code::InternalCheckFailed, "The picture's default settings are out of range."))?;
    stitchcraft_render::preview(back, trims, settings, &mut Budget::DEFAULT.meter()).map_err(|e| e.diagnostic())
}

/// The design name machines show for the design at `path`: its file name without the extension.
pub fn design_name(path: &Path) -> &str {
    path.file_stem().and_then(|s| s.to_str()).unwrap_or("design")
}

/// The machine the design file `design`, of the kind `source`, is planned for: the one `--profile` names,
/// `asked`, or else the one the file keeps, or else the reference machine.
pub fn machine_for(asked: Option<&'static MachineProfile>, source: Source, design: &[u8]) -> &'static MachineProfile {
    asked.or_else(|| source.machine(design)).unwrap_or(profiles::REFERENCE)
}

/// Runs `stitch plan`, a picture's subject cut out by the pinned models when `--subject` asks.
pub fn run(args: &PlanArgs) -> Outcome {
    run_with(args, &subject::fetched)
}

/// Why the cut-out's options don't fit the design `stitch plan` is given: they are for a picture, and the
/// marks and `--cutout` take `--subject` (`REQ-CLI-015`).
fn subject_options(picture: bool, subject: &SubjectArgs) -> Result<(), &'static str> {
    if !picture && subject.given() {
        return Err("--subject and its options are for a picture, a PNG or JPEG file");
    }
    if !subject.subject && subject.given() {
        return Err("--subject-box, --subject-point, --not-subject and --cutout take --subject");
    }
    Ok(())
}

/// Runs `stitch plan`, a picture's subject cut out by the models `models` gives when `--subject` asks.
pub fn run_with(args: &PlanArgs, models: &Provider) -> Outcome {
    let asked = match asked_profile(args.profile.as_deref()) {
        Ok(asked) => asked,
        Err(outcome) => return outcome,
    };
    let format = match output_format(args.format, &args.output) {
        Ok(format) => format,
        Err(outcome) => return outcome,
    };
    let picture = trace::is_picture(&args.design);
    if !picture && args.tracing.given() {
        return Outcome::usage("--vectorcraft-cli, --size, --colours and --traced are for a picture, a PNG or JPEG file");
    }
    if let Err(why) = subject_options(picture, &args.subject) {
        return Outcome::usage(why);
    }
    let writes = [Some(&args.output), args.preview.as_ref(), args.report.as_ref(), args.tracing.traced.as_ref(), args.subject.cutout.as_ref()];
    let writes: Vec<&Path> = writes.into_iter().flatten().map(|p| p.as_path()).collect();
    if let Err(outcome) = distinct(&[&args.design], &writes) {
        return outcome;
    }
    if let Err(outcome) = bundle_free(&args.output, &args.design, writes.get(1..).unwrap_or_default()) {
        return outcome;
    }
    let bytes = match files::read_capped(&args.design) {
        Ok(bytes) => bytes,
        Err(e) => {
            return Outcome { stdout: String::new(), stderr: format!("stitch: cannot read {}: {e}\n", args.design.display()), status: Status::Io };
        }
    };
    let reading = Reading::from(&args.reading);
    // With `--subject`, the picture's subject is cut out, and VectorCraft traces the cut-out.
    let tracing_for = asked.unwrap_or(profiles::REFERENCE);
    let cut = if args.subject.subject {
        match subject::cut(&bytes, &args.subject.marks.marks(), models) {
            Ok(cut) => Some(cut),
            Err(Stopped::Usage(why)) => return Outcome::usage(why),
            Err(Stopped::Said(said)) => return refuse(args, tracing_for, None, &[said]),
        }
    } else {
        None
    };
    // A picture is the document VectorCraft traces it into, every shape of it digitized automatically.
    let (source, design, reading) = if picture {
        let placed = cut.as_ref().map_or(bytes.as_slice(), |cut| cut.png.as_slice());
        match traced(args, tracing_for, placed, cut.as_ref()) {
            Ok(document) => (Source::VectorCraft, document, Reading { auto: true, ..reading }),
            Err(outcome) => return outcome,
        }
    } else {
        (Source::of(&args.design), bytes, reading)
    };
    let profile = machine_for(asked, source, &design);
    let subject = cut.as_ref().map(|cut| &cut.record);
    let run =
        Run { design_path: &args.design, source, reading, name: design_name(&args.design), design: &design, profile, format, says: None, subject };
    // A panic is a bug in StitchCraft: with the design's bytes in hand, and where it came, a bundle can
    // reproduce it (`REQ-CLI-002`).
    let phase = Cell::new(Phase::Plan);
    catch_unwind(AssertUnwindSafe(|| planned(args, &run, &phase, cut.as_ref()))).unwrap_or_else(|payload| {
        let message = bug_report::panic_message(payload.as_ref());
        let stopped =
            Outcome { stdout: String::new(), stderr: format!("stitch: StitchCraft stopped on a bug in itself: {message}\n"), status: Status::Bug };
        bug_report::beside(&args.output, &run, Err((&message, phase.get())), stopped)
    })
}

/// The document VectorCraft's command line traces the picture `bytes` into, for `profile`, as `args` ask
/// ([`trace`]); or what `stitch plan` ends with when it doesn't: `SC-E0815` or `SC-E0816` and the report,
/// or the folder VectorCraft saves into that can't be made beside the machine file. `cut` is the subject the
/// picture was cut out to, if it was.
fn traced(args: &PlanArgs, profile: &MachineProfile, bytes: &[u8], cut: Option<&Subject>) -> Result<Vec<u8>, Outcome> {
    let tracing = &args.tracing;
    let name = args.design.file_name().map_or_else(String::new, |name| name.to_string_lossy().into_owned());
    let steps = trace::steps(&name, bytes, profile, tracing.size, tracing.colours.unwrap_or(trace::COLOURS));
    let scratch = Scratch::beside(&args.output)
        .map_err(|error| Outcome::unwritten("", &Unwritten { path: args.output.clone(), error, written: Vec::new() }))?;
    trace::traced(tracing.vectorcraft_cli.as_deref(), &name, &steps, &scratch).map_err(|untraced| refuse(args, profile, cut, &[untraced]))
}

/// `stitch plan` once the design is read: planned and, unless StitchCraft found a bug in itself, written.
/// `phase` follows where it is, for a bundle, and `cut` is the subject a picture was cut out to.
fn planned(args: &PlanArgs, run: &Run, phase: &Cell<Phase>, cut: Option<&Subject>) -> Outcome {
    let sewn = sew(run.source, run.design, run.reading, run.profile, run.format, run.name);
    if sewn.found_a_bug() {
        let refused = refuse(args, run.profile, cut, &sewn.diagnostics);
        return bug_report::beside(&args.output, run, Ok(&sewn), refused);
    }
    written(args, run, sewn, phase, cut)
}

/// What `stitch plan` writes for the design `sewn`, of `run`: the machine file, and the picture, the report,
/// the traced document and the cut-out `cut` asked for, all or none (`REQ-CLI-006`), each made, with what is
/// printed, before any is written. When anything said is an error, only the report (`REQ-CLI-003`).
fn written(args: &PlanArgs, run: &Run, sewn: Sewn, phase: &Cell<Phase>, cut: Option<&Subject>) -> Outcome {
    let (profile, format) = (run.profile, run.format);
    let Sewn { mut diagnostics, plan: Some(plan), file: Some(machine_file), back: Some(back) } = sewn else {
        return refuse(args, profile, cut, &sewn.diagnostics);
    };
    // The picture is drawn before anything is written, so that one that cannot be drawn stops the rest too.
    phase.set(Phase::Picture);
    let image = match args.preview.as_ref().map(|path| (path, picture(&back, profile.trims))) {
        Some((path, Ok(image))) => Some((path, image)),
        Some((_, Err(e))) => {
            diagnostics.push(e);
            None
        }
        None => None,
    };
    phase.set(Phase::Writing);
    // An element left out with an error leaves the rest of the design planned (`REQ-GEN-002`), but a machine
    // file without it is not the design: Ink/Stitch writes no file when an element fails, and neither does
    // this command.
    if diagnostics.iter().any(|d| d.severity() == Severity::Error) {
        return refuse(args, profile, cut, &diagnostics);
    }
    // What the cut-out kept is said first, as it came first.
    let diagnostics: Vec<Diagnostic> = cut.iter().map(|cut| cut.said.clone()).chain(diagnostics).collect();
    let made = Made { file: Some((format, &machine_file)), planned: Some((&plan, &back)), subject: run.subject };
    let report = args.report.as_ref().map(|path| (path, report(args, profile, &made, &diagnostics)));
    let mut stdout = summary(args, profile, format, (&plan, &back), &machine_file);
    if let Some((path, image)) = &image {
        let _ = writeln!(stdout, "  preview   {} ({} × {} pixels)", path.display(), image.width, image.height);
    }
    if let Some((path, _)) = &report {
        let _ = writeln!(stdout, "  report    {}", path.display());
    }
    // Only a picture takes `--traced`, and its design is the document VectorCraft traced it into.
    let traced = args.tracing.traced.as_ref().map(|path| (path.as_path(), run.design));
    if let Some((path, _)) = traced {
        let _ = writeln!(stdout, "  traced    {}", path.display());
    }
    let cutout = args.subject.cutout.as_ref().zip(cut).map(|(path, cut)| (path.as_path(), cut.png.as_slice()));
    if let (Some((path, _)), Some(cut)) = (cutout, cut) {
        let [width, height] = cut.cropped;
        let _ = writeln!(stdout, "  cutout    {} ({width} × {height} pixels)", path.display());
    }
    let mut files: Vec<(&Path, &[u8])> = vec![(&args.output, &machine_file)];
    files.extend(image.as_ref().map(|(path, image)| (path.as_path(), image.png.as_slice())));
    files.extend(report.as_ref().map(|(path, text)| (path.as_path(), text.as_bytes())));
    files.extend(traced);
    files.extend(cutout);
    let said = render_diagnostics(&diagnostics);
    if let Err(unwritten) = write_files(&files, &said) {
        return unwritten;
    }
    Outcome { stdout, stderr: said, status: Status::Done }
}

/// Nothing is written but the report, which says why: what the cut-out `cut` kept, if a picture was cut out,
/// and `diagnostics`.
fn refuse(args: &PlanArgs, profile: &MachineProfile, cut: Option<&Subject>, diagnostics: &[Diagnostic]) -> Outcome {
    let diagnostics: Vec<Diagnostic> = cut.iter().map(|cut| cut.said.clone()).chain(diagnostics.iter().cloned()).collect();
    let made = Made { file: None, planned: None, subject: cut.map(|cut| &cut.record) };
    if let Some(path) = &args.report
        && let Err(unwritten) = write_files(&[(path, report(args, profile, &made, &diagnostics).as_bytes())], &render_diagnostics(&diagnostics))
    {
        return unwritten;
    }
    Outcome::refuse(String::new(), &diagnostics)
}

/// What a report describes besides what was said: the machine file written, the plan and the file read back,
/// and the record of a picture's cut-out.
#[derive(Clone, Copy)]
struct Made<'a> {
    file: Option<(FormatId, &'a [u8])>,
    planned: Option<(&'a StitchPlan, &'a StitchPlan)>,
    subject: Option<&'a subject::Record>,
}

/// What is written and what the machine will ask for: the file's size and counts, read `back`, and the
/// threads of the `plan`.
fn summary(args: &PlanArgs, profile: &MachineProfile, format: FormatId, (plan, back): (&StitchPlan, &StitchPlan), machine_file: &[u8]) -> String {
    let mut out = String::new();
    let _ = writeln!(out, "{}", args.design.display());
    describe_file(&mut out, &args.output, format, machine_file);
    let _ = writeln!(out, "  profile   {} ({})", profile.id, profile.name);
    describe_plan(&mut out, back, Some(profile));
    describe_threads(&mut out, plan, format.palette().map(|id| Entries::Written(id.palette())));
    out
}

/// The report: the design, the machine file if one was written, its counts and size, read back, the plan's
/// threads, the record of a picture's cut-out, and every diagnostic with its element.
fn report(args: &PlanArgs, profile: &MachineProfile, made: &Made, diagnostics: &[Diagnostic]) -> String {
    let Made { file, planned, subject } = *made;
    let mut report = json!({
        "design": path(&args.design),
        "profile": profile.id,
        "diagnostics": diagnostics.iter().map(|d| json!({
            "code": d.code.to_string(),
            "severity": d.severity().label(),
            "element": d.element.as_ref().map(ToString::to_string),
            "message": d.message,
        })).collect::<Vec<Value>>(),
    });
    if let Some((format, bytes)) = file {
        report["file"] = json!({ "path": path(&args.output), "format": format.name(), "bytes": bytes.len(), "sha256": hex(&Sha256::digest(bytes)) });
    }
    if let Some((plan, back)) = planned {
        let stats = back.stats();
        report["stitches"] = json!(stats.stitches);
        report["jumps"] = json!(stats.jumps);
        report["trims"] = json!(stats.trims);
        report["stops"] = json!(stats.stops);
        report["color_changes"] = json!(stats.color_changes);
        if let Some(bounds) = back.bounds() {
            report["size_mm"] = json!([round(bounds.width()), round(bounds.height())]);
        }
        report["threads"] =
            plan.color_entries().iter().map(|e| json!({ "color": e.thread.color.to_string(), "name": e.thread.name, "stop": e.stop })).collect();
    }
    if let Some(subject) = subject {
        report["subject"] = serde_json::to_value(subject).unwrap_or_default();
    }
    let mut text = serde_json::to_string_pretty(&report).unwrap_or_default();
    text.push('\n');
    text
}

/// A path as the report shows it.
fn path(path: &Path) -> String {
    path.display().to_string()
}

/// `mm` to a tenth of a millimetre, as machine files keep it.
fn round(mm: f64) -> f64 {
    (mm * 10.0).round() / 10.0
}

#[cfg(test)]
mod tests;
