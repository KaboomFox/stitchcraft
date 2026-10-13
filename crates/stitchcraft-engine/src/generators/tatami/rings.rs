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
//!
//! **Near sides only.** Every question about a point, or a stitch, looks at the sides of the rings near it
//! ([`Sides`]), not at every side, and answers as a look at every side would.

use std::cell::RefCell;

use stitchcraft_core::{Exhausted, Meter, Point, exact};

use super::graph::NodeId;
use super::sides::{Side, Sides};
use crate::normalize::region::Polygon;
use crate::normalize::region::geom::on_segment;

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
    /// Their sides, filed by where they lie.
    sides: Sides<'a>,
    /// Where a point lies relative to each ring ([`ON`], [`ODD`]), kept from one question to the next.
    places: RefCell<Vec<u8>>,
}

/// A point lies on the ring.
const ON: u8 = 1;
/// A point lies inside the ring, by the even-odd count, unless it is on it.
const ODD: u8 = 2;

/// Where a point lies on a ring: which ring, and how far along it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Place {
    /// The ring, 0 for the outline.
    pub ring: usize,
    /// How far along it from its start, in millimetres.
    pub at: f64,
}

impl<'a> Rings<'a> {
    /// The rings of `part`. One unit of `meter` for each of their points, and as [`Sides::new`] files them.
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
        let sides = Sides::new(part.rings().collect(), meter)?;
        Ok(Rings { rings, sides, places: RefCell::new(Vec::new()) })
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
    /// point nearest `p`, the first along it on a tie. `None` when no ring is allowed. Costs as
    /// [`Sides::nearest`] does.
    pub fn locate(&self, p: Point, allowed: impl Fn(usize) -> bool, meter: &mut Meter) -> Result<Option<Place>, Exhausted> {
        let near = |side: &Side| distance_to_side(p, side.a, side.b);
        let Some((_, Side { ring, index, a, b })) = self.sides.nearest((p, p), allowed, near, meter)? else { return Ok(None) };
        let (_, t) = nearest_on_side(p, a, b);
        let start = self.rings.get(ring).and_then(|r| r.at.get(index)).copied().unwrap_or(0.0);
        Ok(Some(Place { ring, at: start + t * a.distance(b) }))
    }

    /// Calls `visit` with every side that could meet the stitch from `p` to `q`, in the rings' order and then
    /// along each ring. Costs as [`Sides::in_box`] does.
    pub fn meeting(&self, p: Point, q: Point, meter: &mut Meter, visit: impl FnMut(Side)) -> Result<(), Exhausted> {
        self.sides.in_box(p.x().min(q.x()), p.y().min(q.y()), p.x().max(q.x()), p.y().max(q.y()), meter, visit)
    }

    /// The first of the rings `allowed` that `p` lies within `within` of, by `distance` from its sides.
    /// Costs as [`Sides::in_box`] does.
    pub fn ring_near(
        &self,
        p: Point,
        within: f64,
        allowed: impl Fn(usize) -> bool,
        distance: impl Fn(Point, Point, Point) -> f64,
        meter: &mut Meter,
    ) -> Result<Option<usize>, Exhausted> {
        // A side that near has a point in the box twice as wide, whatever the rounding of its edges.
        let margin = 2.0 * within;
        let mut found = None;
        self.sides.in_box(p.x() - margin, p.y() - margin, p.x() + margin, p.y() + margin, meter, |side| {
            if found.is_none() && allowed(side.ring) && distance(p, side.a, side.b) <= within {
                found = Some(side.ring);
            }
        })?;
        Ok(found)
    }

    /// Whether `p` lies in the part or on its rings, decided as `Polygon::covers` decides it. Costs as
    /// [`Sides::rightward`] does.
    pub fn covers(&self, p: Point, meter: &mut Meter) -> Result<bool, Exhausted> {
        self.place(p, meter, covered)
    }

    /// Whether `p` lies in the part and on none of its rings. Costs as [`Sides::rightward`] does.
    pub fn inside(&self, p: Point, meter: &mut Meter) -> Result<bool, Exhausted> {
        self.place(p, meter, |at| !at.iter().any(|&place| place & ON != 0) && covered(at))
    }

    /// `answer` to where `p` lies relative to each ring, the outline's first: on the ring ([`ON`]), and inside
    /// it ([`ODD`]) by the even-odd count of its sides a ray from `p` to the right crosses, decided exactly
    /// as `locate_in_ring` decides it. Only the sides that could hold `p` or cross the ray are tested.
    fn place<T>(&self, p: Point, meter: &mut Meter, answer: impl FnOnce(&[u8]) -> T) -> Result<T, Exhausted> {
        let mut places = self.places.borrow_mut();
        places.clear();
        places.resize(self.rings.len(), 0);
        self.sides.rightward(p, meter, |Side { ring, a, b, .. }| {
            let Some(place) = places.get_mut(ring) else { return };
            if on_segment(p, a, b) {
                *place |= ON;
            }
            // A side counts when it spans p's height, its lower end included and its upper end not, and p lies
            // to its left, so that it crosses the ray from p to the right.
            let (low, high) = if a.y() <= b.y() { (a, b) } else { (b, a) };
            if low.y() <= p.y() && p.y() < high.y() && exact::side(low, high, p) == exact::Side::Left {
                *place ^= ODD;
            }
        })?;
        Ok(answer(&places))
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
    /// tie it takes the way that does not pass the ring's start. Ink/Stitch takes the way with fewer nodes of
    /// its travel graph instead, which is not always the shorter (`DEV-FILL-011`).
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

/// The nodes `placed` on the rings, each with the next along its ring and the last of a ring with its first,
/// as pairs: the stretches of ring between them. Nodes are taken by ring and then by place, nodes at one
/// place in the order given. A ring of one node has none. One unit of `meter` for each pair.
pub(crate) fn stretches(mut placed: Vec<(NodeId, Place)>, meter: &mut Meter) -> Result<Vec<[(NodeId, Place); 2]>, Exhausted> {
    placed.sort_by(|(_, a), (_, b)| a.ring.cmp(&b.ring).then(a.at.total_cmp(&b.at)));
    let mut pairs = Vec::with_capacity(placed.len());
    for ring in placed.chunk_by(|(_, a), (_, b)| a.ring == b.ring) {
        if ring.len() < 2 {
            continue;
        }
        for (i, &here) in ring.iter().enumerate() {
            meter.charge(1)?;
            let Some(&next) = ring.get((i + 1) % ring.len()) else { continue };
            pairs.push([here, next]);
        }
    }
    Ok(pairs)
}

/// Whether a point lies in the part or on its rings, given where it lies relative to each ring ([`ON`],
/// [`ODD`]), the outline's first: on the outline, or inside it and inside no hole.
fn covered(places: &[u8]) -> bool {
    let inside = |place: u8| place & ON == 0 && place & ODD != 0;
    let Some((&outline, holes)) = places.split_first() else { return false };
    outline & ON != 0 || (inside(outline) && !holes.iter().any(|&hole| inside(hole)))
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
    use stitchcraft_core::{Budget, SplitMix64};

    use super::*;
    use crate::generators::tatami::fixture::{Question, costs, frame, p, scattered};
    use crate::normalize::region::SAME;
    use crate::normalize::region::geom::{Location, locate_in_ring};
    use crate::normalize::stroke::distance_to_segment;

    #[test]
    fn stretches_join_each_node_to_the_next_along_its_ring_and_round_to_the_first() {
        let place = |ring: usize, at: f64| Place { ring, at };
        // Ring 1 has 3 nodes, given out of order, 2 of them at one place; ring 0 has 2, and ring 2 one.
        let placed = vec![(0, place(1, 5.0)), (1, place(0, 3.0)), (2, place(1, 1.0)), (3, place(2, 0.5)), (4, place(0, 1.0)), (5, place(1, 1.0))];
        let pairs: Vec<[usize; 2]> = stretches(placed, &mut Budget::DEFAULT.meter()).unwrap().into_iter().map(|[(a, _), (b, _)]| [a, b]).collect();
        assert_eq!(pairs, [[4, 1], [1, 4], [2, 5], [5, 0], [0, 2]]);
        assert!(stretches(vec![(0, place(0, 0.0)), (1, place(0, 1.0))], &mut Budget { max_stitches: 1, max_work: 1 }.meter()).is_err());
    }

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
    fn points_are_placed_as_a_look_at_every_ring_places_them() {
        // Points inside and outside random rings, some filed in a grid: at a corner, level with one, where a
        // ray to the right meets corners, and on a side as far as rounding lets them.
        let mut rng = SplitMix64::new(10);
        let allowances: [fn(usize) -> bool; 2] = [|_| true, |ring| ring > 0];
        for case in 0..30 {
            let rings = scattered(&mut rng, case);
            let part = Polygon { outline: rings[0].clone(), holes: rings[1..].to_vec() };
            let measured = Rings::new(&part, &mut Budget::DEFAULT.meter()).unwrap();
            let sides: Vec<(Point, Point)> = rings.iter().flat_map(|ring| ring.windows(2).map(|w| (w[0], w[1]))).collect();
            for k in 0..80 {
                let (a, b) = sides[usize::try_from(rng.next_u64() % u64::try_from(sides.len()).unwrap()).unwrap()];
                let q = match k % 4 {
                    0 => p(rng.next_in(-2.0, 22.0), rng.next_in(-2.0, 22.0)),
                    1 => a,
                    2 => p(rng.next_in(-2.0, 22.0), a.y()),
                    _ => a.lerp(b, rng.next_in(0.0, 1.0)),
                };
                let meter = &mut Budget::DEFAULT.meter();
                let on = part.rings().any(|ring| locate_in_ring(q, ring) == Location::Boundary);
                assert_eq!(measured.covers(q, meter).unwrap(), part.covers(q), "case {case}, {q:?}");
                assert_eq!(measured.inside(q, meter).unwrap(), !on && part.covers(q), "case {case}, {q:?}");
                for allowed in allowances {
                    let near = |ring: &[Point]| ring.windows(2).any(|w| distance_to_segment(q, w[0], w[1]) <= SAME);
                    let looked = part.rings().enumerate().find(|&(index, ring)| allowed(index) && near(ring)).map(|(index, _)| index);
                    assert_eq!(measured.ring_near(q, SAME, allowed, distance_to_segment, meter).unwrap(), looked, "case {case}, {q:?}");
                }
            }
        }
    }

    #[test]
    fn questions_cost_the_cells_and_sides_they_look_at() {
        // A part of 116 sides, filed in a grid. The work is pinned: placing a point on the rings, the sides a
        // stitch could meet, the ring a point lies on, and whether a point is covered, or inside.
        let rings = scattered(&mut SplitMix64::new(9), 10);
        let part = Polygon { outline: rings[0].clone(), holes: rings[1..].to_vec() };
        let measured = Rings::new(&part, &mut Budget::DEFAULT.meter()).unwrap();
        let (q, corner) = (p(5.0, 7.0), rings[1][0]);
        let questions: [Question<'_>; 5] = [
            Box::new(|meter| measured.locate(q, |_| true, meter).map(|_| ())),
            Box::new(|meter| measured.meeting(q, p(9.0, 8.0), meter, |_| ())),
            Box::new(|meter| measured.ring_near(corner, SAME, |_| true, distance_to_segment, meter).map(|_| ())),
            Box::new(|meter| measured.covers(q, meter).map(|_| ())),
            Box::new(|meter| measured.inside(corner, meter).map(|_| ())),
        ];
        assert_eq!(costs(&questions), [54, 15, 5, 42, 9]);
    }

    #[test]
    fn measuring_is_charged_to_the_budget() {
        let part = frame();
        let used = |meter: Meter| Budget::DEFAULT.max_work - meter.work_left();
        let mut meter = Budget::DEFAULT.meter();
        let rings = Rings::new(&part, &mut meter).unwrap();
        let mut filing = Budget::DEFAULT.meter();
        Sides::new(part.rings().collect(), &mut filing).unwrap();
        assert_eq!(used(meter), 10 + used(filing), "a unit per point, and the filing of the sides");
        assert!(Rings::new(&part, &mut Budget { max_stitches: 1, max_work: 3 }.meter()).is_err());
        // Placing a point costs work, all of which it needs.
        let mut placing = Budget::DEFAULT.meter();
        rings.locate(p(0.0, 0.0), |_| true, &mut placing).unwrap();
        let cost = used(placing);
        assert!(cost > 0);
        assert!(rings.locate(p(0.0, 0.0), |_| true, &mut Budget { max_stitches: 1, max_work: cost - 1 }.meter()).is_err());
        assert_eq!(distance_to_side(p(0.0, 3.0), p(0.0, 0.0), p(4.0, 0.0)), 3.0);
    }
}
