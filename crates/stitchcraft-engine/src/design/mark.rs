//! Marks a person draws on a design, and the area a marker encloses (roadmap steps M14.5 and M13.39,
//! `docs/src/design/image-to-embroidery.md` › Text in pictures and › Marks that keep parts).
//!
//! **Why on the design.** A mark is no shape to sew: it tells automatic digitizing which parts to keep
//! (`REQ-AUTO-104`), and a guide which filled parts sew rows that follow its line (`REQ-AUTO-109`, roadmap
//! step M13.24). Each reader takes the shapes named [`KEEP`] and [`GUIDE`] out of the elements it reads
//! ([`taken_out`]) and gives them to the [`Design`](super::Design) as its marks and its guides, and planning
//! reads them on every plan. A command that applied them once would go stale when the picture is traced
//! again.
//!
//! **Why here, in the engine.** A text marker of `stitchcraft_svg` and a mark that keeps parts are read the
//! same way: a name in the `stitch:` family ([`note`]), and an area that a closed outline encloses or a band
//! along a line with a stroke ([`Area`]). Automatic digitizing tests parts against the marks, and the text
//! pass lies above the engine, so the one reading of a marker's area lies here, where both reach it.

pub mod frame;

use stitchcraft_core::{ElementId, Exhausted, Meter};

use super::{Element, Path, Shape};
use crate::normalize::stroke::flatten;
use frame::{At, Line};

/// The name of a mark that keeps parts: an element named `stitch:keep`, in any mix of upper and lower case,
/// and a note of the person's own if any, is a mark (`REQ-AUTO-104`).
pub const KEEP: &str = "stitch:keep";

/// The name of a guide of an automatic design: an element named `stitch:guide`, in any mix of upper and
/// lower case, and a note of the person's own if any, is a guide (`REQ-AUTO-111`). VectorCraft documents
/// give the same name to a guide grouped with the shapes it steers.
pub const GUIDE: &str = "stitch:guide";

/// How near a curve its polyline lies when a marker's area is read and a shape is tested against it, in
/// millimetres: a tenth of the shortest letter an embroidery font sews would still measure within 2 %.
pub const FLAT: f64 = 0.05;

/// A mark that keeps the parts of automatic areas lying wholly inside it (`REQ-AUTO-104`), or a guide whose
/// line the filled parts it crosses follow (`REQ-AUTO-109`).
#[derive(Clone, Debug, PartialEq)]
pub struct Mark {
    /// What messages call it: the id of the element it was read from, or the name a host gave it.
    pub id: ElementId,
    /// Its geometry, read as a marker's area is ([`Area::of`]).
    pub shape: Shape,
}

/// The note of `name` when it names the marker `marker`, such as `stitch:keep`: what follows the marker and
/// white space, without the white space at its ends. The marker is read in any case, after any white space.
/// `None` for a name that isn't the marker's.
pub fn note<'n>(name: &'n str, marker: &str) -> Option<&'n str> {
    let name = name.trim_start();
    if !name.get(..marker.len())?.eq_ignore_ascii_case(marker) {
        return None;
    }
    let rest = name.get(marker.len()..)?;
    (rest.is_empty() || rest.starts_with(char::is_whitespace)).then_some(rest.trim())
}

/// What [`taken_out`] takes out of a design's elements.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Taken {
    /// The elements that are neither marks nor guides, in paint order.
    pub elements: Vec<Element>,
    /// The shapes named [`KEEP`], as marks, in paint order.
    pub marks: Vec<Mark>,
    /// The shapes named [`GUIDE`], as guides, in paint order.
    pub guides: Vec<Mark>,
}

/// `elements` without the shapes named [`KEEP`] or [`GUIDE`], and those shapes as marks and as guides, in
/// paint order. An object with both a fill and a stroke is read as 2 elements in a row with one name and the
/// points of one path, its fill's closed: it is one mark or guide, its fill, or its stroke when the path is an
/// open line, whose stroke's width is a mark's band.
pub fn taken_out(elements: Vec<Element>) -> Taken {
    let mut taken = Taken::default();
    let mut last: Option<String> = None;
    for element in elements {
        let named = element.name.as_deref().map(str::to_string);
        let into = match named.as_deref() {
            Some(name) if note(name, KEEP).is_some() => &mut taken.marks,
            Some(name) if note(name, GUIDE).is_some() => &mut taken.guides,
            _ => {
                taken.elements.push(element);
                last = None;
                continue;
            }
        };
        let mark = Mark { id: element.id, shape: element.shape };
        match into.last_mut() {
            Some(before) if last.is_some() && last == named && before.shape.path().points().eq(mark.shape.path().points()) => {
                let open = mark.shape.path().subpaths.first().is_some_and(|first| !first.closed);
                if open && matches!(mark.shape, Shape::Stroke { .. }) {
                    *before = mark;
                }
            }
            _ => into.push(mark),
        }
        last = named;
    }
    taken
}

/// A marker's area, and the line a text marker's text follows.
#[derive(Clone, Debug, PartialEq)]
pub struct Area {
    /// The outline of a marker that is an area, as closed polylines.
    rings: Vec<Vec<At>>,
    /// Half the width of a marker that is a line.
    band: Option<f64>,
    /// The line a text follows: the marker itself, or a level line along the top of its area.
    line: Line,
    /// The box round the area: its left, top, right and bottom.
    around: [f64; 4],
}

impl Area {
    /// The area of a marker drawn as `shape`, flattened within [`FLAT`] with `meter`'s work, or why it has
    /// none. A fill, or a stroke whose first subpath is closed, is an area: what its outline encloses. A
    /// stroke whose first subpath is open is a band along that line, within half the stroke's width of it,
    /// between its ends.
    pub fn of(shape: &Shape, meter: &mut Meter) -> Result<Result<Area, &'static str>, Exhausted> {
        let pieces: Vec<Vec<At>> = flat(shape.path(), meter)?;
        let open = shape.path().subpaths.first().is_some_and(|first| !first.closed);
        if let (Shape::Stroke { width, .. }, true) = (shape, open) {
            let half = width.get() / 2.0;
            let line = pieces.first().and_then(|first| Line::of(first));
            let around = pieces.first().and_then(|first| bounds(first.iter().copied()));
            let area = line.zip(around).map(|(line, [left, top, right, bottom])| Area {
                rings: Vec::new(),
                band: Some(half),
                line,
                around: [left - half, top - half, right + half, bottom + half],
            });
            return Ok(area.ok_or("its line has no length."));
        }
        let rings = pieces;
        let Some(around @ [left, top, right, bottom]) = bounds(rings.iter().flatten().copied()) else { return Ok(Err("it encloses no area.")) };
        // Any level line serves: what is measured across it is placed across it again.
        let line = Line::of(&[(left, top), (right, top)]).filter(|_| bottom > top);
        Ok(line.map(|line| Area { rings, band: None, line, around }).ok_or("it encloses no area."))
    }

    /// Whether `p` lies in the area: inside the outline, or no further from the line than half its width and
    /// not past its ends.
    pub fn holds(&self, p: At) -> bool {
        match self.band {
            None => frame::inside(&self.rings, p),
            Some(half) => {
                let (along, across) = self.line.locate(p);
                across.abs() <= half && along >= 0.0 && along <= self.line.length()
            }
        }
    }

    /// Whether the box from `[left, top]` to `[right, bottom]` lies within the box round the area: what
    /// every point of a shape inside it does.
    pub fn may_hold(&self, [left, top, right, bottom]: [f64; 4]) -> bool {
        let [x0, y0, x1, y1] = self.around;
        left >= x0 && top >= y0 && right <= x1 && bottom <= y1
    }

    /// What testing a point costs: a unit for each segment it is tested against.
    pub fn cost(&self) -> u64 {
        let segments = match self.band {
            None => self.rings.iter().map(Vec::len).sum(),
            Some(_) => self.line.segments(),
        };
        u64::try_from(segments).unwrap_or(u64::MAX)
    }

    /// The line a text marker's text follows: the marker itself when it is a line, or a level line along the
    /// top of its area.
    pub fn line(&self) -> &Line {
        &self.line
    }
}

/// The subpaths of `path` as polylines within [`FLAT`] of them, one for each, charging `meter` as
/// [`flatten`] does.
pub fn flat(path: &Path, meter: &mut Meter) -> Result<Vec<Vec<At>>, Exhausted> {
    Ok(flatten(path, FLAT, meter)?.pieces.into_iter().map(|piece| piece.points.into_iter().map(|p| (p.x(), p.y())).collect()).collect())
}

/// The box round `points`: its left, top, right and bottom. `None` of no points.
pub fn bounds(points: impl Iterator<Item = At>) -> Option<[f64; 4]> {
    points.fold(None, |around: Option<[f64; 4]>, (x, y)| {
        Some(around.map_or([x, y, x, y], |[left, top, right, bottom]| [left.min(x), top.min(y), right.max(x), bottom.max(y)]))
    })
}

#[cfg(test)]
mod tests;
