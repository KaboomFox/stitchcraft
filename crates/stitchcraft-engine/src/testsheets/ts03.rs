//! TS-03 — the running stitch: stitch lengths, bean stitch, curves, and the shortest stitch the machine
//! sews cleanly.
//!
//! Drawn as a design (`designed`), so the engine's running stitch, manual stitch and locks sew it. Top to
//! bottom:
//!
//! 1. Five 60 mm lines at running stitch lengths of 1.5, 2.0, 2.5, 3.0 and 4.0 mm: even stitches?
//! 2. Two 60 mm bean-stitch lines, each stitch sewn three and five times (`bean_stitch_repeats` 1 and 2).
//! 3. Three circles 6 mm across at tolerances of 0.1, 0.2 and 0.5 mm, left to right: the larger the
//!    tolerance, the fewer and straighter the stitches.
//! 4. Five lines of 20 stitches placed by hand (manual stitch), 0.3, 0.4, 0.5, 0.7 and 1.0 mm long, with
//!    back-and-forth locks so they hold. The shortest that sews cleanly is the machine's shortest stitch:
//!    the profile says 0.3 mm until this sheet has been sewn.
//!
//! Every element is trimmed after, so the lines stand apart.

use stitchcraft_plan::StitchPlan;

use super::designed::Drawing;
use super::sketch::SheetError;
use super::{BLUE, thread};

/// The running stitch lengths, top to bottom (mm).
pub const LENGTHS: [f64; 5] = [1.5, 2.0, 2.5, 3.0, 4.0];
/// The bean stitch repeats, top to bottom.
pub const BEANS: [u32; 2] = [1, 2];
/// The circles' tolerances, left to right (mm).
pub const TOLERANCES: [f64; 3] = [0.1, 0.2, 0.5];
/// The hand-placed stitch lengths, top to bottom (mm).
pub const SHORT: [f64; 5] = [0.3, 0.4, 0.5, 0.7, 1.0];
/// Stitches in each hand-placed line.
const SHORT_STITCHES: u32 = 20;
/// The lines' length (mm): a whole number of stitches at every length above.
const LINE: f64 = 60.0;
/// The distance between rows (mm).
const ROW: f64 = 6.0;
/// The circles' radius (mm): small enough that their stitches stray from the curve by more than 0.1 mm.
const RADIUS: f64 = 3.0;

/// Draws TS-03.
pub(super) fn build() -> Result<StitchPlan, SheetError> {
    let blue = thread(BLUE)?;
    let mut d = Drawing::new("ts03");
    let mut y = 0.0;
    for length in LENGTHS {
        let length = length.to_string();
        d.polyline(&format!("running-{length}"), &[(0.0, y), (LINE, y)], &blue, &[("running_stitch_length_mm", &length), ("trim_after", "true")])?;
        y += ROW;
    }
    for beans in BEANS {
        let beans = beans.to_string();
        d.polyline(&format!("bean-{beans}"), &[(0.0, y), (LINE, y)], &blue, &[("bean_stitch_repeats", &beans), ("trim_after", "true")])?;
        y += ROW;
    }
    y += RADIUS;
    for (x, tolerance) in [10.0, 30.0, 50.0].into_iter().zip(TOLERANCES) {
        let tolerance = tolerance.to_string();
        d.circle(&format!("circle-{tolerance}"), (x, y), RADIUS, &blue, &[("running_stitch_tolerance_mm", &tolerance), ("trim_after", "true")])?;
    }
    y += RADIUS + ROW;
    for length in SHORT {
        let points: Vec<(f64, f64)> = (0..=SHORT_STITCHES).map(|k| (f64::from(k) * length, y)).collect();
        let by_hand = [
            ("stroke_method", "manual_stitch"),
            ("force_lock_stitches", "true"),
            ("lock_start", "back_forth"),
            ("lock_end", "back_forth"),
            ("trim_after", "true"),
        ];
        d.polyline(&format!("short-{length}"), &points, &blue, &by_hand)?;
        y += ROW;
    }
    d.plan()
}
