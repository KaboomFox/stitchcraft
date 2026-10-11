//! `cargo xtask perf [--record]`: the performance budget (`docs/src/design/guardrails.md`, NFR-PERF-1).
//!
//! Plans the reference hoop's whole field, a 130 × 180 mm tatami fill at 0.4 mm (`REQ-FILL-TAT-009`), with
//! the engine's `field` example built in release, and measures it 2 ways:
//!
//! - **Instructions**, the guard. Valgrind's callgrind counts the instructions one plan takes, in
//!   `pipeline::plan` and everything it calls. The count does not depend on how fast the machine is or what
//!   else it runs, so a change that makes planning slower fails every time and nothing else does. A count
//!   above the record in `conformance/perf.toml` by more than 2 % fails: the C library's allocator and
//!   copies differ a little from one system to the next. `--record` lowers the record to today's count,
//!   rounded up to a million, and never raises it; raising it is a hand edit, in a pull request that says
//!   why. Without valgrind the count is skipped, except in CI, which installs it.
//! - **Time**, the budget itself. The median of 21 plans, in milliseconds, against NFR-PERF-1's 50 ms. How
//!   fast and how busy the machine is change it, so a plan over the budget is a warning: the count is what
//!   holds a change to account.
//!
//! The release build and callgrind take minutes, so the budget has its own CI job instead of a step in
//! `cargo xtask ci`. In GitHub Actions both numbers go to the job summary and to an annotation.

use std::process::Command;

use serde::Deserialize;

use crate::util::{self, Findings};

const RECORD: &str = "conformance/perf.toml";
/// The example that plans the field, and where a release build puts it.
const EXAMPLE: &str = "field";
const BINARY: &str = "target/release/examples/field";
/// How many timed plans the time is the median of.
const RUNS: usize = 21;
/// How far above the record a count may go, in percent.
const SLACK_PERCENT: u64 = 2;
/// Counts are recorded in whole millions, rounded up.
const MILLION: u64 = 1_000_000;
/// The function whose instructions callgrind counts, with everything it calls.
const COUNTED: &str = "stitchcraft_engine::pipeline::plan";

#[derive(Deserialize)]
struct RecordFile {
    field: Field,
}

/// The field's budget and its record.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq)]
struct Field {
    /// NFR-PERF-1: the most the median plan may take natively, in milliseconds.
    budget_ms: f64,
    /// The instructions one plan takes, as recorded.
    instructions: u64,
}

/// `cargo xtask perf`.
pub fn run(record: bool) -> Result<(), String> {
    let root = util::root();
    let mut build = util::cargo();
    build.args(["build", "--release", "--locked", "--package", "stitchcraft-engine", "--example", EXAMPLE]);
    util::run(build, "cargo build --release --example field")?;
    let path = root.join(RECORD);
    let mut field = toml::from_str::<RecordFile>(&util::read(&path)?).map_err(|e| format!("{RECORD}: {e}"))?.field;
    let binary = root.join(BINARY);
    let mut findings = Findings::default();

    let timed = output(Command::new(&binary).arg(RUNS.to_string()), "the field example")?;
    let median = number_after(&timed, "median ").ok_or(format!("the field example printed no median: {timed}"))?;
    let mut report = vec![format!("the hoop's whole field plans in {median:.1} ms, the median of {RUNS}; the budget is {} ms", field.budget_ms)];
    if median > field.budget_ms {
        findings.warn(format!(
            "planning the hoop's whole field took {median:.1} ms, over the {} ms budget (NFR-PERF-1): a slow or busy machine takes \
             longer, but see whether the instruction count grew",
            field.budget_ms
        ));
    }

    if util::tool_available("valgrind", &["--version"]) {
        let mut callgrind = Command::new("valgrind");
        callgrind.args(["--tool=callgrind", "--callgrind-out-file=/dev/null", &format!("--toggle-collect={COUNTED}")]).arg(&binary).arg("0");
        let counted = errors_of(&mut callgrind, "valgrind --tool=callgrind")?;
        let instructions = number_after(&counted, "Collected : ").ok_or(format!("callgrind counted nothing: {counted}"))?;
        // Instruction counts are far below 2^53, so the conversion is exact.
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let instructions = instructions as u64;
        report.push(format!("one plan takes {instructions} instructions; the record is {}", field.instructions));
        match judged(field, instructions, record) {
            Judged::Within => {}
            Judged::Over(limit) => findings.error(format!(
                "one plan of the hoop's whole field takes {instructions} instructions, more than the {limit} the record in {RECORD} \
                 allows: make it faster, or raise the record by hand, saying why in the pull request"
            )),
            Judged::Lowered(today) => {
                field.instructions = today;
                std::fs::write(&path, render(field)).map_err(|e| format!("{RECORD}: {e}"))?;
                report.push(format!("recorded {today} in {RECORD}"));
            }
        }
    } else if util::in_ci() {
        findings.error("valgrind is not installed: CI needs it to count the plan's instructions");
    } else {
        findings.warn("valgrind is not installed, so the instructions were not counted (skipped)");
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
    findings.finish("perf", "the hoop's whole field plans within its record")
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
}

/// `instructions` against `field`'s record, recording when `record` is set.
fn judged(field: Field, instructions: u64, record: bool) -> Judged {
    let today = instructions.div_ceil(MILLION).saturating_mul(MILLION);
    let limit = field.instructions.saturating_add(field.instructions * SLACK_PERCENT / 100);
    match (record, today < field.instructions, instructions > limit) {
        (true, true, _) => Judged::Lowered(today),
        (false, _, true) => Judged::Over(limit),
        _ => Judged::Within,
    }
}

/// `conformance/perf.toml` with `field` in it.
fn render(field: Field) -> String {
    format!(
        "# The performance budget (`cargo xtask perf`, docs/src/design/guardrails.md): planning the reference hoop's\n\
         # whole field, a 130 × 180 mm tatami fill at 0.4 mm (REQ-FILL-TAT-009, NFR-PERF-1).\n\
         \n\
         [field]\n\
         # The most the median of 21 plans may take natively, in milliseconds (NFR-PERF-1). Over it is a warning.\n\
         budget_ms = {:?}\n\
         # The instructions one plan takes, counted by valgrind's callgrind. More than this plus 2 % fails.\n\
         # `cargo xtask perf --record` lowers it and never raises it: raise it by hand, in a pull request that\n\
         # says why.\n\
         instructions = {}\n",
        field.budget_ms, field.instructions
    )
}

/// The number written after `label` in `text`, up to the next character that is not part of one.
fn number_after(text: &str, label: &str) -> Option<f64> {
    let (_, rest) = text.split_once(label)?;
    let digits: String = rest.chars().take_while(|c| c.is_ascii_digit() || matches!(c, '.' | ',')).filter(|c| *c != ',').collect();
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

    const FIELD: Field = Field { budget_ms: 50.0, instructions: 280_000_000 };

    #[test]
    fn a_count_within_2_percent_of_the_record_passes_and_above_fails() {
        assert_eq!(judged(FIELD, 285_600_000, false), Judged::Within);
        assert_eq!(judged(FIELD, 285_600_001, false), Judged::Over(285_600_000));
        assert_eq!(judged(FIELD, 200_000_000, false), Judged::Within, "faster passes without recording");
    }

    #[test]
    fn recording_lowers_the_record_to_whole_millions_and_never_raises_it() {
        assert_eq!(judged(FIELD, 271_400_001, true), Judged::Lowered(272_000_000));
        assert_eq!(judged(FIELD, 279_000_000, true), Judged::Lowered(279_000_000));
        // Within the same million, or above the record, it stays.
        assert_eq!(judged(FIELD, 279_000_001, true), Judged::Within);
        assert_eq!(judged(FIELD, 290_000_000, true), Judged::Within);
    }

    #[test]
    fn the_numbers_are_read_from_what_the_tools_print() {
        assert_eq!(number_after("median 30.1 ms, 20755 stitches\n", "median "), Some(30.1));
        assert_eq!(number_after("==6616== Collected : 279792645\n==6616== \n", "Collected : "), Some(279_792_645.0));
        assert_eq!(number_after("==1== Collected : 1,234\n", "Collected : "), Some(1234.0));
        assert_eq!(number_after("nothing here", "median "), None);
        assert_eq!(number_after("median ms", "median "), None);
    }

    #[test]
    fn the_record_reads_back_as_it_is_written() {
        assert_eq!(toml::from_str::<RecordFile>(&render(FIELD)).unwrap().field, FIELD);
        // The committed file is as recording writes it, so that recording changes only the number.
        let text = util::read(&util::root().join(RECORD)).unwrap();
        assert_eq!(render(toml::from_str::<RecordFile>(&text).unwrap().field), text);
    }
}
