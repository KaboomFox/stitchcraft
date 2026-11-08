//! A fill's linear gradient, placed as VectorCraft places it, for a linear gradient fill to sew
//! (`docs/src/design/vectorcraft-integration.md` › A gradient's geometry, M10.12).
//!
//! **What a gradient paint holds.** VectorCraft's gradient paint, `{"type": "gradient", "gradient": {"kind",
//! "stops"}, "geom"?, "angle"}`, keeps the gradient's colours apart from where it lies (`crates/color` of its
//! source: `GradientPaint`, `GradientGeom`, read for 0.17.0 to 0.21.0). `geom` gives the gradient's start and
//! end in document coordinates, in points, where VectorCraft moves them with the object. Without it,
//! VectorCraft fits the gradient to the object's bounds at `angle` each time it draws it: through the middle
//! of the bounds, as long as they are across along that way, the angle counter-clockwise as the canvas shows
//! it.
//!
//! **The stops.** Each stop's offset is kept from 0 to 1 and never before the stop before, its opacity
//! from 0 to 1, 1 when the file leaves it out, and its colour converted as a solid paint's is. A stop's
//! midpoint moves where VectorCraft draws the colours half mixed, which rows of whole colours can't follow,
//! and a midpoint other than half way is named (`SC-W0807`).

use stitchcraft_core::math::{sin_cos, to_radians};
use stitchcraft_core::units::MM_PER_POINT;
use stitchcraft_core::{Exhausted, Meter, Point};
use stitchcraft_engine::design::{Gradient, GradientStop, Path, Segment, within_working_limit};
use stitchcraft_params::StitchType;
use stitchcraft_plan::Rgb;

use crate::color;
use crate::wire::{Paint, Stop, WirePoint};

/// Whether a fill whose `fill_method` is `method` sews its linear gradient: a linear gradient fill does.
pub(crate) fn sews(method: Option<&str>) -> bool {
    method.is_some_and(|method| method.trim() == StitchType::LinearGradientFill.id())
}

/// A thread VectorCraft paints black: a colour that doesn't read.
const BLACK: Rgb = Rgb::new(0, 0, 0);

/// How far from half way a midpoint may lie and still be half way, as an `f32` written to JSON reads back.
const HALF_WAY: f64 = 1e-6;

/// A fill's linear gradient as [`linear`] reads it, and what of it isn't sewn.
pub(crate) struct Linear {
    /// The gradient, placed in the design's millimetres; `None` when it lies too far from the origin to sew.
    pub(crate) gradient: Option<Gradient>,
    /// What of it isn't sewn, each the message of a `SC-W0807`.
    pub(crate) notes: Vec<String>,
}

/// The linear gradient `paint` paints `path` with, placed in the design's millimetres as the module's docs
/// say. `None` when `paint` is no linear gradient of 2 stops or more: a gradient of one stop paints its
/// colour, as in Ink/Stitch. Fitting a gradient to the path's bounds charges `meter` for each of its
/// segments.
pub(crate) fn linear(paint: &Paint, path: &Path, meter: &mut Meter) -> Result<Option<Linear>, Exhausted> {
    let Some(gradient) = paint.gradient.as_ref().filter(|_| paint.kind == "gradient") else { return Ok(None) };
    if gradient.kind.as_deref().is_some_and(|kind| !kind.eq_ignore_ascii_case("linear")) || gradient.stops.len() < 2 {
        return Ok(None);
    }
    let mut notes = Vec::new();
    let mut stops: Vec<GradientStop> = Vec::with_capacity(gradient.stops.len());
    for stop in &gradient.stops {
        let before = stops.last().map_or(0.0, |stop| stop.offset);
        stops.push(GradientStop { offset: unit(stop.offset, 0.0).max(before), colour: colour(stop, &mut notes), opacity: unit(stop.opacity, 1.0) });
    }
    if gradient.stops.iter().any(|stop| stop.midpoint.is_some_and(|m| (m - 0.5).abs() > HALF_WAY)) {
        notes.push(
            "Its gradient's midpoints between stops are not sewn: the rows of each blend share out the 2 colours as if each midpoint were half \
             way."
                .to_string(),
        );
    }
    let placed = match &paint.geom {
        Some(geom) => mm(&geom.start).zip(mm(&geom.end)),
        None => fitted(path, paint.angle.unwrap_or(0.0), meter)?,
    };
    let Some((start, end)) = placed else { return Ok(None) };
    if !(within_working_limit(start) && within_working_limit(end)) {
        let note = "Its gradient reaches more than 10 m from the origin, and the fill is sewn without it.".to_string();
        return Ok(Some(Linear { gradient: None, notes: vec![note] }));
    }
    Ok(Some(Linear { gradient: Some(Gradient { start, end, stops }), notes }))
}

/// `value` kept from 0 to 1, or `default` when the file leaves it out or writes `null`.
fn unit(value: Option<f64>, default: f64) -> f64 {
    value.filter(|v| v.is_finite()).unwrap_or(default).clamp(0.0, 1.0)
}

/// A stop's colour, black when it doesn't read, which `notes` says.
fn colour(stop: &Stop, notes: &mut Vec<String>) -> Rgb {
    stop.color.as_ref().and_then(color::rgb).unwrap_or_else(|| {
        notes.push("A gradient stop's colour does not read, and it is sewn in black.".to_string());
        BLACK
    })
}

/// A point in points as one in millimetres; `None` for a coordinate VectorCraft wrote as `null`.
fn mm(point: &WirePoint) -> Option<Point> {
    let (x, y) = point.0?;
    Point::new(x * MM_PER_POINT, y * MM_PER_POINT).ok()
}

/// A gradient without a placement, fitted to the bounds of `path` at `degrees` as VectorCraft fits it: from
/// one side of the bounds to the other through their middle, along the way at `degrees` counter-clockwise
/// as the canvas shows it. `None` for a path without points.
fn fitted(path: &Path, degrees: f64, meter: &mut Meter) -> Result<Option<(Point, Point)>, Exhausted> {
    let Some((least, most)) = bounds(path, meter)? else { return Ok(None) };
    let (sin, cos) = sin_cos(to_radians(degrees));
    let way = (cos, -sin);
    let (width, height) = (most.x() - least.x(), most.y() - least.y());
    let half = (width * way.0.abs() + height * way.1.abs()) / 2.0;
    let (cx, cy) = ((least.x() + most.x()) / 2.0, (least.y() + most.y()) / 2.0);
    Ok(Point::new(cx - way.0 * half, cy - way.1 * half).ok().zip(Point::new(cx + way.0 * half, cy + way.1 * half).ok()))
}

/// The least and greatest corners of the box round `path` as it is drawn: round its curves, not their
/// control points, as VectorCraft measures an object it fits a gradient to. Each segment is a unit of
/// `meter`.
fn bounds(path: &Path, meter: &mut Meter) -> Result<Option<(Point, Point)>, Exhausted> {
    let mut points: Vec<(f64, f64)> = Vec::new();
    for subpath in &path.subpaths {
        let mut from = (subpath.start.x(), subpath.start.y());
        points.push(from);
        for segment in &subpath.segments {
            meter.charge(1)?;
            let xy = |p: Point| (p.x(), p.y());
            let to = xy(segment.end());
            match *segment {
                Segment::Line(_) => {}
                Segment::Quad(control, _) => {
                    let c = xy(control);
                    points.extend(turns(&[from, c, to]).map(|t| bezier(&[from, c, to], t)));
                }
                Segment::Cubic(first, second, _) => {
                    let controls = [from, xy(first), xy(second), to];
                    points.extend(turns(&controls).map(|t| bezier(&controls, t)));
                }
            }
            points.push(to);
            from = to;
        }
    }
    let Some(&first) = points.first() else { return Ok(None) };
    let (least, most) = points.iter().fold((first, first), |(a, b), p| ((a.0.min(p.0), a.1.min(p.1)), (b.0.max(p.0), b.1.max(p.1))));
    Ok(Point::new(least.0, least.1).ok().zip(Point::new(most.0, most.1).ok()))
}

/// The point of the Bézier curve with `controls` (3 or 4 of them) at `t`, by de Casteljau's steps.
fn bezier(controls: &[(f64, f64)], t: f64) -> (f64, f64) {
    let mut points = controls.to_vec();
    while points.len() > 1 {
        points = points
            .windows(2)
            .filter_map(|w| match *w {
                [a, b] => Some(((1.0 - t) * a.0 + t * b.0, (1.0 - t) * a.1 + t * b.1)),
                _ => None,
            })
            .collect();
    }
    points.first().copied().unwrap_or_default()
}

/// Where between 0 and 1, both left out, the Bézier curve with `controls` turns back along x or along y:
/// where its derivative is 0 on either axis.
fn turns(controls: &[(f64, f64)]) -> impl Iterator<Item = f64> {
    let mut found = Vec::new();
    for axis in [0, 1] {
        let c: Vec<f64> = controls.iter().map(|p| if axis == 0 { p.0 } else { p.1 }).collect();
        // The derivative's coefficients, as a polynomial in t: a t² + b t + c.
        let (a, b, k) = match *c.as_slice() {
            [p0, p1, p2] => (0.0, 2.0 * (p2 - 2.0 * p1 + p0), 2.0 * (p1 - p0)),
            [p0, p1, p2, p3] => (3.0 * (-p0 + 3.0 * p1 - 3.0 * p2 + p3), 6.0 * (p0 - 2.0 * p1 + p2), 3.0 * (p1 - p0)),
            _ => continue,
        };
        if a == 0.0 {
            if b != 0.0 {
                found.push(-k / b);
            }
            continue;
        }
        let disc = b * b - 4.0 * a * k;
        if disc >= 0.0 {
            let root = disc.sqrt();
            found.extend([(-b - root) / (2.0 * a), (-b + root) / (2.0 * a)]);
        }
    }
    found.into_iter().filter(|t| *t > 0.0 && *t < 1.0)
}

#[cfg(test)]
mod tests {
    use serde_json::json;
    use stitchcraft_core::Budget;
    use stitchcraft_engine::design::Subpath;

    use super::*;

    fn p(x: f64, y: f64) -> Point {
        Point::new(x, y).unwrap()
    }

    /// A path from the origin along `segments`.
    fn path(segments: Vec<Segment>) -> Path {
        Path { subpaths: vec![Subpath { start: p(0.0, 0.0), segments, closed: false }] }
    }

    #[test]
    fn the_bounds_lie_round_the_curves_not_their_control_points() {
        let meter = &mut Budget::DEFAULT.meter();
        // A quadratic curve whose control point lies at (5, 10) rises to y = 5, half way.
        let (least, most) = bounds(&path(vec![Segment::Quad(p(5.0, 10.0), p(10.0, 0.0))]), meter).unwrap().unwrap();
        assert!(least.distance(p(0.0, 0.0)) < 1e-12 && most.distance(p(10.0, 5.0)) < 1e-12, "{least:?} {most:?}");
        // A cubic one with controls at (0, 8) and (10, -8) turns at y = ±8 / (2√3), and runs on along x.
        let (least, most) = bounds(&path(vec![Segment::Cubic(p(0.0, 8.0), p(10.0, -8.0), p(10.0, 0.0))]), meter).unwrap().unwrap();
        let turn = 8.0 / (2.0 * 3f64.sqrt());
        assert!(least.distance(p(0.0, -turn)) < 1e-12 && most.distance(p(10.0, turn)) < 1e-12, "{least:?} {most:?}");
        // Lines, and a quadratic curve straight along one axis, are their ends.
        let (least, most) = bounds(&path(vec![Segment::Line(p(-3.0, 4.0)), Segment::Quad(p(-3.0, 6.0), p(-3.0, 8.0))]), meter).unwrap().unwrap();
        assert_eq!((least, most), (p(-3.0, 0.0), p(0.0, 8.0)));
        assert_eq!(bounds(&Path { subpaths: Vec::new() }, meter).unwrap(), None);
    }

    #[test]
    fn curves_away_from_the_origin_turn_where_they_turn_back() {
        // Each curve starts at (3, 7), its bounds round where it turns back along x or along y.
        let within = |segment: Segment, least: Point, most: Point| {
            let path = Path { subpaths: vec![Subpath { start: p(3.0, 7.0), segments: vec![segment], closed: false }] };
            let (a, b) = bounds(&path, &mut Budget::DEFAULT.meter()).unwrap().unwrap();
            assert!(a.distance(least) < 1e-12 && b.distance(most) < 1e-12, "{a:?} {b:?}");
        };
        // The curves of the case above, moved: rising to y = 12, and turning 8 / (2√3) either side of y = 7.
        within(Segment::Quad(p(8.0, 17.0), p(13.0, 7.0)), p(3.0, 7.0), p(13.0, 12.0));
        let turn = 8.0 / (2.0 * 3f64.sqrt());
        within(Segment::Cubic(p(3.0, 15.0), p(13.0, -1.0), p(13.0, 7.0)), p(3.0, 7.0 - turn), p(13.0, 7.0 + turn));
        // Turning back along x half way, at x = 8.
        within(Segment::Quad(p(13.0, 12.0), p(3.0, 17.0)), p(3.0, 7.0), p(8.0, 17.0));
        // A curve that would turn back before its start, at t = -1, doesn't: its ends bound it.
        within(Segment::Quad(p(4.0, 8.0), p(6.0, 10.0)), p(3.0, 7.0), p(6.0, 10.0));
    }

    #[test]
    fn fitting_a_gradient_charges_each_segment() {
        let paint: Paint = serde_json::from_value(json!({"type": "gradient", "gradient": {"stops": [
            {"offset": 0, "color": {"model": "rgb", "r": 1, "g": 0, "b": 0}},
            {"offset": 1, "color": {"model": "rgb", "r": 0, "g": 0, "b": 1}}
        ]}, "angle": 0}))
        .unwrap();
        let square = path(vec![Segment::Line(p(10.0, 0.0)), Segment::Line(p(10.0, 10.0)), Segment::Line(p(0.0, 10.0))]);
        let mut meter = Budget { max_work: 2, ..Budget::DEFAULT }.meter();
        assert!(linear(&paint, &square, &mut meter).is_err());
        // A kind left out is linear: through the middle, from the left side to the right.
        let read = linear(&paint, &square, &mut Budget::DEFAULT.meter()).unwrap().unwrap().gradient.unwrap();
        assert_eq!((read.start, read.end), (p(0.0, 5.0), p(10.0, 5.0)));
        // Without a path to fit to, there is no gradient.
        assert!(linear(&paint, &Path { subpaths: Vec::new() }, &mut Budget::DEFAULT.meter()).unwrap().is_none());
    }

    #[test]
    fn a_gradient_is_fitted_to_bounds_wherever_they_lie() {
        // The rectangle from (20, 30) to (30, 50): at 0° from its left side to its right through its middle, and
        // at 90° from its bottom to its top.
        let rectangle = Path {
            subpaths: vec![Subpath {
                start: p(20.0, 30.0),
                segments: vec![Segment::Line(p(30.0, 30.0)), Segment::Line(p(30.0, 50.0)), Segment::Line(p(20.0, 50.0))],
                closed: true,
            }],
        };
        let meter = &mut Budget::DEFAULT.meter();
        let (start, end) = fitted(&rectangle, 0.0, meter).unwrap().unwrap();
        assert!(start.distance(p(20.0, 40.0)) < 1e-9 && end.distance(p(30.0, 40.0)) < 1e-9, "{start:?} {end:?}");
        let (start, end) = fitted(&rectangle, 90.0, meter).unwrap().unwrap();
        assert!(start.distance(p(25.0, 50.0)) < 1e-9 && end.distance(p(25.0, 30.0)) < 1e-9, "{start:?} {end:?}");
    }

    #[test]
    fn a_midpoint_half_way_as_an_f32_writes_it_says_nothing() {
        // Read from text, as a point's coordinates are.
        let paint = |midpoint: f64| -> Paint {
            let paint = json!({"type": "gradient", "gradient": {"stops": [
                {"offset": 0, "color": {"model": "rgb", "r": 1, "g": 0, "b": 0}, "midpoint": midpoint},
                {"offset": 1, "color": {"model": "rgb", "r": 0, "g": 0, "b": 1}}
            ]}, "geom": {"start": [0, 0], "end": [10, 0]}});
            serde_json::from_str(&paint.to_string()).unwrap()
        };
        let notes = |midpoint: f64| linear(&paint(midpoint), &Path { subpaths: Vec::new() }, &mut Budget::DEFAULT.meter()).unwrap().unwrap().notes;
        assert!(notes(0.5).is_empty() && notes(f64::from(0.5_f32 + f32::EPSILON)).is_empty());
        assert_eq!(notes(0.501).len(), 1);
    }

    #[test]
    fn only_a_linear_gradient_fill_sews_a_gradient() {
        assert!(sews(Some(" linear_gradient_fill ")));
        assert!(!sews(Some("tatami_fill")) && !sews(None));
    }
}
