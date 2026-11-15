//! What automatic digitizing leaves of the owner's dog (`REQ-AUTO-025` to `REQ-AUTO-027`), and how often its
//! thread changes (`REQ-AUTO-037`): her 2 photos, each
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
use stitchcraft_plan::{Rgb, StitchPlan};
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

/// Why a row of a fill shows on another colour.
const ROW: &str = "A row of an automatic fill shows on another colour for 1 mm or more. 6 of the 20 such stitches that M13.21 listed were traced, on the dog on turf in 9 threads and in the running dog's ear in 6. 5 lay along a line of fur of another colour, which sews on the fill or which the fill reaches under (REQ-AUTO-007, REQ-AUTO-033): the line sews as wide as its thread, and the row shows beside it. One lay under the edge of a column sewn after the fill, beyond the shapes as drawn. The order with the fewest thread changes (REQ-AUTO-037) took 8 of the 20 away and brought 4, where a fill now reaches under another neighbour: 16 are left, and those not among the 6 were not each traced.";

/// Every stitch of the dogs astray today. The case fails when another stitch is astray, and when one of these
/// no longer is, so the list shrinks as each is fixed: M13.17 and M13.18 empty it. M13.21 fills the dogs'
/// ragged patches, which sewed as columns: the 2 satin stitches that fanned across a shape biting into their
/// column, 7.89 and 4.72 mm long, are gone, and so are the 3 of columns that sewed past a rail and the one
/// of the base layer in a gap. The rows of those fills that show beside a thin line took their place: 20
/// stitches, the longest 4 mm, a fill's stitch length. M13.22's order, in which some columns sew before the fills
/// beside them, leaves 16 of them: 8 fewer, and 4 others.
const ASTRAY: [Astray; 17] = [
    Astray { dog: "dog-on-turf-6", element: "vc:25:fill", stitch: [(45.08, 74.27), (49.0, 74.25)], length: 3.92, cause: ROW, step: "M13.18" },
    Astray { dog: "dog-on-turf-6", element: "vc:97:fill", stitch: [(39.0, 61.75), (41.96, 61.74)], length: 2.96, cause: ROW, step: "M13.18" },
    Astray { dog: "dog-on-turf-9", element: "vc:76:fill", stitch: [(47.75, 67.02), (46.52, 67.0)], length: 1.23, cause: ROW, step: "M13.18" },
    Astray { dog: "dog-on-turf-9", element: "vc:11:fill", stitch: [(46.0, 43.5), (50.0, 43.5)], length: 4.0, cause: ROW, step: "M13.18" },
    Astray { dog: "dog-on-turf-9", element: "vc:18:fill", stitch: [(49.0, 43.25), (45.36, 43.25)], length: 3.64, cause: ROW, step: "M13.18" },
    Astray { dog: "dog-on-turf-9", element: "vc:110:fill", stitch: [(41.22, 79.5), (39.6, 79.5)], length: 1.61, cause: ROW, step: "M13.18" },
    Astray {
        dog: "dog-on-turf-9",
        element: "vc:83:fill",
        stitch: [(50.08, 23.05), (48.93, 21.12)],
        length: 2.25,
        cause: "A narrow part sews 2 lines as one running stitch. The needle goes straight from the end of one to the start of the next, across another colour, and the move, shorter than the 3 mm collapse length, is a stitch.",
        step: "M13.17",
    },
    Astray { dog: "dog-running-6", element: "vc:23:fill", stitch: [(57.0, 71.25), (61.0, 71.25)], length: 4.0, cause: ROW, step: "M13.18" },
    Astray { dog: "dog-running-6", element: "vc:23:fill", stitch: [(60.0, 71.0), (56.0, 71.0)], length: 4.0, cause: ROW, step: "M13.18" },
    Astray { dog: "dog-running-6", element: "vc:61:fill", stitch: [(56.0, 32.02), (54.27, 31.98)], length: 1.73, cause: ROW, step: "M13.18" },
    Astray { dog: "dog-running-6", element: "vc:50:fill", stitch: [(35.13, 23.98), (32.0, 24.0)], length: 3.13, cause: ROW, step: "M13.18" },
    Astray { dog: "dog-running-6", element: "vc:81:fill", stitch: [(46.47, 20.25), (45.0, 20.25)], length: 1.47, cause: ROW, step: "M13.18" },
    Astray { dog: "dog-running-6", element: "vc:81:fill", stitch: [(44.07, 21.0), (48.0, 21.0)], length: 3.93, cause: ROW, step: "M13.18" },
    Astray { dog: "dog-running-6", element: "vc:89:fill", stitch: [(56.75, 55.5), (54.6, 55.49)], length: 2.15, cause: ROW, step: "M13.18" },
    Astray { dog: "dog-running-9", element: "vc:49:fill", stitch: [(56.75, 55.5), (54.0, 55.5)], length: 2.75, cause: ROW, step: "M13.18" },
    Astray { dog: "dog-running-9", element: "vc:68:fill", stitch: [(54.03, 44.25), (55.31, 44.25)], length: 1.28, cause: ROW, step: "M13.18" },
    Astray { dog: "dog-running-9", element: "vc:57:fill", stitch: [(32.0, 24.0), (36.0, 24.0)], length: 4.0, cause: ROW, step: "M13.18" },
];

/// How each traced dog's shapes sew, as `SC-I0901` counts them: as running stitches, as satin columns and as
/// tatami fills. Before M13.21, width alone chose, and the dogs sewed 64, 73, 59 and 79 shapes as satin
/// columns and 4, 3, 6 and 7 as fills.
const SEWN: [(&str, [usize; 3]); 4] =
    [("dog-on-turf-6", [25, 27, 45]), ("dog-on-turf-9", [35, 31, 49]), ("dog-running-6", [15, 21, 47]), ("dog-running-9", [18, 29, 60])];

#[test]
#[cfg_attr(debug_assertions, ignore = "release only: CI's perf job runs tests/auto_photos.rs")]
fn req_auto_032_a_traced_animal_sews_mostly_as_fills_and_no_satin_stitch_too_long() {
    use stitchcraft_core::{Budget, Code};
    use stitchcraft_engine::auto::{LONGEST_SATIN_STITCH, digitized};
    use stitchcraft_engine::design::Shape;
    use stitchcraft_engine::generators::satin::SATIN_COLUMN;
    use stitchcraft_plan::Role;

    let limit = LONGEST_SATIN_STITCH + 2.0 * rules().auto_pull_compensation_mm.get();
    let (mut found, mut longest) = (Vec::new(), Vec::new());
    for (name, design, plan) in planned() {
        // The shapes by the stitches they sew as, read from what is said of them.
        let (elements, said) = digitized(design, &Budget::DEFAULT);
        let counted = said.iter().find(|said| said.code == Code::AutoDigitized).unwrap().message.clone();
        let count = |stitches: &str| {
            let before = counted.split(stitches).next().filter(|before| before.len() < counted.len()).unwrap_or("0 ");
            before.split_whitespace().rev().find_map(|word| word.parse::<usize>().ok()).unwrap_or(0)
        };
        found.push((name.as_str(), [count("running stitch"), count("satin column"), count("tatami fill")]));
        // The longest top stitch of an element sewn as a satin column.
        let satin = |id: &str| {
            elements.iter().any(|e| e.id.as_str() == id && matches!(e.shape, Shape::Stroke { .. }) && e.params.get(SATIN_COLUMN) == Some("true"))
        };
        let stitches = plan.sewn_stitches().into_iter().filter(|stitch| stitch.role == Role::Top && stitch.from_role == Role::Top);
        let of_satin = stitches.filter(|stitch| {
            let element = plan.blocks[stitch.block].stitches[stitch.index].origin.element.and_then(|element| plan.element(element));
            element.is_some_and(|id| satin(id.as_str()))
        });
        longest.push((name.as_str(), of_satin.map(|stitch| stitch.from.distance(stitch.to)).fold(0.0, f64::max)));
    }
    assert_eq!(found, SEWN);
    assert!(longest.iter().all(|(_, length)| *length > 3.0 && *length <= limit), "{longest:?}");
}

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
    let recorded = [("dog-on-turf-6", 7, 77), ("dog-on-turf-9", 11, 78), ("dog-running-6", 3, 70), ("dog-running-9", 5, 95)];
    assert_eq!(apart, recorded.map(|(name, moves, jumps)| (name.to_string(), moves, jumps)));
}

/// What the top thread of each traced dog leaves to the base layer, as measured: how many slivers, how many
/// of them lie at the rim, where the base layer's offset leaves no base layer along the design's outline, the
/// largest in hundredths of a square millimetre, and the share of the design left bare in thousandths of a
/// percent. The case fails when a number changes, so that a dog whose slivers grow shows here before it
/// reaches a limit, and one that loses a sliver gets its row changed. Most slivers lie beside a line of fur
/// that sews as a running stitch, which is as wide as its thread and can't reach under a neighbour.
const SLIVERS: [(&str, usize, usize, f64, f64); 4] =
    [("dog-on-turf-6", 0, 0, 0.0, 18.0), ("dog-on-turf-9", 0, 0, 0.0, 81.0), ("dog-running-6", 1, 0, 26.0, 25.0), ("dog-running-9", 0, 0, 0.0, 1.0)];

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

/// How often each traced dog's thread changes: as sewn, the fewest that any order has in which each shape
/// sews after what it lies on and each running line after the fills it borders, and one less than its
/// threads, which no order betters. Each change is a rethread by hand on a machine with one needle. Before
/// M13.22 the dogs changed thread 12, 22, 16 and 28 times.
const CHANGES: [(&str, usize, usize); 4] = [("dog-on-turf-6", 8, 5), ("dog-on-turf-9", 12, 8), ("dog-running-6", 8, 5), ("dog-running-9", 12, 8)];

#[test]
#[cfg_attr(debug_assertions, ignore = "release only: CI's perf job runs tests/auto_photos.rs")]
fn req_auto_037_digitized_dogs_change_thread_as_seldom_as_their_lines_allow() {
    let found: Vec<(String, usize, usize)> = planned()
        .iter()
        .map(|(name, _, plan)| {
            let threads: std::collections::BTreeSet<Rgb> = plan.blocks.iter().map(|block| block.thread.color).collect();
            (name.clone(), plan.stats().color_changes, threads.len() - 1)
        })
        .collect();
    assert_eq!(found, CHANGES.map(|(name, changes, floor)| (name.to_string(), changes, floor)));
}
