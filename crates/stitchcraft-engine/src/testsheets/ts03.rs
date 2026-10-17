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
//!    the profile says 0.3 mm until this sheet has been sewn, and StitchCraft sews nothing shorter.
//!
//! Every element is trimmed after, so the lines stand apart. The reference machine cuts no jump (MC-1), so
//! each jump's thread stays on the fabric, and no element starts along the thread that brought the needle to
//! it: the long lines run left to right and right to left in turn, the circles are sewn right to left from
//! their right-hand points, and each line of hand-placed stitches starts below where the one before ended.

use stitchcraft_plan::StitchPlan;

use super::designed::{Drawing, Start};
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
/// The circles' centres across the sheet, left to right (mm).
const CIRCLES: [f64; 3] = [10.0, 30.0, 50.0];

/// The ends of the long line at `y`, in the order it is sewn: the `row`th from the top, so the even rows run
/// left to right and the others back.
fn across(row: usize, y: f64) -> [(f64, f64); 2] {
    if row.is_multiple_of(2) { [(0.0, y), (LINE, y)] } else { [(LINE, y), (0.0, y)] }
}

/// Draws TS-03.
pub(super) fn build() -> Result<StitchPlan, SheetError> {
    let blue = thread(BLUE)?;
    let mut d = Drawing::new("ts03");
    let mut y = 0.0;
    let mut row = 0;
    for length in LENGTHS {
        let length = length.to_string();
        d.polyline(&format!("running-{length}"), &across(row, y), &blue, &[("running_stitch_length_mm", &length), ("trim_after", "true")])?;
        (row, y) = (row + 1, y + ROW);
    }
    for beans in BEANS {
        let beans = beans.to_string();
        d.polyline(&format!("bean-{beans}"), &across(row, y), &blue, &[("bean_stitch_repeats", &beans), ("trim_after", "true")])?;
        (row, y) = (row + 1, y + ROW);
    }
    y += RADIUS;
    // Right to left, each from its right-hand point: the jump to the next leaves where the last one ended, on
    // its right, and arrives on the next one's right, so it crosses only circles already sewn.
    for (x, tolerance) in CIRCLES.into_iter().zip(TOLERANCES).rev() {
        let tolerance = tolerance.to_string();
        d.circle(
            &format!("circle-{tolerance}"),
            (x, y),
            RADIUS,
            Start::Right,
            &blue,
            &[("running_stitch_tolerance_mm", &tolerance), ("trim_after", "true")],
        )?;
    }
    y += RADIUS + ROW;
    // Each line starts below where the one before ended, the first below the last circle's end, and they run
    // right and left in turn.
    let (mut x, mut step) = (CIRCLES[0] + RADIUS, 1.0);
    for length in SHORT {
        let points: Vec<(f64, f64)> = (0..=SHORT_STITCHES).map(|k| (x + step * f64::from(k) * length, y)).collect();
        let by_hand = [
            ("stroke_method", "manual_stitch"),
            ("force_lock_stitches", "true"),
            ("lock_start", "back_forth"),
            ("lock_end", "back_forth"),
            ("trim_after", "true"),
        ];
        d.polyline(&format!("short-{length}"), &points, &blue, &by_hand)?;
        (x, step, y) = (x + step * f64::from(SHORT_STITCHES) * length, -step, y + ROW);
    }
    d.plan()
}
