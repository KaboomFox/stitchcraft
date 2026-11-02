//! Polylines simplified by Douglas and Peucker's rule, as GEOS simplifies them for shapely's
//! `simplify` with `preserve_topology` off: a tatami fill's travel measures its distances from its part's
//! rings simplified so, and a contour fill simplifies its rings and its spirals' turns so.
//!
//! A polyline's ends stay, as GEOS keeps a line's ends; a closed one keeps its first point, which is its
//! last, as GEOS keeps a closed line's that is not a ring of a polygon.

use stitchcraft_core::{Exhausted, Meter, Point};

use crate::normalize::stroke::distance_to_segment;

/// `ring` simplified by Douglas and Peucker's rule within `tolerance`: its first and last points stay, and
/// between 2 that stay, the point farthest from the line between them, the first of equally far ones, stays
/// if it lies farther than `tolerance` from it, and so on. One unit of `meter` for each point looked at.
pub(crate) fn simplify(ring: &[Point], tolerance: f64, meter: &mut Meter) -> Result<Vec<Point>, Exhausted> {
    let last = ring.len().saturating_sub(1);
    let mut keep = vec![false; ring.len()];
    for end in [0, last] {
        if let Some(slot) = keep.get_mut(end) {
            *slot = true;
        }
    }
    let mut spans = vec![(0, last)];
    while let Some((i, j)) = spans.pop() {
        let (Some(&a), Some(&b)) = (ring.get(i), ring.get(j)) else { continue };
        let mut farthest: Option<(usize, f64)> = None;
        for (k, &p) in ring.iter().enumerate().take(j).skip(i + 1) {
            meter.charge(1)?;
            let d = distance_to_segment(p, a, b);
            if farthest.is_none_or(|(_, far)| d > far) {
                farthest = Some((k, d));
            }
        }
        if let Some((k, _)) = farthest.filter(|&(_, d)| d > tolerance) {
            if let Some(slot) = keep.get_mut(k) {
                *slot = true;
            }
            spans.push((i, k));
            spans.push((k, j));
        }
    }
    Ok(ring.iter().zip(&keep).filter(|(_, kept)| **kept).map(|(p, _)| *p).collect())
}

#[cfg(test)]
mod tests {
    use stitchcraft_core::Budget;

    use super::*;

    fn p(x: f64, y: f64) -> Point {
        Point::new(x, y).unwrap()
    }

    #[test]
    fn a_ring_is_simplified_by_douglas_and_peucker_s_rule() {
        let meter = &mut Budget::DEFAULT.meter();
        // A bump of 0.4 on a 10 long side stays out of a simplification within 0.5, as does one of exactly 0.5,
        // and one of 0.6 stays in.
        let ring = |bump: f64| vec![p(0.0, 0.0), p(5.0, -bump), p(10.0, 0.0), p(10.0, 4.0), p(0.0, 4.0), p(0.0, 0.0)];
        for bump in [0.4, 0.5] {
            assert_eq!(simplify(&ring(bump), 0.5, meter).unwrap(), [p(0.0, 0.0), p(10.0, 0.0), p(10.0, 4.0), p(0.0, 4.0), p(0.0, 0.0)]);
        }
        assert_eq!(simplify(&ring(0.6), 0.5, meter).unwrap(), ring(0.6));
        assert_eq!(simplify(&[p(1.0, 1.0)], 0.5, meter).unwrap(), [p(1.0, 1.0)]);
        // Of 2 points as far from the line, 1 from the side from (0, 0) to (10, 0), the first stays, and the
        // second then lies 0.28 from the line through it.
        let tie = [p(0.0, 0.0), p(3.0, 1.0), p(5.0, 1.0), p(10.0, 0.0), p(10.0, -5.0), p(0.0, 0.0)];
        assert_eq!(simplify(&tie, 0.5, meter).unwrap(), [p(0.0, 0.0), p(3.0, 1.0), p(10.0, 0.0), p(10.0, -5.0), p(0.0, 0.0)]);
        // Each point between 2 that stay is looked at once for each span it lies in: 4, then 1, 2 and 1.
        let mut counted = Budget::DEFAULT.meter();
        simplify(&ring(0.6), 0.5, &mut counted).unwrap();
        assert_eq!(Budget::DEFAULT.max_work - counted.work_left(), 8);
    }
}
