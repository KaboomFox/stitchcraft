use stitchcraft_core::{Budget, ElementId};

use super::*;
use crate::builder::PlanBuilder;
use crate::plan::Provenance;
use crate::thread::{Rgb, Thread};

fn p(x: f64, y: f64) -> Point {
    Point::new(x, y).unwrap()
}

fn meter() -> Meter {
    Budget::DEFAULT.meter()
}

fn near(a: f64, b: f64) -> bool {
    (a - b).abs() < 1e-9
}

/// An element's id, the role of its stitches, and their needle points.
type Part<'a> = (&'a str, Role, &'a [(f64, f64)]);

/// A plan of one block: each part is an element's stitches in a role, sewn on from the last.
fn plan(parts: &[Part]) -> StitchPlan {
    let mut builder = PlanBuilder::new(Thread::new(Rgb::new(0, 0, 0)));
    for (id, role, points) in parts {
        let element = builder.element(ElementId::new(*id).unwrap()).unwrap();
        for (x, y) in *points {
            builder.stitch(p(*x, *y), Provenance::new(element, *role));
        }
    }
    builder.finish()
}

/// The rows of a fill of the square from (0, 0) to (10, 10), a thread apart from y = 0.1, sewn back and
/// forth `layers` times over, after a needle point at (0, 0): the cells then start at half millimetres. The
/// rows lie on the machine's grid, where a plan is measured, so their lanes lie 0.2 mm apart from y = 0, and
/// one in 5 lies on the edge of a cell, at a whole millimetre, where a rounding decides which cell holds
/// it. The exact cases read spots at whole millimetres of y, whose windows end between 2 lanes.
fn rows(layers: usize) -> Vec<(f64, f64)> {
    let mut points = vec![(0.0, 0.0)];
    let mut side = 0.0;
    for layer in 0..layers {
        for row in 0..25 {
            // Up the square and down it again, so that no stitch crosses it from one layer to the next.
            let row = if layer % 2 == 0 { row } else { 24 - row };
            let y = 0.1 + f64::from(row) * THREAD;
            points.extend([(side, y), (10.0 - side, y)]);
            side = 10.0 - side;
        }
    }
    points
}

fn measured(plan: &StitchPlan) -> Thickness {
    Thickness::of(plan, &mut meter()).unwrap().unwrap()
}

/// The least work that `work` succeeds with.
fn least(work: &dyn Fn(&mut Meter) -> bool) -> u64 {
    (0..1_000_000u64).find(|max_work| work(&mut Budget { max_work: *max_work, ..Budget::DEFAULT }.meter())).unwrap()
}

/// One stitch from `from` to `to`.
fn stitch(from: (f64, f64), to: (f64, f64)) -> SewnStitch {
    SewnStitch { block: 0, index: 1, from: p(from.0, from.1), to: p(to.0, to.1), role: Role::Top, from_role: Role::Top }
}

/// `stitch` with its needle points at their places on `thickness`'s grid, where a pass gives them.
fn placed(thickness: &Thickness, stitch: SewnStitch) -> SewnStitch {
    SewnStitch { from: thickness.placed(stitch.from), to: thickness.placed(stitch.to), ..stitch }
}

#[test]
fn req_auto_060_a_square_sewn_1_2_3_and_6_times_measures_as_many_fills() {
    for layers in [1, 2, 3, 6] {
        let rows = rows(layers);
        let thickness = measured(&plan(&[("a", Role::Top, &rows)]));
        // Inside the square, a millimetre and more from its edge, every window holds 2.5 rows' worth of
        // thread, wherever the rows fall in it: the 2 lanes of the rows lie 0.2 mm apart. Rows a thread
        // apart are 0.625 of a fill, whose rows lie 0.25 mm apart.
        for at in [(5.0, 5.0), (5.2, 4.8), (3.3, 7.2), (1.0, 1.0), (9.0, 9.0)] {
            let fills = thickness.at(Measure::Fills, p(at.0, at.1));
            assert!(near(fills, layers as f64 * 0.625), "{layers} layers at {at:?}: {fills}");
        }
        // Outside it by more than half a window and a lane there is none.
        assert_eq!(thickness.at(Measure::Fills, p(5.0, 10.5)), 0.0);
        assert_eq!(thickness.at(Measure::Fills, p(100.0, 5.0)), 0.0);
        assert_eq!(thickness.at(Measure::Fills, p(5.0, -100.0)), 0.0);
    }
}

#[test]
fn req_auto_060_a_plan_is_measured_where_the_machine_sews_it() {
    // Needle points 0.04 mm to either side of the edge of a cell, at 5 mm, are the same point to the
    // machine, and measure the same: 2 needle points in the cell from 5 mm, and the stack's thread there.
    // Where they are planned, the stitches between them have other lengths, and the points other cells.
    let plan_at = |x: f64| plan(&[("a", Role::Top, &[(0.0, 0.0), (x, 2.2), (x, 2.4)])]);
    let (before, after) = (plan_at(4.96), plan_at(5.04));
    let planned = |plan: &StitchPlan| Thickness::of_stitches(&plan.sewn_stitches(), &mut meter()).unwrap().unwrap();
    assert!(planned(&before) != planned(&after) && planned(&before) != measured(&before) && planned(&after) != measured(&after));
    assert_eq!(measured(&before), measured(&after));
    assert_eq!(measured(&before), Thickness::of_stitches(&plan_at(5.0).sewn_stitches(), &mut meter()).unwrap().unwrap());
    assert_eq!((measured(&before).at(Measure::Points, p(5.5, 2.5)), measured(&before).at(Measure::Points, p(4.5, 2.5))), (2.0, 0.0));
    let stack = |plan: &StitchPlan| measured(plan).stack(plan, p(5.0, 2.5), &mut meter()).unwrap();
    assert_eq!(stack(&before), stack(&after));
    assert!(!stack(&before).is_empty());
    // A needle point half way between 2 of the grid's, but for its last digits, goes where the point half
    // way goes: 4.85 mm to 4.8, the even side, from above as from below, and 4.95 mm to 5.
    assert_eq!((settled(4.85 + 1e-12), settled(4.85 - 1e-12), settled(0.123_456_74)), (4.85, 4.85, 0.123_457));
    assert_eq!(measured(&plan_at(4.85 + 1e-12)), measured(&plan_at(4.8)));
    assert_eq!(measured(&plan_at(4.85 - 1e-12)), measured(&plan_at(4.8)));
    assert_eq!(measured(&plan_at(4.95 - 1e-12)), measured(&plan_at(5.0)));
    assert!(measured(&plan_at(4.8)) != measured(&plan_at(4.9)) && measured(&plan_at(4.9)) != measured(&plan_at(5.0)));
    // A point beyond the machine's grid is measured where it is, and such a plan is too large.
    assert_eq!(Thickness::of(&plan_at(2.0e7), &mut meter()), Err(Exhausted::Work));
}

#[test]
fn req_auto_060_every_millimetre_of_thread_is_counted_once_whatever_its_role_and_direction() {
    let points = [(0.0, 0.0), (7.0, 3.0), (7.0, 3.0), (2.0, 9.5), (2.1, 9.4), (-3.0, 4.0)];
    let plan = plan(&[("a", Role::Underlay, &points[..3]), ("b", Role::Travel, &points[3..])]);
    let thickness = measured(&plan);
    let length: f64 = plan.sewn_stitches().iter().map(SewnStitch::length).sum();
    // Each cell is in the windows of 4 spots, and a spot stands for a quarter of a square millimetre.
    let summed: f64 = thickness.fills.iter().sum::<f64>() * CELL * CELL;
    assert!(near(summed, length * ROWS), "{summed} of {}", length * ROWS);
    assert!(thickness.fills.iter().all(|fills| fills.is_finite()));
    // The grid has a cell of room round the stitches, and a spot at every corner of a cell.
    assert_eq!((thickness.grid.columns, thickness.grid.rows), (20 + 3, 19 + 3));
    assert_eq!(thickness.size(), (24, 23));
    assert_eq!(thickness.grid.corner, (-3.5, -0.5));
}

#[test]
fn req_auto_060_a_ribbon_is_followed_in_steps_of_a_cell_in_2_lanes() {
    // A grid whose cells start at half millimetres, 21 columns of them from -0.5.
    let grid = Grid::round(Rect::around([p(0.0, 0.0), p(9.0, 9.0)]).unwrap()).unwrap();
    assert_eq!((grid.corner, grid.columns, grid.rows), ((-0.5, -0.5), 21, 21));
    let lay = |stitch: &SewnStitch| {
        let mut thread = vec![0.0; grid.columns * grid.rows];
        laid(&grid, stitch, stitch.length(), steps(stitch.length()), |at, length| thread[at] += length);
        thread
    };
    // A stitch along x at y = 0.45: its lanes lie at 0.35 and 0.55, in the rows of cells from 0 and from
    // 0.5. Each of its 6 steps of 0.5 mm lays a quarter of a millimetre in each, from the column at 0 on.
    let along = stitch((0.0, 0.45), (3.0, 0.45));
    assert_eq!(steps(along.length()), 6.0);
    let thread = lay(&along);
    let cell = |column: usize, row: usize| thread[row * grid.columns + column];
    let rows: Vec<f64> = (0..4).map(|row| cell(4, row)).collect();
    assert!(rows.iter().zip([0.0, 0.25, 0.25, 0.0]).all(|(a, b)| near(*a, b)), "{rows:?}");
    let columns: Vec<f64> = (0..9).map(|column| cell(column, 1)).collect();
    assert!(columns.iter().zip([0.0, 0.25, 0.25, 0.25, 0.25, 0.25, 0.25, 0.0, 0.0]).all(|(a, b)| near(*a, b)), "{columns:?}");
    // A stitch down y at x = 0.45 likewise, in the columns from 0 and from 0.5.
    let thread = lay(&stitch((0.45, 3.0), (0.45, 0.0)));
    let cell = |column: usize, row: usize| thread[row * grid.columns + column];
    let columns: Vec<f64> = (0..4).map(|column| cell(column, 4)).collect();
    assert!(columns.iter().zip([0.0, 0.25, 0.25, 0.0]).all(|(a, b)| near(*a, b)), "{columns:?}");
    // A stitch at 45° from a corner of a cell runs through the cells on the diagonal, and each lane lies
    // 0.07 mm to its side: a third of the thread is in the cells beside the diagonal, as much above as
    // below.
    let slanted = stitch((0.0, 0.0), (4.0, 4.0));
    let thread = lay(&slanted);
    let side = |above: bool| -> f64 {
        let cells = (0..grid.rows).flat_map(|row| (0..grid.columns).map(move |column| (column, row)));
        cells.filter(|(column, row)| if above { row > column } else { row < column }).map(|(column, row)| thread[row * grid.columns + column]).sum()
    };
    let length = slanted.length();
    assert!(near(thread.iter().sum::<f64>(), length));
    assert!(side(true) > 0.15 * length && near(side(true), side(false)), "{} and {} of {length}", side(true), side(false));
    // A stitch of no length lays nothing, and is one step.
    let none = stitch((2.0, 2.0), (2.0, 2.0));
    assert_eq!((steps(none.length()), lay(&none).iter().sum::<f64>()), (1.0, 0.0));
    assert_eq!((steps(0.5), steps(0.51), steps(0.0)), (1.0, 2.0, 1.0));
}

#[test]
fn req_auto_060_thread_outside_the_grid_is_laid_in_no_cell() {
    // A grid of 2 by 2 cells from (0, 0). The grid round a plan has a cell of room, so no lane of its
    // stitches lies outside: a stitch that does is laid by hand.
    let grid = Grid { corner: (0.0, 0.0), columns: 2, rows: 2 };
    let lay = |from: (f64, f64), to: (f64, f64)| {
        let (stitch, mut cells) = (stitch(from, to), Vec::new());
        laid(&grid, &stitch, stitch.length(), steps(stitch.length()), |at, _| cells.push(at));
        cells
    };
    // Inside, each lane of a stitch of one step lies in a cell: along x at y = 0.5 in both rows, and down
    // y at x = 0.5 in both columns.
    assert_eq!(lay((0.1, 0.5), (0.4, 0.5)), [2, 0]);
    assert_eq!(lay((0.5, 0.1), (0.5, 0.4)), [0, 1]);
    // To each side of the grid, with the other coordinate inside it, nothing is laid.
    for (from, to) in [((-0.4, 0.5), (-0.1, 0.5)), ((1.1, 0.5), (1.4, 0.5)), ((0.5, -0.4), (0.5, -0.1)), ((0.5, 1.1), (0.5, 1.4))] {
        assert_eq!(lay(from, to), Vec::<usize>::new(), "{from:?} to {to:?}");
    }
    // The grid ends before its last edge: a step whose middle lies on it, 2 cells from the corner, is
    // outside.
    assert_eq!(lay((0.75, 0.5), (1.25, 0.5)), Vec::<usize>::new());
    assert_eq!(lay((0.5, 0.75), (0.5, 1.25)), Vec::<usize>::new());
}

#[test]
fn req_auto_060_needle_points_are_counted_per_square_millimetre_without_the_locks() {
    let top = [(0.0, 0.0), (5.1, 0.1), (5.2, 0.2), (5.3, 0.1), (12.1, 0.1)];
    let lock = [(12.2, 0.1), (12.3, 0.1), (12.2, 0.1)];
    let after = [(12.3, 0.2), (20.1, 0.1)];
    let thickness = measured(&plan(&[("a", Role::Top, &top), ("a", Role::Lock, &lock), ("a", Role::Top, &after)]));
    let points = |x: f64, y: f64| thickness.at(Measure::Points, p(x, y));
    // The first needle point starts the thread and is no stitch's.
    assert_eq!(points(0.0, 0.0), 0.0);
    // The 3 lie in the cell from (5, 0), which is in the windows of the 4 spots at its corners.
    assert_eq!((points(5.0, 0.0), points(5.5, 0.0), points(5.0, 0.5), points(5.5, 0.5)), (3.0, 3.0, 3.0, 3.0));
    assert_eq!((points(4.5, 0.0), points(6.0, 0.0), points(5.0, -0.5), points(5.0, 1.0)), (0.0, 0.0, 0.0, 0.0));
    // The nearest spot answers for a place, the further along where 2 are as near.
    assert_eq!((points(5.2, 0.2), points(4.76, 0.2), points(4.74, 0.2), points(5.75, 0.2), points(5.2, 0.75)), (3.0, 3.0, 0.0, 0.0, 0.0));
    // The point before a lock counts, and neither the lock's stitches nor the one out of it.
    assert_eq!(points(12.0, 0.0), 1.0);
    assert_eq!(points(20.0, 0.0), 1.0);
    assert_eq!(thickness.most(Measure::Points).value, 3.0);
}

/// A thickness of 12 by 12 spots from (1, 2) whose spots in `over` have 3 fills and 4 points.
fn drawn(over: &[usize]) -> Thickness {
    let mut fills = vec![0.5; 144];
    for at in over {
        fills[*at] = 3.0;
    }
    let points = fills.iter().map(|fills| if *fills == 3.0 { 4.0 } else { 0.0 }).collect();
    Thickness { origin: Point::ORIGIN, grid: Grid { corner: (1.0, 2.0), columns: 11, rows: 11 }, fills, points, sewn: Vec::new() }
}

/// The spots of the block of `across` by `down` spots whose first is in `column` and `row`, of 12 columns.
fn block(column: usize, row: usize, across: usize, down: usize) -> Vec<usize> {
    (row..row + down).flat_map(|row| (column..column + across).map(move |column| row * 12 + column)).collect()
}

/// Whether 2 boxes are the same to a nanometre.
fn boxes(a: Option<Rect>, b: Option<Rect>) -> bool {
    let (a, b) = (a.unwrap(), b.unwrap());
    let corners = [(a.min(), b.min()), (a.max(), b.max())];
    corners.iter().all(|(a, b)| near(a.x(), b.x()) && near(a.y(), b.y()))
}

#[test]
fn req_auto_061_a_piece_of_a_square_millimetre_at_the_limit_counts() {
    let whole = drawn(&block(3, 4, 2, 2));
    let over = whole.over(Measure::Fills, 3.0, &mut meter()).unwrap();
    assert!(over.pieces == 1 && boxes(over.at, Rect::around([p(2.5, 4.0), p(3.0, 4.5)])), "{over:?}");
    assert!(near(over.area, PIECE) && near(over.largest, PIECE), "{over:?}");
    // The other measure has its own values.
    assert_eq!(whole.over(Measure::Points, 4.0, &mut meter()).unwrap().pieces, 1);
    assert_eq!(whole.over(Measure::Points, 4.5, &mut meter()).unwrap(), Over::default());
    // A value under the limit is not over it, and one spot less is no piece.
    assert_eq!(whole.over(Measure::Fills, 3.000_001, &mut meter()).unwrap(), Over::default());
    let short = drawn(&block(3, 4, 2, 2)[1..]);
    assert_eq!(short.over(Measure::Fills, 3.0, &mut meter()).unwrap(), Over::default());
    // Every spot over a limit of nothing is one piece.
    assert_eq!(whole.over(Measure::Fills, 0.5, &mut meter()).unwrap().pieces, 1);
    assert!(near(whole.over(Measure::Fills, 0.5, &mut meter()).unwrap().area, 144.0 * CELL * CELL));
}

#[test]
fn req_auto_061_spots_across_a_cell_are_one_piece_and_rows_do_not_wrap() {
    // 2 pairs of spots that only touch across a cell are one piece of 4.
    let corner: Vec<usize> = [block(0, 0, 2, 1), block(2, 1, 2, 1)].concat();
    let over = drawn(&corner).over(Measure::Fills, 3.0, &mut meter()).unwrap();
    assert!(over.pieces == 1 && near(over.area, 4.0 * CELL * CELL), "{over:?}");
    // In every direction: a spot and the 3 round it that touch it only across a cell.
    for spots in [[13, 0, 2, 24], [13, 2, 24, 26], [13, 0, 24, 26], [13, 0, 2, 26]] {
        assert_eq!(drawn(&spots).over(Measure::Fills, 3.0, &mut meter()).unwrap().pieces, 1, "{spots:?}");
    }
    // A spot between parts them.
    let apart: Vec<usize> = [block(0, 0, 2, 1), block(3, 1, 2, 1)].concat();
    assert_eq!(drawn(&apart).over(Measure::Fills, 3.0, &mut meter()).unwrap().pieces, 0);
    let below: Vec<usize> = [block(0, 0, 2, 1), block(0, 2, 2, 1)].concat();
    assert_eq!(drawn(&below).over(Measure::Fills, 3.0, &mut meter()).unwrap().pieces, 0);
    // The last spots of a row and the first of the next don't touch, and the grid's last row and column
    // hold spots as the others do.
    let ends: Vec<usize> = [block(10, 0, 2, 1), block(0, 1, 2, 1)].concat();
    assert_eq!(drawn(&ends).over(Measure::Fills, 3.0, &mut meter()).unwrap().pieces, 0);
    let last = drawn(&block(10, 10, 2, 2)).over(Measure::Fills, 3.0, &mut meter()).unwrap();
    assert!(last.pieces == 1 && boxes(last.at, Rect::around([p(6.0, 7.0), p(6.5, 7.5)])), "{last:?}");
}

#[test]
fn req_auto_061_the_pieces_are_counted_and_the_first_of_the_largest_is_named() {
    let small = block(0, 0, 2, 2);
    let large = block(6, 6, 2, 3);
    let first = Rect::around([p(1.0, 2.0), p(1.5, 2.5)]);
    let second = Rect::around([p(4.0, 5.0), p(4.5, 6.0)]);
    let over = |spots: &[usize]| drawn(spots).over(Measure::Fills, 3.0, &mut meter()).unwrap();
    let both = over(&[small.clone(), large].concat());
    assert!(both.pieces == 2 && near(both.area, 10.0 * CELL * CELL) && near(both.largest, 6.0 * CELL * CELL), "{both:?}");
    assert!(boxes(both.at, second), "{both:?}");
    // A larger piece found first stays the largest, and of 2 as large the first is named.
    let turned = over(&[block(0, 0, 2, 3), block(6, 6, 2, 2)].concat());
    assert!(near(turned.largest, 6.0 * CELL * CELL) && boxes(turned.at, Rect::around([p(1.0, 2.0), p(1.5, 3.0)])), "{turned:?}");
    let equal = over(&[small, block(6, 6, 2, 2)].concat());
    assert!(equal.pieces == 2 && boxes(equal.at, first), "{equal:?}");
}

#[test]
fn req_auto_061_the_thickest_spot_is_a_corner_of_the_cells() {
    let mut thickness = drawn(&[]);
    thickness.fills[2 * 12 + 7] = 4.25;
    thickness.points[5 * 12 + 1] = 9.0;
    assert_eq!(thickness.most(Measure::Fills), Spot { value: 4.25, at: p(1.0 + 7.0 * CELL, 2.0 + 2.0 * CELL) });
    assert_eq!(thickness.most(Measure::Points), Spot { value: 9.0, at: p(1.0 + CELL, 2.0 + 5.0 * CELL) });
    assert_eq!(thickness.at(Measure::Fills, p(4.6, 3.1)), 4.25);
    assert_eq!(thickness.at(Measure::Points, p(1.4, 4.4)), 9.0);
    // The grid's spots: from its corner, which is one, to the far corner of its last cell.
    let grid = &thickness.grid;
    assert_eq!((grid.nearest(1.0, 2.0), grid.nearest(0.76, 1.76), grid.nearest(6.74, 7.74)), (Some(0), Some(0), Some(143)));
    assert_eq!((grid.nearest(0.74, 2.0), grid.nearest(1.0, 7.76), grid.nearest(6.76, 2.0), grid.nearest(f64::NAN, 2.0)), (None, None, None, None));
    assert_eq!((grid.spot(0), grid.spot(143), grid.spot(13)), (p(1.0, 2.0), p(6.5, 7.5), p(1.5, 2.5)));
    // Its cells: from the corner, which is in the first, to the far corner, which is in none.
    assert_eq!(
        (grid.holding(1.0, 2.0), grid.holding(6.499, 7.499), grid.holding(1.5, 2.0), grid.holding(1.0, 2.5)),
        (Some(0), Some(120), Some(1), Some(11))
    );
    assert_eq!((grid.holding(0.999, 2.0), grid.holding(6.5, 2.0), grid.holding(1.0, 7.5), grid.holding(1.0, f64::NAN)), (None, None, None, None));
    // A window is the 4 cells at a spot, or as many as the grid has there.
    let window = |at: usize| grid.window(at).collect::<Vec<usize>>();
    assert_eq!((window(13), window(0), window(11), window(143)), (vec![0, 1, 11, 12], vec![0], vec![10], vec![120]));
    assert_eq!(grid.windows(&(0..121).map(f64::from).collect::<Vec<f64>>(), 2.0).get(13..15), Some(&[48.0, 56.0][..]));
    let none = Thickness {
        origin: Point::ORIGIN,
        grid: Grid { corner: (0.0, 0.0), columns: 0, rows: 0 },
        fills: Vec::new(),
        points: Vec::new(),
        sewn: Vec::new(),
    };
    assert_eq!(none.most(Measure::Fills), Spot { value: 0.0, at: Point::ORIGIN });
    // Of spots with the same value, the last is the one named.
    let equal = drawn(&block(3, 3, 2, 2));
    assert_eq!(equal.most(Measure::Fills), Spot { value: 3.0, at: equal.grid.spot(4 * 12 + 4) });
    assert_eq!(drawn(&[]).most(Measure::Fills), Spot { value: 0.5, at: grid.spot(143) });
    assert_eq!(drawn(&[]).most(Measure::Points), Spot { value: 0.0, at: grid.spot(143) });
}

#[test]
fn req_auto_194_a_spot_is_found_by_its_place_in_the_measure() {
    let mut thickness = drawn(&[]);
    thickness.fills[2 * 12 + 7] = 4.25;
    thickness.points[5 * 12 + 1] = 9.0;
    assert_eq!(thickness.at_spot(Measure::Fills, 2 * 12 + 7), Some(thickness.most(Measure::Fills)));
    assert_eq!(thickness.at_spot(Measure::Points, 5 * 12 + 1), Some(thickness.most(Measure::Points)));
    assert_eq!(thickness.at_spot(Measure::Fills, 0), Some(Spot { value: 0.5, at: p(1.0, 2.0) }));
    assert_eq!((thickness.at_spot(Measure::Fills, 143).is_some(), thickness.at_spot(Measure::Fills, 144)), (true, None));
    // The spots a stitch lays thread in are named by the same places.
    let stitch = placed(&thickness, stitch((2.0, 3.0), (3.0, 3.0)));
    for (at, fills) in thickness.spots(&stitch, &mut meter()).unwrap() {
        assert_eq!(thickness.at_spot(Measure::Fills, at).map(|spot| spot.value), Some(fills));
    }
}

#[test]
fn req_auto_062_the_stack_names_each_elements_thread_in_the_window() {
    let rows = rows(1);
    let base = [(0.0, 5.05), (10.0, 5.05), (10.0, 5.25), (0.0, 5.25)];
    let mut builder = PlanBuilder::new(Thread::new(Rgb::new(0, 0, 0)));
    let (a, b) = (builder.element(ElementId::new("a").unwrap()).unwrap(), builder.element(ElementId::new("b").unwrap()).unwrap());
    builder.stitch(p(0.0, 0.0), Provenance::new(a, Role::Underlay));
    for (x, y) in base {
        builder.stitch(p(x, y), Provenance::new(a, Role::Underlay));
    }
    for (x, y) in &rows[1..] {
        builder.stitch(p(*x, *y), Provenance::new(a, Role::Top));
    }
    builder.change_thread(Thread::new(Rgb::new(255, 0, 0)));
    for (x, y) in &rows[1..] {
        builder.stitch(p(*x, *y), Provenance::new(b, Role::Top));
    }
    let plan = builder.finish();
    let thickness = measured(&plan);
    let at = p(5.0, 5.0);
    let stack = thickness.stack(&plan, at, &mut meter()).unwrap();
    assert_eq!(stack.len(), 2);
    assert_eq!((stack[0].block, stack[0].element, stack[1].block, stack[1].element), (0, Some(a), 1, Some(b)));
    assert!(near(stack[1].under, 0.0) && near(stack[1].top, 0.625), "{stack:?}");
    // 2 rows of underlay cross the window, a millimetre each: half a fill.
    assert!(near(stack[0].top, 0.625) && near(stack[0].under, 0.5), "{stack:?}");
    // What lies in a window is what the spot measures: in the middle, at a corner, where the rows turn, and
    // near the end of a stitch whose middle is far away.
    for at in [at, p(0.0, 0.0), p(10.5, 5.0), p(9.5, 9.5), p(0.5, 9.0)] {
        let stack = thickness.stack(&plan, at, &mut meter()).unwrap();
        let fills = thickness.at(Measure::Fills, at);
        assert!(fills > 0.0 && near(stack.iter().map(|layer| layer.top + layer.under).sum::<f64>(), fills), "{at:?}: {stack:?} of {fills}");
    }
    // Outside the grid nothing lies.
    assert_eq!(thickness.stack(&plan, p(500.0, 0.0), &mut meter()).unwrap(), Vec::new());
}

#[test]
fn req_auto_062_only_the_stitches_near_a_spot_are_followed_for_its_stack() {
    // A stitch is near a spot at (5, 5) when its box reaches within a millimetre of it along x and along y.
    let near_to = |from: (f64, f64), to: (f64, f64)| super::near(&stitch(from, to), p(5.0, 5.0));
    assert!(near_to((5.0, 5.0), (5.0, 5.0)) && near_to((0.0, 0.0), (9.0, 9.0)) && near_to((9.0, 5.5), (5.5, 9.0)));
    // A millimetre away to each side is near, and a tenth more is not, whichever end is the nearer.
    for (edge, beyond) in [(6.0, 6.1), (4.0, 3.9)] {
        let out = edge + (edge - 5.0) * 3.0;
        assert!(near_to((edge, 5.0), (out, 5.0)) && near_to((out, 5.0), (edge, 5.0)), "{edge}");
        assert!(near_to((5.0, edge), (5.0, out)) && near_to((5.0, out), (5.0, edge)), "{edge}");
        assert!(!near_to((beyond, 5.0), (out, 5.0)) && !near_to((out, 5.0), (beyond, 5.0)), "{beyond}");
        assert!(!near_to((5.0, beyond), (5.0, out)) && !near_to((5.0, out), (5.0, beyond)), "{beyond}");
        // Far along one of the 2 is far.
        assert!(!near_to((beyond, 0.0), (out, 9.0)) && !near_to((0.0, beyond), (9.0, out)), "{beyond}");
    }
    // The stack's work is a unit for each stitch, and the steps of the near ones: of 3 rows 10 mm long,
    // 3 mm apart, with the turns between them, only the middle row is near a spot on its middle: the turns
    // lie 5 mm away along x.
    let plan = plan(&[("a", Role::Top, &[(0.0, 0.0), (10.0, 0.0), (10.0, 3.0), (0.0, 3.0), (0.0, 6.0), (10.0, 6.0)])]);
    let thickness = measured(&plan);
    let work = |at: Point| least(&|meter| thickness.stack(&plan, at, meter).is_ok());
    assert_eq!(thickness.stack(&plan, p(5.0, 3.0), &mut meter()).unwrap().len(), 1);
    assert_eq!(work(p(5.0, 3.0)), 5 + 20);
    // Between 2 rows, near no stitch, the stack is empty and no step is followed.
    assert_eq!(thickness.stack(&plan, p(5.0, 1.5), &mut meter()).unwrap(), Vec::new());
    assert_eq!(work(p(5.0, 1.5)), 5);
}

#[test]
fn req_auto_061_a_piece_just_at_the_largest_value_is_found_from_a_report() {
    // 4 spots of 3 fills and 4 needle points, the most of the plan: at limits of just that, both pieces
    // are found, and above it nothing is, for the same work.
    let thickness = drawn(&block(3, 3, 2, 2));
    let limits = |fills, points| Limits { warn_fills: fills, fail_fills: fills, warn_points: points, fail_points: points };
    let report = |limits: Limits| thickness.report(&StitchPlan::default(), &limits, &mut meter()).unwrap();
    let at = report(limits(3.0, 4.0));
    assert_eq!((at.thick.pieces, at.failing.pieces, at.dense.pieces, at.pierced.pieces), (1, 1, 1, 1));
    assert_eq!((at.thick.area, at.dense.area), (1.0, 1.0));
    let above = report(limits(3.5, 4.5));
    assert_eq!((above.thick, above.failing, above.dense, above.pierced), (Over::default(), Over::default(), Over::default(), Over::default()));
    let below = report(limits(2.5, 3.5));
    assert_eq!((below.thick, below.dense), (at.thick, at.dense));
    let work = |limits: Limits| least(&|meter| thickness.report(&StitchPlan::default(), &limits, meter).is_ok());
    assert_eq!(work(limits(3.0, 4.0)), work(limits(3.5, 4.5)));
    assert_eq!(work(limits(3.0, 4.0)), 4 * 144);
}

#[test]
fn req_auto_062_a_report_holds_a_plan_to_its_limits() {
    let rows = rows(3);
    let plan = plan(&[("a", Role::Top, &rows)]);
    let thickness = measured(&plan);
    let limits = |warn_fills, fail_fills, warn_points, fail_points| Limits { warn_fills, fail_fills, warn_points, fail_points };
    let with = |limits: Limits| thickness.report(&plan, &limits, &mut meter()).unwrap();
    // 3 layers of rows a thread apart are 1.875 fills inside the square, and more where the rows turn.
    let report = with(limits(1.8, 6.0, 99.0, 99.0));
    assert!(report.warns() && report.thick.pieces == 1 && report.thick.area > 80.0, "{report:?}");
    assert_eq!((report.failing, report.dense, report.pierced), (Over::default(), Over::default(), Over::default()));
    assert_eq!(report.fills, thickness.most(Measure::Fills));
    assert_eq!(report.points, thickness.most(Measure::Points));
    assert!(near(report.stack.iter().map(|layer| layer.top + layer.under).sum::<f64>(), report.fills.value));
    // Each limit is its own, and a failing limit under the warning's is the warning's.
    assert!(!with(limits(4.5, 6.0, 99.0, 99.0)).warns());
    assert_eq!(with(limits(4.5, 1.8, 99.0, 99.0)).failing, Over::default());
    assert_eq!(with(limits(1.5, 1.8, 99.0, 99.0)).failing, report.thick);
    // The rows' ends lie along 2 of the square's sides, 3 times over.
    let most = report.points.value;
    assert!(most >= 6.0, "{report:?}");
    let dense = with(limits(9.0, 9.0, 2.0, 1.0));
    assert!(dense.warns() && dense.thick.pieces == 0 && dense.dense.pieces == 2 && dense.pierced == dense.dense, "{dense:?}");
    assert_eq!(with(limits(9.0, 9.0, 2.0, 5.0)).pierced.pieces, 2);
    assert_eq!(with(limits(9.0, 9.0, 2.0, 99.0)).pierced.pieces, 0);
}

#[test]
fn req_auto_062_the_steps_of_the_heat_map_start_at_their_limits() {
    let limits = Limits::DEFAULT;
    assert_eq!((limits.warn_fills, limits.fail_fills, limits.warn_points, limits.fail_points), (4.0, 6.0, 15.0, 30.0));
    let steps = [0.0, 0.01, 1.99, 2.0, 3.99, 4.0, 5.99, 6.0, 40.0].map(|fills| limits.step(fills));
    let expected = [Step::Bare, Step::Ordinary, Step::Ordinary, Step::Doubled, Step::Doubled, Step::Thick, Step::Thick, Step::Failing, Step::Failing];
    assert_eq!(steps, expected);
    // A failing limit under the warning's is the warning's.
    let turned = Limits { fail_fills: 1.0, ..limits };
    assert_eq!((turned.step(3.5), turned.step(4.0)), (Step::Doubled, Step::Failing));
}

#[test]
fn a_plan_without_a_stitch_has_no_thickness_and_one_too_large_is_not_measured() {
    assert_eq!(Thickness::of(&StitchPlan::default(), &mut meter()), Ok(None));
    assert_eq!(Thickness::of(&plan(&[("a", Role::Top, &[(1.0, 1.0)])]), &mut meter()), Ok(None));
    let wide = plan(&[("a", Role::Top, &[(0.0, 0.0), (600.0, 0.0), (600.0, 600.0)])]);
    assert_eq!(Thickness::of(&wide, &mut meter()), Err(Exhausted::Work));
    // 1,000 cells on a side are the most.
    let side = |extent: f64| Rect::around([p(0.0, 0.0), p(extent, 498.6)]).unwrap();
    let most = Grid::round(side(498.6)).unwrap();
    assert_eq!((most.columns, most.rows, most.columns * most.rows), (1000, 1000, MAX_CELLS));
    assert_eq!(Grid::round(side(499.1)), None);
}

#[test]
fn measuring_stops_when_the_work_runs_out() {
    let rows = &rows(1)[1..9];
    let plan = plan(&[("a", Role::Top, rows)]);
    let needed = |work: &dyn Fn(&mut Meter) -> bool| {
        let enough = (0..1_000_000u64).find(|max_work| work(&mut Budget { max_work: *max_work, ..Budget::DEFAULT }.meter())).unwrap();
        // With less it stops, at each of its charges.
        assert!((0..enough).step_by(7).all(|max_work| !work(&mut Budget { max_work, ..Budget::DEFAULT }.meter())));
        enough
    };
    // 7 stitches: 4 rows of 20 steps and 3 turns of 1. The cells count 3 times over, and each spot once.
    let (cells, spots, steps) = ((20 + 3) * (2 + 3), (20 + 4) * (2 + 4), 4 * 20 + 3);
    assert_eq!(needed(&|meter| Thickness::of(&plan, meter).is_ok()), 7 + 3 * cells + steps);
    let thickness = measured(&plan);
    assert_eq!(thickness.size(), (24, 6));
    assert_eq!(needed(&|meter| thickness.over(Measure::Fills, 1.0, meter).is_ok()), spots);
    // The 3 turns lie 5 mm from the spot, and are not followed.
    assert_eq!(needed(&|meter| thickness.stack(&plan, p(5.0, 0.5), meter).is_ok()), 7 + steps - 3);
    // The thickest spot lies where the rows turn: 4 rows and 2 turns are near it, by the grid laid from the
    // plan's first needle point, (0, 0.1).
    assert_eq!(needed(&|meter| thickness.report(&plan, &Limits::DEFAULT, meter).is_ok()), 4 * spots + 7 + 4 * 20 + 2);
}

/// A plan of a short stitch across (5, 5) sewn back and forth 200 times over, and 2 rows 5 mm away that
/// widen the grid: one thick place, round (5, 5), a corner of the cells.
fn knot() -> StitchPlan {
    let mut points: Vec<(f64, f64)> = (0..200).map(|k| if k % 2 == 0 { (4.6, 4.6) } else { (5.4, 5.4) }).collect();
    points.extend([(0.0, 0.0), (10.0, 0.0), (10.0, 10.0)]);
    plan(&[("a", Role::Top, &points)])
}

#[test]
fn req_auto_074_stitches_added_and_taken_out_measure_as_the_plan_does() {
    let sewn = plan(&[("a", Role::Top, &rows(2)), ("b", Role::Lock, &[(3.0, 3.0), (3.3, 3.0), (3.0, 3.0)])]);
    let whole = measured(&sewn);
    let mut added = whole.blank();
    assert!(added.sewn().is_empty() && added.most(Measure::Fills).value == 0.0 && added.size() == whole.size());
    for stitch in whole.sewn() {
        added.add(stitch, &mut meter()).unwrap();
    }
    let same = |a: &[f64], b: &[f64]| a.len() == b.len() && a.iter().zip(b).all(|(a, b)| near(*a, *b));
    assert!(same(&added.fills, &whole.fills) && same(&added.points, &whole.points));
    // Each spot's needle points: the locks' are not counted, as measuring leaves them out.
    let mut taken = whole.clone();
    for stitch in whole.sewn() {
        taken.remove(stitch, &mut meter()).unwrap();
    }
    assert!(taken.fills.iter().chain(&taken.points).all(|value| near(*value, 0.0)));
    // The stitches kept for the stack are the plan's until it is measured again.
    assert_eq!(taken.sewn(), whole.sewn());
    let shorter = plan(&[("a", Role::Top, &rows(1))]);
    taken.resewn(&shorter, &mut meter()).unwrap();
    assert_eq!(taken.sewn(), measured(&shorter).sewn());
}

#[test]
fn req_auto_074_the_same_stitches_moved_anywhere_measure_the_same() {
    // The grid of 0.1 mm is laid from the plan's first needle point: a place is rounded from there.
    let origin = p(0.33, 0.47);
    assert_eq!(super::placed(p(0.37, 0.41), origin), p(0.0, -0.1));
    assert_eq!(super::placed(p(0.48, 0.72), origin), Point::from_tenths(2, 2));
    let points: Vec<(f64, f64)> = rows(2).iter().map(|(x, y)| (x * 1.03 + 0.017, y * 0.97 + 0.004)).collect();
    let moved: Vec<(f64, f64)> = points.iter().map(|(x, y)| (x + 62.999_59, y + 84.010_07)).collect();
    let (here, there) = (measured(&plan(&[("a", Role::Top, &points)])), measured(&plan(&[("a", Role::Top, &moved)])));
    assert_eq!((here.fills.clone(), here.points.clone(), here.sewn()), (there.fills.clone(), there.points.clone(), there.sewn()));
    // Its spots are where the plan has them, and a place is found at its point.
    let (most, far) = (here.most(Measure::Fills), there.most(Measure::Fills));
    assert!((far.at.x() - most.at.x() - 62.999_59).abs() < 1e-9 && (far.at.y() - most.at.y() - 84.010_07).abs() < 1e-9, "{most:?} {far:?}");
    assert!(near(there.at(Measure::Fills, far.at), most.value) && near(there.at_place(Measure::Fills, there.placed(far.at)), most.value));
    let place = there.placed(p(70.0, 90.0));
    assert!(there.point(place).distance(p(70.0, 90.0)) <= 0.05 * 2.0_f64.sqrt() + 1e-9);
    assert_eq!(there.placed(there.point(place)), place);
}

#[test]
fn req_auto_074_the_hottest_spot_of_a_stitch_is_the_most_of_the_spots_it_lays_thread_in() {
    let thickness = measured(&knot());
    let most = thickness.most(Measure::Fills);
    assert!(most.value > 10.0, "{most:?}");
    assert_eq!(most.at, p(5.0, 5.0));
    let through = placed(&thickness, stitch((4.6, 4.6), (5.4, 5.4)));
    assert!(near(thickness.hottest(&through, &mut meter()).unwrap(), most.value));
    // A stitch of the far row lays thread only beside it; one outside the grid lays none.
    let row = thickness.hottest(&placed(&thickness, stitch((1.0, 0.0), (2.0, 0.0))), &mut meter()).unwrap();
    assert!(row > 0.0 && row < 1.0, "{row}");
    assert_eq!(thickness.hottest(&placed(&thickness, stitch((50.0, 50.0), (51.0, 50.0))), &mut meter()).unwrap(), 0.0);
}

#[test]
fn req_auto_074_the_spots_of_a_stitch_are_those_it_lays_thread_in_with_their_fills() {
    let thickness = measured(&knot());
    let most = thickness.most(Measure::Fills);
    let through = placed(&thickness, stitch((4.6, 4.6), (5.4, 5.4)));
    let spots = thickness.spots(&through, &mut meter()).unwrap();
    // The thickest of them is the stitch's hottest, and each has the fills the measure has there.
    assert!(spots.len() >= 4, "{spots:?}");
    assert!(near(spots.values().fold(0.0_f64, |most, fills| most.max(*fills)), most.value));
    assert!(spots.iter().all(|(spot, fills)| thickness.fills.get(*spot) == Some(fills)), "{spots:?}");
    assert!(spots.values().all(|fills| *fills > 0.0) && spots.keys().all(|spot| *spot > 1), "{spots:?}");
    // One outside the grid lays thread in none.
    assert!(thickness.spots(&placed(&thickness, stitch((50.0, 50.0), (51.0, 50.0))), &mut meter()).unwrap().is_empty());
    assert_eq!(least(&|meter| thickness.spots(&stitch((5.0, 5.0), (6.0, 5.0)), meter).is_ok()), 2);
}

#[test]
fn req_auto_074_the_hottest_spot_of_a_stitch_is_a_corner_of_its_cell() {
    // A short stitch sewn 100 times in the middle of a cell, 4 mm along and 2 mm up from the first needle
    // point, and another in the cell below it: the corners the 2 cells share are the thickest spots, and the
    // hottest of the upper stitch, which lays thread in its cell's 4 corners.
    let upper = knot_at((4.2, 2.25), (4.3, 2.25));
    let lower = knot_at((4.2, 2.75), (4.3, 2.75));
    let points: Vec<(f64, f64)> = [vec![(0.0, 0.0)], upper, lower, vec![(10.0, 10.0)]].concat();
    let thickness = measured(&plan(&[("a", Role::Top, &points)]));
    let most = thickness.most(Measure::Fills);
    assert!(most.value > 5.0 && near(most.at.y(), 2.5), "{most:?}");
    let through = placed(&thickness, stitch((4.2, 2.25), (4.3, 2.25)));
    assert!(near(thickness.hottest(&through, &mut meter()).unwrap(), most.value));
}

/// A stitch from `a` to `b` sewn back and forth 100 times.
fn knot_at(a: (f64, f64), b: (f64, f64)) -> Vec<(f64, f64)> {
    (0..=100).map(|k| if k % 2 == 0 { a } else { b }).collect()
}

#[test]
fn req_auto_074_stitches_measured_alone_lie_from_their_first_needle_point() {
    let sewn = [stitch((1.5, 2.5), (3.0, 4.5)), stitch((3.0, 4.5), (1.0, 4.0))];
    let thickness = Thickness::of_stitches(&sewn, &mut meter()).unwrap().unwrap();
    let kept: Vec<[(f64, f64); 2]> = thickness.sewn().iter().map(|s| [(s.from.x(), s.from.y()), (s.to.x(), s.to.y())]).collect();
    assert_eq!(kept, [[(0.0, 0.0), (1.5, 2.0)], [(1.5, 2.0), (-0.5, 1.5)]]);
    assert_eq!(thickness.point(p(0.0, 0.0)), p(1.5, 2.5));
}

#[test]
fn req_auto_074_a_zone_is_the_cells_within_reach_of_a_spot_over_its_limit() {
    let thickness = measured(&knot());
    let Spot { value, at } = thickness.most(Measure::Fills);
    let (x, y) = (at.x(), at.y());
    // Over: more than the limit. A spot just at it marks nothing.
    let zone = thickness.near(value - 1e-6, 1.0, &mut meter()).unwrap();
    let none = thickness.near(value, 1.0, &mut meter()).unwrap();
    let short = |x0: f64, y0: f64| placed(&thickness, stitch((x + x0, y + y0), (x + x0 + 0.05, y + y0 + 0.05)));
    assert!(zone.touches(&short(0.0, 0.0)) && !none.touches(&short(0.0, 0.0)));
    // Cells of 0.5 mm: 1 mm of reach marks 3 cells to each side of the spot, 1.5 mm.
    for (x0, y0) in [(1.4, 0.0), (-1.45, 0.0), (0.0, 1.4), (0.0, -1.45), (1.4, 1.4), (-1.45, -1.45)] {
        assert!(zone.touches(&short(x0, y0)), "({x0}, {y0})");
    }
    for (x0, y0) in [(1.55, 0.0), (-1.6, 0.0), (0.0, 1.55), (0.0, -1.6), (1.55, -1.6), (-1.6, 1.4)] {
        assert!(!zone.touches(&short(x0, y0)), "({x0}, {y0})");
    }
    // A stitch beyond the grid touches no cell.
    assert!(
        !zone.touches(&placed(&thickness, stitch((x, y + 30.0), (x, y + 40.0))))
            && !zone.touches(&placed(&thickness, stitch((x - 30.0, y), (x - 20.0, y))))
    );
    // A long stitch whose box holds the zone touches it, wherever its needle points lie.
    assert!(zone.touches(&placed(&thickness, stitch((x - 3.0, y + 3.0), (x + 3.0, y - 3.0)))));
    // A wider reach marks more.
    let wide = thickness.near(value - 1e-6, 2.0, &mut meter()).unwrap();
    assert!(wide.touches(&short(1.55, 0.0)) && !wide.touches(&short(2.55, 0.0)));
}

#[test]
fn req_auto_074_a_zone_lies_round_its_spot_by_column_and_row() {
    // A knot round (7, 3) in a grid 10 mm across: the zone round it touches it, and not the place across the
    // grid's diagonal from it.
    let mut points: Vec<(f64, f64)> = (0..200).map(|k| if k % 2 == 0 { (6.6, 2.6) } else { (7.4, 3.4) }).collect();
    points.extend([(0.0, 0.0), (10.0, 0.0), (10.0, 10.0)]);
    let thickness = measured(&plan(&[("a", Role::Top, &points)]));
    let Spot { value, at } = thickness.most(Measure::Fills);
    assert_eq!(at, p(7.0, 3.0));
    let zone = thickness.near(value - 1e-6, 0.5, &mut meter()).unwrap();
    let short = |x: f64, y: f64| placed(&thickness, stitch((x, y), (x + 0.05, y + 0.05)));
    assert!(zone.touches(&short(7.0, 3.0)) && !zone.touches(&short(3.0, 7.0)));
}

#[test]
fn req_auto_074_the_pass_s_measures_stop_when_the_work_runs_out() {
    let plan = knot();
    let thickness = measured(&plan);
    let through = stitch((5.0, 5.0), (6.0, 5.0));
    // 2 steps along the stitch.
    assert_eq!(least(&|meter| thickness.clone().add(&through, meter).is_ok()), 2);
    assert_eq!(least(&|meter| thickness.clone().remove(&through, meter).is_ok()), 2);
    assert_eq!(least(&|meter| thickness.hottest(&through, meter).is_ok()), 2);
    assert_eq!(least(&|meter| thickness.clone().resewn(&plan, meter).is_ok()), thickness.sewn().len() as u64);
    // Each spot, and the cells marked round the one spot over 30 fills: 1 cell of reach and 1 more, 2 to each side.
    let spots = thickness.fills.len() as u64;
    let over = thickness.fills.iter().filter(|fills| **fills > 30.0).count();
    assert_eq!(over, 1);
    assert_eq!(least(&|meter| thickness.near(30.0, 0.5, meter).is_ok()), spots + 16);
}
