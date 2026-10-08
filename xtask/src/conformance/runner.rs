//! Running cases: data cases in-process, Rust-test cases through `cargo test`.

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::Path;

use sha2::{Digest, Sha256};
use stitchcraft_engine::testsheets;
use stitchcraft_plan::{FormatId, invariants, profiles};

use super::cases::{DataCase, RustCase, SheetExpect, Spec};
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
}

impl Outcome {
    /// Whether the case passed.
    pub fn passed(&self) -> bool {
        self.failures.is_empty()
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
    };
    match &case.spec {
        Spec::Testsheet { sheet, profile, expect } => testsheet(root, sheet, profile, expect, bless, &mut outcome),
    }
    outcome
}

fn testsheet(root: &Path, sheet_id: &str, profile_id: &str, expect: &SheetExpect, bless: bool, outcome: &mut Outcome) {
    let fail = |outcome: &mut Outcome, message: String| outcome.failures.push(message);
    let (Some(sheet), Some(profile)) = (testsheets::find(sheet_id), profiles::find(profile_id)) else {
        return fail(outcome, format!("unknown test sheet `{sheet_id}` or profile `{profile_id}`"));
    };
    let plan = match sheet.plan() {
        Ok(plan) => plan,
        Err(e) => return fail(outcome, format!("the sheet could not be drawn: {e}")),
    };
    // L0: the plan invariants run on every plan the suite produces.
    for violation in invariants::check(&plan, profile) {
        fail(outcome, violation.to_string());
    }
    let Some(bounds) = plan.bounds() else {
        return fail(outcome, "the plan is empty".to_string());
    };
    let codes: Vec<String> = profile.check_fit(bounds).map(|d| d.code.id().to_string()).into_iter().collect();
    if codes != expect.diagnostics {
        fail(outcome, format!("diagnostics {codes:?}, expected {:?}", expect.diagnostics));
    }
    let size = [bounds.width(), bounds.height()];
    if size.iter().zip(expect.size_mm).any(|(got, want)| (got - want).abs() > 1e-6) {
        fail(outcome, format!("size {:.3} × {:.3} mm, expected {} × {} mm", size[0], size[1], expect.size_mm[0], expect.size_mm[1]));
    }
    for golden in &expect.golden {
        let path = root.join("conformance").join(golden);
        let Some(format) = path.extension().and_then(|e| e.to_str()).and_then(FormatId::from_extension) else {
            fail(outcome, format!("{golden}: the extension names no format"));
            continue;
        };
        let bytes = match stitchcraft_formats::encode(&plan, format, sheet.id) {
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
                fail(
                    outcome,
                    format!("{golden}: differs from the golden file at byte {first} (golden {} bytes, now {})", committed.len(), bytes.len()),
                );
            }
            Err(e) => fail(outcome, format!("{golden}: {e} (bless it with `cargo xtask conformance --bless {}`)", outcome.case)),
        }
    }
}

/// Runs every Rust-test case with one `cargo test` and reads the results from its output.
pub fn run_rust_cases(root: &Path, cases: &[RustCase]) -> Result<Vec<Outcome>, String> {
    let output = util::cargo()
        .args(["test", "--workspace", "--locked", "--no-fail-fast", "--", "req_"])
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
                requirements: vec![case.requirement.clone()],
                failures,
                outputs: Vec::new(),
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
