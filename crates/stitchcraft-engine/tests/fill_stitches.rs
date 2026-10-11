//! The needle points of a tatami fill's rows (`REQ-FILL-TAT-003`): on a grid along each row anchored at
//! the design's origin, a `staggers`-th of the longest stitch along from the row before's, as Ink/Stitch
//! places them; or at random lengths. Fills are not sewn until roadmap M5.4 routes their rows, but each
//! row's needle points are placed already.

// Test code may unwrap and index (clippy.toml allows it in tests).
#![allow(clippy::unwrap_used, clippy::panic, clippy::indexing_slicing)]

use proptest::prelude::*;
use stitchcraft_core::rng::SplitMix64;
use stitchcraft_core::{Budget, Point, math};
use stitchcraft_engine::common::CommonParams;
use stitchcraft_engine::generators::running::RunningParams;
use stitchcraft_engine::generators::tatami::TatamiParams;
use stitchcraft_engine::generators::tatami::rows::Grid;
use stitchcraft_engine::generators::tatami::stitches::{Stitching, row_points};
use stitchcraft_params::{Family, ParamSet};
use stitchcraft_testkit::designs::p;

/// Rows at `angle`, 0.25 mm apart, sewn in stitches of 4 mm staggered over 4 rows: Ink/Stitch's defaults.
fn stitching(angle: f64) -> Stitching {
    Stitching { grid: Grid { angle, spacing: 0.25, end_spacing: None }, length: 4.0, staggers: 4.0, skip_last: false, jitter: None }
}

fn sewn(start: Point, end: Point, how: &Stitching) -> Vec<Point> {
    row_points(start, end, how, &mut SplitMix64::new(1), &mut Budget::DEFAULT.meter()).unwrap()
}

/// The x of each point of a horizontal row, rounded to a nanometre.
fn xs(points: &[Point]) -> Vec<f64> {
    points.iter().map(|q| (q.x() * 1e6).round() / 1e6).collect()
}

#[test]
fn req_fill_tat_003_needle_points_lie_on_the_stagger_grid() {
    // Horizontal rows 0.25 mm apart: row k lies at y = 0.25 k, and its grid is a k-th quarter of a 4 mm
    // stitch along from the origin's. Each row from x = 1.3 to 18.7 starts at its start, takes the grid's
    // points past it, and ends at its end.
    let row = |k: f64| xs(&sewn(p(1.3, 0.25 * k), p(18.7, 0.25 * k), &stitching(0.0)));
    assert_eq!(row(0.0), [1.3, 4.0, 8.0, 12.0, 16.0, 18.7]);
    assert_eq!(row(1.0), [1.3, 5.0, 9.0, 13.0, 17.0, 18.7]);
    assert_eq!(row(2.0), [1.3, 2.0, 6.0, 10.0, 14.0, 18.0, 18.7]);
    assert_eq!(row(3.0), [1.3, 3.0, 7.0, 11.0, 15.0, 18.7]);
    assert_eq!(row(4.0), row(0.0), "back after 4 rows");
    assert_eq!(row(-1.0), row(3.0), "above the origin too");
    // Sewn the other way, the same points, from the end.
    assert_eq!(xs(&sewn(p(18.7, 0.0), p(1.3, 0.0), &stitching(0.0))), [18.7, 16.0, 12.0, 8.0, 4.0, 1.3]);
    // At 90° the rows run up the screen, along -y, and lie across along x: the column x = 0.25 is row 1,
    // its grid a quarter stitch up from the origin's. Sewn down the screen, against the rows.
    let ys = |points: &[Point]| points.iter().map(|q| (q.y() * 1e6).round() / 1e6).collect::<Vec<_>>();
    assert_eq!(ys(&sewn(p(0.25, -18.7), p(0.25, -1.3), &stitching(90.0))), [-18.7, -17.0, -13.0, -9.0, -5.0, -1.3]);
    assert_eq!(ys(&sewn(p(0.25, -1.3), p(0.25, -18.7), &stitching(90.0))), [-1.3, -5.0, -9.0, -13.0, -17.0, -18.7]);
    // Segments side by side share the grid.
    let (left, right) = (sewn(p(0.0, 0.25), p(10.0, 0.25), &stitching(0.0)), sewn(p(10.0, 0.25), p(20.0, 0.25), &stitching(0.0)));
    assert_eq!((xs(&left), xs(&right)), (vec![0.0, 1.0, 5.0, 9.0, 10.0], vec![10.0, 13.0, 17.0, 20.0]));
}

#[test]
fn req_fill_tat_003_neighbouring_rows_never_line_their_points_up() {
    // Over 12 rows, a row's points lie a quarter stitch along from the row before's, so no 2 neighbouring
    // rows share a point, and a row shares its points only with rows a whole number of 4 rows away.
    let rows: Vec<Vec<f64>> = (0..12).map(|k| xs(&sewn(p(0.0, 0.25 * f64::from(k)), p(40.0, 0.25 * f64::from(k)), &stitching(0.0)))).collect();
    let inside = |row: &[f64]| row[1..row.len() - 1].to_vec();
    for (k, row) in rows.iter().enumerate() {
        for (j, other) in rows.iter().enumerate().skip(k + 1) {
            let shared = inside(row).iter().any(|x| inside(other).contains(x));
            assert_eq!(shared, (j - k) % 4 == 0, "rows {k} and {j}");
        }
    }
}

#[test]
fn req_fill_tat_003_rows_at_an_angle_take_the_grid_turned_with_them() {
    // At 30°, rows run along (cos 30°, -sin 30°) and lie across (sin 30°, cos 30°). The row through
    // (5, 3) has the number its distance across over the spacing gives, rounded, and its points lie at
    // whole stitches plus its offset along it.
    let (sin, cos) = math::sin_cos(math::to_radians(30.0));
    let (along, across) = ((cos, -sin), (sin, cos));
    let dot = |q: Point, (x, y): (f64, f64)| q.x() * x + q.y() * y;
    let start = p(5.0, 3.0);
    let end = p(5.0 + 30.0 * along.0, 3.0 + 30.0 * along.1);
    let points = sewn(start, end, &stitching(30.0));
    let number = (dot(start, across) / 0.25).round_ties_even();
    let offset = (number / 4.0).rem_euclid(1.0) * 4.0;
    assert!(points.len() > 6, "{points:?}");
    for q in &points[1..points.len() - 1] {
        let place = (dot(*q, along) - offset).rem_euclid(4.0);
        assert!(place.min(4.0 - place) < 1e-9, "{q:?}: {place}");
        assert!((dot(*q, across) - dot(start, across)).abs() < 1e-9, "on the row");
    }
    assert_eq!((points[0], points[points.len() - 1]), (start, end));
}

#[test]
fn req_fill_tat_003_a_row_ends_at_its_end_unless_it_is_skipped_or_too_near() {
    let how = stitching(0.0);
    // The last grid point 0.1 mm before the end, exactly: the end gets no point of its own.
    assert_eq!(xs(&sewn(p(-2.0, 0.0), p(0.1, 0.0), &how)), [-2.0, 0.0]);
    assert_eq!(xs(&sewn(p(-2.0, 0.0), p(0.11, 0.0), &how)), [-2.0, 0.0, 0.11]);
    // With skip_last, never, nor a grid point on the end itself.
    let skipping = Stitching { skip_last: true, ..how };
    assert_eq!(xs(&sewn(p(1.3, 0.0), p(18.7, 0.0), &skipping)), [1.3, 4.0, 8.0, 12.0, 16.0]);
    assert_eq!(xs(&sewn(p(1.3, 0.0), p(8.0, 0.0), &skipping)), [1.3, 4.0]);
    // A segment shorter than the way to the grid: its start and its end. Of no length: one point.
    assert_eq!(xs(&sewn(p(1.3, 0.0), p(2.5, 0.0), &how)), [1.3, 2.5]);
    assert_eq!(sewn(p(1.3, 0.0), p(1.3, 0.0), &how), [p(1.3, 0.0)]);
    // A start on the grid is sewn once.
    assert_eq!(xs(&sewn(p(4.0, 0.0), p(9.0, 0.0), &how)), [4.0, 8.0, 9.0]);
}

#[test]
fn req_fill_tat_003_fractional_staggers_come_back_after_their_whole_cycle() {
    // 2.5 staggers: offsets of 0.4, 0.8, 0.2, 0.6 and 0 of a stitch, back after 5 rows.
    let how = Stitching { staggers: 2.5, ..stitching(0.0) };
    let first = |k: f64| xs(&sewn(p(0.0, 0.25 * k), p(10.0, 0.25 * k), &how))[1];
    assert_eq!([1.0, 2.0, 3.0, 4.0, 5.0].map(first), [1.6, 3.2, 0.8, 2.4, 4.0]);
}

#[test]
fn req_fill_tat_003_random_lengths_start_at_a_random_share_and_vary_by_the_jitter() {
    let how = Stitching { jitter: Some(0.25), ..stitching(0.0) };
    let run = |seed: u64| row_points(p(0.0, 0.0), p(40.0, 0.0), &how, &mut SplitMix64::new(seed), &mut Budget::DEFAULT.meter()).unwrap();
    let points = run(3);
    let x = xs(&points);
    assert_eq!((x[0], x[x.len() - 1]), (0.0, 40.0));
    assert!(x[1] > 0.0 && x[1] < 4.0, "the first a share of a stitch in: {x:?}");
    let steps: Vec<f64> = x[1..x.len() - 1].windows(2).map(|w| w[1] - w[0]).collect();
    assert!(steps.iter().all(|step| (3.0 - 1e-9..5.0).contains(step)), "each a stitch, a quarter shorter or longer at most: {steps:?}");
    assert!(steps.iter().any(|step| (step - 4.0).abs() > 1e-6), "the lengths vary: {steps:?}");
    assert_eq!(run(3), points, "the same draws, the same points");
    assert_ne!(run(4), points);
    // The draws, in order: the first point's share of a stitch, then each step's.
    let mut rolls = SplitMix64::new(3);
    let mut by = 4.0 * rolls.next_f64();
    let mut want = vec![0.0];
    while by < 40.0 {
        want.push((by * 1e6).round() / 1e6);
        by += 4.0 * (1.0 + 0.25 * (rolls.next_f64() - 0.5) * 2.0);
    }
    want.push(40.0);
    assert_eq!(x, want);
}

#[test]
fn req_fill_tat_003_a_fill_s_stitch_settings_are_ink_stitch_s() {
    let none = ParamSet::new();
    let tatami = TatamiParams::from_set(&none).unwrap().params;
    let longest = CommonParams::from_set_for(&none, Family::Fill).unwrap().params.max_stitch_length_mm.unwrap();
    let running = RunningParams::from_set(&none).unwrap().params;
    assert_eq!(
        tatami.stitching(longest, &running),
        Stitching { grid: Grid { angle: 0.0, spacing: 0.25, end_spacing: None }, length: 4.0, staggers: 4.0, skip_last: false, jitter: None }
    );
    let set: ParamSet =
        [("staggers", "2.5"), ("skip_last", "true"), ("enable_random_stitch_length", "true"), ("random_stitch_length_jitter_percent", "30")]
            .into_iter()
            .collect();
    let (tatami, running) = (TatamiParams::from_set(&set).unwrap().params, RunningParams::from_set(&set).unwrap().params);
    let how = tatami.stitching(longest, &running);
    assert_eq!((how.staggers, how.skip_last, how.jitter), (2.5, true, Some(0.3)));
}

#[test]
fn req_fill_tat_003_needle_points_are_charged_to_the_budget() {
    let how = stitching(0.0);
    let mut meter = Budget::DEFAULT.meter();
    row_points(p(1.3, 0.0), p(18.7, 0.0), &how, &mut SplitMix64::new(1), &mut meter).unwrap();
    assert_eq!(Budget::DEFAULT.max_work - meter.work_left(), 6, "a unit per point");
    let tight = &mut Budget { max_stitches: 1, max_work: 3 }.meter();
    assert!(row_points(p(1.3, 0.0), p(18.7, 0.0), &how, &mut SplitMix64::new(1), tight).is_err());
}

proptest! {
    #![proptest_config(stitchcraft_testkit::strategies::config(256))]

    #[test]
    fn req_fill_tat_003_every_point_between_the_ends_is_on_the_grid_and_a_stitch_from_the_next(
        angle in -180.0..180.0_f64,
        x in -50.0..50.0_f64,
        y in -50.0..50.0_f64,
        length in 0.0..60.0_f64,
        stitch in 0.5..8.0_f64,
        staggers in 1.0..6.0_f64,
        backwards in any::<bool>(),
    ) {
        let how = Stitching { grid: Grid { angle, spacing: 0.25, end_spacing: None }, length: stitch, staggers, skip_last: false, jitter: None };
        let (sin, cos) = math::sin_cos(math::to_radians(angle));
        let (along, across) = ((cos, -sin), (sin, cos));
        let dot = |q: Point, (u, v): (f64, f64)| q.x() * u + q.y() * v;
        let (a, b) = (p(x, y), p(x + length * along.0, y + length * along.1));
        let (start, end) = if backwards { (b, a) } else { (a, b) };
        let points = sewn(start, end, &how);
        let number = (dot(start, across) / 0.25).round_ties_even();
        let offset = (number / staggers).rem_euclid(1.0) * stitch;
        prop_assert_eq!(points[0], start);
        let inside = if points.len() > 2 { &points[1..points.len() - 1] } else { &[][..] };
        for q in inside {
            let place = (dot(*q, along) - offset).rem_euclid(stitch);
            prop_assert!(place.min(stitch - place) < 1e-6, "{:?} off the grid by {}", q, place);
        }
        for pair in inside.windows(2) {
            prop_assert!((pair[0].distance(pair[1]) - stitch).abs() < 1e-6);
        }
        if length > 0.0 {
            let last = points[points.len() - 1];
            prop_assert!(last == end || last.distance(end) <= 0.1 + 1e-9, "ends at its end, or within 0.1 mm of it");
        }
    }
}
