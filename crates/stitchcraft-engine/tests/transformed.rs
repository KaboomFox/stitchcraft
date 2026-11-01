//! Designs sew the same mirrored, turned half round, or drawn again the other way round, from another point
//! or with points added (`stitchcraft_testkit::transformed`): every element of every test sheet drawn as a
//! design, with settings or digitized automatically, and random letters digitized automatically. A rule that
//! depends on which way a shape happens to be drawn, or on a tie broken by its coordinates, sews a shape and
//! its mirror image differently. TS-13B's letter A 6 mm tall did: one leg sewed as a satin column and the
//! other as a line, and the other way round mirrored (`req_auto_016_a_letters_legs_sew_alike`).

// Test code may unwrap and index (clippy.toml allows it in tests).
#![allow(clippy::unwrap_used, clippy::panic, clippy::indexing_slicing)]

use proptest::prelude::*;
use stitchcraft_engine::design::{Design, DesignSettings};
use stitchcraft_engine::testsheets::SHEETS;
use stitchcraft_testkit::designs::{RED, automatic, fill, letter};
use stitchcraft_testkit::strategies::letters;
use stitchcraft_testkit::transformed::{Change, Compared, compare};

/// The share of an element's thread that must lie where the change takes it, within the harness's
/// tolerance. On the test sheets each element keeps 0.993 or more of it; the letter A whose leg became a
/// line kept 0.85.
const SAME: f64 = 0.98;

/// The elements of `design` that sew differently once it is changed, each with the change.
fn differing(design: &Design, changes: &[Change]) -> Vec<(Change, Compared)> {
    compare(design, changes)
        .into_iter()
        .flat_map(|(change, compared)| compared.into_iter().filter(|c| c.shared < SAME).map(move |c| (change, c)))
        .collect()
}

#[test]
fn every_test_sheet_sews_the_same_mirrored_turned_or_drawn_again() {
    let mut found = Vec::new();
    for sheet in SHEETS {
        let Some(design) = sheet.design() else { continue };
        found.extend(differing(&design.unwrap(), &Change::ALL).into_iter().map(|(change, c)| format!("{} {change:?}: {c:?}", sheet.id)));
    }
    assert!(found.is_empty(), "{found:#?}");
}

proptest! {
    #![proptest_config(stitchcraft_testkit::strategies::config(64))]

    #[test]
    fn random_letters_digitized_automatically_sew_the_same_mirrored_turned_or_drawn_again((name, height, width) in letters()) {
        let shape = automatic(fill("letter", &[&letter(name, height, width)], &RED, &[]));
        let design = Design::new(vec![shape], DesignSettings::default()).unwrap();
        let found = differing(&design, &Change::ALL);
        prop_assert!(found.is_empty(), "{name} {height} mm tall, {width} mm wide: {found:?}");
    }
}
