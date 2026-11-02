//! The test pictures sewn automatically (`REQ-AUTO-007`, `REQ-AUTO-010`, `REQ-AUTO-023`): how much of the
//! edges where their shapes meet the thread covers once pulled in, sewn as drawn and digitized, what a
//! stacked trace sews, how often the thread changes and the needle jumps, and what pictures traced in layers
//! sew, by either method. The numbers measured are recorded in `docs/src/design/image-to-embroidery.md` ›
//! Neighbours, › Layers and › Sewing order, and these cases keep them from falling back.

// Test code may unwrap and index (clippy.toml allows it in tests).
#![allow(clippy::unwrap_used, clippy::panic, clippy::indexing_slicing)]

use std::collections::BTreeMap;

use stitchcraft_core::{Budget, Diagnostic};
use stitchcraft_engine::auto;
use stitchcraft_engine::design::{Design, DesignSettings, Element, Shape};
use stitchcraft_engine::generators::satin::SATIN_COLUMN;
use stitchcraft_engine::normalize::region;
use stitchcraft_plan::{Rgb, StitchPlan};
use stitchcraft_testkit::designs::{messages, planned, rules};
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
    edge_coverage(&picture.edges, plan, rules().auto_overlap_mm.get())
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
fn req_auto_010_the_test_pictures_sew_with_fewer_thread_changes_and_jumps_than_in_paint_order() {
    // No test picture sews with more thread changes or jumps digitized than as drawn.
    for picture in pictures::all() {
        let [drawn, digitized] = plans(&picture);
        let (before, after) = (drawn.stats(), digitized.stats());
        assert!(
            after.color_changes <= before.color_changes && after.jumps <= before.jumps,
            "{}: {before:?} as drawn, {after:?} digitized",
            picture.name
        );
    }
    // Drawn badge by badge, the row of badges goes red, blue, gold 3 times over: digitized, each thread
    // sews once, and the red discs, which touch, without a jump between them.
    for trace in [Trace::Abutting, Trace::Stacked] {
        let [drawn, digitized] = plans(&pictures::badges(trace));
        let (before, after) = (drawn.stats(), digitized.stats());
        assert_eq!([before.color_changes, after.color_changes, before.jumps, after.jumps], [8, 2, 9, 7], "{trace:?}");
    }
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

/// The elements `picture` sews, digitized, in the order they sew, and what is said of them.
fn digitized(picture: &Picture) -> (Vec<Element>, Vec<Diagnostic>) {
    let design = Design::new(picture.elements.clone(), DesignSettings::default()).unwrap();
    let (elements, said) = auto::digitized(&design, &Budget::DEFAULT);
    (elements.into_owned(), said)
}

/// The colour of the thread `picture`'s shape `id` is sewn with.
fn colour(picture: &Picture, id: &str) -> Rgb {
    picture.elements.iter().find(|element| element.id.as_str() == id).unwrap().thread.color
}

/// The places among `elements` of those sewn with `colour`, in order.
fn sewn_with(elements: &[Element], colour: Rgb) -> Vec<usize> {
    elements.iter().enumerate().filter(|(_, element)| element.thread.color == colour).map(|(at, _)| at).collect()
}

/// Whether `element` is a satin column.
fn satin(element: &Element) -> bool {
    matches!(element.shape, Shape::Stroke { .. }) && element.params.get(SATIN_COLUMN) == Some("true")
}

/// The area the fill `element` sews, in square millimetres.
fn area(element: &Element) -> f64 {
    let Shape::Fill { path, rule } = &element.shape else { panic!("`{}` is not a fill", element.id) };
    region::build(path, *rule, &mut Budget::DEFAULT.meter()).unwrap().region.area()
}

/// What `elements` sew with each thread's colour: each fill's area to a tenth of a square millimetre and each
/// line's or column's width to a hundredth of a millimetre, in order.
fn sewn_by_colour(elements: &[Element]) -> BTreeMap<[u8; 3], Vec<String>> {
    let mut sewn: BTreeMap<[u8; 3], Vec<String>> = BTreeMap::new();
    for element in elements {
        let what = match &element.shape {
            Shape::Fill { .. } => format!("fill of {:.1} mm²", area(element)),
            Shape::Stroke { width, .. } => format!("{} {:.2} mm wide", if satin(element) { "column" } else { "line" }, width.get()),
        };
        let colour = element.thread.color;
        sewn.entry([colour.r, colour.g, colour.b]).or_default().push(what);
    }
    sewn.values_mut().for_each(|what| what.sort());
    sewn
}

/// `picture` traced both ways sews alike once digitized: the same fills, columns and lines with each thread,
/// and as many stitches give or take `slack`. The traces draw their colours in different orders, and the
/// elements whose places in the order differ start and end elsewhere, which moves a few of their stitches.
fn alike(picture: fn(Trace) -> Picture, slack: f64) {
    let [abutting, stacked] = [Trace::Abutting, Trace::Stacked].map(|trace| sewn_by_colour(&digitized(&picture(trace)).0));
    assert_eq!(abutting, stacked);
    let [abutting, stacked] = [Trace::Abutting, Trace::Stacked].map(|trace| {
        let outcome = planned(picture(trace).elements);
        let said = messages(&outcome);
        outcome.plan.unwrap_or_else(|| panic!("{trace:?}: {said:?}")).stats().stitches
    });
    assert!(abutting.abs_diff(stacked) as f64 <= slack * abutting as f64, "abutting {abutting}, stacked {stacked}");
}

#[test]
fn req_auto_023_an_emblem_traced_stacked_sews_as_one_traced_abutting() {
    let drawn = pictures::emblem(Trace::Abutting);
    let [navy, white, red, gold] = ["navy", "white", "star", "bar-1"].map(|id| colour(&drawn, id));
    for trace in [Trace::Abutting, Trace::Stacked] {
        let (elements, said) = digitized(&pictures::emblem(trace));
        let named = |at: &[usize]| at.iter().map(|&k| elements[k].id.as_str().to_string()).collect::<Vec<_>>();
        // The navy square is one fill, the first: the white disc, the star and the bars on it sew after it.
        assert_eq!(sewn_with(&elements, navy), [0], "{trace:?}: {:?} {said:?}", named(&(0..elements.len()).collect::<Vec<_>>()));
        assert!(matches!(elements[0].shape, Shape::Fill { .. }), "{trace:?}");
        // The white disc sews, a fill, though a trace that stacks its colours draws it as part of the
        // background: the disc, 531 mm², less the star, 178 mm², which it reaches under.
        let whites = sewn_with(&elements, white);
        assert_eq!(whites.len(), 1, "{trace:?}: {:?}", named(&whites));
        let disc = area(&elements[whites[0]]);
        assert!((353.0..420.0).contains(&disc), "{trace:?}: {disc}");
        // The star sews after the disc, and the bars as 3 satin columns as wide as drawn, 3 mm.
        assert!(sewn_with(&elements, red).iter().all(|&at| at > whites[0]), "{trace:?}");
        let bars = sewn_with(&elements, gold);
        assert_eq!(bars.len(), 3, "{trace:?}: {:?}", named(&bars));
        for &at in &bars {
            let Shape::Stroke { width, .. } = elements[at].shape else { panic!("{trace:?}: {} is no column", elements[at].id) };
            assert!(satin(&elements[at]) && (width.get() - 3.0).abs() < 0.15, "{trace:?}: {} {width:?}", elements[at].id);
        }
    }
    // Each thread sews the same shapes either way, and the emblem as many stitches, give or take 2 %.
    alike(pictures::emblem, 0.02);
}

#[test]
fn req_auto_023_a_buttons_middle_is_one_fill_under_its_dots_inside_an_outline() {
    let drawn = pictures::button(Trace::Abutting);
    let [black, yellow, orange] = ["rim", "disc", "dot-1"].map(|id| colour(&drawn, id));
    for trace in [Trace::Abutting, Trace::Stacked] {
        let (elements, said) = digitized(&pictures::button(trace));
        let ids: Vec<&str> = elements.iter().map(|element| element.id.as_str()).collect();
        // The yellow is one fill, under the dots, which sew after it, though it is narrower than a satin column
        // between them.
        let middle = sewn_with(&elements, yellow);
        assert_eq!(middle.len(), 1, "{trace:?}: {ids:?} {said:?}");
        assert!(matches!(elements[middle[0]].shape, Shape::Fill { .. }), "{trace:?}");
        assert!(sewn_with(&elements, orange).iter().all(|&at| at > middle[0]), "{trace:?}: {ids:?}");
        // The rim is an outline, narrower than a satin column everywhere, and keeps its hole: a satin column
        // round the yellow, not a fill under it.
        let rim = sewn_with(&elements, black);
        assert!(!rim.is_empty() && rim.iter().all(|&at| satin(&elements[at])), "{trace:?}: {ids:?}");
    }
    alike(pictures::button, 0.02);
}
