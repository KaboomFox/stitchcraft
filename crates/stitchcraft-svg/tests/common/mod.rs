//! What the SVG adapter's tests share: fixtures, reading, and comparing positions.

// Each test file uses some of these helpers, so each sees the others as unused.
#![allow(dead_code)]

use std::path::Path;

use stitchcraft_core::{Budget, Point};
use stitchcraft_engine::design::Element;
use stitchcraft_svg::{Svg, read};

/// One micrometre, in millimetres: how far a position may be from where the file puts it (`REQ-SVG-001`).
pub const UM: f64 = 0.001;

/// A fixture from `conformance/fixtures/svg/`.
pub fn fixture(name: &str) -> Vec<u8> {
    std::fs::read(Path::new(env!("CARGO_MANIFEST_DIR")).join("../../conformance/fixtures/svg").join(name)).unwrap()
}

/// `bytes` read with the default budget; reading must succeed.
pub fn svg(bytes: impl AsRef<[u8]>) -> Svg {
    read(bytes.as_ref(), &Budget::DEFAULT).unwrap()
}

/// The ids of the design's elements, in stitching order.
pub fn ids(svg: &Svg) -> Vec<&str> {
    svg.design.elements().iter().map(|e| e.id.as_str()).collect()
}

/// The codes and messages of the warnings, as users read them.
pub fn warnings(svg: &Svg) -> Vec<String> {
    svg.warnings.iter().map(ToString::to_string).collect()
}

/// Each subpath's start and the end points of its segments.
pub fn ends(element: &Element) -> Vec<Vec<Point>> {
    element.shape.path().subpaths.iter().map(|s| std::iter::once(s.start).chain(s.segments.iter().map(|seg| seg.end())).collect()).collect()
}

/// `p` is (`x`, `y`) to within a micrometre.
#[track_caller]
pub fn at(p: Point, x: f64, y: f64) {
    assert!((p.x() - x).abs() <= UM && (p.y() - y).abs() <= UM, "({}, {}) is not ({x}, {y})", p.x(), p.y());
}

/// The points `ps` are the points `expected`, to within a micrometre.
#[track_caller]
pub fn all_at(ps: &[Point], expected: &[(f64, f64)]) {
    assert_eq!(ps.len(), expected.len(), "{ps:?}");
    for (p, (x, y)) in ps.iter().zip(expected) {
        at(*p, *x, *y);
    }
}
