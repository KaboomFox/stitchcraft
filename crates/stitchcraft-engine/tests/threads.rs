//! Several threads in one element (M10.5): assembly's cases (`REQ-ASM-006`, `REQ-ASM-007`).
//!
//! No stitch type gives its groups threads of their own yet. Tartan fills will from M10.6, and gradient
//! fills from M10.12. So each case gives assembly what such a generator would: an element generated as
//! `plan` generates it, its groups then given their colours (`stitchcraft_testkit::designs::threaded`), and
//! assembled and finalized as `plan` assembles and finalizes them.
//!
//! The shapes Ink/Stitch's plan gives them, run as a black box at `d59c9ab` on a linear gradient fill of 3
//! stops and on a tartan of 3 colours, with its stitch-plan cache off, and written as CSV, its runs of
//! stitches counted (`S633` is 633 stitches, its locks among them):
//!
//! | Ink/Stitch's element | Its plan |
//! |---|---|
//! | Gradient, 3 stops | `J S633 CC J S568 CC J S288` |
//! | The same, `trim_after` on | `J S633 TRIM CC J S568 TRIM CC J S288 TRIM` |
//! | The same in 2 parts, `trim_after` on | `J S647 TRIM J S634 TRIM CC J S568 TRIM J S586 TRIM CC J S288 TRIM J S288 TRIM` |
//! | Tartan of 3 colours, filled black, `trim_after` on | `J S530 CC J S1084 TRIM CC J S1542 TRIM CC J S111 TRIM` |
//!
//! Its underlay sews first, in the gradient's first colour or in the tartan's own fill, and no trim follows
//! it. Each colour of each part follows, and a trim after each when `trim_after` is on.

// Test code may unwrap and index (clippy.toml allows it in tests).
#![allow(clippy::unwrap_used)]

use stitchcraft_core::{Budget, ElementId};
use stitchcraft_engine::assemble::assemble;
use stitchcraft_engine::design::{DesignSettings, Element};
use stitchcraft_engine::finalize::finalize;
use stitchcraft_engine::generate::generate;
use stitchcraft_engine::generators::{Colour, Neighbours};
use stitchcraft_plan::invariants;
use stitchcraft_plan::profiles::REFERENCE;
use stitchcraft_plan::{Rgb, StitchPlan, Thread};
use stitchcraft_testkit::designs::{BLUE, GREEN, RED, block, line, messages, shape, shape_of, stroke, threaded, three_threads, within};

/// A black thread, as a tartan's own fill.
const BLACK: Thread = Thread::new(Rgb::new(0, 0, 0));

/// A stroke `id` of `count` lines 10 mm long, each 5 mm below the one before from `(x, y)`, sewn with
/// `thread` and `params`: a group each, which jump from one to the next.
fn lines(id: &str, (x, y): (f64, f64), count: u32, thread: &Thread, params: &[(&str, &str)]) -> Element {
    let parts: Vec<((f64, f64), f64)> = (0..count).map(|i| ((x, y + 5.0 * f64::from(i)), 10.0)).collect();
    stroke(id, &parts, thread, params)
}

/// The shape of what `lines` of `count` lines, given `colours`, plans to with `params`; its plan must exist,
/// and nothing be said of it.
fn sewn(count: u32, thread: &Thread, colours: Vec<Colour>, params: &[(&str, &str)]) -> String {
    let outcome = threaded(&[(lines("e", (0.0, 0.0), count, thread, params), colours)], DesignSettings::default());
    assert!(outcome.diagnostics.is_empty(), "{:?}", messages(&outcome));
    shape_of(&outcome)
}

/// The colours of `plan`'s blocks, in their order.
fn threads(plan: &StitchPlan) -> Vec<Rgb> {
    plan.blocks.iter().map(|block| block.thread.color).collect()
}

#[test]
fn req_asm_006_an_element_of_3_threads_sews_3_colour_blocks_as_ink_stitch_s_plan_does() {
    let three = || vec![block(&RED), block(&GREEN), block(&BLUE)];
    // Ink/Stitch's gradient of 3 stops: a block each, the needle jumping to each and locking it in.
    assert_eq!(sewn(3, &RED, three(), &[]), "J L4 S5 L4 | J L4 S5 L4 | J L4 S5 L4");
    assert_eq!(sewn(3, &RED, three(), &[("trim_after", "true")]), "J L4 S5 L4 T | J L4 S5 L4 T | J L4 S5 L4 T");
    let plan = three_threads();
    assert_eq!(shape(&plan), "J L4 S5 L4 T | J L4 S5 L4 T | J L4 S5 L4 T");
    assert_eq!(threads(&plan), [RED.color, GREEN.color, BLUE.color]);
    let stats = plan.stats();
    assert_eq!((stats.color_changes, stats.trims, stats.jumps), (2, 3, 3));
    assert_eq!(plan.elements.iter().map(ElementId::as_str).collect::<Vec<_>>(), ["gradient"], "one element, in every block");
    assert!(invariants::check(&plan, REFERENCE).is_empty());
    // Its locks are as `ties` allows, at every thread change.
    assert_eq!(sewn(3, &RED, three(), &[("trim_after", "true"), ("ties", "3")]), "J S5 T | J S5 T | J S5 T");
    assert_eq!(sewn(3, &RED, three(), &[("ties", "1")]), "J L4 S5 | J L4 S5 | J L4 S5");
}

#[test]
fn req_asm_006_a_group_without_a_thread_of_its_own_is_sewn_in_its_element_s() {
    // An element in blue: its first group's colour gives no thread, its second gives red, and its third has
    // no colour at all.
    let outcome = threaded(&[(lines("e", (0.0, 0.0), 3, &BLUE, &[]), vec![within(None), block(&RED)])], DesignSettings::default());
    let plan = outcome.plan.unwrap();
    assert_eq!(shape(&plan), "J L4 S5 L4 | J L4 S5 L4 | J L4 S5 L4");
    assert_eq!(threads(&plan), [BLUE.color, RED.color, BLUE.color]);
    // Colours of one thread are one thread: no change, whatever their names.
    let scarlet = Thread::named(RED.color, "Scarlet");
    let renamed = threaded(&[(lines("e", (0.0, 0.0), 2, &RED, &[]), vec![within(Some(&scarlet)), within(None)])], DesignSettings::default());
    let plan = renamed.plan.unwrap();
    assert_eq!(shape(&plan), "J L4 S5 L4 J L4 S5 L4");
    assert_eq!(plan.blocks[0].thread, scarlet, "the block keeps its first thread");
}

#[test]
fn req_asm_006_the_plan_starts_in_the_thread_of_the_first_group_sewn() {
    // The element is red, its first group green: the plan's first block is green, with no empty red block
    // before it.
    let outcome = threaded(&[(lines("e", (0.0, 0.0), 2, &RED, &[]), vec![block(&GREEN), within(None)])], DesignSettings::default());
    let plan = outcome.plan.unwrap();
    assert_eq!(threads(&plan), [GREEN.color, RED.color]);
    assert!(invariants::check(&plan, REFERENCE).is_empty());
    // A group without stitches is left out, its colour with it, as Ink/Stitch leaves out an empty group.
    let element = lines("e", (0.0, 0.0), 2, &RED, &[("trim_after", "true")]);
    let mut generated = generate(&element, &DesignSettings::default(), REFERENCE, &Neighbours::default(), &Budget::DEFAULT).generated.unwrap();
    generated.groups.insert(0, Vec::new());
    generated.colours = vec![block(&BLUE), block(&GREEN), block(&GREEN)];
    let assembled = assemble(&[(&element, generated)], &DesignSettings::default(), &mut Budget::DEFAULT.meter()).unwrap().unwrap();
    assert_eq!(shape(&assembled.plan), "J L4 S5 L4 T J L4 S5 L4 T");
    assert_eq!(threads(&assembled.plan), [GREEN.color]);
}

#[test]
fn req_asm_006_the_thread_changes_inside_an_element_as_between_2_elements() {
    // A red line, an element of red, green and blue whose first line starts 0.5 mm from the red line's end
    // and whose last ends 0.5 mm from a blue line: the red and the blue are sewn on into the colour blocks of
    // their thread, as Ink/Stitch joins groups of one colour.
    let a = line("a", (0.0, 0.0), 10.0, &RED, &[]);
    let b = stroke("b", &[((10.5, 0.0), 10.0), ((0.0, 5.0), 10.0), ((0.0, 10.0), 10.0)], &RED, &[]);
    let c = line("c", (10.5, 10.0), 10.0, &BLUE, &[]);
    let colours = vec![block(&RED), block(&GREEN), block(&BLUE)];
    let outcome = threaded(&[(a, Vec::new()), (b, colours), (c, Vec::new())], DesignSettings::default());
    assert!(outcome.diagnostics.is_empty(), "{:?}", messages(&outcome));
    let plan = outcome.plan.unwrap();
    assert_eq!(shape(&plan), "J L4 S10 L4 | J L4 S5 L4 | J L4 S10 L4");
    assert_eq!(threads(&plan), [RED.color, GREEN.color, BLUE.color]);
}

#[test]
fn req_asm_007_each_colour_block_is_trimmed_after_and_the_stop_comes_last() {
    let three = || vec![block(&RED), block(&GREEN), block(&BLUE)];
    assert_eq!(sewn(3, &RED, three(), &[("trim_after", "true"), ("stop_after", "true")]), "J L4 S5 L4 T | J L4 S5 L4 T | J L4 S5 L4 T P");
    assert_eq!(sewn(3, &RED, three(), &[("stop_after", "true")]), "J L4 S5 L4 | J L4 S5 L4 | J L4 S5 L4 P");
    // A block of one thread is trimmed after too, and 2 blocks of one thread change no thread between them.
    assert_eq!(sewn(2, &RED, vec![block(&RED), block(&RED)], &[("trim_after", "true")]), "J L4 S5 L4 T J L4 S5 L4 T");
    assert_eq!(sewn(2, &RED, vec![block(&RED), block(&GREEN)], &[("trim_after", "true")]), "J L4 S5 L4 T | J L4 S5 L4 T");
    // Without colours the element is one block, trimmed after its last group only, as before M10.5.
    assert_eq!(sewn(2, &RED, Vec::new(), &[("trim_after", "true")]), "J L4 S5 L4 J L4 S5 L4 T");
}

#[test]
fn req_asm_007_a_group_that_ends_no_colour_block_is_not_trimmed_after() {
    // Ink/Stitch's tartan: its underlay in its own fill's thread, then each colour, trimmed after each.
    let tartan = vec![within(None), block(&RED), block(&GREEN), block(&BLUE)];
    assert_eq!(sewn(4, &BLACK, tartan, &[("trim_after", "true")]), "J L4 S5 L4 | J L4 S5 L4 T | J L4 S5 L4 T | J L4 S5 L4 T");
    // Ink/Stitch's gradient in 2 parts: each part's underlay in the first colour, and its colours sorted
    // so that each sews once, a trim after each part's.
    let parts = vec![within(Some(&RED)), block(&RED), within(Some(&RED)), block(&RED), block(&GREEN), block(&GREEN)];
    assert_eq!(sewn(6, &RED, parts, &[("trim_after", "true")]), "J L4 S5 L4 J L4 S5 L4 T J L4 S5 L4 J L4 S5 L4 T | J L4 S5 L4 T J L4 S5 L4 T");
    // The colour block's last group is trimmed after, not those before it in the block.
    assert_eq!(
        sewn(3, &RED, vec![within(Some(&GREEN)), block(&GREEN), block(&BLUE)], &[("trim_after", "true")]),
        "J L4 S5 L4 J L4 S5 L4 T | J L4 S5 L4 T"
    );
}

#[test]
fn req_asm_007_a_colour_block_of_one_needle_point_is_sewn_and_trimmed_after() {
    // A group of one needle point has no stitch to lock, as any group of one.
    let element = lines("e", (0.0, 0.0), 2, &RED, &[("trim_after", "true")]);
    let mut generated = generate(&element, &DesignSettings::default(), REFERENCE, &Neighbours::default(), &Budget::DEFAULT).generated.unwrap();
    generated.groups[0].truncate(1);
    generated.colours = vec![block(&GREEN), block(&BLUE)];
    let (settings, mut meter) = (DesignSettings::default(), Budget::DEFAULT.meter());
    let assembled = assemble(&[(&element, generated)], &settings, &mut meter).unwrap().unwrap();
    assert_eq!(shape(&assembled.plan), "J S1 T | J L4 S5 L4 T");
    // The machine sews it: finalize keeps the block's one stitch, and the plan passes its checks.
    let finalized = finalize(assembled.plan, REFERENCE, &settings, &assembled.shortest, &mut meter).unwrap();
    assert_eq!(finalized.diagnostics, Vec::new());
    assert_eq!(shape(&finalized.plan.unwrap()), "J S1 T | J L4 S5 L4 T");
}
