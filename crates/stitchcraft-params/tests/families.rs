//! A key declared once for each family of stitch types (REQ-PRM-005). Ink/Stitch's `zigzag_spacing_mm` is
//! a list of lengths for a zigzag stroke and one length for a satin column, and a meander fill reads it as
//! a length that is 0 by default. Each element reads the key as its family's declaration says.

// Test code may unwrap (clippy.toml allows it inside #[test] functions; these helpers are test code too).
#![allow(clippy::unwrap_used)]

use stitchcraft_core::Code;
use stitchcraft_params::{Family, Kind, ParamGroup, ParamSet, StitchType, audit, find_for, params};

params! {
    /// A spacing, as a satin column reads it.
    pub struct SatinSpacingParams for &[StitchType::SatinColumn, StitchType::EStitch];

    "Spacing" {
        /// The spacing.
        a_spacing_mm: Length = "0.4", label "Spacing", range (0.01, 10.0);
    }
}

params! {
    /// The spacing, as a zigzag stroke reads it: a list.
    pub struct StrokeSpacingParams for &[StitchType::ZigzagStitch];

    "Spacing" {
        /// The spacings, in turn.
        a_spacing_mm: LengthList = "0.4", label "Spacing", range (0.01, 10.0);
    }
}

params! {
    /// The spacing, as a meander fill reads it: none by default.
    pub struct FillSpacingParams for &[StitchType::MeanderFill];

    "Spacing" {
        /// The spacing, or 0 for none.
        a_spacing_mm: Length = "0", label "Spacing", range (0.0, 10.0);
    }
}

params! {
    /// The spacing again, for another satin method.
    pub struct SatinAgainParams for &[StitchType::SStitch];

    "Spacing" {
        /// The spacing.
        a_spacing_mm: Length = "0.4", label "Spacing", range (0.01, 10.0);
    }
}

/// The three declarations, one for each family.
const FAMILIES: &[&ParamGroup] = &[&SatinSpacingParams::GROUP, &StrokeSpacingParams::GROUP, &FillSpacingParams::GROUP];

#[test]
fn req_prm_005_a_key_is_declared_once_for_each_family() {
    assert_eq!(audit(FAMILIES), Vec::<String>::new());
    let twice = [FAMILIES, &[&SatinAgainParams::GROUP]].concat();
    assert_eq!(audit(&twice), ["`a_spacing_mm`: declared twice for satin columns"]);
}

#[test]
fn req_prm_005_each_family_finds_its_own_declaration() {
    let kind = |family| find_for(FAMILIES, "a_spacing_mm", family).map(|spec| spec.kind);
    assert_eq!(kind(Family::Stroke), Some(Kind::LengthList { min: 0.01, max: 10.0 }));
    assert!(matches!(kind(Family::Satin), Some(Kind::Length { min: 0.01, .. })));
    assert_eq!(find_for(FAMILIES, "a_spacing_mm", Family::Fill).map(|spec| spec.default), Some("0"));
    assert_eq!(find_for(&[&SatinSpacingParams::GROUP], "a_spacing_mm", Family::Stroke), None);
    assert_eq!(find_for(FAMILIES, "no_such_key", Family::Satin), None);
}

#[test]
fn req_prm_005_an_element_reads_the_key_as_its_family_declares_it() {
    let mut set = ParamSet::new();
    // Unset: each family's default.
    assert_eq!(SatinSpacingParams::from_set_as(&set, StitchType::SatinColumn).unwrap().params.a_spacing_mm.get(), 0.4);
    assert_eq!(FillSpacingParams::from_set_as(&set, StitchType::MeanderFill).unwrap().params.a_spacing_mm.get(), 0.0);
    // 2 lengths: a list for the stroke, and no length for the satin column.
    set.set("a_spacing_mm", "0.5 0.6");
    let lengths = StrokeSpacingParams::from_set_as(&set, StitchType::ZigzagStitch).unwrap().params.a_spacing_mm;
    assert_eq!(lengths.iter().map(|mm| mm.get()).collect::<Vec<_>>(), [0.5, 0.6]);
    let errors = SatinSpacingParams::from_set_as(&set, StitchType::SatinColumn).unwrap_err();
    assert_eq!(errors.iter().map(|d| d.code).collect::<Vec<_>>(), [Code::ParamInvalid]);
    // One length reads in every family.
    set.set("a_spacing_mm", "0.5");
    assert_eq!(SatinSpacingParams::from_set_as(&set, StitchType::SatinColumn).unwrap().params.a_spacing_mm.get(), 0.5);
    assert_eq!(StrokeSpacingParams::from_set_as(&set, StitchType::ZigzagStitch).unwrap().params.a_spacing_mm.len(), 1);
}
