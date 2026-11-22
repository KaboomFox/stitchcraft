//! No spot of a design digitized automatically sews above a cap (`REQ-AUTO-071` to `REQ-AUTO-074`,
//! `docs/src/design/image-to-embroidery.md` › The thickest spot): what a single shape piles up by itself, its
//! columns' crowded needle points (`tests/auto_columns.rs`) and its lock stitches, and the thickest spot of
//! each test picture, the traced pear, the lettered patch and the TS-13 sheets' digitized halves, each
//! planned with its stitches where it draws its shapes. The traced dogs' are `tests/auto_photos.rs`'s, in
//! the release build.

// Test code may unwrap and index (clippy.toml allows it in tests).
#![allow(clippy::unwrap_used, clippy::panic, clippy::indexing_slicing)]

use stitchcraft_core::Budget;
use stitchcraft_engine::auto::{self, DETAILS_LAST, MIN_LIKENESS};
use stitchcraft_engine::common::{LOCK_END, LOCK_END_SCALE, LOCK_START, LOCK_START_SCALE};
use stitchcraft_engine::design::{Design, DesignSettings, Element};
use stitchcraft_engine::testsheets::SHEETS;
use stitchcraft_plan::Role;
use stitchcraft_plan::profiles::REFERENCE;
use stitchcraft_testkit::designs::{BLUE, RED, automatic, fill, in_place, planned, rules};
use stitchcraft_testkit::fabric;
use stitchcraft_testkit::pictures::{self, Trace};
use stitchcraft_testkit::stitches::jump_threads;

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

/// The designs whose thickest spot sews above the cap, with it in fills, as measured on 2026-11-22 with the
/// cap at 3.25 (`REQ-AUTO-073`). The list only shrinks. With smooth satin edges (M13.58) the buttons fall
/// under the cap, TS-13B's thickest spot is where fills alone meet ([`FILLS_ALONE`]), and TS-13A and TS-13C
/// sew where the cap pulled crowded points in: 3.95 and 3.93, from 3.99 and 3.92. TS-13A sewed 4.00 before its
/// abutting badge's blue was filled whole under its gold disc (M13.61). TS-13D sews 3.79, from 3.97, with its
/// white disc filled whole under its star (M13.59, `REQ-AUTO-174`). The pear's stem sews 3.95 where it sewed
/// 3.86 with a column 0.4 mm under a later column (M13.57): before the cap its spot stays under 4, where the cap
/// pulls no point in and takes less of a column's doubled top. It stays under the warning's limit. With a
/// column's pull compensation tapered into its points (M13.62, `REQ-AUTO-183`), the 2 badges sew 3.99 where
/// they sewed 3.96, and TS-13A 3.99 where it sewed 3.95: the cap takes a column's top points only above 4,
/// and a narrower point changes where it stops. Their area above the cap falls.
const ABOVE: [(&str, f64); 8] = [
    ("badges-abutting", 3.99),
    ("badges-stacked", 3.99),
    ("pear", 3.95),
    ("lettered-patch", 3.95),
    ("ts-13a", 3.99),
    ("ts-13b", 3.46),
    ("ts-13c", 3.93),
    ("ts-13d", 3.79),
];

/// The designs whose thickest spot sews above the cap with no satin column there, with it in fills
/// (`REQ-AUTO-073`): TS-13B's red and blue fills meet over the base layer under its window, and the top of
/// each shows, which the cap takes only where a shape lays it twice. It is TS-13B's thickest spot since its
/// columns keep their spacing on the inside of a bend (M13.58), where a column's spot of 3.85 was. The list
/// only shrinks.
const FILLS_ALONE: [(&str, f64); 1] = [("ts-13b", 3.46)];

#[test]
fn req_auto_073_a_spot_of_a_digitized_picture_sews_above_the_cap_only_on_satin_columns() {
    // The cap at the end of planning (REQ-AUTO-100) leaves a satin column's top stitches as they are
    // (REQ-AUTO-112). Where a spot stays above the cap, the rest of its thread is the cap or less: the cap took
    // what it may. On every design listed, 2 columns of one shape meet over a fill, or a column lies over
    // fills whose top shows.
    let (cap, limit) = (rules().auto_thick_cap_fills, rules().auto_thick_warn_fills);
    let (mut above, mut alone) = (Vec::new(), Vec::new());
    for (name, design) in designs() {
        let outcome = in_place(&design);
        let plan = outcome.plan.unwrap_or_else(|| panic!("{name}: {:?}", outcome.diagnostics));
        let (thickest, columns) = fabric::thickest(&plan, &fabric::columns(&design));
        // No spot above the warning's limit: there the cap takes columns' points too, as a last resort.
        assert!(thickest <= limit + 1e-9, "{name}: {thickest:.2} fills");
        if thickest - columns > cap {
            alone.push((name.clone(), hundredths(thickest)));
        }
        if thickest > cap {
            above.push((name, hundredths(thickest)));
        }
    }
    assert_eq!(above, ABOVE.map(|(name, thickest)| (name.to_string(), thickest)));
    assert_eq!(alone, FILLS_ALONE.map(|(name, thickest)| (name.to_string(), thickest)));
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

/// `design` with each automatic shape's `settings` set.
fn with(design: &Design, settings: &[(&str, &str)]) -> Design {
    let mut elements = design.elements().to_vec();
    for element in elements.iter_mut().filter(|element| element.automatic) {
        for (key, value) in settings {
            element.params.set(*key, *value);
        }
    }
    Design::new(elements, design.settings).unwrap()
}

#[test]
fn req_auto_103_with_lines_kept_and_details_last_each_design_sews_above_the_cap_only_on_satin_columns() {
    // Both settings change the order the blocks sew in. Likeness merging is on by default and details last off
    // (M13.48), and both are set here so that the case holds whatever the defaults: at each design's thickest
    // spot, the thread other than satin columns' top stitches, which the cap leaves (REQ-AUTO-112), is the cap
    // or less, and the cap, against a cap of 50 fills above every spot, keeps its jumps, jump threads, thread
    // changes and likeness.
    let reordered = [(MIN_LIKENESS, "0.85"), (DETAILS_LAST, "true")];
    let (cap, limit) = (rules().auto_thick_cap_fills, rules().auto_thick_warn_fills);
    let mut above = Vec::new();
    for (name, design) in designs() {
        let design = with(&design, &reordered);
        let on = in_place(&design).plan.unwrap();
        let off = in_place(&with(&design, &[("auto_thick_cap_fills", "50")])).plan.unwrap();
        let (thickest, columns) = fabric::thickest(&on, &fabric::columns(&design));
        let listed = FILLS_ALONE.iter().any(|(alone, at)| *alone == name && *at == hundredths(thickest));
        if (thickest - columns > cap && !listed) || thickest > limit + 1e-9 {
            above.push((name.clone(), hundredths(thickest), hundredths(columns)));
        }
        let (stats_on, stats_off) = (on.stats(), off.stats());
        assert_eq!((stats_on.jumps, stats_on.trims, stats_on.color_changes), (stats_off.jumps, stats_off.trims, stats_off.color_changes), "{name}");
        assert_eq!(jump_threads(&on).len(), jump_threads(&off).len(), "{name}");
        let (kept_on, kept_off) = (fabric::likeness(&design, &on).likeness(), fabric::likeness(&design, &off).likeness());
        assert!(kept_on >= kept_off, "{name}: {kept_on:.3}, {kept_off:.3} with the cap at 50");
    }
    assert!(above.is_empty(), "{above:?}");
}
