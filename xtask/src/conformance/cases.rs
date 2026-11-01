//! What the conformance suite consists of: requirements, data cases and Rust-test cases.
//!
//! - **Requirements** (`conformance/requirements.toml`): testable statements with stable ids.
//! - **Data cases** (`conformance/cases/<area>/<name>.toml`): an input, a profile and expectations, run
//!   in-process by the runner. Unknown fields are errors, so a typo in a case cannot silently disable a
//!   check.
//! - **Rust-test cases**: a test function named `req_<area>_<nnn>_<what>` is a case for
//!   `REQ-<AREA>-<NNN>`, and one named `diag_sc_<e|w|i><nnnn>_<what>` is a case for the diagnostic code
//!   `SC-<E|W|I><NNNN>`: it produces the code from real input and checks the text a user reads. The name
//!   is the link, so there is nothing to keep in sync; they are found by scanning the source (fast enough
//!   for `--check`) and run with `cargo test`.

use std::collections::BTreeMap;
use std::path::Path;

use serde::Deserialize;

use crate::util::{self, Findings};

pub const REQUIREMENTS: &str = "conformance/requirements.toml";
const CASES: &str = "conformance/cases";
/// Where Rust-test cases live.
const SOURCE_DIRS: &[&str] = &["crates", "apps"];

#[derive(Deserialize)]
struct RequirementsFile {
    req: Vec<Requirement>,
}

/// One requirement.
#[derive(Clone, Debug, Deserialize)]
#[allow(dead_code)] // `area` and `rationale` are read by the docs generator from M3.
pub struct Requirement {
    pub id: String,
    pub area: String,
    pub level: String,
    pub statement: String,
    pub milestone: String,
    pub status: String,
    #[serde(default)]
    pub rationale: Option<String>,
}

/// Every requirement id (used by the docs check for `REQ-…` mentions).
pub fn requirement_ids() -> Result<std::collections::BTreeSet<String>, String> {
    Ok(load_requirements(&util::root())?.into_iter().map(|r| r.id).collect())
}

/// The requirements, in file order.
pub fn load_requirements(root: &Path) -> Result<Vec<Requirement>, String> {
    let file: RequirementsFile = toml::from_str(&util::read(&root.join(REQUIREMENTS))?).map_err(|e| format!("{REQUIREMENTS}: {e}"))?;
    Ok(file.req)
}

/// A data case file.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct CaseFile {
    id: String,
    requirements: Vec<String>,
    kind: String,
    #[serde(default)]
    sheet: Option<String>,
    #[serde(default)]
    design: Option<String>,
    #[serde(default)]
    profile: Option<String>,
    #[serde(default)]
    expect: Option<SheetExpect>,
    #[serde(default)]
    files: Option<Vec<String>>,
    #[serde(default)]
    excused: Option<BTreeMap<String, String>>,
    #[serde(default)]
    letters: Option<Letters>,
}

/// What a test-sheet or plan case expects.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SheetExpect {
    /// Width and height of the design, in millimetres.
    pub size_mm: [f64; 2],
    /// Exactly these diagnostic codes, in order.
    pub diagnostics: Vec<String>,
    /// Golden files (relative to `conformance/`), one per format to encode, the format taken from the
    /// extension.
    pub golden: Vec<String>,
}

/// A data case, ready to run.
#[derive(Clone, Debug)]
pub struct DataCase {
    pub id: String,
    pub requirements: Vec<String>,
    pub file: String,
    pub spec: Spec,
}

/// What a data case does.
#[derive(Clone, Debug)]
pub enum Spec {
    /// Draw a test sheet, check it, encode it and compare with golden files.
    Testsheet { sheet: String, profile: String, expect: SheetExpect },
    /// Read a design, an SVG file or a VectorCraft document (relative to `conformance/`), plan it with the
    /// engine as `stitch plan` does, check it, encode it and compare with golden files: the whole way from
    /// a design to a machine file.
    Plan { design: String, profile: String, expect: SheetExpect },
    /// Read machine files with an independent reader (pinned pyembroidery) and with ours, and compare what
    /// the machine would do. `files` are relative to `conformance/`.
    Oracle { files: Vec<String> },
    /// Sew `subject` with a pinned Ink/Stitch and with StitchCraft, and compare the machine files by measures
    /// (L3, the Ink/Stitch differential). `excused` names the measures a deviation explains, each with the
    /// deviation's id.
    Inkstitch { subject: Subject, excused: BTreeMap<String, String> },
    /// Compare the centre lines of `subject`'s shapes that automatic digitizing sews wholly as running
    /// stitches with the lines a pinned Ink/Stitch's Fill to Stroke draws (L3, the Ink/Stitch differential,
    /// `centre_lines`).
    CentreLines { subject: Subject },
}

/// What an Ink/Stitch differential case gives both programs.
#[derive(Clone, Debug, PartialEq)]
pub enum Subject {
    /// An SVG file, relative to `conformance/`, that both programs read (`design`).
    File(String),
    /// A test sheet drawn as a design (`sheet`): StitchCraft sews the sheet, and Ink/Stitch the elements
    /// StitchCraft sews it with, automatic digitizing's for its automatic shapes, written with their
    /// parameters (`stitchcraft_testkit::inkstitch`), since StitchCraft's SVG reader reads no parameters
    /// until M8.
    Sheet(String),
    /// Letters drawn for the case (`[letters]`), for its centre lines.
    Letters(Letters),
}

/// Letters a case draws (`[letters]`): each of `names` at each of `heights_mm`, drawn in lines of each of
/// `widths_mm`, as `stitchcraft_testkit::designs::letter` draws them.
#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Letters {
    /// The letters, a character each.
    pub names: String,
    /// Their heights, in millimetres.
    pub heights_mm: Vec<f64>,
    /// The widths of their lines, in millimetres.
    pub widths_mm: Vec<f64>,
}

/// The data cases; problems with a file are recorded in `findings` and the file is skipped.
pub fn load_cases(root: &Path, findings: &mut Findings) -> Vec<DataCase> {
    let mut cases = Vec::new();
    for path in util::files(&root.join(CASES), &["toml"]) {
        let file = util::rel(&path);
        let parsed = util::read(&path).and_then(|text| toml::from_str::<CaseFile>(&text).map_err(|e| format!("{file}: {e}")));
        let case = match parsed {
            Ok(case) => case,
            Err(e) => {
                findings.error(e);
                continue;
            }
        };
        let excused = case.excused;
        if case.kind == "centre-lines" {
            let subject = match (case.sheet, case.design, case.letters, case.profile, case.expect, case.files, excused) {
                (Some(sheet), None, None, None, None, None, None) => Subject::Sheet(sheet),
                (None, Some(design), None, None, None, None, None) => Subject::File(design),
                (None, None, Some(letters), None, None, None, None) => Subject::Letters(letters),
                _ => {
                    findings.error(format!("{file}: a centre-lines case needs one of `sheet`, `design` and `[letters]`, and nothing else"));
                    continue;
                }
            };
            cases.push(DataCase { id: case.id, requirements: case.requirements, file, spec: Spec::CentreLines { subject } });
            continue;
        }
        if case.letters.is_some() {
            findings.error(format!("{file}: only a centre-lines case draws `[letters]`"));
            continue;
        }
        let spec = match (case.kind.as_str(), case.sheet, case.design, case.profile, case.expect, case.files) {
            ("testsheet", Some(sheet), None, Some(profile), Some(expect), None) if excused.is_none() => Spec::Testsheet { sheet, profile, expect },
            ("plan", None, Some(design), Some(profile), Some(expect), None) if excused.is_none() => Spec::Plan { design, profile, expect },
            ("oracle", None, None, None, None, Some(files)) if !files.is_empty() && excused.is_none() => Spec::Oracle { files },
            ("inkstitch", None, Some(design), None, None, None) => {
                Spec::Inkstitch { subject: Subject::File(design), excused: excused.unwrap_or_default() }
            }
            ("inkstitch", Some(sheet), None, None, None, None) => {
                Spec::Inkstitch { subject: Subject::Sheet(sheet), excused: excused.unwrap_or_default() }
            }
            ("testsheet", ..) => {
                findings.error(format!("{file}: a testsheet case needs `sheet`, `profile` and `[expect]`, and nothing else"));
                continue;
            }
            ("plan", ..) => {
                findings.error(format!("{file}: a plan case needs `design`, `profile` and `[expect]`, and nothing else"));
                continue;
            }
            ("oracle", ..) => {
                findings.error(format!("{file}: an oracle case needs a non-empty `files` list and nothing else"));
                continue;
            }
            ("inkstitch", ..) => {
                findings.error(format!("{file}: an inkstitch case needs `design` or `sheet`, and may have `[excused]`, and nothing else"));
                continue;
            }
            (other, ..) => {
                findings.error(format!("{file}: unknown case kind `{other}` (known: testsheet, plan, oracle, inkstitch, centre-lines)"));
                continue;
            }
        };
        cases.push(DataCase { id: case.id, requirements: case.requirements, file, spec });
    }
    cases
}

/// A Rust test that is a conformance case.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RustCase {
    /// The test function's name.
    pub name: String,
    /// What its name says it is a case for: a requirement (`REQ-…`) or a diagnostic code (`SC-…`).
    pub covers: String,
    /// Where it is defined.
    pub file: String,
}

/// Every `fn req_…` and `fn diag_…` in the workspace's crates and apps, in file order.
pub fn discover_rust_cases(root: &Path, findings: &mut Findings) -> Vec<RustCase> {
    let mut cases = Vec::new();
    for dir in SOURCE_DIRS {
        for path in util::files(&root.join(dir), &["rs"]) {
            let Ok(text) = util::read(&path) else { continue };
            for name in text.lines().filter_map(test_name) {
                let file = util::rel(&path);
                match requirement_of_test(name).or_else(|| code_of_test(name)) {
                    Some(covers) => cases.push(RustCase { name: name.to_string(), covers, file }),
                    None if name.starts_with("req_") => {
                        findings.error(format!("{file}: `{name}` starts with req_ but names no requirement (req_<area>_<nnn>_<what>)"));
                    }
                    None => findings.error(format!("{file}: `{name}` starts with diag_ but names no code (diag_sc_<e|w|i><nnnn>_<what>)")),
                }
            }
        }
    }
    cases
}

/// The function name if `line` defines `fn req_…` or `fn diag_…`.
fn test_name(line: &str) -> Option<&str> {
    let rest = line.trim_start().strip_prefix("fn ")?;
    let end = rest.find(|c: char| !(c.is_ascii_alphanumeric() || c == '_')).unwrap_or(rest.len());
    let name = rest.get(..end)?;
    (name.starts_with("req_") || name.starts_with("diag_")).then_some(name)
}

/// `diag_sc_e0701_too_wide` → `SC-E0701`.
pub fn code_of_test(name: &str) -> Option<String> {
    let mut parts = name.strip_prefix("diag_sc_")?.split('_');
    let code = parts.next()?;
    let (severity, digits) = (code.get(..1)?, code.get(1..)?);
    let well_formed = matches!(severity, "e" | "w" | "i") && digits.len() == 4 && digits.bytes().all(|b| b.is_ascii_digit());
    well_formed.then(|| format!("SC-{}{digits}", severity.to_ascii_uppercase()))
}

/// `req_fill_tat_006_gap_preserved` → `REQ-FILL-TAT-006`.
pub fn requirement_of_test(name: &str) -> Option<String> {
    let parts: Vec<&str> = name.strip_prefix("req_")?.split('_').collect();
    let number = parts.iter().position(|p| p.len() == 3 && p.chars().all(|c| c.is_ascii_digit()))?;
    if number == 0 {
        return None;
    }
    let area = parts.get(..number)?.join("-").to_ascii_uppercase();
    Some(format!("REQ-{area}-{}", parts.get(number)?))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_names_map_to_requirements() {
        assert_eq!(requirement_of_test("req_plan_002_stitch_lengths").as_deref(), Some("REQ-PLAN-002"));
        assert_eq!(requirement_of_test("req_fill_tat_006_gap_preserved").as_deref(), Some("REQ-FILL-TAT-006"));
        assert_eq!(requirement_of_test("req_thread_001").as_deref(), Some("REQ-THREAD-001"));
        assert_eq!(requirement_of_test("req_002_no_area"), None);
        assert_eq!(requirement_of_test("req_plan_stitch_lengths"), None);
        assert_eq!(requirement_of_test("plan_002"), None);
    }

    #[test]
    fn test_names_map_to_diagnostic_codes() {
        assert_eq!(code_of_test("diag_sc_e0701_too_wide").as_deref(), Some("SC-E0701"));
        assert_eq!(code_of_test("diag_sc_w0102_clamped").as_deref(), Some("SC-W0102"));
        assert_eq!(code_of_test("diag_sc_i0001").as_deref(), Some("SC-I0001"));
        for bad in ["diag_sc_x0701_x", "diag_sc_e701_x", "diag_e0701_x", "diag_sc_e07010_x", "req_sc_e0701"] {
            assert_eq!(code_of_test(bad), None, "{bad}");
        }
    }

    #[test]
    fn only_req_and_diag_functions_are_cases() {
        assert_eq!(test_name("    fn req_plan_001_x() {"), Some("req_plan_001_x"));
        assert_eq!(test_name("fn diag_sc_e0010_empty() {"), Some("diag_sc_e0010_empty"));
        assert_eq!(test_name("fn helper() {"), None);
        assert_eq!(test_name("// fn req_plan_001_commented"), None);
    }

    #[test]
    fn the_workspace_has_cases_for_every_m1_requirement() {
        let mut findings = Findings::default();
        let cases = discover_rust_cases(&util::root(), &mut findings);
        assert!(findings.errors.is_empty(), "{:?}", findings.errors);
        for id in ["REQ-PLAN-002", "REQ-PRF-001", "REQ-THREAD-001", "REQ-FMT-007", "SC-E0701"] {
            assert!(cases.iter().any(|c| c.covers == id), "{id}");
        }
    }
}
