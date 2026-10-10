//! TS-05 — satin columns 1 to 10 mm wide at 3 zigzag spacings: how dense a satin must be to cover the
//! fabric, how wide it sews well, and how much the thread pulls it in.
//!
//! Drawn as a design (`designed`), so the engine's satin column sews it, with Ink/Stitch's other defaults:
//! no underlay and no pull compensation. 3 rows, top to bottom, at zigzag spacings of 0.3, 0.4 and 0.5 mm.
//! Each row has 10 columns 20 mm tall, 1 to 10 mm wide from left to right. Each column is sewn from its
//! top and trimmed after, so the columns stand apart.

use stitchcraft_plan::StitchPlan;

use super::designed::Drawing;
use super::sketch::SheetError;
use super::{BLUE, thread};

/// The zigzag spacings, top to bottom (mm).
pub const SPACINGS: [f64; 3] = [0.3, 0.4, 0.5];
/// The columns' widths, left to right (mm).
pub const WIDTHS: [f64; 10] = [1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0, 10.0];
/// How tall each column is (mm).
const TALL: f64 = 20.0;
/// The space between the columns of a row (mm).
const GAP: f64 = 4.0;
/// From the top of one row to the top of the next (mm).
const ROW: f64 = 28.0;

/// Draws TS-05.
pub(super) fn build() -> Result<StitchPlan, SheetError> {
    let blue = thread(BLUE)?;
    let mut d = Drawing::new("ts05");
    let mut top = 0.0;
    for spacing in SPACINGS {
        let spacing_text = spacing.to_string();
        let mut x = 0.0;
        for width in WIDTHS {
            let rails = [[(x, top), (x, top + TALL)], [(x + width, top), (x + width, top + TALL)]];
            d.satin(&format!("{spacing}-{width}"), rails, &blue, &[("zigzag_spacing_mm", &spacing_text), ("trim_after", "true")])?;
            x += width + GAP;
        }
        top += ROW;
    }
    d.plan()
}
