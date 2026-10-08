//! TS-01 — orientation and scale.
//!
//! A 100 mm cross with ticks every 10 mm (longer at the ends and the centre), a 10 mm square in each
//! corner, and an upright "F" in the top-left quarter, in one thread. Sewn, it answers: is the design
//! mirrored or rotated (the F), is the scale right (the arms measure 100 mm), are x and y scaled alike
//! (the squares are square)?

use stitchcraft_plan::StitchPlan;

use super::sketch::{SheetError, Sketch, letter_f};
use super::{BLACK, thread};

/// Draws TS-01.
pub(super) fn build() -> Result<StitchPlan, SheetError> {
    let mut s = Sketch::new("ts01", thread(BLACK)?);

    // The horizontal arm, left to right, ticks pointing up (−y).
    s.part("cross-horizontal")?;
    s.move_to((-50.0, 0.0), false)?;
    ruler(&mut s, |along, across| (along, -across))?;

    // The vertical arm, top to bottom, ticks pointing right (+x).
    s.part("cross-vertical")?;
    s.move_to((0.0, -50.0), true)?;
    ruler(&mut s, |along, across| (across, along))?;

    s.part("letter-f")?;
    letter_f(&mut s, (-35.0, -15.0), 25.0, true)?;

    for (name, (x, y)) in [
        ("square-top-left", (-60.0, -60.0)),
        ("square-top-right", (50.0, -60.0)),
        ("square-bottom-right", (50.0, 50.0)),
        ("square-bottom-left", (-60.0, 50.0)),
    ] {
        s.part(name)?;
        s.move_to((x, y), true)?;
        s.polyline(&[(x + 10.0, y), (x + 10.0, y + 10.0), (x, y + 10.0), (x, y)])?;
    }
    s.trim();
    Ok(s.finish())
}

/// One 100 mm arm from −50 to 50 along its axis, with a tick every 10 mm: 5 mm at the ends and the
/// centre, 3 mm elsewhere. `at(along, across)` maps arm coordinates to the sheet.
fn ruler(s: &mut Sketch, at: impl Fn(f64, f64) -> (f64, f64)) -> Result<(), SheetError> {
    for k in 0..=10_i32 {
        let along = -50.0 + 10.0 * f64::from(k);
        let tick = if k % 5 == 0 { 5.0 } else { 3.0 };
        s.line_to(at(along, tick))?;
        s.line_to(at(along, 0.0))?;
        if k < 10 {
            s.line_to(at(along + 10.0, 0.0))?;
        }
    }
    Ok(())
}
