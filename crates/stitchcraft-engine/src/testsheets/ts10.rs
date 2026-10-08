//! TS-10 — hoop size: a rectangular frame of a given size with a centre cross and an "F".
//!
//! Three sheets: 150 × 150 mm (the comfort zone), 190 × 150 mm and 150 × 190 mm (near the hoop's edge,
//! landscape and portrait). Sewn, they answer: does the machine accept and show each design from a PES v1
//! file, and does it sew it to size? The "F" in the top-left corner tells landscape from portrait.

use stitchcraft_plan::StitchPlan;

use super::sketch::{SheetError, Sketch, letter_f};
use super::{BLUE, thread};

/// Draws a TS-10 frame `width` × `height` mm.
pub(super) fn build(sheet: &'static str, width: f64, height: f64) -> Result<StitchPlan, SheetError> {
    let (x, y) = (width / 2.0, height / 2.0);
    let mut s = Sketch::new(sheet, thread(BLUE)?);
    s.part("frame")?;
    s.move_to((-x, -y), false)?;
    s.polyline(&[(x, -y), (x, y), (-x, y), (-x, -y)])?;

    s.part("centre-cross")?;
    s.move_to((-5.0, 0.0), true)?;
    s.line_to((5.0, 0.0))?;
    s.move_to((0.0, -5.0), true)?;
    s.line_to((0.0, 5.0))?;

    s.part("letter-f")?;
    letter_f(&mut s, (-x + 10.0, -y + 25.0), 15.0, true)?;
    s.trim();
    Ok(s.finish())
}
