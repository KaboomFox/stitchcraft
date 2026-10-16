//! A design element's parameters, and reading them into typed views.
//!
//! A [`ParamSet`] holds parameters exactly as the design stores them: keys and unparsed text. Hosts fill
//! it from SVG attributes, the command line or VectorCraft effect records, and write it back unchanged,
//! so parameters StitchCraft does not know, or does not use for this element, survive a round trip
//! (REQ-PRM-003). Generators never read it directly: they get a typed view (`CommonParams`, …) from
//! `from_set` or `from_set_as`, which the [`params!`](crate::params) macro writes and which read every
//! parameter with [`read_param`].
//!
//! Empty or blank text is not set, whatever the kind, and the parameter takes its default: Ink/Stitch
//! strips every value and reads an empty one so, and its settings window stores a field the user clears
//! as empty text. An element reads only the parameters its stitch type uses (REQ-PRM-004): one that does
//! not apply to it, or that is shown only while another parameter has other values, takes its default,
//! and its text is kept unread, so that nothing is said of it. Ink/Stitch reads a setting only where its
//! stitch type uses it, and a path keeps a satin's settings when its stroke goes back to running stitch.

use std::collections::BTreeMap;

use stitchcraft_core::{Code, Diagnostic};

use crate::kinds::ParamKind;
use crate::spec::{ParamGroup, ParamSpec};
use crate::stitch_type::StitchType;

/// One element's parameters as its design stores them: keys and their text.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ParamSet {
    values: BTreeMap<String, String>,
}

impl ParamSet {
    /// No parameters: every one takes its default.
    pub fn new() -> Self {
        ParamSet::default()
    }

    /// Sets `key` to `text`; returns the text it had.
    pub fn set(&mut self, key: impl Into<String>, text: impl Into<String>) -> Option<String> {
        self.values.insert(key.into(), text.into())
    }

    /// The text of `key`, if the design sets it.
    pub fn get(&self, key: &str) -> Option<&str> {
        self.values.get(key).map(String::as_str)
    }

    /// Every key and its text, ordered by key.
    pub fn iter(&self) -> impl Iterator<Item = (&str, &str)> {
        self.values.iter().map(|(k, v)| (k.as_str(), v.as_str()))
    }
}

impl<K: Into<String>, V: Into<String>> FromIterator<(K, V)> for ParamSet {
    fn from_iter<I: IntoIterator<Item = (K, V)>>(items: I) -> Self {
        ParamSet { values: items.into_iter().map(|(k, v)| (k.into(), v.into())).collect() }
    }
}

/// A typed view of an element's parameters, with the warnings reading them produced (values clamped
/// into range, `SC-W0102`).
#[derive(Clone, Debug, PartialEq)]
pub struct Validated<T> {
    /// The parameters.
    pub params: T,
    /// What was adjusted, for the host to show.
    pub warnings: Vec<Diagnostic>,
}

/// Reads parameter `spec` from `set` as kind `K`: its text, or its default when the design does not set
/// it or sets it to empty or blank text. With a stitch type, `as_type`, as an element sewn so reads it: with
/// its family's default ([`ParamSpec::in_family`]), and only when it reads the parameter at all
/// ([`ParamSpec::read_by`], `specs` being the declaration's); otherwise every parameter is read, with the
/// plain defaults. Problems go to `problems`; `None` means the value cannot be used (`SC-E0101`, or
/// `SC-E0009` for a registry bug such as a default that does not parse).
pub fn read_param<K: ParamKind>(
    set: &ParamSet,
    spec: Option<&ParamSpec>,
    as_type: Option<StitchType>,
    specs: &[ParamSpec],
    problems: &mut Vec<Diagnostic>,
) -> Option<K::Value> {
    let Some(declared) = spec else {
        problems.push(Diagnostic::new(Code::InternalCheckFailed, "A parameter group has fewer specs than fields."));
        return None;
    };
    let spec = as_type.map_or(*declared, |stitch_type| declared.in_family(stitch_type.family()));
    let read = as_type.is_none_or(|stitch_type| declared.read_by(stitch_type, set, specs));
    let (raw, from_design) = match set.get(spec.key) {
        Some(raw) if read && !raw.trim().is_empty() => (raw, true),
        _ => (spec.default, false),
    };
    match spec.read(raw) {
        Ok((value, warning)) if from_design || warning.is_none() => {
            problems.extend(warning);
            let typed = K::take(value);
            if typed.is_none() {
                problems.push(Diagnostic::new(Code::InternalCheckFailed, format!("`{}` is declared with the wrong kind.", spec.key)));
            }
            typed
        }
        Err(problem) if from_design => {
            problems.push(problem);
            None
        }
        _ => {
            problems.push(Diagnostic::new(Code::InternalCheckFailed, format!("The default of `{}` is not a valid value.", spec.key)));
            None
        }
    }
}

/// `SC-W0105` for every key in `set` that no group of `registry` declares: kept, but ignored.
pub fn unknown_keys(set: &ParamSet, registry: &[&ParamGroup]) -> Vec<Diagnostic> {
    set.iter()
        .filter(|(key, _)| find(registry, key).is_none())
        .map(|(key, _)| {
            Diagnostic::new(Code::ParamUnknown, format!("`{key}` is not a StitchCraft parameter; it was kept but does not change the stitches."))
        })
        .collect()
}

/// The spec of `key` in `registry`.
pub fn find(registry: &[&ParamGroup], key: &str) -> Option<&'static ParamSpec> {
    registry.iter().flat_map(|group| group.specs.iter()).find(|spec| spec.key == key)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kinds::{Length, Toggle};
    use crate::spec::{Empty, Kind, Origin, Stability};
    use crate::stitch_type::Family;

    const fn spec(default: &'static str) -> ParamSpec {
        ParamSpec {
            key: "row_spacing_mm",
            label: "Row spacing",
            help: " Distance between rows.\n",
            kind: Kind::Length { min: 0.1, max: 10.0, empty: Empty::Never },
            default,
            family_defaults: &[(Family::Fill, "4")],
            group: "Fill",
            applies_to: StitchType::ALL,
            visible_when: None,
            stability: Stability::Stable,
            origin: Origin::InkStitch,
        }
    }

    fn codes(problems: &[Diagnostic]) -> Vec<Code> {
        problems.iter().map(|d| d.code).collect()
    }

    #[test]
    fn diag_sc_e0009_a_default_that_needs_clamping_or_does_not_parse_is_a_registry_bug() {
        for default in ["99", "wide"] {
            let mut problems = Vec::new();
            assert_eq!(read_param::<Length>(&ParamSet::new(), Some(&spec(default)), None, &[], &mut problems), None);
            let shown: Vec<String> = problems.iter().map(ToString::to_string).collect();
            assert_eq!(shown, ["error SC-E0009: The default of `row_spacing_mm` is not a valid value."], "{default}");
        }
    }

    #[test]
    fn the_same_values_from_a_design_are_the_designs_problem() {
        let mut problems = Vec::new();
        let design: ParamSet = [("row_spacing_mm", "99")].into_iter().collect();
        assert_eq!(read_param::<Length>(&design, Some(&spec("0.25")), None, &[], &mut problems).map(|mm| mm.get()), Some(10.0));
        assert_eq!(codes(&problems), [Code::ParamClamped]);
        let mut problems = Vec::new();
        let design: ParamSet = [("row_spacing_mm", "wide")].into_iter().collect();
        assert_eq!(read_param::<Length>(&design, Some(&spec("0.25")), None, &[], &mut problems), None);
        assert_eq!(codes(&problems), [Code::ParamInvalid]);
    }

    #[test]
    fn a_family_with_its_own_default_reads_it_and_the_others_read_the_default() {
        let read = |as_type: Option<StitchType>, set: &ParamSet| {
            read_param::<Length>(set, Some(&spec("0.25")), as_type, &[], &mut Vec::new()).map(|mm| mm.get())
        };
        assert_eq!(read(Some(StitchType::TatamiFill), &ParamSet::new()), Some(4.0));
        assert_eq!([read(Some(StitchType::SatinColumn), &ParamSet::new()), read(None, &ParamSet::new())], [Some(0.25); 2]);
        // A value the design sets is the value, whatever the family, and blank text is none.
        let design: ParamSet = [("row_spacing_mm", "2")].into_iter().collect();
        assert_eq!(read(Some(StitchType::TatamiFill), &design), Some(2.0));
        let blank: ParamSet = [("row_spacing_mm", " ")].into_iter().collect();
        assert_eq!([read(Some(StitchType::TatamiFill), &blank), read(None, &blank)], [Some(4.0), Some(0.25)]);
    }

    #[test]
    fn a_stitch_type_the_parameter_does_not_apply_to_reads_the_default() {
        let fills = ParamSpec { applies_to: &[StitchType::TatamiFill], ..spec("0.25") };
        let design: ParamSet = [("row_spacing_mm", "wide")].into_iter().collect();
        let mut problems = Vec::new();
        let read = read_param::<Length>(&design, Some(&fills), Some(StitchType::RunningStitch), &[], &mut problems);
        assert_eq!((read.map(|mm| mm.get()), codes(&problems)), (Some(0.25), Vec::new()));
        assert_eq!(read_param::<Length>(&design, Some(&fills), Some(StitchType::TatamiFill), &[], &mut problems), None);
        assert_eq!(codes(&problems), [Code::ParamInvalid]);
    }

    #[test]
    fn a_missing_spec_or_a_kind_mismatch_is_a_registry_bug() {
        let mut problems = Vec::new();
        assert_eq!(read_param::<Length>(&ParamSet::new(), None, None, &[], &mut problems), None);
        assert_eq!(read_param::<Toggle>(&ParamSet::new(), Some(&spec("0.25")), None, &[], &mut problems), None);
        assert_eq!(codes(&problems), [Code::InternalCheckFailed, Code::InternalCheckFailed]);
    }
}
