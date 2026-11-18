//! Thin lines merged into their neighbours (`REQ-AUTO-090`, M13.18): a part of an automatic area narrower
//! than the narrowest satin stroke everywhere and ragged, as soft shading and fur trace into, merges into the
//! automatic area it shares the longest edge with, and an even line, a line beside no area and the lines of a
//! shape that turns merging off sew as lines. A thin, ragged piece of a broader part merges so too
//! (`REQ-AUTO-091`, M13.38), and an area that gives up a piece and takes nothing in sews as it was sorted
//! (`REQ-AUTO-092`).

// Test code may unwrap and index (clippy.toml allows it in tests).
#![allow(clippy::unwrap_used, clippy::panic, clippy::indexing_slicing)]

use stitchcraft_core::{Budget, Code, Diagnostic};
use stitchcraft_engine::auto::{as_merged, digitized};
use stitchcraft_engine::design::{Design, DesignSettings, Element, Shape};
use stitchcraft_engine::normalize::region;
use stitchcraft_plan::Thread;
use stitchcraft_testkit::designs::{BLUE, RED, automatic, fill, unbased};
use stitchcraft_testkit::pictures::GOLD;

/// An automatic rectangle `id` from `(x0, y0)` to `(x1, y1)` in `thread`.
fn rect(id: &str, (x0, y0): (f64, f64), (x1, y1): (f64, f64), thread: &Thread) -> Element {
    automatic(fill(id, &[&[(x0, y0), (x1, y0), (x1, y1), (x0, y1)]], thread, &[]))
}

/// A gold line 10 mm long from `x` = 10 to the right, straight on its left and ragged on its right: 0.4 mm
/// wide every millimetre and 0.7 mm wide half way between, 0.55 mm on average and 5.5 mm² large, as a strand
/// of fur traces. `params` are its settings.
fn ragged(id: &str, params: &[(&str, &str)]) -> Element {
    let mut ring = vec![(10.0, 0.0), (10.0, 10.0)];
    ring.extend((0..=20).map(|i| (if i % 2 == 0 { 10.4 } else { 10.7 }, 10.0 - f64::from(i) * 0.5)));
    automatic(fill(id, &[&ring], &GOLD, params))
}

/// The elements `elements` are sewn as, in a design with the default settings, and what is said of them.
fn sewn(elements: Vec<Element>) -> (Vec<Element>, Vec<Diagnostic>) {
    let design = Design::new(unbased(elements), DesignSettings::default()).unwrap();
    let (out, said) = digitized(&design, &Budget::DEFAULT);
    (out.into_owned(), said)
}

/// The ids of `elements` and whether each is a fill, in order.
fn kinds(elements: &[Element]) -> Vec<(&str, bool)> {
    elements.iter().map(|element| (element.id.as_str(), matches!(element.shape, Shape::Fill { .. }))).collect()
}

/// The area of the fill named `id` among `elements`, in square millimetres.
fn area(elements: &[Element], id: &str) -> f64 {
    let element = elements.iter().find(|element| element.id.as_str() == id).unwrap();
    let Shape::Fill { path, rule, .. } = &element.shape else { panic!("`{id}` is no fill") };
    region::build(path, *rule, &mut Budget::DEFAULT.meter()).unwrap().region.area()
}

/// What is said with `code` among `said`.
fn said(said: &[Diagnostic], code: Code) -> Vec<String> {
    said.iter().filter(|d| d.code == code).map(ToString::to_string).collect()
}

#[test]
fn diag_sc_i0915_a_ragged_thin_line_merges_into_the_area_it_shares_the_longest_edge_with() {
    // The line runs along `left` for 10 mm and touches `right` only at its widest places. `left` reaches
    // under nothing, so that it sews what it merged and no more.
    let left = automatic(fill("left", &[&[(0.0, 0.0), (10.0, 0.0), (10.0, 10.0), (0.0, 10.0)]], &RED, &[("auto_overlap_mm", "0")]));
    let (out, diagnostics) = sewn(vec![left, ragged("line", &[]), rect("right", (10.7, 0.0), (20.0, 10.0), &BLUE)]);
    assert_eq!(kinds(&out), [("left", true), ("right", true)]);
    assert!((area(&out, "left") - 105.5).abs() < 1e-6, "{}", area(&out, "left"));
    assert_eq!(
        said(&diagnostics, Code::AutoLineMerged),
        ["info SC-I0915: A line of this area, of 5.5 mm², is narrower than the narrowest satin stroke, so it is merged into `left`, which \
          it shares the longest edge with."]
    );
}

#[test]
fn req_auto_090_an_even_thin_line_sews_as_a_line() {
    // Drawn with an even pen, 0.5 mm wide all along.
    let (out, diagnostics) = sewn(vec![
        rect("left", (0.0, 0.0), (10.0, 10.0), &RED),
        rect("line", (10.0, 0.0), (10.5, 10.0), &GOLD),
        rect("right", (10.5, 0.0), (20.0, 10.0), &BLUE),
    ]);
    assert!(kinds(&out).contains(&("line", false)), "{:?}", kinds(&out));
    assert!(said(&diagnostics, Code::AutoLineMerged).is_empty());
}

#[test]
fn req_auto_090_a_shape_that_turns_merging_off_or_names_its_stitch_keeps_its_lines() {
    for params in [[("auto_fill_uneven", "false")], [("auto_stitch", "run")]] {
        let (out, diagnostics) =
            sewn(vec![rect("left", (0.0, 0.0), (10.0, 10.0), &RED), ragged("line", &params), rect("right", (10.7, 0.0), (20.0, 10.0), &BLUE)]);
        assert!(kinds(&out).contains(&("line", false)), "{params:?}: {:?}", kinds(&out));
        assert!(said(&diagnostics, Code::AutoLineMerged).is_empty());
    }
}

#[test]
fn req_auto_090_a_thin_line_beside_no_area_sews_as_a_line() {
    let (out, diagnostics) = sewn(vec![ragged("line", &[]), rect("far", (30.0, 0.0), (40.0, 10.0), &RED)]);
    assert!(kinds(&out).contains(&("line", false)), "{:?}", kinds(&out));
    assert!(said(&diagnostics, Code::AutoLineMerged).is_empty());
}

#[test]
fn req_auto_090_the_design_as_merged_draws_a_merged_line_in_its_neighbours_thread() {
    let design = Design::new(
        unbased(vec![rect("left", (0.0, 0.0), (10.0, 10.0), &RED), ragged("line", &[]), rect("right", (10.7, 0.0), (20.0, 10.0), &BLUE)]),
        DesignSettings::default(),
    )
    .unwrap();
    let merged = as_merged(&design, &Budget::DEFAULT).unwrap();
    let drawn: Vec<(&str, Thread)> = merged.elements().iter().map(|element| (element.id.as_str(), element.thread.clone())).collect();
    assert_eq!(drawn, [("left", RED), ("line", GOLD), ("right", BLUE), ("left~line1", RED)]);
    assert!((area(merged.elements(), "left~line1") - 5.5).abs() < 1e-6);
    // A design whose lines don't merge comes back as it is.
    let even = Design::new(unbased(vec![rect("a", (0.0, 0.0), (10.0, 10.0), &RED)]), DesignSettings::default()).unwrap();
    assert_eq!(as_merged(&even, &Budget::DEFAULT).unwrap().elements(), even.elements());
}

/// A gold square 20 mm wide from the origin with a strand of fur 10 mm long on its top, from `x` = 4 up,
/// straight on its left and ragged on its right as [`ragged`]'s line is: one shape, which sorting splits into
/// its fill and the strand, a thin piece of 5.5 mm².
fn tailed(id: &str) -> Element {
    let mut ring = vec![(0.0, 0.0), (20.0, 0.0), (20.0, 20.0)];
    ring.extend((0..=20).map(|i| (if i % 2 == 0 { 4.4 } else { 4.7 }, 20.0 + f64::from(i) * 0.5)));
    ring.extend([(4.0, 30.0), (4.0, 20.0), (0.0, 20.0)]);
    automatic(fill(id, &[&ring], &GOLD, &[]))
}

#[test]
fn req_auto_091_a_ragged_thin_piece_of_a_broader_part_merges_into_the_area_beside_it() {
    let (out, diagnostics) = sewn(vec![tailed("tailed"), rect("beside", (-10.0, 20.0), (4.0, 30.0), &RED)]);
    // No running stitch is left: the square sews its fill, and `beside` the strand with its own.
    assert!(kinds(&out).iter().all(|(_, filled)| *filled), "{:?}", kinds(&out));
    // `beside` took the strand in, cut off near where it meets the square, which keeps the rest.
    let (tailed, beside) = (area(&out, "tailed"), area(&out, "beside"));
    assert!((145.4..145.5).contains(&beside) && (400.0..405.5).contains(&tailed), "{tailed}, {beside}");
    assert_eq!(
        said(&diagnostics, Code::AutoLineMerged),
        ["info SC-I0915: A thin piece of this area, of 5.48 mm², would sew as a running stitch, so it is merged into `beside`, which it \
          shares the longest edge with."]
    );
    // `tailed` names the rule its fill sews by, and no longer the strand's (`REQ-AUTO-092`).
    let rules = said(&diagnostics, Code::AutoRule);
    assert!(rules.iter().any(|said| said.contains("This shape sews by the rule area")), "{rules:?}");
    assert!(!rules.iter().any(|said| said.contains("line")), "{rules:?}");
}

#[test]
fn req_auto_091_a_ragged_thin_piece_beside_no_other_area_sews_as_a_line_and_is_not_said() {
    let (out, diagnostics) = sewn(vec![tailed("tailed"), rect("far", (40.0, 0.0), (50.0, 10.0), &RED)]);
    assert!(kinds(&out).iter().any(|(_, filled)| !filled), "{:?}", kinds(&out));
    assert!(said(&diagnostics, Code::AutoLineMerged).is_empty());
    let rules = said(&diagnostics, Code::AutoRule);
    assert!(rules.iter().any(|said| said.contains("line")), "{rules:?}");
}
