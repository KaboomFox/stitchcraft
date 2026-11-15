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

/// A plan of one block: each list of points is an element's stitches in a role, sewn on from the last.
fn plan(parts: &[(&str, Role, &[(f64, f64)])]) -> StitchPlan {
    let mut builder = PlanBuilder::new(Thread::new(Rgb::new(0, 0, 0)));
    for (id, role, points) in parts {
        let element = builder.element(ElementId::new(*id).unwrap()).unwrap();
        for (x, y) in *points {
            builder.stitch(p(*x, *y), Provenance::new(element, *role));
        }
    }
    builder.finish()
}

/// The rows of a fill of the square from (0, 0) to (10, 10), a thread apart, sewn back and forth `layers`
/// times over.
fn rows(layers: usize) -> Vec<(f64, f64)> {
    let mut points = Vec::new();
    for _ in 0..layers {
        for row in 0..26 {
            let y = f64::from(row) * THREAD;
            let ends = if row % 2 == 0 { [0.0, 10.0] } else { [10.0, 0.0] };
            points.extend(ends.map(|x| (x, y)));
        }
    }
    points
}

fn measured(plan: &StitchPlan) -> Thickness {
    Thickness::of(plan, &mut meter()).unwrap().unwrap()
}

#[test]
fn req_auto_060_a_square_sewn_1_2_3_and_6_times_measures_as_many_fills() {
    for layers in [1, 2, 3, 6] {
        let rows = rows(layers);
        let thickness = measured(&plan(&[("a", Role::Top, &rows)]));
        // Inside the square, further than half a window from its edge, every window holds 2.5 rows' worth of
        // thread, wherever the rows fall in it: the lanes of a row lie 0.1 mm apart. Rows a thread apart are
        // 0.625 of a fill, whose rows lie 0.25 mm apart.
        for at in [(5.0, 5.0), (5.1, 4.9), (3.3, 7.7), (1.0, 1.0), (9.0, 9.0)] {
            let fills = thickness.at(Measure::Fills, p(at.0, at.1));
            assert!(near(fills, layers as f64 * 0.625), "{layers} layers at {at:?}: {fills}");
        }
        // Outside it by more than half a window and half a thread there is none.
        assert_eq!(thickness.at(Measure::Fills, p(5.0, 10.75)), 0.0);
        assert_eq!(thickness.at(Measure::Fills, p(100.0, 5.0)), 0.0);
        assert_eq!(thickness.at(Measure::Fills, p(5.0, -100.0)), 0.0);
    }
}

#[test]
fn req_auto_060_every_millimetre_of_thread_is_counted_once_whatever_its_role_and_direction() {
    let points = [(0.0, 0.0), (7.0, 3.0), (7.0, 3.0), (2.0, 9.5), (2.13, 9.41), (-3.0, 4.0)];
    let plan = plan(&[("a", Role::Underlay, &points[..3]), ("b", Role::Travel, &points[3..])]);
    let thickness = measured(&plan);
    let length: f64 = plan.sewn_stitches().iter().map(SewnStitch::length).sum();
    let summed: f64 = thickness.fills.iter().sum::<f64>() * CELL * CELL;
    assert!(near(summed, length * ROWS), "{summed} of {}", length * ROWS);
    assert!(thickness.fills.iter().all(|fills| fills.is_finite()));
    // The grid has room for a window round every stitch, and no more.
    assert_eq!(thickness.size(), (50 + 1 + 2 * MARGIN, 47 + 1 + 2 * MARGIN));
    assert_eq!(thickness.grid.corner, (-3.0 - 0.8, 0.0 - 0.8));
}

#[test]
fn req_auto_060_a_ribbon_is_a_thread_wide() {
    // One stitch along x at y = 0.1, on a grid whose cells start at 0: its 4 lanes lie at -0.05, 0.05, 0.15 and 0.25, in the rows of cells
    // from -0.2, 0 and 0.2, which hold 1, 2 and 1 of them.
    let thickness = measured(&plan(&[("a", Role::Top, &[(0.0, 0.0), (10.0, 0.0)])]));
    let grid = &thickness.grid;
    let mut thread = vec![0.0; grid.columns * grid.rows];
    let stitch = SewnStitch { block: 0, index: 1, from: p(0.0, 0.1), to: p(10.0, 0.1), role: Role::Top, from_role: Role::Top };
    laid(grid, &stitch, &mut meter(), |at, length| thread[at] += length).unwrap();
    let column = MARGIN + 20;
    let cell = |row: usize| thread[row * grid.columns + column];
    let rows: Vec<f64> = (MARGIN - 2..MARGIN + 3).map(cell).collect();
    assert!(rows.iter().zip([0.0, 0.05, 0.1, 0.05, 0.0]).all(|(a, b)| near(*a, b)), "{rows:?}");
}

#[test]
fn req_auto_060_needle_points_are_counted_per_square_millimetre_without_the_locks() {
    let top = [(0.0, 0.0), (5.05, 0.05), (5.1, 0.1), (5.15, 0.05), (12.0, 0.05)];
    let lock = [(12.05, 0.05), (12.1, 0.05), (12.05, 0.05)];
    let after = [(12.1, 0.1), (20.0, 0.05)];
    let thickness = measured(&plan(&[("a", Role::Top, &top), ("a", Role::Lock, &lock), ("a", Role::Top, &after)]));
    let points = |x: f64, y: f64| thickness.at(Measure::Points, p(x, y));
    // The first needle point starts the thread and is no stitch's.
    assert_eq!(points(0.0, 0.0), 0.0);
    assert_eq!(points(5.1, 0.1), 3.0);
    // Half a window away the 3 are still in the window, and further they are not.
    assert_eq!((points(5.5, 0.5), points(4.7, -0.3), points(5.7, 0.1), points(5.1, 0.7)), (3.0, 3.0, 0.0, 0.0));
    // The point before a lock counts, and neither the lock's stitches nor the one out of it.
    assert_eq!(points(12.05, 0.05), 1.0);
    assert_eq!(points(20.0, 0.05), 1.0);
    assert_eq!(thickness.most(Measure::Points).value, 3.0);
}

/// A thickness of `columns` by `rows` cells from (1, 2) whose cells in `over` have 3 fills and 4 points.
fn drawn(columns: usize, rows: usize, over: &[usize]) -> Thickness {
    let mut fills = vec![0.5; columns * rows];
    for at in over {
        fills[*at] = 3.0;
    }
    let points = fills.iter().map(|fills| if *fills == 3.0 { 4.0 } else { 0.0 }).collect();
    Thickness { grid: Grid { corner: (1.0, 2.0), columns, rows }, fills, points }
}

/// The cells of the block of `across` by `down` cells whose first is in `column` and `row`, of 12 columns.
fn block(column: usize, row: usize, across: usize, down: usize) -> Vec<usize> {
    (row..row + down).flat_map(|row| (column..column + across).map(move |column| row * 12 + column)).collect()
}

#[test]
fn req_auto_061_a_piece_of_a_square_millimetre_at_the_limit_counts() {
    let whole = drawn(12, 12, &block(3, 4, 5, 5));
    let over = whole.over(Measure::Fills, 3.0, &mut meter()).unwrap();
    assert!(over.pieces == 1 && boxes(over.at, Rect::around([p(1.7, 2.9), p(2.5, 3.7)])), "{over:?}");
    assert!(near(over.area, PIECE) && near(over.largest, PIECE), "{over:?}");
    // The other measure has its own values.
    assert_eq!(whole.over(Measure::Points, 4.0, &mut meter()).unwrap().pieces, 1);
    assert_eq!(whole.over(Measure::Points, 4.5, &mut meter()).unwrap(), Over::default());
    // A value under the limit is not over it, and one cell less is no piece.
    assert_eq!(whole.over(Measure::Fills, 3.000_001, &mut meter()).unwrap(), Over::default());
    let short = drawn(12, 12, &block(3, 4, 5, 5)[1..]);
    assert_eq!(short.over(Measure::Fills, 3.0, &mut meter()).unwrap(), Over::default());
    // Every cell over a limit of nothing is one piece.
    assert_eq!(whole.over(Measure::Fills, 0.5, &mut meter()).unwrap().pieces, 1);
    assert!(near(whole.over(Measure::Fills, 0.5, &mut meter()).unwrap().area, 144.0 * CELL * CELL));
}

#[test]
fn req_auto_061_cells_that_touch_at_a_corner_are_one_piece_and_rows_do_not_wrap() {
    // 2 blocks of 16 cells that touch at a corner are one piece of 32.
    let corner: Vec<usize> = [block(0, 0, 4, 4), block(4, 4, 4, 4)].concat();
    let over = drawn(12, 12, &corner).over(Measure::Fills, 3.0, &mut meter()).unwrap();
    assert!(over.pieces == 1 && near(over.area, 32.0 * CELL * CELL), "{over:?}");
    // A gap of one cell parts them.
    let apart: Vec<usize> = [block(0, 0, 4, 4), block(5, 5, 4, 4)].concat();
    assert_eq!(drawn(12, 12, &apart).over(Measure::Fills, 3.0, &mut meter()).unwrap().pieces, 0);
    // The last cells of a row and the first of the next don't touch.
    let ends: Vec<usize> = [block(8, 0, 4, 4), block(0, 4, 4, 4)].concat();
    assert_eq!(drawn(12, 12, &ends).over(Measure::Fills, 3.0, &mut meter()).unwrap().pieces, 0);
}

#[test]
fn req_auto_061_the_pieces_are_counted_and_the_first_of_the_largest_is_named() {
    let small = block(0, 0, 5, 5);
    let large = block(6, 6, 5, 6);
    let first = Rect::around([p(1.1, 2.1), p(1.9, 2.9)]);
    let second = Rect::around([p(2.3, 3.3), p(3.1, 4.3)]);
    let over = |cells: &[usize]| drawn(12, 12, cells).over(Measure::Fills, 3.0, &mut meter()).unwrap();
    let both = over(&[small.clone(), large.clone()].concat());
    assert!(both.pieces == 2 && near(both.area, 55.0 * CELL * CELL) && near(both.largest, 30.0 * CELL * CELL), "{both:?}");
    assert!(boxes(both.at, second), "{both:?}");
    // A larger piece found first stays the largest, and of 2 as large the first is named.
    let turned = over(&[block(0, 0, 5, 6), block(6, 6, 5, 5)].concat());
    assert!(near(turned.largest, 30.0 * CELL * CELL) && boxes(turned.at, Rect::around([p(1.1, 2.1), p(1.9, 3.1)])), "{turned:?}");
    let equal = over(&[small, block(6, 6, 5, 5)].concat());
    assert!(equal.pieces == 2 && boxes(equal.at, first), "{equal:?}");
}

/// Whether 2 boxes are the same to a nanometre.
fn boxes(a: Option<Rect>, b: Option<Rect>) -> bool {
    let (a, b) = (a.unwrap(), b.unwrap());
    let corners = [(a.min(), b.min()), (a.max(), b.max())];
    corners.iter().all(|(a, b)| near(a.x(), b.x()) && near(a.y(), b.y()))
}

#[test]
fn req_auto_061_the_thickest_spot_is_the_middle_of_its_cell() {
    let mut thickness = drawn(12, 12, &[]);
    thickness.fills[2 * 12 + 7] = 4.25;
    thickness.points[5 * 12 + 1] = 9.0;
    assert_eq!(thickness.most(Measure::Fills), Spot { value: 4.25, at: p(1.0 + 7.5 * CELL, 2.0 + 2.5 * CELL) });
    assert_eq!(thickness.most(Measure::Points), Spot { value: 9.0, at: p(1.0 + 1.5 * CELL, 2.0 + 5.5 * CELL) });
    assert_eq!(thickness.at(Measure::Fills, p(2.5, 2.5)), 4.25);
    assert_eq!(thickness.at(Measure::Points, p(1.3, 3.1)), 9.0);
    // The cells' edges: the first cell reaches from its corner, which is in it, to the next one's.
    assert_eq!(thickness.grid.place(1.0, 2.0), Some((0, 0)));
    assert_eq!(thickness.grid.place(1.0 + 12.0 * CELL - 1e-9, 2.0 + 12.0 * CELL - 1e-9), Some((11, 11)));
    assert_eq!(thickness.grid.place(1.0 - 1e-9, 2.0), None);
    assert_eq!(thickness.grid.place(1.0, 2.0 + 12.0 * CELL + 1e-9), None);
    assert_eq!(thickness.grid.place(f64::NAN, 2.0), None);
    let none = Thickness { grid: Grid { corner: (0.0, 0.0), columns: 0, rows: 0 }, fills: Vec::new(), points: Vec::new() };
    assert_eq!(none.most(Measure::Fills), Spot { value: 0.0, at: Point::ORIGIN });
}

#[test]
fn req_auto_062_the_stack_names_each_elements_thread_in_the_window() {
    let rows = rows(1);
    let base = [(0.0, 5.0), (10.0, 5.0), (10.0, 5.2), (0.0, 5.2)];
    let mut builder = PlanBuilder::new(Thread::new(Rgb::new(0, 0, 0)));
    let (a, b) = (builder.element(ElementId::new("a").unwrap()).unwrap(), builder.element(ElementId::new("b").unwrap()).unwrap());
    for (x, y) in base {
        builder.stitch(p(x, y), Provenance::new(a, Role::Underlay));
    }
    for (x, y) in &rows {
        builder.stitch(p(*x, *y), Provenance::new(a, Role::Top));
    }
    builder.change_thread(Thread::new(Rgb::new(255, 0, 0)));
    for (x, y) in &rows {
        builder.stitch(p(*x, *y), Provenance::new(b, Role::Top));
    }
    let plan = builder.finish();
    let thickness = measured(&plan);
    let at = p(5.0, 5.1);
    let stack = thickness.stack(&plan, at, &mut meter()).unwrap();
    assert_eq!(stack.len(), 2);
    assert_eq!((stack[0].block, stack[0].element, stack[1].block, stack[1].element), (0, Some(a), 1, Some(b)));
    assert!(near(stack[1].under, 0.0) && near(stack[1].top, 0.625), "{stack:?}");
    assert!(near(stack[0].top, 0.625) && stack[0].under > 0.4, "{stack:?}");
    assert!(near(stack[0].top + stack[0].under + 0.625, thickness.at(Measure::Fills, at)), "{stack:?}");
    // Outside the grid nothing lies.
    assert_eq!(thickness.stack(&plan, p(500.0, 0.0), &mut meter()).unwrap(), Vec::new());
}

#[test]
fn req_auto_062_a_report_holds_a_plan_to_its_limits() {
    let rows = rows(3);
    let plan = plan(&[("a", Role::Top, &rows)]);
    let thickness = measured(&plan);
    let limits = |warn_fills, fail_fills, warn_points, fail_points| Limits { warn_fills, fail_fills, warn_points, fail_points };
    let with = |limits: Limits| thickness.report(&plan, &limits, &mut meter()).unwrap();
    let report = with(limits(1.8, 6.0, 99.0, 99.0));
    assert!(report.warns() && report.thick.pieces == 1 && report.thick.area > 80.0, "{report:?}");
    assert_eq!((report.failing, report.dense, report.pierced), (Over::default(), Over::default(), Over::default()));
    assert_eq!(report.fills, thickness.most(Measure::Fills));
    assert_eq!(report.points, thickness.most(Measure::Points));
    assert!(near(report.stack.iter().map(|layer| layer.top + layer.under).sum::<f64>(), report.fills.value));
    // Each limit is its own, and a failing limit under the warning's is the warning's.
    assert!(!with(limits(4.5, 6.0, 99.0, 99.0)).warns());
    assert_eq!(with(limits(4.5, 2.5, 99.0, 99.0)).failing, Over::default());
    assert_eq!(with(limits(1.5, 1.8, 99.0, 99.0)).failing, report.thick);
    // The rows' ends lie along 2 of the square's sides, 3 times over: 6 and 9 to a square millimetre.
    let dense = with(limits(9.0, 9.0, 2.0, 1.0));
    assert!(dense.warns() && dense.thick.pieces == 0 && dense.dense.pieces == 2 && dense.pierced == dense.dense, "{dense:?}");
    assert_eq!(with(limits(9.0, 9.0, 2.0, 6.0)).pierced.pieces, 2);
    assert_eq!(with(limits(9.0, 9.0, 2.0, 99.0)).pierced.pieces, 0);
}

#[test]
fn req_auto_062_the_steps_of_the_heat_map_start_at_their_limits() {
    let limits = Limits::DEFAULT;
    assert_eq!((limits.warn_fills, limits.fail_fills, limits.warn_points, limits.fail_points), (3.0, 6.0, 15.0, 30.0));
    let steps = [0.0, 0.01, 1.99, 2.0, 2.99, 3.0, 5.99, 6.0, 40.0].map(|fills| limits.step(fills));
    let expected = [Step::Bare, Step::Ordinary, Step::Ordinary, Step::Doubled, Step::Doubled, Step::Thick, Step::Thick, Step::Failing, Step::Failing];
    assert_eq!(steps, expected);
    // A failing limit under the warning's is the warning's.
    let turned = Limits { fail_fills: 1.0, ..limits };
    assert_eq!((turned.step(2.5), turned.step(3.0)), (Step::Doubled, Step::Failing));
}

#[test]
fn a_plan_without_a_stitch_has_no_thickness_and_one_too_large_is_not_measured() {
    assert_eq!(Thickness::of(&StitchPlan::default(), &mut meter()), Ok(None));
    assert_eq!(Thickness::of(&plan(&[("a", Role::Top, &[(1.0, 1.0)])]), &mut meter()), Ok(None));
    let wide = plan(&[("a", Role::Top, &[(0.0, 0.0), (600.0, 0.0), (600.0, 600.0)])]);
    assert_eq!(Thickness::of(&wide, &mut meter()), Err(Exhausted::Work));
    // 2,500 cells on a side are the most.
    let side = |extent: f64| Rect::around([p(0.0, 0.0), p(extent, 498.3)]).unwrap();
    let most = Grid::round(side(498.3)).unwrap();
    assert_eq!((most.columns, most.rows, most.columns * most.rows), (2500, 2500, MAX_CELLS));
    assert_eq!(Grid::round(side(498.5)), None);
}

#[test]
fn measuring_stops_when_the_work_runs_out() {
    let rows = &rows(1)[..8];
    let plan = plan(&[("a", Role::Top, rows)]);
    let needed = |work: &dyn Fn(&mut Meter) -> bool| {
        let enough = (0..1_000_000u64).find(|max_work| work(&mut Budget { max_work: *max_work, ..Budget::DEFAULT }.meter())).unwrap();
        // With less it stops, at each of its charges.
        assert!((0..enough).step_by(97).all(|max_work| !work(&mut Budget { max_work, ..Budget::DEFAULT }.meter())));
        enough
    };
    // 7 stitches, the cells 3 times over, and a step for each tenth of a millimetre begun.
    let cells = (50 + 1 + 2 * MARGIN) * (6 + 1 + 2 * MARGIN);
    let steps: usize = plan.sewn_stitches().iter().map(|stitch| (stitch.length() / 0.1).ceil() as usize).sum();
    assert!((412..=415).contains(&steps));
    assert_eq!(needed(&|meter| Thickness::of(&plan, meter).is_ok()), (7 + 3 * cells + steps) as u64);
    let thickness = measured(&plan);
    assert_eq!(needed(&|meter| thickness.over(Measure::Fills, 1.0, meter).is_ok()), cells as u64);
    assert_eq!(needed(&|meter| thickness.stack(&plan, p(5.0, 0.4), meter).is_ok()), (7 + steps) as u64);
    assert_eq!(needed(&|meter| thickness.report(&plan, &Limits::DEFAULT, meter).is_ok()), (4 * cells + 7 + steps) as u64);
}
