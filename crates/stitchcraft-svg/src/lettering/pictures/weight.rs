//! How bold letters are (roadmap step M14.8, `docs/src/design/image-to-embroidery.md` › The font chosen by
//! the letters' weight, `REQ-AUTO-080`): the width of their strokes over their height, measured the same
//! way on outlines traced from a picture and on the satin columns of an embroidery font.
//!
//! **Why.** Lettering that takes the place of bold traced letters looks lighter than the picture when the
//! font's strokes are thinner than the letters'. So the font is chosen by a number that says how bold
//! each is, and the number has to mean the same for both.
//!
//! **The measure** (`samples`). A stroke is as wide as the way across it. From the middle of each piece of
//! an outline a ray goes straight in, as far as the outline's next edge. The widths found are weighed by
//! the length of the piece each was measured from, and the stroke's width is their median (`median`). The
//! long sides of a stroke outweigh its ends and its joints, whose rays run along a stroke and find its
//! length, so the median is the width of the strokes and not of what joins them.
//!
//! **A font** ([`of_font`]) is measured on its capital letters `A` to `Z`: each satin column's 2 rails
//! closed into an outline, as far as the column's drawing goes, without its pull compensation. Its height
//! is the median height of what those letters draw. A plug-in doesn't measure a font when it sews: the
//! number is written beside the font's name in the list of fonts built in, and a test measures it again.
//!
//! **Never crashes, bounded.** A ray is tested against every piece of the outlines, and each test costs a
//! unit of the meter.

use stitchcraft_core::{Budget, Diagnostic, Exhausted, Meter, math};
use stitchcraft_engine::design::Shape;
use stitchcraft_engine::generators::satin;

use super::frame::At;
use super::{bounds, flat};
use crate::font::Font;

/// The setting that makes a glyph's path a satin column.
const SATIN_COLUMN: &str = "satin_column";

/// How far a ray goes before an edge counts, in millimetres: past the piece it starts from.
const CLEAR: f64 = 1e-6;

/// At how many places each outline is measured: enough that the 2 long sides of the shortest stroke of a
/// letter get several, and few enough that a badge's 40 letters cost a plug-in little.
const RAYS: usize = 48;

/// How bold a font's letters are, at the font's own size.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Weight {
    /// How wide its capital letters' satin columns are, in millimetres: the median.
    pub stroke: f64,
    /// How tall its capital letters are drawn, in millimetres: the median.
    pub height: f64,
}

impl Weight {
    /// The width of a stroke in heights of a letter: the number fonts and traced letters are compared by.
    pub fn ratio(&self) -> f64 {
        self.stroke / self.height
    }
}

/// How far from `origin` the ray toward `toward` meets the edge from `c` to `d`, if it does past [`CLEAR`].
/// The edge's end `d` is no part of it, so a ray through a corner of an outline meets one edge there.
fn hit(origin: At, toward: At, c: At, d: At) -> Option<f64> {
    let edge = (d.0 - c.0, d.1 - c.1);
    let to = (c.0 - origin.0, c.1 - origin.1);
    let turn = toward.0 * edge.1 - toward.1 * edge.0;
    if turn == 0.0 {
        return None;
    }
    let far = (to.0 * edge.1 - to.1 * edge.0) / turn;
    let along = (to.0 * toward.1 - to.1 * toward.0) / turn;
    (far > CLEAR && (0.0..1.0).contains(&along)).then_some(far)
}

/// A closed outline: its edges, and the box round them.
struct Outline {
    edges: Vec<(At, At)>,
    around: [f64; 4],
}

impl Outline {
    /// The outline the closed polyline `ring` draws, if it has a point.
    fn of(ring: &[At]) -> Option<Outline> {
        let around = bounds(ring.iter().copied())?;
        let mut from = *ring.last()?;
        let mut edges = Vec::with_capacity(ring.len());
        for &to in ring {
            if to != from {
                edges.push((from, to));
            }
            from = to;
        }
        Some(Outline { edges, around })
    }

    /// Whether the ray from `origin` toward `toward` passes the outline's box by: the box lies behind
    /// it, or wholly to one side of it.
    fn missed(&self, origin: At, toward: At) -> bool {
        let [left, top, right, bottom] = self.around;
        let corners = [(left, top), (right, top), (right, bottom), (left, bottom)].map(|(x, y)| (x - origin.0, y - origin.1));
        let side = |sign: f64| corners.iter().all(|corner| sign * (toward.0 * corner.1 - toward.1 * corner.0) > 0.0);
        corners.iter().all(|corner| corner.0 * toward.0 + corner.1 * toward.1 < 0.0) || side(1.0) || side(-1.0)
    }
}

/// Adds to `into` the widths across the area the closed polylines `rings` enclose by the even-odd rule, each
/// with the length of outline it stands for. Each ring is measured at [`RAYS`] places, evenly spaced along
/// it. A ray goes into the area when it meets the outlines an odd number of times, and the width is how
/// far it goes to the first. A place whose rays meet nothing adds none.
pub(super) fn samples(rings: &[Vec<At>], meter: &mut Meter, into: &mut Vec<(f64, f64)>) -> Result<(), Exhausted> {
    let outlines: Vec<Outline> = rings.iter().filter_map(|ring| Outline::of(ring)).collect();
    for outline in &outlines {
        let lengths: Vec<f64> = outline.edges.iter().map(|&(a, b)| math::hypot(b.0 - a.0, b.1 - a.1)).collect();
        let step = lengths.iter().sum::<f64>() / RAYS as f64;
        if step <= 0.0 {
            continue;
        }
        // The places lie half a step from the ring's start and a step apart.
        let (mut next, mut passed) = (step / 2.0, 0.0);
        for (&(a, b), &length) in outline.edges.iter().zip(&lengths) {
            while next < passed + length {
                let share = (next - passed) / length;
                let place = (a.0 + (b.0 - a.0) * share, a.1 + (b.1 - a.1) * share);
                next += step;
                for side in [1.0, -1.0] {
                    let toward = (side * (a.1 - b.1) / length, side * (b.0 - a.0) / length);
                    let (mut met, mut across) = (0_usize, f64::INFINITY);
                    for other in &outlines {
                        meter.charge(1)?;
                        if other.missed(place, toward) {
                            continue;
                        }
                        meter.charge(u64::try_from(other.edges.len()).unwrap_or(u64::MAX))?;
                        for far in other.edges.iter().filter_map(|&(c, d)| hit(place, toward, c, d)) {
                            met += 1;
                            across = across.min(far);
                        }
                    }
                    if met % 2 == 1 {
                        into.push((across, step));
                        break;
                    }
                }
            }
            passed += length;
        }
    }
    Ok(())
}

/// The median of the widths `samples`, each weighed by its length: the least width that, with the narrower
/// ones, was measured from half the outline or more. `None` of no samples.
pub(super) fn median(mut samples: Vec<(f64, f64)>) -> Option<f64> {
    samples.sort_by(|a, b| a.0.total_cmp(&b.0));
    let half = samples.iter().map(|sample| sample.1).sum::<f64>() / 2.0;
    let mut below = 0.0;
    for &(width, length) in &samples {
        below += length;
        if below >= half {
            return Some(width);
        }
    }
    None
}

/// How bold the font `font` is, measured on the capital letters `A` to `Z` of its default direction within
/// `budget`: `None` for a font that draws none of them as a satin column. Fails with `SC-E0004` when the
/// work needs more than `budget` allows.
pub fn of_font(font: &Font, budget: &Budget) -> Result<Option<Weight>, Diagnostic> {
    measured(font, &mut budget.meter()).map_err(|_| super::exhausted(budget))
}

/// [`of_font`], within `meter`.
fn measured(font: &Font, meter: &mut Meter) -> Result<Option<Weight>, Exhausted> {
    let Some(variant) = font.default_variant() else { return Ok(None) };
    let (mut widths, mut heights) = (Vec::new(), Vec::new());
    for letter in 'A'..='Z' {
        meter.charge(1)?;
        let Some(glyph) = variant.glyphs.get(letter.to_string().as_str()) else { continue };
        let mut drawn: Option<[f64; 4]> = None;
        for element in glyph.design.elements() {
            let pieces = flat(element.shape.path(), meter)?;
            drawn = bounds(drawn.iter().flat_map(|&[left, top, right, bottom]| [(left, top), (right, bottom)]).chain(pieces.into_iter().flatten()));
            let Shape::Stroke { path, width, join, .. } = &element.shape else { continue };
            if !element.params.get(SATIN_COLUMN).is_some_and(crate::inkstitch::yes) {
                continue;
            }
            // What recognition says of the column is the font's reader's to say, not the measure's.
            let Some(column) = satin::rails(path, *width, *join, &mut Vec::new(), meter)? else { continue };
            let [first, second] = &column.rails;
            let ring: Vec<At> = first.iter().chain(second.iter().rev()).map(|point| (point.x(), point.y())).collect();
            samples(&[ring], meter, &mut widths)?;
        }
        if let Some([_, top, _, bottom]) = drawn {
            heights.push((bottom - top, 1.0));
        }
    }
    let (stroke, height) = (median(widths), median(heights));
    Ok(stroke.zip(height).filter(|(_, height)| *height > 0.0).map(|(stroke, height)| Weight { stroke, height }))
}

#[cfg(test)]
mod tests {
    use stitchcraft_core::Budget;

    use super::*;

    fn rect(x0: f64, y0: f64, x1: f64, y1: f64) -> Vec<At> {
        vec![(x0, y0), (x1, y0), (x1, y1), (x0, y1)]
    }

    fn sampled(rings: &[Vec<At>]) -> Vec<(f64, f64)> {
        let mut into = Vec::new();
        samples(rings, &mut Budget::DEFAULT.meter(), &mut into).unwrap();
        into
    }

    #[test]
    fn req_auto_080_a_ray_meets_an_edge_in_front_of_it_once() {
        let ray = |c, d| hit((1.0, 1.0), (1.0, 0.0), c, d);
        assert_eq!(ray((3.0, 0.0), (3.0, 4.0)), Some(2.0));
        assert_eq!(ray((3.0, 4.0), (3.0, 0.0)), Some(2.0));
        // Behind the ray, beside it, along it, and nearer than the piece the ray starts from.
        assert_eq!(ray((-3.0, 0.0), (-3.0, 4.0)), None);
        assert_eq!(ray((3.0, 2.0), (3.0, 4.0)), None);
        assert_eq!(ray((2.0, 1.0), (5.0, 1.0)), None);
        assert_eq!(ray((1.0, 0.0), (1.0, 4.0)), None);
        assert_eq!(ray((1.0 + CLEAR / 2.0, 0.0), (1.0 + CLEAR / 2.0, 4.0)), None);
        assert!(ray((1.0 + 2.0 * CLEAR, 0.0), (1.0 + 2.0 * CLEAR, 4.0)).is_some_and(|far| (far - 2.0 * CLEAR).abs() < 1e-12));
        // A corner belongs to the edge that starts at it.
        assert_eq!(ray((3.0, 1.0), (3.0, 4.0)), Some(2.0));
        assert_eq!(ray((3.0, 0.0), (3.0, 1.0)), None);
        // A slanted edge is met where the ray crosses it.
        assert_eq!(ray((2.0, 0.0), (4.0, 2.0)), Some(2.0));
    }

    #[test]
    fn req_auto_080_a_stroke_is_as_wide_as_the_way_across_its_long_sides() {
        // A bar 8 mm wide and 20 tall: 48 places a step of 56 / 48 mm apart, those on its long sides 8 mm
        // from the other side and those on its ends 20.
        let bar = sampled(&[rect(10.0, 30.0, 18.0, 50.0)]);
        assert_eq!(bar.len(), 48);
        assert!(bar.iter().all(|&(_, length)| (length - 56.0 / 48.0).abs() < 1e-12));
        let near = |width: f64| bar.iter().filter(|sample| (sample.0 - width).abs() < 1e-9).count();
        assert_eq!((near(8.0), near(20.0)), (34, 14));
        assert_eq!(median(bar), Some(8.0));
        // A square ring 2 mm wide, its hole a ring of its own: measured from both, the way across is 2 mm
        // everywhere but within 2 mm of a corner, where a ray from the outside runs along a side: at 4 of the 12
        // places on each outer side.
        let ring = sampled(&[rect(0.0, 0.0, 10.0, 10.0), rect(2.0, 2.0, 8.0, 8.0)]);
        assert_eq!(ring.len(), 96);
        assert_eq!(ring.iter().filter(|sample| (sample.0 - 2.0).abs() < 1e-9).count(), 48 + 32);
        assert_eq!(median(ring), Some(2.0));
        // 2 bars side by side are measured each for itself: a ray stops at its own bar's far side.
        let two = sampled(&[rect(0.0, 0.0, 1.0, 10.0), rect(3.0, 0.0, 6.0, 10.0)]);
        assert_eq!(two.len(), 96);
        assert!(two.iter().all(|sample| [1.0, 3.0, 10.0].iter().any(|width| (sample.0 - width).abs() < 1e-9)), "{two:?}");
        // An outline of no length, and none at all, have no width.
        assert!(sampled(&[vec![(1.0, 1.0), (1.0, 1.0)], Vec::new()]).is_empty());
    }

    #[test]
    fn req_auto_080_a_ray_passes_an_outline_by_when_its_box_is_behind_or_beside_it() {
        let square = Outline::of(&rect(0.0, 0.0, 10.0, 10.0)).unwrap();
        // Ahead, behind, and wholly to either side of the ray.
        assert!(!square.missed((-5.0, 5.0), (1.0, 0.0)));
        assert!(square.missed((20.0, 5.0), (1.0, 0.0)));
        assert!(square.missed((-5.0, -5.0), (1.0, 0.0)));
        assert!(square.missed((-5.0, 15.0), (1.0, 0.0)));
        // A box the ray only grazes along an edge, or starts on the far edge of, is not passed by.
        assert!(!square.missed((-5.0, 0.0), (1.0, 0.0)));
        assert!(!square.missed((10.0, 5.0), (1.0, 0.0)));
        // A slanted ray: the box lies to its right, where the ray mirrored in the x axis would go.
        let small = Outline::of(&rect(7.5, -6.5, 8.5, -5.5)).unwrap();
        assert!(small.missed((0.0, 0.0), (0.8, 0.6)));
        assert!(!small.missed((0.0, 0.0), (0.8, -0.6)));
    }

    #[test]
    fn req_auto_080_the_median_weighs_each_width_by_its_length() {
        assert_eq!(median(Vec::new()), None);
        assert_eq!(median(vec![(3.0, 2.0), (1.0, 1.0)]), Some(3.0));
        assert_eq!(median(vec![(3.0, 2.0), (2.0, 1.0), (1.0, 1.0)]), Some(2.0));
        assert_eq!(median(vec![(5.0, 1.0)]), Some(5.0));
    }

    #[test]
    fn req_auto_080_measuring_stops_with_the_budget() {
        let rings = [rect(0.0, 0.0, 10.0, 10.0), rect(2.0, 2.0, 8.0, 8.0)];
        // The first unit pays for looking at an outline's box, and the next 4 for its edges.
        for allowed in [0, 1, 4] {
            let budget = Budget { max_work: allowed, ..Budget::DEFAULT };
            assert!(samples(&rings, &mut budget.meter(), &mut Vec::new()).is_err(), "{allowed}");
        }
        assert!(samples(&rings, &mut Budget::DEFAULT.meter(), &mut Vec::new()).is_ok());
    }
}
