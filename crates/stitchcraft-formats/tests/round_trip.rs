//! REQ-FMT-002 and REQ-FMT-003: a plan written and read back makes the machine do the same thing — the
//! needle goes down at the same 0.1 mm positions, the thread is cut and the machine pauses at the same
//! places — for every writer/reader pair, on random plans and on a plan with every command.
//! "The same thing" is `stitchcraft_testkit::equivalence::events`, with the cuts each format adds:
//! `format_events` — before three or more jumps in a row for DST (REQ-FMT-008), before every jump for
//! PES (REQ-FMT-010). An element sewn in several threads is written with each of them (REQ-THREAD-003).

// Test code may unwrap (clippy.toml allows it inside #[test] functions; these helpers are test code too).
#![allow(clippy::unwrap_used)]

use proptest::prelude::*;
use stitchcraft_core::Point;
use stitchcraft_formats::{decode, encode};
use stitchcraft_plan::palette::BROTHER_PEC;
use stitchcraft_plan::{FormatId, PlanBuilder, Provenance, Rgb, Role, StitchPlan, Thread};
use stitchcraft_testkit::equivalence::{Event, events, format_events};
use stitchcraft_testkit::{designs, plans, strategies};

fn round_trip(plan: &StitchPlan, format: FormatId) -> StitchPlan {
    let encoded = encode(plan, format, "round trip").unwrap();
    decode(&encoded.bytes).unwrap().plan
}

/// The Brother palette index of every thread the machine asks for (REQ-THREAD-002).
fn pec_threads(plan: &StitchPlan) -> Vec<u8> {
    BROTHER_PEC.assign(plan.color_entries().iter().map(|e| e.thread)).unwrap().entries.iter().map(|e| e.index).collect()
}

#[test]
fn req_fmt_003_every_command_survives_every_format() {
    let plan = plans::every_command();
    let planned = events(&plan);
    assert!(planned.iter().any(|e| matches!(e, Event::Cut(..))) && planned.iter().any(|e| matches!(e, Event::Pause(..))));
    for format in FormatId::ALL {
        let told = format_events(&plan, *format);
        assert_eq!(events(&round_trip(&plan, *format)), told, "{}", format.name());
        // On this plan a format only adds cuts: every needle-down, pause and planned cut is there.
        assert!(planned.iter().all(|e| told.contains(e)), "{}", format.name());
    }
    assert_eq!(pec_threads(&round_trip(&plan, FormatId::PesV1)), pec_threads(&plan));
}

#[test]
fn req_fmt_002_a_stitch_in_the_file_is_the_plans_to_within_the_rounding_of_its_ends() {
    // 11.95 mm from (0.04, 0.04) to (8.49, 8.49) is 12.02 mm from (0, 0) to (8.5, 8.5) in the file, past the
    // machine's 12 mm; and 0.3 mm from 0.15 to 0.45 is 0.2 mm from 0.2 to 0.4, half to even. Within 0.1 mm
    // each way of the plan's stitch, and so within 0.1 × √2 mm in length.
    let mut plan = PlanBuilder::new(Thread::new(Rgb::new(0, 0, 0)));
    let at = |x: f64, y: f64| Point::new(x, y).unwrap();
    for (from, to) in [(at(0.04, 0.04), at(8.49, 8.49)), (at(0.15, 20.0), at(0.45, 20.0))] {
        plan.jump(from, Provenance::plan(Role::Travel));
        plan.stitch(from, Provenance::plan(Role::Top));
        plan.stitch(to, Provenance::plan(Role::Top));
    }
    let plan = plan.finish();
    let planned: Vec<f64> = plan.sewn_stitches().iter().map(|s| s.length()).collect();
    for format in FormatId::ALL {
        let written: Vec<f64> = round_trip(&plan, *format).sewn_stitches().iter().map(|s| s.length()).collect();
        assert_eq!(written.len(), 2, "{}", format.name());
        assert!((written[0] - 8.5 * std::f64::consts::SQRT_2).abs() < 1e-9 && (written[1] - 0.2).abs() < 1e-9, "{}: {written:?}", format.name());
        for (w, p) in written.iter().zip(&planned) {
            assert!((w - p).abs() <= 0.1 * std::f64::consts::SQRT_2, "{}: {w} for {p}", format.name());
        }
    }
}

#[test]
fn req_thread_003_pes_and_dst_files_write_each_thread_of_an_element_sewn_in_3() {
    // One element in red, green and blue, as a gradient fill of 3 stops sews: 3 colour blocks, 2 changes.
    let plan = designs::three_threads();
    assert_eq!(plan.elements.len(), 1);
    for format in FormatId::ALL {
        let back = round_trip(&plan, *format);
        assert_eq!(events(&back), format_events(&plan, *format), "{}", format.name());
        let stats = back.stats();
        assert_eq!((back.blocks.len(), stats.color_changes, stats.stops), (3, 2, 0), "{}", format.name());
    }
    // A PES file names each by its own Brother colour: Red, then a green and a blue, none of them a stop.
    let threads = pec_threads(&round_trip(&plan, FormatId::PesV1));
    assert_eq!(threads, pec_threads(&plan));
    assert_eq!(threads.len(), 3);
    assert!(threads[0] != threads[1] && threads[1] != threads[2] && threads[0] != threads[2], "{threads:?}");
}

proptest! {
    #![proptest_config(strategies::config(256))]

    #[test]
    fn req_fmt_002_random_plans_round_trip_through_pes(plan in strategies::plan(120, 100.0)) {
        let back = round_trip(&plan, FormatId::PesV1);
        prop_assert_eq!(events(&back), format_events(&plan, FormatId::PesV1));
        prop_assert_eq!(pec_threads(&back), pec_threads(&plan));
    }

    #[test]
    fn req_fmt_002_random_plans_round_trip_through_dst(plan in strategies::plan(120, 100.0)) {
        prop_assert_eq!(events(&round_trip(&plan, FormatId::Dst)), format_events(&plan, FormatId::Dst));
    }
}
