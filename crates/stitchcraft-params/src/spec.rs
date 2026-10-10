//! What the registry knows about each parameter.
//!
//! A [`ParamSpec`] is plain `const` data, written by the [`params!`](crate::params) macro from a
//! declaration next to the generator that uses it. Everything about a parameter that people or programs
//! read — the docs, the JSON Schema, a plug-in's manifest, a validation message — comes from here.

use crate::stitch_type::StitchType;

/// What kind of value a parameter takes, with its limits.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Kind {
    /// A length in millimetres within `min..=max`. An `optional` length may be left empty; then the
    /// setting comes from elsewhere (a document setting or the machine profile), or there is none. A
    /// value of 0 or less counts as empty, because Ink/Stitch reads it as "not set".
    Length {
        /// Smallest accepted value, in millimetres.
        min: f64,
        /// Largest accepted value, in millimetres.
        max: f64,
        /// Whether the value may be empty.
        optional: bool,
    },
    /// An angle in degrees, normalized to (−180, 180].
    Angle,
    /// A percentage within `min..=max`.
    Percent {
        /// Smallest accepted value.
        min: f64,
        /// Largest accepted value.
        max: f64,
    },
    /// A number without a unit within `min..=max`, whole or not.
    Number {
        /// Smallest accepted value.
        min: f64,
        /// Largest accepted value.
        max: f64,
    },
    /// A whole number within `min..=max`.
    Count {
        /// Smallest accepted value.
        min: u32,
        /// Largest accepted value.
        max: u32,
    },
    /// On or off.
    Toggle,
    /// One of a fixed set of options.
    Choice {
        /// The options, in the order a user interface lists them.
        options: &'static [ChoiceOption],
    },
    /// A random seed: empty to derive it from the element, or any text.
    Seed,
    /// 1 to [`MAX_LIST`] lengths in millimetres, each within `min..=max`.
    LengthList {
        /// Smallest accepted value, in millimetres.
        min: f64,
        /// Largest accepted value, in millimetres.
        max: f64,
    },
    /// A length in millimetres within `min..=max`, or 2 separated by a space: Ink/Stitch's values "for each
    /// side", one for both or the first's then the second's (a satin's rails, or its start and end).
    LengthPair {
        /// Smallest accepted value, in millimetres.
        min: f64,
        /// Largest accepted value, in millimetres.
        max: f64,
    },
    /// A percentage within `min..=max`, or 2 separated by a space, as for a [`Kind::LengthPair`].
    PercentPair {
        /// Smallest accepted value.
        min: f64,
        /// Largest accepted value.
        max: f64,
    },
    /// 1 to [`MAX_LIST`] percentages, each within `min..=max`.
    PercentList {
        /// Smallest accepted value.
        min: f64,
        /// Largest accepted value.
        max: f64,
    },
    /// 1 to [`MAX_LIST`] whole numbers, each within `min..=max`.
    CountList {
        /// Smallest accepted value.
        min: u32,
        /// Largest accepted value.
        max: u32,
    },
    /// Free text of at most `max_bytes` bytes.
    Text {
        /// The longest accepted text, in bytes.
        max_bytes: usize,
    },
}

/// The most values a list parameter takes.
pub const MAX_LIST: usize = 16;

/// The longest text parameter, in bytes.
pub const MAX_TEXT: usize = 4096;

/// One option of a [`Kind::Choice`] parameter.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ChoiceOption {
    /// What a design stores: the Ink/Stitch value when Ink/Stitch has the parameter.
    pub id: &'static str,
    /// What a user interface shows.
    pub label: &'static str,
}

/// When a user interface shows a parameter: only while another parameter has one of the given values. A
/// lock's size, say, matters only for the lock shapes it sizes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Condition {
    /// The other parameter.
    pub key: &'static str,
    /// The values it may have, written as a design stores them.
    pub any_of: &'static [&'static str],
}

/// How settled a parameter is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Stability {
    /// Its meaning and default will not change without a migration.
    Stable,
    /// It may change; designs that use it may stitch differently in a later version.
    Experimental,
    /// Still read, but no longer needed.
    Deprecated {
        /// The parameter to use instead, if any.
        use_instead: Option<&'static str>,
    },
}

/// Where a parameter's name and meaning come from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Origin {
    /// Ink/Stitch has it, with the same name, meaning and default.
    InkStitch,
    /// Ink/Stitch has it under the same name, but StitchCraft's meaning or default differs, as an entry in
    /// the deviations ledger (`conformance/deviations.toml`) says, with the reason.
    InkStitchDeviates {
        /// The ledger entry's id, such as `DEV-LCK-001`.
        deviation: &'static str,
    },
    /// StitchCraft's own.
    StitchCraft,
}

/// Everything the registry knows about one parameter.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ParamSpec {
    /// The key designs store it under: the Ink/Stitch attribute name when Ink/Stitch has it.
    pub key: &'static str,
    /// A short label for user interfaces.
    pub label: &'static str,
    /// The help text, as its doc comment's lines; [`ParamSpec::help`] gives the Markdown.
    pub help: &'static str,
    /// The kind of value, with its limits.
    pub kind: Kind,
    /// The value when a design does not set it, written as a design would store it; empty for an
    /// optional length, a seed or a text that is empty by default.
    pub default: &'static str,
    /// The section of a user interface or a reference page it belongs to.
    pub group: &'static str,
    /// The stitch types it applies to.
    pub applies_to: &'static [StitchType],
    /// When it is shown, if not always.
    pub visible_when: Option<Condition>,
    /// How settled it is.
    pub stability: Stability,
    /// Where its name and meaning come from.
    pub origin: Origin,
}

impl ParamSpec {
    /// The help text as Markdown.
    pub fn help(&self) -> String {
        stitchcraft_core::text::doc_comment(self.help)
    }
}

/// The parameters one `params!` declaration registers.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ParamGroup {
    /// The declaration's struct name, such as `CommonParams`.
    pub name: &'static str,
    /// What the group is for, as its doc comment's lines; [`ParamGroup::help`] gives the Markdown.
    pub help: &'static str,
    /// The stitch types its parameters apply to unless a parameter says otherwise.
    pub applies_to: &'static [StitchType],
    /// Its parameters, in declaration order.
    pub specs: &'static [ParamSpec],
}

impl ParamGroup {
    /// What the group is for, as Markdown.
    pub fn help(&self) -> String {
        stitchcraft_core::text::doc_comment(self.help)
    }
}

impl Kind {
    /// The unit values are in, for docs and user interfaces: `mm`, `%`, `°`, or `""`.
    pub const fn unit(self) -> &'static str {
        match self {
            Kind::Length { .. } | Kind::LengthList { .. } | Kind::LengthPair { .. } => "mm",
            Kind::Percent { .. } | Kind::PercentPair { .. } | Kind::PercentList { .. } => "%",
            Kind::Angle => "°",
            _ => "",
        }
    }

    /// What the parameter accepts, completing "it must be …": for messages and the reference pages.
    pub fn describe(self) -> String {
        match self {
            Kind::Length { min, max, optional } => {
                let empty = if optional { ", or empty (0 or less counts as empty)" } else { "" };
                format!("a length from {min} to {max} mm{empty}")
            }
            Kind::Angle => "an angle in degrees".to_string(),
            Kind::Percent { min, max } => format!("a percentage from {min} to {max}"),
            Kind::Number { min, max } => format!("a number from {min} to {max}"),
            Kind::Count { min, max } => format!("a whole number from {min} to {max}"),
            Kind::Toggle => "true or false".to_string(),
            Kind::Choice { options } => {
                let ids: Vec<&str> = options.iter().map(|o| o.id).collect();
                format!("one of {}", ids.join(", "))
            }
            Kind::Seed => "a number or any text, or empty to derive it from the element".to_string(),
            Kind::LengthList { min, max } => format!("1 to {MAX_LIST} lengths from {min} to {max} mm, separated by spaces"),
            Kind::LengthPair { min, max } => format!("a length from {min} to {max} mm, or 2 separated by a space"),
            Kind::PercentPair { min, max } => format!("a percentage from {min} to {max}, or 2 separated by a space"),
            Kind::PercentList { min, max } => format!("1 to {MAX_LIST} percentages from {min} to {max}, separated by spaces"),
            Kind::CountList { min, max } => format!("1 to {MAX_LIST} whole numbers from {min} to {max}, separated by spaces"),
            Kind::Text { max_bytes } => format!("text of at most {max_bytes} bytes"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn units_name_what_values_are_in() {
        let kinds = [
            Kind::Length { min: 0.0, max: 1.0, optional: false },
            Kind::LengthList { min: 0.0, max: 1.0 },
            Kind::LengthPair { min: 0.0, max: 1.0 },
            Kind::Percent { min: 0.0, max: 1.0 },
            Kind::PercentPair { min: 0.0, max: 1.0 },
            Kind::PercentList { min: 0.0, max: 1.0 },
            Kind::Angle,
            Kind::Count { min: 0, max: 1 },
            Kind::Toggle,
        ];
        assert_eq!(kinds.map(Kind::unit), ["mm", "mm", "mm", "%", "%", "%", "°", "", ""]);
    }
}
