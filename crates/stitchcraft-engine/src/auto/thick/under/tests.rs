// Test data: small counts of rows, from sizes given in whole steps.
#![allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]

use stitchcraft_core::{Code, ElementId, Point};
use stitchcraft_plan::{PlanBuilder, Provenance, Rgb, Role, StitchPlan, Thread};

use super::{threadings, under};
use crate::PlanOutcome;
use crate::names::Names;

const NAVY: Rgb = Rgb::new(29, 53, 87);
const CREAM: Rgb = Rgb::new(244, 232, 201);

/// A plan of a zigzag 4 mm across, its needle points `apart` above each other, of `0.25 / apart` fills, in
/// each of `threads` in turn, one block each, 10 mm to the right of the one before.
fn zigzag(apart: f64, threads: &[Rgb]) -> StitchPlan {
    let mut builder = PlanBuilder::new(Thread::new(threads[0]));
    let element = builder.element(ElementId::new("shape").unwrap()).unwrap();
    for (at, color) in threads.iter().enumerate() {
        if at > 0 {
            builder.change_thread(Thread::new(*color));
        }
        let count = (4.0 / apart).round() as usize;
        let left = at as f64 * 10.0;
        builder.jump(Point::new(left, 0.0).unwrap(), Provenance::new(element, Role::Travel));
        for k in 0..=count {
            let x = left + if k % 2 == 0 { 0.0 } else { 4.0 };
            builder.stitch(Point::new(x, k as f64 * apart).unwrap(), Provenance::new(element, Role::Top));
        }
    }
    builder.finish()
}

/// `plan` as planning gives it back.
fn outcome(plan: StitchPlan) -> PlanOutcome {
    PlanOutcome { plan: Some(plan), diagnostics: Vec::new(), names: Names::default() }
}

fn codes(outcome: &PlanOutcome) -> Vec<Code> {
    outcome.diagnostics.iter().map(|said| said.code).collect()
}

#[test]
fn req_auto_189_a_design_threaded_once_above_the_limit_sews_strictly_when_that_is_under_it() {
    // Threaded once: 6.08 fills in navy. With each line after its fills: 1.34 fills, navy threaded twice.
    let once = || outcome(zigzag(0.05, &[NAVY]));
    let strict = || (outcome(zigzag(0.25, &[NAVY, CREAM, NAVY])), Some(1.34));
    let kept = under(once(), (4.0, 6.08), strict);
    assert_eq!(kept.plan, strict().0.plan);
    assert_eq!(codes(&kept), [Code::AutoThreadedForThickness]);
    // Both above the limit, or the strict plan not made or not measured: the plan threaded once comes back.
    assert_eq!(under(once(), (4.0, 6.08), || (outcome(zigzag(0.05, &[NAVY, CREAM])), Some(4.5))), once());
    assert_eq!(under(once(), (4.0, 6.08), || (PlanOutcome { plan: None, ..once() }, Some(1.0))), once());
    assert_eq!(under(once(), (4.0, 6.08), || (outcome(zigzag(0.25, &[NAVY])), None)), once());
    // At the limit or under it, the design isn't planned again.
    assert_eq!(under(once(), (4.0, 4.0), || panic!("planned again")), once());
    assert_eq!(under(PlanOutcome { plan: None, ..once() }, (4.0, 6.08), || panic!("planned again")), PlanOutcome { plan: None, ..once() });
}

#[test]
fn diag_sc_w0927_says_how_many_more_times_the_threads_are_threaded() {
    let kept = under(outcome(zigzag(0.05, &[NAVY])), (4.0, 6.08), || (outcome(zigzag(0.25, &[NAVY, CREAM, NAVY])), Some(1.34)));
    let said = kept.diagnostics.iter().find(|said| said.code == Code::AutoThreadedForThickness).unwrap();
    assert_eq!(
        said.message,
        "Threaded once, a spot of the design sews 6.08 fills, above the limit of 4. Sewn with each line after the fills it borders, the \
         design sews 1.34 fills at the most, so it sews that way: its threads are threaded 2 more times, 3 times in all."
    );
}

#[test]
fn a_stop_is_no_threading() {
    assert_eq!(threadings(&zigzag(0.25, &[NAVY, NAVY, CREAM, NAVY])), 3);
    assert_eq!(threadings(&StitchPlan::default()), 0);
}
