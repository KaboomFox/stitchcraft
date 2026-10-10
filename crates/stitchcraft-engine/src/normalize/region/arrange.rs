//! The faces a fill's rings cut the plane into, and how many times the rings wind round each.
//!
//! The rings are cut wherever they cross, touch or run along each other, and every stretch of line between
//! cuts becomes one edge, however many rings run along it: each edge counts the rings that run along it one
//! way less those that run the other ([`cut`]). The edges bound faces. Walking every edge both ways, and at
//! each point taking the sharpest right turn, traces rings with a face on their right: a ring that turns
//! clockwise in y-up axes bounds a face, and one that turns the other way is the outside of a group of
//! faces, which lies inside the smallest face around it, or outside everything ([`Arrangement::faces`]).
//!
//! The winding number outside everything is 0, and crossing an edge from its right to its left adds the
//! edge's count. So every face's winding number follows from its neighbours', in whole numbers, with no
//! point tested: the fill rule then says which faces are inside, nonzero or odd.
//!
//! Sides are decided exactly ([`stitchcraft_core::exact::side`]). Only where 2 segments cross inside both
//! is a point worked out, and a crossing within a billionth of a millimetre of a point already found is
//! taken to be that point, so that 3 segments crossing at one point cut each other there once.

use std::collections::BTreeMap;

use stitchcraft_core::{Exhausted, Meter, Point};

use super::geom::{Envelope, Hit, Location, closed, compare_angle, hit, locate_in_ring, signed_area, without_repeats};

/// Crossings this near a point already found, in millimetres, are that point.
const SAME: f64 = 1e-9;

/// The rings cut where they meet: points, and edges between them.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Arrangement {
    /// The points, numbered in the order the rings reach them.
    pub(crate) points: Vec<Point>,
    /// The edges, each from one point to another: indices into `points`.
    pub(crate) edges: Vec<(usize, usize)>,
    /// For each edge, the rings that run along it from its first point to its second, less those that run
    /// the other way.
    pub(crate) count: Vec<i64>,
}

/// One segment of a ring, and where it is cut.
struct Segment {
    a: Point,
    b: Point,
    cuts: Vec<Point>,
}

/// `rings` (each open or closed, in the direction drawn) cut where they meet. One unit of `meter` per
/// point, per pair of segments whose spans along x overlap, and per segment and cut.
pub(crate) fn cut(rings: &[Vec<Point>], meter: &mut Meter) -> Result<Arrangement, Exhausted> {
    let mut segments: Vec<Segment> = Vec::new();
    for ring in rings {
        let ring = without_repeats(&closed(ring));
        meter.charge(u64::try_from(ring.len()).unwrap_or(u64::MAX))?;
        segments.extend(ring.windows(2).map(|w| Segment { a: w[0], b: w[1], cuts: Vec::new() }));
    }
    // A sweep along x: each segment against those that start before it ends.
    let left = |s: &Segment| s.a.x().min(s.b.x());
    let mut by_left: Vec<usize> = (0..segments.len()).collect();
    by_left.sort_by(|&i, &j| segments.get(i).map_or(0.0, left).total_cmp(&segments.get(j).map_or(0.0, left)).then(i.cmp(&j)));
    let mut found: Vec<(usize, Vec<Point>)> = Vec::new();
    for (k, &i) in by_left.iter().enumerate() {
        let Some(s) = segments.get(i) else { continue };
        let (env, right) = (Envelope::of(&[s.a, s.b]), s.a.x().max(s.b.x()));
        for &j in by_left.iter().skip(k + 1) {
            let Some(t) = segments.get(j) else { continue };
            if left(t) > right {
                break;
            }
            meter.charge(1)?;
            if !env.meets(&Envelope::of(&[t.a, t.b])) {
                continue;
            }
            let points = match hit(s.a, s.b, t.a, t.b) {
                Hit::Apart => continue,
                Hit::Cross(p) | Hit::Touch(p) => vec![p],
                Hit::Overlap(p, q) => vec![p, q],
            };
            found.push((i, points.clone()));
            found.push((j, points));
        }
    }
    for (i, points) in found {
        if let Some(s) = segments.get_mut(i) {
            s.cuts.extend(points);
        }
    }
    let mut points = Points::default();
    // Every point the rings reach is numbered first, so that a crossing found near one becomes it.
    for s in &segments {
        points.exact(s.a);
    }
    let mut edges: Vec<(usize, usize)> = Vec::new();
    let mut count: Vec<i64> = Vec::new();
    let mut by_ends: BTreeMap<(usize, usize), usize> = BTreeMap::new();
    for s in segments {
        meter.charge(u64::try_from(s.cuts.len() + 1).unwrap_or(u64::MAX))?;
        let mut along: Vec<usize> = vec![points.exact(s.a), points.exact(s.b)];
        along.extend(s.cuts.into_iter().map(|p| points.near(p)));
        along.sort_by(|&u, &v| points.distance(u, s.a).total_cmp(&points.distance(v, s.a)).then(u.cmp(&v)));
        along.dedup();
        // Sorted by distance and deduplicated, neighbours are distinct points.
        for pair in along.windows(2) {
            let (u, v) = (pair[0], pair[1]);
            let edge = *by_ends.entry((u.min(v), u.max(v))).or_insert_with(|| {
                edges.push((u, v));
                count.push(0);
                edges.len() - 1
            });
            let step = if edges.get(edge).is_some_and(|&(from, _)| from == u) { 1 } else { -1 };
            count.get_mut(edge).into_iter().for_each(|c| *c += step);
        }
    }
    Ok(Arrangement { points: points.list, edges, count })
}

/// The points found, numbered, with a lookup by position.
#[derive(Default)]
struct Points {
    list: Vec<Point>,
    /// Exact positions, as bits (with -0 as 0).
    exact: BTreeMap<(u64, u64), usize>,
    /// The points in each square of side [`SAME`], for crossings.
    cells: BTreeMap<(i64, i64), Vec<usize>>,
}

impl Points {
    fn key(p: Point) -> (u64, u64) {
        ((p.x() + 0.0).to_bits(), (p.y() + 0.0).to_bits())
    }

    #[allow(clippy::cast_possible_truncation)]
    fn cell(p: Point) -> (i64, i64) {
        ((p.x() / SAME).floor() as i64, (p.y() / SAME).floor() as i64)
    }

    /// The number of `p`, which the rings reach exactly.
    fn exact(&mut self, p: Point) -> usize {
        if let Some(&i) = self.exact.get(&Self::key(p)) {
            return i;
        }
        self.add(p)
    }

    /// The number of a point found within [`SAME`] of `p`, or of `p` added.
    fn near(&mut self, p: Point) -> usize {
        if let Some(&i) = self.exact.get(&Self::key(p)) {
            return i;
        }
        // A point within SAME lies in p's square or one of the 8 around it.
        let (cx, cy) = Self::cell(p);
        for x in cx.saturating_sub(1)..=cx.saturating_add(1) {
            for y in cy.saturating_sub(1)..=cy.saturating_add(1) {
                for &i in self.cells.get(&(x, y)).into_iter().flatten() {
                    if self.list.get(i).is_some_and(|q| (q.x() - p.x()).abs() <= SAME && (q.y() - p.y()).abs() <= SAME) {
                        return i;
                    }
                }
            }
        }
        self.add(p)
    }

    fn add(&mut self, p: Point) -> usize {
        let i = self.list.len();
        self.list.push(p);
        self.exact.insert(Self::key(p), i);
        self.cells.entry(Self::cell(p)).or_default().push(i);
        i
    }

    fn distance(&self, i: usize, from: Point) -> f64 {
        self.list.get(i).map_or(0.0, |p| from.distance(*p))
    }
}

/// The faces of an arrangement and their winding numbers.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Faces {
    /// Each face's winding number; the last is outside everything, 0.
    pub(crate) winding: Vec<i64>,
    /// For each half edge (half `2e` runs along edge `e` from its first point, half `2e + 1` back), the face
    /// on its right.
    pub(crate) right_of: Vec<usize>,
}

impl Arrangement {
    /// Where half edge `half` starts and ends.
    pub(crate) fn ends(&self, half: usize) -> Option<(usize, usize)> {
        let &(u, v) = self.edges.get(half / 2)?;
        Some(if half.is_multiple_of(2) { (u, v) } else { (v, u) })
    }

    /// The point numbered `i`.
    pub(crate) fn point(&self, i: usize) -> Point {
        self.points.get(i).copied().unwrap_or(Point::ORIGIN)
    }

    /// For each half edge, the next half edge counter-clockwise round its start, among the halves `keep`
    /// lets through.
    pub(crate) fn stars(&self, keep: impl Fn(usize) -> bool) -> Vec<usize> {
        let halves = 2 * self.edges.len();
        let mut at: Vec<Vec<usize>> = vec![Vec::new(); self.points.len()];
        for half in (0..halves).filter(|&h| keep(h)) {
            if let Some((from, _)) = self.ends(half)
                && let Some(list) = at.get_mut(from)
            {
                list.push(half);
            }
        }
        let mut next: Vec<usize> = (0..halves).collect();
        for (point, star) in at.iter_mut().enumerate() {
            let origin = self.point(point);
            let toward = |h: usize| self.ends(h).map_or(origin, |(_, to)| self.point(to));
            star.sort_by(|&a, &b| compare_angle(origin, toward(a), toward(b)).then(a.cmp(&b)));
            for (k, &h) in star.iter().enumerate() {
                if let (Some(slot), Some(&n)) = (next.get_mut(h), star.get((k + 1) % star.len())) {
                    *slot = n;
                }
            }
        }
        next
    }

    /// The faces and their winding numbers. One unit of `meter` per half edge and per pair of ring and
    /// face compared.
    pub(crate) fn faces(&self, meter: &mut Meter) -> Result<Faces, Exhausted> {
        let halves = 2 * self.edges.len();
        meter.charge(u64::try_from(halves).unwrap_or(u64::MAX))?;
        let next_ccw = self.stars(|_| true);
        // Trace the rings: after each half, the half leaving its end just counter-clockwise of the way back.
        let mut ring_of: Vec<Option<usize>> = vec![None; halves];
        let mut rings: Vec<Vec<usize>> = Vec::new();
        for start in 0..halves {
            if ring_of.get(start).copied().flatten().is_some() {
                continue;
            }
            let id = rings.len();
            let mut ring = Vec::new();
            let mut half = start;
            while let Some(slot) = ring_of.get_mut(half) {
                if slot.is_some() {
                    break;
                }
                *slot = Some(id);
                ring.push(half);
                half = next_ccw.get(half ^ 1).copied().unwrap_or(half);
            }
            rings.push(ring);
        }
        let polylines: Vec<Vec<Point>> = rings
            .iter()
            .map(|ring| closed(&ring.iter().filter_map(|&h| self.ends(h)).map(|(from, _)| self.point(from)).collect::<Vec<_>>()))
            .collect();
        let areas: Vec<f64> = polylines.iter().map(|ring| signed_area(ring)).collect();
        // Clockwise rings bound faces, numbered in order; the others lie in the smallest face around them.
        let mut face_of_ring: Vec<Option<usize>> = vec![None; rings.len()];
        let mut bounds: Vec<usize> = Vec::new();
        for (r, &area) in areas.iter().enumerate() {
            if area < 0.0 {
                face_of_ring[r] = Some(bounds.len());
                bounds.push(r);
            }
        }
        let outside = bounds.len();
        for (r, polyline) in polylines.iter().enumerate() {
            if face_of_ring.get(r).copied().flatten().is_some() {
                continue;
            }
            meter.charge(u64::try_from(bounds.len()).unwrap_or(u64::MAX))?;
            let probe = polyline.first().copied().unwrap_or(Point::ORIGIN);
            let env = Envelope::of(polyline);
            let around =
                |b: usize| polylines.get(b).is_some_and(|face| Envelope::of(face).covers(&env) && locate_in_ring(probe, face) == Location::Interior);
            let area = |b: usize| areas.get(b).copied().unwrap_or(f64::NEG_INFINITY);
            // Of the faces around it, the smallest: the one whose clockwise ring's area is nearest 0.
            let smallest = bounds.iter().enumerate().filter(|&(_, &b)| around(b)).max_by(|&(_, &a), &(_, &b)| area(a).total_cmp(&area(b)));
            face_of_ring[r] = Some(smallest.map_or(outside, |(f, _)| f));
        }
        let right_of: Vec<usize> = ring_of.iter().map(|r| r.and_then(|r| face_of_ring.get(r).copied().flatten()).unwrap_or(outside)).collect();
        // Winding numbers, from outside everything inwards: crossing a half from its right to its left adds
        // the count of rings running along it.
        let mut by_face: Vec<Vec<usize>> = vec![Vec::new(); outside + 1];
        for (half, &face) in right_of.iter().enumerate() {
            if let Some(list) = by_face.get_mut(face) {
                list.push(half);
            }
        }
        let mut winding: Vec<Option<i64>> = vec![None; outside + 1];
        if let Some(slot) = winding.get_mut(outside) {
            *slot = Some(0);
        }
        let mut queue = vec![outside];
        while let Some(face) = queue.pop() {
            let Some(w) = winding.get(face).copied().flatten() else { continue };
            for &half in by_face.get(face).into_iter().flatten() {
                let left = right_of.get(half ^ 1).copied().unwrap_or(outside);
                let count = self.count.get(half / 2).copied().unwrap_or(0);
                let step = if half.is_multiple_of(2) { count } else { -count };
                if let Some(slot) = winding.get_mut(left)
                    && slot.is_none()
                {
                    *slot = Some(w + step);
                    queue.push(left);
                }
            }
        }
        Ok(Faces { winding: winding.into_iter().map(|w| w.unwrap_or(0)).collect(), right_of })
    }
}

#[cfg(test)]
mod tests {
    use stitchcraft_core::Budget;

    use super::*;

    fn p(x: f64, y: f64) -> Point {
        Point::new(x, y).unwrap()
    }

    fn square(x: f64, y: f64, s: f64) -> Vec<Point> {
        vec![p(x, y), p(x + s, y), p(x + s, y + s), p(x, y + s)]
    }

    fn arrange(rings: &[Vec<Point>]) -> (Arrangement, Faces) {
        let mut meter = Budget::DEFAULT.meter();
        let arrangement = cut(rings, &mut meter).unwrap();
        let faces = arrangement.faces(&mut meter).unwrap();
        (arrangement, faces)
    }

    /// The winding numbers of the faces other than the outside, in order.
    fn windings(rings: &[Vec<Point>]) -> Vec<i64> {
        let (_, faces) = arrange(rings);
        faces.winding[..faces.winding.len() - 1].to_vec()
    }

    #[test]
    fn a_square_drawn_either_way_winds_once_round_its_inside() {
        assert_eq!(windings(&[square(0.0, 0.0, 4.0)]), [1], "counter-clockwise in y-up axes");
        let mut turned = square(0.0, 0.0, 4.0);
        turned.reverse();
        assert_eq!(windings(&[turned]), [-1]);
    }

    #[test]
    fn rings_inside_rings_add_up() {
        let mut hole = square(1.0, 1.0, 2.0);
        assert_eq!(windings(&[square(0.0, 0.0, 4.0), hole.clone()]), [1, 2], "the same way round");
        hole.reverse();
        assert_eq!(windings(&[square(0.0, 0.0, 4.0), hole]), [1, 0], "the other way round");
        assert_eq!(windings(&[square(0.0, 0.0, 4.0), square(10.0, 0.0, 4.0)]), [1, 1], "apart");
        // Drawn middle, outer, inner: each lies in the smallest face around it, whatever the order.
        assert_eq!(windings(&[square(1.0, 1.0, 6.0), square(0.0, 0.0, 8.0), square(2.0, 2.0, 4.0)]), [2, 1, 3]);
    }

    #[test]
    fn crossings_cut_the_rings() {
        // A bow tie: 2 faces, wound opposite ways, meeting at the crossing.
        let (arrangement, faces) = arrange(&[vec![p(0.0, 0.0), p(4.0, 4.0), p(4.0, 0.0), p(0.0, 4.0)]]);
        assert_eq!(arrangement.points.len(), 5);
        assert_eq!(arrangement.points[4], p(2.0, 2.0));
        let mut w = faces.winding[..faces.winding.len() - 1].to_vec();
        w.sort_unstable();
        assert_eq!(w, [-1, 1]);
        // Overlapping squares: the overlap winds twice.
        let mut w = windings(&[square(0.0, 0.0, 4.0), square(2.0, 2.0, 4.0)]);
        w.sort_unstable();
        assert_eq!(w, [1, 1, 2]);
    }

    #[test]
    fn stretches_run_along_by_several_rings_are_one_edge_that_counts_them() {
        // 2 squares sharing an edge, drawn the same way round: the shared edge is run along both ways.
        let (arrangement, faces) = arrange(&[square(0.0, 0.0, 4.0), square(4.0, 0.0, 4.0)]);
        assert_eq!(arrangement.edges.len(), 7);
        assert!(arrangement.count.contains(&0));
        assert_eq!(faces.winding[..faces.winding.len() - 1], [1, 1]);
        // The same square twice: one ring of edges counting 2.
        let (arrangement, faces) = arrange(&[square(0.0, 0.0, 4.0), square(0.0, 0.0, 4.0)]);
        assert_eq!(arrangement.count, [2, 2, 2, 2]);
        assert_eq!(faces.winding[..faces.winding.len() - 1], [2]);
    }

    #[test]
    fn crossings_found_near_a_point_already_found_are_that_point() {
        // 3 lines through one point, each pair crossing there: one point in the middle.
        let star = vec![p(-1.0, -1.0), p(1.0, 1.0), p(1.0, -1.0), p(-1.0, 1.0), p(0.3, -1.0), p(-0.3, 1.0)];
        let (arrangement, _) = arrange(&[star]);
        let middles = arrangement.points.iter().filter(|q| q.x().abs() < 1e-6 && q.y().abs() < 1e-6).count();
        assert_eq!(middles, 1);
        // 3 segments through (0.4, 0.7), whose crossings come out of floating point a hair apart.
        let ring = vec![p(-0.5, 2.0), p(1.3, -0.6), p(-1.2, -0.1), p(2.0, 1.5), p(-1.2, 0.3), p(2.0, 1.1)];
        let (arrangement, _) = arrange(&[ring]);
        let middles = arrangement.points.iter().filter(|q| (q.x() - 0.4).abs() < 1e-6 && (q.y() - 0.7).abs() < 1e-6).count();
        assert_eq!(middles, 1);
        // Within a billionth of a millimetre a point is the one found before it, in its own square or the
        // next one either way; farther in x or in y, a new one.
        let mut points = Points::default();
        let first = points.near(p(1.5e-9, 1.5e-9));
        for near in [p(1.6e-9, 1.4e-9), p(0.6e-9, 1.5e-9), p(2.4e-9, 1.5e-9), p(1.5e-9, 0.6e-9), p(1.5e-9, 2.4e-9)] {
            assert_eq!(points.near(near), first, "{near:?}");
        }
        assert_ne!(points.near(p(1.7e-9, 2.9e-9)), first);
        assert_ne!(points.near(p(2.9e-9, 1.7e-9)), first);
        assert_ne!(points.near(p(0.4, 0.7)), first);
    }

    #[test]
    fn rings_with_nothing_else_are_kept_out() {
        assert_eq!(windings(&[]), Vec::<i64>::new());
        let (arrangement, faces) = arrange(&[vec![p(0.0, 0.0), p(1.0, 0.0)]]);
        assert_eq!(arrangement.edges.len(), 1, "a line out and back");
        assert_eq!(faces.winding, [0], "bounds no face");
        assert_eq!(arrangement.ends(5), None);
        assert_eq!(arrangement.point(9), Point::ORIGIN);
    }

    #[test]
    fn the_budget_bounds_the_work() {
        let rings = [square(0.0, 0.0, 4.0), square(2.0, 2.0, 4.0)];
        for work in [3, 10, 25] {
            assert!(cut(&rings, &mut Budget { max_stitches: 1, max_work: work }.meter()).is_err(), "{work}");
        }
        let arrangement = cut(&rings, &mut Budget::DEFAULT.meter()).unwrap();
        assert!(arrangement.faces(&mut Budget { max_stitches: 1, max_work: 3 }.meter()).is_err());
        assert!(arrangement.faces(&mut Budget { max_stitches: 1, max_work: 24 }.meter()).is_err());
        // A square: 5 points with the first again, 5 pairs of segments side by side along x, and each of
        // its 4 segments with the 2 cuts its neighbours make at its ends.
        let mut meter = Budget::DEFAULT.meter();
        cut(&[square(0.0, 0.0, 4.0)], &mut meter).unwrap();
        assert_eq!(Budget::DEFAULT.max_work - meter.work_left(), 5 + 5 + 4 * 3);
    }

    #[test]
    fn points_are_numbered_once_whatever_the_sign_of_0() {
        assert_eq!(Points::key(p(-0.0, 1.0)), Points::key(p(0.0, 1.0)));
        assert_eq!(Points::key(p(1.0, -0.0)), Points::key(p(1.0, 0.0)));
        // Squares sharing the edge at x = 0, one drawn through -0: one edge between them, run both ways.
        let left = vec![p(-4.0, 0.0), p(-0.0, 0.0), p(-0.0, 4.0), p(-4.0, 4.0)];
        let (arrangement, faces) = arrange(&[left, square(0.0, 0.0, 4.0)]);
        assert_eq!((arrangement.points.len(), arrangement.edges.len()), (6, 7));
        assert_eq!(&faces.winding[..faces.winding.len() - 1], [1, 1]);
    }

    #[test]
    fn crossings_are_looked_up_in_squares_a_billionth_of_a_millimetre_across() {
        assert_eq!(Points::cell(p(0.0, 0.0)), (0, 0));
        assert_eq!(Points::cell(p(2.5e-9, -1.5e-9)), (2, -2));
        assert_eq!(Points::cell(p(-0.5e-9, 7.5e-9)), (-1, 7));
    }
}
