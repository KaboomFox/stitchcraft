//! Manual stitch's conformance cases (`REQ-RUN-006`, `REQ-RUN-008`) and `SC-W0403`, through the engine's
//! public API.

// Test code may unwrap and index (clippy.toml allows it in tests).
#![allow(clippy::unwrap_used)]

use proptest::prelude::*;
use stitchcraft_core::{Budget, Code, Mm, Point};
use stitchcraft_engine::design::{Path, Segment, Subpath};
use stitchcraft_engine::generators::Stitched;
use stitchcraft_engine::generators::manual::{ManualParams, manual_stitch};
use stitchcraft_engine::generators::passes::RepeatParams;
use stitchcraft_params::ParamSet;

fn p(x: f64, y: f64) -> Point {
    Point::new(x, y).unwrap()
}

/// `path` sewn by hand with `settings` (Ink/Stitch keys and values) and the shortest stitch `min`.
fn sew(path: &Path, settings: &[(&str, &str)], min: f64) -> Stitched {
    let set: ParamSet = settings.iter().copied().collect();
    let manual = ManualParams::from_set(&set).unwrap().params;
    let passes = RepeatParams::from_set(&set).unwrap().params;
    manual_stitch(path, &manual, &passes, Mm::new(min).unwrap(), &mut Budget::DEFAULT.meter()).unwrap()
}

/// One open subpath of straight lines through `points`.
fn lines(points: &[(f64, f64)]) -> Path {
    let (first, rest) = points.split_first().unwrap();
    Path {
        subpaths: vec![Subpath { start: p(first.0, first.1), segments: rest.iter().map(|q| Segment::Line(p(q.0, q.1))).collect(), closed: false }],
    }
}

fn messages(stitched: &Stitched) -> Vec<String> {
    stitched.warnings.iter().map(ToString::to_string).collect()
}

#[test]
fn req_run_006_manual_stitch_puts_a_needle_on_every_node() {
    // Lines and a curve: the curve's control points are not stitched. Closed: back to the start.
    let path = Path {
        subpaths: vec![Subpath {
            start: p(0.0, 0.0),
            segments: vec![Segment::Line(p(3.0, 0.0)), Segment::Line(p(3.0, 0.0)), Segment::Cubic(p(9.0, 9.0), p(-9.0, 9.0), p(3.0, 4.0))],
            closed: true,
        }],
    };
    let sewn = sew(&path, &[], 0.3);
    assert_eq!(sewn.runs, [vec![p(0.0, 0.0), p(3.0, 0.0), p(3.0, 4.0), p(0.0, 0.0)]]);
    assert!(sewn.warnings.is_empty());
    // A stitch longer than the longest is split evenly; repeats do not apply; bean stitch does.
    let long = sew(&lines(&[(0.0, 0.0), (10.0, 0.0)]), &[("max_stitch_length_mm", "3"), ("repeats", "3")], 0.3);
    assert_eq!(long.runs, [vec![p(0.0, 0.0), p(2.5, 0.0), p(5.0, 0.0), p(7.5, 0.0), p(10.0, 0.0)]]);
    let bean = sew(&lines(&[(0.0, 0.0), (2.0, 0.0)]), &[("bean_stitch_repeats", "1")], 0.3);
    assert_eq!(bean.runs, [vec![p(0.0, 0.0), p(2.0, 0.0), p(0.0, 0.0), p(2.0, 0.0)]]);
}

#[test]
fn req_run_006_a_longest_stitch_of_zero_or_less_sews_every_stitch_as_drawn() {
    // Ink/Stitch reads 0 and below as "no maximum", so a file that says so is not cut into short stitches.
    let path = lines(&[(0.0, 0.0), (10.0, 0.0), (10.0, 5.0)]);
    for raw in ["0", "-1", ""] {
        let sewn = sew(&path, &[("max_stitch_length_mm", raw)], 0.3);
        assert_eq!(sewn.runs, [vec![p(0.0, 0.0), p(10.0, 0.0), p(10.0, 5.0)]], "{raw:?}");
        assert!(sewn.warnings.is_empty(), "{raw:?}");
    }
    let set: ParamSet = [("max_stitch_length_mm", "0")].into_iter().collect();
    let read = ManualParams::from_set(&set).unwrap();
    assert_eq!((read.params.max_stitch_length_mm, read.warnings.len()), (None, 0));
}

#[test]
fn req_run_008_no_hand_placed_stitch_is_shorter_than_the_shortest_stitch() {
    let sewn = sew(&lines(&[(0.0, 0.0), (0.1, 0.0), (5.0, 0.0), (5.2, 0.0)]), &[], 0.3);
    // (0.1, 0) is too close to the start; (5, 0) too close to the last point, which stays.
    assert_eq!(sewn.runs, [vec![p(0.0, 0.0), p(5.2, 0.0)]]);
    assert_eq!(sewn.warnings.len(), 1);
}

#[test]
fn diag_sc_w0403_hand_placed_stitches_shorter_than_the_shortest_stitch_are_named() {
    let one = sew(&lines(&[(0.0, 0.0), (0.1, 0.0), (5.0, 0.0)]), &[], 0.3);
    assert_eq!(
        messages(&one),
        ["warning SC-W0403: A hand-placed stitch is 0.1 mm long, shorter than the shortest stitch (0.3 mm), so a needle point is left out."]
    );
    let two = sew(&lines(&[(0.0, 0.0), (0.1, 0.0), (5.0, 0.0), (5.25, 0.0), (9.0, 0.0)]), &[], 0.3);
    assert_eq!(
        messages(&two),
        [
            "warning SC-W0403: 2 hand-placed stitches are shorter than the shortest stitch (0.3 mm), the shortest 0.1 mm long, so a needle point of each is left out."
        ]
    );
    assert_eq!(two.warnings[0].code, Code::HandStitchTooShort);
    // Too small for even one stitch, or a single point: SC-W0401, as for the running stitch.
    let tiny = sew(&lines(&[(0.0, 0.0), (0.2, 0.0)]), &[], 0.3);
    assert_eq!(
        messages(&tiny),
        ["warning SC-W0401: A part of the stroke is 0.2 mm long, shorter than the shortest stitch (0.3 mm), so it is not stitched."]
    );
    let point = sew(&lines(&[(4.0, 4.0), (4.0, 4.0)]), &[], 0.3);
    assert_eq!(messages(&point), ["warning SC-W0401: A part of the stroke is a single point, so it is not stitched."]);
    // Long enough, but curled up: a closed square 0.2 mm across.
    let square = Path {
        subpaths: vec![Subpath {
            start: p(0.0, 0.0),
            segments: vec![Segment::Line(p(0.2, 0.0)), Segment::Line(p(0.2, 0.2)), Segment::Line(p(0.0, 0.2))],
            closed: true,
        }],
    };
    assert_eq!(
        messages(&sew(&square, &[], 0.3)),
        [
            "warning SC-W0401: A part of the stroke is 0.8 mm long, but all of it lies within the shortest stitch (0.3 mm) of its ends, so it is not stitched."
        ]
    );
}

proptest! {
    #![proptest_config(stitchcraft_testkit::strategies::config(256))]

    /// Any hand-placed polyline: the stitches are at least the shortest stitch, the needle points are the
    /// polyline's nodes in order (the last one always), and each node left out is counted in the warning.
    #[test]
    fn req_run_008_points_are_nodes_in_order_and_far_enough_apart(
        steps in prop::collection::vec((-3.0..3.0_f64, -3.0..3.0_f64), 1..20),
        min in 0.05..1.0_f64,
    ) {
        let mut points = vec![(0.0, 0.0)];
        for (dx, dy) in &steps {
            let last = *points.last().unwrap();
            points.push((last.0 + dx, last.1 + dy));
        }
        let sewn = sew(&lines(&points), &[], min);
        let nodes: Vec<Point> = points.iter().map(|q| p(q.0, q.1)).collect();
        for run in &sewn.runs {
            for s in run.windows(2) {
                prop_assert!(s[0].distance(s[1]) >= min - 1e-9, "{} < {min}", s[0].distance(s[1]));
            }
            prop_assert_eq!(run.last(), nodes.last());
            // A subsequence of the nodes, in order.
            let mut next = nodes.iter();
            prop_assert!(run.iter().all(|q| next.any(|n| n == q)), "{run:?} is not a subsequence of the nodes");
        }
        prop_assert_eq!(sewn.runs.len() + sewn.warnings.iter().filter(|w| w.code == Code::StrokeTooSmall).count(), 1);
    }
}
