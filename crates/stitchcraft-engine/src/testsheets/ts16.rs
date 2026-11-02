//! TS-16 — an Ink/Stitch file (M8): what StitchCraft applies of Ink/Stitch's commands and of the other
//! features of its files, sewn beside Ink/Stitch's own machine file of the sheet (MC-11). Each step of M8
//! that applies one draws its part.
//!
//! M8.4's part: a tatami fill and a satin column, each with start and end positions, which Ink/Stitch's
//! start and end commands give (`REQ-GEN-001`). The differential writes the sheet as Ink/Stitch writes a
//! file, each command's symbol where its position is, and both programs sew that file (`REQ-SVG-008`,
//! `xtask/src/conformance/differential.rs`). `stitch testsheet` plans the sheet's design itself, which the
//! SVG reader reads back from that file unchanged (`REQ-PRM-003`).
//!
//! The red fill, 40 by 30 mm with the default underlay, starts at its top-right corner and ends at its
//! bottom-left one. The blue satin column below it, 40 mm long and 5 mm wide, starts in its middle: the
//! needle runs under the column to its left end and sews it to three quarters along, runs on under it to its
//! right end, sews back, and ends at its end position, three quarters along. The 2 threads part them, so each
//! element's first stitch follows a stop.
//!
//! The later parts, clip paths (M8.5), the origin and the stop position (M8.8), the rotation on export (M8.9)
//! and clones (M8.10), are features of the file. Each step that adds one decides how the sheet holds it: in
//! the design, which the differential then writes into the file, or in the file itself.

use super::designed::Drawing;
use super::sketch::SheetError;
use super::{BLUE, RED, thread};

/// The fill's corners, left, top, right and bottom (mm).
pub const FILL: [f64; 4] = [0.0, 0.0, 40.0, 30.0];
/// The satin column's rails, at y = 40 and y = 45 from x = 0 to x = 40 (mm).
pub const RAILS: [[(f64, f64); 2]; 2] = [[(0.0, 40.0), (40.0, 40.0)], [(0.0, 45.0), (40.0, 45.0)]];

/// Draws TS-16.
pub(super) fn drawing() -> Result<Drawing, SheetError> {
    let (red, blue) = (thread(RED)?, thread(BLUE)?);
    let mut d = Drawing::new("ts16");
    let [left, top, right, bottom] = FILL;
    let ring = super::designed::polygon(&[(left, top), (right, top), (right, bottom), (left, bottom)])?;
    d.fill("fill", vec![ring], &red, &[])?;
    d.positions((right, top), (left, bottom))?;
    d.satin("column", RAILS, &blue, &[])?;
    d.positions((20.0, 42.5), (30.0, 42.5))?;
    Ok(d)
}
