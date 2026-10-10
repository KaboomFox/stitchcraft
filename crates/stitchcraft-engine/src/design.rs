//! The engine's input: a design, independent of the host it came from.
//!
//! Hosts (the SVG adapter, the VectorCraft adapter) translate their documents into a [`Design`]: elements
//! in stitching order, each with its geometry in millimetres (y down), a thread and its parameters. The
//! engine never sees a host document, so every host gets the same stitches for the same design
//! (`docs/src/design/data-model.md`).
//!
//! Geometry stays exact here: lines and Bézier curves with their control points, already transformed
//! into millimetres. Flattening to stitches is the generators' job, with the tolerance their parameters
//! give, and regions are normalized into polygons with holes when fills arrive (roadmap M5.1).

use std::collections::BTreeSet;

use stitchcraft_core::units::MACHINE_LIMIT;
use stitchcraft_core::{Code, Diagnostic, ElementId, Mm, Point, Rect};
use stitchcraft_params::ParamSet;
use stitchcraft_plan::Thread;

/// How far from the design's origin geometry may lie, in millimetres: the reach of machine-file
/// coordinates (10 m), beyond which no position can be written.
pub const WORKING_LIMIT_MM: f64 = MACHINE_LIMIT as f64 / 10.0;

/// One piece of a subpath, from where the previous piece ended. Points are the control points and the
/// end point, in millimetres.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Segment {
    /// A straight line to the point.
    Line(Point),
    /// A quadratic Bézier curve: control point, end point.
    Quad(Point, Point),
    /// A cubic Bézier curve: first control point, second control point, end point.
    Cubic(Point, Point, Point),
}

impl Segment {
    /// Where the segment ends.
    pub const fn end(self) -> Point {
        match self {
            Segment::Line(end) | Segment::Quad(_, end) | Segment::Cubic(_, _, end) => end,
        }
    }

    /// Its control points and end point, in order.
    pub fn points(self) -> impl Iterator<Item = Point> {
        let (a, b, c) = match self {
            Segment::Line(end) => (None, None, end),
            Segment::Quad(control, end) => (Some(control), None, end),
            Segment::Cubic(first, second, end) => (Some(first), Some(second), end),
        };
        a.into_iter().chain(b).chain(std::iter::once(c))
    }
}

/// A connected run of segments from `start`; a closed subpath returns to `start` at its end.
#[derive(Clone, Debug, PartialEq)]
pub struct Subpath {
    /// Where it starts.
    pub start: Point,
    /// Its segments, in order.
    pub segments: Vec<Segment>,
    /// Whether it closes back to `start`.
    pub closed: bool,
}

/// One or more subpaths: an outline to stitch along, or the boundary of an area to fill.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Path {
    /// The subpaths, in drawing order.
    pub subpaths: Vec<Subpath>,
}

impl Path {
    /// Every point that defines the path: starts, control points and end points. The curves lie within
    /// their control points' hull, so these bound the path.
    pub fn points(&self) -> impl Iterator<Item = Point> + '_ {
        self.subpaths.iter().flat_map(|s| std::iter::once(s.start).chain(s.segments.iter().flat_map(|seg| seg.points())))
    }
}

/// Which parts of a self-overlapping outline are inside an area.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum FillRule {
    /// Inside where the outline winds around a point a non-zero number of times (SVG's default).
    #[default]
    NonZero,
    /// Inside where a ray from a point crosses the outline an odd number of times.
    EvenOdd,
}

/// How a stroke turns its corners, as Ink/Stitch reads `stroke-linejoin` and `stroke-miterlimit`
/// (`REQ-SVG-004`). A satin column drawn as one path turns its rails' corners so.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Join {
    /// A sharp corner, cut square where its point would lie farther from the path than `limit` times half
    /// the stroke's width: 4 when the join is `miter` and no limit is set, as SVG says, and 5 when the
    /// join is not set at all, as Ink/Stitch reads it.
    Miter {
        /// How far the point may reach, in half widths; at least 1.
        limit: f64,
    },
    /// A rounded corner.
    Round,
    /// A corner cut square.
    Bevel,
}

impl Join {
    /// The join of a stroke whose `stroke-linejoin` is not set, or names a join Ink/Stitch does not: a
    /// miter limited at 5.
    pub const UNSET: Join = Join::Miter { limit: 5.0 };
}

/// What an element is, geometrically.
#[derive(Clone, Debug, PartialEq)]
pub enum Shape {
    /// An outline stitched along: running stitch and the other stroke methods.
    Stroke {
        /// The outline.
        path: Path,
        /// The stroke's width, as the transforms scale it: how wide a satin column drawn as one path is.
        width: Mm,
        /// How the stroke turns its corners.
        join: Join,
    },
    /// An area to fill, bounded by `path` under `rule`.
    Fill {
        /// The boundary.
        path: Path,
        /// Which parts are inside.
        rule: FillRule,
    },
}

impl Shape {
    /// A stroke along `path` whose style says nothing: 1 SVG user unit wide, SVG's initial width, with the
    /// join of a stroke that sets none.
    pub fn stroke(path: Path) -> Shape {
        Shape::Stroke { path, width: Mm::SVG_PX, join: Join::UNSET }
    }

    /// The geometry, whichever kind of shape this is.
    pub fn path(&self) -> &Path {
        match self {
            Shape::Stroke { path, .. } | Shape::Fill { path, .. } => path,
        }
    }
}

/// One object of the design, stitched as a unit.
#[derive(Clone, Debug, PartialEq)]
pub struct Element {
    /// The host's stable id (`svg:path7:fill`).
    pub id: ElementId,
    /// The name the user gave it, if any.
    pub name: Option<String>,
    /// Its geometry.
    pub shape: Shape,
    /// The thread it is sewn with.
    pub thread: Thread,
    /// Its embroidery parameters, as the host stores them.
    pub params: ParamSet,
}

/// Settings for the whole design. Ink/Stitch keeps them in the document. The SVG adapter reads the
/// collapse length, the shortest stitch and the narrowest satin stroke from the file's metadata, and the
/// origin and stop position from Ink/Stitch's commands from roadmap M8. Until then those are unset.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DesignSettings {
    /// Moves between groups of the same thread no longer than this are sewn on rather than jumped, for
    /// elements that set no `min_jump_stitch_length_mm` (Ink/Stitch's `collapse_len_mm`).
    pub collapse_len: Mm,
    /// The design's shortest stitch, for elements that set no `min_stitch_length_mm` (Ink/Stitch's
    /// `min_stitch_len_mm`); the machine's is used when it is longer.
    pub min_stitch_len: Option<Mm>,
    /// The point that goes to the hoop's centre (Ink/Stitch's origin command); without one, the centre of
    /// the box around the stitches.
    pub origin: Option<Point>,
    /// Where the frame moves before each stop (Ink/Stitch's stop position command); without one, it stays.
    pub stop_position: Option<Point>,
    /// A satin column drawn as one path whose stroke is no wider than this is sewn as a stroke (Ink/Stitch's
    /// `min_satin_stroke_width_mm`).
    pub min_satin_stroke_width: Mm,
}

impl DesignSettings {
    /// The collapse length a design has unless it says otherwise: 3 mm, as in Ink/Stitch.
    pub const COLLAPSE_LEN: Mm = Mm::from_tenths(30);
    /// The narrowest stroke a satin column drawn as one path is sewn across, unless the design says
    /// otherwise: 1 mm, as in Ink/Stitch.
    pub const MIN_SATIN_STROKE_WIDTH: Mm = Mm::from_tenths(10);
}

impl Default for DesignSettings {
    fn default() -> Self {
        DesignSettings {
            collapse_len: DesignSettings::COLLAPSE_LEN,
            min_stitch_len: None,
            origin: None,
            stop_position: None,
            min_satin_stroke_width: DesignSettings::MIN_SATIN_STROKE_WIDTH,
        }
    }
}

/// A design: elements in stitching order (the host's paint order, bottom first).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Design {
    elements: Vec<Element>,
    /// Settings for the whole design.
    pub settings: DesignSettings,
}

impl Design {
    /// A design of `elements`, checked: every id is unique, every point (the settings' too) lies within
    /// [`WORKING_LIMIT_MM`] of the origin, no stroke is narrower than 0 or limits its miters below 1, and
    /// the settings' lengths are not negative. A failure is a bug in the host adapter that built it, so it
    /// is `SC-E0009`; adapters drop or correct what they cannot represent, with a diagnostic of their own.
    pub fn new(elements: Vec<Element>, settings: DesignSettings) -> Result<Design, Diagnostic> {
        let far = |p: Point| p.x().abs() > WORKING_LIMIT_MM || p.y().abs() > WORKING_LIMIT_MM;
        let mut ids = BTreeSet::new();
        for element in &elements {
            if !ids.insert(&element.id) {
                return Err(Diagnostic::new(Code::InternalCheckFailed, format!("Two elements of the design have the id `{}`.", element.id)));
            }
            if element.shape.path().points().any(far) {
                return Err(Diagnostic::new(Code::InternalCheckFailed, format!("Element `{}` lies more than 10 m from the origin.", element.id))
                    .with_element(element.id.clone()));
            }
            if let Shape::Stroke { width, join, .. } = element.shape
                && (width.get() < 0.0 || matches!(join, Join::Miter { limit } if !(limit >= 1.0 && limit.is_finite())))
            {
                let message = format!("Element `{}` has a stroke narrower than 0, or a miter limit below 1.", element.id);
                return Err(Diagnostic::new(Code::InternalCheckFailed, message).with_element(element.id.clone()));
            }
        }
        if settings.origin.into_iter().chain(settings.stop_position).any(far) {
            return Err(Diagnostic::new(Code::InternalCheckFailed, "The design's origin or stop position lies more than 10 m from the origin."));
        }
        if settings.collapse_len.get() < 0.0 || settings.min_stitch_len.is_some_and(|m| m.get() < 0.0) || settings.min_satin_stroke_width.get() < 0.0
        {
            let message = "The design's collapse length, shortest stitch or narrowest satin stroke is negative.";
            return Err(Diagnostic::new(Code::InternalCheckFailed, message));
        }
        Ok(Design { elements, settings })
    }

    /// The elements, in stitching order.
    pub fn elements(&self) -> &[Element] {
        &self.elements
    }

    /// The smallest rectangle around every element's defining points, if there are any.
    pub fn bounds(&self) -> Option<Rect> {
        Rect::around(self.elements.iter().flat_map(|e| e.shape.path().points()))
    }
}

#[cfg(test)]
mod tests {
    use stitchcraft_plan::Rgb;

    use super::*;

    fn p(x: f64, y: f64) -> Point {
        Point::new(x, y).unwrap()
    }

    fn element(id: &str, path: Path) -> Element {
        Element {
            id: ElementId::new(id).unwrap(),
            name: None,
            shape: Shape::stroke(path),
            thread: Thread::new(Rgb::new(0, 0, 0)),
            params: ParamSet::new(),
        }
    }

    fn line(from: Point, to: Point) -> Path {
        Path { subpaths: vec![Subpath { start: from, segments: vec![Segment::Line(to)], closed: false }] }
    }

    #[test]
    fn a_design_keeps_its_order_and_knows_its_bounds() {
        let curve = Path {
            subpaths: vec![Subpath { start: p(0.0, 0.0), segments: vec![Segment::Cubic(p(0.0, -5.0), p(10.0, 5.0), p(10.0, 0.0))], closed: true }],
        };
        let design = Design::new(vec![element("b", curve), element("a", line(p(-2.0, 1.0), p(3.0, 1.0)))], DesignSettings::default()).unwrap();
        assert_eq!(design.elements().iter().map(|e| e.id.as_str()).collect::<Vec<_>>(), ["b", "a"]);
        let bounds = design.bounds().unwrap();
        assert_eq!((bounds.width(), bounds.height()), (12.0, 10.0));
        assert_eq!(Segment::Quad(p(1.0, 1.0), p(2.0, 0.0)).points().count(), 2);
    }

    #[test]
    fn duplicate_ids_and_far_geometry_are_adapter_bugs() {
        let twice = vec![element("a", line(p(0.0, 0.0), p(1.0, 0.0))), element("a", line(p(0.0, 0.0), p(2.0, 0.0)))];
        assert_eq!(Design::new(twice, DesignSettings::default()).unwrap_err().code, Code::InternalCheckFailed);
        let far = vec![element("far", line(p(0.0, 0.0), p(WORKING_LIMIT_MM + 1.0, 0.0)))];
        let problem = Design::new(far, DesignSettings::default()).unwrap_err();
        assert_eq!((problem.code, problem.element.map(|e| e.to_string())), (Code::InternalCheckFailed, Some("far".to_string())));
        let edge = vec![element("edge", line(p(-WORKING_LIMIT_MM, 0.0), p(WORKING_LIMIT_MM, 0.0)))];
        assert!(Design::new(edge, DesignSettings::default()).is_ok());
        // The limit is the reach of machine files: 10 m either way.
        assert_eq!(WORKING_LIMIT_MM, 10_000.0);
    }

    #[test]
    fn strokes_are_never_narrower_than_0_nor_limit_their_miters_below_1() {
        let stroke = |width: f64, join: Join| {
            let Shape::Stroke { path, .. } = element("a", line(p(0.0, 0.0), p(1.0, 0.0))).shape else { panic!("a stroke") };
            Element { shape: Shape::Stroke { path, width: Mm::new(width).unwrap(), join }, ..element("a", Path::default()) }
        };
        for (width, join) in [
            (-0.1, Join::Round),
            (1.0, Join::Miter { limit: 0.9 }),
            (1.0, Join::Miter { limit: f64::INFINITY }),
            (1.0, Join::Miter { limit: f64::NAN }),
        ] {
            let problem = Design::new(vec![stroke(width, join)], DesignSettings::default()).unwrap_err();
            assert_eq!(
                (problem.code, problem.element.map(|e| e.to_string())),
                (Code::InternalCheckFailed, Some("a".to_string())),
                "{width} {join:?}"
            );
        }
        for (width, join) in [(0.0, Join::Miter { limit: 1.0 }), (2.0, Join::Bevel), (2.0, Join::UNSET)] {
            assert!(Design::new(vec![stroke(width, join)], DesignSettings::default()).is_ok(), "{width} {join:?}");
        }
        // Unless a host says otherwise, a stroke is 1 SVG user unit wide, with the join Ink/Stitch gives one
        // that sets none.
        assert_eq!(element("a", Path::default()).shape, Shape::Stroke { path: Path::default(), width: Mm::SVG_PX, join: Join::Miter { limit: 5.0 } });
    }

    #[test]
    fn settings_default_to_ink_stitch_s_and_are_checked_too() {
        let settings = DesignSettings::default();
        assert_eq!((settings.collapse_len.get(), settings.min_stitch_len, settings.origin, settings.stop_position), (3.0, None, None, None));
        assert_eq!(settings.min_satin_stroke_width.get(), 1.0);
        let one = || vec![element("a", line(p(0.0, 0.0), p(1.0, 0.0)))];
        let at = |x: f64| Some(p(x, 0.0));
        let bad = [
            DesignSettings { origin: at(WORKING_LIMIT_MM + 1.0), ..settings },
            DesignSettings { stop_position: at(-WORKING_LIMIT_MM - 1.0), ..settings },
            DesignSettings { collapse_len: Mm::new(-1.0).unwrap(), ..settings },
            DesignSettings { min_stitch_len: Some(Mm::new(-0.1).unwrap()), ..settings },
            DesignSettings { min_satin_stroke_width: Mm::new(-0.1).unwrap(), ..settings },
        ];
        for settings in bad {
            assert_eq!(Design::new(one(), settings).unwrap_err().code, Code::InternalCheckFailed, "{settings:?}");
        }
        let edge = DesignSettings {
            origin: at(WORKING_LIMIT_MM),
            stop_position: at(0.0),
            collapse_len: Mm::ZERO,
            min_stitch_len: Some(Mm::ZERO),
            min_satin_stroke_width: Mm::ZERO,
        };
        assert!(Design::new(one(), edge).is_ok());
    }
}
