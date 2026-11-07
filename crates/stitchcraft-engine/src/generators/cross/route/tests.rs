// Test code may unwrap and index.
use std::collections::BTreeSet;

use stitchcraft_core::Budget;

use super::*;

const GRID: Grid = Grid { origin: [0.0, 0.0], size: [1.0, 1.0] };

fn p(x: f64, y: f64) -> Point {
    Point::new(x, y).unwrap()
}

/// The crosses of `kind` in the cells of `rows`, each a string with `x` for a cell that sews a cross, row by
/// row from the top.
fn crosses_of(kind: Kind, rows: &[&str]) -> Crosses {
    let mut middles = Vec::new();
    for (j, row) in rows.iter().enumerate() {
        middles.extend(row.chars().enumerate().filter(|(_, c)| *c == 'x').map(|(i, _)| [2 * i as i64 + 1, 2 * j as i64 + 1]));
    }
    Crosses::new(kind, middles)
}

/// Simple crosses in the cells of `rows`.
fn cells(rows: &[&str]) -> Crosses {
    crosses_of(Kind::Simple, rows)
}

/// The corner `[i, j]` of the grid, as a place.
const fn corner(i: i64, j: i64) -> Place {
    [2 * i, 2 * j]
}

/// The middle of the cell whose top left corner is `[i, j]`, as a place.
const fn middle(i: i64, j: i64) -> Place {
    [2 * i + 1, 2 * j + 1]
}

/// How many times each half of each bar of each cross of `run` is passed, by its end and the cross's middle:
/// a stitch between a cross's middle and an end passes that half, and one between 2 ends of a bar both.
fn passes(run: &[Place], crosses: &Crosses) -> BTreeMap<(Place, Place), usize> {
    let mut passes = BTreeMap::new();
    for pair in run.windows(2) {
        let (a, b) = (pair[0], pair[1]);
        let halves = if crosses.find(a).is_some() {
            vec![(b, a)]
        } else if crosses.find(b).is_some() {
            vec![(a, b)]
        } else {
            let middle = [(a[0] + b[0]) / 2, (a[1] + b[1]) / 2];
            vec![(a, middle), (b, middle)]
        };
        for half in halves {
            *passes.entry(half).or_insert(0) += 1;
        }
    }
    passes
}

/// The runs of `crosses` on the 1 mm grid with one pass of each bar there and back, from `needle` to `end`.
fn sewn(crosses: &Crosses, needle: Option<Point>, end: Option<Point>) -> Vec<Vec<Place>> {
    runs(crosses, &GRID, 1, (needle, end), &mut Budget::DEFAULT.meter()).unwrap()
}

/// The groups of `crosses`, each by its crosses' middles.
fn grouped(crosses: &Crosses) -> Vec<Vec<Place>> {
    groups(crosses, &mut Budget::DEFAULT.meter()).unwrap()
}

#[test]
fn a_place_lies_on_a_corner_a_middle_or_the_middle_of_a_side_of_the_grid() {
    let grid = Grid { origin: [0.5, 1.0], size: [2.0, 3.0] };
    assert_eq!([corner(1, 1), middle(1, 1), [3, 2], [-1, -3]].map(|place| grid.place(place)), [p(2.5, 4.0), p(3.5, 5.5), p(3.5, 4.0), p(-0.5, -3.5)]);
}

/// How many crosses of `run` have a stitch along their `\` diagonal after the last along their `/` one: one on
/// top of it.
fn on_top(run: &[Place]) -> usize {
    // For each cell, where along the run each of its diagonals was last passed: `\` first, then `/`.
    let mut last: BTreeMap<Place, [usize; 2]> = BTreeMap::new();
    for (k, pair) in run.windows(2).enumerate() {
        let (a, b) = (pair[0], pair[1]);
        let [du, dv] = [b[0] - a[0], b[1] - a[1]];
        let cell = if a[0] % 2 != 0 {
            a
        } else if b[0] % 2 != 0 {
            b
        } else {
            [(a[0] + b[0]) / 2, (a[1] + b[1]) / 2]
        };
        last.entry(cell).or_insert([0, 0])[usize::from(du * dv < 0)] = k;
    }
    last.values().filter(|[falling, rising]| falling > rising).count()
}

#[test]
fn req_fill_crs_004_crosses_that_share_a_corner_are_one_group_and_the_rest_apart() {
    assert_eq!(grouped(&cells(&["x.x", "...", ".x."])), [vec![middle(0, 0)], vec![middle(2, 0)], vec![middle(1, 2)]]);
    // Corner to corner, either way, and side by side.
    assert_eq!(grouped(&cells(&["x.", ".x"])), [vec![middle(0, 0), middle(1, 1)]]);
    assert_eq!(grouped(&cells(&[".x", "x."])), [vec![middle(1, 0), middle(0, 1)]]);
    assert_eq!(grouped(&cells(&["xx.x"])), [vec![middle(0, 0), middle(1, 0)], vec![middle(3, 0)]]);
    // Crosses found from a cross after them in the group, above, to the left, and above and to the left.
    assert_eq!(grouped(&cells(&["x..x", ".x.x", "..x."])), [vec![middle(0, 0), middle(1, 1), middle(2, 2), middle(3, 1), middle(3, 0)]]);
    assert_eq!(grouped(&cells(&["...x", "x..x", ".xx."])), [vec![middle(3, 0), middle(3, 1), middle(2, 2), middle(1, 2), middle(0, 1)]]);
    let mut few = Budget { max_work: 5, ..Budget::DEFAULT }.meter();
    assert_eq!(groups(&cells(&["xx"]), &mut few), Err(Exhausted::Work));
}

#[test]
fn req_fill_crs_008_upright_crosses_meet_side_by_side_and_dense_ones_through_their_shared_arms() {
    // Upright crosses join at the middles of their sides: corner to corner they don't meet.
    assert_eq!(grouped(&crosses_of(Kind::Upright, &["x.", ".x"])), [vec![middle(0, 0)], vec![middle(1, 1)]]);
    assert_eq!(grouped(&crosses_of(Kind::Upright, &["xx", ".x"])), [vec![middle(0, 0), middle(1, 0), middle(1, 1)]]);
    // A dense upright cross on the corner the 2 share joins them: its arms end at the middles of their sides.
    let dense = Crosses::new(Kind::Upright, vec![middle(0, 0), middle(1, 1), corner(1, 1)]);
    assert_eq!(grouped(&dense), [vec![middle(0, 0), corner(1, 1), middle(1, 1)]]);
    // Smyrna and double crosses join at their corners, as simple ones do.
    for kind in [Kind::Smyrna, Kind::Double] {
        assert_eq!(grouped(&crosses_of(kind, &["x.", ".x"])), [vec![middle(0, 0), middle(1, 1)]], "{kind:?}");
    }
}

#[test]
fn req_fill_crs_004_a_run_sews_every_cross_of_its_group_once_and_ends_where_it_started() {
    for kind in [Kind::Simple, Kind::Upright, Kind::Double, Kind::Smyrna] {
        let blocks: &[&[&str]] = match kind {
            Kind::Upright => &[&["xxx", "xxx", "xxx"], &["xx", "x.", "xx"], &["xxx", "x.x", "xxx"]],
            _ => &[&["xxx", "xxx", "xxx"], &["x..", ".x.", "..x"], &["..x", ".x.", "x.."], &["x.x", ".x.", "x.x"], &["xx", "x.", "xx"]],
        };
        for &rows in blocks {
            let crosses = crosses_of(kind, rows);
            let runs = sewn(&crosses, None, None);
            assert_eq!(runs.len(), 1, "{kind:?} {rows:?}");
            let run = &runs[0];
            assert_eq!(run.first(), run.last(), "{kind:?} {rows:?}");
            let counts = passes(run, &crosses);
            let halves = kind.stack().len() * 2;
            assert_eq!(counts.len(), halves * crosses.middles().len(), "{kind:?} {rows:?}");
            assert!(counts.values().all(|&n| n == 2), "{kind:?} {rows:?}: {counts:?}");
        }
    }
}

#[test]
fn req_fill_crs_008_dense_upright_crosses_of_both_grids_sew_in_one_run() {
    // A 2 × 2 block of cells and the cross on the corner they share: each half of each arm twice.
    let mut middles = vec![middle(0, 0), middle(1, 0), middle(0, 1), middle(1, 1), corner(1, 1)];
    let crosses = Crosses::new(Kind::Upright, middles.clone());
    let runs = sewn(&crosses, None, None);
    assert_eq!(runs.len(), 1);
    let counts = passes(&runs[0], &crosses);
    assert_eq!((counts.len(), counts.values().all(|&n| n == 2)), (4 * 5, true), "{counts:?}");
    // The run starts at the top middle of the first cross, row by row from the top: the one on a corner
    // lies a row above the cells' middles there, so the first cross is the top left cell's.
    assert_eq!(runs[0][0], [1, 0]);
    middles.push(corner(1, 0));
    assert_eq!(sewn(&Crosses::new(Kind::Upright, middles), None, None)[0][0], [2, -1]);
}

#[test]
fn req_fill_crs_004_a_cross_reached_only_by_its_backslash_is_sewn_last_with_a_stitch_on_top() {
    // A row of crosses, and a falling diagonal, which meet only at the corners of their `\` diagonals.
    // The row's crosses join at the corners of their `/` diagonals, and none ends on top; each cross down the
    // falling diagonal but the first is reached at its top left corner, and ends on top.
    assert_eq!(on_top(&sewn(&cells(&["xxx"]), None, None)[0]), 0);
    assert_eq!(on_top(&sewn(&cells(&["..x", ".x.", "x.."]), None, None)[0]), 0);
    assert_eq!(on_top(&sewn(&cells(&["x..", ".x.", "..x"]), None, None)[0]), 2);
    // The run starts with the cross whose `/` diagonal ends at its start, or else one whose `\` diagonal
    // does: from the first cell's top right corner.
    let run = &sewn(&cells(&["x"]), None, None)[0];
    assert_eq!(run[..3], [corner(1, 0), middle(0, 0), corner(0, 0)]);
}

#[test]
fn req_fill_crs_004_at_a_corner_the_cross_below_and_left_is_sewn_before_the_one_above_and_right() {
    // 2 crosses whose `/` diagonals meet at (1, 1), from the needle there: the lower left one first.
    let run = &sewn(&cells(&[".x", "x."]), Some(p(1.0, 1.0)), None)[0];
    assert_eq!(run[..2], [corner(1, 1), middle(0, 1)]);
    let middles: Vec<Place> = run.iter().copied().filter(|s| s[0] % 2 != 0).collect();
    assert_eq!(middles, [middle(0, 1), middle(0, 1), middle(1, 0), middle(1, 0)]);
    // And of 2 whose `\` diagonals meet there, the lower right one first.
    let run = &sewn(&cells(&["x.", ".x"]), Some(p(1.0, 1.0)), None)[0];
    assert_eq!(run[..2], [corner(1, 1), corner(2, 2)]);
    // Of 2 upright crosses side by side, from the middle of the side they share: the right one first, whose
    // left middle it is, a bad join of both; and of 2 one above the other, the lower one first.
    let side = &sewn(&crosses_of(Kind::Upright, &["xx"]), Some(p(1.0, 0.5)), None)[0];
    assert_eq!(side[..2], [[2, 1], [4, 1]]);
    let above = &sewn(&crosses_of(Kind::Upright, &["x", "x"]), Some(p(0.5, 1.0)), None)[0];
    assert_eq!(above[..2], [[1, 2], middle(0, 1)]);
}

#[test]
fn req_fill_crs_004_groups_go_nearest_the_needle_first_and_nearest_the_end_last() {
    // 3 crosses apart in a row, at x = 0, 3 and 6.
    let cells = cells(&["x..x..x"]);
    let starts = |needle: Option<Point>, end: Option<Point>| sewn(&cells, needle, end).iter().map(|run| run[0]).collect::<Vec<Place>>();
    // From the needle at the right, right to left, each from its corner nearest where the last ended.
    assert_eq!(starts(Some(p(9.0, 0.0)), None), [corner(7, 0), corner(4, 0), corner(1, 0)]);
    // With the end at the right, the right cross last, from its corner nearest the end.
    assert_eq!(starts(Some(p(-1.0, 0.0)), Some(p(9.0, 1.0))), [corner(0, 0), corner(3, 0), corner(7, 1)]);
    // With no needle, the first cross from its top right corner, and on from there.
    assert_eq!(starts(None, None), [corner(1, 0), corner(3, 0), corner(6, 0)]);
    // A needle as near 2 corners starts at the first of them, clockwise from the top left: (0, 0), not (1, 0).
    assert_eq!(starts(Some(p(0.5, -1.0)), None)[0], corner(0, 0));
    // With no needle and the end at the left, the left cross last.
    assert_eq!(starts(None, Some(p(0.0, 0.0))), [corner(4, 0), corner(6, 0), corner(0, 0)]);
    // The group of the needle and the end both, with others, goes last, from the end.
    assert_eq!(starts(Some(p(-1.0, 0.0)), Some(p(0.0, 1.0))), [corner(3, 0), corner(6, 0), corner(0, 1)]);
}

#[test]
fn req_fill_crs_004_one_group_with_a_needle_and_an_end_travels_through_the_crosses_first() {
    // A row of 4 crosses, from the needle at its left to the end at its right: from (0, 0) through the
    // middles of the fewest crosses to (4, 1), from corner to corner of each, then the loops from there.
    let cells = cells(&["xxxx"]);
    let runs = sewn(&cells, Some(p(-1.0, 0.0)), Some(p(5.0, 1.0)));
    assert_eq!(runs.len(), 1);
    let run = &runs[0];
    let travel = [corner(0, 0), middle(0, 0), corner(1, 0), middle(1, 0), corner(2, 0), middle(2, 0), corner(3, 0), middle(3, 0), corner(4, 1)];
    assert_eq!(run[..travel.len()], travel);
    assert_eq!(run.len(), travel.len() + 6 * 4);
    assert_eq!(run.last(), Some(&corner(4, 1)));
    // From the needle at its right to the end at its left, back along the top from (4, 1).
    let back = sewn(&cells, Some(p(5.0, 1.0)), Some(p(-1.0, 0.0)));
    let way = [corner(4, 1), middle(3, 0), corner(3, 0), middle(2, 0), corner(2, 0), middle(1, 0), corner(1, 0), middle(0, 0), corner(0, 0)];
    assert_eq!(back[0][..way.len()], way);
    // Up through the cross above and to the right of the needle's corner, to the end's corner.
    let rising = sewn(&self::cells(&[".x", "x."]), Some(p(-0.5, 2.5)), Some(p(2.5, -0.5)));
    let up = [corner(0, 2), middle(0, 1), corner(1, 1), middle(1, 0), corner(2, 0)];
    assert_eq!(rising[0][..up.len()], up);
    // The same corner for both: no travel.
    let runs = sewn(&cells, Some(p(5.0, 1.0)), Some(p(5.0, 1.0)));
    assert_eq!(runs[0].len(), 1 + 6 * 4);
    // Upright crosses travel from the middle of a side to another through their middles.
    let row = crosses_of(Kind::Upright, &["xxx"]);
    let across = sewn(&row, Some(p(-1.0, 0.5)), Some(p(4.0, 0.5)));
    assert_eq!(across[0][..7], [[0, 1], middle(0, 0), [2, 1], middle(1, 0), [4, 1], middle(2, 0), [6, 1]]);
    // Travel stops where it reaches the end, and its steps are charged.
    let mut few = Budget { max_work: 40, ..Budget::DEFAULT }.meter();
    assert_eq!(travel_of(&cells, &mut few), Err(Exhausted::Work));
}

fn travel_of(cells: &Crosses, meter: &mut Meter) -> Result<Vec<Place>, Exhausted> {
    let mut long = cells.middles().to_vec();
    long.extend((4..40).map(|i| middle(i, 0)));
    travel(corner(0, 0), corner(40, 1), &Crosses::new(Kind::Simple, long), meter)
}

#[test]
fn every_kind_s_run_sews_each_cross_from_a_join_of_its_own() {
    // Each place a run passes is a join or a middle of one of its crosses: a run never leaves its crosses.
    for kind in [Kind::Simple, Kind::Upright, Kind::Double, Kind::Smyrna] {
        let crosses = crosses_of(kind, &["xx.", ".xx", "x.x"]);
        let places: BTreeSet<Place> =
            crosses.middles().iter().flat_map(|&m| kind.stack().iter().flat_map(move |bar| bar.ends().map(|o| add(m, o))).chain([m])).collect();
        for run in sewn(&crosses, Some(p(3.5, 0.0)), Some(p(-1.0, 3.0))) {
            assert!(run.iter().all(|spot| places.contains(spot)), "{kind:?}: {run:?}");
        }
    }
}
