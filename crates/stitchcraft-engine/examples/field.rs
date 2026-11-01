//! Plans the reference hoop's whole field, a 130 × 180 mm tatami fill at 0.4 mm with Ink/Stitch's other
//! defaults (`REQ-FILL-TAT-009`), for `cargo xtask perf`, which builds this in release.
//!
//! `field RUNS` plans it once to warm up, then `RUNS` times, and prints the median time in milliseconds
//! and the plan's stitch count: `median 37.3 ms, 20755 stitches`. `field 0` plans it once and prints its
//! stitch count, for valgrind's callgrind to count the instructions one plan takes, in `plan` below.

// A binary prints what it measured, which library code never may (clippy.toml).
#![allow(clippy::print_stdout, clippy::print_stderr)]

mod timing;

use std::process::ExitCode;

use stitchcraft_engine::PlanOutcome;
use stitchcraft_testkit::designs::{RED, fill, planned};

/// The field's corners, centred on the hoop's centre.
const FIELD: &[(f64, f64)] = &[(-65.0, -90.0), (-65.0, 90.0), (65.0, 90.0), (65.0, -90.0)];

/// One plan of the field. Callgrind counts its instructions by its name, `field::plan`, which a rename
/// would lose (`xtask/src/perf.rs`), so it is never inlined into its callers.
#[inline(never)]
fn plan() -> PlanOutcome {
    planned(vec![fill("field", &[FIELD], &RED, &[("row_spacing_mm", "0.4")])])
}

// An example program reads its arguments.
#[allow(clippy::disallowed_methods)]
fn main() -> ExitCode {
    let Some(runs) = std::env::args().nth(1).and_then(|runs| runs.parse::<usize>().ok()) else {
        eprintln!("usage: field RUNS, a whole number of plans to time");
        return ExitCode::FAILURE;
    };
    let Some(sewn) = plan().plan else {
        eprintln!("the field does not plan");
        return ExitCode::FAILURE;
    };
    let stitches = sewn.stats().stitches;
    if runs == 0 {
        println!("{stitches} stitches");
        return ExitCode::SUCCESS;
    }
    let median = timing::median_ms(runs, plan);
    println!("median {median:.1} ms, {stitches} stitches");
    ExitCode::SUCCESS
}
