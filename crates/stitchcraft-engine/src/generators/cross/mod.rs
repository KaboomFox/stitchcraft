//! Cross stitch (milestone M10.1): a cross in each cell of a grid that the fill covers enough of, as
//! Ink/Stitch sews its simple crosses (`docs/src/design/algorithms/cross-stitch.md`, `REQ-FILL-CRS-001`
//! to `REQ-FILL-CRS-006`).
//!
//! **Why a grid.** Cross stitch is counted work: every cross the same size, in rows and columns, as on the
//! even-weave fabric hand embroiderers count crosses on. A shape is filled by choosing cells of the grid,
//! not by following its outline, so its edge steps along the grid's lines. Ink/Stitch's cross-stitch fonts
//! draw their letters so, and sew alike here.
//!
//! **Steps.** For each part of the fill:
//!
//! 1. **Grown.** The fill's parts are grown by `expand_mm`, and parts grown into each other are one, as
//!    Ink/Stitch grows the whole shape at once. A negative `expand_mm` shrinks each part, and a part it wears
//!    away is kept as it is drawn (`SC-W0309`), as every fill's is (`DEV-FILL-024`). The parts are sewn
//!    nearest the needle first, as a tatami fill's are.
//! 2. **The grid.** Its cells are `pattern_size_mm` wide and tall. Its lines pass through the canvas's origin,
//!    or through the top left corner of the part's bounds with `canvas_grid_origin` off, and
//!    `cross_offset_mm` moves them left by its first length and down by its second, as Ink/Stitch moves them.
//!    Every cell the part reaches is looked at, where Ink/Stitch looks from the first line at or above the
//!    part's top, moved down by the offset, and misses a row there (`DEV-FILL-025`).
//! 3. **Coverage.** A cell sews a cross when the part covers `fill_coverage` percent of it, with Ink/Stitch's
//!    slack of a ten-thousandth of a percent ([`grid`]).
//! 4. **Runs.** Crosses that share a corner are sewn in one run, the runs in the order [`route`] gives,
//!    from the needle and ending nearest the next part, or for the last part nearest the next element.
//! 5. **Stitches.** Each run's places are its needle points, and a stitch longer than
//!    `max_cross_stitch_length_mm` is split into the fewest equal stitches no longer, as Ink/Stitch's geometry
//!    library splits it. A needle point nearer the one before than the shortest stitch is left out, as
//!    finalize would leave it, which only a grid too fine for the machine has (`SC-W0321`).
//!
//! **Flipped crosses** lie on the same grid, with their `/` diagonal under their `\` one. A flipped cross
//! is a plain cross turned a quarter turn, so each part, its grid, the needle and the end are turned a
//! quarter turn ([`Frame`]), sewn as plain crosses, and turned back. Ink/Stitch turns the part alone and
//! lays its grid afresh, which takes the pattern size's 2 values the other way round and anchors a grid at
//! the part's bottom edge (`DEV-FILL-026`).

mod grid;
pub mod params;
mod route;

#[cfg(test)]
mod tests;

pub use params::CrossParams;
use std::cmp::Ordering;

use stitchcraft_core::units::at_least;
use stitchcraft_core::{Code, Diagnostic, Exhausted, Meter, Point};

use self::grid::Grid;
use crate::generators::tatami::rows::Segment;
use crate::generators::tatami::sew::{in_order, pieces, shrunk_away};
use crate::generators::{Approach, Own, Stitched, made, mm, settled};
use crate::normalize::near::nearest_pair;
use crate::normalize::region::ops::buffered;
use crate::normalize::region::{Polygon, Region, Unjoined};
use crate::thin::thin;

/// A cross stitch fill as it is sewn.
#[derive(Clone, Debug, PartialEq)]
pub struct Cross {
    /// The cells' width and height, in millimetres (`pattern_size_mm`).
    pub size: [f64; 2],
    /// Whether the grid's lines pass through the canvas's origin, or else through each part's top left
    /// corner (`canvas_grid_origin`).
    pub on_canvas: bool,
    /// How far the grid is moved, in millimetres: left by the first, down by the second (`cross_offset_mm`).
    pub offset: [f64; 2],
    /// How much of a cell, in percent, the fill must cover for it to sew a cross (`fill_coverage`).
    pub coverage: f64,
    /// Whether each cross's `/` diagonal lies under its `\` one (`simple_cross_flipped`).
    pub flipped: bool,
    /// The passes on each half of each diagonal: an even number, 2 at least (`cross_thread_count`).
    pub threads: u32,
    /// The longest stitch, in millimetres: a longer one is split (`max_cross_stitch_length_mm`).
    pub longest: f64,
    /// How far each part is grown, in millimetres; shrunk where it is negative (`expand_mm`).
    pub expand: f64,
    /// The shortest stitch, in millimetres: a needle point nearer the one before is left out.
    pub min_stitch: f64,
}

impl Cross {
    /// The grid laid over `part`, as drawn: through the canvas's origin, or the part's top left corner, moved
    /// left and down by the offset.
    fn grid(&self, part: &Polygon) -> Grid {
        let [left, down] = self.offset;
        let corner = if self.on_canvas { [0.0, 0.0] } else { top_left(part) };
        Grid { origin: [corner[0] - left, corner[1] + down], size: self.size }
    }
}

/// The least x and the least y of `part`'s outline.
fn top_left(part: &Polygon) -> [f64; 2] {
    part.outline.iter().fold([f64::INFINITY; 2], |[x, y], p| [x.min(p.x()), y.min(p.y())])
}

/// Where a part's crosses are laid out: as drawn for plain crosses, or turned a quarter turn for flipped
/// ones, `(x, y)` going to `(−y, x)`, so that a plain cross laid out there is a flipped one once turned back.
/// Both ways are exact, so a point turned and turned back is the point.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Frame {
    /// As drawn.
    Plain,
    /// Turned a quarter turn.
    Turned,
}

impl Frame {
    /// `p` in the frame.
    fn into(self, p: Point) -> Point {
        match self {
            Frame::Plain => p,
            Frame::Turned => Point::new(-p.y(), p.x()).unwrap_or(p),
        }
    }

    /// `p`, a point of the frame, as drawn.
    fn back(self, p: Point) -> Point {
        match self {
            Frame::Plain => p,
            Frame::Turned => Point::new(p.y(), -p.x()).unwrap_or(p),
        }
    }

    /// `part` in the frame: each of its rings' points.
    fn part(self, part: &Polygon) -> Polygon {
        let ring = |ring: &[Point]| ring.iter().map(|&p| self.into(p)).collect();
        Polygon { outline: ring(&part.outline), holes: part.holes.iter().map(|hole| ring(hole)).collect() }
    }

    /// `grid`, laid over a part as drawn, in the frame.
    fn grid(self, grid: Grid) -> Grid {
        match self {
            Frame::Plain => grid,
            Frame::Turned => grid.turned(),
        }
    }
}

/// The stitches of the cross stitch `cross` over `region`: each part grown by `expand_mm`, nearest the
/// needle first, its crosses sewn in runs from the needle where the elements before left it to near the next
/// part, the last near the `next` element. Work is charged to `meter` for every cell looked at, every point
/// of a part's rings cut by a row, every step of a run, and a stitch for every needle point.
pub fn cross_stitch(
    region: &Region,
    cross: &Cross,
    needle: Option<Point>,
    next: Option<&Approach>,
    meter: &mut Meter,
) -> Result<Stitched, Exhausted> {
    settled(meter, |meter| sewn(region, cross, needle, next, meter))
}

/// [`cross_stitch`], before its stitches are settled.
fn sewn(region: &Region, cross: &Cross, needle: Option<Point>, next: Option<&Approach>, meter: &mut Meter) -> Result<Stitched, Exhausted> {
    let mut warnings = Vec::new();
    let (grown, gone) = grown(region, cross.expand, &mut warnings, meter)?;
    let grown = Region { parts: grown };
    let ordered = in_order(&grown, needle, meter)?;
    let end = next.map(|next| next.next_stitch(Own::Area(&region.parts), meter)).transpose()?.flatten();
    let frame = if cross.flipped { Frame::Turned } else { Frame::Plain };
    let (mut needle, mut runs, mut bare) = (needle, Vec::new(), 0);
    for (k, part) in ordered.iter().enumerate() {
        let here = match ordered.get(k + 1) {
            Some(after) => nearest_pair(&rings(part), &rings(after), meter)?.map(|(on, _)| on),
            None => end,
        };
        let grid = frame.grid(cross.grid(part));
        let cells = grid::crossed(&frame.part(part), &grid, cross.coverage, meter)?;
        if cells.is_empty() {
            bare += 1;
            continue;
        }
        let ends = (needle.map(|p| frame.into(p)), here.map(|p| frame.into(p)));
        for run in route::runs(&cells, &grid, cross.threads / 2, ends, meter)? {
            let points = stitched(&run, &grid, frame, cross, meter)?;
            needle = points.last().copied().or(needle);
            runs.push(points);
        }
    }
    warnings.extend(no_cell(bare, ordered.len(), cross.coverage));
    warnings.extend(shrunk_away(Code::FillExpandShrinksAway, "expand_mm", "its crosses are", gone, region.parts.len()));
    Ok(Stitched { runs, warnings })
}

/// The parts `region`'s parts grow into by `expand`, and how many of them it shrank away. Grown, parts that
/// meet are joined. Shrunk, each part is shrunk alone, and one shrunk away is kept as it is drawn. Should the
/// grown area's edges not join into rings, a bug that `warnings` gets, nothing.
fn grown(region: &Region, expand: f64, warnings: &mut Vec<Diagnostic>, meter: &mut Meter) -> Result<(Vec<Polygon>, usize), Exhausted> {
    match expand.partial_cmp(&0.0) {
        Some(Ordering::Greater) => {
            let grown = buffered(&region.parts, expand, meter)?;
            warnings.extend(grown.as_ref().err().copied().map(Unjoined::grown));
            Ok((grown.unwrap_or_default(), 0))
        }
        Some(Ordering::Less) => {
            let (mut parts, mut gone) = (Vec::new(), 0);
            for part in &region.parts {
                let (pieces, shrunk_away) = pieces(part, expand, warnings, meter)?;
                parts.extend(pieces);
                gone += usize::from(shrunk_away);
            }
            Ok((parts, gone))
        }
        Some(Ordering::Equal) | None => Ok((region.parts.clone(), 0)),
    }
}

/// `part`'s rings as lines: its outline, then its holes.
fn rings(part: &Polygon) -> Vec<&[Point]> {
    part.rings().collect()
}

/// The needle points of `run`, places on `grid` in `frame`, as drawn: a stitch longer than `cross`'s longest
/// split into the fewest equal stitches no longer, and a point nearer the one before than its shortest stitch
/// left out. A stitch for each needle point made.
fn stitched(run: &[route::Spot], grid: &Grid, frame: Frame, cross: &Cross, meter: &mut Meter) -> Result<Vec<Point>, Exhausted> {
    let mut points: Vec<Point> = Vec::with_capacity(run.len());
    for spot in run {
        let to = spot.on(grid);
        if let Some(&from) = points.last() {
            let pieces = pieces_of(from.distance(to), cross.longest);
            made(meter, pieces as usize)?;
            points.extend((1..pieces).map(|k| from.lerp(to, f64::from(k) / f64::from(pieces))));
        } else {
            made(meter, 1)?;
        }
        points.push(to);
    }
    let points = points.into_iter().map(|p| frame.back(p)).collect();
    Ok(thin(points, |a, b| !at_least(a.distance(*b), cross.min_stitch), |_| false, |_, _| ()))
}

/// How many equal stitches a stitch `length` long is split into so that none is longer than `longest`: 1 for
/// one no longer.
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn pieces_of(length: f64, longest: f64) -> u32 {
    (length / longest).ceil().clamp(1.0, f64::from(u32::MAX)) as u32
}

/// `SC-W0319` for the `bare` parts of the `parts` that cover no cell enough, `coverage` percent.
fn no_cell(bare: usize, parts: usize, coverage: f64) -> Option<Diagnostic> {
    let coverage = mm(coverage);
    let message = match (bare, parts) {
        (0, _) => return None,
        (1, 1) => format!("This cross stitch fill covers no cell of its grid by {coverage} % or more, so it sews nothing."),
        (1, _) => format!("1 of this cross stitch fill's parts covers no cell of its grid by {coverage} % or more, so it sews nothing."),
        (n, _) => format!("{n} of this cross stitch fill's parts cover no cell of its grid by {coverage} % or more, so they sew nothing."),
    };
    Some(Diagnostic::new(Code::FillCrossNoCell, message))
}

/// Where the crosses of `cross` lie over `region`, each part grown as [`cross_stitch`] grows it: the 2
/// diagonals of each cell that sews a cross, without the stitches or their order. A preview too costly to
/// sew shows them. Work is charged to `meter` as for sewing.
pub fn top_path(region: &Region, cross: &Cross, meter: &mut Meter) -> Result<Vec<Segment>, Exhausted> {
    let mut segments = Vec::new();
    for part in &grown(region, cross.expand, &mut Vec::new(), meter)?.0 {
        let grid = cross.grid(part);
        for &[i, j] in grid::crossed(part, &grid, cross.coverage, meter)?.crossed() {
            let [top_left, top_right, bottom_right, bottom_left] = [[i, j], [i + 1, j], [i + 1, j + 1], [i, j + 1]].map(|corner| grid.corner(corner));
            segments.extend([Segment { start: top_left, end: bottom_right }, Segment { start: bottom_left, end: top_right }]);
        }
    }
    Ok(segments)
}
