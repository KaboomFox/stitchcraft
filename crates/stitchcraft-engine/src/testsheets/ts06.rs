//! TS-06 — satin underlays: the same column with each underlay, to choose the defaults by how the edges,
//! the loft and the fabric come out.
//!
//! Drawn as a design (`designed`), so the engine's satin column and its underlays sew it, with
//! Ink/Stitch's defaults for everything else. 5 columns 6 mm wide and 30 mm tall, left to right: no
//! underlay, a centre walk, a contour, a zigzag, and a contour with a zigzag. Each column is sewn as drawn,
//! not from its nearest point, and trimmed after, so the columns stand apart.
//!
//! The reference machine cuts no jump (MC-1), so no column starts along the thread of the jump to it. The
//! columns are sewn down and up in turn, each from its corner nearest where the one before ended, and the
//! centre walk's first: its underlay starts in the middle of the column's end, along which a jump from beside
//! it would run. Sewn up or from the right, a column's zigzag is its mirror image.

use stitchcraft_plan::StitchPlan;

use super::designed::{AS_DRAWN, Drawing, rails};
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
/// The columns' places from the left, in the order they are sewn: the centre walk, then the others from the
/// left.
pub const SEWN: [usize; 5] = [1, 0, 2, 3, 4];
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
    // Each column's left edge, with its name and underlays, left to right.
    let mut columns = Vec::with_capacity(UNDERLAYS.len());
    let mut x = 0.0;
    for (name, underlays) in UNDERLAYS {
        columns.push((x, name, underlays));
        x += WIDTH + GAP;
    }
    let (mut down, mut before) = (true, None);
    for place in SEWN {
        let Some(&(x, name, underlays)) = columns.get(place) else { continue };
        // From the right only when the column before lies to the right.
        let from_left = before.is_none_or(|before| before < place);
        d.satin(name, rails((x, x + WIDTH), (0.0, TALL), down, from_left), &red, &[underlays, &AS_DRAWN[..]].concat())?;
        (down, before) = (!down, Some(place));
    }
    d.plan()
}
