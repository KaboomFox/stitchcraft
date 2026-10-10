//! Finalizing's conformance cases (`REQ-FIN-001..003`, and `REQ-PLAN-002`'s lock stitches) and
//! `SC-I0504`, `SC-I0703`, through the engine's entry point, and through `finalize` itself for runs of
//! stitches no generator makes.
//!
//! The cases that call `finalize` directly could be unit tests in `finalize.rs`. They are here because
//! line coverage measures the unit tests' build of a function apart from the build these tests link, and
//! takes the better of the two: a function whose lines only both kinds of test together run counts as
//! partly untested.

// Test code may unwrap and index (clippy.toml allows it in tests).
#![allow(clippy::unwrap_used)]

use proptest::prelude::*;
use stitchcraft_core::{Budget, Code, Mm, Point, Size};
use stitchcraft_engine::design::{Design, DesignSettings};
use stitchcraft_engine::finalize::finalize;
use stitchcraft_engine::plan;
use stitchcraft_plan::invariants;
use stitchcraft_plan::profiles::REFERENCE;
use stitchcraft_plan::{MachineProfile, PlanBuilder, Provenance, Rgb, Role, StitchKind, Thread};
use stitchcraft_testkit::designs::{BLUE, RED, line, messages, p, planned, planned_with, shape_of};

/// Two 10 mm lines in one thread, the second starting `gap` mm after the first ends.
fn joined(gap: f64) -> Vec<stitchcraft_engine::design::Element> {
    vec![line("a", (0.0, 0.0), 10.0, &RED, &[]), line("b", (10.0 + gap, 0.0), 10.0, &RED, &[])]
}

#[test]
fn req_fin_001_needle_points_too_close_where_elements_join_are_left_out() {
    // 0.1 mm apart: the second line's first point goes, and the stitch runs on to its second.
    let close = planned(joined(0.1));
    assert_eq!(shape_of(&close), "J L4 S9 L4");
    assert_eq!(messages(&close), ["info SC-I0504: A needle point less than the shortest stitch (0.3 mm) from the one before was left out."]);
    let plan = close.plan.unwrap();
    let sewn = plan.sewn_stitches();
    assert!(sewn.iter().all(|s| s.is_lock() || s.length() >= 0.3), "{sewn:?}");
    // Touching, and the design's own shortest stitch when it is longer than the machine's.
    assert_eq!(shape_of(&planned(joined(0.0))), "J L4 S9 L4");
    let longer = DesignSettings { min_stitch_len: Some(Mm::new(0.5).unwrap()), origin: Some(p(0.0, 0.0)), ..DesignSettings::default() };
    let outcome = planned_with(joined(0.4), longer);
    assert_eq!(messages(&outcome), ["info SC-I0504: A needle point less than the shortest stitch (0.5 mm) from the one before was left out."]);
    // Far enough apart, nothing changes.
    let apart = planned(joined(0.3));
    assert_eq!((shape_of(&apart), messages(&apart)), ("J L4 S10 L4".to_string(), Vec::<String>::new()));
}

const TOP: Role = Role::Top;
const LOCK: Role = Role::Lock;

/// One run of stitches along x, with their roles, after a jump to the first, finalized with no element's
/// shortest stitch: what finalizing keeps (`None` when the plan check refuses it) and what it says.
fn fitted(points: &[(f64, Role)]) -> (Option<Vec<(f64, Role)>>, Vec<String>) {
    let mut b = PlanBuilder::new(Thread::new(Rgb::new(0, 0, 0)));
    let at = |x: f64| Point::new(x, 0.0).unwrap();
    b.jump(at(points[0].0), Provenance::plan(Role::Travel));
    for &(x, role) in points {
        b.stitch(at(x), Provenance::plan(role));
    }
    let out = finalize(b.finish(), REFERENCE, &DesignSettings::default(), &[], &mut Budget::DEFAULT.meter()).unwrap();
    let kept = out.plan.map(|plan| plan.stitches().filter(|s| s.kind == StitchKind::Normal).map(|s| (s.at.x(), s.origin.role)).collect());
    (kept, out.diagnostics.iter().map(ToString::to_string).collect())
}

#[test]
fn req_fin_001_a_run_s_last_point_stays_and_the_one_before_it_goes() {
    let (kept, said) = fitted(&[(0.0, TOP), (5.0, TOP), (5.1, TOP)]);
    assert_eq!(kept, Some(vec![(0.0, TOP), (5.1, TOP)]));
    assert_eq!(said, ["info SC-I0504: A needle point less than the shortest stitch (0.3 mm) from the one before was left out."]);
}

#[test]
fn req_fin_001_a_lock_point_stays_and_the_points_too_close_before_it_go() {
    // The stitch into a lock point is a lock stitch, at least 0.2 mm: the point 0.1 mm before goes,
    // the one before that, 2.1 mm away, stays.
    let (kept, _) = fitted(&[(0.0, TOP), (3.0, TOP), (5.0, TOP), (5.1, LOCK), (7.0, LOCK)]);
    assert_eq!(kept, Some(vec![(0.0, TOP), (3.0, TOP), (5.1, LOCK), (7.0, LOCK)]));
    // Two points 0.3 mm apart, both within 0.2 mm of the lock point: both go.
    let (kept, said) = fitted(&[(0.0, TOP), (4.85, TOP), (5.15, TOP), (5.0, LOCK), (7.0, LOCK)]);
    assert_eq!(kept, Some(vec![(0.0, TOP), (5.0, LOCK), (7.0, LOCK)]));
    assert_eq!(said, ["info SC-I0504: 2 needle points less than the shortest stitch (0.3 mm) from the one before were left out."]);
}

#[test]
fn req_fin_001_a_point_where_the_needle_already_is_is_left_out() {
    // The run comes back to where it landed, and everything between is too close: no stitch in place.
    let (kept, said) = fitted(&[(0.0, TOP), (0.2, TOP), (0.0, TOP)]);
    assert_eq!(kept, Some(vec![(0.0, TOP)]));
    assert_eq!(said, ["info SC-I0504: 2 needle points less than the shortest stitch (0.3 mm) from the one before were left out."]);
}

#[test]
fn req_fin_003_a_run_s_first_point_always_stays() {
    // Where the needle lands is never moved: a run of two points too close is the plan check's to
    // report (a generator's bug), not finalize's to hide.
    let (kept, said) = fitted(&[(0.0, TOP), (0.1, TOP)]);
    assert_eq!(kept, None);
    assert!(said.iter().any(|s| s.starts_with("error SC-E0009")), "{said:?}");
}

#[test]
fn req_fin_001_an_element_keeps_its_own_shortest_stitch() {
    // The design's shortest stitch is 1 mm, and the element's own 0.3 mm: its stitches may be as short.
    let settings = DesignSettings { origin: Some(p(0.0, 0.0)), min_stitch_len: Some(Mm::new(1.0).unwrap()), ..DesignSettings::default() };
    // A 0.8 mm line sewn there and back: the turn stays, so the stitching does not end where it started.
    let back = [("min_stitch_length_mm", "0.3"), ("repeats", "2"), ("ties", "3")];
    let outcome = planned_with(vec![line("a", (0.0, 0.0), 0.8, &RED, &back), line("b", (0.0, 10.0), 10.0, &RED, &[])], settings);
    assert_eq!(shape_of(&outcome), "J S3 J L4 S5 L4");
    assert!(outcome.diagnostics.is_empty(), "{:?}", messages(&outcome));
    // A 5 mm line of 0.5 mm stitches, raised to twice the element's shortest stitch: nine of 0.56 mm.
    let fine = [("min_stitch_length_mm", "0.3"), ("running_stitch_length_mm", "0.5"), ("ties", "3")];
    let outcome = planned_with(vec![line("a", (0.0, 0.0), 5.0, &RED, &fine)], settings);
    let plan = outcome.plan.as_ref().unwrap();
    let lengths: Vec<f64> = plan.sewn_stitches().iter().map(|s| s.length()).collect();
    assert_eq!(lengths.len(), 9, "{lengths:?}");
    assert!(lengths.iter().all(|l| (l - 5.0 / 9.0).abs() < 1e-9), "{lengths:?}");
    assert!(outcome.diagnostics.iter().all(|d| d.code != Code::StitchesMerged), "{:?}", messages(&outcome));
}

#[test]
fn req_fin_001_stitches_longer_than_the_machine_sews_are_split() {
    // A 30 mm stitch placed by hand: three of 10 mm.
    let outcome = planned(vec![line("a", (0.0, 0.0), 30.0, &RED, &[("stroke_method", "manual_stitch")])]);
    assert_eq!(shape_of(&outcome), "J S4");
    let at: Vec<f64> = outcome.plan.as_ref().unwrap().stitches().map(|s| s.at.x()).collect();
    assert_eq!(at, [0.0, 0.0, 10.0, 20.0, 30.0]);
    assert_eq!(messages(&outcome), ["info SC-I0703: A stitch longer than the machine's longest stitch (12 mm) was split into equal parts."]);
    // A long move sewn on, when the element allows one that long.
    let far = vec![line("a", (0.0, 0.0), 10.0, &RED, &[("min_jump_stitch_length_mm", "20")]), line("b", (25.0, 0.0), 10.0, &RED, &[])];
    let outcome = planned(far);
    assert_eq!(shape_of(&outcome), "J L4 S11 L4", "the 15 mm stitch between them in two");
    assert_eq!(messages(&outcome), ["info SC-I0703: A stitch longer than the machine's longest stitch (12 mm) was split into equal parts."]);
}

#[test]
fn diag_sc_i0504_points_left_out_are_counted() {
    let three = vec![line("a", (0.0, 0.0), 10.0, &RED, &[]), line("b", (10.1, 0.0), 10.0, &RED, &[]), line("c", (20.2, 0.0), 10.0, &RED, &[])];
    assert_eq!(
        messages(&planned(three)),
        ["info SC-I0504: 2 needle points less than the shortest stitch (0.3 mm) from the one before were left out."]
    );
    // Each point is held to its own element's shortest stitch, and the message gives them all.
    let mixed = vec![
        line("a", (0.0, 0.0), 10.0, &RED, &[]),
        line("b", (10.1, 0.0), 10.0, &RED, &[("min_stitch_length_mm", "0.5")]),
        line("c", (20.2, 0.0), 10.0, &RED, &[]),
    ];
    assert_eq!(
        messages(&planned(mixed)),
        ["info SC-I0504: 2 needle points less than the shortest stitch (0.3 to 0.5 mm) from the one before were left out."]
    );
}

#[test]
fn diag_sc_i0703_split_stitches_are_counted() {
    let by_hand = [("stroke_method", "manual_stitch")];
    let two = vec![line("a", (0.0, 0.0), 30.0, &RED, &by_hand), line("b", (0.0, 10.0), 20.0, &RED, &by_hand)];
    assert_eq!(messages(&planned(two)), ["info SC-I0703: 2 stitches longer than the machine's longest stitch (12 mm) were split into equal parts."]);
}

#[test]
fn req_fin_002_designs_the_machine_cannot_take_give_no_plan() {
    // Wider than the hoop, either way round.
    let wide = planned_with(vec![line("a", (0.0, 0.0), 190.0, &RED, &[])], DesignSettings::default());
    assert_eq!(wide.plan, None);
    assert_eq!(
        messages(&wide),
        ["error SC-E0701: The design is 190.0 × 0.0 mm, but the Brother PE800 with its 5 × 7 in hoop sews at most 130 × 180 mm."],
        "the size, not the reach"
    );
    // Small enough, but reaching past the edge from where its origin puts it.
    let aside =
        planned_with(vec![line("a", (0.0, 0.0), 100.0, &RED, &[])], DesignSettings { origin: Some(p(0.0, 0.0)), ..DesignSettings::default() });
    assert_eq!(aside.plan, None);
    assert_eq!(
        messages(&aside),
        [
            "error SC-E0701: From its origin, which goes to the hoop's centre, the design reaches 100.0 mm sideways and 0.0 mm up or down, but the Brother PE800 with its 5 × 7 in hoop reaches 65 mm and 90 mm."
        ]
    );
    // A stop position past the edge: named as such, with the design inside the hoop.
    let stop = [("stop_after", "true")];
    for (wide, settings) in [
        (120.0, DesignSettings { stop_position: Some(p(60.0, 100.0)), ..DesignSettings::default() }),
        (10.0, DesignSettings { stop_position: Some(p(0.0, 100.0)), ..DesignSettings::default() }),
        (10.0, DesignSettings { stop_position: Some(p(80.0, 0.0)), ..DesignSettings::default() }),
    ] {
        let outcome = planned_with(vec![line("a", (0.0, 0.0), wide, &RED, &stop), line("b", (0.0, 5.0), 10.0, &RED, &[])], settings);
        assert_eq!(outcome.plan, None);
        assert_eq!(outcome.diagnostics.iter().map(|d| d.code).collect::<Vec<_>>(), [Code::OutsideHoop], "{:?}", messages(&outcome));
        assert!(
            messages(&outcome)[0].starts_with("error SC-E0701: The stop position, where the frame goes before each stop,"),
            "{:?}",
            messages(&outcome)
        );
    }
    // More colour changes than PES records (255); as many is fine.
    let threads = [RED, BLUE];
    let alternating = |n: u32| (0..n).map(|i| line(&format!("e{i}"), (0.0, f64::from(i % 50)), 5.0, &threads[(i % 2) as usize], &[])).collect();
    let many = planned_with(alternating(257), DesignSettings::default());
    assert_eq!(many.plan, None);
    assert_eq!(messages(&many), ["error SC-E0601: The design has 256 colour changes and stops, but PES v1 records at most 255."]);
    assert!(planned_with(alternating(256), DesignSettings::default()).plan.is_some());
}

#[test]
fn req_fin_002_a_design_beyond_the_comfort_zone_is_planned_with_a_warning() {
    // No built-in profile has a comfort zone yet: the reference machine with one of 100 × 100 mm.
    let square = Size::new(Mm::from_tenths(1000), Mm::from_tenths(1000));
    let comfort = MachineProfile { comfort: Some(square), ..REFERENCE.clone() };
    let codes = |length: f64, origin: Option<Point>| {
        let design = Design::new(vec![line("a", (0.0, 0.0), length, &RED, &[])], DesignSettings { origin, ..DesignSettings::default() }).unwrap();
        let outcome = plan(&design, &comfort, &Budget::DEFAULT);
        (outcome.plan.is_some(), outcome.diagnostics.iter().map(|d| d.code).collect::<Vec<_>>())
    };
    assert_eq!(codes(120.0, None), (true, vec![Code::OutsideComfortZone]));
    // Reaching past the edge from its origin as well: the reach is what stops it.
    assert_eq!(codes(120.0, Some(p(0.0, 0.0))), (false, vec![Code::OutsideHoop]));
}

#[test]
fn the_stitches_finalize_adds_count_against_the_budget() {
    // A 100 mm stitch placed by hand, split into nine, within a budget of 2 stitches: no plan.
    let budget = Budget { max_stitches: 2, max_work: 1_000_000 };
    let design = Design::new(vec![line("a", (-50.0, 0.0), 100.0, &RED, &[("stroke_method", "manual_stitch")])], DesignSettings::default()).unwrap();
    let outcome = plan(&design, REFERENCE, &budget);
    assert_eq!((outcome.plan, outcome.diagnostics.iter().map(|d| d.code).collect::<Vec<_>>()), (None, vec![Code::BudgetExhausted]));
}

#[test]
fn req_plan_002_a_tie_in_s_last_stitch_into_the_stitching_is_a_lock_stitch() {
    // A hand-placed stitch of 0.4 mm with forced locks: the half stitch is 0.2 mm, into its first point too.
    let tiny = planned(vec![line("a", (0.0, 0.0), 0.4, &RED, &[("stroke_method", "manual_stitch"), ("force_lock_stitches", "true")])]);
    assert_eq!(shape_of(&tiny), "J L4 S2 L4");
    assert!(tiny.diagnostics.is_empty(), "{:?}", messages(&tiny));
}

proptest! {
    #![proptest_config(stitchcraft_testkit::strategies::config(128))]

    /// Any row of strokes, running or by hand, with any locks, gaps, trims and stops, planned within the
    /// hoop: there is a plan, and it passes every plan invariant for the machine (no `SC-E0009`).
    #[test]
    fn req_fin_003_every_plan_returned_passes_the_invariants(
        strokes in prop::collection::vec((0.0..4.0_f64, 0.3..40.0_f64, any::<bool>(), any::<bool>(), 0..4_u8, any::<bool>(), any::<bool>(), any::<bool>()), 1..8),
    ) {
        let (mut x, mut elements) = (0.0, Vec::new());
        for (i, (gap, length, blue, manual, ties, force, trim, stop)) in strokes.iter().enumerate() {
            let thread = if *blue { &BLUE } else { &RED };
            let method = if *manual { "manual_stitch" } else { "running_stitch" };
            let (ties, force, trim, stop) = (ties.to_string(), force.to_string(), trim.to_string(), stop.to_string());
            let params = [("stroke_method", method), ("ties", ties.as_str()), ("force_lock_stitches", force.as_str()), ("trim_after", trim.as_str()), ("stop_after", stop.as_str())];
            // Rows 10 mm apart, starting within 84 mm of the left: the design fits the 130 mm hoop.
            elements.push(line(&format!("e{i}"), (x % 80.0 + gap, 10.0 * f64::from(u8::try_from(i).unwrap())), *length, thread, &params));
            x += gap + length;
        }
        let outcome = planned_with(elements, DesignSettings::default());
        prop_assert!(outcome.diagnostics.iter().all(|d| d.code != Code::InternalCheckFailed), "{:?}", messages(&outcome));
        let plan = outcome.plan.unwrap();
        prop_assert_eq!(invariants::check(&plan, REFERENCE), Vec::new());
    }
}
