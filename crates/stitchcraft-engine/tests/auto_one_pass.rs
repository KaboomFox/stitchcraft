//! Each thread threaded once (`REQ-AUTO-184` to `REQ-AUTO-187`, M13.63): automatic digitizing threads each thread of
//! a design once wherever the shapes as drawn allow it, and names the shapes where they don't.
//!
//! Design: `docs/src/design/image-to-embroidery.md` › Each thread threaded once. A machine with one needle is
//! rethreaded by hand at each thread change. Before M13.63 a line sewed after every fill it borders (rule 2 of
//! the sewing order), and a dark line beside a light fill drawn on a dark fill threaded the dark thread twice.
//! The base layer sewed in the thread of the largest area, which threaded that thread twice when its shapes
//! couldn't all sew first. Each case draws its shapes in code, but the case of TS-13B's windows, which plans
//! the sheet's digitized half.

// Test code may unwrap and index (clippy.toml allows it in tests).
#![allow(clippy::unwrap_used, clippy::panic, clippy::indexing_slicing)]

use std::collections::BTreeMap;

use stitchcraft_engine::PlanOutcome;
use stitchcraft_engine::design::{Design, DesignSettings, Element};
use stitchcraft_engine::testsheets::SHEETS;
use stitchcraft_plan::{Rgb, StitchPlan, Thread};
use stitchcraft_testkit::designs::{along, automatic, fill, in_place, line, planned, rings, widened};

type Pt = (f64, f64);

const NAVY: Thread = Thread::new(Rgb::new(29, 53, 87));
const CREAM: Thread = Thread::new(Rgb::new(244, 232, 201));
const GOLD: Thread = Thread::new(Rgb::new(232, 169, 0));
const GREEN: Thread = Thread::new(Rgb::new(31, 81, 50));
const PINK: Thread = Thread::new(Rgb::new(229, 152, 155));
const RED: Thread = Thread::new(Rgb::new(200, 16, 46));
const BROWN: Thread = Thread::new(Rgb::new(123, 74, 45));

/// An automatic area `id` of `rings` sewn with `thread`.
fn area(id: &str, rings: &[&[Pt]], thread: &Thread) -> Element {
    automatic(fill(id, rings, thread, &[]))
}

/// The rectangle from `(x0, y0)` to `(x1, y1)`, anticlockwise.
fn rect(x0: f64, y0: f64, x1: f64, y1: f64) -> Vec<Pt> {
    vec![(x0, y0), (x1, y0), (x1, y1), (x0, y1)]
}

/// The rectangle from `(x0, y0)` to `(x1, y1)`, clockwise: a hole in a ring drawn anticlockwise.
fn hole(x0: f64, y0: f64, x1: f64, y1: f64) -> Vec<Pt> {
    vec![(x0, y0), (x0, y1), (x1, y1), (x1, y0)]
}

/// How many times each thread of `plan` is threaded: once for each run of blocks one after another in it.
fn threaded(plan: &StitchPlan) -> BTreeMap<Rgb, usize> {
    let mut threaded = BTreeMap::new();
    let mut last = None;
    for block in &plan.blocks {
        if last != Some(block.thread.color) {
            *threaded.entry(block.thread.color).or_insert(0) += 1;
        }
        last = Some(block.thread.color);
    }
    threaded
}

/// `elements` planned for the reference machine with their stitches where they are drawn ([`in_place`]).
fn placed(elements: Vec<Element>) -> PlanOutcome {
    in_place(&Design::new(elements, DesignSettings::default()).unwrap())
}

/// The plan of `outcome`.
fn plan_of(outcome: &PlanOutcome) -> &StitchPlan {
    outcome.plan.as_ref().unwrap_or_else(|| panic!("{:?}", outcome.diagnostics))
}

/// The sew-out piece of M13.63 (`docs/src/plan/machine-testing.md`): a cream fill with a navy line 0.8 mm wide
/// round it, abutting it as a trace draws a line, on a navy fill broad enough to be digitized whole under it,
/// and a gold square beside them. Each shape has `params`.
fn piece(params: &[(&str, &str)]) -> Vec<Element> {
    let shape = |id: &str, rings: &[&[Pt]], thread: &Thread| automatic(fill(id, rings, thread, params));
    vec![
        shape("dark", &[&rect(2.0, 2.0, 58.0, 42.0)], &NAVY),
        shape("line", &[&rect(15.2, 11.2, 44.8, 32.8), &hole(16.0, 12.0, 44.0, 32.0)], &NAVY),
        shape("light", &[&rect(16.0, 12.0, 44.0, 32.0)], &CREAM),
        shape("gold", &[&rect(62.0, 32.0, 70.0, 42.0)], &GOLD),
    ]
}

/// `REQ-AUTO-184`: the sew-out piece threads each of its 3 threads once. The navy line borders the cream fill,
/// and the navy fill lies under the cream one. Before M13.63 the navy fill sewed first, the cream fill after
/// it, and the line after the cream fill, so navy was threaded twice: 4 threadings and 3 changes.
#[test]
fn req_auto_184_a_dark_line_round_a_light_fill_on_a_dark_fill_threads_each_thread_once() {
    let outcome = placed(piece(&[]));
    let plan = plan_of(&outcome);
    assert_eq!(threaded(plan), BTreeMap::from([(NAVY.color, 1), (CREAM.color, 1), (GOLD.color, 1)]));
    assert_eq!(plan.stats().color_changes, 2);
}

/// `REQ-AUTO-184`: TS-13B's digitized windows, black lines 0.3, 0.6 and 0.9 mm wide between red, blue and gold
/// panes, with its black words, thread each of the 4 threads once, and every line sews after the panes it
/// borders: rule 2 gives way only where it costs a threading, and the order with the fewest threadings that
/// lets the most lines sew after their fills is taken. Before M13.63 black was threaded twice, for the words
/// and for the lines. With the lines sewn first the panes' rows covered the lines 0.3 and 0.6 mm wide.
#[test]
fn req_auto_184_ts_13b_threads_once_and_its_lines_sew_after_the_panes() {
    let sheet = SHEETS.iter().find(|sheet| sheet.id == "TS-13B").unwrap().design().unwrap().unwrap();
    let shapes: Vec<Element> = sheet.elements().iter().filter(|element| element.automatic).cloned().collect();
    let design = Design::new(shapes, sheet.settings).unwrap();
    let outcome = in_place(&design);
    let plan = plan_of(&outcome);
    assert!(threaded(plan).values().all(|&times| times == 1), "{:?}", threaded(plan));
    // The last needle point of each pane and the first of each line, in the order they sew.
    let (mut panes, mut lines): (BTreeMap<String, usize>, BTreeMap<String, usize>) = Default::default();
    for (at, stitch) in plan.blocks.iter().flat_map(|block| &block.stitches).enumerate() {
        let Some(id) = stitch.origin.element.and_then(|element| plan.element(element)).map(ToString::to_string) else { continue };
        let Some(window) = id.strip_prefix("ts13b:auto:").and_then(|rest| rest.split(':').next()).filter(|w| w.starts_with("window")) else {
            continue;
        };
        if id.contains(":line") {
            lines.entry(window.to_string()).or_insert(at);
        } else {
            panes.insert(window.to_string(), at);
        }
    }
    assert_eq!(lines.len(), 3, "{lines:?}");
    for (window, first) in &lines {
        assert!(panes.get(window).is_some_and(|last| last < first), "{window}: panes {panes:?}, lines {lines:?}");
    }
}

/// `REQ-AUTO-185`: a green stem, a satin column, with a pink flower head drawn over its end. The head paints
/// the larger area, but it lies over the stem and can't sew first, so the base layer sews in green, the thread
/// the order starts with. Before M13.63 the base layer sewed in pink, then the stem in green, then the head
/// in pink again.
#[test]
fn req_auto_185_the_base_layer_sews_in_the_thread_the_order_starts_with() {
    let stem = automatic(widened(line("stem", (0.0, 20.0), 30.0, &GREEN, &[]), 2.5));
    let head = area("head", &[&rect(24.0, 8.0, 48.0, 32.0)], &PINK);
    let outcome = planned(vec![stem, head]);
    let plan = plan_of(&outcome);
    assert_eq!(threaded(plan), BTreeMap::from([(GREEN.color, 1), (PINK.color, 1)]));
    assert_eq!(plan.blocks.first().map(|block| block.thread.color), Some(GREEN.color));
}

/// `REQ-AUTO-186`: with `auto_thread_once` off, the line sews after the fill it borders, as before M13.63, and
/// the sew-out piece's navy is threaded twice. The setting is known: nothing says it isn't.
#[test]
fn req_auto_186_auto_thread_once_off_sews_each_line_after_its_fill() {
    let outcome = placed(piece(&[("auto_thread_once", "false")]));
    assert!(outcome.diagnostics.iter().all(|said| !said.message.contains("auto_thread_once")), "{:?}", outcome.diagnostics);
    let plan = plan_of(&outcome);
    assert_eq!(threaded(plan), BTreeMap::from([(NAVY.color, 2), (CREAM.color, 1), (GOLD.color, 1)]));
}

/// `REQ-AUTO-187`: a red chimney with a brown outline, and a red roof with a brown outline drawn over the
/// chimney's foot, as the review set's house draws them. Red lies under brown at the chimney and over it at
/// the roof, so no order threads each thread once. `SC-W0925` says so once, of a shape that lies under a shape
/// of the other thread: the chimney under its outline, or the outline under the roof.
#[test]
fn req_auto_187_a_shade_drawn_under_and_over_another_is_named() {
    let outline = |id: &str, ring: &[Pt]| automatic(widened(along(id, rings(&[ring]), &BROWN, &[]), 0.8));
    let chimney = rect(40.0, 10.0, 45.0, 22.0);
    let roof = vec![(8.0, 28.0), (52.0, 28.0), (30.0, 8.0)];
    let outcome = planned(vec![
        area("chimney", &[&chimney], &RED),
        outline("chimney-outline", &chimney),
        area("roof", &[&roof], &RED),
        outline("roof-outline", &roof),
    ]);
    let plan = plan_of(&outcome);
    assert_eq!(plan.stats().color_changes, 3);
    let said: Vec<_> = outcome.diagnostics.iter().filter(|said| said.code.to_string() == "SC-W0925").collect();
    assert_eq!(said.len(), 1, "{:?}", outcome.diagnostics);
    // It is said of one of the shapes that lie under a shape of the other thread.
    let named = said.first().and_then(|said| said.element.as_ref()).map(ToString::to_string);
    assert!(matches!(named.as_deref(), Some("chimney" | "chimney-outline")), "{said:?}");
}
