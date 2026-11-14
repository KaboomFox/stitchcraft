//! What automatic digitizing leaves of the owner's dog (`REQ-AUTO-025` to `REQ-AUTO-027`): her 2 photos, each
//! cut out by a mask checked by hand and traced by VectorCraft in 6 and in 9 threads (`docs/fixtures/README.md`
//! › The traced dogs), digitized, planned with their stitches where they draw their shapes, and measured as
//! `tests/auto_fabric.rs` measures the test pictures.
//!
//! **Release only.** A traced dog is about a hundred shapes. In the tests' debug build each takes some 15 s to
//! digitize and 30 s to measure, so that build leaves these cases out, and CI's `perf` job runs them in the
//! release build, `cargo test --release --locked -p stitchcraft-engine --test auto_photos`
//! (`docs/src/design/conformance.md` › Cases in the release build). Each dog is planned once for all its cases.

// Test code may unwrap and index (clippy.toml allows it in tests).
#![allow(clippy::unwrap_used, clippy::panic, clippy::indexing_slicing)]

use std::sync::OnceLock;

use stitchcraft_engine::design::Design;
use stitchcraft_plan::StitchPlan;
use stitchcraft_testkit::designs::{in_place, rules};
use stitchcraft_testkit::fabric::{self, GAP};
use stitchcraft_testkit::pictures;

/// Each traced dog by name, digitized automatically and planned in place, once for all the cases.
fn planned() -> &'static [(String, Design, StitchPlan)] {
    static PLANNED: OnceLock<Vec<(String, Design, StitchPlan)>> = OnceLock::new();
    PLANNED.get_or_init(|| {
        let plan = |(name, design): (String, Design)| {
            let outcome = in_place(&design);
            let plan = outcome.plan.unwrap_or_else(|| panic!("{name}: {:?}", outcome.diagnostics));
            (name, design, plan)
        };
        pictures::dogs().into_iter().map(plan).collect()
    })
}

#[test]
#[cfg_attr(debug_assertions, ignore = "release only: CI's perf job runs tests/auto_photos.rs")]
fn req_auto_025_digitized_dogs_leave_no_patch_of_fabric_bare() {
    let smallest = rules().auto_min_area_mm2;
    let bare: Vec<String> = planned()
        .iter()
        .filter_map(|(name, design, plan)| {
            let unsewn = fabric::unsewn(design, plan);
            (unsewn.largest >= smallest).then(|| format!("{name}: {:.3} mm² at {:?}, {} patches", unsewn.largest, unsewn.at, unsewn.patches))
        })
        .collect();
    assert!(bare.is_empty(), "{bare:#?}");
}

/// A stitch of a traced dog that shows where it should not, until the roadmap step that removes it, in the
/// deviations ledger's way: what it is, why, and what removes it.
struct Astray {
    /// The dog, by the name `stitchcraft_testkit::pictures::dogs` gives it.
    dog: &'static str,
    /// The element whose stitch it is.
    element: &'static str,
    /// Where the stitch starts and ends, in millimetres, to a hundredth.
    stitch: [(f64, f64); 2],
    /// How long the stitch is, in millimetres, to a hundredth.
    length: f64,
    /// Why it lies there.
    cause: &'static str,
    /// The roadmap step that removes it (`docs/src/plan/roadmap.md`).
    step: &'static str,
}

/// Why a stitch of a column that sews past a rail shows.
const RAIL: &str = "A satin column sews 1 mm past a rail, under the shape sewn after it that runs along more than half of that rail (REQ-AUTO-029). Where that shape ends the column's stitches still reach 1 mm past the rail, and this one shows on the colour beside it.";

/// Every stitch of the dogs astray today. The case fails when another stitch is astray, and when one of these
/// no longer is, so the list shrinks as each is fixed: M13.17 and M13.18 empty it. M13.16's overlap of 1 mm
/// added the 3 stitches of columns that sew past a rail, and its base layer the one that shows in a gap.
const ASTRAY: [Astray; 7] = [
    Astray {
        dog: "dog-on-turf-6",
        element: "stitchcraft:base",
        stitch: [(40.5, 44.17), (42.0, 41.57)],
        length: 3.0,
        cause: "A stitch of the base layer shows in the gap between 2 lines of fur sewn as running stitches, at (41.1, 43.3). The base layer is there to lie under such a gap, and the measure counts an underlay stitch wherever it shows.",
        step: "M13.18",
    },
    Astray { dog: "dog-on-turf-6", element: "vc:67:fill#3", stitch: [(49.59, 64.05), (49.02, 63.11)], length: 1.1, cause: RAIL, step: "M13.18" },
    Astray {
        dog: "dog-on-turf-6",
        element: "vc:9:fill#18",
        stitch: [(50.51, 64.73), (56.47, 69.9)],
        length: 7.89,
        cause: "A satin column of dark grey fur fans its stitches round a black shape that bites into its side, and this one crosses the black shape's corner.",
        step: "M13.18",
    },
    Astray { dog: "dog-on-turf-9", element: "vc:74:fill#9", stitch: [(39.02, 63.83), (39.49, 61.25)], length: 2.63, cause: RAIL, step: "M13.18" },
    Astray {
        dog: "dog-on-turf-9",
        element: "vc:83:fill",
        stitch: [(50.08, 23.05), (48.93, 21.12)],
        length: 2.25,
        cause: "A narrow part sews 2 lines as one running stitch. The needle goes straight from the end of one to the start of the next, across another colour, and the move, shorter than the 3 mm collapse length, is a stitch.",
        step: "M13.17",
    },
    Astray { dog: "dog-running-9", element: "vc:40:fill#3", stitch: [(60.34, 51.61), (59.54, 50.81)], length: 1.14, cause: RAIL, step: "M13.18" },
    Astray {
        dog: "dog-running-9",
        element: "vc:27:fill",
        stitch: [(33.11, 14.46), (37.32, 12.31)],
        length: 4.72,
        cause: "A satin column of dark grey fur fans its stitches round a lighter grey shape that bites into its side, and this one crosses that shape.",
        step: "M13.18",
    },
];

/// `value` to a hundredth, as a whole number of hundredths.
fn hundredths(value: f64) -> f64 {
    (value * 100.0).round()
}

#[test]
#[cfg_attr(debug_assertions, ignore = "release only: CI's perf job runs tests/auto_photos.rs")]
fn req_auto_026_digitized_dogs_lay_no_thread_that_shows_where_it_should_not_but_as_listed() {
    let (mut found, mut apart) = (Vec::new(), Vec::new());
    for (name, design, plan) in planned() {
        let astray = fabric::astray(design, plan);
        for [from, to] in &astray.stitches {
            let sewn = plan.sewn_stitches().into_iter().find(|stitch| stitch.from == *from && stitch.to == *to);
            let element = sewn
                .and_then(|stitch| plan.blocks.get(stitch.block)?.stitches.get(stitch.index)?.origin.element)
                .and_then(|element| plan.element(element))
                .map(ToString::to_string)
                .unwrap_or_default();
            let at = |p: stitchcraft_core::Point| (hundredths(p.x()), hundredths(p.y()));
            found.push((name.clone(), element, [at(*from), at(*to)], hundredths(from.distance(*to))));
        }
        apart.push((name.clone(), astray.moves.len(), astray.jumps.len()));
    }
    let listed: Vec<_> = ASTRAY
        .iter()
        .map(|a| {
            let at = |(x, y): (f64, f64)| (hundredths(x), hundredths(y));
            (a.dog.to_string(), a.element.to_string(), a.stitch.map(at), hundredths(a.length))
        })
        .collect();
    assert!(ASTRAY.iter().all(|a| !a.cause.is_empty() && ["M13.17", "M13.18"].contains(&a.step)));
    assert_eq!(found, listed, "the stitches astray, as found and as listed in ASTRAY");
    // The moves sewn on from one element to the next, and the jump threads the PE800 leaves, are as the design
    // page records them (image-to-embroidery.md › Sewing order).
    let recorded = [("dog-on-turf-6", 6, 113), ("dog-on-turf-9", 3, 130), ("dog-running-6", 3, 100), ("dog-running-9", 10, 116)];
    assert_eq!(apart, recorded.map(|(name, moves, jumps)| (name.to_string(), moves, jumps)));
}

/// What the top thread of each traced dog leaves to the base layer, as measured: how many slivers, how many
/// of them lie at the rim, where the base layer's offset leaves no base layer along the design's outline, the
/// largest in hundredths of a square millimetre, and the share of the design left bare in thousandths of a
/// percent. The case fails when a number changes, so that a dog whose slivers grow shows here before it
/// reaches a limit, and one that loses a sliver gets its row changed. Most slivers lie beside a line of fur
/// that sews as a running stitch, which is as wide as its thread and can't reach under a neighbour.
const SLIVERS: [(&str, usize, usize, f64, f64); 4] = [
    ("dog-on-turf-6", 4, 2, 20.0, 204.0),
    ("dog-on-turf-9", 10, 1, 34.0, 346.0),
    ("dog-running-6", 3, 0, 26.0, 107.0),
    ("dog-running-9", 2, 0, 14.0, 73.0),
];

#[test]
#[cfg_attr(debug_assertions, ignore = "release only: CI's perf job runs tests/auto_photos.rs")]
fn req_auto_027_digitized_dogs_show_no_fabric_and_leave_the_slivers_recorded() {
    let rules = rules();
    let (mut faults, mut found) = (Vec::new(), Vec::new());
    for (name, design, plan) in planned() {
        let cover = fabric::cover(design, plan, GAP);
        // No fabric shows: each gap is a sliver, and all that is bare is within the limit of a design with a
        // base layer.
        assert!(cover.base > 0.0, "{name} has no base layer");
        faults.extend(cover.faults(&rules).into_iter().map(|fault| format!("{name}: {fault}")));
        let slivers = cover.slivers(&rules);
        let largest = slivers.first().copied().unwrap_or(0.0);
        found.push((name.clone(), slivers.len(), cover.at_the_rim(&rules), hundredths(largest), (cover.bare_share() * 100_000.0).round()));
        println!(
            "{name}: slivers {slivers:?}, beyond {:?}, bare {:.2} of {:.0} mm², open {:.2} mm²",
            cover.beyond, cover.bare, cover.design, cover.open
        );
    }
    assert!(faults.is_empty(), "{faults:#?}");
    assert_eq!(found, SLIVERS.map(|(name, slivers, rim, largest, share)| (name.to_string(), slivers, rim, largest, share)));
}
