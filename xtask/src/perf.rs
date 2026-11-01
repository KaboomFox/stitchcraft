//! `cargo xtask perf [--record]`: the performance budget (`docs/src/design/guardrails.md`, NFR-PERF-1 and
//! NFR-PERF-3).
//!
//! Each benchmark is an example of the engine's, built in release ([`BENCHES`]):
//!
//! - **`field`** plans the reference hoop's whole field, a 130 × 180 mm tatami fill at 0.4 mm
//!   (`REQ-FILL-TAT-009`). NFR-PERF-1 gives it 50 ms.
//! - **`insets`** shrinks a region 150 mm across, of 1,000 points, 300 times, each inset from the last, as
//!   contour fill will. It does so for 4 regions: round or with 7 waves, by 0.25 mm and by 0.4 mm.
//!   NFR-PERF-3, the third criterion of ADR-0005, gives each 200 ms, which the offsetter meets since M7.6.
//!
//! Each is measured 2 ways:
//!
//! - **Instructions**, the guard. Valgrind's callgrind counts the instructions one run takes, in the
//!   example's counted function and everything it calls. The count does not depend on how fast the machine
//!   is or what else it runs, so a change that makes the work slower fails every time and nothing else
//!   does. A count above the record in `conformance/perf.toml` by more than 2 % fails: the C library's
//!   allocator and copies differ a little from one system to the next. `--record` lowers a record to
//!   today's count, rounded up to a million, and never raises it; raising it is a hand edit, in a pull
//!   request that says why. Without valgrind the counts are skipped, except in CI, which installs it.
//! - **The counted function.** Callgrind finds the function by its name, and counts 0, without a word,
//!   when no function has it. So each example defines it itself, never inlined, where a rename shows, and
//!   a count below half the record fails, recording or not: callgrind lost the function, or the work got
//!   so much faster that the record is lowered by hand.
//! - **Time**, the budget itself. The median of 21 runs, in milliseconds, the slowest of an example's
//!   runs where it times several, against its budget. How fast and how busy the machine is change it, so
//!   a time over the budget is a warning: the count is what holds a change to account.
//!
//! The release build and callgrind take minutes, so the budget has its own CI job instead of a step in
//! `cargo xtask ci`. In GitHub Actions the numbers go to the job summary and to an annotation.

use std::collections::BTreeMap;
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::Command;

use serde::Deserialize;

use crate::util::{self, Findings};

const RECORD: &str = "conformance/perf.toml";
/// How many timed runs the time is the median of.
const RUNS: usize = 21;
/// How far above the record a count may go, in percent.
const SLACK_PERCENT: u64 = 2;
/// Counts are recorded in whole millions, rounded up.
const MILLION: u64 = 1_000_000;

/// A benchmark the budget holds: an example of the engine's, its table in the record.
struct Bench {
    /// The example, and its table in the record.
    name: &'static str,
    /// What is timed, as the report says it.
    what: &'static str,
    /// The function whose instructions callgrind counts, with everything it calls: the example's own,
    /// named `<example>::<function>`.
    counted: &'static str,
    /// The record's comment on the time budget, a line at a time.
    budget: &'static [&'static str],
}

/// The benchmarks, in the order the record lists them.
const BENCHES: [Bench; 2] = [
    Bench {
        name: "field",
        what: "planning the hoop's whole field",
        counted: "field::plan",
        budget: &["The most the median of 21 plans may take natively, in milliseconds (NFR-PERF-1). Over it is a warning."],
    },
    Bench {
        name: "insets",
        what: "shrinking a region 150 mm across 300 times, the slowest of 4,",
        counted: "insets::insets",
        budget: &[
            "The most the median of 21 runs may take natively for the slowest of the 4 regions, in milliseconds",
            "(NFR-PERF-3, ADR-0005's third criterion). Over it is a warning.",
        ],
    },
];

impl Bench {
    /// Where a release build in `target` puts the example.
    fn binary(&self, target: &Path) -> PathBuf {
        target.join("release").join("examples").join(format!("{}{}", self.name, std::env::consts::EXE_SUFFIX))
    }

    /// The example's source, which defines the counted function.
    fn source(&self) -> String {
        format!("crates/stitchcraft-engine/examples/{}.rs", self.name)
    }
}

/// A benchmark's budget and its record: its table in `conformance/perf.toml`.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq)]
struct Record {
    /// The most a run may take natively, in milliseconds.
    budget_ms: f64,
    /// The instructions one run takes, as recorded.
    instructions: u64,
}

/// Where Cargo builds, given `CARGO_TARGET_DIR` (`set`): that folder when it is set, resolved against the
/// repository root, where the build runs; else the root's `target`. The gate once looked in the root's
/// `target` whatever the variable said, and found no example there, or an old one.
fn target_dir(root: &Path, set: Option<OsString>) -> PathBuf {
    set.filter(|dir| !dir.is_empty()).map_or_else(|| root.join("target"), |dir| root.join(dir))
}

/// `cargo xtask perf`.
pub fn run(record: bool) -> Result<(), String> {
    let root = util::root();
    let mut build = util::cargo();
    build.args(["build", "--release", "--locked", "--package", "stitchcraft-engine"]);
    for bench in &BENCHES {
        build.args(["--example", bench.name]);
    }
    util::run(build, "cargo build --release of the benchmarks")?;
    let path = root.join(RECORD);
    let mut records = toml::from_str::<BTreeMap<String, Record>>(&util::read(&path)?).map_err(|e| format!("{RECORD}: {e}"))?;
    let target = target_dir(&root, std::env::var_os("CARGO_TARGET_DIR"));
    let counting = util::tool_available("valgrind", &["--version"]);
    let mut findings = Findings::default();
    let mut report = Vec::new();
    let mut lowered = false;
    for bench in &BENCHES {
        let Some(kept) = records.get_mut(bench.name) else {
            findings.error(format!("{RECORD} has no `[{}]` table", bench.name));
            continue;
        };
        let binary = bench.binary(&target);
        let timed = output(Command::new(&binary).arg(RUNS.to_string()), bench.name)?;
        let slowest = medians(&timed).into_iter().reduce(f64::max).ok_or(format!("the {} example printed no median: {timed}", bench.name))?;
        report.push(format!("{} takes {slowest:.1} ms, the median of {RUNS}; the budget is {} ms", bench.what, kept.budget_ms));
        if slowest > kept.budget_ms {
            findings.warn(format!(
                "{} took {slowest:.1} ms, over the {} ms budget: a slow or busy machine takes longer, but see whether the \
                 instruction count grew",
                bench.what, kept.budget_ms
            ));
        }
        if !counting {
            continue;
        }
        let mut callgrind = Command::new("valgrind");
        callgrind.args(["--tool=callgrind", "--callgrind-out-file=/dev/null", &format!("--toggle-collect={}", bench.counted)]).arg(&binary).arg("0");
        let counted = errors_of(&mut callgrind, "valgrind --tool=callgrind")?;
        let instructions = number_after(&counted, "Collected : ").ok_or(format!("callgrind counted nothing: {counted}"))?;
        // Instruction counts are far below 2^53, so the conversion is exact.
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let instructions = instructions as u64;
        report.push(format!("one run of `{}` takes {instructions} instructions; the record is {}", bench.name, kept.instructions));
        match judged(*kept, instructions, record) {
            Judged::Within => {}
            Judged::Over(limit) => findings.error(format!(
                "one run of `{}` takes {instructions} instructions, more than the {limit} the record in {RECORD} allows: make it \
                 faster, or raise the record by hand, saying why in the pull request",
                bench.name
            )),
            Judged::Lowered(today) => {
                kept.instructions = today;
                lowered = true;
                report.push(format!("recorded {today} for {} in {RECORD}", bench.name));
            }
            Judged::Lost => findings.error(format!(
                "callgrind counted {instructions} instructions in `{}`, less than half the {} recorded in {RECORD}: it counts \
                 nothing once the function is renamed or inlined (keep it in {}), and if the work really got this much faster, \
                 lower the record by hand, saying why in the pull request",
                bench.counted,
                kept.instructions,
                bench.source()
            )),
        }
    }
    if lowered {
        std::fs::write(&path, render(&records)).map_err(|e| format!("{RECORD}: {e}"))?;
    }
    if !counting {
        if util::in_ci() {
            findings.error("valgrind is not installed: CI needs it to count the benchmarks' instructions");
        } else {
            findings.warn("valgrind is not installed, so the instructions were not counted (skipped)");
        }
    }

    for line in &report {
        println!("{line}");
    }
    if std::env::var_os("GITHUB_ACTIONS").is_some() {
        println!("{}", util::annotation("notice", "performance budget", &report.join("; ")));
        if let Some(summary) = std::env::var_os("GITHUB_STEP_SUMMARY") {
            let mut text = std::fs::read_to_string(&summary).unwrap_or_default();
            text.push_str(&format!("## Performance budget\n\n{}\n", report.iter().map(|line| format!("- {line}\n")).collect::<String>()));
            let _ = std::fs::write(&summary, text);
        }
    }
    findings.finish("perf", "the benchmarks run within their records")
}

/// What a count says against the record.
#[derive(Debug, PartialEq)]
enum Judged {
    /// Within the record and its slack, or recording and not below it.
    Within,
    /// Above the record by more than the slack: the most allowed.
    Over(u64),
    /// Recording, and below it: the new record, in whole millions.
    Lowered(u64),
    /// Below half the record, recording or not: callgrind no longer counts the benchmark's function.
    Lost,
}

/// `instructions` against `kept`'s record, recording when `record` is set.
fn judged(kept: Record, instructions: u64, record: bool) -> Judged {
    if instructions < kept.instructions / 2 {
        return Judged::Lost;
    }
    let today = instructions.div_ceil(MILLION).saturating_mul(MILLION);
    let limit = kept.instructions.saturating_add(kept.instructions * SLACK_PERCENT / 100);
    match (record, today < kept.instructions, instructions > limit) {
        (true, true, _) => Judged::Lowered(today),
        (false, _, true) => Judged::Over(limit),
        _ => Judged::Within,
    }
}

/// `conformance/perf.toml` with `records` in it, each benchmark's table in the order of [`BENCHES`].
fn render(records: &BTreeMap<String, Record>) -> String {
    let mut out = String::from(
        "# The performance budget (`cargo xtask perf`, docs/src/design/guardrails.md): planning the reference hoop's\n\
         # whole field, a 130 × 180 mm tatami fill at 0.4 mm (REQ-FILL-TAT-009, NFR-PERF-1), and shrinking a region\n\
         # 150 mm across, of 1,000 points, 300 times, as contour fill will (NFR-PERF-3).\n",
    );
    for bench in &BENCHES {
        let Some(kept) = records.get(bench.name) else { continue };
        out.push_str(&format!("\n[{}]\n", bench.name));
        for line in bench.budget {
            out.push_str(&format!("# {line}\n"));
        }
        out.push_str(&format!(
            "budget_ms = {:?}\n\
             # The instructions one run takes, counted by valgrind's callgrind. More than this plus 2 % fails.\n\
             # `cargo xtask perf --record` lowers it and never raises it: raise it by hand, in a pull request that\n\
             # says why.\n\
             instructions = {}\n",
            kept.budget_ms, kept.instructions
        ));
    }
    out
}

/// Every number written after `median ` in `text`: the medians an example prints, one for each thing it
/// times.
fn medians(text: &str) -> Vec<f64> {
    text.split("median ").skip(1).filter_map(leading_number).collect()
}

/// The number written after `label` in `text`.
fn number_after(text: &str, label: &str) -> Option<f64> {
    leading_number(text.split_once(label)?.1)
}

/// The number `text` starts with, up to the first character that is not part of one; commas between
/// thousands are left out.
fn leading_number(text: &str) -> Option<f64> {
    let digits: String = text.chars().take_while(|c| c.is_ascii_digit() || matches!(c, '.' | ',')).filter(|c| *c != ',').collect();
    digits.parse().ok()
}

/// What `cmd` prints on its standard output, run in the repository root; an error if it fails.
fn output(cmd: &mut Command, what: &str) -> Result<String, String> {
    let ran = cmd.current_dir(util::root()).output().map_err(|e| format!("{what}: {e}"))?;
    if ran.status.success() { Ok(String::from_utf8_lossy(&ran.stdout).into_owned()) } else { Err(format!("`{what}` failed ({})", ran.status)) }
}

/// What `cmd` prints on its standard error, where valgrind reports, run in the repository root; an error if
/// it fails.
fn errors_of(cmd: &mut Command, what: &str) -> Result<String, String> {
    let ran = cmd.current_dir(util::root()).output().map_err(|e| format!("{what}: {e}"))?;
    if ran.status.success() { Ok(String::from_utf8_lossy(&ran.stderr).into_owned()) } else { Err(format!("`{what}` failed ({})", ran.status)) }
}

#[cfg(test)]
mod tests {
    use super::*;

    const KEPT: Record = Record { budget_ms: 50.0, instructions: 280_000_000 };

    #[test]
    fn a_count_within_2_percent_of_the_record_passes_and_above_fails() {
        assert_eq!(judged(KEPT, 285_600_000, false), Judged::Within);
        assert_eq!(judged(KEPT, 285_600_001, false), Judged::Over(285_600_000));
        assert_eq!(judged(KEPT, 200_000_000, false), Judged::Within, "faster passes without recording");
    }

    #[test]
    fn a_count_of_0_or_below_half_the_record_fails() {
        // Callgrind counts 0 when the counted function is renamed, moved or inlined.
        for (count, record) in [(0, false), (0, true), (139_999_999, false), (139_999_999, true)] {
            assert_eq!(judged(KEPT, count, record), Judged::Lost, "{count}");
        }
        assert_eq!(judged(KEPT, 140_000_000, false), Judged::Within);
        assert_eq!(judged(KEPT, 140_000_000, true), Judged::Lowered(140_000_000));
    }

    #[test]
    fn each_counted_function_is_its_example_s_own_and_never_inlined() {
        for bench in &BENCHES {
            let name = bench.counted.strip_prefix(&format!("{}::", bench.name)).unwrap();
            let source = util::read(&util::root().join(bench.source())).unwrap().replace("\r\n", "\n");
            assert!(source.contains(&format!("#[inline(never)]\nfn {name}(")), "{} defines `{name}`, never inlined", bench.source());
        }
    }

    #[test]
    fn the_examples_are_where_cargo_builds_them() {
        let root = Path::new("/repo");
        assert_eq!(target_dir(root, None), Path::new("/repo/target"));
        assert_eq!(target_dir(root, Some("".into())), Path::new("/repo/target"));
        assert_eq!(target_dir(root, Some("/elsewhere".into())), Path::new("/elsewhere"));
        assert_eq!(target_dir(root, Some("build".into())), Path::new("/repo/build"));
        for bench in &BENCHES {
            let built = format!("/elsewhere/release/examples/{}{}", bench.name, std::env::consts::EXE_SUFFIX);
            assert_eq!(bench.binary(Path::new("/elsewhere")), Path::new(&built));
        }
    }

    #[test]
    fn recording_lowers_the_record_to_whole_millions_and_never_raises_it() {
        assert_eq!(judged(KEPT, 271_400_001, true), Judged::Lowered(272_000_000));
        assert_eq!(judged(KEPT, 279_000_000, true), Judged::Lowered(279_000_000));
        // Within the same million, or above the record, it stays.
        assert_eq!(judged(KEPT, 279_000_001, true), Judged::Within);
        assert_eq!(judged(KEPT, 290_000_000, true), Judged::Within);
    }

    #[test]
    fn the_numbers_are_read_from_what_the_tools_print() {
        assert_eq!(number_after("median 30.1 ms, 20755 stitches\n", "median "), Some(30.1));
        assert_eq!(number_after("==6616== Collected : 279792645\n==6616== \n", "Collected : "), Some(279_792_645.0));
        assert_eq!(number_after("==1== Collected : 1,234\n", "Collected : "), Some(1234.0));
        assert_eq!(number_after("nothing here", "median "), None);
        assert_eq!(number_after("median ms", "median "), None);
        // An example that times several things prints a median for each.
        let printed = "round 0.25 mm: median 99.4 ms, 299 insets\n7 waves 0.25 mm: median 153.1 ms, 284 insets\n";
        assert_eq!(medians(printed), [99.4, 153.1]);
        assert_eq!(medians("median 30.1 ms, 20755 stitches\n"), [30.1]);
        assert!(medians("no times").is_empty());
    }

    #[test]
    fn the_record_reads_back_as_it_is_written() {
        let records: BTreeMap<String, Record> = BENCHES.iter().map(|bench| (bench.name.to_string(), KEPT)).collect();
        assert_eq!(toml::from_str::<BTreeMap<String, Record>>(&render(&records)).unwrap(), records);
        // The committed file is as recording writes it, so that recording changes only the numbers, and it
        // has a table for each benchmark. Git can check it out with Windows line endings.
        let text = util::read(&util::root().join(RECORD)).unwrap().replace("\r\n", "\n");
        let committed = toml::from_str::<BTreeMap<String, Record>>(&text).unwrap();
        assert_eq!(render(&committed), text);
        assert_eq!(committed.keys().map(String::as_str).collect::<Vec<_>>(), {
            let mut names: Vec<&str> = BENCHES.iter().map(|bench| bench.name).collect();
            names.sort_unstable();
            names
        });
    }
}
