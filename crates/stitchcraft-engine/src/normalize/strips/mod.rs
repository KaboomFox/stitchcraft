//! A narrow part's strips, junctions and ends, found from its constrained Delaunay triangulation, and the
//! lines they join into: the part's centre lines (`REQ-AUTO-014`).
//!
//! Design: `docs/src/design/image-to-embroidery.md` › Stitch types by width, and
//! `docs/src/design/algorithms/centre-lines.md` › Strips, junctions and ends. Cut into triangles about as
//! long as it is wide ([`triangulation`]), a part has triangles of 3 kinds, by how many of their edges are
//! chords, edges inside the part that another triangle shares: an end has 1, a strip triangle 2 and a
//! junction 3. The middles of the chords along a strip follow the strip's middle. Then:
//!
//! 1. **Ends.** An end within the circle on its chord, as a round or a square end is, is a cap, with no line
//!    in it. Each cap grows across the chords while the circle on each holds every corner of the cap:
//!    through each strip triangle, and through a junction once caps reach 2 of its sides, across its third
//!    chord, the 2 growing on as one. The points round a round end lie on a circle, or nearly, and their
//!    triangles are often junctions, which the caps grow through to where the end meets the rest of the
//!    part. The line ends at the middle of the last chord a cap grew across, or at the middle of a junction
//!    whose other sides are caps. Where an end's chord's circle doesn't hold the end's corner, the end is a
//!    tip, and the line runs on to it. This is how Igarashi, Matsuoka and Tanaka's *Teddy* (1999) finds
//!    where a shape's spine ends.
//! 2. **Lines.** The middles of the chords outside the caps, the middle of each junction triangle, and each
//!    tip are joined: across each strip triangle from chord to chord, and from a junction's middle to its
//!    chords outside the caps.
//! 3. **Bumps.** A branch from a junction to an end that is no longer than [`BUMP`] times the junction's
//!    longest edge is a bump of the outline, or the corner of a sharp turn, and is left out, the shortest
//!    first, until no junction has one. A junction left with 2 branches is a bend.
//! 4. **Junctions.** At each junction the 2 branches that run on most nearly straight are one line, and the
//!    third ends there. Each branch's direction is measured from the junction's middle to the first of its
//!    points at least the junction's longest edge away, or to its end.
//! 5. **Order.** The lines are sewn one after another, the longest first, each from its end nearest where
//!    the last one ended, a closed line from its point nearest.
//!
//! A part that is a ring of strips, such as a letter O, has a closed line. A part whose ends' caps meet,
//! a dot, has none. Every step costs a unit of work.

use std::collections::{BTreeMap, BTreeSet};

use stitchcraft_core::{Exhausted, Meter, Point, math};

use super::region::Polygon;
use super::triangulation::{self, Broken, Triangle, Triangulation};

/// How many times its junction triangle's longest edge a branch from a junction to an end may be long and
/// still be a bump of the outline, or the corner of a sharp turn, which a line along the middle cuts. The
/// corner of a turn of 36° lies within it (`req_auto_014_a_sharp_turn_is_one_line`).
pub(crate) const BUMP: f64 = 2.0;

/// A centre line.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Line {
    /// Its points, in the order it is sewn.
    pub(crate) points: Vec<Point>,
    /// Whether it closes, its last point joined to its first.
    pub(crate) closed: bool,
}

impl Line {
    /// Its length, in millimetres, closing it when it closes.
    pub(crate) fn length(&self) -> f64 {
        let closing = self.closed.then(|| self.points.last().zip(self.points.first())).flatten();
        self.points.windows(2).map(|w| w[0].distance(w[1])).sum::<f64>() + closing.map_or(0.0, |(a, b)| a.distance(*b))
    }
}

/// The centre lines of `part`, cut into triangles with no edge of the part longer than `longest`, in the
/// order they are sewn, with `meter`'s work; none for a dot. A triangulation that loses its shape is a bug.
pub(crate) fn centre_lines(part: &Polygon, longest: f64, meter: &mut Meter) -> Result<Result<Vec<Line>, Broken>, Exhausted> {
    let triangulation = match triangulation::triangulate(part, longest, meter)? {
        Ok(triangulation) => triangulation,
        Err(broken) => return Ok(Err(broken)),
    };
    Ok(Ok(through(&triangulation, meter)?))
}

/// The centre lines through the triangles of `triangulation`, in the order they are sewn, with `meter`'s
/// work.
fn through(triangulation: &Triangulation, meter: &mut Meter) -> Result<Vec<Line>, Exhausted> {
    let roles = roles(triangulation, meter)?;
    let mut graph = Graph::of(triangulation, &roles, meter)?;
    graph.prune(meter)?;
    let pairs = graph.pairs(meter)?;
    let lines = graph.lines(&pairs, meter)?;
    ordered(lines, meter)
}

/// What a triangle is to the centre line.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Role {
    /// The line crosses it.
    Line,
    /// It lies in an end's cap, beyond where the line ends.
    Cap,
    /// It is an end whose corner is a tip, which the line runs on to: the corner's index.
    Tip(usize),
}

/// How many chords triangle `t` has.
fn chords(t: &Triangle) -> usize {
    t.across.iter().flatten().count()
}

/// Whether every point of `held` lies within the circle whose diameter runs from `a` to `b`, edge included:
/// where the 2 ends are seen at a right angle or wider.
fn holds(points: &[Point], a: usize, b: usize, held: &[usize]) -> bool {
    let (Some(&a), Some(&b)) = (points.get(a), points.get(b)) else { return false };
    held.iter().filter_map(|&h| points.get(h)).all(|h| (a.x() - h.x()) * (b.x() - h.x()) + (a.y() - h.y()) * (b.y() - h.y()) <= 0.0)
}

/// What each triangle of `triangulation` is to the centre line, with `meter`'s work: a unit for each triangle,
/// and for each corner tested. An end is a cap when the circle on its chord holds its corner, and a tip when
/// it doesn't. Each cap then grows across the chords while the circle on each holds every corner of the cap:
/// on through a strip triangle, and on through a junction that caps reach on 2 of its sides, across its third
/// chord. Which end grows first makes no difference.
fn roles(triangulation: &Triangulation, meter: &mut Meter) -> Result<Vec<Role>, Exhausted> {
    let Triangulation { points, triangles } = triangulation;
    let mut roles = vec![Role::Line; triangles.len()];
    // Each cap as it grows: the triangle it reached last, the chord of it to cross next, and its corners.
    let mut growing: Vec<(usize, usize, Vec<usize>)> = Vec::new();
    for (end, triangle) in triangles.iter().enumerate() {
        meter.charge(1)?;
        let Some(k) = (0..3).find(|&k| triangle.across[k].is_some()).filter(|_| chords(triangle) == 1) else { continue };
        let (a, b) = triangle.edge(k);
        let role = if holds(points, a, b, &[triangle.corners[k]]) { Role::Cap } else { Role::Tip(triangle.corners[k]) };
        if let Some(slot) = roles.get_mut(end) {
            *slot = role;
        }
        if role == Role::Cap {
            growing.push((end, k, triangle.corners.to_vec()));
        }
    }
    // The caps that reached each junction: the junction's edge each came across, and its corners.
    let mut reached: BTreeMap<usize, Vec<(usize, Vec<usize>)>> = BTreeMap::new();
    while let Some((at, chord, mut held)) = growing.pop() {
        let Some(next) = triangles.get(at).and_then(|t| t.across[chord]) else { continue };
        let (Some(beyond), Some(Role::Line)) = (triangles.get(next), roles.get(next)) else { continue };
        let Some(back) = (0..3).find(|&j| beyond.across[j] == Some(at)) else { continue };
        let out = if chords(beyond) == 2 {
            held.push(beyond.corners[back]);
            (0..3).find(|&j| j != back && beyond.across[j].is_some())
        } else {
            // A junction grows only once caps hold 2 of its sides, which then grow on as one.
            let arrived = reached.entry(next).or_default();
            arrived.push((back, held));
            let [(first, one), (second, other)] = arrived.as_slice() else { continue };
            held = one.iter().chain(other).copied().collect::<BTreeSet<usize>>().into_iter().collect();
            (0..3).find(|&j| j != *first && j != *second)
        };
        let Some(out) = out else { continue };
        meter.charge(held.len() as u64)?;
        let (c, d) = beyond.edge(out);
        if holds(points, c, d, &held) {
            if let Some(slot) = roles.get_mut(next) {
                *slot = Role::Cap;
            }
            growing.push((next, out, held));
        }
    }
    Ok(roles)
}

/// The centre line's points and how they join, before it is cut into lines.
struct Graph {
    /// Each point.
    points: Vec<Point>,
    /// The points each point is joined to.
    links: Vec<Vec<usize>>,
    /// For a junction's middle, its triangle's longest edge.
    size: Vec<Option<f64>>,
}

impl Graph {
    /// The points of the centre line through the triangles of `triangulation` that `roles` says the line
    /// crosses: the middle of each chord of such a triangle, of each junction triangle, and each tip.
    fn of(triangulation: &Triangulation, roles: &[Role], meter: &mut Meter) -> Result<Graph, Exhausted> {
        let Triangulation { points, triangles } = triangulation;
        let mut graph = Graph { points: Vec::new(), links: Vec::new(), size: Vec::new() };
        // The point at the middle of each chord, by the 2 triangles it lies between.
        let mut at_chord: BTreeMap<(usize, usize), usize> = BTreeMap::new();
        let corner = |i: usize| points.get(i).copied();
        for (t, (triangle, role)) in triangles.iter().zip(roles).enumerate() {
            meter.charge(1)?;
            if *role == Role::Cap {
                continue;
            }
            // A junction beside a cap is joined only to its other chords: a junction with one left is where the
            // line ends, at its middle.
            let junction = chords(triangle) == 3;
            let mut ends = Vec::with_capacity(3);
            for k in 0..3 {
                let Some(u) = triangle.across[k].filter(|&u| !junction || roles.get(u) != Some(&Role::Cap)) else { continue };
                let key = (t.min(u), t.max(u));
                let node = match at_chord.get(&key) {
                    Some(&node) => node,
                    None => {
                        let (a, b) = triangle.edge(k);
                        let (Some(a), Some(b)) = (corner(a), corner(b)) else { continue };
                        let node = graph.add(a.lerp(b, 0.5), None);
                        at_chord.insert(key, node);
                        node
                    }
                };
                ends.push(node);
            }
            if junction {
                let [a, b, c] = triangle.corners.map(|i| corner(i).unwrap_or(Point::ORIGIN));
                let centre = Point::new((a.x() + b.x() + c.x()) / 3.0, (a.y() + b.y() + c.y()) / 3.0).unwrap_or(a);
                let size = a.distance(b).max(b.distance(c)).max(c.distance(a));
                let node = graph.add(centre, Some(size));
                for &end in &ends {
                    graph.join(node, end);
                }
                continue;
            }
            match (ends.as_slice(), role) {
                ([only], Role::Tip(tip)) => {
                    if let Some(tip) = corner(*tip) {
                        let node = graph.add(tip, None);
                        graph.join(*only, node);
                    }
                }
                ([a, b], _) => graph.join(*a, *b),
                // A lone triangle is a dot, and an end that isn't a tip lies in its cap.
                _ => {}
            }
        }
        Ok(graph)
    }

    /// Adds point `p`, a junction's middle when it has a `size`, and gives its index.
    fn add(&mut self, p: Point, size: Option<f64>) -> usize {
        self.points.push(p);
        self.links.push(Vec::new());
        self.size.push(size);
        self.points.len() - 1
    }

    /// Joins points `a` and `b`.
    fn join(&mut self, a: usize, b: usize) {
        for (from, to) in [(a, b), (b, a)] {
            self.links.get_mut(from).into_iter().for_each(|links| links.push(to));
        }
    }

    /// Parts points `a` and `b`.
    fn part(&mut self, a: usize, b: usize) {
        for (from, to) in [(a, b), (b, a)] {
            self.links.get_mut(from).into_iter().for_each(|links| links.retain(|&p| p != to));
        }
    }

    /// How many points point `p` is joined to.
    fn degree(&self, p: usize) -> usize {
        self.links.get(p).map_or(0, Vec::len)
    }

    /// The point `p`.
    fn point(&self, p: usize) -> Point {
        self.points.get(p).copied().unwrap_or(Point::ORIGIN)
    }

    /// The branch from `from` through its neighbour `first`: the points to the next point not joined to
    /// exactly 2, which ends it, or back to `from` round a ring. A unit of work a point.
    fn branch(&self, from: usize, first: usize, meter: &mut Meter) -> Result<Vec<usize>, Exhausted> {
        let mut path = vec![first];
        let (mut prev, mut at) = (from, first);
        while self.degree(at) == 2 && at != from {
            meter.charge(1)?;
            let Some(&next) = self.links.get(at).and_then(|links| links.iter().find(|&&p| p != prev)) else { break };
            path.push(next);
            (prev, at) = (at, next);
        }
        Ok(path)
    }

    /// The length of the line from `from` along `path`.
    fn along(&self, from: usize, path: &[usize]) -> f64 {
        std::iter::once(from).chain(path.iter().copied()).collect::<Vec<_>>().windows(2).map(|w| self.point(w[0]).distance(self.point(w[1]))).sum()
    }

    /// The junctions: the points with a size, joined to 3.
    fn junctions(&self) -> Vec<usize> {
        (0..self.points.len()).filter(|&p| self.size.get(p).copied().flatten().is_some() && self.degree(p) == 3).collect()
    }

    /// Leaves out the bumps: at each junction, the shortest branch to an end no longer than [`BUMP`] times
    /// the junction's longest edge, until no junction has one.
    fn prune(&mut self, meter: &mut Meter) -> Result<(), Exhausted> {
        loop {
            let mut shortest: Option<(f64, usize, usize, Vec<usize>)> = None;
            for junction in self.junctions() {
                let size = self.size.get(junction).copied().flatten().unwrap_or(0.0);
                for &first in self.links.get(junction).cloned().unwrap_or_default().iter() {
                    let path = self.branch(junction, first, meter)?;
                    let end = path.last().copied().unwrap_or(first);
                    let length = self.along(junction, &path);
                    if self.degree(end) == 1 && length <= BUMP * size && shortest.as_ref().is_none_or(|(best, ..)| length < *best) {
                        shortest = Some((length, junction, first, path));
                    }
                }
            }
            let Some((_, junction, first, path)) = shortest else { return Ok(()) };
            self.part(junction, first);
            for pair in path.windows(2) {
                self.part(pair[0], pair[1]);
            }
        }
    }

    /// At each junction, the 2 branches that run on most nearly straight, as the neighbours they start
    /// with: the pair whose directions are most nearly opposite, each measured to the first of its points
    /// at least as far from the junction's middle as the junction's longest edge, or to its last point.
    fn pairs(&self, meter: &mut Meter) -> Result<BTreeMap<usize, (usize, usize)>, Exhausted> {
        let mut pairs = BTreeMap::new();
        for junction in self.junctions() {
            let size = self.size.get(junction).copied().flatten().unwrap_or(0.0);
            let centre = self.point(junction);
            let mut directions = Vec::with_capacity(3);
            for &first in self.links.get(junction).into_iter().flatten() {
                let path = self.branch(junction, first, meter)?;
                let far = path.iter().map(|&p| self.point(p)).find(|&p| centre.distance(p) >= size).or_else(|| path.last().map(|&p| self.point(p)));
                let (dx, dy) = far.map_or((0.0, 0.0), |p| (p.x() - centre.x(), p.y() - centre.y()));
                directions.push((first, dx, dy));
            }
            let cosine = |a: &(usize, f64, f64), b: &(usize, f64, f64)| {
                let lengths = math::hypot(a.1, a.2) * math::hypot(b.1, b.2);
                if lengths > 0.0 { (a.1 * b.1 + a.2 * b.2) / lengths } else { 1.0 }
            };
            let mut best: Option<(f64, usize, usize)> = None;
            for (i, a) in directions.iter().enumerate() {
                for b in directions.iter().skip(i + 1) {
                    let c = cosine(a, b);
                    if best.is_none_or(|(least, ..)| c < least) {
                        best = Some((c, a.0, b.0));
                    }
                }
            }
            if let Some((_, a, b)) = best {
                pairs.insert(junction, (a, b));
            }
        }
        Ok(pairs)
    }

    /// The lines: from each end, then from each junction along its third branch, then round each ring left,
    /// each line going on through a junction along the branch paired with the one it came by.
    fn lines(&self, pairs: &BTreeMap<usize, (usize, usize)>, meter: &mut Meter) -> Result<Vec<Line>, Exhausted> {
        let mut walked: BTreeSet<(usize, usize)> = BTreeSet::new();
        let mut lines = Vec::new();
        let ends = (0..self.points.len()).filter(|&p| self.degree(p) == 1).map(|p| (p, self.links.get(p).and_then(|l| l.first().copied())));
        let thirds =
            pairs.iter().map(|(&junction, &(a, b))| (junction, self.links.get(junction).and_then(|l| l.iter().copied().find(|&p| p != a && p != b))));
        let starts: Vec<(usize, usize)> = ends.chain(thirds).filter_map(|(from, first)| Some((from, first?))).collect();
        for (from, first) in starts {
            if !walked.contains(&edge(from, first)) {
                lines.push(self.walk(from, first, pairs, &mut walked, meter)?);
            }
        }
        // What is left are rings.
        for from in 0..self.points.len() {
            for &first in self.links.get(from).into_iter().flatten() {
                if !walked.contains(&edge(from, first)) {
                    lines.push(self.walk(from, first, pairs, &mut walked, meter)?);
                }
            }
        }
        lines.retain(|line| line.points.len() >= 2);
        Ok(lines)
    }

    /// The line from `from` through its neighbour `first`, on through each point joined to 2 and each
    /// junction along the paired branch, to an end, to a junction it reaches by its third branch, back to
    /// where it started, when it closes, or to where it would go on along an edge another line took. A line
    /// from a junction's third branch can come back to the junction by one of its paired branches and close
    /// there, and a later line that comes by the other stops, as at the waist of a figure 8. The edges it
    /// takes are added to `walked`.
    fn walk(
        &self,
        from: usize,
        first: usize,
        pairs: &BTreeMap<usize, (usize, usize)>,
        walked: &mut BTreeSet<(usize, usize)>,
        meter: &mut Meter,
    ) -> Result<Line, Exhausted> {
        let mut path = vec![from];
        let (mut prev, mut at) = (from, first);
        loop {
            meter.charge(1)?;
            walked.insert(edge(prev, at));
            if at == from {
                return Ok(Line { points: path.iter().map(|&p| self.point(p)).collect(), closed: true });
            }
            path.push(at);
            let next = match (self.degree(at), pairs.get(&at)) {
                (2, _) => self.links.get(at).and_then(|l| l.iter().copied().find(|&p| p != prev)),
                (3, Some(&(a, b))) if prev == a => Some(b),
                (3, Some(&(a, b))) if prev == b => Some(a),
                _ => None,
            };
            match next {
                Some(next) if !walked.contains(&edge(at, next)) => (prev, at) = (at, next),
                _ => return Ok(Line { points: path.iter().map(|&p| self.point(p)).collect(), closed: false }),
            }
        }
    }
}

/// The edge between points `a` and `b`, either way round.
fn edge(a: usize, b: usize) -> (usize, usize) {
    (a.min(b), a.max(b))
}

/// `lines` in the order they are sewn: the longest first, as found, then each from its end nearest where
/// the last one ended, turned round to start there, and a closed one from its point nearest. On a tie, the
/// line found first, from its start. A unit of work for each point looked at.
fn ordered(mut lines: Vec<Line>, meter: &mut Meter) -> Result<Vec<Line>, Exhausted> {
    let mut out: Vec<Line> = Vec::with_capacity(lines.len());
    let longest = lines
        .iter()
        .enumerate()
        .map(|(i, line)| (i, line.length()))
        .fold(None, |best: Option<(usize, f64)>, (i, length)| if best.is_none_or(|(_, most)| length > most) { Some((i, length)) } else { best });
    let Some((first, _)) = longest else { return Ok(out) };
    out.push(lines.remove(first));
    while !lines.is_empty() {
        let Some(here) = out.last().and_then(|line| if line.closed { line.points.first() } else { line.points.last() }).copied() else { break };
        // (distance², line, point it starts at): an open line's start is its point 0 and its end its last.
        let mut nearest: Option<(f64, usize, usize)> = None;
        for (i, line) in lines.iter().enumerate() {
            let candidates: Vec<usize> = if line.closed { (0..line.points.len()).collect() } else { vec![0, line.points.len().saturating_sub(1)] };
            for at in candidates {
                meter.charge(1)?;
                let Some(&p) = line.points.get(at) else { continue };
                let (dx, dy) = (p.x() - here.x(), p.y() - here.y());
                let d = dx * dx + dy * dy;
                if nearest.is_none_or(|(least, ..)| d < least) {
                    nearest = Some((d, i, at));
                }
            }
        }
        let Some((_, i, at)) = nearest else { break };
        let mut line = lines.remove(i);
        if line.closed {
            line.points.rotate_left(at);
        } else if at > 0 {
            line.points.reverse();
        }
        out.push(line);
    }
    Ok(out)
}

#[cfg(test)]
mod tests;
