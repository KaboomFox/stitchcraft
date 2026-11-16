//! Runs under what is sewn later, in place of jump threads (`REQ-AUTO-053`, M13.17).
//!
//! Design: `docs/src/design/image-to-embroidery.md` › Runs in place of jumps. A machine with one needle cuts
//! no thread at a jump, so each jump between 2 elements of one thread leaves a thread to cut by hand, and a
//! traced photo has scores of them. Where shapes sewn later cover the way from one element to the next, the
//! needle can sew a running stitch along it instead: the later shapes hide it. Plan assembly asks here at
//! each move between 2 groups that automatic digitizing made ([`Routing::run`]), since it alone knows where
//! the needle is and whether it would sew on or jump.
//!
//! **What covers** is read from the stitches themselves and not from the shapes, so that it is true of
//! whatever the generators sewed: a sparse fill covers nothing. [`Cover`] paints every stitch of the top
//! layers, its thread [`THREAD`] wide, on 2 grids of [`CELL`]: the last group of a fill or satin column that
//! covers each cell, and the last group of any kind, whose colour is what shows there. A cell is hidden from
//! a move when it and the cells within [`ROOM`] of it are covered by groups sewn after the move: the run then
//! lies 0.4 mm inside what covers it, as an underlay does. A run keeps to hidden cells, and to cells where
//! stitches of its own colour sewn later are what shows: the outline that follows a fill lies on the run as
//! a second pass does. Within [`STUB`] of its 2 ends it may also cross cells of its own colour sewn before,
//! or covered later without that room: an element ends on its own stitches, and starts on its edge.
//!
//! **The way** is the shortest over the cells, found from the cheapest row first (A*), no longer than
//! [`longest`] allows, then pulled straight wherever the straight line keeps to the cells, and sewn with
//! stitches of [`STITCH`] at most, a needle point at each corner left. Everything is whole-number arithmetic
//! on cells but the sampling of lines, so a design routes the same on every machine.
//!
//! **A move that is sewn on**, within the collapse length, and shows on another colour for [`SHOWS`] or
//! more becomes a run where one is hidden. Where none is, it stays a stitch: a jump there would leave a
//! thread to cut no longer than the stitch, and the dogs measured 5 to 15 more of them each.
//!
//! **Work.** Painting and every search share the budget's work once for the design. When it runs out the
//! moves from there on are left as they were, and `SC-I0914` names the element where it did.

use std::cmp::Reverse;
use std::collections::{BTreeSet, BinaryHeap};

use stitchcraft_core::{Budget, Code, Diagnostic, ElementId, Exhausted, Meter, Mm, Point};
use stitchcraft_params::Family;
use stitchcraft_plan::Rgb;

use crate::assemble::threaded;
use crate::auto::OUTLINE_WIDTH as THREAD;
use crate::design::Element;
use crate::generate::Generated;
use crate::generators::Layer;

/// The side of a cell of the grids, in millimetres: half a thread's width.
const CELL: f64 = 0.2;
/// The longest stitch of a run. A run lies under other stitches, where longer stitches lay less thread.
pub(crate) const STITCH: Mm = Mm::from_tenths(25);
/// How far from its 2 ends a run may leave the hidden cells, in millimetres. Less than the 1 mm at which a
/// stretch of thread counts as showing (`REQ-AUTO-026`).
const STUB: f64 = 0.8;
/// The stretch of a move sewn on, in millimetres, that shows on another colour before a run is looked for:
/// the stretch at which a stitch counts as showing (`REQ-AUTO-026`).
pub(crate) const SHOWS: f64 = 1.0;
/// How many cells on every side of a hidden cell are covered later too: 0.4 mm, as far as an underlay keeps
/// from a fill's edge. The rows of a fill end at its edge, where they cover least, and the fabric pulls
/// them in.
const ROOM: i64 = 2;
/// How far round the box of a move's 2 ends a run is searched for, in millimetres.
const WINDOW: f64 = 10.0;
/// The work a cell of the grids costs to make. The default budget then allows grids of 5 million cells, a
/// design of 450 mm by 450 mm, and no design however large is given more memory than that.
const CELL_WORK: u64 = 100;
/// The cost of a step to a cell beside and to one across a corner, in tenths of a cell.
const STEP: [u32; 2] = [10, 14];
/// The 8 steps from a cell to its neighbours.
const STEPS: [(i64, i64); 8] = [(1, 0), (0, 1), (-1, 0), (0, -1), (1, 1), (-1, 1), (-1, -1), (1, -1)];

/// The longest run that may replace a move of `straight` millimetres, in millimetres: twice the move and
/// 10 mm. A longer way round lays more thread under the shapes than the jump is worth.
pub(crate) fn longest(straight: f64) -> f64 {
    2.0 * straight + 10.0
}

/// A cell of the grids, by column and row. Signed, so that a cell beside the grid is a cell that nothing
/// covers.
type Cell = (i64, i64);

/// What the stitches of a design cover, on grids of [`CELL`] (see the module's docs).
#[derive(Debug, Default)]
struct Cover {
    /// The corner of the grids with the least coordinates.
    min: (f64, f64),
    columns: i64,
    rows: i64,
    /// For each cell, by row then column, one more than the place of the last group of a fill or a satin
    /// column whose thread covers it; 0 where none does.
    later: Vec<u32>,
    /// The same of the last group of any kind.
    top: Vec<u32>,
    /// The colour of each group's thread, by its place.
    colours: Vec<Rgb>,
}

impl Cover {
    /// The cover of `elements` as generated, in the order they sew: of each group that sews a stitch, as
    /// plan assembly counts them ([`threaded`]). Underlays and the base layer cover nothing. A cell costs
    /// [`CELL_WORK`] of `meter`'s work to make, and each stitch a unit for each cell of its box.
    fn painted(elements: &[(&Element, Generated)], meter: &mut Meter) -> Result<Cover, Exhausted> {
        let groups =
            || elements.iter().flat_map(|(element, generated)| threaded(element, generated).into_iter().map(move |group| (group, generated)));
        let points = || groups().flat_map(|(group, _)| group.stitches.iter().copied());
        let low = |of: fn(Point) -> f64| points().map(of).fold(f64::INFINITY, f64::min) - 1.0;
        let high = |of: fn(Point) -> f64| points().map(of).fold(f64::NEG_INFINITY, f64::max) + 1.0;
        let min = (low(Point::x), low(Point::y));
        let (columns, rows) = (cells(high(Point::x) - min.0), cells(high(Point::y) - min.1));
        let count = u64::try_from(columns.saturating_mul(rows)).unwrap_or(0);
        meter.charge(count.saturating_mul(CELL_WORK))?;
        let size = usize::try_from(count).unwrap_or(0);
        let mut cover = Cover { min, columns, rows, later: vec![0; size], top: vec![0; size], colours: Vec::new() };
        for (group, generated) in groups() {
            cover.colours.push(group.thread.color);
            if !matches!(generated.layer, Layer::Whole | Layer::Top) {
                continue;
            }
            let place = u32::try_from(cover.colours.len()).unwrap_or(u32::MAX);
            let dense = generated.stitch_type.family() != Family::Stroke;
            for stitch in group.stitches.windows(2) {
                let [from, to] = [stitch.first().copied().unwrap_or(Point::ORIGIN), stitch.last().copied().unwrap_or(Point::ORIGIN)];
                cover.paint([from, to], place, dense, meter)?;
            }
        }
        Ok(cover)
    }

    /// Marks the cells whose centres lie within half of [`THREAD`] of the stitch `[from, to]` as covered by
    /// the group with `place`, and by a fill or satin column when it is `dense`.
    fn paint(&mut self, [from, to]: [Point; 2], place: u32, dense: bool, meter: &mut Meter) -> Result<(), Exhausted> {
        let reach = THREAD.get() / 2.0;
        let (left, top) = self.cell((from.x().min(to.x()) - reach, from.y().min(to.y()) - reach));
        let (right, bottom) = self.cell((from.x().max(to.x()) + reach, from.y().max(to.y()) + reach));
        meter.charge(u64::try_from((right - left + 1) * (bottom - top + 1)).unwrap_or(u64::MAX))?;
        for row in top..=bottom {
            for column in left..=right {
                if apart(self.centre((column, row)), [from, to]) > reach * reach {
                    continue;
                }
                let Some(at) = self.index((column, row)) else { continue };
                if dense && let Some(later) = self.later.get_mut(at) {
                    *later = place;
                }
                if let Some(top) = self.top.get_mut(at) {
                    *top = place;
                }
            }
        }
        Ok(())
    }

    /// The cell that holds the point `at`.
    fn cell(&self, at: (f64, f64)) -> Cell {
        (whole(((at.0 - self.min.0) / CELL).floor()), whole(((at.1 - self.min.1) / CELL).floor()))
    }

    /// The centre of `cell`.
    #[allow(clippy::cast_precision_loss)] // A cell's column and row are far below 2^52.
    fn centre(&self, (column, row): Cell) -> (f64, f64) {
        (self.min.0 + (column as f64 + 0.5) * CELL, self.min.1 + (row as f64 + 0.5) * CELL)
    }

    /// The place of `cell` in the grids; `None` for a cell beside them.
    fn index(&self, (column, row): Cell) -> Option<usize> {
        ((0..self.columns).contains(&column) && (0..self.rows).contains(&row))
            .then(|| usize::try_from(row * self.columns + column).unwrap_or(usize::MAX))
    }

    /// What `grid` holds for `cell`; 0 for a cell beside it.
    fn at(&self, grid: &[u32], cell: Cell) -> u32 {
        self.index(cell).and_then(|at| grid.get(at)).copied().unwrap_or(0)
    }
}

/// How many cells of [`CELL`] hold `length` millimetres.
fn cells(length: f64) -> i64 {
    whole((length / CELL).ceil())
}

/// `value`, a whole number, as an integer; 0 for one that is not finite, as the size of a design without a
/// stitch is.
#[allow(clippy::cast_possible_truncation)] // Finite, and a design's size in cells is far below 2^63.
fn whole(value: f64) -> i64 {
    if value.is_finite() { value as i64 } else { 0 }
}

/// The square of the distance from `point` to the stitch `[from, to]`.
fn apart(point: (f64, f64), [from, to]: [Point; 2]) -> f64 {
    let (dx, dy) = (to.x() - from.x(), to.y() - from.y());
    let (px, py) = (point.0 - from.x(), point.1 - from.y());
    let length = dx * dx + dy * dy;
    let along = if length > 0.0 { ((px * dx + py * dy) / length).clamp(0.0, 1.0) } else { 0.0 };
    let (x, y) = (px - along * dx, py - along * dy);
    x * x + y * y
}

/// One move of the needle, from the end of one group to the start of the next, over what covers.
struct Move<'c> {
    cover: &'c Cover,
    ends: [Point; 2],
    /// The cells that hold the ends.
    cells: [Cell; 2],
    /// A cell is covered later when its `later` is above this: the place of the group the move leads to.
    after: u32,
    colour: Rgb,
}

impl<'c> Move<'c> {
    /// The move between `ends` to the group with `place`, sewn in a thread of `colour`.
    fn new(cover: &'c Cover, ends: [Point; 2], place: usize, colour: Rgb) -> Self {
        let cells = ends.map(|end| cover.cell((end.x(), end.y())));
        Move { cover, ends, cells, after: u32::try_from(place).unwrap_or(u32::MAX), colour }
    }

    /// Whether a fill or satin column sewn after the move covers `cell`.
    fn later(&self, cell: Cell) -> bool {
        self.cover.at(&self.cover.later, cell) > self.after
    }

    /// Whether what shows at `cell` has the move's colour.
    fn own(&self, cell: Cell) -> bool {
        let top = self.cover.at(&self.cover.top, cell);
        top > 0 && usize::try_from(top - 1).ok().and_then(|group| self.cover.colours.get(group)) == Some(&self.colour)
    }

    /// Whether `cell` and the cells within [`ROOM`] of it are covered later.
    fn hidden(&self, (column, row): Cell) -> bool {
        (-ROOM..=ROOM).all(|across| (-ROOM..=ROOM).all(|down| self.later((column + across, row + down))))
    }

    /// Whether a run may cross `cell`: hidden, or one of the ends' own cells, or showing the move's colour in
    /// stitches sewn later, or within [`STUB`] of an end and of the move's colour or covered later.
    fn open(&self, cell: Cell) -> bool {
        let centre = self.cover.centre(cell);
        let near = self.ends.iter().any(|&end| apart(centre, [end, end]) <= STUB * STUB);
        self.hidden(cell)
            || self.cells.contains(&cell)
            || (self.own(cell) && (near || self.cover.at(&self.cover.top, cell) > self.after))
            || (near && self.later(cell))
    }

    /// The length of the move's straight stitch, in steps of half a cell, and the step's length.
    fn steps(from: Point, to: Point) -> (u32, f64) {
        let length = from.distance(to);
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)] // A length in half cells, from 1 on.
        let steps = (length / (CELL / 2.0)).ceil().max(1.0) as u32;
        (steps, length / f64::from(steps))
    }

    /// The longest stretch of the straight stitch between the ends that shows, in millimetres: on a colour
    /// that is not the move's, or on none, where nothing sewn later covers it.
    fn showing(&self) -> f64 {
        let [from, to] = self.ends;
        let (steps, step) = Self::steps(from, to);
        let (mut run, mut most) = (0_u32, 0_u32);
        for k in 0..steps {
            let at = from.lerp(to, (f64::from(k) + 0.5) / f64::from(steps));
            let cell = self.cover.cell((at.x(), at.y()));
            run = if self.own(cell) || self.later(cell) { 0 } else { run + 1 };
            most = most.max(run);
        }
        f64::from(most) * step
    }

    /// Whether the straight line from `from` to `to` keeps to cells a run may cross. A unit of `meter`'s work
    /// for each half cell of it.
    fn clear(&self, from: Point, to: Point, meter: &mut Meter) -> Result<bool, Exhausted> {
        let (steps, _) = Self::steps(from, to);
        meter.charge(u64::from(steps))?;
        Ok((0..=steps).all(|k| {
            let at = from.lerp(to, f64::from(k) / f64::from(steps));
            self.open(self.cover.cell((at.x(), at.y())))
        }))
    }

    /// The needle points of a run from one end to the other, the ends left out; `None` when no way keeps to
    /// the cells a run may cross within [`longest`]. With `meter`'s work.
    fn run(&self, meter: &mut Meter) -> Result<Option<Vec<Point>>, Exhausted> {
        let [from, to] = self.ends;
        let Some(cells) = self.cells_between(meter)? else { return Ok(None) };
        // The way's corners: the ends, and the centres of the cells between them.
        let inner = cells.get(1..cells.len().saturating_sub(1)).unwrap_or_default();
        let centres = inner.iter().filter_map(|&cell| {
            let (x, y) = self.cover.centre(cell);
            Point::new(x, y).ok()
        });
        let corners: Vec<Point> = std::iter::once(from).chain(centres).chain([to]).collect();
        // Pulled straight: from each corner kept, on to the last corner in a row that a clear line reaches.
        let mut pulled = vec![from];
        let mut at = 0;
        while let Some(&anchor) = corners.get(at) {
            let mut next = at + 1;
            while let Some(&farther) = corners.get(next + 1) {
                if !self.clear(anchor, farther, meter)? {
                    break;
                }
                next += 1;
            }
            pulled.extend(corners.get(next));
            at = next;
        }
        // Sewn: each straight piece in equal stitches of `STITCH` at most, every corner a needle point, so
        // that no stitch leaves the way. Finalize leaves out a point too near the one before it, as it does
        // of any stitches.
        let mut needle = Vec::new();
        for piece in pulled.windows(2) {
            let [start, end] = [piece.first().copied().unwrap_or(from), piece.last().copied().unwrap_or(to)];
            #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)] // A count of stitches, from 1 on.
            let stitches = (start.distance(end) / STITCH.get()).ceil().max(1.0) as u32;
            needle.extend((1..=stitches).map(|k| start.lerp(end, f64::from(k) / f64::from(stitches))));
        }
        needle.pop();
        Ok(Some(needle))
    }

    /// The cells of the shortest way from the first end's cell to the second's that keeps to cells a run may
    /// cross, both among them; `None` when there is none within [`WINDOW`] of the ends' box and no longer
    /// than [`longest`]. A unit of `meter`'s work for each cell of the box, and for each cell looked at.
    fn cells_between(&self, meter: &mut Meter) -> Result<Option<Vec<Cell>>, Exhausted> {
        let [start, goal] = self.cells;
        let margin = cells(WINDOW);
        let corner = (start.0.min(goal.0) - margin, start.1.min(goal.1) - margin);
        let (width, height) = ((start.0 - goal.0).abs() + 2 * margin + 1, (start.1 - goal.1).abs() + 2 * margin + 1);
        let size = usize::try_from(width * height).unwrap_or(0);
        meter.charge(u64::try_from(size).unwrap_or(u64::MAX))?;
        let local = |(column, row): Cell| {
            let (across, down) = (column - corner.0, row - corner.1);
            ((0..width).contains(&across) && (0..height).contains(&down)).then(|| usize::try_from(down * width + across).unwrap_or(usize::MAX))
        };
        // The cost of the cheapest way no shorter than any real one: straight across corners, then along.
        let ahead = |(column, row): Cell| {
            let (across, down) = ((column - goal.0).unsigned_abs(), (row - goal.1).unsigned_abs());
            u32::try_from(10 * across.max(down) + 4 * across.min(down)).unwrap_or(u32::MAX)
        };
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)] // A length in tenths of a cell.
        let limit = (longest(self.ends[0].distance(self.ends[1])) / CELL * 10.0) as u32;
        let mut best = vec![u32::MAX; size];
        let mut came = vec![0_u8; size];
        if let Some(cost) = local(start).and_then(|at| best.get_mut(at)) {
            *cost = 0;
        }
        // Rows by what they cost with the way still ahead, then by their cell, so that ties fall the same way.
        let mut queue = BinaryHeap::from([Reverse((ahead(start), 0_u32, start.1, start.0))]);
        while let Some(Reverse((_, cost, row, column))) = queue.pop() {
            meter.charge(1)?;
            if (column, row) == goal {
                return Ok(Some(self.traced(&came, local)));
            }
            for (step, &(across, down)) in (0_u8..).zip(&STEPS) {
                let next = (column + across, row + down);
                let Some(known) = local(next).and_then(|at| best.get_mut(at)) else { continue };
                let cost = cost + STEP[usize::from(across != 0 && down != 0)];
                if cost >= *known || cost.saturating_add(ahead(next)) > limit || !self.open(next) {
                    continue;
                }
                *known = cost;
                if let Some(from) = local(next).and_then(|at| came.get_mut(at)) {
                    *from = step;
                }
                queue.push(Reverse((cost + ahead(next), cost, next.1, next.0)));
            }
        }
        Ok(None)
    }

    /// The cells from the first end's to the second's, back along the step that `came` holds for each, in the
    /// box that `local` places a cell in.
    fn traced(&self, came: &[u8], local: impl Fn(Cell) -> Option<usize>) -> Vec<Cell> {
        let [start, mut at] = self.cells;
        let mut cells = vec![at];
        while at != start {
            let step = local(at).and_then(|at| came.get(at)).and_then(|&step| STEPS.get(usize::from(step)));
            let Some(&(across, down)) = step else { break };
            at = (at.0 - across, at.1 - down);
            cells.push(at);
        }
        cells.reverse();
        cells
    }
}

/// The moves of one design: what covers, the elements whose moves are looked at, and the work left.
#[derive(Debug)]
pub(crate) struct Routing<'m> {
    cover: Cover,
    /// The elements that automatic digitizing made.
    made: &'m BTreeSet<ElementId>,
    meter: Meter,
    /// The element at whose move the work ran out.
    out_at: Option<ElementId>,
}

impl<'m> Routing<'m> {
    /// The moves of `elements` as generated, in the order they sew, with `budget`'s work once; `None` when
    /// automatic digitizing made none of them, and their moves stay as they are.
    pub(crate) fn of(elements: &[(&Element, Generated)], made: &'m BTreeSet<ElementId>, budget: &Budget) -> Option<Self> {
        let first = elements.iter().find(|(element, _)| made.contains(&element.id))?.0;
        let mut meter = budget.meter();
        Some(match Cover::painted(elements, &mut meter) {
            Ok(cover) => Routing { cover, made, meter, out_at: None },
            Err(_) => Routing { cover: Cover::default(), made, meter, out_at: Some(first.id.clone()) },
        })
    }

    /// The needle points of a run in place of the move between `ends`, from an element to the `place`th
    /// group that sews a stitch, which the second of `elements` sews in a thread of `colour`; `None` to leave
    /// the move as it is. `sews_on` says whether the move is within the collapse length: it is then left
    /// unless its stitch would show for [`SHOWS`] or more.
    pub(crate) fn run(&mut self, place: usize, elements: [&Element; 2], ends: [Point; 2], colour: Rgb, sews_on: bool) -> Option<Vec<Point>> {
        if self.out_at.is_some() || !elements.iter().all(|element| self.made.contains(&element.id)) || ends[0] == ends[1] {
            return None;
        }
        let moved = Move::new(&self.cover, ends, place, colour);
        if sews_on && moved.showing() < SHOWS {
            return None;
        }
        moved.run(&mut self.meter).unwrap_or_else(|_| {
            self.out_at = Some(elements[1].id.clone());
            None
        })
    }

    /// `SC-I0914`, when the work of `budget` ran out: it names the element from whose move on the moves were
    /// left as they were.
    pub(crate) fn said(&self, budget: &Budget) -> Option<Diagnostic> {
        let id = self.out_at.clone()?;
        let message = format!(
            "Finding where the thread can run under the shapes sewn later, in place of a jump, needed more than the work budget of {} units. The moves from `{id}` on are sewn on or jumped as they were.",
            budget.max_work
        );
        Some(Diagnostic::new(Code::AutoRunsLeft, message).with_element(id))
    }
}

#[cfg(test)]
mod tests;
