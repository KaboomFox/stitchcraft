//! Which colour each row of a gradient fill takes, as Ink/Stitch shares a gradient's rows out
//! (`docs/src/design/algorithms/gradient.md` › The colours of the rows, read at `d59c9ab`).
//!
//! **Why a fixed pattern.** Thread can't be mixed, so a gradient fill blends 2 colours by the share of rows
//! each takes. Between 2 stops the first colour's runs of rows shorten and the second's lengthen, in a
//! pattern that depends only on how many rows lie between them. The pattern is Ink/Stitch's, so a file sews
//! the same blend in both programs.
//!
//! **The stops in turn.** The first stop takes the rows from row 0 to its own. Each stop after it blends
//! with the colour before it, over the rows from where the last blend ended to its own row, unless both lie
//! before row 0: then it blends nothing, and the next stop blends from it. A blend of 2 rows or fewer is no
//! blend: the stop is passed over, and the next blend starts a row further on and blends from the colour
//! before it. After the last blend, the rows left take the last stop's colour. Rows before row 0 or past the
//! last are left out, and no blend gives row 0 a colour.
//!
//! **What costs.** Each row a blend lays is charged to the meter, those it lays outside the rows too: a
//! gradient may reach far past its fill.

use stitchcraft_core::{Exhausted, Meter};

/// A stop as the rows see it: the row it lies at, and its colour, by its place among the colours.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Placed {
    /// Its row, which may lie before row 0 or past the last.
    pub(crate) row: i64,
    /// Its colour's place among the gradient's colours.
    pub(crate) colour: usize,
}

/// The rows each colour takes, and the stops passed over.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct Shares {
    /// Each colour's rows, by the colour's place, in the order they come to it.
    pub(crate) rows: Vec<Vec<usize>>,
    /// The stops passed over, by their place among the stops, each with the row its blend leaves out.
    pub(crate) passed: Vec<(usize, i64)>,
}

/// The rows from 0 to `last` shared out among `colours` colours by `stops`, in their order, as the module's
/// docs say. A unit of `meter` for each row a blend lays and each row a colour takes.
pub(crate) fn share(stops: &[Placed], colours: usize, last: usize, meter: &mut Meter) -> Result<Shares, Exhausted> {
    let mut shares = Shares { rows: vec![Vec::new(); colours], passed: Vec::new() };
    let Some((first, rest)) = stops.split_first() else { return Ok(shares) };
    let last_row = i64::try_from(last).unwrap_or(i64::MAX);
    if first.row > 0 {
        take(&mut shares, first.colour, 0..=first.row.min(last_row), meter)?;
    }
    let (mut before, mut colour) = (first.row, first.colour);
    for (i, stop) in rest.iter().enumerate() {
        if before < 0 && stop.row < 0 {
            (before, colour) = (stop.row, stop.colour);
            continue;
        }
        let begin = before.saturating_add(1);
        let Some(half) = half(stop.row.saturating_sub(before), meter)? else {
            shares.passed.push((i + 1, begin));
            before = begin;
            continue;
        };
        let length = i64::try_from(half.len()).unwrap_or(i64::MAX);
        let mirrored = |r: usize| begin.saturating_add(2 * length - 1 - i64::try_from(r).unwrap_or(i64::MAX));
        let laid = |r: usize| begin.saturating_add(i64::try_from(r).unwrap_or(i64::MAX));
        // The colour before takes its rows of the first half, then the second half's rows of the other colour's
        // in the first, which mirror them; the stop's colour the same the other way round.
        for (to, firsts) in [(colour, true), (stop.colour, false)] {
            let own = half.iter().enumerate().filter(|&(_, &first)| first == firsts).map(|(r, _)| laid(r));
            let mirror = half.iter().enumerate().filter(|&(_, &first)| first != firsts).map(|(r, _)| mirrored(r));
            let rows: Vec<i64> = own.chain(mirror).filter(|&row| row > 0 && row <= last_row).collect();
            take(&mut shares, to, rows, meter)?;
        }
        (before, colour) = (begin.saturating_add(2 * length - 1), stop.colour);
    }
    // Past the last blend, every row takes the last stop's colour. Ink/Stitch counts a start before row 0
    // from its last row back (`DEV-FILL-032`).
    let colour = rest.last().unwrap_or(first).colour;
    take(&mut shares, colour, before.saturating_add(1).max(0)..=last_row, meter)?;
    Ok(shares)
}

/// Gives `colour` the `rows` of `shares`, a unit of `meter` for each.
fn take(shares: &mut Shares, colour: usize, rows: impl IntoIterator<Item = i64>, meter: &mut Meter) -> Result<(), Exhausted> {
    let Some(list) = shares.rows.get_mut(colour) else { return Ok(()) };
    for row in rows {
        meter.charge(1)?;
        list.extend(usize::try_from(row).ok());
    }
    Ok(())
}

/// The first half of a blend `rows` long, as the gradient page's pattern lays it: for each row in turn,
/// whether the first colour takes it. `None` when it has no row of the first colour, as a blend of 2 rows or
/// fewer has none. A unit of `meter` for each row.
pub(crate) fn half(rows: i64, meter: &mut Meter) -> Result<Option<Vec<bool>>, Exhausted> {
    let rounds = isqrt(rows.max(0));
    let mut extra = (rows - rounds * rounds) / 2;
    let mut laid: Vec<bool> = Vec::new();
    let mut most: Option<i64> = None;
    for round in 1..=rounds {
        let (each, more) = ((rounds - round) / round, (rounds - round) % round);
        let mut last = 0;
        for run in 0..round {
            let mut firsts = each + i64::from(run < more);
            if extra > 0 {
                firsts += 1;
                extra -= 1;
            }
            firsts = most.map_or(firsts, |most| firsts.min(most));
            last = firsts;
            for _ in 0..firsts {
                meter.charge(1)?;
                laid.push(true);
                // The half ends with the first row of the first colour laid 2 rows short of half the blend or
                // later, and the row of the second colour after it.
                if rows <= 2 * (i64::try_from(laid.len()).unwrap_or(i64::MAX) + 1) {
                    laid.push(false);
                    return Ok(Some(laid));
                }
            }
            meter.charge(1)?;
            laid.push(false);
        }
        most = Some(last);
    }
    Ok(laid.contains(&true).then_some(laid))
}

/// The whole square root of `n`, at least 0.
fn isqrt(n: i64) -> i64 {
    u64::try_from(n).map_or(0, |n| i64::try_from(n.isqrt()).unwrap_or(i64::MAX))
}

#[cfg(test)]
mod tests {
    use stitchcraft_core::Budget;

    use super::*;

    /// The blend `rows` long, its first half and its second, as the colours' numbers, 1 and 2.
    fn blend(rows: i64) -> String {
        let half = half(rows, &mut Budget::DEFAULT.meter()).unwrap().unwrap();
        let second = half.iter().rev().map(|first| !first);
        half.iter().copied().chain(second).map(|first| if first { '1' } else { '2' }).collect()
    }

    #[test]
    fn req_fill_grd_005_a_blend_shortens_the_first_colour_s_runs_and_lengthens_the_second_s() {
        // The page's example, and blends round it: the first half ends a row of the second colour after the
        // first row of the first colour 2 rows short of half the blend or later, so a blend can end before
        // its stop's row or past it.
        assert_eq!(blend(12), "111212121222");
        assert_eq!(blend(13), "11121121221222");
        assert_eq!(blend(8), "1121212122");
        assert_eq!(blend(7), "1122211122");
        assert_eq!(blend(6), "112122");
        assert_eq!(blend(5), "12221112");
        assert_eq!((blend(4), blend(3)), ("1212".to_string(), "1212".to_string()));
        // 2 rows or fewer have no row of the first colour, nor has a blend going back.
        for rows in [2, 1, 0, -1, -40] {
            assert_eq!(half(rows, &mut Budget::DEFAULT.meter()).unwrap(), None, "{rows}");
        }
    }

    #[test]
    fn the_rows_past_n_squared_lengthen_the_first_runs_and_no_run_outgrows_the_round_before() {
        // 23 rows: n is 4, and 3 rows, half the 7 past 16, go to the first 3 runs laid: round 1's run takes 4
        // rows of the first colour, and round 2's 2 runs 2 each.
        assert_eq!(blend(23), "11112112112121212212212222");
        // 27 rows: n is 5 and 1 row past 25 goes to round 1's run.
        assert_eq!(blend(27), "111112112121212121212122122222");
        // 160 rows, as Ink/Stitch sewed them across a square 40 mm wide, run as a black box: from round 2 on a
        // run would take more rows than the round before's last run, which caps it.
        assert_eq!(
            blend(160),
            [
                "1111111111112111111211111121111211112111121112111211211211211212121212121212121212121212121212121212",
                "212212212212221222122221222212222122222212222221222222222222"
            ]
            .concat()
        );
    }

    fn placed(stops: &[(i64, usize)]) -> Vec<Placed> {
        stops.iter().map(|&(row, colour)| Placed { row, colour }).collect()
    }

    fn shared(stops: &[(i64, usize)], colours: usize, last: usize) -> Shares {
        share(&placed(stops), colours, last, &mut Budget::DEFAULT.meter()).unwrap()
    }

    #[test]
    fn req_fill_grd_005_the_first_stop_takes_the_rows_to_its_own_and_the_last_those_after_the_last_blend() {
        // Rows 0 to 30, a stop at row 4 and one at row 16: rows 0 to 4 the first colour's, a blend of 12 from
        // row 5 to 16, and rows 17 to 30 the second's.
        let shares = shared(&[(4, 0), (16, 1)], 2, 30);
        let blend: Vec<usize> = (5..=16).collect();
        let firsts: Vec<usize> = (0..=4).chain([5, 6, 7, 9, 13, 11]).collect();
        assert_eq!(shares.rows[0], firsts);
        // The second colour's rows of the first half, then the second half's from its end back to its middle.
        let seconds: Vec<usize> = [8, 10, 16, 15, 14, 12].into_iter().chain(17..=30).collect();
        assert_eq!(shares.rows[1], seconds);
        assert!(shares.passed.is_empty());
        let mut all: Vec<usize> = shares.rows.concat();
        all.sort_unstable();
        assert_eq!(all, (0..=30).collect::<Vec<_>>(), "every row once: {blend:?}");
        // A first stop at row 0 or before gives row 0 no colour, and a blend never does.
        let shares = shared(&[(0, 0), (12, 1)], 2, 20);
        assert!(!shares.rows.concat().contains(&0));
        let shares = shared(&[(-3, 0), (12, 1)], 2, 20);
        assert_eq!(shares.rows[0], [2, 3, 5, 12, 9, 7]);
        assert_eq!(shares.rows[1], [1, 4, 6, 15, 14, 13, 11, 10, 8, 16, 17, 18, 19, 20]);
        // A first stop past the last row gives it every row.
        assert_eq!(shared(&[(40, 0), (60, 1)], 2, 20).rows, [(0..=20).collect::<Vec<_>>(), vec![]]);
        // Rows of a blend past the last are left out.
        assert_eq!(shared(&[(4, 0), (16, 1)], 2, 10).rows.concat().len(), 11);
    }

    #[test]
    fn req_fill_grd_005_a_blend_of_2_rows_or_fewer_passes_its_stop_over_and_leaves_a_row_out() {
        // Stops at rows 10, 12 and 30: the blend from 11 to 12 is 2 rows, no blend, so the stop at 12 is
        // passed over. The next blend starts at row 12, from the first colour to the third, and row 11 is
        // left out.
        let shares = shared(&[(10, 0), (12, 1), (30, 2)], 3, 40);
        assert_eq!(shares.passed, [(1, 11)]);
        assert!(shares.rows[1].is_empty());
        assert!(!shares.rows.concat().contains(&11));
        assert_eq!((shares.rows[0].iter().max(), shares.rows[2].iter().min()), (Some(&29), Some(&16)));
        // A blend of 3 rows is one.
        assert!(shared(&[(10, 0), (13, 1)], 2, 40).passed.is_empty());
        // Stops at one row, as a gradient whose start is its end puts them: all but the first passed over,
        // each a row further on, and the rows after them the last stop's.
        let shares = shared(&[(10, 0), (10, 1), (10, 2)], 3, 20);
        assert_eq!(shares.passed, [(1, 11), (2, 12)]);
        assert_eq!(shares.rows, [(0..=10).collect::<Vec<_>>(), vec![], (13..=20).collect()]);
        // A stop passed over at the end still gives the rows after the last blend its colour.
        let shares = shared(&[(4, 0), (16, 1), (17, 2)], 3, 20);
        assert_eq!((shares.passed.as_slice(), shares.rows[2].as_slice()), (&[(2, 17)][..], &[18, 19, 20][..]));
    }

    #[test]
    fn stops_before_row_0_blend_nothing_and_the_rows_after_the_last_blend_take_the_last_colour() {
        // 2 stops before row 0: the second takes over from the first without a blend, and every row is its.
        let shares = shared(&[(-30, 0), (-10, 1)], 2, 20);
        assert_eq!(shares.rows, [vec![], (0..=20).collect::<Vec<_>>()]);
        // A third stop then blends from the second: of the blend of 13 rows from row -2, the second colour
        // takes rows 2, 3, 8 and 5, and the first colour none. Row 0 is left out.
        let shares = shared(&[(-5, 0), (-3, 1), (10, 2)], 3, 20);
        assert_eq!(shares.rows[1], [2, 3, 8, 5]);
        assert!(shares.rows[0].is_empty() && shares.passed.is_empty() && !shares.rows.concat().contains(&0), "{shares:?}");
        // From before row 0 to row 0 is a blend of 2 rows, which passes its stop over: the rows after it, row 0
        // among them, take its colour.
        let shares = shared(&[(-2, 0), (0, 1)], 2, 10);
        assert_eq!((shares.rows, shares.passed), (vec![vec![], (0..=10).collect()], vec![(1, -1)]));
        // From before row 0 to past it: the blend's rows from row 1 on, the first half's all before it.
        let shares = shared(&[(-6, 0), (6, 1)], 2, 20);
        assert_eq!(shares.rows[0], [3, 1]);
        assert_eq!(shares.rows[1], [6, 5, 4, 2].into_iter().chain(7..=20).collect::<Vec<_>>());
        // Stops of one colour share their rows: the first stop's, then the blend's first colour's and its
        // second's, then the rows after it.
        let shares = shared(&[(2, 0), (14, 0)], 1, 20);
        assert_eq!(shares.rows[0], [0, 1, 2, 3, 4, 5, 7, 11, 9, 6, 8, 14, 13, 12, 10, 15, 16, 17, 18, 19, 20]);
        // No stop, or a colour out of the list, takes nothing.
        assert_eq!(shared(&[], 1, 20).rows, [Vec::<usize>::new()]);
        assert_eq!(shared(&[(2, 3), (14, 3)], 1, 20).rows, [Vec::<usize>::new()]);
    }

    #[test]
    fn every_row_a_blend_lays_is_charged() {
        let mut meter = Budget { max_stitches: 10, max_work: 100 }.meter();
        assert!(half(1_000, &mut meter).is_err());
        let mut meter = Budget { max_stitches: 10, max_work: 100 }.meter();
        assert!(share(&placed(&[(-500, 0), (500, 1)]), 2, 20, &mut meter).is_err());
    }
}
