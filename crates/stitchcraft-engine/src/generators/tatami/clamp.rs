//! Keeping travel inside its part (`REQ-FILL-TAT-005`), as Ink/Stitch keeps it: the way is cut where it
//! meets the part's rings, and where it leaves the part, or runs along a ring, the ring takes its place, the
//! shorter way round from where the way left to where it comes back in.
//!
//! **Why.** Smoothing cuts a way's corners, and a straight way, where no line joins 2 nodes, can cut across
//! a hole or a notch. The needle must not sew outside the region it fills.
//!
//! The way's ends lie on the part's rings, so they count as inside, as do points a rounding error outside
//! them: a way that leaves the part straight from its start follows the ring from its start. A ring taken
//! is the hole the way leaves into, else the outline. Points a rounding error apart are one point
//! ([`SAME`]), as where the way leaves the part and where the ring takes over, worked out apart.

use stitchcraft_core::units::MM_PER_SVG_PX;
use stitchcraft_core::{Exhausted, Meter, Point};

use super::rings::Rings;
use crate::normalize::region::SAME;
use crate::normalize::region::geom::{Hit, hit};
use crate::normalize::stroke::distance_to_segment;

/// Nearer than this, in millimetres, the way's exit and way back in are one point, and no ring is followed
/// between them: 0.02 CSS pixels, where Ink/Stitch's 2 circles of 0.01 px round them meet.
const NEAR: f64 = 0.02 * MM_PER_SVG_PX;

/// A stretch of the way between 2 points where it meets the rings: inside the part, or not (outside it,
/// or along a ring).
#[derive(Clone, Debug, PartialEq)]
struct Stretch {
    points: Vec<Point>,
    inside: bool,
}

/// `way` kept inside the part whose rings are `rings`. Cutting each stitch where it meets the rings
/// ([`Rings::meeting`]), and placing each piece and each end in the part or out of it ([`Rings::inside`],
/// [`Rings::covers`], [`Rings::ring_near`]), cost what those questions cost, and following a ring one unit
/// of `meter` for each of its points.
pub(crate) fn clamp(way: &[Point], rings: &Rings<'_>, meter: &mut Meter) -> Result<Vec<Point>, Exhausted> {
    let (Some(&start), Some(&end)) = (way.first(), way.last()) else { return Ok(Vec::new()) };
    let stretches = cut(way, rings, meter)?;
    if stretches.iter().all(|stretch| stretch.inside) {
        return Ok(way.to_vec());
    }
    // The way's ends count as stretches of their own, inside when the part covers them or they lie on one
    // of its rings.
    let mut ends = |p: Point| -> Result<Stretch, Exhausted> {
        let on = rings.ring_near(p, SAME, |_| true, distance_to_segment, meter)?.is_some();
        Ok(Stretch { points: vec![p], inside: on || rings.covers(p, meter)? })
    };
    let (at_start, at_end) = (ends(start)?, ends(end)?);
    // A stretch inside starts where the one before it ended, unless the way left the part in between: then
    // the ring takes it from where it left (`exit`) to where it comes back.
    let mut kept: Vec<Point> = Vec::new();
    let mut exit: Option<Point> = None;
    for stretch in std::iter::once(at_start).chain(stretches).chain(std::iter::once(at_end)) {
        if !stretch.inside {
            continue;
        }
        let (Some(&first), Some(&last)) = (stretch.points.first(), stretch.points.last()) else { continue };
        if let Some(left) = exit
            && left.distance(first) > NEAR
        {
            extend(&mut kept, round_ring(left, first, rings, meter)?);
        }
        extend(&mut kept, stretch.points);
        exit = Some(last);
    }
    if kept.is_empty() {
        // Nothing of the way lies inside: the outline from the point nearest its start to that nearest its end.
        return round_ring(start, end, rings, meter);
    }
    // A ring gives back the way's end within a rounding error, and travel must meet its rows exactly.
    if let Some(last) = kept.last_mut().filter(|last| last.distance(end) <= SAME) {
        *last = end;
    }
    Ok(kept)
}

/// The way cut where it meets the part's rings, in stretches inside the part or not, one for each piece
/// between 2 cuts. A stretch along a ring is not inside. Costs as [`Rings::meeting`] does for each stitch,
/// and as [`Rings::inside`] does for each piece.
fn cut(way: &[Point], rings: &Rings<'_>, meter: &mut Meter) -> Result<Vec<Stretch>, Exhausted> {
    let mut stretches: Vec<Stretch> = Vec::new();
    for pair in way.windows(2) {
        // A stitch of no length meets nothing.
        let &[p, q] = pair else { continue };
        if p == q {
            continue;
        }
        let mut cuts: Vec<(f64, Point)> = Vec::new();
        rings.meeting(p, q, meter, |side| match hit(p, q, side.a, side.b) {
            Hit::Cross(x) | Hit::Touch(x) => cuts.push((p.distance(x), x)),
            Hit::Overlap(x, y) => cuts.extend([(p.distance(x), x), (p.distance(y), y)]),
            Hit::Apart => {}
        })?;
        cuts.sort_by(|a, b| a.0.total_cmp(&b.0));
        let points: Vec<Point> = std::iter::once(p).chain(cuts.into_iter().map(|(_, x)| x)).chain(std::iter::once(q)).collect();
        for piece in points.windows(2) {
            let &[u, v] = piece else { continue };
            if u == v {
                continue;
            }
            stretches.push(Stretch { points: vec![u, v], inside: rings.inside(u.lerp(v, 0.5), meter)? });
        }
    }
    Ok(stretches)
}

/// The way along a ring from `from` to `to`, the shorter way round: along the hole `from` lies on, else
/// along the outline. One unit of `meter` for each cell and side looked at, and each point of the ring.
fn round_ring(from: Point, to: Point, rings: &Rings<'_>, meter: &mut Meter) -> Result<Vec<Point>, Exhausted> {
    let ring = rings.ring_near(from, SAME, |r| r > 0, distance_to_segment, meter)?.unwrap_or(0);
    let only = |r: usize| r == ring;
    let (Some(a), Some(b)) = (rings.locate(from, only, meter)?, rings.locate(to, only, meter)?) else { return Ok(vec![from, to]) };
    rings.way(ring, a.at, b.at, rings.forwards(ring, a.at, b.at), meter)
}

/// Adds `points` to `kept`, each unless it is where `kept` already ends, within a rounding error.
fn extend(kept: &mut Vec<Point>, points: Vec<Point>) {
    for p in points {
        if kept.last().is_none_or(|last| last.distance(p) > SAME) {
            kept.push(p);
        }
    }
}

#[cfg(test)]
mod tests {
    use stitchcraft_core::Budget;

    use super::*;
    use crate::generators::tatami::fixture::{frame, p, rectangle};
    use crate::normalize::region::Polygon;

    fn clamped(part: &Polygon, way: &[Point]) -> Vec<Point> {
        let meter = &mut Budget::DEFAULT.meter();
        let rings = Rings::new(part, meter).unwrap();
        clamp(way, &rings, meter).unwrap().into_iter().map(|q| p((q.x() * 1e9).round() / 1e9, (q.y() * 1e9).round() / 1e9)).collect()
    }

    #[test]
    fn a_way_inside_stays_as_it_is() {
        // Its end repeated, as smoothing leaves it.
        let way = [p(1.0, 1.0), p(5.0, 3.0), p(9.0, 1.0), p(9.0, 1.0)];
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
        // The other way, and down across it at a slant, round its left side.
        let way = [p(8.0, 1.5), p(2.0, 1.5)];
        assert_eq!(clamped(&frame(), &way), [p(8.0, 1.5), p(6.0, 1.5), p(6.0, 1.0), p(4.0, 1.0), p(4.0, 1.5), p(2.0, 1.5)]);
        let way = [p(5.0, 0.5), p(4.5, 3.5)];
        assert_eq!(clamped(&frame(), &way), [p(5.0, 0.5), p(4.916666667, 1.0), p(4.0, 1.0), p(4.0, 3.0), p(4.583333333, 3.0), p(4.5, 3.5)]);
    }

    #[test]
    fn a_way_that_leaves_where_a_hole_touches_the_outline_follows_the_hole() {
        // The hole's tip touches the top side at (5, 0). The way leaves there into the hole and comes back
        // in through the hole's right side, so the hole, not the outline, takes its place.
        let part = Polygon { holes: vec![vec![p(5.0, 0.0), p(4.0, 2.0), p(6.0, 2.0), p(5.0, 0.0)]], ..rectangle(4.0) };
        let way = [p(3.0, 1.0), p(5.0, 0.0), p(5.0, 1.0), p(7.0, 1.5)];
        assert_eq!(clamped(&part, &way), [p(3.0, 1.0), p(5.0, 0.0), p(5.571428571, 1.142857143), p(7.0, 1.5)]);
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
    fn a_way_that_comes_back_in_beside_where_it_left_follows_the_ring_unless_nearer_than_0_02_px() {
        // Round the top right corner: out at (9.995, 0), back at (10, 0.005), 0.007 mm apart, more than
        // 0.02 px (0.0053 mm): the ring takes the way round the corner.
        let way = [p(9.987, 0.003), p(10.003, -0.003), p(9.997, 0.013)];
        assert_eq!(clamped(&rectangle(4.0), &way), [p(9.987, 0.003), p(9.995, 0.0), p(10.0, 0.0), p(10.0, 0.005), p(9.997, 0.013)]);
        // Out at (9.998, 0) and back at (10, 0.002), 0.003 mm apart: one point, and the way goes straight on.
        let way = [p(9.994, 0.002), p(10.002, -0.002), p(9.998, 0.006)];
        assert_eq!(clamped(&rectangle(4.0), &way), [p(9.994, 0.002), p(9.998, 0.0), p(10.0, 0.002), p(9.998, 0.006)]);
        // Exactly 0.02 px apart is one point too, as Ink/Stitch's circles that touch meet. The way goes up
        // through the top side at x = 10 - 2^-10, round outside the corner, and back in through the right
        // side at a height that puts the 2 points exactly 0.02 px apart.
        let (x, y, d) = (10.0 - 1.0 / 1024.0, 0.005_200_775_114_798_261, 1.0 / 1024.0);
        let way = [p(x, d), p(x, -d), p(10.0 + d, -d), p(10.0 + d, y), p(10.0 - d, y)];
        let meter = &mut Budget::DEFAULT.meter();
        let part = rectangle(4.0);
        let rings = Rings::new(&part, meter).unwrap();
        let clamped = clamp(&way, &rings, meter).unwrap();
        assert_eq!(clamped, [p(x, d), p(x, 0.0), p(10.0, y), p(10.0 - d, y)]);
        assert_eq!(clamped[1].distance(clamped[2]), NEAR);
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
        let clamped = clamp(&way, &rings, meter).unwrap();
        assert_eq!((clamped.first(), clamped.last()), (Some(&p(1.0, 1.0)), Some(&end)), "{clamped:?}");
    }

    #[test]
    fn a_way_wholly_outside_follows_the_outline_between_its_nearest_points() {
        let way = [p(-1.0, 1.0), p(-1.0, 3.0)];
        assert_eq!(clamped(&rectangle(4.0), &way), [p(0.0, 1.0), p(0.0, 3.0)]);
    }

    #[test]
    fn points_a_rounding_error_apart_are_one_point() {
        // 1e-9 mm apart, the region's tolerance, is one point; any farther, 2.
        let mut kept = vec![p(0.0, 0.0)];
        extend(&mut kept, vec![p(0.0, 1e-9), p(0.0, 2e-9)]);
        assert_eq!(kept, [p(0.0, 0.0), p(0.0, 2e-9)]);
    }

    #[test]
    fn every_look_at_the_rings_is_charged() {
        // A stitch inside the rectangle is cut against its 4 sides, and its middle then placed against them: 8
        // sides looked at, a unit of work each.
        let part = rectangle(4.0);
        let rings = Rings::new(&part, &mut Budget::DEFAULT.meter()).unwrap();
        let way = [p(2.0, 1.5), p(8.0, 2.5)];
        let budget = |max_work| Budget { max_stitches: 1, max_work };
        assert_eq!(clamp(&way, &rings, &mut budget(8).meter()).unwrap(), way);
        assert!(clamp(&way, &rings, &mut budget(7).meter()).is_err());
    }

    #[test]
    fn clamping_is_charged_to_the_budget() {
        let part = frame();
        let meter = &mut Budget::DEFAULT.meter();
        let rings = Rings::new(&part, meter).unwrap();
        let way = [p(2.0, 1.5), p(8.0, 1.5)];
        assert!(clamp(&way, &rings, &mut Budget { max_stitches: 1, max_work: 5 }.meter()).is_err());
        assert!(clamp(&way, &rings, &mut Budget { max_stitches: 1, max_work: 12 }.meter()).is_err());
    }
}
