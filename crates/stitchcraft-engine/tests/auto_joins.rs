//! Where a traced letter's strokes meet (`REQ-AUTO-115`,
//! `docs/src/design/image-to-embroidery.md` › Where a traced letter's strokes meet): letters drawn in code as
//! areas, as a trace of lettering draws them, each planned with its stitches where it draws its shapes, and
//! measured by how its stitches lie to the strokes a person draws it with ([`strokes`]).
//!
//! The letter is drawn as a typeface draws it, not as the word of TS-13B: a V whose legs narrow towards
//! their meeting, as most typefaces draw a V. The digitizer knows no letter, and the case knows only its
//! strokes.

// Test code may unwrap and index (clippy.toml allows it in tests).
#![allow(clippy::unwrap_used, clippy::panic, clippy::indexing_slicing)]

use stitchcraft_engine::design::{Design, DesignSettings, Element};
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
