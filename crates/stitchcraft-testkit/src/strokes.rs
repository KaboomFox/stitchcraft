//! The strokes of the traced word's M and A (`stitchcraft_engine::testsheets::pictures::word`), as a person
//! draws them, and how far each satin stitch of a letter lies from square to the stroke it sews
//! (`REQ-AUTO-087`, `docs/src/design/image-to-embroidery.md` › Letters sewn as strokes).
//!
//! The digitizer finds a shape's strokes from its outline alone, and knows no letter. The cases know the
//! letters: each stroke here is the band a person would draw, its middle line and half its width, in the
//! word's heights from the letter's top-left corner, so that what is measured doesn't depend on what is
//! measured with.

use stitchcraft_core::Point;
use stitchcraft_plan::{Role, StitchKind, StitchPlan};

/// A stroke of a letter: the 2 ends of its middle line and half its width, in heights from the letter's
/// top-left corner, the y axis down.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Stroke {
    /// The ends of its middle line.
    pub ends: [(f64, f64); 2],
    /// Half its width, measured square to its middle line.
    pub half: f64,
}

/// Half the width of the word's strokes, in heights: its letters are drawn in strokes 0.18 wide.
const HALF: f64 = 0.09;

/// How many stitches at each end of a column's run of top stitches [`leans`] leaves out.
pub const ENDS: usize = 2;

/// Where the A starts in the word, in heights: after the M, 0.92 wide, and the gap between letters, 0.14.
pub const A_LEFT: f64 = 1.06;

/// The M's 4 strokes: its stems, and its middle strokes from its top corners to its baseline, their sides
/// 18.9° from upright, whose middle lines lie 0.0951 inside their upper sides along the top.
pub fn m() -> Vec<Stroke> {
    let stroke = |a: (f64, f64), b: (f64, f64)| Stroke { ends: [a, b], half: HALF };
    vec![
        stroke((0.09, 0.0), (0.09, 1.0)),
        stroke((0.1249, 0.0), (0.4678, 1.0)),
        stroke((0.7951, 0.0), (0.4522, 1.0)),
        stroke((0.83, 0.0), (0.83, 1.0)),
    ]
}

/// The A's 3 strokes: its legs, their sides at a slope of 0.35 across for 1 down, and its crossbar from
/// 0.6 to 0.78 down.
pub fn a() -> Vec<Stroke> {
    let stroke = |a: (f64, f64), b: (f64, f64)| Stroke { ends: [a, b], half: HALF };
    vec![stroke((0.09535, 1.0), (0.44535, 0.0)), stroke((0.76465, 1.0), (0.41465, 0.0)), stroke((0.15, 0.69), (0.71, 0.69))]
}

/// `strokes` placed for a word `height` millimetres tall whose letter starts `left` heights from the word's
/// left, the word's top-left corner at `origin`: in millimetres.
pub fn placed(strokes: &[Stroke], origin: (f64, f64), height: f64, left: f64) -> Vec<Stroke> {
    let at = |(u, v): (f64, f64)| (origin.0 + (left + u) * height, origin.1 + v * height);
    strokes.iter().map(|stroke| Stroke { ends: stroke.ends.map(at), half: stroke.half * height }).collect()
}

/// How far a stitch from `a` to `b` lies from square to `stroke`, in degrees: 0 across it, 90 along it.
pub fn lean(stroke: &Stroke, a: Point, b: Point) -> f64 {
    let [(x0, y0), (x1, y1)] = stroke.ends;
    let length = (x1 - x0).hypot(y1 - y0);
    let (ux, uy) = ((x1 - x0) / length, (y1 - y0) / length);
    let (sx, sy) = (b.x() - a.x(), b.y() - a.y());
    let (along, across) = ((sx * ux + sy * uy).abs(), (sx * uy - sy * ux).abs());
    along.atan2(across).to_degrees()
}

/// How far `p` lies outside `stroke`'s band, in millimetres: 0 inside it. The band is the stroke's middle
/// line between its ends, widened by half its width to each side.
pub fn outside(stroke: &Stroke, p: Point) -> f64 {
    let [(x0, y0), (x1, y1)] = stroke.ends;
    let length = (x1 - x0).hypot(y1 - y0);
    let (ux, uy) = ((x1 - x0) / length, (y1 - y0) / length);
    let (dx, dy) = (p.x() - x0, p.y() - y0);
    let along = dx * ux + dy * uy;
    let across = (dx * uy - dy * ux).abs();
    let beyond = if along < 0.0 { -along } else { (along - length).max(0.0) };
    (across - stroke.half).max(0.0).hypot(beyond)
}

/// Each satin stitch of the letter `id` in `plan` at least `shortest` millimetres long: where its middle lies
/// and how far it leans from square to the stroke it sews, the nearest of `strokes` to its middle. Where
/// strokes meet, a stitch sews the one it lies most nearly square to. A satin stitch is a top stitch of one of
/// the letter's elements, the letter's own or one named after it with `#2`, `#3` and so on, that zigzags:
/// the stitches before and after it, those it has, go back the other way, within 60°, each to a point of
/// its own. A run along the column, as a column that walks its middle sews, goes on the same way, also
/// where it turns at an end, and a triple run goes back to the point it came from. The [`ENDS`] stitches at
/// each end of a column's run are left out: they follow the column's end, as drawn, on the slant where the
/// letter's foot is.
pub fn leans(plan: &StitchPlan, id: &str, strokes: &[Stroke], shortest: f64) -> Vec<(Point, f64)> {
    let ours = |stitch: &stitchcraft_plan::Stitch| {
        let named = stitch.origin.element.and_then(|e| plan.element(e)).map(ToString::to_string);
        let mine = named.is_some_and(|name| name == id || name.strip_prefix(id).is_some_and(|rest| rest.starts_with('#')));
        (mine && stitch.origin.role == Role::Top && stitch.kind == StitchKind::Normal).then_some(stitch.origin.element)
    };
    let mut found = Vec::new();
    for block in &plan.blocks {
        // Each run of needle points of one element in a row.
        let mut runs: Vec<Vec<Point>> = Vec::new();
        let mut last = None;
        for stitch in &block.stitches {
            match ours(stitch) {
                Some(element) if last == Some(element) => runs.last_mut().unwrap().push(stitch.at),
                Some(element) => {
                    runs.push(vec![stitch.at]);
                    last = Some(element);
                }
                None => last = None,
            }
        }
        for points in &runs {
            // The 2 stitches at each end of a column follow its end as drawn.
            for k in ENDS..points.len().saturating_sub(1 + ENDS) {
                let (a, b) = (points[k], points[k + 1]);
                // The stitch next to it goes back within 60° of the way it came.
                let back = |c: Point, d: Point, from: Point| {
                    let dot = (d.x() - c.x()) * (b.x() - a.x()) + (d.y() - c.y()) * (b.y() - a.y());
                    dot < -0.5 * c.distance(d) * a.distance(b) && d.distance(from) > 0.05
                };
                let before = (k > 0).then(|| back(points[k - 1], a, b));
                let after = points.get(k + 2).map(|&c| back(b, c, a));
                let zigzags = before.unwrap_or(true) && after.unwrap_or(true) && (before.is_some() || after.is_some());
                if !zigzags || a.distance(b) < shortest {
                    continue;
                }
                let middle = a.lerp(b, 0.5);
                let nearest = strokes.iter().map(|stroke| outside(stroke, middle)).fold(f64::INFINITY, f64::min);
                let degrees = strokes
                    .iter()
                    .filter(|stroke| outside(stroke, middle) <= nearest + 1e-9)
                    .map(|stroke| lean(stroke, a, b))
                    .fold(f64::INFINITY, f64::min);
                found.push((middle, degrees));
            }
        }
    }
    found
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_stitch_across_a_stroke_leans_0_and_one_along_it_90() {
        let stem = Stroke { ends: [(0.0, 0.0), (0.0, 10.0)], half: 1.0 };
        let p = |x: f64, y: f64| Point::new(x, y).unwrap();
        assert!(lean(&stem, p(-1.0, 5.0), p(1.0, 5.0)).abs() < 1e-9);
        assert!((lean(&stem, p(0.0, 4.0), p(0.0, 6.0)) - 90.0).abs() < 1e-9);
        assert!((lean(&stem, p(-1.0, 4.0), p(1.0, 6.0)) - 45.0).abs() < 1e-9);
        assert_eq!(outside(&stem, p(0.5, 5.0)), 0.0);
        assert!((outside(&stem, p(3.0, 5.0)) - 2.0).abs() < 1e-9);
        assert!((outside(&stem, p(1.0, 13.0)) - 3.0).abs() < 1e-9);
        assert!((outside(&stem, p(0.0, -2.0)) - 2.0).abs() < 1e-9);
    }

    #[test]
    fn the_strokes_are_placed_in_millimetres() {
        let placed = placed(&m(), (1.0, 2.0), 10.0, A_LEFT);
        let [(x0, y0), (x1, y1)] = placed[0].ends;
        assert!((x0 - 12.5).abs() < 1e-9 && (x1 - 12.5).abs() < 1e-9 && y0 == 2.0 && (y1 - 12.0).abs() < 1e-9, "{placed:?}");
        assert!((placed[0].half - 0.9).abs() < 1e-12);
    }
}
