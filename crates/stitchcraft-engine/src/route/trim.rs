//! Which jumps a route trims (`docs/src/design/routing.md` › Auto-satin, step 7): with Trim jump stitches,
//! a jump whose length outside the 2 elements it joins is more than 1 mm trims the thread after the piece
//! before it.
//!
//! **What an element hides.** Ink/Stitch takes a satin column as its rails, lines with no area, and a stroke
//! as the convex hull of its subpaths (read at `d59c9ab`). So all of a jump between 2 columns counts but
//! what runs along a rail, and a jump to or from a stroke counts only outside the stroke's hull. A hull of
//! points in a line is that line, and of one point hides nothing. Whether a point lies on a line is decided
//! exactly ([`stitchcraft_core::exact`]), as GEOS decides it.
//!
//! **The hull** is Andrew's monotone chain (A. M. Andrew, *Another efficient algorithm for convex hulls in
//! two dimensions*, 1979): the points sorted, a chain along their lower side and one back along their upper
//! side, each turning the same way at every point.

use stitchcraft_core::exact::{Side, side};
use stitchcraft_core::{Exhausted, Meter, Point};

/// How much of a jump's length outside the elements it joins makes it long, in millimetres: 1 mm,
/// Ink/Stitch's (`PIXELS_PER_MM` CSS pixels). A jump longer than that outside them is trimmed.
pub const TRIMMED: f64 = 1.0;

/// What an element hides of a jump.
#[derive(Clone, Debug, PartialEq)]
pub(super) enum Outline {
    /// Line segments, each from one point to the next: a column's rails, or a stroke's hull when its points
    /// lie in a line. They hide only what runs along them.
    Lines(Vec<[Point; 2]>),
    /// A convex polygon, its corners turning left: a stroke's hull. It hides what lies inside it or on it.
    Area(Vec<Point>),
}

/// The convex hull of `points`: a polygon when they don't all lie in a line, and otherwise the segment
/// between the 2 farthest apart, or nothing for one point. Each point looked at charges `meter` a unit.
pub(super) fn hull(points: &[Point], meter: &mut Meter) -> Result<Outline, Exhausted> {
    let mut sorted = points.to_vec();
    meter.charge(u64::try_from(sorted.len()).unwrap_or(u64::MAX))?;
    sorted.sort_by(|a, b| a.x().total_cmp(&b.x()).then(a.y().total_cmp(&b.y())));
    sorted.dedup();
    let mut chain = |points: &mut dyn Iterator<Item = &Point>| -> Result<Vec<Point>, Exhausted> {
        let mut chain: Vec<Point> = Vec::new();
        for &point in points {
            meter.charge(1)?;
            while let [.., a, b] = chain.as_slice()
                && side(*a, *b, point) != Side::Left
            {
                chain.pop();
            }
            chain.push(point);
        }
        Ok(chain)
    };
    let lower = chain(&mut sorted.iter())?;
    let upper = chain(&mut sorted.iter().rev())?;
    let corners: Vec<Point> =
        lower.iter().take(lower.len().saturating_sub(1)).chain(upper.iter().take(upper.len().saturating_sub(1))).copied().collect();
    Ok(match (corners.len(), sorted.first(), sorted.last()) {
        (3.., ..) => Outline::Area(corners),
        (_, Some(&first), Some(&last)) if first != last => Outline::Lines(vec![[first, last]]),
        _ => Outline::Lines(Vec::new()),
    })
}

/// How long the jump from `from` to `to` is outside both `outlines`, in millimetres. Each side of an
/// outline looked at charges `meter` a unit.
pub(super) fn outside(from: Point, to: Point, outlines: [&Outline; 2], meter: &mut Meter) -> Result<f64, Exhausted> {
    let length = from.distance(to);
    if length <= 0.0 {
        return Ok(0.0);
    }
    // The stretches of the jump each outline hides, as fractions of its length.
    let mut hidden: Vec<[f64; 2]> = Vec::new();
    for outline in outlines {
        match outline {
            Outline::Lines(lines) => {
                for &[a, b] in lines {
                    meter.charge(1)?;
                    hidden.extend(along(from, to, a, b));
                }
            }
            Outline::Area(corners) => hidden.extend(inside(from, to, corners, meter)?),
        }
    }
    hidden.sort_by(|a, b| a[0].total_cmp(&b[0]));
    let (mut covered, mut reach) = (0.0, 0.0_f64);
    for [start, end] in hidden {
        let start = start.max(reach);
        if end > start {
            covered += end - start;
            reach = end;
        }
    }
    Ok(length * (1.0 - covered).max(0.0))
}

/// The stretch of the jump from `from` to `to` that runs along the segment from `a` to `b`, as fractions of
/// the jump; `None` unless the segment lies on the jump's line.
fn along(from: Point, to: Point, a: Point, b: Point) -> Option<[f64; 2]> {
    if side(from, to, a) != Side::On || side(from, to, b) != Side::On {
        return None;
    }
    let (dx, dy) = (to.x() - from.x(), to.y() - from.y());
    let length2 = dx * dx + dy * dy;
    let at = |p: Point| ((p.x() - from.x()) * dx + (p.y() - from.y()) * dy) / length2;
    let (s, t) = (at(a).min(at(b)).max(0.0), at(a).max(at(b)).min(1.0));
    (t > s).then_some([s, t])
}

/// The stretch of the jump from `from` to `to` inside the convex polygon `corners`, which turn left, as
/// fractions of the jump; `None` when it misses it.
fn inside(from: Point, to: Point, corners: &[Point], meter: &mut Meter) -> Result<Option<[f64; 2]>, Exhausted> {
    let (mut s, mut t) = (0.0_f64, 1.0_f64);
    for (k, &a) in corners.iter().enumerate() {
        meter.charge(1)?;
        let Some(&b) = corners.get((k + 1) % corners.len()) else { continue };
        // How far left of the side from `a` to `b` each end of the jump lies: the polygon is on the left.
        let left = |p: Point| (b.x() - a.x()) * (p.y() - a.y()) - (b.y() - a.y()) * (p.x() - a.x());
        let (at_from, at_to) = (left(from), left(to));
        match (at_from >= 0.0, at_to >= 0.0) {
            (true, true) => {}
            (false, false) => return Ok(None),
            (true, false) => t = t.min(at_from / (at_from - at_to)),
            (false, true) => s = s.max(at_from / (at_from - at_to)),
        }
    }
    Ok((t > s).then_some([s, t]))
}

#[cfg(test)]
mod tests {
    use stitchcraft_core::Budget;

    use super::*;

    fn p(x: f64, y: f64) -> Point {
        Point::new(x, y).unwrap()
    }

    #[test]
    fn a_hull_is_the_points_round_turning_left_or_the_line_they_lie_in() {
        let meter = &mut Budget::DEFAULT.meter();
        // A square with a point inside and one on a side: the 4 corners, turning left from the lowest left.
        let square = [p(0.0, 0.0), p(4.0, 0.0), p(4.0, 4.0), p(0.0, 4.0), p(2.0, 2.0), p(2.0, 0.0)];
        assert_eq!(hull(&square, meter).unwrap(), Outline::Area(vec![p(0.0, 0.0), p(4.0, 0.0), p(4.0, 4.0), p(0.0, 4.0)]));
        // Points in a line: the segment between the farthest apart; one point, or none: nothing.
        assert_eq!(hull(&[p(1.0, 1.0), p(3.0, 3.0), p(2.0, 2.0)], meter).unwrap(), Outline::Lines(vec![[p(1.0, 1.0), p(3.0, 3.0)]]));
        assert_eq!(hull(&[p(1.0, 1.0), p(1.0, 1.0)], meter).unwrap(), Outline::Lines(Vec::new()));
        assert_eq!(hull(&[], meter).unwrap(), Outline::Lines(Vec::new()));
    }

    #[test]
    fn a_jump_counts_outside_a_stroke_s_hull_and_off_a_column_s_rails() {
        let meter = &mut Budget::DEFAULT.meter();
        let square = Outline::Area(vec![p(0.0, 0.0), p(4.0, 0.0), p(4.0, 4.0), p(0.0, 4.0)]);
        let none = Outline::Lines(Vec::new());
        // From inside the square to 3 mm past its right side: 3 mm outside.
        assert!((outside(p(2.0, 2.0), p(7.0, 2.0), [&square, &none], meter).unwrap() - 3.0).abs() < 1e-12);
        // Across it: 1 mm before and 2 mm after.
        assert!((outside(p(-1.0, 1.0), p(6.0, 1.0), [&none, &square], meter).unwrap() - 3.0).abs() < 1e-12);
        // Missing it altogether, past its corner, or wholly inside it.
        assert_eq!(outside(p(5.0, 5.0), p(9.0, 5.0), [&square, &square], meter).unwrap(), 4.0);
        assert_eq!(outside(p(1.0, 1.0), p(3.0, 3.0), [&square, &none], meter).unwrap(), 0.0);
        // A rail hides only what runs along it: half of this jump, and nothing of one across it.
        let rail = Outline::Lines(vec![[p(0.0, 0.0), p(5.0, 0.0)]]);
        assert_eq!(outside(p(3.0, 0.0), p(7.0, 0.0), [&rail, &none], meter).unwrap(), 2.0);
        assert_eq!(outside(p(2.0, -1.0), p(2.0, 5.0), [&rail, &none], meter).unwrap(), 6.0);
        // Overlapping stretches count once, and a jump of no length is no length.
        assert_eq!(outside(p(3.0, 0.0), p(7.0, 0.0), [&rail, &rail], meter).unwrap(), 2.0);
        assert_eq!(outside(p(3.0, 0.0), p(3.0, 0.0), [&rail, &square], meter).unwrap(), 0.0);
    }
}
