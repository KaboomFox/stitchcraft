//! A part of a fill grown or shrunk by a distance, with round corners, as GEOS buffers a polygon
//! (`REQ-FILL-TAT-012`).
//!
//! Ink/Stitch sews a fill's underlay on each part shrunk by `fill_underlay_inset_mm`, and its top rows on
//! the part grown by `expand_mm`, with shapely's `buffer`, which is GEOS's. The polygons here are built
//! to be the same, so that a fill sews alike in both, in 2 steps:
//!
//! 1. **The raw curves.** Each ring is offset to its left by the distance, or to its right by a negative
//!    one ([`super::generator`]), with round corners of 16 segments to a quarter circle. An outline runs
//!    clockwise (y up), so its left is outside it, and a hole runs the other way, so its left is inside
//!    the hole: to the left, the part grows. Where the moved segments cross, at a corner the ring turns
//!    away from or where a part too thin for the distance folds over, the curve loops and crosses itself.
//! 2. **The area.** The curves are cut where they meet, and the faces they wind round clockwise at least
//!    once, as the outline winds round the part, are the buffer ([`crate::normalize::region`]). A loop
//!    that folds over winds the other way, or less, and drops out: a corner's crossing, a thin stretch
//!    shrunk away, a hole grown over. What is left falls into parts, each an outline with its holes.
//!
//! GEOS leaves out a ring that it can tell the buffer wears away, before offsetting it, and a small ring's
//! curve that has turned inside out. The engine follows its rules, read in its source and checked against its
//! answers:
//!
//! - **Worn away.** Shrinking wears away a triangle whose inscribed circle is smaller than the distance,
//!   or any other ring whose bounding box is less than twice the distance across. A shrunk outline worn
//!   away leaves nothing of the part, and a grown hole worn away is filled.
//! - **Turned inside out.** The curve of a small ring (fewer than 8 corners) shrunk a little past nothing
//!   crosses itself into a small copy of the ring, turned half round, which winds the same way and would
//!   stay. The curve is left out when none of its points, nor the middle of any of its segments, lies
//!   farther from the ring than 0.99 of the distance, unless it has more than 4 times the ring's points.
//!   Shrunk by less than that past nothing, the copy stays, in GEOS's answers and here.
//!
//! The parts come in order of their point of least x, and of least y among parts level with each other:
//! a run of parts, each with its least x within a nanometre of the one before. GEOS gives them in the
//! order it walks its graph, which this order matches in all but a few cases (`DEV-FILL-008`).
//! A part shrunk away entirely gives no polygon. The answers are checked against shapely's for random
//! polygons (`conformance/oracle/buffer.py`).

use stitchcraft_core::{Exhausted, Meter, Point};

use super::generator::ring_curve;
use crate::normalize::near::{length, to_side};
use crate::normalize::region::geom::{Envelope, signed_area};
use crate::normalize::region::{Polygon, SAME, Unjoined, area};

/// A ring of fewer points than this, its first repeated at its end, is small enough that its curve may
/// turn inside out.
const SMALL_RING: usize = 9;
/// A curve with more than this many points for each of its ring's is not turned inside out.
const CURVE_POINTS: usize = 4;
/// A curve point nearer its ring than this fraction of the distance has not moved out to it.
const NEARNESS: f64 = 0.99;

/// `polygon` grown by `distance`, or shrunk by a negative distance, as GEOS buffers it: the parts left, in
/// order of least x, unless the edges of the area did not join into rings (a bug). No distance leaves the
/// polygon as it is. One unit of `meter` for each point walked, and for the work of cutting the curves and
/// joining the area.
pub(crate) fn buffer(polygon: &Polygon, distance: f64, meter: &mut Meter) -> Result<Result<Vec<Polygon>, Unjoined>, Exhausted> {
    if distance == 0.0 {
        return Ok(Ok(vec![polygon.clone()]));
    }
    let d = distance.abs();
    if distance < 0.0 && worn_away(&polygon.outline, d) {
        return Ok(Ok(Vec::new()));
    }
    let mut curves = Vec::new();
    for (i, ring) in polygon.rings().enumerate() {
        if i > 0 && distance > 0.0 && worn_away(ring, d) {
            continue;
        }
        let curve = ring_curve(ring, distance, meter)?;
        if !inside_out(ring, d, &curve, meter)? {
            curves.push(curve);
        }
    }
    // An outline winds clockwise, -1 round what it bounds, and a part's area winds so.
    let area = area(&curves, |w| w <= -1, meter)?;
    Ok(area.parts.map(in_order))
}

/// `parts` in order of their least x, and of their least y among parts level with each other: a run of
/// parts, each with its least x within [`SAME`] of the one before, is level and comes lowest first, and
/// parts level at the same height keep their order. Comparing 2 parts at a time, as level when their
/// least x lie within `SAME` and by x otherwise, is no order where a run spans more than `SAME`. For least
/// x of 0, 0.6 and 1.2 nm at falling heights, the first comes before the third, the third before the
/// second and the second before the first, and Rust's sort may panic on that.
fn in_order(parts: Vec<Polygon>) -> Vec<Polygon> {
    let mut keyed: Vec<(f64, f64, usize, Polygon)> = parts.into_iter().enumerate().map(|(i, part)| (least(&part), least_y(&part), i, part)).collect();
    keyed.sort_by(|a, b| a.0.total_cmp(&b.0));
    for run in keyed.chunk_by_mut(|a, b| b.0 - a.0 <= SAME) {
        run.sort_by(|a, b| a.1.total_cmp(&b.1).then(a.2.cmp(&b.2)));
    }
    keyed.into_iter().map(|(.., part)| part).collect()
}

/// Whether shrinking the closed `ring` by `d` wears it away, by GEOS's rules: a triangle whose inscribed
/// circle's radius is less than `d`, or another ring whose bounding box is less than twice `d` across.
fn worn_away(ring: &[Point], d: f64) -> bool {
    if let [a, b, c, end] = ring
        && a == end
    {
        let perimeter = length(*a, *b) + length(*b, *c) + length(*c, *a);
        return 2.0 * signed_area(ring).abs() / perimeter < d;
    }
    let env = Envelope::of(ring);
    2.0 * d > (env.max_x - env.min_x).min(env.max_y - env.min_y)
}

/// Whether the `curve` of a small closed `ring` offset by `d` has turned inside out: none of its points,
/// nor the middle of any of its segments, lies farther from the ring than [`NEARNESS`] of `d`. A ring of
/// [`SMALL_RING`] points or more, or a curve of more than [`CURVE_POINTS`] times as many, never has. One
/// unit of `meter` for each point and middle measured.
fn inside_out(ring: &[Point], d: f64, curve: &[Point], meter: &mut Meter) -> Result<bool, Exhausted> {
    if ring.len() <= 3 || ring.len() >= SMALL_RING || curve.len() > CURVE_POINTS * ring.len() {
        return Ok(false);
    }
    let near = NEARNESS * d;
    let middles = curve.windows(2).map(|w| w[0].lerp(w[1], 0.5));
    for p in curve.iter().copied().chain(middles) {
        meter.charge(1)?;
        if ring.windows(2).map(|side| to_side(p, side[0], side[1])).fold(f64::INFINITY, f64::min) > near {
            return Ok(false);
        }
    }
    Ok(true)
}

/// The least x of `polygon`'s outline.
fn least(polygon: &Polygon) -> f64 {
    polygon.outline.iter().map(|p| p.x()).fold(f64::INFINITY, f64::min)
}

/// The least y of the outline's points within [`SAME`] of its least x.
fn least_y(polygon: &Polygon) -> f64 {
    let x = least(polygon);
    polygon.outline.iter().filter(|p| p.x() - x <= SAME).map(|p| p.y()).fold(f64::INFINITY, f64::min)
}

#[cfg(test)]
mod tests {
    use stitchcraft_core::{Budget, SplitMix64};

    use super::*;
    use crate::normalize::fixture::cases;

    /// Shapely's buffers of random polygons (`conformance/fixtures/geometry/shapely-buffer.txt`, written by
    /// `conformance/oracle/buffer.py`).
    const SHAPELY: &str = include_str!("../../../../../conformance/fixtures/geometry/shapely-buffer.txt");

    /// Points this close are the same: GEOS computes crossings in extended precision and arcs with the
    /// platform's sine and cosine, which can differ from the engine's in their last bits.
    const CLOSE: f64 = 1e-9;

    fn close(p: Point, q: Point) -> bool {
        (p.x() - q.x()).abs() <= CLOSE && (p.y() - q.y()).abs() <= CLOSE
    }

    /// A closed ring's points once each, without points within [`CLOSE`] of the one before.
    fn once(ring: &[Point]) -> Vec<Point> {
        let mut kept: Vec<Point> = Vec::new();
        for &p in ring.iter().take(ring.len().saturating_sub(1)) {
            if kept.last().is_none_or(|&q| !close(p, q)) {
                kept.push(p);
            }
        }
        kept
    }

    /// Whether 2 closed rings are the same, whichever point each starts at.
    fn same_ring(a: &[Point], b: &[Point]) -> bool {
        let (a, b) = (once(a), once(b));
        a.len() == b.len() && (0..b.len()).any(|k| a.iter().enumerate().all(|(i, &p)| close(p, b[(i + k) % b.len()])))
    }

    /// Whether 2 polygons are the same: outlines the same, and holes the same in some order.
    fn same(a: &Polygon, b: &Polygon) -> bool {
        same_ring(&a.outline, &b.outline)
            && a.holes.len() == b.holes.len()
            && a.holes.iter().all(|hole| b.holes.iter().any(|other| same_ring(hole, other)))
    }

    #[test]
    fn buffers_are_shapely_s() {
        let (mut differ, mut seen) = (Vec::new(), 0);
        for (number, mut words) in cases(SHAPELY) {
            seen += 1;
            let rings = words.polylines();
            let polygon = Polygon { outline: rings[0].clone(), holes: rings[1..].to_vec() };
            let distance = words.number();
            let count = words.count();
            let expected: Vec<Polygon> = (0..count)
                .map(|_| {
                    let rings = words.polylines();
                    Polygon { outline: rings[0].clone(), holes: rings[1..].to_vec() }
                })
                .collect();
            let found = buffer(&polygon, distance, &mut Budget::DEFAULT.meter()).unwrap().unwrap();
            let agrees = found.len() == expected.len() && found.iter().zip(&expected).all(|(a, b)| same(a, b));
            differ.extend((!agrees).then_some(number));
        }
        assert_eq!(seen, 330, "every case of the fixture is read");
        assert_eq!(differ, Vec::<usize>::new(), "the fixture's polygons whose buffer differs");
    }

    /// The closed ring through `points`.
    fn closed(points: &[(i32, i32)]) -> Vec<Point> {
        points.iter().chain(&points[..1]).map(|&(x, y)| Point::new(f64::from(x), f64::from(y)).unwrap()).collect()
    }

    #[test]
    fn a_triangle_wears_away_past_its_inscribed_circle_and_another_ring_past_half_its_box() {
        // The circle inscribed in a 3-4-5 triangle has a radius of exactly 1, and a 4 × 3 rectangle's box is
        // exactly 3 across: shrunk by exactly 1 and 1.5, neither wears away, and a hair more wears both away.
        let triangle = closed(&[(0, 0), (0, 3), (4, 0)]);
        let rectangle = closed(&[(0, 0), (0, 3), (4, 3), (4, 0)]);
        for (ring, d) in [(&triangle, 1.0), (&rectangle, 1.5)] {
            assert!(!worn_away(ring, d), "{ring:?} by {d}");
            assert!(worn_away(ring, d + 1e-9), "{ring:?} by a hair more than {d}");
        }
    }

    #[test]
    fn a_small_ring_s_curve_is_inside_out_when_none_of_it_lies_farther_than_0_99_of_the_distance() {
        // A square 1,000 across, and a curve 100 from it whose points all lie exactly 99 from the square,
        // 0.99 of the distance: it has turned inside out. A hair farther, it has not, and a curve of more than
        // 4 points for each of the ring's 5 never has.
        let square = closed(&[(0, 0), (0, 1000), (1000, 1000), (1000, 0)]);
        let meter = &mut Budget::DEFAULT.meter();
        let curve = |x: f64, points: usize| vec![Point::new(x, 500.0).unwrap(); points];
        assert!(inside_out(&square, 100.0, &curve(99.0, 20), meter).unwrap());
        assert!(!inside_out(&square, 100.0, &curve(99.0 + 1e-9, 20), meter).unwrap());
        assert!(!inside_out(&square, 100.0, &curve(99.0, 21), meter).unwrap());
    }

    #[test]
    fn pieces_level_at_their_least_x_come_lowest_first() {
        // 2 squares of 8 mm, one above the other, joined by a neck 2 mm wide, listed from the bottom and then
        // from the top. Shrunk by 2 mm, the neck wears away, and the 2 pieces left both reach x = 2: the lower
        // comes first. Shapely gives the lower first for the ring listed from the bottom and the upper for the
        // one listed from the top, in the order its graph takes them (DEV-FILL-008).
        let mut ring = [(0, 0), (0, 8), (3, 8), (3, 12), (0, 12), (0, 20), (8, 20), (8, 12), (5, 12), (5, 8), (8, 8), (8, 0)];
        for _ in 0..2 {
            let pieces = buffer(&Polygon { outline: closed(&ring), holes: Vec::new() }, -2.0, &mut Budget::DEFAULT.meter()).unwrap().unwrap();
            assert_eq!(corners(&pieces), [(2.0, 2.0), (2.0, 14.0)], "{ring:?}");
            ring.rotate_left(5);
        }
    }

    /// Each part's least x, and its least y there.
    fn corners(parts: &[Polygon]) -> Vec<(f64, f64)> {
        parts.iter().map(|part| (least(part), least_y(part))).collect()
    }

    /// A square of side 1 whose lowest left corner is at (`x`, `y`).
    fn square(x: f64, y: f64) -> Polygon {
        let ring = [(x, y), (x, y + 1.0), (x + 1.0, y + 1.0), (x + 1.0, y), (x, y)];
        Polygon { outline: ring.iter().map(|&(x, y)| Point::new(x, y).unwrap()).collect(), holes: Vec::new() }
    }

    #[test]
    fn parts_level_in_a_chain_come_lowest_first_from_any_order() {
        // 30 squares whose least x lie 0.6 nm apart, each lower than the one to its left, and 3 more 1 mm to
        // their right. Compared 2 at a time, as level when their least x lie within SAME and by x otherwise,
        // the 30 go round in circles. In whatever order they come, the 30 are one level run and come lowest
        // first, and then the 3, lowest first too.
        let chain = (0..30).map(|k| square(f64::from(k) * 0.6e-9, -f64::from(k)));
        let parts: Vec<Polygon> = chain.chain((0..3).map(|k| square(1.0 + f64::from(k) * 0.6e-9, f64::from(k)))).collect();
        let expected: Vec<(f64, f64)> = corners(&parts[..30]).into_iter().rev().chain(corners(&parts[30..])).collect();
        let mut rng = SplitMix64::new(1);
        for _ in 0..200 {
            // Shuffled: in the order of a random key for each part.
            let mut keyed: Vec<(u64, Polygon)> = parts.iter().map(|part| (rng.next_u64(), part.clone())).collect();
            keyed.sort_by_key(|&(key, _)| key);
            assert_eq!(corners(&in_order(keyed.into_iter().map(|(_, part)| part).collect())), expected);
        }
    }

    #[test]
    fn parts_are_level_within_same_of_the_one_before_and_keep_their_order_at_one_height() {
        // Least x SAME apart are level, and the lower comes first; 2 SAME apart, the left comes first.
        assert_eq!(corners(&in_order(vec![square(0.0, 0.0), square(SAME, -1.0)])), [(SAME, -1.0), (0.0, 0.0)]);
        assert_eq!(corners(&in_order(vec![square(0.0, 0.0), square(2.0 * SAME, -1.0)])), [(0.0, 0.0), (2.0 * SAME, -1.0)]);
        // Level at the same height, they keep their order, whichever lies farther left.
        assert_eq!(corners(&in_order(vec![square(SAME, 0.0), square(0.0, 0.0)])), [(SAME, 0.0), (0.0, 0.0)]);
    }
}
