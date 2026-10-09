//! Path data and the basic shapes, as subpaths of lines and Bézier curves, mapped into millimetres.
//!
//! Path data is read the way SVG viewers draw it (SVG 1.1 § 8.3, implementation notes F.5–F.6): relative
//! commands, implicit line-tos, the smooth-curve reflections, closepath, and elliptical arcs. Arcs are
//! turned into cubic Béziers here, at most 30° each, which stay within 0.4 micrometres of the true arc
//! for any radius up to a metre. Degenerate arcs follow the spec: identical end points draw nothing, a
//! zero radius draws a straight line, and radii too small for the end points grow until they fit. An
//! error in the data ends the path there and keeps what came before, as viewers do; the caller warns.
//!
//! Points stay in user units until [`to_mm`] maps a whole outline through the element's transform, so
//! an outline is accepted or rejected as a unit.

use std::f64::consts::PI;

use stitchcraft_core::budget::{Exhausted, Meter};
use stitchcraft_core::{Point, math};
use stitchcraft_engine::design::{Path, Segment, Subpath, WORKING_LIMIT_MM};
use svgtypes::{PathParser, PathSegment, PointsParser};

use crate::transform::Affine;

/// A point in user units.
pub type V = (f64, f64);

/// A segment in user units.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Seg {
    /// A line to the point.
    Line(V),
    /// A quadratic curve: control point, end.
    Quad(V, V),
    /// A cubic curve: two control points, end.
    Cubic(V, V, V),
}

/// A subpath in user units.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Sub {
    /// Where it starts.
    pub start: V,
    /// Its segments.
    pub segs: Vec<Seg>,
    /// Whether it closes back to `start`.
    pub closed: bool,
}

/// An outline read from path data: the subpaths, and the parser's complaint if the data had an error.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Outline {
    /// The subpaths before any error.
    pub subs: Vec<Sub>,
    /// What was wrong with the data, if anything.
    pub error: Option<String>,
}

/// The outline `data` draws. Every segment costs one unit of `meter`.
pub fn path_data(data: &str, meter: &mut Meter) -> Result<Outline, Exhausted> {
    let mut b = Builder::default();
    let mut error = None;
    for item in PathParser::from(data) {
        meter.charge(1)?;
        match item {
            Ok(segment) => b.segment(segment),
            Err(e) => {
                error = Some(e.to_string());
                break;
            }
        }
    }
    Ok(Outline { subs: b.finish(), error })
}

/// Turns path commands into subpaths, tracking the pen and the last control point.
#[derive(Default)]
struct Builder {
    subs: Vec<Sub>,
    current: Option<Sub>,
    pen: V,
    start: V,
    /// The previous command's second control point, for `S`; its control point, for `T`.
    last_cubic: Option<V>,
    last_quad: Option<V>,
}

impl Builder {
    fn segment(&mut self, segment: PathSegment) {
        let pen = self.pen;
        let at = |abs: bool, x: f64, y: f64| if abs { (x, y) } else { (pen.0 + x, pen.1 + y) };
        let (last_cubic, last_quad) = (self.last_cubic.take(), self.last_quad.take());
        match segment {
            PathSegment::MoveTo { abs, x, y } => {
                self.end_subpath();
                self.pen = at(abs, x, y);
                self.start = self.pen;
                self.current = Some(Sub { start: self.pen, ..Sub::default() });
            }
            PathSegment::LineTo { abs, x, y } => self.push(Seg::Line(at(abs, x, y))),
            PathSegment::HorizontalLineTo { abs, x } => self.push(Seg::Line((if abs { x } else { pen.0 + x }, pen.1))),
            PathSegment::VerticalLineTo { abs, y } => self.push(Seg::Line((pen.0, if abs { y } else { pen.1 + y }))),
            PathSegment::CurveTo { abs, x1, y1, x2, y2, x, y } => {
                let second = at(abs, x2, y2);
                self.push(Seg::Cubic(at(abs, x1, y1), second, at(abs, x, y)));
                self.last_cubic = Some(second);
            }
            PathSegment::SmoothCurveTo { abs, x2, y2, x, y } => {
                let first = last_cubic.map_or(pen, |c| reflect(c, pen));
                let second = at(abs, x2, y2);
                self.push(Seg::Cubic(first, second, at(abs, x, y)));
                self.last_cubic = Some(second);
            }
            PathSegment::Quadratic { abs, x1, y1, x, y } => {
                let control = at(abs, x1, y1);
                self.push(Seg::Quad(control, at(abs, x, y)));
                self.last_quad = Some(control);
            }
            PathSegment::SmoothQuadratic { abs, x, y } => {
                let control = last_quad.map_or(pen, |c| reflect(c, pen));
                self.push(Seg::Quad(control, at(abs, x, y)));
                self.last_quad = Some(control);
            }
            PathSegment::EllipticalArc { abs, rx, ry, x_axis_rotation, large_arc, sweep, x, y } => {
                let to = at(abs, x, y);
                for seg in arc(pen, rx, ry, x_axis_rotation, large_arc, sweep, to) {
                    self.push(seg);
                }
                self.pen = to;
            }
            PathSegment::ClosePath { .. } => {
                if let Some(sub) = self.current.as_mut() {
                    sub.closed = true;
                }
                self.end_subpath();
                self.pen = self.start;
            }
        }
    }

    /// Appends `seg` to the subpath being drawn, starting one at the pen after a closepath.
    fn push(&mut self, seg: Seg) {
        let start = self.pen;
        let sub = self.current.get_or_insert_with(|| Sub { start, ..Sub::default() });
        sub.segs.push(seg);
        self.pen = match seg {
            Seg::Line(end) | Seg::Quad(_, end) | Seg::Cubic(_, _, end) => end,
        };
    }

    fn end_subpath(&mut self) {
        if let Some(sub) = self.current.take().filter(|s| !s.segs.is_empty()) {
            self.subs.push(sub);
        }
    }

    fn finish(mut self) -> Vec<Sub> {
        self.end_subpath();
        self.subs
    }
}

/// `control` mirrored through `pen`: the implied first control point of a smooth curve.
fn reflect(control: V, pen: V) -> V {
    (2.0 * pen.0 - control.0, 2.0 * pen.1 - control.1)
}

/// The elliptical arc from `from` to `to` (SVG 1.1, F.6.5 and F.6.6) as cubic Béziers of at most 30°.
pub fn arc(from: V, rx: f64, ry: f64, rotation: f64, large_arc: bool, sweep: bool, to: V) -> Vec<Seg> {
    if from == to {
        return Vec::new();
    }
    let (rx, ry) = (rx.abs(), ry.abs());
    if rx == 0.0 || ry == 0.0 || !rx.is_finite() || !ry.is_finite() {
        return vec![Seg::Line(to)];
    }
    let (sin, cos) = math::sin_cos(math::to_radians(rotation));
    // The half chord in the ellipse's own axes: its length `n` and direction (`dx`, `dy`).
    let (hx, hy) = ((from.0 - to.0) / 2.0, (from.1 - to.1) / 2.0);
    let (x1, y1) = (cos * hx + sin * hy, -sin * hx + cos * hy);
    let n = math::hypot(x1, y1);
    if n == 0.0 {
        // End points too close to tell apart once halved.
        return vec![Seg::Line(to)];
    }
    let (dx, dy) = (x1 / n, y1 / n);
    // The rest is worked out where the ellipse is the unit circle (F.6.5 with the radii divided out),
    // which keeps tiny chords and huge radii from underflowing. There the half chord is `h` long.
    let h = n * math::hypot(dx / rx, dy / ry);
    if !large_arc && h < 1e-9 {
        // A small arc on a chord this short next to its radii bends by less than a billionth of the
        // chord: it is straight, and drawing it from a far-away centre would only add rounding.
        return vec![Seg::Line(to)];
    }
    // Radii too small for the end points grow until the chord is a diameter (F.6.6).
    let grow = h.max(1.0);
    let (rx, ry, h) = (rx * grow, ry * grow, h / grow);
    // The half chord's direction there, without dividing by the radii: (dx/rx, dy/ry) scaled by rx·ry.
    let norm = math::hypot(dx * ry, dy * rx);
    let (ex, ey) = (dx * ry / norm, dy * rx / norm);
    let (u, v) = (h * ex, h * ey);
    // The centre lies √(1 − h²) from the chord's midpoint, across the chord, on the side the flags choose.
    let rise = if large_arc == sweep { -1.0 } else { 1.0 } * (1.0 - h * h).max(0.0).sqrt();
    let (ox, oy) = (rise * ey, -rise * ex);
    let centre = (cos * ox * rx - sin * oy * ry + (from.0 + to.0) / 2.0, sin * ox * rx + cos * oy * ry + (from.1 + to.1) / 2.0);
    let start = math::atan2(v - oy, u - ox);
    let end = math::atan2(-v - oy, -u - ox);
    let mut sweep_angle = end - start;
    if sweep && sweep_angle < 0.0 {
        sweep_angle += 2.0 * PI;
    } else if !sweep && sweep_angle > 0.0 {
        sweep_angle -= 2.0 * PI;
    }
    if large_arc && sweep_angle.abs() < PI / 2.0 {
        // The end points sit at the same angle as far as numbers go: the large arc goes all the way round.
        sweep_angle += if sweep { 2.0 * PI } else { -2.0 * PI };
    }
    let pieces = (1..=12_u32).find(|n| f64::from(*n) * PI / 6.0 >= sweep_angle.abs() - 1e-12).unwrap_or(12);
    let step = sweep_angle / f64::from(pieces);
    // Each piece's control points lie along the tangents, `bulge` of the way (the standard cubic arc).
    let bulge = 4.0 / 3.0 * math::tan(step / 4.0);
    // A point of the ellipse at parameter `t`, and its derivative there.
    let point = |t: f64| {
        let (s, c) = math::sin_cos(t);
        (centre.0 + rx * c * cos - ry * s * sin, centre.1 + rx * c * sin + ry * s * cos)
    };
    let tangent = |t: f64| {
        let (s, c) = math::sin_cos(t);
        (-rx * s * cos - ry * c * sin, -rx * s * sin + ry * c * cos)
    };
    (0..pieces)
        .map(|i| {
            let (t1, t2) = (start + step * f64::from(i), start + step * f64::from(i + 1));
            let (p1, d1, d2) = (point(t1), tangent(t1), tangent(t2));
            // The last piece ends exactly where the arc does.
            let p2 = if i + 1 == pieces { to } else { point(t2) };
            Seg::Cubic((p1.0 + bulge * d1.0, p1.1 + bulge * d1.1), (p2.0 - bulge * d2.0, p2.1 - bulge * d2.1), p2)
        })
        .collect()
}

/// The outline of a rectangle, with rounded corners when both radii are positive (SVG 1.1 § 9.2).
pub fn rect(x: f64, y: f64, width: f64, height: f64, rx: f64, ry: f64) -> Vec<Sub> {
    let (rx, ry) = (rx.min(width / 2.0), ry.min(height / 2.0));
    if rx <= 0.0 || ry <= 0.0 {
        return vec![Sub {
            start: (x, y),
            segs: vec![Seg::Line((x + width, y)), Seg::Line((x + width, y + height)), Seg::Line((x, y + height))],
            closed: true,
        }];
    }
    let (right, bottom) = (x + width, y + height);
    let mut segs = Vec::new();
    let corner = |segs: &mut Vec<Seg>, from: V, to: V| segs.extend(arc(from, rx, ry, 0.0, false, true, to));
    segs.push(Seg::Line((right - rx, y)));
    corner(&mut segs, (right - rx, y), (right, y + ry));
    segs.push(Seg::Line((right, bottom - ry)));
    corner(&mut segs, (right, bottom - ry), (right - rx, bottom));
    segs.push(Seg::Line((x + rx, bottom)));
    corner(&mut segs, (x + rx, bottom), (x, bottom - ry));
    segs.push(Seg::Line((x, y + ry)));
    corner(&mut segs, (x, y + ry), (x + rx, y));
    vec![Sub { start: (x + rx, y), segs, closed: true }]
}

/// The outline of an ellipse (a circle when the radii are equal), as four quarter arcs from its right.
pub fn ellipse(cx: f64, cy: f64, rx: f64, ry: f64) -> Vec<Sub> {
    let quarters = [(cx, cy + ry), (cx - rx, cy), (cx, cy - ry), (cx + rx, cy)];
    let mut from = (cx + rx, cy);
    let mut segs = Vec::new();
    for to in quarters {
        segs.extend(arc(from, rx, ry, 0.0, false, true, to));
        from = to;
    }
    vec![Sub { start: (cx + rx, cy), segs, closed: true }]
}

/// The outline through the points of a `points` attribute: a polyline, or a polygon when `closed`.
pub fn polyline(points: &str, closed: bool) -> Vec<Sub> {
    let mut points = PointsParser::from(points);
    let Some(start) = points.next() else { return Vec::new() };
    let segs: Vec<Seg> = points.map(Seg::Line).collect();
    if segs.is_empty() { Vec::new() } else { vec![Sub { start, segs, closed }] }
}

/// `subs` mapped through `map` into millimetres, or why they cannot be: a number too large to compute
/// with, or a point more than 10 m from the origin.
pub fn to_mm(subs: &[Sub], map: Affine) -> Result<Path, String> {
    let point = |v: V| -> Result<Point, String> {
        let (x, y) = map.apply(v.0, v.1);
        if x.abs() > WORKING_LIMIT_MM || y.abs() > WORKING_LIMIT_MM {
            return Err("it lies more than 10 m from the document's origin".to_string());
        }
        Point::new(x, y).map_err(|_| "its numbers are too large to compute with".to_string())
    };
    let mut subpaths = Vec::with_capacity(subs.len());
    for sub in subs {
        let segments = sub
            .segs
            .iter()
            .map(|seg| {
                Ok(match *seg {
                    Seg::Line(end) => Segment::Line(point(end)?),
                    Seg::Quad(control, end) => Segment::Quad(point(control)?, point(end)?),
                    Seg::Cubic(first, second, end) => Segment::Cubic(point(first)?, point(second)?, point(end)?),
                })
            })
            .collect::<Result<Vec<_>, String>>()?;
        subpaths.push(Subpath { start: point(sub.start)?, segments, closed: sub.closed });
    }
    Ok(Path { subpaths })
}

#[cfg(test)]
mod tests {
    use stitchcraft_core::Budget;

    use super::*;

    fn outline(data: &str) -> Outline {
        path_data(data, &mut Budget::DEFAULT.meter()).unwrap()
    }

    fn ends(sub: &Sub) -> Vec<V> {
        sub.segs
            .iter()
            .map(|s| match *s {
                Seg::Line(e) | Seg::Quad(_, e) | Seg::Cubic(_, _, e) => e,
            })
            .collect()
    }

    /// The point at `t` of a cubic from `p0`.
    fn cubic_at(p0: V, seg: Seg, t: f64) -> V {
        let Seg::Cubic(c1, c2, p3) = seg else { panic!("not a cubic: {seg:?}") };
        let u = 1.0 - t;
        let w = [u * u * u, 3.0 * u * u * t, 3.0 * u * t * t, t * t * t];
        (w[0] * p0.0 + w[1] * c1.0 + w[2] * c2.0 + w[3] * p3.0, w[0] * p0.1 + w[1] * c1.1 + w[2] * c2.1 + w[3] * p3.1)
    }

    #[test]
    fn relative_commands_shorthands_and_closepath() {
        let o = outline("m 10 10 h 5 v 5 l -5 0 z m 20 0 L 40 10");
        assert_eq!(o.error, None);
        assert_eq!(o.subs.len(), 2);
        assert_eq!((o.subs[0].start, ends(&o.subs[0]), o.subs[0].closed), ((10.0, 10.0), vec![(15.0, 10.0), (15.0, 15.0), (10.0, 15.0)], true));
        // After closepath the pen is back at (10, 10); the relative move goes from there.
        assert_eq!((o.subs[1].start, ends(&o.subs[1]), o.subs[1].closed), ((30.0, 10.0), vec![(40.0, 10.0)], false));
        // Implicit line-tos after a move-to, and drawing on after closepath starts at the subpath's start.
        let o = outline("M 2 3 1 0 1 1 Z L 5 5");
        assert_eq!(ends(&o.subs[0]), [(1.0, 0.0), (1.0, 1.0)]);
        assert_eq!((o.subs[1].start, ends(&o.subs[1])), ((2.0, 3.0), vec![(5.0, 5.0)]));
    }

    #[test]
    fn smooth_curves_reflect_the_previous_control_point() {
        let o = outline("M 0 0 C 0 10 10 10 10 0 S 20 -10 20 0 Q 25 10 30 0 T 40 0");
        let segs = &o.subs[0].segs;
        assert_eq!(segs[1], Seg::Cubic((10.0, -10.0), (20.0, -10.0), (20.0, 0.0)));
        assert_eq!(segs[3], Seg::Quad((35.0, -10.0), (40.0, 0.0)));
        // Without a previous curve, the reflected point is the pen.
        assert_eq!(outline("M 0 0 S 5 5 10 0").subs[0].segs[0], Seg::Cubic((0.0, 0.0), (5.0, 5.0), (10.0, 0.0)));
        assert_eq!(outline("M 0 0 L 1 0 T 10 0").subs[0].segs[1], Seg::Quad((1.0, 0.0), (10.0, 0.0)));
    }

    #[test]
    fn req_svg_001_arcs_stay_within_a_micrometre_of_the_true_ellipse() {
        // A half ellipse, rotated 30°: every point of every cubic lies on the ellipse.
        let (rx, ry, rotation) = (80.0, 30.0, 30.0_f64);
        let (sin, cos) = math::sin_cos(math::to_radians(rotation));
        let from = (100.0 + rx * cos, 100.0 + rx * sin);
        let to = (100.0 - rx * cos, 100.0 - rx * sin);
        let segs = arc(from, rx, ry, rotation, false, true, to);
        assert_eq!(segs.len(), 6, "180° in pieces of at most 30°");
        let mut start = from;
        for seg in segs {
            for i in 0..=10 {
                let (x, y) = cubic_at(start, seg, f64::from(i) / 10.0);
                // Back into the ellipse's frame: (u/rx)² + (v/ry)² = 1 on the curve.
                let (dx, dy) = (x - 100.0, y - 100.0);
                let (u, v) = (cos * dx + sin * dy, -sin * dx + cos * dy);
                let radial = ((u / rx).powi(2) + (v / ry).powi(2)).sqrt();
                assert!((radial - 1.0).abs() * rx < 0.001, "{radial}");
            }
            start = match seg {
                Seg::Cubic(_, _, end) => end,
                other => panic!("not a cubic: {other:?}"),
            };
        }
        assert_eq!(start, to, "the arc ends exactly at its end point");
    }

    #[test]
    fn degenerate_arcs_follow_the_spec() {
        assert_eq!(arc((5.0, 5.0), 10.0, 10.0, 0.0, false, true, (5.0, 5.0)), []);
        assert_eq!(arc((0.0, 0.0), 0.0, 10.0, 0.0, false, true, (5.0, 5.0)), [Seg::Line((5.0, 5.0))]);
        assert_eq!(arc((0.0, 0.0), f64::INFINITY, 10.0, 0.0, false, true, (5.0, 5.0)), [Seg::Line((5.0, 5.0))]);
        // Radii too small: scaled up to a half circle through both points (centre at their midpoint).
        let segs = arc((0.0, 0.0), 1.0, 1.0, 0.0, false, true, (10.0, 0.0));
        let Seg::Cubic(_, _, mid) = segs[2] else { panic!() };
        assert!((mid.0 - 5.0).abs() < 1e-9 && (mid.1 + 5.0).abs() < 1e-9, "{mid:?}");
        // Negative radii are their absolute value.
        assert_eq!(arc((0.0, 0.0), -5.0, -5.0, 0.0, false, true, (10.0, 0.0)), arc((0.0, 0.0), 5.0, 5.0, 0.0, false, true, (10.0, 0.0)));
    }

    /// The end of piece `i` of an arc.
    fn end_of(segs: &[Seg], i: usize) -> V {
        let Seg::Cubic(_, _, end) = segs[i] else { panic!("not a cubic: {:?}", segs[i]) };
        end
    }

    fn near(a: V, b: V) -> bool {
        (a.0 - b.0).abs() < 1e-9 && (a.1 - b.1).abs() < 1e-9
    }

    #[test]
    fn the_flags_choose_one_of_four_arcs() {
        // A chord of 10 on a circle of radius 10: the centre is 8.66 above or below the chord.
        let rise = 100.0_f64.sqrt() - 75.0_f64.sqrt();
        // Large arc, positive sweep: 300°, round the far side, over the top at (5, −18.66).
        let large = arc((0.0, 0.0), 10.0, 10.0, 0.0, true, true, (10.0, 0.0));
        assert_eq!(large.len(), 10);
        assert!(near(end_of(&large, 4), (5.0, -10.0 - 75.0_f64.sqrt())), "{:?}", end_of(&large, 4));
        // Small arc, positive sweep: 60°, just over the chord.
        let small = arc((0.0, 0.0), 10.0, 10.0, 0.0, false, true, (10.0, 0.0));
        assert_eq!(small.len(), 2);
        assert!(near(end_of(&small, 0), (5.0, -rise)), "{:?}", end_of(&small, 0));
        // The other sweep mirrors both through the chord.
        assert!(near(end_of(&arc((0.0, 0.0), 10.0, 10.0, 0.0, false, false, (10.0, 0.0)), 0), (5.0, rise)));
        assert!(near(end_of(&arc((0.0, 0.0), 10.0, 10.0, 0.0, true, false, (10.0, 0.0)), 4), (5.0, 10.0 + 75.0_f64.sqrt())));
    }

    #[test]
    fn radii_too_small_grow_on_any_chord() {
        // rx 1, ry 2 on a diagonal chord: both grow by the same factor until the chord is a diameter,
        // and the half ellipse passes (7.5, −5), a quarter of the way round.
        let segs = arc((0.0, 0.0), 1.0, 2.0, 0.0, false, true, (10.0, 10.0));
        assert_eq!(segs.len(), 6);
        assert!(near(end_of(&segs, 2), (7.5, -5.0)), "{:?}", end_of(&segs, 2));
    }

    #[test]
    fn a_chord_tiny_next_to_its_radii() {
        // The small arc between points 1e-200 apart on a circle of radius 1 is straight; it is not a half
        // circle of radius 1, which an underflowing centre would make of it.
        assert_eq!(arc((0.0, 0.0), 1.0, 1.0, 0.0, false, true, (1e-200, 0.0)), [Seg::Line((1e-200, 0.0))]);
        assert_eq!(arc((0.0, 0.0), 1e300, 1e300, 0.0, false, true, (1e-300, 0.0)), [Seg::Line((1e-300, 0.0))]);
        // The large arc goes all the way round: here a circle of radius 1e10 above the points.
        let full = arc((0.0, 0.0), 1e10, 1e10, 0.0, true, true, (1e-300, 0.0));
        assert_eq!(full.len(), 12);
        let far = end_of(&full, 5);
        assert!(far.0.abs() < 1e-3 && (far.1 + 2e10).abs() < 1e-3, "{far:?}");
        // End points too close to tell apart once halved.
        assert_eq!(arc((0.0, 0.0), 1.0, 1.0, 0.0, true, true, (5e-324, 0.0)), [Seg::Line((5e-324, 0.0))]);
    }

    #[test]
    fn errors_keep_what_came_before() {
        let o = outline("M 0 0 L 10 0 L 10 x 10 L 0 10");
        assert_eq!(ends(&o.subs[0]), [(10.0, 0.0)]);
        assert!(o.error.is_some());
        let o = outline("L 10 10");
        assert!(o.subs.is_empty() && o.error.is_some(), "path data must start with a move-to");
        assert_eq!(outline("M 5 5"), Outline::default(), "a lone move-to draws nothing");
    }

    #[test]
    fn budgets_bound_the_work() {
        let mut meter = Budget { max_stitches: 1, max_work: 3 }.meter();
        assert_eq!(path_data("M 0 0 L 1 1 L 2 2 L 3 3 L 4 4", &mut meter), Err(Exhausted::Work));
    }

    #[test]
    fn basic_shapes() {
        let square = rect(1.0, 2.0, 10.0, 5.0, 0.0, 0.0);
        assert_eq!((square[0].start, ends(&square[0]), square[0].closed), ((1.0, 2.0), vec![(11.0, 2.0), (11.0, 7.0), (1.0, 7.0)], true));
        let rounded = rect(0.0, 0.0, 10.0, 6.0, 2.0, 20.0);
        // ry is clamped to half the height; each corner is three 30° pieces.
        assert_eq!(rounded[0].segs.len(), 4 + 4 * 3);
        assert_eq!(ends(&rounded[0]).last(), Some(&(2.0, 0.0)));
        // Every side and corner of a rectangle at (1, 2), 10 × 6, with corners 2 across and 1 down.
        let r = rect(1.0, 2.0, 10.0, 6.0, 2.0, 1.0);
        assert_eq!(r[0].start, (3.0, 2.0));
        // Each side is one line and each corner three arc pieces: the ends of the sides and corners.
        let points = ends(&r[0]);
        let expected = [(9.0, 2.0), (11.0, 3.0), (11.0, 7.0), (9.0, 8.0), (3.0, 8.0), (1.0, 7.0), (1.0, 3.0), (3.0, 2.0)];
        let picked: Vec<V> = [0, 3, 4, 7, 8, 11, 12, 15].iter().map(|i| points[*i]).collect();
        assert!(picked.iter().zip(expected).all(|(a, b)| near(*a, b)), "{picked:?}");
        // A radius wider than half the rectangle is clamped to half of it; one radius of zero is square.
        assert!(near(ends(&rect(0.0, 0.0, 10.0, 6.0, 8.0, 1.0)[0])[0], (5.0, 0.0)));
        assert_eq!(rect(0.0, 0.0, 10.0, 6.0, 0.0, 2.0)[0].segs.len(), 3);
        let circle = ellipse(0.0, 0.0, 10.0, 10.0);
        assert_eq!(circle[0].segs.len(), 12);
        assert_eq!(ends(&circle[0])[2], (0.0, 10.0), "a quarter turn clockwise on screen reaches the bottom");
        assert_eq!(polyline("0,0 10,0 10,10", true)[0].segs.len(), 2);
        assert!(polyline("0,0", false).is_empty());
    }

    #[test]
    fn mapping_into_millimetres_rejects_far_or_huge_points() {
        let subs = outline("M 0 0 L 96 0").subs;
        let mm = to_mm(&subs, Affine::scale(25.4 / 96.0, 25.4 / 96.0)).unwrap();
        assert_eq!(mm.subpaths[0].segments, [Segment::Line(Point::new(25.4, 0.0).unwrap())]);
        assert!(to_mm(&outline("M 0 0 L 20000 0").subs, Affine::IDENTITY).unwrap_err().contains("10 m"));
        // Exactly 10 m is still in reach, on either axis; a micrometre more is not.
        assert!(to_mm(&outline("M 10000 0 L 0 -10000").subs, Affine::IDENTITY).is_ok());
        assert!(to_mm(&outline("M 0 0 L 0 10000.001").subs, Affine::IDENTITY).is_err());
        assert!(to_mm(&outline("M 0 0 L -10000.001 0").subs, Affine::IDENTITY).is_err());
        assert!(to_mm(&outline("M 0 0 L 1e308 0").subs, Affine::scale(10.0, 10.0)).is_err());
    }
}
