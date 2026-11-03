//! The cells of a cross stitch fill's grid, and which of them sew a cross: those a part covers
//! `fill_coverage` percent of (`docs/src/design/algorithms/cross-stitch.md` › Coverage).
//!
//! **Why exact areas.** A cell sews a cross when the part covers enough of it, as Ink/Stitch decides it.
//! Its cross-stitch fonts grow their letters by a tenth of a millimetre, and a letter drawn off the grid
//! covers the cells along its edges by half or so. An area sampled at points would put crosses where
//! Ink/Stitch puts none. So each cell's covered area is worked out exactly: the area of the part cut to
//! the cell.
//!
//! **How.** Row by row, each ring of the part is cut to the row's strip, as Sutherland and Hodgman clip a
//! polygon by a line, once for each of the strip's 2 edges. What is left of the ring lies in the strip, its
//! points in the ring's order, and the strip's edges join it where the ring left the strip and came back.
//! By Green's theorem, the area of the cut ring left of any line down the strip is a sum over its sides:
//! the area between each side and the strip's top edge, signed by the way the side runs. A line down the
//! strip adds a side that runs straight down, whose share is nothing, so each side's share falls to the
//! columns it crosses, split where it crosses their lines. A cell's area is its column's sum. The outline
//! and the holes run opposite ways round, so a hole takes its area away.
//!
//! **Cost.** Every cell of the grid that the part's bounds reach is looked at once, and they are charged
//! to the meter before any is held: a fine grid over a large part runs out of work (`SC-E0004`) before it
//! takes memory. A row's cells are held while they are looked at, so a row must fit in the stitches left, as
//! each cell that sews a cross must. Each row then charges 2 units for each point of each ring, one for each
//! edge of the strip it is clipped by, and a unit for each share it splits off.

use std::cmp::Ordering;

use stitchcraft_core::{Exhausted, Meter, Point};

use crate::normalize::region::Polygon;

/// Ink/Stitch's slack on a cell's coverage, in percent: a cell covered `fill_coverage` percent less a
/// ten-thousandth of a percent sews a cross, so a cell covered exactly half sews at 50 %, however the
/// arithmetic rounds.
pub(crate) const SLACK: f64 = 0.0001;

/// A grid of cells `size` wide and tall, across and down, whose lines pass through `origin`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Grid {
    /// A point where 2 of its lines meet, in millimetres.
    pub(crate) origin: [f64; 2],
    /// Its cells' width and height, in millimetres.
    pub(crate) size: [f64; 2],
}

impl Grid {
    /// The point where the grid's line `i` cells right of its origin meets its line `j` cells down. A
    /// cell is named by its top left corner.
    pub(crate) fn corner(&self, [i, j]: [i64; 2]) -> Point {
        let [x, y] = self.line([i, j]);
        Point::new(x, y).unwrap_or(Point::ORIGIN)
    }

    /// The middle of the cell whose top left corner is `[i, j]`.
    pub(crate) fn middle(&self, [i, j]: [i64; 2]) -> Point {
        let ([x0, y0], [x1, y1]) = (self.line([i, j]), self.line([i.saturating_add(1), j.saturating_add(1)]));
        Point::new((x0 + x1) / 2.0, (y0 + y1) / 2.0).unwrap_or(Point::ORIGIN)
    }

    /// Where the grid's lines `[i, j]` lie: across, and down.
    fn line(&self, [i, j]: [i64; 2]) -> [f64; 2] {
        let [ox, oy] = self.origin;
        let [w, h] = self.size;
        [ox + i as f64 * w, oy + j as f64 * h]
    }

    /// The grid turned a quarter turn, as a drawing is turned to lay flipped crosses ([`super::Frame`]): a
    /// point `(x, y)` goes to `(−y, x)`, its lines across become lines down, and its cells are as tall as they
    /// were wide.
    pub(crate) fn turned(self) -> Grid {
        let ([x, y], [w, h]) = (self.origin, self.size);
        Grid { origin: [-y, x], size: [h, w] }
    }
}

/// The cells of a grid that sew a cross for one part, by their top left corners, row after row from the top,
/// each row from the left.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct Cells {
    crossed: Vec<[i64; 2]>,
}

impl Cells {
    /// The cells, row after row from the top, each row from the left.
    pub(crate) fn crossed(&self) -> &[[i64; 2]] {
        &self.crossed
    }

    /// Where the cell whose top left corner is `cell` lies in [`Cells::crossed`]; `None` when it sews no
    /// cross.
    pub(crate) fn find(&self, [i, j]: [i64; 2]) -> Option<usize> {
        self.crossed.binary_search_by(|&[ci, cj]| (cj, ci).cmp(&(j, i))).ok()
    }

    /// Whether no cell sews a cross.
    pub(crate) fn is_empty(&self) -> bool {
        self.crossed.is_empty()
    }

    /// The cells `crossed`, by their top left corners, in any order.
    #[cfg(test)]
    pub(crate) fn of(mut crossed: Vec<[i64; 2]>) -> Cells {
        crossed.sort_by_key(|&[i, j]| (j, i));
        crossed.dedup();
        Cells { crossed }
    }
}

/// The cells of `grid` that `part` covers `coverage` percent of or more, less [`SLACK`]: each cell the
/// part's bounds reach is looked at once. All of them are charged to `meter` before any is looked at, and
/// then each row as the module's docs say. A row of them, and each cell that sews a cross, must fit in the
/// stitches left, so that no more are held than could be sewn.
pub(crate) fn crossed(part: &Polygon, grid: &Grid, coverage: f64, meter: &mut Meter) -> Result<Cells, Exhausted> {
    let ([ox, oy], [w, h]) = (grid.origin, grid.size);
    let ([x0, y0], [x1, y1]) = bounds(&part.outline);
    let (i0, columns) = reach(x0, x1, ox, w);
    let (j0, rows) = reach(y0, y1, oy, h);
    meter.charge(columns.saturating_mul(rows))?;
    let columns = usize::try_from(columns).unwrap_or(usize::MAX);
    meter.room(columns)?;
    let mut sums = vec![0.0; columns];
    let (full, left) = (w * h, ox + i0 as f64 * w);
    let mut crossed = Vec::new();
    for row in 0..rows {
        let j = j0.saturating_add_unsigned(row);
        let (top, bottom) = (oy + j as f64 * h, oy + j.saturating_add(1) as f64 * h);
        sums.iter_mut().for_each(|sum| *sum = 0.0);
        for ring in part.rings() {
            let strip = strip(ring, top, bottom, meter)?;
            share(&strip, top, (left, w), &mut sums, meter)?;
        }
        for (column, sum) in sums.iter().enumerate() {
            if sum.abs() / full * 100.0 + SLACK >= coverage {
                crossed.push([i0.saturating_add_unsigned(column as u64), j]);
                meter.room(crossed.len())?;
            }
        }
    }
    Ok(Cells { crossed })
}

/// The first of the grid's lines at or before `low`, counted from `origin` in steps of `size`, and how many
/// cells from it reach `high`.
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn reach(low: f64, high: f64, origin: f64, size: f64) -> (i64, u64) {
    let first = ((low - origin) / size).floor();
    let past = ((high - origin) / size).ceil();
    (first as i64, (past - first) as u64)
}

/// The least and the greatest x and y of `ring`'s points.
fn bounds(ring: &[Point]) -> ([f64; 2], [f64; 2]) {
    let start = ([f64::INFINITY; 2], [f64::NEG_INFINITY; 2]);
    ring.iter().fold(start, |([x0, y0], [x1, y1]), p| ([x0.min(p.x()), y0.min(p.y())], [x1.max(p.x()), y1.max(p.y())]))
}

/// `ring` cut to the strip from `top` to `bottom` down the page: clipped by its top edge and then by its
/// bottom edge, as Sutherland and Hodgman clip a polygon. One unit of `meter` for each point looked at.
fn strip(ring: &[Point], top: f64, bottom: f64, meter: &mut Meter) -> Result<Vec<[f64; 2]>, Exhausted> {
    let points: Vec<[f64; 2]> = ring.iter().map(|p| [p.x(), p.y()]).collect();
    meter.charge(u64::try_from(points.len()).unwrap_or(u64::MAX).saturating_mul(2))?;
    let below_top = clipped(&points, |y| y >= top, top);
    Ok(clipped(&below_top, |y| y <= bottom, bottom))
}

/// The closed polygon `points`, its last point joined to its first, clipped to the side of the line across at
/// `y` where `keep` holds: the points kept, and where its sides cross the line, in order.
fn clipped(points: &[[f64; 2]], keep: impl Fn(f64) -> bool, y: f64) -> Vec<[f64; 2]> {
    let mut kept = Vec::with_capacity(points.len());
    let Some(&last) = points.last() else { return kept };
    let mut from = last;
    for &to in points {
        match (keep(from[1]), keep(to[1])) {
            (true, true) => kept.push(to),
            (true, false) => kept.push(crossing(from, to, y)),
            (false, true) => kept.extend([crossing(from, to, y), to]),
            (false, false) => {}
        }
        from = to;
    }
    kept
}

/// Where the side from `a` to `b`, which crosses the line across at `y`, meets it.
fn crossing([ax, ay]: [f64; 2], [bx, by]: [f64; 2], y: f64) -> [f64; 2] {
    [ax + (y - ay) * (bx - ax) / (by - ay), y]
}

/// Adds to `sums` each column's share of the closed polygon `points`, which lies in a strip whose top edge is
/// at `top`: for each side, the area between it and the top edge, signed by the way it runs across, split
/// where it crosses the columns' lines, the first at `left` and each `width` past the last. One unit of
/// `meter` for each share. The columns span the part's points: a point that rounding puts a hair past the
/// first or the last line adds nothing past it.
fn share(points: &[[f64; 2]], top: f64, (left, width): (f64, f64), sums: &mut [f64], meter: &mut Meter) -> Result<(), Exhausted> {
    let Some(&last) = points.last() else { return Ok(()) };
    let last_column = sums.len().saturating_sub(1);
    let line = |k: usize| left + k as f64 * width;
    let mut from = last;
    for &to in points {
        let ([ax, ay], [bx, by]) = (from, to);
        from = to;
        let ((u, v), sign) = match ax.partial_cmp(&bx) {
            Some(Ordering::Less) => ((ax, bx), 1.0),
            Some(Ordering::Greater) => ((bx, ax), -1.0),
            Some(Ordering::Equal) | None => continue,
        };
        let height = |x: f64| ay + (x - ax) * (by - ay) / (bx - ax);
        for k in column_of(u, left, width, last_column)..=column_of(v, left, width, last_column) {
            meter.charge(1)?;
            let (start, end) = (u.max(line(k)), v.min(line(k + 1)));
            if let Some(sum) = sums.get_mut(k) {
                *sum += sign * (end - start) * ((height(start) + height(end)) / 2.0 - top);
            }
        }
    }
    Ok(())
}

/// The column that `x` lies in, of columns from 0 to `last`, the first at `left` and each `width` wide: the
/// first or the last for a point past them.
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn column_of(x: f64, left: f64, width: f64, last: usize) -> usize {
    (((x - left) / width).floor().max(0.0) as usize).min(last)
}

#[cfg(test)]
mod tests {
    use stitchcraft_core::Budget;

    use super::*;

    fn p(x: f64, y: f64) -> Point {
        Point::new(x, y).unwrap()
    }

    /// The rectangle from `(x0, y0)` to `(x1, y1)` as a part, wound as a part is: clockwise in y-up axes.
    fn rect((x0, y0): (f64, f64), (x1, y1): (f64, f64)) -> Polygon {
        Polygon { outline: vec![p(x0, y0), p(x0, y1), p(x1, y1), p(x1, y0), p(x0, y0)], holes: Vec::new() }
    }

    fn crossed_cells(part: &Polygon, grid: &Grid, coverage: f64) -> Vec<[i64; 2]> {
        crossed(part, grid, coverage, &mut Budget::DEFAULT.meter()).unwrap().crossed().to_vec()
    }

    const THREE: Grid = Grid { origin: [0.0, 0.0], size: [3.0, 3.0] };

    #[test]
    fn req_fill_crs_002_a_cell_sews_a_cross_when_the_part_covers_enough_of_it() {
        // A rectangle from (10, 9.2) to (22, 19) on a 3 mm grid: the cells from x = 9 to 21 and y = 9 to 18,
        // but the top left one, which it covers 2/3 × 2.8/3, 62 %, in full: 12 cells. At 63 %, that one goes too.
        let part = rect((10.0, 9.2), (22.0, 19.0));
        let found = crossed_cells(&part, &THREE, 50.0);
        let wanted: Vec<[i64; 2]> = (3..6).flat_map(|j| (3..7).map(move |i| [i, j])).collect();
        assert_eq!(found, wanted);
        assert_eq!(crossed_cells(&part, &THREE, 63.0).len(), 11);
        // The cells right of x = 21 and below y = 18 are covered a third, and sew at 33 % but not at 34 %, but
        // for the corners, covered less: 5 columns of 4 rows less 3.
        assert_eq!(crossed_cells(&part, &THREE, 33.0).len(), 4 * 5 - 3);
        assert_eq!(crossed_cells(&part, &THREE, 34.0).len(), 12);
        // Left of and above the origin, the same: the rectangle moved 30 mm left and up, 10 cells each way.
        let moved = rect((-20.0, -20.8), (-8.0, -11.0));
        let wanted_moved: Vec<[i64; 2]> = wanted.iter().map(|&[i, j]| [i - 10, j - 10]).collect();
        assert_eq!(crossed_cells(&moved, &THREE, 50.0), wanted_moved);
        // On a grid whose lines pass through (-1, 2): the columns from x = 11 to 23, the last covered two thirds,
        // and the rows from y = 8 to 20, the first covered 60 % and the last two thirds. The last column's top
        // and bottom cells are covered less than half.
        let off = Grid { origin: [-1.0, 2.0], size: [3.0, 3.0] };
        let wanted_off: Vec<[i64; 2]> = (2..6).flat_map(|j| (4..8).map(move |i| [i, j])).filter(|&[i, j]| i < 7 || (3..5).contains(&j)).collect();
        assert_eq!(crossed_cells(&part, &off, 50.0), wanted_off);
    }

    #[test]
    fn req_fill_crs_002_a_cell_covered_exactly_as_asked_sews_and_a_hole_takes_its_area_away() {
        // Half of each cell of the column x 0 to 3: exactly 50 %, which sews at 50, with the slack, and not at
        // 50.001.
        let half = rect((1.5, 0.0), (6.0, 3.0));
        assert_eq!(crossed_cells(&half, &THREE, 50.0), [[0, 0], [1, 0]]);
        assert_eq!(crossed_cells(&half, &THREE, 50.001), [[1, 0]]);
        // A square of 6 mm with a hole of 2 mm in its top left cell, which leaves it 5/9 covered.
        let holed = Polygon { holes: vec![vec![p(0.5, 0.5), p(2.5, 0.5), p(2.5, 2.5), p(0.5, 2.5), p(0.5, 0.5)]], ..rect((0.0, 0.0), (6.0, 6.0)) };
        assert_eq!(crossed_cells(&holed, &THREE, 55.0), [[0, 0], [1, 0], [0, 1], [1, 1]]);
        assert_eq!(crossed_cells(&holed, &THREE, 56.0), [[1, 0], [0, 1], [1, 1]]);
    }

    #[test]
    fn req_fill_crs_002_a_slanted_side_shares_its_area_among_the_columns_it_crosses() {
        // A triangle under the diagonal of a 6 mm square: each of the 2 cells the diagonal cuts is half covered,
        // the one below it whole, and the one above it not at all.
        let triangle = Polygon { outline: vec![p(0.0, 0.0), p(0.0, 6.0), p(6.0, 6.0), p(0.0, 0.0)], holes: Vec::new() };
        let cells = crossed(&triangle, &THREE, 50.0, &mut Budget::DEFAULT.meter()).unwrap();
        assert_eq!(cells.crossed(), [[0, 0], [0, 1], [1, 1]]);
        assert_eq!(crossed_cells(&triangle, &THREE, 50.1), [[0, 1]]);
        assert_eq!([[0, 1], [1, 1], [1, 0], [5, 5], [-1, 0]].map(|cell| cells.find(cell)), [Some(1), Some(2), None, None, None]);
        // Drawn the other way round, the same.
        let reversed = Polygon { outline: triangle.outline.iter().rev().copied().collect(), holes: Vec::new() };
        assert_eq!(crossed_cells(&reversed, &THREE, 50.0), [[0, 0], [0, 1], [1, 1]]);
        // A side that slants from (0.5, 0.2) to (5.5, 2.8), off the grid's lines, over a side along y = 2.8:
        // the first cell covered 4.875 mm², 54.2 %, and the second 1.625 mm², 18.1 %.
        let slanted = Polygon { outline: vec![p(0.5, 0.2), p(5.5, 2.8), p(0.5, 2.8), p(0.5, 0.2)], holes: Vec::new() };
        assert_eq!(crossed_cells(&slanted, &THREE, 18.0), [[0, 0], [1, 0]]);
        assert_eq!(crossed_cells(&slanted, &THREE, 19.0), [[0, 0]]);
        assert_eq!(crossed_cells(&slanted, &THREE, 54.0), [[0, 0]]);
        assert!(crossed(&slanted, &THREE, 55.0, &mut Budget::DEFAULT.meter()).unwrap().is_empty());
    }

    #[test]
    fn the_grid_lies_where_its_origin_puts_it_and_turns_with_a_quarter_turn() {
        let grid = Grid { origin: [-1.0, 0.5], size: [2.0, 3.0] };
        assert_eq!(grid.corner([1, 1]), p(1.0, 3.5));
        assert_eq!(grid.middle([1, 1]), p(2.0, 5.0));
        let turned = grid.turned();
        assert_eq!(turned, Grid { origin: [-0.5, -1.0], size: [3.0, 2.0] });
        // Turned, each corner is a corner of the turned grid: (1, 3.5) goes to (-3.5, 1).
        assert_eq!(turned.corner([-1, 1]), p(-3.5, 1.0));
    }

    #[test]
    fn req_fill_crs_006_every_cell_the_bounds_reach_is_charged_before_any_is_looked_at() {
        // 12 by 10 cells: 120 units at once, and a budget of 119 runs out before a row is looked at.
        let part = rect((0.0, 0.0), (36.0, 30.0));
        let mut short = Budget { max_work: 119, ..Budget::DEFAULT }.meter();
        assert_eq!(crossed(&part, &THREE, 50.0, &mut short), Err(Exhausted::Work));
        assert_eq!(short.work_left(), 0);
        let cells = crossed(&part, &THREE, 50.0, &mut Budget::DEFAULT.meter()).unwrap();
        assert_eq!((cells.crossed().first(), cells.crossed().last(), cells.crossed().len()), (Some(&[0, 0]), Some(&[11, 9]), 120));
        assert!(!cells.is_empty());
        // A part far finer than its grid's cells reaches one cell, and covers too little of it.
        let speck = rect((1.0, 1.0), (1.1, 1.1));
        assert!(crossed(&speck, &THREE, 1.0, &mut Budget::DEFAULT.meter()).unwrap().is_empty());
        // On a grid of 0.1 mm, a part 1 m wide reaches 10^8 cells: past a budget of 10^7 units, which they
        // spend before any is held.
        let wide = rect((0.0, 0.0), (1000.0, 1000.0));
        let fine = Grid { origin: [0.0, 0.0], size: [0.1, 0.1] };
        let mut meter = Budget { max_work: 10_000_000, ..Budget::DEFAULT }.meter();
        assert_eq!(crossed(&wide, &fine, 50.0, &mut meter), Err(Exhausted::Work));
        // No more cells that sew a cross are held than there are stitches left: 119 of the 120 above.
        let mut few = Budget { max_stitches: 119, ..Budget::DEFAULT }.meter();
        assert_eq!(crossed(&part, &THREE, 50.0, &mut few), Err(Exhausted::Stitches));
        assert!(crossed(&part, &THREE, 50.0, &mut Budget { max_stitches: 120, ..Budget::DEFAULT }.meter()).is_ok());
        // A row of cells is held while they are looked at, so it must fit in the stitches left too: a bar 300 mm
        // long and 0.5 mm tall reaches 100 cells in a row, and covers none of them enough to sew.
        let bar = rect((0.0, 1.0), (300.0, 1.5));
        let mut narrow = Budget { max_stitches: 99, ..Budget::DEFAULT }.meter();
        assert_eq!(crossed(&bar, &THREE, 50.0, &mut narrow), Err(Exhausted::Stitches));
        assert!(crossed(&bar, &THREE, 50.0, &mut Budget { max_stitches: 100, ..Budget::DEFAULT }.meter()).unwrap().is_empty());
    }
}
