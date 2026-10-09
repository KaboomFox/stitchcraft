//! The diagnostics the writers and readers report, from real input, exactly as a user reads them
//! (`diag_` cases: every registered code has one, `docs/src/design/diagnostics.md`).

// Test code may unwrap (clippy.toml allows it inside #[test] functions; these helpers are test code too).
#![allow(clippy::unwrap_used)]

use stitchcraft_core::Point;
use stitchcraft_formats::{decode, encode};
use stitchcraft_plan::{FormatId, PlanBuilder, Provenance, Rgb, Role, StitchPlan, Thread};

fn red() -> Thread {
    Thread::named(Rgb::from_hex(0xED171F), "Red")
}

fn stitch(b: &mut PlanBuilder, x: f64, y: f64) {
    b.stitch(Point::new(x, y).unwrap(), Provenance::plan(Role::Top));
}

fn written(plan: &StitchPlan) -> String {
    encode(plan, FormatId::PesV1, "test").unwrap_err().diagnostic().to_string()
}

#[test]
fn diag_sc_e0010_an_empty_design_is_not_written() {
    assert_eq!(written(&PlanBuilder::new(red()).finish()), "error SC-E0010: The design has no stitches.");
}

#[test]
fn diag_sc_e0601_too_many_colour_changes_name_the_format_limit() {
    let mut b = PlanBuilder::new(red());
    stitch(&mut b, 0.0, 0.0);
    for i in 0..300_u32 {
        b.change_thread(Thread::named(Rgb::from_hex(i * 0x010101), "Thread"));
        stitch(&mut b, f64::from(i % 10), 0.0);
    }
    let max = FormatId::PesV1.max_color_changes();
    assert_eq!(written(&b.finish()), format!("error SC-E0601: The design has 300 colour changes and stops; PES v1 records at most {max}."));
}

#[test]
fn diag_sc_e0602_a_position_beyond_the_machine_grid_is_named() {
    let mut b = PlanBuilder::new(red());
    stitch(&mut b, 0.0, 0.0);
    stitch(&mut b, 12_000.0, 0.0);
    assert_eq!(written(&b.finish()), "error SC-E0602: The position (12000.0, 0.0) mm does not fit the PES v1 format.");
}

#[test]
fn diag_sc_e0603_a_file_that_is_not_embroidery_is_unreadable() {
    let d = decode(b"not embroidery").unwrap_err().diagnostic();
    assert_eq!(d.to_string(), "error SC-E0603: This is not a PES, PEC or DST file.");
}
