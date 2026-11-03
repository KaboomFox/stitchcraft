// Test code may unwrap and index.
use stitchcraft_core::Budget;

use super::*;

const GRID: Grid = Grid { origin: [0.0, 0.0], size: [1.0, 1.0] };

fn p(x: f64, y: f64) -> Point {
    Point::new(x, y).unwrap()
}

/// The cells of `rows`, each a string with `x` for a cell that sews, row by row from the top.
fn cells(rows: &[&str]) -> Cells {
    let mut crossed = Vec::new();
    for (j, row) in rows.iter().enumerate() {
        crossed.extend(row.chars().enumerate().filter(|(_, c)| *c == 'x').map(|(i, _)| [i as i64, j as i64]));
    }
    Cells::of(crossed)
}

/// The points of `ring`, as pairs.
fn pairs(ring: &[Point]) -> Vec<(f64, f64)> {
    ring.iter().map(|q| (q.x(), q.y())).collect()
}

fn outlined(rows: &[&str]) -> Polygon {
    outline(cells(rows).crossed(), &GRID, &mut Budget::DEFAULT.meter()).unwrap()
}

const HALF: Half = Half { flipped: false, passes: 1, longest: 11.0, min_stitch: 0.1 };

#[test]
fn req_fill_crs_010_cells_that_share_a_side_or_a_corner_are_one_piece() {
    let found = |rows: &[&str]| pieces(&cells(rows), &mut Budget::DEFAULT.meter()).unwrap();
    assert_eq!(found(&["x.x", "...", ".x."]), [vec![[0, 0]], vec![[2, 0]], vec![[1, 2]]]);
    assert_eq!(found(&["x.", ".x"]), [vec![[0, 0], [1, 1]]]);
    assert_eq!(found(&[".x", "x."]), [vec![[1, 0], [0, 1]]]);
    // A piece's cells go row by row from the top, whichever way they were reached.
    assert_eq!(found(&["..x", "x.x", "xx."]), [vec![[2, 0], [0, 1], [2, 1], [0, 2], [1, 2]]]);
    let mut few = Budget { max_work: 5, ..Budget::DEFAULT }.meter();
    assert_eq!(pieces(&cells(&["xx"]), &mut few), Err(Exhausted::Work));
}

#[test]
fn req_fill_crs_010_a_piece_s_outline_runs_round_its_cells_with_the_piece_on_its_left() {
    // A 2 × 2 block: from its top left corner down its left side, along its bottom, up its right side and
    // back, counterclockwise as drawn, as a part's outline runs, its corners alone.
    let block = outlined(&["xx", "xx"]);
    assert_eq!(pairs(&block.outline), [(0.0, 0.0), (0.0, 2.0), (2.0, 2.0), (2.0, 0.0), (0.0, 0.0)]);
    assert!(block.holes.is_empty());
    // A ring of cells round an empty one: the hole runs the other way, from its own top left corner.
    let ring = outlined(&["xxx", "x.x", "xxx"]);
    assert_eq!(pairs(&ring.outline), [(0.0, 0.0), (0.0, 3.0), (3.0, 3.0), (3.0, 0.0), (0.0, 0.0)]);
    assert_eq!(ring.holes.iter().map(|hole| pairs(hole)).collect::<Vec<_>>(), [vec![(1.0, 1.0), (2.0, 1.0), (2.0, 2.0), (1.0, 2.0), (1.0, 1.0)]]);
}

#[test]
fn req_fill_crs_010_cells_that_meet_at_a_corner_are_joined_through_it() {
    // Corner to corner either way: one ring, which passes the corner twice, from the one cell to the other.
    let falling = outlined(&["x.", ".x"]);
    assert_eq!(pairs(&falling.outline), [(0.0, 0.0), (0.0, 1.0), (1.0, 1.0), (1.0, 2.0), (2.0, 2.0), (2.0, 1.0), (1.0, 1.0), (1.0, 0.0), (0.0, 0.0)]);
    let rising = outlined(&[".x", "x."]);
    assert_eq!(pairs(&rising.outline), [(1.0, 0.0), (1.0, 1.0), (0.0, 1.0), (0.0, 2.0), (1.0, 2.0), (1.0, 1.0), (2.0, 1.0), (2.0, 0.0), (1.0, 0.0)]);
    // A hole that meets the outside at a corner: the outline and the hole each pass it once.
    let notched = outlined(&[".xx", "x.x", "xxx"]);
    assert_eq!(pairs(&notched.outline), [(1.0, 0.0), (1.0, 1.0), (0.0, 1.0), (0.0, 3.0), (3.0, 3.0), (3.0, 0.0), (1.0, 0.0)]);
    assert_eq!(notched.holes.iter().map(|hole| pairs(hole)).collect::<Vec<_>>(), [vec![(1.0, 1.0), (2.0, 1.0), (2.0, 2.0), (1.0, 2.0), (1.0, 1.0)]]);
}

#[test]
fn req_fill_crs_010_diagonals_that_run_on_from_cell_to_cell_are_one_row() {
    let block = cells(&["xx", "xx"]);
    let rows_of =
        |half: &Half| rows(block.crossed(), &GRID, half, &mut Budget::DEFAULT.meter()).unwrap().iter().map(|row| pairs(row)).collect::<Vec<_>>();
    // `\` from each row's top end, the rows by their first cells, row by row from the top.
    assert_eq!(rows_of(&HALF), [vec![(0.0, 0.0), (1.0, 1.0), (2.0, 2.0)], vec![(1.0, 0.0), (2.0, 1.0)], vec![(0.0, 1.0), (1.0, 2.0)]]);
    // `/` from each row's bottom end.
    assert_eq!(
        rows_of(&Half { flipped: true, ..HALF }),
        [vec![(0.0, 1.0), (1.0, 0.0)], vec![(0.0, 2.0), (1.0, 1.0), (2.0, 0.0)], vec![(1.0, 2.0), (2.0, 1.0)]]
    );
    // A diagonal of 1.41 mm split at 1 mm into 2 equal stitches, and at its own length not.
    let split = rows_of(&Half { longest: 1.0, ..HALF });
    assert_eq!(split[1], [(1.0, 0.0), (1.5, 0.5), (2.0, 1.0)]);
    assert_eq!(rows_of(&Half { longest: 2f64.sqrt(), ..HALF })[1].len(), 2);
}

/// A point to a thousandth of a millimetre.
type Key = (i64, i64);

/// The stitches of `run`, each between 2 points, by their ends in order, and how many times each is sewn.
#[allow(clippy::cast_possible_truncation)]
fn stitch_counts(run: &[Point]) -> std::collections::BTreeMap<(Key, Key), usize> {
    let key = |q: &Point| ((q.x() * 1000.0).round() as i64, (q.y() * 1000.0).round() as i64);
    let mut counts = std::collections::BTreeMap::new();
    for pair in run.windows(2) {
        let (a, b) = (key(&pair[0]), key(&pair[1]));
        if a != b {
            *counts.entry((a.min(b), a.max(b))).or_insert(0) += 1;
        }
    }
    counts
}

#[test]
fn req_fill_crs_010_each_stitch_of_a_row_is_sewn_an_odd_number_of_times_and_the_travel_is_not() {
    // A 2 × 2 block, 3 passes: each diagonal's stitch 3 times, and the travel along the outline as it comes.
    let block = cells(&["xx", "xx"]);
    let half = Half { passes: 3, ..HALF };
    let runs = runs(&block, &GRID, &half, (None, None), &mut Budget::DEFAULT.meter(), &mut Vec::new()).unwrap();
    assert_eq!(runs.len(), 1);
    let counts = stitch_counts(&runs[0]);
    let diagonal = |a: (i64, i64), b: (i64, i64)| counts.get(&(a, b)).copied();
    assert_eq!(
        [diagonal((0, 0), (1000, 1000)), diagonal((1000, 1000), (2000, 2000)), diagonal((1000, 0), (2000, 1000)), diagonal((0, 1000), (1000, 2000))],
        [Some(3); 4]
    );
    // Every other stitch lies along a side of the block, travel, which no stitch is sewn back and forth over:
    // the needle passes a side at most once each way.
    let on_side = |((ax, ay), (bx, by)): ((i64, i64), (i64, i64))| (ax == bx && (ax == 0 || ax == 2000)) || (ay == by && (ay == 0 || ay == 2000));
    assert!(counts.iter().filter(|&(&stitch, _)| on_side(stitch)).all(|(_, &n)| n <= 2), "{counts:?}");
    assert_eq!(counts.len(), 4 + counts.keys().filter(|&&stitch| on_side(stitch)).count(), "{counts:?}");
}

#[test]
fn req_fill_crs_010_pieces_apart_are_sewn_nearest_the_needle_first_each_ending_nearest_the_next() {
    // 2 cells apart, the needle at the right: the right one first, ending at its point nearest the left one,
    // then the left one, ending nearest the end at its top left.
    let apart = cells(&["x..x"]);
    let ends = (Some(p(5.0, 0.5)), Some(p(-1.0, 0.0)));
    let runs = runs(&apart, &GRID, &HALF, ends, &mut Budget::DEFAULT.meter(), &mut Vec::new()).unwrap();
    assert_eq!(runs.len(), 2);
    assert!(runs[0].iter().all(|q| q.x() >= 3.0) && runs[1].iter().all(|q| q.x() <= 1.0), "{runs:?}");
    assert_eq!(runs[0].last().map(|q| q.x()), Some(3.0));
    assert_eq!(runs[1].last(), Some(&p(0.0, 0.0)));
    // Without a needle, the piece farthest left first.
    let first = runs_of(&apart, (None, None));
    assert!(first[0].iter().all(|q| q.x() <= 1.0), "{first:?}");
}

fn runs_of(cells: &Cells, ends: (Option<Point>, Option<Point>)) -> Vec<Vec<Point>> {
    runs(cells, &GRID, &HALF, ends, &mut Budget::DEFAULT.meter(), &mut Vec::new()).unwrap()
}
