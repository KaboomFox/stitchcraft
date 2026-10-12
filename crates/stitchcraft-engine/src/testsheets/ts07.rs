//! TS-07 — registration: a tatami fill inside a running-stitch outline drawn on its edge, at 3 pull
//! compensations, to see whether fill and outline meet and how much compensation closes a gap between them.
//!
//! Drawn as a design (`designed`), so the engine's tatami fill and running stitch sew it, with Ink/Stitch's
//! defaults for everything else. 3 circles 60 mm across, 2 side by side and 1 below them, filled in red
//! with pull compensations of 0, 0.2 and 0.4 mm. The outlines follow in black, all 3 after the fills, so the
//! thread changes once. Each fill and outline is trimmed after, so no thread runs across a circle.

use stitchcraft_plan::StitchPlan;

use super::designed::{Drawing, round};
use super::sketch::SheetError;
use super::{BLACK, RED, thread};

/// Each circle's pull compensation, in millimetres, and its centre: top left, top right, then bottom.
pub const CIRCLES: [(&str, (f64, f64)); 3] = [("0", (-33.0, -33.0)), ("0.2", (33.0, -33.0)), ("0.4", (0.0, 33.0))];
/// The circles' radius (mm).
pub const RADIUS: f64 = 30.0;

/// Draws TS-07.
pub(super) fn build() -> Result<StitchPlan, SheetError> {
    let (red, black) = (thread(RED)?, thread(BLACK)?);
    let mut d = Drawing::new("ts07");
    for (pull, centre) in CIRCLES {
        d.fill(&format!("fill-{pull}"), vec![round(centre, RADIUS)?], &red, &[("pull_compensation_mm", pull), ("trim_after", "true")])?;
    }
    for (pull, centre) in CIRCLES {
        d.circle(&format!("outline-{pull}"), centre, RADIUS, &black, &[("trim_after", "true")])?;
    }
    d.plan()
}
