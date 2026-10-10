//! Nearest points: of polylines to a point, of 2 segments to each other, of polylines to polylines.
//!
//! Where an element starts and ends depends on where its neighbours are (`docs/src/design/adr/
//! 0014-generators-see-their-neighbours.md`), measured as the nearest points between shapes. Shapes are
//! often equally near at several points: rails side by side, an end on a rail, a side along a cut. Of those
//! the first found is taken, so every platform picks the same, and they are found as the geometry library
//! Ink/Stitch uses finds them, so the same one as there. Each rule below was checked against that library.
//!
//! - **Order.** One shape is measured from, against the other: each of its polylines against each of the
//!   other's in turn, and side by side within them, its own sides outermost. Within 2 sides, the measured
//!   side's points nearest the other's start and end come before its own start and end
//!   ([`between_segments`]).
//! - **Meeting.** Sides that cross meet where they cross, and sides that touch meet at the end that
//!   touches, exactly ([`meeting`]). Whether an end lies on a side is decided in double precision, and the
//!   library decides it exactly: an end within a rounding error of a side can touch it here and not there,
//!   where the library takes the foot of that end instead, a rounding error away.
//! - **Measures.** Points are as far apart as the square root of the sum of the squares says. A point is
//!   as far from a side as from its nearer end when its foot lies beyond one, and otherwise as far as it is
//!   square to the side. Sides are 0 apart where they meet, and otherwise as far as the nearest of their
//!   4 ends is from the other side.
//!
//! Only arithmetic and square roots are used (`docs/src/design/determinism.md`).

use stitchcraft_core::{Exhausted, Meter, Point};

/// The sides of the polyline `points`, each from a point to the next; a polyline of one point is one side
/// of no length.
fn sides(points: &[Point]) -> impl Iterator<Item = (Point, Point)> + '_ {
    let single = (points.len() == 1).then(|| points.first().map(|&p| (p, p))).flatten();
    points.windows(2).filter_map(|w| Some((*w.first()?, *w.get(1)?))).chain(single)
}

/// How far apart `p` and `q` are.
pub(crate) fn length(p: Point, q: Point) -> f64 {
    let (dx, dy) = (q.x() - p.x(), q.y() - p.y());
    (dx * dx + dy * dy).sqrt()
}

/// The fraction of the way from `a` to `b` where the foot of `p` on their line lies: NaN when they are one
/// point.
fn foot(p: Point, a: Point, b: Point) -> f64 {
    let (dx, dy) = (b.x() - a.x(), b.y() - a.y());
    ((p.x() - a.x()) * dx + (p.y() - a.y()) * dy) / (dx * dx + dy * dy)
}

/// The point of the side from `a` to `b` nearest `p`: the foot of `p`, or the nearer end when the foot lies
/// beyond one.
fn on_side(p: Point, a: Point, b: Point) -> Point {
    let t = if p == a {
        0.0
    } else if p == b {
        1.0
    } else {
        foot(p, a, b)
    };
    if t > 0.0 && t < 1.0 {
        // Finite inputs and a fraction between 0 and 1 give a finite point.
        return Point::new(a.x() + t * (b.x() - a.x()), a.y() + t * (b.y() - a.y())).unwrap_or(a);
    }
    if length(p, a) < length(p, b) { a } else { b }
}

/// How far `p` is from the side from `a` to `b`.
fn to_side(p: Point, a: Point, b: Point) -> f64 {
    if a == b {
        return length(p, a);
    }
    let t = foot(p, a, b);
    if t <= 0.0 {
        return length(p, a);
    }
    if t >= 1.0 {
        return length(p, b);
    }
    let (dx, dy) = (b.x() - a.x(), b.y() - a.y());
    let squared = dx * dx + dy * dy;
    (((a.y() - p.y()) * dx - (a.x() - p.x()) * dy) / squared).abs() * squared.sqrt()
}

/// Whether `p` lies in the box around the side from `a` to `b`, its edges included.
fn in_box(a: Point, b: Point, p: Point) -> bool {
    a.x().min(b.x()) <= p.x() && p.x() <= a.x().max(b.x()) && a.y().min(b.y()) <= p.y() && p.y() <= a.y().max(b.y())
}

/// Whether the boxes around the sides from `a` to `b` and from `c` to `d` overlap, edges included.
fn boxes_meet(a: Point, b: Point, c: Point, d: Point) -> bool {
    a.x().min(b.x()).max(c.x().min(d.x())) <= a.x().max(b.x()).min(c.x().max(d.x()))
        && a.y().min(b.y()).max(c.y().min(d.y())) <= a.y().max(b.y()).min(c.y().max(d.y()))
}

/// Which side of the line from `a` to `b` `p` lies on: 1 to the left, -1 to the right, 0 on it.
fn turn(a: Point, b: Point, p: Point) -> i8 {
    let cross = (b.x() - a.x()) * (p.y() - a.y()) - (b.y() - a.y()) * (p.x() - a.x());
    if cross > 0.0 {
        1
    } else if cross < 0.0 {
        -1
    } else {
        0
    }
}

/// Where the sides from `a` to `b` and from `c` to `d` meet, if they do. Sides that cross meet where they
/// cross. Sides that touch meet at the end that touches, exactly, the second's start and end before the
/// first's: an end they share lies on both. Sides along one line that overlap meet at the second's start
/// when both its ends lie on the first, at the first's start when both of the first's ends lie on the
/// second, and otherwise at the end of the second that lies on the first.
fn meeting(a: Point, b: Point, c: Point, d: Point) -> Option<Point> {
    if !boxes_meet(a, b, c, d) {
        return None;
    }
    let (sc, sd) = (turn(a, b, c), turn(a, b, d));
    let (sa, sb) = (turn(c, d, a), turn(c, d, b));
    if (sc != 0 && sc == sd) || (sa != 0 && sa == sb) {
        return None;
    }
    if [sc, sd, sa, sb] == [0; 4] {
        let (c_on, d_on) = (in_box(a, b, c), in_box(a, b, d));
        let (a_on, b_on) = (in_box(c, d, a), in_box(c, d, b));
        return if c_on && d_on {
            Some(c)
        } else if a_on && b_on {
            Some(a)
        } else if c_on && (a_on || b_on) {
            Some(c)
        } else {
            (d_on && (a_on || b_on)).then_some(d)
        };
    }
    if sc == 0 || sd == 0 || sa == 0 || sb == 0 {
        return Some(if sc == 0 {
            c
        } else if sd == 0 {
            d
        } else if sa == 0 {
            a
        } else {
            b
        });
    }
    // A proper crossing: the sides' lines are not parallel, so the fraction is finite.
    let across = (b.x() - a.x()) * (d.y() - c.y()) - (b.y() - a.y()) * (d.x() - c.x());
    let t = ((c.x() - a.x()) * (d.y() - c.y()) - (c.y() - a.y()) * (d.x() - c.x())) / across;
    Point::new(a.x() + t * (b.x() - a.x()), a.y() + t * (b.y() - a.y())).ok()
}

/// How far apart the sides from `a` to `b` and from `c` to `d` are: 0 where their lines cross within both,
/// and otherwise as far as the nearest of their 4 ends is from the other side.
fn apart(a: Point, b: Point, c: Point, d: Point) -> f64 {
    if a == b {
        return to_side(a, c, d);
    }
    if c == d {
        return to_side(d, a, b);
    }
    if boxes_meet(a, b, c, d) {
        let across = (b.x() - a.x()) * (d.y() - c.y()) - (b.y() - a.y()) * (d.x() - c.x());
        if across != 0.0 {
            let r = ((a.y() - c.y()) * (d.x() - c.x()) - (a.x() - c.x()) * (d.y() - c.y())) / across;
            let s = ((a.y() - c.y()) * (b.x() - a.x()) - (a.x() - c.x()) * (b.y() - a.y())) / across;
            if (0.0..=1.0).contains(&r) && (0.0..=1.0).contains(&s) {
                return 0.0;
            }
        }
    }
    to_side(a, c, d).min(to_side(b, c, d)).min(to_side(c, a, b).min(to_side(d, a, b)))
}

/// The points of the segments `(a, b)` and `(c, d)` nearest each other, measured from the first: the first
/// segment's point then the second's, and how far apart the segments are. Where they meet, both points are
/// the meeting point ([`meeting`]). Otherwise one of the pair is an end, and of equally near pairs the
/// first of these is taken: the first segment's points nearest `c` and then `d`, then its own ends `a`
/// and `b`.
pub(crate) fn between_segments((a, b): (Point, Point), (c, d): (Point, Point)) -> (Point, Point, f64) {
    let distance = apart(a, b, c, d);
    if let Some(meet) = meeting(a, b, c, d) {
        return (meet, meet, distance);
    }
    let candidates = [(on_side(c, a, b), c), (on_side(d, a, b), d), (a, on_side(a, c, d)), (b, on_side(b, c, d))];
    let mut best = (a, c, f64::INFINITY);
    for (p, q) in candidates {
        let between = length(p, q);
        if between < best.2 {
            best = (p, q, between);
        }
    }
    (best.0, best.1, distance)
}

/// The point of the polylines `lines` nearest `p`, and how far `p` is from them; `None` when they have no
/// point. A unit of `meter`'s work per side.
pub(crate) fn nearest_to_point(lines: &[&[Point]], p: Point, meter: &mut Meter) -> Result<Option<(Point, f64)>, Exhausted> {
    let mut best: Option<(Point, f64)> = None;
    for &line in lines {
        for (a, b) in sides(line) {
            meter.charge(1)?;
            let distance = to_side(p, a, b);
            if best.is_none_or(|(_, d)| distance < d) {
                best = Some((on_side(p, a, b), distance));
            }
        }
    }
    Ok(best)
}

/// The point of the polylines `lines` nearest the polylines `other`, measured from `other`; `None` when
/// either has no point. A unit of `meter`'s work per pair of sides.
pub(crate) fn nearest_to_lines(lines: &[&[Point]], other: &[&[Point]], meter: &mut Meter) -> Result<Option<Point>, Exhausted> {
    let mut best: Option<(Point, f64)> = None;
    for &theirs in other {
        for &line in lines {
            for their_side in sides(theirs) {
                for side in sides(line) {
                    meter.charge(1)?;
                    let distance = apart(their_side.0, their_side.1, side.0, side.1);
                    if best.is_none_or(|(_, d)| distance < d) {
                        best = Some((between_segments(their_side, side).1, distance));
                    }
                }
            }
        }
    }
    Ok(best.map(|(on, _)| on))
}

/// Which of a polyline and a segment measured against each other is measured from: it decides which of
/// a side's equally near points comes first ([`between_segments`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Look {
    /// From the polyline's sides to the segment.
    FromPolyline,
    /// From the segment to the polyline's sides.
    FromSegment,
}

/// Where along the polyline `points` it comes nearest the segment `cut`, measured as `look` says: the point
/// of it nearest the cut, the first of equally near ones, and then the distance along it to where it first
/// passes that point. 0 for a polyline with no point. 2 units of `meter`'s work per side.
pub(crate) fn along_to_segment(points: &[Point], cut: (Point, Point), look: Look, meter: &mut Meter) -> Result<f64, Exhausted> {
    let mut best: Option<(Point, f64)> = None;
    for side in sides(points) {
        meter.charge(1)?;
        let (on, distance) = match look {
            Look::FromPolyline => {
                let (on, _, distance) = between_segments(side, cut);
                (on, distance)
            }
            Look::FromSegment => {
                let (_, on, distance) = between_segments(cut, side);
                (on, distance)
            }
        };
        if best.is_none_or(|(_, d)| distance < d) {
            best = Some((on, distance));
        }
    }
    match best {
        Some((on, _)) => along_to_point(points, on, meter),
        None => Ok(0.0),
    }
}

/// How far along the polyline `points` it first comes nearest `p`, measured as [`Along`] measures it. A unit
/// of `meter`'s work per side.
///
/// [`Along`]: crate::normalize::along::Along
fn along_to_point(points: &[Point], p: Point, meter: &mut Meter) -> Result<f64, Exhausted> {
    let mut best: Option<(f64, f64)> = None;
    let mut start = 0.0;
    for (a, b) in sides(points) {
        meter.charge(1)?;
        let distance = to_side(p, a, b);
        if best.is_none_or(|(d, _)| distance < d) {
            best = Some((distance, start + a.distance(on_side(p, a, b))));
        }
        start += a.distance(b);
    }
    Ok(best.map_or(0.0, |(_, at)| at))
}

#[cfg(test)]
mod tests {
    use stitchcraft_core::Budget;

    use super::*;

    fn p(x: f64, y: f64) -> Point {
        Point::new(x, y).unwrap()
    }

    #[test]
    fn a_foot_on_a_side_is_exact_where_its_coordinates_allow() {
        // 3/5 of the way up the side: interpolating between its ends would give y = 3.9999999999999996.
        assert_eq!(on_side(p(3.0, 4.0), p(4.0, 1.0), p(4.0, 6.0)), p(4.0, 4.0));
        assert_eq!(to_side(p(3.0, 4.0), p(4.0, 1.0), p(4.0, 6.0)), 1.0);
        // Beyond an end, the nearer end.
        assert_eq!((on_side(p(4.0, 9.0), p(4.0, 1.0), p(4.0, 6.0)), to_side(p(4.0, 9.0), p(4.0, 1.0), p(4.0, 6.0))), (p(4.0, 6.0), 3.0));
        assert_eq!((on_side(p(1.0, 1.0), p(4.0, 1.0), p(4.0, 6.0)), to_side(p(1.0, 1.0), p(4.0, 1.0), p(4.0, 6.0))), (p(4.0, 1.0), 3.0));
        // A side of no length is its point.
        assert_eq!((on_side(p(1.0, 1.0), p(4.0, 5.0), p(4.0, 5.0)), to_side(p(1.0, 1.0), p(4.0, 5.0), p(4.0, 5.0))), (p(4.0, 5.0), 5.0));
        // A foot exactly at an end is that end: interpolating to it would give x = -0.8999999999999999.
        assert_eq!(on_side(p(-0.9, 1.0), p(-3.0, 0.0), p(-0.9, 0.0)), p(-0.9, 0.0));
        // Seen from afar, a side too short for its ends to differ in distance gives its end, as shapely does,
        // whether the foot lies beyond its start or square to it.
        let (a, b) = (p(0.0, 0.0), p(1e-14, 0.0));
        assert_eq!(on_side(p(-1000.0, 0.0), a, b), b);
        assert_eq!(on_side(p(0.0, 1000.0), a, b), b);
    }

    #[test]
    fn touching_and_overlapping_sides_meet_at_an_end() {
        // An end on the other side, and a shared end: that end, exactly.
        assert_eq!(meeting(p(6.0, 6.0), p(4.0, 4.0), p(4.0, 1.0), p(4.0, 6.0)), Some(p(4.0, 4.0)));
        assert_eq!(meeting(p(0.0, 0.0), p(2.0, 1.0), p(2.0, 1.0), p(5.0, 0.0)), Some(p(2.0, 1.0)));
        // Whichever end touches, even where computing the crossing would miss it by a rounding error.
        let (on, off) = (p(-0.234375, -0.84375), p(-3.0, 1.6));
        let (start, end) = (p(-1.125, 0.0), p(1.25, -2.25));
        assert_eq!(meeting(start, end, on, off), Some(on), "the second's start");
        assert_eq!(meeting(start, end, off, on), Some(on), "the second's end");
        assert_eq!(meeting(off, on, start, end), Some(on), "the first's end");
        // Along one line: the second's start when it lies within the first, the first's start when the first
        // lies within the second, and otherwise the second's end that lies on the first.
        let (a, b) = (p(0.0, 0.0), p(4.0, 0.0));
        assert_eq!(meeting(a, b, p(1.0, 0.0), p(3.0, 0.0)), Some(p(1.0, 0.0)));
        assert_eq!(meeting(p(1.0, 0.0), p(3.0, 0.0), a, b), Some(p(1.0, 0.0)));
        assert_eq!(meeting(a, b, p(6.0, 0.0), p(2.0, 0.0)), Some(p(2.0, 0.0)));
        assert_eq!(meeting(a, b, p(2.0, 0.0), p(6.0, 0.0)), Some(p(2.0, 0.0)));
        assert_eq!(meeting(a, b, p(5.0, 0.0), p(6.0, 0.0)), None);
        // Apart, on one side, or with no overlap of their boxes.
        assert_eq!(meeting(a, b, p(1.0, 1.0), p(3.0, 2.0)), None);
        assert_eq!(meeting(a, b, p(5.0, -1.0), p(6.0, 1.0)), None);
        assert_eq!(apart(a, b, p(1.0, 1.0), p(3.0, 2.0)), 1.0);
        assert_eq!(apart(a, b, p(2.0, -1.0), p(2.0, 1.0)), 0.0, "crossing");
        assert_eq!(apart(p(2.0, 3.0), p(2.0, 3.0), a, b), 3.0, "a point and a side");
        assert_eq!(apart(a, b, p(7.0, 4.0), p(7.0, 4.0)), 5.0, "a side and a point");
    }

    #[test]
    fn crossing_segments_meet_where_they_cross() {
        assert_eq!(between_segments((p(0.0, 0.0), p(4.0, 4.0)), (p(0.0, 4.0), p(4.0, 0.0))), (p(2.0, 2.0), p(2.0, 2.0), 0.0));
    }

    #[test]
    fn apart_segments_meet_at_an_end_and_its_nearest_point() {
        // The second segment's start is nearest the first, above its middle.
        assert_eq!(between_segments((p(0.0, 0.0), p(4.0, 0.0)), (p(2.0, 1.0), p(3.0, 5.0))), (p(2.0, 0.0), p(2.0, 1.0), 1.0));
        // The first's end is nearest the second.
        assert_eq!(between_segments((p(0.0, 0.0), p(1.0, 0.0)), (p(3.0, -2.0), p(3.0, 2.0))), (p(1.0, 0.0), p(3.0, 0.0), 2.0));
        // Segments touching at an end are 0 apart there.
        assert_eq!(between_segments((p(0.0, 0.0), p(2.0, 0.0)), (p(2.0, 0.0), p(2.0, 3.0))).2, 0.0);
        // A segment of no length is its point.
        assert_eq!(between_segments((p(1.0, 1.0), p(1.0, 1.0)), (p(0.0, 0.0), p(2.0, 0.0))), (p(1.0, 1.0), p(1.0, 0.0), 1.0));
        // Parallel segments side by side: the first segment's point nearest the second's start.
        assert_eq!(between_segments((p(0.0, 0.0), p(2.0, 0.0)), (p(0.0, 1.0), p(2.0, 1.0))), (p(0.0, 0.0), p(0.0, 1.0), 1.0));
    }

    #[test]
    fn of_equally_near_points_a_segment_takes_its_point_nearest_the_other_s_ends_first() {
        // Side by side over 1 to 2: the first segment's point nearest the second's start, then its own end.
        let (this, other) = ((p(0.0, 0.0), p(2.0, 0.0)), (p(1.0, 1.0), p(3.0, 1.0)));
        assert_eq!(between_segments(this, other), (p(1.0, 0.0), p(1.0, 1.0), 1.0));
        assert_eq!(between_segments(other, this), (p(2.0, 1.0), p(2.0, 0.0), 1.0));
        // Over the same span, the other's start comes first, whichever way the first runs.
        assert_eq!(between_segments((p(2.0, 0.0), p(0.0, 0.0)), (p(0.0, 1.0), p(2.0, 1.0))).0, p(0.0, 0.0));
        assert_eq!(between_segments((p(0.0, 0.0), p(2.0, 0.0)), (p(2.0, 1.0), p(0.0, 1.0))).0, p(2.0, 0.0));
    }

    #[test]
    fn a_polyline_and_a_segment_looked_at_from_either() {
        let meter = &mut Budget::DEFAULT.meter();
        // A side beside the segment over 1 to 2: from the side, the point nearest the segment's start; from
        // the segment, the side's end.
        let (side, cut) = ([p(0.0, 0.0), p(2.0, 0.0)], (p(1.0, 1.0), p(3.0, 1.0)));
        assert_eq!(along_to_segment(&side, cut, Look::FromPolyline, meter).unwrap(), 1.0);
        assert_eq!(along_to_segment(&side, cut, Look::FromSegment, meter).unwrap(), 2.0);
    }

    #[test]
    fn nearest_points_take_the_first_of_equals() {
        let meter = &mut Budget::DEFAULT.meter();
        let rails: [&[Point]; 2] = [&[p(0.0, 0.0), p(10.0, 0.0)], &[p(0.0, 6.0), p(10.0, 6.0)]];
        assert_eq!(nearest_to_point(&rails, p(4.0, 2.0), meter).unwrap(), Some((p(4.0, 0.0), 2.0)));
        assert_eq!(nearest_to_point(&rails, p(4.0, 3.0), meter).unwrap(), Some((p(4.0, 0.0), 3.0)), "as near both: the first");
        assert_eq!(nearest_to_point(&rails, p(5.0, 5.0), meter).unwrap(), Some((p(5.0, 6.0), 1.0)));
        assert_eq!(nearest_to_point(&[&[p(1.0, 1.0)]], p(1.0, 3.0), meter).unwrap(), Some((p(1.0, 1.0), 2.0)), "a point is a polyline");
        assert_eq!(nearest_to_point(&[], p(0.0, 0.0), meter).unwrap(), None);
        // A shape beside the rails: the second rail is nearer.
        let other: [&[Point]; 1] = [&[p(3.0, 9.0), p(6.0, 7.0)]];
        assert_eq!(nearest_to_lines(&rails, &other, meter).unwrap(), Some(p(6.0, 6.0)));
        assert_eq!(nearest_to_lines(&rails, &[], meter).unwrap(), None);
    }

    #[test]
    fn equally_near_lines_are_found_from_the_other_shape_first() {
        let meter = &mut Budget::DEFAULT.meter();
        // The other shape's first line is as near the second rail as its second line is to the first rail:
        // its first line is measured first, so the second rail's point is taken.
        let rails: [&[Point]; 2] = [&[p(0.0, 0.0), p(10.0, 0.0)], &[p(0.0, 6.0), p(10.0, 6.0)]];
        let around: [&[Point]; 2] = [&[p(0.0, 8.0), p(10.0, 8.0)], &[p(0.0, -2.0), p(10.0, -2.0)]];
        assert_eq!(nearest_to_lines(&rails, &around, meter).unwrap(), Some(p(0.0, 6.0)));
        assert_eq!(nearest_to_lines(&rails, &[around[1], around[0]], meter).unwrap(), Some(p(0.0, 0.0)));
        // A line of the other shape is measured against each of these lines in turn, side by side: its
        // second side's point on the first line comes before its first side's on the second.
        let lines: [&[Point]; 2] = [&[p(102.0, 25.0), p(102.0, 30.0)], &[p(49.0, 2.0), p(51.0, 2.0)]];
        let other: [&[Point]; 1] = [&[p(0.0, 0.0), p(100.0, 0.0), p(100.0, 50.0)]];
        assert_eq!(nearest_to_lines(&lines, &other, meter).unwrap(), Some(p(102.0, 25.0)));
        assert_eq!(nearest_to_lines(&[lines[1], lines[0]], &other, meter).unwrap(), Some(p(49.0, 2.0)));
    }

    #[test]
    fn a_polyline_comes_nearest_a_segment_where_it_crosses_it() {
        let meter = &mut Budget::DEFAULT.meter();
        let line = [p(0.0, 3.0), p(4.0, 3.0), p(8.0, 3.0)];
        for look in [Look::FromPolyline, Look::FromSegment] {
            assert_eq!(along_to_segment(&line, (p(5.0, 0.0), p(5.0, 6.0)), look, meter).unwrap(), 5.0);
            assert_eq!(along_to_segment(&line, (p(-2.0, 0.0), p(-2.0, 6.0)), look, meter).unwrap(), 0.0, "before it: its start");
            assert_eq!(along_to_segment(&line, (p(9.0, 0.0), p(9.0, 1.0)), look, meter).unwrap(), 8.0, "past it: its end");
            assert_eq!(along_to_segment(&[], (p(0.0, 0.0), p(1.0, 1.0)), look, meter).unwrap(), 0.0);
        }
    }

    #[test]
    fn measuring_costs_work() {
        let rails: [&[Point]; 1] = [&[p(0.0, 0.0), p(1.0, 0.0), p(2.0, 0.0)]];
        assert!(nearest_to_point(&rails, p(0.0, 1.0), &mut Budget { max_stitches: 1, max_work: 1 }.meter()).is_err());
        assert!(nearest_to_lines(&rails, &rails, &mut Budget { max_stitches: 1, max_work: 3 }.meter()).is_err());
        assert!(
            along_to_segment(rails[0], (p(0.0, 0.0), p(0.0, 1.0)), Look::FromPolyline, &mut Budget { max_stitches: 1, max_work: 1 }.meter()).is_err()
        );
    }

    /// Shapely's answers for random shapes on coarse grids, where ties are common
    /// (`conformance/fixtures/geometry/shapely-nearest.txt`, written by `conformance/oracle/nearest.py`).
    const SHAPELY: &str = include_str!("../../../../conformance/fixtures/geometry/shapely-nearest.txt");

    /// Reads the numbers of a case: a count of polylines, each a count of points and their coordinates.
    struct Numbers<'a>(std::str::SplitWhitespace<'a>);

    impl Numbers<'_> {
        fn number(&mut self) -> f64 {
            self.0.next().unwrap().parse().unwrap()
        }
        fn count(&mut self) -> usize {
            self.0.next().unwrap().parse().unwrap()
        }
        fn point(&mut self) -> Point {
            let x = self.number();
            p(x, self.number())
        }
        fn polyline(&mut self) -> Vec<Point> {
            (0..self.count()).map(|_| self.point()).collect()
        }
        fn polylines(&mut self) -> Vec<Vec<Point>> {
            (0..self.count()).map(|_| self.polyline()).collect()
        }
    }

    fn slices(lines: &[Vec<Point>]) -> Vec<&[Point]> {
        lines.iter().map(Vec::as_slice).collect()
    }

    /// Answers this close are the same point: shapely computes where sides cross in extended precision, so a
    /// crossing can differ from the engine's in its last bit, while equally near points lie a grid step apart.
    const CLOSE: f64 = 1e-9;

    #[test]
    fn equally_near_points_are_taken_as_shapely_takes_them() {
        let meter = &mut Budget::DEFAULT.meter();
        let mut differ = Vec::new();
        for (number, case) in SHAPELY.lines().enumerate().filter(|(_, case)| !case.starts_with('#')) {
            let mut words = case.split_whitespace();
            let kind = words.next().unwrap();
            let mut n = Numbers(words);
            assert!(["pt", "ll", "ap", "as"].contains(&kind), "line {}: unknown kind {kind}", number + 1);
            let same = match kind {
                "pt" => {
                    let (lines, from) = (n.polylines(), n.point());
                    nearest_to_point(&slices(&lines), from, meter).unwrap().unwrap().0.distance(n.point()) < CLOSE
                }
                "ll" => {
                    let (lines, other) = (n.polylines(), n.polylines());
                    nearest_to_lines(&slices(&lines), &slices(&other), meter).unwrap().unwrap().distance(n.point()) < CLOSE
                }
                _ => {
                    let (line, cut) = (n.polyline(), (n.point(), n.point()));
                    let look = if kind == "ap" { Look::FromPolyline } else { Look::FromSegment };
                    (along_to_segment(&line, cut, look, meter).unwrap() - n.number()).abs() < CLOSE
                }
            };
            // Every line of the test runs, a line that differs or not.
            differ.extend((!same).then_some(number + 1));
        }
        assert_eq!(differ, Vec::<usize>::new(), "the fixture's lines whose answer differs");
    }
}
