//! Test pictures for automatic digitizing: designs drawn in code, as the test sheets are, that look like a
//! picture of flat colours once traced, and the edges where their shapes meet. No logo or picture of
//! anyone else's is used (`docs/src/design/image-to-embroidery.md` › Testing).
//!
//! A picture is traced one of 2 ways ([`Trace`]), and each sews the same areas once digitized: abutting,
//! as VectorCraft's Abutting method traces it, each area with a hole where the next one is; or stacked, as
//! its Overlapping method and Inkscape's Stack option trace it, each colour a whole shape over the ones
//! under it. Every shape is automatic: a shape without settings, as `--auto` reads it.

use stitchcraft_core::Point;
use stitchcraft_core::math::sin_cos;
use stitchcraft_engine::design::Element;
use stitchcraft_plan::{Rgb, Thread};

use crate::designs::{BLUE, RED, automatic, fill, p};

/// A ring of a shape: its corners, in order.
type Ring<'r> = &'r [(f64, f64)];

/// A gold thread.
pub const GOLD: Thread = Thread::new(Rgb::new(230, 180, 30));

/// How a picture was traced.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Trace {
    /// Each area with a hole where the next one is.
    Abutting,
    /// Each colour a whole shape over the ones under it.
    Stacked,
}

/// A test picture.
#[derive(Clone, Debug)]
pub struct Picture {
    /// Its name.
    pub name: &'static str,
    /// Its shapes, the lowest first, each automatic.
    pub elements: Vec<Element>,
    /// The edges where 2 of its shapes meet, as polylines: where fabric shows when they only meet.
    pub edges: Vec<Vec<Point>>,
}

/// Every test picture.
pub fn all() -> Vec<Picture> {
    vec![stripes(), badge(Trace::Abutting), badge(Trace::Stacked)]
}

/// 3 stripes side by side, 15 mm wide and 40 mm tall, red, blue and gold: 2 straight edges, upright, which
/// the rows of a tatami fill, level by default, end on.
pub fn stripes() -> Picture {
    let stripe = |id: &str, x: f64, thread: &Thread| automatic(fill(id, &[&rectangle((x, 0.0), (x + 15.0, 40.0))], thread, &[]));
    Picture {
        name: "stripes",
        elements: vec![stripe("red", 0.0, &RED), stripe("blue", 15.0, &BLUE), stripe("gold", 30.0, &GOLD)],
        edges: vec![vec![p(15.0, 0.0), p(15.0, 40.0)], vec![p(30.0, 0.0), p(30.0, 40.0)]],
    }
}

/// A badge, 50 mm across: a red disc, a blue square 20 mm wide on it, and a gold disc 10 mm across on the
/// square, traced as `trace` says. Its edges are the square's sides, level and upright, and the gold disc's
/// circle, at every angle to the rows.
pub fn badge(trace: Trace) -> Picture {
    let (outer, square, inner) = (circle((25.0, 25.0), 25.0, 96), rectangle((15.0, 15.0), (35.0, 35.0)), circle((25.0, 25.0), 5.0, 48));
    // Abutting, each lower area has a hole where the next one lies, drawn the other way round so that the
    // nonzero rule leaves it empty.
    let hole = |ring: &[(f64, f64)]| ring.iter().rev().copied().collect::<Vec<_>>();
    let (square_hole, inner_hole) = (hole(&square), hole(&inner));
    let [red, blue]: [Vec<Ring>; 2] = match trace {
        Trace::Stacked => [vec![&outer], vec![&square]],
        Trace::Abutting => [vec![&outer, &square_hole], vec![&square, &inner_hole]],
    };
    let name = match trace {
        Trace::Abutting => "badge, abutting",
        Trace::Stacked => "badge, stacked",
    };
    let ring = |points: &[(f64, f64)]| points.iter().chain(points.first()).map(|&(x, y)| p(x, y)).collect::<Vec<Point>>();
    Picture {
        name,
        elements: vec![
            automatic(fill("red", &red, &RED, &[])),
            automatic(fill("blue", &blue, &BLUE, &[])),
            automatic(fill("gold", &[&inner], &GOLD, &[])),
        ],
        edges: vec![ring(&square), ring(&inner)],
    }
}

/// The rectangle from `(x0, y0)` to `(x1, y1)`, its corners in order.
fn rectangle((x0, y0): (f64, f64), (x1, y1): (f64, f64)) -> Vec<(f64, f64)> {
    vec![(x0, y0), (x1, y0), (x1, y1), (x0, y1)]
}

/// A circle round `centre` of radius `r`, as `n` points.
fn circle((cx, cy): (f64, f64), r: f64, n: u32) -> Vec<(f64, f64)> {
    (0..n)
        .map(|k| {
            let (sin, cos) = sin_cos(std::f64::consts::TAU * f64::from(k) / f64::from(n));
            (cx + r * cos, cy + r * sin)
        })
        .collect()
}
