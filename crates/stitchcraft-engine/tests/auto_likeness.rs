//! How much of a picture's detail automatic digitizing keeps (`REQ-AUTO-093`, `REQ-AUTO-094`), measured as
//! `stitchcraft_testkit::fabric::likeness` measures any design: each colour's parts of the trace that the
//! stitches show in that colour. A speck merged into its neighbour is a part lost. The test pictures, the
//! traced pear, the lettered patch and the test sheets' digitized halves keep their likeness as recorded, and
//! the traced dogs are `tests/auto_photos.rs`'s, in the release build.

// Test code may unwrap and index (clippy.toml allows it in tests).
#![allow(clippy::unwrap_used, clippy::panic, clippy::indexing_slicing)]

use stitchcraft_engine::auto;
use stitchcraft_engine::design::{Design, DesignSettings, Element};
use stitchcraft_engine::testsheets::SHEETS;
use stitchcraft_testkit::designs::{BLUE, GREEN, RED, automatic, fill, in_place};
use stitchcraft_testkit::fabric::likeness::{self, LIKENESS};
use stitchcraft_testkit::pictures;

/// A red field 20 mm wide with a blue speck `side` millimetres wide in its middle, and a green square 5 mm
/// wide beside it, so that the field is no background (REQ-AUTO-009): all automatic, each with the
/// parameters `params`.
fn speck(side: f64, params: &[(&str, &str)]) -> Design {
    let square = |id: &str, (x, y): (f64, f64), side: f64, thread| {
        automatic(fill(id, &[&[(x, y), (x + side, y), (x + side, y + side), (x, y + side)]], thread, params))
    };
    let middle = 10.0 - side / 2.0;
    let shapes =
        vec![square("beside", (25.0, 0.0), 5.0, &GREEN), square("field", (0.0, 0.0), 20.0, &RED), square("speck", (middle, middle), side, &BLUE)];
    Design::new(shapes, DesignSettings::default()).unwrap()
}

/// What the stitches of `design`, digitized automatically and planned in place, keep of it.
fn kept(design: &Design) -> likeness::Likeness {
    let outcome = in_place(design);
    likeness::likeness(design, &outcome.plan.unwrap_or_else(|| panic!("{:?}", outcome.diagnostics)))
}

#[test]
fn req_auto_093_a_speck_merged_into_its_neighbour_is_a_part_lost() {
    // 0.9 mm wide, 0.81 mm², the speck merges into the field (REQ-AUTO-008): its colour has 1 part, lost.
    let merged = kept(&speck(0.9, &[]));
    assert_eq!(merged.likeness(), 2.0 / 3.0, "{merged:?}");
    let lost: Vec<_> = merged.lost().map(|part| part.colour).collect();
    assert_eq!(lost, [BLUE.color]);
    // With a smallest area of 0.5 mm² it is no speck, sews in its own thread and is kept.
    let sewn = kept(&speck(0.9, &[(auto::MIN_AREA, "0.5")]));
    assert_eq!((sewn.likeness(), sewn.lost().count()), (1.0, 0), "{sewn:?}");
    // 1.1 mm wide it is no speck by default.
    assert_eq!(kept(&speck(1.1, &[])).likeness(), 1.0);
}

/// The digitized half of each test sheet that has one, by the name the before and after sheet gives it: its
/// automatic shapes alone, in their paint order.
fn digitized_halves() -> Vec<(String, Design)> {
    let halves = SHEETS.iter().filter(|sheet| sheet.digitized()).map(|sheet| {
        let design = sheet.design().unwrap().unwrap();
        let shapes: Vec<Element> = design.elements().iter().filter(|element| element.automatic).cloned().collect();
        (format!("{}-digitized", sheet.id.to_lowercase()), Design::new(shapes, design.settings).unwrap())
    });
    halves.collect()
}

/// What each picture keeps, as measured on 2026-11-17: its parts, the parts kept and its likeness in
/// thousandths. TS-13A's digitized half loses 7 of its red parts: the specks drawn round its disc, which share
/// no edge and are left out by design (REQ-AUTO-008). The case fails when a number changes, so that a change
/// that takes detail away, or puts it back, shows here.
const KEPT: [(&str, usize, usize, f64); 7] = [
    ("pear", 5, 5, 1000.0),
    ("lettered-patch", 8, 8, 1000.0),
    ("ts-13a-digitized", 23, 16, 767.0),
    ("ts-13b-digitized", 31, 31, 1000.0),
    ("ts-13c-digitized", 11, 11, 1000.0),
    ("ts-13d-digitized", 14, 14, 1000.0),
    ("ts-13e-digitized", 18, 18, 1000.0),
];

#[test]
fn req_auto_094_the_pear_the_patch_and_the_test_sheets_keep_their_likeness_as_recorded() {
    let mut pictures = vec![("pear".to_string(), pictures::pear()), ("lettered-patch".to_string(), pictures::lettered_patch())];
    pictures.extend(digitized_halves());
    let found: Vec<(String, usize, usize, f64)> = pictures
        .iter()
        .map(|(name, design)| {
            let kept = kept(design);
            assert!(kept.likeness() >= LIKENESS, "{name}: {kept:?}");
            (name.clone(), kept.parts.len(), kept.parts.iter().filter(|part| part.kept()).count(), (kept.likeness() * 1000.0).round())
        })
        .collect();
    assert_eq!(found, KEPT.map(|(name, parts, kept, likeness)| (name.to_string(), parts, kept, likeness)));
}
