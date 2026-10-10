//! The plane geometry of rings: areas, bounds, which side of a segment a point lies on, and how two
//! segments meet.
//!
//! Every side of a line is decided exactly ([`stitchcraft_core::exact::side`]), so a point on a segment
//! is on it, and segments that only touch are never taken to cross. Only where two segments cross inside
//! both is a point worked out in floating point. A ring here is a list of points whose last is not its
//! first again unless it says *closed*: a closed ring repeats its first point at its end.

use stitchcraft_core::Point;
use stitchcraft_core::exact::{self, Side};

/// Where a point lies relative to the area a ring bounds.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Location {
    /// Inside, off its boundary.
    Interior,
    /// On its boundary.
    Boundary,
    /// Outside.
    Exterior,
}

/// `ring` with its first point repeated at its end, unless it ends there already.
pub(crate) fn closed(ring: &[Point]) -> Vec<Point> {
    let mut out = ring.to_vec();
    if let (Some(&first), Some(&last)) = (ring.first(), ring.last())
        && first != last
    {
        out.push(first);
    }
    out
}

/// `points` without the points that repeat the one before.
pub(crate) fn without_repeats(points: &[Point]) -> Vec<Point> {
    let mut out: Vec<Point> = Vec::with_capacity(points.len());
    for &p in points {
        if out.last() != Some(&p) {
            out.push(p);
        }
    }
    out
}

/// The signed area of the closed ring `ring`: positive when it turns counter-clockwise in y-up axes
/// (clockwise on screen, where y points down). Every x is taken relative to the first, which keeps the
/// products small for rings far from the origin.
pub(crate) fn signed_area(ring: &[Point]) -> f64 {
    let Some(first) = ring.first() else { return 0.0 };
    let x0 = first.x();
    let sum: f64 = ring.windows(3).map(|w| (w[1].x() - x0) * (w[2].y() - w[0].y())).sum();
    sum / 2.0
}

/// Bounds: the least and greatest x and y.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Envelope {
    pub(crate) min_x: f64,
    pub(crate) min_y: f64,
    pub(crate) max_x: f64,
    pub(crate) max_y: f64,
}

impl Envelope {
    /// The bounds of `points`; of nothing, the bounds of the origin.
    pub(crate) fn of(points: &[Point]) -> Envelope {
        let Some(first) = points.first() else { return Envelope { min_x: 0.0, min_y: 0.0, max_x: 0.0, max_y: 0.0 } };
        let mut env = Envelope { min_x: first.x(), min_y: first.y(), max_x: first.x(), max_y: first.y() };
        for p in points {
            env.min_x = env.min_x.min(p.x());
            env.min_y = env.min_y.min(p.y());
            env.max_x = env.max_x.max(p.x());
            env.max_y = env.max_y.max(p.y());
        }
        env
    }

    /// Whether `other` lies within these bounds, edges included.
    pub(crate) fn covers(&self, other: &Envelope) -> bool {
        other.min_x >= self.min_x && other.max_x <= self.max_x && other.min_y >= self.min_y && other.max_y <= self.max_y
    }

    /// Whether the bounds share a point.
    pub(crate) fn meets(&self, other: &Envelope) -> bool {
        other.min_x <= self.max_x && other.max_x >= self.min_x && other.min_y <= self.max_y && other.max_y >= self.min_y
    }
}

/// Where `p` lies relative to the closed ring `ring`: on it, or inside by the even-odd count of the
/// ring's segments a ray from `p` to the right crosses.
pub(crate) fn locate_in_ring(p: Point, ring: &[Point]) -> Location {
    let mut crossings = 0_u32;
    for pair in ring.windows(2) {
        let (a, b) = (pair[0], pair[1]);
        if on_segment(p, a, b) {
            return Location::Boundary;
        }
        // A segment counts when it spans p's height, its lower end included and its upper end not, and p lies
        // to its left, so that it crosses the ray from p to the right.
        let (low, high) = if a.y() <= b.y() { (a, b) } else { (b, a) };
        if low.y() <= p.y() && p.y() < high.y() && exact::side(low, high, p) == Side::Left {
            crossings += 1;
        }
    }
    if crossings % 2 == 1 { Location::Interior } else { Location::Exterior }
}

/// Whether `p` lies on the segment from `a` to `b`, ends included.
pub(crate) fn on_segment(p: Point, a: Point, b: Point) -> bool {
    exact::side(a, b, p) == Side::On && within(p, a, b)
}

/// Whether `p` lies in the box spanned by `a` and `b`.
fn within(p: Point, a: Point, b: Point) -> bool {
    p.x() >= a.x().min(b.x()) && p.x() <= a.x().max(b.x()) && p.y() >= a.y().min(b.y()) && p.y() <= a.y().max(b.y())
}

/// The quadrant of the direction from `origin` to `p`, numbered counter-clockwise from 0 for x ≥ 0, y ≥ 0:
/// the first step in ordering directions round a point.
fn quadrant(origin: Point, p: Point) -> u8 {
    let (dx, dy) = (p.x() - origin.x(), p.y() - origin.y());
    match (dx >= 0.0, dy >= 0.0) {
        (true, true) => 0,
        (false, true) => 1,
        (false, false) => 2,
        (true, false) => 3,
    }
}

/// How the direction from `origin` to `p` compares with the one to `q`, counter-clockwise from the
/// positive x axis in y-up axes: by quadrant, then by which side of the other each lies, exactly.
pub(crate) fn compare_angle(origin: Point, p: Point, q: Point) -> std::cmp::Ordering {
    use std::cmp::Ordering;
    match quadrant(origin, p).cmp(&quadrant(origin, q)) {
        Ordering::Equal => match exact::side(origin, q, p) {
            Side::Left => Ordering::Greater,
            Side::Right => Ordering::Less,
            Side::On => Ordering::Equal,
        },
        other => other,
    }
}

/// How two segments meet.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Hit {
    /// They do not.
    Apart,
    /// They cross at a point inside both.
    Cross(Point),
    /// They meet at one point, an end of one of them or both.
    Touch(Point),
    /// They lie along one line and share a stretch, between the 2 points.
    Overlap(Point, Point),
}

/// How the segments `a`–`b` and `c`–`d` meet, decided exactly. Only a crossing inside both is worked out
/// in floating point.
pub(crate) fn hit(a: Point, b: Point, c: Point, d: Point) -> Hit {
    let (sc, sd) = (exact::side(a, b, c), exact::side(a, b, d));
    let (sa, sb) = (exact::side(c, d, a), exact::side(c, d, b));
    let apart = |x: Side, y: Side| x != Side::On && x == y;
    if apart(sc, sd) || apart(sa, sb) {
        return Hit::Apart;
    }
    if [sc, sd, sa, sb] == [Side::On; 4] {
        return along(a, b, c, d);
    }
    for (p, (s, t)) in [(c, (a, b)), (d, (a, b)), (a, (c, d)), (b, (c, d))] {
        if within(p, s, t) && exact::side(s, t, p) == Side::On {
            return Hit::Touch(p);
        }
    }
    Hit::Cross(crossing(a, b, c, d))
}

/// Where segments on one line meet: apart, at a shared end, or along their overlap.
fn along(a: Point, b: Point, c: Point, d: Point) -> Hit {
    // Everything is ordered along the line by the coordinate it changes in more.
    let key = |p: Point| if (b.x() - a.x()).abs() >= (b.y() - a.y()).abs() { p.x() } else { p.y() };
    let (lo1, hi1) = if key(a) <= key(b) { (a, b) } else { (b, a) };
    let (lo2, hi2) = if key(c) <= key(d) { (c, d) } else { (d, c) };
    let start = if key(lo1) >= key(lo2) { lo1 } else { lo2 };
    let end = if key(hi1) <= key(hi2) { hi1 } else { hi2 };
    match key(start).partial_cmp(&key(end)) {
        Some(std::cmp::Ordering::Less) => Hit::Overlap(start, end),
        Some(std::cmp::Ordering::Equal) => Hit::Touch(start),
        _ => Hit::Apart,
    }
}

/// Where the lines through 2 segments that cross meet. The coordinates are taken relative to the middle
/// of the box the segments' boxes share, which keeps the products small, and a point that rounding puts
/// outside that box gives way to the end nearest the other segment.
pub(crate) fn crossing(p1: Point, p2: Point, q1: Point, q2: Point) -> Point {
    let (lo_x, hi_x) = (p1.x().min(p2.x()).max(q1.x().min(q2.x())), p1.x().max(p2.x()).min(q1.x().max(q2.x())));
    let (lo_y, hi_y) = (p1.y().min(p2.y()).max(q1.y().min(q2.y())), p1.y().max(p2.y()).min(q1.y().max(q2.y())));
    let (mid_x, mid_y) = ((lo_x + hi_x) / 2.0, (lo_y + hi_y) / 2.0);
    let (p1x, p1y, p2x, p2y) = (p1.x() - mid_x, p1.y() - mid_y, p2.x() - mid_x, p2.y() - mid_y);
    let (q1x, q1y, q2x, q2y) = (q1.x() - mid_x, q1.y() - mid_y, q2.x() - mid_x, q2.y() - mid_y);
    let (px, py, pw) = (p1y - p2y, p2x - p1x, p1x * p2y - p2x * p1y);
    let (qx, qy, qw) = (q1y - q2y, q2x - q1x, q1x * q2y - q2x * q1y);
    let (x, y, w) = (py * qw - qy * pw, qx * pw - px * qw, px * qy - qx * py);
    match Point::new(x / w + mid_x, y / w + mid_y) {
        Ok(p) if p.x() >= lo_x && p.x() <= hi_x && p.y() >= lo_y && p.y() <= hi_y => p,
        _ => nearest_end(p1, p2, q1, q2),
    }
}

/// Of the 4 ends, the one nearest the other segment.
fn nearest_end(p1: Point, p2: Point, q1: Point, q2: Point) -> Point {
    let mut best = (p1, f64::INFINITY);
    for (p, a, b) in [(p1, q1, q2), (p2, q1, q2), (q1, p1, p2), (q2, p1, p2)] {
        let d = crate::normalize::stroke::distance_to_segment(p, a, b);
        if d < best.1 {
            best = (p, d);
        }
    }
    best.0
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(x: f64, y: f64) -> Point {
        Point::new(x, y).unwrap()
    }

    fn square() -> Vec<Point> {
        vec![p(0.0, 0.0), p(4.0, 0.0), p(4.0, 4.0), p(0.0, 4.0), p(0.0, 0.0)]
    }

    #[test]
    fn rings_close_and_drop_repeats() {
        assert_eq!(closed(&[p(0.0, 0.0), p(1.0, 0.0), p(0.0, 1.0)]).len(), 4);
        assert_eq!(closed(&square()), square(), "closed already");
        assert!(closed(&[]).is_empty());
        assert_eq!(without_repeats(&[p(0.0, 0.0), p(0.0, 0.0), p(1.0, 0.0), p(1.0, 0.0)]), [p(0.0, 0.0), p(1.0, 0.0)]);
    }

    #[test]
    fn areas_are_signed_by_the_turn() {
        assert_eq!(signed_area(&square()), 16.0);
        let mut turned = square();
        turned.reverse();
        assert_eq!(signed_area(&turned), -16.0);
        assert_eq!(signed_area(&[p(0.0, 0.0), p(1.0, 1.0)]), 0.0);
        assert_eq!(signed_area(&[]), 0.0);
        let far: Vec<Point> = square().iter().map(|q| p(q.x() + 1e9, q.y())).collect();
        assert_eq!(signed_area(&far), 16.0, "far along x");
    }

    #[test]
    fn envelopes_cover_and_meet() {
        let a = Envelope::of(&square());
        let b = Envelope::of(&[p(1.0, 1.0), p(2.0, 2.0)]);
        assert!(a.covers(&b) && !b.covers(&a));
        assert!(a.meets(&b) && b.meets(&a));
        assert!(!a.meets(&Envelope::of(&[p(5.0, 5.0)])));
        // Bounds reaching out on any one side are not covered; bounds apart on any one side do not meet.
        for (dx, dy) in [(-1.0, 0.0), (1.0, 0.0), (0.0, -1.0), (0.0, 1.0)] {
            let moved = Envelope::of(&[p(1.0 + 2.0 * dx, 1.0 + 2.0 * dy), p(3.0 + 2.0 * dx, 3.0 + 2.0 * dy)]);
            assert!(!a.covers(&moved) && a.meets(&moved), "{moved:?}");
            let apart = Envelope::of(&[p(1.0 + 5.0 * dx, 1.0 + 5.0 * dy), p(3.0 + 5.0 * dx, 3.0 + 5.0 * dy)]);
            assert!(!a.meets(&apart) && !apart.meets(&a), "{apart:?}");
        }
        assert_eq!(Envelope::of(&[]), Envelope { min_x: 0.0, min_y: 0.0, max_x: 0.0, max_y: 0.0 });
    }

    #[test]
    fn points_lie_inside_on_or_outside_rings() {
        let ring = square();
        assert_eq!(locate_in_ring(p(2.0, 2.0), &ring), Location::Interior);
        assert_eq!(locate_in_ring(p(4.0, 2.0), &ring), Location::Boundary);
        assert_eq!(locate_in_ring(p(0.0, 0.0), &ring), Location::Boundary);
        assert_eq!(locate_in_ring(p(5.0, 2.0), &ring), Location::Exterior);
        assert_eq!(locate_in_ring(p(-1.0, 0.0), &ring), Location::Exterior, "level with a horizontal edge");
        assert_eq!(locate_in_ring(p(2.0, 4.0), &ring), Location::Boundary);
        let diamond = [p(2.0, 0.0), p(4.0, 2.0), p(2.0, 4.0), p(0.0, 2.0), p(2.0, 0.0)];
        assert_eq!(locate_in_ring(p(1.0, 2.0), &diamond), Location::Interior, "level with a corner");
        assert_eq!(locate_in_ring(p(-1.0, 2.0), &diamond), Location::Exterior);
    }

    #[test]
    fn directions_are_ordered_round_a_point() {
        use std::cmp::Ordering;
        let o = p(0.0, 0.0);
        assert_eq!([quadrant(o, p(1.0, 0.0)), quadrant(o, p(-1.0, 1.0)), quadrant(o, p(-1.0, -1.0)), quadrant(o, p(1.0, -1.0))], [0, 1, 2, 3]);
        assert_eq!(compare_angle(o, p(1.0, 1.0), p(1.0, 0.0)), Ordering::Greater);
        assert_eq!(compare_angle(o, p(1.0, 0.0), p(1.0, 1.0)), Ordering::Less);
        assert_eq!(compare_angle(o, p(2.0, 2.0), p(1.0, 1.0)), Ordering::Equal);
        assert_eq!(compare_angle(o, p(-1.0, 1.0), p(1.0, 1.0)), Ordering::Greater, "by quadrant");
    }

    #[test]
    fn segments_cross_touch_overlap_or_stay_apart() {
        assert_eq!(hit(p(0.0, 0.0), p(2.0, 2.0), p(0.0, 2.0), p(2.0, 0.0)), Hit::Cross(p(1.0, 1.0)));
        assert_eq!(hit(p(0.0, 0.0), p(2.0, 0.0), p(1.0, 0.0), p(1.0, 1.0)), Hit::Touch(p(1.0, 0.0)));
        assert_eq!(hit(p(1.0, 0.0), p(1.0, 1.0), p(0.0, 0.0), p(2.0, 0.0)), Hit::Touch(p(1.0, 0.0)), "an end of the first");
        assert_eq!(hit(p(0.0, 0.0), p(1.0, 0.0), p(1.0, 0.0), p(1.0, 1.0)), Hit::Touch(p(1.0, 0.0)), "shared end");
        assert_eq!(hit(p(0.0, 0.0), p(3.0, 0.0), p(4.0, 0.0), p(1.0, 0.0)), Hit::Overlap(p(1.0, 0.0), p(3.0, 0.0)));
        assert_eq!(hit(p(0.0, 0.0), p(0.0, 3.0), p(0.0, 3.0), p(0.0, 5.0)), Hit::Touch(p(0.0, 3.0)), "end to end on a line");
        assert_eq!(hit(p(0.0, 0.0), p(1.0, 0.0), p(2.0, 0.0), p(3.0, 0.0)), Hit::Apart, "on a line, apart");
        assert_eq!(hit(p(0.0, 0.0), p(1.0, 0.0), p(0.0, 1.0), p(1.0, 1.0)), Hit::Apart);
        assert_eq!(hit(p(0.0, 0.0), p(1.0, 0.0), p(2.0, -1.0), p(2.0, 1.0)), Hit::Apart, "beyond an end");
        assert_eq!(hit(p(2.0, 0.0), p(2.0, 0.5), p(2.0, 0.25), p(2.0, 1.0)), Hit::Overlap(p(2.0, 0.25), p(2.0, 0.5)), "upright, short");
    }

    #[test]
    fn crossings_are_worked_out_near_the_shared_box() {
        let found = crossing(p(1e6, 1e6), p(1e6 + 2.0, 1e6 + 2.0), p(1e6, 1e6 + 2.0), p(1e6 + 2.0, 1e6));
        assert_eq!(found, p(1e6 + 1.0, 1e6 + 1.0));
        // Segments all but parallel, 9 m from the origin: the crossing is the point nearest where the lines
        // meet, worked out in exact fractions as -7,469.77207422360588… and 5,413.19349334059750…. Taken
        // from anywhere but the middle of the shared box, it is off by more than 2e-11 mm.
        let found = crossing(
            p(-7_470.258_083_194_155, 5_413.269_311_813_379),
            p(-7_467.865_355_854_407, 5_412.896_041_063_916),
            p(-7_470.305_764_348_507, 5_413.273_029_487_017),
            p(-7_468.036_033_787_772, 5_412.934_770_235_462),
        );
        assert_eq!(found, p(-7_469.772_074_223_606, 5_413.193_493_340_597));
        // Lines all but parallel still meet where they cross.
        assert_eq!(crossing(p(0.0, 0.0), p(1.0, 0.0), p(0.0, 1e-300), p(1.0, -1e-300)), p(0.5, 0.0));
        // A point outside the box both segments share, or none, gives way to the end nearest the other
        // segment: the first of equals.
        assert_eq!(crossing(p(0.0, 0.0), p(2.0, 0.0), p(0.0, 1.0), p(2.0, 0.5)), p(2.0, 0.0), "lines meeting beyond the box");
        assert_eq!(crossing(p(0.0, 0.0), p(1.0, 0.0), p(0.0, 1.0), p(1.0, 1.0)), p(0.0, 0.0), "parallel");
        assert_eq!(crossing(p(0.0, 0.0), p(0.0, 1.0), p(-1.0, 2.0), p(1.0, 3.0)), p(0.0, 1.0), "level with the box, above it");
        assert_eq!(nearest_end(p(0.0, 0.0), p(1.0, 0.0), p(0.5, 0.1), p(0.5, 2.0)), p(0.5, 0.1));
    }
}
