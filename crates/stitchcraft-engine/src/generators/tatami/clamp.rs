//! Keeping travel inside its part (`REQ-FILL-TAT-005`), as Ink/Stitch keeps it: the way is cut where it
//! meets the part's rings, and where it leaves the part, or runs along a ring, the ring takes its place, the
//! shorter way round from where the way left to where it comes back in.
//!
//! **Why.** Smoothing cuts a way's corners, and a straight way, where no line joins 2 nodes, can cut across
//! a hole or a notch. The needle must not sew outside the region it fills.
//!
//! The way's ends lie on the part's rings, so they count as inside, as do points a rounding error outside
//! them: a way that leaves the part straight from its start follows the ring from its start. A ring taken
//! is the hole the way leaves into, else the outline.

use stitchcraft_core::units::MM_PER_SVG_PX;
use stitchcraft_core::{Exhausted, Meter, Point};

use super::rings::Rings;
use crate::normalize::region::Polygon;
use crate::normalize::region::geom::{Hit, Location, hit, locate_in_ring};
use crate::normalize::stroke::distance_to_segment;

/// How far apart, in millimetres, a stretch inside may start from where the last one ended before the ring
/// between them is followed: 0.01 CSS pixels, as in Ink/Stitch.
const JOINED: f64 = 0.01 * MM_PER_SVG_PX;
/// Nearer than this, in millimetres, the way's exit and way back in are one point, and no ring is followed
/// between them: 0.02 CSS pixels, where Ink/Stitch's 2 circles of 0.01 px meet.
const NEAR: f64 = 0.02 * MM_PER_SVG_PX;
/// The way's ends count as in the part this near its rings, in millimetres: 1e-9 CSS pixels, by which
/// Ink/Stitch grows the part for the test.
const GROWN: f64 = 1e-9 * MM_PER_SVG_PX;

/// A stretch of the way between 2 points where it meets the rings: inside the part, or not (outside it,
/// or along a ring).
#[derive(Clone, Debug, PartialEq)]
struct Stretch {
    points: Vec<Point>,
    inside: bool,
}

/// `way` kept inside `part`, whose rings are `rings`. One unit of `meter` for each side of a ring each
/// stitch of the way is cut against or each end is looked for on, and for each point of a ring followed.
pub(crate) fn clamp(way: &[Point], part: &Polygon, rings: &Rings<'_>, meter: &mut Meter) -> Result<Vec<Point>, Exhausted> {
    let (Some(&start), Some(&end)) = (way.first(), way.last()) else { return Ok(Vec::new()) };
    let stretches = cut(way, part, meter)?;
    if stretches.iter().all(|stretch| stretch.inside) {
        return Ok(way.to_vec());
    }
    // The way's ends count as stretches of their own, inside when the part covers them or they lie within
    // `GROWN` of its rings.
    let mut ends = |p: Point| -> Result<Stretch, Exhausted> {
        let mut near = false;
        for ring in part.rings() {
            meter.charge(u64::try_from(ring.len()).unwrap_or(u64::MAX))?;
            near |= ring.windows(2).any(|side| side.first().zip(side.get(1)).is_some_and(|(&a, &b)| distance_to_segment(p, a, b) <= GROWN));
        }
        Ok(Stretch { points: vec![p], inside: near || part.covers(p) })
    };
    let (at_start, at_end) = (ends(start)?, ends(end)?);
    let mut kept: Vec<Point> = Vec::new();
    let mut exit: Option<Point> = None;
    let mut was_inside = false;
    for stretch in std::iter::once(at_start).chain(stretches).chain(std::iter::once(at_end)) {
        if !stretch.inside {
            was_inside = false;
            continue;
        }
        let (Some(&first), Some(&last)) = (stretch.points.first(), stretch.points.last()) else { continue };
        if let Some(left) = exit
            && (!was_inside || left.distance(first) > JOINED)
            && left.distance(first) > NEAR
        {
            extend(&mut kept, round_ring(left, first, part, rings, meter)?);
        }
        extend(&mut kept, stretch.points);
        was_inside = true;
        exit = Some(last);
    }
    if kept.is_empty() {
        // Nothing of the way lies inside: the outline from the point nearest its start to that nearest its end.
        return round_ring(start, end, part, rings, meter);
    }
    // A ring gives back the way's end within a rounding error, and travel must meet its rows exactly.
    if let Some(last) = kept.last_mut().filter(|last| last.distance(end) <= JOINED) {
        *last = end;
    }
    Ok(kept)
}

/// The way cut where it meets the part's rings, in stretches inside the part or not. A stretch along a
/// ring is not inside. One unit of `meter` for each side of a ring each stitch is cut against.
fn cut(way: &[Point], part: &Polygon, meter: &mut Meter) -> Result<Vec<Stretch>, Exhausted> {
    let rings: Vec<&[Point]> = part.rings().collect();
    let mut stretches: Vec<Stretch> = Vec::new();
    for pair in way.windows(2) {
        let &[p, q] = pair else { continue };
        let mut cuts: Vec<(f64, Point)> = Vec::new();
        for ring in &rings {
            meter.charge(u64::try_from(ring.len()).unwrap_or(u64::MAX))?;
            for side in ring.windows(2) {
                let &[a, b] = side else { continue };
                match hit(p, q, a, b) {
                    Hit::Cross(x) | Hit::Touch(x) => cuts.push((fraction(p, q, x), x)),
                    Hit::Overlap(x, y) => cuts.extend([(fraction(p, q, x), x), (fraction(p, q, y), y)]),
                    Hit::Apart => {}
                }
            }
        }
        cuts.retain(|&(t, _)| t > 0.0 && t < 1.0);
        cuts.sort_by(|a, b| a.0.total_cmp(&b.0));
        let points: Vec<Point> = std::iter::once(p).chain(cuts.into_iter().map(|(_, x)| x)).chain(std::iter::once(q)).collect();
        for piece in points.windows(2) {
            let &[u, v] = piece else { continue };
            if u == v {
                continue;
            }
            let middle = u.lerp(v, 0.5);
            let inside = !rings.iter().any(|ring| locate_in_ring(middle, ring) == Location::Boundary) && part.covers(middle);
            match stretches.last_mut() {
                Some(last) if last.inside == inside && last.points.last() == Some(&u) => last.points.push(v),
                _ => stretches.push(Stretch { points: vec![u, v], inside }),
            }
        }
    }
    Ok(stretches)
}

/// How far along the segment from `p` to `q` its point `x` lies, from 0 at `p` to 1 at `q`.
fn fraction(p: Point, q: Point, x: Point) -> f64 {
    let (dx, dy) = (q.x() - p.x(), q.y() - p.y());
    let length2 = dx * dx + dy * dy;
    if length2 > 0.0 { ((x.x() - p.x()) * dx + (x.y() - p.y()) * dy) / length2 } else { 0.0 }
}

/// The way along a ring from `from` to `to`, the shorter way round: along the hole `from` lies on, else
/// along the outline. One unit of `meter` for each point of the ring looked at.
fn round_ring(from: Point, to: Point, part: &Polygon, rings: &Rings<'_>, meter: &mut Meter) -> Result<Vec<Point>, Exhausted> {
    let on =
        |ring: &[Point]| ring.windows(2).any(|side| side.first().zip(side.get(1)).is_some_and(|(&a, &b)| distance_to_segment(from, a, b) <= JOINED));
    let ring = part.holes.iter().position(|hole| on(hole)).map_or(0, |hole| hole + 1);
    let only = |r: usize| r == ring;
    let (Some(a), Some(b)) = (rings.locate(from, only, meter)?, rings.locate(to, only, meter)?) else { return Ok(vec![from, to]) };
    rings.way(ring, a.at, b.at, rings.forwards(ring, a.at, b.at), meter)
}

/// Adds `points` to `kept`, each unless it is within `JOINED` of where `kept` already ends: where the way
/// leaves the part and where the ring takes over are worked out apart and can differ in their last bits.
fn extend(kept: &mut Vec<Point>, points: Vec<Point>) {
    for p in points {
        if kept.last().is_none_or(|last| last.distance(p) > JOINED) {
            kept.push(p);
        }
    }
}

#[cfg(test)]
mod tests {
    use stitchcraft_core::Budget;

    use super::*;
    use crate::generators::tatami::fixture::{frame, p, rectangle};

    fn clamped(part: &Polygon, way: &[Point]) -> Vec<Point> {
        let meter = &mut Budget::DEFAULT.meter();
        let rings = Rings::new(part, meter).unwrap();
        clamp(way, part, &rings, meter).unwrap().into_iter().map(|q| p((q.x() * 1e9).round() / 1e9, (q.y() * 1e9).round() / 1e9)).collect()
    }

    #[test]
    fn a_way_inside_stays_as_it_is() {
        let way = [p(1.0, 1.0), p(5.0, 3.0), p(9.0, 1.0)];
        assert_eq!(clamped(&rectangle(4.0), &way), way);
        assert_eq!(clamped(&rectangle(4.0), &[]), Vec::<Point>::new());
    }

    #[test]
    fn a_way_out_of_the_part_follows_the_outline_from_where_it_leaves_to_where_it_comes_back() {
        // Out through the top side at (3, 0) and back in at (7, 0).
        let way = [p(2.0, 1.0), p(5.0, -2.0), p(8.0, 1.0)];
        assert_eq!(clamped(&rectangle(4.0), &way), [p(2.0, 1.0), p(3.0, 0.0), p(7.0, 0.0), p(8.0, 1.0)]);
    }

    #[test]
    fn a_way_across_a_hole_goes_round_it_the_shorter_way() {
        // Across the hole at y = 1.5: round its top, 3 long, not its bottom, 5.
        let way = [p(2.0, 1.5), p(8.0, 1.5)];
        assert_eq!(clamped(&frame(), &way), [p(2.0, 1.5), p(4.0, 1.5), p(4.0, 1.0), p(6.0, 1.0), p(6.0, 1.5), p(8.0, 1.5)]);
    }

    #[test]
    fn a_way_along_a_ring_follows_the_ring() {
        // Down the left side from (0, 1) to (0, 3), then in.
        let way = [p(0.0, 1.0), p(0.0, 3.0), p(2.0, 3.0)];
        assert_eq!(clamped(&rectangle(4.0), &way), [p(0.0, 1.0), p(0.0, 3.0), p(2.0, 3.0)]);
        // Out of the start straight away: the outline from the start to where the way comes back in.
        let way = [p(0.0, 1.0), p(-1.0, 2.0), p(1.0, 3.0)];
        assert_eq!(clamped(&rectangle(4.0), &way), [p(0.0, 1.0), p(0.0, 2.5), p(1.0, 3.0)]);
        // A start worked out on the ring can lie a rounding error outside it, and still counts as on it.
        let way = [p(-1e-12, 1.0), p(-1.0, 2.0), p(1.0, 3.0)];
        assert_eq!(clamped(&rectangle(4.0), &way), [p(0.0, 1.0), p(0.0, 2.5), p(1.0, 3.0)]);
    }

    #[test]
    fn a_way_that_comes_back_along_a_ring_ends_exactly_where_it_ends() {
        // A part with a slanted side, and a way that leaves through it and comes back to a point on it: the
        // ring gives back that point within a rounding error, and the way still ends exactly there.
        let part = Polygon { outline: vec![p(0.0, 0.0), p(0.0, 4.0), p(10.0, 4.0), p(10.0, 3.0), p(0.0, 0.0)], holes: Vec::new() };
        let end = p(0.0, 0.0).lerp(p(10.0, 3.0), 0.7);
        let way = [p(1.0, 1.0), p(5.0, -1.0), end];
        let meter = &mut Budget::DEFAULT.meter();
        let rings = Rings::new(&part, meter).unwrap();
        let clamped = clamp(&way, &part, &rings, meter).unwrap();
        assert_eq!((clamped.first(), clamped.last()), (Some(&p(1.0, 1.0)), Some(&end)), "{clamped:?}");
    }

    #[test]
    fn a_way_wholly_outside_follows_the_outline_between_its_nearest_points() {
        let way = [p(-1.0, 1.0), p(-1.0, 3.0)];
        assert_eq!(clamped(&rectangle(4.0), &way), [p(0.0, 1.0), p(0.0, 3.0)]);
    }

    #[test]
    fn clamping_is_charged_to_the_budget() {
        let part = frame();
        let meter = &mut Budget::DEFAULT.meter();
        let rings = Rings::new(&part, meter).unwrap();
        let way = [p(2.0, 1.5), p(8.0, 1.5)];
        assert!(clamp(&way, &part, &rings, &mut Budget { max_stitches: 1, max_work: 5 }.meter()).is_err());
        assert!(clamp(&way, &part, &rings, &mut Budget { max_stitches: 1, max_work: 12 }.meter()).is_err());
    }
}
