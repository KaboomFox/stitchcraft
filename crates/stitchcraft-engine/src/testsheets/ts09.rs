//! TS-09 — fill underlay and travel: one shape with 2 holes and a part apart, 3 times over, to see whether
//! the travel between rows hides under them, how the needle gets to the part apart, whether the holes stay
//! clean, and whether gap-filling rows close the gap where the rows round a hole meet.
//!
//! Drawn as a design (`designed`), so the engine's tatami fill sews it, with Ink/Stitch's defaults for
//! everything else. Each copy is a rectangle 50 × 30 mm with a square hole 10 mm wide and a round hole 12 mm
//! across, and a bar 10 × 30 mm 6 mm to its right, all one fill, which the engine sews in 2 parts and says
//! so (`SC-W0307`). Top to bottom: without underlay, with Ink/Stitch's default underlay, and with it and 2
//! gap-filling rows. Each copy is trimmed after, so no thread runs across the next.

use stitchcraft_core::Code;
use stitchcraft_plan::StitchPlan;

use super::designed::{Drawing, polygon, round};
use super::sketch::SheetError;
use super::{EMERALD_GREEN, thread};

/// The copies, top to bottom: each one's name and the settings it changes.
pub const COPIES: [(&str, &[(&str, &str)]); 3] =
    [("no-underlay", &[("fill_underlay", "false")]), ("underlay", &[]), ("gap-fill", &[("gap_fill_rows", "2")])];
/// How tall each copy is (mm).
pub const TALL: f64 = 30.0;
/// The space between the copies (mm).
pub const GAP: f64 = 12.0;
/// The square hole's top-left and bottom-right corners, from its copy's top-left corner (mm).
pub const SQUARE: [(f64, f64); 2] = [(8.0, 10.0), (18.0, 20.0)];
/// The round hole's centre, from its copy's top-left corner, and its radius (mm).
pub const ROUND: ((f64, f64), f64) = ((35.0, 15.0), 6.0);
/// How wide the rectangle is (mm).
pub const WIDE: f64 = 50.0;
/// Where the bar apart starts and ends, from its copy's left (mm).
pub const BAR: (f64, f64) = (56.0, 66.0);

/// Draws TS-09.
pub(super) fn build() -> Result<StitchPlan, SheetError> {
    let green = thread(EMERALD_GREEN)?;
    let mut d = Drawing::new("ts09");
    let [(left, top), (right, bottom)] = SQUARE;
    let ((x, y), radius) = ROUND;
    for (k, (name, params)) in COPIES.into_iter().enumerate() {
        let down = k as f64 * (TALL + GAP);
        let rings = vec![
            polygon(&[(0.0, down), (0.0, down + TALL), (WIDE, down + TALL), (WIDE, down)])?,
            polygon(&[(left, down + top), (left, down + bottom), (right, down + bottom), (right, down + top)])?,
            round((x, down + y), radius)?,
            polygon(&[(BAR.0, down), (BAR.0, down + TALL), (BAR.1, down + TALL), (BAR.1, down)])?,
        ];
        d.fill(name, rings, &green, &[params, &[("trim_after", "true")]].concat())?;
    }
    d.plan_saying(&[Code::FillPartsApart; 3])
}
