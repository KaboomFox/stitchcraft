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
    // Shorter than one stitch: one stitch. No pattern at all: one stitch too.
    assert!(close(&fitted(1.0, &[2.5], 0.3), &[1.0]));
    assert!(close(&fitted(4.0, &[], 0.3), &[4.0]));
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
    // Three stitches, 10/7, 5/7 and 20/7, where the middle one is too short: it joins the shorter of its
    // neighbours, the first.
    assert!(close(&fitted(5.0, &[2.0, 1.0, 4.0], 0.8), &[15.0 / 7.0, 20.0 / 7.0]));
    // Neighbours of the same length: the earlier one.
    assert!(close(&fitted(4.0, &[2.0, 1.0, 2.0], 0.9), &[2.4, 1.6]));
}

#[test]
fn short_lengths_are_raised_to_twice_the_shortest_stitch() {
    let mm = |v: f64| Mm::new(v).unwrap();
    let mut warnings = Vec::new();
    assert_eq!(pattern(&[mm(2.5)], 0.3, &mut warnings), [2.5]);
    assert_eq!(pattern(&[mm(0.6)], 0.3, &mut warnings), [0.6], "exactly twice is enough");
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
    // Off the axes, round a centre away from the origin, outwards and past the centre: the point is on
    // the circle, and the side is inside it until there.
    let centre = point(1.0, 1.0);
    for (p, q) in [((1.3, 1.4), (2.8, 3.4)), ((1.5, 1.2), (1.5, 4.0)), ((0.2, 1.5), (3.0, -0.5))] {
        let (p, q) = (point(p.0, p.1), point(q.0, q.1));
        let t = super::leaves(centre, 1.5, p, q).unwrap();
        assert!((p.lerp(q, t).distance(centre) - 1.5).abs() < 1e-12, "{p:?} {q:?} {t}");
        assert!(p.lerp(q, t - 1e-6).distance(centre) < 1.5, "the first crossing");
    }
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
    // The same U turned and moved away from the origin: every coordinate matters.
    let (s, e) = ((1.0, 2.0), (1.3, 2.4));
    let turned = piece(&[s, (0.2, 2.6), (0.5, 3.0), e]);
    let along = Along::new(&turned, &mut meter).unwrap();
    let (distance, far) = along.farthest(point(s.0, s.1), point(e.0, e.1), &mut meter).unwrap().unwrap();
    assert!((distance - 1.0625_f64.sqrt()).abs() < 1e-12 && far.point.distance(point(0.35, 2.8)) < 1e-12 && (far.at - 1.25).abs() < 1e-12);
    // Nothing is that far: no crossing.
    let line = piece(&[(0.0, 0.0), (1.0, 0.0), (2.0, 0.0)]);
    let along = Along::new(&line, &mut meter).unwrap();
    assert!(along.crossing(&along.vertex(0), 5.0, &along.vertex(2), &mut meter).unwrap().is_none());
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

/// [`stitch_piece`] along `points` with the corners at `corners` (indices), for one stitch length, a
/// shortest stitch of 0.3 and a deviation budget of 0.18: the needle points, rounded to a micrometre.
fn stitched(points: &[(f64, f64)], corners: &[usize], length: f64) -> Vec<(f64, f64)> {
    let mut meter = Budget::DEFAULT.meter();
    let piece = Piece { corners: corners.to_vec(), ..piece(points) };
    let along = Along::new(&piece, &mut meter).unwrap();
    let run = stitch_piece(&along, &piece.corners, &[length], 0.3, 0.18, &mut meter).unwrap().unwrap();
    run.iter().map(|q| ((q.x() * 1e6).round() / 1e6, (q.y() * 1e6).round() / 1e6)).collect()
}

#[test]
fn corners_closer_than_the_shortest_stitch_along_the_path_are_not_cut() {
    // The second of two corners 0.2 apart: its spans are joined, and the 5.2 mm after the first corner
    // take three even stitches.
    let step = stitched(&[(0.0, 0.0), (5.0, 0.0), (5.0, 0.2), (10.0, 0.2)], &[1, 2], 2.5);
    assert_eq!(step, [(0.0, 0.0), (2.5, 0.0), (5.0, 0.0), (6.533333, 0.2), (8.266667, 0.2), (10.0, 0.2)]);
    // A corner 0.2 before the end: the last span runs on to the end.
    let hook = stitched(&[(0.0, 0.0), (10.0, 0.0), (10.0, 0.2)], &[1], 2.5);
    assert_eq!(hook, [(0.0, 0.0), (2.04, 0.0), (4.08, 0.0), (6.12, 0.0), (8.16, 0.0), (10.0, 0.2)]);
}

#[test]
fn a_corner_keeps_its_needle_over_the_points_before_it() {
    // The path folds back before the corner at (2, 0.2), so the even spacing puts a needle at (2.1, 0),
    // 0.22 from the corner in a straight line. The needle goes, not the corner.
    let fold = stitched(&[(0.0, 0.0), (3.0, 0.0), (3.0, 0.2), (2.0, 0.2), (2.0, 5.0)], &[3], 2.5);
    assert_eq!(fold, [(0.0, 0.0), (2.0, 0.2), (2.0, 2.6), (2.0, 5.0)]);
}

/// [`follow`] from the first point of `points` to the last, for a deviation budget, a shortest stitch of
/// 0.3 and no practical longest length: the needle points.
fn followed(points: &[(f64, f64)], budget: f64) -> Vec<Point> {
    let mut meter = Budget::DEFAULT.meter();
    let piece = piece(points);
    let along = Along::new(&piece, &mut meter).unwrap();
    let needles = vec![along.vertex(0), along.vertex(points.len() - 1)];
    follow(needles, &along, budget, 0.3, 100.0, &mut meter).unwrap().iter().map(|n| n.point).collect()
}

#[test]
fn curves_are_split_at_their_farthest_point_while_they_stray_too_far() {
    // (1, 1) and (3, 1) both stray 1 from the stitch: the first is taken, and then nothing strays more
    // than 0.7.
    assert_eq!(followed(&[(0.0, 0.0), (1.0, 1.0), (2.0, 0.0), (3.0, 1.0), (4.0, 0.0)], 0.7), [point(0.0, 0.0), point(1.0, 1.0), point(4.0, 0.0)]);
    // Straying exactly the budget is not too far.
    assert_eq!(followed(&[(0.0, 0.0), (2.0, 1.0), (4.0, 0.0)], 1.0), [point(0.0, 0.0), point(4.0, 0.0)]);
}
