//! The needle points along a tatami fill's rows (`REQ-FILL-TAT-003`), as Ink/Stitch places them
//! (`docs/src/design/algorithms/fills.md` › Needle points along a row).
//!
//! **The stagger grid.** Along each row, the needle points lie on a grid the longest stitch apart,
//! anchored at the design's origin: measured along the rows from the origin, at whole numbers of stitches
//! plus the row's offset. A row's number is how far across the rows it lies from the origin, over the row
//! spacing, rounded to the nearest whole number (halves to the even one). Its offset is that number over
//! `staggers`, less its whole part, times the longest stitch. Each row's points then lie a `staggers`-th
//! of a stitch along from the row before's, and come back after `staggers` rows, so that neighbouring
//! rows never line their points up into furrows. Fills side by side share the grid.
//!
//! **A segment**, sewn from its start to its end, takes its start, then each grid point past the start
//! and before the end, and then its end, unless `skip_last` is on or the last point lies within 0.1 mm of
//! it. Routing (roadmap M5.4) decides which way each segment is sewn, and sewn the other way a segment
//! takes the same grid points.
//!
//! **Random lengths** (`enable_random_stitch_length`) take the place of the grid: the first point lies a
//! random share of the longest stitch past the start, and each next one a stitch on, longer or shorter by
//! up to the jitter at random, while they lie before the end. They draw from the element's generator, in
//! the order the segments are sewn. Ink/Stitch draws from a generator of each row's own, and its random
//! values differ (`DEV-FILL-004`).

use stitchcraft_core::rng::SplitMix64;
use stitchcraft_core::{Exhausted, Meter, Point};

use crate::generators::tatami::rows::{Grid, axes};

/// A segment's end gets no needle point of its own when the last one lies this near it, in millimetres:
/// Ink/Stitch's 0.1 mm.
const END_NEAR: f64 = 0.1;

/// How a tatami fill's rows are stitched.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Stitching {
    /// Where the rows lie: the grid is anchored across them at the row spacing.
    pub grid: Grid,
    /// The longest stitch, in millimetres: how far apart the grid's points are.
    pub length: f64,
    /// How many rows the grid takes to come back to where it started.
    pub staggers: f64,
    /// Whether a segment's end goes without a needle point of its own.
    pub skip_last: bool,
    /// With random lengths, how much longer or shorter each stitch may be, as a fraction of the longest
    /// stitch; `None` for the stagger grid.
    pub jitter: Option<f64>,
}

/// The needle points of the row segment from `start` to `end`, sewn that way, as `stitching` says, with
/// random lengths drawn from `rng`. A unit of `meter`'s work per point, and per place the grid has at the
/// start.
pub fn row_points(start: Point, end: Point, stitching: &Stitching, rng: &mut SplitMix64, meter: &mut Meter) -> Result<Vec<Point>, Exhausted> {
    meter.charge(1)?;
    let mut points = vec![start];
    let length = start.distance(end);
    if length == 0.0 {
        return Ok(points);
    }
    let direction = ((end.x() - start.x()) / length, (end.y() - start.y()) / length);
    let at = |by: f64| Point::new(start.x() + by * direction.0, start.y() + by * direction.1).unwrap_or(start);
    let stitch = stitching.length;
    let mut by = match stitching.jitter {
        Some(_) => stitch * rng.next_f64(),
        None => first_on_grid(start, direction, stitching),
    };
    while by < length {
        meter.charge(1)?;
        if by > 0.0 {
            points.push(at(by));
        }
        by += match stitching.jitter {
            Some(jitter) => stitch * (1.0 + jitter * (rng.next_f64() - 0.5) * 2.0),
            None => stitch,
        };
    }
    if !stitching.skip_last && points.last().is_some_and(|last| last.distance(end) > END_NEAR) {
        meter.charge(1)?;
        points.push(end);
    }
    Ok(points)
}

/// How far from `start`, going in `direction`, the first point of its row's grid lies that is not behind
/// it: at `start` itself when it lies on the grid.
fn first_on_grid(start: Point, direction: (f64, f64), stitching: &Stitching) -> f64 {
    let (along, across) = axes(stitching.grid.angle);
    let dot = |(x, y): (f64, f64)| start.x() * x + start.y() * y;
    let number = (dot(across) / stitching.grid.spacing).round_ties_even();
    let offset = (number / stitching.staggers).rem_euclid(1.0) * stitching.length;
    // How far along the rows `start` lies past the grid point before it.
    let past = (dot(along) - offset).rem_euclid(stitching.length);
    // The grid point before `start` lies `past` behind it along the rows: ahead of it when the segment
    // runs against the rows, and otherwise a stitch on from there.
    if along.0 * direction.0 + along.1 * direction.1 < 0.0 { past } else { (stitching.length - past) % stitching.length }
}
