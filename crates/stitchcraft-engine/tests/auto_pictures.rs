//! The test pictures sewn automatically (`REQ-AUTO-007`): how much of the edges where their shapes meet the
//! thread covers once pulled in, sewn as drawn and digitized, and what a stacked trace sews. The numbers
//! measured are recorded in `docs/src/design/image-to-embroidery.md` › Neighbours, and these cases keep
//! them from falling back.

// Test code may unwrap and index (clippy.toml allows it in tests).
#![allow(clippy::unwrap_used, clippy::panic, clippy::indexing_slicing)]

use stitchcraft_engine::auto::OVERLAP;
use stitchcraft_engine::design::Element;
use stitchcraft_plan::StitchPlan;
use stitchcraft_testkit::designs::{messages, planned};
use stitchcraft_testkit::metrics::edge_coverage;
use stitchcraft_testkit::pictures::{self, Picture, Trace};

/// `picture` planned as drawn, its shapes with settings, each a tatami fill; and digitized.
fn plans(picture: &Picture) -> [StitchPlan; 2] {
    let drawn: Vec<Element> = picture.elements.iter().map(|element| Element { automatic: false, ..element.clone() }).collect();
    [planned(drawn), planned(picture.elements.clone())].map(|outcome| {
        let said = messages(&outcome);
        outcome.plan.unwrap_or_else(|| panic!("{}: {said:?}", picture.name))
    })
}

/// How much of `picture`'s edges `plan` covers with the thread pulled in by the overlap: the pull the
/// overlap is there to make up for.
fn covered(picture: &Picture, plan: &StitchPlan) -> f64 {
    edge_coverage(&picture.edges, plan, OVERLAP)
}

#[test]
fn req_auto_007_digitized_the_edges_the_test_pictures_shapes_share_stay_covered_once_pulled_in() {
    for picture in pictures::all() {
        let [drawn, digitized] = plans(&picture);
        let (before, after) = (covered(&picture, &drawn), covered(&picture, &digitized));
        assert!(after >= 0.96, "{}: {before} as drawn, {after} digitized", picture.name);
    }
    // Abutting, the badge's shapes as drawn leave fabric where they meet; digitized, they overlap there.
    let badge = pictures::badge(Trace::Abutting);
    let [drawn, digitized] = plans(&badge);
    let (before, after) = (covered(&badge, &drawn), covered(&badge, &digitized));
    assert!(before < 0.92 && after > before + 0.05, "{before} as drawn, {after} digitized");
}

#[test]
fn req_auto_007_a_stacked_trace_sews_what_an_abutting_one_does() {
    let [stacked_drawn, stacked] = plans(&pictures::badge(Trace::Stacked));
    let [_, abutting] = plans(&pictures::badge(Trace::Abutting));
    let count = |plan: &StitchPlan| plan.stitches().count() as f64;
    // Its covered parts cut away, the stacked badge sews as many stitches as the abutting one, give or take
    // a hundredth, and fewer than as drawn, where its lower colours are sewn twice.
    assert!((count(&stacked) / count(&abutting) - 1.0).abs() < 0.01, "{} and {}", count(&stacked), count(&abutting));
    assert!(count(&stacked) < 0.96 * count(&stacked_drawn), "{} digitized, {} as drawn", count(&stacked), count(&stacked_drawn));
}
