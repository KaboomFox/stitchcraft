//! How thick a plan sews (`docs/src/design/image-to-embroidery.md` › How thick a design sews,
//! `REQ-AUTO-060` to `REQ-AUTO-062`).
//!
//! A one-needle home machine has no way to say that a design is too thick: its needle deflects, the thread
//! shreds, and the owner finds out on the fabric. Shapes that reach under their neighbours, a base layer
//! and outlines all put thread on thread, so a plan is measured before it is sewn. The measure lives beside
//! the plan, not in the test kit, because the warning that planning gives (`SC-W0913`), the heat map
//! (`stitchcraft-render`) and the numbers of the before and after sheet must all be this one measure.
//!
//! **The grid.** The plan's stitches lie on square cells [`CELL`] wide, 4 pixels of the grid that the bare
//! fabric is measured on ([`PIXEL`]), with [`MARGIN`] cells of room round them. Each measure is a sum over
//! the window of 5 by 5 cells round a cell, [`WINDOW`] across, so that it has a value at every cell.
//!
//! **Thread, in fills.** Each stitch is a ribbon of thread [`THREAD`] wide. Its length is followed in steps
//! of half a cell, and each step's length is shared among 4 lanes across the ribbon's width, each lane
//! adding to the cell under it. A window's thread is its length times [`ROWS`] over the window's area: 1.0
//! under a fill with the rows of StitchCraft's own fill, 0.25 mm apart, and so "how many fills deep". A
//! satin column, 2 stitches to 0.4 mm, measures 1.25, and the open base layer 0.3. Every role counts,
//! since the needle goes through underlay and travel stitches too. The sources count in objects on each
//! other, and one fill is the object here. In layers of thread, a fill is 1.6: multiply by
//! [`THREAD`] over [`ROWS`]. Sharing a ribbon among lanes keeps a fill from reading more or less by where
//! its rows fall in the window.
//!
//! **Needle points per mm².** Each stitch's needle point adds 1 to its cell, but for a lock's: a lock ties
//! 3 or more short stitches in one place on purpose, in every design. This measure finds what the first
//! misses, short stitches and turns bunched in a small patch.
//!
//! **Over a limit** ([`Thickness::over`]) are the cells whose value is the limit or more, in pieces: cells
//! that touch at a side or a corner are one piece, and only a piece of [`PIECE`] mm² or more counts, since
//! 3 rows that cross in one point are no fault.

use std::collections::BTreeMap;

use stitchcraft_core::{Exhausted, Meter, Point, Rect};

use crate::plan::{ElementRef, Role, SewnStitch, StitchPlan};

/// The width of 40-weight embroidery thread on fabric, in millimetres, which a fill's rows lie apart.
pub const THREAD: f64 = 0.4;
/// How far apart the rows of an ordinary fill lie, in millimetres: the default of a fill's `row_spacing_mm`,
/// which the engine's cases hold it to. A window's thread is in fills of such rows.
pub const ROWS: f64 = 0.25;
/// The side of a pixel of the grid that the bare fabric is measured on, in millimetres.
pub const PIXEL: f64 = 0.05;
/// The side of a cell, in millimetres: 4 pixels.
pub const CELL: f64 = 4.0 * PIXEL;
/// How many cells a window is across.
const ACROSS: usize = 5;
/// The side of a window, in millimetres.
pub const WINDOW: f64 = 1.0;
/// The smallest piece over a limit that counts, in square millimetres.
pub const PIECE: f64 = 1.0;
/// How many cells make [`PIECE`].
const PIECE_CELLS: usize = 25;
/// How many fills lie over a spot in a design that is sewn as meant: every overlap of 2 neighbours makes 2.
pub const ORDINARY: f64 = 2.0;
/// How many lanes a ribbon of thread is shared among across its width.
const LANES: u32 = 4;
/// How many cells of room lie round the stitches: a window half a millimetre from a stitch still holds it.
pub const MARGIN: usize = 4;
/// The most cells a grid has, 500 mm by 500 mm: a larger plan is not measured.
pub const MAX_CELLS: usize = 6_250_000;

/// The limits a plan is held to. They are starting values, which a sew-out has to confirm.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Limits {
    /// How many fills of thread over one spot a warning names.
    pub warn_fills: f64,
    /// How many fills of thread over one spot fail.
    pub fail_fills: f64,
    /// How many needle points per mm² a warning names.
    pub warn_points: f64,
    /// How many needle points per mm² fail.
    pub fail_points: f64,
}

impl Limits {
    /// The limits for a 75/11 needle and 40-weight polyester thread: 3 fills on each other is what
    /// Brother's troubleshooting names as a fault of the design, and 6 layers is where one vendor's density
    /// map turns red.
    /// No source gives a number of needle points. Measured on 2026-11-15, the row ends of a fill lie 5 to a
    /// square millimetre along its edge, so 15 and 30 are what the edges of 3 and of 6 fills on each other
    /// make, and 2 fills, with 10 or 11, stay under.
    pub const DEFAULT: Limits = Limits { warn_fills: 3.0, fail_fills: 6.0, warn_points: 15.0, fail_points: 30.0 };

    /// The step of the heat map that `fills` of thread over a spot belong to.
    pub fn step(&self, fills: f64) -> Step {
        if fills >= self.fail_fills.max(self.warn_fills) {
            Step::Failing
        } else if fills >= self.warn_fills {
            Step::Thick
        } else if fills >= ORDINARY {
            Step::Doubled
        } else if fills > 0.0 {
            Step::Ordinary
        } else {
            Step::Bare
        }
    }
}

/// A step of the heat map.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Step {
    /// No thread.
    Bare,
    /// Thread, under [`ORDINARY`] fills of it.
    Ordinary,
    /// [`ORDINARY`] fills or more, under the warning's limit.
    Doubled,
    /// The warning's limit or more.
    Thick,
    /// The failing limit or more.
    Failing,
}

/// One of the 2 measures.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Measure {
    /// Thread, in fills.
    Fills,
    /// Needle points per mm².
    Points,
}

/// The largest value of a measure and the middle of the first cell that has it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Spot {
    /// The value.
    pub value: f64,
    /// Where, in the plan's millimetres.
    pub at: Point,
}

/// What of a plan is at a limit or over it.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Over {
    /// How many pieces of [`PIECE`] or more there are.
    pub pieces: usize,
    /// Their area together, in square millimetres.
    pub area: f64,
    /// The largest piece's area, in square millimetres: 0 when there is none.
    pub largest: f64,
    /// The box round the middles of the largest piece's cells; `None` when there is no piece.
    pub at: Option<Rect>,
}

/// The thread of one element in one colour block that lies in a window.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Layer {
    /// The colour block.
    pub block: usize,
    /// The element; `None` for stitches of no element.
    pub element: Option<ElementRef>,
    /// Its top stitches, in fills.
    pub top: f64,
    /// Its other stitches, underlay, travel and locks, in fills.
    pub under: f64,
}

/// What a plan measures against its limits ([`Thickness::report`]).
#[derive(Clone, Debug, PartialEq)]
pub struct Report {
    /// The thickest spot.
    pub fills: Spot,
    /// The spot with the most needle points.
    pub points: Spot,
    /// What is at the warning's limit of thread or over it.
    pub thick: Over,
    /// What is at the failing limit of thread or over it.
    pub failing: Over,
    /// What is at the warning's limit of needle points or over it.
    pub dense: Over,
    /// What is at the failing limit of needle points or over it.
    pub pierced: Over,
    /// What lies in the window of the thickest spot, in sewing order.
    pub stack: Vec<Layer>,
}

impl Report {
    /// Whether a warning is due: a piece is at a warning's limit or over it.
    pub fn warns(&self) -> bool {
        self.thick.pieces + self.dense.pieces > 0
    }
}

/// `value`, a whole number, as an index: 0 below 0 and for NaN, saturated far above.
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)] // a saturating cast of a whole number
fn index(value: f64) -> usize {
    value.max(0.0) as usize
}

/// `count` as work units.
fn units(count: usize) -> u64 {
    u64::try_from(count).unwrap_or(u64::MAX)
}

/// The cells round a plan's stitches, by row then column.
#[derive(Clone, Debug, PartialEq)]
struct Grid {
    /// The corner of the first cell where x and y are least.
    corner: (f64, f64),
    columns: usize,
    rows: usize,
}

impl Grid {
    /// The grid round `bounds`; `None` when it would have more than [`MAX_CELLS`].
    fn round(bounds: Rect) -> Option<Grid> {
        let room = MARGIN as f64 * CELL;
        let count = |extent: f64| index((extent / CELL).floor()) + 1 + 2 * MARGIN;
        let (columns, rows) = (count(bounds.width()), count(bounds.height()));
        let cells = columns.checked_mul(rows)?;
        (cells <= MAX_CELLS).then_some(Grid { corner: (bounds.min().x() - room, bounds.min().y() - room), columns, rows })
    }

    /// The column and the row of the cell that holds (`x`, `y`); `None` outside the grid, and for a
    /// coordinate that is no number.
    fn place(&self, x: f64, y: f64) -> Option<(usize, usize)> {
        let along = |value: f64, corner: f64, count: usize| {
            let k = ((value - corner) / CELL).floor();
            (k >= 0.0 && k < count as f64).then(|| index(k))
        };
        Some((along(x, self.corner.0, self.columns)?, along(y, self.corner.1, self.rows)?))
    }

    /// The cell that holds (`x`, `y`).
    fn holding(&self, x: f64, y: f64) -> Option<usize> {
        self.place(x, y).map(|(column, row)| row * self.columns + column)
    }

    /// The middle of cell `at`.
    fn centre(&self, at: usize) -> Point {
        let (column, row) = (at % self.columns, at / self.columns);
        let middle = |corner: f64, k: usize| corner + (k as f64 + 0.5) * CELL;
        Point::new(middle(self.corner.0, column), middle(self.corner.1, row)).unwrap_or(Point::ORIGIN)
    }

    /// The cells of the window round cell `at`.
    fn window(&self, at: usize) -> impl Iterator<Item = usize> + '_ {
        self.within(at, ACROSS / 2)
    }

    /// The cells no further than `reach` columns and rows from cell `at`, itself among them, in rising order.
    fn within(&self, at: usize, reach: usize) -> impl Iterator<Item = usize> + '_ {
        let (column, row) = (at % self.columns, at / self.columns);
        let columns = column.saturating_sub(reach)..(column + reach + 1).min(self.columns);
        let rows = row.saturating_sub(reach)..(row + reach + 1).min(self.rows);
        rows.flat_map(move |row| columns.clone().map(move |column| row * self.columns + column))
    }
}

/// Lays the thread of `stitch` on `grid`: `lay` is given a cell and the length of thread to add to it, in
/// millimetres. A unit of work for each step. A stitch of no length lays none: its direction is no number,
/// and no cell holds such a place.
fn laid(grid: &Grid, stitch: &SewnStitch, meter: &mut Meter, mut lay: impl FnMut(usize, f64)) -> Result<(), Exhausted> {
    let (from, to) = (stitch.from, stitch.to);
    let length = from.distance(to);
    let steps = (length / (CELL / 2.0)).ceil().max(1.0);
    meter.charge(units(index(steps)))?;
    let (ax, ay) = ((to.x() - from.x()) / length, (to.y() - from.y()) / length);
    let share = length / steps / f64::from(LANES);
    for step in 0..index(steps) {
        let along = (step as f64 + 0.5) / steps * length;
        for lane in 0..LANES {
            let across = (f64::from(lane) + 0.5) / f64::from(LANES) * THREAD - THREAD / 2.0;
            if let Some(at) = grid.holding(from.x() + ax * along - ay * across, from.y() + ay * along + ax * across) {
                lay(at, share);
            }
        }
    }
    Ok(())
}

/// How thick a plan sews, cell by cell (see the module's docs).
#[derive(Clone, Debug, PartialEq)]
pub struct Thickness {
    grid: Grid,
    /// The thread in each cell's window, in fills.
    fills: Vec<f64>,
    /// The needle points in each cell's window, per mm².
    points: Vec<f64>,
}

impl Thickness {
    /// The thickness of `plan`; `None` for a plan that sews no stitch. A unit of work for each stitch, each
    /// step of half a cell along it, and 3 for each cell. A plan whose grid would have more than
    /// [`MAX_CELLS`] is not measured, as one whose measuring runs out of work.
    pub fn of(plan: &StitchPlan, meter: &mut Meter) -> Result<Option<Thickness>, Exhausted> {
        let sewn = plan.sewn_stitches();
        meter.charge(units(sewn.len()))?;
        let Some(bounds) = Rect::around(sewn.iter().flat_map(|stitch| [stitch.from, stitch.to])) else { return Ok(None) };
        let grid = Grid::round(bounds).ok_or(Exhausted::Work)?;
        let cells = grid.columns * grid.rows;
        meter.charge(units(cells).saturating_mul(3))?;
        let (mut thread, mut holes) = (vec![0.0; cells], vec![0.0; cells]);
        for stitch in &sewn {
            laid(&grid, stitch, meter, |at, length| thread.iter_mut().skip(at).take(1).for_each(|cell| *cell += length))?;
            let hole = grid.holding(stitch.to.x(), stitch.to.y()).filter(|_| !stitch.is_lock());
            holes.iter_mut().skip(hole.unwrap_or(cells)).take(1).for_each(|cell| *cell += 1.0);
        }
        let summed = |values: &[f64], scale: f64| -> Vec<f64> {
            (0..cells).map(|at| grid.window(at).filter_map(|cell| values.get(cell)).sum::<f64>() * scale).collect()
        };
        let area = WINDOW * WINDOW;
        Ok(Some(Thickness { fills: summed(&thread, ROWS / area), points: summed(&holes, 1.0 / area), grid }))
    }

    /// The values of `measure`, by row then column.
    fn values(&self, measure: Measure) -> &[f64] {
        match measure {
            Measure::Fills => &self.fills,
            Measure::Points => &self.points,
        }
    }

    /// How many columns and rows of cells there are.
    pub fn size(&self) -> (usize, usize) {
        (self.grid.columns, self.grid.rows)
    }

    /// The value of `measure` in the window round the cell that holds `at`: 0 outside the grid.
    pub fn at(&self, measure: Measure, at: Point) -> f64 {
        self.grid.holding(at.x(), at.y()).and_then(|cell| self.values(measure).get(cell)).copied().unwrap_or(0.0)
    }

    /// The largest value of `measure`, and the middle of the last cell that has it.
    pub fn most(&self, measure: Measure) -> Spot {
        let most = self.values(measure).iter().enumerate().max_by(|a, b| a.1.total_cmp(b.1));
        most.map_or(Spot { value: 0.0, at: Point::ORIGIN }, |(at, value)| Spot { value: *value, at: self.grid.centre(at) })
    }

    /// What of the plan has `limit` of `measure` or more, in pieces of [`PIECE`] or more. A unit of work for
    /// each cell.
    pub fn over(&self, measure: Measure, limit: f64, meter: &mut Meter) -> Result<Over, Exhausted> {
        let values = self.values(measure);
        meter.charge(units(values.len()))?;
        // The cells still to look at: those at the limit or over it that are in no piece yet.
        let mut left: Vec<bool> = values.iter().map(|value| *value >= limit).collect();
        let mut over = Over::default();
        for start in 0..left.len() {
            if !left.get(start).copied().unwrap_or(false) {
                continue;
            }
            let mut piece = vec![start];
            left.iter_mut().skip(start).take(1).for_each(|cell| *cell = false);
            let mut next = 0;
            while let Some(&at) = piece.get(next) {
                next += 1;
                // The cells that touch it, at a side or a corner.
                for beside in self.grid.within(at, 1) {
                    if let Some(cell) = left.get_mut(beside).filter(|cell| **cell) {
                        *cell = false;
                        piece.push(beside);
                    }
                }
            }
            if piece.len() < PIECE_CELLS {
                continue;
            }
            let area = piece.len() as f64 * CELL * CELL;
            over.pieces += 1;
            over.area += area;
            if area > over.largest {
                over.largest = area;
                over.at = Rect::around(piece.iter().map(|&at| self.grid.centre(at)));
            }
        }
        Ok(over)
    }

    /// What of `plan`, the plan this is the thickness of, lies in the window round the cell that holds `at`:
    /// each element's thread in each colour block, in sewing order. A unit of work for each stitch and each
    /// step along it.
    pub fn stack(&self, plan: &StitchPlan, at: Point, meter: &mut Meter) -> Result<Vec<Layer>, Exhausted> {
        let Some(middle) = self.grid.holding(at.x(), at.y()) else { return Ok(Vec::new()) };
        let window: Vec<usize> = self.grid.window(middle).collect();
        let mut layers: BTreeMap<(usize, Option<ElementRef>), (f64, f64)> = BTreeMap::new();
        let scale = ROWS / (WINDOW * WINDOW);
        for stitch in plan.sewn_stitches() {
            meter.charge(1)?;
            let entry = plan.blocks.get(stitch.block).and_then(|block| block.stitches.get(stitch.index));
            let key = (stitch.block, entry.and_then(|entry| entry.origin.element));
            laid(&self.grid, &stitch, meter, |cell, length| {
                if window.binary_search(&cell).is_ok() {
                    let layer = layers.entry(key).or_default();
                    if stitch.role == Role::Top {
                        layer.0 += length * scale;
                    } else {
                        layer.1 += length * scale;
                    }
                }
            })?;
        }
        Ok(layers.into_iter().map(|((block, element), (top, under))| Layer { block, element, top, under }).collect())
    }

    /// What `plan`, the plan this is the thickness of, measures against `limits`.
    pub fn report(&self, plan: &StitchPlan, limits: &Limits, meter: &mut Meter) -> Result<Report, Exhausted> {
        let fills = self.most(Measure::Fills);
        Ok(Report {
            fills,
            points: self.most(Measure::Points),
            thick: self.over(Measure::Fills, limits.warn_fills, meter)?,
            failing: self.over(Measure::Fills, limits.fail_fills.max(limits.warn_fills), meter)?,
            dense: self.over(Measure::Points, limits.warn_points, meter)?,
            pierced: self.over(Measure::Points, limits.fail_points.max(limits.warn_points), meter)?,
            stack: self.stack(plan, fills.at, meter)?,
        })
    }
}

#[cfg(test)]
mod tests;
