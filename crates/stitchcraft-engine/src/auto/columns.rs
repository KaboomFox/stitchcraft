//! The parts of an automatic area no wider than a satin column, each sewn as satin columns between its own
//! outlines (`REQ-AUTO-016` to `REQ-AUTO-018`).
//!
//! Design: `docs/src/design/image-to-embroidery.md` › Stitch types by width, and
//! `docs/src/design/algorithms/centre-lines.md` › Columns. Filled, a part as wide as a satin column sews rows
//! no longer than it is wide wherever they cross it; a satin column sews it with stitches across, the way a
//! digitizer would. So a part no wider than [`SATIN_MAX_WIDTH`] everywhere, and not narrow (`auto::narrow`),
//! sews a column along each of its centre lines (`normalize::strips::columns`):
//!
//! - **Width.** Each column is as wide as the circles that fit inside the part centred on its line, on
//!   average, as a narrow part is. One no wider than the design's narrowest satin stroke is too narrow to
//!   stitch across, and runs along its line as a running stitch instead, tripled from 0.5 mm wide.
//! - **Order.** A column that ends under another sews first, then the longest, and each next the one with
//!   an end nearest where the last one ended, sewn from that end; a ring from its rung nearest.
//! - **Settings.** Each column's underlay is its width's ([`super::underlay`]). One that ends under another
//!   is [`OVERLAP`] longer at that end, by its push compensation, and so reaches under it, as an area reaches
//!   under the shapes sewn after it.
//! - **Drawn.** Each column is drawn with its rails and rungs, and read back as a satin column is read, it
//!   must give them ([`drawn::from_rails`]). A part whose triangles make no column round each line, or one of
//!   whose columns can't be drawn so, stays in the area's fill, and `SC-I0902` says so.
//!
//! Every step costs a unit of work, a part's columns being part of the work of digitizing its area.

use stitchcraft_core::{Code, Diagnostic, ElementId, Exhausted, Fix, Meter, Mm, Point};

use super::OVERLAP;
use super::narrow::{Edges, piece, width};
use super::{SATIN_MAX_WIDTH, at_most};
use crate::design::Path;
use crate::generators::satin::drawn;
use crate::normalize::region::Polygon;
use crate::normalize::strips::{self, Column, Line};

/// A part sewn as columns, and its columns.
#[derive(Clone, Debug, PartialEq)]
pub(super) struct Columns {
    /// The part, which its columns paint.
    pub(super) part: Polygon,
    /// Its columns, in the order they are sewn.
    pub(super) sewn: Vec<Sewn>,
}

/// A column of a part as it is sewn.
#[derive(Clone, Debug, PartialEq)]
pub(super) enum Sewn {
    /// A satin column: the path that draws its rails and rungs, its width, and how much shorter it is made at
    /// its start, then its end (negative: longer).
    Satin { path: Path, width: Mm, push: [f64; 2] },
    /// A running stitch along the column's centre line, too narrow for satin, and the column's width.
    Running { line: Line, width: Mm },
}

/// What becomes of a part no wider than a satin column.
pub(super) enum Made {
    /// It sews as these columns.
    Columns(Vec<Sewn>),
    /// It has no line, a dot, and stays in the fill.
    Dot,
    /// Its columns can't be drawn, and it stays in the fill.
    Unmade,
    /// Its triangulation lost its shape, a bug, and it stays in the fill.
    Broken,
}

/// The columns of `part`, an area's whose narrowest satin stroke is `narrowest`, with `meter`'s work.
pub(super) fn columns(part: &Polygon, narrowest: Mm, meter: &mut Meter) -> Result<Made, Exhausted> {
    let found = match strips::columns(part, piece(part), meter)? {
        Err(_) => return Ok(Made::Broken),
        Ok(None) => return Ok(Made::Unmade),
        Ok(Some(found)) if found.is_empty() => return Ok(Made::Dot),
        Ok(Some(found)) => found,
    };
    let edges = Edges::of(part, piece(part), meter)?;
    let mut widths = Vec::with_capacity(found.len());
    for column in &found {
        let Some(width) = width(&edges, std::slice::from_ref(&column.line), meter)? else { return Ok(Made::Unmade) };
        widths.push(width);
    }
    let mut sewn = Vec::with_capacity(found.len());
    for (index, column) in ordered(&found, meter)? {
        let Some(&width) = widths.get(index) else { return Ok(Made::Unmade) };
        if at_most(width, narrowest) {
            sewn.push(Sewn::Running { line: column.walked, width });
            continue;
        }
        let Ok(drawn) = drawn::from_rails(column.rails, column.rungs, meter)? else { return Ok(Made::Unmade) };
        let push = column.under.map(|under| if under.is_some() { -OVERLAP } else { 0.0 });
        sewn.push(Sewn::Satin { path: drawn.path(), width, push });
    }
    Ok(Made::Columns(sewn))
}

/// `columns` in the order they are sewn, each with its place among them, turned to start where it is sewn
/// from, with `meter`'s work: a column that ends under another before it, then the longest first, and each
/// next the one with an end nearest where the last one ended, sewn from that end, or a ring from its rung
/// nearest. Where the columns that end under each other go round, the one that comes first in that order
/// ends on top. On a tie, the column found first.
fn ordered(columns: &[Column], meter: &mut Meter) -> Result<Vec<(usize, Column)>, Exhausted> {
    let count = columns.len();
    // How many columns end under each, which must sew before it.
    let mut waiting = vec![0_usize; count];
    for column in columns {
        for &under in column.under.iter().flatten() {
            if let Some(slot) = waiting.get_mut(under) {
                *slot += 1;
            }
        }
    }
    let mut done = vec![false; count];
    let mut out: Vec<(usize, Column)> = Vec::with_capacity(count);
    let mut here: Option<Point> = None;
    while out.len() < count {
        meter.charge(u64::try_from(count).unwrap_or(u64::MAX))?;
        let free = |c: usize| !done.get(c).copied().unwrap_or(true);
        let mut ready: Vec<usize> = (0..count).filter(|&c| free(c) && waiting.get(c) == Some(&0)).collect();
        if ready.is_empty() {
            ready = (0..count).filter(|&c| free(c)).collect();
        }
        let pick = match here {
            None => ready.iter().copied().fold(None, |best: Option<(usize, f64)>, c| {
                let length = columns.get(c).map_or(0.0, |column| column.walked.length());
                if best.is_some_and(|(_, most)| most >= length) { best } else { Some((c, length)) }
            }),
            Some(here) => ready.iter().copied().fold(None, |best: Option<(usize, f64)>, c| {
                let distance = columns.get(c).map_or(f64::INFINITY, |column| nearest_end(column, here));
                if best.is_some_and(|(_, least)| least <= distance) { best } else { Some((c, distance)) }
            }),
        };
        let Some((c, _)) = pick else { break };
        let Some(column) = columns.get(c) else { break };
        let column = match here {
            Some(here) if column.closed => column.restarted(here, meter)?,
            Some(here) if end_nearer(column, here) => column.reversed(),
            _ => column.clone(),
        };
        if let Some(slot) = done.get_mut(c) {
            *slot = true;
        }
        for &under in column.under.iter().flatten() {
            if let Some(slot) = waiting.get_mut(under) {
                *slot = slot.saturating_sub(1);
            }
        }
        here = if column.closed { column.rails[0].first().copied() } else { column.line.points.last().copied() };
        out.push((c, column));
    }
    Ok(out)
}

/// How far `here` is from where `column` can start: its centre line's nearer end, or for a ring, its centre
/// line's nearest point.
fn nearest_end(column: &Column, here: Point) -> f64 {
    let points = &column.line.points;
    let candidates: Vec<&Point> = if column.closed { points.iter().collect() } else { points.first().into_iter().chain(points.last()).collect() };
    candidates.into_iter().map(|p| p.distance(here)).fold(f64::INFINITY, f64::min)
}

/// Whether the open `column`'s centre line ends nearer `here` than it starts.
fn end_nearer(column: &Column, here: Point) -> bool {
    let points = &column.line.points;
    match (points.first(), points.last()) {
        (Some(start), Some(end)) => end.distance(here) < start.distance(here),
        _ => false,
    }
}

/// `SC-I0902` for a part of the automatic area `id` no wider than a satin column, whose satin columns can't be
/// drawn: it stays in the area's fill.
pub(super) fn unmade(id: &ElementId) -> Diagnostic {
    let message = format!(
        "A part of this area is no wider than a satin column, {} mm, but no satin columns can be drawn along it between its edges, so it is \
         filled.",
        SATIN_MAX_WIDTH.get()
    );
    Diagnostic::new(Code::AutoSatinFilled, message).with_element(id.clone()).with_fix(Fix::Hint(
        "To sew it as satin columns, draw it with smoother edges, or draw the columns with 2 rails and give them settings.".to_string(),
    ))
}

#[cfg(test)]
mod tests {
    use stitchcraft_core::{Budget, Code, ElementId};

    use super::*;
    use crate::normalize::region::geom;

    /// A band `width` millimetres wide and 20 mm long, its outline wound as a region winds it.
    fn band(width: f64) -> Polygon {
        let mut ring: Vec<Point> =
            [(0.0, 0.0), (20.0, 0.0), (20.0, width), (0.0, width)].iter().filter_map(|&(x, y)| Point::new(x, y).ok()).collect();
        if geom::signed_area(&geom::closed(&ring)) > 0.0 {
            ring.reverse();
        }
        Polygon { outline: geom::closed(&ring), holes: Vec::new() }
    }

    #[test]
    fn req_auto_016_a_column_no_wider_than_the_narrowest_satin_stroke_runs() {
        // A band 1.5 mm wide is one column: a satin column where the narrowest satin stroke is 1 mm, and a
        // running stitch along its line, as wide as the band, where it is 2 mm.
        let made = |narrowest: f64| columns(&band(1.5), Mm::new(narrowest).unwrap(), &mut Budget::DEFAULT.meter()).unwrap();
        let Made::Columns(sewn) = made(1.0) else { panic!("no columns") };
        let [Sewn::Satin { width, push, .. }] = sewn.as_slice() else { panic!("{sewn:?}") };
        assert!((width.get() - 1.5).abs() < 1e-6 && *push == [0.0, 0.0], "{width:?} {push:?}");
        let Made::Columns(sewn) = made(2.0) else { panic!("no columns") };
        let [Sewn::Running { width, line }] = sewn.as_slice() else { panic!("{sewn:?}") };
        assert!((width.get() - 1.5).abs() < 1e-6 && line.points.iter().all(|q| (q.y() - 0.75).abs() < 1e-9), "{width:?} {line:?}");
    }

    #[test]
    fn a_part_no_satin_column_can_be_drawn_along_is_said_to_stay_filled() {
        let id = ElementId::new("letters").unwrap();
        let said = unmade(&id);
        assert_eq!((said.code, said.element.as_ref()), (Code::AutoSatinFilled, Some(&id)));
        assert_eq!(
            said.to_string(),
            "info SC-I0902: A part of this area is no wider than a satin column, 7 mm, but no satin columns can be drawn along it between \
             its edges, so it is filled."
        );
    }
}
