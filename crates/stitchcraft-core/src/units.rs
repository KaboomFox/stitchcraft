//! Millimetre lengths and points.
//!
//! StitchCraft computes in millimetres because embroidery parameters are specified in millimetres and
//! machine files store tenths of a millimetre: the numbers people read and the numbers we compute are
//! the same. Hosts use other units; adapters convert with the constants below.
//!
//! Both types reject non-finite values at construction, so code that holds an [`Mm`] or a [`Point`]
//! never has to check for NaN or infinity again ("validate at the edge, trust inside").

use crate::math;

/// Millimetres per inch.
pub const MM_PER_INCH: f64 = 25.4;
/// Millimetres per PostScript point (VectorCraft's document unit: 72 points per inch).
pub const MM_PER_POINT: f64 = MM_PER_INCH / 72.0;
/// Millimetres per SVG user unit at 96 units per inch (the CSS pixel, and Ink/Stitch's assumption).
pub const MM_PER_SVG_PX: f64 = MM_PER_INCH / 96.0;

/// Why a number could not become a length or a point.
#[derive(Clone, Copy, Debug, PartialEq, thiserror::Error)]
pub enum UnitError {
    /// The value was NaN or infinite.
    #[error("expected a finite number, got {0}")]
    NotFinite(f64),
}

/// A length in millimetres. Always finite.
#[derive(Clone, Copy, Debug, Default, PartialEq, PartialOrd)]
pub struct Mm(f64);

impl Mm {
    /// Zero millimetres.
    pub const ZERO: Mm = Mm(0.0);

    /// A length of `value` millimetres, or an error if `value` is not finite.
    pub fn new(value: f64) -> Result<Self, UnitError> {
        if value.is_finite() { Ok(Mm(value)) } else { Err(UnitError::NotFinite(value)) }
    }

    /// A length given in PostScript points (VectorCraft's unit).
    pub fn from_points(points: f64) -> Result<Self, UnitError> {
        Self::new(points * MM_PER_POINT)
    }

    /// A length given in SVG user units at 96 per inch.
    pub fn from_svg_px(px: f64) -> Result<Self, UnitError> {
        Self::new(px * MM_PER_SVG_PX)
    }

    /// The length in millimetres.
    pub const fn get(self) -> f64 {
        self.0
    }

    /// The length in PostScript points.
    pub fn to_points(self) -> f64 {
        self.0 / MM_PER_POINT
    }
}

/// A point in millimetres, with y pointing down (the SVG and VectorCraft convention). Always finite.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Point {
    x: f64,
    y: f64,
}

impl Point {
    /// The origin.
    pub const ORIGIN: Point = Point { x: 0.0, y: 0.0 };

    /// The point (`x`, `y`) in millimetres, or an error if either coordinate is not finite.
    pub fn new(x: f64, y: f64) -> Result<Self, UnitError> {
        match (x.is_finite(), y.is_finite()) {
            (true, true) => Ok(Point { x, y }),
            (false, _) => Err(UnitError::NotFinite(x)),
            (_, false) => Err(UnitError::NotFinite(y)),
        }
    }

    /// The x coordinate in millimetres.
    pub const fn x(self) -> f64 {
        self.x
    }

    /// The y coordinate in millimetres (down is positive).
    pub const fn y(self) -> f64 {
        self.y
    }

    /// Euclidean distance to `other`, in millimetres.
    pub fn distance(self, other: Point) -> f64 {
        math::hypot(other.x - self.x, other.y - self.y)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lengths_reject_non_finite_values() {
        assert!(Mm::new(f64::NAN).is_err());
        assert_eq!(Mm::new(f64::INFINITY), Err(UnitError::NotFinite(f64::INFINITY)));
        assert_eq!(Mm::new(2.5).map(Mm::get), Ok(2.5));
    }

    #[test]
    fn host_units_convert_to_millimetres() {
        // 72 points and 96 SVG px are both one inch.
        assert!((Mm::from_points(72.0).unwrap().get() - 25.4).abs() < 1e-12);
        assert!((Mm::from_svg_px(96.0).unwrap().get() - 25.4).abs() < 1e-12);
        assert!((Mm::new(25.4).unwrap().to_points() - 72.0).abs() < 1e-12);
    }

    #[test]
    fn non_finite_host_values_are_rejected() {
        assert_eq!(Mm::from_points(f64::INFINITY), Err(UnitError::NotFinite(f64::INFINITY)));
        assert!(Mm::from_svg_px(f64::NAN).is_err());
        // Converting to millimetres shrinks values, so the largest finite input stays finite.
        assert!(Mm::from_points(f64::MAX).is_ok_and(|mm| mm.get().is_finite()));
    }

    #[test]
    fn points_reject_non_finite_coordinates() {
        assert!(Point::new(f64::NAN, 0.0).is_err());
        assert_eq!(Point::new(0.0, f64::NEG_INFINITY), Err(UnitError::NotFinite(f64::NEG_INFINITY)));
        let p = Point::new(3.0, 4.0).unwrap();
        assert_eq!((p.x(), p.y()), (3.0, 4.0));
        assert_eq!(Point::ORIGIN.distance(p), 5.0);
    }
}
