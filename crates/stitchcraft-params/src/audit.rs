//! What REQ-PRM-001 asks of every registered parameter, as a check the registry's own test runs.
//!
//! A declaration that compiles can still be incomplete or contradictory: an empty range, a default
//! outside it, an option listed twice, a visibility condition naming a parameter that does not exist.
//! The generated reference pages, the JSON Schema and every host's settings panel trust that none of
//! that happens, so [`audit()`] lists each such problem and the engine's registry test
//! (`req_prm_001_every_registered_parameter_is_complete`) requires the list to be empty.

use std::collections::BTreeSet;

use crate::set::find;
use crate::spec::{Kind, ParamGroup, ParamSpec, Stability};
use crate::stitch_type::Family;

/// Every problem in `registry`, one line each, naming the group or the parameter.
pub fn audit(registry: &[&ParamGroup]) -> Vec<String> {
    let mut problems = Vec::new();
    let mut keys = BTreeSet::new();
    for group in registry {
        let mut group_problem = |ok: bool, problem: &str| {
            if !ok {
                problems.push(format!("{}: {problem}", group.name));
            }
        };
        group_problem(group.name.len() > "Params".len() && group.name.ends_with("Params"), "the struct name must end in `Params`");
        group_problem(!group.help().is_empty(), "no help text (the struct's doc comment)");
        group_problem(!group.specs.is_empty(), "no parameters");
        group_problem(!group.applies_to.is_empty(), "applies to no stitch type");
        let labels: BTreeSet<&str> = group.specs.iter().map(|spec| spec.label).collect();
        group_problem(labels.len() == group.specs.len(), "two parameters share a label");
        for spec in group.specs {
            if !keys.insert(spec.key) {
                problems.push(format!("`{}`: declared twice", spec.key));
            }
            problems.extend(check(spec, registry).into_iter().map(|problem| format!("`{}`: {problem}", spec.key)));
        }
    }
    problems
}

/// What is wrong with one parameter.
fn check(spec: &ParamSpec, registry: &[&ParamGroup]) -> Vec<&'static str> {
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
    require(!spec.applies_to.is_empty(), "applies to no stitch type");
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
        spec.family_defaults.iter().all(|&(_, value)| value != spec.default && matches!(spec.read(value), Ok((_, None)))),
        "a family's default is the default, or not a value it accepts without a warning",
    );
    if let Some(condition) = spec.visible_when {
        // Shown for some value, and only for values the other parameter takes as they are.
        let possible = !condition.any_of.is_empty()
            && find(registry, condition.key)
                .is_some_and(|other| other.key != spec.key && condition.any_of.iter().all(|value| matches!(other.read(value), Ok((_, None)))));
        require(possible, "it is shown only when another parameter has a value that parameter cannot have");
    }
    if let Stability::Deprecated { use_instead: Some(other) } = spec.stability {
        require(other != spec.key && find(registry, other).is_some(), "it is deprecated in favour of itself or of a parameter that does not exist");
    }
    problems
}

/// Whether a kind's limits leave values to choose from.
fn range_is_usable(kind: Kind) -> bool {
    match kind {
        Kind::Length { min, max, .. }
        | Kind::Percent { min, max }
        | Kind::LengthList { min, max }
        | Kind::LengthPair { min, max, .. }
        | Kind::PercentPair { min, max, .. }
        | Kind::PercentList { min, max }
        | Kind::Number { min, max } => min.is_finite() && max.is_finite() && min < max,
        Kind::Count { min, max } | Kind::CountList { min, max } => min < max,
        Kind::Text { max_bytes } => max_bytes > 0,
        Kind::Angle | Kind::Toggle | Kind::Choice { .. } | Kind::Seed => true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::spec::{ChoiceOption, Condition, Origin};
    use crate::stitch_type::StitchType;

    const GOOD: ParamSpec = ParamSpec {
        key: "row_spacing_mm",
        label: "Row spacing",
        help: " Distance between rows.\n",
        kind: Kind::Length { min: 0.1, max: 10.0, optional: false },
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
            ParamSpec { key: "b", label: "B", kind: Kind::Length { min: 0.0, max: f64::INFINITY, optional: false }, ..GOOD },
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
            ParamSpec { key: "g", label: "G", kind: Kind::Length { min: 1.0, max: 1.0, optional: false }, default: "1", ..GOOD },
            ParamSpec { key: "h", label: "H", kind: Kind::Text { max_bytes: 0 }, default: "", ..GOOD },
            ParamSpec { key: "i", label: "I", stability: Stability::Deprecated { use_instead: Some("i") }, ..GOOD },
            ParamSpec { key: "a", label: "A2", ..GOOD },
            ParamSpec { key: "j", label: "J", visible_when: Some(Condition { key: "c", any_of: &[] }), ..GOOD },
            ParamSpec { key: "k", label: "K", family_defaults: &[(Family::Fill, "1"), (Family::Fill, "2")], ..GOOD },
            ParamSpec { key: "l", label: "L", applies_to: &[StitchType::SatinColumn], family_defaults: &[(Family::Fill, "1")], ..GOOD },
            ParamSpec { key: "m", label: "M", family_defaults: &[(Family::Fill, "0.25")], ..GOOD },
            ParamSpec { key: "n", label: "N", family_defaults: &[(Family::Satin, "99")], ..GOOD },
        ];
        let problems = audit(&[&group(BROKEN)]);
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
                "`f`: it is deprecated in favour of itself or of a parameter that does not exist",
                "`g`: its range is empty or not finite",
                "`h`: its range is empty or not finite",
                "`i`: it is deprecated in favour of itself or of a parameter that does not exist",
                "`a`: declared twice",
                "`j`: it is shown only when another parameter has a value that parameter cannot have",
                "`k`: a family's default is given twice",
                "`l`: it has a default for a family it does not apply to",
                "`m`: a family's default is the default, or not a value it accepts without a warning",
                "`n`: a family's default is the default, or not a value it accepts without a warning",
            ]
        );
    }

    #[test]
    fn groups_need_a_name_help_parameters_and_distinct_labels() {
        const TWINS: &[ParamSpec] = &[GOOD, ParamSpec { key: "other_mm", ..GOOD }];
        let nameless = ParamGroup { name: "Params", help: "", applies_to: &[], specs: &[] };
        assert_eq!(
            audit(&[&nameless, &group(TWINS)]),
            [
                "Params: the struct name must end in `Params`",
                "Params: no help text (the struct's doc comment)",
                "Params: no parameters",
                "Params: applies to no stitch type",
                "TestParams: two parameters share a label",
            ]
        );
    }
}
