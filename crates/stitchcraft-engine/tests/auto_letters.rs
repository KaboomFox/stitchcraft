//! A letter traced as an area keeps its counter open and its edge whole (`REQ-AUTO-084`, `REQ-AUTO-085`,
//! `docs/src/design/image-to-embroidery.md` › Holes and edges of columns): the lettered patch and TS-13B's
//! words, each planned with its stitches where it draws its shapes, and measured by what of each letter's
//! own shape shows ([`fabric::shown`]).

// Test code may unwrap and index (clippy.toml allows it in tests).
#![allow(clippy::unwrap_used, clippy::panic, clippy::indexing_slicing)]

use stitchcraft_engine::design::{Design, DesignSettings};
use stitchcraft_engine::testsheets::pictures::word;
use stitchcraft_testkit::designs::in_place;
use stitchcraft_testkit::fabric::{self, Shown};
use stitchcraft_testkit::pictures;

/// The word MATCH in letters `height` millimetres tall, as TS-13B traces it.
fn traced(height: f64) -> Design {
    Design::new(word((0.0, 0.0), height).unwrap().elements, DesignSettings::default()).unwrap()
}

/// What shows of each letter of `design` planned in place: every area but the patch's field and border.
fn letters(design: &Design) -> Vec<Shown> {
    let plan = in_place(design).plan.unwrap();
    fabric::shown(design, &plan).into_iter().filter(|shown| !shown.id.starts_with("patch")).collect()
}

/// The designs of the cases by name: the lettered patch and TS-13B's words 15, 10, 6 and 4 mm tall.
fn designs() -> Vec<(&'static str, Design)> {
    vec![("lettered patch", pictures::lettered_patch()), ("15 mm", traced(15.0)), ("10 mm", traced(10.0)), ("6 mm", traced(6.0)), ("4 mm", traced(4.0))]
}

#[test]
fn req_auto_084_the_counter_of_a_traced_a_shows_what_lies_under_it() {
    // A counter 1.4 mm across inside, as the 10 mm A's, keeps (1 - 0.4 / 1.4)² of itself where half a
    // thread spreads in from each edge: half. The 6 mm A's, 0.84 mm across, keeps a quarter so. On main at
    // M13.35 the 3 showed 7 %, 25 % and none of the field.
    for (name, design) in designs() {
        let least = match name {
            "6 mm" => 0.25,
            "4 mm" => continue,
            _ => 0.5,
        };
        let shown = letters(&design);
        let a = shown.iter().find(|shown| shown.id == "a").unwrap();
        assert!(a.open_share() >= least, "{name}: {:.0} % of the counter shows, {a:?}", 100.0 * a.open_share());
    }
}

#[test]
fn req_auto_085_a_traced_letter_shows_no_notch_and_no_gap_inside_its_outline() {
    // No letter sewn as columns leaves a strip of another colour as wide as a thread inside its outline: the
    // 4 mm word runs along its letters' middles (REQ-AUTO-016), and beside a running stitch the fabric shows
    // as it did on main. On the patch and the
    // 10 and 6 mm words none leaves a notch either, a pit a disc 0.15 mm across fits in: the M's stems left 6
    // on main at M13.35. The 15 mm word's M and C keep the notches they had there, where their points fan.
    for (name, design) in designs() {
        for letter in letters(&design) {
            assert!(name == "4 mm" || letter.gaps.is_empty(), "{name}: {letter:?}");
            if ["lettered patch", "10 mm", "6 mm"].contains(&name) {
                assert!(letter.notches.is_empty(), "{name}: {letter:?}");
            }
        }
    }
}
