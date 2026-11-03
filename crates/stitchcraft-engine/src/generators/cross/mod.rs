//! Cross stitch (milestones M10.1 and M10.2): a cross in each cell of a grid that the fill covers enough of,
//! as Ink/Stitch sews its 12 cross stitch methods (`docs/src/design/algorithms/cross-stitch.md`,
//! `REQ-FILL-CRS-001` to `REQ-FILL-CRS-006` and `REQ-FILL-CRS-008` to `REQ-FILL-CRS-011`).
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
//!    `cross_rotation` turns the part the other way first, about the point the offset names on the canvas's
//!    grid, and the grid is laid over the part so turned (`Frame`). Every cell the part reaches is looked
//!    at, where Ink/Stitch looks from the first line at or above the part's top, moved down by the offset,
//!    and misses a row there (`DEV-FILL-025`).
//! 3. **Coverage.** A cell sews when the part covers `fill_coverage` percent of it, with Ink/Stitch's slack
//!    of a ten-thousandth of a percent (`grid`). Dense upright crosses look at a second grid of cells too,
//!    moved half a cell left and up.
//! 4. **Runs.** Full crosses that share a join are sewn in one run, the runs in the order `route` gives, from
//!    the needle and ending nearest the next part, or for the last part nearest the next element. Half
//!    crosses are rows, routed as a tatami fill's (`half`).
//! 5. **Stitches.** Each run's places are its needle points, and a stitch longer than
//!    `max_cross_stitch_length_mm` is split into the fewest equal stitches no longer, as Ink/Stitch's geometry
//!    library splits it. A needle point nearer the one before than the shortest stitch is left out, as
//!    finalize would leave it, which only a grid too fine for the machine has (`SC-W0321`).
//!
//! **Flipped full crosses** lie on the same grid, each bar in the place of the bar a quarter turn from it,
//! so that the `/` diagonal goes under the `\` one. A flipped cross is a plain cross turned a quarter turn,
//! so each part, its grid, the needle and the end are turned a quarter turn more (`Frame`), sewn as plain
//! crosses, and turned back. Ink/Stitch turns the part alone and lays its grid afresh, which takes the
//! pattern size's 2 values the other way round and anchors a grid at the part's bottom edge
//! (`DEV-FILL-026`). Flipped half crosses sew each cell's `/` diagonal, on the same grid, as in Ink/Stitch.

mod bars;
mod grid;
mod half;
pub mod params;
mod route;

#[cfg(test)]
mod tests;

pub use params::CrossParams;

use std::cmp::Ordering;

use stitchcraft_core::units::at_least;
use stitchcraft_core::{Code, Diagnostic, Exhausted, Meter, Point};

use self::bars::Kind;
use self::grid::Grid;
use self::half::Half;
use self::route::{Crosses, Place};
use crate::design::Turn;
use crate::generators::tatami::rows::Segment;
use crate::generators::tatami::sew::{in_order, pieces, shrunk_away};
use crate::generators::{Approach, Own, Stitched, made, mm, settled};
use crate::normalize::near::nearest_pair;
use crate::normalize::region::ops::buffered;
use crate::normalize::region::{Polygon, Region, Unjoined};
use crate::thin::thin;

/// What each cell of a cross stitch fill sews (`cross_stitch_method`, without its flip).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Method {
    /// A `\` diagonal under a `/` one.
    Simple,
    /// A bar across under a bar down: a `+`.
    Upright,
    /// Upright crosses, and a second grid of them half a cell left and up, on the first grid's corners.
    DenseUpright,
    /// A `\` diagonal, an upright cross over it, and a `/` diagonal over that.
    Double,
    /// A simple cross, and an upright cross over it.
    Smyrna,
    /// One diagonal, `\`, the diagonals of the cells joined into rows routed as a tatami fill's.
    Half,
}

impl Method {
    /// The full crosses it sews, and whether a second grid of them lies half a cell over; `None` for half
    /// crosses.
    fn full(self) -> Option<(Kind, bool)> {
        match self {
            Method::Simple => Some((Kind::Simple, false)),
            Method::Upright => Some((Kind::Upright, false)),
            Method::DenseUpright => Some((Kind::Upright, true)),
            Method::Double => Some((Kind::Double, false)),
            Method::Smyrna => Some((Kind::Smyrna, false)),
            Method::Half => None,
        }
    }
}

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
    /// How far the grid is turned, in degrees, clockwise as drawn for a positive angle (`cross_rotation`).
    pub rotation: f64,
    /// How much of a cell, in percent, the fill must cover for it to sew (`fill_coverage`).
    pub coverage: f64,
    /// What each cell sews (`cross_stitch_method`).
    pub method: Method,
    /// Whether the method's flipped variant is sewn: a full cross with its bars a quarter turn round, the
    /// `/` diagonal under, or a half cross's `/` diagonal.
    pub flipped: bool,
    /// The passes of the thread: on each half of each bar of a full cross, an even number, 2 at least; along
    /// each stitch of a half cross's rows, an odd number (`cross_thread_count`).
    pub threads: u32,
    /// The longest stitch, in millimetres: a longer one is split, and a half cross's travel takes stitches
    /// this long (`max_cross_stitch_length_mm`).
    pub longest: f64,
    /// How far each part is grown, in millimetres; shrunk where it is negative (`expand_mm`).
    pub expand: f64,
    /// The shortest stitch, in millimetres: a needle point nearer the one before is left out.
    pub min_stitch: f64,
}

impl Cross {
    /// The grid laid over `part`, as the part lies in the turned grid's frame: through the canvas's origin,
    /// or the part's top left corner, moved left and down by the offset.
    fn grid(&self, part: &Polygon) -> Grid {
        let [left, down] = self.offset;
        let corner = if self.on_canvas { [0.0, 0.0] } else { top_left(part) };
        Grid { origin: [corner[0] - left, corner[1] + down], size: self.size }
    }

    /// The frame a part's crosses are laid out in: turned by `cross_rotation`, and for flipped full crosses a
    /// quarter turn more.
    fn frame(&self) -> Frame {
        // On the canvas's grid the grid turns about the point the offset names, as Ink/Stitch turns it; from a
        // part's corner the point changes nothing but rounding, and the canvas's origin rounds least.
        let about = if self.on_canvas { self.offset } else { [0.0, 0.0] };
        let turn = match (Turn::new(-self.rotation), Turn::new(self.rotation)) {
            (Some(into), Some(back)) if into.turns() => Some(Turned { into, back, about }),
            _ => None,
        };
        Frame { turn, quarter: self.flipped && self.method.full().is_some() }
    }
}

/// The least x and the least y of `part`'s outline.
fn top_left(part: &Polygon) -> [f64; 2] {
    part.outline.iter().fold([f64::INFINITY; 2], |[x, y], p| [x.min(p.x()), y.min(p.y())])
}

/// The grid's turn: a point is turned by `into` about `about` into the frame the crosses are laid out in, and
/// by `back` out of it, exactly for a whole number of quarter turns.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Turned {
    into: Turn,
    back: Turn,
    about: [f64; 2],
}

impl Turned {
    /// `p` turned by `turn` about the point.
    fn by(&self, turn: Turn, p: Point) -> Point {
        let [cx, cy] = self.about;
        let moved = Point::new(p.x() - cx, p.y() - cy).ok().and_then(|q| turn.apply(q));
        moved.and_then(|q| Point::new(q.x() + cx, q.y() + cy).ok()).unwrap_or(p)
    }
}

/// Where a part's crosses are laid out: as drawn, turned the other way by `cross_rotation` so that the grid
/// lies square to the frame, and for flipped full crosses turned a quarter turn more, `(x, y)` going to
/// `(−y, x)`, so that a plain cross laid out there is a flipped one once turned back. A quarter turn is
/// exact, and gives no minus zero, so a point turned and turned back is the point.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Frame {
    /// The grid's turn; `None` when it isn't turned.
    turn: Option<Turned>,
    /// Whether the crosses are laid out a quarter turn round.
    quarter: bool,
}

impl Frame {
    /// `p` turned by the grid's turn alone.
    fn turned(&self, p: Point) -> Point {
        self.turn.map_or(p, |turn| turn.by(turn.into, p))
    }

    /// `p`, a point turned by the grid's turn alone, in the frame: turned a quarter turn more for flipped full
    /// crosses.
    fn quartered(&self, p: Point) -> Point {
        if self.quarter { Turn::RIGHT.apply(p).unwrap_or(p) } else { p }
    }

    /// `p` in the frame.
    fn inward(&self, p: Point) -> Point {
        self.quartered(self.turned(p))
    }

    /// `p`, a point of the frame, as drawn.
    fn back(&self, p: Point) -> Point {
        let p = if self.quarter { Turn::LEFT.apply(p).unwrap_or(p) } else { p };
        self.turn.map_or(p, |turn| turn.by(turn.back, p))
    }

    /// `part` with each of its rings' points mapped by `map`.
    fn mapped(part: &Polygon, map: impl Fn(Point) -> Point) -> Polygon {
        let ring = |ring: &[Point]| ring.iter().map(|&p| map(p)).collect();
        Polygon { outline: ring(&part.outline), holes: part.holes.iter().map(|hole| ring(hole)).collect() }
    }

    /// `grid`, laid over a part turned by the grid's turn alone, in the frame.
    fn grid(&self, grid: Grid) -> Grid {
        if self.quarter { grid.turned() } else { grid }
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
    let frame = cross.frame();
    let (mut needle, mut runs, mut bare) = (needle, Vec::new(), 0);
    for (k, part) in ordered.iter().enumerate() {
        let here = match ordered.get(k + 1) {
            Some(after) => nearest_pair(&rings(part), &rings(after), meter)?.map(|(on, _)| on),
            None => end,
        };
        let turned = Frame::mapped(part, |p| frame.turned(p));
        let grid = cross.grid(&turned);
        let sewn = match cross.method.full() {
            Some((kind, dense)) => {
                let (part, grid) = (Frame::mapped(&turned, |p| frame.quartered(p)), frame.grid(grid));
                let crosses = crosses(kind, dense, &part, &grid, cross.coverage, meter)?;
                full(&crosses, &grid, frame, cross, (needle, here), meter)?
            }
            None => halves(&turned, &grid, frame, cross, (needle, here), meter, &mut warnings)?,
        };
        let Some(sewn) = sewn else {
            bare += 1;
            continue;
        };
        for points in sewn {
            needle = points.last().copied().or(needle);
            runs.push(points);
        }
    }
    warnings.extend(no_cell(bare, ordered.len(), cross.coverage));
    warnings.extend(shrunk_away(Code::FillExpandShrinksAway, "expand_mm", "its crosses are", gone, region.parts.len()));
    Ok(Stitched { runs, warnings })
}

/// The full crosses of `kind` that `part`, as it lies in the frame, covers `coverage` percent of on `grid`,
/// and with `dense` those of a second grid half a cell left and up, whose middles are the first grid's
/// corners. Costs as [`grid::crossed`] does, for each grid.
fn crosses(kind: Kind, dense: bool, part: &Polygon, grid: &Grid, coverage: f64, meter: &mut Meter) -> Result<Crosses, Exhausted> {
    let mut middles: Vec<Place> = grid::crossed(part, grid, coverage, meter)?.crossed().iter().map(|&[i, j]| [2 * i + 1, 2 * j + 1]).collect();
    if dense {
        let ([x, y], [w, h]) = (grid.origin, grid.size);
        let over = Grid { origin: [x - w / 2.0, y - h / 2.0], size: grid.size };
        middles.extend(grid::crossed(part, &over, coverage, meter)?.crossed().iter().map(|&[i, j]| [2 * i, 2 * j]));
    }
    Ok(Crosses::new(kind, middles))
}

/// The runs that sew `crosses` on `grid`, both in `frame`, as drawn, from the needle to near the end; `None`
/// when there is no cross.
fn full(
    crosses: &Crosses,
    grid: &Grid,
    frame: Frame,
    cross: &Cross,
    (needle, end): (Option<Point>, Option<Point>),
    meter: &mut Meter,
) -> Result<Option<Vec<Vec<Point>>>, Exhausted> {
    if crosses.is_empty() {
        return Ok(None);
    }
    let ends = (needle.map(|p| frame.inward(p)), end.map(|p| frame.inward(p)));
    let mut runs = Vec::new();
    for run in route::runs(crosses, grid, cross.threads / 2, ends, meter)? {
        runs.push(stitched(&run, grid, frame, cross, meter)?);
    }
    Ok(Some(runs))
}

/// The runs that sew the half crosses of `part`, turned by the grid's turn alone, on `grid`, as drawn, from
/// the needle to near the end; `None` when no cell is covered enough. A jump the needle takes, which a bug
/// would make, is said in `warnings`.
fn halves(
    part: &Polygon,
    grid: &Grid,
    frame: Frame,
    cross: &Cross,
    (needle, end): (Option<Point>, Option<Point>),
    meter: &mut Meter,
    warnings: &mut Vec<Diagnostic>,
) -> Result<Option<Vec<Vec<Point>>>, Exhausted> {
    let cells = grid::crossed(part, grid, cross.coverage, meter)?;
    if cells.is_empty() {
        return Ok(None);
    }
    let half = Half { flipped: cross.flipped, passes: cross.threads, longest: cross.longest, min_stitch: cross.min_stitch };
    let ends = (needle.map(|p| frame.inward(p)), end.map(|p| frame.inward(p)));
    let runs = half::runs(&cells, grid, &half, ends, meter, warnings)?;
    Ok(Some(runs.into_iter().map(|run| thinned(run.into_iter().map(|p| frame.back(p)).collect(), cross)).collect()))
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
fn stitched(run: &[Place], grid: &Grid, frame: Frame, cross: &Cross, meter: &mut Meter) -> Result<Vec<Point>, Exhausted> {
    let mut points: Vec<Point> = Vec::with_capacity(run.len());
    for &place in run {
        let to = grid.place(place);
        if let Some(&from) = points.last() {
            let pieces = pieces_of(from.distance(to), cross.longest);
            made(meter, pieces as usize)?;
            points.extend((1..pieces).map(|k| from.lerp(to, f64::from(k) / f64::from(pieces))));
        } else {
            made(meter, 1)?;
        }
        points.push(to);
    }
    Ok(thinned(points.into_iter().map(|p| frame.back(p)).collect(), cross))
}

/// `points` with each needle point nearer the one before than `cross`'s shortest stitch left out.
fn thinned(points: Vec<Point>, cross: &Cross) -> Vec<Point> {
    thin(points, |a, b| !at_least(a.distance(*b), cross.min_stitch), |_| false, |_, _| ())
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

/// Where the crosses of `cross` lie over `region`, each part grown as [`cross_stitch`] grows it: the bars of
/// each full cross from the bottom up, or each cell's diagonal, without the stitches or their order. A
/// preview too costly to sew shows them. Work is charged to `meter` as for sewing.
pub fn top_path(region: &Region, cross: &Cross, meter: &mut Meter) -> Result<Vec<Segment>, Exhausted> {
    // Flipped crosses lie where plain ones do, so the preview lays them out unflipped.
    let frame = Frame { quarter: false, ..cross.frame() };
    let mut segments = Vec::new();
    for part in &grown(region, cross.expand, &mut Vec::new(), meter)?.0 {
        let turned = Frame::mapped(part, |p| frame.turned(p));
        let grid = cross.grid(&turned);
        let mut bar = |[u, v]: Place, [a, b]: [[i64; 2]; 2]| {
            let [start, end] = [a, b].map(|[du, dv]| frame.back(grid.place([u + du, v + dv])));
            segments.push(Segment { start, end });
        };
        match cross.method.full() {
            Some((kind, dense)) => {
                for &middle in crosses(kind, dense, &turned, &grid, cross.coverage, meter)?.middles() {
                    for bars in kind.stack() {
                        bar(middle, bars.ends());
                    }
                }
            }
            None => {
                let diagonal = if cross.flipped { [[-1, 1], [1, -1]] } else { [[-1, -1], [1, 1]] };
                for &[i, j] in grid::crossed(&turned, &grid, cross.coverage, meter)?.crossed() {
                    bar([2 * i + 1, 2 * j + 1], diagonal);
                }
            }
        }
    }
    Ok(segments)
}
