//! VectorCraft's paths as the engine's, in millimetres, and the engine's points back in VectorCraft's.
//!
//! **Why here, once.** A document's paths and the geometry a live effect is given are the same JSON,
//! VectorCraft's `PathData`, so the reader ([`crate::read`]) and the live effect
//! ([`crate::effect_element`]) read them with the same code.
//!
//! **How a path reads.** Coordinates are points with y down, in document coordinates, and are multiplied by
//! 25.4/72 into millimetres. An anchor's handles are absolute positions. A handle left out, or within
//! VectorCraft's tolerance of its anchor, leaves that end of a segment straight, and a segment with
//! neither handle is a line. A closed subpath has a segment back to its first anchor, and a subpath of one
//! anchor draws nothing.

use stitchcraft_core::units::MM_PER_POINT;
use stitchcraft_core::{Exhausted, Meter, Point};
use stitchcraft_engine::design::{Path, Segment, Subpath, WORKING_LIMIT_MM};

use crate::wire::{Anchor, PathData, WirePoint};

/// A handle this close to its anchor, in points, is none: VectorCraft's own tolerance.
const NO_HANDLE: f64 = 1e-9;

/// What to do about a path that cannot be used.
pub(crate) const UNUSABLE_HINT: &str = "Check its anchors, or remove stray objects far from the design.";

/// Why a path cannot be used: what [`path`] gives instead of one.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Unusable {
    /// A coordinate is not a number.
    NotANumber,
    /// It reaches past the working limit.
    TooFar,
    /// It draws nothing: no subpath has 2 anchors.
    Nothing,
}

impl Unusable {
    /// Why, as the start of a sentence.
    pub(crate) const fn why(self) -> &'static str {
        match self {
            Unusable::NotANumber => "A coordinate of its path is not a number",
            Unusable::TooFar => "It lies more than 10 m from the document's origin",
            Unusable::Nothing => "It draws nothing",
        }
    }
}

/// The subpaths of `data` in millimetres, or why they cannot be used. Each subpath charges `meter` one unit
/// and one more for each of its anchors.
pub(crate) fn path(data: &[&PathData], meter: &mut Meter) -> Result<Result<Path, Unusable>, Exhausted> {
    let mut subpaths = Vec::new();
    for sub in data.iter().flat_map(|d| &d.subpaths) {
        meter.charge(1 + sub.anchors.len() as u64)?;
        let Some(anchors) = sub.anchors.iter().map(points).collect::<Option<Vec<_>>>() else { return Ok(Err(Unusable::NotANumber)) };
        let n = anchors.len();
        // A single anchor draws nothing.
        let (Some(first), true) = (anchors.first(), n > 1) else { continue };
        let count = if sub.closed { n } else { n - 1 };
        let mut segments = Vec::with_capacity(count);
        for i in 0..count {
            let (Some(a), Some(b)) = (anchors.get(i), anchors.get((i + 1) % n)) else { continue };
            let straight = !handle(a.p, a.out) && !handle(b.p, b.h_in);
            let Some(segment) = segment(a, b, straight) else { return Ok(Err(Unusable::TooFar)) };
            segments.push(segment);
        }
        let Some(start) = mm(first.p) else { return Ok(Err(Unusable::TooFar)) };
        subpaths.push(Subpath { start, segments, closed: sub.closed });
    }
    Ok(if subpaths.is_empty() { Err(Unusable::Nothing) } else { Ok(Path { subpaths }) })
}

/// A point in millimetres as VectorCraft's coordinates, in points.
pub(crate) fn in_points(p: Point) -> [f64; 2] {
    [p.x() / MM_PER_POINT, p.y() / MM_PER_POINT]
}

/// An anchor's three points, in points: `None` when a coordinate is not a number.
#[derive(Clone, Copy)]
struct Points {
    p: (f64, f64),
    h_in: (f64, f64),
    out: (f64, f64),
}

fn points(anchor: &Anchor) -> Option<Points> {
    let at = |point: &WirePoint| point.0;
    let p = at(&anchor.p)?;
    let h_in = anchor.h_in.as_ref().map_or(Some(p), at)?;
    let out = anchor.h_out.as_ref().map_or(Some(p), at)?;
    Some(Points { p, h_in, out })
}

/// Whether `handle` is a handle of the anchor at `p`: farther from it than VectorCraft's tolerance, as
/// VectorCraft measures it (`Anchor::has_in` in its `crates/geom`), without a square root.
fn handle(p: (f64, f64), handle: (f64, f64)) -> bool {
    let (dx, dy) = (handle.0 - p.0, handle.1 - p.1);
    dx * dx + dy * dy > NO_HANDLE * NO_HANDLE
}

/// The segment from `a` to `b` in millimetres, a line when `straight`; `None` when it reaches past the
/// working limit.
fn segment(a: &Points, b: &Points, straight: bool) -> Option<Segment> {
    Some(if straight { Segment::Line(mm(b.p)?) } else { Segment::Cubic(mm(a.out)?, mm(b.h_in)?, mm(b.p)?) })
}

/// A point in points as one in millimetres, within the working limit.
fn mm((x, y): (f64, f64)) -> Option<Point> {
    let (x, y) = (x * MM_PER_POINT, y * MM_PER_POINT);
    if x.abs() > WORKING_LIMIT_MM || y.abs() > WORKING_LIMIT_MM {
        return None;
    }
    Point::new(x, y).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_point_at_the_working_limit_is_in_reach_and_the_next_one_out_is_not() {
        let mut x = WORKING_LIMIT_MM / MM_PER_POINT;
        while x.next_up() * MM_PER_POINT <= WORKING_LIMIT_MM {
            x = x.next_up();
        }
        assert_eq!(x * MM_PER_POINT, WORKING_LIMIT_MM);
        for (inside, outside) in [((x, 0.0), (x.next_up(), 0.0)), ((0.0, -x), (0.0, -x.next_up()))] {
            assert!(mm(inside).is_some() && mm(outside).is_none(), "{inside:?} {outside:?}");
        }
    }

    #[test]
    fn a_point_goes_back_to_where_it_came_from() {
        let p = mm((180.179_334_388_384_18, -930.039_763_579_936_7)).unwrap();
        let [x, y] = in_points(p);
        assert!((x - 180.179_334_388_384_18).abs() < 1e-12 && (y + 930.039_763_579_936_7).abs() < 1e-12, "{x} {y}");
    }
}
