//! The runs that sew a part's crosses (`docs/src/design/algorithms/cross-stitch.md` › Sewing the crosses).
//!
//! **A cross is a loop.** Each cross is sewn as a loop from one of its corners back to it: its `\` diagonal
//! first, its `/` diagonal over it, and the needle passing through its middle in between, where the second
//! diagonal covers it. From a corner of its `/` diagonal, the loop goes to the middle along that diagonal,
//! which the loop sews last, so nothing of the loop shows on top. From a corner of its `\` diagonal it ends
//! with a stitch from the middle back to its start, on top of the `/` diagonal. Each half of each diagonal
//! takes `threads` passes, the needle going back and forth along it. A flipped cross is a plain one turned a
//! quarter turn, which the caller sees to (`super::Frame`).
//!
//! **Crosses that meet sew in one run.** Crosses that share a corner, side by side or corner to corner, form
//! a group, and each group is one run, from a corner back to it. The run starts with a cross at its corner,
//! and goes round its loop. At each corner it reaches, before going on, it sews the loop of each cross not
//! sewn yet whose `/` diagonal ends there: the cross below and to the left first, then the one above and to
//! the right, each loop sewn the same way as it goes. Where it has been round and crosses are left, it takes
//! the first corner along it where a cross not sewn yet has its `\` diagonal's end, the cross below and to
//! the right first, sews that loop there, and goes on as before. So few loops end on top of their `/`
//! diagonal, and every cross of the group is sewn, since each corner of a cross sewn lies on the run.
//!
//! **Groups in order.** The group with the corner nearest the end is sewn last, from that corner, so the
//! part ends there. The others go first, each the group with the corner nearest the needle, from that
//! corner, and with no needle the group of the first cross, row by row from the top, from that cross's top
//! right corner. A part of one group with both a needle and an end first travels from the corner nearest
//! the needle to the corner nearest the end, from corner to corner through the middles of the fewest
//! crosses, and sews from there: the crosses sewn later cover the travel.
//!
//! **Cost.** Every step charges `meter`: each place the run passes, each cross looked for at a corner, and
//! each corner the travel reaches.

use std::collections::{BTreeMap, VecDeque};

use stitchcraft_core::{Exhausted, Meter, Point};

use super::grid::{Cells, Grid};

/// A place a run passes: a corner of the grid, or the middle of the cell whose top left corner is given.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Spot {
    /// The corner where the grid's lines `[i, j]` meet.
    Corner([i64; 2]),
    /// The middle of the cell whose top left corner is `[i, j]`.
    Middle([i64; 2]),
}

impl Spot {
    /// Where it lies on `grid`.
    pub(crate) fn on(self, grid: &Grid) -> Point {
        match self {
            Spot::Corner(corner) => grid.corner(corner),
            Spot::Middle(cell) => grid.middle(cell),
        }
    }
}

/// The runs that sew the crosses of `cells` on `grid`, each a list of places from its first, with
/// `repeats` passes on each half of each diagonal: one run for each group of crosses that share corners,
/// in the order the module's docs give, from the `needle` and to end nearest `end` where they are known.
pub(crate) fn runs(
    cells: &Cells,
    grid: &Grid,
    repeats: u32,
    (needle, end): (Option<Point>, Option<Point>),
    meter: &mut Meter,
) -> Result<Vec<Vec<Spot>>, Exhausted> {
    let groups = groups(cells, meter)?;
    let mut pending = vec![true; cells.crossed().len()];
    let all: Vec<usize> = (0..groups.len()).collect();
    let last = end.map(|end| nearest(&groups, &all, grid, end, meter)).transpose()?.flatten();
    let mut left: Vec<usize> = all.into_iter().filter(|&group| last.is_none_or(|(sewn_last, _)| sewn_last != group)).collect();
    let mut runs = Vec::with_capacity(groups.len());
    let mut at = needle;
    while !left.is_empty() {
        let next = match at {
            Some(p) => nearest(&groups, &left, grid, p, meter)?,
            None => left.first().and_then(|&group| Some((group, top_right(*groups.get(group)?.first()?)))),
        };
        let Some((group, start)) = next else { break };
        left.retain(|&other| other != group);
        runs.push(run(start, cells, &mut pending, repeats, meter)?);
        at = Some(grid.corner(start));
    }
    if let Some((group, start)) = last {
        let mut sewn = vec![Spot::Corner(start)];
        if let (Some(needle), [_]) = (needle, groups.as_slice()) {
            let from = nearest(&groups, &[group], grid, needle, meter)?.map_or(start, |(_, corner)| corner);
            sewn = travel(from, start, cells, meter)?;
        }
        sewn.extend(run(start, cells, &mut pending, repeats, meter)?.into_iter().skip(1));
        runs.push(sewn);
    }
    Ok(runs)
}

/// The 8 cells round a cell, by how far they lie across and down from it: the row above, the 2 beside it and
/// the row below, each from the left.
const AROUND: [[i64; 2]; 8] = [[-1, -1], [0, -1], [1, -1], [-1, 0], [1, 0], [-1, 1], [0, 1], [1, 1]];

/// The groups of crosses of `cells` that share corners, each its crosses in the order they were reached
/// from its first, the groups in the order of their first crosses, row by row from the top. One unit of
/// `meter` for each cross, and one for each of its neighbours looked for.
fn groups(cells: &Cells, meter: &mut Meter) -> Result<Vec<Vec<[i64; 2]>>, Exhausted> {
    let crossed = cells.crossed();
    let mut seen = vec![false; crossed.len()];
    let mut groups = Vec::new();
    for (k, &cell) in crossed.iter().enumerate() {
        meter.charge(1)?;
        if seen.get(k).copied().unwrap_or(true) {
            continue;
        }
        if let Some(mark) = seen.get_mut(k) {
            *mark = true;
        }
        let mut group = vec![cell];
        let mut next = 0;
        while let Some(&[i, j]) = group.get(next) {
            next += 1;
            for [di, dj] in AROUND {
                meter.charge(1)?;
                let neighbour = [i.saturating_add(di), j.saturating_add(dj)];
                if let Some(mark) = cells.find(neighbour).and_then(|n| seen.get_mut(n))
                    && !*mark
                {
                    *mark = true;
                    group.push(neighbour);
                }
            }
        }
        groups.push(group);
    }
    Ok(groups)
}

/// Of the groups `among`, the one with the corner nearest `p` on `grid`, and that corner: by its place in
/// `among`, and the corner of its crosses in their order, each cross's corners clockwise from its top left,
/// the first of those equally near. One unit of `meter` for each cross.
fn nearest(groups: &[Vec<[i64; 2]>], among: &[usize], grid: &Grid, p: Point, meter: &mut Meter) -> Result<Option<(usize, [i64; 2])>, Exhausted> {
    let mut best: Option<(f64, usize, [i64; 2])> = None;
    for &group in among {
        for &cell in groups.get(group).map_or(&[][..], Vec::as_slice) {
            meter.charge(1)?;
            for corner in corners(cell) {
                let d = grid.corner(corner).distance(p);
                if best.is_none_or(|(least, ..)| d < least) {
                    best = Some((d, group, corner));
                }
            }
        }
    }
    Ok(best.map(|(_, group, corner)| (group, corner)))
}

/// The corners of the cell whose top left corner is `[i, j]`, clockwise from it: top left, top right,
/// bottom right, bottom left.
fn corners([i, j]: [i64; 2]) -> [[i64; 2]; 4] {
    [[i, j], [i + 1, j], [i + 1, j + 1], [i, j + 1]]
}

/// The top right corner of the cell whose top left corner is `[i, j]`: a corner of its `/` diagonal.
fn top_right([i, j]: [i64; 2]) -> [i64; 2] {
    [i + 1, j]
}

/// The cells whose `/` diagonal ends at `corner`, in the order they are sewn: the one below and to the left,
/// whose top right corner it is, then the one above and to the right, whose bottom left corner it is.
fn slashed([i, j]: [i64; 2]) -> [[i64; 2]; 2] {
    [[i - 1, j], [i, j - 1]]
}

/// The cells whose `\` diagonal ends at `corner`, in the order they are sewn: the one below and to the
/// right, whose top left corner it is, then the one above and to the left, whose bottom right corner it is.
fn backslashed([i, j]: [i64; 2]) -> [[i64; 2]; 2] {
    [[i, j], [i - 1, j - 1]]
}

/// The places of the loop that sews the cross in `cell` from its corner `start` back to it, after `start`,
/// with `repeats` passes on each half of each diagonal, as the module's docs give it.
fn cross_loop(cell: [i64; 2], start: [i64; 2], repeats: u32) -> Vec<Spot> {
    let [top_left, top_right, bottom_right, bottom_left] = corners(cell).map(Spot::Corner);
    let middle = Spot::Middle(cell);
    let back_and_forth = |a: Spot, b: Spot, times: u32| (0..times).flat_map(move |_| [a, b]);
    let corner = Spot::Corner(start);
    let mut spots = Vec::with_capacity(4 * repeats as usize + 2);
    if corner == top_right || corner == bottom_left {
        // From a corner of the `/` diagonal: in to the middle along it, the `\` diagonal from the corner on the
        // start's side, back to the middle, and the `/` diagonal from its far end.
        let (near, far, other) = if corner == top_right { (top_left, bottom_right, bottom_left) } else { (bottom_right, top_left, top_right) };
        spots.push(middle);
        spots.extend(back_and_forth(near, far, repeats));
        spots.push(middle);
        spots.extend(back_and_forth(other, corner, repeats));
    } else {
        // From a corner of the `\` diagonal: along it to its far end and back, ending at the far end, to the
        // middle, the `/` diagonal from its top, and back through the middle to the start.
        let far = if corner == top_left { bottom_right } else { top_left };
        spots.extend(back_and_forth(far, corner, repeats.saturating_sub(1)));
        spots.extend([far, middle]);
        spots.extend(back_and_forth(top_right, bottom_left, repeats));
        spots.extend([middle, corner]);
    }
    spots
}

/// A run being built: its places linked in sewing order, so that a loop goes in after any of them at once.
struct Linked {
    spots: Vec<Spot>,
    next: Vec<Option<usize>>,
}

impl Linked {
    /// The place after `at`.
    fn after(&self, at: usize) -> Option<usize> {
        self.next.get(at).copied().flatten()
    }

    /// Puts `loop_` in after the place `at`.
    fn insert(&mut self, at: usize, loop_: Vec<Spot>) {
        let after = self.after(at);
        let mut previous = at;
        for spot in loop_ {
            let new = self.spots.len();
            self.spots.push(spot);
            self.next.push(after);
            if let Some(link) = self.next.get_mut(previous) {
                *link = Some(new);
            }
            previous = new;
        }
    }

    /// The places from the first, in sewing order.
    fn in_order(&self) -> Vec<Spot> {
        let mut spots = Vec::with_capacity(self.spots.len());
        let mut at = Some(0);
        while let Some(k) = at {
            spots.extend(self.spots.get(k).copied());
            at = self.after(k);
        }
        spots
    }
}

/// The run that sews, from the corner `start` back to it, the crosses of `cells` not sewn yet that share
/// corners with a cross at `start`, as the module's docs give it, `pending` saying which are not sewn yet.
fn run(start: [i64; 2], cells: &Cells, pending: &mut [bool], repeats: u32, meter: &mut Meter) -> Result<Vec<Spot>, Exhausted> {
    let mut linked = Linked { spots: vec![Spot::Corner(start)], next: vec![None] };
    let mut take = |cell: [i64; 2]| cells.find(cell).and_then(|k| pending.get_mut(k)).is_some_and(|left| std::mem::replace(left, false));
    if let Some(cell) = slashed(start).into_iter().chain(backslashed(start)).find(|&cell| take(cell)) {
        linked.insert(0, cross_loop(cell, start, repeats));
    }
    walk(&mut linked, (0, None), &mut take, repeats, meter)?;
    let mut from = linked.after(0);
    while let Some((k, corner, cell)) = first_backslashed(&linked, from, &mut take, meter)? {
        let after = linked.after(k);
        linked.insert(k, cross_loop(cell, corner, repeats));
        walk(&mut linked, (k, after), &mut take, repeats, meter)?;
        from = Some(k);
    }
    Ok(linked.in_order())
}

/// The first corner along `linked` from the place `from` where `take` gives a cross whose `\` diagonal ends
/// there, the place, the corner and the cross's cell. One unit of `meter` for each place passed.
fn first_backslashed(
    linked: &Linked,
    from: Option<usize>,
    take: &mut impl FnMut([i64; 2]) -> bool,
    meter: &mut Meter,
) -> Result<Option<(usize, [i64; 2], [i64; 2])>, Exhausted> {
    let mut at = from;
    while let Some(k) = at {
        meter.charge(1)?;
        if let Some(Spot::Corner(corner)) = linked.spots.get(k).copied()
            && let Some(cell) = backslashed(corner).into_iter().find(|&cell| take(cell))
        {
            return Ok(Some((k, corner, cell)));
        }
        at = linked.after(k);
    }
    Ok(None)
}

/// Goes along `linked` from the place after `from` up to `until`, or to its end: at each corner, the loop of
/// each cross `take` gives whose `/` diagonal ends there goes in after it, the first to be sewn first, and the
/// walk goes on along the loops.
fn walk(
    linked: &mut Linked,
    (from, until): (usize, Option<usize>),
    take: &mut impl FnMut([i64; 2]) -> bool,
    repeats: u32,
    meter: &mut Meter,
) -> Result<(), Exhausted> {
    let mut at = linked.after(from);
    while let Some(k) = at.filter(|&k| Some(k) != until) {
        meter.charge(1)?;
        if let Some(Spot::Corner(corner)) = linked.spots.get(k).copied() {
            for cell in slashed(corner).into_iter().rev() {
                if take(cell) {
                    linked.insert(k, cross_loop(cell, corner, repeats));
                }
            }
        }
        at = linked.after(k);
    }
    Ok(())
}

/// The way from the corner `from` to the corner `to` through the crosses of `cells`, its places from `from`:
/// from corner to corner of a cross through its middle, through the fewest crosses, each corner reached
/// first from the corner before it in the order the way is searched. One unit of `meter` for each corner
/// reached and each cross looked at.
fn travel(from: [i64; 2], to: [i64; 2], cells: &Cells, meter: &mut Meter) -> Result<Vec<Spot>, Exhausted> {
    let mut came: BTreeMap<[i64; 2], Option<([i64; 2], [i64; 2])>> = BTreeMap::from([(from, None)]);
    let mut queue = VecDeque::from([from]);
    while let Some(corner) = queue.pop_front().filter(|_| !came.contains_key(&to)) {
        let [i, j] = corner;
        for cell in [[i - 1, j - 1], [i, j - 1], [i - 1, j], [i, j]] {
            meter.charge(1)?;
            if cells.find(cell).is_none() {
                continue;
            }
            for next in corners(cell) {
                if !came.contains_key(&next) {
                    came.insert(next, Some((corner, cell)));
                    queue.push_back(next);
                }
            }
        }
    }
    let mut way = vec![Spot::Corner(to)];
    let mut at = to;
    while let Some(&Some((before, cell))) = came.get(&at) {
        way.extend([Spot::Middle(cell), Spot::Corner(before)]);
        at = before;
    }
    way.reverse();
    Ok(way)
}

#[cfg(test)]
mod tests {
    use stitchcraft_core::Budget;

    use super::*;

    const GRID: Grid = Grid { origin: [0.0, 0.0], size: [1.0, 1.0] };

    /// The cells of `rows`, each a string with `x` for a cell that sews a cross, row by row from the top.
    fn cells(rows: &[&str]) -> Cells {
        let mut crossed = Vec::new();
        for (j, row) in rows.iter().enumerate() {
            crossed.extend(row.chars().enumerate().filter(|(_, c)| *c == 'x').map(|(i, _)| [i as i64, j as i64]));
        }
        Cells::of(crossed)
    }

    /// How many times each half of each diagonal of each cross of `run` is passed, by its corner and cell.
    fn passes(run: &[Spot]) -> BTreeMap<([i64; 2], [i64; 2]), usize> {
        let mut passes = BTreeMap::new();
        for pair in run.windows(2) {
            match (pair[0], pair[1]) {
                (Spot::Corner(c), Spot::Middle(m)) | (Spot::Middle(m), Spot::Corner(c)) => *passes.entry((c, m)).or_insert(0) += 1,
                (Spot::Corner(a), Spot::Corner(b)) => {
                    let cell = [a[0].min(b[0]), a[1].min(b[1])];
                    *passes.entry((a, cell)).or_insert(0) += 1;
                    *passes.entry((b, cell)).or_insert(0) += 1;
                }
                other => panic!("{other:?}"),
            }
        }
        passes
    }

    #[test]
    fn req_fill_crs_003_a_cross_from_a_corner_of_its_slash_sews_its_backslash_then_its_slash_and_shows_no_travel() {
        let tr = Spot::Corner([1, 0]);
        let (tl, br, bl, m) = (Spot::Corner([0, 0]), Spot::Corner([1, 1]), Spot::Corner([0, 1]), Spot::Middle([0, 0]));
        assert_eq!(cross_loop([0, 0], [1, 0], 1), [m, tl, br, m, bl, tr]);
        assert_eq!(cross_loop([0, 0], [1, 0], 2), [m, tl, br, tl, br, m, bl, tr, bl, tr]);
        assert_eq!(cross_loop([0, 0], [0, 1], 1), [m, br, tl, m, tr, bl]);
        // From a corner of its `\` diagonal, the last stitch, from the middle to the start, lies on its `/`.
        assert_eq!(cross_loop([0, 0], [0, 0], 1), [br, m, tr, bl, m, tl]);
        assert_eq!(cross_loop([0, 0], [0, 0], 2), [br, tl, br, m, tr, bl, tr, bl, m, tl]);
        assert_eq!(cross_loop([0, 0], [1, 1], 1), [tl, m, tr, bl, m, br]);
        // Each half of each diagonal is passed twice for each repeat, whatever the start.
        for start in [[0, 0], [1, 0], [1, 1], [0, 1]] {
            for repeats in [1, 2, 3] {
                let mut run = vec![Spot::Corner(start)];
                run.extend(cross_loop([0, 0], start, repeats));
                let counts = passes(&run);
                assert_eq!(counts.len(), 4, "{start:?} {repeats}");
                assert!(counts.values().all(|&n| n == 2 * repeats as usize), "{start:?} {repeats}: {counts:?}");
                assert_eq!(run.last(), Some(&Spot::Corner(start)));
            }
        }
    }

    fn p(x: f64, y: f64) -> Point {
        Point::new(x, y).unwrap()
    }

    /// The runs of `cells` on the 1 mm grid with one pass of each diagonal there and back, from `needle` to
    /// `end`.
    fn sewn(cells: &Cells, needle: Option<Point>, end: Option<Point>) -> Vec<Vec<Spot>> {
        runs(cells, &GRID, 1, (needle, end), &mut Budget::DEFAULT.meter()).unwrap()
    }

    /// How many crosses of `run` have a stitch along their `\` diagonal after the last along their `/` one:
    /// one on top of it.
    fn on_top(run: &[Spot]) -> usize {
        // For each cell, where along the run each of its diagonals was last passed: `\` first, then `/`.
        let mut last: BTreeMap<[i64; 2], [usize; 2]> = BTreeMap::new();
        for (k, pair) in run.windows(2).enumerate() {
            let (cell, falling) = match (pair[0], pair[1]) {
                (Spot::Corner(c), Spot::Middle(m)) | (Spot::Middle(m), Spot::Corner(c)) => (m, c == m || c == [m[0] + 1, m[1] + 1]),
                (Spot::Corner(a), Spot::Corner(b)) => ([a[0].min(b[0]), a[1].min(b[1])], (b[0] - a[0]) * (b[1] - a[1]) > 0),
                other => panic!("{other:?}"),
            };
            last.entry(cell).or_insert([0, 0])[usize::from(!falling)] = k;
        }
        last.values().filter(|[falling, rising]| falling > rising).count()
    }

    #[test]
    fn req_fill_crs_004_crosses_that_share_a_corner_are_one_group_and_the_rest_apart() {
        let groups = |rows: &[&str]| groups(&cells(rows), &mut Budget::DEFAULT.meter()).unwrap();
        assert_eq!(groups(&["x.x", "...", ".x."]), [vec![[0, 0]], vec![[2, 0]], vec![[1, 2]]]);
        // Corner to corner, either way, and side by side.
        assert_eq!(groups(&["x.", ".x"]), [vec![[0, 0], [1, 1]]]);
        assert_eq!(groups(&[".x", "x."]), [vec![[1, 0], [0, 1]]]);
        assert_eq!(groups(&["xx.x"]), [vec![[0, 0], [1, 0]], vec![[3, 0]]]);
        let mut few = Budget { max_work: 5, ..Budget::DEFAULT }.meter();
        assert_eq!(super::groups(&cells(&["xx"]), &mut few), Err(Exhausted::Work));
    }

    #[test]
    fn req_fill_crs_004_a_run_sews_every_cross_of_its_group_once_and_ends_where_it_started() {
        for rows in [&["xxx", "xxx", "xxx"][..], &["x..", ".x.", "..x"], &["..x", ".x.", "x.."], &["x.x", ".x.", "x.x"], &["xx", "x.", "xx"]] {
            let cells = cells(rows);
            let runs = sewn(&cells, None, None);
            assert_eq!(runs.len(), 1, "{rows:?}");
            let run = &runs[0];
            assert_eq!(run.len(), 1 + 6 * cells.crossed().len(), "{rows:?}");
            assert_eq!(run.first(), run.last(), "{rows:?}");
            let counts = passes(run);
            assert_eq!(counts.len(), 4 * cells.crossed().len(), "{rows:?}");
            assert!(counts.values().all(|&n| n == 2), "{rows:?}: {counts:?}");
        }
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
        assert_eq!(run[..3], [Spot::Corner([1, 0]), Spot::Middle([0, 0]), Spot::Corner([0, 0])]);
    }

    #[test]
    fn req_fill_crs_004_at_a_corner_the_cross_below_and_left_is_sewn_before_the_one_above_and_right() {
        // 2 crosses whose `/` diagonals meet at (1, 1), from the needle there: the lower left one first.
        let run = &sewn(&cells(&[".x", "x."]), Some(p(1.0, 1.0)), None)[0];
        assert_eq!(run[..2], [Spot::Corner([1, 1]), Spot::Middle([0, 1])]);
        let middles: Vec<Spot> = run.iter().copied().filter(|s| matches!(s, Spot::Middle(_))).collect();
        assert_eq!(middles, [Spot::Middle([0, 1]), Spot::Middle([0, 1]), Spot::Middle([1, 0]), Spot::Middle([1, 0])]);
        // And of 2 whose `\` diagonals meet there, the lower right one first.
        let run = &sewn(&cells(&["x.", ".x"]), Some(p(1.0, 1.0)), None)[0];
        assert_eq!(run[..2], [Spot::Corner([1, 1]), Spot::Corner([2, 2])]);
    }

    #[test]
    fn req_fill_crs_004_groups_go_nearest_the_needle_first_and_nearest_the_end_last() {
        // 3 crosses apart in a row, at x = 0, 3 and 6.
        let cells = cells(&["x..x..x"]);
        let starts = |needle: Option<Point>, end: Option<Point>| sewn(&cells, needle, end).iter().map(|run| run[0]).collect::<Vec<Spot>>();
        // From the needle at the right, right to left, each from its corner nearest where the last ended.
        assert_eq!(starts(Some(p(9.0, 0.0)), None), [Spot::Corner([7, 0]), Spot::Corner([4, 0]), Spot::Corner([1, 0])]);
        // With the end at the right, the right cross last, from its corner nearest the end.
        assert_eq!(starts(Some(p(-1.0, 0.0)), Some(p(9.0, 1.0))), [Spot::Corner([0, 0]), Spot::Corner([3, 0]), Spot::Corner([7, 1])]);
        // With no needle, the first cross from its top right corner, and on from there.
        assert_eq!(starts(None, None), [Spot::Corner([1, 0]), Spot::Corner([3, 0]), Spot::Corner([6, 0])]);
        // A needle as near 2 corners starts at the first of them, clockwise from the top left: (0, 0), not (1, 0).
        assert_eq!(starts(Some(p(0.5, -1.0)), None)[0], Spot::Corner([0, 0]));
        // With no needle and the end at the left, the left cross last.
        assert_eq!(starts(None, Some(p(0.0, 0.0))), [Spot::Corner([4, 0]), Spot::Corner([6, 0]), Spot::Corner([0, 0])]);
        // The group of the needle and the end both, with others, goes last, from the end.
        assert_eq!(starts(Some(p(-1.0, 0.0)), Some(p(0.0, 1.0))), [Spot::Corner([3, 0]), Spot::Corner([6, 0]), Spot::Corner([0, 1])]);
    }

    #[test]
    fn req_fill_crs_004_one_group_with_a_needle_and_an_end_travels_through_the_crosses_first() {
        // A row of 4 crosses, from the needle at its left to the end at its right: from (0, 0) through the
        // middles of the fewest crosses to (4, 1), from corner to corner of each, then the loops from there.
        let cells = cells(&["xxxx"]);
        let runs = sewn(&cells, Some(p(-1.0, 0.0)), Some(p(5.0, 1.0)));
        assert_eq!(runs.len(), 1);
        let run = &runs[0];
        let travel = [
            Spot::Corner([0, 0]),
            Spot::Middle([0, 0]),
            Spot::Corner([1, 0]),
            Spot::Middle([1, 0]),
            Spot::Corner([2, 0]),
            Spot::Middle([2, 0]),
            Spot::Corner([3, 0]),
            Spot::Middle([3, 0]),
            Spot::Corner([4, 1]),
        ];
        assert_eq!(run[..travel.len()], travel);
        assert_eq!(run.len(), travel.len() + 6 * 4);
        assert_eq!(run.last(), Some(&Spot::Corner([4, 1])));
        // The same corner for both: no travel.
        let runs = sewn(&cells, Some(p(5.0, 1.0)), Some(p(5.0, 1.0)));
        assert_eq!(runs[0].len(), 1 + 6 * 4);
        // Travel stops where it reaches the end, and its steps are charged.
        let mut few = Budget { max_work: 40, ..Budget::DEFAULT }.meter();
        assert_eq!(travel_of(&cells, &mut few), Err(Exhausted::Work));
    }

    fn travel_of(cells: &Cells, meter: &mut Meter) -> Result<Vec<Spot>, Exhausted> {
        let mut long = cells.crossed().to_vec();
        long.extend((4..40).map(|i| [i, 0]));
        travel([0, 0], [40, 1], &Cells::of(long), meter)
    }
}
