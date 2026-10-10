//! Satin columns (milestone M4): a band of stitches that swing from one rail to the other, the look of
//! lettering and borders (`docs/src/design/algorithms/satin.md`).
//!
//! An element is a satin column when its `satin_column` setting is on, as in Ink/Stitch, which shows the
//! one setting on both its satin and its stroke tabs. Its path is first recognized as rails and rungs
//! ([`crate::normalize::satin`]): a path that cannot be a satin column gets an error and no stitches, and
//! what recognition took by length, stood in for or left out is named. Then the rails are turned the way
//! they are sewn and cut into sections at the rungs (the `column` module), and needle points are placed in
//! pairs across the column along the sections (`pairs`), sewn rail to rail.
//!
//! These are the top stitches as Ink/Stitch places them, and the later steps of M4 add the rest: pull
//! compensation (M4.3), short stitches on curves (M4.4), split stitches (M4.5) and underlays (M4.6). A
//! path of 1 subpath, sewn along its centre line, follows in M4.8.

mod column;
mod pairs;

use stitchcraft_core::{Diagnostic, Exhausted, Meter};
use stitchcraft_params::{ChoiceOption, StitchType, params};

use crate::design::Path;
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

/// The satin column `satin` sewn as `params` say: one run of needle points, a pair across the column at
/// a time, from the rails' starts to their ends.
pub fn satin_stitch(satin: &Satin, params: &SatinParams, meter: &mut Meter) -> Result<Stitched, Exhausted> {
    let mut warnings = Vec::new();
    let sections = column::sections(satin, params.swap_satin_rails, params.reverse_rails, &mut warnings, meter)?;
    let run = pairs::pairs(&sections, params.zigzag_spacing_mm.get(), meter)?.into_iter().flatten().collect();
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
