//! The order elements sew in (`REQ-AUTO-010` to `REQ-AUTO-012`): shapes that overlap keep their paint
//! order, a line sews after the fill it borders, each thread sews as long as it can and the nearest element
//! next; the elements with settings keep their order unless the design asks for every element ordered
//! (`--sort-colours`), and nothing moves across a stop.

// Test code may unwrap and index (clippy.toml allows it in tests).
#![allow(clippy::unwrap_used, clippy::panic, clippy::indexing_slicing)]

use stitchcraft_core::{Budget, Code, Diagnostic};
use stitchcraft_engine::auto::digitized;
use stitchcraft_engine::design::{Design, DesignSettings, Element};
use stitchcraft_plan::Thread;
use stitchcraft_testkit::designs::{BLUE, RED, along, automatic, fill, polylines, widened};
use stitchcraft_testkit::pictures::GOLD;

/// An automatic square `id`, `size` millimetres wide, from `(x, y)`, sewn with `thread`.
fn square(id: &str, (x, y): (f64, f64), size: f64, thread: &Thread) -> Element {
    automatic(fill(id, &[&[(x, y), (x + size, y), (x + size, y + size), (x, y + size)]], thread, &[]))
}

/// A line `id` along `points` with no settings, `width` millimetres wide, sewn with `thread`: a running stitch
/// up to 1 mm wide, and a satin column up to 7 mm.
fn line(id: &str, points: &[(f64, f64)], width: f64, thread: &Thread) -> Element {
    automatic(widened(along(id, polylines(&[points]), thread, &[]), width))
}

/// `element` with settings: no longer automatic, sewn as a tatami fill or a running stitch as it is drawn.
fn set(element: Element) -> Element {
    Element { automatic: false, ..element }
}

/// `element` with a stop after it.
fn stopping(element: Element) -> Element {
    let mut element = element;
    element.params.set("stop_after", "true");
    element
}

/// The ids of `elements` in the order they sew, in a design with the default settings that asks for every
/// element ordered when `sort_colours` says so, and what is said of them with `code`.
fn order_of(elements: Vec<Element>, sort_colours: bool, code: Code) -> (Vec<String>, Vec<String>) {
    let mut design = Design::new(elements, DesignSettings::default()).unwrap();
    design.sort_colours = sort_colours;
    let (out, said) = digitized(&design, &Budget::DEFAULT);
    (out.iter().map(|element| element.id.as_str().to_string()).collect(), messages(&said, code))
}

/// What `said` says with `code`, as a user reads it.
fn messages(said: &[Diagnostic], code: Code) -> Vec<String> {
    said.iter().filter(|d| d.code == code).map(ToString::to_string).collect()
}

/// The ids of the automatic `elements` in the order they sew, and what `SC-I0907` says of them.
fn sewn(elements: Vec<Element>) -> (Vec<String>, Vec<String>) {
    order_of(elements, false, Code::AutoOrder)
}

#[test]
fn req_auto_010_each_thread_sews_as_long_as_it_can() {
    // 4 squares apart from each other, drawn red, blue, red, blue: the reds sew first, then the blues, the
    // nearer first.
    let (ids, said) = sewn(vec![
        square("r1", (0.0, 0.0), 10.0, &RED),
        square("b1", (20.0, 0.0), 10.0, &BLUE),
        square("r2", (40.0, 0.0), 10.0, &RED),
        square("b2", (55.0, 0.0), 10.0, &BLUE),
    ]);
    assert_eq!(ids, ["r1", "r2", "b2", "b1"]);
    assert_eq!(said, ["info SC-I0907: The elements are sewn in an order of their own, with 1 thread change where paint order has 3."]);
}

#[test]
fn req_auto_010_shapes_that_overlap_keep_their_paint_order() {
    // Each square lies half on the one drawn before it: none can sew before the one under it.
    let stacked = || vec![square("r1", (0.0, 0.0), 10.0, &RED), square("b", (5.0, 0.0), 10.0, &BLUE), square("r2", (10.0, 0.0), 10.0, &RED)];
    let (ids, said) = sewn(stacked());
    assert_eq!(ids, ["r1", "b", "r2"]);
    assert!(said.is_empty(), "{said:?}");
    // With the second red beside the blue instead of on it, the reds sew one after the other.
    let (ids, said) = sewn(vec![square("r1", (0.0, 0.0), 10.0, &RED), square("b", (5.0, 0.0), 10.0, &BLUE), square("r2", (15.0, 20.0), 10.0, &RED)]);
    assert_eq!(ids, ["r1", "r2", "b"]);
    assert_eq!(said, ["info SC-I0907: The elements are sewn in an order of their own, with 1 thread change where paint order has 2."]);
}

#[test]
fn req_auto_010_shapes_that_only_meet_sew_in_either_order() {
    // Red, blue and red stripes side by side, the blue one reaching `over` each red one: sharing edges, the
    // reds sew one after the other.
    let stripes = |over: f64| {
        let blue = automatic(fill("b", &[&[(10.0 - over, 0.0), (20.0 + over, 0.0), (20.0 + over, 10.0), (10.0 - over, 10.0)]], &BLUE, &[]));
        vec![square("r1", (0.0, 0.0), 10.0, &RED), blue, square("r2", (20.0, 0.0), 10.0, &RED)]
    };
    assert_eq!(sewn(stripes(0.0)).0, ["r1", "r2", "b"]);
    // An overlap of 0.04 mm, less than twice the shapes' tolerance, is rounding where edges drawn as one
    // meet; one of 0.1 mm is drawn, and keeps paint order.
    assert_eq!(sewn(stripes(0.04)).0, ["r1", "r2", "b"]);
    assert_eq!(sewn(stripes(0.1)).0, ["r1", "b", "r2"]);
}

#[test]
fn req_auto_010_a_line_sews_after_the_fill_it_borders() {
    // A satin column 2 mm wide drawn first along the square's top edge, outside it, and a running line
    // 0.5 mm wide along its bottom edge: both sew after the square, over its edges, the line first, whose
    // middle is nearer the square's.
    let column = line("column", &[(0.0, 11.0), (10.0, 11.0)], 2.0, &BLUE);
    let running = line("running", &[(0.0, -0.25), (10.0, -0.25)], 0.5, &BLUE);
    let (ids, said) = sewn(vec![column, running, square("square", (0.0, 0.0), 10.0, &RED)]);
    assert_eq!(ids, ["square", "running", "column"]);
    assert_eq!(said, ["info SC-I0907: The elements are sewn in an order of their own, with 1 thread change where paint order has 1."]);
    // A line drawn after a square, along its top edge: blue is the thread sewing, after a blue square far
    // away, but the line waits for the square it borders.
    let (ids, said) = sewn(vec![
        square("far", (100.0, 0.0), 10.0, &BLUE),
        square("square", (0.0, 0.0), 10.0, &RED),
        line("line", &[(0.0, 10.25), (10.0, 10.25)], 0.5, &BLUE),
    ]);
    assert_eq!(ids, ["far", "square", "line"]);
    assert!(said.is_empty(), "{said:?}");
    // A line drawn first across the square's corner, 3.5 mm from it, has a box that meets the square's, but
    // it borders nothing and keeps its place.
    let (ids, said) = sewn(vec![line("line", &[(20.0, 5.0), (5.0, 20.0)], 0.5, &BLUE), square("square", (0.0, 0.0), 10.0, &RED)]);
    assert_eq!(ids, ["line", "square"]);
    assert!(said.is_empty(), "{said:?}");
}

#[test]
fn req_auto_010_a_line_that_overlaps_a_fill_keeps_its_paint_order() {
    // A line drawn under the square, across it: the square covers it, as drawn.
    let (ids, said) = sewn(vec![line("under", &[(-5.0, 5.0), (15.0, 5.0)], 2.0, &BLUE), square("square", (0.0, 0.0), 10.0, &RED)]);
    assert_eq!(ids, ["under", "square"]);
    assert!(said.is_empty(), "{said:?}");
    // A column along the edge of a square drawn after it, which it only touches: the square on it, which it
    // must sew before, lies under the square, so the column can't sew after the square, and keeps its
    // place.
    let (ids, said) = sewn(vec![
        line("edge", &[(5.0, 0.0), (5.0, 20.0)], 2.0, &BLUE),
        automatic(fill("on", &[&[(5.0, 12.0), (15.0, 12.0), (15.0, 22.0), (5.0, 22.0)]], &GOLD, &[])),
        automatic(fill("square", &[&[(6.0, 0.0), (16.0, 0.0), (16.0, 14.0), (6.0, 14.0)]], &RED, &[])),
    ]);
    assert_eq!(ids, ["edge", "on", "square"]);
    assert!(said.is_empty(), "{said:?}");
}

#[test]
fn req_auto_010_the_nearest_element_sews_next() {
    // Red squares drawn at x = 0, 100 and 10: the one at 10 is nearer the first.
    let (ids, _) = sewn(vec![square("first", (0.0, 0.0), 5.0, &RED), square("far", (100.0, 0.0), 5.0, &RED), square("near", (10.0, 0.0), 5.0, &RED)]);
    assert_eq!(ids, ["first", "near", "far"]);
    // Boxes the same distance apart: the nearer centre, then the one drawn first.
    let wide = automatic(fill("wide", &[&[(20.0, 0.0), (40.0, 0.0), (40.0, 10.0), (20.0, 10.0)]], &RED, &[]));
    let (ids, _) = sewn(vec![square("first", (0.0, 0.0), 10.0, &RED), wide, square("narrow", (-20.0, 0.0), 10.0, &RED)]);
    assert_eq!(ids, ["first", "narrow", "wide"]);
    let (ids, _) =
        sewn(vec![square("first", (0.0, 0.0), 10.0, &RED), square("above", (0.0, 20.0), 10.0, &RED), square("below", (0.0, -20.0), 10.0, &RED)]);
    assert_eq!(ids, ["first", "above", "below"]);
}

#[test]
fn req_auto_010_the_parts_of_a_shape_keep_their_order() {
    // A line of 3 subpaths, a satin column each: the third is nearer the first than the second is, but the
    // columns sew in the line's order, so that what follows the line follows its last part.
    let line = automatic(widened(
        along("line", polylines(&[&[(0.0, 0.0), (10.0, 0.0)], &[(100.0, 0.0), (110.0, 0.0)], &[(20.0, 0.0), (30.0, 0.0)]]), &RED, &[]),
        2.0,
    ));
    let (ids, said) = sewn(vec![line, square("square", (200.0, 0.0), 10.0, &RED)]);
    assert_eq!(ids, ["line", "line#2", "line#3", "square"]);
    assert!(said.is_empty(), "{said:?}");
}

#[test]
fn req_auto_010_elements_with_settings_keep_their_order_and_automatic_ones_move_round_them() {
    // Red, blue and red squares with settings, and an automatic blue one after them, all apart.
    let elements = || {
        vec![
            set(square("s1", (0.0, 0.0), 10.0, &RED)),
            set(square("s2", (20.0, 0.0), 10.0, &BLUE)),
            set(square("s3", (40.0, 0.0), 10.0, &RED)),
            square("a", (60.0, 0.0), 10.0, &BLUE),
        ]
    };
    let (ids, said) = sewn(elements());
    assert_eq!(ids, ["s1", "s2", "a", "s3"]);
    assert_eq!(said, ["info SC-I0907: The elements are sewn in an order of their own, with 2 thread changes where paint order has 3."]);
    // A design without automatic elements sews as drawn, and says nothing of its order.
    let (ids, said) = sewn(elements().into_iter().take(3).collect());
    assert_eq!((ids, said), (vec!["s1".to_string(), "s2".to_string(), "s3".to_string()], Vec::new()));
}

#[test]
fn req_auto_011_sort_colours_orders_the_elements_with_settings_too() {
    let elements =
        || vec![set(square("s1", (0.0, 0.0), 10.0, &RED)), set(square("s2", (20.0, 0.0), 10.0, &BLUE)), set(square("s3", (40.0, 0.0), 10.0, &RED))];
    let (ids, said) = order_of(elements(), true, Code::AutoOrder);
    assert_eq!(ids, ["s1", "s3", "s2"]);
    assert_eq!(said, ["info SC-I0907: The elements are sewn in an order of their own, with 1 thread change where paint order has 2."]);
    // What overlaps still keeps its paint order.
    let (ids, _) = order_of(
        vec![set(square("s1", (0.0, 0.0), 10.0, &RED)), set(square("s2", (5.0, 0.0), 10.0, &BLUE)), set(square("s3", (10.0, 0.0), 10.0, &RED))],
        true,
        Code::AutoOrder,
    );
    assert_eq!(ids, ["s1", "s2", "s3"]);
}

#[test]
fn req_auto_010_nothing_moves_across_a_stop() {
    // Red, blue with a stop after it, red: the second red can't sew before the stop.
    let (ids, said) =
        sewn(vec![square("r1", (0.0, 0.0), 10.0, &RED), stopping(square("b", (20.0, 0.0), 10.0, &BLUE)), square("r2", (40.0, 0.0), 10.0, &RED)]);
    assert_eq!(ids, ["r1", "b", "r2"]);
    assert!(said.is_empty(), "{said:?}");
    // On each side of the stop, the threads still sew as long as they can, from the thread sewing.
    let (ids, _) = sewn(vec![
        square("r1", (0.0, 0.0), 10.0, &RED),
        square("b1", (20.0, 0.0), 10.0, &BLUE),
        square("r2", (40.0, 0.0), 10.0, &RED),
        stopping(square("b2", (60.0, 0.0), 10.0, &BLUE)),
        square("r3", (80.0, 0.0), 10.0, &RED),
        square("b3", (100.0, 0.0), 10.0, &BLUE),
    ]);
    assert_eq!(ids, ["r1", "r2", "b1", "b2", "b3", "r3"]);
}

#[test]
fn diag_sc_i0907_the_elements_are_sewn_in_an_order_of_their_own() {
    let (ids, said) = sewn(vec![square("r1", (0.0, 0.0), 10.0, &RED), square("b", (20.0, 0.0), 10.0, &BLUE), square("r2", (40.0, 0.0), 10.0, &RED)]);
    assert_eq!(ids, ["r1", "r2", "b"]);
    assert_eq!(said, ["info SC-I0907: The elements are sewn in an order of their own, with 1 thread change where paint order has 2."]);
    assert_eq!(Code::AutoOrder.id(), "SC-I0907");
}

#[test]
fn req_auto_010_an_automatic_element_drawn_over_one_with_settings_sews_after_it() {
    // A red square far away, a blue square with settings, a red square drawn half on it and a blue square
    // apart: the red square on the blue one waits for it, though red is the thread sewing first.
    let (ids, said) = sewn(vec![
        square("far", (100.0, 0.0), 10.0, &RED),
        set(square("set", (0.0, 0.0), 10.0, &BLUE)),
        square("on", (5.0, 0.0), 10.0, &RED),
        square("apart", (40.0, 0.0), 10.0, &BLUE),
    ]);
    assert_eq!(ids, ["far", "apart", "set", "on"]);
    assert_eq!(said, ["info SC-I0907: The elements are sewn in an order of their own, with 2 thread changes where paint order has 3."]);
}

#[test]
fn req_auto_010_a_part_thinner_than_twice_the_tolerance_overlaps_what_it_crosses() {
    // A red line far away, a blue line 0.1 mm wide, and a red one as wide drawn across it: where they cross,
    // the red lies on the blue, so the blue sews first, though red is the thread sewing.
    let (ids, said) = sewn(vec![
        line("far", &[(100.0, 0.0), (110.0, 0.0)], 0.1, &RED),
        line("blue", &[(0.0, 0.0), (20.0, 0.0)], 0.1, &BLUE),
        line("red", &[(10.0, -10.0), (10.0, 10.0)], 0.1, &RED),
    ]);
    assert_eq!(ids, ["far", "blue", "red"]);
    assert!(said.is_empty(), "{said:?}");
    // A comb drawn over a red line, a tooth 0.1 mm wide reaching across the line: the comb sews after the
    // line, though blue is the thread sewing.
    let teeth = [(0.0, 5.0), (10.0, 5.0), (10.0, -5.0), (10.1, -5.0), (10.1, 5.0), (20.0, 5.0), (20.0, 15.0), (0.0, 15.0)];
    let comb = automatic(fill("comb", &[&teeth], &BLUE, &[]));
    let (ids, said) = sewn(vec![square("far", (100.0, 0.0), 10.0, &BLUE), line("line", &[(0.0, 0.0), (20.0, 0.0)], 0.8, &RED), comb]);
    assert_eq!(ids, ["far", "line", "comb"]);
    assert!(said.is_empty(), "{said:?}");
}

#[test]
fn req_auto_010_a_speck_left_out_has_no_place_in_the_order() {
    // A speck 2 mm from the first square shares no edge with an area and is left out. The order goes on from
    // the first square to the nearer of the other 2, on its left, not to the one on the right that is
    // nearer the speck.
    let (ids, said) = sewn(vec![
        square("first", (0.0, 0.0), 10.0, &RED),
        square("speck", (12.0, 0.0), 0.5, &RED),
        square("right", (25.0, 0.0), 10.0, &RED),
        square("left", (-20.0, 0.0), 10.0, &RED),
    ]);
    assert_eq!(ids, ["first", "left", "right"]);
    assert_eq!(said, ["info SC-I0907: The elements are sewn in an order of their own, with 0 thread changes where paint order has 0."]);
}
