//! Marks that keep parts (roadmap step M13.39, `REQ-AUTO-104` to `REQ-AUTO-107`): a part of an automatic
//! area wholly inside a mark sews by its width where automatic digitizing would merge it, and nothing else
//! changes. The designs are drawn here in code, as automatic digitizing is given them.

// Test code may unwrap, panic and index (clippy.toml allows it in tests, but not in their helpers).
#![allow(clippy::unwrap_used, clippy::panic, clippy::indexing_slicing)]

use stitchcraft_core::{Budget, Code, Diagnostic, ElementId, Mm};
use stitchcraft_engine::auto::{as_merged, digitized};
use stitchcraft_engine::design::mark::Mark;
use stitchcraft_engine::design::{Cap, Design, DesignSettings, Element, Join, Path, Segment, Shape, Subpath};
use stitchcraft_plan::Thread;
use stitchcraft_testkit::designs::{BLUE, GREEN, RED, automatic, fill, unbased};

fn rect(id: &str, (x0, y0): (f64, f64), (x1, y1): (f64, f64), thread: &Thread) -> Element {
    automatic(fill(id, &[&[(x0, y0), (x1, y0), (x1, y1), (x0, y1)]], thread, &[]))
}

/// A ragged line 10 mm long from (10, 0) down, 0.4 and 0.7 mm wide by turns: a thin line that merges.
fn line(id: &str) -> Element {
    let mut ring = vec![(10.0, 0.0), (10.0, 10.0)];
    ring.extend((0..=20).map(|i| (10.0 + if i % 2 == 0 { 0.4 } else { 0.7 }, 10.0 - f64::from(i) * 0.5)));
    automatic(fill(id, &[&ring], &GREEN, &[]))
}

/// A mark `id` round the box from (`x0`, `y0`) to (`x1`, `y1`), each side cut into `n` segments, drawn with a
/// stroke and no fill: 4 segments in all for a rectangle, more for a mark of many segments.
fn mark(id: &str, [x0, y0, x1, y1]: [f64; 4], n: u32) -> Mark {
    let corners = [(x0, y0), (x1, y0), (x1, y1), (x0, y1), (x0, y0)];
    let ring: Vec<(f64, f64)> = corners
        .windows(2)
        .flat_map(|pair| {
            let (from, to) = (pair[0], pair[1]);
            (0..n).map(move |i| {
                let t = f64::from(i) / f64::from(n);
                (from.0 + (to.0 - from.0) * t, from.1 + (to.1 - from.1) * t)
            })
        })
        .collect();
    let point = |(x, y): (f64, f64)| stitchcraft_core::Point::new(x, y).unwrap();
    let outline = Subpath { start: point(ring[0]), segments: ring[1..].iter().map(|&at| Segment::Line(point(at))).collect(), closed: true };
    let shape = Shape::Stroke { path: Path { subpaths: vec![outline] }, width: Mm::new(0.2).unwrap(), join: Join::Round, cap: Cap::Butt };
    Mark { id: ElementId::new(id).unwrap(), shape }
}

fn design(elements: Vec<Element>, marks: Vec<Mark>) -> Design {
    let mut design = Design::new(unbased(elements), DesignSettings::default()).unwrap();
    design.marks = marks;
    design
}

fn sewn(design: &Design, budget: &Budget) -> (Vec<Element>, Vec<Diagnostic>) {
    let (out, said) = digitized(design, budget);
    (out.into_owned(), said)
}

fn said(said: &[Diagnostic], code: Code) -> Vec<String> {
    said.iter().filter(|d| d.code == code).map(ToString::to_string).collect()
}

/// The elements of `elements` in `thread`, with whether each runs.
fn in_thread(elements: &[Element], thread: &Thread) -> Vec<(String, bool)> {
    elements.iter().filter(|e| e.thread == *thread).map(|e| (e.id.to_string(), matches!(e.shape, Shape::Stroke { .. }))).collect()
}

/// A red square from (0, 0) to (20, 20), and a blue speck of 0.8 mm² on it at (5, 5).
fn speckled() -> Vec<Element> {
    vec![rect("square", (0.0, 0.0), (20.0, 20.0), &RED), rect("speck", (5.0, 5.0), (6.6, 5.5), &BLUE)]
}

#[test]
fn req_auto_104_a_speck_wholly_inside_a_mark_sews_by_its_width() {
    let (plain, said_plain) = sewn(&design(speckled(), Vec::new()), &Budget::DEFAULT);
    assert!(in_thread(&plain, &BLUE).is_empty(), "{plain:?}");
    assert_eq!(said(&said_plain, Code::AutoSpeck).len(), 1);
    let (out, diagnostics) = sewn(&design(speckled(), vec![mark("eye", [4.0, 4.0, 7.0, 6.0], 1)]), &Budget::DEFAULT);
    // A running stitch along its middle, 1.6 mm long, in its own thread.
    assert_eq!(in_thread(&out, &BLUE), [("speck".to_string(), true)]);
    assert!(said(&diagnostics, Code::AutoSpeck).is_empty());
    assert_eq!(
        said(&diagnostics, Code::AutoKept),
        ["info SC-I0916: The mark `eye` holds 1 part of automatic areas wholly. Of them, 1 speck and 0 thin lines sew by their width, where \
          without the mark they would merge into a neighbour."]
    );
}

#[test]
fn req_auto_104_a_speck_of_a_shape_asked_to_fill_stays_a_fill_of_its_own() {
    // The speck's shape asks to be filled: kept, it sews as a small fill in its own thread.
    let mut elements = speckled();
    elements[1] = automatic(fill("speck", &[&[(5.0, 5.0), (6.6, 5.0), (6.6, 5.5), (5.0, 5.5)]], &BLUE, &[("auto_stitch", "fill")]));
    let (out, diagnostics) = sewn(&design(elements, vec![mark("eye", [4.0, 4.0, 7.0, 6.0], 1)]), &Budget::DEFAULT);
    assert_eq!(in_thread(&out, &BLUE), [("speck".to_string(), false)]);
    assert!(said(&diagnostics, Code::AutoSpeck).is_empty(), "{diagnostics:?}");
}

#[test]
fn req_auto_105_a_thin_line_wholly_inside_a_mark_sews_as_a_line() {
    // The green line is its colour's one part, so at the default likeness of 0.85 it would sew as a line
    // with no mark too (REQ-AUTO-095): at 0 it merges, and the mark alone keeps it.
    let merging = || {
        let mut line = line("line");
        line.params.set("auto_min_likeness", "0");
        line
    };
    let elements = || vec![rect("left", (0.0, 0.0), (10.0, 10.0), &RED), merging(), rect("right", (10.7, 0.0), (20.0, 10.0), &BLUE)];
    let (plain, said_plain) = sewn(&design(elements(), Vec::new()), &Budget::DEFAULT);
    assert!(in_thread(&plain, &GREEN).is_empty());
    assert_eq!(said(&said_plain, Code::AutoLineMerged).len(), 1);
    let marked = design(elements(), vec![mark("strand", [9.5, -0.5, 11.0, 10.5], 1)]);
    let (out, diagnostics) = sewn(&marked, &Budget::DEFAULT);
    assert_eq!(in_thread(&out, &GREEN), [("line".to_string(), true)]);
    assert!(said(&diagnostics, Code::AutoLineMerged).is_empty());
    assert_eq!(said(&diagnostics, Code::AutoKept).len(), 1);
    assert!(said(&diagnostics, Code::AutoKept)[0].contains("Of them, 0 specks and 1 thin line sew"), "{diagnostics:?}");
    // The design as merged, which a measure compares with, draws no merged line over the kept one.
    assert_eq!(as_merged(&marked, &Budget::DEFAULT).unwrap().elements().len(), marked.elements().len());
    assert_eq!(as_merged(&design(elements(), Vec::new()), &Budget::DEFAULT).unwrap().elements().len(), marked.elements().len() + 1);
}

#[test]
fn req_auto_107_a_mark_holding_nothing_whole_changes_nothing() {
    let plain = sewn(&design(speckled(), Vec::new()), &Budget::DEFAULT).0;
    // Round half the speck only, and round nothing.
    for at in [[4.0, 4.0, 6.0, 6.0], [10.0, 10.0, 12.0, 12.0]] {
        let (out, diagnostics) = sewn(&design(speckled(), vec![mark("half", at, 1)]), &Budget::DEFAULT);
        assert_eq!(out, plain, "{at:?}");
        assert_eq!(said(&diagnostics, Code::AutoMarkUnused).len(), 1, "{at:?}");
    }
}

#[test]
fn diag_sc_w0917_a_mark_in_a_design_with_no_automatic_shape_is_not_used() {
    let mut square = fill("square", &[&[(0.0, 0.0), (20.0, 0.0), (20.0, 20.0), (0.0, 20.0)]], &RED, &[]);
    square.automatic = false;
    let (out, diagnostics) = sewn(&design(vec![square.clone()], vec![mark("eye", [4.0, 4.0, 7.0, 6.0], 1)]), &Budget::DEFAULT);
    assert_eq!(out, [square]);
    assert_eq!(
        said(&diagnostics, Code::AutoMarkUnused),
        ["warning SC-W0917: The mark `eye` is not used, and the design sews as without it: the design has no shape digitized automatically."]
    );
}

#[test]
fn req_auto_104_a_speck_that_needs_more_work_to_test_than_the_budget_allows_is_left_out_with_its_area() {
    // Testing the speck's 5 points against a mark of 4,000 segments costs 20,000 units, more than its
    // shape's budget once its parts are found: the shape is left out (`SC-E0004`), and the square sews.
    let budget = Budget { max_work: 12_000, ..Budget::DEFAULT };
    let (out, diagnostics) = sewn(&design(speckled(), vec![mark("eye", [4.0, 4.0, 7.0, 6.0], 1_000)]), &budget);
    assert!(in_thread(&out, &BLUE).is_empty());
    assert!(
        diagnostics.iter().any(|d| d.code == Code::BudgetExhausted && d.element.as_ref().is_some_and(|e| e.as_str() == "speck")),
        "{diagnostics:?}"
    );
}
