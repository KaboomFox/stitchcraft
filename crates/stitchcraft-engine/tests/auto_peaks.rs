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

/// The thickest spot of each design still above the cap, in fills to the hundredth, as measured. The list
/// only shrinks: a design that comes under the cap leaves it. Since the measure's grid is laid from the
/// plan's first needle point (`REQ-AUTO-074`), the badges measure 4.40, where they measured 4.20 on the
/// machine file's grid: a needle point on a cell's edge counts on the other side.
/// The lettered patch came to 4.00 when its letters sewed as strokes (M13.37), and TS-13D went to 4.23 from 4.17
/// at its stacked emblem's white ring, whose frame then sews as strokes and whose column starts elsewhere:
/// its area at 3 fills or more fell by 16.5 mm².
const ABOVE: [(&str, f64); 7] =
    [("badges-abutting", 4.4), ("badges-stacked", 4.4), ("pear", 4.2), ("lettered-patch", 4.0), ("ts-13a", 4.28), ("ts-13c", 4.49), ("ts-13d", 4.23)];

#[test]
fn req_auto_073_no_spot_of_a_digitized_picture_sews_above_the_cap_but_as_listed() {
    let mut above = Vec::new();
    for (name, design) in designs() {
        let outcome = in_place(&design);
        let plan = outcome.plan.unwrap_or_else(|| panic!("{name}: {:?}", outcome.diagnostics));
        let thickest = fabric::thickness(&plan, &Limits::DEFAULT).fills.value;
        if thickest > CAP {
            above.push((name, hundredths(thickest)));
        }
    }
    assert_eq!(above, ABOVE.map(|(name, thickest)| (name.to_string(), thickest)));
}
