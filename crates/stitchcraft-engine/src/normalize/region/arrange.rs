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
//! The rings are cut on a fine grid ([`super::snap`]), so that their edges meet only at their ends however
//! rounding left them, and every point is a grid point. Which way a ring turns, and which of 2 rings is the
//! smaller, is then worked out exactly, however thin the ring, and sides are decided exactly too
//! ([`stitchcraft_core::exact::side`]).

use std::collections::BTreeMap;

use stitchcraft_core::{Exhausted, Meter, Point};

use super::assemble::Unjoined;
use super::geom::{Envelope, Location, closed, compare_angle, locate_in_ring};
use super::snap::{self, Node, TwiceArea};

/// The rings cut where they meet: points, and edges between them.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Arrangement {
    /// The points, numbered: first the rings' corners in the order the rings reach them, then the other
    /// points where the rings are cut, in the order the rings reach those.
    pub(crate) points: Vec<Point>,
    /// The points as grid points, in the same order.
    nodes: Vec<Node>,
    /// The edges, each from one point to another: indices into `points`.
    pub(crate) edges: Vec<(usize, usize)>,
    /// For each edge, the rings that run along it from its first point to its second, less those that run
    /// the other way.
    pub(crate) count: Vec<i64>,
}

/// `rings` (each open or closed, in the direction drawn) cut where they meet, on the grid; `Unjoined`
/// should a point lie beyond the grid's reach or the edges not settle, a bug. A ring whose points all round
/// to one grid point is left out. One unit of `meter` per point, per edge bent and hot point tested against
/// it, per pair of edges whose spans along x overlap, and per edge.
pub(crate) fn cut(rings: &[Vec<Point>], meter: &mut Meter) -> Result<Result<Arrangement, Unjoined>, Exhausted> {
    let mut on_grid: Vec<Vec<Node>> = Vec::new();
    for ring in rings {
        let ring = closed(ring);
        meter.charge(u64::try_from(ring.len()).unwrap_or(u64::MAX))?;
        let Some(mut nodes) = ring.iter().map(|&p| Node::of(p)).collect::<Option<Vec<Node>>>() else { return Ok(Err(Unjoined)) };
        nodes.dedup();
        if nodes.len() > 1 {
            on_grid.push(nodes);
        }
    }
    // Rounding settles every drawing tried, so this return is a bug's. It stays on one line: coverage
    // counts a line that no test can reach as untested.
    let Ok(paths) = snap::round(&on_grid, meter)? else { return Ok(Err(Unjoined)) };
    let mut numbers: BTreeMap<Node, usize> = BTreeMap::new();
    let mut nodes: Vec<Node> = Vec::new();
    // The corners first, so that the rings of the parts start at a corner the drawing reaches first.
    for &node in on_grid.iter().chain(&paths).flatten() {
        numbers.entry(node).or_insert_with(|| {
            nodes.push(node);
            nodes.len() - 1
        });
    }
    let number = |node: &Node| numbers.get(node).copied().unwrap_or(0);
    let mut edges: Vec<(usize, usize)> = Vec::new();
    let mut count: Vec<i64> = Vec::new();
    let mut by_ends: BTreeMap<(usize, usize), usize> = BTreeMap::new();
    for path in &paths {
        meter.charge(u64::try_from(path.len()).unwrap_or(u64::MAX))?;
        for pair in path.windows(2) {
            let (u, v) = (number(&pair[0]), number(&pair[1]));
            let edge = *by_ends.entry((u.min(v), u.max(v))).or_insert_with(|| {
                edges.push((u, v));
                count.push(0);
                edges.len() - 1
            });
            let step = if edges.get(edge).is_some_and(|&(from, _)| from == u) { 1 } else { -1 };
            count.get_mut(edge).into_iter().for_each(|c| *c += step);
        }
    }
    let points = nodes.iter().map(|node| node.point()).collect();
    Ok(Ok(Arrangement { points, nodes, edges, count }))
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

    /// Twice the area of the closed ring through the points numbered in `ring`, exactly.
    pub(crate) fn twice_area(&self, ring: &[usize]) -> TwiceArea {
        TwiceArea::of(&ring.iter().filter_map(|&i| self.nodes.get(i).copied()).collect::<Vec<_>>())
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
        let starts: Vec<Vec<usize>> = rings.iter().map(|ring| ring.iter().filter_map(|&h| self.ends(h)).map(|(from, _)| from).collect()).collect();
        let polylines: Vec<Vec<Point>> = starts.iter().map(|ring| closed(&ring.iter().map(|&i| self.point(i)).collect::<Vec<_>>())).collect();
        let areas: Vec<TwiceArea> = starts.iter().map(|ring| self.twice_area(ring)).collect();
        // Clockwise rings bound faces, numbered in order; the others lie in the smallest face around them.
        let mut face_of_ring: Vec<Option<usize>> = vec![None; rings.len()];
        let mut bounds: Vec<usize> = Vec::new();
        for (r, area) in areas.iter().enumerate() {
            if area.clockwise() {
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
            // Of the faces around it, the smallest: the one whose clockwise ring's area is nearest 0.
            let smallest = bounds.iter().enumerate().filter(|&(_, &b)| around(b)).max_by_key(|&(_, &b)| areas.get(b).copied());
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
        let arrangement = cut(rings, &mut meter).unwrap().unwrap();
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
    fn crossings_at_one_point_are_one_point() {
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
        let arrangement = cut(&rings, &mut Budget::DEFAULT.meter()).unwrap().unwrap();
        assert!(arrangement.faces(&mut Budget { max_stitches: 1, max_work: 3 }.meter()).is_err());
        assert!(arrangement.faces(&mut Budget { max_stitches: 1, max_work: 24 }.meter()).is_err());
        // A square: 5 points with the first again; its 4 edges, the 2 along x each tested against the 4
        // corners within its span along x and the 2 upright ones against the 2 on their line; 5 pairs of
        // edges side by side along x; and the 5 points of the ring cut.
        let mut meter = Budget::DEFAULT.meter();
        cut(&[square(0.0, 0.0, 4.0)], &mut meter).unwrap().unwrap();
        assert_eq!(Budget::DEFAULT.max_work - meter.work_left(), 5 + (4 + 2 * 4 + 2 * 2) + 5 + 5);
    }

    #[test]
    fn points_are_numbered_once_whatever_the_sign_of_0() {
        // Squares sharing the edge at x = 0, one drawn through -0: one edge between them, run both ways.
        let left = vec![p(-4.0, 0.0), p(-0.0, 0.0), p(-0.0, 4.0), p(-4.0, 4.0)];
        let (arrangement, faces) = arrange(&[left, square(0.0, 0.0, 4.0)]);
        assert_eq!((arrangement.points.len(), arrangement.edges.len()), (6, 7));
        assert_eq!(&faces.winding[..faces.winding.len() - 1], [1, 1]);
    }
}
