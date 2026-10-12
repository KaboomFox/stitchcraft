//! TS-02 — machine commands: colour changes, a stop, jumps, and trims.
//!
//! Four rows of two 10 mm dashes, the dashes of each row separated by a jump of 2, 5, 15 or 30 mm (top
//! to bottom). The left half (red) puts a trim before every jump; the right half (blue) has no trims.
//! A PES file flags every jump as a trim (REQ-FMT-010), so in PES the halves differ only in the plan:
//! at MC-1 the PE800 sewed nothing after the plain-flagged jumps PES files first had for the right half.
//! In DST, machines leave the right half's jumps of up to 2 records (24.2 mm) uncut (REQ-FMT-008). A
//! green line at the bottom has a stop in its middle. Sewn, it answers: does the machine stop for each colour and for
//! the stop, does every dash sew, and which jumps does it cut?
//!
//! The dashes have no lock stitches, so the first row, where the thread starts, can pull out once the
//! machine moves on without cutting; TS-02B is the same sheet with the engine's locks.

use stitchcraft_plan::StitchPlan;

use super::sketch::{SheetError, Sketch};
use super::{BLUE, EMERALD_GREEN, RED, thread};

/// The jump lengths, one per row, top to bottom (mm).
pub const JUMPS: [f64; 4] = [2.0, 5.0, 15.0, 30.0];
/// The rows' heights (mm).
pub(super) const ROWS: [f64; 4] = [-35.0, -20.0, -5.0, 10.0];
/// Where each half's rows start (mm): 50 mm wide each, 20 mm apart, the sheet 120 mm wide to fit the
/// reference machine's 130 mm field with room to spare.
pub(super) const HALVES: [f64; 2] = [-60.0, 10.0];

/// Draws TS-02.
pub(super) fn build() -> Result<StitchPlan, SheetError> {
    let mut s = Sketch::new("ts02", thread(RED)?);
    s.part("left-trimmed")?;
    rows(&mut s, HALVES[0], true)?;
    // No trim before the thread change: PEC carries a trim to the next jump, which would be the right
    // half's first jump — and the right half must have none. The machine cuts for the change anyway.

    s.change_thread(thread(BLUE)?);
    s.part("right-jumps-only")?;
    rows(&mut s, HALVES[1], false)?;

    s.change_thread(thread(EMERALD_GREEN)?);
    s.part("stop-line")?;
    s.move_to((-15.0, 35.0), false)?;
    s.line_to((0.0, 35.0))?;
    s.stop();
    s.line_to((15.0, 35.0))?;
    s.trim();
    Ok(s.finish())
}

/// The four rows starting at `x`: dash, jump, dash. With `trims`, every jump follows a trim.
fn rows(s: &mut Sketch, x: f64, trims: bool) -> Result<(), SheetError> {
    for (i, (jump, y)) in JUMPS.iter().zip(ROWS).enumerate() {
        // The first move of a half needs no trim: it starts the plan, or follows a thread change.
        s.move_to((x, y), trims && i > 0)?;
        s.line_to((x + 10.0, y))?;
        s.move_to((x + 10.0 + jump, y), trims)?;
        s.line_to((x + 20.0 + jump, y))?;
    }
    Ok(())
}
