use stitchcraft_core::{Code, Point};
use stitchcraft_plan::profiles::REFERENCE;
use stitchcraft_plan::{PlanBuilder, Provenance, Rgb, StitchKind, Thread};

use super::*;
use crate::pipeline::after_finalize;

fn p(x: f64) -> Point {
    Point::new(x, 0.0).unwrap()
}

/// A plan of element `e`: a run with both locks, ended by a trim; a run with none, ended by a jump; one
/// with none after a stop, ended by the thread change; and one with none in the second thread, ended by the
/// plan's end.
fn plan() -> StitchPlan {
    let mut b = PlanBuilder::new(Thread::new(Rgb::new(0, 0, 0)));
    let e = b.element(ElementId::new("e").unwrap()).unwrap();
    let (travel, lock, top) = (Provenance::new(e, Role::Travel), Provenance::new(e, Role::Lock), Provenance::new(e, Role::Top));
    b.jump(p(0.0), travel); // 0
    b.stitch(p(0.0), lock); // 1
    b.stitch(p(1.0), top); // 2
    b.stitch(p(2.0), top); // 3
    b.stitch(p(3.0), lock); // 4
    b.trim(Some(e)); // 5
    b.jump(p(10.0), travel); // 6
    b.stitch(p(10.0), top); // 7
    b.stitch(p(12.0), top); // 8
    b.stop(Some(e)); // 9
    b.stitch(p(14.0), top); // 10
    b.stitch(p(16.0), top); // 11
    b.change_thread(Thread::new(Rgb::new(200, 0, 0)));
    b.jump(p(20.0), travel); // 0
    b.stitch(p(20.0), top); // 1
    b.stitch(p(22.0), top); // 2
    b.finish()
}

fn asking(tie_in: bool, tie_off: bool) -> BTreeMap<ElementId, (bool, bool)> {
    BTreeMap::from([(ElementId::new("e").unwrap(), (tie_in, tie_off))])
}

/// Where the check finds a lock missing, as (block, entry).
fn missing(plan: &StitchPlan, asked: &BTreeMap<ElementId, (bool, bool)>) -> Vec<(usize, usize)> {
    locks(plan, asked).iter().map(|v| (v.block, v.stitch.unwrap())).collect()
}

#[test]
fn req_plan_008_each_run_has_the_locks_its_element_asks_for() {
    let plan = plan();
    // A stop ends a run (REQ-PLAN-010), and so do a jump, a thread change and the end.
    assert_eq!(missing(&plan, &asking(true, false)), [(0, 7), (0, 10), (1, 1)]);
    assert_eq!(missing(&plan, &asking(false, true)), [(0, 8), (0, 11), (1, 2)]);
    assert_eq!(missing(&plan, &asking(true, true)), [(0, 7), (0, 8), (0, 10), (0, 11), (1, 1), (1, 2)]);
    assert_eq!(missing(&plan, &asking(false, false)), []);
    assert_eq!(missing(&plan, &BTreeMap::new()), [], "an element with no settings asks for nothing");
    let said: Vec<String> = locks(&plan, &asking(true, true)).iter().take(2).map(ToString::to_string).collect();
    assert_eq!(
        said,
        [
            "REQ-PLAN-008 at block 0, entry 7: the run starts without the tie-in that element `e` asks for",
            "REQ-PLAN-008 at block 0, entry 8: the run ends without the tie-off that element `e` asks for",
        ]
    );
}

#[test]
fn req_plan_008_an_entry_that_names_no_element_asks_for_nothing() {
    let mut b = PlanBuilder::new(Thread::new(Rgb::new(0, 0, 0)));
    b.element(ElementId::new("e").unwrap()).unwrap();
    b.jump(p(0.0), Provenance::plan(Role::Travel));
    b.stitch(p(0.0), Provenance::plan(Role::Top));
    b.stitch(p(2.0), Provenance::plan(Role::Top));
    assert_eq!(missing(&b.finish(), &asking(true, true)), []);
}

#[test]
fn req_plan_008_reports_are_capped() {
    let mut b = PlanBuilder::new(Thread::new(Rgb::new(0, 0, 0)));
    let e = b.element(ElementId::new("e").unwrap()).unwrap();
    for i in 0..60 {
        b.jump(p(f64::from(i)), Provenance::new(e, Role::Travel));
        b.stitch(p(f64::from(i)), Provenance::new(e, Role::Top));
    }
    assert_eq!(locks(&b.finish(), &asking(true, false)).len(), invariants::MAX_REPORTED);
}

#[test]
fn asked_takes_each_elements_first_setting() {
    use stitchcraft_params::StitchType;

    use crate::common::CommonParams;
    use crate::generators::Layer;
    let generated = |ties: &'static str, stitch_type: StitchType| Generated {
        common: CommonParams { ties, ..CommonParams::from_set(&stitchcraft_params::ParamSet::new()).unwrap().params },
        stitch_type,
        groups: Vec::new(),
        colours: Vec::new(),
        min_stitch: REFERENCE.min_stitch,
        layer: Layer::Whole,
        underlay_points: 0,
    };
    let (a, b) = (ElementId::new("a").unwrap(), ElementId::new("b").unwrap());
    let (first, second, by_hand) =
        (generated("1", StitchType::RunningStitch), generated("2", StitchType::RunningStitch), generated("0", StitchType::ManualStitch));
    let asked = asked([(&a, &first), (&a, &second), (&b, &by_hand)]);
    assert_eq!(asked, BTreeMap::from([(a, (true, false)), (b, (false, false))]));
}

/// The plan the engine returns is checked after the cap (`REQ-FIN-003`, `REQ-PLAN-008`). Finalize checked
/// the plan before it; a stand-in for the cap breaks it after, and the plan is not returned.
#[test]
fn req_fin_003_the_plan_is_checked_after_the_cap() {
    let checked = |cap: &dyn Fn(&mut StitchPlan)| {
        let mut said = Vec::new();
        let plan = after_finalize(
            plan(),
            |plan| {
                cap(plan);
                vec![Diagnostic::new(Code::AutoThinned, "thinned")]
            },
            REFERENCE,
            &asking(false, false),
            &mut said,
        );
        (plan, said.iter().map(|d| (d.code, d.message.clone())).collect::<Vec<_>>())
    };
    let thinned = (Code::AutoThinned, "thinned".to_string());
    assert_eq!(checked(&|_| {}), (Some(plan()), vec![thinned.clone()]), "a plan the cap leaves valid is returned");
    // A cap that leaves a 0.1 mm stitch, shorter than the machine's shortest.
    let (short, said) = checked(&|plan| plan.blocks[0].stitches[3].at = p(1.1));
    assert_eq!(short, None);
    let refused = (
        Code::InternalCheckFailed,
        "The stitch plan breaks REQ-PLAN-002 at block 0, entry 3: the stitch is 0.100 mm, shorter than 0.3 mm".to_string(),
    );
    assert_eq!(said, [thinned.clone(), refused]);
    // A cap that takes a tie-in's lock point out, where the element asks for a tie-in.
    let mut said = Vec::new();
    let strip = |plan: &mut StitchPlan| {
        plan.blocks[0].stitches.remove(1);
        Vec::new()
    };
    assert_eq!(after_finalize(plan(), strip, REFERENCE, &asking(true, false), &mut said), None);
    let codes: Vec<Code> = said.iter().map(|d| d.code).collect();
    assert_eq!(codes, [Code::InternalCheckFailed; 4]);
    assert!(said[0].message.ends_with("block 0, entry 1: the run starts without the tie-in that element `e` asks for"), "{}", said[0].message);
    assert_eq!(plan().blocks[0].stitches[1].kind, StitchKind::Normal);
}
