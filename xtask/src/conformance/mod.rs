//! `cargo xtask conformance [--check] [--filter <text>] [--bless <case>]`: the conformance suite
//! (`docs/src/design/conformance.md`).
//!
//! - `--check` (fast, part of `docs`-style consistency): requirement ids are unique and well formed;
//!   levels, statuses and milestones are valid; every data case parses and names existing requirements;
//!   every Rust test named `req_…` names an existing requirement; every `active` requirement has at least
//!   one case; the Ink/Stitch facts file parses.
//! - Without `--check`, the suite also **runs**: data cases in-process, Rust-test cases through one
//!   `cargo test`. It writes `target/conformance/report.md` (the requirement × case matrix, failures
//!   first; appended to the GitHub job summary when `GITHUB_STEP_SUMMARY` is set), `report.json` and
//!   `hashes.json` (one SHA-256 per output, for the cross-platform determinism job). It fails when a case
//!   fails or an active requirement has no passing case.
//! - `--filter <text>` runs only the cases whose id, test name or requirements contain the text.
//! - `--bless <case>` rewrites the golden files of one data case instead of comparing them. A changed
//!   golden file is a changed machine file: bless only on purpose, in a PR that says why.

mod cases;
mod report;
mod runner;

use std::collections::BTreeSet;

pub use cases::requirement_ids;

use crate::util::{self, Findings};

const INKSTITCH: &str = "conformance/inkstitch-params.toml";

/// `cargo xtask conformance …`.
pub fn run(args: &[String]) -> Result<(), String> {
    let check_only = args.iter().any(|a| a == "--check");
    let value = |flag: &str| -> Result<Option<String>, String> {
        match args.iter().position(|a| a == flag) {
            Some(i) => Ok(Some(args.get(i + 1).ok_or(format!("{flag} needs a value"))?.clone())),
            None => Ok(None),
        }
    };
    let bless = value("--bless")?;
    let filter = value("--filter")?.map(|f| f.to_ascii_lowercase());
    let root = util::root();
    let mut findings = Findings::default();
    let requirements = cases::load_requirements(&root)?;
    let known = check_requirements(&requirements, &mut findings);
    let data = cases::load_cases(&root, &mut findings);
    let rust = cases::discover_rust_cases(&root, &mut findings);

    let mut case_ids = BTreeSet::new();
    for case in &data {
        if !case_ids.insert(case.id.clone()) {
            findings.error(format!("{}: duplicate case id `{}`", case.file, case.id));
        }
        if case.requirements.is_empty() {
            findings.error(format!("{}: a case must name at least one requirement", case.file));
        }
        for r in case.requirements.iter().filter(|r| !known.contains(r.as_str())) {
            findings.error(format!("{}: unknown requirement {r}", case.file));
        }
    }
    for case in rust.iter().filter(|c| !known.contains(c.requirement.as_str())) {
        findings.error(format!("{}: `{}` names {}, which is not in {}", case.file, case.name, case.requirement, cases::REQUIREMENTS));
    }
    let covered: BTreeSet<&str> =
        data.iter().flat_map(|c| c.requirements.iter().map(String::as_str)).chain(rust.iter().map(|c| c.requirement.as_str())).collect();
    for r in requirements.iter().filter(|r| r.status == "active" && !covered.contains(r.id.as_str())) {
        findings.error(format!("{} is active but no case covers it", r.id));
    }
    let facts: toml::Table = toml::from_str(&util::read(&root.join(INKSTITCH))?).map_err(|e| format!("{INKSTITCH}: {e}"))?;
    if facts.get("param").and_then(toml::Value::as_array).is_none_or(Vec::is_empty) {
        findings.error(format!("{INKSTITCH}: no [[param]] entries"));
    }
    let active = requirements.iter().filter(|r| r.status == "active").count();
    let summary = format!("{} requirements ({active} active), {} data cases, {} Rust-test cases", requirements.len(), data.len(), rust.len());
    if check_only || !findings.errors.is_empty() {
        return findings.finish("conformance", &summary);
    }

    if let Some(id) = &bless {
        let Some(case) = data.iter().find(|c| &c.id == id) else {
            return Err(format!("there is no data case `{id}`"));
        };
        let outcome = runner::run_data_case(&root, case, true);
        if !outcome.passed() {
            return Err(format!("{id}: {}", outcome.failures.join("; ")));
        }
        println!("conformance: blessed {} golden file(s) of {id}; review the diff and say why in the PR", outcome.outputs.len());
        return Ok(());
    }

    let selected = |id: &str, requirements: &[String]| {
        filter.as_deref().is_none_or(|f| id.to_ascii_lowercase().contains(f) || requirements.iter().any(|r| r.to_ascii_lowercase().contains(f)))
    };
    let data: Vec<_> = data.into_iter().filter(|c| selected(&c.id, &c.requirements)).collect();
    let rust: Vec<_> = rust.into_iter().filter(|c| selected(&c.name, std::slice::from_ref(&c.requirement))).collect();
    let mut outcomes: Vec<runner::Outcome> = data.iter().map(|c| runner::run_data_case(&root, c, false)).collect();
    outcomes.extend(runner::run_rust_cases(&root, &rust)?);
    let markdown = report::write(&root.join("target/conformance"), &requirements, &outcomes, &commit())?;
    if let Some(path) = std::env::var_os("GITHUB_STEP_SUMMARY") {
        use std::io::Write as _;
        let appended = std::fs::OpenOptions::new().append(true).create(true).open(path).and_then(|mut f| f.write_all(markdown.as_bytes()));
        if let Err(e) = appended {
            findings.warn(format!("could not write the job summary: {e}"));
        }
    }
    for o in outcomes.iter().filter(|o| !o.passed()) {
        findings.error(format!("{} ({}): {}", o.case, o.file, o.failures.join("; ")));
    }
    // A filtered run is partial: requirements outside the filter are not expected to have run.
    for (r, verdict) in report::verdicts(&requirements, &outcomes).into_iter().filter(|_| filter.is_none()) {
        if r.status == "active" && verdict != report::Verdict::Pass {
            findings.error(format!("{} is active but has no passing case", r.id));
        }
    }
    let failed = outcomes.iter().filter(|o| !o.passed()).count();
    findings.finish("conformance", &format!("{summary}; {} cases run, {failed} failed; report in target/conformance/report.md", outcomes.len()))
}

/// Checks the requirements themselves and returns their ids.
fn check_requirements<'a>(requirements: &'a [cases::Requirement], findings: &mut Findings) -> BTreeSet<&'a str> {
    let mut seen = BTreeSet::new();
    for r in requirements {
        if !seen.insert(r.id.as_str()) {
            findings.error(format!("{}: duplicate id {}", cases::REQUIREMENTS, r.id));
        }
        if !well_formed(&r.id) {
            findings.error(format!("{}: `{}` is not REQ-AREA-NNN", cases::REQUIREMENTS, r.id));
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
    seen
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

/// The short commit id, for the report.
fn commit() -> String {
    std::process::Command::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .current_dir(util::root())
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map_or_else(|| "unknown".to_string(), |o| String::from_utf8_lossy(&o.stdout).trim().to_string())
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
