//! Paths smoothed as Ink/Stitch smooths them (its `smooth_path`): a tatami fill's travel between rows, by a
//! smoothness of 2 pixels, and a contour fill's path from its innermost rings out, by `smoothness_mm`.
//!
//! **Why stitches first.** Cutting corners smooths a path best whose sides are about as long as each
//! other: a long side among short ones pulls the curve away from the path, most at its ends. So the path is
//! first cut into running stitches 5 times the smoothness long, within twice the smoothness of it, and the
//! corners of those are cut.

use stitchcraft_core::{Exhausted, Meter, Point};

use crate::generators::running::along_line;

/// How long the stitches a path is cut into are, and how far they may stray from it, in smoothnesses.
const LENGTH: f64 = 5.0;
const TOLERANCE: f64 = 2.0;
/// How many times a path's corners are cut.
const SMOOTHING_ROUNDS: usize = 5;

/// `way` smoothed as Ink/Stitch smooths a path by `smoothness`: cut into stitches 5 times `smoothness` long,
/// within twice it of the way and none shorter than `min_stitch`, whose needle points after the first have
/// their corners cut 5 times (a quarter of the way along each side from each end, Chaikin's rule), and its
/// own ends added back at either end. So the way leaves its start straight, to the first point after it,
/// and curves from there. One unit of `meter` for each point made.
pub(crate) fn smooth(way: &[Point], smoothness: f64, min_stitch: f64, meter: &mut Meter) -> Result<Vec<Point>, Exhausted> {
    let (Some(&first), Some(&last)) = (way.first(), way.last()) else { return Ok(Vec::new()) };
    let mut points: Vec<Point> = along_line(way, LENGTH * smoothness, TOLERANCE * smoothness, min_stitch, meter)?.into_iter().skip(1).collect();
    for _ in 0..SMOOTHING_ROUNDS {
        let mut cut = Vec::with_capacity(2 * points.len());
        cut.extend(points.first().copied());
        for pair in points.windows(2) {
            let &[p, q] = pair else { continue };
            meter.charge(2)?;
            cut.push(p.lerp(q, 0.25));
            cut.push(p.lerp(q, 0.75));
        }
        cut.extend(points.last().copied());
        points = cut;
    }
    Ok(std::iter::once(first).chain(points).chain(std::iter::once(last)).collect())
}

#[cfg(test)]
mod tests {
    use stitchcraft_core::Budget;
    use stitchcraft_core::math::sin_cos;
    use stitchcraft_core::units::MM_PER_SVG_PX;

    use super::*;

    /// A tatami fill's travel's smoothness, 2 pixels.
    const TRAVEL: f64 = 2.0 * MM_PER_SVG_PX;

    fn p(x: f64, y: f64) -> Point {
        Point::new(x, y).unwrap()
    }

    #[test]
    fn a_way_is_smoothed_by_cutting_its_corners_keeping_its_ends() {
        let meter = &mut Budget::DEFAULT.meter();
        let way = [p(0.0, 0.0), p(4.0, 0.0), p(4.0, 4.0)];
        let smoothed = smooth(&way, TRAVEL, 0.1, meter).unwrap();
        assert_eq!((smoothed.first(), smoothed.last()), (Some(&p(0.0, 0.0)), Some(&p(4.0, 4.0))));
        // Cut into stitches of 10 px (2.6 mm), the first side takes 2 of 2 mm: the way leaves its start
        // straight to the first needle point after it, and curves from there.
        assert_eq!(smoothed.get(1), Some(&p(2.0, 0.0)));
        // The corner is cut: no point comes within 0.2 mm of it, and the points stay within the way's box.
        assert!(smoothed.iter().all(|q| q.distance(p(4.0, 0.0)) > 0.2), "{smoothed:?}");
        assert!(smoothed.iter().all(|q| q.x() >= -1e-9 && q.x() <= 4.0 + 1e-9 && q.y() >= -1e-9 && q.y() <= 4.0 + 1e-9));
        assert!(smooth(&[], TRAVEL, 0.1, meter).unwrap().is_empty());
    }

    #[test]
    fn a_way_is_cut_within_4_px_of_it_before_it_is_smoothed() {
        // A hairpin 0.34 mm wide. Stitches of 10 px across its turn would stray 1.15 mm from it, more than
        // 4 px (1.06 mm), so a point goes on the turn, and the smoothed way passes within 0.3 mm of it.
        let meter = &mut Budget::DEFAULT.meter();
        let r = 0.169;
        let mut way = vec![p(0.0, 3.4845)];
        for i in 0..=12 {
            let (sin, cos) = sin_cos(std::f64::consts::PI * f64::from(i) / 12.0);
            way.push(p(r - r * cos, -r * sin));
        }
        way.push(p(2.0 * r, 3.4845));
        let turn = p(r, -r);
        let nearest = smooth(&way, TRAVEL, 0.3, meter).unwrap().iter().map(|q| q.distance(turn)).fold(f64::INFINITY, f64::min);
        assert!(nearest < 0.5, "{nearest}");
    }
}
