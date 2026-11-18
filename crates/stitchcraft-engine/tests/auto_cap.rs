//! The cap at the end of planning (`REQ-AUTO-099` to `REQ-AUTO-103`, `docs/src/design/image-to-embroidery.md`
//! › The cap at the end of planning): after the last pass, where a spot of a design digitized automatically
//! still sews above the cap, needle points leave the finished plan's runs wherever the thread round them
//! covers what their stitches covered. The cap knows nothing of the shapes or their order, so it holds
//! whatever made the stack. The cases here plan the traced pear and TS-13A's digitized half, which the last
//! pass leaves above 4 fills. `tests/auto_peaks.rs` and `tests/auto_photos.rs` hold every test design to it.

// Test code may unwrap and index (clippy.toml allows it in tests).
#![allow(clippy::unwrap_used, clippy::panic, clippy::indexing_slicing)]

use stitchcraft_core::Code;
use stitchcraft_engine::auto::{ADVANCED, AutoParams};
use stitchcraft_engine::design::{Design, Element};
use stitchcraft_engine::testsheets::find;
use stitchcraft_plan::{Role, Stitch, StitchKind, StitchPlan};
use stitchcraft_testkit::designs::{in_place, rules};
use stitchcraft_testkit::fabric::{self, GAP, Limits};
use stitchcraft_testkit::pictures;

/// The key of the cap.
const CAP: &str = "auto_thick_cap_fills";

/// `design` with `value` as the cap of every automatic shape.
fn capped(design: &Design, value: &str) -> Design {
    let mut elements = design.elements().to_vec();
    for element in elements.iter_mut().filter(|element| element.automatic) {
        element.params.set(CAP, value);
    }
    Design::new(elements, design.settings).unwrap()
}

/// The traced pear and TS-13A's digitized half, by name.
fn designs() -> Vec<(&'static str, Design)> {
    let sheet = find("TS-13A").unwrap().design().unwrap().unwrap();
    let shapes: Vec<Element> = sheet.elements().iter().filter(|element| element.automatic).cloned().collect();
    vec![("pear", pictures::pear()), ("ts-13a", Design::new(shapes, sheet.settings).unwrap())]
}

/// Whether the stitches of `some` are those of `all`, in the same order, with some of `all`'s left out.
fn among(some: &[Stitch], all: &[Stitch]) -> bool {
    let mut rest = all.iter();
    some.iter().all(|stitch| rest.any(|other| other == stitch))
}

/// The plan's commands and lock stitches: what the cap never takes.
fn kept(plan: &StitchPlan) -> Vec<Vec<Stitch>> {
    let held = |stitch: &&Stitch| stitch.kind != StitchKind::Normal || stitch.origin.role == Role::Lock;
    plan.blocks.iter().map(|block| block.stitches.iter().filter(held).copied().collect()).collect()
}

#[test]
fn req_auto_099_the_cap_is_an_advanced_setting_of_4_fills() {
    let spec = AutoParams::SPECS.iter().find(|spec| spec.key == CAP);
    assert!(spec.is_some_and(|spec| spec.default == "4" && spec.group == ADVANCED), "{:?}", spec.map(|spec| (spec.default, spec.group)));
    // It sits beside the 4 limits of how thick a design sews, after them.
    let keys: Vec<&str> = AutoParams::SPECS.iter().map(|spec| spec.key).collect();
    let at = keys.iter().position(|key| *key == CAP).unwrap();
    assert_eq!(keys[at - 1], "auto_thick_fail_points");
}

#[test]
fn req_auto_100_the_cap_only_takes_needle_points_out_and_bares_nothing() {
    let limits = rules();
    for (name, design) in designs() {
        // A cap of 50 fills is above every spot, so nothing is taken.
        let (on, off) = (in_place(&design).plan.unwrap(), in_place(&capped(&design, "50")).plan.unwrap());
        let (before, after) = (fabric::thickness(&off, &Limits::DEFAULT), fabric::thickness(&on, &Limits::DEFAULT));
        assert!(before.fills.value > 4.0, "{name}: {:.2}", before.fills.value);
        assert!(after.fills.value <= 4.0, "{name}: {:.2} fills at its thickest spot, {:.2} with the cap off", after.fills.value, before.fills.value);
        // Every needle point left is one the plan had, in its order: no run is cut and no stitch moves.
        assert_eq!(on.blocks.len(), off.blocks.len(), "{name}");
        for (block, (left, was)) in on.blocks.iter().zip(&off.blocks).enumerate() {
            assert_eq!(left.thread, was.thread, "{name}: block {block}");
            assert!(among(&left.stitches, &was.stitches), "{name}: block {block}");
        }
        assert!(on.stats().stitches < off.stats().stitches, "{name}");
        // The jumps, trims, stops, thread changes and locks stay as they were.
        assert_eq!(kept(&on), kept(&off), "{name}");
        // What the top thread leaves bare stays within the cover's limit, with no sliver more.
        let (cover_on, cover_off) = (fabric::cover(&design, &on, GAP), fabric::cover(&design, &off, GAP));
        assert!(cover_on.bare_share() * 100.0 <= limits.auto_base_bare_percent, "{name}: {:.4} %", cover_on.bare_share() * 100.0);
        assert!(cover_on.slivers(&limits).len() <= cover_off.slivers(&limits).len(), "{name}");
        // And so does the likeness.
        assert!(fabric::likeness(&design, &on).likeness() >= fabric::likeness(&design, &off).likeness(), "{name}");
    }
}

#[test]
fn diag_sc_i1119_the_cap_says_what_it_took_of_each_kind_and_what_is_left() {
    // The traced pear: the last pass leaves it above 4 fills, and the cap takes the rest
    // (`REQ-AUTO-101`).
    let outcome = in_place(&pictures::pear());
    let said: Vec<&str> =
        outcome.diagnostics.iter().filter(|said| said.code == Code::AutoThinned).map(|said| said.message.as_str()).collect();
    assert_eq!(said.len(), 1, "{:?}", outcome.diagnostics);
    assert!(said[0].contains(" fills or more. The last pass took ") && said[0].contains(", and the cap took "), "{}", said[0]);
    assert!(said[0].contains("Left: 0.0 mm² at 4 fills or more"), "{}", said[0]);
}
