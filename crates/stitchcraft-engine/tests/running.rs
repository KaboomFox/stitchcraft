//! The running stitch's conformance cases (`REQ-RUN-001` to `REQ-RUN-003`) and its diagnostics, through
//! the engine's public API.

// Test code may unwrap and index (clippy.toml allows it in tests).
#![allow(clippy::unwrap_used)]

use std::f64::consts::PI;

use proptest::prelude::*;
use stitchcraft_core::{Budget, Code, Mm, Point, math};
use stitchcraft_engine::design::{Path, Segment, Subpath};
use stitchcraft_engine::generators::running::{RunningParams, Stitched, running_stitch};
use stitchcraft_engine::normalize::stroke::distance_to_segment;
use stitchcraft_params::ParamSet;

fn p(x: f64, y: f64) -> Point {
    Point::new(x, y).unwrap()
}

fn params(length: &str, tolerance: &str) -> RunningParams {
    let set: ParamSet = [("running_stitch_length_mm", length), ("running_stitch_tolerance_mm", tolerance)].into_iter().collect();
    RunningParams::from_set(&set).unwrap().params
}

fn stitch(path: &Path, params: &RunningParams, min: f64) -> Stitched {
    running_stitch(path, params, Mm::new(min).unwrap(), &mut Budget::DEFAULT.meter()).unwrap()
}

/// A path through `points`, closed or not.
fn polyline(points: &[(f64, f64)], closed: bool) -> Path {
    let (first, rest) = points.split_first().unwrap();
    Path { subpaths: vec![Subpath { start: p(first.0, first.1), segments: rest.iter().map(|q| Segment::Line(p(q.0, q.1))).collect(), closed }] }
}

/// A circle of `radius` round (`cx`, `cy`), as four cubic curves (the usual 0.5523 handles).
fn circle(cx: f64, cy: f64, radius: f64) -> Path {
    let k = radius * 0.552_284_749_830_793_4;
    let segments = vec![
        Segment::Cubic(p(cx + radius, cy + k), p(cx + k, cy + radius), p(cx, cy + radius)),
        Segment::Cubic(p(cx - k, cy + radius), p(cx - radius, cy + k), p(cx - radius, cy)),
        Segment::Cubic(p(cx - radius, cy - k), p(cx - k, cy - radius), p(cx, cy - radius)),
        Segment::Cubic(p(cx + k, cy - radius), p(cx + radius, cy - k), p(cx + radius, cy)),
    ];
    Path { subpaths: vec![Subpath { start: p(cx + radius, cy), segments, closed: true }] }
}

/// A cubic curve with a cusp at (5, 7.5): it comes to a point there and goes back the way it came.
fn cusp() -> Path {
    Path { subpaths: vec![Subpath { start: p(0.0, 0.0), segments: vec![Segment::Cubic(p(10.0, 10.0), p(0.0, 10.0), p(10.0, 0.0))], closed: false }] }
}

/// Points of the source path, `per_segment` along each segment (Bernstein form, independent of the
/// engine's halving).
fn samples(path: &Path, per_segment: u32) -> Vec<Point> {
    let mut out = Vec::new();
    for subpath in &path.subpaths {
        let mut from = subpath.start;
        for segment in &subpath.segments {
            for i in 0..=per_segment {
                let t = f64::from(i) / f64::from(per_segment);
                let u = 1.0 - t;
                let (x, y) = match *segment {
                    Segment::Line(e) => (from.x() * u + e.x() * t, from.y() * u + e.y() * t),
                    Segment::Quad(c, e) => {
                        let w = [u * u, 2.0 * u * t, t * t];
                        (w[0] * from.x() + w[1] * c.x() + w[2] * e.x(), w[0] * from.y() + w[1] * c.y() + w[2] * e.y())
                    }
                    Segment::Cubic(c1, c2, e) => {
                        let w = [u * u * u, 3.0 * u * u * t, 3.0 * u * t * t, t * t * t];
                        (
                            w[0] * from.x() + w[1] * c1.x() + w[2] * c2.x() + w[3] * e.x(),
                            w[0] * from.y() + w[1] * c1.y() + w[2] * c2.y() + w[3] * e.y(),
                        )
                    }
                };
                out.push(p(x, y));
            }
            from = segment.end();
        }
    }
    out
}

/// How far `q` is from the stitches of `run`.
fn off_run(q: Point, run: &[Point]) -> f64 {
    run.windows(2).map(|s| distance_to_segment(q, s[0], s[1])).fold(f64::MAX, f64::min)
}

/// How far `q` is from the source points (dense enough to stand for the curve).
fn off_curve(q: Point, curve: &[Point]) -> f64 {
    curve.windows(2).map(|s| distance_to_segment(q, s[0], s[1])).fold(f64::MAX, f64::min)
}

fn lengths(run: &[Point]) -> Vec<f64> {
    run.windows(2).map(|s| s[0].distance(s[1])).collect()
}

#[test]
fn req_run_001_stitches_are_spread_evenly_between_corners() {
    // 100 mm of straight line: 40 stitches of exactly 2.5 mm.
    let straight = stitch(&polyline(&[(0.0, 0.0), (100.0, 0.0)], false), &params("2.5", "0.2"), 0.3);
    assert_eq!(straight.runs.len(), 1);
    assert!(lengths(&straight.runs[0]).iter().all(|l| (l - 2.5).abs() < 1e-9));
    assert_eq!(straight.runs[0].len(), 41);
    // An L: 10 mm then, round a corner, 7 mm: four stitches of 2.5, then three of 7/3, none left over.
    let ell = stitch(&polyline(&[(0.0, 0.0), (10.0, 0.0), (10.0, 7.0)], false), &params("2.5", "0.2"), 0.3);
    let l = lengths(&ell.runs[0]);
    assert_eq!(l.len(), 7);
    assert!(l[..4].iter().all(|x| (x - 2.5).abs() < 1e-9) && l[4..].iter().all(|x| (x - 7.0 / 3.0).abs() < 1e-9), "{l:?}");
    // A pattern: long, short, long, short.
    let pattern = stitch(&polyline(&[(0.0, 0.0), (8.0, 0.0)], false), &params("3 1", "0.2"), 0.3);
    let l = lengths(&pattern.runs[0]);
    assert!(l.len() == 4 && (l[0] - 3.0).abs() < 1e-9 && (l[1] - 1.0).abs() < 1e-9, "{l:?}");
    assert!(straight.warnings.is_empty() && ell.warnings.is_empty() && pattern.warnings.is_empty());
}

#[test]
fn req_run_001_stitches_are_measured_straight_where_the_path_bends_back() {
    // A stitch is the straight line between two needle points. Around a cusp, two points far apart
    // along the path can be close in a straight line; with a tolerance too loose to split the stitch
    // between them, it would be shorter than the shortest stitch.
    for length in ["1", "1.5", "2", "2.5", "3", "3.5", "4"] {
        let stitched = stitch(&cusp(), &params(length, "1"), 0.3);
        let longest: f64 = length.parse().unwrap();
        let l = lengths(&stitched.runs[0]);
        assert!(l.iter().all(|x| *x >= 0.3 - 1e-9 && *x <= longest + 1e-9), "{length}: {l:?}");
    }
    // A circle 0.5 mm across, with a tolerance it fits within: two stitches across it, not one in place.
    let ring = stitch(&circle(0.0, 0.0, 0.25), &params("2.5", "1"), 0.3);
    let l = lengths(&ring.runs[0]);
    assert!(l.len() == 2 && l.iter().all(|x| *x >= 0.3), "{l:?}");
    assert!(ring.warnings.is_empty());
}

#[test]
fn req_run_002_curves_are_followed_within_the_tolerance() {
    for (radius, tolerance) in [(20.0, 0.2), (2.0, 0.2), (5.0, 0.05), (40.0, 1.0)] {
        let path = circle(50.0, 50.0, radius);
        let stitched = stitch(&path, &params("2.5", &tolerance.to_string()), 0.3);
        let run = &stitched.runs[0];
        let curve = samples(&path, 400);
        let worst_curve = curve.iter().map(|q| off_run(*q, run)).fold(0.0, f64::max);
        // The stitches' own points and midpoints, against the curve.
        let mut on_stitches = run.clone();
        on_stitches.extend(run.windows(2).map(|s| s[0].lerp(s[1], 0.5)));
        let worst_stitch = on_stitches.iter().map(|q| off_curve(*q, &curve)).fold(0.0, f64::max);
        assert!(worst_curve <= tolerance + 1e-6 && worst_stitch <= tolerance + 1e-6, "r {radius}: {worst_curve} {worst_stitch}");
        assert!(lengths(run).iter().all(|l| *l <= 2.5 + 1e-9 && *l >= 0.3 - 1e-9), "r {radius}");
    }
}

#[test]
fn req_run_003_corners_are_needle_penetrations() {
    // A ten-pointed star: every one of its corners is a needle point.
    let star: Vec<(f64, f64)> = (0..=10)
        .map(|i| {
            let r = if i % 2 == 0 { 40.0 } else { 15.0 };
            let (s, c) = math::sin_cos(f64::from(i) * PI / 5.0);
            (50.0 + r * c, 50.0 + r * s)
        })
        .collect();
    let stitched = stitch(&polyline(&star, false), &params("2.5", "0.2"), 0.3);
    let run = &stitched.runs[0];
    for corner in &star[1..10] {
        assert!(run.contains(&p(corner.0, corner.1)), "{corner:?} is not a needle point");
    }
    // Corners closer together than the shortest stitch are the exception: no stitch is shorter.
    let zigzag: Vec<(f64, f64)> = (0..=20).map(|i| (f64::from(i) * 0.2, if i % 2 == 0 { 0.0 } else { 0.2 })).collect();
    let stitched = stitch(&polyline(&zigzag, false), &params("2.5", "0.2"), 0.3);
    assert!(lengths(&stitched.runs[0]).iter().all(|l| *l >= 0.3 - 1e-9), "{:?}", stitched.runs[0]);
}

#[test]
fn diag_sc_w0401_a_part_shorter_than_the_shortest_stitch_is_named_and_skipped() {
    let path = Path {
        subpaths: vec![
            Subpath { start: p(0.0, 0.0), segments: vec![Segment::Line(p(0.2, 0.0))], closed: false },
            Subpath { start: p(10.0, 0.0), segments: vec![Segment::Line(p(20.0, 0.0))], closed: false },
        ],
    };
    let stitched = stitch(&path, &params("2.5", "0.2"), 0.3);
    assert_eq!(stitched.runs.len(), 1, "the 10 mm part is stitched");
    assert_eq!(
        stitched.warnings.iter().map(ToString::to_string).collect::<Vec<_>>(),
        ["warning SC-W0401: A part of the stroke is 0.2 mm long, shorter than the shortest stitch (0.3 mm), so it is not stitched."]
    );
    // Long enough, but too small: a closed square 0.15 mm a side is 0.6 mm round, yet its far corner is
    // only 0.21 mm from where it starts and ends.
    let square = polyline(&[(0.0, 0.0), (0.15, 0.0), (0.15, 0.15), (0.0, 0.15)], true);
    let stitched = stitch(&square, &params("2.5", "0.2"), 0.3);
    assert!(stitched.runs.is_empty(), "{:?}", stitched.runs);
    assert_eq!(
        stitched.warnings.iter().map(ToString::to_string).collect::<Vec<_>>(),
        [
            "warning SC-W0401: A part of the stroke is 0.6 mm long, but all of it lies within the shortest stitch (0.3 mm) of its ends, so it is not stitched."
        ]
    );
}

#[test]
fn diag_sc_w0402_a_stitch_length_below_twice_the_shortest_stitch_is_raised() {
    let stitched = stitch(&polyline(&[(0.0, 0.0), (6.0, 0.0)], false), &params("0.4", "0.2"), 0.3);
    assert_eq!(
        stitched.warnings.iter().map(ToString::to_string).collect::<Vec<_>>(),
        ["warning SC-W0402: The stitch length 0.4 mm is shorter than twice the shortest stitch (0.3 mm), so 0.6 mm is used."]
    );
    assert!(lengths(&stitched.runs[0]).iter().all(|l| (l - 0.6).abs() < 1e-9));
}

/// A random stroke: lines and curves through a square `extent` mm each way from the origin, possibly
/// closed.
fn stroke(extent: f64) -> impl Strategy<Value = Path> {
    let coordinate = -extent..extent;
    let point = (coordinate.clone(), coordinate).prop_map(|(x, y)| p(x, y));
    let segment = prop_oneof![
        3 => point.clone().prop_map(Segment::Line),
        1 => (point.clone(), point.clone()).prop_map(|(c, e)| Segment::Quad(c, e)),
        2 => (point.clone(), point.clone(), point.clone()).prop_map(|(c1, c2, e)| Segment::Cubic(c1, c2, e)),
    ];
    (point, prop::collection::vec(segment, 1..6), any::<bool>())
        .prop_map(|(start, segments, closed)| Path { subpaths: vec![Subpath { start, segments, closed }] })
}

/// A random pattern of one to three lengths.
fn pattern() -> impl Strategy<Value = String> {
    prop::collection::vec(0.2..8.0_f64, 1..4).prop_map(|l| l.iter().map(|x| format!("{x:.2}")).collect::<Vec<_>>().join(" "))
}

proptest! {
    #![proptest_config(stitchcraft_testkit::strategies::config(256))]

    /// Whatever the stroke, the pattern and the shortest stitch: every stitch, measured straight, is
    /// between the shortest stitch and the longest length of the pattern (raised to twice the shortest
    /// stitch); the run starts and ends where the stroke does; and a stroke that is not stitched is
    /// reported. Tiny strokes (a few millimetres) have loops and cusps about as small as the shortest
    /// stitch.
    #[test]
    fn req_run_001_no_stitch_is_too_long_or_too_short(
        path in prop_oneof![stroke(50.0), stroke(2.0)],
        pattern in pattern(),
        min in 0.05..0.8_f64,
        tolerance in 0.05..1.0_f64,
    ) {
        let params = params(&pattern, &format!("{tolerance:.3}"));
        let longest = params.running_stitch_length_mm.iter().map(|l| l.get().max(2.0 * min)).fold(0.0, f64::max);
        let stitched = stitch(&path, &params, min);
        for run in &stitched.runs {
            prop_assert!(run.len() >= 2);
            for l in lengths(run) {
                prop_assert!(l >= min - 1e-9 && l <= longest + 1e-9, "{l} not within [{min}, {longest}]");
            }
        }
        let subpath = &path.subpaths[0];
        let end = if subpath.closed { subpath.start } else { subpath.segments.last().unwrap().end() };
        if let Some(run) = stitched.runs.first() {
            prop_assert_eq!((run[0], *run.last().unwrap()), (subpath.start, end));
        }
        let skipped = stitched.warnings.iter().filter(|w| w.code == Code::StrokeTooSmall).count();
        prop_assert_eq!(stitched.runs.len() + skipped, 1, "stitched or reported: {:?}", stitched.warnings);
    }

    /// Circular arcs of any size above a few millimetres, joined by straight lines: the stitches stay
    /// within the tolerance of the stroke, both ways round.
    #[test]
    fn req_run_002_any_gentle_curve_is_followed_within_the_tolerance(radius in 2.0..60.0_f64, length in 1.0..6.0_f64, tolerance in 0.05..1.0_f64) {
        let path = circle(0.0, 0.0, radius);
        let stitched = stitch(&path, &params(&format!("{length:.2}"), &format!("{tolerance:.3}")), 0.3);
        let run = &stitched.runs[0];
        let curve = samples(&path, 200);
        for q in &curve {
            prop_assert!(off_run(*q, run) <= tolerance + 1e-6);
        }
        for s in run.windows(2) {
            prop_assert!(off_curve(s[0].lerp(s[1], 0.5), &curve) <= tolerance + 1e-6);
        }
    }

    /// Polylines with sides of at least a millimetre: every turn of more than 30° is a needle point.
    #[test]
    fn req_run_003_every_sharp_turn_is_a_needle_point(turns in prop::collection::vec((-180.0..180.0_f64, 1.0..20.0_f64), 2..12)) {
        let mut points = vec![(0.0, 0.0)];
        let mut heading = 0.0_f64;
        for (turn, side) in &turns {
            heading += turn;
            let (s, c) = math::sin_cos(math::to_radians(heading));
            let last = *points.last().unwrap();
            points.push((last.0 + side * c, last.1 + side * s));
        }
        let stitched = stitch(&polyline(&points, false), &params("2.5", "0.2"), 0.3);
        let run = &stitched.runs[0];
        // The turn at each inner point, measured independently with atan2.
        for w in points.windows(3) {
            let (u, v) = ((w[1].0 - w[0].0, w[1].1 - w[0].1), (w[2].0 - w[1].0, w[2].1 - w[1].1));
            let angle = math::atan2(u.0 * v.1 - u.1 * v.0, u.0 * v.0 + u.1 * v.1).abs();
            if angle > PI / 6.0 + 1e-9 {
                prop_assert!(run.contains(&p(w[1].0, w[1].1)), "the corner {:?} turns {angle} rad", w[1]);
            }
        }
    }
}
