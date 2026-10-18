//! Strokes as polylines: curves flattened within a tolerance, and the corners marked.
//!
//! Running stitches are placed along a stroke by distance, so the stroke becomes a polyline first. Each
//! curve is halved (de Casteljau) until its control points lie within the tolerance of the line between
//! its ends: a Bézier curve lies inside the hull of its control points, so the whole curve is then within
//! the tolerance of that line too, and the line's ends lie on the curve. Lines stay as they are. Only
//! additions, multiplications and square roots are used, so every platform computes the same points
//! (`docs/src/design/determinism.md`).
//!
//! Corners are where the stroke turns sharply between two of its segments: by more than 30° from the
//! direction one segment ends in to the direction the next one starts in. A curve's own bend is never a
//! corner, however tight; the running stitch follows it within its tolerance instead. Segments that do
//! not move (a zero-length line, a curve whose points all coincide) are dropped first, so every corner
//! is measured between two segments that have a direction. A subpath with nothing else is kept as a
//! piece of a single point, so that a generator reports it instead of losing it without a word.
//!
//! A line a generator builds point by point, such as a satin column's underlay, is a polyline already:
//! every join between its segments may be a corner, by the same 30°.

use stitchcraft_core::units::MM_PER_SVG_PX;
use stitchcraft_core::{Exhausted, Meter, Point};

use crate::design::{Path, Segment, Subpath};

/// How far Ink/Stitch lets a shape's polylines stray from its curves, in millimetres: a tenth of a CSS
/// pixel. A fill's subpaths and a satin column's are flattened within it, by the same halving, so that
/// their points are the ones Ink/Stitch works from. A running stitch follows its own tolerance.
pub const SHAPE_TOLERANCE: f64 = 0.1 * MM_PER_SVG_PX;

/// The cosine of 30°: a join turns by more than 30° when the cosine of its turn is below this.
const CORNER_COS: f64 = 0.866_025_403_784_438_6;

/// The most times one curve is halved: 2⁴⁰ pieces is far finer than any tolerance needs, and the budget
/// stops runaway work long before.
const MAX_DEPTH: u32 = 40;

/// A stroke ready for stitching: its subpaths as polylines, in drawing order.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct StrokePath {
    /// The pieces, one per subpath, in drawing order.
    pub pieces: Vec<Piece>,
}

/// One subpath as a polyline.
#[derive(Clone, Debug, PartialEq)]
pub struct Piece {
    /// The points, in order, no two in a row the same: at least two, or one for a subpath that does not
    /// move. A closed subpath ends where it starts.
    pub points: Vec<Point>,
    /// The corners, as indices into `points`, in increasing order. The first and last points are never
    /// listed: they always get a stitch.
    pub corners: Vec<usize>,
}

/// `path` as polylines within `tolerance` millimetres of it, with their corners: one piece per subpath.
/// Each segment and each halving of a curve costs one unit of `meter`.
pub fn flatten(path: &Path, tolerance: f64, meter: &mut Meter) -> Result<StrokePath, Exhausted> {
    let mut pieces = Vec::with_capacity(path.subpaths.len());
    for subpath in &path.subpaths {
        pieces.push(piece(subpath, tolerance, meter)?);
    }
    Ok(StrokePath { pieces })
}

/// The polyline `points` as a piece: without points that repeat the one before, and with a corner at each
/// join that turns by more than 30°.
pub(crate) fn polyline(points: &[Point]) -> Piece {
    let mut kept = Vec::with_capacity(points.len());
    for point in points {
        push(&mut kept, *point);
    }
    let corners = kept
        .windows(3)
        .enumerate()
        .filter(|(_, w)| matches!(w, [a, b, c] if turns(towards(*a, &[*b]), towards(*b, &[*c]))))
        .map(|(i, _)| i + 1)
        .collect();
    Piece { points: kept, corners }
}

/// The line segments of the polyline `points`, each from a point to the next.
pub(crate) fn segments(points: &[Point]) -> impl Iterator<Item = (Point, Point)> + '_ {
    points.iter().copied().zip(points.iter().copied().skip(1))
}

/// How far `p` is from the line segment from `a` to `b`, in millimetres.
pub fn distance_to_segment(p: Point, a: Point, b: Point) -> f64 {
    p.distance(nearest_on_segment(p, a, b))
}

/// The point of the line segment from `a` to `b` nearest `p`.
pub fn nearest_on_segment(p: Point, a: Point, b: Point) -> Point {
    let (dx, dy) = (b.x() - a.x(), b.y() - a.y());
    let length2 = dx * dx + dy * dy;
    if length2 == 0.0 {
        return a;
    }
    a.lerp(b, ((p.x() - a.x()) * dx + (p.y() - a.y()) * dy) / length2)
}

/// One segment with its start: a line, or a cubic Bézier curve (quadratics are raised to cubics).
#[derive(Clone, Copy, Debug)]
enum Seg {
    Line(Point, Point),
    Cubic(Point, Point, Point, Point),
}

impl Seg {
    fn new(start: Point, segment: Segment) -> Seg {
        match segment {
            Segment::Line(end) => Seg::Line(start, end),
            // The same curve as a cubic: each inner control point two thirds of the way to the quadratic's.
            Segment::Quad(control, end) => Seg::Cubic(start, start.lerp(control, 2.0 / 3.0), end.lerp(control, 2.0 / 3.0), end),
            Segment::Cubic(first, second, end) => Seg::Cubic(start, first, second, end),
        }
    }

    /// Whether it does not move at all.
    fn is_point(self) -> bool {
        match self {
            Seg::Line(a, b) => a == b,
            Seg::Cubic(a, b, c, d) => a == b && b == c && c == d,
        }
    }

    /// The direction it starts in: towards its first control point that is not its start.
    fn start_direction(self) -> (f64, f64) {
        match self {
            Seg::Line(a, b) => towards(a, &[b]),
            Seg::Cubic(a, b, c, d) => towards(a, &[b, c, d]),
        }
    }

    /// The direction it ends in: from its last control point that is not its end.
    fn end_direction(self) -> (f64, f64) {
        let (x, y) = match self {
            Seg::Line(a, b) => towards(b, &[a]),
            Seg::Cubic(a, b, c, d) => towards(d, &[c, b, a]),
        };
        (-x, -y)
    }
}

/// The direction from `from` to the first of `points` that differs from it.
fn towards(from: Point, points: &[Point]) -> (f64, f64) {
    points.iter().find(|p| **p != from).map_or((0.0, 0.0), |p| (p.x() - from.x(), p.y() - from.y()))
}

/// Whether going on in direction `next` after direction `previous` turns by more than 30°. A direction
/// of no length turns nowhere: the dot product and the lengths are then both 0.
fn turns(previous: (f64, f64), next: (f64, f64)) -> bool {
    let dot = previous.0 * next.0 + previous.1 * next.1;
    let lengths = (previous.0 * previous.0 + previous.1 * previous.1).sqrt() * (next.0 * next.0 + next.1 * next.1).sqrt();
    dot < CORNER_COS * lengths
}

fn piece(subpath: &Subpath, tolerance: f64, meter: &mut Meter) -> Result<Piece, Exhausted> {
    let mut segs = Vec::with_capacity(subpath.segments.len() + 1);
    let mut at = subpath.start;
    for segment in &subpath.segments {
        meter.charge(1)?;
        segs.push(Seg::new(at, *segment));
        at = segment.end();
    }
    if subpath.closed && at != subpath.start {
        segs.push(Seg::Line(at, subpath.start));
    }
    segs.retain(|s| !s.is_point());
    // The dropped segments do not move, so what is left starts where the subpath does.
    let mut points = vec![subpath.start];
    let mut corners = Vec::new();
    let mut previous: Option<Seg> = None;
    for seg in segs {
        if previous.is_some_and(|p| turns(p.end_direction(), seg.start_direction())) {
            corners.push(points.len() - 1);
        }
        match seg {
            Seg::Line(_, end) => push(&mut points, end),
            Seg::Cubic(a, b, c, d) => cubic([a, b, c, d], tolerance, MAX_DEPTH, &mut points, meter)?,
        }
        previous = Some(seg);
    }
    let last = points.len() - 1;
    corners.retain(|c| *c > 0 && *c < last);
    corners.dedup();
    Ok(Piece { points, corners })
}

/// Appends `p` unless it is where the polyline already is.
fn push(points: &mut Vec<Point>, p: Point) {
    if points.last() != Some(&p) {
        points.push(p);
    }
}

/// Appends the points of a flattened cubic, its start excluded, halving it at most `max_depth` times
/// (into at most 2^`max_depth` pieces) however tight the tolerance.
fn cubic(curve: [Point; 4], tolerance: f64, max_depth: u32, points: &mut Vec<Point>, meter: &mut Meter) -> Result<(), Exhausted> {
    let mut stack = vec![(curve, 0)];
    while let Some(([a, b, c, d], depth)) = stack.pop() {
        meter.charge(1)?;
        if depth >= max_depth || (distance_to_segment(b, a, d) <= tolerance && distance_to_segment(c, a, d) <= tolerance) {
            push(points, d);
            continue;
        }
        // Halve it (de Casteljau), and do the first half first.
        let (ab, bc, cd) = (a.lerp(b, 0.5), b.lerp(c, 0.5), c.lerp(d, 0.5));
        let (abc, bcd) = (ab.lerp(bc, 0.5), bc.lerp(cd, 0.5));
        let middle = abc.lerp(bcd, 0.5);
        stack.push(([middle, bcd, cd, d], depth + 1));
        stack.push(([a, ab, abc, middle], depth + 1));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use stitchcraft_core::{Budget, math};

    use super::*;

    fn p(x: f64, y: f64) -> Point {
        Point::new(x, y).unwrap()
    }

    fn path(start: Point, segments: Vec<Segment>, closed: bool) -> Path {
        Path { subpaths: vec![Subpath { start, segments, closed }] }
    }

    fn flat(path: &Path, tolerance: f64) -> StrokePath {
        flatten(path, tolerance, &mut Budget::DEFAULT.meter()).unwrap()
    }

    #[test]
    fn a_square_has_corners_at_its_inner_vertices() {
        let square = path(p(0.0, 0.0), vec![Segment::Line(p(10.0, 0.0)), Segment::Line(p(10.0, 10.0)), Segment::Line(p(0.0, 10.0))], true);
        let piece = &flat(&square, 0.1).pieces[0];
        assert_eq!(piece.points, [p(0.0, 0.0), p(10.0, 0.0), p(10.0, 10.0), p(0.0, 10.0), p(0.0, 0.0)]);
        assert_eq!(piece.corners, [1, 2, 3]);
    }

    #[test]
    fn corners_turn_by_more_than_30_degrees() {
        let turn = |degrees: f64| {
            let (s, c) = math::sin_cos(math::to_radians(degrees));
            let bent = path(p(0.0, 0.0), vec![Segment::Line(p(10.0, 0.0)), Segment::Line(p(10.0 + 10.0 * c, 10.0 * s))], false);
            flat(&bent, 0.1).pieces[0].corners.len()
        };
        assert_eq!((turn(29.0), turn(31.0), turn(90.0), turn(179.0)), (0, 1, 1, 1));
        // Exactly 30° is not more than 30°: (cos 30°, ½ + an ulp) is exactly one long.
        let (cos, sin) = (CORNER_COS, 0.5 + f64::EPSILON / 2.0);
        assert_eq!((cos * cos + sin * sin).sqrt(), 1.0);
        assert!(!turns((1.0, 0.0), (cos, sin)));
    }

    #[test]
    fn curves_bend_without_corners_and_stay_within_the_tolerance() {
        // A quarter circle of radius 10 as one cubic (the standard 0.5523 handles), then on in its
        // direction: no corners anywhere, and every point on the circle.
        let k = 10.0 * 0.552_284_749_830_793_4;
        let arc = path(p(10.0, 0.0), vec![Segment::Cubic(p(10.0, k), p(k, 10.0), p(0.0, 10.0)), Segment::Line(p(-5.0, 10.0))], false);
        let piece = &flat(&arc, 0.01).pieces[0];
        assert!(piece.corners.is_empty());
        assert!(piece.points.len() > 8, "a fine tolerance needs many points: {}", piece.points.len());
        for q in &piece.points[..piece.points.len() - 1] {
            // The cubic is within 0.03 % of the circle; the flattening adds nothing.
            assert!((q.distance(Point::ORIGIN) - 10.0).abs() < 0.003, "{q:?}");
        }
        // The same curve met at a right angle by a line: one corner, where they join.
        let kinked = path(p(10.0, 0.0), vec![Segment::Cubic(p(10.0, k), p(k, 10.0), p(0.0, 10.0)), Segment::Line(p(0.0, 20.0))], false);
        let piece = &flat(&kinked, 0.01).pieces[0];
        assert_eq!(piece.corners.len(), 1);
        assert_eq!(piece.points[piece.corners[0]], p(0.0, 10.0));
    }

    #[test]
    fn quadratics_are_the_same_curve_as_cubics() {
        let quad = flat(&path(p(0.0, 0.0), vec![Segment::Quad(p(5.0, 10.0), p(10.0, 0.0))], false), 0.001);
        // The quadratic's middle is at (5, 5).
        let middle = quad.pieces[0].points.iter().map(|q| q.distance(p(5.0, 5.0))).fold(f64::MAX, f64::min);
        assert!(middle < 0.002, "{middle}");
    }

    #[test]
    fn segments_that_do_not_move_are_dropped() {
        let stuttering = path(
            p(0.0, 0.0),
            vec![Segment::Line(p(0.0, 0.0)), Segment::Line(p(5.0, 0.0)), Segment::Line(p(5.0, 0.0)), Segment::Line(p(10.0, 0.0))],
            false,
        );
        let piece = &flat(&stuttering, 0.1).pieces[0];
        assert_eq!((piece.points.len(), piece.corners.len()), (3, 0));
        let still = path(p(1.0, 1.0), vec![Segment::Line(p(1.0, 1.0)), Segment::Cubic(p(1.0, 1.0), p(1.0, 1.0), p(1.0, 1.0))], true);
        assert_eq!(flat(&still, 0.1).pieces, [Piece { points: vec![p(1.0, 1.0)], corners: vec![] }], "a point, kept to be reported");
        // A curve with a coincident control point still has a direction at each end, and is kept.
        let curl = path(p(0.0, 0.0), vec![Segment::Line(p(10.0, 0.0)), Segment::Cubic(p(10.0, 0.0), p(20.0, 0.0), p(20.0, 10.0))], false);
        let piece = &flat(&curl, 0.1).pieces[0];
        assert!(piece.corners.is_empty(), "the curve starts straight on");
        assert_eq!(piece.points.last(), Some(&p(20.0, 10.0)));
        let both = path(p(0.0, 0.0), vec![Segment::Line(p(10.0, 0.0)), Segment::Cubic(p(10.0, 0.0), p(20.0, 10.0), p(20.0, 10.0))], false);
        assert_eq!(flat(&both, 0.1).pieces[0].points.last(), Some(&p(20.0, 10.0)), "coincident at both ends, and still a curve");
        // Dropped segments do not hide a corner: the turn is measured across them.
        for still in [Segment::Line(p(10.0, 0.0)), Segment::Cubic(p(10.0, 0.0), p(10.0, 0.0), p(10.0, 0.0))] {
            let elbow = path(p(0.0, 0.0), vec![Segment::Line(p(10.0, 0.0)), still, Segment::Line(p(10.0, 10.0))], false);
            assert_eq!(flat(&elbow, 0.1).pieces[0].corners, [1], "{still:?}");
        }
    }

    #[test]
    fn the_ends_are_never_corners() {
        // A loop smaller than the tolerance flattens to nothing. At the start, the turn after it would
        // land on the first point; at the end, on the last.
        let first = path(p(0.0, 0.0), vec![Segment::Cubic(p(0.01, 0.01), p(0.02, 0.0), p(0.0, 0.0)), Segment::Line(p(10.0, 0.0))], false);
        let last = path(p(0.0, 0.0), vec![Segment::Line(p(10.0, 0.0)), Segment::Cubic(p(10.0, 0.01), p(10.01, 0.01), p(10.0, 0.0))], false);
        for looped in [first, last] {
            let piece = &flat(&looped, 1.0).pieces[0];
            assert_eq!((piece.points.as_slice(), piece.corners.as_slice()), ([p(0.0, 0.0), p(10.0, 0.0)].as_slice(), [].as_slice()));
        }
    }

    #[test]
    fn lopsided_curves_stay_within_the_tolerance() {
        // One control point on the chord, the other far off it: halving goes on until both are close.
        let (a, b, c, d) = (p(0.0, 0.0), p(5.0, 0.0), p(5.0, 10.0), p(10.0, 0.0));
        let piece = &flat(&path(a, vec![Segment::Cubic(b, c, d)], false), 0.05).pieces[0];
        for i in 0..=400 {
            let t = f64::from(i) / 400.0;
            let u = 1.0 - t;
            let w = [u * u * u, 3.0 * u * u * t, 3.0 * u * t * t, t * t * t];
            let q = p(w[0] * a.x() + w[1] * b.x() + w[2] * c.x() + w[3] * d.x(), w[0] * a.y() + w[1] * b.y() + w[2] * c.y() + w[3] * d.y());
            let off = piece.points.windows(2).map(|s| distance_to_segment(q, s[0], s[1])).fold(f64::MAX, f64::min);
            assert!(off <= 0.05 + 1e-9, "t = {t}: {off}");
        }
    }

    #[test]
    fn the_depth_limit_caps_the_pieces() {
        // A tolerance no halving meets (none, or NaN) stops at 2^depth pieces, not at the budget.
        for tolerance in [0.0, f64::NAN] {
            let mut points = vec![p(0.0, 0.0)];
            let mut meter = Budget { max_stitches: 1, max_work: 1000 }.meter();
            cubic([p(0.0, 0.0), p(0.0, 10.0), p(10.0, 10.0), p(10.0, 0.0)], tolerance, 2, &mut points, &mut meter).unwrap();
            assert_eq!(points.len(), 1 + 4, "{tolerance}");
        }
    }

    #[test]
    fn a_polyline_drops_repeated_points_and_has_corners_where_it_turns_sharply() {
        let points = [p(0.0, 0.0), p(5.0, 0.0), p(5.0, 0.0), p(10.0, 1.0), p(10.0, 6.0), p(10.0, 10.0)];
        let piece = polyline(&points);
        assert_eq!(piece.points, [p(0.0, 0.0), p(5.0, 0.0), p(10.0, 1.0), p(10.0, 6.0), p(10.0, 10.0)]);
        // A turn of 11° at (5, 0), of 79° at (10, 1), and none at (10, 6).
        assert_eq!(piece.corners, [2]);
        // Straight on across the y axis, and a right angle away from the origin.
        assert!(polyline(&[p(1.0, 0.0), p(-1.0, 0.0), p(-3.0, 0.0)]).corners.is_empty());
        assert_eq!(polyline(&[p(10.0, 10.0), p(5.0, 10.0), p(5.0, 15.0)]).corners, [1]);
        assert_eq!(polyline(&[p(1.0, 1.0), p(1.0, 1.0)]), Piece { points: vec![p(1.0, 1.0)], corners: vec![] });
    }

    #[test]
    fn distances_to_segments() {
        assert_eq!(distance_to_segment(p(5.0, 3.0), p(0.0, 0.0), p(10.0, 0.0)), 3.0);
        assert_eq!(distance_to_segment(p(13.0, 4.0), p(0.0, 0.0), p(10.0, 0.0)), 5.0, "beyond the end");
        assert_eq!(distance_to_segment(p(3.0, 4.0), p(0.0, 0.0), p(0.0, 0.0)), 5.0, "to a point");
        assert_eq!(nearest_on_segment(p(5.0, 3.0), p(0.0, 0.0), p(10.0, 0.0)), p(5.0, 0.0));
        assert_eq!(nearest_on_segment(p(-2.0, 1.0), p(0.0, 0.0), p(10.0, 0.0)), p(0.0, 0.0), "before the start");
        assert_eq!(nearest_on_segment(p(3.0, 4.0), p(1.0, 1.0), p(1.0, 1.0)), p(1.0, 1.0), "a segment that is a point");
    }

    #[test]
    fn the_budget_bounds_the_work() {
        // Run out while flattening a curve, and while reading a path's segments.
        let curve = path(p(0.0, 0.0), vec![Segment::Cubic(p(0.0, 100.0), p(100.0, 100.0), p(100.0, 0.0))], false);
        let mut meter = Budget { max_stitches: 1, max_work: 20 }.meter();
        assert_eq!(flatten(&curve, 0.0001, &mut meter), Err(Exhausted::Work));
        let lines = path(p(0.0, 0.0), vec![Segment::Line(p(1.0, 0.0)), Segment::Line(p(2.0, 0.0))], false);
        let mut meter = Budget { max_stitches: 1, max_work: 0 }.meter();
        assert_eq!(flatten(&lines, 0.0001, &mut meter), Err(Exhausted::Work));
    }
}
