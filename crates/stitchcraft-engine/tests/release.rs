//! Plans a release build once got wrong, checked in the build they run in: the tests' debug build, and
//! the release build `stitch` and the plug-ins ship in, which CI's `perf` job runs them in
//! (`docs/src/design/guardrails.md`). A release build is optimized further, with one code unit per crate
//! and link-time optimization across crates, and a compiler can get that wrong where the debug build is
//! right. Run them as CI does with `cargo test --release -p stitchcraft-engine --test release`.
//!
//! Each test plans a shape that a release build planned wrong, and checks the plan: an ordinary test, which
//! a wrong build fails, by a crash or by stitches the shape doesn't call for.

// Test code may unwrap and index (clippy.toml allows it in tests).
#![allow(clippy::unwrap_used, clippy::panic, clippy::indexing_slicing)]

use stitchcraft_core::{Code, ElementId, Mm, Point};
use stitchcraft_engine::design::{Cap, Element, Join, Path, Segment, Shape, Subpath};
use stitchcraft_params::ParamSet;
use stitchcraft_testkit::designs::{RED, automatic, messages, p, planned};

/// The circle around (`x`, `y`) of radius `r`, as 4 cubic Béziers, closed.
fn circle(x: f64, y: f64, r: f64) -> Subpath {
    // The control points' distance that makes a quarter circle of a cubic Bézier.
    let k = 0.552_284_749_830_793_6 * r;
    Subpath {
        start: p(x + r, y),
        segments: vec![
            Segment::Cubic(p(x + r, y + k), p(x + k, y + r), p(x, y + r)),
            Segment::Cubic(p(x - k, y + r), p(x - r, y + k), p(x - r, y)),
            Segment::Cubic(p(x - r, y - k), p(x - k, y - r), p(x, y - r)),
            Segment::Cubic(p(x + k, y - r), p(x + r, y - k), p(x + r, y)),
        ],
        closed: true,
    }
}

#[test]
fn req_auto_003_a_closed_line_wider_than_the_circle_it_follows_is_filled_in_a_release_build_too() {
    // The pupils of TS-13C's face: 2 circles 1.8 mm across, stroked 2 mm wide, without settings. A satin
    // column 2 mm wide can't turn round a circle that small, so each fills the area its stroke paints,
    // 1.9 mm round its middle (`REQ-AUTO-003`). Rust 1.97.0's release build wrote past the end of a vector
    // making their columns' rails, and then crashed or sewed stitches 30 mm away.
    let (middles, painted) = ([p(16.0, 21.0), p(32.0, 21.0)], 0.9 + 1.0);
    let path = Path { subpaths: middles.iter().map(|m| circle(m.x(), m.y(), 0.9)).collect() };
    let pupils = Element {
        id: ElementId::new("pupils").unwrap(),
        name: None,
        shape: Shape::Stroke { path, width: Mm::new(2.0).unwrap(), join: Join::Round, cap: Cap::Butt },
        thread: RED,
        params: ParamSet::new(),
        automatic: false,
    };
    let outcome = planned(vec![automatic(pupils)]);
    let codes: Vec<Code> = outcome.diagnostics.iter().map(|d| d.code).collect();
    assert_eq!(codes, [Code::AutoDigitized, Code::AutoSatinFilled, Code::AutoSatinFilled], "{:?}", messages(&outcome));
    let points: Vec<Point> = outcome.plan.as_ref().unwrap().stitches().map(|s| s.at).collect();
    let near = |middle: Point| points.iter().filter(|q| q.distance(middle) <= painted + 1e-6).count();
    // Every needle point lies in one of the areas, and each area is sewn.
    assert_eq!(near(middles[0]) + near(middles[1]), points.len(), "{points:?}");
    assert!(near(middles[0]) > 10 && near(middles[1]) > 10, "{points:?}");
}
