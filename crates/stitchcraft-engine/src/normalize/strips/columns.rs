//! The satin columns of a part no wider than a satin column: one along each of its centre lines, between
//! the part's own outlines (`REQ-AUTO-016`, `REQ-AUTO-017`).
//!
//! Design: `docs/src/design/image-to-embroidery.md` › Stitch types by width, and
//! `docs/src/design/algorithms/centre-lines.md` › Columns. A column covers the triangles of the part
//! around its line ([`super::Skeleton`]). Its lines are found as a narrow part's are, but a branch is a bump
//! only when it ends at a tip and at its junction too, the junction's 2 other branches running on
//! ([`Bumps::Tips`]): a column beside a branch would cover it with stitches across the column and out along
//! the branch, as a stem would sew the crossbar of a t with stitches from one end of it to the other. So
//! each arm of a t's crossbar is a column of its own, or runs on through the stem, which ends under it; and a
//! sharp corner that is a branch sews as part of the column of the arm that runs on into it most nearly
//! straight.
//!
//! 1. **Its lines.** A line that turns by more than [`CORNER`] at a junction's middle, as the line of an L
//!    does where its corner is a cap beside the junction, is cut there: a column that turned so would sew
//!    its stitches into a fan at the inside of the corner. The column before the corner covers it, and the
//!    one after ends under it, as an arm of a T ends under its bar. A line that comes back to a junction it
//!    ran through, as the line of an R runs up its stem, round its bowl and back, is cut there into the line
//!    up to the junction and a ring, whose column goes round the hole as a ring's does.
//! 2. **Its triangles.** A triangle goes to the line that crosses it, and a junction to the line that runs
//!    on through it, which crosses it twice, or to the one before it where 2 meet there. The caps of a
//!    line's ends and the bumps left out of it, which no line crosses, go to the column beside them.
//! 3. **Its outline.** The edges of a column's triangles that no other of its triangles shares are its
//!    outline: the part's own edges, and the chords where it meets another column. An open line's column
//!    has one ring round it, and a closed line's 2, round the part's outline and its hole. Any other is no
//!    column, and the part isn't sewn as columns.
//! 4. **Its ends.** A line that ends at a junction another line runs on through, or where another line has
//!    sewn the line on, ends at the chord there, which cuts its column across: it ends under that column.
//!    Otherwise the column's own end lies beyond the last chord the line crosses, and takes in a bend the
//!    line runs through on the way there, a junction whose third side is a cap or a bump, as beside the
//!    blunt corner of a slanted end. The end is where the column reaches farthest along the line's way at
//!    that chord, from the line's point before it: the last piece of a line, to a tip or to a junction's
//!    middle, runs off the column's way. Where an edge of the outline there lies across that way, within
//!    20° of square to it, and is no longer than twice that chord, the column ends along all of it, as far
//!    as it runs straight, past where the end's triangles stop; otherwise at that point. A longer edge runs
//!    along the column, as its side does where its line turns into a tip. A column that is no more than its
//!    end, as a stub beyond a junction is, has that chord on its outline, and its end goes round the other
//!    way.
//! 5. **Its rails** are the outline on either side between its ends, both from the line's start. A ring's
//!    are its 2 rings, both the same way round, started at a rung whose second point is the inner ring's
//!    point nearest its first, so that a satin column starts them together (`DEV-SAT-009`).
//! 6. **Its rungs.** A triangle with one edge on a rail has a corner on the other, and a rung runs from that
//!    corner across the column, at a right angle to the column's way there, to the edge: where it meets it,
//!    or else to the edge's nearer end. The column's way is the mean of its 2 rails' ways, the corner's
//!    rail at the corner and the other along the edge: on a band or round a ring it runs along the middle,
//!    and where a column tapers or ends at a corner, between its sides. Of a corner's rungs, the one that
//!    comes nearest that right angle is kept, and so of rungs whose points on a rail lie closer than twice
//!    the reach a rung is drawn with past the rails (`drawn::REACH`): drawn so, they would cross, as the
//!    rungs that crowd into the inside of a tight curve do. A rung at a rail's end is left out: the rails'
//!    ends pair up.
//!
//! Every step costs a unit of work.

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use stitchcraft_core::{Exhausted, Meter, Point, math};

use super::{Bumps, Line, Node, Skeleton, Walked, edge, skeleton};
use crate::generators::satin::drawn::REACH;
use crate::normalize::region::Polygon;
use crate::normalize::satin::SAME_POINT;
use crate::normalize::triangulation::Broken;

/// The sharpest turn a line takes at a junction's middle and still sews as one column: the cosine of the angle
/// between its 2 ways from there, 120°, for a turn of 60°. Each way is measured as a junction's branches are,
/// to the line's first point at least the junction triangle's longest edge away.
const CORNER: f64 = -0.5;

/// How far from square to the line an edge across a column's end may lie, and the column still end along
/// it: the sine of 20°.
const SQUARE: f64 = 0.342_020_143_325_668_7;

/// How far, in millimetres, a point may lie off the line through the points before it and still be on a
/// straight edge of the outline: the points added along an edge lie on it to within the region's grid.
const STRAIGHT: f64 = 1e-6;

/// A satin column along one of a part's centre lines, between the part's own outlines.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Column {
    /// Its 2 rails, both from its start. A ring's go round the part's outline and its hole, and end where
    /// they start.
    pub(crate) rails: [Vec<Point>; 2],
    /// Its rungs in order along it, each as the point it pairs on the first rail, then on the second. The
    /// rails' starts pair up, and so do their ends, without a rung.
    pub(crate) rungs: Vec<[Point; 2]>,
    /// Whether it is a ring.
    pub(crate) closed: bool,
    /// Its centre line from its start, without a junction's middle that an end of it reaches and the column
    /// doesn't cover.
    pub(crate) line: Line,
    /// Its centre line as walked, from its start.
    pub(crate) walked: Line,
    /// For its start, then its end, the column it ends under: the one that runs on through the junction it
    /// ends at, or along the line beyond its end.
    pub(crate) under: [Option<usize>; 2],
    /// A ring's rails as found, and the rungs it may start at.
    ring: Option<Ring>,
}

/// A ring as found, before it is started at a rung.
#[derive(Clone, Debug, PartialEq)]
struct Ring {
    /// The outer ring and the inner one, the same way round, each ending where it starts.
    rails: [Vec<Point>; 2],
    /// Each ring's length.
    lengths: [f64; 2],
    /// Every rung, with how far along each ring it lies.
    rungs: Vec<Rung>,
}

/// A rung across a column.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Rung {
    /// The point it pairs on the first rail, then on the second.
    points: [Point; 2],
    /// How far along each rail they lie.
    at: [f64; 2],
    /// How far the edge's nearer end, when it ran to that, lies from where a rung at a right angle would
    /// meet the edge's line: 0 for a rung at a right angle.
    off: f64,
}

impl Column {
    /// The column the other way round: from its end to its start. A ring is itself.
    pub(crate) fn reversed(&self) -> Column {
        if self.closed {
            return self.clone();
        }
        let back = |line: &Line| Line { points: line.points.iter().rev().copied().collect(), closed: line.closed };
        Column {
            rails: self.rails.clone().map(|mut rail| {
                rail.reverse();
                rail
            }),
            rungs: self.rungs.iter().rev().copied().collect(),
            closed: false,
            line: back(&self.line),
            walked: back(&self.walked),
            under: [self.under[1], self.under[0]],
            ring: None,
        }
    }

    /// A ring started at the rung nearest `near` at which it can start, with `meter`'s work; an open column
    /// as it is. A ring that started once starts again: which rungs it can start at doesn't depend on `near`.
    pub(crate) fn restarted(&self, near: Point, meter: &mut Meter) -> Result<Column, Exhausted> {
        let Some(ring) = &self.ring else { return Ok(self.clone()) };
        let Some(Started { rails, rungs }) = started(ring, near, meter)? else { return Ok(self.clone()) };
        Ok(Column { rails, rungs, ..self.clone() })
    }
}

/// A ring's rails and rungs, started at a rung.
struct Started {
    rails: [Vec<Point>; 2],
    rungs: Vec<[Point; 2]>,
}

/// The columns of `part`, one along each of its centre lines, its edges cut into pieces no longer than
/// `longest`, with `meter`'s work: none for a dot, and `None` when the triangles round a line make no column.
/// A triangulation that loses its shape is a bug.
pub(crate) fn columns(part: &Polygon, longest: f64, meter: &mut Meter) -> Result<Result<Option<Vec<Column>>, Broken>, Exhausted> {
    match skeleton(part, longest, Bumps::Tips, meter)? {
        Ok(skeleton) => Ok(Ok(built(&skeleton, meter)?)),
        Err(broken) => Ok(Err(broken)),
    }
}

/// The columns of `skeleton`'s lines, with `meter`'s work; `None` when the triangles round one make no column.
fn built(skeleton: &Skeleton, meter: &mut Meter) -> Result<Option<Vec<Column>>, Exhausted> {
    let lines = unlooped(&cornered(skeleton, &skeleton.walked));
    if lines.is_empty() {
        return Ok(Some(Vec::new()));
    }
    let Some(owner) = owners(skeleton, &lines, meter)? else { return Ok(None) };
    let owning: BTreeSet<usize> = owner.iter().copied().collect();
    let mut columns = Vec::new();
    let mut numbered = BTreeMap::new();
    // A line that crosses no triangle of its own, only a junction another runs on through, covers nothing.
    for (line, walked) in lines.iter().enumerate().filter(|(line, _)| owning.contains(line)) {
        let Some(column) = column(skeleton, &owner, line, walked, meter)? else { return Ok(None) };
        numbered.insert(line, columns.len());
        columns.push(column);
    }
    for column in &mut columns {
        column.under = column.under.map(|under| under.and_then(|line| numbered.get(&line).copied()));
    }
    Ok(Some(columns))
}

/// The lines `walked`, through `skeleton`'s points, cut at each junction's middle where they turn by more than
/// [`CORNER`]: a closed line with 2 or more such corners becomes as many open lines, each from one to the next.
fn cornered(skeleton: &Skeleton, walked: &[Walked]) -> Vec<Walked> {
    let graph = &skeleton.graph;
    let mut lines = Vec::with_capacity(walked.len());
    for line in walked {
        let nodes = &line.nodes;
        let count = nodes.len();
        // How sharply the line turns at its point `i`, a junction's middle: the cosine of the angle between its
        // 2 ways from there, 1 at the sharpest; `None` elsewhere.
        let turn = |i: usize| -> Option<f64> {
            let size = nodes.get(i).and_then(|&node| graph.size.get(node).copied().flatten())?;
            let at = nodes.get(i).map_or(Point::ORIGIN, |&node| graph.point(node));
            // The way from the middle to the first point at least `size` away, stepping by `step`. An open line
            // has no way past its ends, and so no turn at them.
            let way = |step: isize| {
                let mut k = i;
                let mut far = at;
                for _ in 1..count {
                    let next = if line.closed { Some((k as isize + step).rem_euclid(count as isize) as usize) } else { k.checked_add_signed(step) };
                    let Some((next, &node)) = next.and_then(|j| Some((j, nodes.get(j)?))) else { break };
                    k = next;
                    far = graph.point(node);
                    if far.distance(at) >= size {
                        break;
                    }
                }
                (far.x() - at.x(), far.y() - at.y())
            };
            let (a, b) = (way(-1), way(1));
            let lengths = math::hypot(a.0, a.1) * math::hypot(b.0, b.1);
            (lengths > 0.0).then(|| (a.0 * b.0 + a.1 * b.1) / lengths)
        };
        // The corners, and of 2 corners that lie within twice the larger junction's size of each other, as
        // where a V ends flat, only the sharper: a column between them would be no longer than a junction.
        let mut corners: Vec<(usize, f64)> = Vec::new();
        for i in 0..count {
            let Some(cosine) = turn(i).filter(|&cosine| cosine > CORNER) else { continue };
            let size = |k: usize| nodes.get(k).and_then(|&node| graph.size.get(node).copied().flatten()).unwrap_or(0.0);
            let at = |k: usize| nodes.get(k).map_or(Point::ORIGIN, |&node| graph.point(node));
            match corners.last_mut() {
                Some(last) if at(last.0).distance(at(i)) < 2.0 * size(last.0).max(size(i)) => {
                    if cosine > last.1 {
                        *last = (i, cosine);
                    }
                }
                _ => corners.push((i, cosine)),
            }
        }
        let corners: Vec<usize> = corners.into_iter().map(|(i, _)| i).collect();
        match (line.closed, corners.first()) {
            // A ring cut at one corner would still go round its hole: it stays a ring.
            (_, None) | (true, Some(_)) if line.closed && corners.len() < 2 => lines.push(line.clone()),
            (_, None) => lines.push(line.clone()),
            (false, Some(_)) => {
                let mut from = 0;
                for &at in &corners {
                    lines.push(Walked { nodes: nodes.get(from..=at).unwrap_or_default().to_vec(), closed: false });
                    from = at;
                }
                lines.push(Walked { nodes: nodes.get(from..).unwrap_or_default().to_vec(), closed: false });
            }
            (true, Some(&first)) => {
                // Round from the first corner, each piece to the next, the last back to the first.
                let round: Vec<usize> = (0..=count).map(|k| nodes.get((first + k) % count).copied().unwrap_or_default()).collect();
                let mut from = 0;
                for &at in corners.iter().skip(1).map(|c| c - first).chain(std::iter::once(count)).collect::<Vec<_>>().iter() {
                    lines.push(Walked { nodes: round.get(from..=at).unwrap_or_default().to_vec(), closed: false });
                    from = at;
                }
            }
        }
    }
    lines
}

/// The lines `walked`, each that ends at a junction it ran through cut there in 2: the line up to the
/// junction, and the ring from it round back to it, a closed line. A line comes back so by the junction's
/// third branch, where it stops.
fn unlooped(walked: &[Walked]) -> Vec<Walked> {
    let mut lines = Vec::with_capacity(walked.len());
    for line in walked {
        let back = line.nodes.split_last().filter(|_| !line.closed).and_then(|(last, before)| before.iter().position(|node| node == last));
        match back {
            // The ring needs 3 points, and the line up to it 2.
            Some(at) if at >= 1 && line.nodes.len() >= at + 4 => {
                lines.push(Walked { nodes: line.nodes.get(..=at).unwrap_or_default().to_vec(), closed: false });
                lines.push(Walked { nodes: line.nodes.get(at..line.nodes.len() - 1).unwrap_or_default().to_vec(), closed: true });
            }
            _ => lines.push(line.clone()),
        }
    }
    lines
}

/// The line of `lines`, `skeleton`'s, that each of its triangles goes to, with `meter`'s work: the line with
/// the most pieces in it, the first of as many, and for a triangle no line crosses, the line of a triangle
/// beside it, nearest first. `None` when a triangle has none, which no part has.
fn owners(skeleton: &Skeleton, lines: &[Walked], meter: &mut Meter) -> Result<Option<Vec<usize>>, Exhausted> {
    let triangles = &skeleton.triangulation.triangles;
    let mut crossed: Vec<BTreeMap<usize, usize>> = vec![BTreeMap::new(); triangles.len()];
    for (line, walked) in lines.iter().enumerate() {
        for (a, b) in pieces(walked) {
            meter.charge(1)?;
            let Some(count) = skeleton.graph.inside.get(&edge(a, b)).and_then(|&t| crossed.get_mut(t)) else { continue };
            *count.entry(line).or_default() += 1;
        }
    }
    let mut owner: Vec<Option<usize>> = crossed
        .iter()
        .map(|lines| {
            lines
                .iter()
                .fold(None, |best: Option<(usize, usize)>, (&line, &n)| if best.is_some_and(|(_, most)| most >= n) { best } else { Some((line, n)) })
        })
        .map(|best| best.map(|(line, _)| line))
        .collect();
    let mut queue: VecDeque<usize> = (0..triangles.len()).filter(|&t| owner.get(t).copied().flatten().is_some()).collect();
    while let Some(t) = queue.pop_front() {
        meter.charge(1)?;
        let (Some(triangle), Some(&Some(line))) = (triangles.get(t), owner.get(t)) else { continue };
        for u in triangle.across.iter().flatten() {
            if let Some(slot @ None) = owner.get_mut(*u) {
                *slot = Some(line);
                queue.push_back(*u);
            }
        }
    }
    Ok(owner.into_iter().collect())
}

/// The pieces of `walked`, as the points each joins, its closing piece included.
fn pieces(walked: &Walked) -> impl Iterator<Item = (usize, usize)> + '_ {
    let closing = walked.closed.then(|| walked.nodes.last().copied().zip(walked.nodes.first().copied())).flatten();
    walked.nodes.windows(2).map(|w| (w[0], w[1])).chain(closing)
}

/// A column's outline.
struct Outline {
    /// Its rings, each as the points it passes, the column on its left (counter-clockwise round the outside,
    /// in y-up axes, as the triangles' corners run).
    rings: Vec<Vec<usize>>,
    /// The triangle each edge of the rings belongs to, by the edge's points in the rings' order.
    triangle: BTreeMap<(usize, usize), usize>,
}

/// The outline of `line`'s column, whose triangles `owner` says, with `meter`'s work; `None` when its edges
/// don't make rings, as where the column touches itself at a point.
fn outline(skeleton: &Skeleton, owner: &[usize], line: usize, meter: &mut Meter) -> Result<Option<Outline>, Exhausted> {
    let mut next: BTreeMap<usize, usize> = BTreeMap::new();
    let mut triangle = BTreeMap::new();
    for (t, corners) in skeleton.triangulation.triangles.iter().enumerate() {
        meter.charge(1)?;
        if owner.get(t) != Some(&line) {
            continue;
        }
        for k in 0..3 {
            let inside = corners.across[k].is_some_and(|u| owner.get(u) == Some(&line));
            if inside {
                continue;
            }
            let (a, b) = corners.edge(k);
            if next.insert(a, b).is_some() {
                return Ok(None);
            }
            triangle.insert((a, b), t);
        }
    }
    let mut rings = Vec::new();
    let mut seen = BTreeSet::new();
    for &start in next.keys() {
        if !seen.insert(start) {
            continue;
        }
        // The edges of a set of triangles make closed rings, and no 2 of them start at a point (checked above),
        // so the walk comes back to `start`. The checks below keep it finite were that ever not so.
        let mut ring = vec![start];
        let mut at = start;
        loop {
            meter.charge(1)?;
            let Some(&to) = next.get(&at) else { return Ok(None) };
            if to == start {
                break;
            }
            let true = seen.insert(to) else { return Ok(None) };
            ring.push(to);
            at = to;
        }
        rings.push(ring);
    }
    Ok(Some(Outline { rings, triangle }))
}

/// The column of `line`, walked as `walked`, whose triangles `owner` says, with `meter`'s work; `None` when
/// its triangles make no column.
fn column(skeleton: &Skeleton, owner: &[usize], line: usize, walked: &Walked, meter: &mut Meter) -> Result<Option<Column>, Exhausted> {
    let Some(outline) = outline(skeleton, owner, line, meter)? else { return Ok(None) };
    let found = skeleton.line(walked);
    if walked.closed {
        return ring(skeleton, owner, line, &outline, found, meter);
    }
    strip(skeleton, owner, line, &outline, walked, found, meter)
}

/// How a column ends.
enum End {
    /// Cut across at the chord between the corners, under the column of line `under`.
    Cut { corners: (usize, usize), under: usize },
    /// At its own end, beyond the chord between the corners, from triangle `beyond` on, the line going `way`
    /// where it crosses that chord.
    Own { corners: (usize, usize), beyond: usize, way: (f64, f64) },
}

/// How `line`'s column ends at one end of its walked line, whose points `inward` lists from that end towards
/// its middle, and whether the end is a junction's middle the column doesn't cover; `None` when the points
/// aren't where a line ends.
fn end(skeleton: &Skeleton, owner: &[usize], line: usize, inward: &[usize]) -> Option<(End, bool)> {
    let graph = &skeleton.graph;
    let (&at, &next) = (inward.first()?, inward.get(1)?);
    let chord = |node: usize| if let Some(Node::Chord { corners, .. }) = graph.nodes.get(node) { Some(*corners) } else { None };
    // The place in `inward` of the chord the column's own end lies beyond, and the triangle beyond it.
    let (mut place, mut beyond) = match *graph.nodes.get(at)? {
        Node::Junction(junction) if owner.get(junction) != Some(&line) => {
            return Some((End::Cut { corners: chord(next)?, under: *owner.get(junction)? }, true));
        }
        Node::Junction(beyond) | Node::Tip { triangle: beyond } => (1, beyond),
        Node::Chord { triangles: (t, u), corners } => {
            let body = *graph.inside.get(&edge(next, at))?;
            let beyond = if body == t { u } else { t };
            let under = *owner.get(beyond)?;
            let true = under == line else { return Some((End::Cut { corners, under }, false)) };
            (0, beyond)
        }
    };
    // A bend the line runs through on the way, a junction whose third side is a cap or a bump, as beside the
    // blunt corner of a slanted end, lies in the end, with that side.
    while inward.get(place + 2).is_some() {
        let Some(&bend) = inward.get(place + 1) else { break };
        match graph.nodes.get(bend) {
            Some(&Node::Junction(triangle)) if graph.degree(bend) == 2 => {
                place += 2;
                beyond = triangle;
            }
            _ => break,
        }
    }
    let corners = chord(*inward.get(place)?)?;
    // The line's way where it crosses that chord, from its point before: the last piece of a line to a tip
    // or a junction's middle runs off its way.
    let (from, to) = match (inward.get(place + 1), inward.get(place)) {
        (Some(&from), Some(&to)) => (from, to),
        _ => (next, at),
    };
    let (a, b) = (graph.point(from), graph.point(to));
    Some((End::Own { corners, beyond, way: (b.x() - a.x(), b.y() - a.y()) }, false))
}

/// The column of the open `line`, walked as `walked` through the points of `found`, with the outline
/// `outline`; `None` when it makes no column.
fn strip(
    skeleton: &Skeleton,
    owner: &[usize],
    line: usize,
    outline: &Outline,
    walked: &Walked,
    found: Line,
    meter: &mut Meter,
) -> Result<Option<Column>, Exhausted> {
    let [ring] = outline.rings.as_slice() else { return Ok(None) };
    // Each end's points, from that end to the line's middle.
    let half = walked.nodes.len() / 2 + 1;
    let from_start: Vec<usize> = walked.nodes.iter().take(half).copied().collect();
    let from_finish: Vec<usize> = walked.nodes.iter().rev().take(half).copied().collect();
    let Some((start, cut_start)) = end(skeleton, owner, line, &from_start) else { return Ok(None) };
    let Some((finish, cut_finish)) = end(skeleton, owner, line, &from_finish) else { return Ok(None) };
    let points = &skeleton.triangulation.points;
    let place = Place { skeleton, owner, line, outline, ring };
    let (Some(a), Some(b)) = (place.range(&start, meter)?, place.range(&finish, meter)?) else { return Ok(None) };
    let Some(rails) = rails(ring, a, b) else { return Ok(None) };
    let candidates = rungs(skeleton, owner, line, &rails, meter)?;
    let lengths = rails.each_ref().map(|rail| length(points, rail));
    let rungs = ordered(candidates, lengths, None);
    let under = |end: &End| match end {
        End::Cut { under, .. } => Some(*under),
        End::Own { .. } => None,
    };
    let mut trimmed = found.points.clone();
    if cut_finish {
        trimmed.pop();
    }
    if cut_start && !trimmed.is_empty() {
        trimmed.remove(0);
    }
    Ok(Some(Column {
        rails: rails.map(|rail| rail.iter().filter_map(|&i| points.get(i).copied()).collect()),
        rungs: rungs.iter().map(|rung| rung.points).collect(),
        closed: false,
        line: Line { points: trimmed, closed: false },
        walked: found,
        under: [under(&start), under(&finish)],
        ring: None,
    }))
}

/// A column's outline ring, as its ends are placed on it.
struct Place<'a> {
    skeleton: &'a Skeleton,
    owner: &'a [usize],
    line: usize,
    outline: &'a Outline,
    ring: &'a [usize],
}

impl Place<'_> {
    /// Where on the ring the column's end `end` lies: the places in the ring of the end's first and last
    /// point, in the ring's order; one place for an end at a point. `None` when the end isn't on the ring.
    fn range(&self, end: &End, meter: &mut Meter) -> Result<Option<(usize, usize)>, Exhausted> {
        let n = self.ring.len();
        let place = |point: usize| self.ring.iter().position(|&p| p == point);
        meter.charge(u64::try_from(n).unwrap_or(u64::MAX))?;
        match *end {
            // The chord is an edge of the ring, from the corner before to the one after.
            End::Cut { corners: (a, b), .. } => {
                let (Some(i), Some(j)) = (place(a), place(b)) else { return Ok(None) };
                Ok([(i, j), (j, i)].into_iter().find(|&(from, to)| (from + 1) % n == to))
            }
            End::Own { corners: (a, b), beyond, way } => {
                let region = self.beyond(beyond, (a, b), meter)?;
                let (Some(i), Some(j)) = (place(a), place(b)) else { return Ok(None) };
                // An edge of the ring from place `from` that is one of the region's, but for the chord itself,
                // which lies on the ring where the column is no more than its end, as a stub is.
                let in_region = |from: usize| {
                    let (Some(&p), Some(&q)) = (self.ring.get(from), self.ring.get((from + 1) % n)) else { return false };
                    edge(p, q) != edge(a, b) && self.outline.triangle.get(&(p, q)).is_some_and(|t| region.contains(t))
                };
                // The arc round the end runs from one corner of the chord to the other, along the region's edges:
                // the region is bounded by the chord and that arc.
                let Some((from, to)) = [(i, j), (j, i)].into_iter().find(|&(from, _)| in_region(from)) else { return Ok(None) };
                let steps = (to + n - from) % n;
                meter.charge(u64::try_from(steps).unwrap_or(u64::MAX))?;
                let arc: Vec<usize> = (0..=steps).map(|k| (from + k) % n).collect();
                let true = arc.iter().take(steps).all(|&place| in_region(place)) else { return Ok(None) };
                // An edge longer than twice the chord the end begins at runs along the column, as a side does
                // where its line turns into a tip at its end: no end.
                let points = &self.skeleton.triangulation.points;
                let longest = points.get(a).zip(points.get(b)).map_or(0.0, |(p, q)| 2.0 * p.distance(*q));
                Ok(self.farthest(&arc, way, longest))
            }
        }
    }

    /// The triangles of the column beyond the chord between `corners`, from `beyond` on, with `meter`'s work.
    fn beyond(&self, beyond: usize, corners: (usize, usize), meter: &mut Meter) -> Result<BTreeSet<usize>, Exhausted> {
        let triangles = &self.skeleton.triangulation.triangles;
        let mut region = BTreeSet::from([beyond]);
        let mut queue = vec![beyond];
        while let Some(t) = queue.pop() {
            meter.charge(1)?;
            let Some(triangle) = triangles.get(t) else { continue };
            for k in 0..3 {
                let (i, j) = triangle.edge(k);
                let Some(u) = triangle.across[k] else { continue };
                if edge(i, j) != edge(corners.0, corners.1) && self.owner.get(u) == Some(&self.line) && region.insert(u) {
                    queue.push(u);
                }
            }
        }
        Ok(region)
    }

    /// Where on the ring the end along `arc`, places in the ring, lies, its line going `way`: the straight
    /// edge across the line, within [`SQUARE`] of square to it and no longer than `longest`, where the end
    /// reaches farthest along the line, or that farthest point. The edge runs on round the ring past the arc
    /// as far as it is straight: the triangles of an end can stop at a chord from a point between an edge's
    /// pieces, which leaves the edge's other piece beside them.
    fn farthest(&self, arc: &[usize], way: (f64, f64), longest: f64) -> Option<(usize, usize)> {
        let points = &self.skeleton.triangulation.points;
        let point = |place: usize| self.ring.get(place).and_then(|&p| points.get(p)).copied();
        let along = |p: Point| p.x() * way.0 + p.y() * way.1;
        let length = math::hypot(way.0, way.1);
        let mut far: Option<(usize, f64)> = None;
        for &place in arc {
            let reach = along(point(place)?);
            if far.is_none_or(|(_, most)| reach > most) {
                far = Some((place, reach));
            }
        }
        let (k, _) = far?;
        // How far from square to the way the edge between 2 places lies, as a sine, when within [`SQUARE`] and
        // no longer than `longest`.
        let square = |(from, to): (usize, usize)| {
            let (p, q) = (point(from)?, point(to)?);
            let span = p.distance(q);
            let across = ((q.x() - p.x()) * way.0 + (q.y() - p.y()) * way.1).abs() / (span * length);
            (from != to && span > 0.0 && span <= longest && length > 0.0 && across <= SQUARE).then_some(across)
        };
        let (back, on) = (self.straight(k, false), self.straight(k, true));
        // Where the end reaches as far at the points of a straight edge, the farthest can lie between its
        // pieces: the edge is then both ways from it.
        if back != k && on != k && self.in_line(back, k, on) && square((back, on)).is_some() {
            return Some((back, on));
        }
        let best = [(k, on), (back, k)].into_iter().filter_map(|span| Some((span, square(span)?))).fold(
            None,
            |best: Option<((usize, usize), f64)>, (span, across)| {
                if best.is_some_and(|(_, least)| least <= across) { best } else { Some((span, across)) }
            },
        );
        Some(best.map_or((k, k), |(span, _)| span))
    }

    /// The last place of the ring from place `from`, forward round it or back, before its points stop lying on
    /// the straight line through the first 2.
    fn straight(&self, from: usize, forward: bool) -> usize {
        let n = self.ring.len();
        let next = |k: usize| if forward { (k + 1) % n } else { (k + n - 1) % n };
        let mut last = next(from);
        for _ in 2..n {
            let k = next(last);
            if !self.in_line(from, last, k) {
                break;
            }
            last = k;
        }
        last
    }

    /// Whether the point at place `c` of the ring lies on the straight line through those at places `a` and
    /// `b`, to within the region's grid ([`STRAIGHT`]).
    fn in_line(&self, a: usize, b: usize, c: usize) -> bool {
        let points = &self.skeleton.triangulation.points;
        let point = |place: usize| self.ring.get(place).and_then(|&p| points.get(p)).copied();
        let (Some(a), Some(b), Some(c)) = (point(a), point(b), point(c)) else { return false };
        let (dx, dy) = (b.x() - a.x(), b.y() - a.y());
        let length = math::hypot(dx, dy);
        length > 0.0 && ((c.x() - a.x()) * dy - (c.y() - a.y()) * dx).abs() / length <= STRAIGHT
    }
}

/// The rails between the ends at places `a` and `b` of `ring`, each its first and last place in the ring's
/// order: the ring from `a`'s last to `b`'s first, and from `b`'s last back to `a`'s first turned round, so
/// that both run from `a` to `b`. `None` when the ends overlap or leave a rail of one point.
fn rails(ring: &[usize], (a0, a1): (usize, usize), (b0, b1): (usize, usize)) -> Option<[Vec<usize>; 2]> {
    let n = ring.len();
    let offset = |place: usize| (place + n - a0) % n;
    let (a1, b0, b1) = (offset(a1), offset(b0), offset(b1));
    if !(a1 < b0 && b0 <= b1) {
        return None;
    }
    // Each rail has 2 points or more: the first runs from `a1` on to `b0`, past it, and the second from `b1`,
    // before the ring's last place, on round to `a0`.
    let at = |o: usize| ring.get((a0 + o) % n).copied();
    let first: Vec<usize> = (a1..=b0).filter_map(at).collect();
    let mut second: Vec<usize> = (b1..=n).filter_map(at).collect();
    second.reverse();
    Some([first, second])
}

/// The column of the closed `line`, through the points of `found`, with the outline `outline`: its outer
/// and its inner ring the same way round; `None` when it isn't a ring.
fn ring(skeleton: &Skeleton, owner: &[usize], line: usize, outline: &Outline, found: Line, meter: &mut Meter) -> Result<Option<Column>, Exhausted> {
    let points = &skeleton.triangulation.points;
    let area = |ring: &[usize]| {
        let at = |i: usize| ring.get(i % ring.len()).and_then(|&p| points.get(p)).copied().unwrap_or(Point::ORIGIN);
        (0..ring.len()).map(|i| at(i).x() * at(i + 1).y() - at(i + 1).x() * at(i).y()).sum::<f64>()
    };
    let [one, other] = outline.rings.as_slice() else { return Ok(None) };
    let (outer, inner) = match (area(one) > 0.0, area(other) > 0.0) {
        (true, false) => (one, other),
        (false, true) => (other, one),
        _ => return Ok(None),
    };
    let closed = |ring: &[usize], back: bool| {
        let mut rail: Vec<usize> = ring.to_vec();
        if back {
            rail.reverse();
        }
        rail.extend(rail.first().copied());
        rail
    };
    let indices = [closed(outer, false), closed(inner, true)];
    let candidates = rungs(skeleton, owner, line, &indices, meter)?;
    let rails = indices.map(|rail| rail.iter().filter_map(|&i| points.get(i).copied()).collect::<Vec<_>>());
    let lengths = rails.each_ref().map(|rail| rail.windows(2).map(|w| w[0].distance(w[1])).sum());
    let ring = Ring { rails, lengths, rungs: candidates };
    let Some(&near) = found.points.first() else { return Ok(None) };
    let Some(Started { rails, rungs }) = started(&ring, near, meter)? else { return Ok(None) };
    Ok(Some(Column { rails, rungs, closed: true, line: found.clone(), walked: found, under: [None, None], ring: Some(ring) }))
}

/// `ring` started at the rung nearest `near` at which it can start: one whose point on the inner ring is
/// that ring's point nearest its point on the outer, so that a satin column starts the rings together there,
/// with `meter`'s work; `None` when it can start at none.
fn started(ring: &Ring, near: Point, meter: &mut Meter) -> Result<Option<Started>, Exhausted> {
    let mut by_distance: Vec<&Rung> = ring.rungs.iter().collect();
    meter.charge(u64::try_from(by_distance.len()).unwrap_or(u64::MAX))?;
    by_distance.sort_by(|a, b| a.points[0].distance(near).total_cmp(&b.points[0].distance(near)));
    for start in by_distance {
        meter.charge(u64::try_from(ring.rails[1].len()).unwrap_or(u64::MAX))?;
        let rails = [0, 1].map(|r| from(&ring.rails[r], start.at[r], start.points[r]));
        let [Some(outer), Some(inner)] = rails else { continue };
        let gap = start.points[1].distance(start.points[0]);
        // A satin column starts the inner ring at its point nearest the outer's start, the first of those as
        // near (`DEV-SAT-009`).
        if inner.iter().skip(1).any(|p| p.distance(start.points[0]) < gap) {
            continue;
        }
        let shifted: Vec<Rung> = ring
            .rungs
            .iter()
            .map(|rung| {
                let at = [0, 1].map(|r| (rung.at[r] - start.at[r]).rem_euclid(ring.lengths[r]));
                Rung { at, ..*rung }
            })
            .collect();
        let rungs = ordered(shifted, ring.lengths, Some(start.points));
        return Ok(Some(Started { rails: [outer, inner], rungs: rungs.iter().map(|rung| rung.points).collect() }));
    }
    Ok(None)
}

/// The ring `rail`, whose last point is its first, started at `point`, which lies `at` along it, and taken
/// round back to it; `None` for a rail of no length.
fn from(rail: &[Point], at: f64, point: Point) -> Option<Vec<Point>> {
    let mut gone = 0.0;
    for (k, pair) in rail.windows(2).enumerate() {
        let [a, b] = pair else { continue };
        let piece = a.distance(*b);
        if gone + piece >= at || k + 2 == rail.len() {
            let round = rail.get(..rail.len().saturating_sub(1))?;
            let after = round.get(k + 1..)?.iter().chain(round.get(..=k)?);
            let mut started = vec![point];
            started.extend(after.copied().filter(|p| p.distance(point) >= SAME_POINT));
            started.push(point);
            return Some(started);
        }
        gone += piece;
    }
    None
}

/// The length of the rail through `points`' points `rail`.
fn length(points: &[Point], rail: &[usize]) -> f64 {
    rail.windows(2).filter_map(|w| Some(points.get(w[0])?.distance(*points.get(w[1])?))).sum()
}

/// Every rung of `line`'s column, whose triangles `owner` says, between the rails through the points
/// `rails`, at most one from each corner, with `meter`'s work.
fn rungs(skeleton: &Skeleton, owner: &[usize], line: usize, rails: &[Vec<usize>; 2], meter: &mut Meter) -> Result<Vec<Rung>, Exhausted> {
    let points = &skeleton.triangulation.points;
    let point = |i: usize| points.get(i).copied();
    // Each rail's segments, either way round, with the rail and how far along it each starts; and each point's
    // places on the rails.
    let mut segments: BTreeMap<(usize, usize), (usize, usize, f64)> = BTreeMap::new();
    let mut on: BTreeMap<usize, Vec<(usize, usize, f64)>> = BTreeMap::new();
    for (r, rail) in rails.iter().enumerate() {
        let mut gone = 0.0;
        for (k, &i) in rail.iter().enumerate() {
            meter.charge(1)?;
            on.entry(i).or_default().push((r, k, gone));
            let (Some(&j), Some(a)) = (rail.get(k + 1), point(i)) else { continue };
            segments.insert(edge(i, j), (r, k, gone));
            gone += point(j).map_or(0.0, |b| a.distance(b));
        }
    }
    let mut best: BTreeMap<usize, Rung> = BTreeMap::new();
    for (t, triangle) in skeleton.triangulation.triangles.iter().enumerate() {
        meter.charge(1)?;
        if owner.get(t) != Some(&line) {
            continue;
        }
        let inside: Vec<bool> = (0..3).map(|k| triangle.across[k].is_some_and(|u| owner.get(u) == Some(&line))).collect();
        let [k] = (0..3).filter(|&k| !inside[k]).collect::<Vec<_>>()[..] else { continue };
        let (a, b) = triangle.edge(k);
        let Some(&(r, segment, start)) = segments.get(&edge(a, b)) else { continue };
        let apex = triangle.corners[k];
        let places = on.get(&apex).map_or(&[][..], Vec::as_slice);
        let Some(&(other, index, apex_at)) = places.iter().find(|(rail, ..)| *rail != r) else { continue };
        // A corner on both rails, where they meet at a point, gives no rung.
        let false = places.iter().any(|(rail, ..)| *rail == r) else { continue };
        let (Some(rail), Some(c)) = (rails.get(r), point(apex)) else { continue };
        let (Some(p), Some(q)) = (rail.get(segment).and_then(|&i| point(i)), rail.get(segment + 1).and_then(|&i| point(i))) else { continue };
        let (ex, ey) = (q.x() - p.x(), q.y() - p.y());
        // The column's way: the corner's rail's way at the corner, from the point before it to the one after,
        // and the edge's, each of length 1, added.
        let unit = |(x, y): (f64, f64)| {
            let length = math::hypot(x, y);
            if length > 0.0 { (x / length, y / length) } else { (0.0, 0.0) }
        };
        let Some(side) = rails.get(other) else { continue };
        let before = index.checked_sub(1).and_then(|i| side.get(i)).and_then(|&i| point(i)).unwrap_or(c);
        let after = side.get(index + 1).and_then(|&i| point(i)).unwrap_or(c);
        let (along, across) = (unit((after.x() - before.x(), after.y() - before.y())), unit((ex, ey)));
        let (dx, dy) = (along.0 + across.0, along.1 + across.1);
        // Where the line from the corner at a right angle to the column's way, along (-dy, dx), meets the
        // edge's line: p + u (q - p). An edge of a rail runs along the column: it lies square to the way, and
        // gives no rung, only where it has no length or the rails run opposite ways.
        let denominator = ex * dx + ey * dy;
        let span = math::hypot(ex, ey);
        if denominator.abs() <= 1e-12 * span * math::hypot(dx, dy) || span <= 0.0 {
            continue;
        }
        let u = ((c.x() - p.x()) * dx + (c.y() - p.y()) * dy) / denominator;
        let held = u.clamp(0.0, 1.0);
        let foot = p.lerp(q, held);
        let off = (u - held).abs() * span;
        let (points, at) = if r == 0 { ([foot, c], [start + held * span, apex_at]) } else { ([c, foot], [apex_at, start + held * span]) };
        let rung = Rung { points, at, off };
        if best.get(&apex).is_none_or(|kept| off < kept.off) {
            best.insert(apex, rung);
        }
    }
    Ok(best.into_values().collect())
}

/// `rungs` in order along rails `lengths` long, without those at a rail's ends, and of those whose points lie
/// too near each other on a rail ([`apart`]), or would cross, the one nearer a right angle, the first of as
/// near. A ring started at the rung `start` leaves that one out: its rails start there.
fn ordered(mut rungs: Vec<Rung>, lengths: [f64; 2], start: Option<[Point; 2]>) -> Vec<Rung> {
    rungs.retain(|rung| {
        let at_start = start.is_some_and(|points| points.iter().zip(&rung.points).all(|(a, b)| a.distance(*b) < SAME_POINT));
        !at_start && (0..2).all(|r| rung.at[r] >= SAME_POINT && rung.at[r] <= lengths[r] - SAME_POINT)
    });
    rungs.sort_by(|a, b| a.at[0].total_cmp(&b.at[0]).then(a.at[1].total_cmp(&b.at[1])));
    let mut kept: Vec<Rung> = Vec::with_capacity(rungs.len());
    for rung in rungs {
        match kept.last() {
            Some(last) if !apart(last, &rung) => {
                // The better one takes the last one's place where it lies far enough past the one before.
                let fits = kept.len() < 2 || kept.get(kept.len() - 2).is_some_and(|before| apart(before, &rung));
                if rung.off < last.off && fits {
                    kept.pop();
                    kept.push(rung);
                }
            }
            _ => kept.push(rung),
        }
    }
    kept
}

/// Whether rung `after` lies far enough past rung `before` on both rails: by twice the reach either is drawn
/// with past the rails, so that drawn, they don't cross, and by a millionth of a millimetre at least.
fn apart(before: &Rung, after: &Rung) -> bool {
    let length = |rung: &Rung| rung.points[0].distance(rung.points[1]);
    let gap = (2.0 * REACH * length(before).max(length(after))).max(SAME_POINT);
    (0..2).all(|r| after.at[r] >= before.at[r] + gap)
}

#[cfg(test)]
mod tests;
