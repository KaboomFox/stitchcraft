//! Ink/Stitch's next stitch: where an element ends depends on where the next one starts, which Ink/Stitch
//! takes to be the point of the element's own shape nearest the shape the next element offers
//! ([`Approach`]), as its geometry library (GEOS) finds the nearest points of 2 shapes
//! (`docs/src/design/adr/0014-generators-see-their-neighbours.md`).
//!
//! **Inside an area.** An area is 0 away from every point in it or on its rings, so before it measures
//! between lines the library looks for such a point. It tests the next element's points first, then
//! those of the element's own shape, each point against every part of the other's area before the next
//! point, and the first that lies in a part is the next stitch. A point is tested as itself, polylines (a satin column's
//! rails) by each one's start in order, and an area (a fill's parts) by each part's outline's start in
//! order, never a hole's. Polylines have no area to lie in. These rules were checked against shapely 2.2.0
//! as a black box.
//!
//! **Otherwise** the next stitch is the point of the element's polylines, or of its area's rings, nearest
//! the next element's point, polylines or rings, measured from the next element's
//! ([`crate::normalize::near`]).

use stitchcraft_core::{Exhausted, Meter, Point};

use super::Approach;
use crate::normalize::near::{nearest_to_lines, nearest_to_point};
use crate::normalize::region::Polygon;

/// An element's own shape, on which its next stitch is found.
#[derive(Clone, Copy, Debug)]
pub(crate) enum Own<'a> {
    /// Polylines: a satin column's rails, swapped and turned as they are sewn.
    Lines(&'a [&'a [Point]]),
    /// An area: a tatami fill's parts.
    Area(&'a [Polygon]),
}

impl Approach {
    /// The next stitch on `own`: the first point of either shape that lies in the other's area, the next
    /// element's tested first, and otherwise the point of `own` nearest what the next element offers. `None`
    /// when a shape has no point. One unit of `meter` for each ring point a point is tested against, and
    /// one for each side or pair of sides measured.
    pub(crate) fn next_stitch(&self, own: Own<'_>, meter: &mut Meter) -> Result<Option<Point>, Exhausted> {
        let (next_starts, next_parts): (Vec<Point>, &[Polygon]) = match self {
            Approach::Point(p) => (vec![*p], &[]),
            Approach::Shape(lines) => (starts(lines.iter().map(Vec::as_slice)), &[]),
            Approach::Area(parts) => (starts(outlines(parts)), parts),
        };
        let (own_starts, own_parts, own_lines): (Vec<Point>, &[Polygon], Vec<&[Point]>) = match own {
            Own::Lines(lines) => (starts(lines.iter().copied()), &[], lines.to_vec()),
            Own::Area(parts) => (starts(outlines(parts)), parts, rings(parts)),
        };
        for (points, parts) in [(next_starts, own_parts), (own_starts, next_parts)] {
            if let Some(p) = first_covered(&points, parts, meter)? {
                return Ok(Some(p));
            }
        }
        match self {
            Approach::Point(p) => Ok(nearest_to_point(&own_lines, *p, meter)?.map(|(on, _)| on)),
            Approach::Shape(lines) => nearest_to_lines(&own_lines, &lines.iter().map(Vec::as_slice).collect::<Vec<_>>(), meter),
            Approach::Area(parts) => nearest_to_lines(&own_lines, &rings(parts), meter),
        }
    }
}

/// The starts of `lines`, in order.
fn starts<'a>(lines: impl Iterator<Item = &'a [Point]>) -> Vec<Point> {
    lines.filter_map(|line| line.first().copied()).collect()
}

/// The outlines of `parts`, in order.
fn outlines(parts: &[Polygon]) -> impl Iterator<Item = &[Point]> {
    parts.iter().map(|part| part.outline.as_slice())
}

/// The rings of `parts`: part by part, the outline before the holes.
fn rings(parts: &[Polygon]) -> Vec<&[Point]> {
    parts.iter().flat_map(Polygon::rings).collect()
}

/// The first of `points` that lies in one of `parts` or on its rings, each point tested against every part
/// before the next point. One unit of `meter` for each ring point a point is tested against.
fn first_covered(points: &[Point], parts: &[Polygon], meter: &mut Meter) -> Result<Option<Point>, Exhausted> {
    for &p in points {
        for part in parts {
            if covers(part, p, meter)? {
                return Ok(Some(p));
            }
        }
    }
    Ok(None)
}

/// Whether `p` lies in `part` or on its rings. One unit of `meter` for each of their points.
pub(crate) fn covers(part: &Polygon, p: Point, meter: &mut Meter) -> Result<bool, Exhausted> {
    meter.charge(part.rings().map(|ring| u64::try_from(ring.len()).unwrap_or(u64::MAX)).fold(0, u64::saturating_add))?;
    Ok(part.covers(p))
}

#[cfg(test)]
mod tests {
    use stitchcraft_core::Budget;

    use super::*;

    fn p(x: f64, y: f64) -> Point {
        Point::new(x, y).unwrap()
    }

    /// The square from (x0, y0) to (x1, y1), from its corner `start` (0 to 3, round from (x0, y0)), with
    /// `holes`.
    fn square(x0: f64, y0: f64, x1: f64, y1: f64, start: usize, holes: Vec<Vec<Point>>) -> Polygon {
        let mut corners = vec![p(x0, y0), p(x1, y0), p(x1, y1), p(x0, y1)];
        corners.rotate_left(start);
        corners.push(corners[0]);
        Polygon { outline: corners, holes }
    }

    fn next_stitch(next: &Approach, own: Own<'_>) -> Option<Point> {
        next.next_stitch(own, &mut Budget::DEFAULT.meter()).unwrap()
    }

    #[test]
    fn the_next_element_s_points_are_tested_first_each_against_every_part() {
        // 2 parts; the first line starts in the second part and the second line in the first: the first
        // line's start, as GEOS takes the points in order, each against every part.
        let own = [square(0.0, 0.0, 4.0, 4.0, 0, Vec::new()), square(10.0, 0.0, 14.0, 4.0, 0, Vec::new())];
        let lines = Approach::Shape(vec![vec![p(50.0, 50.0), p(60.0, 60.0)], vec![p(12.0, 2.0), p(40.0, 40.0)], vec![p(2.0, 2.0), p(2.0, 40.0)]]);
        assert_eq!(next_stitch(&lines, Own::Area(&own)), Some(p(12.0, 2.0)));
        // Of 2 areas that each hold the other's start, the next element's start.
        let own = [square(0.0, 0.0, 10.0, 10.0, 2, Vec::new())];
        let next = Approach::Area(vec![square(5.0, 5.0, 20.0, 20.0, 0, Vec::new())]);
        assert_eq!(next_stitch(&next, Own::Area(&own)), Some(p(5.0, 5.0)));
        // A start on a ring counts.
        let rails: [&[Point]; 2] = [&[p(20.0, 5.0), p(30.0, 5.0)], &[p(10.0, 3.0), p(30.0, 3.0)]];
        let fill = Approach::Area(vec![square(0.0, 0.0, 10.0, 10.0, 0, Vec::new())]);
        assert_eq!(next_stitch(&fill, Own::Lines(&rails)), Some(p(10.0, 3.0)));
    }

    #[test]
    fn an_outline_counts_by_its_first_point_and_a_hole_not_at_all() {
        // The next area's outline starts outside, its second corner inside: where the rings first meet.
        let own = [square(0.0, 0.0, 10.0, 10.0, 0, Vec::new())];
        let next = Approach::Area(vec![Polygon { outline: vec![p(30.0, 30.0), p(5.0, 5.0), p(30.0, 5.0), p(30.0, 30.0)], holes: Vec::new() }]);
        assert_eq!(next_stitch(&next, Own::Area(&own)), Some(p(10.0, 10.0)));
        // The own area's hole starts in the next area, and its outline does not: where the rings meet.
        let own = [square(-50.0, -50.0, 50.0, 50.0, 1, vec![square(10.0, 10.0, 30.0, 30.0, 0, Vec::new()).outline])];
        let next = Approach::Area(vec![square(5.0, 5.0, 15.0, 15.0, 2, Vec::new())]);
        assert_eq!(next_stitch(&next, Own::Area(&own)), Some(p(10.0, 15.0)));
    }

    #[test]
    fn with_no_point_inside_the_nearest_points_are_measured_from_the_next_element() {
        let rails: [&[Point]; 2] = [&[p(0.0, 0.0), p(10.0, 0.0)], &[p(0.0, 6.0), p(10.0, 6.0)]];
        assert_eq!(next_stitch(&Approach::Point(p(5.0, -3.0)), Own::Lines(&rails)), Some(p(5.0, 0.0)));
        // A point in a fill is its own next stitch; outside, the fill's point nearest it.
        let own = [square(0.0, 0.0, 10.0, 10.0, 0, Vec::new())];
        assert_eq!(next_stitch(&Approach::Point(p(3.0, 4.0)), Own::Area(&own)), Some(p(3.0, 4.0)));
        assert_eq!(next_stitch(&Approach::Point(p(13.0, 4.0)), Own::Area(&own)), Some(p(10.0, 4.0)));
        // An area beside the rails: the rails' point nearest its rings.
        let beside = Approach::Area(vec![square(12.0, -1.0, 20.0, 1.0, 0, Vec::new())]);
        assert_eq!(next_stitch(&beside, Own::Lines(&rails)), Some(p(10.0, 0.0)));
        // Nothing to measure.
        assert_eq!(next_stitch(&Approach::Shape(Vec::new()), Own::Lines(&rails)), None);
    }

    #[test]
    fn each_ring_point_a_point_is_tested_against_is_charged() {
        // 2 points, 2 parts of 5 ring points each: the second part holds neither, the first holds the second.
        let own = [square(0.0, 0.0, 4.0, 4.0, 0, Vec::new()), square(10.0, 0.0, 14.0, 4.0, 0, Vec::new())];
        let mut meter = Budget::DEFAULT.meter();
        assert_eq!(first_covered(&[p(20.0, 20.0), p(2.0, 2.0)], &own, &mut meter).unwrap(), Some(p(2.0, 2.0)));
        assert_eq!(Budget::DEFAULT.max_work - meter.work_left(), 15);
        assert!(first_covered(&[p(20.0, 20.0), p(2.0, 2.0)], &own, &mut Budget { max_stitches: 1, max_work: 14 }.meter()).is_err());
    }
}
