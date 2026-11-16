//! How thick a plan sews (`docs/src/design/image-to-embroidery.md` › How thick a design sews,
//! `REQ-AUTO-060` to `REQ-AUTO-062`).
//!
//! A one-needle home machine has no way to say that a design is too thick: its needle deflects, the thread
//! shreds, and the owner finds out on the fabric. Shapes that reach under their neighbours, a base layer
//! and outlines all put thread on thread, so a plan is measured before it is sewn. The measure lives beside
//! the plan, not in the test kit, because the warning that planning gives (`SC-W0913`), the heat map
//! (`stitchcraft-render`) and the numbers of the before and after sheet must all be this one measure.
//!
//! **What is measured** is what the machine sews: each needle point where a machine file puts it, on the
//! machine's grid of 0.1 mm. The engine's own positions have digits that no file shows, and a
//! needle point on the edge of a cell counts on one side or the other by them: measured on those, the
//! warning of VectorCraft's export differed from `stitch export`'s for the same file.
//!
//! **The grid.** The plan's stitches lie on square cells [`CELL`] wide, 10 pixels of the grid that the bare
//! fabric is measured on ([`PIXEL`]), with a cell of room round them. A measure has a value at each **spot**,
//! a corner of the cells: the sum over the 4 cells that meet there, a window [`WINDOW`] across. Spots lie
//! half a window apart, so every square millimetre of the plan is the window of one and part of 8 others.
//! The cells are this coarse because planning measures every design: on cells of 0.2 mm the measure took
//! 80 % as long as planning a test sheet, which VectorCraft's export has no instructions to spare for.
//!
//! **Thread, in fills.** Each stitch is a ribbon of thread [`THREAD`] wide. Its length is followed in steps
//! of a cell at the most, and each step's length is shared between 2 lanes, a quarter of the thread's width
//! to each side of the stitch, each lane adding to the cell under it. A window's thread is its length times
//! [`ROWS`] over the window's area: 1.0 under a fill with the rows of StitchCraft's own fill, 0.25 mm
//! apart, and so "how many fills deep". A satin column, 2 stitches to 0.4 mm, measures 1.25, and the open
//! base layer 0.3. Every role counts, since the needle goes through underlay and travel stitches too. The
//! sources count in objects on each other, and one fill is the object here. In layers of thread, a fill
//! is 1.6: multiply by [`THREAD`] over [`ROWS`]. The 2 lanes keep rows a thread apart from reading 0.8 or
//! 1.2 of what they lay by where they fall in the window.
//!
//! **Needle points per mm².** Each stitch's needle point adds 1 to its cell, but for a lock's: a lock ties
//! 3 or more short stitches in one place on purpose, in every design. This measure finds what the first
//! misses, short stitches and turns bunched in a small patch.
//!
//! **Over a limit** ([`Thickness::over`]) are the spots whose value is the limit or more, in pieces: spots
//! beside each other, along a side of a cell or across it, are one piece, and each stands for the quarter
//! of a square millimetre round it. Only a piece of [`PIECE`] mm² or more counts, 4 spots, since 3 rows that
//! cross in one point are no fault.

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
/// The side of a cell, in millimetres: 10 pixels, half a window.
pub const CELL: f64 = 10.0 * PIXEL;
/// The side of a window, in millimetres, 2 cells: its area is 1 mm², so a sum over it is a value per mm².
pub const WINDOW: f64 = 1.0;
/// The smallest piece over a limit that counts, in square millimetres.
pub const PIECE: f64 = 1.0;
/// How many spots make [`PIECE`].
const PIECE_SPOTS: usize = 4;
/// How many fills lie over a spot in a design that is sewn as meant: every overlap of 2 neighbours makes 2.
pub const ORDINARY: f64 = 2.0;
/// How far from a stitch each of its 2 lanes lies, in millimetres: a quarter of the thread's width.
const LANE: f64 = THREAD / 4.0;
/// How far from a spot the middle of a stitch may lie, beyond half the stitch's length, for the stitch to
/// lay thread in the spot's window, in millimetres: more than half the window's diagonal and a lane.
const NEAR: f64 = 1.0;
/// The most cells a grid has, 500 mm by 500 mm: a larger plan is not measured.
pub const MAX_CELLS: usize = 1_000_000;

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

/// The largest value of a measure and the spot that has it.
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
    /// The box round the largest piece's spots; `None` when there is no piece.
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

/// The cells round a plan's stitches, by row then column, and the spots at their corners, likewise.
#[derive(Clone, Debug, PartialEq)]
struct Grid {
    /// The corner of the first cell where x and y are least: the first spot.
    corner: (f64, f64),
    /// How many columns of cells there are: one fewer than columns of spots.
    columns: usize,
    /// How many rows of cells there are.
    rows: usize,
}

impl Grid {
    /// The grid round `bounds`, with a cell of room on each side; `None` when it would have more than
    /// [`MAX_CELLS`].
    fn round(bounds: Rect) -> Option<Grid> {
        let count = |extent: f64| index((extent / CELL).floor()) + 3;
        let (columns, rows) = (count(bounds.width()), count(bounds.height()));
        let cells = columns.checked_mul(rows)?;
        (cells <= MAX_CELLS).then_some(Grid { corner: (bounds.min().x() - CELL, bounds.min().y() - CELL), columns, rows })
    }

    /// The cell that holds (`x`, `y`); `None` outside the grid, and for a coordinate that is no number.
    fn holding(&self, x: f64, y: f64) -> Option<usize> {
        let along = |value: f64, corner: f64, count: usize| {
            let k = ((value - corner) / CELL).floor();
            (k >= 0.0 && k < count as f64).then(|| index(k))
        };
        Some(along(y, self.corner.1, self.rows)? * self.columns + along(x, self.corner.0, self.columns)?)
    }

    /// How many spots lie in a row: one more than cells.
    fn across(&self) -> usize {
        self.columns + 1
    }

    /// The spot nearest to (`x`, `y`), the further along on a tie; `None` when it is not one of the grid's.
    fn nearest(&self, x: f64, y: f64) -> Option<usize> {
        let along = |value: f64, corner: f64, count: usize| {
            let k = ((value - corner) / CELL + 0.5).floor();
            (k >= 0.0 && k <= count as f64).then(|| index(k))
        };
        Some(along(y, self.corner.1, self.rows)? * self.across() + along(x, self.corner.0, self.columns)?)
    }

    /// Where spot `at` lies.
    fn spot(&self, at: usize) -> Point {
        let (column, row) = (at % self.across(), at / self.across());
        Point::new(self.corner.0 + column as f64 * CELL, self.corner.1 + row as f64 * CELL).unwrap_or(Point::ORIGIN)
    }

    /// The cells of the window of spot `at`: the 4 that meet there, or as many as the grid has of them.
    fn window(&self, at: usize) -> impl Iterator<Item = usize> + '_ {
        let (column, row) = (at % self.across(), at / self.across());
        let cells = move |last: usize, count: usize| last.saturating_sub(1)..(last + 1).min(count);
        cells(row, self.rows).flat_map(move |row| cells(column, self.columns).map(move |column| row * self.columns + column))
    }

    /// The sum of `values`, a value for each cell, over each spot's window, times `scale`: each cell adds to
    /// the 4 spots at its corners.
    fn windows(&self, values: &[f64], scale: f64) -> Vec<f64> {
        let across = self.across();
        let mut sums = vec![0.0; across * (self.rows + 1)];
        for (row, cells) in values.chunks(self.columns.max(1)).enumerate() {
            // Most cells of a design's box are empty, and add nothing.
            for (column, value) in cells.iter().enumerate().filter(|(_, value)| **value != 0.0) {
                let first = row * across + column;
                for spot in [first, first + 1, first + across, first + across + 1] {
                    if let Some(sum) = sums.get_mut(spot) {
                        *sum += value * scale;
                    }
                }
            }
        }
        sums
    }

    /// The spots beside spot `at`, itself among them: those a column and a row away at the most.
    fn beside(&self, at: usize) -> impl Iterator<Item = usize> + '_ {
        let (column, row) = (at % self.across(), at / self.across());
        let spots = move |middle: usize, count: usize| middle.saturating_sub(1)..(middle + 2).min(count);
        spots(row, self.rows + 1).flat_map(move |row| spots(column, self.across()).map(move |column| row * self.across() + column))
    }
}

/// The stitches `plan` sews, each needle point where a machine file puts it (`Point::to_tenths`). A point
/// too far out for the machine's grid stays where it is.
fn sewn(plan: &StitchPlan) -> Vec<SewnStitch> {
    let on_grid = |point: Point| point.to_tenths().map_or(point, |(x, y)| Point::from_tenths(x, y));
    plan.sewn_stitches().into_iter().map(|stitch| SewnStitch { from: on_grid(stitch.from), to: on_grid(stitch.to), ..stitch }).collect()
}

/// How many steps the thread of a stitch `length` long is followed in: one for each cell's width of it
/// begun, and one at the least.
fn steps(length: f64) -> f64 {
    (length / CELL).ceil().max(1.0)
}

/// Lays the thread of `stitch`, `length` long, on `grid` in `steps` steps: `lay` is given a cell and the length of thread
/// to add to it, in millimetres. The places are worked out in cells from the grid's corner, so that a
/// step costs a few additions. A stitch of no length lays none: its direction is no number, and no cell
/// holds such a place.
fn laid(grid: &Grid, stitch: &SewnStitch, length: f64, steps: f64, mut lay: impl FnMut(usize, f64)) {
    let (from, to) = (stitch.from, stitch.to);
    let (across, down) = ((to.x() - from.x()) / CELL, (to.y() - from.y()) / CELL);
    // A lane's distance across the stitch, in cells.
    let (nx, ny) = (-down / length * LANE, across / length * LANE);
    let (x, y) = ((from.x() - grid.corner.0) / CELL, (from.y() - grid.corner.1) / CELL);
    let share = length / steps / 2.0;
    let (columns, rows) = (grid.columns as f64, grid.rows as f64);
    for step in 0..index(steps) {
        let along = (step as f64 + 0.5) / steps;
        let (x, y) = (x + across * along, y + down * along);
        for (x, y) in [(x + nx, y + ny), (x - nx, y - ny)] {
            if x >= 0.0 && x < columns && y >= 0.0 && y < rows {
                lay(index(y) * grid.columns + index(x), share);
            }
        }
    }
}

/// How thick a plan sews, spot by spot (see the module's docs).
#[derive(Clone, Debug, PartialEq)]
pub struct Thickness {
    grid: Grid,
    /// The thread in each spot's window, in fills.
    fills: Vec<f64>,
    /// The needle points in each spot's window, per mm².
    points: Vec<f64>,
}

impl Thickness {
    /// The thickness of `plan`; `None` for a plan that sews no stitch. A unit of work for each stitch, each
    /// step along it, and 3 for each cell. A plan whose grid would have more than [`MAX_CELLS`] is not
    /// measured, as one whose measuring runs out of work.
    pub fn of(plan: &StitchPlan, meter: &mut Meter) -> Result<Option<Thickness>, Exhausted> {
        Thickness::of_stitches(&sewn(plan), meter)
    }

    /// The thickness of the stitches `sewn`, as they are: for a picture of what a machine file sews, whose
    /// stitches are read from the file.
    pub fn of_stitches(sewn: &[SewnStitch], meter: &mut Meter) -> Result<Option<Thickness>, Exhausted> {
        meter.charge(units(sewn.len()))?;
        let Some(bounds) = Rect::around(sewn.iter().flat_map(|stitch| [stitch.from, stitch.to])) else { return Ok(None) };
        let grid = Grid::round(bounds).ok_or(Exhausted::Work)?;
        let cells = grid.columns * grid.rows;
        meter.charge(units(cells).saturating_mul(3))?;
        let (mut thread, mut holes) = (vec![0.0; cells], vec![0.0; cells]);
        for stitch in sewn {
            let length = stitch.length();
            let steps = steps(length);
            meter.charge(units(index(steps)))?;
            laid(&grid, stitch, length, steps, |at, length| {
                if let Some(cell) = thread.get_mut(at) {
                    *cell += length;
                }
            });
            let hole = grid.holding(stitch.to.x(), stitch.to.y()).filter(|_| !stitch.is_lock());
            if let Some(cell) = hole.and_then(|at| holes.get_mut(at)) {
                *cell += 1.0;
            }
        }
        // A window is 1 mm², so its sums are per square millimetre as they are.
        Ok(Some(Thickness { fills: grid.windows(&thread, ROWS), points: grid.windows(&holes, 1.0), grid }))
    }

    /// The values of `measure`, by row then column.
    fn values(&self, measure: Measure) -> &[f64] {
        match measure {
            Measure::Fills => &self.fills,
            Measure::Points => &self.points,
        }
    }

    /// How many columns and rows of spots there are.
    pub fn size(&self) -> (usize, usize) {
        (self.grid.across(), self.grid.rows + 1)
    }

    /// The value of `measure` at the spot nearest to `at`: 0 outside the grid.
    pub fn at(&self, measure: Measure, at: Point) -> f64 {
        self.grid.nearest(at.x(), at.y()).and_then(|spot| self.values(measure).get(spot)).copied().unwrap_or(0.0)
    }

    /// The largest value of `measure`, and the last spot that has it.
    pub fn most(&self, measure: Measure) -> Spot {
        let most = self.values(measure).iter().enumerate().max_by(|a, b| a.1.total_cmp(b.1));
        most.map_or(Spot { value: 0.0, at: Point::ORIGIN }, |(at, value)| Spot { value: *value, at: self.grid.spot(at) })
    }

    /// What of the plan has `limit` of `measure` or more, in pieces of [`PIECE`] or more. A unit of work for
    /// each spot.
    pub fn over(&self, measure: Measure, limit: f64, meter: &mut Meter) -> Result<Over, Exhausted> {
        let values = self.values(measure);
        meter.charge(units(values.len()))?;
        // The spots still to look at: those at the limit or over it that are in no piece yet.
        let mut left: Vec<bool> = values.iter().map(|value| *value >= limit).collect();
        let mut over = Over::default();
        for start in 0..left.len() {
            if !left.get(start).copied().unwrap_or(false) {
                continue;
            }
            let mut piece = vec![start];
            left.iter_mut().skip(start).take(1).for_each(|spot| *spot = false);
            let mut next = 0;
            while let Some(&at) = piece.get(next) {
                next += 1;
                for beside in self.grid.beside(at) {
                    if let Some(spot) = left.get_mut(beside).filter(|spot| **spot) {
                        *spot = false;
                        piece.push(beside);
                    }
                }
            }
            if piece.len() < PIECE_SPOTS {
                continue;
            }
            let area = piece.len() as f64 * CELL * CELL;
            over.pieces += 1;
            over.area += area;
            if area > over.largest {
                over.largest = area;
                over.at = Rect::around(piece.iter().map(|&at| self.grid.spot(at)));
            }
        }
        Ok(over)
    }

    /// What of `plan`, the plan this is the thickness of, lies in the window of the spot nearest to `at`:
    /// each element's thread in each colour block, in sewing order. A unit of work for each stitch and each
    /// step along it. Only a stitch near the spot is followed.
    pub fn stack(&self, plan: &StitchPlan, at: Point, meter: &mut Meter) -> Result<Vec<Layer>, Exhausted> {
        let Some(spot) = self.grid.nearest(at.x(), at.y()) else { return Ok(Vec::new()) };
        let (window, middle): (Vec<usize>, Point) = (self.grid.window(spot).collect(), self.grid.spot(spot));
        let mut layers: BTreeMap<(usize, Option<ElementRef>), (f64, f64)> = BTreeMap::new();
        for stitch in sewn(plan) {
            let length = stitch.length();
            let steps = steps(length);
            meter.charge(units(index(steps)).saturating_add(1))?;
            if stitch.from.lerp(stitch.to, 0.5).distance(middle) > length / 2.0 + NEAR {
                continue;
            }
            let entry = plan.blocks.get(stitch.block).and_then(|block| block.stitches.get(stitch.index));
            let key = (stitch.block, entry.and_then(|entry| entry.origin.element));
            laid(&self.grid, &stitch, length, steps, |cell, length| {
                if window.contains(&cell) {
                    let layer = layers.entry(key).or_default();
                    if stitch.role == Role::Top {
                        layer.0 += length * ROWS;
                    } else {
                        layer.1 += length * ROWS;
                    }
                }
            });
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
