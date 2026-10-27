//! `stitch bug-report DESIGN`: one file that reproduces what StitchCraft does with a design, to attach to
//! an issue; and `stitch bug-report --replay BUNDLE`, which plans it again and compares.
//!
//! StitchCraft is deterministic (`docs/src/design/determinism.md`): the same design, profile, format,
//! version and reading (`--auto`, `--sort-colours`) give the same plan, bit for bit, on every platform. So a
//! bundle holds exactly those — the design file itself, an SVG file or a VectorCraft document under a field
//! named for its kind ([`Source`]), the profile's id, the format, the version, whether the run digitized
//! shapes automatically and whether it ordered the shapes with settings too — and what came of them: a
//! digest of every entry of the plan,
//! the machine file's size and digest, every diagnostic, and a panic's message if there was one. Replaying
//! plans the design again through the same code as `stitch plan` ([`sew`]) and compares each of them
//! (`REQ-CLI-001`).
//!
//! Of a VectorCraft document, a bundle keeps the part its reader reads: its JSON, without the images and
//! other data VectorCraft keeps after it. That part reads as the whole file does, keeps a bundle small
//! enough to attach to an issue, and leaves the pictures a design was traced from out of it.
//!
//! `stitch plan` writes a bundle by itself when it finds a bug in StitchCraft — a failed plan check
//! (`SC-E0009`) or a panic, which it catches with the design in hand, and `main` with [`guarded`] before
//! then — next to the machine file it was asked for, and ends with exit status 4 (`REQ-CLI-002`). A
//! panic's bundle says where it came ([`Phase`]): while planning, which a replay reruns, while drawing the
//! picture, which a replay draws too, or while writing, which it does not. A bundle is JSON, so a
//! maintainer can read the design out of it and the person reporting can see what it holds: their whole
//! design, which is why `stitch` says so whenever it writes one.
//!
//! Every build of the workspace says the version it will be released as, so a bundle also records the
//! commit it was built from, when it was built from a git checkout (`build.rs`). A replay reads the
//! bundle's format first, so a bundle of a format it does not know says so whatever the rest holds.
//!
//! A bundle records the design name the run gave the machine file, and the replay uses it as it is: rebuilt
//! from the file name, it would come out otherwise where that name is not UTF-8, or on another platform
//! (`a\b.svg` names the design `a\b` on Linux and `b` on Windows). And anyone can attach a bundle to an
//! issue, so the replay prints what it takes from one with each character a terminal would act on, or not
//! show as itself, written as an escape such as `\u{1b}` (`REQ-CLI-004`).

use std::any::Any;
use std::fmt::Write as _;
use std::panic::{UnwindSafe, catch_unwind};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use stitchcraft_core::{Code, Diagnostic};
use stitchcraft_engine::design::Reading;
use stitchcraft_plan::{FormatId, MachineProfile, StitchPlan, profiles};

use super::plan::{Sewn, Source, design_name, machine_for, picture, sew};
use super::{Outcome, Status, asked_profile, counts, distinct, hex, output_format, render_diagnostics, unhex};
use crate::cli::{BugReportArgs, Command};
use crate::files;

/// The bundle format this StitchCraft writes and replays.
const BUNDLE_FORMAT: u32 = 1;

/// The largest bundle a replay reads: a bundle can hold a design of [`files::MAX_READ`] bytes, twice as
/// many in hexadecimal or in text with its characters escaped, and what was said of it (`REQ-CLI-005`).
const BUNDLE_MAX: u64 = 3 * files::MAX_READ;
/// The commit this StitchCraft was built from, or empty when it was not built from a git checkout.
pub const COMMIT: &str = env!("STITCHCRAFT_COMMIT");
/// Where bugs are reported.
pub const ISSUES: &str = "https://github.com/KaboomFox/stitchcraft/issues";
/// What every bundle written is followed by.
const HOLDS_THE_DESIGN: &str = "It holds your design, so share it only where you are happy for the design to be seen.";

/// One run of `stitch plan`, as a bundle records it.
pub struct Run<'a> {
    /// Where the design was read from; its file name goes into the bundle, for people.
    pub design_path: &'a Path,
    /// What the design file is, which decides how it is read.
    pub source: Source,
    /// How it is read: with `--auto` and `--sort-colours`, or not.
    pub reading: Reading,
    /// The design name machines show, which the machine file's label holds ([`design_name`]).
    pub name: &'a str,
    /// The design file's bytes.
    pub design: &'a [u8],
    /// The machine profile it was planned for.
    pub profile: &'a MachineProfile,
    /// The format it was written in.
    pub format: FormatId,
    /// What the person reporting says goes wrong.
    pub says: Option<String>,
}

/// Where in `stitch plan` a panic came.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Phase {
    /// Planning the design and writing the machine file in memory ([`sew`]), which a replay reruns.
    Plan,
    /// Drawing the picture ([`picture`]), which a replay draws too.
    Picture,
    /// Making the report and the summary, and writing the files, which a replay does not do.
    Writing,
}

/// The first field of a bundle of any format.
#[derive(Deserialize)]
struct Head {
    stitchcraft_bug_report: u32,
}

/// A bug-report bundle (format 1).
#[derive(Debug, Serialize, Deserialize)]
struct Bundle {
    /// The bundle format: [`BUNDLE_FORMAT`].
    stitchcraft_bug_report: u32,
    /// The version of StitchCraft that ran.
    version: String,
    /// The commit it was built from, when it was built from a git checkout.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    commit: Option<String>,
    /// The operating system and processor it ran on.
    platform: String,
    /// What the person reporting says goes wrong.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    says: Option<String>,
    /// The command that ran: `plan`.
    command: String,
    /// The machine profile's id.
    profile: String,
    /// The format's file extension: `pes` or `dst`.
    format: String,
    /// Whether the run digitized the shapes without settings automatically (`--auto`). Bundles of runs
    /// without it, and those written before it was recorded, have none.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    auto: bool,
    /// Whether the run sewed the shapes with settings in the order StitchCraft chooses too
    /// (`--sort-colours`). Bundles of runs without it, and those written before it was recorded, have none.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    sort_colours: bool,
    /// The design.
    design: DesignRecord,
    /// What came of it.
    result: Record,
}

/// The design file in a bundle, the part of it its reader reads ([`Source::read_part`]), under the fields of
/// its kind: `svg` or `vectorcraft` for its text when it is UTF-8, `svg_hex` or `vectorcraft_hex` for its
/// bytes in hexadecimal otherwise, such as a compressed VectorCraft document's. A bundle written before
/// VectorCraft documents were read holds an SVG file.
#[derive(Debug, Serialize, Deserialize)]
struct DesignRecord {
    /// The file's name, for people.
    file: String,
    /// The design name machines show, as the run gave it to the machine file. Bundles written before it was
    /// recorded have none, and the replay takes the name from `file`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    name: Option<String>,
    /// The SHA-256 of its bytes when the bundle was written.
    sha256: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    svg: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    svg_hex: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    vectorcraft: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    vectorcraft_hex: Option<String>,
}

/// What came of planning a design.
#[derive(Debug, PartialEq, Serialize, Deserialize)]
struct Record {
    /// The plan, when there was one.
    plan: Option<PlanRecord>,
    /// The machine file, when there was one.
    file: Option<FileRecord>,
    /// Every diagnostic's first line, as `stitch` prints it.
    diagnostics: Vec<String>,
    /// A panic's message, when StitchCraft panicked.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    panic: Option<String>,
    /// Where the panic came; bundles written before it was recorded have none, which is while planning.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    phase: Option<Phase>,
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
struct PlanRecord {
    /// [`digest`] of the plan.
    sha256: String,
    /// Its counts, for people.
    counts: String,
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
struct FileRecord {
    /// Its size in bytes.
    bytes: usize,
    /// The SHA-256 of its bytes.
    sha256: String,
}

/// Runs `stitch bug-report`.
pub fn run(args: &BugReportArgs) -> Outcome {
    match (&args.replay, &args.design) {
        (Some(bundle), _) => replay(bundle),
        (None, Some(design)) => make(args, design),
        (None, None) => Outcome::usage("name a design, or a bundle to replay with --replay"),
    }
}

/// Runs `run` — a command — and turns a panic in it, a bug in StitchCraft, into a report with exit
/// status 4; for `stitch plan`, with a bug-report bundle next to the file it was asked for.
pub fn guarded(command: &Command, run: impl FnOnce() -> Outcome + UnwindSafe) -> Outcome {
    catch_unwind(run).unwrap_or_else(|payload| after_panic(command, &panic_message(payload.as_ref())))
}

/// Writes a bundle of `run` and what came of it (`sewn`, or a panic's message and where it came) next to
/// `output`, the file `stitch plan` was asked to write, and says so after `outcome`, which ends with exit
/// status 4.
pub fn beside(output: &Path, run: &Run, sewn: Result<&Sewn, (&str, Phase)>, outcome: Outcome) -> Outcome {
    let path = output.with_extension("bug-report.json");
    let mut stderr = outcome.stderr;
    match write(&path, &bundle(run, sewn)) {
        Ok(()) => {
            let _ = writeln!(stderr, "stitch: this is a bug in StitchCraft. A bug-report bundle was written to {}:", path.display());
            let _ = writeln!(stderr, "please attach it to an issue at {ISSUES}. {HOLDS_THE_DESIGN}");
        }
        Err(e) => {
            let _ = writeln!(stderr, "stitch: this is a bug in StitchCraft, and the bug-report bundle {} could not be written: {e}.", path.display());
            let _ = writeln!(stderr, "Please report it at {ISSUES} with your design.");
        }
    }
    Outcome { stdout: outcome.stdout, stderr, status: Status::Bug }
}

/// What `stitch` does after a panic in `command`.
fn after_panic(command: &Command, message: &str) -> Outcome {
    let said = format!("stitch: StitchCraft stopped on a bug in itself: {message}\n");
    let stopped = Outcome { stdout: String::new(), stderr: said.clone(), status: Status::Bug };
    if let Command::Plan(args) = command
        && let (Ok(asked), Ok(format), Ok(design)) =
            (asked_profile(args.profile.as_deref()), output_format(args.format, &args.output), files::read_capped(&args.design))
    {
        let source = Source::of(&args.design);
        let profile = machine_for(asked, source, &design);
        let reading = Reading::from(&args.reading);
        let run = Run { design_path: &args.design, source, reading, name: design_name(&args.design), design: &design, profile, format, says: None };
        return beside(&args.output, &run, Err((message, Phase::Plan)), stopped);
    }
    Outcome { stderr: said + &format!("Please report it at {ISSUES}, with the command you ran and the files it read.\n"), ..stopped }
}

/// A panic's message.
pub fn panic_message(payload: &(dyn Any + Send)) -> String {
    payload
        .downcast_ref::<&str>()
        .map(|s| (*s).to_string())
        .or_else(|| payload.downcast_ref::<String>().cloned())
        .unwrap_or_else(|| "a panic without a message".to_string())
}

/// `stitch bug-report DESIGN`.
fn make(args: &BugReportArgs, design_path: &Path) -> Outcome {
    let asked = match asked_profile(args.profile.as_deref()) {
        Ok(asked) => asked,
        Err(outcome) => return outcome,
    };
    let path = args.output.clone().unwrap_or_else(|| design_path.with_extension("bug-report.json"));
    if let Err(outcome) = distinct(&[design_path], &[&path]) {
        return outcome;
    }
    let design = match files::read_capped(design_path) {
        Ok(bytes) => bytes,
        Err(e) => {
            return Outcome { stdout: String::new(), stderr: format!("stitch: cannot read {}: {e}\n", design_path.display()), status: Status::Io };
        }
    };
    let source = Source::of(design_path);
    let reading = Reading::from(&args.reading);
    let profile = machine_for(asked, source, &design);
    let format = args.format.map_or(profile.format, FormatId::from);
    let run = Run { design_path, source, reading, name: design_name(design_path), design: &design, profile, format, says: args.says.clone() };
    let bundle = bundle(&run, borrowed(&guarded_sew(&run, Phase::Plan)));
    if let Err(e) = write(&path, &bundle) {
        return Outcome { stdout: String::new(), stderr: format!("stitch: cannot write {}: {e}\n", path.display()), status: Status::Io };
    }
    let mut out = String::new();
    let _ = writeln!(out, "{}", path.display());
    let _ = writeln!(out, "  design        {}, planned for {} as {}", bundle.design.file, profile.id, format.name());
    describe(&mut out, &bundle.result);
    let _ = writeln!(out, "To report a bug, attach it to an issue at {ISSUES} and say what goes wrong.");
    let _ = writeln!(out, "{HOLDS_THE_DESIGN}");
    Outcome::done(out)
}

/// What came of a run, as report lines.
fn describe(out: &mut String, record: &Record) {
    let _ = writeln!(out, "  plan          {}", record.plan.as_ref().map_or("none", |p| p.counts.as_str()));
    let _ = writeln!(out, "  machine file  {}", record.file.as_ref().map_or_else(|| "none".to_string(), |f| format!("{} bytes", f.bytes)));
    let _ = writeln!(out, "  diagnostics   {}", record.diagnostics.len());
    if let Some(panic) = &record.panic {
        let _ = writeln!(out, "  panic         {panic}");
    }
}

/// Plans `run`'s design as `stitch plan` does, and draws its picture too when the run being reproduced got
/// as far as `phase` [`Phase::Picture`], or the message of the panic that stopped it and where it came.
fn guarded_sew(run: &Run, phase: Phase) -> Result<Sewn, (String, Phase)> {
    let sewn = catch_unwind(|| sew(run.source, run.design, run.reading, run.profile, run.format, run.name))
        .map_err(|payload| (panic_message(payload.as_ref()), Phase::Plan))?;
    if let (Phase::Picture, Some(back)) = (phase, &sewn.back) {
        // What the picture comes to is the run's, which the bundle does not record: only whether it panics.
        let _ = catch_unwind(|| picture(back)).map_err(|payload| (panic_message(payload.as_ref()), Phase::Picture))?;
    }
    Ok(sewn)
}

/// What [`guarded_sew`] returns, borrowed as [`bundle`] takes it.
fn borrowed(sewn: &Result<Sewn, (String, Phase)>) -> Result<&Sewn, (&str, Phase)> {
    sewn.as_ref().map_err(|(message, phase)| (message.as_str(), *phase))
}

/// The bundle of `run` and what came of it.
fn bundle(run: &Run, sewn: Result<&Sewn, (&str, Phase)>) -> Bundle {
    let kept = run.source.read_part(run.design);
    let (text, hexed) = match std::str::from_utf8(kept) {
        Ok(text) => (Some(text.to_string()), None),
        Err(_) => (None, Some(hex(kept))),
    };
    let ((svg, svg_hex), (vectorcraft, vectorcraft_hex)) = match run.source {
        Source::Svg => ((text, hexed), (None, None)),
        Source::VectorCraft => ((None, None), (text, hexed)),
    };
    let file = run.design_path.file_name().map_or_else(String::new, |name| name.to_string_lossy().into_owned());
    Bundle {
        stitchcraft_bug_report: BUNDLE_FORMAT,
        version: env!("CARGO_PKG_VERSION").to_string(),
        commit: Some(COMMIT.to_string()).filter(|commit| !commit.is_empty()),
        platform: format!("{} {}", std::env::consts::OS, std::env::consts::ARCH),
        says: run.says.clone(),
        command: "plan".to_string(),
        profile: run.profile.id.to_string(),
        format: run.format.extension().to_string(),
        auto: run.reading.auto,
        sort_colours: run.reading.sort_colours,
        design: DesignRecord {
            file,
            name: Some(run.name.to_string()),
            sha256: hex(&Sha256::digest(kept)),
            svg,
            svg_hex,
            vectorcraft,
            vectorcraft_hex,
        },
        result: match sewn {
            // Planning came through before the panic, and planned again it is what the run planned.
            Err((message, phase)) if phase != Phase::Plan => {
                Record { panic: Some(message.to_string()), phase: Some(phase), ..record(borrowed(&guarded_sew(run, Phase::Plan))) }
            }
            sewn => record(sewn),
        },
    }
}

/// What came of planning: the plan, the file and the diagnostics, or a panic and where it came.
fn record(sewn: Result<&Sewn, (&str, Phase)>) -> Record {
    match sewn {
        Ok(sewn) => Record {
            plan: sewn.plan.as_ref().map(|plan| PlanRecord { sha256: digest(plan), counts: counts(plan) }),
            file: sewn.file.as_ref().map(|bytes| FileRecord { bytes: bytes.len(), sha256: hex(&Sha256::digest(bytes)) }),
            diagnostics: sewn.diagnostics.iter().map(Diagnostic::headline).collect(),
            panic: None,
            phase: None,
        },
        Err((message, phase)) => Record { plan: None, file: None, diagnostics: Vec::new(), panic: Some(message.to_string()), phase: Some(phase) },
    }
}

/// The SHA-256 of every entry of `plan`, bit for bit: the element ids, each block's thread, and each
/// entry's kind, position (the bits of its coordinates), role and element.
fn digest(plan: &StitchPlan) -> String {
    let mut text = String::new();
    for id in &plan.elements {
        let _ = writeln!(text, "element {id}");
    }
    for block in &plan.blocks {
        let _ = writeln!(text, "thread {} {:?}", block.thread.color, block.thread.name);
        for s in &block.stitches {
            let element = s.origin.element.map_or_else(|| "-".to_string(), |e| e.index().to_string());
            let (x, y) = (s.at.x().to_bits(), s.at.y().to_bits());
            let _ = writeln!(text, "{:?} {x:016x} {y:016x} {:?} {element}", s.kind, s.origin.role);
        }
    }
    hex(&Sha256::digest(text.as_bytes()))
}

/// Writes `bundle` to `path`.
fn write(path: &Path, bundle: &Bundle) -> Result<(), String> {
    let mut text = serde_json::to_string_pretty(bundle).map_err(|e| e.to_string())?;
    text.push('\n');
    files::write_atomically(path, text.as_bytes()).map_err(|e| e.to_string())
}

/// `stitch bug-report --replay BUNDLE`.
fn replay(path: &Path) -> Outcome {
    let bytes = match files::read_at_most(path, BUNDLE_MAX) {
        Ok(bytes) => bytes,
        Err(e) => return Outcome { stdout: String::new(), stderr: format!("stitch: cannot read {}: {e}\n", path.display()), status: Status::Io },
    };
    let then = match readable(path, &bytes) {
        Ok(then) => then,
        Err(message) => {
            let unreadable = Diagnostic::new(Code::BundleUnreadable, message);
            return Outcome { stdout: String::new(), stderr: render_diagnostics(&[unreadable]), status: Status::DesignErrors };
        }
    };
    let Replayable { bundle: then, source, design, profile, format } = then;
    let design_path = PathBuf::from(&then.design.file);
    let name = then.design.name.clone().unwrap_or_else(|| design_name(&design_path).to_string());
    let reading = Reading { auto: then.auto, sort_colours: then.sort_colours };
    let run = Run { design_path: &design_path, source, reading, name: &name, design: &design, profile, format, says: then.says.clone() };
    let phase = then.result.phase.unwrap_or(Phase::Plan);
    let now = bundle(&run, borrowed(&guarded_sew(&run, phase)));
    let mut out = String::new();
    let _ = writeln!(
        out,
        "{}: written by StitchCraft {} on {}, replayed by {} on {}",
        path.display(),
        printable(&built(&then)),
        printable(&then.platform),
        printable(&built(&now)),
        printable(&now.platform)
    );
    if phase == Phase::Writing {
        let _ = writeln!(out, "  The panic came while writing the files, which a replay does not do: what came before it is compared.");
    }
    let mut same = compare(&mut out, "design", &then.design.sha256, &now.design.sha256, |sha| format!("{} (sha256 {sha})", now.design.file));
    let plan = |p: &Option<PlanRecord>| p.as_ref().map_or_else(|| "none".to_string(), |p| format!("{} (sha256 {})", p.counts, p.sha256));
    same &= compare(&mut out, "plan", &then.result.plan, &now.result.plan, plan);
    let file = |f: &Option<FileRecord>| f.as_ref().map_or_else(|| "none".to_string(), |f| format!("{} bytes (sha256 {})", f.bytes, f.sha256));
    same &= compare(&mut out, "machine file", &then.result.file, &now.result.file, file);
    same &= compare_diagnostics(&mut out, &then.result.diagnostics, &now.result.diagnostics);
    if phase != Phase::Writing && (then.result.panic.is_some() || now.result.panic.is_some()) {
        let panic = |record: &Record| record.panic.as_ref().map(|message| (message.clone(), record.phase.unwrap_or(Phase::Plan)));
        let shown = |p: &Option<(String, Phase)>| {
            p.as_ref().map_or_else(|| "none".to_string(), |(message, phase)| format!("{message} (while {})", phase.while_()))
        };
        same &= compare(&mut out, "panic", &panic(&then.result), &panic(&now.result), shown);
    }
    let _ = writeln!(out, "{}", if same { "reproduced" } else { "not reproduced" });
    Outcome { stdout: out, stderr: String::new(), status: if same { Status::Done } else { Status::DesignErrors } }
}

/// A bundle this StitchCraft can replay, with the design, profile and format it names.
struct Replayable {
    bundle: Bundle,
    source: Source,
    design: Vec<u8>,
    profile: &'static MachineProfile,
    format: FormatId,
}

/// The bundle in `bytes` (read from `path`), or why it cannot be replayed (`SC-E0012`). Its format is read
/// first, alone, since another format may hold other fields.
fn readable(path: &Path, bytes: &[u8]) -> Result<Replayable, String> {
    let shown = path.display();
    let not_a_bundle = |e: serde_json::Error| format!("{shown} is not a bug-report bundle: {}.", printable(&e.to_string()));
    let head: Head = serde_json::from_slice(bytes).map_err(not_a_bundle)?;
    if head.stitchcraft_bug_report != BUNDLE_FORMAT {
        return Err(format!("{shown} is a bundle of format {}, and this StitchCraft replays format {BUNDLE_FORMAT}.", head.stitchcraft_bug_report));
    }
    let bundle: Bundle = serde_json::from_slice(bytes).map_err(not_a_bundle)?;
    if bundle.command != "plan" {
        return Err(format!("{shown} records `stitch {}`, which this StitchCraft does not replay.", printable(&bundle.command)));
    }
    let (source, design) = {
        let record = &bundle.design;
        let fields = [
            (Source::Svg, &record.svg, false),
            (Source::Svg, &record.svg_hex, true),
            (Source::VectorCraft, &record.vectorcraft, false),
            (Source::VectorCraft, &record.vectorcraft_hex, true),
        ];
        let mut held = fields.into_iter().filter_map(|(source, field, hexed)| field.as_ref().map(|text| (source, text, hexed)));
        let (Some((source, text, hexed)), None) = (held.next(), held.next()) else {
            return Err(format!(
                "{shown} must hold its design once: as `svg` or `svg_hex` for an SVG file, as `vectorcraft` or `vectorcraft_hex` for a VectorCraft document."
            ));
        };
        let design =
            if hexed { unhex(text).ok_or_else(|| format!("{shown} holds a design that is not hexadecimal."))? } else { text.as_bytes().to_vec() };
        (source, design)
    };
    let profile = profiles::find(&bundle.profile)
        .ok_or_else(|| format!("{shown} names the profile `{}`, which this StitchCraft does not have.", printable(&bundle.profile)))?;
    let format = FormatId::from_extension(&bundle.format)
        .ok_or_else(|| format!("{shown} names the format `{}`, which this StitchCraft does not write.", printable(&bundle.format)))?;
    Ok(Replayable { bundle, source, design, profile, format })
}

/// The StitchCraft a bundle was written by: its version, and the commit it was built from when there was one.
fn built(bundle: &Bundle) -> String {
    match &bundle.commit {
        Some(commit) => format!("{} (commit {commit})", bundle.version),
        None => bundle.version.clone(),
    }
}

impl Phase {
    /// What `stitch plan` was doing, completing "while …".
    const fn while_(self) -> &'static str {
        match self {
            Phase::Plan => "planning",
            Phase::Picture => "drawing the picture",
            Phase::Writing => "writing",
        }
    }
}

/// `text` from a bundle as it can be printed: each character a terminal would act on or not show as itself
/// (control characters, line breaks among them, and invisible format characters) as its escape, `\u{1b}`.
fn printable(text: &str) -> String {
    text.chars()
        .map(|c| {
            let escaped = c.escape_debug().to_string();
            if c.is_control() || escaped.starts_with("\\u") { escaped } else { c.to_string() }
        })
        .collect()
}

/// One line comparing `what` then and now, shown by `show`; whether they are the same.
fn compare<T: PartialEq>(out: &mut String, what: &str, then: &T, now: &T, show: impl Fn(&T) -> String) -> bool {
    if then == now {
        let _ = writeln!(out, "  {what:<13} same: {}", printable(&show(now)));
        return true;
    }
    let _ = writeln!(out, "  {what:<13} differs");
    let _ = writeln!(out, "    then:       {}", printable(&show(then)));
    let _ = writeln!(out, "    now:        {}", printable(&show(now)));
    false
}

/// The diagnostics then and now: the same, or those said only then and only now; whether they are the
/// same.
fn compare_diagnostics(out: &mut String, then: &[String], now: &[String]) -> bool {
    if then == now {
        let _ = writeln!(out, "  diagnostics   same: {}", now.len());
        return true;
    }
    let _ = writeln!(out, "  diagnostics   differs");
    for d in then.iter().filter(|d| !now.contains(d)) {
        let _ = writeln!(out, "    then only:  {}", printable(d));
    }
    for d in now.iter().filter(|d| !then.contains(d)) {
        let _ = writeln!(out, "    now only:   {}", printable(d));
    }
    if then.iter().all(|d| now.contains(d)) && now.iter().all(|d| then.contains(d)) {
        let _ = writeln!(out, "    the same ones, in another order or number");
    }
    false
}

#[cfg(test)]
mod tests;
