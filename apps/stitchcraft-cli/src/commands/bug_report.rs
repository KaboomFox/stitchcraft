//! `stitch bug-report DESIGN.svg`: one file that reproduces what StitchCraft does with a design, to attach
//! to an issue; and `stitch bug-report --replay BUNDLE`, which plans it again and compares.
//!
//! StitchCraft is deterministic (`docs/src/design/determinism.md`): the same design, profile, format and
//! version give the same plan, bit for bit, on every platform. So a bundle holds exactly those — the SVG
//! file itself, the profile's id, the format and the version — and what came of them: a digest of every
//! entry of the plan, the machine file's size and digest, every diagnostic, and a panic's message if there
//! was one. Replaying plans the design again through the same code as `stitch plan` ([`sew`]) and
//! compares each of them (`REQ-CLI-001`).
//!
//! `stitch plan` writes a bundle by itself when it finds a bug in StitchCraft — a failed plan check
//! (`SC-E0009`) or a panic, which `main` catches with [`guarded`] — next to the machine file it was asked
//! for, and ends with exit status 4 (`REQ-CLI-002`). A bundle is JSON, so a maintainer can read the design
//! out of it and the person reporting can see what it holds: their whole design, which is why `stitch`
//! says so whenever it writes one.

use std::any::Any;
use std::fmt::Write as _;
use std::panic::{UnwindSafe, catch_unwind};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use stitchcraft_core::{Code, Diagnostic};
use stitchcraft_plan::{FormatId, MachineProfile, StitchPlan, profiles};

use super::plan::{Sewn, design_name, sew};
use super::{Outcome, Status, counts, headline, hex, output_format, render_diagnostics, write_file};
use crate::cli::{BugReportArgs, Command};
use crate::files;

/// The bundle format this StitchCraft writes and replays.
const BUNDLE_FORMAT: u32 = 1;
/// Where bugs are reported.
pub const ISSUES: &str = "https://github.com/KaboomFox/stitchcraft/issues";
/// What every bundle written is followed by.
const HOLDS_THE_DESIGN: &str = "It holds your design, so share it only where you are happy for the design to be seen.";

/// One run of `stitch plan`, as a bundle records it.
pub struct Run<'a> {
    /// Where the design was read from; its file name goes into the bundle.
    pub design_path: &'a Path,
    /// The design file's bytes.
    pub design: &'a [u8],
    /// The machine profile it was planned for.
    pub profile: &'a MachineProfile,
    /// The format it was written in.
    pub format: FormatId,
    /// What the person reporting says goes wrong.
    pub says: Option<String>,
}

/// A bug-report bundle (format 1).
#[derive(Debug, Serialize, Deserialize)]
struct Bundle {
    /// The bundle format: [`BUNDLE_FORMAT`].
    stitchcraft_bug_report: u32,
    /// The version of StitchCraft that ran.
    version: String,
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
    /// The design.
    design: DesignRecord,
    /// What came of it.
    result: Record,
}

/// The design file in a bundle: its text when it is UTF-8 (SVG files are), its bytes in hexadecimal
/// otherwise.
#[derive(Debug, Serialize, Deserialize)]
struct DesignRecord {
    /// The file's name, for people.
    file: String,
    /// The SHA-256 of its bytes when the bundle was written.
    sha256: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    svg: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    svg_hex: Option<String>,
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

/// Writes a bundle of `run` and what came of it (`sewn`, or a panic's message) next to `output`, the
/// file `stitch plan` was asked to write, and says so after `outcome`, which ends with exit status 4.
pub fn beside(output: &Path, run: &Run, sewn: Result<&Sewn, &str>, outcome: Outcome) -> Outcome {
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
        && let (Some(profile), Ok(format), Ok(design)) =
            (profiles::find(&args.profile), output_format(args.format, &args.output), files::read_capped(&args.design))
    {
        return beside(&args.output, &Run { design_path: &args.design, design: &design, profile, format, says: None }, Err(message), stopped);
    }
    Outcome { stderr: said + &format!("Please report it at {ISSUES}, with the command you ran and the files it read.\n"), ..stopped }
}

/// A panic's message.
fn panic_message(payload: &(dyn Any + Send)) -> String {
    payload
        .downcast_ref::<&str>()
        .map(|s| (*s).to_string())
        .or_else(|| payload.downcast_ref::<String>().cloned())
        .unwrap_or_else(|| "a panic without a message".to_string())
}

/// `stitch bug-report DESIGN.svg`.
fn make(args: &BugReportArgs, design_path: &Path) -> Outcome {
    let Some(profile) = profiles::find(&args.profile) else {
        return Outcome::usage(format!("there is no profile `{}`; `stitch profiles` shows them", args.profile));
    };
    let format = args.format.map_or(profile.format, FormatId::from);
    let design = match files::read_capped(design_path) {
        Ok(bytes) => bytes,
        Err(e) => {
            return Outcome { stdout: String::new(), stderr: format!("stitch: cannot read {}: {e}\n", design_path.display()), status: Status::Io };
        }
    };
    let run = Run { design_path, design: &design, profile, format, says: args.says.clone() };
    let bundle = bundle(&run, guarded_sew(&run).as_ref().map_err(String::as_str));
    let path = args.output.clone().unwrap_or_else(|| design_path.with_extension("bug-report.json"));
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

/// Plans `run`'s design as `stitch plan` does, or the message of the panic that stopped it.
fn guarded_sew(run: &Run) -> Result<Sewn, String> {
    let name = design_name(run.design_path);
    catch_unwind(|| sew(run.design, run.profile, run.format, name)).map_err(|payload| panic_message(payload.as_ref()))
}

/// The bundle of `run` and what came of it.
fn bundle(run: &Run, sewn: Result<&Sewn, &str>) -> Bundle {
    let (svg, svg_hex) = match std::str::from_utf8(run.design) {
        Ok(text) => (Some(text.to_string()), None),
        Err(_) => (None, Some(hex(run.design))),
    };
    let file = run.design_path.file_name().map_or_else(String::new, |name| name.to_string_lossy().into_owned());
    Bundle {
        stitchcraft_bug_report: BUNDLE_FORMAT,
        version: env!("CARGO_PKG_VERSION").to_string(),
        platform: format!("{} {}", std::env::consts::OS, std::env::consts::ARCH),
        says: run.says.clone(),
        command: "plan".to_string(),
        profile: run.profile.id.to_string(),
        format: run.format.extension().to_string(),
        design: DesignRecord { file, sha256: hex(&Sha256::digest(run.design)), svg, svg_hex },
        result: record(sewn),
    }
}

/// What came of planning: the plan, the file and the diagnostics, or a panic.
fn record(sewn: Result<&Sewn, &str>) -> Record {
    match sewn {
        Ok(sewn) => Record {
            plan: sewn.plan.as_ref().map(|plan| PlanRecord { sha256: digest(plan), counts: counts(plan) }),
            file: sewn.file.as_ref().map(|bytes| FileRecord { bytes: bytes.len(), sha256: hex(&Sha256::digest(bytes)) }),
            diagnostics: sewn.diagnostics.iter().map(headline).collect(),
            panic: None,
        },
        Err(message) => Record { plan: None, file: None, diagnostics: Vec::new(), panic: Some(message.to_string()) },
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
    write_file(path, text.as_bytes()).map_err(|outcome| outcome.stderr.trim_end().to_string())
}

/// `stitch bug-report --replay BUNDLE`.
fn replay(path: &Path) -> Outcome {
    let bytes = match files::read_capped(path) {
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
    let Replayable { bundle: then, design, profile, format } = then;
    let design_path = PathBuf::from(&then.design.file);
    let run = Run { design_path: &design_path, design: &design, profile, format, says: then.says.clone() };
    let now = bundle(&run, guarded_sew(&run).as_ref().map_err(String::as_str));
    let mut out = String::new();
    let _ = writeln!(
        out,
        "{}: written by StitchCraft {} on {}, replayed by {} on {}",
        path.display(),
        then.version,
        then.platform,
        now.version,
        now.platform
    );
    let mut same = compare(&mut out, "design", &then.design.sha256, &now.design.sha256, |sha| format!("{} (sha256 {sha})", now.design.file));
    let plan = |p: &Option<PlanRecord>| p.as_ref().map_or_else(|| "none".to_string(), |p| format!("{} (sha256 {})", p.counts, p.sha256));
    same &= compare(&mut out, "plan", &then.result.plan, &now.result.plan, plan);
    let file = |f: &Option<FileRecord>| f.as_ref().map_or_else(|| "none".to_string(), |f| format!("{} bytes (sha256 {})", f.bytes, f.sha256));
    same &= compare(&mut out, "machine file", &then.result.file, &now.result.file, file);
    same &= compare_diagnostics(&mut out, &then.result.diagnostics, &now.result.diagnostics);
    if then.result.panic.is_some() || now.result.panic.is_some() {
        same &= compare(&mut out, "panic", &then.result.panic, &now.result.panic, |p| p.clone().unwrap_or_else(|| "none".to_string()));
    }
    let _ = writeln!(out, "{}", if same { "reproduced" } else { "not reproduced" });
    Outcome { stdout: out, stderr: String::new(), status: if same { Status::Done } else { Status::DesignErrors } }
}

/// A bundle this StitchCraft can replay, with the design, profile and format it names.
struct Replayable {
    bundle: Bundle,
    design: Vec<u8>,
    profile: &'static MachineProfile,
    format: FormatId,
}

/// The bundle in `bytes` (read from `path`), or why it cannot be replayed (`SC-E0012`).
fn readable(path: &Path, bytes: &[u8]) -> Result<Replayable, String> {
    let shown = path.display();
    let bundle: Bundle = serde_json::from_slice(bytes).map_err(|e| format!("{shown} is not a bug-report bundle: {e}."))?;
    if bundle.stitchcraft_bug_report != BUNDLE_FORMAT {
        return Err(format!("{shown} is a bundle of format {}, and this StitchCraft replays format {BUNDLE_FORMAT}.", bundle.stitchcraft_bug_report));
    }
    if bundle.command != "plan" {
        return Err(format!("{shown} records `stitch {}`, which this StitchCraft does not replay.", bundle.command));
    }
    let design = match (&bundle.design.svg, &bundle.design.svg_hex) {
        (Some(svg), None) => svg.as_bytes().to_vec(),
        (None, Some(text)) => unhex(text).ok_or_else(|| format!("{shown} holds a design that is not hexadecimal."))?,
        _ => return Err(format!("{shown} must hold its design once, as `svg` or as `svg_hex`.")),
    };
    let profile = profiles::find(&bundle.profile)
        .ok_or_else(|| format!("{shown} names the profile `{}`, which this StitchCraft does not have.", bundle.profile))?;
    let format = FormatId::from_extension(&bundle.format)
        .ok_or_else(|| format!("{shown} names the format `{}`, which this StitchCraft does not write.", bundle.format))?;
    Ok(Replayable { bundle, design, profile, format })
}

/// Bytes from lower- or upper-case hexadecimal, two digits each.
fn unhex(text: &str) -> Option<Vec<u8>> {
    let digits = text.as_bytes();
    if !digits.len().is_multiple_of(2) {
        return None;
    }
    digits.chunks(2).map(|pair| std::str::from_utf8(pair).ok().and_then(|pair| u8::from_str_radix(pair, 16).ok())).collect()
}

/// One line comparing `what` then and now, shown by `show`; whether they are the same.
fn compare<T: PartialEq>(out: &mut String, what: &str, then: &T, now: &T, show: impl Fn(&T) -> String) -> bool {
    if then == now {
        let _ = writeln!(out, "  {what:<13} same: {}", show(now));
        return true;
    }
    let _ = writeln!(out, "  {what:<13} differs");
    let _ = writeln!(out, "    then:       {}", show(then));
    let _ = writeln!(out, "    now:        {}", show(now));
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
        let _ = writeln!(out, "    then only:  {d}");
    }
    for d in now.iter().filter(|d| !then.contains(d)) {
        let _ = writeln!(out, "    now only:   {d}");
    }
    if then.iter().all(|d| now.contains(d)) && now.iter().all(|d| then.contains(d)) {
        let _ = writeln!(out, "    the same ones, in another order or number");
    }
    false
}

#[cfg(test)]
mod tests {
    use serde_json::Value;

    use super::*;
    use crate::cli::{Format, PlanArgs};

    fn conformance(name: &str) -> PathBuf {
        PathBuf::from(format!("{}/../../conformance/{name}", env!("CARGO_MANIFEST_DIR")))
    }

    fn temp(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("stitchcraft-bug-report-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        dir.join(name)
    }

    /// A change to a bundle's JSON.
    type Change = fn(&mut Value);

    fn args(design: Option<PathBuf>) -> BugReportArgs {
        BugReportArgs { design, replay: None, output: None, profile: profiles::REFERENCE.id.to_string(), format: None, says: None }
    }

    fn json(path: &Path) -> Value {
        serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap()
    }

    fn replayed(bundle: &Path) -> Outcome {
        run(&BugReportArgs { replay: Some(bundle.to_path_buf()), ..args(None) })
    }

    /// `bundle` with `change` made to its JSON, written as `name`.
    fn changed(bundle: &Path, name: &str, change: impl FnOnce(&mut Value)) -> PathBuf {
        let mut value = json(bundle);
        change(&mut value);
        let path = temp(name);
        std::fs::write(&path, serde_json::to_vec(&value).unwrap()).unwrap();
        path
    }

    #[test]
    fn req_cli_001_a_bundle_reproduces_its_plan_byte_for_byte() {
        let (design, bundle) = (conformance("fixtures/svg/strokes.svg"), temp("strokes.bug-report.json"));
        let says = Some("the zigzag looks wrong".to_string());
        let out = run(&BugReportArgs { output: Some(bundle.clone()), says, ..args(Some(design.clone())) });
        assert_eq!(out.status, Status::Done, "{}", out.stderr);
        assert!(out.stdout.contains("  plan          425 stitches, 6 jumps, 0 trims, 3 colour changes, 0 stops\n"), "{}", out.stdout);
        assert!(out.stdout.ends_with(&format!("{HOLDS_THE_DESIGN}\n")));
        // The design itself, the profile, the format, and the digest of what `stitch plan` writes.
        let written = json(&bundle);
        assert_eq!(written["design"]["svg"].as_str().map(str::as_bytes), Some(std::fs::read(&design).unwrap().as_slice()));
        assert_eq!(
            (written["profile"].as_str(), written["format"].as_str(), written["says"].as_str()),
            (Some(profiles::REFERENCE.id), Some("pes"), Some("the zigzag looks wrong"))
        );
        let golden = std::fs::read(conformance("golden/plans/strokes.pes")).unwrap();
        assert_eq!(written["result"]["file"]["sha256"].as_str(), Some(hex(&Sha256::digest(&golden)).as_str()));
        // Replayed: all the same.
        let out = replayed(&bundle);
        assert_eq!(out.status, Status::Done, "{}", out.stdout);
        assert!(out.stdout.contains("  plan          same: 425 stitches, 6 jumps, 0 trims, 3 colour changes, 0 stops (sha256 "), "{}", out.stdout);
        assert!(out.stdout.ends_with("  diagnostics   same: 2\nreproduced\n"), "{}", out.stdout);
        // A design changed in the bundle is not the one its results came from.
        let moved = changed(&bundle, "moved.json", |value| {
            let svg = value["design"]["svg"].as_str().unwrap().replace("M 10,35 H 25", "M 10,35 H 26");
            value["design"]["svg"] = Value::from(svg);
        });
        let out = replayed(&moved);
        assert_eq!(out.status, Status::DesignErrors);
        assert!(out.stdout.contains("  design        differs\n") && out.stdout.contains("  plan          differs\n"), "{}", out.stdout);
        assert!(out.stdout.ends_with("not reproduced\n"));
        // A changed result: only it differs.
        let other = changed(&bundle, "other.json", |value| value["result"]["diagnostics"][1] = Value::from("info SC-I0504: Something else."));
        let out = replayed(&other);
        assert_eq!(out.status, Status::DesignErrors);
        assert!(
            out.stdout.contains("  plan          same") && out.stdout.contains("    then only:  info SC-I0504: Something else.\n"),
            "{}",
            out.stdout
        );
    }

    #[test]
    fn req_cli_001_each_difference_is_named_and_any_one_fails_the_replay() {
        let bundle = temp("each.bug-report.json");
        assert_eq!(run(&BugReportArgs { output: Some(bundle.clone()), ..args(Some(conformance("fixtures/svg/strokes.svg"))) }).status, Status::Done);
        // The underline moved up 1 mm sews the same counts: only the plan's digest tells.
        let shifted = changed(&bundle, "shifted.json", |value| {
            let svg = value["design"]["svg"].as_str().unwrap().replace("M 10,50 H 40", "M 10,49 H 40");
            value["design"]["svg"] = Value::from(svg);
        });
        let out = replayed(&shifted);
        assert!(
            out.stdout.contains(
                "  plan          differs
    then:       425 stitches, 6 jumps, 0 trims, 3 colour changes, 0 stops (sha256 "
            ),
            "{}",
            out.stdout
        );
        assert!(out.stdout.contains("    now:        425 stitches, 6 jumps, 0 trims, 3 colour changes, 0 stops (sha256 "), "{}", out.stdout);
        // One recorded result changed at a time.
        let cases: [(&str, Change, &str); 3] = [
            ("plan.json", |v| v["result"]["plan"]["sha256"] = Value::from("0"), "  plan          differs\n"),
            ("file.json", |v| v["result"]["file"]["sha256"] = Value::from("0"), "  machine file  differs\n"),
            ("panic.json", |v| v["result"]["panic"] = Value::from("boom"), "  panic         differs\n"),
        ];
        for (name, change, line) in cases {
            let out = replayed(&changed(&bundle, name, change));
            assert_eq!(out.status, Status::DesignErrors, "{name}");
            assert!(out.stdout.contains(line) && out.stdout.ends_with("not reproduced\n"), "{name}: {}", out.stdout);
        }
        // A diagnostic left out, and the same ones in another order.
        let fewer = changed(&bundle, "fewer.json", |v| v["result"]["diagnostics"].as_array_mut().unwrap().truncate(1));
        let out = replayed(&fewer);
        let merged = "info SC-I0504: A needle point less than the shortest stitch (0.3 mm) from the one before was left out.";
        assert!(out.stdout.contains(&format!("  diagnostics   differs\n    now only:   {merged}\n")), "{}", out.stdout);
        assert!(!out.stdout.contains("another order"), "{}", out.stdout);
        let swapped = changed(&bundle, "swapped.json", |v| v["result"]["diagnostics"].as_array_mut().unwrap().reverse());
        assert!(replayed(&swapped).stdout.contains("  diagnostics   differs\n    the same ones, in another order or number\n"));
    }

    #[test]
    fn req_cli_001_a_design_stitchcraft_refuses_replays_too() {
        // Not UTF-8: kept in hexadecimal, and refused the same way when replayed. By default the bundle
        // goes next to the design.
        let design = temp("latin1.svg");
        std::fs::write(&design, b"<svg>\xe9</svg>").unwrap();
        let out = run(&BugReportArgs { format: Some(Format::Dst), ..args(Some(design)) });
        assert_eq!(out.status, Status::Done, "{}", out.stderr);
        let bundle = temp("latin1.bug-report.json");
        let written = json(&bundle);
        assert_eq!((written["design"]["svg_hex"].as_str(), written["format"].as_str()), (Some("3c7376673ee93c2f7376673e"), Some("dst")));
        assert!(written["result"]["diagnostics"][0].as_str().unwrap().starts_with("error SC-E0801: "), "{written}");
        assert_eq!((written["result"]["plan"].clone(), written["result"]["file"].clone()), (Value::Null, Value::Null));
        assert_eq!(replayed(&bundle).status, Status::Done);
    }

    #[test]
    fn req_cli_002_a_bug_found_while_planning_writes_a_bundle() {
        // A failed plan check: a bundle next to the machine file asked for, and exit status 4.
        let design = std::fs::read(conformance("fixtures/svg/strokes.svg")).unwrap();
        let profile = profiles::REFERENCE;
        let run = Run { design_path: Path::new("strokes.svg"), design: &design, profile, format: FormatId::PesV1, says: None };
        let check = Diagnostic::new(Code::InternalCheckFailed, "A stitch of 0.1 mm.");
        let sewn = Sewn { diagnostics: vec![check.clone()], plan: None, file: None };
        assert!(sewn.found_a_bug());
        assert!(!Sewn { diagnostics: vec![Diagnostic::new(Code::StitchesMerged, "Merged.")], plan: None, file: None }.found_a_bug());
        let out = beside(&temp("checked.pes"), &run, Ok(&sewn), Outcome::refuse(String::new(), &[check]));
        assert_eq!(out.status, Status::Bug);
        let bundle = temp("checked.bug-report.json");
        assert!(out.stderr.contains(&format!("A bug-report bundle was written to {}:\n", bundle.display())), "{}", out.stderr);
        assert_eq!(json(&bundle)["result"]["diagnostics"][0], "error SC-E0009: A stitch of 0.1 mm.");
        // A panic in `stitch plan`: caught, said, and bundled with the design.
        let plan = PlanArgs {
            design: conformance("fixtures/svg/strokes.svg"),
            output: temp("panicked.pes"),
            profile: profiles::REFERENCE.id.to_string(),
            format: None,
            preview: None,
            report: None,
        };
        let out = guarded(&Command::Plan(plan), || panic!("a test of the panic guard"));
        assert_eq!(out.status, Status::Bug);
        assert!(out.stderr.starts_with("stitch: StitchCraft stopped on a bug in itself: a test of the panic guard\n"), "{}", out.stderr);
        let bundle = temp("panicked.bug-report.json");
        assert_eq!(json(&bundle)["result"]["panic"], "a test of the panic guard");
        // This design plans without a panic, so the replay says what differs.
        assert!(replayed(&bundle).stdout.contains("  panic         differs\n    then:       a test of the panic guard\n    now:        none\n"));
        // Any other command says how to report it; a command that ends normally is left alone.
        let out = guarded(&Command::Profiles, || panic!("{}", String::from("another test")));
        assert_eq!(
            out.stderr,
            format!(
                "stitch: StitchCraft stopped on a bug in itself: another test\nPlease report it at {ISSUES}, with the command you ran and the files it read.\n"
            )
        );
        assert_eq!(guarded(&Command::Profiles, || Outcome::done("fine".to_string())).stdout, "fine");
    }

    #[test]
    fn diag_sc_e0012_a_file_that_is_not_a_bundle_is_not_replayed() {
        let notes = temp("notes.txt");
        std::fs::write(&notes, b"the zigzag looks wrong").unwrap();
        let out = replayed(&notes);
        assert_eq!(out.status, Status::DesignErrors);
        assert_eq!(
            out.stderr,
            format!(
                "error SC-E0012: {} is not a bug-report bundle: expected ident at line 1 column 2.\n  more: stitch explain SC-E0012\n",
                notes.display()
            )
        );
        // Bundles this version cannot replay say why.
        let bundle = temp("whole.bug-report.json");
        assert_eq!(run(&BugReportArgs { output: Some(bundle.clone()), ..args(Some(conformance("fixtures/svg/strokes.svg"))) }).status, Status::Done);
        let cases: [(&str, Change, &str); 5] = [
            ("newer.json", |v| v["stitchcraft_bug_report"] = Value::from(2), "is a bundle of format 2, and this StitchCraft replays format 1."),
            ("export.json", |v| v["command"] = Value::from("export"), "records `stitch export`, which this StitchCraft does not replay."),
            ("singer.json", |v| v["profile"] = Value::from("singer"), "names the profile `singer`, which this StitchCraft does not have."),
            ("jef.json", |v| v["format"] = Value::from("jef"), "names the format `jef`, which this StitchCraft does not write."),
            ("twice.json", |v| v["design"]["svg_hex"] = Value::from("00"), "must hold its design once, as `svg` or as `svg_hex`."),
        ];
        for (name, change, says) in cases {
            let path = changed(&bundle, name, change);
            let out = replayed(&path);
            assert_eq!(out.stderr, format!("error SC-E0012: {} {says}\n  more: stitch explain SC-E0012\n", path.display()), "{name}");
        }
        assert_eq!((unhex("3c7E"), unhex("3c7"), unhex("zz")), (Some(vec![0x3c, 0x7e]), None, None));
    }
}
