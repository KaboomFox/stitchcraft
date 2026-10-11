//! A part's rings as lines measured along: where on them a point lies, and the way along a ring between
//! 2 of its points ([`super::route`] and its travel).
//!
//! **Why measure along the rings.** The ends of a fill's row segments lie on the rings of its part: the
//! outline and the holes. Routing orders them along each ring, and the needle travels along the rings
//! between rows. A point's place on a ring is how far along the ring it lies from the ring's start, in
//! the ring's own direction, as Ink/Stitch measures it. A point belongs to the ring nearest it, the
//! outline before a hole when both are as near, and lies at the point of that ring nearest it, the first
//! along the ring when several are as near. The rings of [`crate::normalize::region`] run clockwise for
//! outlines and counter-clockwise for holes in y-up axes, as Ink/Stitch's do, but start where the drawing
//! starts them (`DEV-FILL-005`).

use stitchcraft_core::{Exhausted, Meter, Point};

use crate::normalize::region::Polygon;

/// One ring: its points, closed (the first repeated at the end), and how far along it each lies.
#[derive(Clone, Debug)]
struct Ring<'a> {
    points: &'a [Point],
    at: Vec<f64>,
}

impl Ring<'_> {
    /// How long the ring is.
    fn length(&self) -> f64 {
        self.at.last().copied().unwrap_or(0.0)
    }
}

/// A part's rings, the outline first and then its holes.
#[derive(Clone, Debug)]
pub(crate) struct Rings<'a> {
    rings: Vec<Ring<'a>>,
}

/// Where a point lies on a ring: which ring, and how far along it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Place {
    /// The ring, 0 for the outline.
    pub ring: usize,
    /// How far along it from its start, in millimetres.
    pub at: f64,
}

impl<'a> Rings<'a> {
    /// The rings of `part`. One unit of `meter` for each of their points.
    pub fn new(part: &'a Polygon, meter: &mut Meter) -> Result<Rings<'a>, Exhausted> {
        let mut rings = Vec::with_capacity(1 + part.holes.len());
        for points in std::iter::once(&part.outline).chain(&part.holes) {
            meter.charge(u64::try_from(points.len()).unwrap_or(u64::MAX))?;
            let mut at = Vec::with_capacity(points.len());
            let mut along = 0.0;
            for (i, point) in points.iter().enumerate() {
                if let Some(before) = i.checked_sub(1).and_then(|b| points.get(b)) {
                    along += before.distance(*point);
                }
                at.push(along);
            }
            rings.push(Ring { points, at });
        }
        Ok(Rings { rings })
    }

    /// How many rings there are.
    pub fn count(&self) -> usize {
        self.rings.len()
    }

    /// How long ring `ring` is; 0 for a ring that does not exist.
    pub fn length(&self, ring: usize) -> f64 {
        self.rings.get(ring).map_or(0.0, Ring::length)
    }

    /// Where `p` lies on the rings `allowed` lets it lie on: the nearest of them, the first on a tie, at its
    /// point nearest `p`, the first along it on a tie. `None` when no ring is allowed. One unit of
    /// `meter` for each side of a ring looked at.
    pub fn locate(&self, p: Point, allowed: impl Fn(usize) -> bool, meter: &mut Meter) -> Result<Option<Place>, Exhausted> {
        let mut best: Option<(f64, Place)> = None;
        for (index, ring) in self.rings.iter().enumerate().filter(|(index, _)| allowed(*index)) {
            meter.charge(u64::try_from(ring.points.len()).unwrap_or(u64::MAX))?;
            for (pair, at) in ring.points.windows(2).zip(&ring.at) {
                let [a, b] = [pair[0], pair[1]];
                let (distance, t) = nearest_on_side(p, a, b);
                if best.is_none_or(|(d, _)| distance < d) {
                    best = Some((distance, Place { ring: index, at: at + t * a.distance(b) }));
                }
            }
        }
        Ok(best.map(|(_, place)| place))
    }

    /// The point `at` along ring `ring`; `None` when there is no such ring.
    pub fn point(&self, place: Place) -> Option<Point> {
        let ring = self.rings.get(place.ring)?;
        let side = ring.at.partition_point(|&a| a <= place.at).clamp(1, ring.points.len().max(2) - 1);
        let (a, b) = (ring.points.get(side - 1)?, ring.points.get(side)?);
        let (from, to) = (ring.at.get(side - 1)?, ring.at.get(side)?);
        let t = if to > from { ((place.at - from) / (to - from)).clamp(0.0, 1.0) } else { 0.0 };
        Some(a.lerp(*b, t))
    }

    /// Whether the shorter way along ring `ring` from `from` to `to` runs in the ring's own direction. On a
    /// tie it takes the way that does not pass the ring's start, as Ink/Stitch's travel does.
    pub fn forwards(&self, ring: usize, from: f64, to: f64) -> bool {
        let length = self.length(ring);
        let ahead = (to - from).rem_euclid(length.max(f64::MIN_POSITIVE));
        let behind = length - ahead;
        if ahead == behind { from <= to } else { ahead < behind }
    }

    /// How long the way along ring `ring` from `from` to `to` is, in the ring's direction or against it.
    pub fn way_length(&self, ring: usize, from: f64, to: f64, forwards: bool) -> f64 {
        let length = self.length(ring);
        let ahead = (to - from).rem_euclid(length.max(f64::MIN_POSITIVE));
        if forwards { ahead } else { (length - ahead) % length.max(f64::MIN_POSITIVE) }
    }

    /// The way along ring `ring` from `from` to `to`, in the ring's direction or against it: the 2 points
    /// and the ring's points between them. One unit of `meter` for each point.
    pub fn way(&self, ring: usize, from: f64, to: f64, forwards: bool, meter: &mut Meter) -> Result<Vec<Point>, Exhausted> {
        let Some(r) = self.rings.get(ring) else { return Ok(Vec::new()) };
        let point = |at| self.point(Place { ring, at });
        let (Some(start), Some(end)) = (point(from), point(to)) else { return Ok(Vec::new()) };
        let length = r.length().max(f64::MIN_POSITIVE);
        // The ring's points by how far along the way they lie, the ring's start counted once.
        let ahead = |at: f64| if forwards { (at - from).rem_euclid(length) } else { (from - at).rem_euclid(length) };
        let span = self.way_length(ring, from, to, forwards);
        let mut between: Vec<(f64, Point)> = Vec::new();
        for (point, &at) in r.points.iter().zip(&r.at).skip(1) {
            meter.charge(1)?;
            let d = ahead(at);
            if d > 0.0 && d < span {
                between.push((d, *point));
            }
        }
        between.sort_by(|a, b| a.0.total_cmp(&b.0));
        let mut way = Vec::with_capacity(between.len() + 2);
        way.push(start);
        way.extend(between.into_iter().map(|(_, p)| p));
        way.push(end);
        Ok(way)
    }
}

/// How far `p` lies from the side from `a` to `b`, and how far along the side, from 0 at `a` to 1 at `b`,
/// its nearest point lies.
fn nearest_on_side(p: Point, a: Point, b: Point) -> (f64, f64) {
    let (dx, dy) = (b.x() - a.x(), b.y() - a.y());
    let length2 = dx * dx + dy * dy;
    let t = if length2 > 0.0 { (((p.x() - a.x()) * dx + (p.y() - a.y()) * dy) / length2).clamp(0.0, 1.0) } else { 0.0 };
    (p.distance(a.lerp(b, t)), t)
}

/// How far `p` lies from the side from `a` to `b`.
pub(crate) fn distance_to_side(p: Point, a: Point, b: Point) -> f64 {
    nearest_on_side(p, a, b).0
}

#[cfg(test)]
mod tests {
    use stitchcraft_core::Budget;

    use super::*;
    use crate::generators::tatami::fixture::{frame, p};

    #[test]
    fn a_point_lies_on_the_nearest_ring_at_its_nearest_point() {
        let part = frame();
        let rings = Rings::new(&part, &mut Budget::DEFAULT.meter()).unwrap();
        let locate = |q: Point| rings.locate(q, |_| true, &mut Budget::DEFAULT.meter()).unwrap().unwrap();
        assert_eq!((rings.count(), rings.length(0), rings.length(1)), (2, 28.0, 8.0));
        assert_eq!(locate(p(0.0, 1.0)), Place { ring: 0, at: 1.0 });
        assert_eq!(locate(p(10.0, 1.0)), Place { ring: 0, at: 4.0 + 10.0 + 3.0 });
        assert_eq!(locate(p(5.0, 2.5)), Place { ring: 1, at: 2.0 + 2.0 + 1.0 }, "the hole is nearer");
        // The start of a ring is 0 along it, not its length, and a corner lies where the side before it
        // ends.
        assert_eq!(locate(p(-1.0, -1.0)), Place { ring: 0, at: 0.0 });
        assert_eq!(locate(p(-1.0, 5.0)), Place { ring: 0, at: 4.0 });
        // As near both: the outline, the first ring.
        let touching = Polygon { outline: part.outline.clone(), holes: vec![vec![p(0.0, 1.0), p(2.0, 1.0), p(1.0, 2.0), p(0.0, 1.0)]] };
        let rings = Rings::new(&touching, &mut Budget::DEFAULT.meter()).unwrap();
        assert_eq!(rings.locate(p(0.0, 1.0), |_| true, &mut Budget::DEFAULT.meter()).unwrap(), Some(Place { ring: 0, at: 1.0 }));
        assert_eq!(rings.locate(p(0.0, 1.0), |r| r == 1, &mut Budget::DEFAULT.meter()).unwrap().map(|q| q.ring), Some(1));
        assert_eq!(rings.locate(p(0.0, 1.0), |_| false, &mut Budget::DEFAULT.meter()).unwrap(), None);
    }

    #[test]
    fn a_side_of_no_length_has_its_nearest_point_at_its_start() {
        // The outline's first point drawn twice: a point nearest it lies 0 along the outline.
        let part = Polygon { outline: vec![p(0.0, 0.0), p(0.0, 0.0), p(0.0, 4.0), p(10.0, 4.0), p(10.0, 0.0), p(0.0, 0.0)], holes: Vec::new() };
        let rings = Rings::new(&part, &mut Budget::DEFAULT.meter()).unwrap();
        assert_eq!(rings.locate(p(-1.0, -1.0), |_| true, &mut Budget::DEFAULT.meter()).unwrap(), Some(Place { ring: 0, at: 0.0 }));
        assert_eq!(distance_to_side(p(3.0, 4.0), p(0.0, 0.0), p(0.0, 0.0)), 5.0);
    }

    #[test]
    fn points_along_a_ring_come_back_from_their_places() {
        let part = frame();
        let rings = Rings::new(&part, &mut Budget::DEFAULT.meter()).unwrap();
        let at = |ring, at| rings.point(Place { ring, at }).unwrap();
        assert_eq!([at(0, 0.0), at(0, 1.0), at(0, 4.0), at(0, 9.0), at(0, 28.0)], [p(0.0, 0.0), p(0.0, 1.0), p(0.0, 4.0), p(5.0, 4.0), p(0.0, 0.0)]);
        assert_eq!(at(1, 7.0), p(4.0, 2.0));
        assert_eq!(rings.point(Place { ring: 2, at: 0.0 }), None);
    }

    #[test]
    fn the_way_between_two_places_takes_the_shorter_side_and_its_corners() {
        let part = frame();
        let rings = Rings::new(&part, &mut Budget::DEFAULT.meter()).unwrap();
        // Rounded to a nanometre: a point inside a side is interpolated.
        let way = |from, to| {
            let forwards = rings.forwards(0, from, to);
            let points = rings.way(0, from, to, forwards, &mut Budget::DEFAULT.meter()).unwrap();
            (forwards, points.into_iter().map(|q| p((q.x() * 1e9).round() / 1e9, (q.y() * 1e9).round() / 1e9)).collect::<Vec<_>>())
        };
        // From the left side 1 down from the top, down round the bottom corners to the right side half up
        // from the bottom: 13.5 that way, 14.5 the other.
        assert_eq!(way(1.0, 14.5), (true, vec![p(0.0, 1.0), p(0.0, 4.0), p(10.0, 4.0), p(10.0, 3.5)]));
        assert_eq!(way(14.5, 1.0), (false, vec![p(10.0, 3.5), p(10.0, 4.0), p(0.0, 4.0), p(0.0, 1.0)]));
        // Past the ring's start: from 26 on the top side round its start to 1.
        assert_eq!(way(26.0, 1.0), (true, vec![p(2.0, 0.0), p(0.0, 0.0), p(0.0, 1.0)]));
        // As long either way: the way that does not pass the start.
        assert!(way(2.0, 16.0).0);
        assert!(!way(16.0, 2.0).0);
        assert_eq!(rings.way_length(0, 16.0, 2.0, false), 14.0);
        assert_eq!(rings.way_length(0, 3.0, 3.0, true), 0.0);
        assert_eq!(way(3.0, 3.0), (true, vec![p(0.0, 3.0), p(0.0, 3.0)]));
        // From a corner and to one: each corner once, as an end of the way.
        assert_eq!(way(4.0, 15.0), (true, vec![p(0.0, 4.0), p(10.0, 4.0), p(10.0, 3.0)]));
        assert_eq!(way(1.0, 14.0), (true, vec![p(0.0, 1.0), p(0.0, 4.0), p(10.0, 4.0)]));
        assert!(rings.way(7, 0.0, 1.0, true, &mut Budget::DEFAULT.meter()).unwrap().is_empty());
    }

    #[test]
    fn measuring_is_charged_to_the_budget() {
        let part = frame();
        let mut meter = Budget::DEFAULT.meter();
        let rings = Rings::new(&part, &mut meter).unwrap();
        assert_eq!(Budget::DEFAULT.max_work - meter.work_left(), 10, "a unit per point");
        assert!(rings.locate(p(0.0, 0.0), |_| true, &mut Budget { max_stitches: 1, max_work: 6 }.meter()).is_err());
        assert!(Rings::new(&part, &mut Budget { max_stitches: 1, max_work: 3 }.meter()).is_err());
        assert_eq!(distance_to_side(p(0.0, 3.0), p(0.0, 0.0), p(4.0, 0.0)), 3.0);
    }
}
