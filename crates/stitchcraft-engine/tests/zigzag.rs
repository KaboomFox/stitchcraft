//! The zigzag stroke's conformance cases (`REQ-RUN-009` to `REQ-RUN-012`), through the engine's public API.
//! A path drawn along x goes east, and y grows down the page, so the right of the way it goes is +y.

// Test code may unwrap and index (clippy.toml allows it in tests).
#![allow(clippy::unwrap_used)]

use stitchcraft_core::units::MM_PER_SVG_PX;
use stitchcraft_core::{Budget, Code, Mm, Point};
use stitchcraft_engine::design::{Path, Segment, Subpath};
use stitchcraft_engine::generators::Stitched;
use stitchcraft_engine::generators::passes::RepeatParams;
use stitchcraft_engine::generators::zigzag::{ZigzagStrokeParams, zigzag_stroke};
use stitchcraft_params::ParamSet;
use stitchcraft_plan::StitchKind;
use stitchcraft_testkit::designs::{RED, line, messages, planned, polylines, rings, widened};

/// `path` sewn `width` millimetres wide, with `settings` for its zigzag, repeats and bean stitch.
fn sewn(path: &Path, width: f64, settings: &[(&str, &str)]) -> Stitched {
    let set: ParamSet = settings.iter().copied().collect();
    let zigzag = ZigzagStrokeParams::from_set(&set).unwrap().params;
    let passes = RepeatParams::from_set(&set).unwrap().params;
    zigzag_stroke(path, Mm::new(width).unwrap(), &zigzag, &passes, Mm::new(0.3).unwrap(), &mut Budget::DEFAULT.meter()).unwrap()
}

/// The only run of `path` sewn so.
fn run(path: &Path, width: f64, settings: &[(&str, &str)]) -> Vec<(f64, f64)> {
    let stitched = sewn(path, width, settings);
    assert_eq!((stitched.runs.len(), stitched.warnings), (1, Vec::new()));
    stitched.runs[0].iter().map(|q| (q.x(), q.y())).collect()
}

fn near(a: (f64, f64), b: (f64, f64)) -> bool {
    (a.0 - b.0).abs() < 1e-9 && (a.1 - b.1).abs() < 1e-9
}

fn east(length: f64) -> Path {
    polylines(&[&[(0.0, 0.0), (length, 0.0)]])
}

/// East 10 mm, then south 10 mm.
fn corner() -> Path {
    polylines(&[&[(0.0, 0.0), (10.0, 0.0), (10.0, 10.0)]])
}

/// A quarter of a circle 10 mm round the origin, from (10, 0) to (0, 10), as a cubic curve (the usual
/// 0.5523 handles, within 0.003 mm of the circle).
fn quarter() -> Path {
    let p = |x: f64, y: f64| Point::new(x, y).unwrap();
    let k = 10.0 * 0.552_284_749_830_793_4;
    let curve = Segment::Cubic(p(10.0, k), p(k, 10.0), p(0.0, 10.0));
    Path { subpaths: vec![Subpath { start: p(10.0, 0.0), segments: vec![curve], closed: false }] }
}

#[test]
fn req_run_009_the_points_are_spread_evenly_in_a_whole_number_of_zigs_and_zags() {
    // 10 mm at 0.4 mm: 50 half spacings, 51 points 0.2 mm apart, which a width of 0 leaves on the path.
    let points = run(&east(10.0), 0.0, &[]);
    assert_eq!(points.len(), 51);
    assert!(points.iter().enumerate().all(|(i, &q)| near(q, (i as f64 * 0.2, 0.0))), "{points:?}");
    // Measured along the path, round a corner: the 50th of 100 lands on it.
    let bent = run(&corner(), 0.0, &[]);
    assert_eq!(bent.len(), 101);
    assert!(near(bent[50], (10.0, 0.0)) && near(bent[75], (10.0, 5.0)), "{bent:?}");
    // The count of half spacings is rounded half to even, at least 2, and even.
    let count = |length: f64, spacing: &str| run(&east(length), 0.0, &[("zigzag_spacing_mm", spacing)]).len();
    assert_eq!(count(12.625, "0.5"), 51, "50.5 half spacings round to 50");
    assert_eq!(count(13.125, "0.5"), 53, "52.5 half spacings round to 52");
    assert_eq!(count(12.6875, "0.5"), 53, "50.75 half spacings round to 51, odd, so 52");
    assert_eq!(count(2.75, "0.5"), 13, "11 half spacings, odd, make 12");
    assert_eq!(count(0.1, "0.5"), 3, "at least 2");
    // A curve is flattened within a tenth of a CSS pixel: points on it lie that close to the circle.
    let arc = run(&quarter(), 0.0, &[]);
    let off = arc.iter().map(|&(x, y)| ((x * x + y * y).sqrt() - 10.0).abs()).fold(0.0, f64::max);
    assert!(off < 0.03, "{off} mm off the circle");
    // Only the first spacing of a list counts, as in Ink/Stitch.
    assert_eq!(run(&east(10.0), 0.0, &[("zigzag_spacing_mm", "0.4 2")]), points);
}

#[test]
fn req_run_010_each_point_moves_square_to_its_direction_to_its_side() {
    // 2 mm wide: the points numbered 0, 2, 4 and on go 1 mm to the right of the way east, the others 1 mm
    // to the left.
    let points = run(&east(10.0), 2.0, &[]);
    assert!(points.iter().enumerate().all(|(i, &q)| near(q, (i as f64 * 0.2, if i % 2 == 0 { 1.0 } else { -1.0 }))), "{points:?}");
    // Pull compensation: the first value for the left, the second for the right.
    let pulled = run(&east(10.0), 2.0, &[("stroke_pull_compensation_mm", "0.5 1")]);
    assert!(near(pulled[0], (0.0, 2.0)) && near(pulled[1], (0.2, -1.5)), "{pulled:?}");
    // Below minus half the width, the sides cross over.
    let crossed = run(&east(10.0), 2.0, &[("stroke_pull_compensation_mm", "-1.5")]);
    assert!(near(crossed[0], (0.0, -0.5)) && near(crossed[1], (0.2, 0.5)), "{crossed:?}");
    // The angle turns the way across clockwise, as the drawing shows it.
    let turned = run(&east(10.0), 2.0, &[("zigzag_angle", "30")]);
    let (sin, cos) = (0.5, 3.0_f64.sqrt() / 2.0);
    assert!(near(turned[0], (-sin, cos)) && near(turned[1], (0.2 + sin, -cos)), "{turned:?}");
    // Any way round and anywhere: a line 10 mm long from (0, 5) towards (8, 11), 3 mm wide and turned 30°.
    // Each point goes 1.5 mm across, square to the line and turned clockwise by 30°.
    let slanted = run(&polylines(&[&[(0.0, 5.0), (8.0, 11.0)]]), 3.0, &[("zigzag_angle", "30")]);
    let (ux, uy) = (0.8, 0.6);
    let (sx, sy) = (-uy * cos - ux * sin, -uy * sin + ux * cos);
    assert_eq!(slanted.len(), 51);
    let placed = |i: usize, q: (f64, f64)| {
        let (along, by) = (i as f64 * 0.2, if i.is_multiple_of(2) { 1.5 } else { -1.5 });
        near(q, (along * ux + by * sx, 5.0 + along * uy + by * sy))
    };
    assert!(slanted.iter().enumerate().all(|(i, &q)| placed(i, q)), "{slanted:?}");
    // At a corner a point leans evenly into both sides: its direction is halfway between east and south.
    let half = 0.5_f64.sqrt();
    let bent = run(&corner(), 2.0, &[]);
    assert!(near(bent[50], (10.0 - half, half)), "{:?}", bent[50]);
    // Lines of different lengths count alike: each is taken with length 1. 20.1 mm makes 100.5 half
    // spacings, rounded to 100, so point 50 lies 0.05 mm past the corner, its line from point 49 cutting it.
    let past = run(&polylines(&[&[(0.0, 0.0), (10.0, 0.0), (10.0, 10.1)]]), 2.0, &[]);
    let unit = |(x, y): (f64, f64)| {
        let length = (x * x + y * y).sqrt();
        (x / length, y / length)
    };
    let (from, to) = (unit((10.0 - 9.849, 0.05)), (0.0, 1.0));
    let (x, y) = unit(((from.0 + to.0) / 2.0, (from.1 + to.1) / 2.0));
    assert!(near(past[50], (10.0 - y, 0.05 + x)), "{:?}", past[50]);
    // Where the path turns straight back, a point has no direction and stays on the path.
    let back = run(&polylines(&[&[(0.0, 0.0), (10.0, 0.0), (0.0, 0.0)]]), 2.0, &[]);
    assert!(near(back[50], (10.0, 0.0)) && near(back[49], (9.8, -1.0)), "{:?}", &back[49..=51]);
}

#[test]
fn req_run_010_a_zigzag_stroke_is_sewn_as_wide_as_its_stroke() {
    let zigzag = widened(line("z", (0.0, 0.0), 10.0, &RED, &[("stroke_method", "zigzag_stitch")]), 2.0);
    let outcome = planned(vec![zigzag]);
    assert_eq!(messages(&outcome), Vec::<String>::new());
    let plan = outcome.plan.unwrap();
    let across: Vec<f64> = plan.blocks.iter().flat_map(|b| &b.stitches).filter(|s| s.kind == StitchKind::Normal).map(|s| s.at.y()).collect();
    assert!(across.iter().any(|y| (y - 1.0).abs() < 0.06) && across.iter().any(|y| (y + 1.0).abs() < 0.06), "{across:?}");
}

#[test]
fn req_run_011_a_closed_stroke_ends_on_its_first_needle_point() {
    let square = [(0.0, 0.0), (10.0, 0.0), (10.0, 10.0), (0.0, 10.0)];
    let closed = run(&rings(&[&square]), 2.0, &[]);
    assert_eq!(closed.first(), closed.last());
    // Its first point is placed as an open path's: across the line to the next, 1 mm right of east.
    assert!(near(closed[0], (0.0, 1.0)), "{:?}", closed[0]);
    // A subpath of one point does not count: the stroke still closes, and SC-W0401 names the point. Ink/Stitch
    // counts a subpath of no length as a line, and keeps such a zigzag open (DEV-RUN-008).
    let with_point = sewn(&rings(&[&square, &[(30.0, 30.0)]]), 2.0, &[]);
    assert_eq!(with_point.runs.len(), 1);
    assert_eq!(with_point.runs[0].first(), with_point.runs[0].last());
    assert_eq!(with_point.warnings.iter().map(|d| d.code).collect::<Vec<_>>(), [Code::StrokeTooSmall]);
    // Ends nearer each other than 0.05 CSS pixels close a path. Ends that far apart or further do not.
    let ending = |gap: f64| run(&polylines(&[&[(0.0, 0.0), (10.0, 0.0), (10.0, 10.0), (0.0, 10.0), (0.0, gap)]]), 2.0, &[]);
    let nearly = ending(0.01);
    assert_eq!(nearly.first(), nearly.last());
    let open = ending(0.02);
    assert_ne!(open.first(), open.last());
    let apart = ending(0.05 * MM_PER_SVG_PX);
    assert_ne!(apart.first(), apart.last());
    // A stroke of 2 closed subpaths is not closed, as in Ink/Stitch: each run ends across its own end.
    let shifted: Vec<(f64, f64)> = square.iter().map(|&(x, y)| (x + 20.0, y)).collect();
    let both = sewn(&rings(&[&square, &shifted]), 2.0, &[]);
    assert_eq!(both.runs.len(), 2);
    assert!(both.runs.iter().all(|run| run.first() != run.last()));
}

#[test]
fn req_run_012_repeats_turn_without_a_step_and_bean_stitch_counts_each_point() {
    // 1 mm at 0.4 mm: 5 half spacings, made 6, so 7 points.
    let once = run(&east(1.0), 2.0, &[]);
    assert_eq!(once.len(), 7);
    // 2 repeats: there, then back from the point before the turn.
    let twice = run(&east(1.0), 2.0, &[("repeats", "2")]);
    let back: Vec<(f64, f64)> = once.iter().rev().skip(1).copied().collect();
    assert_eq!(twice, [once.clone(), back].concat());
    // Bean stitch "1 0" over both passes, a step for each stitch: the turn takes none, so the first stitch
    // back, the 7th, is sewn 3 times.
    let beaned = |passes: &[(f64, f64)]| {
        let mut expected = vec![passes[0]];
        for (k, pair) in passes.windows(2).enumerate() {
            expected.push(pair[1]);
            if k % 2 == 0 {
                expected.extend([pair[0], pair[1]]);
            }
        }
        expected
    };
    assert_eq!(run(&east(1.0), 2.0, &[("repeats", "2"), ("bean_stitch_repeats", "1 0")]), beaned(&twice));
    // A third pass goes there again from the point after the second turn, and no turn takes a step.
    let thrice = [twice, once.iter().skip(1).copied().collect()].concat();
    assert_eq!(run(&east(1.0), 2.0, &[("repeats", "3")]), thrice);
    assert_eq!(run(&east(1.0), 2.0, &[("repeats", "3"), ("bean_stitch_repeats", "1 0")]), beaned(&thrice));
}

#[test]
fn diag_sc_e0009_a_zigzag_stroke_without_a_spacing_is_not_sewn() {
    // The registry reads 1 spacing or more, so only a bug in StitchCraft can leave the list empty.
    let set = ParamSet::new();
    let mut zigzag = ZigzagStrokeParams::from_set(&set).unwrap().params;
    zigzag.zigzag_spacing_mm.clear();
    let passes = RepeatParams::from_set(&set).unwrap().params;
    let mm = |value| Mm::new(value).unwrap();
    let stitched = zigzag_stroke(&east(5.0), mm(2.0), &zigzag, &passes, mm(0.3), &mut Budget::DEFAULT.meter()).unwrap();
    assert!(stitched.runs.is_empty());
    assert_eq!(stitched.warnings.iter().map(|d| d.code).collect::<Vec<_>>(), [Code::InternalCheckFailed]);
}

#[test]
fn diag_sc_w0401_a_part_that_is_one_point_is_not_stitched() {
    let stitched = sewn(&polylines(&[&[(5.0, 5.0)], &[(0.0, 0.0), (1.0, 0.0)]]), 2.0, &[]);
    assert_eq!(stitched.runs.len(), 1);
    let said: Vec<(Code, &str)> = stitched.warnings.iter().map(|d| (d.code, d.message.as_str())).collect();
    assert_eq!(said, [(Code::StrokeTooSmall, "A part of the stroke is a single point, so it is not stitched.")]);
    assert!(stitched.runs[0].iter().all(|q: &Point| q.y().abs() > 0.9));
}
