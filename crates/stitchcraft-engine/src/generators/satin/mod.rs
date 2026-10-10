//! Satin columns (milestone M4): a band of stitches that swing from one rail to the other, the look of
//! lettering and borders (`docs/src/design/algorithms/satin.md`).
//!
//! An element is a satin column when its `satin_column` setting is on, as in Ink/Stitch, which shows the
//! one setting on both its satin and its stroke tabs. Its path is first recognized as rails and rungs
//! ([`crate::normalize::satin`]): a path that cannot be a satin column gets an error and no stitches, and
//! what recognition took by length, stood in for or left out is named. Then the rails are turned the way
//! they are sewn, shortened or lengthened by push compensation and cut into sections at the rungs (the
//! `column` module), and needle points are placed in pairs across the column along the sections (`pairs`),
//! each pair widened by pull compensation (`compensation`). Needle points that crowd together on a rail
//! are inset (`short`), and the pairs are sewn rail to rail, with long stitches split (`split`).
//!
//! These are the top stitches as Ink/Stitch places them, and the next step of M4 adds underlays (M4.6).
//! A path of 1 subpath, sewn along its centre line, follows in M4.8.

mod column;
mod compensation;
mod pairs;
mod short;
mod split;

use stitchcraft_core::rng::SplitMix64;
use stitchcraft_core::{Diagnostic, Exhausted, Meter, Mm};
use stitchcraft_params::{ChoiceOption, StitchType, params};

use crate::design::Path;
use crate::generators::satin::compensation::Processor;
use crate::generators::satin::split::Splitter;
use crate::generators::{Stitched, method};
use crate::normalize::satin::{Recognition, Satin, Shape, recognize};

/// The satin methods `satin_method` offers, in Ink/Stitch's order, which its files count on: Ink/Stitch
/// gives the parameter's default as the first one's place in this list.
pub const SATIN_METHODS: &[ChoiceOption] =
    &[method(StitchType::SatinColumn), method(StitchType::EStitch), method(StitchType::SStitch), method(StitchType::SatinZigzag)];

/// The stitch types of satin columns, one per method.
const SATINS: &[StitchType] = &[StitchType::SatinColumn, StitchType::EStitch, StitchType::SStitch, StitchType::SatinZigzag];

/// The stitch types the `satin_column` setting chooses between: those of strokes, with it off, and those
/// of satin columns, with it on.
const STROKES_AND_SATINS: &[StitchType] = &[
    StitchType::RunningStitch,
    StitchType::ManualStitch,
    StitchType::ZigzagStitch,
    StitchType::RippleStitch,
    StitchType::SatinColumn,
    StitchType::EStitch,
    StitchType::SStitch,
    StitchType::SatinZigzag,
];

params! {
    /// Satin column: a band of stitches between two rails.
    pub struct SatinParams for SATINS;

    "Satin column" {
        /// Sew the path as a satin column. Two of its subpaths are the rails, the column's edges, and the
        /// others are rungs across both, which say which point of one rail goes with which of the other.
        /// Without rungs, the rails' nodes pair up instead. A path of 1 subpath is the column's centre
        /// line, which a later version sews. Off, the path is sewn as a stroke, by its `stroke_method`.
        satin_column: Toggle = "false", label "Satin column", applies STROKES_AND_SATINS;

        /// How the column is sewn. A satin column sews stitches straight across it, from one rail to the
        /// other and back. E, S and zigzag stitches arrive in later versions, and until then an element
        /// set to one is skipped (`SC-W0011`).
        satin_method: Choice = "satin_column", label "Method", choices SATIN_METHODS;

        /// The distance from one stitch across the column to the next that goes the same way: from one
        /// rail to the other and back again is one spacing. It is measured across the column at its
        /// outside edge, and on a curve the stitches fan out from the inside edge.
        zigzag_spacing_mm: Length = "0.4", label "Zigzag spacing", range (0.01, 10.0);
    }

    "Rails" {
        /// Which rails are sewn against the way they are drawn, so that both run the same way. Automatic
        /// reverses the second rail when it runs against the first.
        reverse_rails: Choice = "automatic", label "Reverse rails",
            options ["automatic" => "Automatic", "none" => "Neither", "first" => "The first", "second" => "The second", "both" => "Both"];

        /// Make the second rail the first. The column starts on its first rail, and each stitch across it
        /// goes from the first rail to the second.
        swap_satin_rails: Toggle = "false", label "Swap rails";
    }

    "Compensation" {
        /// How far each end of every stitch reaches past its rail. The thread pulls the fabric in across the
        /// column as it sews, so a satin comes out narrower than drawn, and this makes up for it. Negative
        /// values make the column narrower. 2 values set the first rail's side, then the second's.
        pull_compensation_mm: LengthPair = "0", label "Pull compensation", range (-10.0, 10.0);

        /// More pull compensation, in percent of the column's width at each stitch, added to the length
        /// above: wide parts of a column reach out further than narrow ones. 2 values set the first rail's
        /// side, then the second's.
        pull_compensation_percent: PercentPair = "0", label "Pull compensation (% of width)", range (-100.0, 100.0);

        /// How much shorter the column is made at its start and its end. Satin stitches push the fabric out
        /// along the column, so it comes out longer than drawn, and this makes up for it. Negative values
        /// lengthen the column. 2 values set the start, then the end.
        push_compensation_mm: LengthPair = "0", label "Push compensation", range (-10.0, 10.0);
    }

    "Random variation" {
        /// How much narrower than the compensated column a stitch may come out on each side, chosen at
        /// random for each stitch, in percent of the column's width there. A ragged edge looks like fur or
        /// grass. 2 values set the first rail's side, then the second's.
        random_width_decrease_percent: PercentPair = "0", label "Random width decrease", range (0.0, 100.0);

        /// How much wider than the compensated column a stitch may come out on each side, chosen at random
        /// for each stitch, in percent of the column's width there. 2 values set the first rail's side, then
        /// the second's.
        random_width_increase_percent: PercentPair = "0", label "Random width increase", range (0.0, 100.0);

        /// How much the distance to each stitch may differ from the zigzag spacing, chosen at random, in
        /// percent of the spacing, longer or shorter.
        random_zigzag_spacing_percent: Percent = "0", label "Random zigzag spacing", range (0.0, 100.0);
    }

    "Short stitches" {
        /// How far a crowded needle point is moved in along its stitch, in percent of the stitch's width.
        /// On the inside of a tight curve the needle points of a rail crowd together, and the thread piles
        /// up there. Moving some of them in spreads them out. Points that crowd one after another take
        /// turns with several values separated by spaces.
        short_stitch_inset: PercentList = "15", label "Short stitch inset", range (0.0, 50.0);

        /// How close a needle point may come to the last one left in place on its rail before it is moved
        /// in. 0 moves none.
        short_stitch_distance_mm: Length = "0.25", label "Short stitch distance", range (0.0, 5.0);
    }

    "Split stitches" {
        /// How stitches longer than the longest stitch (`max_stitch_length_mm`) are split. Default splits
        /// each into the fewest equal parts no longer than it. Simple splits at whole multiples of it from
        /// the stitch's start. Staggered moves those splits along from one stitch to the next, so the needle
        /// holes of neighbouring stitches do not line up in a row.
        split_method: Choice = "default", label "Split method",
            options ["default" => "Default", "simple" => "Simple", "staggered" => "Staggered"];

        /// How far each split may move at random, in percent of a part, either way. With a random split
        /// phase, how much each part's length may vary instead.
        random_split_jitter_percent: Percent = "0", label "Split jitter", range (0.0, 100.0), when split_method == "default";

        /// Start each stitch's splits at a random distance from its start, and space them by the longest
        /// stitch, instead of dividing the stitch evenly. The needle holes of neighbouring stitches then
        /// fall apart, at the cost of a few more stitches.
        random_split_phase: Toggle = "false", label "Random split phase", when split_method == "default";

        /// With a random split phase, also split stitches longer than this but no longer than the longest
        /// stitch. Empty: the longest stitch.
        min_random_split_length_mm: OptionalLength = "", label "Shortest split stitch", range (0.1, 25.0), when split_method == "default";

        /// How many stitches the staggered splits take to come back to where they started. A fraction draws
        /// diagonals that show less than whole numbers do.
        split_staggers: Number = "4", label "Staggers", range (0.01, 100.0), when split_method == "staggered";
    }
}

/// What a satin column's `path` is, with what recognition took by length, stood in for or left out
/// added to `diagnostics`; `None`, with the reason added, when the path cannot be a satin column.
pub fn shape(path: &Path, diagnostics: &mut Vec<Diagnostic>, meter: &mut Meter) -> Result<Option<Shape>, Exhausted> {
    let Recognition { shape, warnings } = recognize(path, meter)?;
    diagnostics.extend(warnings);
    Ok(match shape {
        Ok(shape) => Some(shape),
        Err(error) => {
            diagnostics.push(error);
            None
        }
    })
}

/// The satin column `satin` sewn as `params` say, for an element whose shortest stitch is `min_stitch` and
/// longest `max_stitch`, if it sets one, with its random variation drawn from the element's `rng`: one run
/// of needle points, a pair across the column at a time, from the rails' starts to their ends.
pub fn satin_stitch(
    satin: &Satin,
    params: &SatinParams,
    min_stitch: Mm,
    max_stitch: Option<Mm>,
    rng: &mut SplitMix64,
    meter: &mut Meter,
) -> Result<Stitched, Exhausted> {
    let mut warnings = Vec::new();
    let sections = column::sections(satin, params, &mut warnings, meter)?;
    let placed = pairs::pairs(&sections, params.zigzag_spacing_mm.get(), &mut Processor::new(params, rng), meter)?;
    let mut splitter = Splitter::new(params, max_stitch.map(Mm::get), min_stitch.get(), rng);
    let insets: Vec<f64> = params.short_stitch_inset.iter().map(|percent| percent / 100.0).collect();
    let short = short::inset(&placed, params.short_stitch_distance_mm.get(), &insets, splitter.inset_limit());
    let run = splitter.sew(&placed, &short, meter)?;
    Ok(Stitched { runs: vec![run], warnings })
}

#[cfg(test)]
mod tests {
    use stitchcraft_params::Family;

    use super::*;

    #[test]
    fn the_setting_applies_to_every_stroke_and_satin_stitch_type() {
        let of = |families: &[Family]| -> Vec<StitchType> { StitchType::ALL.iter().copied().filter(|t| families.contains(&t.family())).collect() };
        assert_eq!(STROKES_AND_SATINS, of(&[Family::Stroke, Family::Satin]));
        assert_eq!(SATINS, of(&[Family::Satin]));
    }
}
