//! Tatami fill (milestone M5): parallel rows of running stitches across a fill's area, the needle points
//! of neighbouring rows offset so that they never line up into furrows (`docs/src/design/algorithms/fills.md`).
//!
//! A fill sews the drawing's shape with Ink/Stitch's stitches: its region is the area the drawing shows
//! ([`crate::normalize::region`]), and the rows across it lie where Ink/Stitch lays them ([`rows`]).
//! Needle points along the rows, the order the rows are sewn in, travel, underlay and compensation follow
//! in the rest of M5. Until fills are sewn, their settings are read and checked, so that a value out of
//! range is said now.

pub mod rows;

use stitchcraft_core::Mm;
use stitchcraft_params::{Origin, StitchType, params};

/// The stitch types a tatami fill's settings apply to.
const TATAMI: &[StitchType] = &[StitchType::TatamiFill];

params! {
    /// Tatami fill: rows of running stitches across the fill's area, the needle points of neighbouring rows
    /// offset so that they never line up. Ink/Stitch's default fill.
    pub struct TatamiParams for TATAMI;

    "Rows" {
        /// The direction of the rows, in degrees counter-clockwise from horizontal: 0 sews horizontal rows
        /// and 90 upright ones.
        angle: Angle = "0", label "Angle";

        /// How far apart the rows are, measured square to them. Rows closer together cover the fabric more
        /// fully, and pull it in more.
        row_spacing_mm: Length = "0.25", label "Row spacing", range (0.1, 100.0);

        /// The spacing of the last rows, for a fill that fades: from the first row the spacing changes
        /// steadily towards it, reaching it the fill's height away. Empty, every row is the row spacing from
        /// the next.
        end_row_spacing_mm: OptionalLength = "", label "End row spacing", range (0.1, 100.0),
            origin Origin::InkStitchDeviates { deviation: "DEV-FILL-003" };
    }
}

impl TatamiParams {
    /// Where the rows lie.
    pub fn grid(&self) -> rows::Grid {
        rows::Grid { angle: self.angle, spacing: self.row_spacing_mm.get(), end_spacing: self.end_row_spacing_mm.map(Mm::get) }
    }
}
