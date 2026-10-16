//! What an element's settings sew, read as Ink/Stitch reads them (`REQ-PRM-002`, `REQ-PRM-004`,
//! `REQ-RUN-007`, `REQ-FILL-003`): a blank value is not set, a comma is no separator, an element reads only
//! the settings its stitch type uses, a `random_seed` replaces the element's id as the seed, and a fill
//! whose `fill` is off sews nothing.

// Test code may unwrap and index (clippy.toml allows it in tests).
#![allow(clippy::unwrap_used, clippy::panic, clippy::indexing_slicing)]

use stitchcraft_core::Code;
use stitchcraft_engine::PlanOutcome;
use stitchcraft_plan::StitchKind;
use stitchcraft_testkit::designs::{RED, fill, line, messages, planned};

/// The needle points the machine sews for element `id`, jumps, trims and stops left out.
fn needles_of(outcome: &PlanOutcome, id: &str) -> Vec<(f64, f64)> {
    let Some(plan) = outcome.plan.as_ref() else { return Vec::new() };
    plan.blocks
        .iter()
        .flat_map(|b| b.stitches.iter())
        .filter(|s| s.kind == StitchKind::Normal)
        .filter(|s| s.origin.element.is_some_and(|e| plan.elements[e.index()].as_str() == id))
        .map(|s| (s.at.x(), s.at.y()))
        .collect()
}

fn has(outcome: &PlanOutcome, code: Code) -> bool {
    outcome.diagnostics.iter().any(|d| d.code == code)
}

const SQUARE: &[(f64, f64)] = &[(0.0, 0.0), (10.0, 0.0), (10.0, 10.0), (0.0, 10.0)];

/// A U-shaped fill, whose needle travels between its arms.
const U: &[(f64, f64)] = &[(0.0, 0.0), (30.0, 0.0), (30.0, 30.0), (20.0, 30.0), (20.0, 10.0), (10.0, 10.0), (10.0, 30.0), (0.0, 30.0)];

#[test]
fn req_prm_002_a_blank_value_is_not_set_and_sews_as_the_default() {
    // Ink/Stitch strips every value and reads an empty one as not set, and its settings window stores a
    // field the user clears as an empty value.
    let plain = planned(vec![line("a", (0.0, 0.0), 10.0, &RED, &[])]);
    let mut refused = Vec::new();
    for key in [
        "running_stitch_length_mm",
        "running_stitch_tolerance_mm",
        "repeats",
        "bean_stitch_repeats",
        "enable_random_stitch_length",
        "ties",
        "lock_start",
        "trim_after",
        "stroke_method",
    ] {
        for blank in ["", " ", "\t"] {
            let outcome = planned(vec![line("a", (0.0, 0.0), 10.0, &RED, &[(key, blank)])]);
            if outcome.plan != plain.plan || !outcome.diagnostics.is_empty() {
                refused.push(format!("stroke {key}={blank:?}: {:?}", messages(&outcome)));
            }
        }
    }
    let plain_fill = planned(vec![fill("f", &[SQUARE], &RED, &[])]);
    for key in ["pull_compensation_mm", "row_spacing_mm", "angle", "fill_underlay", "staggers"] {
        let outcome = planned(vec![fill("f", &[SQUARE], &RED, &[(key, "")])]);
        if outcome.plan != plain_fill.plan || has(&outcome, Code::ParamInvalid) {
            refused.push(format!("fill {key}: {:?}", messages(&outcome)));
        }
    }
    assert!(refused.is_empty(), "{refused:#?}");
}

#[test]
fn req_prm_002_a_comma_in_a_list_refuses_the_element() {
    // `3,5`, a decimal comma or 2 lengths: Ink/Stitch splits a list at spaces and reads neither.
    let outcome = planned(vec![line("a", (0.0, 0.0), 10.0, &RED, &[("running_stitch_length_mm", "3,5")]), line("b", (0.0, 5.0), 10.0, &RED, &[])]);
    assert!(has(&outcome, Code::ParamInvalid), "{:?}", messages(&outcome));
    assert!(needles_of(&outcome, "a").is_empty());
}

#[test]
fn req_prm_004_an_element_reads_only_the_settings_its_stitch_type_uses() {
    // A running stitch with a zigzag stroke's spacing, a list Ink/Stitch's zigzag reads and a satin's
    // spacing cannot be: the attribute stays on a path whose method goes back to running stitch.
    let plain = planned(vec![line("a", (0.0, 0.0), 10.0, &RED, &[])]);
    for kept in [
        &[("stroke_method", "running_stitch"), ("zigzag_spacing_mm", "0.4 0.8")][..],
        // A meander fill's spacing of 0, which a stroke on the same path does not read.
        &[("zigzag_spacing_mm", "0")],
        // Running stitch has no longest stitch.
        &[("stroke_method", "running_stitch"), ("max_stitch_length_mm", "abc")],
        // A length variation, hidden while random stitch length is off.
        &[("random_stitch_length_jitter_percent", "lots")],
    ] {
        let outcome = planned(vec![line("a", (0.0, 0.0), 10.0, &RED, kept)]);
        assert_eq!((messages(&outcome), outcome.plan == plain.plan), (Vec::<String>::new(), true), "{kept:?}");
    }
    // Manual stitch has no repeats.
    let manual = |extra: &[(&str, &str)]| {
        let mut params = vec![("stroke_method", "manual_stitch")];
        params.extend_from_slice(extra);
        planned(vec![line("a", (0.0, 0.0), 10.0, &RED, &params)])
    };
    let (alone, with_repeats) = (manual(&[]), manual(&[("repeats", "2.5")]));
    assert_eq!((messages(&with_repeats), with_repeats.plan), (Vec::<String>::new(), alone.plan));
    // A setting the stitch type uses is still read, and refuses the element when it cannot be read: a
    // running stitch's repeats, or the variation shown while random stitch length is on.
    for wrong in [&[("repeats", "2.5")][..], &[("enable_random_stitch_length", "true"), ("random_stitch_length_jitter_percent", "lots")]] {
        let outcome = planned(vec![line("a", (0.0, 0.0), 10.0, &RED, wrong)]);
        assert!(has(&outcome, Code::ParamInvalid), "{wrong:?}: {:?}", messages(&outcome));
    }
}

#[test]
fn req_run_007_a_random_seed_replaces_the_element_s_id_as_the_seed() {
    // Ink/Stitch seeds an element with its `random_seed` text when it has one, and with its id only when it
    // is empty: 2 lines alike with the same seed sew the same random lengths, and "0" is a seed too.
    let random = |seed: Option<&'static str>| {
        let mut params = vec![("enable_random_stitch_length", "true"), ("random_stitch_length_jitter_percent", "50")];
        params.extend(seed.map(|seed| ("random_seed", seed)));
        params
    };
    let xs = |outcome: &PlanOutcome, id: &str| needles_of(outcome, id).iter().map(|(x, _)| (x * 1e6).round()).collect::<Vec<_>>();
    let same = planned(vec![line("a", (0.0, 0.0), 40.0, &RED, &random(Some("12345"))), line("b", (0.0, 5.0), 40.0, &RED, &random(Some("12345")))]);
    assert_eq!(xs(&same, "a"), xs(&same, "b"));
    let unseeded = planned(vec![line("a", (0.0, 0.0), 40.0, &RED, &random(None)), line("b", (0.0, 5.0), 40.0, &RED, &random(None))]);
    assert_ne!(xs(&unseeded, "a"), xs(&unseeded, "b"), "without a seed, each element's id seeds it");
    let one = |seed| xs(&planned(vec![line("a", (0.0, 0.0), 40.0, &RED, &random(seed))]), "a");
    assert_ne!(one(Some("0")), one(None), "\"0\" is a seed, not the element's id");
    assert_ne!(one(Some("7")), one(Some("0")));
}

#[test]
fn req_fill_003_a_fill_set_not_to_fill_sews_nothing() {
    // Ink/Stitch's `fill` off sews nothing for the fill, and the next element still sews.
    let outcome = planned(vec![fill("f", &[SQUARE], &RED, &[("fill", "false")]), line("a", (0.0, 20.0), 10.0, &RED, &[])]);
    assert_eq!(needles_of(&outcome, "f").len(), 0, "{:?}", messages(&outcome));
    assert!(!needles_of(&outcome, "a").is_empty());
    assert_eq!(messages(&outcome), Vec::<String>::new());
    let on = planned(vec![fill("f", &[SQUARE], &RED, &[("fill", "true")])]);
    assert_eq!(on.plan, planned(vec![fill("f", &[SQUARE], &RED, &[])]).plan);
    // It is still the next element of the one before it, which ends near it, as in Ink/Stitch.
    let before = |on: &str| {
        let far: &[(f64, f64)] = &[(40.0, 0.0), (50.0, 0.0), (50.0, 10.0), (40.0, 10.0)];
        needles_of(&planned(vec![fill("e", &[far], &RED, &[]), fill("f", &[SQUARE], &RED, &[("fill", on)])]), "e")
    };
    assert_eq!(before("false"), before("true"));
}

#[test]
fn a_fill_travels_in_the_first_of_a_list_of_running_lengths() {
    // DEV-SAT-005, for fills too: the key is declared once, as a list, and a fill's travel takes its first
    // length, where Ink/Stitch's fill reads the key as one number and travels in its default 2.5 mm.
    let sew = |lengths: &str| needles_of(&planned(vec![fill("u", &[U], &RED, &[("running_stitch_length_mm", lengths)])]), "u");
    let (short, default, pattern) = (sew("1"), sew("2.5"), sew("1 2.5"));
    assert_ne!(short, default);
    assert_eq!(pattern, short);
}
