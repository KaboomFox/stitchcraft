//! Stitch types, named by their Ink/Stitch method ids.
//!
//! The ids are the interoperability contract (`conformance/inkstitch-params.toml`): an SVG says
//! `inkstitch:fill_method="contour_fill"`, and StitchCraft reads the same id. The list is the whole
//! vocabulary, including stitch types that arrive in later milestones, so parameters can say which types
//! they apply to from the start.

/// The three families of shapes a stitch type works on; each has its own method parameter.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Family {
    /// Stitches along a path.
    Stroke,
    /// A column of zigzag stitches between two rails.
    Satin,
    /// Stitches that cover an area.
    Fill,
}

impl Family {
    /// The parameter that chooses the stitch type within the family: `stroke_method`, `satin_method` or
    /// `fill_method`.
    pub const fn method_param(self) -> &'static str {
        match self {
            Family::Stroke => "stroke_method",
            Family::Satin => "satin_method",
            Family::Fill => "fill_method",
        }
    }
}

/// A way of turning a shape into stitches.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum StitchType {
    /// Running stitch along a path (with repeats and bean stitch).
    RunningStitch,
    /// Exactly the path's nodes, as stitches.
    ManualStitch,
    /// Zigzag along a path.
    ZigzagStitch,
    /// Ripple: repeated, morphing copies of a path.
    RippleStitch,
    /// Satin column between two rails.
    SatinColumn,
    /// E-stitch (blanket stitch) along a satin column.
    EStitch,
    /// S-stitch along a satin column.
    SStitch,
    /// Zigzag between the rails of a satin column.
    SatinZigzag,
    /// Tatami: rows of running stitch across an area.
    TatamiFill,
    /// Rings following the area's outline.
    ContourFill,
    /// A meandering pattern across the area.
    MeanderFill,
    /// A spiral around a centre.
    CircularFill,
    /// Rows that follow a guide line.
    GuidedFill,
    /// Rows whose spacing changes across the area.
    LinearGradientFill,
    /// A tartan pattern.
    TartanFill,
    /// Cross stitches on a grid.
    CrossStitch,
    /// Ink/Stitch's earlier fill algorithm.
    LegacyFill,
}

impl StitchType {
    /// Every stitch type, in a fixed order.
    pub const ALL: &'static [StitchType] = &[
        StitchType::RunningStitch,
        StitchType::ManualStitch,
        StitchType::ZigzagStitch,
        StitchType::RippleStitch,
        StitchType::SatinColumn,
        StitchType::EStitch,
        StitchType::SStitch,
        StitchType::SatinZigzag,
        StitchType::TatamiFill,
        StitchType::ContourFill,
        StitchType::MeanderFill,
        StitchType::CircularFill,
        StitchType::GuidedFill,
        StitchType::LinearGradientFill,
        StitchType::TartanFill,
        StitchType::CrossStitch,
        StitchType::LegacyFill,
    ];

    /// The Ink/Stitch method id, such as `running_stitch` or `tatami_fill`.
    pub const fn id(self) -> &'static str {
        match self {
            StitchType::RunningStitch => "running_stitch",
            StitchType::ManualStitch => "manual_stitch",
            StitchType::ZigzagStitch => "zigzag_stitch",
            StitchType::RippleStitch => "ripple_stitch",
            StitchType::SatinColumn => "satin_column",
            StitchType::EStitch => "e_stitch",
            StitchType::SStitch => "s_stitch",
            StitchType::SatinZigzag => "zigzag",
            StitchType::TatamiFill => "tatami_fill",
            StitchType::ContourFill => "contour_fill",
            StitchType::MeanderFill => "meander_fill",
            StitchType::CircularFill => "circular_fill",
            StitchType::GuidedFill => "guided_fill",
            StitchType::LinearGradientFill => "linear_gradient_fill",
            StitchType::TartanFill => "tartan_fill",
            StitchType::CrossStitch => "cross_stitch",
            StitchType::LegacyFill => "legacy_fill",
        }
    }

    /// Its name, for people: `Running stitch`, `Tatami fill`.
    pub const fn name(self) -> &'static str {
        match self {
            StitchType::RunningStitch => "Running stitch",
            StitchType::ManualStitch => "Manual stitch",
            StitchType::ZigzagStitch => "Zigzag stitch",
            StitchType::RippleStitch => "Ripple stitch",
            StitchType::SatinColumn => "Satin column",
            StitchType::EStitch => "E-stitch",
            StitchType::SStitch => "S-stitch",
            StitchType::SatinZigzag => "Zigzag satin",
            StitchType::TatamiFill => "Tatami fill",
            StitchType::ContourFill => "Contour fill",
            StitchType::MeanderFill => "Meander fill",
            StitchType::CircularFill => "Circular fill",
            StitchType::GuidedFill => "Guided fill",
            StitchType::LinearGradientFill => "Linear gradient fill",
            StitchType::TartanFill => "Tartan fill",
            StitchType::CrossStitch => "Cross stitch",
            StitchType::LegacyFill => "Legacy fill",
        }
    }

    /// The family it belongs to.
    pub const fn family(self) -> Family {
        match self {
            StitchType::RunningStitch | StitchType::ManualStitch | StitchType::ZigzagStitch | StitchType::RippleStitch => Family::Stroke,
            StitchType::SatinColumn | StitchType::EStitch | StitchType::SStitch | StitchType::SatinZigzag => Family::Satin,
            _ => Family::Fill,
        }
    }

    /// The stitch type of `family` whose method id is `id`. Ids are only unique within a family:
    /// `zigzag` is a satin method, `zigzag_stitch` a stroke method.
    pub fn from_id(family: Family, id: &str) -> Option<StitchType> {
        StitchType::ALL.iter().copied().find(|t| t.family() == family && t.id() == id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_round_trip_within_each_family() {
        for t in StitchType::ALL {
            assert_eq!(StitchType::from_id(t.family(), t.id()), Some(*t));
        }
        assert_eq!(StitchType::from_id(Family::Satin, "zigzag"), Some(StitchType::SatinZigzag));
        assert_eq!(StitchType::from_id(Family::Stroke, "zigzag"), None);
        assert_eq!(Family::Fill.method_param(), "fill_method");
    }

    #[test]
    fn names_are_distinct() {
        let names: std::collections::BTreeSet<&str> = StitchType::ALL.iter().map(|t| t.name()).collect();
        assert_eq!(names.len(), StitchType::ALL.len());
    }
}
