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
//! without the job's log, in the files the summary measures: its untested lines, a warning per file on the
//! first of them, from the run's lcov report, and from its JSON export the functions and closures no test
//! calls, and the lines the summary counts untested though the lcov report counts them run.
//!
//! The last two lists are there because line coverage adds up function by function, and the lcov report
//! merges them line by line. A function has a build for each test binary kind, the unit tests' and the one
//! the integration tests link, and one per type a generic function is used with, and a closure is a
//! function of its own. For each function, the summary counts the lines of the build that runs the most of
//! them ([`lines`] says which lines a build has, and which it runs). So a closure that never runs is an
//! untested line even where the line around it runs. And the lcov report can count a line run that the
//! summary counts untested: the function's code on it runs only in another build, or none of it runs and a
//! closure on the line does, as with a `?` after a closure argument whose error no test brings.

use std::cmp::Reverse;
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
    util::run(
        measure,
        "cargo llvm-cov (is cargo-llvm-cov installed, with `rustup component add llvm-tools` or with LLVM_COV and \
         LLVM_PROFDATA set, as docs/src/design/guardrails.md says?)",
    )?;
    let files = summary_files(&util::read(&root.join(REPORT))?)?;
    let measured = per_crate(&files);
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
        let paths = files.into_iter().filter(|file| below.contains(&file.crate_name)).map(|file| file.path).collect();
        match untested(&root, &paths) {
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

/// A file the summary measures: its path as the reports give it, its crate, and its line counts.
#[derive(Clone, Debug, PartialEq)]
struct Measured {
    path: String,
    crate_name: String,
    lines: Lines,
}

/// The files cargo-llvm-cov's JSON summary measures, in the crates of this repository.
fn summary_files(json: &str) -> Result<Vec<Measured>, String> {
    let report: serde_json::Value = serde_json::from_str(json).map_err(|e| format!("{REPORT}: {e}"))?;
    let files = report.pointer("/data/0/files").and_then(serde_json::Value::as_array).ok_or(format!("{REPORT}: no files"))?;
    let measured = files.iter().filter_map(|file| {
        let path = file.get("filename").and_then(serde_json::Value::as_str)?;
        let lines = file.pointer("/summary/lines");
        let field = |key: &str| lines.and_then(|l| l.get(key)).and_then(serde_json::Value::as_u64).unwrap_or(0);
        Some(Measured {
            path: path.to_string(),
            crate_name: crate_of(Path::new(path))?,
            lines: Lines { covered: field("covered"), count: field("count") },
        })
    });
    Ok(measured.collect())
}

/// Line counts per crate.
fn per_crate(files: &[Measured]) -> BTreeMap<String, Lines> {
    let mut out: BTreeMap<String, Lines> = BTreeMap::new();
    for file in files {
        let entry = out.entry(file.crate_name.clone()).or_default();
        entry.covered += file.lines.covered;
        entry.count += file.lines.count;
    }
    out
}

/// What no test runs in the files `paths` names, as warnings on the lines in question, from the last run's
/// reports: the lines the lcov report merges to untested, then what only the function by function counts
/// miss (see the module's documentation).
fn untested(root: &Path, paths: &BTreeSet<String>) -> Result<Vec<String>, String> {
    let shown = |path: &str| Path::new(path).strip_prefix(root).unwrap_or(Path::new(path)).to_string_lossy().replace('\\', "/");
    let mut warnings = Vec::new();
    let missed = missed(&report(root, "--lcov", LCOV)?, paths);
    for (path, lines) in &missed {
        let first = lines.first().copied().unwrap_or(1);
        warnings.push(format!("{}:{first}: untested lines {}", shown(path), spans(lines)));
    }
    let Builds { never_called, lost } = builds(&report(root, "--json", FUNCTIONS)?, paths, &missed)?;
    for (path, line) in never_called {
        warnings.push(format!("{}:{line}: no test calls the function or closure that starts here", shown(&path)));
    }
    for ((path, start), lines) in lost {
        let first = lines.first().copied().unwrap_or(start);
        warnings.push(format!(
            "{}:{first}: coverage counts lines {} untested in the function that starts at line {start}, whose build that \
             runs the most does not run its code there. Another build runs it, or a closure on the line runs, so the lcov \
             report counts them run. Test that code from the kind of test, unit or integration, that runs most of the \
             function",
            shown(&path),
            spans(&lines)
        ));
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

/// Functions and closures by their builds, each by its file's path and its first line.
#[derive(Debug, Default, PartialEq)]
struct Builds {
    /// No build calls them.
    never_called: BTreeSet<(String, u64)>,
    /// The lines each counts as untested that the lcov report counts run.
    lost: BTreeMap<(String, u64), Vec<u64>>,
}

/// The functions and closures in the files `paths` names that no build calls, and the lines the others
/// count as untested beyond `missed`, the lines the lcov report counts untested, from the JSON export
/// `json`. The export lists each build of a function apart (the unit tests' and the integration tests', and
/// each instantiation of a generic function): the builds of one function start at the same place.
fn builds(json: &str, paths: &BTreeSet<String>, missed: &BTreeMap<String, Vec<u64>>) -> Result<Builds, String> {
    let report: serde_json::Value = serde_json::from_str(json).map_err(|e| format!("{FUNCTIONS}: {e}"))?;
    let functions = report.pointer("/data/0/functions").and_then(serde_json::Value::as_array).ok_or(format!("{FUNCTIONS}: no functions"))?;
    // (path, first line, first column) -> each build's lines, and whether each ran.
    let mut grouped: BTreeMap<(String, u64, u64), Vec<BTreeMap<u64, bool>>> = BTreeMap::new();
    for function in functions {
        let Some(path) = function.pointer("/filenames/0").and_then(serde_json::Value::as_str).filter(|path| paths.contains(*path)) else { continue };
        let regions: Vec<Region> =
            function.get("regions").and_then(serde_json::Value::as_array).into_iter().flatten().filter_map(Region::of).collect();
        let Some(first) = regions.first() else { continue };
        grouped.entry((path.to_string(), first.start.0, first.start.1)).or_default().push(lines(&regions));
    }
    let ran = |lines: &BTreeMap<u64, bool>| lines.values().filter(|ran| **ran).count();
    let mut builds = Builds::default();
    for ((path, start, _), runs) in grouped {
        let Some(best) = runs.iter().max_by_key(|lines| ran(lines)) else { continue };
        if ran(best) == 0 {
            builds.never_called.insert((path, start));
            continue;
        }
        let untested = missed.get(&path);
        let lost: BTreeSet<u64> = runs
            .iter()
            .flat_map(BTreeMap::keys)
            .copied()
            .filter(|line| best.get(line) != Some(&true) && untested.is_none_or(|lines| !lines.contains(line)))
            .collect();
        if !lost.is_empty() {
            builds.lost.insert((path, start), lost.into_iter().collect());
        }
    }
    Ok(builds)
}

/// A region of one build of a function: where it starts and where it ends, as line and column, and how
/// many times it ran.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Region {
    start: (u64, u64),
    end: (u64, u64),
    count: u64,
}

impl Region {
    /// A region of the JSON export, `[line, column, end line, end column, count, file, expanded file, kind]`,
    /// when it is code (kind 0) in the function's own file (file 0), as rustc's regions are.
    fn of(value: &serde_json::Value) -> Option<Region> {
        let field = |i: usize| value.get(i).and_then(serde_json::Value::as_u64);
        let region = Region { start: (field(0)?, field(1)?), end: (field(2)?, field(3)?), count: field(4)? };
        (field(5)? == 0 && field(7)? == 0).then_some(region)
    }
}

/// The lines one build of a function has, from its `regions`, and whether it runs each, as llvm-cov's
/// summary counts them: a line is the build's when a region starts on it or is open at its start, and run
/// when one of those ran, the innermost of the regions open at its start standing for all of them. A
/// region that never runs inside one that does leaves its first line run: the line starts in the outer one.
fn lines(regions: &[Region]) -> BTreeMap<u64, bool> {
    let mut out = BTreeMap::new();
    let (Some(first), Some(last)) = (regions.iter().map(|r| r.start.0).min(), regions.iter().map(|r| r.end.0).max()) else {
        return out;
    };
    for line in first..=last {
        // The innermost region open at the line's start: the last to start before it, of those the first to
        // end. Regions over the same code count as one, which ran when one of them did.
        let innermost = regions.iter().filter(|r| r.start.0 < line && r.end > (line, 1)).map(|r| (r.start, Reverse(r.end))).max();
        let mut counted = regions.iter().filter(|r| r.start.0 == line || innermost == Some((r.start, Reverse(r.end)))).peekable();
        if counted.peek().is_some() {
            out.insert(line, counted.any(|r| r.count > 0));
        }
    }
    out
}

/// The lines lcov `text` counts as run 0 times, in the files `paths` names.
fn missed(text: &str, paths: &BTreeSet<String>) -> BTreeMap<String, Vec<u64>> {
    let mut out: BTreeMap<String, Vec<u64>> = BTreeMap::new();
    let mut file = None;
    for line in text.lines() {
        if let Some(path) = line.strip_prefix("SF:") {
            file = paths.contains(path).then_some(path);
        } else if let (Some(path), Some(data)) = (file, line.strip_prefix("DA:")) {
            let mut fields = data.split(',').map(str::parse::<u64>);
            if let (Some(Ok(number)), Some(Ok(0))) = (fields.next(), fields.next()) {
                out.entry(path.to_string()).or_default().push(number);
            }
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
        let files = summary_files(json).unwrap();
        assert_eq!(
            files.iter().map(|file| file.path.as_str()).collect::<Vec<_>>(),
            ["/w/crates/stitchcraft-core/src/a.rs", "/w/crates/stitchcraft-core/src/b.rs", "/w/apps/stitchcraft-cli/src/main.rs"]
        );
        let measured = per_crate(&files);
        assert_eq!(measured.len(), 2);
        assert_eq!(measured["stitchcraft-core"], Lines { covered: 30, count: 40 });
        assert_eq!(measured["stitchcraft-core"].percent(), 75.0);
        assert_eq!(measured["stitchcraft-cli"].percent(), 0.0);
        assert_eq!(Lines::default().percent(), 100.0);
        assert!(summary_files("{}").is_err());
    }

    #[test]
    fn lines_are_counted_as_llvm_cov_counts_them() {
        let r = |start: (u64, u64), end: (u64, u64), count: u64| Region { start, end, count };
        // A body that ran from line 10 to 13. In it, a block from 11 to 12 that did not: line 11 starts in
        // the body, line 12 in the block. On 14, after the body, a `?` that never ran, and on 16, 2 regions
        // over the same code, one of which ran. Line 15 has no code.
        let regions = [r((10, 1), (13, 2), 3), r((11, 5), (12, 6), 0), r((14, 11), (14, 12), 0), r((16, 5), (16, 9), 0), r((16, 5), (16, 9), 1)];
        assert_eq!(lines(&regions), BTreeMap::from([(10, true), (11, true), (12, false), (13, true), (14, false), (16, true)]));
        // Of 2 regions open at a line's start, the inner one counts, and of those starting at one place the
        // one ending first is the inner one: lines 21 and 22 start in the block that did not run.
        let nested = lines(&[r((20, 1), (24, 2), 1), r((20, 1), (22, 2), 0), r((23, 1), (23, 9), 0)]);
        assert_eq!([21, 22, 23, 24].map(|line| nested.get(&line).copied()), [Some(false), Some(false), Some(true), Some(true)]);
        assert!(lines(&[]).is_empty());
        let json = |region: &str| serde_json::from_str::<serde_json::Value>(region).unwrap();
        assert_eq!(Region::of(&json("[4,5,6,7,8,0,0,0]")), Some(r((4, 5), (6, 7), 8)));
        // Another file's code, a region of another kind, and something else are left out.
        assert_eq!(Region::of(&json("[4,5,6,7,8,1,0,0]")), None);
        assert_eq!(Region::of(&json("[4,5,6,7,8,0,0,2]")), None);
        assert_eq!(Region::of(&json("[4,5]")), None);
    }

    #[test]
    fn functions_no_build_calls_and_lines_the_best_build_misses_are_listed() {
        let json = r#"{"data":[{"functions":[
            {"filenames":["/w/crates/stitchcraft-engine/src/a.rs"],"regions":[[42,70,42,91,0,0,0,0]]},
            {"filenames":["/w/crates/stitchcraft-engine/src/a.rs"],"regions":[[42,20,42,40,1,0,0,0]]},
            {"filenames":["/w/crates/stitchcraft-engine/src/a.rs"],"regions":[[30,1,40,2,3,0,0,0],[33,5,34,6,0,0,0,0]]},
            {"filenames":["/w/crates/stitchcraft-engine/src/a.rs"],"regions":[[30,1,40,2,1,0,0,0],[33,5,34,6,1,0,0,0]]},
            {"filenames":["/w/crates/stitchcraft-engine/src/a.rs"],"regions":[[50,1,60,2,2,0,0,0],[52,5,53,6,1,0,0,0],[55,5,56,6,0,0,0,0]]},
            {"filenames":["/w/crates/stitchcraft-engine/src/a.rs"],"regions":[[50,1,60,2,4,0,0,0],[52,5,53,6,0,0,0,0],[55,5,56,6,4,0,0,0]]},
            {"filenames":["/w/crates/stitchcraft-engine/src/a.rs"],"regions":[[62,1,64,2,2,0,0,0],[63,5,63,9,1,0,0,0],[63,20,63,21,0,0,0,0]]},
            {"filenames":["/w/crates/stitchcraft-engine/src/a.rs"],"regions":[[62,1,64,2,2,0,0,0],[63,5,63,9,1,0,0,0],[63,20,63,21,1,0,0,0]]},
            {"filenames":["/w/crates/stitchcraft-engine/src/a.rs"],"regions":[[70,1,72,2,0,0,0,0]]},
            {"filenames":["/w/crates/stitchcraft-engine/src/a.rs"],"regions":[[70,1,72,2,2,0,0,0]]},
            {"filenames":["/w/crates/stitchcraft-engine/src/a.rs"],"regions":[[80,1,82,2,5,0,0,0]]},
            {"filenames":["/w/crates/stitchcraft-engine/src/a.rs"],"regions":[]},
            {"filenames":["/w/crates/stitchcraft-engine/src/a.rs"],"regions":[[90,1,90,20,2,0,0,0],[91,9,91,29,2,0,0,0],[93,11,93,12,0,0,0,0],[94,9,94,15,2,0,0,0]]},
            {"filenames":["/w/crates/stitchcraft-engine/src/a.rs"],"regions":[[91,30,93,10,5,0,0,0]]},
            {"filenames":["/w/crates/stitchcraft-engine/src/a.rs"],"regions":[[100,1,100,20,1,0,0,0],[101,5,101,9,0,0,0,0]]},
            {"filenames":["/w/crates/stitchcraft-engine/tests/c.rs"],"regions":[[7,1,9,2,0,0,0,0]]},
            {"filenames":["/w/crates/stitchcraft-core/src/b.rs"],"regions":[[7,1,9,2,0,0,0,0]]}
        ]}]}"#;
        let a = "/w/crates/stitchcraft-engine/src/a.rs".to_string();
        let paths = BTreeSet::from([a.clone()]);
        let missed = BTreeMap::from([(a.clone(), vec![101])]);
        let found = builds(json, &paths, &missed).unwrap();
        let at = |line: u64| (a.clone(), line);
        // Line 42: a closure no build calls, beside one that runs. Line 30: one build runs all the other
        // does. Line 50: each build runs a line the other does not, and the second, as good as the first,
        // misses 53. Line 62: each build runs a region the other does not, on a line both run. Line 70: one
        // build of two runs. Line 80: a function with one build, which runs. Line 90: a `?` after a closure
        // argument, from 91 to 93, that never meets its error: the lcov report counts line 93 run, since the
        // closure runs there. Line 101: a line no build runs, which the lcov report lists. Files the summary
        // does not measure are left out.
        assert_eq!(found, Builds { never_called: BTreeSet::from([at(42)]), lost: BTreeMap::from([(at(50), vec![53]), (at(90), vec![93])]) });
        assert!(builds("{}", &paths, &missed).is_err());
    }

    #[test]
    fn untested_lines_are_listed_for_the_files_asked() {
        let lcov = "SF:/w/crates/stitchcraft-engine/src/a.rs\nDA:3,1\nDA:4,0\nDA:5,0\nDA:6,0\nDA:9,0\nDA:10,2\nend_of_record\n\
                    SF:/w/crates/stitchcraft-engine/src/b.rs\nDA:1,5\nend_of_record\n\
                    SF:/w/crates/stitchcraft-core/src/c.rs\nDA:7,0\nend_of_record\n";
        let a = "/w/crates/stitchcraft-engine/src/a.rs".to_string();
        let paths = BTreeSet::from([a.clone(), "/w/crates/stitchcraft-engine/src/b.rs".to_string()]);
        let missed = missed(lcov, &paths);
        assert_eq!(missed, BTreeMap::from([(a.clone(), vec![4, 5, 6, 9])]));
        assert_eq!(spans(&missed[&a]), "4-6, 9");
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
