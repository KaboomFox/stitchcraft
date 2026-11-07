// Test code may unwrap and index.
use stitchcraft_core::Budget;

use super::*;

fn p(x: f64, y: f64) -> Point {
    Point::new(x, y).unwrap()
}

/// The rectangle from `(x0, y0)` to `(x1, y1)` as a part, wound as a part is: clockwise in y-up axes.
fn rect((x0, y0): (f64, f64), (x1, y1): (f64, f64)) -> Polygon {
    Polygon { outline: vec![p(x0, y0), p(x0, y1), p(x1, y1), p(x1, y0), p(x0, y0)], holes: Vec::new() }
}

fn region(parts: Vec<Polygon>) -> Region {
    Region { parts }
}

/// Ink/Stitch's defaults: square cells of 3 mm on the canvas's grid, half covered, 4 threads.
fn plain() -> Cross {
    Cross {
        size: [3.0, 3.0],
        on_canvas: true,
        offset: [0.0, 0.0],
        rotation: 0.0,
        coverage: 50.0,
        method: Method::Simple,
        flipped: false,
        threads: 4,
        longest: 11.0,
        expand: 0.0,
        min_stitch: 0.3,
    }
}

fn sewn(region: &Region, cross: &Cross, needle: Option<Point>, next: Option<&Approach>) -> Stitched {
    cross_stitch(region, cross, needle, next, &mut Budget::DEFAULT.meter()).unwrap()
}

/// The cells, by their top left corners, whose 2 diagonals `stitched` passes along, on square cells of
/// `size` from the origin: each needle point lies on a corner or a middle of one.
#[allow(clippy::cast_possible_truncation)]
fn crosses(runs: &[Vec<Point>], size: f64) -> Vec<(i64, i64)> {
    let mut cells: Vec<(i64, i64)> = runs
        .iter()
        .flat_map(|run| run.windows(2))
        .filter(|w| ((w[0].x() - w[1].x()).abs() - size).abs() < 1e-9 && ((w[0].y() - w[1].y()).abs() - size).abs() < 1e-9)
        .map(|w| ((w[0].x().min(w[1].x()) / size).round() as i64, (w[0].y().min(w[1].y()) / size).round() as i64))
        .collect();
    cells.sort_unstable();
    cells.dedup();
    cells
}

#[test]
fn req_fill_crs_001_a_square_on_the_grid_sews_a_cross_in_each_of_its_cells() {
    // A 6 mm square from the origin on 3 mm cells: 4 crosses in one run from the first cross's top right
    // corner back to it, each of 4 threads: 10 needle points a cross, and the start.
    let stitched = sewn(&region(vec![rect((0.0, 0.0), (6.0, 6.0))]), &plain(), None, None);
    assert_eq!(stitched.warnings, Vec::new());
    assert_eq!(stitched.runs.len(), 1);
    let run = &stitched.runs[0];
    assert_eq!((run.len(), run.first(), run.last()), (41, Some(&p(3.0, 0.0)), Some(&p(3.0, 0.0))));
    assert_eq!(crosses(&stitched.runs, 3.0), [(0, 0), (0, 1), (1, 0), (1, 1)]);
    // With 2 threads, 6 needle points a cross.
    assert_eq!(sewn(&region(vec![rect((0.0, 0.0), (6.0, 6.0))]), &Cross { threads: 2, ..plain() }, None, None).runs[0].len(), 25);
}

#[test]
fn req_fill_crs_001_the_grid_lies_on_the_canvas_or_on_the_part_moved_by_the_offset() {
    // A rectangle from (10, 9.2) to (22, 19), as Ink/Stitch sews it: on the canvas's grid, crosses from x = 9
    // to 21 and from y = 9 to 18; moved 1 mm left, from 11 to 23 (the line at -1 lies at 2 and 5 too); moved
    // 2 mm down, from 11 to 20, where Ink/Stitch misses the row from 8 to 11 (DEV-FILL-025).
    let part = region(vec![rect((10.0, 9.2), (22.0, 19.0))]);
    let bounds = |cross: &Cross| {
        let points: Vec<Point> = sewn(&part, cross, None, None).runs.concat();
        let xs = points.iter().map(|q| q.x());
        let ys = points.iter().map(|q| q.y());
        [
            xs.clone().fold(f64::INFINITY, f64::min),
            ys.clone().fold(f64::INFINITY, f64::min),
            xs.fold(f64::NEG_INFINITY, f64::max),
            ys.fold(f64::NEG_INFINITY, f64::max),
        ]
    };
    let near = |a: [f64; 4], b: [f64; 4]| a.iter().zip(b).all(|(a, b)| (a - b).abs() < 1e-9);
    assert!(near(bounds(&plain()), [9.0, 9.0, 21.0, 18.0]), "{:?}", bounds(&plain()));
    assert!(near(bounds(&Cross { offset: [1.0, 0.0], ..plain() }), [11.0, 9.0, 23.0, 18.0]));
    assert!(near(bounds(&Cross { offset: [0.0, 2.0], ..plain() }), [9.0, 8.0, 21.0, 20.0]));
    // Off the canvas's grid, the grid starts at the part's top left corner, moved 0.5 mm left and 1 mm down:
    // its lines lie at x = 9.5 and y = 10.2, and the row above, which the part covers a third of, sews none.
    let off = Cross { on_canvas: false, offset: [0.5, 1.0], ..plain() };
    assert!(near(bounds(&off), [9.5, 10.2, 21.5, 19.2]), "{:?}", bounds(&off));
    // 2 lengths make the cells 2 mm wide and 3 tall, the canvas's grid through the origin.
    assert!(near(bounds(&Cross { size: [2.0, 3.0], ..plain() }), [10.0, 9.0, 22.0, 18.0]));
}

#[test]
#[allow(clippy::cast_possible_truncation)]
fn req_fill_crs_003_flipped_crosses_lie_on_the_same_grid_with_their_slash_under_their_backslash() {
    // The same cells as plain crosses, on cells 2 mm wide and 3 tall moved off the origin, where Ink/Stitch's
    // would be 3 wide and 2 tall (DEV-FILL-026); each cross's last pass lies along its `\` diagonal, where a
    // plain cross's lies along its `/`.
    let part = region(vec![rect((10.0, 9.2), (22.0, 19.0))]);
    let cross = Cross { size: [2.0, 3.0], offset: [0.5, 1.0], threads: 2, ..plain() };
    let cells = |cross: &Cross| {
        let mut found: Vec<(i64, i64)> = sewn(&part, cross, None, None)
            .runs
            .concat()
            .windows(2)
            .filter(|w| (w[0].x() - w[1].x()).abs() > 1.9 && (w[0].y() - w[1].y()).abs() > 2.9)
            .map(|w| (((w[0].x().min(w[1].x()) + 0.5) / 2.0).round() as i64, ((w[0].y().min(w[1].y()) - 1.0) / 3.0).round() as i64))
            .collect();
        found.sort_unstable();
        found.dedup();
        found
    };
    let flipped = Cross { flipped: true, ..cross };
    assert_eq!(cells(&flipped), cells(&cross));
    assert_eq!(cells(&cross).len(), 18);
    // A square frame 12 mm wide round a hole 6 mm wide, on the default grid: its hole is turned with it, and
    // its 12 crosses lie round the hole, flipped or not.
    let hole = vec![p(3.0, 3.0), p(9.0, 3.0), p(9.0, 9.0), p(3.0, 9.0), p(3.0, 3.0)];
    let frame = region(vec![Polygon { holes: vec![hole], ..rect((0.0, 0.0), (12.0, 12.0)) }]);
    let framed = |cross: &Cross| crosses(&sewn(&frame, cross, None, None).runs, 3.0);
    let ring: Vec<(i64, i64)> =
        (0..4).flat_map(|i| (0..4).map(move |j| (i, j))).filter(|&(i, j)| !(1..3).contains(&i) || !(1..3).contains(&j)).collect();
    assert_eq!(framed(&plain()), ring);
    assert_eq!(framed(&Cross { flipped: true, ..plain() }), ring);
    // A single cross from the origin, 2 mm wide and 3 tall: plain, its last diagonal rises to the right (`/`);
    // flipped, it falls to the right (`\`).
    let one = region(vec![rect((0.0, 0.0), (2.0, 3.0))]);
    let last_diagonal = |cross: &Cross| {
        let run = sewn(&one, cross, None, None).runs.concat();
        let diagonals: Vec<(Point, Point)> = run.windows(2).filter(|w| (w[0].x() - w[1].x()).abs() > 1.9).map(|w| (w[0], w[1])).collect();
        let (a, b) = *diagonals.last().unwrap();
        (b.x() - a.x()) * (b.y() - a.y()) < 0.0
    };
    let small = Cross { size: [2.0, 3.0], ..plain() };
    assert!(last_diagonal(&small), "a plain cross ends along its `/` diagonal");
    assert!(!last_diagonal(&Cross { flipped: true, ..small }), "a flipped one along its `\\` diagonal");
}

#[test]
fn req_fill_crs_005_a_stitch_longer_than_the_longest_is_split_and_one_as_long_is_not() {
    // A 3 mm cell: its diagonals 4.24 mm long and its halves 2.12. At 2 mm, a diagonal is 3 stitches and a
    // half 2. At exactly a half's length, a half is one stitch and a diagonal 2.
    let one = region(vec![rect((0.0, 0.0), (3.0, 3.0))]);
    let cross = Cross { threads: 2, ..plain() };
    let lengths = |longest: f64| {
        let run = sewn(&one, &Cross { longest, ..cross.clone() }, None, None).runs.concat();
        run.windows(2).map(|w| (w[0].distance(w[1]) * 1000.0).round() / 1000.0).collect::<Vec<f64>>()
    };
    // Unsplit: TR, middle, TL, BR, middle, BL, TR: 2 halves, a diagonal, 2 halves, a diagonal.
    assert_eq!(lengths(11.0), [2.121, 2.121, 4.243, 2.121, 2.121, 4.243]);
    assert_eq!(lengths(2.0).len(), 2 + 2 + 3 + 2 + 2 + 3);
    assert!(lengths(2.0).iter().all(|&l| l <= 2.0));
    let half = 1.5 * 2f64.sqrt();
    assert_eq!(lengths(half), [2.121, 2.121, 2.121, 2.121, 2.121, 2.121, 2.121, 2.121]);
    assert_eq!(pieces_of(half, half), 1);
    assert_eq!(pieces_of(half * 1.000_001, half), 2);
    assert_eq!(pieces_of(0.0, half), 1);
}

/// Whether `x` lies on the segment from `p` to `q`.
fn on_segment(x: Point, p: Point, q: Point) -> bool {
    let cross = (q.x() - p.x()) * (x.y() - p.y()) - (q.y() - p.y()) * (x.x() - p.x());
    cross.abs() < 1e-9 && x.distance(p) + x.distance(q) < p.distance(q) + 1e-9
}

/// How many stitches of `runs` pass over the half bar from `middle` to `end`: those whose line holds both.
fn sewn_over(runs: &[Vec<Point>], middle: Point, end: Point) -> usize {
    runs.iter().flat_map(|run| run.windows(2)).filter(|w| on_segment(middle, w[0], w[1]) && on_segment(end, w[0], w[1])).count()
}

/// Whether the last stitch of `runs` through `middle` along a diagonal falls to the right as drawn, `\\`;
/// `None` when none passes it.
fn on_top(runs: &[Vec<Point>], middle: Point) -> Option<bool> {
    let mut through =
        runs.iter().flat_map(|run| run.windows(2)).filter(|w| on_segment(middle, w[0], w[1]) && w[0].x() != w[1].x() && w[0].y() != w[1].y());
    through.next_back().map(|w| (w[1].x() - w[0].x()) * (w[1].y() - w[0].y()) > 0.0)
}

/// The ends of the bars `bars` names in the cell `size` mm wide from (`x`, `y`): `\` and `/` its diagonals,
/// `-` its bar across and `|` its bar down.
fn bar_ends((x, y): (f64, f64), size: f64, bars: &[&str]) -> Vec<Point> {
    let at = |u: f64, v: f64| p(x + u * size, y + v * size);
    bars.iter()
        .flat_map(|bar| match *bar {
            "\\" => [at(0.0, 0.0), at(1.0, 1.0)],
            "/" => [at(0.0, 1.0), at(1.0, 0.0)],
            "-" => [at(0.0, 0.5), at(1.0, 0.5)],
            _ => [at(0.5, 0.0), at(0.5, 1.0)],
        })
        .collect()
}

#[test]
fn req_fill_crs_008_each_method_sews_each_half_of_each_of_its_bars_its_thread_count_times() {
    // A 6 mm square on the 3 mm grid, 2 threads: each half of each bar of each cross twice, whichever
    // method, and no other stitch through a cross's middle.
    let square = region(vec![rect((0.0, 0.0), (6.0, 6.0))]);
    for (method, bars) in [
        (Method::Simple, &["\\", "/"][..]),
        (Method::Upright, &["-", "|"]),
        (Method::Double, &["\\", "-", "|", "/"]),
        (Method::Smyrna, &["\\", "/", "-", "|"]),
    ] {
        for flipped in [false, true] {
            let stitched = sewn(&square, &Cross { method, flipped, threads: 2, ..plain() }, None, None);
            assert_eq!(stitched.warnings, Vec::new(), "{method:?}");
            for cell in [(0.0, 0.0), (3.0, 0.0), (0.0, 3.0), (3.0, 3.0)] {
                let middle = p(cell.0 + 1.5, cell.1 + 1.5);
                for end in bar_ends(cell, 3.0, bars) {
                    assert_eq!(sewn_over(&stitched.runs, middle, end), 2, "{method:?} {flipped} {cell:?} {end:?}");
                }
            }
        }
    }
}

#[test]
fn req_fill_crs_008_dense_upright_crosses_add_a_grid_of_them_on_the_corners_half_covered_ones_too() {
    // A 6 mm square: its 4 cells' upright crosses, and those of the second grid on the corners it covers by
    // half or more: the one in its middle, wholly, and the 4 on the middles of its sides, by half.
    let square = region(vec![rect((0.0, 0.0), (6.0, 6.0))]);
    let stitched = sewn(&square, &Cross { method: Method::DenseUpright, threads: 2, ..plain() }, None, None);
    for corner in [(3.0, 3.0), (3.0, 0.0), (0.0, 3.0), (6.0, 3.0), (3.0, 6.0)] {
        let middle = p(corner.0, corner.1);
        for end in bar_ends((corner.0 - 1.5, corner.1 - 1.5), 3.0, &["-", "|"]) {
            assert_eq!(sewn_over(&stitched.runs, middle, end), 2, "{corner:?} {end:?}");
        }
    }
    // The corners of the square are covered a quarter, and sew none.
    let reach = |runs: &[Vec<Point>]| runs.concat().iter().fold([f64::INFINITY, f64::NEG_INFINITY], |[lo, hi], q| [lo.min(q.x()), hi.max(q.x())]);
    assert_eq!(reach(&stitched.runs), [-1.5, 7.5]);
    assert_eq!(sewn_over(&stitched.runs, p(0.0, 0.0), p(-1.5, 0.0)), 0);
    // All of them in one run: they share the ends of their arms.
    assert_eq!(stitched.runs.len(), 1);
}

#[test]
fn req_fill_crs_010_half_crosses_sew_one_diagonal_in_each_cell_an_odd_number_of_times() {
    // A 6 mm square, 3 threads: each cell's `\` diagonal 3 times, or flipped its `/`, and neither the other.
    let square = region(vec![rect((0.0, 0.0), (6.0, 6.0))]);
    for (flipped, sewn_bar, other) in [(false, "\\", "/"), (true, "/", "\\")] {
        let stitched = sewn(&square, &Cross { method: Method::Half, flipped, threads: 3, ..plain() }, None, None);
        assert_eq!((stitched.warnings.len(), stitched.runs.len()), (0, 1), "{flipped}");
        for cell in [(0.0, 0.0), (3.0, 0.0), (0.0, 3.0), (3.0, 3.0)] {
            let middle = p(cell.0 + 1.5, cell.1 + 1.5);
            for end in bar_ends(cell, 3.0, &[sewn_bar]) {
                assert_eq!(sewn_over(&stitched.runs, middle, end), 3, "{flipped} {cell:?}");
            }
            for end in bar_ends(cell, 3.0, &[other]) {
                assert_eq!(sewn_over(&stitched.runs, middle, end), 0, "{flipped} {cell:?}");
            }
        }
    }
    // A part too small for a cell sews nothing, and says so, as a full cross's does.
    let speck = sewn(&region(vec![rect((0.0, 0.0), (1.0, 1.0))]), &Cross { method: Method::Half, ..plain() }, None, None);
    assert_eq!((speck.runs.len(), speck.warnings.iter().map(|d| d.code).collect::<Vec<_>>()), (0, vec![Code::FillCrossNoCell]));
}

#[test]
fn req_fill_crs_011_a_grid_turned_with_its_part_sews_the_part_s_crosses_turned() {
    // A 6 mm square turned 30° about the origin, its grid turned with it from its own corner: the plain
    // square's crosses, turned 30°, whichever method.
    let (sin, cos) = stitchcraft_core::math::sin_cos(30f64.to_radians());
    let turn = |q: Point| p(q.x() * cos - q.y() * sin, q.x() * sin + q.y() * cos);
    let square = rect((0.0, 0.0), (6.0, 6.0));
    let turned = Polygon { outline: square.outline.iter().map(|&q| turn(q)).collect(), holes: Vec::new() };
    for method in [Method::Simple, Method::Upright, Method::Half] {
        let own = Cross { method, on_canvas: false, threads: 3, ..plain() };
        let plain_runs = sewn(&region(vec![square.clone()]), &own, None, None).runs;
        let turned_runs = sewn(&region(vec![turned.clone()]), &Cross { rotation: 30.0, ..own.clone() }, None, None).runs;
        let expected: Vec<Point> = plain_runs.concat().into_iter().map(turn).collect();
        let found = turned_runs.concat();
        assert_eq!(found.len(), expected.len(), "{method:?}");
        assert!(found.iter().zip(&expected).all(|(a, b)| a.distance(*b) < 1e-9), "{method:?}: {found:?}");
    }
}

#[test]
fn req_fill_crs_011_on_the_canvas_s_grid_the_grid_turns_about_the_point_the_offset_names() {
    // A quarter turn of the canvas's square grid lays its lines where they were, and sews each simple cross
    // with its `/` diagonal under, as flipped crosses are: exactly, each needle point on the grid to the last
    // bit, and no minus zero.
    let square = region(vec![rect((0.0, 0.0), (6.0, 6.0))]);
    let turned = sewn(&square, &Cross { rotation: 90.0, ..plain() }, None, None).runs;
    let flipped = sewn(&square, &Cross { flipped: true, ..plain() }, None, None).runs;
    for runs in [&turned, &flipped] {
        let exact = |v: f64| (v / 1.5).fract() == 0.0 && v.to_bits() != (-0.0f64).to_bits();
        assert!(runs.concat().iter().all(|q| exact(q.x()) && exact(q.y())), "{runs:?}");
    }
    for cell in [(0.0, 0.0), (3.0, 0.0), (0.0, 3.0), (3.0, 3.0)] {
        let middle = p(cell.0 + 1.5, cell.1 + 1.5);
        for end in bar_ends(cell, 3.0, &["\\", "/"]) {
            assert_eq!((sewn_over(&turned, middle, end), sewn_over(&flipped, middle, end)), (4, 4), "{cell:?} {end:?}");
        }
        assert_eq!((on_top(&turned, middle), on_top(&flipped, middle)), (Some(true), Some(true)), "{cell:?}");
    }
    // Turned 30° about the point (1, 0.5), the offset's, where the grid's lines pass through (-1, 0.5): the
    // square's crosses lie on that grid turned about that point.
    let off = Cross { rotation: 30.0, offset: [1.0, 0.5], ..plain() };
    let (sin, cos) = stitchcraft_core::math::sin_cos(30f64.to_radians());
    let back = |q: Point| {
        let (x, y) = (q.x() - 1.0, q.y() - 0.5);
        p(1.0 + x * cos + y * sin, 0.5 - x * sin + y * cos)
    };
    for q in sewn(&square, &off, None, None).runs.concat() {
        // Turned back by 30° about (1, 0.5), each needle point lies on a corner or a middle of the grid
        // through (-1, 0.5).
        let r = back(q);
        let on = |v: f64| ((v / 1.5).round() * 1.5 - v).abs() < 1e-9;
        assert!(on(r.x() + 1.0) && on(r.y() - 0.5), "{q:?} back to {r:?}");
    }
    // No turn, or a whole turn either way, turns nothing.
    for rotation in [0.0, -0.0, 360.0, -360.0] {
        assert_eq!(Cross { rotation, ..plain() }.frame().turn, None, "{rotation}");
    }
    assert_eq!(Cross { rotation: 90.0, ..plain() }.frame().turn.map(|t| t.about), Some([0.0, 0.0]));
    assert_eq!(Cross { rotation: 90.0, on_canvas: false, offset: [1.0, 0.5], ..plain() }.frame().turn.map(|t| t.about), Some([0.0, 0.0]));
    assert_eq!(off.frame().turn.map(|t| t.about), Some([1.0, 0.5]));
}

#[test]
fn the_preview_shows_each_method_s_bars_and_half_crosses_diagonals() {
    let square = region(vec![rect((0.0, 0.0), (3.0, 3.0))]);
    let preview =
        |cross: &Cross| top_path(&square, cross, &mut Budget::DEFAULT.meter()).unwrap().iter().map(|s| (s.start, s.end)).collect::<Vec<_>>();
    let (falling, rising) = ((p(0.0, 0.0), p(3.0, 3.0)), (p(0.0, 3.0), p(3.0, 0.0)));
    let (across, down) = ((p(0.0, 1.5), p(3.0, 1.5)), (p(1.5, 0.0), p(1.5, 3.0)));
    assert_eq!(preview(&plain()), [falling, rising]);
    assert_eq!(preview(&Cross { method: Method::Upright, ..plain() }), [across, down]);
    assert_eq!(preview(&Cross { method: Method::Double, flipped: true, ..plain() }), [falling, across, down, rising]);
    assert_eq!(preview(&Cross { method: Method::Smyrna, ..plain() }), [falling, rising, across, down]);
    assert_eq!(preview(&Cross { method: Method::Half, ..plain() }), [falling]);
    assert_eq!(preview(&Cross { method: Method::Half, flipped: true, ..plain() }), [rising]);
    // Dense: the cell's cross, and none of the second grid, whose cells it covers a quarter.
    assert_eq!(preview(&Cross { method: Method::DenseUpright, ..plain() }), [across, down]);
    // A turned grid's bars, turned back: a quarter turn lays the plain cross's diagonals the other way.
    assert_eq!(preview(&Cross { rotation: 90.0, ..plain() }), [(p(3.0, 0.0), p(0.0, 3.0)), (p(0.0, 0.0), p(3.0, 3.0))]);
}
