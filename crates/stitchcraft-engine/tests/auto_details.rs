//! Details sew last (`REQ-AUTO-096`, `REQ-AUTO-097`, M13.40): a shape that the shapes round it close in
//! on, smaller than they are, such as an eye, a nose or a mark on a patch, sews after them where that costs
//! no thread change, so that no fill's rows lie over it. The rule runs when the design's lowest automatic
//! shape turns `auto_details_last` on, which is off by default (`REQ-AUTO-098`): each case here turns it on,
//! but the one that checks the default. Design: `docs/src/design/image-to-embroidery.md` › Details last.
//!
//! The layers' rule (`REQ-AUTO-023`) already sews an area with a detail in one of its holes before the
//! detail. The cases here draw what that rule misses: a detail where 2 patches meet, which lies in no
//! area's hole, and an area that can't move back before its detail in paint order. A detail in the hole of
//! an area no wider than a satin column, which keeps its hole, is no miss: the column sews after it and
//! covers its edge, as an outline does, and the detail reaches under the column.

// Test code may unwrap and index (clippy.toml allows it in tests).
#![allow(clippy::unwrap_used, clippy::panic, clippy::indexing_slicing)]

use stitchcraft_core::{Budget, Code, Diagnostic};
use stitchcraft_engine::auto::{DETAILS_LAST, STITCH, digitized};
use stitchcraft_engine::design::{Design, DesignSettings, Element, Shape};
use stitchcraft_plan::{Rgb, Thread};
use stitchcraft_testkit::designs::{BLUE, along, automatic, fill, in_place, polylines, unbased};
use stitchcraft_testkit::fabric;
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

/// `elements`, each with `auto_details_last` set to `on`.
fn details_last(elements: Vec<Element>, on: &str) -> Vec<Element> {
    let set = |mut element: Element| {
        element.params.set(DETAILS_LAST, on);
        element
    };
    elements.into_iter().map(set).collect()
}

/// The elements of `elements` in the order they sew, with the details sewn last, each its id and how it
/// sews, `fill` or `stroke`, and what is said with `code`.
fn sewn(elements: Vec<Element>, code: Code) -> (Vec<String>, Vec<String>) {
    sewn_as(details_last(elements, "true"), code)
}

/// The elements of `elements` in the order they sew as their settings say ([`sewn`]).
fn sewn_as(elements: Vec<Element>, code: Code) -> (Vec<String>, Vec<String>) {
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
    // satin column and keeps its hole, it asks to sew as a fill, and the square is drawn first. Before
    // M13.37 the ring filled by the rules, and the square sewed first. Since, it sews as satin columns
    // unless it asks for a fill, and the square sews after the ring with the setting off too.
    let disc = square(27.0, 12.0, 6.0, false);
    let ring = [square(24.0, 9.0, 12.0, false), square(27.0, 12.0, 6.0, true)];
    let patch = [square(0.0, 0.0, 60.0, false), square(24.0, 9.0, 12.0, true)];
    let mut filled = area("ring", &[&ring[0], &ring[1]], &BLUE);
    filled.params.set(STITCH, "fill");
    let shapes = vec![area("disc", &[&disc], &BLACK), filled, area("patch", &[&patch[0], &patch[1]], &GREEN)];
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
        let design = Design::new(unbased(details_last(picture.elements, "true")), DesignSettings::default()).unwrap();
        let plan = in_place(&design).plan.unwrap();
        assert_eq!(plan.stats().color_changes, 3, "{}", picture.name);
    }
}

#[test]
fn req_auto_098_details_sew_last_only_when_the_lowest_automatic_shape_asks_for_it() {
    // By default the mark, a detail where the 2 patches meet, sews last.
    let (ids, _) = sewn_as(patches(), Code::AutoOrder);
    assert_eq!(at(&ids, "mark"), 2, "{ids:?}");
    // With the switch off, the mark drawn first sews first, as the order without the rule puts it, and
    // nothing is said of details.
    let (ids, said) = sewn_as(details_last(patches(), "false"), Code::AutoOrder);
    assert_eq!(at(&ids, "mark"), 0, "{ids:?}");
    assert!(!said.iter().any(|said| said.contains("details")), "{said:?}");
    // The design goes by its lowest automatic shape, the mark: turned on there alone, the mark sews last.
    let mut on = details_last(patches(), "false");
    on[0].params.set(DETAILS_LAST, "true");
    let (ids, _) = sewn_as(on, Code::AutoOrder);
    assert_eq!(at(&ids, "mark"), 2, "{ids:?}");
    // Turned on everywhere but there, it stays off.
    let mut off = details_last(patches(), "true");
    off[0].params.set(DETAILS_LAST, "false");
    let (ids, _) = sewn_as(off, Code::AutoOrder);
    assert_eq!(at(&ids, "mark"), 0, "{ids:?}");
}

/// A blue iris 12 mm square where 3 patches of fur meet, a black one and a green one above and a black
/// one below, and a black pupil 4 mm square on the iris against the black patch above, as VectorCraft's
/// Abutting method traces them: each shape has a notch where the next one lies. The iris is drawn first,
/// then the pupil and the patches.
fn nest() -> Vec<Element> {
    let iris = [(24.0, 24.0), (36.0, 24.0), (36.0, 36.0), (24.0, 36.0), (24.0, 30.0), (28.0, 30.0), (28.0, 26.0), (24.0, 26.0)];
    let left = [(0.0, 0.0), (30.0, 0.0), (30.0, 24.0), (24.0, 24.0), (24.0, 30.0), (0.0, 30.0)];
    let right = [(30.0, 0.0), (60.0, 0.0), (60.0, 30.0), (36.0, 30.0), (36.0, 24.0), (30.0, 24.0)];
    let below = [(0.0, 30.0), (24.0, 30.0), (24.0, 36.0), (36.0, 36.0), (36.0, 30.0), (60.0, 30.0), (60.0, 45.0), (0.0, 45.0)];
    vec![
        area("iris", &[&iris], &BLUE),
        area("pupil", &[&square(24.0, 26.0, 4.0, false)], &BLACK),
        area("left", &[&left], &BLACK),
        area("right", &[&right], &GREEN),
        area("below", &[&below], &BLACK),
    ]
}

/// What of `id`'s own shape shows once `elements` are planned with `auto_details_last` set to `on`, in
/// square millimetres.
fn shows(elements: Vec<Element>, on: &str, id: &str) -> f64 {
    let design = Design::new(unbased(details_last(elements, on)), DesignSettings::default()).unwrap();
    let plan = in_place(&design).plan.unwrap();
    let shown = fabric::shown(&design, &plan).into_iter().find(|shown| shown.id == id).unwrap();
    shown.area - shown.other
}

#[test]
fn req_auto_108_a_detail_moves_only_with_the_details_on_it() {
    // The iris is a detail of the 3 patches, and the pupil a detail of the iris and of the black patch above.
    // Sewing the iris after the black patches costs no thread change, and sewing the pupil after the iris
    // then costs one, since the pupil is black as they are. So the iris sews after the green patch alone,
    // and the pupil after the iris, as it does with the setting off.
    let (off, _) = sewn_as(details_last(nest(), "false"), Code::AutoOrder);
    assert!(at(&off, "pupil") > at(&off, "iris"), "{off:?}");
    let (ids, _) = sewn(nest(), Code::AutoOrder);
    assert!(at(&ids, "pupil") > at(&ids, "iris"), "the pupil sews after the iris: {ids:?}");
    // And the pupil shows as much as with the setting off, to the pixels along its edge: sewn last, it
    // reaches under no later shape.
    let (before, after) = (shows(nest(), "false", "pupil"), shows(nest(), "true", "pupil"));
    assert!(after >= 0.99 * before, "{after:.3} mm² of the pupil shows, {before:.3} with the setting off");
}
