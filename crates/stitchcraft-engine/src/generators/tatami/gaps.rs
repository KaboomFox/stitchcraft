//! Gap-fill rows (`gap_fill_rows`, `REQ-FILL-TAT-007`): rows sewn twice where the needle leaves a stretch
//! of rows for another.
//!
//! The route sews a part's rows in stretches: back and forth, row after row, each handing over to the one
//! beside it, until the needle travels to another stretch. The rows on the far side of that last row are
//! sewn later, and the fabric between can be pulled apart by then, which shows as a gap along the last
//! row. Ink/Stitch makes up for it as the route leaves a stretch of more than 3 rows: it sews that many more
//! rows past the last one, rounded up to an even number so that the needle ends on the same side, and
//! comes back to where the last row ended.
//!
//! Ink/Stitch repeats the last row, moved a row spacing further each time, so a repeat keeps the last row's
//! length even where the part narrows, and can run outside it. Here each extra row is a row of the grid
//! past the last one, the part of its segment beside the last row: it covers the same stretch but stops
//! at the part's edge (`DEV-FILL-010`). Where the grid has no segment beside the last row, the part ends,
//! there is no gap to fill, and the extra rows stop.

use std::cmp::Ordering;

use stitchcraft_core::{Exhausted, Meter, Point};

/// The rows that fill the gap after row segment `last`, sewn after `before` in the same stretch: up to
/// `count` rows, from the grid rows past `last`'s row, away from `before`'s, each the stretch of that row's
/// segment beside `last` that overlaps it most, from its start to its end along the rows. `segments` are
/// the part's row segments, and `rows` the grid row of each, in order across; none past the part's edge or
/// for 2 segments of one row. One unit of `meter` for each segment looked at.
pub(super) fn gap_rows(
    segments: &[(Point, Point)],
    rows: &[usize],
    last: usize,
    before: usize,
    count: usize,
    meter: &mut Meter,
) -> Result<Vec<(Point, Point)>, Exhausted> {
    let (Some(&(start, end)), Some(&row), Some(&row_before)) = (segments.get(last), rows.get(last), rows.get(before)) else {
        return Ok(Vec::new());
    };
    let length = start.distance(end);
    // How far along the last row a point lies, measured from its start.
    let along = |p: Point| ((p.x() - start.x()) * (end.x() - start.x()) + (p.y() - start.y()) * (end.y() - start.y())) / length;
    let mut found = Vec::new();
    for k in 1..=count {
        let target = match row.cmp(&row_before) {
            Ordering::Greater => row.checked_add(k),
            Ordering::Less => row.checked_sub(k),
            Ordering::Equal => None,
        };
        let Some(target) = target else { break };
        let in_row = rows.partition_point(|&r| r < target)..rows.partition_point(|&r| r <= target);
        let mut beside: Option<(f64, Point, Point)> = None;
        for &(s, e) in segments.get(in_row).unwrap_or_default() {
            meter.charge(1)?;
            let (ts, te) = (along(s), along(e));
            let (low, high) = (ts.min(te).max(0.0), ts.max(te).min(length));
            if high - low > beside.map_or(0.0, |(overlap, _, _)| overlap) {
                let at = |t: f64| s.lerp(e, (t - ts) / (te - ts));
                let (a, b) = if ts <= te { (at(low), at(high)) } else { (at(high), at(low)) };
                beside = Some((high - low, a, b));
            }
        }
        let Some((_, a, b)) = beside else { break };
        found.push((a, b));
    }
    Ok(found)
}

#[cfg(test)]
mod tests {
    use stitchcraft_core::Budget;

    use super::*;
    use crate::generators::tatami::fixture::p;

    /// Row segments 1 mm apart across a part that narrows upwards: rows 0 to 2 run from x = 0 to 10, row 3
    /// from 2 to 6 and from 7 to 10, and row 4 from 5 to 4, the other way.
    fn narrowing() -> (Vec<(Point, Point)>, Vec<usize>) {
        let segments = vec![
            (p(0.0, 0.0), p(10.0, 0.0)),
            (p(0.0, 1.0), p(10.0, 1.0)),
            (p(0.0, 2.0), p(10.0, 2.0)),
            (p(2.0, 3.0), p(6.0, 3.0)),
            (p(7.0, 3.0), p(10.0, 3.0)),
            (p(5.0, 4.0), p(4.0, 4.0)),
        ];
        (segments, vec![0, 1, 2, 3, 3, 4])
    }

    fn gaps(last: usize, before: usize, count: usize) -> Vec<(Point, Point)> {
        let (segments, rows) = narrowing();
        gap_rows(&segments, &rows, last, before, count, &mut Budget::DEFAULT.meter()).unwrap()
    }

    #[test]
    fn the_rows_past_the_last_are_the_grid_s_beside_it_and_stop_where_the_part_ends() {
        // After row 1, sewn after row 0: row 2 whole, then of row 3 the segment that overlaps row 1 most,
        // 2 to 6, then row 4 within row 1's span, and none past it.
        let rows = vec![(p(0.0, 2.0), p(10.0, 2.0)), (p(2.0, 3.0), p(6.0, 3.0)), (p(5.0, 4.0), p(4.0, 4.0))];
        assert_eq!(gaps(1, 0, 2), rows[..2]);
        assert_eq!(gaps(1, 0, 6), rows);
        // After row 3's first segment, sewn after row 2: row 4, within the last row's span of 2 to 6.
        assert_eq!(gaps(3, 2, 2), [(p(5.0, 4.0), p(4.0, 4.0))]);
    }

    #[test]
    fn going_down_the_rows_past_the_last_are_those_below_it() {
        assert_eq!(gaps(2, 3, 2), [(p(0.0, 1.0), p(10.0, 1.0)), (p(0.0, 0.0), p(10.0, 0.0))]);
        // Row 3's segment from 2 to 6, sewn after row 4: the rows below within its span.
        assert_eq!(gaps(3, 5, 2), [(p(2.0, 2.0), p(6.0, 2.0)), (p(2.0, 1.0), p(6.0, 1.0))]);
        // Row 4 has nothing above it.
        assert_eq!(gaps(5, 3, 2), Vec::new());
    }

    #[test]
    fn a_row_past_the_last_is_cut_to_the_last_row_s_span() {
        // Row 3's second segment, from 7 to 10, after row 2: row 4, from 5 to 4, does not overlap it.
        assert_eq!(gaps(4, 2, 2), Vec::new());
        // The same after row 4: rows 2 and 1, cut to 7 to 10.
        assert_eq!(gaps(4, 5, 2), [(p(7.0, 2.0), p(10.0, 2.0)), (p(7.0, 1.0), p(10.0, 1.0))]);
    }

    #[test]
    fn a_last_row_sewn_after_a_segment_of_its_own_row_has_none() {
        assert_eq!(gaps(4, 3, 2), Vec::new());
        assert_eq!(gaps(9, 3, 2), Vec::new(), "no such segment");
    }

    #[test]
    fn each_segment_looked_at_is_charged_to_the_budget() {
        // Rows 2, 3 and 4: 1, 2 and 1 segments.
        let (segments, rows) = narrowing();
        let spend = |work| gap_rows(&segments, &rows, 1, 0, 6, &mut Budget { max_stitches: 1, max_work: work }.meter()).is_ok();
        assert_eq!((spend(4), spend(3)), (true, false));
    }
}
