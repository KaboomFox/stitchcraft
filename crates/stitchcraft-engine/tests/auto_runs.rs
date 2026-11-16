//! The runs that automatic digitizing sews in place of jumps (`REQ-AUTO-053`), through planning: 2 squares
//! of one thread, with a bar of another thread across them and without it.
//! `src/moves/tests.rs` has the rules' cases on stitches laid by hand, `tests/auto_fabric.rs` and
//! `tests/auto_sheets.rs` the jump threads of the test pictures, the traced pear and the TS-13 sheets, and
//! `tests/auto_photos.rs` those of the traced dogs.

// Test code may unwrap and index (clippy.toml allows it in tests).
#![allow(clippy::unwrap_used, clippy::panic, clippy::indexing_slicing)]

use stitchcraft_core::{Budget, Code};
use stitchcraft_engine::design::{Design, DesignSettings, Element};
use stitchcraft_engine::plan;
use stitchcraft_plan::profiles::REFERENCE;
use stitchcraft_plan::{Role, SewnStitch, StitchPlan};
use stitchcraft_testkit::designs::{BLUE, RED, fill, in_place, outlined};
use stitchcraft_testkit::fabric;

/// 2 red squares 10 mm wide and 20 mm apart as shapes without settings, and with `bar` a blue bar 6 mm tall
/// drawn over them, from 2 mm inside the first to 2 mm inside the second.
fn squares(bar: bool) -> Design {
    let square = |id: &str, left: f64| outlined(fill(id, &[&[(left, 0.0), (left + 10.0, 0.0), (left + 10.0, 10.0), (left, 10.0)]], &RED, &[]));
    let mut elements: Vec<Element> = vec![square("left", 0.0), square("right", 30.0)];
    if bar {
        elements.push(outlined(fill("bar", &[&[(8.0, 2.0), (32.0, 2.0), (32.0, 8.0), (8.0, 8.0)]], &BLUE, &[])));
    }
    Design::new(elements, DesignSettings::default()).unwrap()
}

/// The stitches of `plan` that move the needle under other stitches.
fn runs(plan: &StitchPlan) -> Vec<SewnStitch> {
    plan.sewn_stitches().into_iter().filter(|stitch| stitch.role == Role::Travel).collect()
}

/// How often `plan` jumps, how many jump threads the reference machine is left with, and how many stitches
/// and moves show where they should not in `design`.
fn left(design: &Design, plan: &StitchPlan) -> (usize, usize, usize) {
    let astray = fabric::astray(design, plan);
    (plan.stats().jumps, astray.jumps.len(), astray.stitches.len() + astray.moves.len())
}

#[test]
fn req_auto_053_the_needle_runs_from_one_shape_to_the_next_under_a_shape_sewn_later() {
    let design = squares(true);
    let outcome = in_place(&design);
    let plan = outcome.plan.unwrap();
    assert!(!outcome.diagnostics.iter().any(|said| said.code == Code::AutoRunsLeft));
    // The red thread sews both squares without a jump between them: the plan jumps to its first stitch and
    // at its one thread change, the machine is left no jump thread, and nothing shows where it should not.
    assert_eq!((left(&design, &plan), plan.stats().color_changes), ((2, 0, 0), 1));
    // The run is the right square's, 7 stitches of 2.375 mm from where the left square's outline ends, at
    // the bar's upper edge, to 2.375 mm before the right square's first stitch, under the bar all the way.
    let run = runs(&plan);
    let id = |stitch: &SewnStitch| plan.element(plan.blocks[stitch.block].stitches[stitch.index].origin.element.unwrap()).unwrap().as_str();
    assert_eq!(run.iter().map(id).collect::<Vec<_>>(), ["right"; 7]);
    assert!(run.windows(2).all(|pair| pair[0].to == pair[1].from && pair[0].block == pair[1].block));
    let (first, last) = (run[0], run[6]);
    assert_eq!((first.from.x(), first.from.y(), first.from_role), (10.0, 8.0, Role::Top));
    assert!((last.to.x() - 26.625).abs() < 1e-9 && (7.9..8.1).contains(&last.to.y()), "{last:?}");
    assert!(run.iter().all(|stitch| (stitch.from.distance(stitch.to) - 2.375).abs() < 1e-3), "{run:?}");
    // The same design sews the same.
    assert_eq!(in_place(&design).plan.unwrap(), plan);
    // Without the bar nothing covers a way. The thread jumps from one square to the other, and so does the
    // base layer's, from its piece under one to its piece under the other, and from there back to the first:
    // 3 jump threads.
    let bare = squares(false);
    let plan = in_place(&bare).plan.unwrap();
    assert_eq!((left(&bare, &plan), runs(&plan).len(), plan.stats().color_changes), ((4, 3, 0), 0, 0));
}

#[test]
fn diag_sc_i0914_a_design_whose_runs_need_more_work_than_the_budget_sews_with_its_jumps() {
    // The grids of this design cost 1.26 million units of work, more than this budget's million, which each
    // element has to itself and needs less of.
    let design = squares(true);
    let budget = Budget { max_work: 1_000_000, ..Budget::DEFAULT };
    let outcome = plan(&design, REFERENCE, &budget);
    let said: Vec<_> = outcome.diagnostics.iter().filter(|said| said.code == Code::AutoRunsLeft).collect();
    assert_eq!(said.len(), 1, "{:?}", outcome.diagnostics);
    assert_eq!(said[0].element.as_ref().map(|id| id.as_str()), Some("stitchcraft:base"));
    let plan = outcome.plan.unwrap();
    assert_eq!((runs(&plan).len(), plan.stats().jumps), (0, 3));
}
