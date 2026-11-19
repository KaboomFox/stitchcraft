//! Where a traced letter's strokes meet (`REQ-AUTO-115`, `REQ-AUTO-116`,
//! `docs/src/design/image-to-embroidery.md` › Where a traced letter's strokes meet): letters drawn in code as
//! areas, as a trace of lettering draws them, each planned with its stitches where it draws its shapes, and
//! measured by how its stitches lie to the strokes a person draws it with ([`strokes`]) and how thick its
//! joins sew against its strokes.
//!
//! The letters are drawn as a typeface draws them, not as the word of TS-13B: a V whose legs narrow towards
//! their meeting, as most typefaces draw a V, and an L and a T in bars of even width. The digitizer knows no
//! letter, and the cases know only their strokes.

// Test code may unwrap and index (clippy.toml allows it in tests).
#![allow(clippy::unwrap_used, clippy::panic, clippy::indexing_slicing)]

use stitchcraft_core::{Budget, Point, math};
use stitchcraft_engine::design::{Design, DesignSettings, Element};
use stitchcraft_plan::thickness::{Measure, Thickness};
use stitchcraft_plan::{Rgb, Thread};
use stitchcraft_testkit::designs::{fill, in_place};
use stitchcraft_testkit::strokes::{self, Stroke};

/// The heights the letters are drawn at, in millimetres: from the smallest lettering a patch carries to a
/// large initial.
const HEIGHTS: [f64; 3] = [6.0, 12.0, 25.0];

/// A letter `id` drawn as one automatic area, its ring in heights from its top-left corner, `height` mm tall.
fn letter(id: &str, ring: &[(f64, f64)], height: f64) -> Design {
    let ring: Vec<(f64, f64)> = ring.iter().map(|&(u, v)| (u * height, v * height)).collect();
    let element = Element { automatic: true, ..fill(id, &[&ring], &Thread::new(Rgb::new(20, 20, 20)), &[]) };
    Design::new(vec![element], DesignSettings::default()).unwrap()
}

/// A V whose legs are 0.2 of its height wide across the top and narrow to 0.14 where they meet, as a
/// typeface draws one: its outer sides lean 19.3° from upright and its inner sides 17°. The legs meet in a
/// flat foot 0.14 wide.
const V: [(f64, f64); 7] = [(0.0, 0.0), (0.2, 0.0), (0.42, 0.72), (0.64, 0.0), (0.84, 0.0), (0.49, 1.0), (0.35, 1.0)];

/// The V's 2 strokes, each its middle line from the middle of its top to the middle of the foot, in heights.
fn v_strokes() -> Vec<Stroke> {
    vec![Stroke { ends: [(0.1, 0.0), (0.42, 1.0)], half: 0.085 }, Stroke { ends: [(0.74, 0.0), (0.42, 1.0)], half: 0.085 }]
}

/// An L in bars 0.2 of its height wide: a stem and a foot 0.6 wide.
const L: [(f64, f64); 6] = [(0.0, 0.0), (0.2, 0.0), (0.2, 0.8), (0.6, 0.8), (0.6, 1.0), (0.0, 1.0)];

/// A T in bars 0.2 of its height wide: a bar 0.7 wide and a stem under its middle.
const T: [(f64, f64); 8] = [(0.0, 0.0), (0.7, 0.0), (0.7, 0.2), (0.45, 0.2), (0.45, 1.0), (0.25, 1.0), (0.25, 0.2), (0.0, 0.2)];

#[test]
fn req_auto_115_a_column_doesnt_turn_through_the_corner_where_2_strokes_meet() {
    // A V's centre lines meet at its foot: one runs down a leg into the foot, and the other's column ends
    // under it there, so that the first turns through the foot and its stitches fan round the turn. On
    // main at M13.45 this V leans up to 37.9°, 36.0° and 36.1° at 6, 12 and 25 mm below its legs' tops (2,
    // 1 and 3 stitches past 30°), and Source Sans 3 Bold's V, whose column crowds more round its foot, is
    // filled at 12 mm (SC-I0902). Cut where its strokes meet, each leg is a column of its own, its stitches
    // across it, and the 2 meet along the line that halves the angle between them (REQ-AUTO-088). The tops
    // are cut on the slant, 18° from square to the legs, and a column's end follows such an edge: a
    // stitch there leans 30.2° at 25 mm on main as on the branch, which is no join's doing.
    for height in HEIGHTS {
        let design = letter("v", &V, height);
        let outcome = in_place(&design);
        let filled: Vec<_> = outcome.diagnostics.iter().filter(|d| d.code.to_string() == "SC-I0902").collect();
        assert!(filled.is_empty(), "{height} mm: {filled:?}");
        let plan = outcome.plan.unwrap();
        let placed = strokes::placed(&v_strokes(), (0.0, 0.0), height, 0.0);
        let leans = strokes::leans(&plan, "v", &placed, 0.4);
        assert!(leans.len() >= 20, "{height} mm: {} satin stitches", leans.len());
        let leaning: Vec<_> = leans.iter().filter(|(at, degrees)| at.y() > 0.2 * height && *degrees > 30.0).collect();
        assert!(leaning.is_empty(), "{height} mm: {} of {} lean more than 30°: {leaning:?}", leaning.len(), leans.len());
    }
}

/// A letter of the case of `REQ-AUTO-116`: its id, its ring, its join and 2 stretches of its strokes' middle
/// lines a width and more away from the join, in heights.
type Joined = (&'static str, &'static [(f64, f64)], (f64, f64), [((f64, f64), (f64, f64)); 2]);

/// The thickest spot of `thickness` within `reach` mm of `at`, sampled every 0.25 mm.
fn thickest_near(thickness: &Thickness, at: (f64, f64), reach: f64) -> f64 {
    let mut most: f64 = 0.0;
    let mut y = at.1 - reach;
    while y <= at.1 + reach {
        let mut x = at.0 - reach;
        while x <= at.0 + reach {
            if math::hypot(x - at.0, y - at.1) <= reach {
                most = most.max(thickness.at(Measure::Fills, Point::new(x, y).unwrap()));
            }
            x += 0.25;
        }
        y += 0.25;
    }
    most
}

/// The thickest spot of `thickness` along the middle line from `a` to `b`, sampled every 0.25 mm.
fn thickest_along(thickness: &Thickness, a: (f64, f64), b: (f64, f64)) -> f64 {
    let length = math::hypot(b.0 - a.0, b.1 - a.1);
    let (mut most, mut along): (f64, f64) = (0.0, 0.0);
    while along <= length {
        let t = along / length;
        most = most.max(thickness.at(Measure::Fills, Point::new(a.0 + (b.0 - a.0) * t, a.1 + (b.1 - a.1) * t).unwrap()));
        along += 0.25;
    }
    most
}

#[test]
fn req_auto_116_a_join_sews_no_more_than_one_layer_of_satin_thicker_than_its_strokes() {
    // Where one stroke stops against another, the 2 columns overlap, and a spot there has both: one layer of
    // satin, 1.25 fills, more than either stroke. The seam along the side of the stroke that runs through
    // sews more today: on main at M13.45 this L 12 mm tall measures 3.13 fills at its join where its strokes
    // measure 1.42, the end of the foot's column and the stem's pull compensation lying on each other there.
    const ONE_LAYER: f64 = 1.25;
    // Each letter's join, in heights, and the stretches of its strokes' middle lines a width and more away
    // from it.
    let letters: [Joined; 2] = [
        ("l", &L, (0.1, 0.9), [((0.1, 0.1), (0.1, 0.6)), ((0.4, 0.9), (0.5, 0.9))]),
        ("t", &T, (0.35, 0.1), [((0.05, 0.1), (0.15, 0.1)), ((0.35, 0.4), (0.35, 0.9))]),
    ];
    for height in [12.0, 25.0] {
        for (id, ring, join, stretches) in letters {
            let plan = in_place(&letter(id, ring, height)).plan.unwrap();
            let thickness = Thickness::of(&plan, &mut Budget::DEFAULT.meter()).unwrap().unwrap();
            let mm = |(u, v): (f64, f64)| (u * height, v * height);
            let own = stretches.iter().map(|&(a, b)| thickest_along(&thickness, mm(a), mm(b))).fold(0.0, f64::max);
            let at_join = thickest_near(&thickness, mm(join), 0.2 * height);
            assert!(at_join <= own + ONE_LAYER, "{id} {height} mm: {at_join:.2} fills at the join, {own:.2} along its strokes");
        }
    }
}
