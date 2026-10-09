//! Plan assembly's conformance cases (`REQ-ASM-001`, `-002`, `-003`, `-005`, `REQ-LCK-001`, `REQ-GEN-002`)
//! and `SC-W0011`, through the engine's entry point, [`plan`].

// Test code may unwrap and index (clippy.toml allows it in tests).
#![allow(clippy::unwrap_used)]

use proptest::prelude::*;
use stitchcraft_core::{Budget, Code, ElementId, Mm, Rect};
use stitchcraft_engine::design::{Design, DesignSettings, Element, FillRule, Path, Segment, Shape, Subpath};
use stitchcraft_engine::plan;
use stitchcraft_plan::profiles::BROTHER_200X200;
use stitchcraft_plan::{Rgb, Role, StitchKind};
use stitchcraft_testkit::designs::{BLUE, RED, line, messages, p, planned as sewn, planned_with as sewn_with, shape, shape_of, stroke};

#[test]
fn req_asm_001_elements_are_sewn_in_document_order_a_new_thread_a_new_block() {
    // 10 mm lines, 2.5 mm stitches: five needle points each, and a half stitch of four at either end.
    let one = sewn(vec![line("a", (0.0, 0.0), 10.0, &RED, &[])]);
    assert_eq!(shape_of(&one), "J L4 S5 L4");
    assert!(one.diagnostics.is_empty(), "{:?}", messages(&one));
    let three = sewn(vec![line("a", (0.0, 0.0), 10.0, &RED, &[]), line("b", (0.0, 5.0), 10.0, &BLUE, &[]), line("c", (0.0, 10.0), 10.0, &RED, &[])]);
    let plan = three.plan.unwrap();
    assert_eq!(shape(&plan), "J L4 S5 L4 | J L4 S5 L4 | J L4 S5 L4");
    assert_eq!(plan.blocks.iter().map(|b| b.thread.color).collect::<Vec<_>>(), [RED.color, BLUE.color, RED.color], "never reordered");
    assert_eq!(plan.elements.iter().map(ElementId::as_str).collect::<Vec<_>>(), ["a", "b", "c"]);
    let first = plan.blocks[0].stitches.iter().find(|s| s.origin.role == Role::Top).unwrap();
    assert_eq!((first.at, plan.element(first.origin.element.unwrap()).unwrap().as_str()), (p(0.0, 0.0), "a"));
    // A thread change is a change of colour: two names for one colour are one block, as in Ink/Stitch.
    let scarlet = Thread::named(RED.color, "Scarlet");
    let renamed = sewn(vec![line("a", (0.0, 0.0), 10.0, &RED, &[]), line("b", (0.0, 5.0), 10.0, &scarlet, &[])]).plan.unwrap();
    assert_eq!(shape(&renamed), "J L4 S5 L4 J L4 S5 L4");
    assert_eq!(renamed.blocks[0].thread, RED, "the block keeps its first thread");
}

#[test]
fn req_asm_002_close_groups_are_sewn_on_far_ones_tie_off_jump_and_tie_in() {
    let pair = |gap: f64, first: &[(&str, &str)], settings: DesignSettings| {
        let elements = vec![line("a", (0.0, 0.0), 10.0, &RED, first), line("b", (10.0 + gap, 0.0), 10.0, &RED, &[])];
        shape_of(&sewn_with(elements, DesignSettings { origin: Some(p(0.0, 0.0)), ..settings }))
    };
    let default = DesignSettings::default();
    assert_eq!(pair(2.0, &[], default), "J L4 S10 L4", "within the 3 mm collapse length: sewn on, no locks between");
    assert_eq!(pair(3.0, &[], default), "J L4 S10 L4", "exactly the collapse length");
    assert_eq!(pair(5.0, &[], default), "J L4 S5 L4 J L4 S5 L4");
    assert_eq!(pair(5.0, &[("min_jump_stitch_length_mm", "6")], default), "J L4 S10 L4", "the element's own limit");
    assert_eq!(pair(2.0, &[("min_jump_stitch_length_mm", "1")], default), "J L4 S5 L4 J L4 S5 L4");
    assert_eq!(pair(2.0, &[("min_jump_stitch_length_mm", "0")], default), "J L4 S10 L4", "0 counts as not set, as in Ink/Stitch");
    assert_eq!(pair(2.0, &[("force_lock_stitches", "true")], default), "J L4 S5 L4 J L4 S5 L4", "forced locks always jump");
    let short = DesignSettings { collapse_len: Mm::new(1.0).unwrap(), ..default };
    assert_eq!(pair(2.0, &[], short), "J L4 S5 L4 J L4 S5 L4", "the design's collapse length");
    // The parts of one element are groups too: two subpaths 5 mm apart.
    let parts = stroke("a", &[((0.0, 0.0), 10.0), ((15.0, 0.0), 10.0)], &RED, &[]);
    assert_eq!(shape_of(&sewn(vec![parts])), "J L4 S5 L4 J L4 S5 L4");
}

#[test]
fn req_asm_003_trims_and_stops_come_after_the_element_with_locks_around_them() {
    let pair = |first: &[(&str, &str)], settings: DesignSettings| {
        let elements = vec![line("a", (0.0, 0.0), 10.0, &RED, first), line("b", (12.0, 0.0), 10.0, &RED, &[])];
        sewn_with(elements, DesignSettings { origin: Some(p(0.0, 0.0)), ..settings })
    };
    let default = DesignSettings::default();
    assert_eq!(shape_of(&pair(&[("trim_after", "true")], default)), "J L4 S5 L4 T J L4 S5 L4", "close, but trimmed");
    assert_eq!(shape_of(&pair(&[("stop_after", "true")], default)), "J L4 S5 L4 P J L4 S5 L4");
    assert_eq!(shape_of(&pair(&[("trim_after", "true"), ("stop_after", "true")], default)), "J L4 S5 L4 T P J L4 S5 L4");
    assert_eq!(shape_of(&pair(&[("trim_after", "true"), ("ties", "3")], default)), "J S5 T J L4 S5 L4", "as `ties` allows");
    let stopped = pair(&[("stop_after", "true")], DesignSettings { stop_position: Some(p(50.0, 60.0)), ..default });
    assert_eq!(shape_of(&stopped), "J L4 S5 L4 J P J L4 S5 L4", "to the stop position first");
    let plan = stopped.plan.unwrap();
    let stop = plan.blocks[0].stitches.iter().position(|s| s.kind == StitchKind::Stop).unwrap();
    assert_eq!((plan.blocks[0].stitches[stop - 1].at, plan.blocks[0].stitches[stop].at), (p(50.0, 60.0), p(50.0, 60.0)));
    // Only the element's last group is followed by its trim.
    let parts = stroke("a", &[((0.0, 0.0), 10.0), ((15.0, 0.0), 10.0)], &RED, &[("trim_after", "true")]);
    assert_eq!(shape_of(&sewn(vec![parts])), "J L4 S5 L4 J L4 S5 L4 T");
}

#[test]
fn req_asm_005_the_origin_goes_to_the_hoop_centre() {
    let elements = || vec![line("a", (100.0, 50.0), 10.0, &RED, &[]), line("b", (100.0, 70.0), 30.0, &RED, &[])];
    // Without an origin: the centre of the box around the stitches.
    let centred = sewn_with(elements(), DesignSettings::default()).plan.unwrap();
    let sewn_box = Rect::around(centred.stitches().filter(|s| s.kind == StitchKind::Normal).map(|s| s.at)).unwrap();
    assert!(sewn_box.center().distance(p(0.0, 0.0)) < 1e-9, "{sewn_box:?}");
    // With one: that point is at the centre, and the stop position moves with everything else.
    let settings = DesignSettings { origin: Some(p(100.0, 50.0)), stop_position: Some(p(90.0, 40.0)), ..DesignSettings::default() };
    let mut stopping = elements();
    stopping[0].params.set("stop_after", "true");
    let plan = sewn_with(stopping, settings).plan.unwrap();
    let first = plan.stitches().find(|s| s.origin.role == Role::Top).unwrap();
    assert_eq!(first.at, p(0.0, 0.0));
    let stop = plan.stitches().find(|s| s.kind == StitchKind::Stop).unwrap();
    assert_eq!(stop.at, p(-10.0, -10.0));
}

#[test]
fn req_lck_001_locks_go_where_ties_says_and_only_there() {
    let pair = |params: &[(&str, &str)]| shape_of(&sewn(vec![line("a", (0.0, 0.0), 10.0, &RED, params), line("b", (20.0, 0.0), 10.0, &RED, params)]));
    assert_eq!(pair(&[("ties", "0")]), "J L4 S5 L4 J L4 S5 L4");
    assert_eq!(pair(&[("ties", "1")]), "J L4 S5 J L4 S5", "before: tie-ins only");
    assert_eq!(pair(&[("ties", "2")]), "J S5 L4 J S5 L4", "after: tie-offs only");
    assert_eq!(pair(&[("ties", "3")]), "J S5 J S5");
    assert_eq!(pair(&[("ties", "3"), ("force_lock_stitches", "true")]), "J S5 L4 J S5 L4", "forcing adds the tie-off, never a tie-in");
    // Manual stitch: none unless forced, and then as `ties` says.
    let manual = [("stroke_method", "manual_stitch")];
    assert_eq!(pair(&manual), "J S2 J S2");
    assert_eq!(pair(&[manual[0], ("force_lock_stitches", "true")]), "J L4 S2 L4 J L4 S2 L4");
    assert_eq!(pair(&[manual[0], ("force_lock_stitches", "true"), ("ties", "1")]), "J L4 S2 L4 J L4 S2 L4");
    // Groups sewn on from one to the next get none between them.
    let close = sewn(vec![line("a", (0.0, 0.0), 10.0, &RED, &[]), line("b", (12.0, 0.0), 10.0, &RED, &[]), line("c", (24.0, 0.0), 10.0, &RED, &[])]);
    assert_eq!(shape_of(&close), "J L4 S15 L4");
}

#[test]
fn req_gen_002_an_element_that_cannot_be_sewn_is_skipped_and_the_rest_still_plans() {
    let square =
        Path { subpaths: vec![Subpath { start: p(0.0, 0.0), segments: vec![Segment::Line(p(5.0, 0.0)), Segment::Line(p(5.0, 5.0))], closed: true }] };
    let fill = Element { shape: Shape::Fill { path: square, rule: FillRule::NonZero }, ..line("fill", (0.0, 0.0), 1.0, &RED, &[]) };
    let ripple = line("ripple", (0.0, 5.0), 10.0, &RED, &[("stroke_method", "ripple_stitch")]);
    let wrong = line("wrong", (0.0, 10.0), 10.0, &RED, &[("running_stitch_length_mm", "long")]);
    let fine = line("fine", (0.0, 15.0), 10.0, &RED, &[]);
    let outcome = sewn(vec![fill, ripple, wrong, fine]);
    assert_eq!(shape_of(&outcome), "J L4 S5 L4");
    assert_eq!(outcome.plan.unwrap().elements.iter().map(ElementId::as_str).collect::<Vec<_>>(), ["fine"]);
    let codes: Vec<(Code, Option<&str>)> = outcome.diagnostics.iter().map(|d| (d.code, d.element.as_ref().map(ElementId::as_str))).collect();
    assert_eq!(codes, [(Code::StitchTypeNotYet, Some("fill")), (Code::StitchTypeNotYet, Some("ripple")), (Code::ParamInvalid, Some("wrong"))]);
    // Each element has the work budget to itself: a long one runs out, a short one does not.
    let budget = Budget { max_stitches: 1000, max_work: 60 };
    let design =
        Design::new(vec![line("long", (0.0, 0.0), 150.0, &RED, &[]), line("short", (0.0, 5.0), 5.0, &RED, &[])], DesignSettings::default()).unwrap();
    let outcome = plan(&design, &BROTHER_200X200, &budget);
    assert_eq!(outcome.plan.unwrap().elements.iter().map(ElementId::as_str).collect::<Vec<_>>(), ["short"]);
    assert_eq!(
        outcome.diagnostics.iter().map(|d| (d.code, d.element.as_ref().map(ElementId::as_str))).collect::<Vec<_>>(),
        [(Code::BudgetExhausted, Some("long"))]
    );
    // Nothing left to sew: no plan.
    let nothing = sewn(vec![line("tiny", (0.0, 0.0), 0.1, &RED, &[])]);
    assert_eq!(nothing.plan, None);
    assert_eq!(
        messages(&nothing),
        [
            "warning SC-W0401: A part of the stroke is 0.1 mm long, shorter than the shortest stitch (0.3 mm), so it is not stitched.",
            "error SC-E0010: The design has nothing to stitch: it has no elements, or every one was skipped."
        ]
    );
    // The design's stitch limit holds for the whole plan.
    let few = Budget { max_stitches: 10, max_work: 1_000_000 };
    let design = Design::new(vec![line("a", (0.0, 0.0), 30.0, &RED, &[])], DesignSettings::default()).unwrap();
    let outcome = plan(&design, &BROTHER_200X200, &few);
    assert_eq!((outcome.plan, outcome.diagnostics[0].code), (None, Code::BudgetExhausted));
}

#[test]
fn diag_sc_w0011_a_stitch_type_not_sewn_yet_is_named() {
    let ripple = sewn(vec![line("r", (0.0, 0.0), 10.0, &RED, &[("stroke_method", "zigzag_stitch")]), line("ok", (0.0, 5.0), 10.0, &RED, &[])]);
    assert_eq!(
        messages(&ripple),
        ["warning SC-W0011: This element's stroke method, `zigzag_stitch`, is not sewn by this version of StitchCraft yet, so it is skipped."]
    );
    let square = Path { subpaths: vec![Subpath { start: p(0.0, 0.0), segments: vec![Segment::Line(p(5.0, 5.0))], closed: true }] };
    let fill = Element { shape: Shape::Fill { path: square, rule: FillRule::EvenOdd }, ..line("f", (0.0, 0.0), 1.0, &RED, &[]) };
    let outcome = sewn(vec![fill, line("ok", (0.0, 5.0), 10.0, &RED, &[])]);
    assert_eq!(
        messages(&outcome),
        ["warning SC-W0011: This element is a fill, and this version of StitchCraft does not sew fills yet, so it is skipped."]
    );
    assert_eq!(outcome.diagnostics[0].element.as_ref().map(ElementId::as_str), Some("f"));
}

#[test]
fn diag_sc_w0505_a_trim_or_stop_after_an_element_that_sews_nothing_is_named() {
    // Too small for a stitch, a stitch type not sewn yet, and a stop alone: none of them sews, so their
    // trims and stops have nothing to come after.
    let tiny = line("tiny", (0.0, 0.0), 0.1, &RED, &[("trim_after", "true")]);
    let ripple = line("ripple", (0.0, 5.0), 10.0, &RED, &[("stroke_method", "ripple_stitch"), ("trim_after", "true"), ("stop_after", "true")]);
    let stop = line("stop", (0.0, 10.0), 0.1, &RED, &[("stop_after", "true")]);
    let outcome = sewn(vec![tiny, ripple, stop, line("ok", (0.0, 15.0), 10.0, &RED, &[])]);
    assert_eq!(shape_of(&outcome), "J L4 S5 L4");
    let left_out: Vec<(String, Option<&str>)> = outcome
        .diagnostics
        .iter()
        .filter(|d| d.code == Code::TrimOrStopLeftOut)
        .map(|d| (d.to_string(), d.element.as_ref().map(ElementId::as_str)))
        .collect();
    assert_eq!(
        left_out,
        [
            ("warning SC-W0505: This element sews no stitch, so the trim after it is left out.".to_string(), Some("tiny")),
            ("warning SC-W0505: This element sews no stitch, so the trim and the stop after it are left out.".to_string(), Some("ripple")),
            ("warning SC-W0505: This element sews no stitch, so the stop after it is left out.".to_string(), Some("stop")),
        ]
    );
    // An element that sews nothing and sets neither says nothing more.
    let quiet = sewn(vec![line("tiny", (0.0, 0.0), 0.1, &RED, &[]), line("ok", (0.0, 15.0), 10.0, &RED, &[])]);
    assert!(quiet.diagnostics.iter().all(|d| d.code != Code::TrimOrStopLeftOut));
}

#[test]
fn lock_warnings_name_their_element_and_unknown_settings_are_reported() {
    let outcome = sewn(vec![line("a", (0.0, 0.0), 10.0, &RED, &[("lock_start", "custom"), ("sparkle", "yes")])]);
    let named: Vec<(Code, Option<&str>)> = outcome.diagnostics.iter().map(|d| (d.code, d.element.as_ref().map(ElementId::as_str))).collect();
    assert_eq!(named, [(Code::ParamUnknown, Some("a")), (Code::CustomLockUnusable, Some("a"))]);
}

proptest! {
    #![proptest_config(stitchcraft_testkit::strategies::config(128))]

    /// Any row of strokes in two threads, with any locks, gaps, trims and stops: every jump lands where the
    /// needle goes down next, the threads change only where the elements' do, every element with a
    /// stitch is in the plan, and its stitches are centred on the hoop.
    #[test]
    fn plans_are_well_formed(
        strokes in prop::collection::vec((0.0..8.0_f64, 1.0..15.0_f64, any::<bool>(), 0..4_u8, any::<bool>(), any::<bool>()), 1..8),
    ) {
        let mut x = 0.0;
        let mut elements = Vec::new();
        for (i, (gap, length, blue, ties, trim, stop)) in strokes.iter().enumerate() {
            let thread = if *blue { &BLUE } else { &RED };
            let (ties, trim, stop) = (ties.to_string(), trim.to_string(), stop.to_string());
            let params = [("ties", ties.as_str()), ("trim_after", trim.as_str()), ("stop_after", stop.as_str())];
            elements.push(line(&format!("e{i}"), (x + gap, 0.0), *length, thread, &params));
            x += gap + length;
        }
        let threads: Vec<Rgb> = elements.iter().map(|e| e.thread.color).collect();
        let plan = sewn_with(elements, DesignSettings::default()).plan.unwrap();
        for block in &plan.blocks {
            for pair in block.stitches.windows(2) {
                if pair[0].kind == StitchKind::Jump && pair[1].kind != StitchKind::Stop {
                    prop_assert_eq!((pair[1].kind, pair[1].at), (StitchKind::Normal, pair[0].at));
                }
            }
        }
        let mut changes = threads.clone();
        changes.dedup();
        prop_assert_eq!(plan.blocks.iter().map(|b| b.thread.color).collect::<Vec<_>>(), changes);
        prop_assert_eq!(plan.elements.len(), threads.len());
        let sewn_box = Rect::around(plan.stitches().filter(|s| s.kind == StitchKind::Normal).map(|s| s.at)).unwrap();
        prop_assert!(sewn_box.center().distance(p(0.0, 0.0)) < 1e-6, "{:?}", sewn_box);
    }
}
