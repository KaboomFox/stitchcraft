//! REQ-FMT-002 and REQ-FMT-003: a plan written and read back makes the machine do the same thing — the
//! needle goes down at the same 0.1 mm positions, the thread is cut and the machine pauses at the same
//! places — for every writer/reader pair, on random plans and on a plan with every command.
//! "The same thing" is `stitchcraft_testkit::equivalence::events`; for DST, whose machines also cut the
//! thread before three or more jumps in a row, `dst_events` (REQ-FMT-008).

// Test code may unwrap (clippy.toml allows it inside #[test] functions; these helpers are test code too).
#![allow(clippy::unwrap_used)]

use proptest::prelude::*;
use stitchcraft_formats::{decode, encode};
use stitchcraft_plan::palette::BROTHER_PEC;
use stitchcraft_plan::{FormatId, StitchPlan};
use stitchcraft_testkit::equivalence::{Event, dst_events, events};
use stitchcraft_testkit::{plans, strategies};

fn round_trip(plan: &StitchPlan, format: FormatId) -> StitchPlan {
    let encoded = encode(plan, format, "round trip").unwrap();
    decode(&encoded.bytes).unwrap().plan
}

/// The Brother palette index of every thread the machine asks for.
fn pec_threads(plan: &StitchPlan) -> Vec<u8> {
    plan.color_entries().iter().filter_map(|e| BROTHER_PEC.nearest(e.thread.color)).map(|e| e.index).collect()
}

#[test]
fn req_fmt_003_every_command_survives_every_format() {
    let plan = plans::every_command();
    let expected = events(&plan);
    assert!(expected.iter().any(|e| matches!(e, Event::Cut(..))) && expected.iter().any(|e| matches!(e, Event::Pause(..))));
    for format in FormatId::ALL {
        assert_eq!(events(&round_trip(&plan, *format)), expected, "{}", format.name());
    }
    assert_eq!(pec_threads(&round_trip(&plan, FormatId::PesV1)), pec_threads(&plan));
}

proptest! {
    #![proptest_config(strategies::config(256))]

    #[test]
    fn req_fmt_002_random_plans_round_trip_through_pes(plan in strategies::plan(120, 100.0)) {
        let back = round_trip(&plan, FormatId::PesV1);
        prop_assert_eq!(events(&back), events(&plan));
        prop_assert_eq!(pec_threads(&back), pec_threads(&plan));
    }

    #[test]
    fn req_fmt_002_random_plans_round_trip_through_dst(plan in strategies::plan(120, 100.0)) {
        prop_assert_eq!(events(&round_trip(&plan, FormatId::Dst)), dst_events(&plan));
    }
}
