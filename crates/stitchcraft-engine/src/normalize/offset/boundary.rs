//! The edge of a stroke: which parts of the ring around a polyline lie on the boundary of the area it
//! covers.
//!
//! The ring of raw curves ([`super::generator::ring`]) crosses itself wherever the stroke folds over:
//! inside tight corners, where the line comes back near itself, along the closing segments. The area the
//! stroke covers is where the ring winds round at least once, counted clockwise, the way GEOS counts the
//! depth of a buffer. Its boundary is made of the pieces of the ring with that area on one side only.
//!
//! The ring is cut wherever it meets itself. Each piece is then judged by the winding of the ring at two
//! points just beside its middle, one on each side, close enough that no other piece passes between.
//! Boundary pieces are turned to keep the covered area on their right, and joined end to end into the
//! rings of the boundary. Work is charged to the meter: one unit per pair of segments and per piece and
//! segment measured.

use std::collections::{BTreeMap, BTreeSet};

use stitchcraft_core::exact::{self, Side};
use stitchcraft_core::{Exhausted, Meter, Point, math};

use super::generator::{Meet, segments_meet};
use crate::normalize::near::{length, to_side};

/// How far beside a piece's middle its sides are judged, at most, as a fraction of the stroke's half
/// width.
const BESIDE: f64 = 1e-3;
/// And at least, so that the two points differ from the middle.
const BESIDE_LEAST: f64 = 1e-12;
/// A segment's end this near another segment, as a fraction of the stroke's half width, lies on it.
const ON_SEGMENT: f64 = 1e-9;

/// The rings of the boundary of the area that `ring` (closed: its last point is its first) winds round
/// clockwise, each as its pieces in order with the area on their right. `half` is the stroke's half
/// width, the scale for nearness.
pub(super) fn boundary(ring: &[Point], half: f64, meter: &mut Meter) -> Result<Vec<Vec<(Point, Point)>>, Exhausted> {
    let pieces = cut(ring, half * ON_SEGMENT, meter)?;
    let mut edges = Vec::new();
    let mut seen = BTreeSet::new();
    for (k, &(p, q)) in pieces.iter().enumerate() {
        let span = length(p, q);
        let middle = p.lerp(q, 0.5);
        let mut beside = half * BESIDE;
        for (j, &(a, b)) in pieces.iter().enumerate() {
            meter.charge(1)?;
            let gap = to_side(middle, a, b);
            if j != k && gap > 0.0 {
                beside = beside.min(gap / 4.0);
            }
        }
        let beside = beside.max(half * BESIDE_LEAST);
        let (nx, ny) = ((p.y() - q.y()) / span, (q.x() - p.x()) / span);
        let point = |s: f64| Point::new(middle.x() + s * nx, middle.y() + s * ny).unwrap_or(middle);
        let (left, right) = (depth(ring, point(beside), meter)?, depth(ring, point(-beside), meter)?);
        let edge = if left == 0 && right >= 1 {
            (p, q)
        } else if right == 0 && left >= 1 {
            (q, p)
        } else {
            continue;
        };
        // Where the ring runs over itself, its pieces there coincide, and are one edge.
        if seen.insert((key(edge.0), key(edge.1))) {
            edges.push(edge);
        }
    }
    Ok(rings(&edges))
}

/// How many times `ring` winds round `p` clockwise.
fn depth(ring: &[Point], p: Point, meter: &mut Meter) -> Result<i64, Exhausted> {
    meter.charge(u64::try_from(ring.len()).unwrap_or(u64::MAX))?;
    Ok(-winding(ring.windows(2).map(|pair| (pair[0], pair[1])), p))
}

/// How many times `segments`, end to end round a ring, wind round `p` counter-clockwise.
fn winding(segments: impl Iterator<Item = (Point, Point)>, p: Point) -> i64 {
    let mut winding = 0_i64;
    for (a, b) in segments {
        if a.y() <= p.y() {
            if b.y() > p.y() && exact::side(a, b, p) == Side::Left {
                winding += 1;
            }
        } else if b.y() <= p.y() && exact::side(a, b, p) == Side::Right {
            winding -= 1;
        }
    }
    winding
}

/// The pieces of `ring`'s segments between the points where it meets itself, in ring order. An end of a
/// segment within `near` of another segment, and farther than `near` from both its ends, counts as on it.
fn cut(ring: &[Point], near: f64, meter: &mut Meter) -> Result<Vec<(Point, Point)>, Exhausted> {
    let segments: Vec<(Point, Point)> = ring.windows(2).map(|pair| (pair[0], pair[1])).collect();
    let mut cuts: Vec<Vec<Point>> = vec![Vec::new(); segments.len()];
    for (i, &(a, b)) in segments.iter().enumerate() {
        for (j, &(c, d)) in segments.iter().enumerate().skip(i + 1) {
            meter.charge(1)?;
            if !boxes_meet(a, b, c, d, near) {
                continue;
            }
            // Segments that cross or touch are cut where they meet. Where they overlap, the ends of the
            // overlap are ends of the segments lying on the other, which the rule below cuts at.
            if let Meet::At(p) = segments_meet(a, b, c, d) {
                for k in [i, j] {
                    if let Some(list) = cuts.get_mut(k) {
                        list.push(p);
                    }
                }
            }
            // An end of one segment within `near` of the other's middle stretch cuts it there too: it lies
            // on that segment, or rounding has put it a hair to one side.
            for (k, p, (s, t)) in [(i, c, (a, b)), (i, d, (a, b)), (j, a, (c, d)), (j, b, (c, d))] {
                if length(p, s) > near
                    && length(p, t) > near
                    && to_side(p, s, t) <= near
                    && let Some(list) = cuts.get_mut(k)
                {
                    list.push(p);
                }
            }
        }
    }
    let mut nodes = Nodes { near, cells: BTreeMap::new() };
    let mut pieces = Vec::new();
    for (&(a, b), points) in segments.iter().zip(&cuts) {
        // The cuts in order from `a`. The segment's own ends among them add no piece: a piece already
        // starts or ends at each.
        let mut along: Vec<(f64, Point)> = points.iter().map(|&p| (length(a, p), p)).collect();
        along.sort_by(|x, y| x.0.total_cmp(&y.0));
        let mut from = nodes.node(a);
        for p in along.into_iter().map(|(_, p)| p).chain([b]) {
            let p = nodes.node(p);
            if p != from {
                pieces.push((from, p));
                from = p;
            }
        }
    }
    Ok(pieces)
}

/// The points where pieces of the ring meet. Points nearer each other than `near` are one, the first
/// seen: where segments nearly coincide, rounding puts the points they meet at a hair apart, and the
/// ring must still run through one point there.
struct Nodes {
    near: f64,
    /// The points seen, by the square of side `near` they lie in.
    cells: BTreeMap<(i64, i64), Vec<Point>>,
}

impl Nodes {
    /// The point that stands for `p`: a point seen before within `near` of it, or `p` itself.
    fn node(&mut self, p: Point) -> Point {
        // Saturating casts: a cell index beyond i64 only merges cells far beyond any drawing.
        #[allow(clippy::cast_possible_truncation)]
        let cell = |v: f64| (v / self.near).floor() as i64;
        let (cx, cy) = (cell(p.x()), cell(p.y()));
        for x in cx.saturating_sub(1)..=cx.saturating_add(1) {
            for y in cy.saturating_sub(1)..=cy.saturating_add(1) {
                if let Some(&q) = self.cells.get(&(x, y)).and_then(|points| points.iter().find(|&&q| length(p, q) < self.near)) {
                    return q;
                }
            }
        }
        self.cells.entry((cx, cy)).or_default().push(p);
        p
    }
}

/// Whether the boxes around the segments `a`–`b` and `c`–`d`, grown by `near`, meet.
fn boxes_meet(a: Point, b: Point, c: Point, d: Point, near: f64) -> bool {
    a.x().max(b.x()) + near >= c.x().min(d.x())
        && c.x().max(d.x()) + near >= a.x().min(b.x())
        && a.y().max(b.y()) + near >= c.y().min(d.y())
        && c.y().max(d.y()) + near >= a.y().min(b.y())
}

/// A point as a key: its coordinates' bits, with −0 taken as 0.
fn key(p: Point) -> (u64, u64) {
    ((p.x() + 0.0).to_bits(), (p.y() + 0.0).to_bits())
}

/// `edges` joined end to end into rings, each started at the first edge not yet used. Where several unused
/// edges leave a point, the ring takes the one that turns farthest to the right. With the covered area on
/// the right of every edge, that keeps each ring as small as it can be, as the rings of GEOS's polygons
/// are where they touch themselves.
fn rings(edges: &[(Point, Point)]) -> Vec<Vec<(Point, Point)>> {
    let mut from: BTreeMap<(u64, u64), Vec<usize>> = BTreeMap::new();
    for (k, &(p, _)) in edges.iter().enumerate() {
        from.entry(key(p)).or_default().push(k);
    }
    let mut used = vec![false; edges.len()];
    let mut chains = Vec::new();
    for (start, &edge) in edges.iter().enumerate() {
        if used.get(start).copied().unwrap_or(true) {
            continue;
        }
        let mut chain = vec![edge];
        if let Some(u) = used.get_mut(start) {
            *u = true;
        }
        let mut last = edge;
        while let Some((k, e)) = from.get(&key(last.1)).and_then(|ks| {
            ks.iter()
                .filter(|&&k| !used.get(k).copied().unwrap_or(true))
                .filter_map(|&k| edges.get(k).map(|&e| (k, e)))
                .min_by(|a, b| turn(last, a.1).total_cmp(&turn(last, b.1)))
        }) {
            if let Some(u) = used.get_mut(k) {
                *u = true;
            }
            chain.push(e);
            last = e;
        }
        chains.push(chain);
    }
    chains
}

/// How far `outgoing` turns from `incoming`, in radians, positive to the left and negative to the right; π
/// when it goes straight back.
fn turn(incoming: (Point, Point), outgoing: (Point, Point)) -> f64 {
    let (ux, uy) = (incoming.1.x() - incoming.0.x(), incoming.1.y() - incoming.0.y());
    let (vx, vy) = (outgoing.1.x() - outgoing.0.x(), outgoing.1.y() - outgoing.0.y());
    math::atan2(ux * vy - uy * vx, ux * vx + uy * vy)
}

/// Twice the signed area `ring` encloses: positive when it runs counter-clockwise in y-up axes. Areas are
/// only told apart by sign and compared, so the half is never taken.
pub(super) fn doubled_area(ring: &[(Point, Point)]) -> f64 {
    ring.iter().map(|(p, q)| p.x() * q.y() - q.x() * p.y()).sum::<f64>()
}

/// Whether `p` lies inside `ring`: whether the ring winds round it.
pub(super) fn encloses(ring: &[(Point, Point)], p: Point) -> bool {
    winding(ring.iter().copied(), p) != 0
}

#[cfg(test)]
mod tests {
    use stitchcraft_core::Budget;

    use super::*;

    fn p(x: f64, y: f64) -> Point {
        Point::new(x, y).unwrap()
    }

    #[test]
    fn a_square_drawn_clockwise_is_its_own_boundary() {
        // Clockwise in y-up axes: (0,0) → (0,1) → (1,1) → (1,0).
        let ring = [p(0.0, 0.0), p(0.0, 1.0), p(1.0, 1.0), p(1.0, 0.0), p(0.0, 0.0)];
        let found = boundary(&ring, 1.0, &mut Budget::DEFAULT.meter()).unwrap();
        assert_eq!(found, [vec![(p(0.0, 0.0), p(0.0, 1.0)), (p(0.0, 1.0), p(1.0, 1.0)), (p(1.0, 1.0), p(1.0, 0.0)), (p(1.0, 0.0), p(0.0, 0.0))]]);
        // Counter-clockwise it covers nothing.
        let reversed: Vec<Point> = ring.iter().rev().copied().collect();
        assert!(boundary(&reversed, 1.0, &mut Budget::DEFAULT.meter()).unwrap().is_empty());
    }

    #[test]
    fn pieces_that_run_over_each_other_are_one_edge_turned_to_keep_the_area_on_its_right() {
        // A square drawn clockwise, then out along its bottom edge and back: 3 pieces there, 2 running
        // left and 1 right, with the square above them.
        let ring = [p(0.0, 0.0), p(0.0, 1.0), p(1.0, 1.0), p(1.0, 0.0), p(0.0, 0.0), p(1.0, 0.0), p(0.0, 0.0)];
        let found = boundary(&ring, 1.0, &mut Budget::DEFAULT.meter()).unwrap();
        assert_eq!(found, [vec![(p(0.0, 0.0), p(0.0, 1.0)), (p(0.0, 1.0), p(1.0, 1.0)), (p(1.0, 1.0), p(1.0, 0.0)), (p(1.0, 0.0), p(0.0, 0.0))]]);
    }

    #[test]
    fn points_near_apart_are_2_nodes_and_an_end_near_a_segment_s_end_does_not_cut_it() {
        // D lies exactly `near` (5/16) from A, the end of AB, and 1/4 beside AB; A lies exactly `near` from
        // CD's end D. Neither is farther than `near` from the segment's ends, so neither cuts it, and A and
        // D stay 2 nodes: the ring's pieces are its segments.
        let (a, b, c, d) = (p(0.0, 0.0), p(4.0, 0.0), p(4.0, 2.0), p(0.1875, 0.25));
        let found = cut(&[a, b, c, d, a], 0.3125, &mut Budget::DEFAULT.meter()).unwrap();
        assert_eq!(found, [(a, b), (b, c), (c, d), (d, a)]);
    }

    #[test]
    fn a_sliver_of_the_stroke_is_judged_within_it() {
        // A rectangle 0.0004 high, clockwise, in a stroke of half width 1: its sides are judged a quarter of
        // the way across it, not a thousandth of the half width away, which would reach the other side.
        let h = 0.0004;
        let ring = [p(0.0, 0.0), p(0.0, h), p(10.0, h), p(10.0, 0.0), p(0.0, 0.0)];
        let edges: Vec<(Point, Point)> = ring.windows(2).map(|pair| (pair[0], pair[1])).collect();
        assert_eq!(boundary(&ring, 1.0, &mut Budget::DEFAULT.meter()).unwrap(), [edges]);
    }

    #[test]
    fn pieces_that_run_over_each_other_far_from_the_origin_are_judged_as_near_it() {
        // The square with its bottom edge run over twice, 5 m from the origin in a stroke 0.1 mm wide:
        // pieces lying on each other are not beside each other, so the points that judge them stay where
        // the coordinates can still tell them from the middle.
        let at = |x: f64, y: f64| p(5_000.0 + x, 5_000.0 + y);
        let ring = [at(0.0, 0.0), at(0.0, 1.0), at(1.0, 1.0), at(1.0, 0.0), at(0.0, 0.0), at(1.0, 0.0), at(0.0, 0.0)];
        let found = boundary(&ring, 0.05, &mut Budget::DEFAULT.meter()).unwrap();
        assert_eq!(
            found,
            [vec![(at(0.0, 0.0), at(0.0, 1.0)), (at(0.0, 1.0), at(1.0, 1.0)), (at(1.0, 1.0), at(1.0, 0.0)), (at(1.0, 0.0), at(0.0, 0.0))]]
        );
    }

    #[test]
    fn a_turn_is_measured_from_the_incoming_edge_left_positive() {
        let incoming = (p(1.0, 1.0), p(3.0, 4.0));
        // (2, 3) then (−4, 5): cross product 22, dot product 7.
        assert_eq!(turn(incoming, (p(3.0, 4.0), p(-1.0, 9.0))), math::atan2(22.0, 7.0));
        assert_eq!(turn(incoming, (p(3.0, 4.0), p(1.0, 1.0))), math::atan2(0.0, -13.0), "straight back: π");
    }

    #[test]
    fn edges_meeting_at_minus_0_and_0_join() {
        // −0 and 0 are one coordinate, whichever sign a computation gave it.
        let edges = [(p(0.0, 1.0), p(1.0, -0.0)), (p(1.0, 0.0), p(-0.0, 0.0)), (p(0.0, 0.0), p(0.0, 1.0))];
        assert_eq!(rings(&edges), [edges.to_vec()]);
    }

    #[test]
    fn a_ring_touching_itself_turns_farthest_right() {
        // 2 triangles, both clockwise, touching at the origin. The left one's edge into the origin is listed
        // first and the right one's edges next: the ring turns right there, round its own triangle.
        let o = p(0.0, 0.0);
        let left = [(p(-2.0, 1.0), o), (o, p(-2.0, -1.0)), (p(-2.0, -1.0), p(-2.0, 1.0))];
        let right = [(o, p(0.1, 5.0)), (p(0.1, 5.0), p(3.0, 5.0)), (p(3.0, 5.0), o)];
        let edges = [left[0], right[0], right[1], right[2], left[1], left[2]];
        assert_eq!(rings(&edges), [left.to_vec(), right.to_vec()]);
    }

    #[test]
    fn a_point_level_with_a_corner_is_wound_round_once_at_most() {
        // A diamond, counter-clockwise, and points level with its side corners: each edge counts from its
        // lower end up to but not including its upper end, so a corner on the level is counted once.
        let diamond = [(p(0.0, -1.0), p(1.0, 0.0)), (p(1.0, 0.0), p(0.0, 1.0)), (p(0.0, 1.0), p(-1.0, 0.0)), (p(-1.0, 0.0), p(0.0, -1.0))];
        let wound = |x: f64| winding(diamond.iter().copied(), p(x, 0.0));
        assert_eq!([wound(-2.0), wound(0.0), wound(2.0)], [0, 1, 0]);
    }

    #[test]
    fn a_ring_that_crosses_itself_is_cut_where_it_does() {
        // A figure of eight: one lobe clockwise, one counter-clockwise; only the clockwise one covers.
        let ring = [p(0.0, 0.0), p(0.0, 1.0), p(2.0, -1.0), p(2.0, 0.0), p(0.0, 0.0)];
        let found = boundary(&ring, 1.0, &mut Budget::DEFAULT.meter()).unwrap();
        assert_eq!(found, [vec![(p(0.0, 0.0), p(0.0, 1.0)), (p(0.0, 1.0), p(1.0, 0.0)), (p(1.0, 0.0), p(0.0, 0.0))]]);
    }
}
