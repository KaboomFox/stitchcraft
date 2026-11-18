//! What automatic digitizing leaves of the owner's dog (`REQ-AUTO-025` to `REQ-AUTO-027`), how often its
//! thread changes (`REQ-AUTO-037`) and how much of its detail it keeps (`REQ-AUTO-093`, `REQ-AUTO-094`): her 2
//! photos, each
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

use stitchcraft_engine::auto;
use stitchcraft_engine::design::{Design, Element};
use stitchcraft_plan::{Rgb, StitchPlan};
use stitchcraft_testkit::designs::{in_place, rules};
use stitchcraft_testkit::fabric::likeness::{self, LIKENESS};
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

/// Why a stitch between 2 lines of one part shows on another colour.
const LINES: &str = "A narrow part sews 2 lines as one running stitch. The needle goes straight from the end of one to the start of the next, across another colour, and the move, shorter than the 3 mm collapse length, is a stitch. M13.17 looked for a run under what is sewn later (REQ-AUTO-053) and found no way that is hidden. A jump there would leave a thread as long as the stitch.";

/// Why a row of a fill shows on another colour beside a line.
const ACROSS: &str = "A row of an automatic fill shows on another colour for 1 mm or more, since M13.30. Where 2 areas lie beside a line of fur, the one sewn first now sews the line's part alone, and the area beyond the line gives it up (REQ-AUTO-067). A row along that part then shows beside the line, where a row of the area beyond covered it before. Each such row was traced: its middle or an end lies on the part of a running line 0.5 to 0.8 mm wide, whose thread is 0.4 mm wide.";

/// Why a stitch of an outline shows on another colour.
const OUTLINE: &str = "A stitch of a fill's outline (REQ-AUTO-051) shows on another colour for 1 mm or more. The outline is one ring round its fill, and the stitch lies along it, its ends within 0.15 mm of the ring. The fill is a thin, ragged shape of a traced photo beside others of its kind. What lies beside the ring there was not traced.";

const MOVED: &str = "A row of an automatic fill shows on another colour for 1 mm or more. The same fill had a row astray a row or 2 away before M13.18 merged the ragged thin lines beside it into their neighbours (REQ-AUTO-090), which moved its rows. Not traced further.";

/// Every stitch of the dogs astray today. The case fails when another stitch is astray, and when one of these
/// no longer is, so the list shrinks as each is fixed. M13.18 merged the ragged thin lines into their
/// neighbours (REQ-AUTO-090): 10 of the 18 are gone, and 3 fills that had a row astray show 4 rows a row or 2
/// from it. M13.38 is to empty it. M13.21 fills the dogs'
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
/// way that is hidden, and stay. M13.30 lets one area alone sew under a shape (REQ-AUTO-067): 8 of the 14
/// no longer showed, and 13 others did, 12 of them rows of a fill ([`ACROSS`]) and one a stitch of an outline: 19
/// stitches. It then fills the parts whose one satin column would crowd its stitches (REQ-AUTO-068), which
/// sew as fills in another place of the order: 7 of the 19 no longer show, and 4 others do, 3 rows on a
/// line's part and a stitch of an outline: 16 stitches. M13.35's last pass moves row ends back under later
/// shapes (REQ-AUTO-074), and a row of `vc:17:fill` on the dog on turf in 9 threads no longer shows: 15. Its
/// outlines sew through no more than 12 mm under a later fill (REQ-AUTO-076), so that some fills end elsewhere:
/// 2 rows and a stitch of an outline more show, and a stitch of an outline is sewn the other way: 18. Which
/// change moved each was not traced.
const ASTRAY: [Astray; 12] = [
    Astray { dog: "dog-on-turf-6", element: "vc:68:fill", stitch: [(45.58, 74.75), (43.22, 74.77)], length: 2.35, cause: MOVED, step: "M13.38" },
    Astray { dog: "dog-on-turf-6", element: "vc:68:fill", stitch: [(45.0, 74.25), (42.3, 74.26)], length: 2.7, cause: MOVED, step: "M13.38" },
    Astray { dog: "dog-on-turf-6", element: "vc:81:fill#2", stitch: [(49.08, 80.85), (49.52, 82.56)], length: 1.76, cause: OUTLINE, step: "M13.38" },
    Astray { dog: "dog-on-turf-6", element: "vc:75:fill", stitch: [(70.57, 39.75), (68.28, 39.72)], length: 2.29, cause: MOVED, step: "M13.38" },
    Astray { dog: "dog-on-turf-6", element: "vc:71:fill#2", stitch: [(51.96, 22.56), (51.83, 24.9)], length: 2.34, cause: OUTLINE, step: "M13.38" },
    Astray { dog: "dog-on-turf-9", element: "vc:6:fill", stitch: [(53.0, 44.25), (55.14, 44.25)], length: 2.14, cause: ACROSS, step: "M13.38" },
    Astray { dog: "dog-on-turf-9", element: "vc:6:fill", stitch: [(56.0, 44.0), (52.0, 44.0)], length: 4.0, cause: ACROSS, step: "M13.38" },
    Astray { dog: "dog-running-6", element: "vc:26:fill#2", stitch: [(41.19, 4.89), (38.98, 4.59)], length: 2.24, cause: OUTLINE, step: "M13.38" },
    Astray { dog: "dog-running-9", element: "vc:21:fill", stitch: [(49.31, 51.51), (46.0, 51.5)], length: 3.31, cause: MOVED, step: "M13.38" },
    Astray { dog: "dog-running-9", element: "vc:52:fill", stitch: [(60.0, 74.0), (64.0, 74.0)], length: 4.0, cause: ACROSS, step: "M13.38" },
    Astray {
        dog: "dog-running-9",
        element: "vc:43:fill#2~2",
        stitch: [(51.81, 61.71), (50.01, 61.42)],
        length: 1.82,
        cause: OUTLINE,
        step: "M13.38",
    },
    Astray { dog: "dog-running-9", element: "vc:33:fill#2", stitch: [(55.64, 34.0), (54.3, 32.39)], length: 2.1, cause: LINES, step: "M13.38" },
];

/// How each traced dog's shapes sew, as `SC-I0901` counts them: as running stitches, as satin columns and as
/// tatami fills. Before M13.21, width alone chose, and the dogs sewed 64, 73, 59 and 79 shapes as satin
/// columns and 4, 3, 6 and 7 as fills. M13.27 fills 2 shapes with arms of the dog on turf in 6 threads, and one of
/// the running dog in 9. M13.30 fills the parts whose one satin column would crowd its stitches
/// (REQ-AUTO-068): 15, 18, 15 and 17 shapes that sewed as satin columns, when they were 25, 31, 21 and 28.
/// With the ragged thin lines merged into their neighbours (REQ-AUTO-090, M13.18), the dogs sew 3, 4, 1 and 3 shapes
/// as running stitches, where they sewed 25, 34, 15 and 18. Merging keeps the picture's likeness
/// (REQ-AUTO-095, M13.44), and the dog on turf in 9 threads keeps one line, which sews as a running stitch.
const SEWN: [(&str, [usize; 3]); 4] =
    [("dog-on-turf-6", [3, 10, 62]), ("dog-on-turf-9", [5, 12, 70]), ("dog-running-6", [1, 6, 62]), ("dog-running-9", [3, 11, 78])];

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
    // (REQ-AUTO-068), each dog's longest satin stitch is 2.8 to 3.8 mm.
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
    assert!(ASTRAY.iter().all(|a| !a.cause.is_empty() && a.step == "M13.38"));

    // The moves sewn on from one element to the next, and the jump threads the PE800 leaves, are as the design
    // page records them (image-to-embroidery.md › Sewing order).
    // With an outline after each fill (M13.28) the dogs left 76, 84, 73 and 96 jump threads, 5, 6, 3 and 3 more
    // than before. Where shapes sewn later cover a way, the needle now runs under them (REQ-AUTO-053, M13.17): 7,
    // 5, 13 and 13 jumps are runs, and so is one move that showed of each dog but the first.
    // With one overlap under a shape (REQ-AUTO-067, M13.30) a fill ends further from the shape sewn next in a
    // few places, and the dogs leave 4, 1 and 1 more jump threads and the same. The moves that show were 5, 9,
    // 2 and 6.
    // With the parts filled whose one column would crowd its stitches (REQ-AUTO-068), the dogs leave 71, 80,
    // 58 and 74: 2 and 1 more than before M13.30, and 2 and 9 fewer.
    // With narrow columns that end where they start (REQ-AUTO-075, M13.35), jump threads start or end
    // elsewhere: the dogs leave 73, 81, 58 and 72, and the running dog in 9 threads shows 10 moves.
    // With outlines that sew through no more than 12 mm under a later fill (REQ-AUTO-076), 74, 84, 61 and 74,
    // and the 2 dogs in 6 threads show 6 and 3 moves.
    // With narrow columns that start at their start (REQ-AUTO-086, M13.36), the dogs in 9 threads leave one
    // jump thread more each, 85 and 75, and the running one shows 9 moves.
    // With the ragged thin lines merged into their neighbours (REQ-AUTO-090, M13.18), 57, 58, 49 and 64, and
    // the dogs on turf show 4 and 5 moves. The line the dog on turf in 9 threads keeps, as merging keeps its
    // likeness (REQ-AUTO-095, M13.44), leaves it one jump thread more.
    let recorded = [("dog-on-turf-6", 4, 57), ("dog-on-turf-9", 5, 59), ("dog-running-6", 3, 49), ("dog-running-9", 9, 64)];
    let recorded = recorded.map(|(name, moves, jumps)| (name.to_string(), moves, jumps));
    assert_eq!((found, apart), (listed, recorded.to_vec()), "the stitches astray, as found and as listed in ASTRAY, and the moves and jumps");
}

/// What the top thread of each traced dog leaves to the base layer, as measured: how many slivers, how many
/// of them lie at the rim, where the base layer's offset leaves no base layer along the design's outline, the
/// largest in hundredths of a square millimetre, and the share of the design left bare in thousandths of a
/// percent. The case fails when a number changes, so that a dog whose slivers grow shows here before it
/// reaches a limit, and one that loses a sliver gets its row changed. Most slivers lie beside a line of fur
/// that sews as a running stitch, which is as wide as its thread and can't reach under a neighbour. With the
/// columns' crowded needle points moved in (REQ-AUTO-071) the dog on turf in 9 threads leaves 41 where it left
/// 38, and 45 with the last pass's row ends moved back (REQ-AUTO-074), not traced, and 43 with outlines that
/// sew through no more than 12 mm under a later fill (REQ-AUTO-076). With the ragged thin lines merged into their
/// neighbours (REQ-AUTO-090), its sliver is gone, and the dogs leave 10, 10, 0 and 5 thousandths of a percent bare.
const SLIVERS: [(&str, usize, usize, f64, f64); 4] =
    [("dog-on-turf-6", 0, 0, 0.0, 10.0), ("dog-on-turf-9", 0, 0, 0.0, 10.0), ("dog-running-6", 0, 0, 0.0, 0.0), ("dog-running-9", 0, 0, 0.0, 5.0)];

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
/// 629.2 mm² at 3 or more, and 11.8, 11.0, 6.5 and 6.2 mm² at 6 or more, and after it 177.5, 205.75, 183.5 and
/// 280.0 mm² at 3 or more, before M13.35's locks, columns, last pass and narrow columns walked once
/// (REQ-AUTO-071 to REQ-AUTO-075) and outlines no longer sewn through more than 12 mm under a later fill
/// (REQ-AUTO-076), 132.25, 160.0, 146.25 and 209.75 mm² before M13.36's holes, edges and narrow columns
/// started at their start (REQ-AUTO-084 to REQ-AUTO-086), and 130.5, 156.5, 142.0 and 207.75 before the ragged
/// thin lines merged into their neighbours (REQ-AUTO-090), and 153.25 for the dog on turf in 9 threads before
/// merging kept its likeness (REQ-AUTO-095). The case fails when a number changes, so that a
/// change that piles thread up again shows here.
const THICK: [(&str, f64, f64); 4] =
    [("dog-on-turf-6", 14150.0, 0.0), ("dog-on-turf-9", 15275.0, 0.0), ("dog-running-6", 13050.0, 0.0), ("dog-running-9", 18750.0, 0.0)];

#[test]
#[cfg_attr(debug_assertions, ignore = "release only: CI's perf job runs tests/auto_photos.rs")]
fn req_auto_067_digitized_dogs_sew_thick_on_the_area_recorded() {
    let found: Vec<(String, f64, f64)> = planned()
        .iter()
        .map(|(name, _, plan)| {
            let report = fabric::thickness(plan, &fabric::Limits::DEFAULT);
            (name.clone(), hundredths(report.thick.area), hundredths(report.failing.area))
        })
        .collect();
    assert_eq!(found, THICK.map(|(name, thick, failing)| (name.to_string(), thick, failing)));
}

/// The thickest spot of each traced dog, in hundredths of a fill (REQ-AUTO-073): M13.30 left 5.54, 5.54, 6.05
/// and 7.01, M13.35 4.53, 4.51, 5.07 and 5.89, and M13.36 4.53, 4.49, 5.07 and 5.63. Every dog is still above
/// the cap of 4 fills, where 5 to 8 elements meet. With the ragged thin lines merged (REQ-AUTO-090), the running
/// dog in 6 threads sews 0.45 fills less at its thickest spot and the one in 9 as much, and the dogs on turf 0.16
/// and 0.31 more, at new spots where a satin column or a fill lies over 3 or 4 other fills.
const THICKEST: [(&str, f64); 4] = [("dog-on-turf-6", 469.0), ("dog-on-turf-9", 480.0), ("dog-running-6", 462.0), ("dog-running-9", 563.0)];

#[test]
#[cfg_attr(debug_assertions, ignore = "release only: CI's perf job runs tests/auto_photos.rs")]
fn req_auto_073_digitized_dogs_sew_their_thickest_spot_as_recorded() {
    let found: Vec<(String, f64)> =
        planned().iter().map(|(name, _, plan)| (name.clone(), hundredths(fabric::thickness(plan, &fabric::Limits::DEFAULT).fills.value))).collect();
    assert_eq!(found, THICKEST.map(|(name, thickest)| (name.to_string(), thickest)));
}

/// How often each traced dog's thread changes: as sewn, the fewest that any order has in which each shape
/// sews after what it lies on and each running line after the fills it borders, and one less than its
/// threads, which no order betters. Each change is a rethread by hand on a machine with one needle. Before
/// M13.22 the dogs changed thread 12, 22, 16 and 28 times. Since M13.30 fills the parts whose one satin
/// column would crowd its stitches (REQ-AUTO-068), the dog on turf in 9 threads changes thread 13 times
/// where it changed 12. Traced on 2026-11-16 by sewing each of its 19 crowded parts as a column again: with
/// the parts of the grey shapes `vc:23` and `vc:24` as columns it changes thread 12 times, with 2 parts of a
/// tan and a pale grey shape too, and with no one part. A line sews after the fill it borders (rule 2 of the
/// sewing order), and nothing holds a line back for a column, so each new fill makes the lines of other
/// threads beside it wait for its thread. The search is exact: within the rules, 13 is the fewest. With the
/// ragged thin lines merged into their neighbours (REQ-AUTO-090), fewer lines wait for a fill, and the dog on turf
/// in 9 threads changes thread 12 times again, the running dog in 9 threads 11 times.
const CHANGES: [(&str, usize, usize); 4] = [("dog-on-turf-6", 8, 5), ("dog-on-turf-9", 12, 8), ("dog-running-6", 8, 5), ("dog-running-9", 11, 8)];

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

/// What each traced dog keeps of its detail (REQ-AUTO-093), as measured on 2026-11-17: its parts, the parts
/// kept and its likeness in thousandths. What is lost is mostly strands of fur and shading that merge as thin
/// lines (REQ-AUTO-090), and on the dog on turf the pale strip above its nose. Merging keeps the picture's
/// likeness (REQ-AUTO-095, M13.44), and the dog on turf in 9 threads keeps 84, 0.792, where it kept 83, 0.779.
/// The case fails when a number changes, so that a change that takes detail away, or puts it back, shows here.
const LIKE: [(&str, usize, usize, f64); 4] =
    [("dog-on-turf-6", 97, 75, 814.0), ("dog-on-turf-9", 111, 84, 792.0), ("dog-running-6", 77, 66, 898.0), ("dog-running-9", 100, 88, 921.0)];

/// What `likeness` keeps, as [`LIKE`] records it.
fn recorded(name: &str, likeness: &likeness::Likeness) -> (String, usize, usize, f64) {
    (name.to_string(), likeness.parts.len(), likeness.parts.iter().filter(|part| part.kept()).count(), (likeness.likeness() * 1000.0).round())
}

#[test]
#[cfg_attr(debug_assertions, ignore = "release only: CI's perf job runs tests/auto_photos.rs")]
fn req_auto_094_digitized_dogs_keep_their_likeness_as_recorded() {
    let found: Vec<(String, usize, usize, f64)> = planned()
        .iter()
        .map(|(name, design, plan)| {
            let kept = likeness::likeness(design, plan);
            assert!(kept.likeness() >= LIKENESS, "{name}: {:.3}", kept.likeness());
            recorded(name, &kept)
        })
        .collect();
    assert_eq!(found, LIKE.map(|(name, parts, kept, likeness)| (name.to_string(), parts, kept, likeness)));
}

/// What each traced dog keeps with its thin lines sewn as lines, as before they merged (`auto_fill_uneven`
/// off, REQ-AUTO-090), as measured on 2026-11-17.
const LINED: [(&str, usize, usize, f64); 4] =
    [("dog-on-turf-6", 97, 96, 993.0), ("dog-on-turf-9", 111, 105, 947.0), ("dog-running-6", 77, 73, 970.0), ("dog-running-9", 100, 97, 983.0)];

#[test]
#[cfg_attr(debug_assertions, ignore = "release only: CI's perf job runs tests/auto_photos.rs")]
fn req_auto_093_a_dog_whose_thin_lines_merge_keeps_less_of_its_detail() {
    let found: Vec<(String, usize, usize, f64)> = pictures::dogs()
        .into_iter()
        .map(|(name, design)| {
            let lined = design.elements().iter().cloned().map(|mut element: Element| {
                element.params.set(auto::FILL_UNEVEN, "false");
                element
            });
            let lined = Design::new(lined.collect(), design.settings).unwrap();
            let outcome = in_place(&lined);
            let kept = likeness::likeness(&design, &outcome.plan.unwrap());
            recorded(&name, &kept)
        })
        .collect();
    assert_eq!(found, LINED.map(|(name, parts, kept, likeness)| (name.to_string(), parts, kept, likeness)));
    for ((_, _, merged, _), (name, _, lined, _)) in LIKE.iter().zip(LINED) {
        assert!(lined > *merged, "{name}");
    }
}
