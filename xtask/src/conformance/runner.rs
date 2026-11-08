//! Running cases: data cases in-process, Rust-test cases through `cargo test`.

use std::collections::BTreeMap;
use std::path::Path;

use stitchcraft_cli::commands::plan::Source;
use stitchcraft_core::Budget;
use stitchcraft_engine::testsheets;
use stitchcraft_plan::{FormatId, MachineProfile, StitchPlan, invariants, profiles};
use stitchcraft_render::{Settings, Style};

use super::cases::{DataCase, MODELS_ONLY, RustCase, SheetExpect, Spec, Subject};
use super::{centre_lines, differential, fonts, oracle, routing};
use crate::util;

/// How one case went.
#[derive(Clone, Debug)]
pub struct Outcome {
    /// The case id (data cases) or test function name (Rust cases).
    pub case: String,
    /// `data` or `rust`.
    pub kind: &'static str,
    /// Where the case is defined.
    pub file: String,
    /// The requirements it covers.
    pub requirements: Vec<String>,
    /// What went wrong; empty when it passed.
    pub failures: Vec<String>,
    /// The golden files it compared its outputs with, or wrote.
    pub outputs: Vec<String>,
    /// Why the case could not run here (a tool is missing), if it did not.
    pub skipped: Option<String>,
    /// Differences a deviation excuses, which don't fail it (the Ink/Stitch differential).
    pub notes: Vec<String>,
    /// What it measured, for its report (the Ink/Stitch differential).
    pub details: Vec<String>,
}

impl Outcome {
    /// Whether the case ran and passed.
    pub fn passed(&self) -> bool {
        self.failures.is_empty() && self.skipped.is_none()
    }

    /// Whether the case ran and failed.
    pub fn failed(&self) -> bool {
        !self.failures.is_empty()
    }
}

/// Runs a data case. With `bless`, golden files are rewritten instead of compared.
pub fn run_data_case(root: &Path, case: &DataCase, bless: bool) -> Outcome {
    let mut outcome = Outcome {
        case: case.id.clone(),
        kind: "data",
        file: case.file.clone(),
        requirements: case.requirements.clone(),
        failures: Vec::new(),
        outputs: Vec::new(),
        skipped: None,
        notes: Vec::new(),
        details: Vec::new(),
    };
    match &case.spec {
        Spec::Testsheet { sheet, profile, expect } => testsheet(root, sheet, profile, expect, bless, &mut outcome),
        Spec::Plan { design, profile, expect } => planned(root, design, profile, expect, bless, &mut outcome),
        Spec::Oracle { files } => oracle::run(root, files, &mut outcome),
        Spec::Inkstitch { subject: Subject::Sheet(sheet), excused, route: Some(route) } => {
            routing::run(root, &case.id, sheet, route, excused, &mut outcome);
        }
        Spec::Inkstitch { subject, excused, .. } => differential::run(root, subject, excused, &mut outcome),
        Spec::CentreLines { subject } => centre_lines::run(root, subject, &mut outcome),
        Spec::Fonts { options } => fonts::run(root, options, &mut outcome),
    }
    outcome
}

fn testsheet(root: &Path, sheet_id: &str, profile_id: &str, expect: &SheetExpect, bless: bool, outcome: &mut Outcome) {
    let (Some(sheet), Some(profile)) = (testsheets::find(sheet_id), profiles::find(profile_id)) else {
        return outcome.failures.push(format!("unknown test sheet `{sheet_id}` or profile `{profile_id}`"));
    };
    match sheet.plan() {
        Ok(plan) => {
            let codes: Vec<String> =
                plan.bounds().and_then(|bounds| profile.check_fit(bounds)).map(|d| d.code.id().to_string()).into_iter().collect();
            checked(root, &plan, profile, &codes, sheet.id, expect, bless, outcome);
        }
        Err(e) => outcome.failures.push(format!("the sheet could not be drawn: {e}")),
    }
}

/// A plan case: the design, an SVG file or a VectorCraft document, read and planned as `stitch plan` does,
/// then checked like a test sheet.
fn planned(root: &Path, file: &str, profile_id: &str, expect: &SheetExpect, bless: bool, outcome: &mut Outcome) {
    let Some(profile) = profiles::find(profile_id) else {
        return outcome.failures.push(format!("unknown profile `{profile_id}`"));
    };
    let source = Source::of(Path::new(file));
    let bytes = std::fs::read(root.join("conformance").join(file)).map_err(|e| e.to_string());
    let (design, warnings) =
        match bytes.and_then(|bytes| source.read(&bytes, stitchcraft_engine::design::Reading::default()).map_err(|e| e.to_string())) {
            Ok(read) => read,
            Err(e) => return outcome.failures.push(format!("{file}: {e}")),
        };
    let planned = stitchcraft_engine::plan(&design, profile, &Budget::DEFAULT);
    let codes: Vec<String> = warnings.iter().chain(&planned.diagnostics).map(|d| d.code.id().to_string()).collect();
    let name = Path::new(file).file_stem().and_then(|s| s.to_str()).unwrap_or("design");
    match planned.plan {
        Some(plan) => checked(root, &plan, profile, &codes, name, expect, bless, outcome),
        None => outcome.failures.push(format!("no plan; diagnostics {codes:?}")),
    }
}

/// What every case that makes a plan checks: the plan invariants (L0, on every plan the suite produces),
/// the diagnostic `codes`, the size and the golden files (labelled `name`).
#[allow(clippy::too_many_arguments)] // Each is one thing a case names; a struct would only rename them.
fn checked(
    root: &Path,
    plan: &StitchPlan,
    profile: &MachineProfile,
    codes: &[String],
    name: &str,
    expect: &SheetExpect,
    bless: bool,
    outcome: &mut Outcome,
) {
    for violation in invariants::check(plan, profile) {
        outcome.failures.push(violation.to_string());
    }
    if codes != expect.diagnostics {
        outcome.failures.push(format!("diagnostics {codes:?}, expected {:?}", expect.diagnostics));
    }
    let Some(bounds) = plan.bounds() else {
        return outcome.failures.push("the plan is empty".to_string());
    };
    let size = [bounds.width(), bounds.height()];
    if size.iter().zip(expect.size_mm).any(|(got, want)| (got - want).abs() > 1e-6) {
        outcome.failures.push(format!("size {:.3} × {:.3} mm, expected {} × {} mm", size[0], size[1], expect.size_mm[0], expect.size_mm[1]));
    }
    goldens(root, plan, name, &expect.golden, bless, outcome);
}

/// `plan` encoded as each golden file's format, with the label `name`, and compared with it — or, with
/// `bless`, written to it.
fn goldens(root: &Path, plan: &StitchPlan, name: &str, golden_files: &[String], bless: bool, outcome: &mut Outcome) {
    let fail = |outcome: &mut Outcome, message: String| outcome.failures.push(message);
    for golden in golden_files {
        let path = root.join("conformance").join(golden);
        let Some(format) = path.extension().and_then(|e| e.to_str()).and_then(FormatId::from_extension) else {
            fail(outcome, format!("{golden}: the extension names no format"));
            continue;
        };
        let bytes = match stitchcraft_formats::encode(plan, format, name) {
            Ok(encoded) => encoded.bytes,
            Err(e) => {
                fail(outcome, format!("{}: {e}", format.name()));
                continue;
            }
        };
        outcome.outputs.push(golden.clone());
        if bless {
            let written = path.parent().map_or(Ok(()), std::fs::create_dir_all).and_then(|()| std::fs::write(&path, &bytes));
            if let Err(e) = written {
                fail(outcome, format!("{golden}: cannot bless: {e}"));
            }
            continue;
        }
        match std::fs::read(&path) {
            Ok(committed) if committed == bytes => {}
            Ok(committed) => {
                let first = committed.iter().zip(&bytes).position(|(a, b)| a != b).unwrap_or(committed.len().min(bytes.len()));
                let previews = draw_difference(root, &outcome.case, golden, &committed, &bytes);
                fail(
                    outcome,
                    format!(
                        "{golden}: differs from the golden file at byte {first} (golden {} bytes, now {}){previews}",
                        committed.len(),
                        bytes.len()
                    ),
                );
            }
            Err(e) => fail(outcome, format!("{golden}: {e} (bless it with `cargo xtask conformance --bless {}`)", outcome.case)),
        }
    }
}

/// Previews of a changed machine file, golden and new, in `target/conformance/diffs/` (CI keeps them when
/// a run fails), so a reviewer sees what sews differently. Returns a note for the failure message, empty
/// when nothing could be drawn (a golden file that no longer reads, say).
fn draw_difference(root: &Path, case: &str, golden: &str, before: &[u8], after: &[u8]) -> String {
    let dir = root.join("target/conformance/diffs");
    let stem = golden.trim_start_matches("golden/").replace(['/', '.'], "-");
    let Some(settings) = Settings::new(Style::Simple, Settings::DEFAULT_SCALE) else { return String::new() };
    let mut written = Vec::new();
    for (label, bytes) in [("golden", before), ("now", after)] {
        let Ok(decoded) = stitchcraft_formats::decode(bytes) else { continue };
        // As the reference machine sews it, which cuts no jump (MC-1).
        let Ok(image) = stitchcraft_render::preview(&decoded.plan, profiles::REFERENCE.trims, settings, &mut Budget::DEFAULT.meter()) else {
            continue;
        };
        let path = dir.join(format!("{case}--{stem}--{label}.png"));
        if std::fs::create_dir_all(&dir).and_then(|()| std::fs::write(&path, &image.png)).is_ok() {
            written.push(util::rel(&path));
        }
    }
    if written.is_empty() { String::new() } else { format!("; previews: {}", written.join(", ")) }
}

/// Runs every Rust-test case with one `cargo test` and reads the results from its output.
pub fn run_rust_cases(root: &Path, cases: &[RustCase]) -> Result<Vec<Outcome>, String> {
    let output = util::cargo()
        .arg("test")
        .args(util::packages()?.args())
        .args(["--locked", "--no-fail-fast", "--", "req_", "diag_"])
        .current_dir(root)
        .output()
        .map_err(|e| format!("cargo test: {e}"))?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    let results = verdicts(&stdout);
    if !output.status.success() && results.values().flatten().all(|verdict| *verdict != Verdict::Failed) {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let tail: Vec<&str> = stderr.lines().rev().take(15).collect();
        return Err(format!("cargo test failed before running the cases:\n{}", tail.into_iter().rev().collect::<Vec<_>>().join("\n")));
    }
    Ok(cases
        .iter()
        .map(|case| {
            let (failures, skipped) = judged(results.get(case.name.as_str()).map(Vec::as_slice));
            Outcome {
                case: case.name.clone(),
                kind: "rust",
                file: case.file.clone(),
                requirements: vec![case.covers.clone()],
                failures,
                outputs: Vec::new(),
                skipped,
                notes: Vec::new(),
                details: Vec::new(),
            }
        })
        .collect())
}

/// What `cargo test` said of a test.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Verdict<'a> {
    /// `ok`.
    Passed,
    /// `FAILED`.
    Failed,
    /// `ignored`, with the reason its `ignore` attribute gives, empty when it gives none.
    Ignored(&'a str),
}

/// The verdicts `cargo test` printed in `stdout`, by the test's function name, each test binary that holds a
/// test of that name giving one: `test path::to::req_plan_002_stitch_lengths ... ok`, `... FAILED`, `...
/// ignored` or `... ignored, REASON`.
fn verdicts(stdout: &str) -> BTreeMap<&str, Vec<Verdict<'_>>> {
    let mut results: BTreeMap<&str, Vec<Verdict<'_>>> = BTreeMap::new();
    for line in stdout.lines() {
        let Some(rest) = line.strip_prefix("test ") else { continue };
        let Some((path, said)) = rest.split_once(" ... ") else { continue };
        let name = path.rsplit("::").next().unwrap_or(path);
        let verdict = match said.trim() {
            "ok" => Verdict::Passed,
            "FAILED" => Verdict::Failed,
            "ignored" => Verdict::Ignored(""),
            said => match said.strip_prefix("ignored, ") {
                Some(reason) => Verdict::Ignored(reason),
                None => continue,
            },
        };
        results.entry(name).or_default().push(verdict);
    }
    results
}

/// How a case went from the verdicts of its runs, `runs`, or `None` when no test of its name ran: its
/// failures, and where it runs instead when it is ignored as models only ([`MODELS_ONLY`]). Ignored for any
/// other reason, it fails: no case may hide behind `#[ignore]`.
fn judged(runs: Option<&[Verdict<'_>]>) -> (Vec<String>, Option<String>) {
    let Some(runs) = runs else { return (vec!["did not run (is it a #[test]? does it compile?)".to_string()], None) };
    if runs.contains(&Verdict::Failed) {
        return (vec!["the test failed (run `cargo test` for details)".to_string()], None);
    }
    let reasons: Vec<&str> = runs.iter().filter_map(|verdict| if let Verdict::Ignored(reason) = verdict { Some(*reason) } else { None }).collect();
    match reasons.iter().find(|reason| !reason.starts_with(MODELS_ONLY)) {
        Some(reason) => {
            let why = if reason.is_empty() { String::new() } else { format!(" (\"{reason}\")") };
            (vec![format!("it is ignored{why}: a case may be ignored only as \"{MODELS_ONLY}…\"")], None)
        }
        None if runs.contains(&Verdict::Passed) => (Vec::new(), None),
        None => (Vec::new(), reasons.first().map(ToString::to_string)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// What `cargo test` prints of 5 tests of 2 test binaries: one passes, one fails, one is ignored as models
    /// only, one is ignored for another reason and one with none.
    const SAID: &str = "\
running 3 tests
test tests::req_plan_002_stitch_lengths ... ok
test req_cli_019_the_cut_out_from_u2netps_box_keeps_each_dog_within_the_bounds ... ignored, models only: the models job runs tests/models.rs
test req_cli_014_slow ... ignored, too slow
test result: FAILED. 1 passed; 1 failed; 2 ignored; 0 measured; 0 filtered out; finished in 0.01s

running 2 tests
test req_plan_003_broken ... FAILED
test diag_sc_e0701_too_wide ... ignored
";

    #[test]
    fn the_runner_reads_each_tests_verdict_and_why_it_was_ignored() {
        let read = verdicts(SAID);
        let expected = BTreeMap::from([
            ("diag_sc_e0701_too_wide", vec![Verdict::Ignored("")]),
            ("req_cli_014_slow", vec![Verdict::Ignored("too slow")]),
            (
                "req_cli_019_the_cut_out_from_u2netps_box_keeps_each_dog_within_the_bounds",
                vec![Verdict::Ignored("models only: the models job runs tests/models.rs")],
            ),
            ("req_plan_002_stitch_lengths", vec![Verdict::Passed]),
            ("req_plan_003_broken", vec![Verdict::Failed]),
        ]);
        assert_eq!(read, expected);
        // A line that is no test's verdict is no case's.
        assert!(verdicts("test result: ok. 3 passed\ntest req_plan_001_x ... bench: 12 ns/iter\n").is_empty());
    }

    #[test]
    fn a_case_ignored_as_models_only_runs_in_the_models_job_and_any_other_ignored_case_fails() {
        let models = Verdict::Ignored("models only: the models job runs tests/models.rs");
        assert_eq!(judged(Some(&[Verdict::Passed])), (Vec::new(), None));
        assert_eq!(judged(Some(&[models])), (Vec::new(), Some("models only: the models job runs tests/models.rs".to_string())));
        // Passed in one test binary, it passed, whatever another left out.
        assert_eq!(judged(Some(&[models, Verdict::Passed])), (Vec::new(), None));
        assert_eq!(judged(Some(&[Verdict::Passed, Verdict::Failed])).0, ["the test failed (run `cargo test` for details)"]);
        assert_eq!(judged(Some(&[models, Verdict::Failed])).0, ["the test failed (run `cargo test` for details)"]);
        let (failures, skipped) = judged(Some(&[Verdict::Ignored("too slow")]));
        assert_eq!(failures, ["it is ignored (\"too slow\"): a case may be ignored only as \"models only: …\""]);
        assert_eq!(skipped, None);
        let (failures, _) = judged(Some(&[models, Verdict::Ignored("")]));
        assert_eq!(failures, ["it is ignored: a case may be ignored only as \"models only: …\""]);
        assert_eq!(judged(None).0, ["did not run (is it a #[test]? does it compile?)"]);
    }
}
