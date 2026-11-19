//! The cap at the end of planning (`REQ-AUTO-099` to `REQ-AUTO-103`, `docs/src/design/image-to-embroidery.md`
//! › The cap at the end of planning): after the last pass, where a spot of a design digitized automatically
//! still sews above the cap, needle points leave the finished plan's runs wherever the thread round them
//! covers what their stitches covered. The cap knows nothing of the shapes or their order, so it holds
//! whatever made the stack. The cases here plan the traced pear and TS-13A's digitized half, which the last
//! pass leaves above 4 fills. `tests/auto_peaks.rs` and `tests/auto_photos.rs` hold every test design to it.

// Test code may unwrap and index (clippy.toml allows it in tests).
#![allow(clippy::unwrap_used, clippy::panic, clippy::indexing_slicing)]

use std::collections::BTreeSet;

use stitchcraft_core::{Code, math};
use stitchcraft_engine::auto::{ADVANCED, AutoParams};
use stitchcraft_engine::design::{Design, DesignSettings, Element};
use stitchcraft_engine::testsheets::find;
use stitchcraft_plan::{Role, Stitch, StitchKind, StitchPlan};
use stitchcraft_testkit::designs::{BLUE, RED, along, automatic, fill, in_place, letter, polylines, rules, widened};
use stitchcraft_testkit::fabric::{self, GAP, Limits};
use stitchcraft_testkit::metrics::edge_coverage;
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

/// Whether the stitches of `some` are those of `all`, in the same order, with some of `all`'s left out, or
/// row ends of theirs that the last pass moved back along their rows, by the overlap of 1 mm at the most
/// (`REQ-AUTO-074`).
fn among_or_moved(some: &[Stitch], all: &[Stitch]) -> bool {
    let mut rest = all.iter();
    let moved = |stitch: &Stitch, other: &Stitch| other.kind == stitch.kind && other.origin == stitch.origin && other.at.distance(stitch.at) <= 1.0;
    some.iter().all(|stitch| rest.any(|other| other == stitch || moved(stitch, other)))
}

/// The plan's commands, or its lock stitches, by block.
fn kept(plan: &StitchPlan, locks: bool) -> Vec<Vec<Stitch>> {
    let held = |stitch: &&Stitch| {
        if locks { stitch.kind == StitchKind::Normal && stitch.origin.role == Role::Lock } else { stitch.kind != StitchKind::Normal }
    };
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
        // The cap lowers the thickest spot. What holds it above the cap is a satin column's top, which the cap
        // leaves (`REQ-AUTO-112`), and `tests/auto_peaks.rs` lists it.
        assert!(before.fills.value > 4.0, "{name}: {:.2}", before.fills.value);
        assert!(
            after.fills.value < before.fills.value,
            "{name}: {:.2} fills at its thickest spot, {:.2} with the cap off",
            after.fills.value,
            before.fills.value
        );
        // Every needle point left is one the plan had, in its order, but for the row ends the last pass moves:
        // no run is cut.
        assert_eq!(on.blocks.len(), off.blocks.len(), "{name}");
        for (block, (left, was)) in on.blocks.iter().zip(&off.blocks).enumerate() {
            assert_eq!(left.thread, was.thread, "{name}: block {block}");
            assert!(among_or_moved(&left.stitches, &was.stitches), "{name}: block {block}");
        }
        assert!(on.stats().stitches < off.stats().stitches, "{name}");
        // The jumps, trims, stops and thread changes stay as they were. The cap takes no lock: the last pass,
        // which runs below the cap too, takes those beyond one back and forth (`auto/cap/tests.rs` holds the
        // cap alone to it).
        assert_eq!(kept(&on, false), kept(&off, false), "{name}");
        assert!(kept(&on, true).iter().zip(&kept(&off, true)).all(|(on, off)| among(on, off)), "{name}");
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
    // The traced pear: the last pass leaves it above the cap, and the cap takes what it may. A spot that a
    // satin column holds is left above it (`REQ-AUTO-101`, `REQ-AUTO-112`).
    let outcome = in_place(&pictures::pear());
    let said: Vec<&str> = outcome.diagnostics.iter().filter(|said| said.code == Code::AutoThinned).map(|said| said.message.as_str()).collect();
    assert_eq!(said.len(), 1, "{:?}", outcome.diagnostics);
    assert!(said[0].contains(" fills or more. The last pass took ") && said[0].contains(", and the cap took "), "{}", said[0]);
    assert!(said[0].contains("Left: ") && said[0].contains(" at 4 fills or more"), "{}", said[0]);
}

/// Each column's id and its top stitches, as their 2 needle points.
type Tops = Vec<(String, Vec<[(f64, f64); 2]>)>;

/// The top stitches of each of `columns` in `plan`, as their 2 needle points, in the plan's order.
fn tops(plan: &StitchPlan, columns: &BTreeSet<String>) -> Tops {
    let id = |stitch: &Stitch| stitch.origin.element.and_then(|element| plan.elements.get(element.index())).map(ToString::to_string);
    let mut each: Tops = columns.iter().map(|column| (column.clone(), Vec::new())).collect();
    for block in &plan.blocks {
        for pair in block.stitches.windows(2) {
            let (before, stitch) = (&pair[0], &pair[1]);
            let sewn = before.kind == StitchKind::Normal && stitch.kind == StitchKind::Normal && stitch.origin.role == Role::Top;
            if let Some(at) = id(stitch).filter(|_| sewn).and_then(|id| each.iter().position(|(column, _)| *column == id)) {
                each[at].1.push([(before.at.x(), before.at.y()), (stitch.at.x(), stitch.at.y())]);
            }
        }
    }
    each
}

/// A circle round the origin `radius` mm across, by 96 points.
fn circle(radius: f64) -> Vec<(f64, f64)> {
    (0..96).map(|k| f64::from(k) * std::f64::consts::TAU / 96.0).map(|t| (radius * math::cos(t), radius * math::sin(t))).collect()
}

/// The 2 drawn shapes of `REQ-AUTO-112`'s case: a ring of satin 4 mm wide on the edge of a disc 20 mm
/// across, which reaches under it, and a T of strokes 2.5 mm wide over a plate, its bar and its stem meeting.
fn drawn_columns() -> Vec<(&'static str, Design)> {
    let disc = automatic(fill("disc", &[&circle(10.0)], &RED, &[]));
    let edge = circle(10.0);
    let ring = automatic(widened(along("ring", polylines(&[&[&edge[..], &edge[..1]].concat()]), &BLUE, &[]), 4.0));
    let plate = automatic(fill("plate", &[&[(-4.0, -4.0), (13.0, -4.0), (13.0, 19.0), (-4.0, 19.0)]], &RED, &[]));
    let t = automatic(fill("t", &[&letter('T', 15.0, 2.5)], &BLUE, &[]));
    let design = |elements: Vec<Element>| Design::new(elements, DesignSettings::default()).unwrap();
    vec![("ring over a disc", design(vec![disc, ring])), ("T over a plate", design(vec![plate, t]))]
}

#[test]
fn req_auto_112_the_cap_leaves_every_top_stitch_of_a_satin_column_as_digitizing_laid_it() {
    for (name, design) in drawn_columns() {
        let columns = fabric::columns(&design);
        assert!(!columns.is_empty(), "{name}: no column");
        // A cap of 50 fills is above every spot, so nothing is taken. At 2, the cap takes thread wherever the
        // column lies over the area under it, its inner edge and its parts' join among those places.
        let off = in_place(&capped(&design, "50")).plan.unwrap();
        let on = in_place(&capped(&design, "2")).plan.unwrap();
        let thickest = fabric::thickness(&off, &Limits::DEFAULT).fills.value;
        assert!(thickest > 2.0, "{name}: {thickest:.2} fills at its thickest spot with the cap off");
        assert!(on.stats().stitches < off.stats().stitches, "{name}: the cap took nothing");
        // Every top stitch of every column is where digitizing laid it: no needle point at its edge goes, and
        // no stitch turns from square to its column.
        assert_eq!(tops(&on, &columns), tops(&off, &columns), "{name}");
    }
}

#[test]
fn req_auto_114_the_cap_keeps_the_edge_under_a_later_shape_covered_as_the_fabric_pulls() {
    // The ring of satin lies on the disc's edge, and the disc reaches under it by the overlap. Pulled in by
    // the overlap where the thread turns, as `REQ-AUTO-007` measures, the thread still covers the ring's inner
    // edge, which the disc's row ends under the ring cover: the cap at 2 fills takes none of them there.
    let (_, design) = drawn_columns().into_iter().next().unwrap();
    let overlap = rules().auto_overlap_mm.get();
    let edge = vec![circle(8.0).into_iter().chain(circle(8.0).into_iter().take(1)).map(|(x, y)| stitchcraft_testkit::designs::p(x, y)).collect()];
    let (off, on) = (in_place(&capped(&design, "50")).plan.unwrap(), in_place(&capped(&design, "2")).plan.unwrap());
    let (was, is) = (edge_coverage(&edge, &off, overlap), edge_coverage(&edge, &on, overlap));
    assert!(was > 0.99, "{was}");
    assert!(is >= was, "{is:.4} of the edge covered, {was:.4} with the cap off");
}
