//! What shows where 2 strokes of a part sewn as strokes meet (`REQ-AUTO-117`, `REQ-AUTO-118`,
//! `docs/src/design/image-to-embroidery.md` › Joins of letters sewn as strokes): drawn letters made in code,
//! each one automatic area planned with its stitches where it is drawn, and read column by column from the
//! plan's top stitches.

// Test code may unwrap and index (clippy.toml allows it in tests).
#![allow(clippy::unwrap_used, clippy::panic, clippy::indexing_slicing)]

use stitchcraft_engine::design::{Design, DesignSettings, Element};
use stitchcraft_plan::{Rgb, Role, StitchKind, StitchPlan, Thread};
use stitchcraft_testkit::designs::{fill, in_place};
use stitchcraft_testkit::stitches::jump_threads;

/// A T 12 mm wide and 14 mm tall, its bar and stem 2.5 mm wide.
const T: &[(f64, f64)] = &[(0.0, 0.0), (12.0, 0.0), (12.0, 2.5), (7.25, 2.5), (7.25, 14.0), (4.75, 14.0), (4.75, 2.5), (0.0, 2.5)];

/// An H 12 mm wide and 14 mm tall, its stems and bar 2.5 mm wide.
const H: &[(f64, f64)] = &[
    (0.0, 0.0),
    (2.5, 0.0),
    (2.5, 6.0),
    (9.5, 6.0),
    (9.5, 0.0),
    (12.0, 0.0),
    (12.0, 14.0),
    (9.5, 14.0),
    (9.5, 8.5),
    (2.5, 8.5),
    (2.5, 14.0),
    (0.0, 14.0),
];

/// An M 12 mm wide and 14 mm tall, its stems 2.5 mm wide and the strokes of its V about 2.2 mm wide,
/// meeting at about 41° above a flat bottom 1.4 mm across, 2 mm above the stems' feet.
const M: &[(f64, f64)] = &[
    (0.0, 0.0),
    (3.2, 0.0),
    (6.0, 7.5),
    (8.8, 0.0),
    (12.0, 0.0),
    (12.0, 14.0),
    (9.5, 14.0),
    (9.5, 4.5),
    (6.7, 12.0),
    (5.3, 12.0),
    (2.5, 4.5),
    (2.5, 14.0),
    (0.0, 14.0),
];

/// The plan of one automatic area drawn as `rings`.
fn planned(name: &str, rings: &[&[(f64, f64)]]) -> StitchPlan {
    let element = Element { automatic: true, ..fill(name, rings, &Thread::new(Rgb::new(20, 20, 20)), &[]) };
    let design = Design::new(vec![element], DesignSettings::default()).unwrap();
    in_place(&design).plan.unwrap()
}

/// Each column's top needle points, in the order the columns sew: a column is a run of top stitches of one
/// element of the plan.
fn columns(plan: &StitchPlan) -> Vec<(String, Vec<(f64, f64)>)> {
    let mut out: Vec<(String, Vec<(f64, f64)>)> = Vec::new();
    for stitch in plan.stitches().filter(|s| s.kind == StitchKind::Normal && s.origin.role == Role::Top) {
        let id = stitch.origin.element.and_then(|e| plan.element(e)).map_or(String::new(), ToString::to_string);
        match out.iter_mut().find(|(name, _)| *name == id) {
            Some((_, points)) => points.push((stitch.at.x(), stitch.at.y())),
            None => out.push((id, vec![(stitch.at.x(), stitch.at.y())])),
        }
    }
    out
}

/// Where in the sewing order the column whose needle points lie mostly within `x` and `y` comes, and its
/// points.
fn column_in(columns: &[(String, Vec<(f64, f64)>)], x: (f64, f64), y: (f64, f64)) -> (usize, Vec<(f64, f64)>) {
    let inside = |p: &(f64, f64)| (x.0..=x.1).contains(&p.0) && (y.0..=y.1).contains(&p.1);
    let (index, (_, points)) =
        columns.iter().enumerate().max_by_key(|(_, (_, points))| points.iter().filter(|p| inside(p)).count() * 1000 / points.len().max(1)).unwrap();
    (index, points.clone())
}

#[test]
fn req_auto_117_a_stroke_that_stops_sews_last_and_ends_straight_along_the_other() {
    // A T's stem stops against its bar, and an H's bar against both stems. The stopping stroke sews after
    // the one it stops against, so that its own end, one straight stitch across it, is the seam, and the
    // other's zigzag edge and needle points lie under it. Its needle points within a width of the join lie
    // no farther along it than a thread past the line it ends on. Planning it last costs no jump thread. On
    // main at M13.48 the stem and the bar sew first and the bar's and the stems' zigzag edges lie on them.
    for (name, ring, stop_x, stop_y, run_x, run_y, end_y) in
        [("T", T, (4.75, 7.25), (5.0, 14.0), (0.0, 12.0), (0.0, 2.5), 2.5), ("H", H, (3.0, 9.0), (6.0, 8.5), (0.0, 2.5), (0.0, 14.0), f64::NAN)]
    {
        let plan = planned(name, &[ring]);
        let columns = columns(&plan);
        let (stopping, points) = column_in(&columns, stop_x, stop_y);
        let (running, _) = column_in(&columns, run_x, run_y);
        assert!(stopping > running, "{name}: the stopping stroke sews {stopping}th, the one it stops against {running}th: {columns:?}");
        if !end_y.is_nan() {
            let near: Vec<f64> = points.iter().filter(|p| p.1 <= end_y + 2.5).map(|p| p.1).collect();
            let least = near.iter().copied().fold(f64::INFINITY, f64::min);
            assert!(least >= end_y - 0.45, "{name}: the stem's end reaches {least:.2}, the bar's side lies at {end_y}");
        }
        assert_eq!(jump_threads(&plan).len(), 0, "{name}: jump threads");
    }
}

#[test]
fn req_auto_118_at_an_apex_one_stroke_runs_through_and_the_other_ends_along_it() {
    // The 2 strokes of an M's V end where they meet. As a font sews them, one runs on to the bottom and
    // takes in the drawn end, and the other ends along its inner side: one column's needle points within
    // 0.8 mm of the bottom lie on both sides of its middle, 0.6 mm or more from it. On main at M13.48 the
    // join is cut along the line that halves the angle, each stroke takes one half of the bottom, and a
    // seam runs up the middle.
    let plan = planned("m", &[M]);
    let columns = columns(&plan);
    let spans = |points: &[(f64, f64)]| {
        let bottom: Vec<f64> = points.iter().filter(|p| (11.2..=12.6).contains(&p.1) && (4.0..=8.0).contains(&p.0)).map(|p| p.0).collect();
        bottom.iter().any(|&x| x <= 5.4) && bottom.iter().any(|&x| x >= 6.6)
    };
    let whole: Vec<&String> = columns.iter().filter(|(_, points)| spans(points)).map(|(name, _)| name).collect();
    assert_eq!(whole.len(), 1, "columns that take in the bottom: {whole:?} of {columns:?}");
}
