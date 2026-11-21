//! The thread's state in the plan's checks (M13.56): the cases of `REQ-PLAN-008` to `REQ-PLAN-010`, planned.
//! Design: `docs/src/design/data-model.md` › The thread's state. Each case is ignored until the step that
//! builds its check makes the requirement active.

// Test code may unwrap and index (clippy.toml allows it in tests).
#![allow(clippy::unwrap_used, clippy::indexing_slicing)]

use stitchcraft_core::ElementId;
use stitchcraft_engine::design::Element;
use stitchcraft_plan::invariants::check;
use stitchcraft_plan::profiles::REFERENCE;
use stitchcraft_plan::{PlanBuilder, Provenance, Role, StitchKind, StitchPlan};
use stitchcraft_testkit::designs::{BLUE, RED, line, p, planned};

/// Each run of `plan`, the needle points sewn between 2 places where the thread may be cut (the plan's start,
/// a thread change, a jump, a trim, a stop, the end), as the element of its first and last points and
/// whether each of those is a lock stitch.
fn runs(plan: &StitchPlan) -> Vec<(String, bool, String, bool)> {
    let mut runs = Vec::new();
    for block in &plan.blocks {
        let mut run: Vec<&stitchcraft_plan::Stitch> = Vec::new();
        for stitch in &block.stitches {
            if stitch.kind == StitchKind::Normal {
                run.push(stitch);
                continue;
            }
            runs.extend(ends(plan, &run));
            run.clear();
        }
        runs.extend(ends(plan, &run));
    }
    runs
}

fn ends(plan: &StitchPlan, run: &[&stitchcraft_plan::Stitch]) -> Option<(String, bool, String, bool)> {
    let (first, last) = (run.first()?, run.last()?);
    let name = |stitch: &stitchcraft_plan::Stitch| plan.element(stitch.origin.element.unwrap()).unwrap().as_str().to_string();
    Some((name(first), first.origin.role == Role::Lock, name(last), last.origin.role == Role::Lock))
}

#[test]
fn req_plan_008_each_run_starts_and_ends_with_the_locks_its_element_asks_for() {
    // Each element alone, and after one in another thread, with a trim and with a stop: every place the
    // thread may be cut. The check builds this walk into the engine, after the cap, with each element's
    // `ties`, `force_lock_stitches` and stitch type. Assembly makes these locks today, so this case passes:
    // the requirement is planned until the check guards them in every returned plan.
    let asked = |params: &[(&str, &str)]| -> (bool, bool) {
        let ties = params.iter().find(|(key, _)| *key == "ties").map_or("0", |(_, value)| *value);
        let forced = params.contains(&("force_lock_stitches", "true"));
        let manual = params.contains(&("stroke_method", "manual_stitch")) && !forced;
        (matches!(ties, "0" | "1") && !manual, (matches!(ties, "0" | "2") || forced) && !manual)
    };
    let settings: [&[(&str, &str)]; 7] = [
        &[],
        &[("ties", "1")],
        &[("ties", "2")],
        &[("ties", "3")],
        &[("ties", "3"), ("force_lock_stitches", "true")],
        &[("stroke_method", "manual_stitch")],
        &[("trim_after", "true"), ("stop_after", "true")],
    ];
    for params in settings {
        let elements: Vec<Element> =
            vec![line("a", (0.0, 0.0), 10.0, &RED, params), line("b", (0.0, 10.0), 10.0, &RED, params), line("c", (0.0, 20.0), 10.0, &BLUE, params)];
        let plan = planned(elements).plan.unwrap();
        let (tie_in, tie_off) = asked(params);
        for (first, locked_in, last, locked_off) in runs(&plan) {
            assert_eq!(locked_in, tie_in, "{params:?}: the run of {first} starts with a tie-in only if asked");
            assert_eq!(locked_off, tie_off, "{params:?}: the run of {last} ends with a tie-off only if asked");
        }
    }
}

/// A plan of one line, `e`, from a jump to its start: `S` a stitch 2 mm on, `T` a trim, `P` a stop.
fn sewn(entries: &str) -> StitchPlan {
    let mut builder = PlanBuilder::new(RED);
    let e = builder.element(ElementId::new("e").unwrap()).unwrap();
    builder.jump(p(0.0, 0.0), Provenance::new(e, Role::Travel));
    let mut x = 0.0;
    for entry in entries.chars() {
        match entry {
            'S' => {
                builder.stitch(p(x, 0.0), Provenance::new(e, Role::Top));
                x += 2.0;
            }
            'T' => builder.trim(Some(e)),
            'P' => builder.stop(Some(e)),
            _ => {}
        }
    }
    builder.finish()
}

#[test]
fn req_plan_009_a_trim_cuts_a_thread_that_has_sewn_since_the_last_cut() {
    let broken = |plan: &StitchPlan| check(plan, REFERENCE).iter().filter(|v| v.requirement == "REQ-PLAN-009").count();
    assert_eq!(broken(&sewn("SSST")), 0);
    assert_eq!(broken(&sewn("SSSTP")), 0, "a stop after a trim cuts again, as trim_after and stop_after sew");
    assert_eq!(broken(&sewn("SSSTT")), 1, "2 trims in a row");
    // A trim before anything is sewn, and a block that starts with a trim.
    let mut first = sewn("SSS");
    let jump = first.blocks[0].stitches[0];
    first.blocks[0].stitches.insert(0, jump);
    first.blocks[0].stitches[0].kind = StitchKind::Trim;
    first.blocks[0].stitches[0].at = p(0.0, 0.0);
    assert_eq!(broken(&first), 1, "a trim as a block's first entry");
}

#[test]
fn req_plan_010_a_stop_ends_the_run_as_the_machine_cuts_there() {
    // The PE800 cuts the thread at a stop (MC-1), which PES and DST write as a thread change: the stitch
    // after it starts a new run and lays no thread from the hole before the stop.
    let plan = sewn("SSPSS");
    let across: Vec<usize> = plan.sewn_stitches().iter().map(|stitch| stitch.index).collect();
    assert_eq!(across, [2, 5], "the entries sewn from a hole: none across the stop at entry 3");
}
