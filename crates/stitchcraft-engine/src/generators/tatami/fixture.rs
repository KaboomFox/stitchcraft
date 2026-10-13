//! Parts the tatami tests share, drawn as the region gives them: an outline from its top left corner
//! down its left side, counter-clockwise on screen, and a hole the other way round. Also how the tests
//! pin the work a question about a part costs.

// Test code may unwrap (clippy.toml allows it in tests).
#![allow(clippy::unwrap_used)]

use stitchcraft_core::{Budget, Exhausted, Meter, Point, SplitMix64};

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

/// Random closed rings: a square of 20 mm from (1, 2) to (21, 22), off the origin so that a grid over them
/// starts off it too, then `holes` small polygons of 3 to 22 sides inside it, which may overlap. From a few
/// sides in all to some hundreds, as a fill's part has, so that both a look at every side and the grid of
/// [`super::sides`] answer questions about them.
pub(crate) fn scattered(rng: &mut SplitMix64, holes: usize) -> Vec<Vec<Point>> {
    let mut out = vec![vec![p(1.0, 2.0), p(21.0, 2.0), p(21.0, 22.0), p(1.0, 22.0), p(1.0, 2.0)]];
    for _ in 0..holes {
        let (cx, cy, r) = (rng.next_in(3.0, 19.0), rng.next_in(4.0, 20.0), rng.next_in(0.1, 2.0));
        let n = 3 + rng.next_u64() % 20;
        let mut ring: Vec<Point> = (0..n)
            .map(|k| {
                let (s, c) = stitchcraft_core::math::sin_cos(std::f64::consts::TAU * (k as f64) / (n as f64));
                p(cx + r * c, cy + r * s)
            })
            .collect();
        ring.push(ring[0]);
        out.push(ring);
    }
    out
}

/// A question that charges a meter.
pub(crate) type Question<'a> = Box<dyn Fn(&mut Meter) -> Result<(), Exhausted> + 'a>;

/// The work each of `questions` charges. Each needs all of it: with any less, it runs out, wherever the
/// meter stops it.
pub(crate) fn costs(questions: &[Question<'_>]) -> Vec<u64> {
    let budget = |work: u64| Budget { max_stitches: 1, max_work: work }.meter();
    questions
        .iter()
        .map(|ask| {
            let mut meter = Budget::DEFAULT.meter();
            ask(&mut meter).unwrap();
            let work = Budget::DEFAULT.max_work - meter.work_left();
            assert!((0..work).all(|less| ask(&mut budget(less)).is_err()), "{work}");
            work
        })
        .collect()
}
