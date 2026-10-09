//! Running cases: data cases in-process, Rust-test cases through `cargo test`.

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::Path;

use sha2::{Digest, Sha256};
use stitchcraft_core::Budget;
use stitchcraft_engine::testsheets;
use stitchcraft_plan::{FormatId, MachineProfile, StitchPlan, invariants, profiles};
use stitchcraft_render::{Settings, Style};

use super::cases::{DataCase, RustCase, SheetExpect, Spec};
use super::oracle;
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
    /// Outputs it produced, with their SHA-256 (for the cross-platform determinism check).
    pub outputs: Vec<(String, String)>,
    /// Why the case could not run here (a tool is missing), if it did not.
    pub skipped: Option<String>,
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
    };
    match &case.spec {
        Spec::Testsheet { sheet, profile, expect } => testsheet(root, sheet, profile, expect, bless, &mut outcome),
        Spec::Plan { svg, profile, expect } => planned(root, svg, profile, expect, bless, &mut outcome),
        Spec::Oracle { files } => oracle::run(root, files, &mut outcome),
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

/// A plan case: the SVG design read and planned as `stitch plan` does, then checked like a test sheet.
fn planned(root: &Path, svg: &str, profile_id: &str, expect: &SheetExpect, bless: bool, outcome: &mut Outcome) {
    let Some(profile) = profiles::find(profile_id) else {
        return outcome.failures.push(format!("unknown profile `{profile_id}`"));
    };
    let read = std::fs::read(root.join("conformance").join(svg)).map_err(|e| e.to_string());
    let design = match read.and_then(|bytes| stitchcraft_svg::read(&bytes, &Budget::DEFAULT).map_err(|e| e.to_string())) {
        Ok(design) => design,
        Err(e) => return outcome.failures.push(format!("{svg}: {e}")),
    };
    let planned = stitchcraft_engine::plan(&design.design, profile, &Budget::DEFAULT);
    let codes: Vec<String> = design.warnings.iter().chain(&planned.diagnostics).map(|d| d.code.id().to_string()).collect();
    let name = Path::new(svg).file_stem().and_then(|s| s.to_str()).unwrap_or("design");
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
            Ok(bytes) => bytes,
            Err(e) => {
                fail(outcome, format!("{}: {e}", format.name()));
                continue;
            }
        };
        outcome.outputs.push((golden.clone(), hex(&Sha256::digest(&bytes))));
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
        let Ok(image) = stitchcraft_render::preview(&decoded.plan, settings, &mut Budget::DEFAULT.meter()) else { continue };
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
    // `test path::to::req_plan_002_stitch_lengths ... ok`
    let mut results: BTreeMap<&str, Vec<bool>> = BTreeMap::new();
    for line in stdout.lines() {
        let Some(rest) = line.strip_prefix("test ") else { continue };
        let Some((path, verdict)) = rest.split_once(" ... ") else { continue };
        let name = path.rsplit("::").next().unwrap_or(path);
        match verdict.trim() {
            "ok" => results.entry(name).or_default().push(true),
            "FAILED" => results.entry(name).or_default().push(false),
            _ => {}
        }
    }
    if !output.status.success() && results.values().all(|r| r.iter().all(|ok| *ok)) {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let tail: Vec<&str> = stderr.lines().rev().take(15).collect();
        return Err(format!("cargo test failed before running the cases:\n{}", tail.into_iter().rev().collect::<Vec<_>>().join("\n")));
    }
    Ok(cases
        .iter()
        .map(|case| {
            let failures = match results.get(case.name.as_str()) {
                None => vec!["did not run (is it a #[test]? does it compile?)".to_string()],
                Some(runs) if runs.iter().all(|ok| *ok) => Vec::new(),
                Some(_) => vec!["the test failed (run `cargo test` for details)".to_string()],
            };
            Outcome {
                case: case.name.clone(),
                kind: "rust",
                file: case.file.clone(),
                requirements: vec![case.covers.clone()],
                failures,
                outputs: Vec::new(),
                skipped: None,
            }
        })
        .collect())
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().fold(String::with_capacity(bytes.len() * 2), |mut s, b| {
        let _ = write!(s, "{b:02x}");
        s
    })
}
