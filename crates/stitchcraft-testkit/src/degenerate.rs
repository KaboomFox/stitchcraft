//! The degenerate corpus: the paths that a generator must survive however they are sewn
//! (`docs/src/design/algorithms/README.md`, property 5, and `docs/src/design/conformance.md` › L2).
//!
//! **Why.** People draw shapes that collapse: a ring of points along one line, a rectangle a thousandth of a
//! millimetre wide, a figure eight, a node clicked twice, a ring pasted over itself. A file or a host may
//! hand over worse: a subpath of one point, a curve whose control points coincide, a shape 10 m from the
//! origin. Each generator has cases of its own for what it does with them, and this corpus is what every
//! stitch type is run over, as a fill, a stroke and a satin column, so that a new one is too: none may
//! panic, report a bug (`SC-E0009`), or spend more than its budget without saying so (`SC-E0004`).
//!
//! Each shape is a [`Path`] in millimetres, near the origin so that it fits the reference machine's hoop,
//! but for [`FAR`], which lies near the edge of where a design may lie at all. [`long`] gives the path of
//! 10⁶ points apart, since sewing it is a question of the budget.

use stitchcraft_core::Point;
use stitchcraft_engine::design::{Path, Segment, Subpath, WORKING_LIMIT_MM};

use crate::designs::p;

/// A shape of the corpus: its name, which a failing case reports, and its path.
pub struct Degenerate {
    /// What is degenerate about it, in a few words.
    pub name: &'static str,
    /// Its subpaths.
    pub path: Path,
}

/// How far from the origin [`FAR`] lies along each axis: just within where a design may lie (mm).
pub const FAR: f64 = WORKING_LIMIT_MM - 20.0;

/// The open polyline through `points`.
fn open(points: &[(f64, f64)]) -> Subpath {
    polyline(points, false)
}

/// The ring through `points`, closed back to the first.
fn ring(points: &[(f64, f64)]) -> Subpath {
    polyline(points, true)
}

fn polyline(points: &[(f64, f64)], closed: bool) -> Subpath {
    let (&(x, y), rest) = points.split_first().unwrap_or((&(0.0, 0.0), &[]));
    Subpath { start: p(x, y), segments: rest.iter().map(|&(x, y)| Segment::Line(p(x, y))).collect(), closed }
}

/// The square `side` mm across from (`x`, `y`).
fn square((x, y): (f64, f64), side: f64) -> Subpath {
    ring(&[(x, y), (x + side, y), (x + side, y + side), (x, y + side)])
}

/// A path of `subpaths`.
fn path(subpaths: Vec<Subpath>) -> Path {
    Path { subpaths }
}

/// The corpus, each shape once.
pub fn corpus() -> Vec<Degenerate> {
    let shape = |name, subpaths| Degenerate { name, path: path(subpaths) };
    let point = Subpath { start: p(5.0, 5.0), segments: Vec::new(), closed: false };
    let same = |q: Point| Segment::Cubic(q, q, q);
    vec![
        shape("a point", vec![point.clone()]),
        shape("a closed point", vec![Subpath { closed: true, ..point.clone() }]),
        shape("a line of no length", vec![open(&[(5.0, 5.0), (5.0, 5.0)])]),
        shape("a curve whose control points coincide", vec![Subpath { start: p(5.0, 5.0), segments: vec![same(p(5.0, 5.0))], closed: false }]),
        shape(
            "a curve that loops back to its start",
            vec![Subpath { start: p(0.0, 0.0), segments: vec![Segment::Cubic(p(10.0, 10.0), p(-10.0, 10.0), p(0.0, 0.0))], closed: false }],
        ),
        shape("a ring along one line", vec![ring(&[(0.0, 0.0), (10.0, 0.0), (5.0, 0.0)])]),
        shape("a line there and back", vec![open(&[(0.0, 0.0), (10.0, 0.0), (0.0, 0.0)])]),
        shape("a hair-thin rectangle", vec![ring(&[(0.0, 0.0), (10.0, 0.0), (10.0, 1e-6), (0.0, 1e-6)])]),
        shape("a sliver of a triangle", vec![ring(&[(0.0, 0.0), (10.0, 0.0), (10.0, 1e-9)])]),
        shape("a square a millionth of a millimetre across", vec![square((5.0, 5.0), 1e-6)]),
        shape(
            "a square with every point twice",
            vec![ring(&[(0.0, 0.0), (0.0, 0.0), (10.0, 0.0), (10.0, 0.0), (10.0, 10.0), (10.0, 10.0), (0.0, 10.0), (0.0, 10.0), (0.0, 0.0)])],
        ),
        shape("a figure eight", vec![ring(&[(0.0, 0.0), (10.0, 10.0), (10.0, 0.0), (0.0, 10.0)])]),
        shape("a ring that touches itself at a point", vec![ring(&[(0.0, 0.0), (5.0, 5.0), (10.0, 0.0), (10.0, 10.0), (5.0, 5.0), (0.0, 10.0)])]),
        shape(
            "a square with a spike of no width",
            vec![ring(&[(0.0, 0.0), (10.0, 0.0), (10.0, 5.0), (15.0, 5.0), (10.0, 5.0), (10.0, 10.0), (0.0, 10.0)])],
        ),
        shape("a ring twice over", vec![square((0.0, 0.0), 10.0), square((0.0, 0.0), 10.0)]),
        shape("2 rings a rounding error apart", vec![square((0.0, 0.0), 10.0), square((1e-9, 1e-9), 10.0)]),
        shape("a hole that touches its outline", vec![square((0.0, 0.0), 10.0), square((0.0, 2.0), 4.0)]),
        shape("a hole as large as its outline", vec![square((0.0, 0.0), 10.0), ring(&[(0.0, 0.0), (0.0, 10.0), (10.0, 10.0), (10.0, 0.0)])]),
        shape("2 rails that are one", vec![open(&[(0.0, 0.0), (10.0, 0.0)]), open(&[(0.0, 0.0), (10.0, 0.0)])]),
        shape("2 rails that cross", vec![open(&[(0.0, 0.0), (10.0, 4.0)]), open(&[(0.0, 4.0), (10.0, 0.0)])]),
        shape("a rail of one point", vec![open(&[(0.0, 0.0), (10.0, 0.0)]), point]),
        shape("1,000 points along one line", vec![open(&(0..1000).map(|k| (f64::from(k) / 100.0, 0.0)).collect::<Vec<_>>())]),
        shape("a square near the edge of where a design may lie", vec![square((FAR, FAR), 10.0)]),
    ]
}

/// A random path that tends to degenerate: 1 to 3 subpaths of 1 to 7 points on a grid of 2 mm squares 10 mm
/// across, so that points coincide and lie in line often, each moved by a rounding error or not, each
/// segment a line, or a curve whose control points lie on the grid too, each subpath open or closed. The
/// strategy for the property test that sews such paths every way (`tests/degenerate.rs`).
pub fn random() -> impl proptest::strategy::Strategy<Value = Path> {
    use proptest::prelude::*;
    let at = || {
        (0u8..=5, 0u8..=5, prop::bool::weighted(0.2)).prop_map(|(i, j, nudged)| {
            let nudge = if nudged { 1e-9 } else { 0.0 };
            p(f64::from(i) * 2.0 + nudge, f64::from(j) * 2.0)
        })
    };
    let segment = prop_oneof![3 => at().prop_map(Segment::Line), 1 => (at(), at(), at()).prop_map(|(c1, c2, to)| Segment::Cubic(c1, c2, to))];
    let subpath =
        (at(), proptest::collection::vec(segment, 0..7), any::<bool>()).prop_map(|(start, segments, closed)| Subpath { start, segments, closed });
    proptest::collection::vec(subpath, 1..=3).prop_map(path)
}

/// A ring of 10⁶ points round a circle 20 mm across, its points about 0.06 µm apart.
pub fn long() -> Path {
    let points: Vec<(f64, f64)> = (0..1_000_000)
        .map(|k| {
            let (sin, cos) = stitchcraft_core::math::sin_cos(std::f64::consts::TAU * f64::from(k) / 1e6);
            (10.0 + 10.0 * cos, 10.0 + 10.0 * sin)
        })
        .collect();
    path(vec![ring(&points)])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_shape_is_named_once_and_all_but_the_far_one_lie_near_the_origin() {
        let corpus = corpus();
        let mut names: Vec<&str> = corpus.iter().map(|d| d.name).collect();
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), corpus.len());
        let near = |q: Point| q.x().abs() <= 20.0 && q.y().abs() <= 20.0;
        for d in &corpus {
            let mut points = d.path.subpaths.iter().flat_map(|s| std::iter::once(s.start).chain(s.segments.iter().map(|segment| segment.end())));
            assert_eq!(points.all(near), d.name != "a square near the edge of where a design may lie", "{}", d.name);
        }
        assert_eq!(long().subpaths[0].segments.len(), 999_999);
    }
}
