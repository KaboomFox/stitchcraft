//! `cargo xtask coverage [--record]`: the coverage ratchet (`docs/src/design/guardrails.md`).
//!
//! Runs the whole test suite under cargo-llvm-cov and adds up line coverage per crate (test support,
//! the fuzz targets and xtask itself are left out: they are the measuring stick, not what is measured).
//! Each crate has a floor in `conformance/coverage.toml`; coverage below it fails. `--record` raises every floor to
//! today's coverage rounded down to a whole percent, and never lowers one. Lowering a floor is a
//! hand edit, in a pull request that says why. In GitHub Actions the table also goes to the job summary,
//! and the numbers to one annotation.
//!
//! Coverage runs the suite a second time, instrumented, so it has its own CI job instead of being a step
//! of `cargo xtask ci`.
//!
//! A crate below its floor gets what no test runs listed as warnings, which show on the pull request's page
//! without the job's log: its untested lines, a warning per file on the first of them, from the run's lcov
//! report, and the functions and closures no test calls, from its JSON export. Line coverage adds up
//! function by function, so a closure that never runs is an untested line even where the line around it
//! runs, and only the second list shows it.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::path::Path;

use serde::Deserialize;

use crate::util::{self, Findings};

const FLOORS: &str = "conformance/coverage.toml";
const REPORT: &str = "target/coverage/summary.json";
/// The same run's reports, line by line and function by function, written when a crate is below its floor.
const LCOV: &str = "target/coverage/lcov.info";
const FUNCTIONS: &str = "target/coverage/functions.json";
/// Files that are not measured.
const IGNORED: &str = "(^|/)(xtask|fuzz|crates/stitchcraft-testkit)/";

#[derive(Deserialize, Default)]
struct FloorsFile {
    #[serde(default)]
    floor: BTreeMap<String, u32>,
}

/// Covered and total lines.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Lines {
    /// Lines run by at least one test.
    pub covered: u64,
    /// Instrumented lines.
    pub count: u64,
}

impl Lines {
    /// Percent covered; 100 for a crate without instrumented lines.
    pub fn percent(self) -> f64 {
        // Line counts are far below 2^53, so the conversions are exact.
        #[allow(clippy::cast_precision_loss)]
        let percent = if self.count == 0 { 100.0 } else { self.covered as f64 * 100.0 / self.count as f64 };
        percent
    }
}

/// `cargo xtask coverage`.
pub fn run(record: bool) -> Result<(), String> {
    let root = util::root();
    let report_dir = root.join(REPORT).parent().map(Path::to_path_buf).unwrap_or_else(|| root.clone());
    std::fs::create_dir_all(&report_dir).map_err(|e| format!("{}: {e}", report_dir.display()))?;
    let mut measure = util::cargo();
    measure.arg("llvm-cov").args(util::packages()?.args()).args(["--locked", "--json", "--summary-only", "--output-path", REPORT]);
    measure.args(["--ignore-filename-regex", IGNORED]);
    util::run(measure, "cargo llvm-cov (is cargo-llvm-cov installed, with `rustup component add llvm-tools`?)")?;
    let measured = per_crate(&util::read(&root.join(REPORT))?)?;
    let path = root.join(FLOORS);
    let mut floors: FloorsFile = toml::from_str(&util::read(&path)?).map_err(|e| format!("{FLOORS}: {e}"))?;

    let mut findings = Findings::default();
    let mut table = String::from("| Crate | Lines | Covered | Coverage | Floor |\n|---|---:|---:|---:|---:|\n");
    let mut numbers = Vec::new();
    let mut below = BTreeSet::new();
    for (crate_name, lines) in &measured {
        let percent = lines.percent();
        // Shown rounded down, like the floors, so 95.96 % reads 95.9 %, not a floor-passing 96.0 %.
        let shown_percent = (percent * 10.0).floor() / 10.0;
        let floor = floors.floor.get(crate_name).copied();
        let shown = floor.map_or_else(|| "none".to_string(), |f| format!("{f} %"));
        let _ = writeln!(table, "| `{crate_name}` | {} | {} | {shown_percent:.1} % | {shown} |", lines.count, lines.covered);
        numbers.push(format!("{crate_name} {shown_percent:.1}"));
        match floor {
            None if !record => findings.error(format!("{crate_name} has no floor in {FLOORS}: run `cargo xtask coverage --record`")),
            Some(f) if !record && percent < f64::from(f) => {
                below.insert(crate_name.clone());
                findings.error(format!(
                    "{crate_name}: line coverage {shown_percent:.1} % is below its floor of {f} %; add tests for the new code \
                     (or lower the floor in {FLOORS}, saying why in the pull request)"
                ));
            }
            _ => {}
        }
        if record {
            // Rounded down, so the floor holds on every platform; never lowered.
            #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
            let today = percent.floor() as u32;
            let entry = floors.floor.entry(crate_name.clone()).or_insert(today);
            *entry = (*entry).max(today);
        }
    }
    if !below.is_empty() {
        match untested(&root, &below) {
            Ok(found) if found.is_empty() => findings.warn("the reports list nothing untested in the crates below their floors"),
            Ok(found) => found.into_iter().for_each(|warning| findings.warn(warning)),
            Err(e) => findings.warn(format!("the untested lines could not be listed: {e}")),
        }
    }
    for crate_name in floors.floor.keys().filter(|name| !measured.contains_key(*name)) {
        findings.error(format!("{FLOORS} has a floor for {crate_name}, which was not measured: remove it"));
    }
    println!("{table}");
    if std::env::var_os("GITHUB_ACTIONS").is_some() {
        println!("{}", util::annotation("notice", "line coverage", &numbers.join(", ")));
        if let Some(summary) = std::env::var_os("GITHUB_STEP_SUMMARY") {
            let mut text = std::fs::read_to_string(&summary).unwrap_or_default();
            text.push_str(&format!("## Line coverage\n\n{table}"));
            let _ = std::fs::write(&summary, text);
        }
    }
    if record {
        std::fs::write(&path, render(&floors.floor)).map_err(|e| format!("{FLOORS}: {e}"))?;
        println!("recorded the floors in {FLOORS}");
    }
    findings.finish("coverage", &format!("{} crates at or above their floors", measured.len()))
}

/// Line counts per crate from cargo-llvm-cov's JSON summary.
fn per_crate(json: &str) -> Result<BTreeMap<String, Lines>, String> {
    let report: serde_json::Value = serde_json::from_str(json).map_err(|e| format!("{REPORT}: {e}"))?;
    let files = report.pointer("/data/0/files").and_then(serde_json::Value::as_array).ok_or(format!("{REPORT}: no files"))?;
    let mut out: BTreeMap<String, Lines> = BTreeMap::new();
    for file in files {
        let name = file.get("filename").and_then(serde_json::Value::as_str).unwrap_or_default();
        let Some(crate_name) = crate_of(Path::new(name)) else { continue };
        let lines = file.pointer("/summary/lines");
        let field = |key: &str| lines.and_then(|l| l.get(key)).and_then(serde_json::Value::as_u64).unwrap_or(0);
        let entry = out.entry(crate_name).or_default();
        entry.covered += field("covered");
        entry.count += field("count");
    }
    Ok(out)
}

/// What no test runs in the files of `crates`, as warnings on the lines in question, from the last run's
/// reports. The line counts add up function by function, so a closure no test calls is an untested line
/// even when the line around it runs: the lcov report merges the two and misses it, and the functions
/// no test calls are listed besides.
fn untested(root: &Path, crates: &BTreeSet<String>) -> Result<Vec<String>, String> {
    let mut warnings = Vec::new();
    for (path, lines) in missed(&report(root, "--lcov", LCOV)?, root, crates) {
        let first = lines.first().copied().unwrap_or(1);
        warnings.push(format!("{path}:{first}: untested lines {}", spans(&lines)));
    }
    for (path, line) in never_called(&report(root, "--json", FUNCTIONS)?, root, crates)? {
        warnings.push(format!("{path}:{line}: no test calls the function or closure that starts here"));
    }
    Ok(warnings)
}

/// The last run's report in `format`, written to `output` and read back.
fn report(root: &Path, format: &str, output: &str) -> Result<String, String> {
    let mut report = util::cargo();
    report.args(["llvm-cov", "report", format, "--output-path", output, "--ignore-filename-regex", IGNORED]);
    util::run(report, "cargo llvm-cov report")?;
    util::read(&root.join(output))
}

/// The functions and closures in the files of `crates` that the JSON export `json` counts as called 0
/// times in every instantiation, each as its file's path from `root` and its first line.
fn never_called(json: &str, root: &Path, crates: &BTreeSet<String>) -> Result<BTreeSet<(String, u64)>, String> {
    let report: serde_json::Value = serde_json::from_str(json).map_err(|e| format!("{FUNCTIONS}: {e}"))?;
    let functions = report.pointer("/data/0/functions").and_then(serde_json::Value::as_array).ok_or(format!("{FUNCTIONS}: no functions"))?;
    // Each instantiation of a generic function is a function of its own: (path, first line) -> called.
    let mut called: BTreeMap<(String, u64), bool> = BTreeMap::new();
    for function in functions {
        let Some(path) = function.pointer("/filenames/0").and_then(serde_json::Value::as_str).map(Path::new) else { continue };
        let Some(line) = function.pointer("/regions/0/0").and_then(serde_json::Value::as_u64) else { continue };
        if crate_of(path).is_none_or(|name| !crates.contains(&name)) {
            continue;
        }
        let shown = path.strip_prefix(root).unwrap_or(path).to_string_lossy().replace('\\', "/");
        let count = function.get("count").and_then(serde_json::Value::as_u64).unwrap_or(0);
        *called.entry((shown, line)).or_default() |= count > 0;
    }
    Ok(called.into_iter().filter(|(_, called)| !called).map(|(at, _)| at).collect())
}

/// The lines lcov `text` counts as run 0 times, in the files of `crates`.
fn missed(text: &str, root: &Path, crates: &BTreeSet<String>) -> Vec<(String, Vec<u64>)> {
    let mut out = Vec::new();
    let mut file: Option<(String, Vec<u64>)> = None;
    for line in text.lines() {
        if let Some(path) = line.strip_prefix("SF:") {
            let path = Path::new(path);
            let shown = path.strip_prefix(root).unwrap_or(path).to_string_lossy().replace('\\', "/");
            file = crate_of(path).filter(|name| crates.contains(name)).map(|_| (shown, Vec::new()));
        } else if let (Some((_, lines)), Some(data)) = (file.as_mut(), line.strip_prefix("DA:")) {
            let mut fields = data.split(',').map(str::parse::<u64>);
            if let (Some(Ok(number)), Some(Ok(0))) = (fields.next(), fields.next()) {
                lines.push(number);
            }
        } else if line == "end_of_record"
            && let Some(done) = file.take().filter(|(_, lines)| !lines.is_empty())
        {
            out.push(done);
        }
    }
    out
}

/// Line numbers in order as runs: `31-33, 40`.
fn spans(lines: &[u64]) -> String {
    let mut runs: Vec<(u64, u64)> = Vec::new();
    for &line in lines {
        match runs.last_mut() {
            Some((_, end)) if *end + 1 == line => *end = line,
            _ => runs.push((line, line)),
        }
    }
    runs.iter().map(|&(start, end)| if start == end { start.to_string() } else { format!("{start}-{end}") }).collect::<Vec<_>>().join(", ")
}

/// The crate a source file belongs to: the directory after `crates/` or `apps/`.
fn crate_of(path: &Path) -> Option<String> {
    let parts: Vec<_> = path.components().map(|c| c.as_os_str().to_string_lossy().into_owned()).collect();
    parts.windows(2).rev().find(|w| w[0] == "crates" || w[0] == "apps").map(|w| w[1].clone())
}

fn render(floors: &BTreeMap<String, u32>) -> String {
    let mut out = String::from(
        "# Line-coverage floors per crate, in percent (`cargo xtask coverage`, docs/src/design/guardrails.md).\n\
         # CI fails when a crate's coverage drops below its floor. `cargo xtask coverage --record` raises the\n\
         # floors to today's coverage, rounded down, and never lowers one: lower a floor only by hand, in a\n\
         # pull request that says why.\n\n[floor]\n",
    );
    for (name, floor) in floors {
        let _ = writeln!(out, "{name} = {floor}");
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn files_add_up_per_crate() {
        let json = r#"{"data":[{"files":[
            {"filename":"/w/crates/stitchcraft-core/src/a.rs","summary":{"lines":{"count":10,"covered":9}}},
            {"filename":"/w/crates/stitchcraft-core/src/b.rs","summary":{"lines":{"count":30,"covered":21}}},
            {"filename":"/w/apps/stitchcraft-cli/src/main.rs","summary":{"lines":{"count":4,"covered":0}}},
            {"filename":"/rustc/library/core/src/x.rs","summary":{"lines":{"count":7,"covered":7}}}
        ]}]}"#;
        let measured = per_crate(json).unwrap();
        assert_eq!(measured.len(), 2);
        assert_eq!(measured["stitchcraft-core"], Lines { covered: 30, count: 40 });
        assert_eq!(measured["stitchcraft-core"].percent(), 75.0);
        assert_eq!(measured["stitchcraft-cli"].percent(), 0.0);
        assert_eq!(Lines::default().percent(), 100.0);
    }

    #[test]
    fn functions_no_instantiation_calls_are_listed() {
        let json = r#"{"data":[{"functions":[
            {"count":0,"filenames":["/w/crates/stitchcraft-engine/src/a.rs"],"regions":[[42,70,42,91,0,0,0,0]]},
            {"count":3,"filenames":["/w/crates/stitchcraft-engine/src/a.rs"],"regions":[[30,1,40,2,3,0,0,0]]},
            {"count":0,"filenames":["/w/crates/stitchcraft-engine/src/a.rs"],"regions":[[50,1,52,2,0,0,0,0]]},
            {"count":2,"filenames":["/w/crates/stitchcraft-engine/src/a.rs"],"regions":[[50,1,52,2,2,0,0,0]]},
            {"count":0,"filenames":["/w/crates/stitchcraft-core/src/b.rs"],"regions":[[7,1,9,2,0,0,0,0]]}
        ]}]}"#;
        let engine = BTreeSet::from(["stitchcraft-engine".to_string()]);
        let found = never_called(json, Path::new("/w"), &engine).unwrap();
        assert_eq!(found, BTreeSet::from([("crates/stitchcraft-engine/src/a.rs".to_string(), 42)]), "a generic one called once is called");
        assert!(never_called("{}", Path::new("/w"), &engine).is_err());
    }

    #[test]
    fn untested_lines_are_listed_for_the_crates_asked() {
        let lcov = "SF:/w/crates/stitchcraft-engine/src/a.rs\nDA:3,1\nDA:4,0\nDA:5,0\nDA:6,0\nDA:9,0\nDA:10,2\nend_of_record\n\
                    SF:/w/crates/stitchcraft-engine/src/b.rs\nDA:1,5\nend_of_record\n\
                    SF:/w/crates/stitchcraft-core/src/c.rs\nDA:7,0\nend_of_record\n";
        let engine = BTreeSet::from(["stitchcraft-engine".to_string()]);
        let missed = missed(lcov, Path::new("/w"), &engine);
        assert_eq!(missed, [("crates/stitchcraft-engine/src/a.rs".to_string(), vec![4, 5, 6, 9])]);
        assert_eq!(spans(&missed[0].1), "4-6, 9");
        assert_eq!(spans(&[]), "");
    }

    #[test]
    fn floors_render_sorted_with_their_rules() {
        let floors = BTreeMap::from([("b".to_string(), 80), ("a".to_string(), 91)]);
        let text = render(&floors);
        assert!(text.ends_with("[floor]\na = 91\nb = 80\n"));
        let parsed: FloorsFile = toml::from_str(&text).unwrap();
        assert_eq!(parsed.floor, floors);
    }
}
