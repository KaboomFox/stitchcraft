//! A design element's parameters, and reading them into typed views.
//!
//! A [`ParamSet`] holds parameters exactly as the design stores them: keys and unparsed text. Hosts fill
//! it from SVG attributes, the command line or VectorCraft effect records, and write it back unchanged,
//! so parameters StitchCraft does not know, or does not use for this element, survive a round trip
//! (REQ-PRM-003). Generators never read it directly: they get a typed view (`CommonParams`, …) from
//! `from_set`, which the [`params!`](crate::params) macro writes and which reads every parameter with
//! [`read_param`].

use std::collections::BTreeMap;

use stitchcraft_core::{Code, Diagnostic};

use crate::kinds::ParamKind;
use crate::spec::{ParamGroup, ParamSpec};

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
/// it. Problems go to `problems`; `None` means the value cannot be used (`SC-E0101`, or `SC-E0009` for a
/// registry bug such as a default that does not parse).
pub fn read_param<K: ParamKind>(set: &ParamSet, spec: Option<&ParamSpec>, problems: &mut Vec<Diagnostic>) -> Option<K::Value> {
    let Some(spec) = spec else {
        problems.push(Diagnostic::new(Code::InternalCheckFailed, "A parameter group has fewer specs than fields."));
        return None;
    };
    let (raw, from_design) = match set.get(spec.key) {
        Some(raw) => (raw, true),
        None => (spec.default, false),
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
