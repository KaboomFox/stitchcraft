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

/// That share for an open layer of rows: the base layer of a design digitized automatically, one layer of
/// rows 2.6 mm apart since M13.30, and the layer under each square and block of TS-13F, which is sewn as
/// it. Its rows lie where the change takes them. Between 2 rows the needle goes along the layer's edge, and
/// that is a third of its thread. Which edge it goes along depends on the corner the layer starts from,
/// which is the one nearest the element sewn before it. Measured on 2026-11-16: a base layer keeps 0.933
/// or more, and a layer of TS-13F, 6 rows in a square of 15 mm, 0.728 or more. With 3 layers, as before
/// M13.30, every edge had thread along it whichever corner a layer started from.
const OPEN: f64 = 0.7;

/// That share for a patch of TS-13F's bottom row, a fill 1.5 to 4 mm across of 6 to 16 rows: the stitches
/// along its edge from one row to the next are a fifth of its thread, and lie on the side the needle
/// comes from. Measured on 2026-11-16: 0.820 or more.
const PATCH: f64 = 0.8;

/// The share that the element `id` must keep.
fn same(id: &str) -> f64 {
    if id.starts_with("stitchcraft:base") || (id.starts_with("ts13f:") && id.ends_with("-base")) {
        OPEN
    } else if id.starts_with("ts13f:patches-") {
        PATCH
    } else {
        SAME
    }
}

/// The elements of `design` that sew differently once it is changed, each with the change.
fn differing(design: &Design, changes: &[Change]) -> Vec<(Change, Compared)> {
    compare(design, changes)
        .into_iter()
        .flat_map(|(change, compared)| compared.into_iter().filter(|c| c.shared < same(&c.element)).map(move |c| (change, c)))
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
    assert_eq!(
        [same("stitchcraft:base"), same("stitchcraft:base-2"), same("ts13f:bare-1-base"), same("ts13f:patches-3-0-0")],
        [OPEN, OPEN, OPEN, PATCH]
    );
    assert_eq!([same("ts13f:bare-1-fill-0"), same("ts08:0.25-0-base"), same("vc:1:fill")], [SAME; 3]);
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
