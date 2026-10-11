//! Tatami fill (milestone M5): parallel rows of running stitches across a fill's area, the needle points
//! of neighbouring rows offset so that they never line up into furrows (`docs/src/design/algorithms/fills.md`).
//!
//! A fill sews the drawing's shape with Ink/Stitch's stitches: its region is the area the drawing shows
//! ([`crate::normalize::region`]), the rows across it lie where Ink/Stitch lays them ([`rows`]), and the
//! needle points along each row lie where Ink/Stitch places them ([`stitches`]). Each part's rows are sewn
//! once each, in the order Ink/Stitch routes them (`route`, on a graph kept in Ink/Stitch's order by
//! `graph`, measured along the part's rings by `rings`), part after part ([`sew`]). Between rows the needle
//! travels under the rows sewn after it (`underpath`), kept inside the part (`clamp`), or with `underpath`
//! off along the outline (`travel`). Underlay and compensation follow in the rest of M5.

mod clamp;
#[cfg(test)]
mod fixture;
mod graph;
mod rings;
mod route;
pub mod rows;
pub mod sew;
pub mod stitches;
mod travel;
mod underpath;

use stitchcraft_core::Mm;
use stitchcraft_params::{Origin, StitchType, params};

use crate::generators::running::RunningParams;

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

    "Stitches" {
        /// How many rows the needle points take to come back to where they started. Along each row they lie
        /// the longest stitch (`max_stitch_length_mm`) apart, a `staggers`-th of a stitch along from the
        /// row before's, so that neighbouring rows never line them up. A fraction draws diagonals that show
        /// less than whole numbers do.
        staggers: Number = "4", label "Staggers", range (0.01, 100.0);

        /// Leave out the needle point at the end of each row. It lies close to the start of the next row,
        /// and leaving it out lowers the stitch count and the density along the fill's edge.
        skip_last: Toggle = "false", label "Skip last stitch";
    }

    "Travel" {
        /// Travel between rows runs inside the fill, under rows sewn after it, which hide it. Off, the
        /// needle travels along the fill's outline, where the travel can show along the edge.
        underpath: Toggle = "true", label "Underpath";
    }
}

impl TatamiParams {
    /// Where the rows lie.
    pub fn grid(&self) -> rows::Grid {
        rows::Grid { angle: self.angle, spacing: self.row_spacing_mm.get(), end_spacing: self.end_row_spacing_mm.map(Mm::get) }
    }

    /// How the rows are stitched, in stitches as long as the element's `longest`, with the random lengths
    /// its `running` settings turn on.
    pub fn stitching(&self, longest: Mm, running: &RunningParams) -> stitches::Stitching {
        stitches::Stitching {
            grid: self.grid(),
            length: longest.get(),
            staggers: self.staggers,
            skip_last: self.skip_last,
            jitter: running.enable_random_stitch_length.then_some(running.random_stitch_length_jitter_percent / 100.0),
        }
    }
}
