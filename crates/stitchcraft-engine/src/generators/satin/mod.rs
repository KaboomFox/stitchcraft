//! Satin columns (milestone M4): a band of stitches that swing from one rail to the other, the look of
//! lettering and borders (`docs/src/design/algorithms/satin.md`).
//!
//! An element is a satin column when its `satin_column` setting is on, as in Ink/Stitch, which shows the
//! one setting on both its satin and its stroke tabs. Its path is first recognized as rails and rungs
//! ([`crate::normalize::satin`]): a path that cannot be a satin column gets an error and no stitches, and
//! what recognition took by length, stood in for or left out is named. The stitches between the rails
//! come from M4.2 on, and a path of one subpath, sewn along its centre line, from M4.8.

use stitchcraft_core::{Diagnostic, Exhausted, Meter};
use stitchcraft_params::{StitchType, params};

use crate::design::Path;
use crate::normalize::satin::{Recognition, Shape, recognize};

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
    pub struct SatinParams for &[StitchType::SatinColumn];

    "Satin column" {
        /// Sew the path as a satin column. Two of its subpaths are the rails, the column's edges, and the
        /// others are rungs across both, which say which point of one rail goes with which of the other. A
        /// path of 1 subpath is the column's centre line. Off, the path is sewn as a stroke, by its
        /// `stroke_method`. This version reads a satin column's rails and rungs and reports what it finds,
        /// then skips the element (`SC-W0011`). Satin stitches arrive in a later version.
        satin_column: Toggle = "false", label "Satin column", applies STROKES_AND_SATINS;
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

#[cfg(test)]
mod tests {
    use stitchcraft_params::Family;

    use super::*;

    #[test]
    fn the_setting_applies_to_every_stroke_and_satin_stitch_type() {
        let along_paths: Vec<StitchType> = StitchType::ALL.iter().copied().filter(|t| matches!(t.family(), Family::Stroke | Family::Satin)).collect();
        assert_eq!(STROKES_AND_SATINS, along_paths);
    }
}
