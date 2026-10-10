//! The raw curves an offset is cut from: each side of a polyline moved out by a distance, its corners
//! joined, before anything decides which parts of it lie on the edge of the stroke.
//!
//! The rules here are those GEOS (the geometry library under shapely, and so under Ink/Stitch) follows,
//! learned from its published design and checked against its answers (`conformance/oracle/offset.py`):
//!
//! - **Simplifying.** Before offsetting a side, a corner where the line turns towards that side is
//!   dropped when it lies nearer than a hundredth of the distance to the corner kept before it: there the
//!   moved segments would cross and leave a small loop. GEOS also measures the corner against the line
//!   past it and the lines to the points between, which a corner that near the one before always passes.
//!   The first and last segment always stay.
//! - **Each segment** is moved sideways by the distance.
//! - **An outside corner** (turning away from the side) gets the join: a miter point where the moved
//!   segments' lines meet, unless it lies farther from the corner than the limit times the distance;
//!   then a bevel across at that distance (a limited miter), or a plain bevel when even that is too far.
//!   A round join is an arc of segments, each at most a 16th of a right angle, all of one length; a
//!   bevel joins the moved ends. Ends closer than a thousandth of the distance are one point, the first.
//! - **An inside corner** gets the point where the moved segments cross. When they do not cross, the
//!   curve goes from the first moved end towards the corner and back to the next moved start, along short
//!   closing segments that the stroke covers (1/81 of the way to the corner for round joins, half way
//!   for the others).
//! - **A turn back** along the same line is a half circle for round joins, and a step across for the
//!   others.
//! - Points closer than a ten-thousandth of the distance to the last one are left out.
//!
//! The ring around a whole polyline is the left side forward, a round cap around the end, the right side
//! backward and a round cap around the start, each side simplified on its own.

use stitchcraft_core::exact::{self, Side};
use stitchcraft_core::{Exhausted, Meter, Point, math};

use crate::design::Join;
use crate::normalize::near::{length, to_side};

/// How many segments approximate a quarter circle of a round join or cap: shapely's default for
/// `offset_curve`, which Ink/Stitch uses.
pub(super) const QUADRANT_SEGMENTS: u32 = 16;

/// The simplifying tolerance, as a fraction of the distance.
const SIMPLIFY: f64 = 0.01;
/// Moved ends of an outside corner closer than this fraction of the distance are one point.
const SEPARATION: f64 = 1e-3;
/// Moved ends of an inside corner closer than this fraction of the distance are one point.
const INSIDE_SNAP: f64 = 1e-3;
/// Curve points closer than this fraction of the distance to the last point are left out.
const VERTEX_SNAP: f64 = 1e-4;
/// How many times shorter than the way to the corner a closing segment is, for round joins.
const ROUND_CLOSING: f64 = 80.0;
/// A sine or cosine nearer 0 than this is 0, as GEOS takes it: a point a quarter turn round a centre then
/// lies exactly on its axis.
const SNAP: f64 = 5e-16;

/// The point (`x`, `y`), which the arithmetic of finite inputs keeps finite; `fallback` if it ever were not.
fn at(x: f64, y: f64, fallback: Point) -> Point {
    Point::new(x, y).unwrap_or(fallback)
}

/// The segment from `p` to `q` moved `distance` to its left (y-up axes), as GEOS moves it.
fn moved(p: Point, q: Point, distance: f64) -> (Point, Point) {
    let (dx, dy) = (q.x() - p.x(), q.y() - p.y());
    let len = (dx * dx + dy * dy).sqrt();
    let (ux, uy) = (distance * dx / len, distance * dy / len);
    (at(p.x() - uy, p.y() + ux, p), at(q.x() - uy, q.y() + ux, q))
}

/// Where the lines through `a`, `b` and through `c`, `d` meet; `None` when they are parallel.
pub(super) fn lines_meet(a: Point, b: Point, c: Point, d: Point) -> Option<Point> {
    let (rx, ry) = (b.x() - a.x(), b.y() - a.y());
    let (sx, sy) = (d.x() - c.x(), d.y() - c.y());
    let den = rx * sy - ry * sx;
    if den == 0.0 {
        return None;
    }
    let t = ((c.x() - a.x()) * sy - (c.y() - a.y()) * sx) / den;
    Point::new(a.x() + t * rx, a.y() + t * ry).ok()
}

/// How segments meet: not at all, at one point, or along a stretch (its first point).
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Meet {
    /// They do not meet.
    Apart,
    /// They meet at one point.
    At(Point),
    /// They lie along one line and share more than a point; the first shared point.
    Along(Point),
}

/// Where the segments `a`–`b` and `c`–`d` meet, decided with exact sides: a shared end first, then an end
/// that lies on the other segment (the second's, then the first's), then a crossing.
pub(crate) fn segments_meet(a: Point, b: Point, c: Point, d: Point) -> Meet {
    let (sc, sd) = (exact::side(a, b, c), exact::side(a, b, d));
    let (sa, sb) = (exact::side(c, d, a), exact::side(c, d, b));
    let apart = |x: Side, y: Side| x != Side::On && x == y;
    if apart(sc, sd) || apart(sa, sb) {
        return Meet::Apart;
    }
    if [sc, sd, sa, sb] == [Side::On; 4] {
        return along(a, b, c, d);
    }
    if let Some(&(shared, _)) = [(a, c), (a, d), (b, c), (b, d)].iter().find(|(p, q)| p == q) {
        return Meet::At(shared);
    }
    match (sc, sd, sa) {
        (Side::On, _, _) => Meet::At(c),
        (_, Side::On, _) => Meet::At(d),
        (_, _, Side::On) => Meet::At(a),
        _ if sb == Side::On => Meet::At(b),
        _ => lines_meet(a, b, c, d).map_or(Meet::Apart, Meet::At),
    }
}

/// Where the segments `a`–`b` and `c`–`d`, on one line, meet: GEOS's order of cases, which says which
/// shared point comes first, and whether the overlap is more than a point.
fn along(a: Point, b: Point, c: Point, d: Point) -> Meet {
    let inside = |p: Point, q: Point, r: Point| {
        p.x() >= q.x().min(r.x()) && p.x() <= q.x().max(r.x()) && p.y() >= q.y().min(r.y()) && p.y() <= q.y().max(r.y())
    };
    let (c_in, d_in, a_in, b_in) = (inside(c, a, b), inside(d, a, b), inside(a, c, d), inside(b, c, d));
    let (first, one_point) = if c_in && d_in {
        (c, false)
    } else if a_in && b_in {
        (a, false)
    } else if c_in && a_in {
        (c, c == a && !d_in && !b_in)
    } else if c_in && b_in {
        (c, c == b && !d_in && !a_in)
    } else if d_in && a_in {
        (d, d == a && !c_in && !b_in)
    } else if d_in && b_in {
        (d, d == b && !c_in && !a_in)
    } else {
        return Meet::Apart;
    };
    if one_point { Meet::At(first) } else { Meet::Along(first) }
}

/// A curve's points, in order, without points that nearly repeat the last.
pub(super) struct Curve {
    points: Vec<Point>,
    gap: f64,
}

impl Curve {
    fn new(distance: f64) -> Self {
        Curve { points: Vec::new(), gap: distance * VERTEX_SNAP }
    }

    fn add(&mut self, p: Point) {
        if self.points.last().is_some_and(|&last| length(p, last) < self.gap) {
            return;
        }
        self.points.push(p);
    }

    fn close(&mut self) {
        if let (Some(&first), Some(&last)) = (self.points.first(), self.points.last())
            && first != last
        {
            self.points.push(first);
        }
    }
}

/// Builds one curve from the segments of a polyline, one corner at a time, always offsetting to the left
/// of the way it is walked.
struct Generator {
    distance: f64,
    join: Join,
    quantum: f64,
    closing: f64,
    curve: Curve,
    /// The last three points walked, and the last two segments moved.
    s: [Point; 3],
    moved: [(Point, Point); 2],
}

impl Generator {
    fn new(distance: f64, join: Join, start: Point, next: Point) -> Self {
        let quantum = std::f64::consts::FRAC_PI_2 / f64::from(QUADRANT_SEGMENTS);
        let closing = if join == Join::Round { ROUND_CLOSING } else { 1.0 };
        let first = moved(start, next, distance);
        Generator { distance, join, quantum, closing, curve: Curve::new(distance), s: [start, start, next], moved: [first, first] }
    }

    /// Starts a new side at the segment from `start` to `next`, keeping the curve built so far.
    fn restart(&mut self, start: Point, next: Point) {
        self.s = [start, start, next];
        let first = moved(start, next, self.distance);
        self.moved = [first, first];
    }

    fn first(&mut self) {
        self.curve.add(self.moved[1].0);
    }

    fn last(&mut self) {
        self.curve.add(self.moved[1].1);
    }

    /// Walks on to `p`, adding the corner at the last point.
    fn next(&mut self, p: Point) {
        let [_, s1, s2] = self.s;
        if s2 == p {
            return;
        }
        self.s = [s1, s2, p];
        self.moved = [moved(s1, s2, self.distance), moved(s2, p, self.distance)];
        match exact::side(s1, s2, p) {
            Side::On => self.straight(),
            Side::Right => self.outside(),
            Side::Left => self.inside(),
        }
    }

    /// A corner of no turn, or a turn straight back.
    fn straight(&mut self) {
        let [s0, s1, s2] = self.s;
        if let Meet::Along(_) = segments_meet(s0, s1, s1, s2) {
            let (end, start) = (self.moved[0].1, self.moved[1].0);
            if self.join == Join::Round {
                self.arc_between(s1, end, start);
            } else {
                self.curve.add(end);
                self.curve.add(start);
            }
        }
    }

    fn outside(&mut self) {
        let (end, start) = (self.moved[0].1, self.moved[1].0);
        if length(end, start) < self.distance * SEPARATION {
            self.curve.add(end);
            return;
        }
        match self.join {
            Join::Miter { limit } => self.miter(limit),
            Join::Bevel => {
                self.curve.add(end);
                self.curve.add(start);
            }
            Join::Round => {
                self.curve.add(end);
                self.arc_between(self.s[1], end, start);
                self.curve.add(start);
            }
        }
    }

    fn inside(&mut self) {
        let [(a, b), (c, d)] = self.moved;
        match segments_meet(a, b, c, d) {
            Meet::At(p) | Meet::Along(p) => self.curve.add(p),
            Meet::Apart if length(b, c) < self.distance * INSIDE_SNAP => self.curve.add(b),
            Meet::Apart => {
                let (corner, f) = (self.s[1], self.closing);
                let toward = |p: Point| at((f * p.x() + corner.x()) / (f + 1.0), (f * p.y() + corner.y()) / (f + 1.0), p);
                for p in [b, toward(b), toward(c), c] {
                    self.curve.add(p);
                }
            }
        }
    }

    fn miter(&mut self, limit: f64) {
        let [(a, b), (c, d)] = self.moved;
        let corner = self.s[1];
        let reach = limit * self.distance;
        if let Some(p) = lines_meet(a, b, c, d).filter(|&p| length(p, corner) <= reach) {
            self.curve.add(p);
        } else if to_side(corner, b, c) >= reach {
            self.curve.add(b);
            self.curve.add(c);
        } else {
            self.limited_miter(reach);
        }
    }

    /// A bevel across the corner's outside at `reach` from it, perpendicular to the corner's bisector.
    fn limited_miter(&mut self, reach: f64) {
        let [(a, b), (c, d)] = self.moved;
        let [s0, corner, s2] = self.s;
        let unit = |(x, y): (f64, f64)| {
            let len = (x * x + y * y).sqrt();
            (x / len, y / len)
        };
        let ((x0, y0), (x2, y2)) = (unit((s0.x() - corner.x(), s0.y() - corner.y())), unit((s2.x() - corner.x(), s2.y() - corner.y())));
        // Away from the corner's inside, along the bisector of its 2 segments; the corner turns, so they
        // are not in line and their directions do not cancel.
        let (ox, oy) = unit((-(x0 + x2), -(y0 + y2)));
        let middle = at(corner.x() + reach * ox, corner.y() + reach * oy, corner);
        // The bevel runs square to the bisector, the offset's distance to each side of its middle.
        let (ax, ay) = (-oy * self.distance, ox * self.distance);
        let (e0, e1) = (at(middle.x() + ax, middle.y() + ay, middle), at(middle.x() - ax, middle.y() - ay, middle));
        // Where the bevel misses an offset line, which the limits above leave to rounding, a plain bevel.
        let (p, q) = line_meets_segment(a, b, e0, e1).zip(line_meets_segment(c, d, e0, e1)).unwrap_or((b, c));
        self.curve.add(p);
        self.curve.add(q);
    }

    /// The arc around `centre` from `from` clockwise to `to`, both added. Every arc of an offset to the left
    /// turns clockwise, round the outside of a right turn.
    fn arc_between(&mut self, centre: Point, from: Point, to: Point) {
        let angle = |p: Point| math::atan2(p.y() - centre.y(), p.x() - centre.x());
        let (mut start, end) = (angle(from), angle(to));
        if start <= end {
            start += std::f64::consts::TAU;
        }
        self.curve.add(from);
        self.arc(centre, start, end);
        self.curve.add(to);
    }

    /// The clockwise arc's points from the angle `start` down to, not including, `end`, in steps of one
    /// length; none when it is under half a step.
    fn arc(&mut self, centre: Point, start: f64, end: f64) {
        let total = (start - end).abs();
        // Rounded to the nearest whole number of steps; never more than a few hundred.
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let steps = (total / self.quantum + 0.5) as u32;
        let step = total / f64::from(steps);
        for i in 0..steps {
            let (sin, cos) = snapped_sin_cos(start - f64::from(i) * step);
            self.curve.add(at(centre.x() + self.distance * cos, centre.y() + self.distance * sin, centre));
        }
    }

    /// The round cap around `end`, the segment from `before` to it ending there.
    fn cap(&mut self, before: Point, end: Point) {
        let (left, right) = (moved(before, end, self.distance), moved(before, end, -self.distance));
        let angle = math::atan2(end.y() - before.y(), end.x() - before.x());
        self.curve.add(left.1);
        self.arc(end, angle + std::f64::consts::FRAC_PI_2, angle - std::f64::consts::FRAC_PI_2);
        self.curve.add(right.1);
    }
}

/// The sine and cosine of `angle`, with values nearer 0 than [`SNAP`] taken as 0.
fn snapped_sin_cos(angle: f64) -> (f64, f64) {
    let snap = |v: f64| if v.abs() < SNAP { 0.0 } else { v };
    let (sin, cos) = math::sin_cos(angle);
    (snap(sin), snap(cos))
}

/// Where the line through `a` and `b` meets the segment from `c` to `d`: an end of the segment on the
/// line, the crossing, or `None` when the segment lies to one side.
fn line_meets_segment(a: Point, b: Point, c: Point, d: Point) -> Option<Point> {
    let side_c = exact::side(a, b, c);
    if side_c == Side::On {
        return Some(c);
    }
    let side_d = exact::side(a, b, d);
    if side_d == Side::On {
        return Some(d);
    }
    if side_c == side_d {
        return None;
    }
    lines_meet(a, b, c, d)
}

/// `points` without the corners that bend towards the offset side, the left for `Side::Left` and the right
/// for `Side::Right`, nearer than `tolerance` to the corner kept before them (see the module docs).
pub(super) fn simplified(points: &[Point], tolerance: f64, toward: Side, meter: &mut Meter) -> Result<Vec<Point>, Exhausted> {
    let mut kept = vec![true; points.len()];
    let next = |kept: &[bool], i: usize| (i + 1..points.len()).find(|&j| kept.get(j).copied().unwrap_or(false)).unwrap_or(points.len());
    // The loop keeps `start` < `middle` < `end` < the number of points.
    let point = |i: usize| points.get(i).copied().unwrap_or(Point::ORIGIN);
    loop {
        let mut changed = false;
        let mut start = 1;
        let mut middle = next(&kept, start);
        let mut end = next(&kept, middle);
        while end < points.len() {
            meter.charge(1)?;
            let (p0, p1, p2) = (point(start), point(middle), point(end));
            if exact::side(p0, p1, p2) == toward && length(p0, p1) < tolerance {
                if let Some(k) = kept.get_mut(middle) {
                    *k = false;
                }
                changed = true;
                start = end;
            } else {
                start = middle;
            }
            middle = next(&kept, start);
            end = next(&kept, middle);
        }
        if !changed {
            return Ok(points.iter().zip(&kept).filter(|(_, k)| **k).map(|(p, _)| *p).collect());
        }
    }
}

/// The raw offset curve of `points` (at least 2 different points, no point repeated in a row) at
/// `distance`, to the left for a positive distance and to the right for a negative one (y-up axes),
/// running the same way as `points`.
pub(super) fn raw(points: &[Point], distance: f64, join: Join, meter: &mut Meter) -> Result<Vec<Point>, Exhausted> {
    let d = distance.abs();
    let tolerance = d * SIMPLIFY;
    let reversed = distance < 0.0;
    let mut walk = simplified(points, tolerance, if reversed { Side::Right } else { Side::Left }, meter)?;
    if reversed {
        walk.reverse();
    }
    let curve = side_curve(&walk, d, join, true, meter)?;
    let mut out = curve.points;
    if reversed {
        out.reverse();
    }
    Ok(out)
}

/// One side's curve, walking `walk` and offsetting to its left: its first moved point when `with_first`,
/// then a corner for each inner point, then its last moved point.
fn side_curve(walk: &[Point], d: f64, join: Join, with_first: bool, meter: &mut Meter) -> Result<Curve, Exhausted> {
    let mut walker = match walk {
        [a, b, ..] => Generator::new(d, join, *a, *b),
        _ => return Ok(Curve::new(d)),
    };
    if with_first {
        walker.first();
    }
    for &p in walk.iter().skip(2) {
        meter.charge(1)?;
        walker.next(p);
    }
    walker.last();
    Ok(walker.curve)
}

/// The closed ring around `points` (at least 2 different points, none repeated in a row) at `distance`:
/// the left side forward, a cap, the right side backward and a cap, each side simplified on its own.
pub(super) fn ring(points: &[Point], distance: f64, join: Join, meter: &mut Meter) -> Result<Vec<Point>, Exhausted> {
    let tolerance = distance * SIMPLIFY;
    let left = simplified(points, tolerance, Side::Left, meter)?;
    let mut right = simplified(points, tolerance, Side::Right, meter)?;
    right.reverse();
    let (Some([a, b]), Some([l1, l2]), Some([ra, rb]), Some([r1, r2])) =
        (left.first_chunk::<2>(), left.last_chunk::<2>(), right.first_chunk::<2>(), right.last_chunk::<2>())
    else {
        return Ok(Vec::new());
    };
    let (l1, l2, ra, rb, r1, r2) = (*l1, *l2, *ra, *rb, *r1, *r2);
    let mut walker = Generator::new(distance, join, *a, *b);
    for &p in left.iter().skip(2) {
        meter.charge(1)?;
        walker.next(p);
    }
    walker.last();
    walker.cap(l1, l2);
    walker.restart(ra, rb);
    for &p in right.iter().skip(2) {
        meter.charge(1)?;
        walker.next(p);
    }
    walker.last();
    walker.cap(r1, r2);
    walker.curve.close();
    Ok(walker.curve.points)
}

#[cfg(test)]
mod tests {
    use stitchcraft_core::Budget;

    use super::*;

    fn p(x: f64, y: f64) -> Point {
        Point::new(x, y).unwrap()
    }

    fn points(coordinates: &[(f64, f64)]) -> Vec<Point> {
        coordinates.iter().map(|&(x, y)| p(x, y)).collect()
    }

    /// The raw curve of `line` offset by `distance` to its left.
    fn raw_of(line: &[(f64, f64)], distance: f64, join: Join) -> Vec<Point> {
        raw(&points(line), distance, join, &mut Budget::DEFAULT.meter()).unwrap()
    }

    /// Whether `found` are the points `expected`, each within a billionth.
    fn near(found: &[Point], expected: &[(f64, f64)]) -> bool {
        found.len() == expected.len() && found.iter().zip(expected).all(|(q, &(x, y))| (q.x() - x).abs() < 1e-9 && (q.y() - y).abs() < 1e-9)
    }

    #[test]
    fn lines_meet_unless_parallel() {
        assert_eq!(lines_meet(p(0.0, 0.0), p(1.0, 0.0), p(2.0, -1.0), p(2.0, 1.0)), Some(p(2.0, 0.0)));
        assert_eq!(lines_meet(p(0.0, 0.0), p(1.0, 0.0), p(0.0, 1.0), p(1.0, 1.0)), None);
    }

    #[test]
    fn segments_meet_at_a_shared_end_then_at_an_end_on_the_other_then_where_they_cross() {
        let at = |x: f64, y: f64| Meet::At(p(x, y));
        assert_eq!(segments_meet(p(0.0, 0.0), p(1.0, 0.0), p(1.0, 0.0), p(1.0, 1.0)), at(1.0, 0.0), "a shared end");
        assert_eq!(segments_meet(p(0.0, 0.0), p(2.0, 0.0), p(1.0, 0.0), p(1.0, 1.0)), at(1.0, 0.0), "c on the first");
        assert_eq!(segments_meet(p(0.0, 0.0), p(2.0, 0.0), p(1.0, 1.0), p(1.0, 0.0)), at(1.0, 0.0), "d on the first");
        assert_eq!(segments_meet(p(1.0, 0.0), p(1.0, 1.0), p(0.0, 0.0), p(2.0, 0.0)), at(1.0, 0.0), "a on the second");
        assert_eq!(segments_meet(p(1.0, 1.0), p(1.0, 0.0), p(0.0, 0.0), p(2.0, 0.0)), at(1.0, 0.0), "b on the second");
        assert_eq!(segments_meet(p(0.0, 0.0), p(2.0, 2.0), p(0.0, 2.0), p(2.0, 0.0)), at(1.0, 1.0), "a crossing");
        // An end on the other segment is taken as it is: where the lines cross, worked out, rounds.
        let (a, b, on, off) = (p(0.5, 0.25), p(3.5, 6.25), p(1.5, 2.25), p(0.3, -4.4));
        assert_eq!(segments_meet(a, b, on, off), Meet::At(on), "c on the first, exactly");
        assert_eq!(segments_meet(a, b, off, on), Meet::At(on), "d on the first, exactly");
        let (start, end, on) = (p(-2.09, 1.74), p(-7.7, 4.32), p(-3.96, 2.6));
        assert_eq!(segments_meet(on, p(0.75, 1.56), start, end), Meet::At(on), "a on the second, exactly");
        assert_eq!(segments_meet(p(0.0, 0.0), p(1.0, 0.0), p(0.0, 1.0), p(1.0, 2.0)), Meet::Apart, "to one side");
        assert_eq!(segments_meet(p(0.0, 0.0), p(1.0, 0.0), p(2.0, -1.0), p(2.0, 1.0)), Meet::Apart, "beyond an end");
    }

    #[test]
    fn segments_on_one_line_meet_at_the_first_shared_point_in_geos_s_order() {
        let meet = |a: f64, b: f64, c: f64, d: f64| segments_meet(p(a, 0.0), p(b, 0.0), p(c, 0.0), p(d, 0.0));
        let (at, along) = (|x: f64| Meet::At(p(x, 0.0)), |x: f64| Meet::Along(p(x, 0.0)));
        assert_eq!(meet(0.0, 4.0, 1.0, 2.0), along(1.0), "the second within the first");
        assert_eq!(meet(1.0, 2.0, 0.0, 4.0), along(1.0), "the first within the second");
        assert_eq!(meet(1.0, 3.0, 2.0, 0.0), along(2.0), "c and a inside");
        assert_eq!(meet(1.0, 2.0, 1.0, 0.0), at(1.0), "c and a, one point");
        assert_eq!(meet(0.0, 2.0, 1.0, 3.0), along(1.0), "c and b inside");
        assert_eq!(meet(0.0, 1.0, 1.0, 2.0), at(1.0), "c and b, one point");
        assert_eq!(meet(1.0, 3.0, 0.0, 2.0), along(2.0), "d and a inside");
        assert_eq!(meet(1.0, 2.0, 0.0, 1.0), at(1.0), "d and a, one point");
        assert_eq!(meet(0.0, 2.0, 3.0, 1.0), along(1.0), "d and b inside");
        assert_eq!(meet(0.0, 1.0, 2.0, 1.0), at(1.0), "d and b, one point");
        assert_eq!(meet(0.0, 1.0, 2.0, 3.0), Meet::Apart, "apart");
    }

    #[test]
    fn a_line_meets_a_segment_at_an_end_on_it_or_where_it_crosses() {
        let (a, b) = (p(0.0, 0.0), p(1.0, 0.0));
        assert_eq!(line_meets_segment(a, b, p(5.0, 0.0), p(5.0, 1.0)), Some(p(5.0, 0.0)), "its start on the line");
        assert_eq!(line_meets_segment(a, b, p(5.0, 1.0), p(5.0, 0.0)), Some(p(5.0, 0.0)), "its end on the line");
        assert_eq!(line_meets_segment(a, b, p(5.0, 1.0), p(6.0, 2.0)), None, "to one side");
        assert_eq!(line_meets_segment(a, b, p(5.0, -1.0), p(5.0, 1.0)), Some(p(5.0, 0.0)), "across");
    }

    #[test]
    fn corners_turn_with_the_join() {
        // Shapely's offset curves of these lines, which nothing else covers.
        let corner = [(0.0, 0.0), (10.0, 0.0), (10.0, -10.0)];
        assert!(near(&raw_of(&corner, 1.0, Join::Bevel), &[(0.0, 1.0), (10.0, 1.0), (11.0, 0.0), (11.0, -10.0)]));
        assert!(near(&raw_of(&corner, 1.0, Join::Miter { limit: 5.0 }), &[(0.0, 1.0), (11.0, 1.0), (11.0, -10.0)]));
        let limited = [(0.0, 1.0), (10.414_213_562_373_096, 1.0), (11.0, 0.414_213_562_373_095), (11.0, -10.0)];
        assert!(near(&raw_of(&corner, 1.0, Join::Miter { limit: 1.0 }), &limited));
        // A limit so small that even the bevel lies beyond it.
        assert!(near(&raw_of(&corner, 1.0, Join::Miter { limit: 0.5 }), &[(0.0, 1.0), (10.0, 1.0), (11.0, 0.0), (11.0, -10.0)]));
        // A leg too short for the limited mitre: its offset line still meets the bevel.
        let short = [(0.0, 1.0), (0.424_213_562_373_095, 1.0), (1.01, 0.414_213_562_373_095), (1.01, -10.0)];
        assert!(near(&raw_of(&[(0.0, 0.0), (0.01, 0.0), (0.01, -10.0)], 1.0, Join::Miter { limit: 1.0 }), &short));
        // A corner whose legs point either side of the negative x axis.
        let across =
            [(-0.832_404_506, 1.155_855_847), (0.575_950_356, 0.916_435_52), (1.055_276_173, 0.240_759_368), (0.815_855_847, -1.167_595_494)];
        let found = raw_of(&[(-1.0, 0.17), (0.0, 0.0), (-0.17, -1.0)], 1.0, Join::Miter { limit: 1.0 });
        assert!(found.len() == 4 && found.iter().zip(across).all(|(q, (x, y))| (q.x() - x).abs() < 1e-8 && (q.y() - y).abs() < 1e-8), "{found:?}");
        // A round join less than half a step round has no points between the moved ends.
        let round = [(0.0, 1.0), (10.0, 1.0), (10.029_986_509, 0.999_550_304), (20.029_986_509, 0.699_550_304)];
        let found = raw_of(&[(0.0, 0.0), (10.0, 0.0), (20.0, -0.3)], 1.0, Join::Round);
        assert!(found.len() == 4 && found.iter().zip(round).all(|(q, (x, y))| (q.x() - x).abs() < 1e-8 && (q.y() - y).abs() < 1e-8), "{found:?}");
        // Moved ends nearer than a thousandth of the distance are one point, the first.
        let found = raw_of(&[(0.0, 0.0), (10.0, 0.0), (20.0, -0.001)], 1.0, Join::Miter { limit: 5.0 });
        assert!(found.len() == 3 && found.get(1) == Some(&p(10.0, 1.0)), "{found:?}");
    }

    #[test]
    fn inside_corners_meet_or_close() {
        // Moved segments that miss each other by less than a thousandth of the distance: the first's end.
        let snapped = [(0.0, 1.0), (2e-4, 1.0), (-9.999_993_750_001_163e-5, 0.999_999_975_000_023_4)];
        assert!(near(&raw_of(&[(0.0, 0.0), (2e-4, 0.0), (4e-4, 1e-7)], 1.0, Join::Bevel), &snapped));
        // By more: in to the corner and back out, halfway for a bevel, and 1/81 of the way for a round join.
        let bevel = [(0.0, 1.0), (0.1, 1.0), (0.1, 0.5), (-0.4, 0.0), (-0.9, 0.0), (-0.9, 0.1)];
        assert!(near(&raw_of(&[(0.0, 0.0), (0.1, 0.0), (0.1, 0.1)], 1.0, Join::Bevel), &bevel));
        let round = [(0.0, 1.0), (0.1, 1.0), (0.1, 80.0 / 81.0), (-71.9 / 81.0, 0.0), (-0.9, 0.0), (-0.9, 0.1)];
        assert!(near(&raw_of(&[(0.0, 0.0), (0.1, 0.0), (0.1, 0.1)], 1.0, Join::Round), &round));
    }

    #[test]
    fn a_corner_turning_to_the_side_near_the_one_kept_before_is_dropped() {
        let meter = &mut Budget::DEFAULT.meter();
        let line = |x: f64| points(&[(0.0, 0.0), (1.0, 0.0), (x, 0.0), (2.0, 0.5), (3.0, 0.5)]);
        // 0.2 from the corner before and turning left: dropped for the left side, kept for the right.
        let dropped = points(&[(0.0, 0.0), (1.0, 0.0), (2.0, 0.5), (3.0, 0.5)]);
        assert_eq!(simplified(&line(1.2), 0.25, Side::Left, meter).unwrap(), dropped);
        assert_eq!(simplified(&line(1.2), 0.25, Side::Right, meter).unwrap(), line(1.2));
        // Exactly the tolerance away: kept, as GEOS keeps it.
        assert_eq!(simplified(&line(1.25), 0.25, Side::Left, meter).unwrap(), line(1.25));
    }

    #[test]
    fn sines_and_cosines_nearer_0_than_5e_16_are_0() {
        // As GEOS snaps them: below 5e-16, and not at it.
        assert_eq!(snapped_sin_cos(4e-16), (0.0, 1.0));
        assert_eq!(snapped_sin_cos(5e-16), (5e-16, 1.0));
    }

    #[test]
    fn moved_ends_exactly_the_snap_apart_are_2_points() {
        // As GEOS takes them: ends nearer than a thousandth of the distance are one point, and ends exactly
        // that far apart are joined, outside a corner and inside it.
        let moved = [(p(0.0, 0.0), p(1.0, 0.0)), (p(1.0, 0.001), p(2.0, 0.001))];
        let mut outside = Generator::new(1.0, Join::Bevel, p(0.0, 0.0), p(1.0, 0.0));
        outside.moved = moved;
        outside.outside();
        assert_eq!(outside.curve.points, points(&[(1.0, 0.0), (1.0, 0.001)]));
        let mut inside = Generator::new(1.0, Join::Bevel, p(0.0, 0.0), p(1.0, 0.0));
        inside.moved = moved;
        inside.inside();
        assert_eq!(inside.curve.points, points(&[(1.0, 0.0), (0.5, 0.0), (0.5, 0.0005), (1.0, 0.001)]), "in to the corner and back out");
    }

    #[test]
    fn a_line_in_line_goes_straight_on_and_a_turn_back_steps_or_curves_across() {
        assert!(near(&raw_of(&[(0.0, 0.0), (1.0, 0.0), (2.0, 0.0)], 1.0, Join::UNSET), &[(0.0, 1.0), (2.0, 1.0)]));
        let back = [(0.0, 0.0), (2.0, 0.0), (1.0, 0.0)];
        assert!(near(&raw_of(&back, 1.0, Join::Bevel), &[(0.0, 1.0), (2.0, 1.0), (2.0, -1.0), (1.0, -1.0)]));
        // Round: a half circle round the tip, in 32 steps.
        let round = raw_of(&back, 1.0, Join::Round);
        assert_eq!(round.len(), 35);
        assert!(round[2..33].iter().all(|q| (q.distance(p(2.0, 0.0)) - 1.0).abs() < 1e-12 && q.x() > 2.0));
        assert!(round.contains(&p(3.0, 0.0)), "a quarter turn round lies exactly on the axis");
    }

    #[test]
    fn a_walk_passes_over_a_repeated_point_and_a_curve_closes() {
        let meter = &mut Budget::DEFAULT.meter();
        let walked = side_curve(&points(&[(0.0, 0.0), (1.0, 0.0), (1.0, 0.0), (2.0, 0.0)]), 1.0, Join::UNSET, true, meter).unwrap();
        assert_eq!(walked.points, points(&[(0.0, 1.0), (2.0, 1.0)]));
        assert!(side_curve(&points(&[(0.0, 0.0)]), 1.0, Join::UNSET, true, meter).unwrap().points.is_empty());
        assert!(ring(&points(&[(0.0, 0.0)]), 1.0, Join::UNSET, meter).unwrap().is_empty());
        let mut curve = Curve::new(1.0);
        for q in points(&[(0.0, 0.0), (1.0, 0.0), (1.0, 0.00001), (1.0, 1.0)]) {
            curve.add(q);
        }
        curve.close();
        assert_eq!(curve.points, points(&[(0.0, 0.0), (1.0, 0.0), (1.0, 1.0), (0.0, 0.0)]), "a point within a ten-thousandth left out");
        let mut curve = Curve::new(1.0);
        curve.add(p(0.0, 0.0));
        curve.add(p(1e-4, 0.0));
        assert_eq!(curve.points, points(&[(0.0, 0.0), (1e-4, 0.0)]), "a point exactly a ten-thousandth away kept, as GEOS keeps it");
    }
}
