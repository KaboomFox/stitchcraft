//! The `params!` declaration end to end, with one parameter of every kind: what the registry records,
//! and how designs' values are read (REQ-PRM-002: invalid values are errors that say what is accepted,
//! out-of-range values are clamped with a warning, and nothing falls back to a default in silence).

// Test code may unwrap (clippy.toml allows it inside #[test] functions; these helpers are test code too).
#![allow(clippy::unwrap_used)]

use stitchcraft_core::{Code, Severity};
use stitchcraft_params::{Condition, Kind, Origin, ParamSet, StitchType, find, params, unknown_keys};

params! {
    /// One parameter of every kind.
    pub struct Everything for StitchType::ALL;

    "Numbers" {
        /// A length.
        a_length_mm: Length = "2.5", label "Length", range (0.1, 10.0);

        /// A length that may be empty.
        an_optional_mm: OptionalLength = "", label "Optional length", range (0.0, 5.0);

        /// An angle.
        an_angle: Angle = "0", label "Angle";

        /// A percentage.
        a_percent: Percent = "100", label "Percentage", range (0.0, 200.0);

        /// A whole number.
        a_count: Count = "4", label "Count", range (1, 20);
    }

    "Others" {
        /// On or off.
        a_toggle: Toggle = "false", label "Toggle";

        /// A choice.
        a_choice: Choice = "one", label "Choice", options ["one" => "One", "two" => "Two"];

        /// Detail for the second choice.
        a_detail: Text = "", label "Detail", when a_choice == "two";

        /// A seed.
        a_seed: Seed = "", label "Seed", origin Origin::StitchCraft;

        /// Lengths, for running stitch only.
        some_lengths_mm: LengthList = "2.5", label "Lengths", range (0.3, 12.0), applies &[StitchType::RunningStitch];

        /// Whole numbers.
        some_counts: CountList = "0", label "Counts", range (0, 9);
    }
}

fn set(pairs: &[(&str, &str)]) -> ParamSet {
    pairs.iter().copied().collect()
}

#[test]
fn defaults_apply_when_the_design_says_nothing() {
    let read = Everything::from_set(&ParamSet::new()).unwrap();
    assert!(read.warnings.is_empty());
    let p = read.params;
    assert_eq!((p.a_length_mm.get(), p.an_optional_mm, p.an_angle, p.a_percent, p.a_count), (2.5, None, 0.0, 100.0, 4));
    assert_eq!((p.a_toggle, p.a_choice, p.a_detail.as_str(), p.a_seed), (false, "one", "", None));
    assert_eq!((p.some_lengths_mm.len(), p.some_counts.as_slice()), (1, [0].as_slice()));
}

#[test]
fn req_prm_002_invalid_values_are_errors_that_say_what_is_accepted() {
    let problems = Everything::from_set(&set(&[("a_length_mm", "long"), ("a_choice", "three"), ("a_toggle", "maybe")])).unwrap_err();
    let messages: Vec<String> = problems.iter().map(|d| format!("{}: {}", d.code, d.message)).collect();
    assert_eq!(
        messages,
        [
            "SC-E0101: `a_length_mm` is \"long\", but it must be a length from 0.1 to 10 mm.",
            "SC-E0101: `a_toggle` is \"maybe\", but it must be true or false.",
            "SC-E0101: `a_choice` is \"three\", but it must be one of one, two.",
        ]
    );
    assert!(problems.iter().all(|d| d.severity() == Severity::Error));
}

#[test]
fn req_prm_002_out_of_range_values_are_clamped_with_a_warning() {
    let read = Everything::from_set(&set(&[("a_length_mm", "25"), ("a_percent", "250"), ("a_count", "0"), ("some_lengths_mm", "2.5 0.1")])).unwrap();
    assert_eq!(read.params.a_length_mm.get(), 10.0);
    assert_eq!(read.params.a_percent, 200.0);
    assert_eq!(read.params.a_count, 1);
    assert_eq!(read.params.some_lengths_mm.iter().map(|mm| mm.get()).collect::<Vec<_>>(), [2.5, 0.3]);
    let messages: Vec<String> = read.warnings.iter().map(|d| format!("{}: {}", d.code, d.message)).collect();
    assert_eq!(
        messages,
        [
            "SC-W0102: `a_length_mm` is 25, outside 0.1 to 10 mm; 10 mm is used.",
            "SC-W0102: `a_percent` is 250, outside 0 to 200%; 200% is used.",
            "SC-W0102: `a_count` is 0, outside 1 to 20; 1 is used.",
            "SC-W0102: `some_lengths_mm` is 2.5 0.1, outside 0.3 to 12 mm; 2.5 0.3 mm is used.",
        ]
    );
}

#[test]
fn diag_sc_e0101_a_value_of_the_wrong_kind_says_what_is_accepted() {
    let problems = Everything::from_set(&set(&[("a_choice", "three")])).unwrap_err();
    let shown: Vec<String> = problems.iter().map(ToString::to_string).collect();
    assert_eq!(shown, ["error SC-E0101: `a_choice` is \"three\", but it must be one of one, two."]);
}

#[test]
fn diag_sc_w0102_a_value_out_of_range_is_clamped_and_says_so() {
    let read = Everything::from_set(&set(&[("a_length_mm", "25")])).unwrap();
    let shown: Vec<String> = read.warnings.iter().map(ToString::to_string).collect();
    assert_eq!(shown, ["warning SC-W0102: `a_length_mm` is 25, outside 0.1 to 10 mm; 10 mm is used."]);
}

#[test]
fn diag_sc_w0105_unknown_keys_are_kept_and_reported() {
    let design = set(&[("a_length_mm", "3"), ("sparkle_mm", "1")]);
    let warnings = unknown_keys(&design, &[&Everything::GROUP]);
    assert_eq!(warnings.len(), 1);
    assert_eq!(warnings[0].code, Code::ParamUnknown);
    assert_eq!(
        warnings[0].to_string(),
        "warning SC-W0105: `sparkle_mm` is not a StitchCraft parameter; it was kept but does not change the stitches."
    );
    // Reading the known ones is unaffected, and the set still holds the unknown one for writing back.
    assert_eq!(Everything::from_set(&design).unwrap().params.a_length_mm.get(), 3.0);
    assert_eq!(design.get("sparkle_mm"), Some("1"));
}

#[test]
fn declarations_record_what_the_registry_needs() {
    let g = Everything::GROUP;
    assert_eq!(g.name, "Everything");
    assert_eq!(g.help(), "One parameter of every kind.");
    assert_eq!(g.applies_to, StitchType::ALL);
    assert_eq!(g.specs.len(), 11);
    let length = find(&[&g], "a_length_mm").unwrap();
    assert_eq!((length.label, length.help().as_str(), length.group, length.default), ("Length", "A length.", "Numbers", "2.5"));
    assert_eq!(length.kind, Kind::Length { min: 0.1, max: 10.0, optional: false });
    assert_eq!(length.applies_to, StitchType::ALL);
    assert_eq!(length.origin, Origin::InkStitch);
    let lengths = find(&[&g], "some_lengths_mm").unwrap();
    assert_eq!(lengths.applies_to, [StitchType::RunningStitch]);
    assert_eq!(find(&[&g], "a_detail").unwrap().visible_when, Some(Condition { key: "a_choice", equals: "two" }));
    assert_eq!(find(&[&g], "a_seed").unwrap().origin, Origin::StitchCraft);
}
