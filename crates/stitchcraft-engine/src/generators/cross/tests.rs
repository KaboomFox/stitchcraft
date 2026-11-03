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
        coverage: 50.0,
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
    let flipped = Cross { flipped: true, ..cross.clone() };
    assert_eq!(cells(&flipped), cells(&cross));
    assert_eq!(cells(&cross).len(), 18);
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
