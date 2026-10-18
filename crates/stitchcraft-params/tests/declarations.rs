//! The `params!` declaration end to end, with one parameter of every kind: what the registry records,
//! and how designs' values are read (REQ-PRM-002: invalid values are errors that say what is accepted,
//! out-of-range values are clamped with a warning, and nothing falls back to a default in silence; an
//! empty value is not set), and read for one stitch type (REQ-PRM-004: only the settings it uses).

// Test code may unwrap (clippy.toml allows it inside #[test] functions; these helpers are test code too).
#![allow(clippy::unwrap_used)]

use stitchcraft_core::{Code, Diagnostic, Fix, Severity};
use stitchcraft_params::{Condition, Empty, Family, Kind, Origin, ParamSet, StitchType, Validated, find, params, unknown_keys};

params! {
    /// One parameter of every kind.
    pub struct Everything for StitchType::ALL;

    "Numbers" {
        /// A length.
        a_length_mm: Length = "2.5", label "Length", range (0.1, 10.0);

        /// A length that may be empty.
        an_optional_mm: OptionalLength = "", label "Optional length", range (0.0, 5.0);

        /// A length that is empty unless the stitch type is a fill's.
        a_fill_length_mm: OptionalLength = "", label "Fill length", range (0.1, 25.0), defaults [Family::Fill => "4.0"];

        /// An angle.
        an_angle: Angle = "0", label "Angle";

        /// An angle within a range.
        a_tilt: BoundedAngle = "0", label "Tilt", range (-89.0, 89.0);

        /// A percentage.
        a_percent: Percent = "100", label "Percentage", range (0.0, 200.0);

        /// A whole number.
        a_count: Count = "4", label "Count", range (1, 20);
    }

    "Others" {
        /// On or off.
        a_toggle: Toggle = "false", label "Toggle";

        /// A length shown while the toggle is on.
        a_toggled_mm: Length = "1", label "Toggled length", range (0.1, 10.0), when a_toggle == "true";

        /// A choice.
        a_choice: Choice = "one", label "Choice", options ["one" => "One", "two" => "Two"];

        /// Detail for the second choice.
        a_detail: Text = "", label "Detail", when a_choice == "two";

        /// A seed, shown for either choice.
        a_seed: Seed = "", label "Seed", when a_choice in &["one", "two"], origin Origin::StitchCraft;

        /// Lengths, for running stitch only.
        some_lengths_mm: LengthList = "2.5", label "Lengths", range (0.3, 12.0), applies &[StitchType::RunningStitch];

        /// Whole numbers.
        some_counts: CountList = "0", label "Counts", range (0, 9);

        /// Angles, or none.
        some_angles: OptionalAngleList = "", label "Angles";
    }
}

fn set(pairs: &[(&str, &str)]) -> ParamSet {
    pairs.iter().copied().collect()
}

/// The codes of what reading said: its warnings, or its errors.
fn codes<T>(read: &Result<Validated<T>, Vec<Diagnostic>>) -> Vec<Code> {
    let said = match read {
        Ok(read) => &read.warnings,
        Err(problems) => problems,
    };
    said.iter().map(|d| d.code).collect()
}

#[test]
fn defaults_apply_when_the_design_says_nothing() {
    let read = Everything::from_set(&ParamSet::new()).unwrap();
    assert!(read.warnings.is_empty());
    let p = read.params;
    assert_eq!((p.a_length_mm.get(), p.an_optional_mm, p.an_angle, p.a_percent, p.a_count), (2.5, None, 0.0, 100.0, 4));
    assert_eq!((p.a_toggle, p.a_choice, p.a_detail.as_str(), p.a_seed), (false, "one", "", None));
    assert_eq!((p.some_lengths_mm.len(), p.some_counts.as_slice(), p.some_angles), (1, [0].as_slice(), None));
}

#[test]
fn a_list_of_angles_is_read_normalized_or_left_empty() {
    let read = |value: &str| Everything::from_set(&set(&[("some_angles", value)])).map(|read| read.params.some_angles);
    assert_eq!(read(" "), Ok(None));
    assert_eq!(read("90 270"), Ok(Some(vec![90.0, -90.0])));
    let problems = read("90 up").unwrap_err();
    assert_eq!(
        problems.iter().map(ToString::to_string).collect::<Vec<_>>(),
        ["error SC-E0101: `some_angles` is \"90 up\", but it must be 1 to 16 angles in degrees, separated by spaces, or empty."]
    );
    let spec = find(&[&Everything::GROUP], "some_angles").unwrap();
    assert_eq!((spec.kind, spec.kind.unit()), (Kind::AngleList { optional: true }, "°"));
}

#[test]
fn a_family_reads_its_own_default_and_the_others_the_plain_one() {
    let spec = find(&[&Everything::GROUP], "a_fill_length_mm").unwrap();
    assert_eq!((spec.default, spec.family_defaults), ("", &[(Family::Fill, "4.0")][..]));
    assert_eq!(Family::ALL.iter().map(|family| spec.default_for(*family)).collect::<Vec<_>>(), ["", "", "4.0"]);
    let fill = Everything::from_set_as(&ParamSet::new(), StitchType::TatamiFill).unwrap().params;
    assert_eq!(fill.a_fill_length_mm.map(|mm| mm.get()), Some(4.0));
    assert_eq!(Everything::from_set_as(&ParamSet::new(), StitchType::SatinColumn).unwrap().params.a_fill_length_mm, None);
    assert_eq!(Everything::from_set(&ParamSet::new()).unwrap().params.a_fill_length_mm, None);
    // A family with a default of its own needs a value, as Ink/Stitch's fills need a longest stitch: left
    // empty, the length is the family's default, and 0 or less is raised to the least it accepts. The
    // other families read both as no value.
    let fill = |value: &str| {
        let read = Everything::from_set_as(&set(&[("a_fill_length_mm", value)]), StitchType::TatamiFill).unwrap();
        (read.params.a_fill_length_mm.map(|mm| mm.get()), read.warnings.iter().map(|d| format!("{}: {}", d.code, d.message)).collect::<Vec<_>>())
    };
    assert_eq!(fill(" "), (Some(4.0), vec![]));
    assert_eq!(fill("2.5"), (Some(2.5), vec![]));
    assert_eq!(fill("0"), (Some(0.1), vec!["SC-W0102: `a_fill_length_mm` is 0, outside 0.1 to 25 mm; 0.1 mm is used.".to_string()]));
    assert_eq!(fill("-2").0, Some(0.1));
    for value in ["", "0", "-2"] {
        let read = |stitch_type| Everything::from_set_as(&set(&[("a_fill_length_mm", value)]), stitch_type).unwrap();
        let others = [StitchType::RunningStitch, StitchType::SatinColumn].map(read);
        let named = usize::from(value == "-2");
        assert!(others.iter().all(|read| read.params.a_fill_length_mm.is_none() && read.warnings.len() == named), "{value}");
        assert_eq!(Everything::from_set(&set(&[("a_fill_length_mm", value)])).unwrap().params.a_fill_length_mm, None);
    }
}

#[test]
fn req_prm_002_an_empty_or_blank_value_is_not_set_whatever_the_kind() {
    // Ink/Stitch strips every value and reads an empty one as not set, and its settings window stores a
    // field the user clears as an empty value.
    let plain = Everything::from_set(&ParamSet::new());
    for spec in Everything::SPECS {
        for blank in ["", " ", "\t \n"] {
            assert_eq!(Everything::from_set(&set(&[(spec.key, blank)])), plain, "{} = {blank:?}", spec.key);
        }
    }
}

#[test]
fn req_prm_002_a_comma_neither_separates_values_nor_marks_decimals() {
    // Ink/Stitch splits a list at its spaces, and reads a number with a comma in it as no number.
    let message = "error SC-E0101: `some_lengths_mm` is \"3,5\", but it must be 1 to 16 lengths from 0.3 to 12 mm, separated by spaces.";
    let problems = Everything::from_set(&set(&[("some_lengths_mm", "3,5")])).unwrap_err();
    assert_eq!(problems.iter().map(ToString::to_string).collect::<Vec<_>>(), [message]);
    let hint = Some(Fix::Hint("Separate values with spaces, and write decimals with a point.".to_string()));
    assert_eq!(problems[0].fix, hint);
    for (key, text) in [("some_counts", "1,0"), ("some_angles", "45,90"), ("a_length_mm", "2,5"), ("an_angle", "1,5")] {
        let problems = Everything::from_set(&set(&[(key, text)])).unwrap_err();
        assert_eq!(problems.iter().map(|d| (d.code, d.fix.clone())).collect::<Vec<_>>(), [(Code::ParamInvalid, hint.clone())], "{key}");
    }
    // A value that does not read for another reason has no such hint, nor has a comma where values are not
    // numbers.
    assert_eq!(Everything::from_set(&set(&[("some_counts", "1 x")])).unwrap_err()[0].fix, None);
    assert_eq!(Everything::from_set(&set(&[("a_choice", "one,two")])).unwrap_err()[0].fix, None);
    // Ink/Stitch drops a comma at the end of each of a fill's underlay angles, which older files wrote.
    assert_eq!(Everything::from_set(&set(&[("some_angles", "45, 90,")])).unwrap().params.some_angles, Some(vec![45.0, 90.0]));
}

#[test]
fn req_prm_002_a_toggle_reads_ink_stitch_s_words_and_refuses_on() {
    // Ink/Stitch reads yes, y, true, t and 1, in any case, as on, and any other text as off: `on` as well.
    let toggle = |text: &str| Everything::from_set(&set(&[("a_toggle", text)])).map(|read| read.params.a_toggle);
    for on in ["true", "True", "yes", "Y", "t", "1", " T "] {
        assert_eq!(toggle(on), Ok(true), "{on}");
    }
    for off in ["false", "FALSE", "no", "n", "f", "0", "off"] {
        assert_eq!(toggle(off), Ok(false), "{off}");
    }
    let problems = toggle("on").unwrap_err();
    assert_eq!(
        problems.iter().map(ToString::to_string).collect::<Vec<_>>(),
        [
            "error SC-E0101: `a_toggle` is \"on\", but it must be true or false, in any case, or yes, y, t or 1 for true and no, n, f, 0 or off for false."
        ]
    );
}

#[test]
fn req_prm_002_text_is_stripped_as_ink_stitch_strips_it() {
    // Ink/Stitch strips every value before it reads it: a text's tabs, line ends and other white space round
    // it, as well as its spaces.
    let detail = |text: &str| Everything::from_set(&set(&[("a_choice", "two"), ("a_detail", text)])).unwrap().params.a_detail;
    for text in ["kept", " kept ", "\tkept\r\n", "\nkept\u{a0}"] {
        assert_eq!(detail(text), "kept", "{text:?}");
    }
    assert_eq!(detail(" kept as is "), "kept as is", "white space inside stays");
}

#[test]
fn req_prm_002_a_length_below_0_that_leaves_it_unset_is_named() {
    // 0 leaves an optional length unset, as Ink/Stitch reads it. A length below 0 does too, and is named:
    // Ink/Stitch reads it as a length, and a jump length below 0 jumps between every 2 groups there.
    let optional = |text: &str| {
        let read = Everything::from_set(&set(&[("an_optional_mm", text)])).unwrap();
        (read.params.an_optional_mm, read.warnings.iter().map(ToString::to_string).collect::<Vec<_>>())
    };
    assert_eq!(optional("0"), (None, vec![]));
    assert_eq!(optional("-0"), (None, vec![]));
    assert_eq!(optional("-1"), (None, vec!["warning SC-W0102: `an_optional_mm` is -1, below 0, so it is not set.".to_string()]));
}

#[test]
fn req_prm_002_a_unit_follows_its_number_with_no_space() {
    // A single value read a unit after a space, which a list could not, since it splits at spaces first.
    let length = |text: &str| Everything::from_set(&set(&[("a_length_mm", text)])).map(|read| read.params.a_length_mm.get());
    assert_eq!(length("2.5mm"), Ok(2.5));
    assert_eq!(length(" 0.1in "), Ok(2.54));
    for spaced in ["2.5 mm", "0.1 in"] {
        assert_eq!(codes(&Everything::from_set(&set(&[("a_length_mm", spaced)]))), [Code::ParamInvalid], "{spaced}");
    }
    assert_eq!(codes(&Everything::from_set(&set(&[("some_lengths_mm", "2.5 mm")]))), [Code::ParamInvalid]);
    assert_eq!(codes(&Everything::from_set(&set(&[("a_percent", "50 %")]))), [Code::ParamInvalid]);
}

#[test]
fn req_prm_002_a_value_longer_than_4096_bytes_does_not_read_and_its_message_quotes_its_start() {
    // A list's values are read up to the 17th, and no value is read past 4,096 bytes, so a long text costs
    // no more than that. The message quotes the text's first 64 characters.
    let long = "1 ".repeat(3000);
    let problems = Everything::from_set(&set(&[("some_lengths_mm", long.as_str())])).unwrap_err();
    let start = "1 ".repeat(32);
    assert_eq!(
        problems.iter().map(ToString::to_string).collect::<Vec<_>>(),
        [format!(
            "error SC-E0101: `some_lengths_mm` is \"{start}…\" (6000 bytes), but it must be 1 to 16 lengths from 0.3 to 12 mm, separated by spaces."
        )]
    );
    let seed = "x".repeat(4097);
    assert_eq!(codes(&Everything::from_set(&set(&[("a_seed", seed.as_str())]))), [Code::ParamInvalid]);
    assert_eq!(codes(&Everything::from_set(&set(&[("a_seed", &seed[1..])]))), []);
}

#[test]
fn req_prm_004_a_stitch_type_reads_only_the_settings_it_uses() {
    // `some_lengths_mm` applies to running stitch alone. Another stitch type takes its default and keeps
    // its text unread: text it could not read refuses nothing, and a value out of range is not clamped.
    for text in ["abc", "99"] {
        let design = set(&[("some_lengths_mm", text)]);
        let fill = Everything::from_set_as(&design, StitchType::TatamiFill).unwrap();
        assert_eq!((fill.params.some_lengths_mm.iter().map(|mm| mm.get()).collect::<Vec<_>>(), fill.warnings), (vec![2.5], vec![]), "{text}");
        assert_eq!(design.get("some_lengths_mm"), Some(text));
    }
    assert_eq!(codes(&Everything::from_set_as(&set(&[("some_lengths_mm", "abc")]), StitchType::RunningStitch)), [Code::ParamInvalid]);
    // A setting shown only while another has a value is read only then: `a_toggled_mm` while `a_toggle` is
    // on, however the toggle is written.
    let toggled = |toggle: Option<&str>| {
        let mut pairs = vec![("a_toggled_mm", "abc")];
        pairs.extend(toggle.map(|toggle| ("a_toggle", toggle)));
        Everything::from_set_as(&set(&pairs), StitchType::RunningStitch)
    };
    for off in [None, Some("false"), Some("0"), Some(" ")] {
        assert_eq!(toggled(off).map(|read| read.params.a_toggled_mm.get()), Ok(1.0), "{off:?}");
    }
    for on in ["true", "True", "1", "yes"] {
        assert_eq!(codes(&toggled(Some(on))), [Code::ParamInvalid], "{on}");
    }
    // While the other setting does not read, the setting is read too, and both are said.
    assert_eq!(codes(&toggled(Some("maybe"))), [Code::ParamInvalid, Code::ParamInvalid]);
    // `a_detail` is shown for the second choice, and the first is the default.
    let detail = |pairs: &[(&str, &str)]| Everything::from_set_as(&set(pairs), StitchType::RunningStitch).unwrap().params.a_detail;
    assert_eq!(detail(&[("a_detail", "kept")]), "");
    assert_eq!(detail(&[("a_choice", "two"), ("a_detail", "kept")]), "kept");
    // `from_set` reads every setting, for a host that checks them all.
    assert_eq!(codes(&Everything::from_set(&set(&[("some_lengths_mm", "abc")]))), [Code::ParamInvalid]);
    assert_eq!(codes(&Everything::from_set(&set(&[("a_toggled_mm", "abc")]))), [Code::ParamInvalid]);
}

#[test]
fn req_prm_002_invalid_values_are_errors_that_say_what_is_accepted() {
    let problems = Everything::from_set(&set(&[("a_length_mm", "long"), ("a_choice", "three"), ("a_toggle", "maybe")])).unwrap_err();
    let messages: Vec<String> = problems.iter().map(|d| format!("{}: {}", d.code, d.message)).collect();
    assert_eq!(
        messages,
        [
            "SC-E0101: `a_length_mm` is \"long\", but it must be a length from 0.1 to 10 mm.",
            "SC-E0101: `a_toggle` is \"maybe\", but it must be true or false, in any case, or yes, y, t or 1 for true and no, n, f, 0 or off for false.",
            "SC-E0101: `a_choice` is \"three\", but it must be one of one, two.",
        ]
    );
    assert!(problems.iter().all(|d| d.severity() == Severity::Error));
}

#[test]
fn req_prm_002_out_of_range_values_are_clamped_with_a_warning() {
    // A list is read whole, but an element may sew only some of its values, as a zigzag sews its first
    // spacing: the warning says what the list is read as, and a single value's what is used.
    let read = Everything::from_set(&set(&[
        ("a_length_mm", "25"),
        ("a_tilt", "270"),
        ("a_percent", "250"),
        ("a_count", "0"),
        ("some_lengths_mm", "2.5 0.1"),
    ]))
    .unwrap();
    assert_eq!(read.params.a_length_mm.get(), 10.0);
    assert_eq!(read.params.a_tilt, 89.0, "held in its range as given, not turned to -90");
    assert_eq!(read.params.a_percent, 200.0);
    assert_eq!(read.params.a_count, 1);
    assert_eq!(read.params.some_lengths_mm.iter().map(|mm| mm.get()).collect::<Vec<_>>(), [2.5, 0.3]);
    let messages: Vec<String> = read.warnings.iter().map(|d| format!("{}: {}", d.code, d.message)).collect();
    assert_eq!(
        messages,
        [
            "SC-W0102: `a_length_mm` is 25, outside 0.1 to 10 mm; 10 mm is used.",
            "SC-W0102: `a_tilt` is 270, outside -89 to 89°; 89° is used.",
            "SC-W0102: `a_percent` is 250, outside 0 to 200%; 200% is used.",
            "SC-W0102: `a_count` is 0, outside 1 to 20; 1 is used.",
            "SC-W0102: `some_lengths_mm` is 2.5 0.1, outside 0.3 to 12 mm, and is read as 2.5 0.3 mm.",
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
    assert_eq!(g.specs.len(), 15);
    assert_eq!(find(&[&g], "a_tilt").map(|spec| spec.kind), Some(Kind::BoundedAngle { min: -89.0, max: 89.0 }));
    let length = find(&[&g], "a_length_mm").unwrap();
    assert_eq!((length.label, length.help().as_str(), length.group, length.default), ("Length", "A length.", "Numbers", "2.5"));
    assert!(length.family_defaults.is_empty(), "the same default for every family unless it says");
    assert_eq!(length.kind, Kind::Length { min: 0.1, max: 10.0, empty: Empty::Never });
    assert_eq!(length.applies_to, StitchType::ALL);
    assert_eq!(length.origin, Origin::InkStitch);
    let lengths = find(&[&g], "some_lengths_mm").unwrap();
    assert_eq!(lengths.applies_to, [StitchType::RunningStitch]);
    assert_eq!(find(&[&g], "a_detail").unwrap().visible_when, Some(Condition { key: "a_choice", any_of: &["two"] }));
    let seed = find(&[&g], "a_seed").unwrap();
    assert_eq!((seed.visible_when, seed.origin), (Some(Condition { key: "a_choice", any_of: &["one", "two"] }), Origin::StitchCraft));
}
