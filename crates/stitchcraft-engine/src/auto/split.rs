//! The broad and the narrow parts of a part of an automatic area that is wider than a satin column in
//! places (`REQ-AUTO-019`): its broad parts stay its fill, and its narrow parts sew over the fill as lines
//! and columns, each sorted by its width as a part is (`auto::narrow`).
//!
//! Design: `docs/src/design/image-to-embroidery.md` › Shapes in parts. A shape traced from a picture is
//! often broad in places and narrow in others, as a balloon on its string is, and filled whole, its narrow
//! parts would sew rows no longer than they are wide.
//!
//! - **Its broad parts** are what an opening with a disc as wide as the widest satin column leaves of it:
//!   the part shrunk by the disc's radius and grown back by as much with the polygon buffer
//!   (`ops::buffered`), and kept within the part, whose outline the buffer's arcs follow only to within
//!   their steps. The radius is half of the area's widest satin column taken half a micrometre wider, as
//!   the bound is ([`narrow::half`]), so that a part as wide as a satin column has none.
//! - **Its narrow parts** are the rest of it, in pieces. A speck, a piece smaller than the area's smallest
//!   area, stays in the fill. So does a piece that reaches less than a speck beyond the disc's radius from
//!   the broad parts: the opening rounds each convex
//!   corner of a broad part and leaves the corner as such a piece, as it does a stub too short for a
//!   column. How far a piece reaches is measured against the shrunk part grown by twice the radius, which
//!   is the broad parts grown by it. A square corner's piece runs 6 mm along its centre line, less than half a
//!   millimetre wide, so its length tells no corner.
//! - **The cut.** Where a narrow part meets a broad one, the opening leaves an arc, and a column would end
//!   there in a point at one of its corners. The points of a piece's outline that lie off the part's outline
//!   are the arc's ([`ON`]), and each run of them is replaced by a chord across the piece, square to the way
//!   its sides run on from there ([`chord`]): from the end of the run that lies farther along that way, so
//!   that the wedge before the chord goes to the fill, or from end to end, which takes in the lens between
//!   the arc and the chord. Across a tapering point, the shortest chord would lie square to one side only,
//!   and the column's stitches would fan out from its end. A chord that would cross the outline of the piece
//!   or of the part is left out, and the arc stays.
//! - **The fill** is the part without its narrow parts, and is the part itself when it has none.
//!
//! Every step costs a unit of work, and the buffer and the region code cost theirs.

use stitchcraft_core::{Exhausted, Meter, Point, math};

use super::AutoParams;
use super::narrow::{self, Edges};
use crate::normalize::region::Polygon;
use crate::normalize::region::geom::{self, Hit};
use crate::normalize::region::ops::{self, area_of};
use crate::normalize::stroke::segments;

/// How near the part's outline a point of a piece lies to be on it, in millimetres. The region code puts
/// the points where an arc meets the outline on a grid far finer than this, and the opening's arcs leave
/// the outline by far more between their ends.
const ON: f64 = 1e-6;

/// A part split into its broad and its narrow parts.
#[derive(Clone, Debug, PartialEq)]
pub(super) struct Split {
    /// The part without its narrow parts: its broad parts, with the corners, stubs and specks that stay in
    /// them.
    pub(super) fill: Vec<Polygon>,
    /// Its narrow parts, each cut square where it meets the broad parts.
    pub(super) narrow: Vec<Piece>,
}

/// A narrow part of a part split, and where it was cut.
#[derive(Clone, Debug, PartialEq)]
pub(super) struct Piece {
    /// The narrow part.
    pub(super) part: Polygon,
    /// The chords it was cut along, where it meets the broad parts: a column's end along one lies along all
    /// of it (`normalize::strips::columns`).
    pub(super) cuts: Vec<[Point; 2]>,
}

/// `part` split into its broad and its narrow parts by its area's settings of the rules, `rules`, with
/// `meter`'s work; `None` when a region's edges can't be joined into rings, a bug.
pub(super) fn split(part: &Polygon, rules: &AutoParams, meter: &mut Meter) -> Result<Option<Split>, Exhausted> {
    let whole = std::slice::from_ref(part);
    let (radius, smallest) = (narrow::half(rules.auto_max_satin_width_mm), rules.auto_min_area_mm2);
    let Ok(shrunk) = ops::buffered(whole, -radius, meter)? else { return Ok(None) };
    let Ok(opened) = ops::buffered(&shrunk, radius, meter)? else { return Ok(None) };
    let Ok(broad) = ops::intersection(&opened, whole, meter)? else { return Ok(None) };
    let Ok(rest) = ops::difference(whole, &broad, meter)? else { return Ok(None) };
    let Ok(reach) = ops::buffered(&shrunk, 2.0 * radius, meter)? else { return Ok(None) };
    let edges = Edges::of(part, narrow::piece(part), meter)?;
    let mut pieces = Vec::new();
    for piece in rest {
        meter.charge(1)?;
        if piece.area() < smallest {
            continue;
        }
        let Ok(beyond) = ops::difference(std::slice::from_ref(&piece), &reach, meter)? else { return Ok(None) };
        if area_of(&beyond) < smallest {
            continue;
        }
        pieces.push(cut(piece, part, &edges, meter)?);
    }
    if pieces.is_empty() {
        return Ok(Some(Split { fill: vec![part.clone()], narrow: pieces }));
    }
    let parts: Vec<Polygon> = pieces.iter().map(|piece| piece.part.clone()).collect();
    let Ok(fill) = ops::difference(whole, &parts, meter)? else { return Ok(None) };
    Ok(Some(Split { fill, narrow: pieces }))
}

/// `piece`, a narrow part of `part`, whose outline `edges` holds, cut square where it meets the broad
/// parts, with `meter`'s work: each run of the points of its outline that lie off the part's outline
/// replaced by a [`chord`]. A run with no chord across it, or with no point on the part's outline beside it,
/// stays.
fn cut(mut piece: Polygon, part: &Polygon, edges: &Edges, meter: &mut Meter) -> Result<Piece, Exhausted> {
    let mut ring = piece.outline.clone();
    ring.pop();
    let mut stays: Vec<Point> = Vec::new();
    let mut cuts = Vec::new();
    loop {
        meter.charge(1)?;
        let mut off = Vec::with_capacity(ring.len());
        for &q in &ring {
            off.push(edges.nearest(q, meter)? > ON);
        }
        let Some((before, after)) = run(&off, &ring, &stays) else { break };
        match chord(&ring, before, after, part, meter)? {
            Some(cut) => {
                cuts.extend(cut.last().zip(cut.first()).map(|(&from, &to)| [from, to]));
                ring = cut;
            }
            None => stays.extend(ring.get(before).copied()),
        }
    }
    piece.outline = geom::closed(&ring);
    Ok(Piece { part: piece, cuts })
}

/// The first run of the points of `ring` that `off` says lie off the part's outline, and that doesn't follow
/// a point of `stays`: the places in `ring` of the points on the outline just before it and just after it.
/// `None` when there is none, or when no other point is on the outline.
fn run(off: &[bool], ring: &[Point], stays: &[Point]) -> Option<(usize, usize)> {
    let n = off.len();
    let on = |i: usize| off.get(i % n).is_some_and(|&off| !off);
    (0..n).find_map(|before| {
        let starts = on(before) && !on(before + 1) && ring.get(before).is_some_and(|q| !stays.contains(q));
        let after = (before + 2..before + n).find(|&i| on(i)).filter(|_| starts)?;
        Some((before, after % n))
    })
}

/// `ring` with the run of points between its places `before` and `after` replaced by a chord across the
/// narrow part, with `meter`'s work: the chord [`square`] to the way the part runs on, and else, or where it
/// would cross the new ring or `part`'s outline, the chord that joins the run's ends, which takes in the lens
/// between it and the run. `None` when neither is left, or when the ring keeps no more than the run's ends.
fn chord(ring: &[Point], before: usize, after: usize, part: &Polygon, meter: &mut Meter) -> Result<Option<Vec<Point>>, Exhausted> {
    // The ring from the point after the run round to the one before it: a ring whose last edge, back to its
    // first point, is the chord that joins the run's ends.
    let n = ring.len();
    let steps = (before + n - after) % n;
    let kept: Vec<Point> = (0..=steps).filter_map(|k| ring.get((after + k) % n).copied()).collect();
    if kept.len() < 3 {
        return Ok(None);
    }
    for cut in square(&kept, steps).into_iter().chain(std::iter::once(kept)) {
        meter.charge(1)?;
        if crosses_nothing(&cut, part, meter)? {
            return Ok(Some(cut));
        }
    }
    Ok(None)
}

/// `kept`, the ring after a run up to the point before it, `steps` points round from its first, closed by a
/// chord square to the way the part runs on from the run: the mean of its 2 sides' ways, each from the run's
/// end to the first point of its side at least half the run's own chord away. The chord runs from the end of
/// the run that lies farther along that way to the other side, no farther than twice the run's chord from
/// that end, and what lies between the chord and the run goes to the fill. `None` when the ends lie as far
/// along the way, within [`ON`], or the other side reaches as far within that distance nowhere.
fn square(kept: &[Point], steps: usize) -> Option<Vec<Point>> {
    let (&a, &b) = (kept.last()?, kept.first()?);
    let across = a.distance(b);
    let (on_a, on_b) = (way(a, kept.iter().rev().copied(), across / 2.0)?, way(b, kept.iter().copied(), across / 2.0)?);
    let along = math::hypot(on_a.0 + on_b.0, on_a.1 + on_b.1);
    let depth = |q: Point| (q.x() * (on_a.0 + on_b.0) + q.y() * (on_a.1 + on_b.1)) / along;
    let within = |from: Point, q: Point| q.distance(from) <= 2.0 * across;
    if depth(a) > depth(b) + ON {
        // From the run's start square across to the side that runs on from its end.
        let edges = kept.windows(2).map(|w| (w[0], w[1])).take_while(|&(from, _)| within(a, from));
        let (i, foot) = crossing(depth(a), &depth, edges)?;
        Some(std::iter::once(foot).chain(kept.iter().skip(i + 1).copied().filter(|&q| q != foot)).collect())
    } else if depth(b) > depth(a) + ON {
        // From the run's end square across to the side that runs back from its start.
        let edges = kept.windows(2).rev().map(|w| (w[1], w[0])).take_while(|&(from, _)| within(b, from));
        let (i, foot) = crossing(depth(b), &depth, edges)?;
        Some(kept.iter().take(steps - i).copied().filter(|&q| q != foot).chain(std::iter::once(foot)).collect())
    } else {
        None
    }
}

/// The way a side of the outline runs from `from`, through the points of `side` in order: to the first at
/// least `reach` away, with a length of 1. `None` when none is.
fn way(from: Point, mut side: impl Iterator<Item = Point>, reach: f64) -> Option<(f64, f64)> {
    let to = side.find(|q| q.distance(from) >= reach)?;
    let length = from.distance(to);
    Some(((to.x() - from.x()) / length, (to.y() - from.y()) / length))
}

/// The first of `edges` that runs from no farther along the way than `level`, as `depth` measures it, to as
/// far or farther: its place among them, and the point where it reaches `level`.
fn crossing(level: f64, depth: &impl Fn(Point) -> f64, edges: impl Iterator<Item = (Point, Point)>) -> Option<(usize, Point)> {
    edges.enumerate().find_map(|(i, (from, to))| {
        let (s, t) = (depth(from) - level, depth(to) - level);
        (s <= 0.0 && t >= 0.0 && t > s).then(|| (i, from.lerp(to, -s / (t - s))))
    })
}

/// Whether the chord that closes `ring`, from its last point back to its first, crosses none of `ring`'s other
/// edges and none of `part`'s, with `meter`'s work. Its ends lie on the part's outline, so only the chord
/// between a hundredth of its length from each end is tried against the part's.
fn crosses_nothing(ring: &[Point], part: &Polygon, meter: &mut Meter) -> Result<bool, Exhausted> {
    let (Some(&from), Some(&to)) = (ring.last(), ring.first()) else { return Ok(false) };
    let inner = (from.lerp(to, 0.01), from.lerp(to, 0.99));
    for (u, v) in ring.windows(2).map(|w| (w[0], w[1])).chain(part.rings().flat_map(segments)) {
        meter.charge(1)?;
        let shares = [u, v].contains(&from) || [u, v].contains(&to);
        if !shares && geom::hit(inner.0, inner.1, u, v) != Hit::Apart {
            return Ok(false);
        }
    }
    Ok(true)
}

#[cfg(test)]
mod tests;
