//! The settings of automatic digitizing's rules (`REQ-AUTO-020`): StitchCraft's own parameters, registered
//! as any other (`docs/src/design/params.md`), which no stitch type reads.
//!
//! Each rule goes by a value: the widest satin column, the narrowest triple run, the overlap, the smallest
//! area, and a switch that leaves the background out (`docs/src/design/image-to-embroidery.md` ›
//! Settings). MC-8 confirms the values on the machine or changes them, and a picture can need others, so
//! each is a parameter of the shape it digitizes, read when the shape is digitized ([`AutoParams`]). A
//! shape that sets none takes the defaults, as every shape does with `--auto`, and the requirements give
//! the defaults as their numbers. VectorCraft's Auto effect (M13.7) sets them from its dialog, for the
//! object it is on and the paths inside the group or the layer it is on.
//!
//! They apply to no stitch type ([`AUTOMATIC`]): an automatic element reads them before it is sewn as any,
//! and the elements it becomes don't, so no stitch family's dialog shows them. The registry's audit keeps
//! them StitchCraft's own, since Ink/Stitch digitizes nothing automatically.

use stitchcraft_params::{AUTOMATIC, Origin, Stability, params};

use crate::design::DesignSettings;
use crate::generators::satin::LONG_STITCH;

/// The key of the widest satin column.
pub const MAX_SATIN_WIDTH: &str = "auto_max_satin_width_mm";
/// The key of the narrowest triple run.
pub const MIN_TRIPLE_RUN_WIDTH: &str = "auto_min_triple_run_width_mm";
/// The key of the overlap.
pub const OVERLAP: &str = "auto_overlap_mm";
/// The key of the smallest area.
pub const MIN_AREA: &str = "auto_min_area_mm2";
/// The key of the switch that leaves the background out.
pub const IGNORE_BACKGROUND: &str = "auto_ignore_background";

params! {
    /// Automatic digitizing: the settings of the rules that choose the stitches of a shape without settings
    /// by its width. Each rule goes by the settings of the shape it digitizes, and a shape that sets none
    /// takes the defaults, as every shape does with `--auto`. The machine checkpoint MC-8 confirms the
    /// defaults or changes them.
    pub struct AutoParams for AUTOMATIC;

    "Stitches by width" {
        /// The widest a line or a part of an area sews as a satin column: satin stitches any longer lie
        /// loose. A line up to this wide is a satin column as wide as the line, and a wider one fills the
        /// area it paints. A part of an area no wider than this everywhere sews as satin columns between its
        /// outlines, and a part wider somewhere is filled, with its narrow parts sewn over the fill as lines
        /// and columns.
        auto_max_satin_width_mm: Length = "7", label "Widest satin column",
            range (DesignSettings::MIN_SATIN_STROKE_WIDTH.get(), LONG_STITCH),
            origin Origin::StitchCraft, stability Stability::Experimental;

        /// The narrowest running stitch sewn as a triple run, each stitch sewn 3 times (bean stitch 1). A
        /// line no wider than the design's narrowest satin stroke runs, and from this width it sews bolder
        /// than one stitch, which is as fine as the thread. A narrow part of an area is as wide as the
        /// circles that fit inside it along its centre line.
        auto_min_triple_run_width_mm: Length = "0.5", label "Narrowest triple run", range (0.0, 10.0),
            origin Origin::StitchCraft, stability Stability::Experimental;
    }

    "Neighbours" {
        /// How far an area reaches under each shape sewn after it that it touches, and how far inside the
        /// edge of an area sewn after it that covers it the area is cut away. The thread pulls stitches in
        /// from the edges of their shape, and without the overlap fabric shows between shapes that only
        /// meet. A satin column reaches as far past its rails, by its pull compensation.
        auto_overlap_mm: Length = "0.4", label "Overlap", range (0.0, 2.0),
            origin Origin::StitchCraft, stability Stability::Experimental;

        /// The smallest area sewn. An area, or a part of one, smaller than this is a speck, which would sew a
        /// knot: it is merged into the area beside it that it shares the longest edge with, or left out
        /// when it shares none.
        auto_min_area_mm2: Area = "1", label "Smallest area", range (0.0, 10.0),
            origin Origin::StitchCraft, stability Stability::Experimental;

        /// Whether the lowest shape is left out when it is the picture's background: an area under the
        /// other shapes that covers the whole design. A background is usually the fabric's own colour, and
        /// sewing it doubles the stitches and stiffens the design. Off, it sews.
        auto_ignore_background: Toggle = "true", label "Leave out the background",
            origin Origin::StitchCraft, stability Stability::Experimental;
    }
}

#[cfg(test)]
impl AutoParams {
    /// The settings of a shape that sets none of the rules', each its default.
    pub(crate) fn defaults() -> AutoParams {
        AutoParams::from_set(&stitchcraft_params::ParamSet::new()).unwrap().params
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_key_names_a_setting_in_the_order_they_are_declared() {
        let keys: Vec<&str> = AutoParams::SPECS.iter().map(|spec| spec.key).collect();
        assert_eq!(keys, [MAX_SATIN_WIDTH, MIN_TRIPLE_RUN_WIDTH, OVERLAP, MIN_AREA, IGNORE_BACKGROUND]);
    }
}
