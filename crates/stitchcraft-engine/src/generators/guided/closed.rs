//! Rows that close on themselves: what the parallel offset and buffer strategies do with them, as shapely
//! does it for Ink/Stitch (read at `d59c9ab`, `lib/utils/geometry.py` and `lib/stitches/guided_fill.py`).
//!
//! **Why.** An offset of a closed guide, and every ring round a buffered one, is a row with no ends. Where
//! it starts decides where its needle points start, and Ink/Stitch moves the start of such a row, and of
//! rows joined into it, along it: to where an anchor line crosses it, or out of the part, so that the cut by
//! the part doesn't split a line where the row closes. These are the tools for that, each charged to the
//! meter for every point it walks.
//!
//! - **Rolled**: a closed row started at a point along it, its points round from there and back to it.
//!   A row that doesn't close is closed first, as shapely closes a line it makes a ring of.
//! - **Counter-clockwise**: a ring running counter-clockwise as the drawing's coordinates read with y up,
//!   as shapely's `is_ccw` tells, its start kept.
//! - **Covered**: whether a ring lies in another, none of it outside.
//! - **Outside**: the middle of the first stretch of a row outside an area, along the row from its start,
//!   and **inside**: the stretches of a row in an area or on its edge.
//! - **Segmentized**: a line with each side longer than a length cut into equal parts, as shapely's
//!   `segmentize` cuts them.

use stitchcraft_core::{Exhausted, Meter, Point};

use crate::generators::tatami::rings::Rings;
use crate::normalize::along::Along;
use crate::normalize::region::geom::{Hit, hit, signed_area};

/// Whether `line` closes on itself: its last point its first, with a side between.
pub(super) fn closed(line: &[Point]) -> bool {
    line.len() > 2 && line.first() == line.last()
}

/// `line`, closed: its first point added at its end when it isn't there.
pub(super) fn closed_up(line: &[Point]) -> Vec<Point> {
    let mut ring = line.to_vec();
    if !closed(line)
        && let Some(&first) = line.first()
    {
        ring.push(first);
    }
    ring
}

/// Whether the closed `ring` runs counter-clockwise in the drawing's coordinates read with y up, as shapely's
/// `is_ccw` tells: its signed area is above 0.
pub(super) fn counter_clockwise(ring: &[Point]) -> bool {
    signed_area(ring) > 0.0
}

/// The closed `ring` running counter-clockwise ([`counter_clockwise`]), turned round if it doesn't. Its first
/// point, which is also its last, stays first.
pub(super) fn made_counter_clockwise(mut ring: Vec<Point>) -> Vec<Point> {
    if !counter_clockwise(&ring) {
        ring.reverse();
    }
    ring
}

/// The closed `ring` started at the point `at` along it: that point, its points round from there to its own
/// start and on, and the point again. A point at one of its own points starts there, and `at` at its start
/// or past its end leaves it as it is, as shapely's `roll_linear_ring` does. One unit of `meter`'s work for
/// each point.
pub(super) fn rolled(ring: &[Point], at: f64, meter: &mut Meter) -> Result<Vec<Point>, Exhausted> {
    let along = Along::new(ring, meter)?;
    if at <= 0.0 || at >= along.length() {
        return Ok(ring.to_vec());
    }
    // The first point at or past `at`: the cut lies on the side that ends there.
    let next = along.at.partition_point(|&a| a < at);
    let tail = ring.get(next..).unwrap_or_default();
    let head = ring.get(1..next).unwrap_or_default();
    if along.at.get(next) == Some(&at) {
        // At one of its points: from that point round to it again.
        return Ok(tail.iter().chain(head).chain(ring.get(next)).copied().collect());
    }
    let point = along.point(at);
    Ok(std::iter::once(point).chain(tail.iter().copied()).chain(head.iter().copied()).chain(std::iter::once(point)).collect())
}

/// Whether the area that `outer` bounds covers the closed `ring`: none of it lies outside. Each of the ring's
/// points and the middle of each of its sides lies in the area or on its edge, and no side crosses the
/// edge. Costs as [`Rings::covers`] and [`Rings::meeting`] do.
pub(super) fn covered(outer: &Rings<'_>, ring: &[Point], meter: &mut Meter) -> Result<bool, Exhausted> {
    for side in ring.windows(2) {
        meter.charge(1)?;
        let (p, q) = (side[0], side[1]);
        if !outer.covers(p, meter)? || !outer.covers(p.lerp(q, 0.5), meter)? {
            return Ok(false);
        }
        let mut crosses = false;
        outer.meeting(p, q, meter, |edge| crosses |= matches!(hit(p, q, edge.a, edge.b), Hit::Cross(x) if x != p && x != q))?;
        if crosses {
            return Ok(false);
        }
    }
    Ok(true)
}

/// How far along `line` the middle of its first stretch outside the area `rings` bound lies, as
/// [`stretches`] finds them; `None` when no stretch is outside.
pub(super) fn outside(line: &[Point], rings: &Rings<'_>, meter: &mut Meter) -> Result<Option<f64>, Exhausted> {
    Ok(stretches(line, rings, meter)?.into_iter().find(|stretch| !stretch.covered).map(|stretch| (stretch.from + stretch.to) / 2.0))
}

/// The parts of `line` in the area `rings` bound or on its edge, as [`stretches`] finds them, in order
/// along it, as shapely's intersection of a line with a polygon gives them.
pub(super) fn inside(line: &[Point], rings: &Rings<'_>, meter: &mut Meter) -> Result<Vec<Vec<Point>>, Exhausted> {
    let along = Along::new(line, meter)?;
    Ok(stretches(line, rings, meter)?.into_iter().filter(|stretch| stretch.covered).map(|stretch| along.part(stretch.from, stretch.to)).collect())
}

/// A stretch of a line between 2 points where it meets an area's edge: where it starts and ends, as
/// distances along the line, and whether the area covers it.
struct Stretch {
    from: f64,
    to: f64,
    covered: bool,
}

/// The stretches of `line` between the points where it crosses or touches a ring of `rings`, or starts or
/// stops running along one, each of some length. One is covered when its middle lies in the area or on its
/// edge. Costs as [`Rings::meeting`] and [`Rings::covers`] do.
fn stretches(line: &[Point], rings: &Rings<'_>, meter: &mut Meter) -> Result<Vec<Stretch>, Exhausted> {
    let along = Along::new(line, meter)?;
    let mut cuts = vec![0.0];
    for (side, &start) in line.windows(2).zip(&along.at) {
        meter.charge(1)?;
        let (p, q) = (side[0], side[1]);
        let mut met = Vec::new();
        rings.meeting(p, q, meter, |edge| match hit(p, q, edge.a, edge.b) {
            Hit::Cross(x) | Hit::Touch(x) => met.push(start + p.distance(x)),
            Hit::Overlap(x, y) => met.extend([start + p.distance(x), start + p.distance(y)]),
            Hit::Apart => {}
        })?;
        met.sort_by(f64::total_cmp);
        cuts.extend(met);
    }
    cuts.push(along.length());
    let mut found = Vec::new();
    for pair in cuts.windows(2) {
        meter.charge(1)?;
        let (from, to) = (pair[0], pair[1]);
        if to > from {
            found.push(Stretch { from, to, covered: rings.covers(along.point((from + to) / 2.0), meter)? });
        }
    }
    Ok(found)
}

/// `line` with each side longer than `longest` cut into as few equal parts as are no longer than it, as
/// shapely's `segmentize` cuts it. One unit of `meter`'s work for each point added and each side.
pub(super) fn segmentized(line: &[Point], longest: f64, meter: &mut Meter) -> Result<Vec<Point>, Exhausted> {
    let mut points: Vec<Point> = line.first().copied().into_iter().collect();
    for side in line.windows(2) {
        meter.charge(1)?;
        let (p, q) = (side[0], side[1]);
        let length = p.distance(q);
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)] // A count of parts held to the meter.
        let parts = if longest > 0.0 && length > longest { (length / longest).ceil() as u64 } else { 1 };
        for k in 1..parts {
            meter.charge(1)?;
            #[allow(clippy::cast_precision_loss)] // Counts far below 2⁵².
            points.push(p.lerp(q, k as f64 / parts as f64));
        }
        points.push(q);
    }
    Ok(points)
}

#[cfg(test)]
mod tests {
    use stitchcraft_core::Budget;

    use super::*;
    use crate::generators::tatami::fixture::{p, rectangle};

    fn xy(points: &[Point]) -> Vec<(f64, f64)> {
        points.iter().map(|q| ((q.x() * 1e9).round() / 1e9, (q.y() * 1e9).round() / 1e9)).collect()
    }

    #[test]
    fn a_ring_is_rolled_to_a_point_along_it_or_one_of_its_own() {
        let meter = &mut Budget::DEFAULT.meter();
        let square = [p(0.0, 0.0), p(4.0, 0.0), p(4.0, 4.0), p(0.0, 4.0), p(0.0, 0.0)];
        // Half way along its second side: that point, round, and back to it.
        let rolled_square = rolled(&square, 6.0, meter).unwrap();
        assert_eq!(xy(&rolled_square), [(4.0, 2.0), (4.0, 4.0), (0.0, 4.0), (0.0, 0.0), (4.0, 0.0), (4.0, 2.0)]);
        // At its third point: from there.
        assert_eq!(xy(&rolled(&square, 8.0, meter).unwrap()), [(4.0, 4.0), (0.0, 4.0), (0.0, 0.0), (4.0, 0.0), (4.0, 4.0)]);
        // At its start or past its end: as it is.
        assert_eq!(rolled(&square, 0.0, meter).unwrap(), square);
        assert_eq!(rolled(&square, 16.0, meter).unwrap(), square);
        assert!(rolled(&square, 6.0, &mut Budget { max_stitches: 1, max_work: 2 }.meter()).is_err());
    }

    #[test]
    fn a_line_is_closed_and_a_ring_turned_counter_clockwise_keeps_its_start() {
        let open = [p(0.0, 0.0), p(4.0, 0.0), p(4.0, 4.0)];
        assert!(!closed(&open) && !closed(&open[..1]));
        let ring = closed_up(&open);
        assert_eq!(xy(&ring), [(0.0, 0.0), (4.0, 0.0), (4.0, 4.0), (0.0, 0.0)]);
        assert_eq!(closed_up(&ring), ring);
        // Counter-clockwise with y up already, and the other way round.
        assert!(counter_clockwise(&ring));
        let reversed: Vec<Point> = ring.iter().rev().copied().collect();
        assert!(!counter_clockwise(&reversed));
        assert_eq!(made_counter_clockwise(reversed.clone()), ring);
        assert_eq!(made_counter_clockwise(ring.clone()), ring);
        assert_eq!(made_counter_clockwise(reversed).first(), ring.first());
    }

    #[test]
    fn a_ring_is_covered_when_none_of_it_lies_outside() {
        let meter = &mut Budget::DEFAULT.meter();
        let part = rectangle(4.0);
        let rings = Rings::new(&part, meter).unwrap();
        let inside = [p(1.0, 1.0), p(3.0, 1.0), p(3.0, 3.0), p(1.0, 1.0)];
        assert!(covered(&rings, &inside, meter).unwrap());
        // Along the part's edge, still covered; out past its top side, not.
        let along = [p(0.0, 0.0), p(10.0, 0.0), p(10.0, 2.0), p(0.0, 0.0)];
        assert!(covered(&rings, &along, meter).unwrap());
        let out = [p(1.0, 1.0), p(3.0, 1.0), p(3.0, 5.0), p(1.0, 1.0)];
        assert!(!covered(&rings, &out, meter).unwrap());
        // A side that leaves and comes back between its ends: its middle lies outside.
        let bent = [p(-1.0, 2.0), p(11.0, 2.0)];
        assert!(!covered(&rings, &bent, meter).unwrap());
        // Its ends on the edge and its middle inside, but crossing the edge on the way: a corner cut off.
        let corner = [p(0.0, 3.0), p(2.0, 5.0), p(4.0, 3.0)];
        assert!(!covered(&rings, &corner, meter).unwrap());
    }

    #[test]
    fn the_first_stretch_outside_is_found_from_the_line_s_start() {
        let meter = &mut Budget::DEFAULT.meter();
        let part = rectangle(4.0);
        let rings = Rings::new(&part, meter).unwrap();
        // In, out through the top side between 5 and 7 along it, in, and out through the right side.
        let line = [p(1.0, 2.0), p(3.0, 2.0), p(3.0, 6.0), p(5.0, 6.0), p(5.0, 2.0), p(12.0, 2.0)];
        assert_eq!(outside(&line, &rings, meter).unwrap(), Some(7.0));
        // Wholly inside, or only along an edge: none.
        assert_eq!(outside(&[p(1.0, 1.0), p(3.0, 1.0)], &rings, meter).unwrap(), None);
        assert_eq!(outside(&[p(0.0, 0.0), p(10.0, 0.0)], &rings, meter).unwrap(), None);
        // The parts inside: up to the top side, and from it on to the right side.
        let parts: Vec<_> = inside(&line, &rings, meter).unwrap().iter().map(|part| xy(part)).collect();
        assert_eq!(parts, [vec![(1.0, 2.0), (3.0, 2.0), (3.0, 4.0)], vec![(5.0, 4.0), (5.0, 2.0), (10.0, 2.0)]]);
        assert!(inside(&[p(-1.0, -1.0), p(-1.0, 5.0)], &rings, meter).unwrap().is_empty());
    }

    #[test]
    fn long_sides_are_cut_into_equal_parts() {
        let meter = &mut Budget::DEFAULT.meter();
        let line = [p(0.0, 0.0), p(3.0, 0.0), p(3.0, 1.0)];
        assert_eq!(xy(&segmentized(&line, 1.0, meter).unwrap()), [(0.0, 0.0), (1.0, 0.0), (2.0, 0.0), (3.0, 0.0), (3.0, 1.0)]);
        assert_eq!(xy(&segmentized(&line, 1.4, meter).unwrap()), [(0.0, 0.0), (1.0, 0.0), (2.0, 0.0), (3.0, 0.0), (3.0, 1.0)]);
        assert_eq!(xy(&segmentized(&line, 5.0, meter).unwrap()), xy(&line));
        assert!(segmentized(&[], 1.0, meter).unwrap().is_empty());
    }
}
