//! Half crosses (milestone M10.2): one diagonal in each cell a part covers enough of, joined into rows that
//! are routed as a tatami fill's rows, each stitch sewn an odd number of times
//! (`docs/src/design/algorithms/cross-stitch.md` › Half crosses).
//!
//! **Why rows.** A half cross has no second diagonal to hide the needle's way under, and no middle to pass
//! through. Its diagonals that run on from cell to cell are one straight row, as a tatami fill's rows at 45°
//! are on square cells, and the needle travels between rows along the outline of the cells, as Ink/Stitch's
//! half crosses do (read at `d59c9ab`).
//!
//! **Steps.** For the cells of a part that sew:
//!
//! 1. **Pieces.** Cells that share a side or a corner are one piece. Each piece is sewn on its own, the piece
//!    nearest the needle first, as a fill's parts are, each ending nearest the next.
//! 2. **Outline.** A piece's rings are the sides of its cells that no other cell of it shares, joined end to
//!    end, the piece on their left as a part's rings have it, so the outline goes round counterclockwise as
//!    drawn and a hole the other way. Where 2 cells of the piece meet only at a corner, the ring runs through
//!    the corner from the one to the other, as Ink/Stitch's union of the cells, grown by a hair, joins them.
//!    Each ring starts at its top left corner.
//! 3. **Rows.** Each cell's diagonal, `\` from its top left corner or `/` from its bottom left one when
//!    flipped, is split into the fewest equal stitches no longer than the longest stitch, and diagonals that
//!    run on from cell to cell are one row, from its top end for `\` and its bottom end for `/`. The rows go
//!    row by row from the cell at their start.
//! 4. **Route.** The rows are routed as a tatami fill's rows with `underpath` off, each sewn once, and the
//!    needle travels between them along the rings in running stitches of the longest cross stitch, within
//!    0.2 CSS pixels of the way, as Ink/Stitch's half crosses travel (`tatami::sew::lines`).
//! 5. **Passes.** Each stitch of a row is sewn back and forth until it lies an odd number of times, the
//!    travel once (`passes::beaned`).

use std::collections::{BTreeMap, BTreeSet};

use stitchcraft_core::units::MM_PER_SVG_PX;
use stitchcraft_core::{Diagnostic, Exhausted, Meter, Point};

use super::grid::{Cells, Grid};
use crate::generators::made;
use crate::generators::passes::beaned;
use crate::generators::tatami::rings::Rings;
use crate::generators::tatami::sew::lines::{Given, lines_sewn};
use crate::generators::tatami::sew::{Travel, in_order};
use crate::normalize::near::nearest_pair;
use crate::normalize::region::{Polygon, Region};

/// How far a travel stitch may stray from the way, in millimetres: 0.2 CSS pixels, Ink/Stitch's tolerance
/// for a half cross's travel.
const TOLERANCE: f64 = 0.2 * MM_PER_SVG_PX;

/// How a part's half crosses are sewn.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Half {
    /// Whether each cell's diagonal is its `/` one rather than its `\` one.
    pub(crate) flipped: bool,
    /// How many times each stitch of a row lies: an odd number.
    pub(crate) passes: u32,
    /// The longest stitch, in millimetres, of the rows and of the travel between them.
    pub(crate) longest: f64,
    /// The shortest stitch, in millimetres.
    pub(crate) min_stitch: f64,
}

/// A cell, by its top left corner, in cells right of the grid's origin and down.
type Cell = [i64; 2];

/// The runs that sew the half crosses of `cells` on `grid` as `half` says, from the `needle` and to end
/// nearest `end` where they are known: a run for each piece, nearest the needle first. A jump the needle
/// takes within a piece, which a bug would make, is said in `warnings`. Work is charged to `meter` for every
/// cell, side, point and step, and a stitch for every needle point.
pub(crate) fn runs(
    cells: &Cells,
    grid: &Grid,
    half: &Half,
    (needle, end): (Option<Point>, Option<Point>),
    meter: &mut Meter,
    warnings: &mut Vec<Diagnostic>,
) -> Result<Vec<Vec<Point>>, Exhausted> {
    let pieces = pieces(cells, meter)?;
    let mut outlined = Vec::with_capacity(pieces.len());
    for piece in &pieces {
        outlined.push(outline(piece, grid, meter)?);
    }
    let region = Region { parts: outlined };
    let ordered = in_order(&region, needle, meter)?;
    // The jump length joins no runs here: `lines_sewn` sews one run unless a bug makes it jump.
    let travel = Travel { length: half.longest, tolerance: TOLERANCE, min_stitch: half.min_stitch, underpath: false, jump: 0.0 };
    let given = Given { travel, angle: 0.0, skip_last: false };
    let beans = [half.passes.saturating_sub(1) / 2];
    let (mut needle, mut runs) = (needle, Vec::new());
    for (k, part) in ordered.iter().enumerate() {
        let here = match ordered.get(k + 1) {
            Some(after) => nearest_pair(&rings_of(part), &rings_of(after), meter)?.map(|(on, _)| on),
            None => end,
        };
        // The piece the part was outlined from: `in_order` gives the region's own parts.
        let cells = region.parts.iter().position(|p| std::ptr::eq(p, *part)).and_then(|at| pieces.get(at)).map_or(&[][..], Vec::as_slice);
        let rows = rows(cells, grid, half, meter)?;
        let rings = Rings::new(part, meter)?;
        for marked in lines_sewn(part, &rings, &rows, &given, needle, here, meter, warnings)?.unwrap_or_default() {
            let run = beaned(&marked.points, &beans, &marked.travel, meter)?;
            needle = run.last().copied().or(needle);
            runs.push(run);
        }
    }
    Ok(runs)
}

/// `part`'s rings as lines: its outline, then its holes.
fn rings_of(part: &Polygon) -> Vec<&[Point]> {
    part.rings().collect()
}

/// The cells round a cell, by how far they lie across and down from it: the row above, the 2 beside it and
/// the row below, each from the left.
const AROUND: [Cell; 8] = [[-1, -1], [0, -1], [1, -1], [-1, 0], [1, 0], [-1, 1], [0, 1], [1, 1]];

/// The pieces of `cells`: the cells that share a side or a corner, each piece's cells row by row from the
/// top, the pieces in the order of their first cells. One unit of `meter` for each cell, and one for each of
/// its neighbours looked for.
fn pieces(cells: &Cells, meter: &mut Meter) -> Result<Vec<Vec<Cell>>, Exhausted> {
    let crossed = cells.crossed();
    let mut seen = vec![false; crossed.len()];
    let mut pieces = Vec::new();
    for (k, &cell) in crossed.iter().enumerate() {
        meter.charge(1)?;
        if seen.get(k).copied().unwrap_or(true) {
            continue;
        }
        if let Some(mark) = seen.get_mut(k) {
            *mark = true;
        }
        let mut piece = vec![cell];
        // Each cell of the piece is looked round once, and the piece holds no more cells than there are.
        for next in 0..crossed.len() {
            let Some(&[i, j]) = piece.get(next) else { break };
            for [di, dj] in AROUND {
                meter.charge(1)?;
                let neighbour = [i.saturating_add(di), j.saturating_add(dj)];
                if let Some(mark) = cells.find(neighbour).and_then(|n| seen.get_mut(n))
                    && !*mark
                {
                    *mark = true;
                    piece.push(neighbour);
                }
            }
        }
        piece.sort_by_key(|&[i, j]| (j, i));
        pieces.push(piece);
    }
    Ok(pieces)
}

/// A way along a side of a cell, across and down: east, south, west or north as drawn.
type Way = [i64; 2];

/// The way a quarter turn to the right of `way`, as drawn with y down: east to south, south to west.
const fn right([du, dv]: Way) -> Way {
    [-dv, du]
}

/// The rings of `piece`, its cells sorted row by row from the top, on `grid`: the outline, then the holes,
/// as the module's docs give them. One unit of `meter` for each side of a cell, and for each corner a ring
/// passes.
fn outline(piece: &[Cell], grid: &Grid, meter: &mut Meter) -> Result<Polygon, Exhausted> {
    let has = |cell: Cell| piece.binary_search_by(|&[i, j]| (j, i).cmp(&(cell[1], cell[0]))).is_ok();
    // The sides no other cell shares, by the corner each starts from (row by row from the top) and its way,
    // the cell on their left as drawn: down its left side, along its bottom, up its right side and back along
    // its top.
    let mut sides: BTreeMap<(i64, i64), Vec<Way>> = BTreeMap::new();
    for &[i, j] in piece {
        for (neighbour, from, way) in
            [([i - 1, j], [i, j], [0, 1]), ([i, j + 1], [i, j + 1], [1, 0]), ([i + 1, j], [i + 1, j + 1], [0, -1]), ([i, j - 1], [i + 1, j], [-1, 0])]
        {
            meter.charge(1)?;
            if !has(neighbour) {
                sides.entry((from[1], from[0])).or_default().push(way);
            }
        }
    }
    let total = sides.values().map(Vec::len).sum::<usize>();
    let mut used: BTreeSet<((i64, i64), Way)> = BTreeSet::new();
    let mut rings = Vec::new();
    for (&(v, u), ways) in &sides {
        for &way in ways {
            if used.contains(&((v, u), way)) {
                continue;
            }
            let mut corners = vec![[u, v]];
            let (mut at, mut going) = ([u, v], way);
            // Each side is passed once.
            for _ in 0..total {
                meter.charge(1)?;
                used.insert(((at[1], at[0]), going));
                let to = [at[0] + going[0], at[1] + going[1]];
                let leaving = sides.get(&(to[1], to[0])).map_or(&[][..], Vec::as_slice);
                // At a corner 2 cells of the piece share, the way on is the one to the right, into the other cell.
                let on = match leaving {
                    [only] => *only,
                    _ => right(going),
                };
                if (to, on) == ([u, v], way) {
                    break;
                }
                if on != going {
                    corners.push(to);
                }
                (at, going) = (to, on);
            }
            corners.push([u, v]);
            rings.push(corners.into_iter().map(|[i, j]| grid.place([2 * i, 2 * j])).collect::<Vec<Point>>());
        }
    }
    let mut rings = rings.into_iter();
    Ok(Polygon { outline: rings.next().unwrap_or_default(), holes: rings.collect() })
}

/// The rows of `piece`'s half crosses on `grid`, as the module's docs give them: each cell's diagonal split
/// into the fewest equal stitches no longer than the longest, those that run on from cell to cell one row.
/// One unit of `meter` for each cell, and a stitch for each needle point.
fn rows(piece: &[Cell], grid: &Grid, half: &Half, meter: &mut Meter) -> Result<Vec<Vec<Point>>, Exhausted> {
    let has = |cell: Cell| piece.binary_search_by(|&[i, j]| (j, i).cmp(&(cell[1], cell[0]))).is_ok();
    // Along the row from cell to cell, and the corners of a cell the diagonal runs between.
    let (step, from, to): (Cell, Cell, Cell) = if half.flipped { ([1, -1], [0, 1], [1, 0]) } else { ([1, 1], [0, 0], [1, 1]) };
    let corner = |[i, j]: Cell, [di, dj]: Cell| grid.place([2 * (i + di), 2 * (j + dj)]);
    let mut rows = Vec::new();
    for &[i, j] in piece {
        if has([i - step[0], j - step[1]]) {
            continue;
        }
        let mut row = vec![corner([i, j], from)];
        made(meter, 1)?;
        let mut at = [i, j];
        // Each cell of the piece lies on one row.
        for _ in 0..piece.len() {
            meter.charge(1)?;
            if !has(at) {
                break;
            }
            let (a, b) = (corner(at, from), corner(at, to));
            let pieces = super::pieces_of(a.distance(b), half.longest);
            made(meter, pieces as usize)?;
            row.extend((1..pieces).map(|k| a.lerp(b, f64::from(k) / f64::from(pieces))));
            row.push(b);
            at = [at[0] + step[0], at[1] + step[1]];
        }
        rows.push(row);
    }
    Ok(rows)
}

#[cfg(test)]
mod tests;
