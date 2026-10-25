//! TS-08 — fill density and angle: tatami squares at 5 row spacings and 3 angles, to choose the default
//! spacing by how the fill covers the fabric, how stiff it sews and whether its needle points line up.
//!
//! Drawn as a design (`designed`), so the engine's tatami fill sews it, with Ink/Stitch's defaults for
//! everything else, underlay included. 15 squares 30 mm wide, 5 mm apart: rows at spacings of 0.25, 0.3,
//! 0.4, 0.5 and 0.6 mm, top to bottom, and columns at angles of 0°, 45° and 90°, left to right. Each square
//! is trimmed after, so no thread runs across the next.

use super::designed::{Drawing, polygon};
use super::sketch::SheetError;
use super::{BLUE, thread};

/// The row spacings of the rows of squares, top to bottom, in millimetres.
pub const SPACINGS: [&str; 5] = ["0.25", "0.3", "0.4", "0.5", "0.6"];
/// The angles of the columns of squares, left to right, in degrees.
pub const ANGLES: [&str; 3] = ["0", "45", "90"];
/// How wide each square is (mm).
pub const SIDE: f64 = 30.0;
/// The space between the squares (mm).
const GAP: f64 = 5.0;

/// Draws TS-08.
pub(super) fn drawing() -> Result<Drawing, SheetError> {
    let blue = thread(BLUE)?;
    let mut d = Drawing::new("ts08");
    for (row, spacing) in SPACINGS.into_iter().enumerate() {
        for (column, angle) in ANGLES.into_iter().enumerate() {
            let (x, y) = (column as f64 * (SIDE + GAP), row as f64 * (SIDE + GAP));
            let square = polygon(&[(x, y), (x, y + SIDE), (x + SIDE, y + SIDE), (x + SIDE, y)])?;
            d.fill(&format!("{spacing}-{angle}"), vec![square], &blue, &[("row_spacing_mm", spacing), ("angle", angle), ("trim_after", "true")])?;
        }
    }
    Ok(d)
}
