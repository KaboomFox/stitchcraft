//! Finalizing's conformance cases (`REQ-FIN-001..003`, and `REQ-PLAN-002`'s lock stitches) and
//! `SC-I0504`, `SC-I0703`, through the engine's entry point.

// Test code may unwrap and index (clippy.toml allows it in tests).
#![allow(clippy::unwrap_used)]

use proptest::prelude::*;
use stitchcraft_core::{Code, Mm};
use stitchcraft_engine::design::DesignSettings;
use stitchcraft_plan::invariants;
use stitchcraft_plan::profiles::BROTHER_200X200;
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
}

#[test]
fn diag_sc_i0703_split_stitches_are_counted() {
    let by_hand = [("stroke_method", "manual_stitch")];
    let two = vec![line("a", (0.0, 0.0), 30.0, &RED, &by_hand), line("b", (0.0, 10.0), 20.0, &RED, &by_hand)];
    assert_eq!(messages(&planned(two)), ["info SC-I0703: 2 stitches longer than the machine's longest stitch (12 mm) were split into equal parts."]);
}

#[test]
fn req_fin_002_designs_the_machine_cannot_take_give_no_plan() {
    // Wider than the hoop.
    let wide = planned_with(vec![line("a", (0.0, 0.0), 210.0, &RED, &[])], DesignSettings::default());
    assert_eq!((wide.plan.is_none(), wide.diagnostics.iter().map(|d| d.code).collect::<Vec<_>>()), (true, vec![Code::OutsideHoop]));
    // Small enough, but reaching past the edge from where its origin puts it.
    let aside =
        planned_with(vec![line("a", (0.0, 0.0), 120.0, &RED, &[])], DesignSettings { origin: Some(p(0.0, 0.0)), ..DesignSettings::default() });
    assert_eq!(aside.plan, None);
    assert_eq!(
        messages(&aside),
        [
            "error SC-E0701: From its origin, which goes to the hoop's centre, the design reaches 120.0 mm sideways and 0.0 mm up or down; the hoop of Brother, 200 × 200 mm hoop reaches 100 mm and 100 mm."
        ]
    );
    // Larger than the comfort zone: planned, with a warning.
    let big = planned_with(vec![line("a", (0.0, 0.0), 160.0, &RED, &[])], DesignSettings::default());
    assert_eq!((big.plan.is_some(), big.diagnostics.iter().map(|d| d.code).collect::<Vec<_>>()), (true, vec![Code::OutsideComfortZone]));
    // More colour changes than PES records (255); as many is fine.
    let threads = [RED, BLUE];
    let alternating = |n: u32| (0..n).map(|i| line(&format!("e{i}"), (0.0, f64::from(i % 50)), 5.0, &threads[(i % 2) as usize], &[])).collect();
    let many = planned_with(alternating(257), DesignSettings::default());
    assert_eq!(many.plan, None);
    assert_eq!(messages(&many), ["error SC-E0601: The design has 256 colour changes and stops, but PES v1 records at most 255."]);
    assert!(planned_with(alternating(256), DesignSettings::default()).plan.is_some());
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
            // Rows 10 mm apart, so the design stays within the comfort zone.
            elements.push(line(&format!("e{i}"), (x % 120.0 + gap, 10.0 * f64::from(u8::try_from(i).unwrap())), *length, thread, &params));
            x += gap + length;
        }
        let outcome = planned_with(elements, DesignSettings::default());
        prop_assert!(outcome.diagnostics.iter().all(|d| d.code != Code::InternalCheckFailed), "{:?}", messages(&outcome));
        let plan = outcome.plan.unwrap();
        prop_assert_eq!(invariants::check(&plan, &BROTHER_200X200), Vec::new());
    }
}
