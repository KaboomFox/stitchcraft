//! Shrinks a region 150 mm across, of 1,000 points, 300 times, each inset from the last, as contour fill
//! lays its rows: the third criterion of ADR-0005 (`docs/src/design/adr/0005-geometry-stack.md`), which
//! roadmap step M7.6 has the offsetter meet before contour fill relies on it.
//!
//! `insets RUNS` shrinks each of the 4 regions once to warm up, then `RUNS` times, and prints for each the
//! median time in milliseconds, how many insets had parts, and their points: `round 0.25 mm: median 99.4 ms,
//! 299 insets, 299299 points`. A region shrunk to nothing stops early: the round one lasts 299 insets of
//! 0.25 mm. `insets 0` shrinks them once, for valgrind's callgrind to count the instructions in `insets`
//! below.

// A binary prints what it measured, which library code never may (clippy.toml).
#![allow(clippy::print_stdout, clippy::print_stderr)]

mod timing;

use std::process::ExitCode;

use stitchcraft_core::{Budget, Point, math};
use stitchcraft_engine::normalize::offset::buffer;
use stitchcraft_engine::normalize::region::Polygon;

/// The region's radius, in millimetres: 150 mm across.
const RADIUS: f64 = 75.0;
/// The points of its outline.
const POINTS: usize = 1000;
/// How many times it is shrunk.
const INSETS: usize = 300;
/// The waves round the wavy region, and how far each reaches in and out, in millimetres.
const WAVES: f64 = 7.0;
const WAVE: f64 = 2.0;
/// The regions and the steps they are shrunk by, as ADR-0005 measured them.
const CASES: [(&str, bool, f64); 4] = [("round", false, 0.25), ("round", false, 0.4), ("7 waves", true, 0.25), ("7 waves", true, 0.4)];

/// A region 150 mm across, of [`POINTS`] points, round or with [`WAVES`] waves: its outline clockwise in
/// y-up axes, as a region's is, its first point repeated at its end.
fn region(wavy: bool) -> Polygon {
    let mut outline: Vec<Point> = (0..POINTS)
        .filter_map(|i| {
            #[allow(clippy::cast_precision_loss)] // a count of points, far below 2^52
            let turn = -(i as f64) / POINTS as f64 * 2.0 * std::f64::consts::PI;
            let radius = if wavy { RADIUS - WAVE + WAVE * math::cos(WAVES * turn) } else { RADIUS };
            Point::new(radius * math::cos(turn), radius * math::sin(turn)).ok()
        })
        .collect();
    outline.extend(outline.first().copied());
    Polygon { outline, holes: Vec::new() }
}

/// `region` shrunk [`INSETS`] times by `step`, each inset from the parts of the last: how many insets had
/// parts, and their points. Callgrind counts its instructions by its name, `insets::insets`, so it is never
/// inlined into its callers.
#[inline(never)]
fn insets(region: &Polygon, step: f64) -> Option<(usize, usize)> {
    let mut meter = Budget { max_stitches: 0, max_work: u64::MAX }.meter();
    let (mut parts, mut kept, mut points) = (vec![region.clone()], 0, 0);
    for _ in 0..INSETS {
        let mut next = Vec::new();
        for part in &parts {
            next.extend(buffer(part, -step, &mut meter).ok()?.ok()?);
        }
        if next.is_empty() {
            break;
        }
        kept += 1;
        points += next.iter().map(|part| part.outline.len() + part.holes.iter().map(Vec::len).sum::<usize>()).sum::<usize>();
        parts = next;
    }
    Some((kept, points))
}

// An example program reads its arguments.
#[allow(clippy::disallowed_methods)]
fn main() -> ExitCode {
    let Some(runs) = std::env::args().nth(1).and_then(|runs| runs.parse::<usize>().ok()) else {
        eprintln!("usage: insets RUNS, a whole number of runs to time");
        return ExitCode::FAILURE;
    };
    for (name, wavy, step) in CASES {
        let region = region(wavy);
        let Some((kept, points)) = insets(&region, step) else {
            eprintln!("{name} {step} mm does not shrink");
            return ExitCode::FAILURE;
        };
        if runs == 0 {
            println!("{name} {step} mm: {kept} insets, {points} points");
            continue;
        }
        let median = timing::median_ms(runs, || insets(&region, step));
        println!("{name} {step} mm: median {median:.1} ms, {kept} insets, {points} points");
    }
    ExitCode::SUCCESS
}
