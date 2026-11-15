//! A guided fill's rows over one piece made into the lines it sews: each row smoothed, stitched from its
//! stagger, started outside the piece where it closes, and cut by the piece, as Ink/Stitch staggers and cuts
//! its rows (read at `d59c9ab`, `REQ-FILL-GDD-005`, `REQ-FILL-GDD-012`).
//!
//! **Smoothed** by `smoothness_mm`, each row first.
//!
//! **A closed row** that doesn't cross itself, and starts inside the piece or has an anchor line beside it,
//! is started elsewhere before it is stitched. Where it crosses the anchor line, it is cut at each crossing,
//! and each piece is stitched on its own from the crossing it starts at, the anchor line setting where its
//! needle points start ([`anchored`]). Otherwise it starts at the middle of its first stretch outside the
//! piece's outline, holes left out, and is stitched from there. A closed row with no such stretch is left
//! as it is, its own points its needle points, as Ink/Stitch leaves it. Any other row is stitched from its
//! own start.
//!
//! **The stagger** counts from the row's number in the order the rows are taken, and for a buffer's ring
//! from the whole number of row spacings, rounded half to even as Python rounds, that it lies from the guide,
//! so that rings the same distance away start alike ([`super::stitched`]).
//!
//! **Started outside.** A row the anchor line started, and a stitched row that starts inside the piece, is
//! started again at the middle of its first stretch outside the outline, so that the cut by the piece starts
//! there. A row that doesn't close is closed first, as shapely does to a line it rolls.
//!
//! **Cut** by the piece ([`super::cut`]).

use stitchcraft_core::rng::SplitMix64;
use stitchcraft_core::{Exhausted, Meter, Point};

use super::closed::{closed, closed_up, outside, rolled};
use super::line::simple;
use super::stitched::{smoothed, stitched};
use super::{Guided, Strategy, cut};
use crate::generators::tatami::rings::Rings;
use crate::normalize::along::Along;
use crate::normalize::near::nearest_pair;
use crate::normalize::region::Polygon;
use crate::normalize::region::geom::{Hit, hit};

/// What the rows of a piece are stitched with: the lines the strategy offset or copied, which a buffer's
/// rings are measured from, and the anchor line's subpaths, flattened, none without one.
pub(super) struct Lines<'a> {
    pub(super) guide: &'a [Vec<Point>],
    pub(super) anchor: &'a [Vec<Point>],
}

/// The lines `rows`, taken in their order, sew over `piece`, whose rings are `rings`, as the module says.
/// Each row's random lengths draw from a stream of `rng` that its number keys. Costs as stitching and
/// cutting the rows do.
pub(super) fn lines(
    piece: &Polygon,
    rings: &Rings<'_>,
    rows: &[Vec<Point>],
    guided: &Guided,
    with: &Lines<'_>,
    rng: &SplitMix64,
    meter: &mut Meter,
) -> Result<Vec<Vec<Point>>, Exhausted> {
    let outline = Polygon { outline: piece.outline.clone(), holes: Vec::new() };
    let outline_rings = Rings::new(&outline, meter)?;
    let mut lines = Vec::new();
    for (i, row) in rows.iter().enumerate() {
        let row = smoothed(row, guided, meter)?;
        let starts_inside = |line: &[Point], meter: &mut Meter| match line.first() {
            Some(&p) => rings.inside(p, meter),
            None => Ok(false),
        };
        let ring = closed(&row) && simple(&row, meter)?;
        let mut moved = false;
        let sewn = if ring && (!with.anchor.is_empty() || starts_inside(&row, meter)?) {
            match anchored(&row, i, guided, with, rng, meter)? {
                Some(sewn) => {
                    moved = true;
                    sewn
                }
                None => match outside(&row, &outline_rings, meter)? {
                    Some(at) => {
                        let started = rolled(&row, at, meter)?;
                        stitched(&started, place(&started, i, guided, with, meter)?, i, guided, rng, meter)?
                    }
                    None => row,
                },
            }
        } else {
            stitched(&row, place(&row, i, guided, with, meter)?, i, guided, rng, meter)?
        };
        let sewn = match moved || starts_inside(&sewn, meter)? {
            true => match outside(&sewn, &outline_rings, meter)? {
                Some(at) => rolled(&closed_up(&sewn), at, meter)?,
                None => sewn,
            },
            false => sewn,
        };
        lines.extend(cut::cut(&sewn, rings, meter)?);
    }
    Ok(lines)
}

/// Where the stagger of `line`, the `i`-th row, counts from: `i`, or for a buffer's ring the row spacings
/// it lies from the guide, rounded half to even. Costs a unit of work for each pair of sides measured.
fn place(line: &[Point], i: usize, guided: &Guided, with: &Lines<'_>, meter: &mut Meter) -> Result<f64, Exhausted> {
    if guided.strategy != Strategy::Buffer {
        #[allow(clippy::cast_precision_loss)] // Row counts are far below 2⁵².
        return Ok(i as f64);
    }
    let guide: Vec<&[Point]> = with.guide.iter().map(Vec::as_slice).collect();
    let away = nearest_pair(&[line], &guide, meter)?.map_or(0.0, |(a, b)| a.distance(b));
    Ok((away / guided.spacing).round_ties_even())
}

/// The closed `row`, the `i`-th, stitched from where it crosses or touches the anchor line: started at the
/// first crossing along it, cut at the others, and each piece stitched from its start, as the module says;
/// `None` where it doesn't meet the anchor line. Costs as finding the crossings, stitching and measuring do.
fn anchored(
    row: &[Point],
    i: usize,
    guided: &Guided,
    with: &Lines<'_>,
    rng: &SplitMix64,
    meter: &mut Meter,
) -> Result<Option<Vec<Point>>, Exhausted> {
    let along = Along::new(row, meter)?;
    let mut crossings = Vec::new();
    for (side, &start) in row.windows(2).zip(&along.at) {
        for anchor in with.anchor {
            for other in anchor.windows(2) {
                meter.charge(1)?;
                if let Hit::Cross(x) | Hit::Touch(x) = hit(side[0], side[1], other[0], other[1]) {
                    crossings.push(start + side[0].distance(x));
                }
            }
        }
    }
    crossings.sort_by(f64::total_cmp);
    crossings.dedup();
    let Some(&first) = crossings.first() else { return Ok(None) };
    let started = rolled(row, first, meter)?;
    let around = Along::new(&started, meter)?;
    let cuts: Vec<f64> = std::iter::once(0.0).chain(crossings.iter().skip(1).map(|at| at - first)).chain(std::iter::once(around.length())).collect();
    let mut sewn = Vec::new();
    for pair in cuts.windows(2) {
        let (from, to) = (pair[0], pair[1]);
        if to > from {
            let piece = around.part(from, to);
            sewn.extend(stitched(&piece, place(&piece, i, guided, with, meter)?, i, guided, rng, meter)?);
        }
    }
    Ok(Some(sewn))
}

#[cfg(test)]
mod tests {
    use stitchcraft_core::Budget;

    use super::*;
    use crate::generators::guided::tests::guided;
    use crate::generators::tatami::fixture::{costs, p, rectangle};

    fn xy(points: &[Point]) -> Vec<(f64, f64)> {
        points.iter().map(|q| ((q.x() * 1e6).round() / 1e6, (q.y() * 1e6).round() / 1e6)).collect()
    }

    /// The closed square from (`x0`, `y0`) to (`x1`, `y1`), from that first corner along x first.
    fn square(x0: f64, y0: f64, x1: f64, y1: f64) -> Vec<Point> {
        vec![p(x0, y0), p(x1, y0), p(x1, y1), p(x0, y1), p(x0, y0)]
    }

    /// A guided fill by `strategy` with rows 1 mm apart, in stitches of 3 mm staggered over 4 rows.
    fn fill(strategy: Strategy) -> Guided {
        Guided { strategy, ..guided(&[(4.0, 2.0), (6.0, 2.0)], 1.0) }
    }

    /// The lines `rows` sew over a rectangle 10 mm wide and 4 mm high, with `anchor`.
    fn sewn(rows: &[Vec<Point>], guided: &Guided, anchor: &[Vec<Point>]) -> Vec<Vec<(f64, f64)>> {
        let meter = &mut Budget::DEFAULT.meter();
        let piece = rectangle(4.0);
        let rings = Rings::new(&piece, meter).unwrap();
        let guide = [vec![p(4.0, 2.0), p(6.0, 2.0)]];
        let with = Lines { guide: &guide, anchor };
        lines(&piece, &rings, rows, guided, &with, &SplitMix64::new(7), meter).unwrap().iter().map(|line| xy(line)).collect()
    }

    #[test]
    fn req_fill_gdd_012_a_buffer_s_stagger_counts_the_row_spacings_from_the_guide_rounded_half_to_even() {
        let meter = &mut Budget::DEFAULT.meter();
        let guide = [vec![p(4.0, 2.0), p(6.0, 2.0)]];
        let with = Lines { guide: &guide, anchor: &[] };
        let level = |y: f64| [p(0.0, y), p(10.0, y)];
        let buffer = fill(Strategy::Buffer);
        // 0.5, 1.5, 2.5 and 3.5 row spacings away count as 0, 2, 2 and 4, and 1.4 and 1.6 as 1 and 2.
        let places: Vec<f64> = [2.5, 3.5, 4.5, 5.5, 3.4, 3.6].iter().map(|&y| place(&level(y), 7, &buffer, &with, meter).unwrap()).collect();
        assert_eq!(places, [0.0, 2.0, 2.0, 4.0, 1.0, 2.0]);
        // Half the row spacing, twice the count; and a row of no point lies nowhere.
        let close = Guided { spacing: 0.5, ..buffer.clone() };
        assert_eq!(place(&level(3.0), 7, &close, &with, meter).unwrap(), 2.0);
        assert_eq!(place(&[], 7, &buffer, &with, meter).unwrap(), 0.0);
        // The other strategies count the row's number.
        for strategy in [Strategy::Copy, Strategy::Offset] {
            assert_eq!(place(&level(4.5), 7, &fill(strategy), &with, meter).unwrap(), 7.0);
        }
    }

    #[test]
    fn req_fill_gdd_012_a_row_that_doesn_t_close_is_stitched_from_its_start_and_cut() {
        // 3 level rows 16 mm long through the rectangle, each stitched from a quarter of a stitch further along
        // than the one before, 0, 0.75 and 1.5 mm, its stitches spread evenly over the rest, and cut.
        let level = |y: f64| vec![p(-3.0, y), p(13.0, y)];
        let rows = sewn(&[level(3.0), level(2.0), level(1.0)], &fill(Strategy::Offset), &[]);
        assert_eq!(rows[0], [(0.0, 3.0), (2.333333, 3.0), (5.0, 3.0), (7.666667, 3.0), (10.0, 3.0)]);
        assert_eq!(rows[1], [(0.0, 2.0), (0.291667, 2.0), (2.833333, 2.0), (5.375, 2.0), (7.916667, 2.0), (10.0, 2.0)]);
        assert_eq!(rows[2], [(0.0, 1.0), (1.4, 1.0), (4.3, 1.0), (7.2, 1.0), (10.0, 1.0)]);
        // A row that starts inside the piece is stitched from there, its first point kept before its stitches.
        // It is then closed and started at the middle of its first stretch outside the piece, (5, 6), so the
        // side that closes it, from (8, 1) to (2, 1), is sewn too, as Ink/Stitch sews it.
        let hook = vec![p(2.0, 1.0), p(2.0, 6.0), p(8.0, 6.0), p(8.0, 1.0)];
        let lines = sewn(&[hook], &fill(Strategy::Offset), &[]);
        assert_eq!(lines, [vec![(8.0, 4.0), (8.0, 3.5), (8.0, 1.0), (2.0, 1.0), (2.0, 1.0), (2.0, 3.5), (2.0, 4.0)]]);
    }

    #[test]
    fn req_fill_gdd_012_a_closed_row_starts_on_the_anchor_line_or_outside_the_piece() {
        // A ring that leaves the piece through its bottom side, from a start inside it: without an anchor line
        // it starts at the middle of its stretch outside, (5, 6), and is stitched from there, each side of 6 mm
        // in 2 stitches and each of 5 mm in 2.
        let ring = square(2.0, 1.0, 8.0, 6.0);
        let offset = fill(Strategy::Offset);
        let outside = sewn(std::slice::from_ref(&ring), &offset, &[]);
        assert_eq!(outside, [vec![(2.0, 4.0), (2.0, 3.5), (2.0, 1.0), (5.0, 1.0), (8.0, 1.0), (8.0, 3.5), (8.0, 4.0)]]);
        // An anchor line across its top side at x = 4 starts it there: the stitches count from (4, 1), which is
        // there twice, as the row's first point and as its first stitch's start.
        let anchor = [vec![p(4.0, 0.5), p(4.0, 1.5)]];
        let anchored = sewn(std::slice::from_ref(&ring), &offset, &anchor);
        let from_4 = vec![(2.0, 4.0), (2.0, 3.5), (2.0, 1.0), (4.0, 1.0), (4.0, 1.0), (6.0, 1.0), (8.0, 1.0), (8.0, 3.5), (8.0, 4.0)];
        assert_eq!(anchored.as_slice(), std::slice::from_ref(&from_4));
        // The next ring's stitches start a quarter of a stitch on from there.
        let next = sewn(&[ring.clone(), ring.clone()], &offset, &anchor);
        assert_eq!(next[0], from_4);
        assert_eq!(next[1], [(2.0, 4.0), (2.0, 3.5), (2.0, 1.0), (4.0, 1.0), (4.75, 1.0), (6.375, 1.0), (8.0, 1.0), (8.0, 3.5), (8.0, 4.0)]);
        // One that crosses it twice cuts it in 2, each stitched from its crossing, and one that only touches
        // it counts too.
        let twice = [vec![p(4.0, 0.5), p(4.0, 1.5)], vec![p(1.5, 2.5), p(2.5, 2.5)]];
        let cut = sewn(std::slice::from_ref(&ring), &offset, &twice);
        let from_both =
            [(2.0, 4.0), (2.0, 2.5), (2.0, 2.5), (2.0, 2.5), (2.0, 1.0), (4.0, 1.0), (4.0, 1.0), (6.0, 1.0), (8.0, 1.0), (8.0, 3.5), (8.0, 4.0)];
        assert_eq!(cut, [from_both.to_vec()]);
        let touching = [vec![p(4.0, 1.0), p(4.0, 1.5)]];
        assert_eq!(sewn(std::slice::from_ref(&ring), &offset, &touching), anchored);
        // An anchor line that misses the ring leaves it to start outside the piece.
        let aside = [vec![p(20.0, 0.0), p(21.0, 0.0)]];
        assert_eq!(sewn(std::slice::from_ref(&ring), &offset, &aside), outside);
        // A ring that starts outside the piece and has no anchor line is stitched from its own start.
        let from_outside = sewn(&[square(2.0, 6.0, 8.0, 1.0)], &offset, &[]);
        assert_eq!(from_outside, [vec![(8.0, 4.0), (8.0, 3.5), (8.0, 1.0), (5.0, 1.0), (2.0, 1.0), (2.0, 3.5), (2.0, 4.0)]]);
        // A ring inside the piece that no anchor line meets has no stretch outside: its own points are its
        // needle points, and with an anchor line it starts there.
        let within = square(3.0, 1.0, 7.0, 3.0);
        assert_eq!(sewn(std::slice::from_ref(&within), &offset, &[]), [xy(&within)]);
        assert_eq!(sewn(std::slice::from_ref(&within), &offset, &aside), [xy(&within)]);
        assert_eq!(
            sewn(std::slice::from_ref(&within), &offset, &anchor),
            [vec![(4.0, 1.0), (7.0, 1.0), (7.0, 3.0), (5.0, 3.0), (3.0, 3.0), (3.0, 1.0), (4.0, 1.0)]]
        );
        // An anchor line that ends on the ring's first point meets its first side and its last there: one
        // crossing, and the ring is stitched once round from it.
        let at_start = [vec![p(2.5, 0.5), p(3.0, 1.0)]];
        assert_eq!(
            sewn(std::slice::from_ref(&within), &offset, &at_start),
            [vec![(3.0, 1.0), (5.0, 1.0), (7.0, 1.0), (7.0, 3.0), (5.0, 3.0), (3.0, 3.0), (3.0, 1.0)]]
        );
        // A row of no point sews nothing.
        assert!(sewn(&[Vec::new()], &offset, &anchor).is_empty());
        // A row that crosses itself is no ring: it is stitched from its start, whatever the anchor line.
        let eight = vec![p(3.0, 1.0), p(7.0, 3.0), p(7.0, 1.0), p(3.0, 3.0), p(3.0, 1.0)];
        assert_eq!(sewn(std::slice::from_ref(&eight), &offset, &anchor), sewn(std::slice::from_ref(&eight), &offset, &[]));
    }

    #[test]
    fn stitching_and_cutting_the_rows_runs_out_of_a_budget_of_any_less_work_than_it_takes() {
        let piece = rectangle(4.0);
        let rings = Rings::new(&piece, &mut Budget::DEFAULT.meter()).unwrap();
        let guide = [vec![p(4.0, 2.0), p(6.0, 2.0)]];
        let anchor = [vec![p(4.0, 0.5), p(4.0, 1.5)], vec![p(1.5, 2.5), p(2.5, 2.5)]];
        let rows = [square(2.0, 1.0, 8.0, 6.0), vec![p(2.0, 1.0), p(2.0, 6.0), p(8.0, 6.0), p(8.0, 1.0)], square(3.0, 1.0, 7.0, 3.0)];
        let smooth = Guided { smoothness: 0.3, ..fill(Strategy::Buffer) };
        let run = |anchor: &[Vec<Point>], guided: &Guided, meter: &mut Meter| {
            let with = Lines { guide: &guide, anchor };
            lines(&piece, &rings, &rows, guided, &with, &SplitMix64::new(7), meter).map(|_| ())
        };
        let work = costs(&[Box::new(|meter| run(&anchor, &smooth, meter)), Box::new(|meter| run(&[], &fill(Strategy::Offset), meter))]);
        assert!(work[0] > work[1] && work[1] > 100, "{work:?}");
    }
}
