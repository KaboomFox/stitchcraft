//! Machine units: positions and moves in the 0.1 mm units of machine files.
//!
//! Positions are rounded with [`Point::to_tenths`] — the one rounding StitchCraft applies, shared with
//! the previews (REQ-RND-001) and documented there — exactly once per position; moves are differences of
//! rounded positions, so rounding never accumulates along a design (REQ-FMT-002).

use core::ops::Sub;

use stitchcraft_core::Point;

/// A position in machine units (0.1 mm), y down, relative to the machine origin.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Units {
    /// x in 0.1 mm.
    pub x: i32,
    /// y in 0.1 mm, down.
    pub y: i32,
}

/// A move in machine units: the difference of two positions.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Delta {
    /// Along x, in 0.1 mm.
    pub dx: i32,
    /// Along y, in 0.1 mm, down.
    pub dy: i32,
}

impl Delta {
    /// No movement.
    pub const ZERO: Delta = Delta { dx: 0, dy: 0 };
}

impl Sub for Units {
    type Output = Delta;

    fn sub(self, from: Units) -> Delta {
        // Both operands are within ±MACHINE_LIMIT, so the difference cannot overflow.
        Delta { dx: self.x - from.x, dy: self.y - from.y }
    }
}

/// `point` in machine units; `None` when a coordinate is beyond ±10 m.
pub fn quantize_point(point: Point) -> Option<Units> {
    let (x, y) = point.to_tenths()?;
    Some(Units { x, y })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rounding_never_accumulates() {
        // 1,000 stitches of 2.54 mm: each hole within 0.05 mm of the plan, the last one exactly at 2540.
        let mut previous = Units::default();
        let mut total = 0i64;
        for i in 1..=1000 {
            let at = quantize_point(Point::new(2.54 * f64::from(i), 0.0).unwrap()).unwrap();
            total += i64::from((at - previous).dx);
            previous = at;
        }
        assert_eq!(total, 25_400);
    }
}
