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
use stitchcraft_core::{Code, Diagnostic, ElementId, Point, Rect};
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

/// What an element is, geometrically.
#[derive(Clone, Debug, PartialEq)]
pub enum Shape {
    /// An outline stitched along: running stitch and the other stroke methods.
    Stroke(Path),
    /// An area to fill, bounded by `path` under `rule`.
    Fill {
        /// The boundary.
        path: Path,
        /// Which parts are inside.
        rule: FillRule,
    },
}

impl Shape {
    /// The geometry, whichever kind of shape this is.
    pub fn path(&self) -> &Path {
        match self {
            Shape::Stroke(path) | Shape::Fill { path, .. } => path,
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

/// Settings for the whole design.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct DesignSettings {}

/// A design: elements in stitching order (the host's paint order, bottom first).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Design {
    elements: Vec<Element>,
    /// Settings for the whole design.
    pub settings: DesignSettings,
}

impl Design {
    /// A design of `elements`, checked: every id is unique and every point lies within
    /// [`WORKING_LIMIT_MM`] of the origin. A failure is a bug in the host adapter that built it, so it is
    /// `SC-E0009`; adapters drop what they cannot represent, with a diagnostic of their own.
    pub fn new(elements: Vec<Element>, settings: DesignSettings) -> Result<Design, Diagnostic> {
        let mut ids = BTreeSet::new();
        for element in &elements {
            if !ids.insert(&element.id) {
                return Err(Diagnostic::new(Code::InternalCheckFailed, format!("Two elements of the design have the id `{}`.", element.id)));
            }
            if element.shape.path().points().any(|p| p.x().abs() > WORKING_LIMIT_MM || p.y().abs() > WORKING_LIMIT_MM) {
                return Err(Diagnostic::new(Code::InternalCheckFailed, format!("Element `{}` lies more than 10 m from the origin.", element.id))
                    .with_element(element.id.clone()));
            }
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
            shape: Shape::Stroke(path),
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
}
