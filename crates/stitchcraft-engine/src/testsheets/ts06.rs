//! TS-06 — satin underlays: the same column with each underlay, to choose the defaults by how the edges,
//! the loft and the fabric come out.
//!
//! Drawn as a design (`designed`), so the engine's satin column and its underlays sew it, with
//! Ink/Stitch's defaults for everything else. 5 columns 6 mm wide and 30 mm tall, left to right: no
//! underlay, a centre walk, a contour, a zigzag, and a contour with a zigzag. Each column is sewn from its
//! top and trimmed after, so the columns stand apart.

use stitchcraft_plan::StitchPlan;

use super::designed::Drawing;
use super::sketch::SheetError;
use super::{RED, thread};

/// The columns, left to right: each one's name and the underlays it turns on.
pub const UNDERLAYS: [(&str, &[(&str, &str)]); 5] = [
    ("none", &[]),
    ("centre-walk", &[("center_walk_underlay", "true")]),
    ("contour", &[("contour_underlay", "true")]),
    ("zigzag", &[("zigzag_underlay", "true")]),
    ("contour-zigzag", &[("contour_underlay", "true"), ("zigzag_underlay", "true")]),
];
/// How wide each column is (mm).
pub const WIDTH: f64 = 6.0;
/// How tall each column is (mm).
const TALL: f64 = 30.0;
/// The space between the columns (mm).
const GAP: f64 = 8.0;

/// Draws TS-06.
pub(super) fn build() -> Result<StitchPlan, SheetError> {
    let red = thread(RED)?;
    let mut d = Drawing::new("ts06");
    let mut x = 0.0;
    for (name, underlays) in UNDERLAYS {
        let rails = [[(x, 0.0), (x, TALL)], [(x + WIDTH, 0.0), (x + WIDTH, TALL)]];
        d.satin(name, rails, &red, &[underlays, &[("trim_after", "true")]].concat())?;
        x += WIDTH + GAP;
    }
    d.plan()
}
