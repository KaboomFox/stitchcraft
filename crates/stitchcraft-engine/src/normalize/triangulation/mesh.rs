//! The mesh a triangulation is built in: triangles that know the triangles across their edges, grown a
//! point at a time and kept Delaunay by flipping edges, then made to hold the part's edges.
//!
//! Each triangle lists its corners counter-clockwise in y-up axes, and edge `k` is the one opposite corner
//! `k`. Every question about where a point lies is asked of [`exact`], so that a point on an edge is on it,
//! and 4 points on a circle are on it: flipping stops, and a search for a point ends.

use std::collections::VecDeque;

use stitchcraft_core::exact::{self, Circle, Side};
use stitchcraft_core::{Exhausted, Meter, Point};

/// Why the mesh could not be built.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Failed {
    /// It needed more work than the budget allows.
    Work(Exhausted),
    /// It lost its shape, a bug: a triangle it points to is missing, a search found nothing, or an edge of
    /// the part crosses another.
    Broken,
}

impl From<Exhausted> for Failed {
    fn from(exhausted: Exhausted) -> Self {
        Failed::Work(exhausted)
    }
}

/// What a step of building gives.
pub(super) type Built<T> = Result<T, Failed>;

/// A triangle of the mesh.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Tri {
    /// Its corners, counter-clockwise, as indices of [`Mesh::points`].
    pub(super) v: [usize; 3],
    /// The triangle across each edge: none across the box's edges.
    pub(super) n: [Option<usize>; 3],
    /// Whether each edge is an edge of the part, which is never flipped.
    pub(super) fixed: [bool; 3],
}

impl Tri {
    /// Corner `k`, counted round from corner 0.
    pub(super) fn corner(&self, k: usize) -> usize {
        self.v[k % 3]
    }

    /// The triangle across edge `k`.
    pub(super) fn across(&self, k: usize) -> Option<usize> {
        self.n[k % 3]
    }

    /// Whether edge `k` is an edge of the part.
    pub(super) fn is_fixed(&self, k: usize) -> bool {
        self.fixed[k % 3]
    }

    /// Which corner point `p` is, if it is one.
    fn place(&self, p: usize) -> Option<usize> {
        self.v.iter().position(|&v| v == p)
    }

    /// Which edge lies against triangle `t`.
    fn edge_to(&self, t: usize) -> Option<usize> {
        self.n.iter().position(|&n| n == Some(t))
    }
}

/// Where a point lies in the mesh.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Found {
    /// Inside triangle `t`.
    Inside(usize),
    /// On edge `k` of triangle `t`, between its ends.
    Edge(usize, usize),
    /// At an existing point.
    Point(usize),
}

/// How the segment of an edge of the part leaves a point.
enum Leaving {
    /// Through the edges listed, each as its 2 ends.
    Crossing(VecDeque<(usize, usize)>),
    /// Through a point that lies on it, between its ends.
    Through(usize),
}

/// The steps of a walk round a point or across the mesh, each a unit of work. A walk that takes more steps
/// than it can, more than there are triangles for a search, has gone round in a circle: the mesh lost its
/// shape, a bug.
struct Walk {
    /// The steps it has left.
    left: usize,
}

impl Walk {
    /// A walk of no more than `steps` steps.
    fn of(steps: usize) -> Walk {
        Walk { left: steps }
    }

    /// The next step, with a unit of `meter`'s work: `Broken` past the last.
    fn step(&mut self, meter: &mut Meter) -> Built<()> {
        meter.charge(1)?;
        self.left = self.left.checked_sub(1).ok_or(Failed::Broken)?;
        Ok(())
    }
}

/// A triangulation as it is built.
#[derive(Clone, Debug)]
pub(super) struct Mesh {
    /// Its points: a box's 4 corners, then the points inserted.
    pub(super) points: Vec<Point>,
    /// Its triangles.
    pub(super) tris: Vec<Tri>,
    /// A triangle at each point.
    at: Vec<usize>,
    /// The triangle the last search for a point ended in, where the next one starts.
    last: usize,
}

impl Mesh {
    /// A mesh of 2 triangles: a box round `low` to `high`, a box's size from them on every side, its
    /// corners on whole millimetres. `None` for a box beyond what a [`Point`] holds.
    pub(super) fn boxed(low: Point, high: Point) -> Option<Mesh> {
        let room = (high.x() - low.x()).max(high.y() - low.y()) + 1.0;
        let (x0, y0) = ((low.x() - room).floor(), (low.y() - room).floor());
        let (x1, y1) = ((high.x() + room).ceil(), (high.y() + room).ceil());
        let points = vec![Point::new(x0, y0).ok()?, Point::new(x1, y0).ok()?, Point::new(x1, y1).ok()?, Point::new(x0, y1).ok()?];
        let tris = vec![
            Tri { v: [0, 1, 2], n: [None, Some(1), None], fixed: [false; 3] },
            Tri { v: [0, 2, 3], n: [None, None, Some(0)], fixed: [false; 3] },
        ];
        Some(Mesh { points, tris, at: vec![0, 0, 0, 1], last: 0 })
    }

    /// Triangle `t`.
    pub(super) fn tri(&self, t: usize) -> Built<Tri> {
        self.tris.get(t).copied().ok_or(Failed::Broken)
    }

    /// Point `p`.
    pub(super) fn point(&self, p: usize) -> Built<Point> {
        self.points.get(p).copied().ok_or(Failed::Broken)
    }

    /// Sets triangle `t`.
    fn set(&mut self, t: usize, tri: Tri) -> Built<()> {
        *self.tris.get_mut(t).ok_or(Failed::Broken)? = tri;
        for p in tri.v {
            *self.at.get_mut(p).ok_or(Failed::Broken)? = t;
        }
        Ok(())
    }

    /// Makes triangle `t`'s edge that lay against triangle `from` lie against triangle `to`.
    fn repoint(&mut self, t: Option<usize>, from: usize, to: usize) -> Built<()> {
        let Some(t) = t else { return Ok(()) };
        let tri = self.tris.get_mut(t).ok_or(Failed::Broken)?;
        let k = tri.edge_to(from).ok_or(Failed::Broken)?;
        tri.n[k] = Some(to);
        Ok(())
    }

    /// Adds `p` and gives its index, or the index of the point already there. The edges round it are
    /// flipped until the mesh is Delaunay again (Lawson's method), the part's edges excepted. A unit of work
    /// for each step of the search and each edge tested.
    pub(super) fn insert(&mut self, p: Point, meter: &mut Meter) -> Built<usize> {
        let index = self.points.len();
        let opposite = match self.locate(p, meter)? {
            Found::Point(q) => return Ok(q),
            Found::Inside(t) => {
                self.points.push(p);
                self.at.push(t);
                self.split(t, index)?
            }
            Found::Edge(t, k) => {
                self.points.push(p);
                self.at.push(t);
                self.split_edge(t, k, index)?
            }
        };
        // The edges opposite the new point, each as a triangle and the new point's corner in it.
        let mut stack = opposite;
        while let Some((t, k)) = stack.pop() {
            meter.charge(1)?;
            if self.not_delaunay(t, k)? {
                let u = self.flip(t, k)?;
                // The new point is corner 0 of both, facing the 2 edges that were the other triangle's.
                stack.extend([(t, 0), (u, 0)]);
            }
        }
        Ok(index)
    }

    /// Where `p` lies: found by walking from the last triangle found towards it, across the first edge it
    /// lies beyond. In a Delaunay mesh such a walk never comes back to a triangle it left, so it takes no
    /// more steps than there are triangles.
    fn locate(&mut self, p: Point, meter: &mut Meter) -> Built<Found> {
        let (mut t, mut walk) = (self.last, self.walk());
        loop {
            walk.step(meter)?;
            let tri = self.tri(t)?;
            let mut on = Vec::with_capacity(2);
            let mut beyond = None;
            for k in 0..3 {
                match exact::side(self.point(tri.corner(k + 1))?, self.point(tri.corner(k + 2))?, p) {
                    Side::Right => {
                        beyond = Some(k);
                        break;
                    }
                    Side::On => on.push(k),
                    Side::Left => {}
                }
            }
            if let Some(k) = beyond {
                t = tri.across(k).ok_or(Failed::Broken)?;
                continue;
            }
            self.last = t;
            return match on.as_slice() {
                [] => Ok(Found::Inside(t)),
                [k] => Ok(Found::Edge(t, *k)),
                // On 2 edges: at the corner they share, the one opposite neither.
                [a, b] => Ok(Found::Point(tri.corner(3 - a - b))),
                _ => Err(Failed::Broken),
            };
        }
    }

    /// A walk of a search through the mesh: no more steps than there are triangles, and one to start.
    fn walk(&self) -> Walk {
        Walk::of(self.tris.len() + 1)
    }

    /// Splits triangle `t` round the new point `p` inside it into 3, and gives each with `p`'s corner.
    fn split(&mut self, t: usize, p: usize) -> Built<Vec<(usize, usize)>> {
        let tri = self.tri(t)?;
        let [a, b, c] = tri.v;
        let (t1, t2) = (self.tris.len(), self.tris.len() + 1);
        self.tris.extend([tri, tri]);
        self.set(t, Tri { v: [a, b, p], n: [Some(t1), Some(t2), tri.n[2]], fixed: [false, false, tri.fixed[2]] })?;
        self.set(t1, Tri { v: [b, c, p], n: [Some(t2), Some(t), tri.n[0]], fixed: [false, false, tri.fixed[0]] })?;
        self.set(t2, Tri { v: [c, a, p], n: [Some(t), Some(t1), tri.n[1]], fixed: [false, false, tri.fixed[1]] })?;
        self.repoint(tri.n[0], t, t1)?;
        self.repoint(tri.n[1], t, t2)?;
        Ok(vec![(t, 2), (t1, 2), (t2, 2)])
    }

    /// Splits the 2 triangles beside edge `k` of triangle `t` round the new point `p` on it into 4, and
    /// gives each with `p`'s corner. A fixed edge stays fixed in its halves.
    fn split_edge(&mut self, t: usize, k: usize, p: usize) -> Built<Vec<(usize, usize)>> {
        let tri = self.tri(t)?;
        let u = tri.across(k).ok_or(Failed::Broken)?;
        let other = self.tri(u)?;
        let j = other.edge_to(t).ok_or(Failed::Broken)?;
        let (a, b, c, d) = (tri.corner(k), tri.corner(k + 1), tri.corner(k + 2), other.corner(j));
        let fixed = tri.is_fixed(k);
        let (t1, u1) = (self.tris.len(), self.tris.len() + 1);
        self.tris.extend([tri, tri]);
        // (a, b, p) and (a, p, c) where t was; (d, c, p) and (d, p, b) where u was.
        let ab = (tri.across(k + 2), tri.is_fixed(k + 2));
        let ca = (tri.across(k + 1), tri.is_fixed(k + 1));
        let bd = (other.across(j + 1), other.is_fixed(j + 1));
        let dc = (other.across(j + 2), other.is_fixed(j + 2));
        self.set(t, Tri { v: [a, b, p], n: [Some(u1), Some(t1), ab.0], fixed: [fixed, false, ab.1] })?;
        self.set(t1, Tri { v: [a, p, c], n: [Some(u), ca.0, Some(t)], fixed: [fixed, ca.1, false] })?;
        self.set(u, Tri { v: [d, c, p], n: [Some(t1), Some(u1), dc.0], fixed: [fixed, false, dc.1] })?;
        self.set(u1, Tri { v: [d, p, b], n: [Some(t), bd.0, Some(u)], fixed: [fixed, bd.1, false] })?;
        self.repoint(ca.0, t, t1)?;
        self.repoint(bd.0, u, u1)?;
        Ok(vec![(t, 2), (t1, 1), (u, 2), (u1, 1)])
    }

    /// Whether edge `k` of triangle `t` fails the Delaunay test and may be flipped: the point across it lies
    /// inside the circle through `t`'s corners, the edge is no edge of the part, and the 2 triangles beside
    /// it make a convex quadrilateral, as they always do when the point lies inside.
    fn not_delaunay(&self, t: usize, k: usize) -> Built<bool> {
        let tri = self.tri(t)?;
        let Some(u) = tri.across(k) else { return Ok(false) };
        if tri.is_fixed(k) {
            return Ok(false);
        }
        let other = self.tri(u)?;
        let d = other.corner(other.edge_to(t).ok_or(Failed::Broken)?);
        let [a, b, c] = [tri.corner(0), tri.corner(1), tri.corner(2)].map(|q| self.point(q));
        if exact::incircle(a?, b?, c?, self.point(d)?) != Circle::Inside {
            return Ok(false);
        }
        self.convex(t, k)
    }

    /// Whether the 2 triangles beside edge `k` of triangle `t` make a strictly convex quadrilateral, so that
    /// the edge can be flipped: the other diagonal crosses it between its ends.
    fn convex(&self, t: usize, k: usize) -> Built<bool> {
        let tri = self.tri(t)?;
        let other = self.tri(tri.across(k).ok_or(Failed::Broken)?)?;
        let d = other.corner(other.edge_to(t).ok_or(Failed::Broken)?);
        let (a, d) = (self.point(tri.corner(k))?, self.point(d)?);
        let (b, c) = (self.point(tri.corner(k + 1))?, self.point(tri.corner(k + 2))?);
        Ok(matches!((exact::side(a, d, b), exact::side(a, d, c)), (Side::Right, Side::Left) | (Side::Left, Side::Right)))
    }

    /// Flips edge `k` of triangle `t`: the triangles (a, b, c) and (d, c, b) beside it become (a, b, d), where
    /// `t` was, and (a, d, c), where the other was, whose index is given. Corner a is corner 0 of both.
    pub(super) fn flip(&mut self, t: usize, k: usize) -> Built<usize> {
        let tri = self.tri(t)?;
        let u = tri.across(k).ok_or(Failed::Broken)?;
        let other = self.tri(u)?;
        let j = other.edge_to(t).ok_or(Failed::Broken)?;
        let (a, b, c, d) = (tri.corner(k), tri.corner(k + 1), tri.corner(k + 2), other.corner(j));
        let (ca, ab) = ((tri.across(k + 1), tri.is_fixed(k + 1)), (tri.across(k + 2), tri.is_fixed(k + 2)));
        let (bd, dc) = ((other.across(j + 1), other.is_fixed(j + 1)), (other.across(j + 2), other.is_fixed(j + 2)));
        self.set(t, Tri { v: [a, b, d], n: [bd.0, Some(u), ab.0], fixed: [bd.1, false, ab.1] })?;
        self.set(u, Tri { v: [a, d, c], n: [dc.0, ca.0, Some(t)], fixed: [dc.1, ca.1, false] })?;
        self.repoint(bd.0, u, t)?;
        self.repoint(ca.0, t, u)?;
        Ok(u)
    }

    /// A triangle with the edge from `p` to `q`, and the corner opposite it; `None` when there is no such
    /// edge. Found by turning round `p`, a unit of work a triangle: counter-clockwise from the triangle at
    /// `p`, and where the triangles round `p` don't close, as round a corner of the box, clockwise from it too.
    pub(super) fn edge(&self, p: usize, q: usize, meter: &mut Meter) -> Built<Option<(usize, usize)>> {
        let start = *self.at.get(p).ok_or(Failed::Broken)?;
        let (mut t, mut clockwise, mut walk) = (start, false, self.walk());
        loop {
            walk.step(meter)?;
            let tri = self.tri(t)?;
            let k = tri.place(p).ok_or(Failed::Broken)?;
            if tri.corner(k + 1) == q {
                return Ok(Some((t, (k + 2) % 3)));
            }
            if tri.corner(k + 2) == q {
                return Ok(Some((t, (k + 1) % 3)));
            }
            // On to the next triangle round p: counter-clockwise across the edge from p to corner k + 2, and
            // clockwise across the edge from p to corner k + 1.
            match tri.across(if clockwise { k + 2 } else { k + 1 }) {
                Some(next) if next != start => t = next,
                None if !clockwise => (t, clockwise) = (start, true),
                _ => return Ok(None),
            }
        }
    }

    /// Makes the edge from `p` to `q` an edge of the mesh that is never flipped, with `meter`'s work. The
    /// edges it crosses are flipped until none does (Sloan's method); a point on it between its ends cuts
    /// it in 2. An edge it crosses that is already fixed is a bug.
    pub(super) fn constrain(&mut self, p: usize, q: usize, meter: &mut Meter) -> Built<()> {
        let mut pending = vec![(p, q)];
        while let Some((p, q)) = pending.pop() {
            meter.charge(1)?;
            if p == q {
                continue;
            }
            if self.edge(p, q, meter)?.is_none() {
                match self.leaving(p, q, meter)? {
                    Leaving::Through(r) => {
                        pending.extend([(r, q), (p, r)]);
                        continue;
                    }
                    Leaving::Crossing(crossing) => self.uncross(p, q, crossing, meter)?,
                }
            }
            let (t, k) = self.edge(p, q, meter)?.ok_or(Failed::Broken)?;
            // An edge of the part lies inside the box, with a triangle on each side.
            let tri = self.tris.get_mut(t).ok_or(Failed::Broken)?;
            tri.fixed[k] = true;
            let u = tri.n[k].ok_or(Failed::Broken)?;
            let other = self.tris.get_mut(u).ok_or(Failed::Broken)?;
            let j = other.edge_to(t).ok_or(Failed::Broken)?;
            other.fixed[j] = true;
        }
        Ok(())
    }

    /// How the segment from `p` to `q` leaves `p`: the edges it crosses on its way to `q`, or the first
    /// point between them that it passes through.
    fn leaving(&self, p: usize, q: usize, meter: &mut Meter) -> Built<Leaving> {
        let (from, to) = (self.point(p)?, self.point(q)?);
        // Ahead: a point on the segment's line, on the side of p that q is on.
        let ahead = |r: Point| (r.x() - from.x()) * (to.x() - from.x()) + (r.y() - from.y()) * (to.y() - from.y()) > 0.0;
        // The triangle round p whose corner the segment leaves through.
        let start = *self.at.get(p).ok_or(Failed::Broken)?;
        let (mut t, mut walk) = (start, self.walk());
        let (mut k, mut x, mut y) = loop {
            walk.step(meter)?;
            let tri = self.tri(t)?;
            let k = tri.place(p).ok_or(Failed::Broken)?;
            let (x, y) = (tri.corner(k + 1), tri.corner(k + 2));
            let (px, py) = (self.point(x)?, self.point(y)?);
            match (exact::side(from, to, px), exact::side(from, to, py)) {
                (Side::On, _) if ahead(px) => return Ok(Leaving::Through(x)),
                (_, Side::On) if ahead(py) => return Ok(Leaving::Through(y)),
                (Side::Right, Side::Left) => break (k, x, y),
                _ => {}
            }
            t = tri.across(k + 1).filter(|&next| next != start).ok_or(Failed::Broken)?;
        };
        // Walk on through the triangles it crosses, x on its right and y on its left.
        let (mut crossing, mut walk) = (VecDeque::new(), self.walk());
        loop {
            walk.step(meter)?;
            crossing.push_back((x, y));
            let tri = self.tri(t)?;
            let next = tri.across(k).ok_or(Failed::Broken)?;
            let beyond = self.tri(next)?;
            let j = beyond.edge_to(t).ok_or(Failed::Broken)?;
            let z = beyond.corner(j);
            if z == q {
                return Ok(Leaving::Crossing(crossing));
            }
            let pz = self.point(z)?;
            // The segment leaves by the edge between z and the corner on the other side of its line. Listed
            // counter-clockwise from z, `beyond`'s corners are z, y, x, so the edge from z to y lies opposite x,
            // corner j + 2, and the edge from x to z opposite y, corner j + 1.
            (t, k, x, y) = match exact::side(from, to, pz) {
                Side::On => return Ok(Leaving::Through(z)),
                Side::Right => (next, (j + 2) % 3, z, y),
                Side::Left => (next, (j + 1) % 3, x, z),
            };
        }
    }

    /// Flips the edges `crossing`, which the segment from `p` to `q` crosses, until none crosses it: an edge
    /// whose quadrilateral is not convex waits its turn again, and an edge a flip leaves crossing the
    /// segment is flipped again in its turn. Sloan (1993) shows that this ends; a run longer than it can
    /// take is a bug.
    fn uncross(&mut self, p: usize, q: usize, mut crossing: VecDeque<(usize, usize)>, meter: &mut Meter) -> Built<()> {
        let (from, to) = (self.point(p)?, self.point(q)?);
        let mut walk = Walk::of((crossing.len() + 1).saturating_mul(crossing.len() + 1).saturating_mul(4));
        loop {
            walk.step(meter)?;
            let Some((x, y)) = crossing.pop_front() else { return Ok(()) };
            let (t, k) = self.edge(x, y, meter)?.ok_or(Failed::Broken)?;
            if self.tri(t)?.is_fixed(k) {
                return Err(Failed::Broken);
            }
            if !self.convex(t, k)? {
                crossing.push_back((x, y));
                continue;
            }
            let u = self.flip(t, k)?;
            // The new edge runs from corner 0 to corner 1 of `u`: from a to d.
            let other = self.tri(u)?;
            let (a, d) = (other.corner(0), other.corner(1));
            let (pa, pd) = (self.point(a)?, self.point(d)?);
            if crosses(from, to, pa, pd) {
                crossing.push_back((a, d));
            }
        }
    }

    /// Whether each triangle lies inside the part: the triangles round the box's corners lie outside, and
    /// crossing a fixed edge goes in or out. A unit of work for each edge crossed; an edge that would make a
    /// triangle both, because the part's edges don't close, is a bug.
    pub(super) fn inside(&self, meter: &mut Meter) -> Built<Vec<bool>> {
        let mut inside: Vec<Option<bool>> = vec![None; self.tris.len()];
        let start = *self.at.first().ok_or(Failed::Broken)?;
        *inside.get_mut(start).ok_or(Failed::Broken)? = Some(false);
        let mut queue = VecDeque::from([start]);
        while let Some(t) = queue.pop_front() {
            let tri = self.tri(t)?;
            let here = inside.get(t).copied().flatten().ok_or(Failed::Broken)?;
            for k in 0..3 {
                meter.charge(1)?;
                let Some(u) = tri.across(k) else { continue };
                let there = here != tri.is_fixed(k);
                match inside.get_mut(u).ok_or(Failed::Broken)? {
                    Some(known) if *known != there => return Err(Failed::Broken),
                    Some(_) => {}
                    slot @ None => {
                        *slot = Some(there);
                        queue.push_back(u);
                    }
                }
            }
        }
        inside.into_iter().map(|known| known.ok_or(Failed::Broken)).collect()
    }

    /// Flips the edges between triangles inside the part until every one passes the Delaunay test (Lawson's
    /// method): the part's edges are never flipped, so the mesh inside it becomes its constrained Delaunay
    /// triangulation. A unit of work for each edge tested.
    pub(super) fn settle(&mut self, inside: &[bool], meter: &mut Meter) -> Built<()> {
        let within = |t: Option<usize>| t.is_some_and(|t| inside.get(t).copied().unwrap_or(false));
        let mut stack: Vec<(usize, usize)> = Vec::new();
        for (t, tri) in self.tris.iter().enumerate() {
            for k in 0..3 {
                if within(Some(t)) && within(tri.across(k)) && tri.across(k) > Some(t) {
                    stack.push((t, k));
                }
            }
        }
        while let Some((t, k)) = stack.pop() {
            meter.charge(1)?;
            let tri = self.tri(t)?;
            if !within(Some(t)) || !within(tri.across(k)) || !self.not_delaunay(t, k)? {
                continue;
            }
            let u = self.flip(t, k)?;
            // The 4 edges round the new pair: (b, d) and (a, b) of `t`, (d, c) and (c, a) of `u`.
            stack.extend([(t, 0), (t, 2), (u, 0), (u, 1)]);
        }
        Ok(())
    }
}

/// Whether the segments from `a` to `b` and from `c` to `d` cross between their ends.
fn crosses(a: Point, b: Point, c: Point, d: Point) -> bool {
    let apart = |s: Side, t: Side| matches!((s, t), (Side::Left, Side::Right) | (Side::Right, Side::Left));
    apart(exact::side(a, b, c), exact::side(a, b, d)) && apart(exact::side(c, d, a), exact::side(c, d, b))
}
