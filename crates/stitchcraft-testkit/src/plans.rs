//! Canonical plans: small, hand-built plans whose encodings are committed as golden files
//! (`conformance/golden/formats/`, REQ-FMT-001). Each exercises format features on purpose, so a change
//! in how any writer spells an operation shows up as a golden diff that has to be explained.

use stitchcraft_core::Point;
use stitchcraft_plan::{PlanBuilder, Provenance, Rgb, Role, StitchPlan, Thread};

/// Every canonical plan with its name (the golden files are `<name>.<extension>`).
pub fn canonical() -> Vec<(&'static str, StitchPlan)> {
    vec![("every-command", every_command()), ("one-stitch", one_stitch()), ("long-jumps", long_jumps())]
}

fn p(x: f64, y: f64) -> Point {
    Point::new(x, y).unwrap()
}

/// Two threads; short and long sewn moves in every direction; a trim before a jump longer than one DST
/// record; a stop; a trim before a sewn move; a trim before the end.
pub fn every_command() -> StitchPlan {
    let (top, travel) = (Provenance::plan(Role::Top), Provenance::plan(Role::Travel));
    let mut b = PlanBuilder::new(Thread::named(Rgb::from_hex(0xED171F), "Red"));
    b.jump(p(-20.0, -10.0), travel);
    for at in [p(-20.0, -10.0), p(-17.5, -10.0), p(-15.0, -10.0), p(-8.7, -10.0), p(-8.7, -15.8)] {
        b.stitch(at, top);
    }
    b.trim(None);
    b.jump(p(20.0, -10.0), travel);
    b.stitch(p(20.0, -10.0), top);
    b.stitch(p(22.0, -8.0), top);
    b.stop(None);
    b.stitch(p(24.0, -6.0), top);
    b.change_thread(Thread::named(Rgb::from_hex(0x0A55A3), "Blue"));
    b.jump(p(0.0, 10.0), travel);
    b.stitch(p(0.0, 10.0), top);
    b.stitch(p(-2.5, 12.5), top);
    b.trim(None);
    b.stitch(p(5.0, 12.5), top);
    b.stitch(p(7.5, 12.5), top);
    b.trim(None);
    b.finish()
}

/// Untrimmed jumps of two and three DST records (24.2 and 30 mm): from the start, between stitches and
/// after a thread change. DST machines cut the thread before three or more jump records in a row, but
/// only where something was sewn since it was last cut or changed — so of these, only the 30 mm jump
/// between stitches is cut (REQ-FMT-008).
pub fn long_jumps() -> StitchPlan {
    let (top, travel) = (Provenance::plan(Role::Top), Provenance::plan(Role::Travel));
    let mut b = PlanBuilder::new(Thread::named(Rgb::from_hex(0xED171F), "Red"));
    b.jump(p(-30.0, 0.0), travel);
    b.stitch(p(-30.0, 0.0), top);
    b.stitch(p(-27.0, 0.0), top);
    b.jump(p(-2.8, 0.0), travel);
    b.stitch(p(-2.8, 0.0), top);
    b.stitch(p(0.0, 0.0), top);
    b.jump(p(30.0, 0.0), travel);
    b.stitch(p(30.0, 0.0), top);
    b.stitch(p(33.0, 0.0), top);
    b.change_thread(Thread::named(Rgb::from_hex(0x0A55A3), "Blue"));
    b.jump(p(0.0, 30.0), travel);
    b.stitch(p(0.0, 30.0), top);
    b.stitch(p(3.0, 30.0), top);
    b.finish()
}

/// The smallest plan a writer accepts: one stitch at the origin.
pub fn one_stitch() -> StitchPlan {
    let mut b = PlanBuilder::new(Thread::new(Rgb::new(0, 0, 0)));
    b.stitch(Point::ORIGIN, Provenance::plan(Role::Top));
    b.finish()
}
