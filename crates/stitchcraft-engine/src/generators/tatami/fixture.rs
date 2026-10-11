//! Parts the tatami tests share, drawn as the region gives them: an outline from its top left corner
//! down its left side, counter-clockwise on screen, and a hole the other way round.

// Test code may unwrap (clippy.toml allows it in tests).
#![allow(clippy::unwrap_used)]

use stitchcraft_core::Point;

use crate::normalize::region::Polygon;

pub(crate) fn p(x: f64, y: f64) -> Point {
    Point::new(x, y).unwrap()
}

/// A rectangle 10 wide and `height` tall.
pub(crate) fn rectangle(height: f64) -> Polygon {
    Polygon { outline: vec![p(0.0, 0.0), p(0.0, height), p(10.0, height), p(10.0, 0.0), p(0.0, 0.0)], holes: Vec::new() }
}

/// A 10 × 4 rectangle with a 2 × 2 hole whose left side is at x = 4 and top at y = 1. Along the outline,
/// 28 long, the left side runs from 0 to 4 and the right side from 14 to 18; along the hole, 8 long, its
/// right side runs from 2 to 4 and its left side from 6 to 8.
pub(crate) fn frame() -> Polygon {
    Polygon { holes: vec![vec![p(4.0, 1.0), p(6.0, 1.0), p(6.0, 3.0), p(4.0, 3.0), p(4.0, 1.0)]], ..rectangle(4.0) }
}
