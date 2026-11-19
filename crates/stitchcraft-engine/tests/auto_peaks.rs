//! No spot of a design digitized automatically sews above a cap (`REQ-AUTO-071` to `REQ-AUTO-074`,
//! `docs/src/design/image-to-embroidery.md` › The thickest spot): what a single shape piles up by itself, its
//! columns' crowded needle points (`tests/auto_columns.rs`) and its lock stitches, and the thickest spot of
//! each test picture, the traced pear, the lettered patch and the TS-13 sheets' digitized halves, each
//! planned with its stitches where it draws its shapes. The traced dogs' are `tests/auto_photos.rs`'s, in
//! the release build.

// Test code may unwrap and index (clippy.toml allows it in tests).
#![allow(clippy::unwrap_used, clippy::panic, clippy::indexing_slicing)]

use stitchcraft_core::Budget;
use stitchcraft_engine::auto::{self};
use stitchcraft_engine::common::{LOCK_END, LOCK_END_SCALE, LOCK_START, LOCK_START_SCALE};
use stitchcraft_engine::design::{Design, DesignSettings, Element};
use stitchcraft_engine::testsheets::SHEETS;
use stitchcraft_plan::Role;
use stitchcraft_plan::profiles::REFERENCE;
use stitchcraft_testkit::designs::{BLUE, RED, automatic, fill, in_place, planned};
use stitchcraft_testkit::fabric::{self, Limits};
use stitchcraft_testkit::pictures::{self, Trace};

/// The most thread, in fills, that a spot of a millimetre of a design digitized automatically may sew
/// (`REQ-AUTO-073`).
const CAP: f64 = 4.0;

/// A square from (`x`, `y`), `side` millimetres across, automatic, in `thread` with the settings `params`.
fn square(id: &str, (x, y): (f64, f64), side: f64, params: &[(&str, &str)]) -> Element {
    let corners = [(x, y), (x + side, y), (x + side, y + side), (x, y + side)];
    let thread = if id.starts_with('b') { BLUE } else { RED };
    automatic(fill(id, &[&corners], &thread, params))
}

/// The setting `key` of `element`.
fn setting<'e>(element: &'e Element, key: &str) -> Option<&'e str> {
    element.params.iter().find(|(k, _)| *k == key).map(|(_, value)| value)
}

#[test]
fn req_auto_072_what_digitizing_makes_locks_in_steps_of_the_shortest_stitch_unless_its_shape_sets_its_lock() {
    // 2 squares far apart, so that the thread jumps between them and each locks; the second sets its own end
    // lock, the third its start lock's step.
    let shapes = vec![
        square("red", (0.0, 0.0), 10.0, &[]),
        square("blue", (40.0, 0.0), 10.0, &[(LOCK_END, "star")]),
        square("rose", (80.0, 0.0), 10.0, &[(LOCK_START_SCALE, "1")]),
    ];
    let design = Design::new(shapes.clone(), DesignSettings::default()).unwrap();
    let (elements, _) = auto::digitized(&design, &Budget::DEFAULT);
    let step = REFERENCE.min_stitch.get().to_string();
    let locks = |id: &str| {
        let element = elements.iter().find(|element| element.id.as_str() == id).unwrap();
        [LOCK_START, LOCK_START_SCALE, LOCK_END, LOCK_END_SCALE].map(|key| setting(element, key).map(str::to_string))
    };
    let (ours, steps) = (Some("back_forth".to_string()), Some(step));
    assert_eq!(locks("red"), [ours.clone(), steps.clone(), ours.clone(), steps.clone()]);
    assert_eq!(locks("blue"), [ours.clone(), steps.clone(), Some("star".to_string()), None]);
    assert_eq!(locks("rose"), [None, Some("1".to_string()), ours, steps]);
    // Each lock stitch of the first square, the one sewn with the rules' own locks, is a step long at most.
    let plan = planned(shapes[..1].iter().cloned().chain([square("red-2", (40.0, 0.0), 10.0, &[])]).collect()).plan.unwrap();
    let lengths: Vec<f64> = plan
        .sewn_stitches()
        .iter()
        .filter(|stitch| stitch.role == Role::Lock && stitch.from_role == Role::Lock)
        .map(|stitch| stitch.length())
        .collect();
    assert!(lengths.len() >= 8, "{lengths:?}");
    assert!(lengths.iter().all(|length| *length <= REFERENCE.min_stitch.get() + 1e-6), "{lengths:?}");
}

/// Each design the cases hold to the cap, by name: the test pictures, the traced pear, the lettered patch and
/// each TS-13 sheet's digitized half (its automatic shapes, by the sheet's settings), planned in place.
fn designs() -> Vec<(String, Design)> {
    let mut designs: Vec<(String, Design)> = pictures::all()
        .into_iter()
        .chain([Trace::Abutting, Trace::Stacked].into_iter().flat_map(|trace| [pictures::emblem(trace), pictures::button(trace)]))
        .map(|picture| (picture.name, Design::new(picture.elements, DesignSettings::default()).unwrap()))
        .collect();
    designs.push(("pear".to_string(), pictures::pear()));
    designs.push(("lettered-patch".to_string(), pictures::lettered_patch()));
    for sheet in SHEETS.iter().filter(|sheet| sheet.digitized()) {
        let design = sheet.design().unwrap().unwrap();
        let shapes: Vec<Element> = design.elements().iter().filter(|element| element.automatic).cloned().collect();
        designs.push((sheet.id.to_lowercase(), Design::new(shapes, design.settings).unwrap()));
    }
    designs
}

/// `value` to the hundredth.
fn hundredths(value: f64) -> f64 {
    (value * 100.0).round() / 100.0
}

#[test]
fn req_auto_073_no_spot_of_a_digitized_picture_sews_above_the_cap() {
    // The cap at the end of planning (REQ-AUTO-100) emptied the list that this case kept of the designs
    // above 4 fills: the badges at 4.40, the pear at 4.20, the lettered patch at 4.00 and the digitized
    // halves of TS-13A, C and D at 4.28, 4.49 and 4.23.
    let mut above = Vec::new();
    for (name, design) in designs() {
        let outcome = in_place(&design);
        let plan = outcome.plan.unwrap_or_else(|| panic!("{name}: {:?}", outcome.diagnostics));
        let thickest = fabric::thickness(&plan, &Limits::DEFAULT).fills.value;
        if thickest > CAP {
            above.push((name, hundredths(thickest)));
        }
    }
    assert!(above.is_empty(), "{above:?}");
}

#[test]
fn req_auto_102_the_cap_keeps_each_design_s_jumps_and_thread_changes() {
    // Each design with the cap at 50 fills, above every spot, and as digitized: the cap and the last pass
    // take stitches, and never a jump or a thread change.
    for (name, design) in designs() {
        let mut elements = design.elements().to_vec();
        for element in elements.iter_mut().filter(|element| element.automatic) {
            element.params.set("auto_thick_cap_fills", "50");
        }
        let off = in_place(&Design::new(elements, design.settings).unwrap()).plan.unwrap();
        let on = in_place(&design).plan.unwrap();
        let (on, off) = (on.stats(), off.stats());
        assert_eq!((on.jumps, on.trims, on.color_changes), (off.jumps, off.trims, off.color_changes), "{name}");
        assert!(on.stitches <= off.stitches, "{name}: {} stitches, {} with the cap off", on.stitches, off.stitches);
    }
}
