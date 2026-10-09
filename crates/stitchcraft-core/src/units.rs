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
/// Millimetres per SVG user unit at 96 units per inch (the CSS pixel).
pub const MM_PER_SVG_PX: f64 = MM_PER_INCH / 96.0;

/// The largest coordinate StitchCraft rounds to machine units, in machine units (0.1 mm): ±10,000 mm,
/// the data model's limit for any coordinate.
pub const MACHINE_LIMIT: i32 = 100_000;

/// Lengths closer than this, in millimetres, count as equal: far below the 0.1 mm resolution of machine
/// files, so floating-point rounding in a length that is exactly at a limit does not cross it. Generators
/// place stitches with it and the plan checker checks them with it, so the two always agree.
pub const LENGTH_SLACK: f64 = 1e-9;

/// `mm` in machine units (0.1 mm), rounded half to even; `None` beyond ±[`MACHINE_LIMIT`].
///
/// This is the one rounding StitchCraft applies to positions. Writers use it for every position they
/// encode and previews for every position they draw, so a preview shows exactly the needle holes a
/// machine file makes (REQ-RND-001). Machine files store relative moves, but rounding each *move* would
/// let errors accumulate along a design (a thousand 2.54 mm stitches drift by a whole millimetre), so
/// each *absolute position* is rounded once and moves are differences of rounded positions: every hole
/// lands within 0.05 mm of the plan, however long the design. Ties round to even, so there is no
/// systematic drift either. Multiplying and rounding are correctly rounded IEEE-754 operations, so the
/// result is the same on every platform (`docs/src/design/determinism.md`, REQ-FMT-002).
pub fn to_tenths(mm: f64) -> Option<i32> {
    let units = (mm * 10.0).round_ties_even();
    if units.abs() <= f64::from(MACHINE_LIMIT) {
        // In range and integral, so the conversion is exact.
        #[allow(clippy::cast_possible_truncation)]
        Some(units as i32)
    } else {
        None
    }
}

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

    /// A length given in tenths of a millimetre, the resolution of machine files. Integers are always
    /// finite, so this constructor cannot fail and works in constants (machine profiles use it).
    pub const fn from_tenths(tenths: i32) -> Self {
        Mm(tenths as f64 / 10.0)
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

/// A width and a height in millimetres: a hoop, a comfort zone, the extent of a design.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Size {
    /// Extent along x.
    pub width: Mm,
    /// Extent along y.
    pub height: Mm,
}

impl Size {
    /// A `width` × `height` size.
    pub const fn new(width: Mm, height: Mm) -> Self {
        Size { width, height }
    }

    /// The same size turned 90 degrees.
    pub const fn rotated(self) -> Self {
        Size { width: self.height, height: self.width }
    }

    /// Whether something `width` × `height` millimetres fits inside this size without turning it.
    pub fn holds(self, width: f64, height: f64) -> bool {
        width <= self.width.0 && height <= self.height.0
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

    /// A point given in tenths of a millimetre (machine-file units). Integers are always finite, so this
    /// cannot fail; readers use it for every decoded position.
    pub fn from_tenths(x: i32, y: i32) -> Self {
        Point { x: f64::from(x) / 10.0, y: f64::from(y) / 10.0 }
    }

    /// The point in machine units (0.1 mm), each coordinate rounded with [`to_tenths`]: where a machine
    /// file puts the needle. `None` when a coordinate is beyond ±10 m.
    pub fn to_tenths(self) -> Option<(i32, i32)> {
        Some((to_tenths(self.x)?, to_tenths(self.y)?))
    }

    /// A point from coordinates the caller has already proven finite (for example, the minimum of two
    /// finite values). Crate-internal so that untrusted numbers always go through [`Point::new`].
    pub(crate) const fn from_finite(x: f64, y: f64) -> Self {
        Point { x, y }
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

    /// The point a fraction `t` of the way from this point to `other`. `t` is clamped to 0..=1, and a
    /// NaN `t` counts as 0, so the result lies between the two points and is finite like them; should
    /// rounding at the very edge of the `f64` range overflow, the nearer end is returned.
    pub fn lerp(self, other: Point, t: f64) -> Point {
        let t = if t.is_nan() { 0.0 } else { t.clamp(0.0, 1.0) };
        let (x, y) = (self.x * (1.0 - t) + other.x * t, self.y * (1.0 - t) + other.y * t);
        if x.is_finite() && y.is_finite() {
            Point::from_finite(x, y)
        } else if t < 0.5 {
            self
        } else {
            other
        }
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
    fn tenths_are_exact_machine_units() {
        assert_eq!(Mm::from_tenths(120).get(), 12.0);
        assert_eq!(Mm::from_tenths(3).get(), 0.3);
        assert_eq!(Mm::from_tenths(-2000).get(), -200.0);
    }

    #[test]
    fn points_between_points() {
        let (a, b) = (Point::new(1.0, 2.0).unwrap(), Point::new(3.0, -2.0).unwrap());
        assert_eq!(a.lerp(b, 0.5), Point::new(2.0, 0.0).unwrap());
        assert_eq!((a.lerp(b, 0.0), a.lerp(b, 1.0)), (a, b));
        assert_eq!((a.lerp(b, -1.0), a.lerp(b, 7.0), a.lerp(b, f64::NAN)), (a, b, a));
        // At the edge of the range: still finite.
        let (low, high) = (Point::new(-f64::MAX, f64::MAX).unwrap(), Point::new(f64::MAX, f64::MAX).unwrap());
        assert!(low.lerp(high, 0.5).x().is_finite() && high.lerp(high, 0.3).y().is_finite());
    }

    #[test]
    fn points_from_machine_units_are_exact_tenths() {
        let p = Point::from_tenths(-25, 1234);
        assert_eq!((p.x(), p.y()), (-2.5, 123.4));
        assert_eq!(Point::from_tenths(i32::MIN, i32::MAX).x(), -214_748_364.8);
    }

    #[test]
    fn machine_units_round_half_to_even() {
        // Values whose tenfold is exactly representable, so the tie is real.
        for (mm, tenths) in [(0.25, 2), (0.75, 8), (1.25, 12), (-0.25, -2), (-0.75, -8), (2.54, 25), (0.0, 0)] {
            assert_eq!(to_tenths(mm), Some(tenths), "{mm}");
        }
        assert_eq!(Point::new(1.04, -0.05).unwrap().to_tenths(), Some((10, 0)));
    }

    #[test]
    fn machine_units_refuse_coordinates_beyond_the_limit() {
        assert_eq!(to_tenths(10_000.0), Some(MACHINE_LIMIT));
        assert_eq!(to_tenths(-10_000.0), Some(-MACHINE_LIMIT));
        assert_eq!(to_tenths(10_000.1), None);
        assert_eq!(to_tenths(f64::MAX), None);
        assert_eq!(Point::new(0.0, 20_000.0).unwrap().to_tenths(), None);
    }

    #[test]
    fn sizes_hold_smaller_extents_and_rotate() {
        let hoop = Size::new(Mm::from_tenths(2000), Mm::from_tenths(1500));
        assert!(hoop.holds(200.0, 150.0));
        assert!(!hoop.holds(150.0, 200.0));
        assert!(hoop.rotated().holds(150.0, 200.0));
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
