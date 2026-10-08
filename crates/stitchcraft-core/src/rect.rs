//! Axis-aligned rectangles: the bounds of a design or of one of its parts.
//!
//! Hoop checks, previews, thumbnails and machine-file headers all need "how big is this and where is
//! it". A [`Rect`] is built only from [`Point`]s, so its corners are finite and `min ≤ max` by
//! construction; there is no empty rectangle — "no points" is `None` from [`Rect::around`].

use crate::units::Point;

/// An axis-aligned rectangle in millimetres (y down), with `min` the top-left corner.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rect {
    min: Point,
    max: Point,
}

impl Rect {
    /// The smallest rectangle containing every point, or `None` when there are no points.
    pub fn around(points: impl IntoIterator<Item = Point>) -> Option<Rect> {
        let mut points = points.into_iter();
        let first = points.next()?;
        Some(points.fold(Rect { min: first, max: first }, Rect::include))
    }

    /// This rectangle grown to contain `point`.
    #[must_use]
    pub fn include(self, point: Point) -> Rect {
        Rect {
            min: Point::from_finite(self.min.x().min(point.x()), self.min.y().min(point.y())),
            max: Point::from_finite(self.max.x().max(point.x()), self.max.y().max(point.y())),
        }
    }

    /// The top-left corner (smallest x and y).
    pub const fn min(self) -> Point {
        self.min
    }

    /// The bottom-right corner (largest x and y).
    pub const fn max(self) -> Point {
        self.max
    }

    /// Extent along x, in millimetres.
    pub fn width(self) -> f64 {
        self.max.x() - self.min.x()
    }

    /// Extent along y, in millimetres.
    pub fn height(self) -> f64 {
        self.max.y() - self.min.y()
    }

    /// The centre. Halves are added rather than the sum halved, so huge coordinates cannot overflow.
    pub fn center(self) -> Point {
        Point::from_finite(self.min.x() / 2.0 + self.max.x() / 2.0, self.min.y() / 2.0 + self.max.y() / 2.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(x: f64, y: f64) -> Point {
        Point::new(x, y).unwrap()
    }

    #[test]
    fn no_points_have_no_bounds() {
        assert_eq!(Rect::around([]), None);
    }

    #[test]
    fn bounds_contain_every_point() {
        let r = Rect::around([p(1.0, 5.0), p(-3.0, 2.0), p(4.0, -1.0)]).unwrap();
        assert_eq!((r.min(), r.max()), (p(-3.0, -1.0), p(4.0, 5.0)));
        assert_eq!((r.width(), r.height()), (7.0, 6.0));
        assert_eq!(r.center(), p(0.5, 2.0));
    }

    #[test]
    fn huge_coordinates_do_not_overflow_the_centre() {
        let r = Rect::around([p(f64::MAX, f64::MAX), p(f64::MAX, f64::MAX)]).unwrap();
        assert!(r.center().x().is_finite());
    }
}
