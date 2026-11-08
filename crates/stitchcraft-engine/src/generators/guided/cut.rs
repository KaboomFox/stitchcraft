//! A guided fill's stitched row cut by its piece: the lines it sews, as Ink/Stitch cuts each row by the
//! piece's outline (read at `d59c9ab`, `REQ-FILL-GDD-006`).
//!
//! **Where it is cut.** The row, its needle points joined by straight stitches, is cut at every point where
//! it crosses one of the piece's rings or touches one. Where it runs along a ring, it is not cut at the ends
//! of that stretch, nor at the points of it where it touches another side, as GEOS gives Ink/Stitch that
//! stretch as a line and Ink/Stitch cuts only at points.
//!
//! **What is kept.** A stretch between 2 cuts, or between a cut and the row's end, is a line of the fill when
//! the piece holds its middle fifth, from 40 % to 60 % of its length: no part of that fifth outside the piece,
//! and some of it inside, off the rings. Each line runs the row's way, from the point it was cut at, through
//! the row's needle points, to the next cut. A stretch of no length is none.
//!
//! **Cost.** Only the sides of the rings near each stitch of the row are looked at, as [`Rings::meeting`]
//! finds them, and a stretch's middle is asked about where it lies.

use stitchcraft_core::{Exhausted, Meter, Point};

use crate::generators::tatami::rings::Rings;
use crate::normalize::along::Along;
use crate::normalize::region::SAME;
use crate::normalize::region::geom::{Hit, hit};

/// Where the middle fifth of a stretch starts and ends, as a share of its length.
const MIDDLE: [f64; 2] = [0.4, 0.6];

/// The lines of the stitched `row` inside the piece whose rings are `rings`, in order along it, each
/// running its way.
pub(super) fn cut(row: &[Point], rings: &Rings<'_>, meter: &mut Meter) -> Result<Vec<Vec<Point>>, Exhausted> {
    let along = Along::new(row, meter)?;
    let Meetings { points, along_rings } = meetings(&along, rings, meter)?;
    // A point on a stretch along a ring is no cut.
    let on_stretch = |at: f64| along_rings.iter().any(|&[from, to]| at >= from - SAME && at <= to + SAME);
    let mut cuts: Vec<f64> = points.into_iter().filter(|&at| !on_stretch(at)).collect();
    cuts.sort_by(f64::total_cmp);
    let bounds: Vec<f64> = std::iter::once(0.0).chain(cuts).chain(std::iter::once(along.length())).collect();
    let mut lines = Vec::new();
    for pair in bounds.windows(2) {
        let (from, to) = (pair[0], pair[1]);
        meter.charge(1)?;
        // A point the row meets 2 sides at, where they join, is 2 cuts: the stretch between them is none.
        if to - from <= SAME {
            continue;
        }
        let [a, b] = MIDDLE.map(|share| from + share * (to - from));
        if held(&along, [a, b], &along_rings, rings, meter)? {
            lines.push(along.part(from, to));
        }
    }
    Ok(lines)
}

/// Where a row meets the rings of its piece, as distances along it: the points where it crosses or touches
/// one, and the stretches where it runs along one.
struct Meetings {
    points: Vec<f64>,
    along_rings: Vec<[f64; 2]>,
}

/// Where the row `along` meets `rings`. A unit of `meter`'s work for each stitch, and as [`Rings::meeting`]
/// looks.
fn meetings(along: &Along, rings: &Rings<'_>, meter: &mut Meter) -> Result<Meetings, Exhausted> {
    let mut found = Meetings { points: Vec::new(), along_rings: Vec::new() };
    for (side, &start) in along.points.windows(2).zip(&along.at) {
        meter.charge(1)?;
        let (p, q) = (side[0], side[1]);
        rings.meeting(p, q, meter, |ring| match hit(p, q, ring.a, ring.b) {
            Hit::Cross(x) | Hit::Touch(x) => found.points.push(start + p.distance(x)),
            Hit::Overlap(x, y) => {
                let (dx, dy) = (start + p.distance(x), start + p.distance(y));
                found.along_rings.push([dx.min(dy), dx.max(dy)]);
            }
            Hit::Apart => {}
        })?;
    }
    Ok(found)
}

/// Whether the piece holds the stretch of the row `along` from `a` to `b`: none of it outside the piece, and
/// some of it inside, off the rings, as GEOS decides that the piece contains a line. The stretch is taken in
/// parts at the ends of the stretches the row runs along a ring, `along_rings`, which lie on the rings, and
/// every other part is asked about at its middle.
fn held(along: &Along, [a, b]: [f64; 2], along_rings: &[[f64; 2]], rings: &Rings<'_>, meter: &mut Meter) -> Result<bool, Exhausted> {
    let mut ends: Vec<f64> = along_rings.iter().flat_map(|&[from, to]| [from, to]).filter(|&at| at > a && at < b).collect();
    ends.extend([a, b]);
    ends.sort_by(f64::total_cmp);
    let mut inside = false;
    for pair in ends.windows(2) {
        let (from, to) = (pair[0], pair[1]);
        meter.charge(1)?;
        let middle = (from + to) / 2.0;
        if along_rings.iter().any(|&[start, end]| middle >= start && middle <= end) {
            continue;
        }
        let point = along.point(middle);
        if !rings.covers(point, meter)? {
            return Ok(false);
        }
        inside |= rings.inside(point, meter)?;
    }
    Ok(inside)
}

#[cfg(test)]
mod tests {
    use stitchcraft_core::Budget;

    use super::*;
    use crate::generators::tatami::fixture::{frame, p, rectangle};
    use crate::normalize::region::Polygon;

    fn cut_by(part: &Polygon, row: &[(f64, f64)]) -> Vec<Vec<(f64, f64)>> {
        let meter = &mut Budget::DEFAULT.meter();
        let rings = Rings::new(part, meter).unwrap();
        let row: Vec<Point> = row.iter().map(|&(x, y)| p(x, y)).collect();
        // To 1e-9 mm: a cut's place along a side is measured, and comes back a hair off.
        let near = |v: f64| (v * 1e9).round() / 1e9;
        cut(&row, &rings, meter).unwrap().iter().map(|line| line.iter().map(|q| (near(q.x()), near(q.y()))).collect()).collect()
    }

    #[test]
    fn req_fill_gdd_006_a_row_is_cut_where_it_crosses_the_rings_and_keeps_its_needle_points() {
        // In and out of a 10 × 4 rectangle twice: 2 lines, each from where the row enters to where it leaves,
        // through its needle points between.
        let part = rectangle(4.0);
        let row = [(-1.0, 1.0), (1.0, 1.0), (1.0, 5.0), (3.0, 5.0), (3.0, 1.0), (6.0, 1.0), (11.0, 1.0)];
        assert_eq!(cut_by(&part, &row), [vec![(0.0, 1.0), (1.0, 1.0), (1.0, 4.0)], vec![(3.0, 4.0), (3.0, 1.0), (6.0, 1.0), (10.0, 1.0)]]);
        // Round the frame's hole: 2 lines.
        assert_eq!(cut_by(&frame(), &[(-1.0, 2.0), (11.0, 2.0)]), [vec![(0.0, 2.0), (4.0, 2.0)], vec![(6.0, 2.0), (10.0, 2.0)]]);
        // Wholly outside, nothing.
        assert!(cut_by(&part, &[(-1.0, -1.0), (11.0, -1.0)]).is_empty());
        // Past the top left corner, over the box of both sides there, meeting neither: nothing.
        assert!(cut_by(&part, &[(-1.0, 3.5), (1.0, 5.5)]).is_empty());
        // Out past that corner and in through the top: one line, from where it enters.
        assert_eq!(cut_by(&part, &[(-1.0, 3.5), (1.0, 5.5), (3.0, 3.0)]), [vec![(2.2, 4.0), (3.0, 3.0)]]);
    }

    #[test]
    fn req_fill_gdd_006_a_touch_from_inside_cuts_and_one_from_outside_keeps_nothing() {
        // Down to the bottom side at (5, 0) and back up, inside: 2 lines that meet there.
        let part = rectangle(4.0);
        assert_eq!(cut_by(&part, &[(3.0, 2.0), (5.0, 0.0), (7.0, 2.0)]), [vec![(3.0, 2.0), (5.0, 0.0)], vec![(5.0, 0.0), (7.0, 2.0)]]);
        // Outside, up to (5, 0) and back down: nothing.
        assert!(cut_by(&part, &[(3.0, -2.0), (5.0, 0.0), (7.0, -2.0)]).is_empty());
    }

    #[test]
    fn req_fill_gdd_006_a_row_along_a_ring_is_no_line_and_is_not_cut_where_it_runs_along_it() {
        // Along the bottom side and out on both ends: GEOS gives Ink/Stitch that stretch as a line, which it
        // doesn't cut at, and the middle of the row lies on the ring, not inside: no line.
        let part = rectangle(4.0);
        assert!(cut_by(&part, &[(-1.0, 0.0), (11.0, 0.0)]).is_empty());
        // In through the left side, along the bottom from (2, 0) to (4, 0), and out through the right side:
        // one line, uncut where it runs along the bottom, which its middle fifth holds.
        let row = [(-1.0, 1.0), (1.0, 1.0), (2.0, 0.0), (4.0, 0.0), (5.0, 1.0), (11.0, 1.0)];
        assert_eq!(cut_by(&part, &row), [vec![(0.0, 1.0), (1.0, 1.0), (2.0, 0.0), (4.0, 0.0), (5.0, 1.0), (10.0, 1.0)]]);
        // Up from below along the left side and in, out through the top: no cut where it joins the side or
        // leaves it, so its first stretch runs from its start, outside, to the top. Its middle fifth lies
        // along the side and then inside, none of it outside, so it is a line, its start outside and all, as
        // shapely 2.2 on GEOS 3.14 keeps it for Ink/Stitch.
        let along_side = [(0.0, -2.0), (0.0, 2.0), (2.0, 3.0), (2.0, 6.0)];
        assert_eq!(cut_by(&part, &along_side), [vec![(0.0, -2.0), (0.0, 2.0), (2.0, 3.0), (2.0, 4.0)]]);
    }

    #[test]
    fn a_stretch_wholly_along_a_ring_in_its_middle_is_not_held() {
        // A row whose middle fifth lies along the bottom side alone: neither outside nor inside, so not held.
        let part = rectangle(4.0);
        let meter = &mut Budget::DEFAULT.meter();
        let rings = Rings::new(&part, meter).unwrap();
        let row = [p(0.0, 0.0), p(10.0, 0.0)];
        let along = Along::new(&row, meter).unwrap();
        assert!(!held(&along, [4.0, 6.0], &[[0.0, 10.0]], &rings, meter).unwrap());
    }

    #[test]
    fn cutting_is_charged_to_the_budget() {
        let part = rectangle(4.0);
        let meter = &mut Budget::DEFAULT.meter();
        let rings = Rings::new(&part, meter).unwrap();
        let row = [p(-1.0, 1.0), p(11.0, 1.0)];
        assert!(cut(&row, &rings, &mut Budget { max_stitches: 1, max_work: 2 }.meter()).is_err());
    }
}
