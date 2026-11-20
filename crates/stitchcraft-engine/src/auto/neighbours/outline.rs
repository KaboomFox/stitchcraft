//! The outline round a fill (`REQ-AUTO-051`): a running stitch along the edge of each automatic area where the
//! edge shows, sewn after the area's rows in the area's own thread.
//!
//! Design: `docs/src/design/image-to-embroidery.md` › An outline round a fill. A tatami fill's rows end at its
//! edge, each a stitch's width from the next, and the thread pulls each row's end in. On a small shape with
//! points, a star 5 mm across, the rows' ends are most of what shows, and its points sew blunt. A digitizer
//! runs a stitch round such a shape after filling it, which draws the edge as one line. A design asks for it
//! with `auto_fill_outline`.
//!
//! - **Where.** The edge of an area is the rings of its region as the layers and the specks left it, before
//!   it reaches under its neighbours: what shows of it. A shape sewn after the area that touches its edge
//!   covers the edge there, by the overlap the area reaches under it, and a run there would lie under that
//!   shape's stitches and thicken its edge. So the outline leaves out the edge within [`TOUCHING`] of each
//!   shape sewn after the area, and of each other element of the area's own shape, whose columns and lines
//!   are the same shape going on. A line digitized from a stroke goes over an area and covers none of it.
//! - **How.** Each edge is cut into pieces no longer than [`STEP`], and a piece shows when its middle lies in
//!   none of those shapes. The pieces that show in a row are one line, a ring's when all of it shows, and a
//!   line shorter than [`SHORTEST`] is left out: it would sew a knot beside its neighbour's edge.
//! - **Through.** A stretch of the edge under those shapes no longer than [`BRIDGE`] is sewn through: the
//!   needle goes on along the edge, under the shape that covers it, and the lines on both sides are one, or
//!   the ring closes. Left out, such a stretch parts 2 lines by a jump, a thread to cut by hand, and leaves
//!   the outline 2 ends where a ring can start and end anywhere. An edge that shows for less than
//!   [`SHORTEST`] in all is left as it is, with no outline.
//! - **Not through a fill's seam.** A stretch under a fill sewn after the area is sewn through only when it is
//!   no longer than [`FILL_BRIDGE`] (`REQ-AUTO-076`). The run would lie along the seam where that fill's rows
//!   end over the area's, the fill's own outline beside it, and the seams of the traced dogs are where most of
//!   their thread piles up. The fill hides the stretch, so leaving it out changes nothing that shows.
//! - **Where it ends.** The fill before an outline ends where the outline starts, as every fill ends near the
//!   element after it. The outline's last line is the one with an end, or a corner when it is a ring, nearest
//!   where the element sewn next starts, and it ends there ([`ending_near`]). A ring starts and ends at one
//!   point, so a fill outlined whole is left where it would be left without its outline.
//! - **Work.** Each area has the budget's work to itself: a unit for each shape it looks past, the work of
//!   growing those near, and for each piece a unit for each corner of the shapes whose bounds hold its middle.
//!   Finding where an outline ends costs a unit for each corner of what the next element offers, for each end
//!   or corner looked at. An area that needs more sews no outline, and that is said (`SC-I0904`).

use stitchcraft_core::{Budget, Code, Diagnostic, ElementId, Exhausted, Meter, Point};

use super::{Failed, Kind, Shapes, TOUCHING, envelope, near};
use crate::generators::Approach;
use crate::normalize::region::Polygon;
use crate::normalize::region::geom::{self, Envelope, Location};
use crate::normalize::strips::Line;
use crate::normalize::stroke::distance_to_segment;

/// The longest piece an edge is cut into to find where it shows, in millimetres: an outline starts and ends
/// within this of where a neighbour takes the edge over, a fifth of a running stitch.
pub(super) const STEP: f64 = 0.5;

/// The shortest line of an outline that sews, in millimetres: 2 running stitches. A shorter one is a knot.
pub(super) const SHORTEST: f64 = 3.0;

/// The longest stretch of an edge under other shapes that an outline sews through, in millimetres: 10 running
/// stitches under the shape that covers it, in place of a jump and its thread to cut.
pub(super) const BRIDGE: f64 = 25.0;

/// The longest stretch of an edge under a fill sewn after the area that an outline sews through, in
/// millimetres (`REQ-AUTO-076`). Such a stretch lies along the seam where the fill's rows end over the area,
/// and its thread thickens the seam, so it is left out where that costs little: a short stretch lays little
/// thread and would cost a jump all the same. Measured on the 4 traced dogs on 2026-11-17, leaving out every
/// stretch under a later fill took their area at 3 fills or more down by 12 to 19 % for 3 to 11 jump threads
/// more each, and leaving out those longer than 12 mm by 6 to 12 % for 1 to 3 more.
pub(super) const FILL_BRIDGE: f64 = 12.0;

/// The sine of the turn below which 2 sides of a line run straight on when an outline moves in
/// (`REQ-AUTO-130`).
const STRAIGHT: f64 = 1e-12;

/// The cosine of the widest step of an arc round an inward corner when an outline moves in: 22.5 degrees,
/// 0.04 mm off the arc at the distance 0.1 mm.
const ARC_STEP: f64 = 0.923_879_532_511_286_7;

/// The most times a step of an arc is halved: a half turn takes 3.
const ARC_DEPTH: u32 = 4;

/// For each automatic area of `shapes` whose edge shows somewhere, in the order they sew, its id and the lines
/// of its outline, each area with `budget`'s work to itself; and `SC-I0904` for each area that needed more,
/// which sews no outline.
pub(super) fn outlines(shapes: &mut Shapes, budget: &Budget) -> (Vec<(ElementId, Vec<Line>)>, Vec<Diagnostic>) {
    let (mut found, mut said) = (Vec::new(), Vec::new());
    for index in 0..shapes.len() {
        let (Some(id), Some(region)) = (shapes.id(index), shapes.region(index).map(<[Polygon]>::to_vec)) else { continue };
        match shown(shapes, index, &region, &mut budget.meter()) {
            Ok(lines) if lines.is_empty() => {}
            Ok(lines) => found.push((id, lines)),
            Err(failed) => {
                let (finding, consequence) = ("where this area's edge meets the shapes sewn after it", "it sews no outline");
                said.push(failed.said(Code::AutoUnder, finding, consequence, budget.max_work, id));
            }
        }
    }
    (found, said)
}

/// The lines of the outline of the automatic area `index` of `shapes`, whose region is `region`: its edge
/// outside the shapes sewn after it and the other elements of its own shape, with `meter`'s work.
fn shown(shapes: &mut Shapes, index: usize, region: &[Polygon], meter: &mut Meter) -> Result<Vec<Line>, Failed> {
    let Some(bounds) = envelope(region) else { return Ok(Vec::new()) };
    // The fills sewn after the area first, so that a piece under one of them is found to be.
    let (mut fills, mut others) = (Vec::new(), Vec::new());
    for other in 0..shapes.len() {
        meter.charge(1)?;
        let (later, kind) = (other > index, shapes.kind(other));
        let own = other != index && shapes.of_shape.get(other) == shapes.of_shape.get(index);
        let covers = (later || own) && kind != Some(Kind::Line);
        if !covers || !shapes.bounds(other).is_some_and(|theirs| near(&bounds, TOUCHING, &theirs)) {
            continue;
        }
        let grown = shapes.grown(other, meter)?.unwrap_or_default();
        if later && kind == Some(Kind::Area) { fills.extend(grown) } else { others.extend(grown) }
    }
    let covering: Vec<(Envelope, &Polygon)> = fills.iter().chain(&others).map(|part| (Envelope::of(&part.outline), part)).collect();
    let mut lines = Vec::new();
    for ring in region.iter().flat_map(Polygon::rings) {
        lines.extend(uncovered(ring, &covering, fills.len(), meter)?);
    }
    Ok(lines)
}

/// Whether `p` lies in one of `parts`, each with its bounds, on its edge included, with `meter`'s work: `None`
/// where it doesn't, and whether it lies in one of the first `fills` of them where it does.
fn covered(p: Point, parts: &[(Envelope, &Polygon)], fills: usize, meter: &mut Meter) -> Result<Option<bool>, Failed> {
    let here = Envelope::of(&[p]);
    for (at, (bounds, part)) in parts.iter().enumerate() {
        meter.charge(1)?;
        if !bounds.covers(&here) {
            continue;
        }
        meter.charge(u64::try_from(part.rings().map(<[Point]>::len).sum::<usize>()).unwrap_or(u64::MAX))?;
        let inside = geom::locate_in_ring(p, &part.outline) != Location::Exterior;
        if inside && !part.holes.iter().any(|hole| geom::locate_in_ring(p, hole) == Location::Interior) {
            return Ok(Some(at < fills));
        }
    }
    Ok(None)
}

/// The lines along the closed `ring` that lie outside `parts`, the first `fills` of them fills sewn after the
/// area: the ring cut into pieces no longer than [`STEP`], those whose middles `parts` don't hold joined in
/// rows, with the hidden rows between them that [`through`] shows, the ring whole when all of it shows, and the
/// lines shorter than [`SHORTEST`] left out. A line keeps the ring's corners, and its ends lie where a piece
/// ends.
fn uncovered(ring: &[Point], parts: &[(Envelope, &Polygon)], fills: usize, meter: &mut Meter) -> Result<Vec<Line>, Failed> {
    // Each piece: where it ends, whether that is a corner of the ring, whether it shows, and whether a fill
    // sewn after the area hides it.
    let mut pieces = Vec::new();
    let Some(&start) = ring.first() else { return Ok(Vec::new()) };
    for pair in ring.windows(2) {
        let (Some(&a), Some(&b)) = (pair.first(), pair.last()) else { continue };
        let count = (a.distance(b) / STEP).ceil().max(1.0);
        let mut from = a;
        let mut k = 1.0;
        while k <= count {
            let to = if k == count { b } else { a.lerp(b, k / count) };
            let under = covered(from.lerp(to, 0.5), parts, fills, meter)?;
            pieces.push((to, k == count, under.is_none(), under == Some(true)));
            (from, k) = (to, k + 1.0);
        }
    }
    through(&mut pieces, meter)?;
    let Some(hidden) = pieces.iter().position(|&(_, _, shows, _)| !shows) else {
        // All of it shows: the ring, by its corners.
        let points: Vec<Point> = pieces.iter().filter(|&&(_, corner, ..)| corner).map(|&(to, ..)| to).collect();
        let line = Line { points, closed: true };
        return Ok(if line.length() >= SHORTEST { vec![line] } else { Vec::new() });
    };
    // From the piece after a hidden one round to it again, so that no line is cut where the ring starts.
    let mut lines = Vec::new();
    let mut points: Vec<Point> = Vec::new();
    let mut from = pieces.get(hidden).map_or(start, |&(to, ..)| to);
    for step in 1..=pieces.len() {
        let Some(&(to, corner, shows, _)) = pieces.get((hidden + step) % pieces.len()) else { continue };
        if shows {
            if points.is_empty() {
                points.push(from);
            }
            if corner {
                points.push(to);
            }
        } else if !points.is_empty() {
            if points.last() != Some(&from) {
                points.push(from);
            }
            lines.push(Line { points: std::mem::take(&mut points), closed: false });
        }
        from = to;
    }
    lines.retain(|line| line.length() >= SHORTEST);
    Ok(lines)
}

/// Shows each row of hidden `pieces` of a ring that is no longer than [`BRIDGE`], and no longer than
/// [`FILL_BRIDGE`] when a fill sewn after the area hides one of its pieces, with `meter`'s work: the lines on
/// both sides of it are one line, or the ring closes. A ring that shows for less than [`SHORTEST`] in all, or
/// not at all, stays as it is.
fn through(pieces: &mut [(Point, bool, bool, bool)], meter: &mut Meter) -> Result<(), Failed> {
    let count = pieces.len();
    let Some(shown) = pieces.iter().position(|&(_, _, shows, _)| shows) else { return Ok(()) };
    // Each hidden row, its pieces, its length and whether a later fill hides some of it, from the piece after
    // one that shows round to that piece again, so that no row is cut where the pieces start.
    let (mut rows, mut hidden, mut showing) = (Vec::<(Vec<usize>, f64, bool)>::new(), false, 0.0);
    for step in 1..=count {
        meter.charge(1)?;
        let at = (shown + step) % count;
        let (Some(&(from, ..)), Some(&(to, _, shows, fill))) = (pieces.get((at + count - 1) % count), pieces.get(at)) else { continue };
        let length = from.distance(to);
        if shows {
            showing += length;
        } else {
            if !hidden {
                rows.push((Vec::new(), 0.0, false));
            }
            if let Some((row, sum, under_fill)) = rows.last_mut() {
                row.push(at);
                *sum += length;
                *under_fill |= fill;
            }
        }
        hidden = !shows;
    }
    if showing < SHORTEST {
        return Ok(());
    }
    let sewn_through = |&&(_, length, fill): &&(Vec<usize>, f64, bool)| length <= BRIDGE && (!fill || length <= FILL_BRIDGE);
    for at in rows.iter().filter(sewn_through).flat_map(|(row, ..)| row) {
        if let Some(piece) = pieces.get_mut(*at) {
            piece.2 = true;
        }
    }
    Ok(())
}

/// `lines`, the outline of the area `id`, with the line that comes nearest `target` last, ending there: what
/// the element sewn next offers to end near. An open line comes as near as its nearer end, which it ends at,
/// and a ring as near as its nearest corner, where it starts and ends. Each end or corner looked at costs
/// `budget` a unit for each corner of `target`. An outline that needs more work is not sewn, and `Err` says so
/// (`SC-I0904`).
pub(in crate::auto) fn ending_near(mut lines: Vec<Line>, target: &Approach, budget: &Budget, id: &ElementId) -> Result<Vec<Line>, Diagnostic> {
    let mut meter = budget.meter();
    let mut nearest: Option<(f64, usize, usize)> = None;
    for (at, line) in lines.iter().enumerate() {
        let last = line.points.len().saturating_sub(1);
        for (corner, &point) in line.points.iter().enumerate().filter(|(corner, _)| line.closed || [0, last].contains(corner)) {
            let away = away(point, target, &mut meter).map_err(|_| {
                let (finding, consequence) = ("where this area's outline ends nearest the shape sewn next", "it sews no outline");
                Failed::Work.said(Code::AutoUnder, finding, consequence, budget.max_work, id.clone())
            })?;
            if nearest.is_none_or(|(least, ..)| away < least) {
                nearest = Some((away, at, corner));
            }
        }
    }
    let Some((_, at, corner)) = nearest else { return Ok(lines) };
    let mut last = lines.remove(at);
    if last.closed {
        last.points.rotate_left(corner);
    } else if corner == 0 {
        last.points.reverse();
    }
    lines.push(last);
    Ok(lines)
}

/// `lines`, the outline of the area `id` along its edge as [`outlines`] found it, moved `by` into the area
/// (`REQ-AUTO-130`), with `budget`'s work ([`moved_right`]). A ring stays a ring, and a line that can't be
/// moved keeps to its edge. A line shorter than [`SHORTEST`] is left out. A ring's line runs as the ring
/// does, so the area lies on its right with y up, the outline's clockwise and a hole's the other way. An
/// outline that needs more work is not sewn, and `Err` says so (`SC-I0904`).
pub(in crate::auto) fn inset(lines: Vec<Line>, by: f64, budget: &Budget, id: &ElementId) -> Result<Vec<Line>, Diagnostic> {
    let mut meter = budget.meter();
    let mut moved = Vec::with_capacity(lines.len());
    for line in lines {
        let edge = if line.closed { opened(&line.points) } else { line.points.clone() };
        let curve = moved_right(&edge, by, &mut meter).map_err(|_| {
            let (finding, consequence) = ("where this area's outline runs inside its edge", "it sews no outline");
            Failed::Work.said(Code::AutoUnder, finding, consequence, budget.max_work, id.clone())
        })?;
        moved.push(match curve {
            // A ring's line runs from beside the opening round to beside it again: its last point is its first.
            Some(mut curve) => {
                if line.closed {
                    curve.pop();
                }
                Line { points: curve, closed: line.closed }
            }
            None => line,
        });
    }
    moved.retain(|line| line.length() >= SHORTEST);
    Ok(moved)
}

/// A side of a line: where it starts and ends, its direction, and its normal to the right with y up, both
/// a millimetre long.
#[derive(Clone, Copy)]
struct Side {
    from: Point,
    to: Point,
    along: (f64, f64),
    right: (f64, f64),
}

/// `p` moved `by` millimetres along `normal`.
fn shifted(p: Point, (x, y): (f64, f64), by: f64) -> Point {
    Point::new(p.x() + x * by, p.y() + y * by).unwrap_or(p)
}

/// The line `points` moved `by` to its right with y up, with `meter`'s work, a unit for each side: each
/// side moved, the moved sides of a corner that turns right meeting where they cross, and those of a corner
/// that turns left joined by an arc round the corner `by` from it. A side whose moved line falls behind the
/// point where it would start is left out, and its neighbours meet. `None` where the line has no side, where
/// its first or last side would be left out, where a side left out starts at an arc, or where the sides on
/// both sides of one left out don't turn right.
fn moved_right(points: &[Point], by: f64, meter: &mut Meter) -> Result<Option<Vec<Point>>, Exhausted> {
    let mut sides = Vec::with_capacity(points.len());
    for pair in points.windows(2) {
        meter.charge(1)?;
        let (Some(&from), Some(&to)) = (pair.first(), pair.last()) else { continue };
        let length = from.distance(to);
        if length > 0.0 {
            let along = ((to.x() - from.x()) / length, (to.y() - from.y()) / length);
            sides.push(Side { from, to, along, right: (along.1, -along.0) });
        }
    }
    let Some(&first) = sides.first() else { return Ok(None) };
    let mut curve = vec![shifted(first.from, first.right, by)];
    // The sides kept so far: each, where on the curve its moved line starts, and whether that is an arc's end.
    let mut kept: Vec<(Side, usize, bool)> = vec![(first, 0, false)];
    for &side in sides.iter().skip(1) {
        let mut dropped = false;
        loop {
            meter.charge(1)?;
            let (Some(&(top, at, arced)), Some(&start)) = (kept.last(), curve.get(kept.last().map_or(0, |k| k.1))) else { return Ok(None) };
            let turn = top.along.0 * side.along.1 - top.along.1 * side.along.0;
            let ahead = top.along.0 * side.along.0 + top.along.1 * side.along.1;
            if dropped && turn >= -STRAIGHT {
                // The sides on both sides of one left out don't cross: the line folds over itself there.
                return Ok(None);
            }
            if turn > STRAIGHT || (turn >= -STRAIGHT && ahead < 0.0) {
                // A left turn, or straight back: round the corner.
                curve.truncate(at + 1);
                curve.push(shifted(side.from, top.right, by));
                arc(&mut curve, side.from, (top.right, side.right), top.along, by, 0);
                curve.push(shifted(side.from, side.right, by));
                kept.push((side, curve.len() - 1, true));
                break;
            }
            if turn >= -STRAIGHT {
                // Straight on: the side goes on along the same moved line.
                kept.push((side, at, arced));
                break;
            }
            // A right turn: the 2 moved lines cross. A side whose crossing lies behind its start is left out.
            let next = shifted(side.from, side.right, by);
            let across = ((next.x() - start.x()) * side.along.1 - (next.y() - start.y()) * side.along.0) / turn;
            if across >= 0.0 {
                curve.truncate(at + 1);
                curve.push(Point::new(start.x() + top.along.0 * across, start.y() + top.along.1 * across).unwrap_or(next));
                kept.push((side, curve.len() - 1, false));
                break;
            }
            kept.pop();
            if arced || kept.is_empty() {
                return Ok(None);
            }
            curve.truncate(at);
            dropped = true;
        }
    }
    let (Some(&(last, at, _)), Some(&start)) = (kept.last(), curve.get(kept.last().map_or(0, |k| k.1))) else { return Ok(None) };
    let end = shifted(last.to, last.right, by);
    if (end.x() - start.x()) * last.along.0 + (end.y() - start.y()) * last.along.1 < 0.0 {
        return Ok(None);
    }
    curve.truncate(at + 1);
    curve.push(end);
    Ok(Some(curve))
}

/// Adds to `curve` the points of the arc round `corner`, `by` from it, between the directions `from` and
/// `to`, turning left, each step no wider than [`ARC_STEP`]: halved, through `forward` when the 2 are
/// opposite, `depth` halvings so far.
fn arc(curve: &mut Vec<Point>, corner: Point, (from, to): ((f64, f64), (f64, f64)), forward: (f64, f64), by: f64, depth: u32) {
    if from.0 * to.0 + from.1 * to.1 >= ARC_STEP || depth >= ARC_DEPTH {
        return;
    }
    let (x, y) = (from.0 + to.0, from.1 + to.1);
    let length = stitchcraft_core::math::hypot(x, y);
    let middle = if length > STRAIGHT { (x / length, y / length) } else { forward };
    arc(curve, corner, (from, middle), forward, by, depth + 1);
    curve.push(shifted(corner, middle, by));
    arc(curve, corner, (middle, to), forward, by, depth + 1);
}

/// The ring `points`, its last point joined to its first, as an open line from the middle of its longest
/// side round to it again: a start where the ring doesn't turn, so that its offset needs no join there.
fn opened(points: &[Point]) -> Vec<Point> {
    let count = points.len();
    let side = |k: usize| points.get(k).zip(points.get((k + 1) % count)).map(|(&a, &b)| (k, a, b));
    let longer =
        |best: (usize, Point, Point), next: (usize, Point, Point)| if next.1.distance(next.2) > best.1.distance(best.2) { next } else { best };
    let Some((k, a, b)) = (0..count).filter_map(side).reduce(longer) else { return points.to_vec() };
    let middle = a.lerp(b, 0.5);
    std::iter::once(middle).chain(points.iter().cycle().skip(k + 1).take(count).copied()).chain([middle]).collect()
}

/// How far `p` is from where `target` lets an element start, in millimetres: its point, or the nearest point
/// of its lines or of its area's rings, with `meter`'s work.
fn away(p: Point, target: &Approach, meter: &mut Meter) -> Result<f64, Exhausted> {
    let lines: Vec<&[Point]> = match target {
        Approach::Point(q) => return Ok(p.distance(*q)),
        Approach::Shape(lines) => lines.iter().map(Vec::as_slice).collect(),
        Approach::Area(parts) => parts.iter().flat_map(Polygon::rings).collect(),
    };
    let mut least = f64::INFINITY;
    for line in lines {
        meter.charge(u64::try_from(line.len()).unwrap_or(u64::MAX))?;
        for pair in line.windows(2) {
            if let (Some(&a), Some(&b)) = (pair.first(), pair.last()) {
                least = least.min(distance_to_segment(p, a, b));
            }
        }
    }
    Ok(least)
}

#[cfg(test)]
mod tests {
    use stitchcraft_core::{Budget, Mm};
    use stitchcraft_params::ParamSet;

    use super::super::tests::{filled, square};
    use super::super::{AutoParams, Sewn};
    use super::*;
    use crate::design::{Cap, DesignSettings, Element, Join, Path, Segment, Shape, Subpath};

    fn p(x: f64, y: f64) -> Point {
        Point::new(x, y).unwrap()
    }

    /// An automatic area `id`, `part`, of the shape at `shape` in the design.
    fn area(id: &str, part: Polygon, shape: usize) -> Sewn {
        Sewn { element: filled(id, &[part]), area: Some(AutoParams::defaults()), automatic: true, reach: None, shape, part: None }
    }

    /// An area's id, and each line of its outline: its length to the micrometre, and whether it closes.
    type Outlined = (String, Vec<(f64, bool)>);

    /// The outlines of `sewn`, by area, found with `budget`, and how much was said.
    fn outlined(sewn: Vec<Sewn>, budget: &Budget) -> (Vec<Outlined>, usize) {
        let mut shapes = Shapes::new(sewn, &DesignSettings::default(), &Budget::DEFAULT);
        let (found, said) = outlines(&mut shapes, budget);
        assert!(said.iter().all(|said| said.code == Code::AutoUnder && said.message.ends_with("so it sews no outline.")), "{said:?}");
        let lines = |lines: &[Line]| lines.iter().map(|line| ((line.length() * 1000.0).round() / 1000.0, line.closed)).collect();
        (found.iter().map(|(id, found)| (id.as_str().to_string(), lines(found))).collect(), said.len())
    }

    /// [`outlined`] with the default budget, of which nothing is said.
    fn outlines_of(sewn: Vec<Sewn>) -> Vec<Outlined> {
        let (found, said) = outlined(sewn, &Budget::DEFAULT);
        assert_eq!(said, 0);
        found
    }

    /// 2 opposite corners of a square, in millimetres.
    type Corners = ((f64, f64), (f64, f64));

    /// A satin column `id` of the shape at `shape`, between 2 rails that run up from `(x0, y0)` and `(x1, y0)`
    /// to `y1`: it paints the box between those corners.
    fn column(id: &str, ((x0, y0), (x1, y1)): Corners, shape: usize) -> Sewn {
        let rail = |x: f64| Subpath { start: p(x, y0), segments: vec![Segment::Line(p(x, y1))], closed: false };
        let stroke = Shape::stroke(Path { subpaths: vec![rail(x0), rail(x1)] });
        let params = [("satin_column", "true")].into_iter().collect();
        Sewn { element: Element { shape: stroke, params, ..filled(id, &[]) }, area: None, automatic: true, reach: None, shape, part: None }
    }

    /// The lines of the outline of a square 64 mm across, with a satin column sewn after it over each of
    /// `notches`.
    fn notched(notches: &[Corners]) -> Vec<Line> {
        let sewn = notches.iter().enumerate().map(|(at, corners)| column(&format!("n{at}"), *corners, at + 1)).collect();
        notched_by(sewn)
    }

    /// The lines of the outline of a square 64 mm across, sewn before `after`.
    fn notched_by(after: Vec<Sewn>) -> Vec<Line> {
        let mut sewn = vec![area("a", square((0.0, 0.0), (64.0, 64.0)), 0)];
        sewn.extend(after);
        let mut shapes = Shapes::new(sewn, &DesignSettings::default(), &Budget::DEFAULT);
        let (mut found, said) = outlines(&mut shapes, &Budget::DEFAULT);
        assert!(said.is_empty() && found[0].0.as_str() == "a", "{said:?}");
        found.remove(0).1
    }

    fn one(id: &str, length: f64, closed: bool) -> Outlined {
        (id.to_string(), vec![(length, closed)])
    }

    #[test]
    fn req_auto_051_an_area_s_outline_runs_round_its_edge_where_no_shape_sewn_after_it_covers_it() {
        let left = square((0.0, 0.0), (10.0, 10.0));
        // Alone, a square is outlined whole: its ring, by its 4 corners.
        let mut shapes = Shapes::new(vec![area("a", left.clone(), 0)], &DesignSettings::default(), &Budget::DEFAULT);
        let (found, said) = outlines(&mut shapes, &Budget::DEFAULT);
        let [(id, lines)] = found.as_slice() else { panic!("{found:?}") };
        assert_eq!((id.as_str(), said.len()), ("a", 0));
        assert_eq!(lines, &[Line { points: vec![p(0.0, 10.0), p(10.0, 10.0), p(10.0, 0.0), p(0.0, 0.0)], closed: true }]);
        // Beside a square sewn after it, a square 64 mm across leaves out the edge they share: the later
        // square covers that edge, and is outlined whole itself.
        let (large, beside) = (square((0.0, 0.0), (64.0, 64.0)), square((64.0, 0.0), (128.0, 64.0)));
        let pair = outlines_of(vec![area("a", large.clone(), 0), area("b", beside.clone(), 1)]);
        assert_eq!(pair, [one("a", 192.0, false), one("b", 256.0, true)]);
        // 2 elements of one shape leave the edge between them out both: the shape goes on across it.
        let own = outlines_of(vec![area("a", large, 0), area("b", beside, 0)]);
        assert_eq!(own, [one("a", 192.0, false), one("b", 192.0, false)]);
        // A line that starts where the ring does isn't cut there: a square that covers 25.3 mm of the middle of
        // the first side hides the 51 pieces of 0.5 mm whose middles it holds, and leaves one line of 230.5 mm,
        // from one of its sides round to the other.
        let cut = notched(&[((-1.0, 19.6), (1.0, 44.9))]);
        let line = &cut[0];
        assert_eq!((cut.len(), line.closed, line.points.first(), line.points.last()), (1, false, Some(&p(0.0, 45.0)), Some(&p(0.0, 19.5))));
        assert!((line.length() - 230.5).abs() < 1e-9 && line.points.len() == 6, "{line:?}");
        // A shape far from it covers none of it, and neither does a line digitized from a stroke, which
        // goes over the area.
        let far = square((50.0, 50.0), (60.0, 60.0));
        assert_eq!(outlines_of(vec![area("a", left.clone(), 0), area("f", far, 1)])[0], one("a", 40.0, true));
        let along = Subpath { start: p(10.0, -5.0), segments: vec![Segment::Line(p(10.0, 15.0))], closed: false };
        let stroke = Shape::Stroke { path: Path { subpaths: vec![along] }, width: Mm::new(0.4).unwrap(), join: Join::Round, cap: Cap::Round };
        let line = Sewn {
            element: Element { shape: stroke, params: ParamSet::default(), ..filled("l", std::slice::from_ref(&left)) },
            area: None,
            automatic: true,
            reach: Some(1.0),
            shape: 1,
            part: None,
        };
        assert_eq!(outlines_of(vec![area("a", left, 0), line]), [one("a", 40.0, true)]);
    }

    #[test]
    fn req_auto_052_an_outline_sews_through_a_stretch_of_25_mm_or_less_under_another_shape() {
        let lengths =
            |lines: &[Line]| lines.iter().map(|line| ((line.length() * 1000.0).round(), line.closed, line.points.len())).collect::<Vec<_>>();
        let whole = [(256_000.0, true, 4)];
        // The sides of a square 64 mm across are cut into 128 pieces each. A square over 24.8 mm of the first
        // side hides 50 pieces, 25 mm: the outline sews through under it, all the way round. One piece more,
        // 25.5 mm, and the outline is a line that leaves the stretch out.
        assert_eq!(lengths(&notched(&[((-1.0, 19.6), (1.0, 44.4))])), whole);
        let cut = notched(&[((-1.0, 19.6), (1.0, 44.9))]);
        assert_eq!(lengths(&cut), [(230_500.0, false, 6)]);
        // Each stretch is measured alone: a short one that follows a long one is sewn through, and the long one
        // still parts the line's 2 ends. 2 long ones part 2 lines, and 2 short ones none.
        assert_eq!(notched(&[((-1.0, 19.6), (1.0, 44.9)), ((30.6, 63.0), (31.4, 65.0))]), cut);
        assert_eq!(lengths(&notched(&[((-1.0, 19.6), (1.0, 44.9)), ((19.6, 63.0), (44.9, 65.0))])), [(38_500.0, false, 3), (166_500.0, false, 5)]);
        assert_eq!(lengths(&notched(&[((-1.0, 19.6), (1.0, 20.4)), ((30.6, 63.0), (31.4, 65.0))])), whole);
        // A square 10 mm across beside one sewn after it is outlined whole, under its neighbour too, and so
        // are 2 elements of one shape.
        let (left, right) = (square((0.0, 0.0), (10.0, 10.0)), square((10.0, 0.0), (20.0, 10.0)));
        assert_eq!(outlines_of(vec![area("a", left.clone(), 0), area("b", right.clone(), 1)]), [one("a", 40.0, true), one("b", 40.0, true)]);
        assert_eq!(outlines_of(vec![area("a", left, 0), area("b", right, 0)]), [one("a", 40.0, true), one("b", 40.0, true)]);
        // Pieces of other lengths: 17 mm and 8 mm hidden in a row are 25 mm, each measured from the piece
        // before it, the first piece from the last. A row that starts the pieces is found from the first
        // piece that shows.
        let (s, h) = (true, false);
        let pieces = [(p(4.0, 0.0), s, s), (p(4.0, 17.0), s, h), (p(-4.0, 17.0), h, h), (p(0.0, 0.0), s, s)];
        let shows = |pieces: &[(Point, bool, bool)]| {
            let mut pieces: Vec<_> = pieces.iter().map(|&(to, corner, shows)| (to, corner, shows, false)).collect();
            through(&mut pieces, &mut Budget::DEFAULT.meter()).unwrap();
            pieces.iter().map(|&(_, _, shows, _)| shows).collect::<Vec<_>>()
        };
        assert_eq!(shows(&pieces), [s, s, s, s]);
        assert_eq!(shows(&[pieces[1], pieces[2], pieces[3], pieces[0]]), [s, s, s, s]);
        // A millimetre more, and the row stays hidden.
        assert_eq!(shows(&[pieces[0], (p(4.0, 18.0), s, h), (p(-4.0, 18.0), h, h), pieces[3]]), [s, h, h, s]);
        // An edge that shows for 3 mm in all is sewn through, and one that shows for less is not: 2 pieces
        // of 1.5 mm, then of 1.25 mm, each after a hidden one of 1 mm.
        let short = |shown: f64| [(p(1.0, 0.0), s, h), (p(1.0 + shown, 0.0), s, s), (p(2.0 + shown, 0.0), s, h), (p(2.0 + 2.0 * shown, 0.0), s, s)];
        assert_eq!(shows(&short(1.5)), [s, s, s, s]);
        assert_eq!(shows(&short(1.25)), [h, s, h, s]);
        // An edge with no piece that shows stays hidden, and one with none hidden shows.
        assert_eq!(shows(&[pieces[1], pieces[2]]), [h, h]);
        assert_eq!(shows(&[pieces[0], pieces[3]]), [s, s]);
        assert_eq!(shows(&[]), [false; 0]);
    }

    #[test]
    fn req_auto_076_an_outline_sews_through_a_stretch_under_a_later_fill_only_when_it_is_12_mm_or_shorter() {
        let lengths = |lines: &[Line]| lines.iter().map(|line| ((line.length() * 1000.0).round(), line.closed)).collect::<Vec<_>>();
        let whole = [(256_000.0, true)];
        let fill = |id: &str, corners: Corners, shape: usize| area(id, square(corners.0, corners.1), shape);
        // The first side of a square 64 mm across runs up from its corner at the origin in pieces of 0.5 mm. A
        // fill sewn after it over 12 mm of that side, 24 pieces, is sewn through; over 12.5 mm it is left out.
        let (twelve, more) = (((-1.0, 19.6), (1.0, 31.6)), ((-1.0, 19.6), (1.0, 32.1)));
        assert_eq!(lengths(&notched_by(vec![fill("f", twelve, 1)])), whole);
        assert_eq!(lengths(&notched_by(vec![fill("f", more, 1)])), [(243_500.0, false)]);
        // A satin column over the same 12.5 mm is sewn through, as a stretch under any other shape is up to
        // 25 mm (`REQ-AUTO-052`).
        assert_eq!(lengths(&notched(&[more])), whole);
        // A fill of the area's own shape sewn after it counts as a later fill, and one sewn before it doesn't.
        assert_eq!(lengths(&notched_by(vec![fill("f", more, 0)])), [(243_500.0, false)]);
        let before = outlines_of(vec![fill("f", more, 0), area("a", square((0.0, 0.0), (64.0, 64.0)), 0)]);
        assert_eq!(before.last(), Some(&one("a", 256.0, true)));
        // A stretch hidden by a column and then a fill, 13.5 mm in all, is a stretch under a later fill: it
        // is left out, and so is one hidden by the fill first.
        let (low, high) = (((-1.0, 19.6), (1.0, 30.0)), ((-1.0, 29.6), (1.0, 33.0)));
        assert_eq!(lengths(&notched_by(vec![column("c", low, 1), fill("f", high, 2)])), [(242_500.0, false)]);
        let (low, high) = (((-1.0, 19.6), (1.0, 23.0)), ((-1.0, 22.6), (1.0, 33.0)));
        assert_eq!(lengths(&notched_by(vec![fill("f", low, 1), column("c", high, 2)])), [(242_500.0, false)]);
    }

    #[test]
    fn req_auto_052_an_outline_s_last_line_ends_nearest_where_the_next_element_starts() {
        let id = ElementId::new("a").unwrap();
        let open = |points: &[(f64, f64)]| Line { points: points.iter().map(|&(x, y)| p(x, y)).collect(), closed: false };
        let ring = Line { points: vec![p(20.0, 0.0), p(30.0, 0.0), p(30.0, 10.0), p(20.0, 10.0)], closed: true };
        let (low, high) = (open(&[(0.0, 0.0), (10.0, 0.0)]), open(&[(0.0, 20.0), (5.0, 26.0), (10.0, 20.0)]));
        let lines = || vec![low.clone(), ring.clone(), high.clone()];
        let near = |target: Approach| ending_near(lines(), &target, &Budget::DEFAULT, &id).unwrap();
        // The line with the nearest end goes last, and ends there: as it runs, or turned round.
        assert_eq!(near(Approach::Point(p(11.0, 20.0))), [low.clone(), ring.clone(), high.clone()]);
        let turned = open(&[(10.0, 20.0), (5.0, 26.0), (0.0, 20.0)]);
        assert_eq!(near(Approach::Point(p(-1.0, 20.0))), [low.clone(), ring.clone(), turned]);
        assert_eq!(near(Approach::Point(p(11.0, 1.0))), [ring.clone(), high.clone(), low.clone()]);
        // A ring starts and ends at its nearest corner.
        let from_corner = Line { points: vec![p(30.0, 10.0), p(20.0, 10.0), p(20.0, 0.0), p(30.0, 0.0)], closed: true };
        assert_eq!(near(Approach::Point(p(31.0, 11.0))), [low.clone(), high.clone(), from_corner]);
        // An open line ends at an end: the corner in its middle, 1 mm from the point, counts for nothing
        // beside the ring's corner 5 mm from it.
        let beside = Line { points: vec![p(20.0, 10.0), p(20.0, 0.0), p(30.0, 0.0), p(30.0, 10.0)], closed: true };
        assert_eq!(near(Approach::Point(p(5.0, 27.0))).last(), Some(&open(&[(10.0, 20.0), (5.0, 26.0), (0.0, 20.0)])));
        assert_eq!(near(Approach::Point(p(15.0, 10.0))).last(), Some(&beside));
        // Of 2 ends as near, the first found is kept: the first line's, and a line's start.
        let (west, east) = (open(&[(0.0, 0.0), (4.0, 0.0)]), open(&[(10.0, 0.0), (6.0, 0.0)]));
        let tied = ending_near(vec![west.clone(), east.clone()], &Approach::Point(p(5.0, 0.0)), &Budget::DEFAULT, &id).unwrap();
        assert_eq!(tied, [east, west]);
        let across = ending_near(vec![open(&[(4.0, 0.0), (6.0, 0.0)])], &Approach::Point(p(5.0, 0.0)), &Budget::DEFAULT, &id).unwrap();
        assert_eq!(across, [open(&[(6.0, 0.0), (4.0, 0.0)])]);
        // No line, and none is made.
        assert_eq!(ending_near(Vec::new(), &Approach::Point(p(0.0, 0.0)), &Budget::DEFAULT, &id), Ok(Vec::new()));
        // A satin column offers its rails, and a fill its area: the nearest point of any of their lines, a
        // hole's ring too.
        let meter = &mut Budget::DEFAULT.meter();
        let rails = Approach::Shape(vec![vec![p(0.0, 9.0), p(10.0, 9.0)], vec![p(0.0, 5.0), p(10.0, 5.0)], vec![p(0.0, 7.0), p(10.0, 7.0)]]);
        assert_eq!(away(p(5.0, 0.0), &rails, meter).unwrap(), 5.0);
        let hole = vec![p(4.0, 7.0), p(6.0, 7.0), p(6.0, 9.0), p(4.0, 9.0), p(4.0, 7.0)];
        let holed = Approach::Area(vec![Polygon { holes: vec![hole], ..square((0.0, 0.0), (10.0, 16.0)) }]);
        assert_eq!(away(p(5.0, 8.0), &holed, meter).unwrap(), 1.0);
        assert_eq!(away(p(5.0, -3.0), &holed, meter).unwrap(), 3.0);
        assert_eq!(near(holed.clone()).last(), Some(&open(&[(10.0, 0.0), (0.0, 0.0)])));
        // Each end or corner looked at costs a unit for each corner of what is offered: 8 of them, 10 units
        // each here. With less, the outline is not sewn, and that is said of its area.
        let with = |work: u64| ending_near(lines(), &holed, &Budget { max_work: work, ..Budget::DEFAULT }, &id);
        assert_eq!(with(80), Ok(near(holed.clone())));
        let said = with(79).unwrap_err();
        assert_eq!(said.element.as_ref().map(|id| id.as_str()), Some("a"));
        assert_eq!(
            said.to_string(),
            "info SC-I0904: Finding where this area's outline ends nearest the shape sewn next needed more than the work budget of 79 units, \
             so it sews no outline."
        );
        assert!((0..80).all(|work| with(work).is_err()));
        // A point costs nothing.
        assert!(ending_near(lines(), &Approach::Point(p(0.0, 0.0)), &Budget { max_work: 0, ..Budget::DEFAULT }, &id).is_ok());
    }

    /// `lines` moved 0.1 mm in with the default budget: each line's points, rounded to the micrometre, and
    /// whether it closes.
    fn moved_in(lines: Vec<Line>) -> Vec<(Vec<(f64, f64)>, bool)> {
        let id = ElementId::new("a").unwrap();
        let micrometres = |v: f64| (v * 1000.0).round() / 1000.0;
        let moved = inset(lines, 0.1, &Budget::DEFAULT, &id).unwrap();
        moved.into_iter().map(|line| (line.points.iter().map(|q| (micrometres(q.x()), micrometres(q.y()))).collect(), line.closed)).collect()
    }

    fn ring(points: &[(f64, f64)]) -> Line {
        Line { points: points.iter().map(|&(x, y)| p(x, y)).collect(), closed: true }
    }

    fn open(points: &[(f64, f64)]) -> Line {
        Line { points: points.iter().map(|&(x, y)| p(x, y)).collect(), closed: false }
    }

    #[test]
    fn req_auto_130_an_outline_runs_its_tolerance_inside_its_area_s_edge() {
        // A square's outline, clockwise with y up, moves in and closes, its corners square: it starts at the
        // middle of its longest side, the first, where its offset needs no join. A hole's, the other way,
        // moves out, into the area round it, its corners round.
        let square = [(0.0, 0.0), (0.0, 10.0), (10.0, 10.0), (10.0, 0.0)];
        let inside = [(0.1, 5.0), (0.1, 9.9), (9.9, 9.9), (9.9, 0.1), (0.1, 0.1), (0.1, 5.0)];
        assert_eq!(moved_in(vec![ring(&square)]), [(inside[..5].to_vec(), true)]);
        let hole: Vec<(f64, f64)> = square.iter().rev().copied().collect();
        let rings = moved_in(vec![ring(&hole)]);
        let around = &rings[0].0;
        assert_eq!((rings.len(), rings[0].1), (1, true));
        assert_eq!((around.first(), around.len() > 8), (Some(&(10.1, 5.0)), true));
        assert!(around.iter().all(|&(x, y)| (-0.1 - 1e-9..=10.1 + 1e-9).contains(&x) && (-0.1 - 1e-9..=10.1 + 1e-9).contains(&y)));
        // An open line moves to its right with y up, and is left out when it comes to less than 3 mm.
        assert_eq!(moved_in(vec![open(&[(0.0, 0.0), (0.0, 3.0)]), open(&[(5.0, 0.0), (5.0, 2.999)])]), [(vec![(0.1, 0.0), (0.1, 3.0)], false)]);
        // A point where a line goes straight on adds none. Where it turns straight back, the line rounds the
        // end in a half circle 0.1 mm from it.
        assert_eq!(moved_in(vec![open(&[(0.0, 0.0), (0.0, 1.0), (0.0, 3.0)])]), [(vec![(0.1, 0.0), (0.1, 3.0)], false)]);
        let back = moved_in(vec![open(&[(0.0, 0.0), (0.0, 5.0), (0.0, 2.0)])]);
        let (first, last) = (back[0].0[0], *back[0].0.last().unwrap());
        assert_eq!((back.len(), first, last, back[0].0.len() > 6), (1, (0.1, 0.0), (-0.1, 2.0), true));
        assert!(back[0].0[1..back[0].0.len() - 1].iter().all(|&(x, y)| (stitchcraft_core::math::hypot(x, y - 5.0) - 0.1).abs() < 2e-3), "{back:?}");
        // A line keeps to its edge where its first or last side moves in to nothing, and where a side that
        // does, after an arc, would part the arc from the side after it.
        for edge in [
            &[(0.0, 0.0), (0.0, 5.0), (0.05, 5.0)][..],
            &[(0.0, 0.0), (0.05, 0.0), (0.05, -5.0)],
            &[(0.0, 0.0), (0.0, 5.0), (-0.05, 5.0), (-0.05, 10.0)],
        ] {
            assert_eq!(moved_in(vec![open(edge)]), [(edge.to_vec(), false)]);
        }
        // A ring with a bar 0.15 mm wide keeps to its edge: the bar's end moves in to nothing, and its 2 sides,
        // moved, don't meet. So does a ring narrower than that throughout. A ring of no points is left out.
        let flagged = [(0.0, 0.0), (0.0, 5.0), (2.5, 5.0), (2.5, 15.0), (2.65, 15.0), (2.65, 5.0), (5.0, 5.0), (5.0, 0.0)];
        assert_eq!(moved_in(vec![ring(&flagged)]), [(flagged.to_vec(), true)]);
        let bar = [(0.0, 0.0), (0.0, 10.0), (0.15, 10.0), (0.15, 0.0)];
        assert_eq!(moved_in(vec![ring(&bar), ring(&[])]), [(bar.to_vec(), true)]);
        // 2 squares joined by a neck 0.15 mm wide move in as one ring: along the neck, the 2 moved sides pass
        // each other and lie inside it, 2.5 and 2.55 mm up, rounding its 4 inward corners.
        let dumbbell = [
            (0.0, 0.0),
            (0.0, 5.0),
            (5.0, 5.0),
            (5.0, 2.6),
            (7.0, 2.6),
            (7.0, 5.0),
            (12.0, 5.0),
            (12.0, 0.0),
            (7.0, 0.0),
            (7.0, 2.45),
            (5.0, 2.45),
            (5.0, 0.0),
        ];
        let rings = moved_in(vec![ring(&dumbbell)]);
        assert_eq!((rings.len(), rings[0].1), (1, true));
        let neck: Vec<f64> = rings[0].0.iter().filter(|&&(x, _)| (5.0..=7.0).contains(&x)).map(|&(_, y)| y).collect();
        assert!(neck.len() >= 4 && neck.iter().all(|y| (2.45..=2.6).contains(y)), "{rings:?}");
        // A ring of 100 sides, a polygon round a circle 10 mm across clockwise with y up: every point of it lies
        // 0.1 mm inside.
        let round: Vec<(f64, f64)> = (0..100)
            .map(|k| stitchcraft_core::math::sin_cos(-f64::from(k) * std::f64::consts::TAU / 100.0))
            .map(|(sin, cos)| (5.0 * cos, 5.0 * sin))
            .collect();
        let rings = moved_in(vec![ring(&round)]);
        let side = |(x, y): (f64, f64)| {
            let near = |k: usize| distance_to_segment(p(x, y), p(round[k].0, round[k].1), p(round[(k + 1) % 100].0, round[(k + 1) % 100].1));
            (0..100).map(near).fold(f64::INFINITY, f64::min)
        };
        assert_eq!((rings.len(), rings[0].1, rings[0].0.len()), (1, true, 101));
        assert!(rings[0].0.iter().all(|&q| (side(q) - 0.1).abs() < 2e-3), "{rings:?}");
    }

    #[test]
    fn req_auto_130_an_outline_that_needs_more_work_to_move_in_than_its_budget_allows_says_so() {
        let id = ElementId::new("a").unwrap();
        let with = |work: u64| {
            inset(vec![ring(&[(0.0, 0.0), (0.0, 10.0), (10.0, 10.0), (10.0, 0.0)])], 0.1, &Budget { max_work: work, ..Budget::DEFAULT }, &id)
        };
        assert!(with(9).is_ok());
        // Short of a unit for each side, and short of one more for each corner.
        assert!(with(4).is_err());
        let said = with(8).unwrap_err();
        assert_eq!(
            said.to_string(),
            "info SC-I0904: Finding where this area's outline runs inside its edge needed more than the work budget of 8 units, so it sews no \
             outline."
        );
    }

    #[test]
    fn req_auto_051_an_outline_shorter_than_2_stitches_is_left_out() {
        // A square 1 mm across has a ring of 4 mm, which sews, and one 0.7 mm across a ring of 2.8 mm, which
        // doesn't: the area has no outline.
        assert_eq!(outlines_of(vec![area("a", square((0.0, 0.0), (1.0, 1.0)), 0)]), [one("a", 4.0, true)]);
        assert_eq!(outlines_of(vec![area("a", square((0.0, 0.0), (0.7, 0.7)), 0)]), []);
        // A ring exactly 3 mm long sews.
        assert_eq!(outlines_of(vec![area("a", square((0.0, 0.0), (0.75, 0.75)), 0)]), [one("a", 3.0, true)]);
        // A square 2.5 mm across in the mouth of a U sewn after it shows one side: 2.5 mm, left out.
        let u = Polygon {
            outline: vec![p(-5.0, -5.0), p(-5.0, 2.5), p(0.0, 2.5), p(0.0, 0.0), p(2.5, 0.0), p(2.5, 2.5), p(7.5, 2.5), p(7.5, -5.0), p(-5.0, -5.0)],
            holes: Vec::new(),
        };
        let small = square((0.0, 0.0), (2.5, 2.5));
        assert_eq!(outlines_of(vec![area("a", small, 0), area("u", u.clone(), 1)]).iter().map(|(id, _)| id.as_str()).collect::<Vec<_>>(), ["u"]);
        // In a mouth 3 mm wide the side of 3 mm sews, and with it the 3 sides under the U, 8 mm
        // (`REQ-AUTO-052`).
        let wide = Polygon { outline: u.outline.iter().map(|q| if q.x() == 2.5 { p(3.0, q.y()) } else { *q }).collect(), holes: Vec::new() };
        let found = outlines_of(vec![area("a", square((0.0, 0.0), (3.0, 2.5)), 0), area("u", wide, 1)]);
        assert_eq!(found[0], one("a", 11.0, true));
    }

    #[test]
    fn req_auto_051_an_edge_in_a_hole_of_a_later_shape_shows() {
        // A square in the hole of a frame sewn after it, 1 mm clear of it all round, is outlined whole: the
        // frame's bounds hold it, and its hole does too. A square that fills the hole shows no edge.
        let frame = |hole: Polygon| Polygon { holes: vec![hole.outline.iter().rev().copied().collect()], ..square((0.0, 0.0), (10.0, 10.0)) };
        let clear = outlines_of(vec![area("a", square((4.0, 4.0), (6.0, 6.0)), 0), area("f", frame(square((3.0, 3.0), (7.0, 7.0))), 1)]);
        assert_eq!(clear[0], one("a", 8.0, true));
        let filling = outlines_of(vec![area("a", square((4.0, 4.0), (6.0, 6.0)), 0), area("f", frame(square((4.0, 4.0), (6.0, 6.0))), 1)]);
        assert_eq!(filling.iter().map(|(id, _)| id.as_str()).collect::<Vec<_>>(), ["f"]);
        // A part with no ring at all has no outline.
        assert_eq!(uncovered(&[], &[], 0, &mut Budget::DEFAULT.meter()).map_err(|_| ()), Ok(Vec::new()));
    }

    #[test]
    fn req_auto_051_an_area_whose_outline_needs_more_work_than_its_budget_allows_sews_none_and_says_so() {
        let sewn = || vec![area("a", square((0.0, 0.0), (10.0, 10.0)), 0), area("b", square((10.0, 0.0), (20.0, 10.0)), 1)];
        let full = outlined(sewn(), &Budget::DEFAULT);
        let with = |work: u64| outlined(sewn(), &Budget { max_work: work, ..Budget::DEFAULT });
        let least = (0..1 << 20).find(|work| with(*work) == full).unwrap();
        // With less, the first area, which looks at the second, has no outline, and that is said of it; the
        // second, which needs less, may have its own.
        for work in 0..least {
            let (found, said) = with(work);
            assert!(said >= 1 && found.iter().all(|(id, _)| id == "b"), "{work}: {found:?}");
        }
        assert!(least > 80, "{least}");
        let mut shapes = Shapes::new(sewn(), &DesignSettings::default(), &Budget::DEFAULT);
        let (_, said) = outlines(&mut shapes, &Budget { max_work: 0, ..Budget::DEFAULT });
        assert_eq!(said[0].element.as_ref().map(|id| id.as_str()), Some("a"));
        assert_eq!(
            said[0].to_string(),
            "info SC-I0904: Finding where this area's edge meets the shapes sewn after it needed more than the work budget of 0 units, so it \
             sews no outline."
        );
    }
}
