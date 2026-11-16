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

/// Why a stitch between 2 lines of one part shows on another colour.
const LINES: &str = "A narrow part sews 2 lines as one running stitch. The needle goes straight from the end of one to the start of the next, across another colour, and the move, shorter than the 3 mm collapse length, is a stitch. M13.17 looked for a run under what is sewn later (REQ-AUTO-053) and found no way that is hidden. A jump there would leave a thread as long as the stitch.";

/// Why a stitch of an outline shows on another colour.
const ACROSS: &str = "A row of an automatic fill shows on another colour for 1 mm or more, since M13.30. Where 2 areas lie beside a line of fur, the one sewn first now sews the line's part alone, and the area beyond the line gives it up (REQ-AUTO-063). A row along that part then shows beside the line, where a row of the area beyond covered it before. Each such row was traced: its middle or an end lies on the part of a running line 0.5 to 0.8 mm wide, whose thread is 0.4 mm wide.";

const OUTLINE: &str = "A stitch of a fill's outline (REQ-AUTO-051) shows on another colour for 1 mm or more. The outline is one ring round its fill, and the stitch lies along it, its ends within 0.15 mm of the ring. The fill is a thin, ragged shape of a traced photo beside others of its kind. What lies beside the ring there was not traced.";

/// Every stitch of the dogs astray today. The case fails when another stitch is astray, and when one of these
/// no longer is, so the list shrinks as each is fixed: M13.18 is to empty it. M13.21 fills the dogs'
/// ragged patches, which sewed as columns: the 2 satin stitches that fanned across a shape biting into their
/// column, 7.89 and 4.72 mm long, are gone, and so are the 3 of columns that sewed past a rail and the one
/// of the base layer in a gap. The rows of those fills that show beside a thin line took their place: 20
/// stitches, the longest 4 mm, a fill's stitch length. M13.22's order, in which some columns sew before the fills
/// beside them, leaves 16 of them: 8 fewer, and 4 others. M13.27 fills 5 blobs of fur that sewed as columns, each
/// with arms that no centre line follows: a row of the largest, the same blob in both running dogs, shows beside
/// a line of fur, and a line's move crosses another, where a row of the running dog in 9 threads no longer shows.
/// M13.28 sews an outline after each fill (REQ-AUTO-051), so that the fills end elsewhere: 8 rows no longer show
/// and one other does, 2 rows are sewn the other way, and 2 stitches of outlines show beside their fills: 14
/// stitches. Which neighbour hid each of the 8 rows was not traced. M13.17 runs the thread under later shapes in
/// place of jumps (REQ-AUTO-053), which moves no stitch of these: the 2 moves between a part's lines have no
/// way that is hidden, and stay. M13.30 lets one area alone sew under a shape (REQ-AUTO-063): 8 of the 14
/// no longer showed, and 13 others did, 12 of them rows of a fill ([`ACROSS`]) and one a stitch of an outline: 19
/// stitches. It then fills the parts whose one satin column would crowd its stitches (REQ-AUTO-064), which
/// sew as fills in another place of the order: 7 of the 19 no longer show, and 4 others do, 3 rows on a
/// line's part and a stitch of an outline: 16 stitches.
const ASTRAY: [Astray; 16] = [
    Astray { dog: "dog-on-turf-6", element: "vc:15:fill", stitch: [(47.0, 43.75), (50.42, 43.77)], length: 3.42, cause: ACROSS, step: "M13.18" },
    Astray { dog: "dog-on-turf-6", element: "vc:68:fill", stitch: [(43.22, 74.73), (46.51, 74.76)], length: 3.29, cause: ACROSS, step: "M13.18" },
    Astray { dog: "dog-on-turf-6", element: "vc:81:fill#2", stitch: [(49.08, 80.85), (49.52, 82.56)], length: 1.76, cause: OUTLINE, step: "M13.18" },
    Astray { dog: "dog-on-turf-6", element: "vc:75:fill", stitch: [(68.49, 40.52), (70.0, 40.5)], length: 1.51, cause: ACROSS, step: "M13.18" },
    Astray { dog: "dog-on-turf-6", element: "vc:71:fill#2", stitch: [(51.96, 22.56), (51.83, 24.9)], length: 2.34, cause: OUTLINE, step: "M13.18" },
    Astray { dog: "dog-on-turf-9", element: "vc:11:fill", stitch: [(45.0, 43.25), (49.0, 43.25)], length: 4.0, cause: ACROSS, step: "M13.18" },
    Astray { dog: "dog-on-turf-9", element: "vc:11:fill", stitch: [(50.0, 43.5), (46.0, 43.5)], length: 4.0, cause: ACROSS, step: "M13.18" },
    Astray { dog: "dog-on-turf-9", element: "vc:6:fill", stitch: [(53.0, 44.25), (55.14, 44.25)], length: 2.14, cause: ACROSS, step: "M13.18" },
    Astray { dog: "dog-on-turf-9", element: "vc:6:fill", stitch: [(56.0, 44.0), (52.0, 44.0)], length: 4.0, cause: ACROSS, step: "M13.18" },
    Astray { dog: "dog-on-turf-9", element: "vc:17:fill", stitch: [(49.0, 74.25), (46.88, 74.23)], length: 2.12, cause: ACROSS, step: "M13.18" },
    Astray { dog: "dog-on-turf-9", element: "vc:78:fill", stitch: [(46.68, 74.75), (43.23, 74.77)], length: 3.45, cause: ACROSS, step: "M13.18" },
    Astray { dog: "dog-on-turf-9", element: "vc:100:fill", stitch: [(44.53, 19.5), (46.0, 19.5)], length: 1.47, cause: ACROSS, step: "M13.18" },
    Astray { dog: "dog-on-turf-9", element: "vc:83:fill", stitch: [(50.08, 23.05), (48.93, 21.12)], length: 2.25, cause: LINES, step: "M13.18" },
    Astray { dog: "dog-running-6", element: "vc:26:fill#2", stitch: [(38.98, 4.59), (41.19, 4.89)], length: 2.24, cause: OUTLINE, step: "M13.18" },
    Astray { dog: "dog-running-9", element: "vc:21:fill", stitch: [(49.78, 51.5), (46.0, 51.5)], length: 3.78, cause: ACROSS, step: "M13.18" },
    Astray { dog: "dog-running-9", element: "vc:33:fill#2", stitch: [(55.64, 34.0), (54.3, 32.39)], length: 2.1, cause: LINES, step: "M13.18" },
];

/// How each traced dog's shapes sew, as `SC-I0901` counts them: as running stitches, as satin columns and as
/// tatami fills. Before M13.21, width alone chose, and the dogs sewed 64, 73, 59 and 79 shapes as satin
/// columns and 4, 3, 6 and 7 as fills. M13.27 fills 2 shapes with arms of the dog on turf in 6 threads, and one of
/// the running dog in 9. M13.30 fills the parts whose one satin column would crowd its stitches
/// (REQ-AUTO-064): 15, 18, 15 and 17 shapes that sewed as satin columns, when they were 25, 31, 21 and 28.
const SEWN: [(&str, [usize; 3]); 4] =
    [("dog-on-turf-6", [25, 10, 62]), ("dog-on-turf-9", [34, 13, 68]), ("dog-running-6", [15, 6, 62]), ("dog-running-9", [18, 11, 78])];

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
    // The columns left are the slender ones: with the parts filled whose column would crowd its stitches
    // (REQ-AUTO-064), each dog's longest satin stitch is 2.8 to 3.8 mm.
    assert!(longest.iter().all(|(_, length)| *length > 2.0 && *length <= limit), "{longest:?}");
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
    assert!(ASTRAY.iter().all(|a| !a.cause.is_empty() && a.step == "M13.18"));

    // The moves sewn on from one element to the next, and the jump threads the PE800 leaves, are as the design
    // page records them (image-to-embroidery.md › Sewing order).
    // With an outline after each fill (M13.28) the dogs left 76, 84, 73 and 96 jump threads, 5, 6, 3 and 3 more
    // than before. Where shapes sewn later cover a way, the needle now runs under them (REQ-AUTO-053, M13.17): 7,
    // 5, 13 and 13 jumps are runs, and so is one move that showed of each dog but the first.
    // With one overlap under a shape (REQ-AUTO-063, M13.30) a fill ends further from the shape sewn next in a
    // few places, and the dogs leave 4, 1 and 1 more jump threads and the same. The moves that show were 5, 9,
    // 2 and 6.
    // With the parts filled whose one column would crowd its stitches (REQ-AUTO-064), the dogs leave 71, 80,
    // 58 and 74: 2 and 1 more than before M13.30, and 2 and 9 fewer.
    let recorded = [("dog-on-turf-6", 6, 71), ("dog-on-turf-9", 5, 80), ("dog-running-6", 5, 58), ("dog-running-9", 8, 74)];
    let recorded = recorded.map(|(name, moves, jumps)| (name.to_string(), moves, jumps));
    assert_eq!((found, apart), (listed, recorded.to_vec()), "the stitches astray, as found and as listed in ASTRAY, and the moves and jumps");
}

/// What the top thread of each traced dog leaves to the base layer, as measured: how many slivers, how many
/// of them lie at the rim, where the base layer's offset leaves no base layer along the design's outline, the
/// largest in hundredths of a square millimetre, and the share of the design left bare in thousandths of a
/// percent. The case fails when a number changes, so that a dog whose slivers grow shows here before it
/// reaches a limit, and one that loses a sliver gets its row changed. Most slivers lie beside a line of fur
/// that sews as a running stitch, which is as wide as its thread and can't reach under a neighbour.
const SLIVERS: [(&str, usize, usize, f64, f64); 4] =
    [("dog-on-turf-6", 0, 0, 0.0, 17.0), ("dog-on-turf-9", 1, 0, 18.0, 38.0), ("dog-running-6", 0, 0, 0.0, 0.0), ("dog-running-9", 0, 0, 0.0, 0.0)];

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

/// How thick each traced dog sews, as measured: the area with 3 fills of thread or more and the area with 6
/// or more, in hundredths of a square millimetre (REQ-AUTO-060). Before M13.30 the 4 had 436.0, 480.2, 511.0 and
/// 629.2 mm² at 3 or more, and 11.8, 11.0, 6.5 and 6.2 mm² at 6 or more. The case fails when a number changes,
/// so that a change that piles thread up again shows here.
const THICK: [(&str, f64, f64); 4] =
    [("dog-on-turf-6", 17750.0, 0.0), ("dog-on-turf-9", 20575.0, 0.0), ("dog-running-6", 18350.0, 0.0), ("dog-running-9", 28000.0, 0.0)];

#[test]
#[cfg_attr(debug_assertions, ignore = "release only: CI's perf job runs tests/auto_photos.rs")]
fn req_auto_063_digitized_dogs_sew_thick_on_the_area_recorded() {
    let found: Vec<(String, f64, f64)> = planned()
        .iter()
        .map(|(name, _, plan)| {
            let report = fabric::thickness(plan, &fabric::Limits::DEFAULT);
            (name.clone(), hundredths(report.thick.area), hundredths(report.failing.area))
        })
        .collect();
    assert_eq!(found, THICK.map(|(name, thick, failing)| (name.to_string(), thick, failing)));
}

/// How often each traced dog's thread changes: as sewn, the fewest that any order has in which each shape
/// sews after what it lies on and each running line after the fills it borders, and one less than its
/// threads, which no order betters. Each change is a rethread by hand on a machine with one needle. Before
/// M13.22 the dogs changed thread 12, 22, 16 and 28 times. Since M13.30 fills the parts whose one satin
/// column would crowd its stitches (REQ-AUTO-064), the dog on turf in 9 threads changes thread 13 times
/// where it changed 12: which of its new fills costs the change was not traced.
const CHANGES: [(&str, usize, usize); 4] = [("dog-on-turf-6", 8, 5), ("dog-on-turf-9", 13, 8), ("dog-running-6", 8, 5), ("dog-running-9", 12, 8)];

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
