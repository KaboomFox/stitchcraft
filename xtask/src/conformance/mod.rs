//! `cargo xtask conformance [--check] [--filter <text>] [--bless <case>]`: the conformance suite
//! (`docs/src/design/conformance.md`).
//!
//! - `--check` (fast, part of `docs`-style consistency): requirement ids are unique and well formed;
//!   levels, statuses and milestones are valid; every data case parses and names existing requirements;
//!   every Rust test named `req_…` names an existing requirement; every `active` requirement has at least
//!   one case; the Ink/Stitch facts file parses; every fixture has its row in the fixtures index, which says
//!   where it comes from and under which licence.
//! - Without `--check`, the suite also **runs**: data cases in-process, Rust-test cases through one
//!   `cargo test`. It writes `target/conformance/report.md` (the requirement × case matrix, failures
//!   first; appended to the GitHub job summary when `GITHUB_STEP_SUMMARY` is set) and `report.json`. It
//!   fails when a case fails or an active requirement has no passing case. A case that needs an optional tool is skipped
//!   without it, and fails in CI: outside CI, a requirement whose cases were all skipped is skipped too.
//! - `--filter <text>` runs only the cases whose id, test name or requirements contain the text.
//! - `--bless <case>` rewrites the golden files of one data case instead of comparing them. A changed
//!   golden file is a changed machine file: bless only on purpose, in a PR that says why.
//! - `--differential` runs the Ink/Stitch differential instead (L3, [`differential`]): only the `inkstitch`
//!   cases, which need a pinned Ink/Stitch, set up as `conformance/inkstitch/README.md` says, and judge
//!   only the requirements of level L3. It writes `target/conformance/differential.md`, and fails when a case
//!   fails or doesn't run. The other runs leave those cases and requirements to it.

mod cases;
mod differential;
mod oracle;
mod report;
mod runner;
mod wasm;

use std::collections::BTreeSet;
use std::path::Path;

pub use cases::requirement_ids;
pub use wasm::made_as_wasm;

use stitchcraft_core::Code;

use crate::util::{self, Findings};

const INKSTITCH: &str = "conformance/inkstitch-params.toml";
/// The fixtures, and their index, `README.md`.
const FIXTURES: &str = "conformance/fixtures";

/// `cargo xtask conformance …`, the Ink/Stitch differential with `differential`.
pub fn run(check_only: bool, filter: Option<&str>, bless: Option<&str>, differential: bool) -> Result<(), String> {
    let filter = filter.map(str::to_ascii_lowercase);
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
    let codes: BTreeSet<&str> = Code::ALL.iter().map(|c| c.id()).collect();
    for case in rust.iter().filter(|c| !known.contains(c.covers.as_str()) && !codes.contains(c.covers.as_str())) {
        let registry = if case.covers.starts_with("SC-") { "the diagnostics registry" } else { cases::REQUIREMENTS };
        findings.error(format!("{}: `{}` names {}, which is not in {registry}", case.file, case.name, case.covers));
    }
    let covered: BTreeSet<&str> =
        data.iter().flat_map(|c| c.requirements.iter().map(String::as_str)).chain(rust.iter().map(|c| c.covers.as_str())).collect();
    for r in requirements.iter().filter(|r| r.status == "active" && !covered.contains(r.id.as_str())) {
        findings.error(format!("{} is active but no case covers it", r.id));
    }
    // Every registered code is reachable and renders: a `diag_` case produces it from real input.
    for code in codes.iter().filter(|c| !covered.contains(*c)) {
        findings.error(format!(
            "{code} is registered but no diag_ case produces it (name a test diag_{}_<what>)",
            code.to_ascii_lowercase().replace('-', "_")
        ));
    }
    let facts: toml::Table = toml::from_str(&util::read(&root.join(INKSTITCH))?).map_err(|e| format!("{INKSTITCH}: {e}"))?;
    if facts.get("param").and_then(toml::Value::as_array).is_none_or(Vec::is_empty) {
        findings.error(format!("{INKSTITCH}: no [[param]] entries"));
    }
    check_differential(&root, &data, &mut findings)?;
    for fixture in unindexed(&root)? {
        findings.error(format!("{FIXTURES}/{fixture} has no row in {FIXTURES}/README.md, which gives each fixture's origin and licence"));
    }
    let active = requirements.iter().filter(|r| r.status == "active").count();
    let summary = format!(
        "{} requirements ({active} active), {} diagnostic codes, {} data cases, {} Rust-test cases",
        requirements.len(),
        codes.len(),
        data.len(),
        rust.len()
    );
    if check_only || !findings.errors.is_empty() {
        return findings.finish("conformance", &summary);
    }

    if let Some(id) = bless {
        let Some(case) = data.iter().find(|c| c.id == id) else {
            return Err(format!("there is no data case `{id}`"));
        };
        if is_differential(case) {
            return Err(format!("`{id}` is an Ink/Stitch differential case, which has no golden files"));
        }
        let outcome = runner::run_data_case(&root, case, true);
        if outcome.failed() {
            return Err(format!("{id}: {}", outcome.failures.join("; ")));
        }
        println!("conformance: blessed {} golden file(s) of {id}; review the diff and say why in the PR", outcome.outputs.len());
        return Ok(());
    }

    let selected = |id: &str, requirements: &[String]| {
        filter.as_deref().is_none_or(|f| id.to_ascii_lowercase().contains(f) || requirements.iter().any(|r| r.to_ascii_lowercase().contains(f)))
    };
    // The Ink/Stitch differential's cases and requirements are its own, and the other runs leave them to it.
    let (l3, requirements): (Vec<_>, Vec<_>) = requirements.into_iter().partition(|r| r.level == "L3");
    let data: Vec<_> = data.into_iter().filter(|c| selected(&c.id, &c.requirements) && is_differential(c) == differential).collect();
    if differential {
        let outcomes: Vec<runner::Outcome> = data.iter().map(|c| runner::run_data_case(&root, c, false)).collect();
        let markdown = differential::report(&outcomes, &commit(), &differential::pinned(&root)?);
        let dir = root.join("target/conformance");
        std::fs::create_dir_all(&dir)
            .and_then(|()| std::fs::write(dir.join("differential.md"), &markdown))
            .map_err(|e| format!("{}: {e}", dir.display()))?;
        summarize(&markdown, &mut findings);
        for o in &outcomes {
            if o.failed() {
                findings.error(format!("{} ({}): {}", o.case, o.file, o.failures.join("; ")));
            }
            if let Some(why) = &o.skipped {
                findings.error(format!("{} did not run: {why}", o.case));
            }
        }
        for (r, verdict) in report::verdicts(&l3, &outcomes).into_iter().filter(|_| filter.is_none()) {
            if r.status == "active" && verdict != report::Verdict::Pass {
                findings.error(format!("{} is active but has no passing case", r.id));
            }
        }
        let failed = outcomes.iter().filter(|o| !o.passed()).count();
        let summary = format!("{} cases run against Ink/Stitch, {failed} failed; report in target/conformance/differential.md", outcomes.len());
        return findings.finish("conformance --differential", &summary);
    }
    let rust: Vec<_> = rust.into_iter().filter(|c| selected(&c.name, std::slice::from_ref(&c.covers))).collect();
    let mut outcomes: Vec<runner::Outcome> = data.iter().map(|c| runner::run_data_case(&root, c, false)).collect();
    outcomes.extend(runner::run_rust_cases(&root, &rust)?);
    let markdown = report::write(&root.join("target/conformance"), &requirements, &outcomes, &commit())?;
    summarize(&markdown, &mut findings);
    for o in outcomes.iter().filter(|o| o.failed()) {
        findings.error(format!("{} ({}): {}", o.case, o.file, o.failures.join("; ")));
    }
    for o in &outcomes {
        if let Some(why) = &o.skipped {
            let message = format!("{} did not run: {why}", o.case);
            if util::in_ci() { findings.error(format!("required in CI: {message}")) } else { findings.warn(message) }
        }
    }
    // A filtered run is partial: requirements and codes outside the filter are not expected to have run.
    for (r, verdict) in report::verdicts(&requirements, &outcomes).into_iter().filter(|_| filter.is_none()) {
        if r.status == "active" && !verdict.acceptable(util::in_ci()) {
            findings.error(format!("{} is active but has no passing case", r.id));
        }
    }
    for (code, verdict) in report::code_verdicts(&outcomes).into_iter().filter(|_| filter.is_none()) {
        if !verdict.acceptable(util::in_ci()) {
            findings.error(format!("{} has no passing diag_ case", code.id()));
        }
    }
    let failed = outcomes.iter().filter(|o| o.failed()).count();
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

/// The files under the fixtures folder, by path in it, that its index has no row for. A row names its files in
/// its first cell, each in inline code.
fn unindexed(root: &Path) -> Result<Vec<String>, String> {
    let dir = root.join(FIXTURES);
    let index = util::read(&dir.join("README.md"))?;
    let named: BTreeSet<&str> =
        index.lines().filter_map(|line| line.strip_prefix("| ")?.split(" | ").next()).flat_map(|cell| cell.split('`').skip(1).step_by(2)).collect();
    let files = util::every_file(&dir).into_iter().filter_map(|path| Some(path.strip_prefix(&dir).ok()?.to_string_lossy().replace('\\', "/")));
    Ok(files.filter(|file| file != "README.md" && !named.contains(file.as_str())).collect())
}

/// Whether `case` is one of the Ink/Stitch differential's.
fn is_differential(case: &cases::DataCase) -> bool {
    matches!(case.spec, cases::Spec::Inkstitch { .. })
}

/// What is wrong with the Ink/Stitch differential's cases: a design that isn't there, and a measure excused
/// that isn't one, or by an entry the deviations ledger doesn't have.
fn check_differential(root: &Path, data: &[cases::DataCase], findings: &mut Findings) -> Result<(), String> {
    let ledger: BTreeSet<String> = crate::deviations::parse(&util::read(&root.join(crate::deviations::LEDGER))?)?.into_iter().map(|d| d.id).collect();
    for case in data {
        let cases::Spec::Inkstitch { design, excused } = &case.spec else { continue };
        if !root.join("conformance").join(design).is_file() {
            findings.error(format!("{}: the design `{design}` is not in conformance/", case.file));
        }
        for (measure, deviation) in excused {
            if differential::Measure::named(measure).is_none() {
                let names: Vec<&str> = differential::Measure::ALL.iter().map(|m| m.name()).collect();
                findings.error(format!("{}: `{measure}` is not a measure ({})", case.file, names.join(", ")));
            }
            if !ledger.contains(deviation) {
                findings.error(format!("{}: {deviation}, which excuses {measure}, is not in {}", case.file, crate::deviations::LEDGER));
            }
        }
    }
    Ok(())
}

/// Appends `markdown` to the GitHub job summary, when there is one.
fn summarize(markdown: &str, findings: &mut Findings) {
    if let Some(path) = std::env::var_os("GITHUB_STEP_SUMMARY") {
        use std::io::Write as _;
        let appended = std::fs::OpenOptions::new().append(true).create(true).open(path).and_then(|mut f| f.write_all(markdown.as_bytes()));
        if let Err(e) = appended {
            findings.warn(format!("could not write the job summary: {e}"));
        }
    }
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
    fn every_data_case_but_the_oracle_passes_here() {
        // `cargo test` runs on Linux, macOS and Windows in CI (`ci.yml`), so there each test sheet's and plan's
        // machine files are compared with their golden files byte for byte (docs/src/design/determinism.md).
        // The oracle needs Python and pyembroidery, which only the gates job installs.
        let root = util::root();
        let mut findings = Findings::default();
        let data = cases::load_cases(&root, &mut findings);
        assert!(findings.errors.is_empty() && data.len() > 10, "{:?}", findings.errors);
        let failed: Vec<String> = data
            .iter()
            .filter(|case| !matches!(case.spec, cases::Spec::Oracle { .. }) && !is_differential(case))
            .map(|case| runner::run_data_case(&root, case, false))
            .filter(|outcome| !outcome.passed())
            .map(|outcome| format!("{} ({}): {}", outcome.case, outcome.file, outcome.failures.join("; ")))
            .collect();
        assert_eq!(failed, Vec::<String>::new());
    }

    #[test]
    fn every_fixture_has_its_row_in_the_index() {
        assert_eq!(unindexed(&util::root()).unwrap(), Vec::<String>::new());
    }

    #[test]
    fn the_committed_requirements_parse() {
        let ids = requirement_ids().unwrap();
        assert!(ids.contains("REQ-PLAN-001"));
    }
}
