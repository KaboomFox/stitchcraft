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
//! - `--differential` runs the Ink/Stitch differential instead (L3, [`differential`], [`centre_lines`]):
//!   only the `inkstitch` and `centre-lines` cases, which need a pinned Ink/Stitch, set up as
//!   `conformance/inkstitch/README.md` says, and judge only the requirements of level L3. It writes `target/conformance/differential.md`, and fails when a case
//!   fails or doesn't run. The other runs leave those cases and requirements to it.

mod cases;
mod centre_lines;
mod differential;
pub mod fonts;
mod oracle;
mod report;
mod routing;
mod runner;
mod wasm;

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

pub use cases::requirement_ids;
pub use wasm::made_as_wasm;

use stitchcraft_core::Code;

use crate::util::{self, Findings};

const INKSTITCH: &str = "conformance/inkstitch-params.toml";
/// The fixtures, and their index, `README.md`.
const FIXTURES: &str = "conformance/fixtures";

/// What a run of the glyph differential sews, from the command line: every glyph, not the sample, only the
/// glyphs of some fonts, and only a part of them.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Glyphs {
    /// Every glyph (`--every-glyph`).
    pub every: bool,
    /// Only these fonts' glyphs, by their folders' names (`--font`), or every font's when there are none.
    pub fonts: Vec<String>,
    /// Only this part (`--shard`).
    pub shard: Option<fonts::Shard>,
}

/// `cargo xtask conformance …`, the Ink/Stitch differential with `differential`, which sews the fonts'
/// glyphs as `glyphs` says.
pub fn run(check_only: bool, filter: Option<&str>, bless: Option<&str>, differential: bool, glyphs: &Glyphs) -> Result<(), String> {
    let filter = filter.map(str::to_ascii_lowercase);
    let root = util::root();
    let mut findings = Findings::default();
    let requirements = cases::load_requirements(&root)?;
    let known = check_requirements(&requirements, &mut findings);
    let mut data = cases::load_cases(&root, &mut findings);
    for case in &mut data {
        if let cases::Spec::Fonts { options } = &mut case.spec {
            options.every = glyphs.every;
            options.fonts.clone_from(&glyphs.fonts);
            options.shard = glyphs.shard;
        }
    }
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
    // Every registered code is reachable and renders: a `diag_` case produces it from real input. A retired code
    // is given by nothing, and none produces it.
    let gone: BTreeSet<&str> = Code::ALL.iter().filter(|code| retired(**code)).map(|code| code.id()).collect();
    for code in gone.iter().filter(|c| covered.contains(*c)) {
        findings.error(format!("{code} is retired, but a diag_ case names it: StitchCraft no longer gives it"));
    }
    for code in codes.iter().filter(|c| !covered.contains(*c) && !gone.contains(*c)) {
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
    check_models_only(&root, &rust, &mut findings)?;
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
    // A models-only case runs in the models' job, which `--check` finds for it, and only the report lists it here.
    for o in &outcomes {
        let Some(why) = o.skipped.as_deref().filter(|why| !why.starts_with(cases::MODELS_ONLY)) else { continue };
        let message = format!("{} did not run: {why}", o.case);
        if util::in_ci() { findings.error(format!("required in CI: {message}")) } else { findings.warn(message) }
    }
    // A filtered run is partial: requirements and codes outside the filter are not expected to have run.
    for (r, verdict) in report::verdicts(&requirements, &outcomes).into_iter().filter(|_| filter.is_none()) {
        if r.status == "active" && !verdict.acceptable(util::in_ci()) {
            findings.error(format!("{} is active but has no passing case", r.id));
        }
    }
    for (code, verdict) in report::code_verdicts(&outcomes).into_iter().filter(|(code, _)| filter.is_none() && !retired(*code)) {
        if !verdict.acceptable(util::in_ci()) {
            findings.error(format!("{} has no passing diag_ case", code.id()));
        }
    }
    let failed = outcomes.iter().filter(|o| o.failed()).count();
    findings.finish("conformance", &format!("{summary}; {} cases run, {failed} failed; report in target/conformance/report.md", outcomes.len()))
}

/// The models' workflow, a step of which runs each test file that holds models-only cases.
const MODELS_WORKFLOW: &str = ".github/workflows/models.yml";

/// Checks that each test file holding a case that every build leaves out as models only
/// (`cases::MODELS_ONLY`) is a test target of its own, `<folder>/<package>/tests/<target>.rs`, which a step of
/// the models' workflow runs with `--ignored`: `cargo test … -p <package> --test <target> -- --ignored`. So no
/// such case is left without a job that runs it.
fn check_models_only(root: &Path, rust: &[cases::RustCase], findings: &mut Findings) -> Result<(), String> {
    let files: BTreeSet<&str> = rust.iter().filter(|case| case.models_only.is_some()).map(|case| case.file.as_str()).collect();
    if files.is_empty() {
        return Ok(());
    }
    let workflow = util::read(&root.join(MODELS_WORKFLOW))?;
    for file in files {
        match test_target(file) {
            None => findings.error(format!(
                "{file}: models-only cases belong in a test file of their own, `<package>/tests/<target>.rs`, which a step of \
                 {MODELS_WORKFLOW} runs with --ignored"
            )),
            Some((package, target)) if !runs_ignored(&workflow, package, target) => findings.error(format!(
                "{file} holds models-only cases, but no step of {MODELS_WORKFLOW} runs them: `cargo test --locked -p {package} --test {target} -- \
                 --ignored`"
            )),
            Some(_) => {}
        }
    }
    Ok(())
}

/// The package and the test target of the integration test `file`, `<folder>/<package>/tests/<target>.rs`, the
/// package named as its folder is; `None` for any other file.
fn test_target(file: &str) -> Option<(&str, &str)> {
    let parts: Vec<&str> = file.split('/').collect();
    let [_, package, "tests", name] = parts.as_slice() else { return None };
    Some((package, name.strip_suffix(".rs")?))
}

/// Whether a line of `workflow` runs `cargo test` on the test target `target` of the package `package`, its
/// ignored tests included.
fn runs_ignored(workflow: &str, package: &str, target: &str) -> bool {
    workflow.lines().any(|line| {
        let words: Vec<&str> = line.split_whitespace().collect();
        let given = |flag: &str, value: &str| words.windows(2).any(|pair| pair == [flag, value]);
        line.contains("cargo test")
            && given("-p", package)
            && given("--test", target)
            && words.iter().any(|w| *w == "--ignored" || *w == "--include-ignored")
    })
}

/// Whether `code` is retired: registered still, with "(retired)" at the end of its title, as codes are never
/// reused, and given by nothing (`docs/src/design/diagnostics.md`).
fn retired(code: Code) -> bool {
    code.title().ends_with("(retired)")
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
    matches!(case.spec, cases::Spec::Inkstitch { .. } | cases::Spec::CentreLines { .. } | cases::Spec::Fonts { .. })
}

/// What is wrong with the Ink/Stitch differential's cases: a design that isn't there, a sheet that isn't
/// drawn as a design, and a measure excused that isn't one, or by an entry the deviations ledger doesn't have.
fn check_differential(root: &Path, data: &[cases::DataCase], findings: &mut Findings) -> Result<(), String> {
    let ledger: BTreeSet<String> = crate::deviations::parse(&util::read(&root.join(crate::deviations::LEDGER))?)?.into_iter().map(|d| d.id).collect();
    let empty = BTreeMap::new();
    for case in data {
        let (subject, excused) = match &case.spec {
            cases::Spec::Inkstitch { subject, excused, .. } => (subject, excused),
            cases::Spec::CentreLines { subject } => (subject, &empty),
            _ => continue,
        };
        match subject {
            cases::Subject::File(design) if !root.join("conformance").join(design).is_file() => {
                findings.error(format!("{}: the design `{design}` is not in conformance/", case.file));
            }
            cases::Subject::Sheet(id) | cases::Subject::Part { sheet: id, .. }
                if !stitchcraft_engine::testsheets::find(id).is_some_and(|sheet| sheet.designed()) =>
            {
                findings.error(format!("{}: `{id}` is no test sheet drawn as a design", case.file));
            }
            cases::Subject::Letters(letters) => {
                if let Err(e) = centre_lines::drawn(letters) {
                    findings.error(format!("{}: {e}", case.file));
                }
            }
            cases::Subject::File(_) | cases::Subject::Sheet(_) | cases::Subject::Part { .. } => {}
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
    fn a_retired_code_says_so_at_the_end_of_its_title() {
        assert!(retired(Code::FillTartanHerringboneNotYet));
        assert!(!retired(Code::FillTartanTooFine));
    }

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
    fn a_models_only_case_is_in_a_test_target_that_a_step_of_the_models_job_runs_with_ignored() {
        assert_eq!(test_target("apps/stitchcraft-cutout/tests/models.rs"), Some(("stitchcraft-cutout", "models")));
        for elsewhere in ["apps/stitchcraft-cutout/src/run.rs", "apps/stitchcraft-cutout/tests/common/mod.rs", "tests/x.rs", "a/b/tests/x.txt"] {
            assert_eq!(test_target(elsewhere), None, "{elsewhere}");
        }
        let workflow = "      - run: cargo test --locked -p stitchcraft-cutout --test models -- --ignored --test-threads=1\n      - run: cargo test --locked -p stitchcraft-cli --test picture -- --include-ignored\n      - run: cargo test --locked -p stitchcraft-cutout --test photos\n";
        assert!(runs_ignored(workflow, "stitchcraft-cutout", "models"));
        assert!(runs_ignored(workflow, "stitchcraft-cli", "picture"));
        // Run without its ignored tests, on another package or another target, it isn't run.
        for (package, target) in [("stitchcraft-cutout", "photos"), ("stitchcraft-cli", "models"), ("stitchcraft-cutout", "run")] {
            assert!(!runs_ignored(workflow, package, target), "{package} {target}");
        }
    }

    #[test]
    fn each_models_only_case_of_the_workspace_has_its_step_in_the_models_job() {
        let (root, mut findings) = (util::root(), Findings::default());
        let rust = cases::discover_rust_cases(&root, &mut findings);
        assert!(rust.iter().any(|case| case.models_only.is_some()), "the cut-out's models' cases");
        check_models_only(&root, &rust, &mut findings).unwrap();
        assert!(findings.errors.is_empty(), "{:?}", findings.errors);
        // Written elsewhere, or with no step to run it, a models-only case is found out.
        let case = |file: &str| cases::RustCase {
            name: "req_cli_019_x".into(),
            covers: "REQ-CLI-019".into(),
            file: file.into(),
            models_only: Some(format!("{}the models job runs it", cases::MODELS_ONLY)),
        };
        check_models_only(&root, &[case("apps/stitchcraft-cutout/src/run.rs"), case("apps/stitchcraft-cutout/tests/nowhere.rs")], &mut findings)
            .unwrap();
        assert_eq!(findings.errors.len(), 2, "{:?}", findings.errors);
        assert!(findings.errors[0].contains("belong in a test file of their own"), "{}", findings.errors[0]);
        assert!(findings.errors[1].contains("--test nowhere"), "{}", findings.errors[1]);
    }

    #[test]
    fn the_committed_requirements_parse() {
        let ids = requirement_ids().unwrap();
        assert!(ids.contains("REQ-PLAN-001"));
    }
}
