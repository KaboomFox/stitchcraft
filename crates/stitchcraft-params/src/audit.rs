//! What REQ-PRM-001 asks of every registered parameter, as a check the registry's own test runs.
//!
//! A declaration that compiles can still be incomplete or contradictory: an empty range, a default
//! outside it, an option listed twice, a visibility condition naming a parameter that does not exist, or
//! one an element could not read it by: of another declaration, shown only sometimes itself, or not read
//! by every stitch type the condition's own parameter applies to. A declaration that applies to no stitch
//! type declares the settings of automatic digitizing ([`AUTOMATIC`](crate::AUTOMATIC)): each of its
//! parameters applies to none either, is StitchCraft's own, since Ink/Stitch digitizes nothing
//! automatically, and has its key to itself, since no stitch type reads it.
//! The generated reference pages, the JSON Schema and every host's settings panel trust that none of
//! that happens, so [`audit()`] lists each such problem and the engine's registry test
//! (`req_prm_001_every_registered_parameter_is_complete`) requires the list to be empty.

use std::collections::{BTreeMap, BTreeSet};

use crate::set::declarations;
use crate::spec::{Kind, Origin, ParamGroup, ParamSpec, Stability, listed};
use crate::stitch_type::Family;

/// Every problem in `registry`, one line each, naming the group or the parameter.
pub fn audit(registry: &[&ParamGroup]) -> Vec<String> {
    let mut problems = Vec::new();
    // The families each key is declared for so far. A key has at most one declaration for each family.
    let mut keys: BTreeMap<&str, BTreeSet<Family>> = BTreeMap::new();
    for group in registry {
        let mut group_problem = |ok: bool, problem: &str| {
            if !ok {
                problems.push(format!("{}: {problem}", group.name));
            }
        };
        group_problem(group.name.len() > "Params".len() && group.name.ends_with("Params"), "the struct name must end in `Params`");
        group_problem(!group.help().is_empty(), "no help text (the struct's doc comment)");
        group_problem(!group.specs.is_empty(), "no parameters");
        let labels: BTreeSet<&str> = group.specs.iter().map(|spec| spec.label).collect();
        group_problem(labels.len() == group.specs.len(), "two parameters share a label");
        for spec in group.specs {
            let families: BTreeSet<Family> = spec.applies_to.iter().map(|t| t.family()).collect();
            let declared = keys.entry(spec.key).or_default();
            let twice: Vec<&str> = families.intersection(declared).map(|family| family.plural()).collect();
            if !twice.is_empty() {
                problems.push(format!("`{}`: declared twice for {}", spec.key, listed(&twice, "and")));
            }
            declared.extend(families);
            problems.extend(check(spec, group, registry).into_iter().map(|problem| format!("`{}`: {problem}", spec.key)));
        }
    }
    problems
}

/// What is wrong with one parameter of `group`.
fn check(spec: &ParamSpec, group: &ParamGroup, registry: &[&ParamGroup]) -> Vec<&'static str> {
    let mut problems = Vec::new();
    let mut require = |ok: bool, problem: &'static str| {
        if !ok {
            problems.push(problem);
        }
    };
    require(!spec.key.is_empty() && spec.key.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_'), "the key is not snake_case");
    require(!spec.label.trim().is_empty(), "no label");
    require(!spec.help().is_empty(), "no help text (the field's doc comment)");
    require(!spec.group.trim().is_empty(), "no group");
    if group.automatic() {
        require(spec.applies_to.is_empty(), "a setting of automatic digitizing that applies to a stitch type");
        require(spec.origin == Origin::StitchCraft, "a setting of automatic digitizing that isn't StitchCraft's own");
        require(declarations(registry, spec.key).count() == 1, "a setting of automatic digitizing whose key is declared again");
    } else {
        require(!spec.applies_to.is_empty(), "applies to no stitch type");
    }
    require(range_is_usable(spec.kind), "its range is empty or not finite");
    if let Kind::Choice { options } = spec.kind {
        let ids: BTreeSet<&str> = options.iter().map(|o| o.id).collect();
        require(!options.is_empty() && ids.len() == options.len(), "its options are missing or repeated");
        require(options.iter().all(|o| !o.id.trim().is_empty() && !o.label.trim().is_empty()), "an option has no id or no label");
    }
    require(matches!(spec.read(spec.default), Ok((_, None))), "its default is not a value it accepts without a warning");
    let families: BTreeSet<Family> = spec.family_defaults.iter().map(|&(family, _)| family).collect();
    require(families.len() == spec.family_defaults.len(), "a family's default is given twice");
    require(
        families.iter().all(|family| spec.applies_to.iter().any(|t| t.family() == *family)),
        "it has a default for a family it does not apply to",
    );
    require(
        spec.family_defaults.iter().all(|&(family, value)| value != spec.default && matches!(spec.in_family(family).read(value), Ok((_, None)))),
        "a family's default is the default, or not a value the family accepts without a warning",
    );
    if let Some(condition) = spec.visible_when {
        // Shown for some value, and only for values the other parameter takes as they are.
        // Only this declaration is searched. The next check names a parameter of another declaration.
        let possible = !condition.any_of.is_empty()
            && group
                .specs
                .iter()
                .find(|other| other.key == condition.key)
                .is_none_or(|other| other.key != spec.key && condition.any_of.iter().all(|value| matches!(other.read(value), Ok((_, None)))));
        require(possible, "it is shown only when another parameter has a value that parameter cannot have");
        // An element reads it by the other parameter's value as it reads that parameter: from the same
        // declaration, always, for every stitch type this one applies to (`ParamSpec::read_by`).
        let readable = group
            .specs
            .iter()
            .any(|other| other.key == condition.key && other.visible_when.is_none() && spec.applies_to.iter().all(|t| other.applies_to.contains(t)));
        require(
            readable,
            "it is shown by a parameter its declaration does not have, one shown only sometimes, or one that applies to fewer stitch types",
        );
    }
    if let Stability::Deprecated { use_instead: Some(other) } = spec.stability {
        // Each element that reads it has the other to read instead.
        let replaced = spec.applies_to.iter().all(|t| declarations(registry, other).any(|them| them.applies_to.contains(t)));
        require(other != spec.key && replaced, "it is deprecated in favour of itself or of a parameter not declared for each of its stitch types");
    }
    problems
}

/// Whether a kind's limits leave values to choose from.
fn range_is_usable(kind: Kind) -> bool {
    match kind {
        Kind::Length { min, max, .. }
        | Kind::BoundedAngle { min, max }
        | Kind::Percent { min, max }
        | Kind::LengthList { min, max }
        | Kind::LengthPair { min, max, .. }
        | Kind::PercentPair { min, max, .. }
        | Kind::PercentList { min, max }
        | Kind::Number { min, max }
        | Kind::Area { min, max } => min.is_finite() && max.is_finite() && min < max,
        Kind::Count { min, max } | Kind::CountList { min, max } => min < max,
        Kind::Text { max_bytes } => max_bytes > 0,
        Kind::Angle | Kind::AngleList { .. } | Kind::Toggle | Kind::Choice { .. } | Kind::Seed => true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::spec::{AUTOMATIC, ChoiceOption, Condition, Empty};
    use crate::stitch_type::StitchType;

    const GOOD: ParamSpec = ParamSpec {
        key: "row_spacing_mm",
        label: "Row spacing",
        help: " Distance between rows.\n",
        kind: Kind::Length { min: 0.1, max: 10.0, empty: Empty::Never },
        default: "0.25",
        family_defaults: &[],
        group: "Fill",
        applies_to: StitchType::ALL,
        visible_when: None,
        stability: Stability::Stable,
        origin: Origin::InkStitch,
    };

    fn group(specs: &'static [ParamSpec]) -> ParamGroup {
        ParamGroup { name: "TestParams", help: " A test.\n", applies_to: StitchType::ALL, specs }
    }

    #[test]
    fn a_complete_registry_has_no_problems() {
        const SPECS: &[ParamSpec] = &[
            GOOD,
            ParamSpec { key: "rows", label: "Rows", kind: Kind::Count { min: 1, max: 20 }, default: "4", ..GOOD },
            ParamSpec {
                key: "method",
                label: "Method",
                kind: Kind::Choice { options: &[ChoiceOption { id: "a", label: "A" }, ChoiceOption { id: "b", label: "B" }] },
                default: "a",
                ..GOOD
            },
            ParamSpec {
                key: "note",
                label: "Note",
                kind: Kind::Text { max_bytes: 10 },
                default: "",
                visible_when: Some(Condition { key: "method", any_of: &["a", "b"] }),
                ..GOOD
            },
            ParamSpec {
                key: "old_spacing_mm",
                label: "Old spacing",
                stability: Stability::Deprecated { use_instead: Some("row_spacing_mm") },
                ..GOOD
            },
        ];
        assert_eq!(audit(&[&group(SPECS)]), Vec::<String>::new());
    }

    #[test]
    fn every_incomplete_declaration_is_named() {
        const BROKEN: &[ParamSpec] = &[
            ParamSpec { key: "Row Spacing", label: " ", help: "", group: "", applies_to: &[], ..GOOD },
            ParamSpec { key: "a", label: "A", kind: Kind::Count { min: 3, max: 3 }, default: "3", ..GOOD },
            ParamSpec { key: "b", label: "B", kind: Kind::Length { min: 0.0, max: f64::INFINITY, empty: Empty::Never }, ..GOOD },
            ParamSpec {
                key: "c",
                label: "C",
                kind: Kind::Choice { options: &[ChoiceOption { id: "x", label: "X" }, ChoiceOption { id: "x", label: "" }] },
                default: "y",
                ..GOOD
            },
            ParamSpec { key: "d", label: "D", visible_when: Some(Condition { key: "c", any_of: &["x", "z"] }), ..GOOD },
            ParamSpec { key: "e", label: "E", visible_when: Some(Condition { key: "e", any_of: &["0.25"] }), ..GOOD },
            ParamSpec { key: "f", label: "F", stability: Stability::Deprecated { use_instead: Some("gone") }, ..GOOD },
            ParamSpec { key: "g", label: "G", kind: Kind::Length { min: 1.0, max: 1.0, empty: Empty::Never }, default: "1", ..GOOD },
            ParamSpec { key: "h", label: "H", kind: Kind::Text { max_bytes: 0 }, default: "", ..GOOD },
            ParamSpec { key: "i", label: "I", stability: Stability::Deprecated { use_instead: Some("i") }, ..GOOD },
            ParamSpec { key: "a", label: "A2", ..GOOD },
            ParamSpec { key: "j", label: "J", visible_when: Some(Condition { key: "c", any_of: &[] }), ..GOOD },
            ParamSpec { key: "k", label: "K", family_defaults: &[(Family::Fill, "1"), (Family::Fill, "2")], ..GOOD },
            ParamSpec { key: "l", label: "L", applies_to: &[StitchType::SatinColumn], family_defaults: &[(Family::Fill, "1")], ..GOOD },
            ParamSpec { key: "m", label: "M", family_defaults: &[(Family::Fill, "0.25")], ..GOOD },
            ParamSpec { key: "n", label: "N", family_defaults: &[(Family::Satin, "99")], ..GOOD },
            // A family with a default of its own needs a value: empty is not one there.
            ParamSpec {
                key: "o",
                label: "O",
                kind: Kind::Length { min: 0.1, max: 10.0, empty: Empty::AtMostZero },
                family_defaults: &[(Family::Fill, "")],
                ..GOOD
            },
            // Shown by a parameter shown only sometimes, by one of another declaration, by one that applies
            // to fewer stitch types, and by one that no declaration has.
            ParamSpec { key: "p", label: "P", visible_when: Some(Condition { key: "d", any_of: &["0.25"] }), ..GOOD },
            ParamSpec { key: "q", label: "Q", visible_when: Some(Condition { key: "elsewhere", any_of: &["0.25"] }), ..GOOD },
            ParamSpec { key: "r", label: "R", applies_to: &[StitchType::SatinColumn], ..GOOD },
            ParamSpec { key: "s", label: "S", visible_when: Some(Condition { key: "r", any_of: &["0.25"] }), ..GOOD },
            ParamSpec { key: "t", label: "T", visible_when: Some(Condition { key: "gone", any_of: &["0.25"] }), ..GOOD },
            // Deprecated in favour of a parameter that one of its stitch types lacks, and of one that another
            // family has alone.
            ParamSpec {
                key: "u",
                label: "U",
                applies_to: &[StitchType::RunningStitch, StitchType::ManualStitch],
                stability: Stability::Deprecated { use_instead: Some("v") },
                ..GOOD
            },
            ParamSpec { key: "v", label: "V", applies_to: &[StitchType::RunningStitch], ..GOOD },
            ParamSpec { key: "w", label: "W", stability: Stability::Deprecated { use_instead: Some("r") }, ..GOOD },
        ];
        const ELSEWHERE: &[ParamSpec] = &[ParamSpec { key: "elsewhere", label: "Elsewhere", ..GOOD }];
        let problems = audit(&[&group(BROKEN), &ParamGroup { name: "OtherParams", ..group(ELSEWHERE) }]);
        let unreadable =
            "it is shown by a parameter its declaration does not have, one shown only sometimes, or one that applies to fewer stitch types";
        let unreplaced = "it is deprecated in favour of itself or of a parameter not declared for each of its stitch types";
        assert_eq!(
            problems,
            [
                "`Row Spacing`: the key is not snake_case",
                "`Row Spacing`: no label",
                "`Row Spacing`: no help text (the field's doc comment)",
                "`Row Spacing`: no group",
                "`Row Spacing`: applies to no stitch type",
                "`a`: its range is empty or not finite",
                "`b`: its range is empty or not finite",
                "`c`: its options are missing or repeated",
                "`c`: an option has no id or no label",
                "`c`: its default is not a value it accepts without a warning",
                "`d`: it is shown only when another parameter has a value that parameter cannot have",
                "`e`: it is shown only when another parameter has a value that parameter cannot have",
                &format!("`e`: {unreadable}"),
                &format!("`f`: {unreplaced}"),
                "`g`: its range is empty or not finite",
                "`h`: its range is empty or not finite",
                &format!("`i`: {unreplaced}"),
                "`a`: declared twice for strokes, satin columns and fills",
                "`j`: it is shown only when another parameter has a value that parameter cannot have",
                "`k`: a family's default is given twice",
                "`l`: it has a default for a family it does not apply to",
                "`m`: a family's default is the default, or not a value the family accepts without a warning",
                "`n`: a family's default is the default, or not a value the family accepts without a warning",
                "`o`: a family's default is the default, or not a value the family accepts without a warning",
                &format!("`p`: {unreadable}"),
                &format!("`q`: {unreadable}"),
                &format!("`s`: {unreadable}"),
                &format!("`t`: {unreadable}"),
                &format!("`u`: {unreplaced}"),
                &format!("`w`: {unreplaced}"),
            ]
        );
    }

    #[test]
    fn req_prm_005_a_condition_reads_its_own_declaration_of_the_key() {
        // One length for satin columns, declared first, and a list for zigzag strokes, whose note is shown
        // for 2 lengths: one length cannot be 2.
        const SATIN: &[ParamSpec] = &[ParamSpec { key: "spacing_mm", label: "Spacing", applies_to: &[StitchType::SatinColumn], ..GOOD }];
        const STROKE: &[ParamSpec] = &[
            ParamSpec {
                key: "spacing_mm",
                label: "Spacing",
                kind: Kind::LengthList { min: 0.1, max: 10.0 },
                applies_to: &[StitchType::ZigzagStitch],
                ..GOOD
            },
            ParamSpec {
                key: "note",
                label: "Note",
                kind: Kind::Text { max_bytes: 10 },
                default: "",
                applies_to: &[StitchType::ZigzagStitch],
                visible_when: Some(Condition { key: "spacing_mm", any_of: &["0.5 0.6"] }),
                ..GOOD
            },
        ];
        let satin = ParamGroup { name: "SatinParams", applies_to: SATIN[0].applies_to, ..group(SATIN) };
        let stroke = ParamGroup { name: "StrokeParams", applies_to: STROKE[0].applies_to, ..group(STROKE) };
        assert_eq!(audit(&[&satin, &stroke]), Vec::<String>::new());
    }

    #[test]
    fn groups_need_a_name_help_parameters_and_distinct_labels() {
        const TWINS: &[ParamSpec] = &[GOOD, ParamSpec { key: "other_mm", ..GOOD }];
        let nameless = ParamGroup { name: "Params", help: "", applies_to: StitchType::ALL, specs: &[] };
        assert_eq!(
            audit(&[&nameless, &group(TWINS)]),
            [
                "Params: the struct name must end in `Params`",
                "Params: no help text (the struct's doc comment)",
                "Params: no parameters",
                "TestParams: two parameters share a label",
            ]
        );
    }

    #[test]
    fn a_setting_of_automatic_digitizing_is_stitchcraft_s_own_applies_to_no_stitch_type_and_has_its_key_to_itself() {
        const FILL: &[ParamSpec] = &[GOOD];
        const OWN: ParamSpec = ParamSpec { key: "auto_overlap_mm", label: "Overlap", applies_to: AUTOMATIC, origin: Origin::StitchCraft, ..GOOD };
        const AUTO: &[ParamSpec] = &[
            OWN,
            ParamSpec { key: "inkstitch", label: "Ink/Stitch's", origin: Origin::InkStitch, ..OWN },
            ParamSpec { key: "satin", label: "A satin column's", applies_to: &[StitchType::SatinColumn], ..OWN },
            ParamSpec { key: "row_spacing_mm", label: "Row spacing", ..OWN },
        ];
        let automatic = ParamGroup { name: "AutoParams", applies_to: AUTOMATIC, ..group(AUTO) };
        assert_eq!(audit(&[&group(FILL), &ParamGroup { specs: &AUTO[..1], ..automatic }]), Vec::<String>::new());
        assert_eq!(
            audit(&[&group(FILL), &automatic]),
            [
                "`inkstitch`: a setting of automatic digitizing that isn't StitchCraft's own",
                "`satin`: a setting of automatic digitizing that applies to a stitch type",
                "`row_spacing_mm`: a setting of automatic digitizing whose key is declared again",
            ]
        );
    }
}
