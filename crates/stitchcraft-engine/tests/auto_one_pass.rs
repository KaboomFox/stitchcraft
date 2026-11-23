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
use stitchcraft_plan::{Rgb, Role, StitchPlan, Thread};
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
    // `SC-I0909` names the thread the base layer sews in.
    let base: Vec<&str> = outcome.diagnostics.iter().filter(|said| said.code.to_string() == "SC-I0909").map(|said| said.message.as_str()).collect();
    assert!(base.len() == 1 && base.iter().all(|message| message.ends_with(&format!("in the thread {}.", GREEN.color))), "{base:?}");
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

/// The share of the top thread of element `id` of `plan` that no later top stitch covers: each top stitch
/// a band 0.4 mm wide, a thread, painted in the order they sew on a grid of 0.05 mm.
// A cell of a plan 500 mm across at most: a whole number far inside an i64.
#[allow(clippy::cast_possible_truncation)]
fn shown(plan: &StitchPlan, id: &str) -> f64 {
    const STEP: f64 = 0.05;
    const HALF: f64 = 0.2;
    let tops: Vec<(bool, [f64; 4])> = plan
        .sewn_stitches()
        .iter()
        .filter(|stitch| stitch.role == Role::Top)
        .filter_map(|stitch| {
            let element = plan.blocks.get(stitch.block)?.stitches.get(stitch.index)?.origin.element?;
            let own = plan.element(element).is_some_and(|element| element.as_str() == id);
            Some((own, [stitch.from.x(), stitch.from.y(), stitch.to.x(), stitch.to.y()]))
        })
        .collect();
    let mut top: BTreeMap<(i64, i64), bool> = BTreeMap::new();
    let mut own: Vec<(i64, i64)> = Vec::new();
    for (mine, [ax, ay, bx, by]) in tops {
        let cell = |v: f64| (v / STEP).round() as i64;
        let (dx, dy) = (bx - ax, by - ay);
        let length = dx * dx + dy * dy;
        for row in cell(ay.min(by) - HALF)..=cell(ay.max(by) + HALF) {
            for column in cell(ax.min(bx) - HALF)..=cell(ax.max(bx) + HALF) {
                let (px, py) = (column as f64 * STEP, row as f64 * STEP);
                let along = if length > 0.0 { (((px - ax) * dx + (py - ay) * dy) / length).clamp(0.0, 1.0) } else { 0.0 };
                let (qx, qy) = (ax + along * dx - px, ay + along * dy - py);
                if qx * qx + qy * qy <= HALF * HALF {
                    top.insert((column, row), mine);
                    if mine {
                        own.push((column, row));
                    }
                }
            }
        }
    }
    own.sort_unstable();
    own.dedup();
    own.iter().filter(|cell| top.get(cell) == Some(&true)).count() as f64 / own.len().max(1) as f64
}

/// The sew-out piece with its navy line `width` mm wide, each shape with `params`.
fn lined(width: f64, params: &[(&str, &str)]) -> Vec<Element> {
    let shape = |id: &str, rings: &[&[Pt]], thread: &Thread| automatic(fill(id, rings, thread, params));
    vec![
        shape("dark", &[&rect(2.0, 2.0, 58.0, 42.0)], &NAVY),
        shape("line", &[&rect(16.0 - width, 12.0 - width, 44.0 + width, 32.0 + width), &hole(16.0, 12.0, 44.0, 32.0)], &NAVY),
        shape("light", &[&rect(16.0, 12.0, 44.0, 32.0)], &CREAM),
        shape("gold", &[&rect(62.0, 32.0, 70.0, 42.0)], &GOLD),
    ]
}

/// `REQ-AUTO-188`: how much of a navy line's thread still shows round a cream fill sewn after it, as recorded on
/// 2026-11-22, for lines 0.3, 0.6, 0.8 and 1.5 mm wide, in thousandths: with `auto_thread_once` on, as by
/// default, and off, which sews the line after the fill, as before M13.63. A line narrower than the narrowest
/// satin stroke sews as a running stitch along its middle, and the cream fill's rows, sewn after it, reach
/// their pull compensation onto it. The line 1.5 mm wide sews as a satin column, which sews after the fill
/// either way (rule 4).
#[test]
fn req_auto_188_a_line_sewn_before_the_fill_beside_it_keeps_the_share_recorded() {
    let share = |width: f64, params: &[(&str, &str)]| {
        let outcome = placed(lined(width, params));
        let plan = plan_of(&outcome);
        (width, (shown(plan, "svg:line:fill").max(shown(plan, "line")) * 1000.0).round())
    };
    let on: Vec<(f64, f64)> = [0.3, 0.6, 0.8, 1.5].iter().map(|&width| share(width, &[])).collect();
    let off: Vec<(f64, f64)> = [0.3, 0.6, 0.8, 1.5].iter().map(|&width| share(width, &[("auto_thread_once", "false")])).collect();
    // On, the cream rows cover about half of the 0.3 mm line's thread, and less of the wider lines', whose thread
    // lies farther from the fill's edge. Off, the line's thread shows whole. The column is the same either way.
    assert_eq!(
        (on, off),
        (vec![(0.3, 491.0), (0.6, 823.0), (0.8, 924.0), (1.5, 514.0)], vec![(0.3, 1000.0), (0.6, 1000.0), (0.8, 1000.0), (1.5, 514.0)])
    );
}

/// `SC-I0926`: the sew-out piece's navy line sews before the cream fill it borders, and the notice counts it and
/// names it. With `auto_thread_once` off it sews after the fill, and nothing is said.
#[test]
fn diag_sc_i0926_a_line_sewn_before_the_fill_beside_it_is_counted_and_named() {
    let codes = |outcome: &PlanOutcome| outcome.diagnostics.iter().filter(|said| said.code.to_string() == "SC-I0926").cloned().collect::<Vec<_>>();
    let said = codes(&placed(piece(&[])));
    assert_eq!(said.len(), 1, "{said:?}");
    assert!(
        said.iter()
            .all(|said| said.message.starts_with("1 line sews before a fill of another thread beside them") && said.message.contains("\"line\"")),
        "{said:?}"
    );
    assert_eq!(said.first().and_then(|said| said.element.as_ref()).map(ToString::to_string).as_deref(), Some("line"));
    assert!(codes(&placed(piece(&[("auto_thread_once", "false")]))).is_empty());
}

/// `REQ-AUTO-189`: the sew-out piece held to a limit of 0.5 fills sews above it threaded once and with its line
/// after the cream fill alike, so it sews as it was planned, threaded once, and only the thickness warning is
/// said.
#[test]
fn req_auto_189_a_design_above_the_limit_either_way_keeps_the_plan_threaded_once() {
    let outcome = placed(piece(&[("auto_thick_warn_fills", "0.5")]));
    let plan = plan_of(&outcome);
    assert_eq!(threaded(plan), BTreeMap::from([(NAVY.color, 1), (CREAM.color, 1), (GOLD.color, 1)]));
    let codes: Vec<String> = outcome.diagnostics.iter().map(|said| said.code.to_string()).collect();
    assert!(codes.contains(&"SC-W0913".to_string()) && !codes.contains(&"SC-W0927".to_string()), "{codes:?}");
}
