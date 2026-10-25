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
//!
//! The area a wide stroke paints is made of its segments one by one (`flat_segments`): its joins go
//! where its segments meet, and its curves bend smoothly between. Its ends and its corners are set square
//! to the chords that end there, and half the stroke's width turns the small angle a chord makes with its
//! curve into a shift larger than the tolerance. So a curve is halved further where it ends the subpath or
//! meets the next segment at a corner, until the chord there lies within the angle the stroke's width
//! allows. Where 2 segments meet smoothly, their chords turn alike on either side, and the join between
//! them is within the tolerance already.

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

/// The directions a subpath starts and ends in, each a vector of any length: [`end_directions`].
pub(crate) type Ends = ((f64, f64), (f64, f64));

/// A segment of a subpath that moves, flattened: the points after its start, and the directions it starts
/// and ends in.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct FlatSegment {
    /// Its points after its start, in order, the last its end, none the same as the one before it. A curve
    /// that comes back to its start within the tolerance has none.
    pub(crate) points: Vec<Point>,
    /// The direction it starts in: towards its first control point that is not its start. Not of unit
    /// length.
    pub(crate) start_direction: (f64, f64),
    /// The direction it ends in: from its last control point that is not its end. Not of unit length.
    pub(crate) end_direction: (f64, f64),
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
    // The segments that do not move are dropped, so what is left starts where the subpath does.
    let mut points = vec![subpath.start];
    let mut corners = Vec::new();
    let mut previous: Option<(f64, f64)> = None;
    for segment in flat_segments(subpath, tolerance, None, meter)? {
        if previous.is_some_and(|end| turns(end, segment.start_direction)) {
            corners.push(points.len() - 1);
        }
        for p in segment.points {
            push(&mut points, p);
        }
        previous = Some(segment.end_direction);
    }
    let last = points.len() - 1;
    corners.retain(|c| *c > 0 && *c < last);
    corners.dedup();
    Ok(Piece { points, corners })
}

/// The segments of `subpath` that move, each flattened within `tolerance`, in order, with the line that
/// closes a closed subpath. With `ends`, a curve is halved further at each of its ends that ends the
/// subpath or meets the next segment at a corner, one that turns by an angle whose sine is more than
/// `ends`, until the chord there leaves the curve at an angle whose sine is at most `ends`. Each segment
/// and each halving costs one unit of `meter`.
pub(crate) fn flat_segments(subpath: &Subpath, tolerance: f64, ends: Option<f64>, meter: &mut Meter) -> Result<Vec<FlatSegment>, Exhausted> {
    let segs = moving(subpath, meter)?;
    let mut flat = Vec::with_capacity(segs.len());
    for (i, &seg) in segs.iter().enumerate() {
        // The segments before and after it: around a closed subpath, the last comes before the first.
        let before = if i > 0 { segs.get(i - 1) } else { segs.last().filter(|_| subpath.closed) };
        let after = segs.get(i + 1).or_else(|| segs.first().filter(|_| subpath.closed));
        // Square at an end of the subpath, or at a corner.
        let square = |turn: Option<((f64, f64), (f64, f64))>| ends.filter(|&sine| turn.is_none_or(|(from, to)| !within(from, to, sine)));
        let squares =
            [square(before.map(|b| (b.end_direction(), seg.start_direction()))), square(after.map(|a| (seg.end_direction(), a.start_direction())))];
        let (start, mut points) = match seg {
            Seg::Line(start, end) => (start, vec![end]),
            Seg::Cubic(a, b, c, d) => {
                let mut points = vec![a];
                cubic([a, b, c, d], tolerance, squares, MAX_DEPTH, &mut points, meter)?;
                (a, points)
            }
        };
        // The start is the end of the segment before, and not the segment's own.
        if points.first() == Some(&start) {
            points.remove(0);
        }
        flat.push(FlatSegment { points, start_direction: seg.start_direction(), end_direction: seg.end_direction() });
    }
    Ok(flat)
}

/// The directions `subpath` starts and ends in, as its first and last segments that move start and end
/// ([`FlatSegment`]); `None` when it does not move. Each segment costs one unit of `meter`.
pub(crate) fn end_directions(subpath: &Subpath, meter: &mut Meter) -> Result<Option<Ends>, Exhausted> {
    let segs = moving(subpath, meter)?;
    Ok(segs.first().zip(segs.last()).map(|(first, last)| (first.start_direction(), last.end_direction())))
}

/// The segments of `subpath` that move, each with its start, and the line that closes a closed subpath.
/// Each segment costs one unit of `meter`.
fn moving(subpath: &Subpath, meter: &mut Meter) -> Result<Vec<Seg>, Exhausted> {
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
    Ok(segs)
}

/// Appends `p` unless it is where the polyline already is.
fn push(points: &mut Vec<Point>, p: Point) {
    if points.last() != Some(&p) {
        points.push(p);
    }
}

/// Appends the points of a flattened cubic, its start excluded, halving it at most `max_depth` times
/// (into at most 2^`max_depth` pieces) however tight the tolerance. With `squares`, the piece at its start
/// and the one at its end are halved until their chords leave the curve at an angle whose sine is at most
/// the one given for that end.
fn cubic(
    curve: [Point; 4],
    tolerance: f64,
    [at_start, at_end]: [Option<f64>; 2],
    max_depth: u32,
    points: &mut Vec<Point>,
    meter: &mut Meter,
) -> Result<(), Exhausted> {
    // Each piece, with whether it starts the curve and whether it ends it.
    let mut stack = vec![(curve, 0, true, true)];
    while let Some(([a, b, c, d], depth, first, last)) = stack.pop() {
        meter.charge(1)?;
        let flat = distance_to_segment(b, a, d) <= tolerance && distance_to_segment(c, a, d) <= tolerance;
        let chord = (d.x() - a.x(), d.y() - a.y());
        let (x, y) = towards(d, &[c, b, a]);
        let square = |end: bool, bound: Option<f64>, direction: (f64, f64)| !end || bound.is_none_or(|sine| within(direction, chord, sine));
        if depth >= max_depth || (flat && square(first, at_start, towards(a, &[b, c, d])) && square(last, at_end, (-x, -y))) {
            push(points, d);
            continue;
        }
        // Halve it (de Casteljau), and do the first half first.
        let (ab, bc, cd) = (a.lerp(b, 0.5), b.lerp(c, 0.5), c.lerp(d, 0.5));
        let (abc, bcd) = (ab.lerp(bc, 0.5), bc.lerp(cd, 0.5));
        let middle = abc.lerp(bcd, 0.5);
        stack.push(([middle, bcd, cd, d], depth + 1, false, last));
        stack.push(([a, ab, abc, middle], depth + 1, first, false));
    }
    Ok(())
}

/// Whether `other` goes the way of `direction`, at an angle to it whose sine is at most `sine`.
fn within(direction: (f64, f64), other: (f64, f64), sine: f64) -> bool {
    let cross = direction.0 * other.1 - direction.1 * other.0;
    let dot = direction.0 * other.0 + direction.1 * other.1;
    let lengths = (direction.0 * direction.0 + direction.1 * direction.1).sqrt() * (other.0 * other.0 + other.1 * other.1).sqrt();
    dot > 0.0 && cross.abs() <= sine * lengths
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
            cubic([p(0.0, 0.0), p(0.0, 10.0), p(10.0, 10.0), p(10.0, 0.0)], tolerance, [None, None], 2, &mut points, &mut meter).unwrap();
            assert_eq!(points.len(), 1 + 4, "{tolerance}");
        }
    }

    #[test]
    fn a_curve_is_halved_at_its_ends_until_its_end_chords_leave_it_square() {
        // The quarter circle from (10, 0) to (0, 10): it starts upwards and ends leftwards. Flattened within
        // the shapes' tolerance, its first chord leaves it at some 3°. Asked for a sine of 0.001, its first
        // and last chords leave it within that, and the points between stay as they were.
        let k = 10.0 * 0.552_284_749_830_793_4;
        let arc = Subpath { start: p(10.0, 0.0), segments: vec![Segment::Cubic(p(10.0, k), p(k, 10.0), p(0.0, 10.0))], closed: false };
        let meter = &mut Budget::DEFAULT.meter();
        let [plain] = flat_segments(&arc, SHAPE_TOLERANCE, None, meter).unwrap().try_into().unwrap();
        let [squared] = flat_segments(&arc, SHAPE_TOLERANCE, Some(0.001), meter).unwrap().try_into().unwrap();
        assert_eq!((squared.start_direction, squared.end_direction), ((0.0, k), (-k, 0.0)));
        let sine = |from: Point, to: Point, direction: (f64, f64)| {
            let (x, y) = (to.x() - from.x(), to.y() - from.y());
            (x * direction.1 - y * direction.0).abs() / (x * x + y * y).sqrt() / (direction.0 * direction.0 + direction.1 * direction.1).sqrt()
        };
        assert!(sine(p(10.0, 0.0), plain.points[0], (0.0, 1.0)) > 0.01);
        assert!(sine(p(10.0, 0.0), squared.points[0], (0.0, 1.0)) <= 0.001);
        let ending = squared.points.len();
        assert!(sine(squared.points[ending - 2], squared.points[ending - 1], (-1.0, 0.0)) <= 0.001);
        assert!(plain.points.iter().all(|q| squared.points.contains(q)), "halving further keeps the points");
        assert!(squared.points.len() <= plain.points.len() + 2 * 12, "{} points, from {}", squared.points.len(), plain.points.len());
        // A segment's start is not its own point, and a line is its end.
        let line = Subpath { start: p(0.0, 0.0), segments: vec![Segment::Line(p(5.0, 0.0))], closed: true };
        let flat = flat_segments(&line, SHAPE_TOLERANCE, Some(0.001), meter).unwrap();
        let ends: Vec<Vec<Point>> = flat.into_iter().map(|segment| segment.points).collect();
        assert_eq!(ends, [vec![p(5.0, 0.0)], vec![p(0.0, 0.0)]], "the closing line included");
    }

    #[test]
    fn segments_that_meet_smoothly_are_not_halved_further() {
        // A circle of 4 quarters, closed: each meets the next smoothly, the last the first, and it has no
        // end, so asking for a sine of 0.001 halves nothing further. Open, its first and last ends are
        // halved further, as for the quarter circle above. The same turned by 45°, so that no direction
        // lies along an axis.
        let k = 10.0 * 0.552_284_749_830_793_4;
        let r = std::f64::consts::FRAC_1_SQRT_2;
        for turned in [false, true] {
            let at = |x: f64, y: f64| if turned { p(r * (x - y), r * (x + y)) } else { p(x, y) };
            let quarters = vec![
                Segment::Cubic(at(10.0, k), at(k, 10.0), at(0.0, 10.0)),
                Segment::Cubic(at(-k, 10.0), at(-10.0, k), at(-10.0, 0.0)),
                Segment::Cubic(at(-10.0, -k), at(-k, -10.0), at(0.0, -10.0)),
                Segment::Cubic(at(k, -10.0), at(10.0, -k), at(10.0, 0.0)),
            ];
            let flat = |closed, ends| {
                let circle = Subpath { start: at(10.0, 0.0), segments: quarters.clone(), closed };
                let segments = flat_segments(&circle, SHAPE_TOLERANCE, ends, &mut Budget::DEFAULT.meter()).unwrap();
                segments.into_iter().map(|segment| segment.points.len()).collect::<Vec<_>>()
            };
            assert_eq!(flat(true, Some(0.001)), flat(true, None), "turned: {turned}");
            let (open, plain) = (flat(false, Some(0.001)), flat(false, None));
            let further = |i: usize| open[i] - plain[i];
            assert!(
                (1..=12).contains(&further(0)) && (1..=12).contains(&further(3)) && open[1..3] == plain[1..3],
                "turned: {turned}: {open:?} against {plain:?}"
            );
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
