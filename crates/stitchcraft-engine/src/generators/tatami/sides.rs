//! A part's ring sides filed in a grid of square cells, so that a question about a point or a short line
//! looks only at the sides near it (`REQ-FILL-TAT-013`).
//!
//! **Why.** Every stitch of the needle's travel between rows is cut against the part's rings and tested
//! for lying inside the part, and every end of a row is placed on the rings. Asked of every side of every
//! ring, those questions made a fill's work grow with the square of its drawing: a background with 60
//! detailed holes ran out of the work budget and was not sewn.
//!
//! **Same answers.** Each side is filed under every cell its box covers. A question about a box looks at
//! the cells the box covers, and gets each side filed there once, in the rings' order and the sides' order
//! along them. The caller then tests those sides as it tested every side before, so its answers are the
//! same, ties included. A side whose box misses the box asked about cannot meet it, and one rounding finds
//! the cells of sides and of questions alike, never smaller for a larger coordinate, so no side that could
//! meet is missed.
//!
//! **Size.** There are about as many cells as sides, and a cell is no smaller than a side is long on
//! average, so a long side is filed under few cells. Filing, and every question, cost a unit of work for
//! each cell looked at and each side found there.

use std::cell::RefCell;
use std::ops::RangeInclusive;

use stitchcraft_core::{Exhausted, Meter, Point};

/// A side of a ring: the ring, its place along the ring (from point `index` to the next), and its ends.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Side {
    pub ring: usize,
    pub index: usize,
    pub a: Point,
    pub b: Point,
}

/// The grid: where its cells start, how wide each is, and how many there are across and up.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Grid {
    left: f64,
    bottom: f64,
    cell: f64,
    columns: usize,
    rows: usize,
}

impl Grid {
    /// The cells covering the box of the points of `rings`, about as many as `sides`, each at least `least`
    /// wide; `None` for no points.
    #[allow(clippy::cast_possible_truncation)]
    fn over(rings: &[&[Point]], sides: usize, least: f64) -> Option<Grid> {
        let (mut left, mut bottom, mut right, mut top) = (f64::INFINITY, f64::INFINITY, f64::NEG_INFINITY, f64::NEG_INFINITY);
        for p in rings.iter().flat_map(|ring| ring.iter()) {
            (left, bottom, right, top) = (left.min(p.x()), bottom.min(p.y()), right.max(p.x()), top.max(p.y()));
        }
        if left > right {
            return None;
        }
        let (width, height, n) = (right - left, top - bottom, sides.max(1) as f64);
        let cell = (width * height / n).sqrt().max(width.max(height) / n).max(least).max(f64::MIN_POSITIVE);
        // Both quotients are at most `n` and so fit a `usize`; a cell holds its left and bottom edges, so the
        // box's right and top edges need one cell more.
        let count = |length: f64| (length / cell).floor() as usize + 1;
        Some(Grid { left, bottom, cell, columns: count(width), rows: count(height) })
    }

    /// The column holding `x`, or the nearest, as a signed number of cells from the grid's left edge.
    #[allow(clippy::cast_possible_truncation)]
    fn column(&self, x: f64) -> i64 {
        // Coordinates lie within 10 m of the origin, so the quotient fits an `i64` with room to spare.
        ((x - self.left) / self.cell).floor() as i64
    }

    /// The row holding `y`, as [`Grid::column`] gives a column.
    #[allow(clippy::cast_possible_truncation)]
    fn row(&self, y: f64) -> i64 {
        ((y - self.bottom) / self.cell).floor() as i64
    }

    /// The columns or rows from `low` to `high` that lie in the grid of `count`, or `None` when none does.
    fn within(low: i64, high: i64, count: usize) -> Option<RangeInclusive<usize>> {
        let last = i64::try_from(count).ok()? - 1;
        if high < 0 || low > last {
            return None;
        }
        Some(usize::try_from(low.max(0)).ok()?..=usize::try_from(high.min(last)).ok()?)
    }

    /// The cells the box from (`x0`, `y0`) to (`x1`, `y1`) covers, as ranges of columns and rows; `None` when
    /// it misses the grid.
    fn cells(&self, x0: f64, y0: f64, x1: f64, y1: f64) -> Option<(RangeInclusive<usize>, RangeInclusive<usize>)> {
        Some((Grid::within(self.column(x0), self.column(x1), self.columns)?, Grid::within(self.row(y0), self.row(y1), self.rows)?))
    }
}

/// Up to this many sides, a question looks at every side in order: filing them would cost more than it saves.
const SCAN: usize = 64;

/// A part's ring sides, filed in a grid.
#[derive(Clone, Debug)]
pub(crate) struct Sides<'a> {
    /// The rings, each closed (its first point repeated at its end).
    rings: Vec<&'a [Point]>,
    /// The number of the first side of each ring: sides are numbered through the rings in order.
    first: Vec<usize>,
    /// The cells; `None` where every side is looked at, being few or none.
    grid: Option<Grid>,
    /// Where each cell's sides start in `filed`, row after row, and where the last cell's end.
    starts: Vec<usize>,
    /// The numbers of the sides filed under each cell, in increasing order.
    filed: Vec<usize>,
    /// The numbers of the sides a question gathers, kept from one question to the next.
    gathered: RefCell<Vec<usize>>,
}

impl<'a> Sides<'a> {
    /// `rings`' sides, filed when there are more than a few. One unit of `meter` for each point and for each
    /// cell a side is filed under.
    pub fn new(rings: Vec<&'a [Point]>, meter: &mut Meter) -> Result<Sides<'a>, Exhausted> {
        let mut first = Vec::with_capacity(rings.len());
        let (mut count, mut length) = (0_usize, 0.0);
        for ring in &rings {
            meter.charge(u64::try_from(ring.len()).unwrap_or(u64::MAX))?;
            first.push(count);
            count += ring.len().saturating_sub(1);
            length += ring.windows(2).map(|side| side[0].distance(side[1])).sum::<f64>();
        }
        let mean = if count > 0 { length / count as f64 } else { 0.0 };
        let grid = if count > SCAN { Grid::over(&rings, count, mean) } else { None };
        let mut sides = Sides { rings, first, grid, starts: Vec::new(), filed: Vec::new(), gathered: RefCell::new(Vec::new()) };
        let Some(grid) = grid else { return Ok(sides) };
        // Counted first, then filed: every cell's sides in increasing order, as the sides come.
        let cells = grid.columns.saturating_mul(grid.rows);
        let mut counts = vec![0_usize; cells];
        let each = |sides: &Sides<'a>, meter: &mut Meter, file: &mut dyn FnMut(usize, usize)| -> Result<(), Exhausted> {
            for (id, side) in sides.every().enumerate() {
                let (x0, y0, x1, y1) = side.bounds();
                let Some((columns, rows)) = grid.cells(x0, y0, x1, y1) else { continue };
                for row in rows {
                    meter.charge(u64::try_from(columns.clone().count()).unwrap_or(u64::MAX))?;
                    for column in columns.clone() {
                        file(row * grid.columns + column, id);
                    }
                }
            }
            Ok(())
        };
        each(&sides, meter, &mut |cell, _| {
            if let Some(n) = counts.get_mut(cell) {
                *n += 1;
            }
        })?;
        let mut starts = Vec::with_capacity(cells + 1);
        let mut total = 0;
        for n in &counts {
            starts.push(total);
            total += n;
        }
        starts.push(total);
        let mut filed = vec![0_usize; total];
        let mut next = starts.clone();
        each(&sides, meter, &mut |cell, id| {
            if let Some(slot) = next.get_mut(cell)
                && let Some(entry) = filed.get_mut(*slot)
            {
                *entry = id;
                *slot += 1;
            }
        })?;
        (sides.starts, sides.filed) = (starts, filed);
        Ok(sides)
    }

    /// Every side, in order.
    fn every(&self) -> impl Iterator<Item = Side> + '_ {
        self.rings
            .iter()
            .enumerate()
            .flat_map(|(ring, points)| points.windows(2).enumerate().map(move |(index, w)| Side { ring, index, a: w[0], b: w[1] }))
    }

    /// Side number `id`.
    fn side(&self, id: usize) -> Option<Side> {
        let ring = self.first.partition_point(|&start| start <= id).checked_sub(1)?;
        let index = id - self.first.get(ring)?;
        let points = self.rings.get(ring)?;
        Some(Side { ring, index, a: *points.get(index)?, b: *points.get(index + 1)? })
    }

    /// Calls `visit` with each side `keep` keeps, of those filed under the cells of `columns` in `rows`, each
    /// once, in order; with no grid, of every side. One unit of `meter` for each cell and each side filed
    /// there, or for each side.
    fn visit(
        &self,
        cells: Option<(RangeInclusive<usize>, RangeInclusive<usize>)>,
        keep: impl Fn(&Side) -> bool,
        meter: &mut Meter,
        mut visit: impl FnMut(Side),
    ) -> Result<(), Exhausted> {
        let Some(grid) = self.grid else {
            for side in self.every() {
                meter.charge(1)?;
                if keep(&side) {
                    visit(side);
                }
            }
            return Ok(());
        };
        let Some((columns, rows)) = cells else { return Ok(()) };
        let mut ids = self.gathered.borrow_mut();
        ids.clear();
        for row in rows {
            for column in columns.clone() {
                let cell = row * grid.columns + column;
                let (Some(&start), Some(&end)) = (self.starts.get(cell), self.starts.get(cell + 1)) else { continue };
                let filed = self.filed.get(start..end).unwrap_or_default();
                meter.charge(1 + u64::try_from(filed.len()).unwrap_or(u64::MAX))?;
                ids.extend_from_slice(filed);
            }
        }
        ids.sort_unstable();
        ids.dedup();
        for side in ids.iter().filter_map(|&id| self.side(id)) {
            if keep(&side) {
                visit(side);
            }
        }
        Ok(())
    }

    /// Calls `visit` with every side that could share a point with the box from (`x0`, `y0`) to (`x1`, `y1`),
    /// each once, in the rings' order and then along each ring. One unit of `meter` for each cell looked at
    /// and each side filed there, or with few sides, for each side.
    pub fn in_box(&self, x0: f64, y0: f64, x1: f64, y1: f64, meter: &mut Meter, visit: impl FnMut(Side)) -> Result<(), Exhausted> {
        let cells = self.grid.and_then(|grid| grid.cells(x0, y0, x1, y1));
        let meets = |side: &Side| {
            let (a0, b0, a1, b1) = side.bounds();
            a0 <= x1 && a1 >= x0 && b0 <= y1 && b1 >= y0
        };
        self.visit(cells, meets, meter, visit)
    }

    /// Calls `visit` with every side that could hold `p` or cross the ray from `p` to the right, each once,
    /// in order: one that does neither lies above, below or wholly left of `p`. Costs as [`Sides::in_box`]
    /// does, the cells being those of `p`'s row from its own rightwards.
    pub fn rightward(&self, p: Point, meter: &mut Meter, visit: impl FnMut(Side)) -> Result<(), Exhausted> {
        let cells = self.grid.and_then(|grid| grid.cells(p.x(), p.y(), grid.left + grid.cell * grid.columns as f64, p.y()));
        let reaches = |side: &Side| {
            let (_, y0, x1, y1) = side.bounds();
            y0 <= p.y() && p.y() <= y1 && x1 >= p.x()
        };
        self.visit(cells, reaches, meter, visit)
    }

    /// Of the sides of the rings `allowed`, the one nearest the stretch from `p` to `q` by `distance`, and how
    /// near: the first in order on a tie, as a look at every side in order finds it. `None` when no allowed
    /// ring has a side. Cells are looked at in frames growing round the cells of the stretch's box, until
    /// the next frame lies farther than the side found, with a cell to spare for rounding. That stop needs
    /// `distance` to be no less than how far the stretch's box lies from the side's box, as the distance
    /// from the stretch to the side is. One unit of `meter` for each frame, and for each cell looked at and
    /// side filed there, or with few sides, for each side.
    pub fn nearest(
        &self,
        (p, q): (Point, Point),
        allowed: impl Fn(usize) -> bool,
        distance: impl Fn(&Side) -> f64,
        meter: &mut Meter,
    ) -> Result<Option<(f64, Side)>, Exhausted> {
        let mut best: Option<(f64, usize, Side)> = None;
        let offer = |best: &mut Option<(f64, usize, Side)>, side: Side| {
            let d = distance(&side);
            let id = self.first.get(side.ring).map_or(0, |first| first + side.index);
            if best.is_none_or(|(bd, bid, _)| d < bd || (d == bd && id < bid)) {
                *best = Some((d, id, side));
            }
        };
        let found = |best: Option<(f64, usize, Side)>| best.map(|(d, _, side)| (d, side));
        let Some(grid) = self.grid else {
            self.visit(None, |side| allowed(side.ring), meter, |side| offer(&mut best, side))?;
            return Ok(found(best));
        };
        let (c0, c1, r0, r1) = (grid.column(p.x().min(q.x())), grid.column(p.x().max(q.x())), grid.row(p.y().min(q.y())), grid.row(p.y().max(q.y())));
        let (last_column, last_row) = (i64::try_from(grid.columns).unwrap_or(i64::MAX) - 1, i64::try_from(grid.rows).unwrap_or(i64::MAX) - 1);
        // The first frame that reaches the grid, and the one round all of it.
        let reach = |low: i64, high: i64, last: i64| high.saturating_neg().max(low.saturating_sub(last)).max(0);
        let from = reach(c0, c1, last_column).max(reach(r0, r1, last_row));
        let to = c0.max(last_column.saturating_sub(c1)).max(r0.max(last_row.saturating_sub(r1)));
        for size in from..=to.max(from) {
            meter.charge(1)?;
            // Every cell of this frame lies at least `size - 1` cells from the stretch's box: one less, should
            // rounding have put a corner of the box in the cell beside its own.
            if best.is_some_and(|(d, _, _)| d < (size as f64 - 2.0) * grid.cell) {
                break;
            }
            let (left, right, bottom, top) = (c0.saturating_sub(size), c1.saturating_add(size), r0.saturating_sub(size), r1.saturating_add(size));
            // The box's own cells first, then each frame's 4 sides of cells: its bottom and top rows whole, its
            // left and right columns between.
            let strips = if size == 0 {
                [Some((left, right, bottom, top)), None, None, None]
            } else {
                let (above, below) = (bottom.saturating_add(1), top.saturating_sub(1));
                [
                    Some((left, right, bottom, bottom)),
                    Some((left, right, top, top)),
                    Some((left, left, above, below)),
                    Some((right, right, above, below)),
                ]
            };
            for (c0, c1, r0, r1) in strips.into_iter().flatten() {
                let (Some(columns), Some(rows)) = (Grid::within(c0, c1, grid.columns), Grid::within(r0, r1, grid.rows)) else { continue };
                self.visit(Some((columns, rows)), |side| allowed(side.ring), meter, |side| offer(&mut best, side))?;
            }
        }
        Ok(found(best))
    }
}

impl Side {
    /// Its box: least x and y, then greatest.
    fn bounds(&self) -> (f64, f64, f64, f64) {
        (self.a.x().min(self.b.x()), self.a.y().min(self.b.y()), self.a.x().max(self.b.x()), self.a.y().max(self.b.y()))
    }
}

#[cfg(test)]
mod tests {
    use stitchcraft_core::{Budget, SplitMix64};

    use super::*;
    use crate::generators::tatami::fixture::{p, scattered};

    /// Every side of `rings`, in order.
    fn all(rings: &[Vec<Point>]) -> Vec<Side> {
        rings
            .iter()
            .enumerate()
            .flat_map(|(ring, points)| points.windows(2).enumerate().map(move |(index, w)| Side { ring, index, a: w[0], b: w[1] }))
            .collect()
    }

    fn filed(rings: &[Vec<Point>]) -> Sides<'_> {
        Sides::new(rings.iter().map(Vec::as_slice).collect(), &mut Budget::DEFAULT.meter()).unwrap()
    }

    fn in_box(sides: &Sides<'_>, x0: f64, y0: f64, x1: f64, y1: f64) -> Vec<Side> {
        let mut found = Vec::new();
        sides.in_box(x0, y0, x1, y1, &mut Budget::DEFAULT.meter(), |side| found.push(side)).unwrap();
        found
    }

    fn rightward(sides: &Sides<'_>, q: Point) -> Vec<Side> {
        let mut found = Vec::new();
        sides.rightward(q, &mut Budget::DEFAULT.meter(), |side| found.push(side)).unwrap();
        found
    }

    #[test]
    fn a_box_gets_every_side_whose_box_meets_it_once_in_order() {
        let mut rng = SplitMix64::new(7);
        for case in 0..40 {
            let rings = scattered(&mut rng, case);
            let (sides, every) = (filed(&rings), all(&rings));
            for _ in 0..50 {
                let (x, y, w, h) = (rng.next_in(-3.0, 23.0), rng.next_in(-3.0, 23.0), rng.next_in(0.0, 3.0), rng.next_in(0.0, 3.0));
                let meeting: Vec<Side> = every
                    .iter()
                    .filter(|s| {
                        s.a.x().min(s.b.x()) <= x + w && s.a.x().max(s.b.x()) >= x && s.a.y().min(s.b.y()) <= y + h && s.a.y().max(s.b.y()) >= y
                    })
                    .copied()
                    .collect();
                assert_eq!(in_box(&sides, x, y, x + w, y + h), meeting, "case {case}");
                // Rightwards from the box's corner: every side that holds it or spans its height to its right.
                let q = p(x, y);
                let reaching: Vec<Side> =
                    every.iter().filter(|s| s.a.y().min(s.b.y()) <= y && y <= s.a.y().max(s.b.y()) && s.a.x().max(s.b.x()) >= x).copied().collect();
                assert_eq!(rightward(&sides, q), reaching, "case {case}");
            }
        }
    }

    #[test]
    fn the_nearest_side_is_the_one_a_look_at_every_side_finds_first() {
        let mut rng = SplitMix64::new(8);
        for case in 0..40 {
            let rings = scattered(&mut rng, case);
            let (sides, every) = (filed(&rings), all(&rings));
            for k in 0..60 {
                // Points inside, outside and far outside the square, points on a ring, and stretches from
                // a point to one up to 5 mm away, by the distance travel's lines are weighed by.
                let mut point = || match k % 3 {
                    0 => p(rng.next_in(-30.0, 50.0), rng.next_in(-30.0, 50.0)),
                    1 => p(rng.next_in(0.0, 20.0), rng.next_in(0.0, 20.0)),
                    _ => every[usize::try_from(rng.next_u64() % u64::try_from(every.len()).unwrap()).unwrap()].a,
                };
                let a = point();
                let b = if k % 2 == 0 { a } else { p(a.x() + rng.next_in(-5.0, 5.0), a.y() + rng.next_in(-5.0, 5.0)) };
                let distance = |s: &Side| crate::normalize::near::apart(a, b, s.a, s.b);
                let allowed = |ring: usize| case % 2 == 0 || ring != 0;
                let looked = every.iter().filter(|s| allowed(s.ring)).fold(None::<(f64, Side)>, |best, s| {
                    let d = distance(s);
                    if best.is_none_or(|(bd, _)| d < bd) { Some((d, *s)) } else { best }
                });
                let found = sides.nearest((a, b), allowed, distance, &mut Budget::DEFAULT.meter()).unwrap();
                assert_eq!(found, looked, "case {case}, {a:?} to {b:?}");
            }
        }
    }

    #[test]
    fn rings_with_no_sides_have_none_to_give() {
        let none = Sides::new(Vec::new(), &mut Budget::DEFAULT.meter()).unwrap();
        assert!(in_box(&none, 0.0, 0.0, 1.0, 1.0).is_empty() && rightward(&none, p(0.0, 0.0)).is_empty());
        assert_eq!(none.nearest((p(0.0, 0.0), p(1.0, 1.0)), |_| true, |_| 0.0, &mut Budget::DEFAULT.meter()).unwrap(), None);
        let point = [p(1.0, 1.0)];
        let lone = Sides::new(vec![&point[..]], &mut Budget::DEFAULT.meter()).unwrap();
        assert!(in_box(&lone, 0.0, 0.0, 2.0, 2.0).is_empty());
        assert_eq!(Grid::over(&[], 0, 1.0), None);
    }

    /// A question that charges a meter.
    type Question<'a> = Box<dyn Fn(&mut Meter) -> Result<(), Exhausted> + 'a>;

    /// The work `ask` charges.
    fn cost(ask: impl FnOnce(&mut Meter)) -> u64 {
        let mut meter = Budget::DEFAULT.meter();
        ask(&mut meter);
        Budget::DEFAULT.max_work - meter.work_left()
    }

    #[test]
    fn more_sides_than_scan_are_filed_in_a_grid() {
        let polygon = |n: usize| -> Vec<Point> {
            (0..=n)
                .map(|k| {
                    let (s, c) = stitchcraft_core::math::sin_cos(std::f64::consts::TAU * k as f64 / n as f64);
                    p(10.0 * c, 10.0 * s)
                })
                .collect()
        };
        for (n, gridded) in [(SCAN, false), (SCAN + 1, true)] {
            let ring = polygon(n);
            assert_eq!(Sides::new(vec![&ring[..]], &mut Budget::DEFAULT.meter()).unwrap().grid.is_some(), gridded, "{n} sides");
        }
    }

    #[test]
    fn sides_along_one_line_are_filed_too() {
        // A grid one cell wide: the sides' box has no width.
        let line: Vec<Point> = (0..=SCAN + 1).map(|k| p(3.0, k as f64)).collect();
        let sides = Sides::new(vec![&line[..]], &mut Budget::DEFAULT.meter()).unwrap();
        assert_eq!(sides.grid.map(|grid| (grid.columns, grid.rows)), Some((1, SCAN + 2)));
        let every = all(std::slice::from_ref(&line));
        assert_eq!(in_box(&sides, 2.0, 10.5, 4.0, 12.5), every[10..13]);
    }

    #[test]
    fn filing_and_questions_cost_the_cells_and_sides_they_look_at() {
        // Rings of more than `SCAN` sides are filed in a grid, and a square's 4 sides are looked at one by one.
        // The work is pinned: a change to the grid, or to what is charged, shows here. Filing, a box, a ray,
        // and the side nearest a stretch inside the rings, a point far below and right of them, one far above
        // and left, and a corner of theirs.
        let many = scattered(&mut SplitMix64::new(9), 10);
        let few = [p(0.0, 0.0), p(4.0, 0.0), p(4.0, 4.0), p(0.0, 4.0), p(0.0, 0.0)];
        let (stretch, far) = ((p(2.0, 3.0), p(3.0, 3.5)), [p(40.0, -5.0), p(-5.0, 40.0), many[1][0]]);
        let near = |s: &Side| crate::normalize::near::apart(stretch.0, stretch.1, s.a, s.b);
        let from = |q: Point| move |s: &Side| crate::normalize::near::apart(q, q, s.a, s.b);
        for (points, gridded, costs) in
            [(many.iter().map(Vec::as_slice).collect::<Vec<_>>(), true, [513, 8, 24, 35, 41, 57, 77]), (vec![&few[..]], false, [5, 4, 4, 4, 4, 4, 4])]
        {
            let sides = Sides::new(points.clone(), &mut Budget::DEFAULT.meter()).unwrap();
            assert_eq!(sides.grid.is_some(), gridded);
            let mut questions: Vec<Question<'_>> = vec![
                Box::new(|meter| Sides::new(points.clone(), meter).map(|_| ())),
                Box::new(|meter| sides.in_box(2.0, 3.0, 4.0, 5.0, meter, |_| ())),
                Box::new(|meter| sides.rightward(p(2.0, 3.0), meter, |_| ())),
                Box::new(|meter| sides.nearest(stretch, |_| true, near, meter).map(|_| ())),
            ];
            let filed = &sides;
            for q in far {
                questions.push(Box::new(move |meter| filed.nearest((q, q), |_| true, from(q), meter).map(|_| ())));
            }
            let asked: Vec<u64> = questions.iter().map(|ask| cost(|meter| ask(meter).unwrap())).collect();
            assert_eq!(asked, costs, "gridded: {gridded}");
            // Each needs all the work it is charged: a unit less runs out.
            for (ask, units) in questions.iter().zip(asked) {
                assert!(ask(&mut Budget { max_stitches: 1, max_work: units - 1 }.meter()).is_err());
            }
        }
    }
}
