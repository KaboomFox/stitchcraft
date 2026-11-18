//! Details sew last (`REQ-AUTO-096`, `REQ-AUTO-097`, M13.40): a shape that the shapes round it close in
//! on, smaller than they are, such as an eye, a nose or a mark on a patch, sews after them where that costs
//! no thread change, so that no fill's rows lie over it. Design:
//! `docs/src/design/image-to-embroidery.md` › Details last.
//!
//! The layers' rule (`REQ-AUTO-023`) already sews an area with a detail in one of its holes before the
//! detail. The cases here draw what that rule misses: a detail where 2 patches meet, which lies in no
//! area's hole, and an area that can't move back before its detail in paint order. A detail in the hole of
//! an area no wider than a satin column, which keeps its hole, is no miss: the column sews after it and
//! covers its edge, as an outline does, and the detail reaches under the column.

// Test code may unwrap and index (clippy.toml allows it in tests).
#![allow(clippy::unwrap_used, clippy::panic, clippy::indexing_slicing)]

use stitchcraft_core::{Budget, Code, Diagnostic, Severity};
use stitchcraft_engine::auto::digitized;
use stitchcraft_engine::design::{Design, DesignSettings, Element, Shape};
use stitchcraft_plan::{Rgb, Thread};
use stitchcraft_testkit::designs::{BLUE, along, automatic, fill, in_place, polylines, unbased};
use stitchcraft_testkit::pictures::{self, Trace};

const GREEN: Thread = Thread::new(Rgb::new(0, 160, 0));
const BLACK: Thread = Thread::new(Rgb::new(20, 20, 20));

/// An automatic area `id` of `rings`, sewn with `thread`: a hole's ring runs the other way round.
fn area(id: &str, rings: &[&[(f64, f64)]], thread: &Thread) -> Element {
    automatic(fill(id, rings, thread, &[]))
}

/// The square ring `size` wide from `(x, y)`, anticlockwise as drawn, or the other way round for a hole.
fn square(x: f64, y: f64, size: f64, hole: bool) -> Vec<(f64, f64)> {
    let ring = vec![(x, y), (x + size, y), (x + size, y + size), (x, y + size)];
    if hole { ring.into_iter().rev().collect() } else { ring }
}

/// The elements of `elements` in the order they sew, each its id and how it sews, `fill` or `stroke`, and
/// what is said with `code`.
fn sewn(elements: Vec<Element>, code: Code) -> (Vec<String>, Vec<String>) {
    let design = Design::new(unbased(elements), DesignSettings::default()).unwrap();
    let (out, said) = digitized(&design, &Budget::DEFAULT);
    let said: Vec<String> = said.iter().filter(|d: &&Diagnostic| d.code == code).map(ToString::to_string).collect();
    let kind = |element: &Element| if matches!(element.shape, Shape::Fill { .. }) { "fill" } else { "stroke" };
    (out.iter().map(|element| format!("{} {}", element.id.as_str(), kind(element))).collect(), said)
}

/// Where element `id` sews among `ids` ([`sewn`]).
fn at(ids: &[String], id: &str) -> usize {
    ids.iter().position(|sewn| sewn.split(' ').next() == Some(id)).unwrap()
}

/// 2 patches 30 mm square side by side, blue and green, and a black mark 12 mm square where they meet,
/// each patch with a notch where the mark is, as VectorCraft's Abutting method traces them. The mark is
/// drawn first.
fn patches() -> Vec<Element> {
    let left = [(0.0, 0.0), (30.0, 0.0), (30.0, 9.0), (24.0, 9.0), (24.0, 21.0), (30.0, 21.0), (30.0, 30.0), (0.0, 30.0)];
    let right = [(30.0, 0.0), (60.0, 0.0), (60.0, 30.0), (30.0, 30.0), (30.0, 21.0), (36.0, 21.0), (36.0, 9.0), (30.0, 9.0)];
    vec![area("mark", &[&square(24.0, 9.0, 12.0, false)], &BLACK), area("left", &[&left], &BLUE), area("right", &[&right], &GREEN)]
}

#[test]
fn req_auto_096_a_detail_where_2_patches_meet_sews_after_both() {
    let (ids, _) = sewn(patches(), Code::AutoOrder);
    assert!(at(&ids, "mark") > at(&ids, "left") && at(&ids, "mark") > at(&ids, "right"), "the mark sews last: {ids:?}");
}

#[test]
fn req_auto_096_a_detail_in_the_hole_of_a_ring_filled_sews_after_the_ring() {
    // A blue ring 3 mm wide round a black square 6 mm wide, on a green patch: the ring is no wider than a
    // satin column and keeps its hole, and the square is drawn first.
    let disc = square(27.0, 12.0, 6.0, false);
    let ring = [square(24.0, 9.0, 12.0, false), square(27.0, 12.0, 6.0, true)];
    let patch = [square(0.0, 0.0, 60.0, false), square(24.0, 9.0, 12.0, true)];
    let shapes = vec![area("disc", &[&disc], &BLACK), area("ring", &[&ring[0], &ring[1]], &BLUE), area("patch", &[&patch[0], &patch[1]], &GREEN)];
    let (ids, _) = sewn(shapes, Code::AutoOrder);
    assert!(
        ids.iter().any(|id| id == "ring fill") && at(&ids, "disc") > at(&ids, "ring"),
        "the ring is filled, and the square sews after it: {ids:?}"
    );
}

#[test]
fn req_auto_097_an_area_that_cant_move_back_still_sews_before_its_detail() {
    // A blue patch 40 mm square with a black mark 12 mm square in its hole, drawn first, and between them a
    // green line with settings across the patch: the patch can't move back past the line, whose box meets
    // its own, and its hole opens again.
    let patch = [square(0.0, 0.0, 40.0, false), square(14.0, 14.0, 12.0, true)];
    let line = Element { automatic: false, ..along("line", polylines(&[&[(2.0, 4.0), (38.0, 4.0)]]), &GREEN, &[]) };
    let shapes = vec![area("mark", &[&square(14.0, 14.0, 12.0, false)], &BLACK), line, area("patch", &[&patch[0], &patch[1]], &BLUE)];
    let (ids, _) = sewn(shapes, Code::AutoOrder);
    assert!(at(&ids, "mark") > at(&ids, "patch"), "the mark sews after the patch: {ids:?}");
}

#[test]
fn req_auto_096_the_marked_patch_sews_its_details_last_and_changes_thread_3_times() {
    // Traced either way, the mark sews after both halves and the dot after the left half, and the patch
    // changes thread 3 times, blue, green, gold and black, as few as its 4 threads need.
    for trace in [Trace::Abutting, Trace::Stacked] {
        let picture = pictures::marked(trace);
        let (ids, _) = sewn(picture.elements.clone(), Code::AutoOrder);
        let (left, right) = (at(&ids, "left"), at(&ids, "right"));
        assert!(at(&ids, "mark") > left.max(right) && at(&ids, "dot") > left, "{}: {ids:?}", picture.name);
        let design = Design::new(unbased(picture.elements), DesignSettings::default()).unwrap();
        let plan = in_place(&design).plan.unwrap();
        assert_eq!(plan.stats().color_changes, 3, "{}", picture.name);
    }
}

/// The ids of the elements of `plan`'s design in the order the plan sews them, and what is said with
/// `SC-I0907`.
fn planned(elements: Vec<Element>, budget: &Budget) -> (Option<Vec<String>>, Vec<String>) {
    let design = Design::new(unbased(elements), DesignSettings::default()).unwrap();
    let outcome = stitchcraft_engine::plan(&design, stitchcraft_plan::profiles::REFERENCE, budget);
    let said = outcome.diagnostics.iter().filter(|d| d.code == Code::AutoOrder).map(ToString::to_string).collect();
    (outcome.plan.map(|plan| plan.elements.iter().map(ToString::to_string).collect()), said)
}

/// What the guard says when it keeps the plan made without the details' rule.
const WORSE: &str = "the details made the plan worse";

#[test]
fn req_auto_098_the_marked_patch_keeps_its_details_last_as_its_plan_measures_no_worse() {
    // Traced either way, the plan that sews the details last is no thicker, with no more jumps or thread
    // changes, and is kept.
    for trace in [Trace::Abutting, Trace::Stacked] {
        let (ids, said) = planned(pictures::marked(trace).elements, &Budget::DEFAULT);
        let ids = ids.unwrap();
        let at = |id: &str| ids.iter().position(|sewn| sewn == id).unwrap();
        assert!(at("mark") > at("left").max(at("right")) && at("dot") > at("left"), "{ids:?} {said:?}");
        assert!(!said.iter().any(|said| said.contains(WORSE)), "{said:?}");
    }
}

#[test]
fn req_auto_098_a_plan_that_sews_thicker_with_its_details_last_keeps_the_order_found_without_them() {
    // The mark where 2 patches meet, sewn after them, lies on the overlaps that both reach under it, and
    // its plan's thickest spot has 4.12 fills where the plan with the mark first has 3.93: the mark sews
    // first, as it was drawn, and SC-I0907 says why.
    let (ids, said) = planned(patches(), &Budget::DEFAULT);
    assert_eq!(ids.unwrap(), ["mark", "left", "right"]);
    assert_eq!(
        said,
        [
            "info SC-I0907: 1 of 1 details sews before a larger area round it: sewn after it, it made the plan sew thicker, or with more jump \
             stitches or thread changes.",
            "info SC-I0907: Sewn after the larger areas round them, the details made the plan worse: its thickest spot 4.12 fills against \
             3.93, 3 jump stitches against 3 and 2 thread changes against 2. They sew in the order found without that rule."
        ]
    );
}

#[test]
fn req_auto_098_both_plans_charge_the_stitch_budget() {
    // Each plan has the budget's stitches. With the fewest that give the marked patch a plan, it sews its
    // details last, as with the default budget. A stitch less, neither plan fits: no plan comes, and the
    // error says why.
    let elements = pictures::marked(Trace::Abutting).elements;
    let with = |stitches: u32| {
        let design = Design::new(unbased(elements.clone()), DesignSettings::default()).unwrap();
        stitchcraft_engine::plan(&design, stitchcraft_plan::profiles::REFERENCE, &Budget { max_stitches: stitches, ..Budget::DEFAULT })
    };
    let (mut low, mut high) = (0, Budget::DEFAULT.max_stitches);
    while low < high {
        let middle = low + (high - low) / 2;
        if with(middle).plan.is_some() { high = middle } else { low = middle + 1 }
    }
    let ids: Vec<String> = with(low).plan.unwrap().elements.iter().map(ToString::to_string).collect();
    assert_eq!(ids, ["left", "ring", "right", "mark", "dot"]);
    let short = with(low - 1);
    assert!(short.plan.is_none() && short.diagnostics.iter().any(|said| said.severity() == Severity::Error), "{:?}", short.diagnostics);
}
