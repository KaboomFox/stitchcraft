//! Unit tests of the running stitch's steps; the conformance cases are in `tests/running.rs`.

use stitchcraft_core::Budget;

use super::*;

fn fitted(length: f64, pattern: &[f64], min: f64) -> Vec<f64> {
    fit(length, pattern, &mut 0, min, &mut Budget::DEFAULT.meter()).unwrap()
}

fn close(a: &[f64], b: &[f64]) -> bool {
    a.len() == b.len() && a.iter().zip(b).all(|(x, y)| (x - y).abs() < 1e-9)
}

#[test]
fn spans_are_divided_evenly() {
    // A whole number of stitches long: no extra stitch for rounding.
    assert!(close(&fitted(10.0, &[2.5], 0.3), &[2.5; 4]));
    assert!(close(&fitted(0.1 + 0.2 + 0.7, &[0.5], 0.0), &[0.5; 2]), "0.1 + 0.2 + 0.7 is not exactly 1");
    // A little more: one more stitch, all of them shorter.
    assert!(close(&fitted(10.1, &[2.5], 0.3), &[2.02; 5]));
    // Shorter than one stitch: one stitch.
    assert!(close(&fitted(1.0, &[2.5], 0.3), &[1.0]));
}

#[test]
fn patterns_carry_on_from_span_to_span() {
    let mut next = 0;
    let mut meter = Budget::DEFAULT.meter();
    let first = fit(8.0, &[3.0, 1.0], &mut next, 0.3, &mut meter).unwrap();
    assert!(close(&first, &[3.0, 1.0, 3.0, 1.0]));
    // The next span starts where the pattern left off, and shrinks the stitches to end on its corner.
    let second = fit(3.6, &[3.0, 1.0], &mut next, 0.3, &mut meter).unwrap();
    assert!(close(&second, &[2.7, 0.9]));
    assert_eq!(next, 6);
}

#[test]
fn stitches_shrunk_below_the_shortest_stitch_join_a_neighbour() {
    // 0.7 mm of a (0.6, 5) pattern: 5.6 mm of pattern shrunk to an eighth, 0.075 and 0.625; the first is
    // too short and joins the second.
    assert!(close(&fitted(0.7, &[0.6, 5.0], 0.3), &[0.7]));
    // Three stitches where the middle one is too short: it joins the shorter of its neighbours.
    let lengths = fitted(5.0, &[2.0, 1.0, 4.0], 0.8);
    assert!(lengths.iter().all(|l| *l >= 0.8 - 1e-9), "{lengths:?}");
    assert!((lengths.iter().sum::<f64>() - 5.0).abs() < 1e-9);
}

#[test]
fn short_lengths_are_raised_to_twice_the_shortest_stitch() {
    let mm = |v: f64| Mm::new(v).unwrap();
    let mut warnings = Vec::new();
    assert_eq!(pattern(&[mm(2.5)], 0.3, &mut warnings), [2.5]);
    assert!(warnings.is_empty());
    assert_eq!(pattern(&[mm(0.5)], 0.3, &mut warnings), [0.6]);
    assert_eq!(pattern(&[mm(0.2), mm(3.0), mm(0.4)], 0.3, &mut warnings), [0.6, 3.0, 0.6]);
    assert_eq!(pattern(&[], 0.3, &mut warnings), [0.6]);
    let messages: Vec<String> = warnings.iter().map(ToString::to_string).collect();
    assert_eq!(
        messages,
        [
            "warning SC-W0402: The stitch length 0.5 mm is shorter than twice the shortest stitch (0.3 mm), so 0.6 mm is used.",
            "warning SC-W0402: The stitch lengths 0.2, 0.4 mm are shorter than twice the shortest stitch (0.3 mm), so 0.6 mm is used for each.",
            "warning SC-W0402: No stitch length is given, so 0.6 mm, twice the shortest stitch, is used.",
        ]
    );
}

#[test]
fn millimetres_read_well() {
    assert_eq!((mm(2.5), mm(0.3), mm(10.0), mm(0.0), mm(0.126)), ("2.5".into(), "0.3".into(), "10".into(), "0".into(), "0.13".into()));
}

#[test]
fn the_budget_bounds_the_work() {
    let mut meter = Budget { max_stitches: 1, max_work: 3 }.meter();
    assert_eq!(fit(100.0, &[1.0], &mut 0, 0.3, &mut meter), Err(Exhausted::Work));
}

fn point(x: f64, y: f64) -> Point {
    Point::new(x, y).unwrap()
}

/// A piece through `points`, with no corners (the needles given to [`space`] say which are corners).
fn piece(points: &[(f64, f64)]) -> Piece {
    Piece { points: points.iter().map(|(x, y)| point(*x, *y)).collect(), corners: Vec::new() }
}

#[test]
fn sides_leave_circles_where_they_cross_them() {
    let near = |t: Option<f64>, expected: f64| t.is_some_and(|t| (t - expected).abs() < 1e-12);
    let leaves = |p: (f64, f64), q: (f64, f64)| leaves(Point::ORIGIN, 1.0, point(p.0, p.1), point(q.0, q.1));
    assert!(near(leaves((0.0, 0.0), (2.0, 0.0)), 0.5));
    assert!(near(leaves((0.5, 0.0), (2.0, 0.0)), 1.0 / 3.0), "moving away from the centre");
    assert!(near(leaves((0.5, 0.0), (-2.0, 0.0)), 0.6), "passing the centre first");
    assert!(near(leaves((0.0, 0.0), (1.0, 0.0)), 1.0), "ending on the circle");
    assert!(near(leaves((1.0, 0.0), (1.0, 0.0)), 0.0), "a side that does not move, on the circle");
    assert_eq!(leaves((0.0, 0.0), (0.5, 0.0)), None, "still inside");
}

#[test]
fn the_farthest_point_from_both_ends() {
    let mut meter = Budget::DEFAULT.meter();
    // A closed square: the far corner.
    let square = piece(&[(0.0, 0.0), (1.0, 0.0), (1.0, 1.0), (0.0, 1.0), (0.0, 0.0)]);
    let along = Along::new(&square, &mut meter).unwrap();
    let (distance, far) = along.farthest(point(0.0, 0.0), point(0.0, 0.0), &mut meter).unwrap().unwrap();
    assert_eq!((distance, far.point, far.at), (2.0_f64.sqrt(), point(1.0, 1.0), 2.0));
    // A U whose ends are 0.5 apart: halfway along the bottom, not at either corner.
    let u = piece(&[(0.0, 0.0), (0.0, 1.0), (0.5, 1.0), (0.5, 0.0)]);
    let along = Along::new(&u, &mut meter).unwrap();
    let (distance, far) = along.farthest(point(0.0, 0.0), point(0.5, 0.0), &mut meter).unwrap().unwrap();
    assert_eq!((distance, far.point, far.at), (1.0625_f64.sqrt(), point(0.25, 1.0), 1.25));
    // Two points equally far: the first.
    let triangle = piece(&[(0.0, 0.0), (1.0, 1.0), (1.0, -1.0), (0.0, 0.0)]);
    let along = Along::new(&triangle, &mut meter).unwrap();
    let (_, far) = along.farthest(point(0.0, 0.0), point(0.0, 0.0), &mut meter).unwrap().unwrap();
    assert_eq!(far.point, point(1.0, 1.0));
}

/// [`space`] on needles at `ats` along `piece` (those in `corners` on corners), for a shortest stitch of
/// 0.3 and a longest length of 2.5: the positions of the needles it keeps or adds.
fn spaced(piece: &Piece, ats: &[f64], corners: &[f64]) -> Option<Vec<f64>> {
    let mut meter = Budget::DEFAULT.meter();
    let along = Along::new(piece, &mut meter).unwrap();
    let needles = ats.iter().map(|at| Needle { corner: corners.contains(at), ..along.needle(*at) }).collect();
    let kept = space(needles, &along, 0.3, 2.5, &mut meter).unwrap()?;
    assert!(kept.iter().all(|n| n.point.distance(along.needle(n.at).point) < 1e-12), "points match their distances along");
    Some(kept.iter().map(|n| (n.at * 1e9).round() / 1e9).collect())
}

#[test]
fn needles_closer_than_the_shortest_stitch_are_dropped() {
    let line = piece(&[(0.0, 0.0), (10.0, 0.0)]);
    let even = Some(vec![0.0, 2.5, 5.0, 7.5, 10.0]);
    assert_eq!(spaced(&line, &[0.0, 2.5, 5.0, 7.5, 10.0], &[]), even, "nothing to do");
    assert_eq!(spaced(&line, &[0.0, 2.5, 2.6, 5.0, 7.5, 10.0], &[]), even, "the later of two close needles goes");
    // A corner drops the needle before it instead; the stitch to it is then too long, and is halved.
    assert_eq!(spaced(&line, &[0.0, 2.5, 2.6, 5.0, 7.5, 10.0], &[2.6]), Some(vec![0.0, 1.3, 2.6, 5.0, 7.5, 10.0]));
    // ... but not the start, nor another corner: then it goes itself.
    assert_eq!(spaced(&line, &[0.0, 0.1, 2.5, 5.0, 7.5, 10.0], &[0.1]), even);
    assert_eq!(spaced(&line, &[0.0, 2.5, 2.6, 5.0, 7.5, 10.0], &[2.5, 2.6]), even);
    // The end drops whatever is too close before it, corners too.
    assert_eq!(spaced(&line, &[0.0, 2.5, 5.0, 7.5, 9.9, 10.0], &[9.9]), even);
    // A stitch more than twice the longest length is split a longest length at a time.
    assert_eq!(spaced(&line, &[0.0, 10.0], &[]), even);
}

#[test]
fn a_closed_piece_goes_by_way_of_its_farthest_point() {
    let square = piece(&[(0.0, 0.0), (1.0, 0.0), (1.0, 1.0), (0.0, 1.0), (0.0, 0.0)]);
    assert_eq!(spaced(&square, &[0.0, 4.0], &[]), Some(vec![0.0, 2.0, 4.0]));
    // All of it within the shortest stitch of its ends: nothing to stitch.
    let tiny = piece(&[(0.0, 0.0), (0.15, 0.0), (0.15, 0.15), (0.0, 0.15), (0.0, 0.0)]);
    assert_eq!(spaced(&tiny, &[0.0, 0.6], &[]), None);
}
