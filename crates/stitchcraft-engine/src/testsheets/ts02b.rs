//! TS-02B — TS-02 drawn as a design: colour changes, a stop, jumps, and the trims elements ask for.
//!
//! TS-02's rows of two 10 mm dashes, the dashes 2, 5, 15 and 30 mm apart (top to bottom), but each dash an
//! element and each command one an element asks for, so the engine's plan assembly decides the jumps,
//! trims, locks and stops that TS-02 spells out stitch by stitch (`designed`). Left half (red): every dash
//! but the last says `trim_after`, so the engine sews a tie-off, trims, jumps and sews a tie-in. Right half
//! (blue): no element asks for a trim; the 2 mm gap is within the collapse length (3 mm), so it is sewn
//! across, and the others get a tie-off, a jump and a tie-in. A green line at the bottom stops halfway
//! (`stop_after`). Sewn, it answers: does the machine trim where an element asks, and do the ends hold?

use stitchcraft_plan::{StitchPlan, Thread};

use super::designed::Drawing;
use super::sketch::SheetError;
use super::ts02::{HALVES, JUMPS, ROWS};
use super::{BLUE, EMERALD_GREEN, RED, thread};

/// Draws TS-02B.
pub(super) fn build() -> Result<StitchPlan, SheetError> {
    let mut d = Drawing::new("ts02b");
    // As in TS-02, no trim before the thread change: it would ride on the right half's first jump.
    rows(&mut d, "left", HALVES[0], &thread(RED)?, true)?;
    rows(&mut d, "right", HALVES[1], &thread(BLUE)?, false)?;
    let green = thread(EMERALD_GREEN)?;
    d.polyline("stop-line-1", &[(-15.0, 35.0), (0.0, 35.0)], &green, &[("stop_after", "true")])?;
    d.polyline("stop-line-2", &[(0.5, 35.0), (15.0, 35.0)], &green, &[("trim_after", "true")])?;
    d.plan()
}

/// The four rows of the half named `half`, starting at `x`. With `trims`, every dash but the last asks
/// for a trim after it.
fn rows(d: &mut Drawing, half: &str, x: f64, thread: &Thread, trims: bool) -> Result<(), SheetError> {
    for (row, (jump, y)) in JUMPS.iter().zip(ROWS).enumerate() {
        let last = row + 1 == ROWS.len();
        let first_trim: &[(&str, &str)] = if trims { &[("trim_after", "true")] } else { &[] };
        let second_trim: &[(&str, &str)] = if trims && !last { &[("trim_after", "true")] } else { &[] };
        d.polyline(&format!("{half}-row-{}-a", row + 1), &[(x, y), (x + 10.0, y)], thread, first_trim)?;
        d.polyline(&format!("{half}-row-{}-b", row + 1), &[(x + 10.0 + jump, y), (x + 20.0 + jump, y)], thread, second_trim)?;
    }
    Ok(())
}
