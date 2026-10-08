//! `cargo xtask conformance [--check]`: the conformance suite's consistency rules (and, from M1.9, its
//! runner). See `docs/src/design/conformance.md`.
//!
//! `--check` enforces: requirement ids are unique and well formed; levels, statuses and milestones are
//! valid; every case names existing requirements; every `active` requirement has at least one case; the
//! Ink/Stitch facts file parses.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use serde::Deserialize;

use crate::util::{self, Findings};

const REQUIREMENTS: &str = "conformance/requirements.toml";
const CASES: &str = "conformance/cases";
const INKSTITCH: &str = "conformance/inkstitch-params.toml";

#[derive(Deserialize)]
struct Requirements {
    req: Vec<Requirement>,
}

#[derive(Deserialize)]
#[allow(dead_code)] // `area` and `rationale` are read by the docs generator from M3.
struct Requirement {
    id: String,
    area: String,
    level: String,
    statement: String,
    milestone: String,
    status: String,
    #[serde(default)]
    rationale: Option<String>,
}

fn load(root: &Path) -> Result<Requirements, String> {
    toml::from_str(&util::read(&root.join(REQUIREMENTS))?).map_err(|e| format!("{REQUIREMENTS}: {e}"))
}

/// Every requirement id in `conformance/requirements.toml`.
pub fn requirement_ids() -> Result<BTreeSet<String>, String> {
    Ok(load(&util::root())?.req.into_iter().map(|r| r.id).collect())
}

/// `cargo xtask conformance`.
pub fn run(check_only: bool) -> Result<(), String> {
    let root = util::root();
    let mut findings = Findings::default();
    let requirements = load(&root)?;
    let mut seen = BTreeSet::new();
    for r in &requirements.req {
        if !seen.insert(r.id.clone()) {
            findings.error(format!("{REQUIREMENTS}: duplicate id {}", r.id));
        }
        if !well_formed(&r.id) {
            findings.error(format!("{REQUIREMENTS}: `{}` is not REQ-AREA-NNN", r.id));
        }
        if !matches!(r.level.as_str(), "L0" | "L1" | "L2" | "L3" | "L4") {
            findings.error(format!("{}: level `{}` is not L0–L4", r.id, r.level));
        }
        if !matches!(r.status.as_str(), "planned" | "active" | "retired") {
            findings.error(format!("{}: status `{}` is not planned, active or retired", r.id, r.status));
        }
        let milestone_ok = r.milestone.strip_prefix('M').is_some_and(|n| !n.is_empty() && n.chars().all(|c| c.is_ascii_digit()));
        if !milestone_ok {
            findings.error(format!("{}: milestone `{}` is not M<number>", r.id, r.milestone));
        }
        if r.statement.trim().is_empty() {
            findings.error(format!("{}: empty statement", r.id));
        }
    }

    let mut coverage: BTreeMap<String, usize> = BTreeMap::new();
    let mut case_ids = BTreeSet::new();
    let case_files = util::files(&root.join(CASES), &["toml"]);
    for path in &case_files {
        let here = util::rel(path);
        let case: toml::Table = toml::from_str(&util::read(path)?).map_err(|e| format!("{here}: {e}"))?;
        let Some(id) = case.get("id").and_then(toml::Value::as_str) else {
            findings.error(format!("{here}: missing `id`"));
            continue;
        };
        if !case_ids.insert(id.to_string()) {
            findings.error(format!("{here}: duplicate case id `{id}`"));
        }
        let refs = case.get("requirements").and_then(toml::Value::as_array).cloned().unwrap_or_default();
        if refs.is_empty() {
            findings.error(format!("{here}: a case must name at least one requirement"));
        }
        for r in refs.iter().filter_map(toml::Value::as_str) {
            if seen.contains(r) {
                *coverage.entry(r.to_string()).or_insert(0) += 1;
            } else {
                findings.error(format!("{here}: unknown requirement {r}"));
            }
        }
    }
    for r in requirements.req.iter().filter(|r| r.status == "active") {
        if !coverage.contains_key(&r.id) {
            findings.error(format!("{} is active but no case covers it", r.id));
        }
    }

    let facts: toml::Table = toml::from_str(&util::read(&root.join(INKSTITCH))?).map_err(|e| format!("{INKSTITCH}: {e}"))?;
    let params = facts.get("param").and_then(toml::Value::as_array).map_or(0, Vec::len);
    if params == 0 {
        findings.error(format!("{INKSTITCH}: no [[param]] entries"));
    }

    let active = requirements.req.iter().filter(|r| r.status == "active").count();
    let summary =
        format!("{} requirements ({active} active), {} cases, {params} Ink/Stitch parameter facts", requirements.req.len(), case_files.len());
    if !check_only {
        findings.warn("the conformance runner arrives in roadmap step M1.9; only the consistency checks ran");
    }
    findings.finish("conformance", &summary)
}

/// `REQ-` + one or more upper-case segments + a three-digit number.
fn well_formed(id: &str) -> bool {
    let Some(rest) = id.strip_prefix("REQ-") else { return false };
    let parts: Vec<&str> = rest.split('-').collect();
    let Some((number, areas)) = parts.split_last() else { return false };
    number.len() == 3
        && number.chars().all(|c| c.is_ascii_digit())
        && !areas.is_empty()
        && areas.iter().all(|a| !a.is_empty() && a.chars().all(|c| c.is_ascii_uppercase()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn requirement_id_format() {
        assert!(well_formed("REQ-FILL-TAT-006"));
        assert!(well_formed("REQ-RUN-001"));
        assert!(!well_formed("REQ-001"));
        assert!(!well_formed("REQ-run-001"));
        assert!(!well_formed("REQ-RUN-1"));
        assert!(!well_formed("RUN-001"));
    }

    #[test]
    fn the_committed_requirements_parse() {
        let ids = requirement_ids().unwrap();
        assert!(ids.contains("REQ-PLAN-001"));
    }
}
