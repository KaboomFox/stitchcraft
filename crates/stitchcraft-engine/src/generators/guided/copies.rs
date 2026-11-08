//! A guided fill's rows over one piece: copies of its guide line, moved a row spacing at a time across the
//! piece, as Ink/Stitch copies its guide (read at `d59c9ab`, `REQ-FILL-GDD-003`, `REQ-FILL-GDD-004`).
//!
//! **Lengthened.** The guide is lengthened straight on at both ends, along the line from its first point to
//! its last, by the larger of the distances from its ends to the piece's farthest corner from them. Each copy
//! then runs from outside the piece's bounds to outside them again, however it is moved.
//!
//! **Moved.** The copies move square to the line between the guide's ends, turned a right angle clockwise
//! from it as the drawing shows it (y down), or at `guided_fill_angle`, clockwise from the x axis as the
//! drawing shows it. Copy `k` lies `k` row spacings from the guide that way. Where the lengthened guide
//! meets the piece, the copies start from it, copy 0. Elsewhere they start from the whole number of row
//! spacings the guide's point nearest the piece's centroid lies from it, rounded towards 0, counted negative
//! where the centroid lies behind the guide. From there they go on outward, one way and then the other,
//! while they meet the piece's bounds, and those that meet the piece are its rows. With `smoothness_mm`
//! each copy is smoothed first.
//!
//! **In order.** The rows are taken in order of their first points: 2 by x and then y, and more by how far
//! along the diagonal of their first points' box, from its corner of least x and y, each lies, as
//! Ink/Stitch orders them by projecting them on the rectangle round them. Their stagger and their random
//! lengths count in that order.
//!
//! **Cost.** Each copy charges a unit of work for each of its points, and each test of a copy against the
//! piece its sides near the piece's rings.

use stitchcraft_core::math::sin_cos;
use stitchcraft_core::units::at_least;
use stitchcraft_core::{Exhausted, Meter, Point};

use super::Guided;
use crate::generators::smoothing::smooth;
use crate::generators::tatami::rings::Rings;
use crate::normalize::region::Polygon;
use crate::normalize::region::geom::{Hit, hit};
use crate::normalize::stroke::nearest_on_segment;

/// The rows the guide line `guide`, ready to copy, gives over `piece`, whose rings are `rings`, as
/// `guided` places them: its copies that meet the piece, smoothed, in the order they are taken.
pub(super) fn rows(piece: &Polygon, rings: &Rings<'_>, guide: &[Point], guided: &Guided, meter: &mut Meter) -> Result<Vec<Vec<Point>>, Exhausted> {
    let (Some(&first), Some(&last)) = (guide.first(), guide.last()) else { return Ok(Vec::new()) };
    let bounds = Bounds::of(&piece.outline);
    let along = unit(first, last);
    // Far enough that each end lies outside the bounds, however the copies are moved.
    let reach = [first, last].iter().map(|&end| bounds.farthest(end)).fold(0.0, f64::max);
    let lengthened: Vec<Point> =
        std::iter::once(moved(first, along, -reach)).chain(guide.iter().copied()).chain(std::iter::once(moved(last, along, reach))).collect();
    meter.charge(u64::try_from(lengthened.len()).unwrap_or(u64::MAX))?;
    let step = match guided.angle {
        Some(degrees) => {
            let (sin, cos) = sin_cos(degrees.to_radians());
            [cos, sin]
        }
        // A right angle clockwise as the drawing shows it, y down.
        None => [-along[1], along[0]],
    };
    let start = start(piece, rings, &lengthened, step, guided.spacing, meter)?;
    let smoothed = if guided.smoothness > 0.0 { smooth(&lengthened, guided.smoothness, guided.travel.min_stitch, meter)? } else { lengthened };
    let copy = |k: i64, meter: &mut Meter| -> Result<Vec<Point>, Exhausted> {
        meter.charge(u64::try_from(smoothed.len()).unwrap_or(u64::MAX))?;
        #[allow(clippy::cast_precision_loss)] // Row numbers are far below 2⁵².
        let by = k as f64 * guided.spacing;
        Ok(smoothed.iter().map(|&p| moved(p, step, by)).collect())
    };
    let mut rows = Vec::new();
    for way in [1_i64, -1] {
        let mut k = if way == 1 { start } else { start - 1 };
        loop {
            let row = copy(k, meter)?;
            if !bounds.met_by(&row, meter)? {
                break;
            }
            if meets(&row, rings, meter)? {
                rows.push(row);
            }
            k += way;
        }
    }
    sorted(&mut rows, meter)?;
    Ok(rows)
}

/// The copy the rows start from, by its number: 0 where `lengthened` meets the piece, and otherwise the
/// whole number of `spacing`s, rounded towards 0, the point of it nearest the piece's centroid lies from the
/// centroid, negative where `step` points away from the centroid.
fn start(piece: &Polygon, rings: &Rings<'_>, lengthened: &[Point], step: [f64; 2], spacing: f64, meter: &mut Meter) -> Result<i64, Exhausted> {
    if meets(lengthened, rings, meter)? {
        return Ok(0);
    }
    let centroid = piece.centroid(meter)?;
    let mut nearest: Option<(f64, Point)> = None;
    for side in lengthened.windows(2) {
        meter.charge(1)?;
        let on = nearest_on_segment(centroid, side[0], side[1]);
        let distance = on.distance(centroid);
        if nearest.is_none_or(|(best, _)| distance < best) {
            nearest = Some((distance, on));
        }
    }
    let Some((distance, on)) = nearest else { return Ok(0) };
    #[allow(clippy::cast_possible_truncation)] // A row count far below i64's range, the bounds held to the working limit.
    let rows = (distance / spacing).trunc() as i64;
    // The sign of the product, -0 included, as Ink/Stitch copies it onto the count.
    let towards = (centroid.x() - on.x()) * step[0] + (centroid.y() - on.y()) * step[1];
    Ok(if towards.is_sign_negative() { -rows } else { rows })
}

/// Whether the line `points` meets the piece whose rings are `rings`: crosses or touches one of its rings, or
/// lies in it. Costs as [`Rings::meeting`] and [`Rings::covers`] do.
fn meets(points: &[Point], rings: &Rings<'_>, meter: &mut Meter) -> Result<bool, Exhausted> {
    let mut met = false;
    for side in points.windows(2) {
        meter.charge(1)?;
        let (p, q) = (side[0], side[1]);
        rings.meeting(p, q, meter, |ring| met |= hit(p, q, ring.a, ring.b) != Hit::Apart)?;
        if met {
            return Ok(true);
        }
    }
    match points.first() {
        Some(&p) => rings.covers(p, meter),
        None => Ok(false),
    }
}

/// `rows` in the order they are taken, as the module says: a stable sort, so that rows whose first points
/// lie as far along keep the order they were found in.
pub(super) fn sorted(rows: &mut [Vec<Point>], meter: &mut Meter) -> Result<(), Exhausted> {
    meter.charge(u64::try_from(rows.len()).unwrap_or(u64::MAX))?;
    let firsts: Vec<Point> = rows.iter().filter_map(|row| row.first().copied()).collect();
    if let [_, _] = firsts.as_slice() {
        rows.sort_by(|a, b| first_of(a).x().total_cmp(&first_of(b).x()).then(first_of(a).y().total_cmp(&first_of(b).y())));
        return Ok(());
    }
    let box_of = Bounds::of(&firsts);
    let diagonal = [box_of.max[0] - box_of.min[0], box_of.max[1] - box_of.min[1]];
    let along = |row: &Vec<Point>| (first_of(row).x() - box_of.min[0]) * diagonal[0] + (first_of(row).y() - box_of.min[1]) * diagonal[1];
    rows.sort_by(|a, b| along(a).total_cmp(&along(b)));
    Ok(())
}

/// The first point of `row`, or the origin for a row of none.
fn first_of(row: &[Point]) -> Point {
    row.first().copied().unwrap_or(Point::ORIGIN)
}

/// The unit vector from `from` to `to`; nothing for points at one place.
fn unit(from: Point, to: Point) -> [f64; 2] {
    let (dx, dy) = (to.x() - from.x(), to.y() - from.y());
    let length = (dx * dx + dy * dy).sqrt();
    if length > 0.0 { [dx / length, dy / length] } else { [0.0, 0.0] }
}

/// `p` moved `by` along the unit vector `along`.
fn moved(p: Point, along: [f64; 2], by: f64) -> Point {
    Point::new(p.x() + along[0] * by, p.y() + along[1] * by).unwrap_or(p)
}

/// The box round a piece's outline, where its copies are looked for.
#[derive(Clone, Copy, Debug)]
struct Bounds {
    min: [f64; 2],
    max: [f64; 2],
}

impl Bounds {
    /// The box round `points`.
    fn of(points: &[Point]) -> Bounds {
        let mut bounds = Bounds { min: [f64::INFINITY; 2], max: [f64::NEG_INFINITY; 2] };
        for p in points {
            bounds.min = [bounds.min[0].min(p.x()), bounds.min[1].min(p.y())];
            bounds.max = [bounds.max[0].max(p.x()), bounds.max[1].max(p.y())];
        }
        bounds
    }

    /// How far its corner farthest from `p` lies from it: the discrete Hausdorff distance from the point to
    /// the box, which Ink/Stitch lengthens its guide by.
    fn farthest(&self, p: Point) -> f64 {
        let (dx, dy) = ((p.x() - self.min[0]).abs().max((p.x() - self.max[0]).abs()), (p.y() - self.min[1]).abs().max((p.y() - self.max[1]).abs()));
        (dx * dx + dy * dy).sqrt()
    }

    /// Whether the line `points` meets the box, its edges included. A unit of `meter`'s work for each side.
    fn met_by(&self, points: &[Point], meter: &mut Meter) -> Result<bool, Exhausted> {
        for side in points.windows(2) {
            meter.charge(1)?;
            if self.meets(side[0], side[1]) {
                return Ok(true);
            }
        }
        Ok(points.first().is_some_and(|&p| self.holds(p)))
    }

    /// Whether `p` lies in the box or on its edges.
    fn holds(&self, p: Point) -> bool {
        at_least(p.x(), self.min[0]) && at_least(self.max[0], p.x()) && at_least(p.y(), self.min[1]) && at_least(self.max[1], p.y())
    }

    /// Whether the segment from `p` to `q` meets the box: the part of it between the box's edges along x,
    /// clipped, overlaps the box along y.
    fn meets(&self, p: Point, q: Point) -> bool {
        let (mut from, mut to) = (0.0_f64, 1.0_f64);
        for axis in 0..2 {
            let (start, end) = if axis == 0 { (p.x(), q.x()) } else { (p.y(), q.y()) };
            let delta = end - start;
            let (lo, hi) = (self.min[axis], self.max[axis]);
            if delta == 0.0 {
                if start < lo || start > hi {
                    return false;
                }
                continue;
            }
            let (a, b) = ((lo - start) / delta, (hi - start) / delta);
            from = from.max(a.min(b));
            to = to.min(a.max(b));
            if from > to {
                return false;
            }
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(x: f64, y: f64) -> Point {
        Point::new(x, y).unwrap()
    }

    #[test]
    fn a_box_is_met_by_a_segment_through_it_along_it_or_ending_in_it() {
        let bounds = Bounds::of(&[p(0.0, 0.0), p(10.0, 4.0)]);
        assert!(bounds.meets(p(-1.0, 2.0), p(11.0, 2.0)));
        assert!(bounds.meets(p(-1.0, -1.0), p(1.0, 1.0)));
        assert!(bounds.meets(p(0.0, -1.0), p(0.0, 5.0)), "along its left edge");
        assert!(bounds.meets(p(5.0, 1.0), p(5.0, 1.0)), "a point inside");
        assert!(!bounds.meets(p(-1.0, 5.0), p(11.0, 5.0)));
        assert!(!bounds.meets(p(-2.0, 0.0), p(0.0, -2.0)), "past its corner");
        assert!(!bounds.meets(p(11.0, 2.0), p(11.0, 3.0)), "beside it");
        assert!(!bounds.meets(p(2.0, -1.0), p(3.0, -1.0)), "below it");
        assert_eq!(bounds.farthest(p(0.0, 0.0)), (100.0_f64 + 16.0).sqrt());
        assert_eq!(bounds.farthest(p(-3.0, 2.0)), (169.0_f64 + 4.0).sqrt());
    }
}
