//! Distances along a polyline: where each of its points lies, measured along it from its start, the point
//! at any distance, and the distance of the point nearest a given one.
//!
//! Running stitches are placed by distance along a stroke, and a satin column's stitches by distance
//! along the sections of its rails between rungs, so both measure their polylines here. Distances are sums
//! of point-to-point distances, square roots and arithmetic only, so every platform measures the same
//! (`docs/src/design/determinism.md`).

use stitchcraft_core::{Exhausted, Meter, Point};

use crate::normalize::stroke::nearest_on_segment;

/// A polyline, with the distance along it to each of its points.
pub(crate) struct Along<'a> {
    /// The points, in order.
    pub(crate) points: &'a [Point],
    /// The distance along the polyline to each point: `at[0]` is 0, and the last is its length.
    pub(crate) at: Vec<f64>,
}

impl<'a> Along<'a> {
    /// `points` measured, at a unit of `meter`'s work per point.
    pub(crate) fn new(points: &'a [Point], meter: &mut Meter) -> Result<Along<'a>, Exhausted> {
        let mut at = Vec::with_capacity(points.len());
        let mut total = 0.0;
        let mut previous = None;
        for point in points {
            meter.charge(1)?;
            total += previous.map_or(0.0, |p: Point| p.distance(*point));
            at.push(total);
            previous = Some(*point);
        }
        Ok(Along { points, at })
    }

    /// Its length.
    pub(crate) fn length(&self) -> f64 {
        self.at.last().copied().unwrap_or(0.0)
    }

    /// The point `at` along it, held to its ends.
    pub(crate) fn point(&self, at: f64) -> Point {
        // The side the distance falls on: from the last point at or before it to the next one. A side
        // with no length gives a NaN fraction, which puts the point on the side's first point, and a
        // polyline of one point has no side: its point is everywhere.
        let end = self.at.partition_point(|a| *a <= at).clamp(1, self.points.len().saturating_sub(1).max(1));
        let vertex = |i: usize| (self.points.get(i).copied(), self.at.get(i).copied().unwrap_or(0.0));
        let ((from, start), (to, stop)) = (vertex(end - 1), vertex(end));
        let from = from.unwrap_or(Point::ORIGIN);
        from.lerp(to.unwrap_or(from), (at - start) / (stop - start))
    }

    /// The distance along it to its point nearest `p`, the first of equally near ones, at a unit of
    /// `meter`'s work per side.
    pub(crate) fn project(&self, p: Point, meter: &mut Meter) -> Result<f64, Exhausted> {
        let mut best: Option<(f64, f64)> = None;
        for (side, start) in self.points.windows(2).zip(&self.at) {
            meter.charge(1)?;
            if let [a, b] = side {
                let on = nearest_on_segment(p, *a, *b);
                let distance = p.distance(on);
                if best.is_none_or(|(d, _)| distance < d) {
                    best = Some((distance, start + a.distance(on)));
                }
            }
        }
        Ok(best.map_or(0.0, |(_, at)| at))
    }

    /// The part of it from `from` to `to` along it, `from` before `to`: the points there, and its own
    /// points between them.
    pub(crate) fn part(&self, from: f64, to: f64) -> Vec<Point> {
        let inside = self.points.iter().zip(&self.at).filter(|(_, at)| from < **at && **at < to).map(|(p, _)| *p);
        std::iter::once(self.point(from)).chain(inside).chain(std::iter::once(self.point(to))).collect()
    }
}

#[cfg(test)]
mod tests {
    use stitchcraft_core::Budget;

    use super::*;

    fn p(x: f64, y: f64) -> Point {
        Point::new(x, y).unwrap()
    }

    #[test]
    fn points_are_found_by_distance_and_distances_by_point() {
        let points = [p(0.0, 0.0), p(3.0, 4.0), p(3.0, 10.0)];
        let along = Along::new(&points, &mut Budget::DEFAULT.meter()).unwrap();
        assert_eq!((along.at.clone(), along.length()), (vec![0.0, 5.0, 11.0], 11.0));
        assert_eq!(along.point(2.5), p(1.5, 2.0));
        assert_eq!(along.point(8.0), p(3.0, 7.0));
        assert_eq!((along.point(-1.0), along.point(12.0)), (p(0.0, 0.0), p(3.0, 10.0)), "held to the ends");
        let meter = &mut Budget::DEFAULT.meter();
        assert_eq!(along.project(p(5.0, 7.0), meter).unwrap(), 8.0);
        assert_eq!(along.project(p(-2.0, -1.0), meter).unwrap(), 0.0);
        assert_eq!(along.part(2.5, 8.0), [p(1.5, 2.0), p(3.0, 4.0), p(3.0, 7.0)]);
        assert_eq!(along.part(5.0, 11.0), [p(3.0, 4.0), p(3.0, 10.0)], "a point at an end is not repeated");
    }

    #[test]
    fn of_equally_near_points_the_first_is_taken() {
        // A point below a V is as near its first side as its second.
        let points = [p(0.0, 0.0), p(5.0, 5.0), p(10.0, 0.0)];
        let along = Along::new(&points, &mut Budget::DEFAULT.meter()).unwrap();
        assert_eq!(along.project(p(5.0, -1.0), &mut Budget::DEFAULT.meter()).unwrap(), 8.0_f64.sqrt());
        let one = [p(1.0, 1.0)];
        let dot = Along::new(&one, &mut Budget::DEFAULT.meter()).unwrap();
        assert_eq!((dot.length(), dot.point(3.0), dot.project(p(0.0, 0.0), &mut Budget::DEFAULT.meter()).unwrap()), (0.0, p(1.0, 1.0), 0.0));
    }
}
