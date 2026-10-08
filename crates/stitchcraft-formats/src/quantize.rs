//! Quantization: millimetres to the 0.1 mm units of machine files, exactly once per position.
//!
//! Machine files store relative moves in tenths of a millimetre. Rounding each *move* would let errors
//! accumulate along a design (a thousand 2.54 mm stitches drift by a whole millimetre), so StitchCraft
//! rounds each *absolute position* once and takes moves as differences of rounded positions: every needle
//! hole lands within 0.05 mm of where the plan put it, however long the design. Ties round to even, so
//! there is no systematic drift either. Multiplication and rounding are exact IEEE-754 operations, so
//! the result is the same on every platform (`docs/src/design/determinism.md`, REQ-FMT-002).

use core::ops::Sub;

use stitchcraft_core::Point;

/// The largest coordinate StitchCraft quantizes: ±10,000 mm, the data model's limit for any coordinate.
pub const LIMIT: i32 = 100_000;

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
        // Both operands are within ±LIMIT, so the difference cannot overflow.
        Delta { dx: self.x - from.x, dy: self.y - from.y }
    }
}

/// `mm` in machine units, rounded half to even; `None` beyond ±[`LIMIT`].
pub fn quantize(mm: f64) -> Option<i32> {
    let units = (mm * 10.0).round_ties_even();
    if units.abs() <= f64::from(LIMIT) {
        // In range and integral, so the conversion is exact.
        #[allow(clippy::cast_possible_truncation)]
        Some(units as i32)
    } else {
        None
    }
}

/// `point` in machine units; `None` when a coordinate is beyond ±[`LIMIT`].
pub fn quantize_point(point: Point) -> Option<Units> {
    Some(Units { x: quantize(point.x())?, y: quantize(point.y())? })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rounds_half_to_even() {
        // Values whose tenfold is exactly representable, so the tie is real.
        assert_eq!(quantize(0.25), Some(2));
        assert_eq!(quantize(0.75), Some(8));
        assert_eq!(quantize(1.25), Some(12));
        assert_eq!(quantize(-0.25), Some(-2));
        assert_eq!(quantize(-0.75), Some(-8));
        assert_eq!(quantize(2.54), Some(25));
        assert_eq!(quantize(0.0), Some(0));
    }

    #[test]
    fn refuses_coordinates_beyond_the_limit() {
        assert_eq!(quantize(10_000.0), Some(LIMIT));
        assert_eq!(quantize(-10_000.0), Some(-LIMIT));
        assert_eq!(quantize(10_000.1), None);
        assert_eq!(quantize(f64::MAX), None);
    }

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
