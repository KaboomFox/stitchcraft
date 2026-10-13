//! The diagnostics the writers and readers report, from real input, exactly as a user reads them
//! (`diag_` cases: every registered code has one, `docs/src/design/diagnostics.md`).

// Test code may unwrap (clippy.toml allows it inside #[test] functions; these helpers are test code too).
#![allow(clippy::unwrap_used)]

use stitchcraft_core::Point;
use stitchcraft_formats::{decode, encode};
use stitchcraft_plan::{FormatId, PlanBuilder, Provenance, Rgb, Role, StitchPlan, Thread};
use stitchcraft_testkit::plans;

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
    // PES records where the design's top-left corner is from the start in 12 bits (REQ-FMT-011).
    let mut b = PlanBuilder::new(red());
    stitch(&mut b, -250.0, 3.0);
    stitch(&mut b, -248.0, 5.0);
    assert_eq!(
        written(&b.finish()),
        "error SC-E0602: A design whose top-left corner is (-250.0, 3.0) mm from where the machine starts does not fit the PES v1 format."
    );
}

#[test]
fn diag_sc_e0603_a_file_that_is_not_embroidery_is_unreadable() {
    let d = decode(b"not embroidery").unwrap_err().diagnostic();
    assert_eq!(d.to_string(), "error SC-E0603: This is not a PES, PEC or DST file.");
}

/// What writing `plan` in `format` says about the file.
fn notes(plan: &StitchPlan, format: FormatId) -> Vec<String> {
    encode(plan, format, "test").unwrap().notes.iter().map(ToString::to_string).collect()
}

#[test]
fn diag_sc_i0605_dst_says_where_its_machines_cut_and_the_plan_does_not() {
    // Of the canonical long jumps, DST machines cut before the one between stitches only.
    assert_eq!(
        notes(&plans::long_jumps(), FormatId::Dst),
        [
            "info SC-I0605: The thread will be cut at 1 place the plan does not trim: DST machines cut it before 3 or more jump records in a row, and a jump longer than 24.2 mm takes that many."
        ]
    );
    assert_eq!(notes(&plans::long_jumps(), FormatId::PesV1), Vec::<String>::new());
    // A trim before a long jump is the plan's own cut.
    assert_eq!(notes(&plans::every_command(), FormatId::Dst), Vec::<String>::new());
    // A trim after a long jump: the machine cuts before the jump, not where the plan trims. A sewn move of
    // four records is three jumps and a stitch, so it is cut before too.
    let mut b = PlanBuilder::new(red());
    stitch(&mut b, 0.0, 0.0);
    stitch(&mut b, 3.0, 0.0);
    b.jump(Point::new(33.0, 0.0).unwrap(), Provenance::plan(Role::Travel));
    b.trim(None);
    stitch(&mut b, 36.0, 0.0);
    stitch(&mut b, 76.0, 0.0);
    assert_eq!(
        notes(&b.finish(), FormatId::Dst),
        [
            "info SC-I0605: The thread will be cut at 2 places the plan does not trim: DST machines cut it before 3 or more jump records in a row, and a jump longer than 24.2 mm takes that many."
        ]
    );
}

#[test]
fn diag_sc_w0606_threads_beyond_the_palette_share_its_colours() {
    // The 61 Brother thread colours, each a thread of its own, then 2 more threads: their nearest colours,
    // Red and White, are taken, so each is shown as Red or White too.
    let colors: Vec<Rgb> = stitchcraft_plan::palette::BROTHER_PEC.entries.iter().filter(|e| e.matchable).map(|e| e.color).collect();
    let mut b = PlanBuilder::new(Thread::new(colors[0]));
    stitch(&mut b, 0.0, 0.0);
    for color in colors.iter().skip(1).copied().chain([Rgb::from_hex(0xD81B24), Rgb::from_hex(0xFFFFFF)]) {
        b.change_thread(Thread::new(color));
        stitch(&mut b, 1.0, 0.0);
    }
    assert_eq!(
        notes(&b.finish(), FormatId::PesV1),
        [
            "warning SC-W0606: 2 of the design's 63 different threads have the Brother colour of another thread: a PES file names each thread by one of the 61 Brother thread colours, so the machine shows one colour for different threads."
        ]
    );
    // One thread fewer: 1 thread shares.
    let mut b = PlanBuilder::new(Thread::new(colors[0]));
    stitch(&mut b, 0.0, 0.0);
    for color in colors.iter().skip(1).copied().chain([Rgb::from_hex(0xD81B24)]) {
        b.change_thread(Thread::new(color));
        stitch(&mut b, 1.0, 0.0);
    }
    assert_eq!(
        notes(&b.finish(), FormatId::PesV1),
        [
            "warning SC-W0606: 1 of the design's 62 different threads has the Brother colour of another thread: a PES file names each thread by one of the 61 Brother thread colours, so the machine shows one colour for different threads."
        ]
    );
}
