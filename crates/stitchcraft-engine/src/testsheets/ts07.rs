//! TS-07 — registration: a tatami fill inside a running-stitch outline drawn on its edge, at 3 pull
//! compensations, to see whether fill and outline meet and how much compensation closes a gap between them.
//!
//! Drawn as a design (`designed`), so the engine's tatami fill and running stitch sew it, with Ink/Stitch's
//! defaults for everything else. 3 circles 60 mm across, 2 side by side and 1 below them, filled in red
//! with pull compensations of 0, 0.2 and 0.4 mm. The outlines follow in black, all 3 after the fills, so the
//! thread changes once. Each fill and outline is trimmed after, so no thread runs across a circle.
//!
//! The reference machine cuts no jump (MC-1), but it stops at the thread change, where the operator snips the
//! red jump threads. The black ones stay until the end, so each outline starts where the jump to it arrives,
//! and no outline crosses a jump thread: the top-right one at its left-hand point, across the gap from where
//! the top-left one ends, and the bottom one at its top.

use super::designed::{Drawing, Start, round};
use super::sketch::SheetError;
use super::{BLACK, RED, thread};

/// Each circle's pull compensation, in millimetres, and its centre: top left, top right, then bottom.
pub const CIRCLES: [(&str, (f64, f64)); 3] = [("0", (-33.0, -33.0)), ("0.2", (33.0, -33.0)), ("0.4", (0.0, 33.0))];
/// Where each outline starts, in the order of [`CIRCLES`].
pub(super) const STARTS: [Start; 3] = [Start::Right, Start::Left, Start::Top];
/// The circles' radius (mm).
pub const RADIUS: f64 = 30.0;

/// Draws TS-07.
pub(super) fn drawing() -> Result<Drawing, SheetError> {
    let (red, black) = (thread(RED)?, thread(BLACK)?);
    let mut d = Drawing::new("ts07");
    for (pull, centre) in CIRCLES {
        d.fill(&format!("fill-{pull}"), vec![round(centre, RADIUS)?], &red, &[("pull_compensation_mm", pull), ("trim_after", "true")])?;
    }
    for ((pull, centre), start) in CIRCLES.into_iter().zip(STARTS) {
        d.circle(&format!("outline-{pull}"), centre, RADIUS, start, &black, &[("trim_after", "true")])?;
    }
    Ok(d)
}
