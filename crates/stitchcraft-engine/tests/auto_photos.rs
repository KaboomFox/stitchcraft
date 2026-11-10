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

use stitchcraft_core::Budget;
use stitchcraft_engine::auto;
use stitchcraft_engine::design::{Design, Element, Shape};
use stitchcraft_plan::StitchPlan;
use stitchcraft_testkit::designs::{in_place, rules};
use stitchcraft_testkit::fabric::{self, BARE, GAP};
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

/// A gap a traced dog still leaves, until the roadmap step that closes it, as [`Astray`] lists a stitch.
/// Only a gap beside a thin strand may be listed: a line of fur that sews as a running stitch, or as a satin
/// column narrower than 1 mm. The overlap of `REQ-AUTO-029` can't close it: a running stitch is as wide as
/// its thread, and such a column hardly wider.
struct Gap {
    /// The dog, by the name `stitchcraft_testkit::pictures::dogs` gives it.
    dog: &'static str,
    /// The 2 elements nearest the gap's middle, the nearer first, the base layer aside.
    beside: [&'static str; 2],
    /// The middle of the box round the gap, in millimetres, to a tenth.
    at: (f64, f64),
    /// The roadmap step that closes it (`docs/src/plan/roadmap.md`).
    step: &'static str,
}

/// Every gap of the dogs that M13.18 is left to close, by merging thin lines into their neighbours: what
/// M13.16's base layer and overlap leave of the 44 gaps the dogs had before it. 17 have a running stitch
/// among their 2 nearest elements. The first of the dog on turf in 6 threads and the last of the running dog
/// in 6 lie between 2 satin columns under 1 mm wide.
const GAPS: [Gap; 19] = [
    Gap { dog: "dog-on-turf-6", beside: ["vc:83:fill#5", "vc:83:fill#3"], at: (47.5, 14.2), step: "M13.18" },
    Gap { dog: "dog-on-turf-6", beside: ["vc:84:fill#2", "vc:62:fill#3"], at: (48.8, 24.7), step: "M13.18" },
    Gap { dog: "dog-on-turf-6", beside: ["vc:88:fill", "vc:8:fill"], at: (41.1, 43.3), step: "M13.18" },
    Gap { dog: "dog-on-turf-6", beside: ["vc:68:fill#3", "vc:101:fill"], at: (52.5, 73.2), step: "M13.18" },
    Gap { dog: "dog-on-turf-9", beside: ["vc:70:fill", "vc:82:fill"], at: (45.1, 19.5), step: "M13.18" },
    Gap { dog: "dog-on-turf-9", beside: ["vc:83:fill", "vc:27:fill#4"], at: (48.3, 20.8), step: "M13.18" },
    Gap { dog: "dog-on-turf-9", beside: ["vc:101:fill", "vc:83:fill"], at: (47.1, 21.4), step: "M13.18" },
    Gap { dog: "dog-on-turf-9", beside: ["vc:83:fill", "vc:27:fill#4"], at: (48.4, 21.7), step: "M13.18" },
    Gap { dog: "dog-on-turf-9", beside: ["vc:29:fill", "vc:83:fill"], at: (50.4, 23.1), step: "M13.18" },
    Gap { dog: "dog-on-turf-9", beside: ["vc:31:fill#7", "vc:6:fill#2"], at: (61.5, 41.1), step: "M13.18" },
    Gap { dog: "dog-on-turf-9", beside: ["vc:117:fill#2", "vc:31:fill"], at: (62.3, 41.6), step: "M13.18" },
    Gap { dog: "dog-on-turf-9", beside: ["vc:87:fill#2", "vc:35:fill"], at: (54.9, 43.9), step: "M13.18" },
    Gap { dog: "dog-on-turf-9", beside: ["vc:40:fill", "vc:6:fill#4"], at: (61.0, 46.1), step: "M13.18" },
    Gap { dog: "dog-on-turf-9", beside: ["vc:92:fill#2", "vc:46:fill"], at: (52.6, 63.3), step: "M13.18" },
    Gap { dog: "dog-running-6", beside: ["vc:4:fill", "vc:26:fill#5"], at: (38.7, 23.9), step: "M13.18" },
    Gap { dog: "dog-running-6", beside: ["vc:71:fill#2", "vc:37:fill#3"], at: (31.0, 51.6), step: "M13.18" },
    Gap { dog: "dog-running-6", beside: ["vc:46:fill", "vc:78:fill"], at: (56.9, 71.9), step: "M13.18" },
    Gap { dog: "dog-running-9", beside: ["vc:43:fill#3", "vc:74:fill#3"], at: (49.1, 58.4), step: "M13.18" },
    Gap { dog: "dog-running-9", beside: ["vc:80:fill", "vc:77:fill"], at: (39.9, 69.7), step: "M13.18" },
];

/// The dogs that leave more of their design bare than [`BARE`] until M13.18, each with the share it leaves
/// as measured, which is its limit: the owner's exception for the dog on turf in 9 threads (2026-11-10),
/// and the dog on turf in 6, which is over since a column's end reaches no further than its pull
/// compensation under the column it ends under.
const BARE_OVER: [(&str, f64); 2] = [("dog-on-turf-6", 0.002_04), ("dog-on-turf-9", 0.003_46)];

/// Whether `element` sews as a thin strand: a running stitch, or a satin column narrower than 1 mm.
fn thin(element: &Element) -> bool {
    match &element.shape {
        Shape::Stroke { width, .. } => element.params.get("satin_column") != Some("true") || width.get() < 1.0,
        Shape::Fill { .. } => false,
    }
}

#[test]
#[cfg_attr(debug_assertions, ignore = "release only: CI's perf job runs tests/auto_photos.rs")]
fn req_auto_027_digitized_dogs_leave_no_gap_a_thread_wide_but_as_listed_and_little_bare() {
    let tenths = |value: f64| (value * 10.0).round();
    let mut found = Vec::new();
    for (name, design, plan) in planned() {
        let cover = fabric::cover(design, plan, GAP);
        // Each gap by the middle of its box, as found and as listed.
        let mut gaps: Vec<(f64, f64)> = cover.places.iter().map(|(place, _, _)| (tenths(place.center().x()), tenths(place.center().y()))).collect();
        let mut listed: Vec<(f64, f64)> = GAPS.iter().filter(|gap| gap.dog == name).map(|gap| (tenths(gap.at.0), tenths(gap.at.1))).collect();
        gaps.sort_by(|a, b| a.partial_cmp(b).unwrap());
        listed.sort_by(|a, b| a.partial_cmp(b).unwrap());
        // A dog over the limit has its measured share for a limit, and must still be over the limit.
        let over = BARE_OVER.iter().find(|(dog, _)| dog == name).map(|(_, share)| *share);
        let within = match over {
            Some(share) => cover.bare_share() > BARE && cover.bare_share() <= share + 5e-6,
            None => cover.bare_share() <= BARE,
        };
        if gaps != listed || !within {
            found.push(format!(
                "{name}: gaps at {gaps:?}, listed at {listed:?}, of {:.2} mm², the largest {:.3} mm²; bare {:.2} of {:.0} mm², {:.3} %, its limit {:.3} %, most in {:?}",
                cover.in_gaps,
                cover.largest,
                cover.bare,
                cover.design,
                100.0 * cover.bare_share(),
                100.0 * over.unwrap_or(BARE),
                cover.shapes.iter().take(3).collect::<Vec<_>>()
            ));
        }
        // Each listed gap lies beside a thin strand: one of the 2 elements it names sews as one.
        let (elements, _) = auto::digitized(design, &Budget::DEFAULT);
        for gap in GAPS.iter().filter(|gap| gap.dog == name) {
            let beside: Vec<&Element> = gap.beside.iter().filter_map(|id| elements.iter().find(|element| element.id.as_str() == *id)).collect();
            assert!(
                gap.step == "M13.18" && beside.len() == 2 && beside.iter().any(|element| thin(element)),
                "{name}: the gap at {:?} beside {:?}",
                gap.at,
                gap.beside
            );
        }
    }
    assert!(found.is_empty(), "{found:#?}");
}
