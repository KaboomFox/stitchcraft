//! Tatami fill (milestone M5): parallel rows of running stitches across a fill's area, the needle points
//! of neighbouring rows offset so that they never line up into furrows (`docs/src/design/algorithms/fills.md`).
//!
//! A fill sews the drawing's shape with Ink/Stitch's stitches: its region is the area the drawing shows
//! ([`crate::normalize::region`]), the rows across it lie where Ink/Stitch lays them ([`rows`]), and the
//! needle points along each row lie where Ink/Stitch places them ([`stitches`]). Each part's rows are sewn
//! once each, in the order Ink/Stitch routes them (`route`, on a graph kept in Ink/Stitch's order by
//! `graph`, measured along the part's rings by `rings`), part after part ([`sew`]). Between rows the needle
//! travels under the rows sewn after it (`underpath`), kept inside the part (`clamp`), or with `underpath`
//! off along the outline (`travel`). Before its top rows each part sews its underlay: rows farther apart,
//! square to the top rows unless set otherwise, a layer for each angle, each sewn as the top rows are
//! ([`TatamiParams::underlay`]), on the part shrunk by the underlay's inset. The top rows cover the part
//! grown by `expand_mm` ([`TatamiParams::fill`]). Compensation follows in the rest of M5.

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
use stitchcraft_params::{Origin, StitchType, normalized_angle, params};

use crate::generators::running::RunningParams;
use crate::generators::tatami::rows::Grid;
use crate::generators::tatami::sew::{Fill, Layer, Travel};
use crate::generators::tatami::stitches::Stitching;

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

        /// How far past the drawn edge the top rows reach: each part is grown by this much, with round
        /// corners, before its rows are laid. The thread's pull opens gaps between fills sewn side by side,
        /// and this closes them. Negative values pull the rows in from the edge.
        expand_mm: Length = "0", label "Expand", range (-10.0, 10.0);
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

    "Underlay" {
        /// Sew underlay before the top rows: rows of stitches farther apart than the top rows, square to
        /// them unless set otherwise. It holds the fabric still while the fill is sewn and lifts the top
        /// rows, so less fabric shows between them.
        fill_underlay: Toggle = "true", label "Underlay";

        /// The direction of the underlay's rows, in degrees counter-clockwise from horizontal. Several
        /// angles separated by spaces sew a layer of underlay at each, one after another. Empty: square to
        /// the top rows, at the fill's angle plus 90°.
        fill_underlay_angle: OptionalAngleList = "", label "Underlay angle";

        /// How far apart the underlay's rows are, measured square to them. Empty: 3 times the row spacing.
        fill_underlay_row_spacing_mm: OptionalLength = "", label "Underlay row spacing", range (0.1, 100.0);

        /// How far apart the needle points along the underlay's rows are. Empty: the fill's longest stitch
        /// (`max_stitch_length_mm`).
        fill_underlay_max_stitch_length_mm: OptionalLength = "", label "Underlay longest stitch", range (0.1, 25.0);

        /// How far inside the fill's edge its underlay stops: each part is shrunk by this much, with round
        /// corners, before the underlay's rows are laid, so that none of the underlay shows beyond the top
        /// rows. Negative values let it reach past the edge.
        fill_underlay_inset_mm: Length = "0", label "Underlay inset", range (-10.0, 10.0);

        /// Leave out the needle point at the end of each underlay row, as `skip_last` does for the top rows.
        fill_underlay_skip_last: Toggle = "false", label "Skip last underlay stitch";

        /// Travel between the underlay's rows runs inside the fill, under the underlay rows sewn after it.
        /// Off, the needle travels along the fill's outline.
        underlay_underpath: Toggle = "true", label "Underlay underpath";
    }
}

impl TatamiParams {
    /// Where the rows lie.
    pub fn grid(&self) -> Grid {
        Grid { angle: self.angle, spacing: self.row_spacing_mm.get(), end_spacing: self.end_row_spacing_mm.map(Mm::get) }
    }

    /// How the rows are stitched, in stitches as long as the element's `longest`, with the random lengths
    /// its `running` settings turn on.
    pub fn stitching(&self, longest: Mm, running: &RunningParams) -> Stitching {
        Stitching {
            grid: self.grid(),
            length: longest.get(),
            staggers: self.staggers,
            skip_last: self.skip_last,
            jitter: running.enable_random_stitch_length.then_some(running.random_stitch_length_jitter_percent / 100.0),
        }
    }

    /// The fill as it is sewn: its top rows, stitched with the element's `longest` stitch and the random
    /// lengths its `running` settings turn on, over each part grown by `expand_mm`, after its underlay
    /// over each part shrunk by `fill_underlay_inset_mm`, the needle travelling as `travel` says.
    pub fn fill(&self, longest: Mm, running: &RunningParams, travel: Travel) -> Fill {
        Fill {
            top: Layer { stitching: self.stitching(longest, running), travel },
            expand: self.expand_mm.get(),
            underlay: self.underlay(longest, travel),
            inset: self.fill_underlay_inset_mm.get(),
        }
    }

    /// The layers of underlay, one for each underlay angle in turn, or none with `fill_underlay` off, as
    /// Ink/Stitch sews them. With no angle set, one square to the top rows. A layer's rows lie evenly the
    /// underlay's row spacing apart (3 times the row spacing unless set), and are stitched on the stagger
    /// grid with the fill's staggers, in stitches of the underlay's longest stitch (the element's
    /// `longest` unless set). The needle travels as `travel` says, under the rows with
    /// `underlay_underpath`.
    pub fn underlay(&self, longest: Mm, travel: Travel) -> Vec<Layer> {
        if !self.fill_underlay {
            return Vec::new();
        }
        let spacing = self.fill_underlay_row_spacing_mm.map_or(3.0 * self.row_spacing_mm.get(), Mm::get);
        let length = self.fill_underlay_max_stitch_length_mm.unwrap_or(longest).get();
        let angles = self.fill_underlay_angle.clone().unwrap_or_else(|| vec![normalized_angle(self.angle + 90.0)]);
        let travel = Travel { underpath: self.underlay_underpath, ..travel };
        let layer = |angle| Layer {
            stitching: Stitching {
                grid: Grid { angle, spacing, end_spacing: None },
                length,
                staggers: self.staggers,
                skip_last: self.fill_underlay_skip_last,
                jitter: None,
            },
            travel,
        };
        angles.into_iter().map(layer).collect()
    }
}

#[cfg(test)]
mod tests {
    use stitchcraft_params::ParamSet;

    use super::*;

    const TRAVEL: Travel = Travel { length: 2.5, tolerance: 0.2, min_stitch: 0.3, underpath: false, jump: 3.0 };

    /// The tatami settings `pairs` give.
    fn read(pairs: &[(&str, &str)]) -> TatamiParams {
        TatamiParams::from_set(&pairs.iter().copied().collect::<ParamSet>()).unwrap().params
    }

    #[test]
    fn req_fill_tat_011_the_underlay_s_layers_come_from_its_settings() {
        let longest = Mm::new(4.0).unwrap();
        // By default one layer, square to the top rows and 3 times as far apart, in the fill's longest stitch
        // and with its staggers, travelling under its rows whatever `underpath` says. A layer's rows are
        // evenly spaced, and stitched on the stagger grid.
        let fill = read(&[("angle", "30"), ("row_spacing_mm", "0.4"), ("staggers", "3"), ("end_row_spacing_mm", "1"), ("underpath", "false")]);
        let stitching = Stitching {
            grid: Grid { angle: 120.0, spacing: 3.0 * 0.4, end_spacing: None },
            length: 4.0,
            staggers: 3.0,
            skip_last: false,
            jitter: None,
        };
        assert_eq!(fill.underlay(longest, TRAVEL), [Layer { stitching, travel: Travel { underpath: true, ..TRAVEL } }]);
        // The fill's angle plus 90°, turned into (−180, 180] as an angle typed in is.
        assert_eq!(read(&[("angle", "135")]).underlay(longest, TRAVEL)[0].stitching.grid.angle, -135.0);
        // Its own settings, a layer for each angle in turn.
        let own = read(&[
            ("fill_underlay_angle", "0 -45"),
            ("fill_underlay_row_spacing_mm", "2"),
            ("fill_underlay_max_stitch_length_mm", "3"),
            ("fill_underlay_skip_last", "true"),
            ("underlay_underpath", "false"),
        ]);
        let layers = own.underlay(longest, Travel { underpath: true, ..TRAVEL });
        assert_eq!(layers.iter().map(|layer| layer.stitching.grid.angle).collect::<Vec<_>>(), [0.0, -45.0]);
        for layer in layers {
            let Stitching { grid, length, skip_last, .. } = layer.stitching;
            assert_eq!((grid.spacing, length, skip_last, layer.travel.underpath), (2.0, 3.0, true, false));
        }
        assert_eq!(read(&[("fill_underlay", "false")]).underlay(longest, TRAVEL), Vec::new());
    }

    #[test]
    fn req_fill_tat_012_the_fill_grows_its_top_rows_and_shrinks_its_underlay_as_set() {
        let longest = Mm::new(4.0).unwrap();
        let running = RunningParams::from_set(&ParamSet::new()).unwrap().params;
        let set = [("angle", "10"), ("expand_mm", "0.5"), ("fill_underlay_inset_mm", "-0.25")];
        let fill = read(&set).fill(longest, &running, TRAVEL);
        assert_eq!((fill.expand, fill.inset), (0.5, -0.25));
        assert_eq!(fill.top, Layer { stitching: read(&set).stitching(longest, &running), travel: TRAVEL });
        assert_eq!(fill.underlay, read(&set).underlay(longest, TRAVEL));
        // Neither, by default.
        let plain = read(&[]).fill(longest, &running, TRAVEL);
        assert_eq!((plain.expand, plain.inset), (0.0, 0.0));
    }
}
