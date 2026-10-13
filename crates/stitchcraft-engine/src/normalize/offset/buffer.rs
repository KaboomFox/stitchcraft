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
//! The parts come in order of their point of least x, and of least y among points level with it. GEOS gives
//! them in the order it walks its graph, which this order matches in all but a few cases (`DEV-FILL-008`).
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
    Ok(area.parts.map(|mut parts| {
        parts.sort_by(|a, b| {
            let (xa, xb) = (least(a), least(b));
            if (xa - xb).abs() <= SAME { least_y(a).total_cmp(&least_y(b)) } else { xa.total_cmp(&xb) }
        });
        parts
    }))
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
    use stitchcraft_core::Budget;

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

    #[test]
    fn req_fill_tat_012_a_part_shrunk_by_exactly_half_its_neck_keeps_both_ends() {
        // 2 squares of 8 mm joined by a neck 1 mm wide, turned by each whole degree, shrunk by 0.5 mm: the
        // neck's 2 sides move onto each other, and their curves cross all along it in rounding. Shapely keeps
        // both squares at every angle, 98.2159 mm² in 1 piece or 2.
        let dumbbell = [
            (0.0, 0.0),
            (0.0, 8.0),
            (8.0, 8.0),
            (8.0, 4.5),
            (12.0, 4.5),
            (12.0, 8.0),
            (20.0, 8.0),
            (20.0, 0.0),
            (12.0, 0.0),
            (12.0, 3.5),
            (8.0, 3.5),
            (8.0, 0.0),
        ];
        for degrees in 0..180 {
            let (sin, cos) = stitchcraft_core::math::sin_cos(f64::from(degrees).to_radians());
            let turn = |(x, y): (f64, f64)| Point::new(x * cos - y * sin, x * sin + y * cos).unwrap();
            let outline: Vec<Point> = dumbbell.iter().chain(&dumbbell[..1]).map(|&q| turn(q)).collect();
            let pieces = buffer(&Polygon { outline, holes: Vec::new() }, -0.5, &mut Budget::DEFAULT.meter()).unwrap().unwrap();
            let area: f64 = pieces.iter().map(Polygon::area).sum();
            assert!((area - 98.215_862_877_363_5).abs() < 1e-8, "{degrees}°: {area} mm² in {} pieces", pieces.len());
            for centre in [turn((4.0, 4.0)), turn((16.0, 4.0))] {
                assert!(pieces.iter().any(|piece| piece.covers(centre)), "{degrees}°: {centre:?}");
            }
        }
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
            let starts: Vec<(f64, f64)> = pieces.iter().map(|piece| (least(piece), least_y(piece))).collect();
            assert_eq!(starts, [(2.0, 2.0), (2.0, 14.0)], "{ring:?}");
            ring.rotate_left(5);
        }
    }
}
