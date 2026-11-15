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
//! - **Work.** Each area has the budget's work to itself: a unit for each shape it looks past, the work of
//!   growing those near, and for each piece a unit for each corner of the shapes whose bounds hold its middle.
//!   An area that needs more sews no outline, and that is said (`SC-I0904`).

use stitchcraft_core::{Budget, Code, Diagnostic, ElementId, Meter, Point};

use super::{Failed, Kind, Shapes, TOUCHING, envelope, near};
use crate::normalize::region::Polygon;
use crate::normalize::region::geom::{self, Envelope, Location};
use crate::normalize::strips::Line;

/// The longest piece an edge is cut into to find where it shows, in millimetres: an outline starts and ends
/// within this of where a neighbour takes the edge over, a fifth of a running stitch.
pub(super) const STEP: f64 = 0.5;

/// The shortest line of an outline that sews, in millimetres: 2 running stitches. A shorter one is a knot.
pub(super) const SHORTEST: f64 = 3.0;

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
    let mut covering = Vec::new();
    for other in (0..shapes.len()).filter(|&other| other != index) {
        meter.charge(1)?;
        let own = shapes.of_shape.get(other) == shapes.of_shape.get(index);
        let covers = (other > index || own) && shapes.kind(other) != Some(Kind::Line);
        if !covers || !shapes.bounds(other).is_some_and(|theirs| near(&bounds, TOUCHING, &theirs)) {
            continue;
        }
        covering.extend(shapes.grown(other, meter)?.unwrap_or_default());
    }
    let covering: Vec<(Envelope, &Polygon)> = covering.iter().map(|part| (Envelope::of(&part.outline), part)).collect();
    let mut lines = Vec::new();
    for ring in region.iter().flat_map(Polygon::rings) {
        lines.extend(uncovered(ring, &covering, meter)?);
    }
    Ok(lines)
}

/// Whether `p` lies in one of `parts`, each with its bounds, on its edge included, with `meter`'s work.
fn covered(p: Point, parts: &[(Envelope, &Polygon)], meter: &mut Meter) -> Result<bool, Failed> {
    let here = Envelope::of(&[p]);
    for (bounds, part) in parts {
        meter.charge(1)?;
        if !bounds.covers(&here) {
            continue;
        }
        meter.charge(u64::try_from(part.rings().map(<[Point]>::len).sum::<usize>()).unwrap_or(u64::MAX))?;
        let inside = geom::locate_in_ring(p, &part.outline) != Location::Exterior;
        if inside && !part.holes.iter().any(|hole| geom::locate_in_ring(p, hole) == Location::Interior) {
            return Ok(true);
        }
    }
    Ok(false)
}

/// The lines along the closed `ring` that lie outside `parts`: the ring cut into pieces no longer than
/// [`STEP`], those whose middles `parts` don't hold joined in rows, the ring whole when all of it shows, and
/// the lines shorter than [`SHORTEST`] left out. A line keeps the ring's corners, and its ends lie where a
/// piece ends.
fn uncovered(ring: &[Point], parts: &[(Envelope, &Polygon)], meter: &mut Meter) -> Result<Vec<Line>, Failed> {
    // Each piece: where it ends, whether that is a corner of the ring, and whether it shows.
    let mut pieces = Vec::new();
    let Some(&start) = ring.first() else { return Ok(Vec::new()) };
    for pair in ring.windows(2) {
        let (Some(&a), Some(&b)) = (pair.first(), pair.last()) else { continue };
        let count = (a.distance(b) / STEP).ceil().max(1.0);
        let mut from = a;
        let mut k = 1.0;
        while k <= count {
            let to = if k == count { b } else { a.lerp(b, k / count) };
            pieces.push((to, k == count, !covered(from.lerp(to, 0.5), parts, meter)?));
            (from, k) = (to, k + 1.0);
        }
    }
    let Some(hidden) = pieces.iter().position(|&(_, _, shows)| !shows) else {
        // All of it shows: the ring, by its corners.
        let points: Vec<Point> = pieces.iter().filter(|&&(_, corner, _)| corner).map(|&(to, ..)| to).collect();
        let line = Line { points, closed: true };
        return Ok(if line.length() >= SHORTEST { vec![line] } else { Vec::new() });
    };
    // From the piece after a hidden one round to it again, so that no line is cut where the ring starts.
    let mut lines = Vec::new();
    let mut points: Vec<Point> = Vec::new();
    let mut from = pieces.get(hidden).map_or(start, |&(to, ..)| to);
    for step in 1..=pieces.len() {
        let Some(&(to, corner, shows)) = pieces.get((hidden + step) % pieces.len()) else { continue };
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
