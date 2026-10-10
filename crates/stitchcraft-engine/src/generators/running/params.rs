//! The running stitch's parameters, declared once with Ink/Stitch's keys and defaults, which are the
//! interoperability contract (`conformance/inkstitch-params.toml`; `cargo xtask docs --check` compares the
//! two). Ripple stitch (M10) sews each of its lines with the same settings. The stitches that join a satin
//! column's underlays are no longer than the running stitch's length, and its way to its start and from
//! its end keeps to the running stitch's length and tolerance, which Ink/Stitch stores under the same keys.

use stitchcraft_params::{Origin, StitchType, params};

/// The stitch types that sew stitches of the running stitch's length and tolerance: running and ripple
/// stitch along their lines, and satin columns, whatever their method, between their underlays and on
/// their way to their start and from their end.
const RUNS: &[StitchType] = &[
    StitchType::RunningStitch,
    StitchType::RippleStitch,
    StitchType::SatinColumn,
    StitchType::EStitch,
    StitchType::SStitch,
    StitchType::SatinZigzag,
];

params! {
    /// Running stitch: single stitches along the path, for outlines, details and travel.
    pub struct RunningParams for &[StitchType::RunningStitch, StitchType::RippleStitch];

    "Running stitch" {
        /// How long each stitch is. Between corners the stitches are spread evenly, so each one is at most
        /// this long. Several lengths separated by spaces sew as a repeating pattern: "2.5 1" sews long,
        /// short, long, short. The stitches that join a satin column's underlays are no longer than the
        /// first length.
        running_stitch_length_mm: LengthList = "2.5", label "Stitch length", range (0.1, 25.0), applies RUNS;

        /// How far a stitch may stray from a curve. A smaller tolerance follows curves more closely, with
        /// more and shorter stitches. A satin column's way to its start and from its end keeps to it too.
        running_stitch_tolerance_mm: Length = "0.2", label "Curve tolerance", range (0.01, 5.0), applies RUNS,
            origin Origin::InkStitchDeviates { deviation: "DEV-SAT-006" };

        /// Vary the stitch lengths at random instead of spreading them evenly. Lines sewn close together
        /// then do not line their needle holes up, which avoids moiré patterns.
        enable_random_stitch_length: Toggle = "false", label "Random stitch length";

        /// How much each stitch may be longer or shorter than the stitch length, in percent of it. Where the
        /// random lengths start is the element's `random_seed`.
        random_stitch_length_jitter_percent: Percent = "10", label "Length variation", range (0.0, 100.0),
            when enable_random_stitch_length == "true";
    }
}
