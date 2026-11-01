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
//! kept in a bug-report bundle in the picture's place.
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
    Outcome, Status, asked_profile, describe_file, describe_plan, describe_threads, distinct, hex, output_format, render_diagnostics, write_files,
};
use crate::cli::PlanArgs;
use crate::files::{self, Scratch, Unwritten};
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

/// Runs `stitch plan`.
pub fn run(args: &PlanArgs) -> Outcome {
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
    let writes = [Some(&args.output), args.preview.as_ref(), args.report.as_ref(), args.tracing.traced.as_ref()];
    let writes: Vec<&Path> = writes.into_iter().flatten().map(|p| p.as_path()).collect();
    if let Err(outcome) = distinct(&[&args.design], &writes) {
        return outcome;
    }
    let bytes = match files::read_capped(&args.design) {
        Ok(bytes) => bytes,
        Err(e) => {
            return Outcome { stdout: String::new(), stderr: format!("stitch: cannot read {}: {e}\n", args.design.display()), status: Status::Io };
        }
    };
    let reading = Reading::from(&args.reading);
    // A picture is the document VectorCraft traces it into, every shape of it digitized automatically.
    let (source, design, reading) = if picture {
        match traced(args, asked.unwrap_or(profiles::REFERENCE), &bytes) {
            Ok(document) => (Source::VectorCraft, document, Reading { auto: true, ..reading }),
            Err(outcome) => return outcome,
        }
    } else {
        (Source::of(&args.design), bytes, reading)
    };
    let profile = machine_for(asked, source, &design);
    let run = Run { design_path: &args.design, source, reading, name: design_name(&args.design), design: &design, profile, format, says: None };
    // A panic is a bug in StitchCraft: with the design's bytes in hand, and where it came, a bundle can
    // reproduce it (`REQ-CLI-002`).
    let phase = Cell::new(Phase::Plan);
    catch_unwind(AssertUnwindSafe(|| planned(args, &run, &phase))).unwrap_or_else(|payload| {
        let message = bug_report::panic_message(payload.as_ref());
        let stopped =
            Outcome { stdout: String::new(), stderr: format!("stitch: StitchCraft stopped on a bug in itself: {message}\n"), status: Status::Bug };
        bug_report::beside(&args.output, &run, Err((&message, phase.get())), stopped)
    })
}

/// The document VectorCraft's command line traces the picture `bytes` into, for `profile`, as `args` ask
/// ([`trace`]); or what `stitch plan` ends with when it doesn't: `SC-E0815` or `SC-E0816` and the report,
/// or the folder VectorCraft saves into that can't be made beside the machine file.
fn traced(args: &PlanArgs, profile: &MachineProfile, bytes: &[u8]) -> Result<Vec<u8>, Outcome> {
    let tracing = &args.tracing;
    let name = args.design.file_name().map_or_else(String::new, |name| name.to_string_lossy().into_owned());
    let steps = trace::steps(&name, bytes, profile, tracing.size, tracing.colours.unwrap_or(trace::COLOURS));
    let scratch = Scratch::beside(&args.output)
        .map_err(|error| Outcome::unwritten("", &Unwritten { path: args.output.clone(), error, written: Vec::new() }))?;
    trace::traced(tracing.vectorcraft_cli.as_deref(), &name, &steps, &scratch).map_err(|untraced| refuse(args, profile, &[untraced]))
}

/// `stitch plan` once the design is read: planned and, unless StitchCraft found a bug in itself, written.
/// `phase` follows where it is, for a bundle.
fn planned(args: &PlanArgs, run: &Run, phase: &Cell<Phase>) -> Outcome {
    let sewn = sew(run.source, run.design, run.reading, run.profile, run.format, run.name);
    if sewn.found_a_bug() {
        let refused = refuse(args, run.profile, &sewn.diagnostics);
        return bug_report::beside(&args.output, run, Ok(&sewn), refused);
    }
    written(args, run, sewn, phase)
}

/// What `stitch plan` writes for the design `sewn`, of `run`: the machine file, and the picture, the report
/// and the traced document asked for, all or none (`REQ-CLI-006`), each made, with what is printed, before
/// any is written. When anything said is an error, only the report (`REQ-CLI-003`).
fn written(args: &PlanArgs, run: &Run, sewn: Sewn, phase: &Cell<Phase>) -> Outcome {
    let (profile, format) = (run.profile, run.format);
    let Sewn { mut diagnostics, plan: Some(plan), file: Some(machine_file), back: Some(back) } = sewn else {
        return refuse(args, profile, &sewn.diagnostics);
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
        return refuse(args, profile, &diagnostics);
    }
    let report = args.report.as_ref().map(|path| (path, report(args, profile, Some((format, &machine_file)), Some((&plan, &back)), &diagnostics)));
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
    let mut files: Vec<(&Path, &[u8])> = vec![(&args.output, &machine_file)];
    files.extend(image.as_ref().map(|(path, image)| (path.as_path(), image.png.as_slice())));
    files.extend(report.as_ref().map(|(path, text)| (path.as_path(), text.as_bytes())));
    files.extend(traced);
    let said = render_diagnostics(&diagnostics);
    if let Err(unwritten) = write_files(&files, &said) {
        return unwritten;
    }
    Outcome { stdout, stderr: said, status: Status::Done }
}

/// Nothing is written but the report, which says why.
fn refuse(args: &PlanArgs, profile: &MachineProfile, diagnostics: &[Diagnostic]) -> Outcome {
    if let Some(path) = &args.report
        && let Err(unwritten) = write_files(&[(path, report(args, profile, None, None, diagnostics).as_bytes())], &render_diagnostics(diagnostics))
    {
        return unwritten;
    }
    Outcome::refuse(String::new(), diagnostics)
}

/// What is written and what the machine will ask for: the file's size and counts, read `back`, and the
/// threads of the `plan`.
fn summary(args: &PlanArgs, profile: &MachineProfile, format: FormatId, (plan, back): (&StitchPlan, &StitchPlan), machine_file: &[u8]) -> String {
    let mut out = String::new();
    let _ = writeln!(out, "{}", args.design.display());
    describe_file(&mut out, &args.output, format, machine_file);
    let _ = writeln!(out, "  profile   {} ({})", profile.id, profile.name);
    describe_plan(&mut out, back, Some(profile));
    describe_threads(&mut out, plan, format.palette().map(|id| id.palette()));
    out
}

/// The report: the design, the machine file if one was written, its counts and size, read back, the plan's
/// threads, and every diagnostic with its element.
fn report(
    args: &PlanArgs,
    profile: &MachineProfile,
    file: Option<(FormatId, &[u8])>,
    planned: Option<(&StitchPlan, &StitchPlan)>,
    diagnostics: &[Diagnostic],
) -> String {
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
mod tests {
    use std::path::PathBuf;

    use stitchcraft_core::ElementId;

    use stitchcraft_testkit::temp::TempDir;

    use stitchcraft_core::Mm;
    use stitchcraft_engine::design::DesignSettings;
    use stitchcraft_plan::profiles::{BROTHER_PE800_SMALL, REFERENCE};
    use stitchcraft_vectorcraft::plugin_id;
    use stitchcraft_vectorcraft::settings::{SETTINGS_FILTER, kept_data};

    use super::*;
    use crate::cli::{Format, ReadingArgs, TracingArgs};

    fn fixture(name: &str) -> PathBuf {
        PathBuf::from(format!("{}/../../conformance/fixtures/svg/{name}", env!("CARGO_MANIFEST_DIR")))
    }

    fn vectorcraft(name: &str) -> PathBuf {
        PathBuf::from(format!("{}/../../conformance/fixtures/vectorcraft/{name}", env!("CARGO_MANIFEST_DIR")))
    }

    fn args(design: PathBuf, output: PathBuf) -> PlanArgs {
        PlanArgs {
            design,
            output,
            profile: None,
            format: None,
            preview: None,
            report: None,
            reading: ReadingArgs::default(),
            tracing: TracingArgs::default(),
        }
    }

    #[test]
    fn writes_the_machine_file_a_picture_and_a_report() {
        let dir = TempDir::new("plan");
        let (output, preview, report) = (dir.join("strokes.pes"), dir.join("strokes.png"), dir.join("strokes.json"));
        let out = run(&PlanArgs { preview: Some(preview.clone()), report: Some(report.clone()), ..args(fixture("strokes.svg"), output.clone()) });
        assert_eq!(out.status, Status::Done, "{}", out.stderr);
        assert!(std::fs::read(&output).unwrap().starts_with(b"#PES0001"));
        assert!(std::fs::read(&preview).unwrap().starts_with(b"\x89PNG"));
        // The file's counts: a PES file flags 5 of the plan's 6 jumps as trims, which the machine cuts.
        assert!(out.stdout.contains("  stitches  556 stitches, 6 jumps, 5 trims, 3 colour changes, 0 stops\n"), "{}", out.stdout);
        assert!(out.stdout.contains("  threads   1. unnamed (#c00000), shown as Brother PEC 5 \"Red\"\n"));
        // What was said names its element, on the terminal and in the report.
        assert_eq!(
            out.stderr,
            "warning SC-W0307 (svg:patch:fill): This fill's area falls into 2 parts that are sewn one after another, with a jump between each.\n  \
             hint: To choose the order the parts are sewn in, break the fill apart into one element for each part.\n\
             info SC-I0504: A needle point less than the shortest stitch (0.3 mm) from a needle point next to it was left out.\n"
        );
        let report: Value = serde_json::from_slice(&std::fs::read(&report).unwrap()).unwrap();
        assert_eq!((report["stitches"].as_u64(), report["trims"].as_u64(), report["color_changes"].as_u64()), (Some(556), Some(5), Some(3)));
        // The file's size, its needle points on the machine's 0.1 mm grid: the plan's is 59.9 mm wide.
        assert_eq!(report["size_mm"], json!([60.0, 45.0]));
        assert_eq!(report["diagnostics"][0]["element"], "svg:patch:fill");
        assert_eq!(report["file"]["sha256"], hex(&Sha256::digest(std::fs::read(&output).unwrap())));
        assert_eq!((report["design"].as_str(), report["file"]["path"].as_str()), (fixture("strokes.svg").to_str(), output.to_str()));
    }

    /// A line 3 mm wide and 50 mm long, without settings: a satin column when digitized automatically.
    const LINE: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" width="100mm" height="100mm" viewBox="0 0 100 100"><path id="line" d="M 10 50 L 60 50" stroke="red" stroke-width="3" fill="none"/></svg>"#;

    #[test]
    fn req_auto_001_stitch_plan_auto_digitizes_the_shapes_without_settings() {
        let dir = TempDir::new("plan-auto");
        let design = dir.join("line.svg");
        std::fs::write(&design, LINE).unwrap();
        let (auto, plain) = (dir.join("auto.pes"), dir.join("plain.pes"));
        let out = run(&PlanArgs { reading: ReadingArgs { auto: true, ..ReadingArgs::default() }, ..args(design.clone(), auto.clone()) });
        assert_eq!(out.status, Status::Done, "{}", out.stderr);
        assert_eq!(out.stderr, "info SC-I0901: 1 shape is digitized automatically: it sews as a satin column.\n");
        // Without the switch, the line runs, as it did before.
        let out = run(&args(design, plain.clone()));
        assert_eq!((out.status, out.stderr.as_str()), (Status::Done, ""));
        assert_ne!(std::fs::read(auto).unwrap(), std::fs::read(plain).unwrap());
    }

    /// Red, blue and red squares 10 mm apart, without settings: tatami fills, as Ink/Stitch sews them.
    const SQUARES: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" width="100mm" height="100mm" viewBox="0 0 100 100"><rect id="r1" x="0" y="0" width="10" height="10" fill="red"/><rect id="b" x="20" y="0" width="10" height="10" fill="blue"/><rect id="r2" x="40" y="0" width="10" height="10" fill="red"/></svg>"#;

    #[test]
    fn req_auto_011_stitch_plan_sort_colours_sews_the_shapes_with_settings_by_thread() {
        let dir = TempDir::new("plan-sorted");
        let design = dir.join("squares.svg");
        std::fs::write(&design, SQUARES).unwrap();
        let (sorted, drawn) = (dir.join("sorted.json"), dir.join("drawn.json"));
        let sorting = ReadingArgs { sort_colours: true, ..ReadingArgs::default() };
        let out = run(&PlanArgs { report: Some(sorted.clone()), reading: sorting, ..args(design.clone(), dir.join("sorted.pes")) });
        assert_eq!(out.status, Status::Done, "{}", out.stderr);
        assert!(
            out.stderr.contains("info SC-I0907: The elements are sewn in an order of their own, with 1 thread change where paint order has 2.\n"),
            "{}",
            out.stderr
        );
        // Without the switch, the squares sew as drawn.
        let out = run(&PlanArgs { report: Some(drawn.clone()), ..args(design, dir.join("drawn.pes")) });
        assert_eq!(out.status, Status::Done, "{}", out.stderr);
        let changes = |report: &Path| serde_json::from_slice::<Value>(&std::fs::read(report).unwrap()).unwrap()["color_changes"].as_u64();
        assert_eq!((changes(&sorted), changes(&drawn)), (Some(1), Some(2)));
    }

    #[test]
    fn req_vc_013_a_vectorcraft_document_is_planned_with_the_settings_and_for_the_machine_it_keeps() {
        let dir = TempDir::new("plan-settings");
        // The patch, with the small hoop and a shortest jump of 10 mm kept by the Design Settings filter.
        let patch = std::fs::read(vectorcraft("patch.vectorcraft")).unwrap();
        let mut file: Value = serde_json::from_slice(stitchcraft_vectorcraft::read_part(&patch)).unwrap();
        let settings = DesignSettings { collapse_len: Mm::new(10.0).unwrap(), ..DesignSettings::default() };
        file["document"]["unknown"] = json!({"pluginData": {plugin_id(SETTINGS_FILTER): kept_data(&settings, &BROTHER_PE800_SMALL)}});
        let kept = dir.join("kept.vectorcraft");
        std::fs::write(&kept, serde_json::to_vec(&file).unwrap()).unwrap();
        assert_eq!(machine_for(None, Source::VectorCraft, &std::fs::read(&kept).unwrap()), &BROTHER_PE800_SMALL);
        // Planned for the small hoop, which the patch doesn't fit, so nothing is written.
        let small = dir.join("small.pes");
        let out = run(&args(kept.clone(), small.clone()));
        assert_eq!(out.status, Status::DesignErrors);
        assert!(out.stderr.contains("error SC-E0701: ") && !small.exists(), "{}", out.stderr);
        // `--profile` names another, and the settings still hold: the shortest jump sews the patch otherwise.
        let on_reference = |design: PathBuf, output: PathBuf| {
            let out = run(&PlanArgs { profile: Some(REFERENCE.id.to_string()), ..args(design, output.clone()) });
            assert_eq!(out.status, Status::Done, "{}", out.stderr);
            assert!(out.stdout.contains(&format!("  profile   {} (", REFERENCE.id)), "{}", out.stdout);
            std::fs::read(output).unwrap()
        };
        assert_ne!(on_reference(kept, dir.join("named.pes")), on_reference(vectorcraft("patch.vectorcraft"), dir.join("plain.pes")));
        // An SVG file keeps no machine.
        assert_eq!(machine_for(None, Source::Svg, &std::fs::read(fixture("strokes.svg")).unwrap()), REFERENCE);
    }

    #[test]
    fn req_vc_008_a_design_is_told_by_its_extension() {
        let documents = ["patch.vectorcraft", "old/patch.drawcraft", "PATCH.VectorCraft", "start.vctemplate"];
        let svg = ["patch.svg", "vectorcraft", "patch.vectorcraft.svg", "patch.vectorcraf"];
        for (paths, source) in [(&documents[..], Source::VectorCraft), (&svg[..], Source::Svg)] {
            for path in paths {
                assert_eq!(Source::of(Path::new(path)), source, "{path}");
            }
        }
    }

    #[test]
    fn req_vc_008_stitch_plan_reads_a_vectorcraft_document() {
        let dir = TempDir::new("plan");
        let (output, preview, report) = (dir.join("patch.pes"), dir.join("patch.png"), dir.join("patch.json"));
        let out =
            run(&PlanArgs { preview: Some(preview.clone()), report: Some(report.clone()), ..args(vectorcraft("patch.vectorcraft"), output.clone()) });
        assert_eq!(out.status, Status::Done, "{}", out.stderr);
        assert!(std::fs::read(&output).unwrap().starts_with(b"#PES0001"));
        assert!(std::fs::read(&preview).unwrap().starts_with(b"\x89PNG"));
        // What the reader said comes first: the placement line left out by its name, and the start marker,
        // which is not applied yet.
        assert!(out.stderr.starts_with("info SC-I0811 (vc:10): ") && out.stderr.contains("\nwarning SC-W0807 (vc:4): "), "{}", out.stderr);
        let report: Value = serde_json::from_slice(&std::fs::read(&report).unwrap()).unwrap();
        let codes: Vec<&str> = report["diagnostics"].as_array().unwrap().iter().filter_map(|d| d["code"].as_str()).collect();
        assert!(codes.starts_with(&["SC-I0811", "SC-W0807"]), "{codes:?}");
        // The background's red, the border's blue, the stem's green and the dots' gold.
        assert!(out.stdout.contains(", 3 colour changes, "), "{}", out.stdout);
        // The same document under VectorCraft's former extension, which names the design the same.
        let old = dir.join("patch.drawcraft");
        std::fs::copy(vectorcraft("patch.vectorcraft"), &old).unwrap();
        let again = dir.join("patch-again.pes");
        assert_eq!(run(&args(old, again.clone())).status, Status::Done);
        assert_eq!(std::fs::read(&again).unwrap(), std::fs::read(&output).unwrap());
        // A file that isn't a VectorCraft document is refused as one.
        let not_vectorcraft = dir.join("not.vectorcraft");
        std::fs::write(&not_vectorcraft, b"<svg/>").unwrap();
        let refused = run(&args(not_vectorcraft, dir.join("not.pes")));
        assert_eq!(refused.status, Status::DesignErrors);
        assert!(refused.stderr.starts_with("error SC-E0806: "), "{}", refused.stderr);
    }

    #[test]
    fn req_cli_013_the_tracing_options_are_for_a_picture_only() {
        let dir = TempDir::new("plan-tracing-options");
        let given = [
            TracingArgs { vectorcraft_cli: Some(PathBuf::from("vectorcraft-cli")), ..TracingArgs::default() },
            TracingArgs { size: Some(Mm::new(80.0).unwrap()), ..TracingArgs::default() },
            TracingArgs { colours: Some(4), ..TracingArgs::default() },
            TracingArgs { traced: Some(dir.join("traced.vectorcraft")), ..TracingArgs::default() },
        ];
        for tracing in given {
            let out = run(&PlanArgs { tracing, ..args(fixture("strokes.svg"), dir.join("strokes.pes")) });
            let usage = "stitch: --vectorcraft-cli, --size, --colours and --traced are for a picture, a PNG or JPEG file\n";
            assert_eq!((out.status, out.stderr.as_str()), (Status::Usage, usage));
        }
        assert!(!dir.join("strokes.pes").exists() && !dir.join("traced.vectorcraft").exists());
    }

    #[test]
    fn req_cli_013_without_vectorcraft_s_command_line_nothing_is_written_but_the_report() {
        let dir = TempDir::new("plan-no-vectorcraft");
        let picture = dir.join("badge.png");
        std::fs::write(&picture, b"\x89PNG\r\n\x1a\n").unwrap();
        let (output, report, traced) = (dir.join("badge.pes"), dir.join("badge.json"), dir.join("traced.vectorcraft"));
        let tracing = TracingArgs { vectorcraft_cli: Some(dir.join("no-vectorcraft-cli")), traced: Some(traced.clone()), ..TracingArgs::default() };
        let out = run(&PlanArgs { report: Some(report.clone()), tracing, ..args(picture.clone(), output.clone()) });
        assert_eq!(out.status, Status::DesignErrors);
        assert!(
            out.stderr.starts_with("error SC-E0815: VectorCraft's command line, ") && out.stderr.ends_with("nothing was written\n"),
            "{}",
            out.stderr
        );
        assert!(!output.exists() && !traced.exists());
        let said: Value = serde_json::from_slice(&std::fs::read(&report).unwrap()).unwrap();
        assert_eq!((said["diagnostics"][0]["code"].as_str(), said.get("file")), (Some("SC-E0815"), None));
        // The folder VectorCraft would have saved into is gone too.
        let mut left: Vec<String> = std::fs::read_dir(dir.path()).unwrap().map(|entry| entry.unwrap().file_name().into_string().unwrap()).collect();
        left.sort();
        assert_eq!(left, ["badge.json", "badge.png"]);
        // A machine file in a folder that doesn't exist can't have VectorCraft's folder beside it.
        let nowhere = dir.join("no-such-folder").join("badge.pes");
        let out = run(&PlanArgs {
            tracing: TracingArgs { vectorcraft_cli: Some(dir.join("no-vectorcraft-cli")), ..TracingArgs::default() },
            ..args(picture, nowhere.clone())
        });
        assert_eq!(out.status, Status::Io);
        assert!(
            out.stderr.starts_with(&format!("stitch: cannot write {}: ", nowhere.display())) && out.stderr.ends_with("nothing was written\n"),
            "{}",
            out.stderr
        );
    }

    #[test]
    fn req_cli_008_the_picture_is_of_the_file_as_written() {
        let dir = TempDir::new("plan");
        // The picture is of the file read back, as `stitch preview` of the file draws it, for the machine and
        // in the design's colours. A DST file cuts the thread before the 47 mm jump, where the plan leaves a
        // jump thread: a machine that obeys the trim command cuts there, and the reference PE800 cuts no
        // jump (MC-1).
        let (output, preview) = (dir.join("picture.dst"), dir.join("picture.png"));
        let out = run(&PlanArgs { preview: Some(preview.clone()), ..args(fixture("strokes.svg"), output) });
        assert_eq!(out.status, Status::Done, "{}", out.stderr);
        let sewn =
            sew(Source::Svg, &std::fs::read(fixture("strokes.svg")).unwrap(), Reading::default(), profiles::REFERENCE, FormatId::Dst, "picture");
        let drawn = std::fs::read(&preview).unwrap();
        let (back, plan) = (sewn.back.as_ref().unwrap(), sewn.plan.as_ref().unwrap());
        assert_eq!(drawn, picture(back, profiles::REFERENCE.trims).unwrap().png);
        assert_eq!(drawn, picture(plan, profiles::REFERENCE.trims).unwrap().png, "the jump thread the PE800 leaves");
        let cutting = |plan| picture(plan, TrimSupport::Command).unwrap().png;
        assert_ne!(cutting(back), cutting(plan), "the plan's jump thread, where the file cuts");
        let back = sewn.back.unwrap();
        assert_eq!(
            back.blocks.iter().map(|b| &b.thread).collect::<Vec<_>>(),
            sewn.plan.unwrap().blocks.iter().map(|b| &b.thread).collect::<Vec<_>>()
        );
    }

    #[test]
    fn a_design_with_errors_writes_only_its_report() {
        let dir = TempDir::new("plan");
        let (output, report) = (dir.join("unreadable.pes"), dir.join("unreadable.json"));
        let not_svg = dir.join("not.svg");
        std::fs::write(&not_svg, b"stitches, please").unwrap();
        let out = run(&PlanArgs { report: Some(report.clone()), ..args(not_svg, output.clone()) });
        assert_eq!(out.status, Status::DesignErrors);
        assert!(out.stderr.starts_with("error SC-E0801: "), "{}", out.stderr);
        assert!(out.stderr.ends_with("nothing was written\n"));
        assert!(!output.exists());
        let report: Value = serde_json::from_slice(&std::fs::read(&report).unwrap()).unwrap();
        assert_eq!((report["diagnostics"][0]["code"].as_str(), report.get("file")), (Some("SC-E0801"), None));
        // A design with nothing to sew: a shape neither filled nor stroked.
        let unpainted = dir.join("unpainted.svg");
        std::fs::write(
            &unpainted,
            br#"<svg xmlns="http://www.w3.org/2000/svg" width="20mm" height="20mm" viewBox="0 0 20 20"><rect width="10" height="10" fill="none"/></svg>"#,
        )
        .unwrap();
        let out = run(&args(unpainted, dir.join("unpainted.pes")));
        assert_eq!(out.status, Status::DesignErrors);
        assert!(out.stderr.contains("error SC-E0010: The design has nothing to stitch"), "{}", out.stderr);
    }

    #[test]
    fn req_cli_003_an_element_left_out_with_an_error_writes_only_the_report() {
        let dir = TempDir::new("plan");
        // The engine leaves out an element whose planning runs past its work budget, with `SC-E0004`, and plans
        // the rest (`REQ-GEN-002`), as it does for a fill of 2,000 spikes after 12 s.
        let (output, preview, report) = (dir.join("left-out.pes"), dir.join("left-out.png"), dir.join("left-out.json"));
        let design = std::fs::read(fixture("strokes.svg")).unwrap();
        let mut sewn = sew(Source::Svg, &design, Reading::default(), profiles::REFERENCE, FormatId::PesV1, "strokes");
        let fill = ElementId::new("svg:patch:fill").unwrap();
        sewn.diagnostics.push(Budget::DEFAULT.meter().charge(u64::MAX).unwrap_err().diagnostic(&Budget::DEFAULT, Some(fill)));
        assert!(sewn.plan.is_some() && sewn.file.is_some());
        let asked = PlanArgs { preview: Some(preview.clone()), report: Some(report.clone()), ..args(fixture("strokes.svg"), output.clone()) };
        let (path, reading, profile, format) = (fixture("strokes.svg"), Reading::default(), profiles::REFERENCE, FormatId::PesV1);
        let run = Run { design_path: &path, source: Source::Svg, reading, name: "strokes", design: &design, profile, format, says: None };
        let out = written(&asked, &run, sewn, &Cell::new(Phase::Plan));
        assert_eq!(out.status, Status::DesignErrors);
        assert!(out.stderr.contains("\nerror SC-E0004 (svg:patch:fill): ") && out.stderr.ends_with("nothing was written\n"), "{}", out.stderr);
        assert!(!output.exists() && !preview.exists());
        let report: Value = serde_json::from_slice(&std::fs::read(&report).unwrap()).unwrap();
        let said: Vec<&str> = report["diagnostics"].as_array().unwrap().iter().filter_map(|d| d["code"].as_str()).collect();
        assert_eq!((said.last(), report.get("file")), (Some(&"SC-E0004"), None));
    }

    #[test]
    fn formats_profiles_and_files_are_checked_first() {
        let dir = TempDir::new("plan");
        let dst = dir.join("strokes-as-dst.bin");
        let out = run(&PlanArgs { format: Some(Format::Dst), ..args(fixture("strokes.svg"), dst.clone()) });
        assert_eq!(out.status, Status::Done, "{}", out.stderr);
        assert!(std::fs::read(&dst).unwrap().starts_with(b"LA:strokes"));
        // What the format adds is said with the rest: DST machines cut before the 47 mm jump.
        assert!(out.stderr.ends_with("info SC-I0605: The thread will be cut at 1 place the plan does not trim: DST machines cut it before 3 or more jump records in a row, and a jump longer than 24.2 mm takes that many.\n"), "{}", out.stderr);
        assert_eq!(run(&args(fixture("strokes.svg"), dir.join("x.unknown"))).status, Status::Usage);
        assert_eq!(run(&PlanArgs { profile: Some("singer".to_string()), ..args(fixture("strokes.svg"), dir.join("x.pes")) }).status, Status::Usage);
        let missing = run(&args(fixture("missing.svg"), dir.join("x.pes")));
        assert_eq!(missing.status, Status::Io);
        assert!(missing.stderr.starts_with("stitch: cannot read "));
    }

    #[test]
    fn req_plan_006_a_file_is_held_to_its_own_formats_colour_changes() {
        // 257 lines 0.5 mm apart, red and blue in turn: 256 thread changes, for the PE800, whose PES records
        // at most 255.
        let lines: String = (0..257)
            .map(|i| format!(r#"<path d="M 0 {} H 10" stroke="{}" fill="none"/>"#, f64::from(i) * 0.5, if i % 2 == 0 { "red" } else { "blue" }))
            .collect();
        let svg = format!(r#"<svg xmlns="http://www.w3.org/2000/svg" width="20mm" height="140mm" viewBox="0 0 20 140">{lines}</svg>"#);
        let sewn = |format| sew(Source::Svg, svg.as_bytes(), Reading::default(), REFERENCE, format, "many");
        let errors = |sewn: &Sewn| sewn.diagnostics.iter().filter(|d| d.severity() == Severity::Error).map(ToString::to_string).collect::<Vec<_>>();
        let pes = sewn(FormatId::PesV1);
        assert_eq!(errors(&pes), ["error SC-E0601: The design has 256 colour changes and stops, but PES v1 records at most 255."]);
        assert!(pes.file.is_none());
        // DST records 999: the file is written.
        let dst = sewn(FormatId::Dst);
        assert_eq!(errors(&dst), Vec::<String>::new());
        assert!(dst.file.is_some_and(|bytes| bytes.starts_with(b"LA:many")));
    }

    #[test]
    fn req_cli_006_when_a_file_cannot_be_written_none_is_and_what_was_said_is_printed() {
        let dir = TempDir::new("plan");
        // A folder that does not exist: the machine file, the picture, the report, and the report of a
        // design that was refused. The other files are not written either, and what was said about the
        // design is printed before the file that could not be written.
        let nowhere = dir.join("no-such-folder").join("x");
        let output = dir.join("unwritten.pes");
        let _ = std::fs::remove_file(&output);
        let not_svg = dir.join("not-an-svg.svg");
        std::fs::write(&not_svg, b"stitches, please").unwrap();
        let cases = [
            (args(fixture("strokes.svg"), nowhere.join("strokes.pes")), nowhere.join("strokes.pes"), "SC-I0504"),
            (
                PlanArgs { preview: Some(nowhere.join("strokes.png")), ..args(fixture("strokes.svg"), output.clone()) },
                nowhere.join("strokes.png"),
                "SC-I0504",
            ),
            (
                PlanArgs { report: Some(nowhere.join("strokes.json")), ..args(fixture("strokes.svg"), output.clone()) },
                nowhere.join("strokes.json"),
                "SC-I0504",
            ),
            (PlanArgs { report: Some(nowhere.join("refused.json")), ..args(not_svg, output.clone()) }, nowhere.join("refused.json"), "SC-E0801"),
        ];
        for (asked, unwritten, said) in cases {
            let out = run(&asked);
            assert_eq!(out.status, Status::Io, "{}", out.stderr);
            assert!(out.stderr.contains(said) && out.stderr.contains(&format!("stitch: cannot write {}: ", unwritten.display())), "{}", out.stderr);
            assert!(out.stderr.ends_with("nothing was written\n") && out.stdout.is_empty(), "{}", out.stderr);
            assert!(!output.exists(), "{}", unwritten.display());
        }
    }
}
