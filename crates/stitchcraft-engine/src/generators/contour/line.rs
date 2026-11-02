//! Polylines measured along, as a contour fill handles its rings: how long one is, where along it a point
//! lies, a polyline cut or started again at a place along it, and the first place along it that comes within
//! a distance of another.
//!
//! **Why measured along.** Ink/Stitch's contour fill works with its rings as lines: it starts each ring at
//! the point of it nearest the needle (shapely's `project`), cuts it where a ring inside it is entered and
//! a row spacing past that (`interpolate`), and enters each ring inside it at the first point along it
//! within a distance of that ring. These are the same questions, answered with the same rules: the
//! nearest of equally near sides is the first along the line, a distance at a point of the line cuts it at
//! that point, and a cut at the start or past the end leaves it whole.
//!
//! **Within a distance.** Ink/Stitch finds where a ring first comes within a distance of another by
//! intersecting it with that ring's buffer, a polygon whose round ends have 16 sides to a quarter circle.
//! Here the distance is measured exactly: a point is within it of a side when it lies in the side's capsule,
//! the union of the discs at its ends and the band along it. Where a ring enters a buffer at a round end, the
//! 2 differ by at most a thousandth of the distance.

use stitchcraft_core::{Exhausted, Meter, Point};

use crate::generators::tatami::sides::Sides;
use crate::normalize::near::nearest_to_point;
use crate::normalize::stroke::distance_to_segment;

/// How long `line` is: the lengths of its sides added up.
pub(super) fn length(line: &[Point]) -> f64 {
    line.windows(2).map(|side| side[0].distance(side[1])).sum()
}

/// How far along `line` its point nearest `p` lies: on the first of the sides as near as any, at the foot
/// of `p` on it, or at its nearer end. 0 for a line of fewer than 2 points.
pub(super) fn project(line: &[Point], p: Point) -> f64 {
    let (mut best, mut at, mut start) = (f64::INFINITY, 0.0, 0.0);
    for side in line.windows(2) {
        let (a, b) = (side[0], side[1]);
        let length = a.distance(b);
        let distance = distance_to_segment(p, a, b);
        if distance < best {
            best = distance;
            at = start + fraction(p, a, b) * length;
        }
        start += length;
    }
    at
}

/// Where the foot of `p` lies along the side from `a` to `b`, as a fraction of it held to 0 and 1: 0 for a
/// side of no length.
fn fraction(p: Point, a: Point, b: Point) -> f64 {
    let (dx, dy) = (b.x() - a.x(), b.y() - a.y());
    let squared = dx * dx + dy * dy;
    if squared == 0.0 {
        return 0.0;
    }
    (((p.x() - a.x()) * dx + (p.y() - a.y()) * dy) / squared).clamp(0.0, 1.0)
}

/// The point `at` along `line`: on the side where the distance falls, the end of a line it is not shorter
/// than, or its start for a distance of 0 or less.
pub(super) fn point_at(line: &[Point], at: f64) -> Option<Point> {
    let first = *line.first()?;
    if at <= 0.0 {
        return Some(first);
    }
    let mut start = 0.0;
    for side in line.windows(2) {
        let (a, b) = (side[0], side[1]);
        let length = a.distance(b);
        if start + length > at {
            return Some(a.lerp(b, (at - start) / length));
        }
        start += length;
    }
    line.last().copied()
}

/// `line` cut in 2 at `at` along it: the part before and the part after, each `None` where it would be
/// empty, as Ink/Stitch cuts a line. A distance at or before the start leaves the whole line after it, and
/// one at or past the end the whole line before it. A distance that reaches a point of the line exactly
/// cuts it at that point; elsewhere the cut adds the point at that distance to both parts.
pub(super) fn cut(line: &[Point], at: f64) -> (Option<Vec<Point>>, Option<Vec<Point>>) {
    if at <= 0.0 {
        return (None, Some(line.to_vec()));
    }
    if at >= length(line) {
        return (Some(line.to_vec()), None);
    }
    let mut traveled = 0.0;
    for (i, side) in line.windows(2).enumerate() {
        traveled += side[1].distance(side[0]);
        // The side ends at point i + 1.
        if traveled == at {
            return (Some(line.iter().take(i + 2).copied().collect()), Some(line.iter().skip(i + 1).copied().collect()));
        }
        if traveled > at {
            let Some(cut) = point_at(line, at) else { break };
            let before = line.iter().take(i + 1).copied().chain(std::iter::once(cut)).collect();
            let after = std::iter::once(cut).chain(line.iter().skip(i + 1).copied()).collect();
            return (Some(before), Some(after));
        }
    }
    (Some(line.to_vec()), None)
}

/// The closed `ring` started again `at` along it, as Ink/Stitch rolls a ring: from the point there round to
/// it. A distance at or before its start, or at or past its end, leaves it as it is.
pub(super) fn roll(ring: &[Point], at: f64) -> Vec<Point> {
    match cut(ring, at) {
        (Some(before), Some(after)) => after.into_iter().chain(before.into_iter().skip(1)).collect(),
        _ => ring.to_vec(),
    }
}

/// How far along the path the stretches `stretches` make, end to end, each after the first starts, as a
/// share of the path's length: where a stretch begins once the needle has stepped to it.
pub(super) fn starts(stretches: &[Vec<Point>]) -> Vec<f64> {
    let (mut at, mut starts, mut end): (f64, Vec<f64>, Option<Point>) = (0.0, Vec::new(), None);
    for stretch in stretches {
        let Some(&first) = stretch.first() else { continue };
        if let Some(end) = end {
            at += end.distance(first);
            starts.push(at);
        }
        at += length(stretch);
        end = stretch.last().copied();
    }
    if at > 0.0 { starts.iter().map(|start| start / at).collect() } else { Vec::new() }
}

/// `line` cut at each of `shares` of its length, in order: the pieces between, each starting where the last
/// ended. A share at or before its start, or at or past its end, cuts nothing.
pub(super) fn cut_at(line: &[Point], shares: &[f64]) -> Vec<Vec<Point>> {
    let total = length(line);
    let mut cuts = shares.iter().map(|share| share * total).filter(|&at| at > 0.0 && at < total).peekable();
    let (mut pieces, mut current, mut start) = (Vec::new(), line.first().copied().into_iter().collect::<Vec<_>>(), 0.0);
    for side in line.windows(2) {
        let (a, b) = (side[0], side[1]);
        let long = a.distance(b);
        while let Some(&at) = cuts.peek() {
            if at >= start + long {
                break;
            }
            let cut = if long > 0.0 { a.lerp(b, ((at - start) / long).max(0.0)) } else { a };
            current.push(cut);
            pieces.push(std::mem::replace(&mut current, vec![cut]));
            cuts.next();
        }
        current.push(b);
        start += long;
    }
    pieces.push(current);
    pieces
}

/// The first point along `line` within `reach` of the polylines `other`, and how far along `line` it lies;
/// `None` when no point of it comes that near. Each side of `line` in turn is measured against the sides of
/// `other` near it, and the first side that comes within `reach` gives the point where it first does. One
/// unit of `meter` for each side of `line` and each side of `other` measured.
pub(super) fn first_within(line: &[Point], other: &Sides<'_>, reach: f64, meter: &mut Meter) -> Result<Option<(f64, Point)>, Exhausted> {
    let mut start = 0.0;
    for side in line.windows(2) {
        meter.charge(1)?;
        let (a, b) = (side[0], side[1]);
        let (x0, y0, x1, y1) = (a.x().min(b.x()) - reach, a.y().min(b.y()) - reach, a.x().max(b.x()) + reach, a.y().max(b.y()) + reach);
        let mut first: Option<f64> = None;
        other.in_box(x0, y0, x1, y1, meter, |near| {
            if let Some(t) = entry(a, b, near.a, near.b, reach) {
                first = Some(first.map_or(t, |f: f64| f.min(t)));
            }
        })?;
        let length = a.distance(b);
        if let Some(t) = first {
            return Ok(Some((start + t * length, a.lerp(b, t))));
        }
        start += length;
    }
    Ok(None)
}

/// The point of the polylines `lines` nearest `p`: the first of equally near ones. One unit of `meter` for
/// each side.
pub(super) fn nearest(lines: &[&[Point]], p: Point, meter: &mut Meter) -> Result<Option<Point>, Exhausted> {
    Ok(nearest_to_point(lines, p, meter)?.map(|(on, _)| on))
}

/// Where along the segment from `a` to `b`, as a fraction of it, it first comes within `reach` of the side
/// from `c` to `d`: the least of where it enters the discs round the side's ends and the band along it.
/// `None` when it never does.
fn entry(a: Point, b: Point, c: Point, d: Point, reach: f64) -> Option<f64> {
    let discs = [disc(a, b, c, reach), disc(a, b, d, reach)];
    discs.into_iter().chain(std::iter::once(band(a, b, c, d, reach))).flatten().reduce(f64::min)
}

/// Where along the segment from `a` to `b`, as a fraction of it, it first comes within `reach` of `centre`.
fn disc(a: Point, b: Point, centre: Point, reach: f64) -> Option<f64> {
    let (ux, uy) = (b.x() - a.x(), b.y() - a.y());
    let (wx, wy) = (a.x() - centre.x(), a.y() - centre.y());
    let c = wx * wx + wy * wy - reach * reach;
    if c <= 0.0 {
        return Some(0.0);
    }
    let qa = ux * ux + uy * uy;
    let qb = ux * wx + uy * wy;
    let discriminant = qb * qb - qa * c;
    if qa == 0.0 || discriminant < 0.0 {
        return None;
    }
    let t = (-qb - discriminant.sqrt()) / qa;
    (0.0..=1.0).contains(&t).then_some(t)
}

/// Where along the segment from `a` to `b`, as a fraction of it, it first enters the band along the side
/// from `c` to `d`: the points whose foot on the side's line falls on the side, no farther than `reach`
/// from it. `None` for a side of no length, or a segment that misses the band.
fn band(a: Point, b: Point, c: Point, d: Point, reach: f64) -> Option<f64> {
    let (vx, vy) = (d.x() - c.x(), d.y() - c.y());
    let length = (vx * vx + vy * vy).sqrt();
    if length == 0.0 {
        return None;
    }
    // Along the side, as a fraction of it, and across it, in units of length, at a and per unit of t.
    let (wx, wy) = (a.x() - c.x(), a.y() - c.y());
    let (ux, uy) = (b.x() - a.x(), b.y() - a.y());
    let along = ((wx * vx + wy * vy) / (length * length), (ux * vx + uy * vy) / (length * length));
    let across = ((vx * wy - vy * wx) / length, (vx * uy - vy * ux) / length);
    let (mut low, mut high) = (0.0_f64, 1.0_f64);
    for (value, rate, least, most) in [(along.0, along.1, 0.0, 1.0), (across.0, across.1, -reach, reach)] {
        if rate == 0.0 {
            if value < least || value > most {
                return None;
            }
            continue;
        }
        let (t0, t1) = ((least - value) / rate, (most - value) / rate);
        low = low.max(t0.min(t1));
        high = high.min(t0.max(t1));
    }
    (low <= high).then_some(low)
}

#[cfg(test)]
mod tests {
    use stitchcraft_core::Budget;

    use super::*;

    fn p(x: f64, y: f64) -> Point {
        Point::new(x, y).unwrap()
    }

    fn square() -> Vec<Point> {
        vec![p(0.0, 0.0), p(4.0, 0.0), p(4.0, 4.0), p(0.0, 4.0), p(0.0, 0.0)]
    }

    #[test]
    fn a_point_projects_to_the_first_of_the_sides_as_near_as_any() {
        let ring = square();
        assert_eq!(length(&ring), 16.0);
        assert_eq!(project(&ring, p(1.0, -1.0)), 1.0);
        assert_eq!(project(&ring, p(5.0, 2.0)), 6.0);
        // (2, 2) is as near every side: the first's foot is 2 along.
        assert_eq!(project(&ring, p(2.0, 2.0)), 2.0);
        // Past a corner the foot is the corner.
        assert_eq!(project(&ring, p(5.0, -1.0)), 4.0);
        assert_eq!(project(&[p(1.0, 1.0)], p(0.0, 0.0)), 0.0);
        // A side of no length is measured from its point, at a fraction of 0.
        assert_eq!(fraction(p(3.0, 1.0), p(2.0, 2.0), p(2.0, 2.0)), 0.0);
        assert_eq!(project(&[p(0.0, 0.0), p(0.0, 0.0), p(4.0, 0.0)], p(-1.0, 0.0)), 0.0);
    }

    #[test]
    fn a_line_cut_at_a_point_of_it_is_cut_there_and_elsewhere_gains_the_cut() {
        let ring = square();
        assert_eq!(cut(&ring, 0.0), (None, Some(ring.clone())));
        assert_eq!(cut(&ring, 16.0), (Some(ring.clone()), None));
        assert_eq!(cut(&ring, 4.0), (Some(vec![p(0.0, 0.0), p(4.0, 0.0)]), Some(vec![p(4.0, 0.0), p(4.0, 4.0), p(0.0, 4.0), p(0.0, 0.0)])));
        let (before, after) = cut(&ring, 5.0);
        assert_eq!(before, Some(vec![p(0.0, 0.0), p(4.0, 0.0), p(4.0, 1.0)]));
        assert_eq!(after, Some(vec![p(4.0, 1.0), p(4.0, 4.0), p(0.0, 4.0), p(0.0, 0.0)]));
        assert_eq!(point_at(&ring, 9.0), Some(p(3.0, 4.0)));
        assert_eq!(point_at(&ring, 20.0), Some(p(0.0, 0.0)));
        assert_eq!(point_at(&[], 1.0), None);
        // A distance that is no number lies neither before the start nor along the line: it cuts nothing off.
        assert_eq!(cut(&ring, f64::NAN), (Some(ring.clone()), None));
    }

    #[test]
    fn a_ring_rolled_starts_at_the_point_that_far_along() {
        let ring = square();
        assert_eq!(roll(&ring, 5.0), [p(4.0, 1.0), p(4.0, 4.0), p(0.0, 4.0), p(0.0, 0.0), p(4.0, 0.0), p(4.0, 1.0)]);
        assert_eq!(roll(&ring, 4.0), [p(4.0, 0.0), p(4.0, 4.0), p(0.0, 4.0), p(0.0, 0.0), p(4.0, 0.0)]);
        assert_eq!(roll(&ring, 0.0), ring);
        assert_eq!(roll(&ring, 16.0), ring);
    }

    #[test]
    fn a_line_first_comes_within_reach_where_it_enters_a_capsule() {
        let meter = &mut Budget::DEFAULT.meter();
        // A side from (10, 0) to (10, 4), reached 1 away: a line along y = 2 enters its band at x = 9, and one
        // along y = 5 its upper disc, at x = 10.
        let other = [p(10.0, 0.0), p(10.0, 4.0)];
        let sides = Sides::new(vec![&other], meter).unwrap();
        assert_eq!(first_within(&[p(0.0, 2.0), p(20.0, 2.0)], &sides, 1.0, meter).unwrap(), Some((9.0, p(9.0, 2.0))));
        assert_eq!(first_within(&[p(0.0, 5.0), p(20.0, 5.0)], &sides, 1.0, meter).unwrap(), Some((10.0, p(10.0, 5.0))));
        // A line that passes 1.5 away never reaches it, and one that starts within reach does at its start.
        assert_eq!(first_within(&[p(0.0, 5.5), p(20.0, 5.5)], &sides, 1.0, meter).unwrap(), None);
        assert_eq!(first_within(&[p(10.5, 2.0), p(20.0, 2.0)], &sides, 1.0, meter).unwrap(), Some((0.0, p(10.5, 2.0))));
        // The second side of a line, when its first misses: down x = 9.5, it enters the upper disc where
        // (y - 4)² = 0.75, 9.5 along the first side and 10 - 4.866 down the second.
        let bent = [p(0.0, 10.0), p(9.5, 10.0), p(9.5, 0.0)];
        let (at, point) = first_within(&bent, &sides, 1.0, meter).unwrap().unwrap();
        let y = 4.0 + 0.75_f64.sqrt();
        assert!((at - (9.5 + 10.0 - y)).abs() < 1e-12 && point.distance(p(9.5, y)) < 1e-12, "{at} {point:?}");
        assert_eq!(nearest(&[&other], p(12.0, 9.0), meter).unwrap(), Some(p(10.0, 4.0)));
        // A unit of work for the line's first side, and none left for the side of the other near it.
        let short = &mut Budget { max_stitches: 1, max_work: 1 }.meter();
        assert!(first_within(&[p(0.0, 2.0), p(20.0, 2.0)], &sides, 1.0, short).is_err());
    }

    #[test]
    fn a_path_is_cut_again_where_its_stretches_met() {
        // Stretches of 4 and 2 with a step of 2 between them: the second starts 6 along a path of 8.
        let stretches = [vec![p(0.0, 0.0), p(4.0, 0.0)], vec![p(4.0, 2.0), p(6.0, 2.0)]];
        assert_eq!(starts(&stretches), [0.75]);
        assert_eq!(starts(&[vec![p(1.0, 1.0)]]), Vec::<f64>::new());
        let line = [p(0.0, 0.0), p(4.0, 0.0), p(4.0, 4.0)];
        assert_eq!(cut_at(&line, &[0.75]), [vec![p(0.0, 0.0), p(4.0, 0.0), p(4.0, 2.0)], vec![p(4.0, 2.0), p(4.0, 4.0)]]);
        // Cuts at a point of the line, at its ends, and none.
        assert_eq!(cut_at(&line, &[0.5]), [vec![p(0.0, 0.0), p(4.0, 0.0), p(4.0, 0.0)], vec![p(4.0, 0.0), p(4.0, 4.0)]]);
        assert_eq!(cut_at(&line, &[0.0, 1.0]), [line.to_vec()]);
        assert_eq!(cut_at(&line, &[]), [line.to_vec()]);
    }

    #[test]
    fn a_segment_enters_a_capsule_at_its_discs_or_its_band() {
        let (c, d) = (p(0.0, 0.0), p(4.0, 0.0));
        // Straight down onto the band, onto a disc, past both, along the side's line, and a side of no length.
        assert_eq!(entry(p(2.0, 3.0), p(2.0, -3.0), c, d, 1.0), Some(2.0 / 6.0));
        assert_eq!(entry(p(-3.0, 0.0), p(-1.0, 0.0), c, d, 1.0), Some(1.0));
        assert_eq!(entry(p(-3.0, 2.0), p(7.0, 2.0), c, d, 1.0), None);
        assert_eq!(band(p(-3.0, 0.0), p(7.0, 0.0), c, d, 1.0), Some(0.3));
        assert_eq!(band(p(-3.0, 2.0), p(7.0, 2.0), c, c, 1.0), None);
        assert_eq!(disc(p(1.0, 1.0), p(1.0, 1.0), c, 1.0), None);
    }
}
