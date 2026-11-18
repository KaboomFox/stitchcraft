//! A letter traced as an area keeps its counter open and its edge whole (`REQ-AUTO-084`, `REQ-AUTO-085`,
//! `docs/src/design/image-to-embroidery.md` › Holes and edges of columns), and sews as strokes, as a font's
//! letters do (`REQ-AUTO-087`, `REQ-AUTO-088`, › Letters sewn as strokes): the lettered patch and TS-13B's
//! words, each planned with its stitches where it draws its shapes, and measured by what of each letter's
//! own shape shows ([`fabric::shown`]), how its stitches lie to the strokes a person draws it with
//! ([`strokes`]), and how thick it sews.

// Test code may unwrap and index (clippy.toml allows it in tests).
#![allow(clippy::unwrap_used, clippy::panic, clippy::indexing_slicing)]

use stitchcraft_core::{Budget, Point, Rect};
use stitchcraft_engine::design::{Design, DesignSettings};
use stitchcraft_engine::testsheets::pictures::word;
use stitchcraft_plan::thickness::{Measure, Thickness};
use stitchcraft_testkit::designs::in_place;
use stitchcraft_testkit::fabric::{self, Shown};
use stitchcraft_testkit::{pictures, strokes};

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
    vec![
        ("lettered patch", pictures::lettered_patch()),
        ("15 mm", traced(15.0)),
        ("10 mm", traced(10.0)),
        ("6 mm", traced(6.0)),
        ("4 mm", traced(4.0)),
    ]
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

/// How tall each design's letters are, in millimetres.
fn height(name: &str) -> f64 {
    match name {
        "lettered patch" | "10 mm" => 10.0,
        other => other.trim_end_matches(" mm").parse().unwrap(),
    }
}

/// The M and the A, each its id, its strokes placed in `design`'s letters `height` mm tall, and its shape's
/// box.
fn m_and_a(design: &Design, height: f64) -> Vec<(&'static str, Vec<strokes::Stroke>, Rect)> {
    let placed = |strokes: Vec<strokes::Stroke>, left| strokes::placed(&strokes, (0.0, 0.0), height, left);
    let bounds = |id: &str| {
        let element = design.elements().iter().find(|element| element.id.to_string() == id).unwrap();
        Rect::around(element.shape.path().points()).unwrap()
    };
    vec![("m", placed(strokes::m(), 0.0), bounds("m")), ("a", placed(strokes::a(), strokes::A_LEFT), bounds("a"))]
}

#[test]
fn req_auto_087_a_traced_letters_satin_stitches_lie_square_to_its_strokes() {
    // Every satin stitch of the M and the A at least a thread wide lies within 30° of square to the stroke
    // it sews. On main at M13.36 the M's middle strokes leaned up to 56° where they meet its stems (16 to 29
    // stitches past 30°), as its columns ran on round the corners, and the 15 mm A's up to 32°. The 4 mm
    // word runs along its letters' middles.
    for (name, design) in designs().into_iter().filter(|(name, _)| *name != "4 mm") {
        let plan = in_place(&design).plan.unwrap();
        for (id, placed, _) in m_and_a(&design, height(name)) {
            let leans = strokes::leans(&plan, id, &placed, 0.4);
            assert!(leans.len() >= 50, "{name} {id}: {} satin stitches", leans.len());
            let leaning: Vec<_> = leans.iter().filter(|(_, degrees)| *degrees > 30.0).collect();
            assert!(leaning.is_empty(), "{name} {id}: {} of {} lean more than 30°: {leaning:?}", leaning.len(), leans.len());
        }
    }
}

#[test]
fn req_auto_088_a_traced_letter_sews_no_thicker_than_a_fonts() {
    // Small Font's M and A, set at the same height and planned as these are, measure at their thickest
    // (the probe of M13.37 on 2026-11-17): the 15, 10 and 6 mm words' M 3.58, 3.71 and 3.29 fills and A
    // 3.40, 3.31 and 3.44, and on MC-8's lettered patch (item 15, read from its machine file, the field
    // under the letters) the M 4.16 and the A 4.22. A traced letter is drawn in strokes 0.18 of its height
    // wide, the font's in strokes 0.13 wide, and from 2 mm a column's underlay is a contour, a run along
    // each rail, where the font's is a centre walk. So a traced letter may lay one run of thread more
    // through a spot, a run across a square millimetre being 0.25 fills.
    const MARGIN: f64 = 0.25;
    let font = |name: &str, id: &str| match (name, id) {
        ("lettered patch", "m") => 4.16,
        ("lettered patch", _) => 4.22,
        ("15 mm", "m") => 3.58,
        ("15 mm", _) => 3.40,
        ("10 mm", "m") => 3.71,
        ("10 mm", _) => 3.31,
        ("6 mm", "m") => 3.29,
        _ => 3.44,
    };
    for (name, design) in designs().into_iter().filter(|(name, _)| *name != "4 mm") {
        let plan = in_place(&design).plan.unwrap();
        let thickness = Thickness::of(&plan, &mut Budget::DEFAULT.meter()).unwrap().unwrap();
        for (id, _, bounds) in m_and_a(&design, height(name)) {
            let mut thickest: f64 = 0.0;
            let (mut x, mut y) = (bounds.min().x(), bounds.min().y());
            while y <= bounds.max().y() {
                while x <= bounds.max().x() {
                    thickest = thickest.max(thickness.at(Measure::Fills, Point::new(x, y).unwrap()));
                    x += 0.25;
                }
                (x, y) = (bounds.min().x(), y + 0.25);
            }
            assert!(thickest <= font(name, id) + MARGIN, "{name} {id}: {thickest:.2} fills, the font's {}", font(name, id));
        }
    }
}
