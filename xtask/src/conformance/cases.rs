//! What the conformance suite consists of: requirements, data cases and Rust-test cases.
//!
//! - **Requirements** (`conformance/requirements.toml`): testable statements with stable ids.
//! - **Data cases** (`conformance/cases/<area>/<name>.toml`): an input, a profile and expectations, run
//!   in-process by the runner. Unknown fields are errors, so a typo in a case cannot silently disable a
//!   check.
//! - **Rust-test cases**: a test function named `req_<area>_<nnn>_<what>` is a case for
//!   `REQ-<AREA>-<NNN>`. The name is the link, so there is nothing to keep in sync; they are found by
//!   scanning the source (fast enough for `--check`) and run with `cargo test`.

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
    profile: Option<String>,
    #[serde(default)]
    expect: Option<SheetExpect>,
}

/// What a test-sheet case expects.
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
        let spec = match (case.kind.as_str(), case.sheet, case.profile, case.expect) {
            ("testsheet", Some(sheet), Some(profile), Some(expect)) => Spec::Testsheet { sheet, profile, expect },
            ("testsheet", ..) => {
                findings.error(format!("{file}: a testsheet case needs `sheet`, `profile` and `[expect]`"));
                continue;
            }
            (other, ..) => {
                findings.error(format!("{file}: unknown case kind `{other}` (known: testsheet)"));
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
    /// The requirement its name refers to.
    pub requirement: String,
    /// Where it is defined.
    pub file: String,
}

/// Every `fn req_…` in the workspace's crates and apps, in file order.
pub fn discover_rust_cases(root: &Path, findings: &mut Findings) -> Vec<RustCase> {
    let mut cases = Vec::new();
    for dir in SOURCE_DIRS {
        for path in util::files(&root.join(dir), &["rs"]) {
            let Ok(text) = util::read(&path) else { continue };
            for name in text.lines().filter_map(test_name) {
                match requirement_of_test(name) {
                    Some(requirement) => cases.push(RustCase { name: name.to_string(), requirement, file: util::rel(&path) }),
                    None => {
                        let file = util::rel(&path);
                        findings.error(format!("{file}: `{name}` starts with req_ but names no requirement (req_<area>_<nnn>_<what>)"));
                    }
                }
            }
        }
    }
    cases
}

/// The function name if `line` defines `fn req_…`.
fn test_name(line: &str) -> Option<&str> {
    let rest = line.trim_start().strip_prefix("fn ")?;
    let end = rest.find(|c: char| !(c.is_ascii_alphanumeric() || c == '_')).unwrap_or(rest.len());
    let name = rest.get(..end)?;
    name.starts_with("req_").then_some(name)
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
    fn only_req_functions_are_cases() {
        assert_eq!(test_name("    fn req_plan_001_x() {"), Some("req_plan_001_x"));
        assert_eq!(test_name("fn helper() {"), None);
        assert_eq!(test_name("// fn req_plan_001_commented"), None);
    }

    #[test]
    fn the_workspace_has_cases_for_every_m1_requirement() {
        let mut findings = Findings::default();
        let cases = discover_rust_cases(&util::root(), &mut findings);
        assert!(findings.errors.is_empty(), "{:?}", findings.errors);
        for id in ["REQ-PLAN-002", "REQ-PRF-001", "REQ-THREAD-001", "REQ-FMT-007"] {
            assert!(cases.iter().any(|c| c.requirement == id), "{id}");
        }
    }
}
