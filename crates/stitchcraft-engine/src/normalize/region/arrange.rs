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

use std::cell::OnceCell;

use stitchcraft_core::{Exhausted, Meter, Point};

use super::assemble::Unjoined;
use super::geom::{Envelope, Location, closed, closed_of, compare_angle, locate_in_ring};
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
    /// The halves leaving each point in order round it, once found ([`Arrangement::stars`]).
    star: OnceCell<Star>,
}

/// The halves leaving each point of an arrangement, side by side in order of the points: those of the point
/// `p` are `leaving[offsets[p]..offsets[p + 1]]`, counter-clockwise round it from the positive x axis, and
/// in order of their numbers where 2 leave the same way.
#[derive(Clone, Debug, Default, PartialEq)]
struct Star {
    offsets: Vec<usize>,
    leaving: Vec<usize>,
}

/// `rings` (each open or closed, in the direction drawn) cut where they meet, on the grid; `Unjoined`
/// should a point lie beyond the grid's reach or the edges not settle, a bug. A ring whose points all round
/// to one grid point is left out. One unit of `meter` per point and per edge, and what putting the edges on
/// the grid takes ([`snap::round`]).
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
    let Ok(snap::Rounded { paths, hot }) = snap::round(&on_grid, meter)? else { return Ok(Err(Unjoined)) };
    // The corners first, so that the rings of the parts start at a corner the drawing reaches first.
    // Paths that rounding didn't bend reach no point their corners don't, and are numbered as they are.
    // Every point is hot, so this return is a bug's too.
    let unbent = paths == on_grid;
    let reached: Vec<Node> =
        if unbent { on_grid.iter().flatten().copied().collect() } else { on_grid.iter().chain(&paths).flatten().copied().collect() };
    let Some((nodes, number)) = numbered(&reached, &hot) else { return Ok(Err(Unjoined)) };
    // The paths' points' numbers, in order: after the corners', or the corners' own. Each step from one
    // point to the next, in the order the paths run.
    let corners: usize = on_grid.iter().map(Vec::len).sum();
    let mut at = if unbent { 0 } else { corners };
    let mut steps: Vec<(usize, usize)> = Vec::with_capacity(paths.iter().map(Vec::len).sum());
    for path in &paths {
        meter.charge(u64::try_from(path.len()).unwrap_or(u64::MAX))?;
        let numbers = number.get(at..at + path.len()).unwrap_or_default();
        at += path.len();
        steps.extend(numbers.windows(2).filter_map(|pair| Some((*pair.first()?, *pair.get(1)?))));
    }
    let (edges, count) = edges(&steps, nodes.len());
    let points = nodes.iter().map(|node| node.point()).collect();
    Ok(Ok(Arrangement { points, nodes, edges, count, star: OnceCell::new() }))
}

/// The edges the paths' `steps` run along, between points numbered below `points`, and for each, the steps
/// along it less the steps back. Steps between the same 2 points, either way, are one edge, which runs the
/// way the first of them is taken; the edges are numbered in the order they are first taken. The steps are
/// grouped by their lower point, and a group is sorted by the higher point: a point has few steps, and those
/// to the same point then lie side by side.
fn edges(steps: &[(usize, usize)], points: usize) -> (Vec<(usize, usize)>, Vec<i64>) {
    let high = |s: &usize| steps.get(*s).map(|&(u, v)| u.max(v));
    let (offsets, mut grouped) = grouped(steps.iter().map(|&(u, v)| Some(u.min(v))), points);
    // For each step, the first step between the same 2 points: in a group sorted alike, the first of a run.
    let mut first: Vec<usize> = (0..steps.len()).collect();
    for span in offsets.windows(2) {
        let &[from, to] = span else { continue };
        let Some(group) = grouped.get_mut(from..to) else { continue };
        group.sort_by_key(high);
        let mut run = None;
        for &s in group.iter() {
            match run {
                Some((h, f)) if h == high(&s) => first.get_mut(s).into_iter().for_each(|slot| *slot = f),
                _ => run = Some((high(&s), s)),
            }
        }
    }
    let (mut edges, mut count) = (Vec::new(), Vec::new());
    let mut edge_of = vec![0; steps.len()];
    for (s, (&(u, v), &f)) in steps.iter().zip(&first).enumerate() {
        let edge = if f == s {
            edges.push((u, v));
            count.push(0);
            edges.len() - 1
        } else {
            edge_of.get(f).copied().unwrap_or(0)
        };
        edge_of.get_mut(s).into_iter().for_each(|slot| *slot = edge);
        let step = if edges.get(edge).is_some_and(|&(from, _)| from == u) { 1 } else { -1 };
        count.get_mut(edge).into_iter().for_each(|c| *c += step);
    }
    (edges, count)
}

/// The items, numbered in the order `keys` gives their keys, grouped by key with a counting sort: those
/// whose key is `k`, below `groups`, are `items[offsets[k]..offsets[k + 1]]`, in order. An item without a
/// key is left out.
fn grouped(keys: impl Iterator<Item = Option<usize>> + Clone, groups: usize) -> (Vec<usize>, Vec<usize>) {
    let mut offsets = vec![0; groups + 1];
    for key in keys.clone().flatten() {
        if let Some(slot) = key.checked_add(1).and_then(|next| offsets.get_mut(next)) {
            *slot += 1;
        }
    }
    for i in 1..offsets.len() {
        let before = offsets.get(i - 1).copied().unwrap_or(0);
        offsets.get_mut(i).into_iter().for_each(|slot| *slot += before);
    }
    let mut placed = offsets.clone();
    let mut items = vec![0; offsets.last().copied().unwrap_or(0)];
    for (item, key) in keys.enumerate() {
        if let Some(at) = key.and_then(|k| placed.get_mut(k))
            && let Some(slot) = items.get_mut(*at)
        {
            *slot = item;
            *at += 1;
        }
    }
    (offsets, items)
}

/// No number yet.
const NONE: usize = usize::MAX;

/// The distinct points of `reached`, in the order it first reaches them, and the number each of its points
/// has among them, each found in `hot`, every point reached in order, each once; `None` should one not be
/// there, a bug.
fn numbered(reached: &[Node], hot: &[Node]) -> Option<(Vec<Node>, Vec<usize>)> {
    let mut number_of = vec![NONE; hot.len()];
    let mut nodes = Vec::new();
    let mut number = Vec::with_capacity(reached.len());
    for &node in reached {
        let slot = number_of.get_mut(hot.binary_search(&node).ok()?)?;
        if *slot == NONE {
            *slot = nodes.len();
            nodes.push(node);
        }
        number.push(*slot);
    }
    Some((nodes, number))
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
        TwiceArea::along(ring.iter().filter_map(|&i| self.nodes.get(i).copied()))
    }

    /// For each half edge, the next half edge counter-clockwise round its start, among the halves `keep`
    /// lets through; a half it keeps out is its own. The faces and the parts each ask for these, with
    /// different halves kept, and the order round each point is found once for both: the order of the halves
    /// kept is the order of all of them, with the others left out.
    pub(crate) fn stars(&self, keep: impl Fn(usize) -> bool) -> Vec<usize> {
        let star = self.star.get_or_init(|| self.star());
        let mut next: Vec<usize> = (0..2 * self.edges.len()).collect();
        let mut kept = Vec::new();
        for span in star.offsets.windows(2) {
            let &[from, to] = span else { continue };
            kept.clear();
            kept.extend(star.leaving.get(from..to).unwrap_or_default().iter().copied().filter(|&h| keep(h)));
            for (k, &h) in kept.iter().enumerate() {
                if let (Some(slot), Some(&n)) = (next.get_mut(h), kept.get((k + 1) % kept.len())) {
                    *slot = n;
                }
            }
        }
        next
    }

    /// The halves leaving each point, in order round it ([`Star`]).
    fn star(&self) -> Star {
        let halves = 2 * self.edges.len();
        let (offsets, mut leaving) = grouped((0..halves).map(|h| self.ends(h).map(|(from, _)| from)), self.points.len());
        for (point, span) in offsets.windows(2).enumerate() {
            let &[from, to] = span else { continue };
            let Some(star) = leaving.get_mut(from..to) else { continue };
            let origin = self.point(point);
            let toward = |h: usize| self.ends(h).map_or(origin, |(_, to)| self.point(to));
            star.sort_by(|&a, &b| compare_angle(origin, toward(a), toward(b)).then(a.cmp(&b)));
        }
        Star { offsets, leaving }
    }

    /// The faces and their winding numbers. One unit of `meter` per half edge, per point of the rings
    /// traced (for their bounds), per pair of a ring bounding no face and a face compared, and per point of
    /// each face whose bounds cover such a ring and that the ring is then located in, the smallest first
    /// until one holds it.
    pub(crate) fn faces(&self, meter: &mut Meter) -> Result<Faces, Exhausted> {
        let halves = 2 * self.edges.len();
        meter.charge(u64::try_from(halves).unwrap_or(u64::MAX))?;
        let next_ccw = self.stars(|_| true);
        // Trace the rings: after each half, the half leaving its end just counter-clockwise of the way back.
        let mut ring_of: Vec<Option<usize>> = vec![None; halves];
        // Each ring as the points its halves start at, all of them in one list.
        let (mut points, mut bounds): (Vec<usize>, Vec<usize>) = (Vec::with_capacity(halves), vec![0]);
        for start in 0..halves {
            if ring_of.get(start).copied().flatten().is_some() {
                continue;
            }
            let id = bounds.len() - 1;
            let mut half = start;
            while let Some(slot) = ring_of.get_mut(half) {
                if slot.is_some() {
                    break;
                }
                *slot = Some(id);
                points.extend(self.ends(half).map(|(from, _)| from));
                half = next_ccw.get(half ^ 1).copied().unwrap_or(half);
            }
            bounds.push(points.len());
        }
        let starts: Vec<&[usize]> = bounds.windows(2).map(|span| span.first().zip(span.get(1)).and_then(|(&from, &to)| points.get(from..to)).unwrap_or_default()).collect();
        let polylines: Vec<Vec<Point>> = starts.iter().map(|ring| closed_of(ring.iter().map(|&i| self.point(i)), ring.len())).collect();
        let areas: Vec<TwiceArea> = starts.iter().map(|ring| self.twice_area(ring)).collect();
        // Each ring's bounds, once: a ring is compared with every face, and a face with every ring.
        let mut envelopes: Vec<Envelope> = Vec::with_capacity(polylines.len());
        for polyline in &polylines {
            meter.charge(u64::try_from(polyline.len()).unwrap_or(u64::MAX))?;
            envelopes.push(Envelope::of(polyline));
        }
        // Clockwise rings bound faces, numbered in order; the others lie in the smallest face around them.
        let mut face_of_ring: Vec<Option<usize>> = vec![None; starts.len()];
        let mut bounds: Vec<usize> = Vec::new();
        for (r, area) in areas.iter().enumerate() {
            if area.clockwise() {
                face_of_ring[r] = Some(bounds.len());
                bounds.push(r);
            }
        }
        let outside = bounds.len();
        for (r, (polyline, env)) in polylines.iter().zip(&envelopes).enumerate() {
            if face_of_ring.get(r).copied().flatten().is_some() {
                continue;
            }
            meter.charge(u64::try_from(bounds.len()).unwrap_or(u64::MAX))?;
            let probe = polyline.first().copied().unwrap_or(Point::ORIGIN);
            // The faces around it are those whose bounds cover its bounds and inside which its first point
            // lies, and it lies in the smallest: the one whose clockwise ring's area is nearest 0, and the
            // last of those as small. So the faces whose bounds cover its own are looked at in that order,
            // until one holds the point.
            let mut covering: Vec<(usize, usize)> =
                bounds.iter().enumerate().filter(|&(_, &b)| envelopes.get(b).is_some_and(|face| face.covers(env))).map(|(f, &b)| (f, b)).collect();
            covering.sort_by(|&(f, b), &(g, c)| areas.get(c).cmp(&areas.get(b)).then(g.cmp(&f)));
            let mut smallest = outside;
            for &(f, b) in &covering {
                let Some(face) = polylines.get(b) else { continue };
                meter.charge(u64::try_from(face.len()).unwrap_or(u64::MAX))?;
                if locate_in_ring(probe, face) == Location::Interior {
                    smallest = f;
                    break;
                }
            }
            face_of_ring[r] = Some(smallest);
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
        // A square: 5 points with the first again; its 4 corners put in 2 buckets each way, 1 in each; its
        // 4 edges, each looking in 2 buckets and tested against the corner in each; the edges put in the
        // same 2 buckets each, and the 2 that share each bucket tested; and the 5 points of the ring cut.
        let mut meter = Budget::DEFAULT.meter();
        cut(&[square(0.0, 0.0, 4.0)], &mut meter).unwrap().unwrap();
        assert_eq!(Budget::DEFAULT.max_work - meter.work_left(), 5 + 4 + (4 + 4 * (2 + 2)) + (4 * 2 + 4) + 5);
        // A square with a square hole apart from it: 16 halves; 4 rings traced, of 5 points each with the
        // first again, for their bounds; 2 rings bounding no face, each compared with the 2 faces; and the
        // faces whose bounds cover them located in, the square's for both and the hole's for the hole.
        let mut hole = square(1.0, 1.0, 2.0);
        hole.reverse();
        let arrangement = cut(&[square(0.0, 0.0, 4.0), hole], &mut Budget::DEFAULT.meter()).unwrap().unwrap();
        let mut meter = Budget::DEFAULT.meter();
        arrangement.faces(&mut meter).unwrap();
        assert_eq!(Budget::DEFAULT.max_work - meter.work_left(), 16 + 4 * 5 + 2 * 2 + 3 * 5);
    }

    #[test]
    fn points_are_numbered_in_the_order_they_are_first_reached() {
        let [a, b, c] = [p(0.0, 0.0), p(2.0, 0.0), p(-2.0, 4.0)].map(|q| Node::of(q).unwrap());
        let hot = [c, a, b];
        assert_eq!(numbered(&[c, a, c, b, a], &hot), Some((vec![c, a, b], vec![0, 1, 0, 2, 1])));
        assert_eq!(numbered(&[], &hot), Some((vec![], vec![])));
        assert_eq!(numbered(&[a, b], &[a]), None, "a point that is not hot is a bug");
        // Grouped by key, each group in order, those without a key left out.
        let (offsets, items) = grouped([Some(2), None, Some(0), Some(2), Some(9)].into_iter(), 3);
        assert_eq!((offsets, items), (vec![0, 1, 1, 3], vec![2, 0, 3]));
        // Steps between the same 2 points are one edge, whichever way they run: the first taken gives its
        // direction (`cut`).
        let (arrangement, _) = arrange(&[vec![p(0.0, 0.0), p(2.0, 0.0), p(2.0, 2.0)], vec![p(2.0, 0.0), p(0.0, 0.0), p(0.0, -2.0)]]);
        assert_eq!(arrangement.edges, [(0, 1), (1, 2), (2, 0), (0, 3), (3, 1)]);
        assert_eq!(arrangement.count, [0, 1, 1, 1, 1], "the shared edge run both ways counts 0");
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
