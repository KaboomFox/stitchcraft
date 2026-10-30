//! Reading a VectorCraft document with `--auto` (`REQ-AUTO-001`, `REQ-AUTO-003`, `REQ-AUTO-005`): an object
//! without StitchCraft effects, its own or a group's around it, is an automatic element; a stroke's cap is
//! read; and what VectorCraft draws across an automatic stroke sewn across its width, and its stitches don't
//! follow, is named. A line traced in VectorCraft then sews as wide as VectorCraft draws it, caps included.

// Test code may unwrap, panic and index (clippy.toml allows it in tests, but not in their helpers).
#![allow(clippy::unwrap_used, clippy::panic, clippy::indexing_slicing)]

mod common;

use common::{document, element, line, square};
use stitchcraft_core::units::MM_PER_POINT;
use stitchcraft_core::{Budget, Point};
use stitchcraft_engine::auto::OVERLAP;
use stitchcraft_engine::design::{Cap, Design, DesignSettings, Reading, Shape};
use stitchcraft_plan::profiles::REFERENCE;
use stitchcraft_vectorcraft::{VectorCraft, read_with};

/// `document` read with `--auto` or without.
fn read(document: &[u8], auto: bool) -> VectorCraft {
    read_with(document, &Budget::DEFAULT, Reading { auto, ..Reading::default() }).unwrap()
}

/// A black stroke `mm` millimetres wide, with `extra` fields.
fn stroke(mm: f64, extra: &str) -> String {
    let width = mm / MM_PER_POINT;
    format!(r#"{{"kind": "stroke", "width": {width}, "paint": {{"type": "solid", "color": {{"model": "gray", "k": 1}}}} {extra}}}"#)
}

/// An appearance of `items` and the StitchCraft effects of the `families` given, in order.
fn appearance(items: &[String], families: &[&str]) -> String {
    let effects: Vec<String> = families.iter().map(|family| format!(r#"{{"id": "plugin.dev.stitchcraft.{family}", "params": {{}}}}"#)).collect();
    format!(r#"{{"items": [{}], "effects": [{}]}}"#, items.join(","), effects.join(","))
}

/// A red fill.
fn red() -> String {
    r#"{"kind": "fill", "paint": {"type": "solid", "color": {"model": "rgb", "r": 1, "g": 0, "b": 0}}}"#.to_string()
}

/// A group with `id`, `appearance` and `children`.
fn group(id: u32, appearance: &str, children: &[String]) -> String {
    format!(r#"{{"id": {id}, "appearance": {appearance}, "kind": {{"type": "group", "children": [{}]}}}}"#, children.join(","))
}

#[test]
fn req_auto_001_an_object_without_stitchcraft_effects_is_automatic() {
    let objects = [
        line(2, "", &appearance(&[stroke(3.0, "")], &[])),
        square(3, 60.0, 0.0, &appearance(&[red()], &[])),
        line(4, "", &appearance(&[stroke(3.0, "")], &["satin"])),
        group(5, &appearance(&[], &["fill"]), &[square(6, 120.0, 0.0, &appearance(&[red()], &[]))]),
    ]
    .join(",");
    let with = read(&document(4, &objects), true);
    let automatic: Vec<(&str, bool)> = with.design.elements().iter().map(|e| (e.id.as_str(), e.automatic)).collect();
    assert_eq!(automatic, [("vc:2:stroke", true), ("vc:3:fill", true), ("vc:4:satin", false), ("vc:6:fill", false)]);
    // Without the switch, nothing is, and the design is otherwise the same.
    let without = read(&document(4, &objects), false);
    assert!(without.design.elements().iter().all(|e| !e.automatic));
    let unmarked: Vec<_> = with.design.elements().iter().map(|e| stitchcraft_engine::design::Element { automatic: false, ..e.clone() }).collect();
    assert_eq!(unmarked, without.design.elements());
}

#[test]
fn req_auto_003_a_stroke_s_cap_is_read() {
    let cap = |extra: &str| {
        let read = read(&document(4, &line(2, "", &appearance(&[stroke(3.0, extra)], &[]))), true);
        let Shape::Stroke { cap, .. } = element(&read, "vc:2:stroke").shape else { panic!() };
        cap
    };
    assert_eq!(
        (cap(""), cap(r#", "cap": "Butt""#), cap(r#", "cap": "Round""#), cap(r#", "cap": "Square""#)),
        (Cap::Butt, Cap::Butt, Cap::Round, Cap::Square)
    );
}

#[test]
fn req_auto_005_what_an_automatic_stroke_sewn_across_does_not_follow_is_named() {
    let named = |what: &str, hint: &str| (format!("warning SC-W0807: {what}"), hint.to_string());
    let said = |object: &str, auto: bool| -> Vec<(String, String)> {
        read(&document(4, object), auto).warnings.iter().map(|w| (w.to_string(), w.fix.as_ref().map(|f| f.describe()).unwrap_or_default())).collect()
    };
    // A satin column is centred on its path, and as wide as the stroke all along.
    let inside = square(2, 0.0, 0.0, &appearance(&[stroke(3.0, r#", "align": "Inside""#)], &[]));
    assert_eq!(
        said(&inside, true),
        [named(
            "The satin column is centred on its path, where VectorCraft draws the stroke inside it.",
            "Align the stroke to its centre in the Stroke panel."
        )]
    );
    let profile = r#", "profile": {"points": [[0, 0, 0], [0.5, 1, 1], [1, 0, 0]]}"#;
    assert_eq!(
        said(&line(2, "", &appearance(&[stroke(3.0, profile)], &[])), true),
        [named(
            "The satin column is as wide as the stroke all along, without its width profile.",
            "Set the stroke's profile to Uniform in the Stroke panel, or draw the column with 2 rails."
        )]
    );
    // So is the fill of a line wider than 7 mm.
    assert_eq!(
        said(&line(2, "", &appearance(&[stroke(10.0, profile)], &[])), true),
        [named(
            "The fill is as wide as the stroke all along, without its width profile.",
            "Set the stroke's profile to Uniform in the Stroke panel, or make a shape of it with Object › Path › Outline Stroke."
        )]
    );
    // Its caps are followed, and said of none. Nor is anything across a line that runs, or across one sewn
    // without the switch, which runs too.
    for (object, auto, case) in [
        (line(2, "", &appearance(&[stroke(3.0, r#", "cap": "Round""#)], &[])), true, "round caps"),
        (line(2, "", &appearance(&[stroke(10.0, r#", "cap": "Square""#)], &[])), true, "square caps of a fill"),
        (square(2, 0.0, 0.0, &appearance(&[stroke(0.5, &format!(r#", "align": "Inside"{profile}"#))], &[])), true, "a line that runs"),
        (inside, false, "without the switch"),
    ] {
        assert_eq!(said(&object, auto), Vec::<(String, String)>::new(), "{case}");
    }
}

#[test]
fn req_auto_003_a_line_traced_in_vectorcraft_sews_as_wide_and_as_long_as_drawn() {
    // As Image Trace's Create Strokes draws a line: a stroked centre line with round caps, in the group of
    // the trace. It is 50 points long and 2.5 mm wide, and sews as wide once the thread pulls it in: its
    // stitches reach the overlap past each edge (REQ-AUTO-018).
    let traced = group(9, &appearance(&[], &[]), &[line(2, "", &appearance(&[stroke(2.5, r#", "cap": "Round""#)], &[]))]);
    let read = read(&document(4, &traced), true);
    let origin = Point::new(0.0, 0.0).unwrap();
    let design = Design::new(read.design.elements().to_vec(), DesignSettings { origin: Some(origin), ..read.design.settings }).unwrap();
    let outcome = stitchcraft_engine::plan(&design, REFERENCE, &Budget::DEFAULT);
    let points: Vec<Point> = outcome.plan.as_ref().unwrap().stitches().map(|s| s.at).collect();
    let extent = |values: Vec<f64>| values.into_iter().fold((f64::MAX, f64::MIN), |(lo, hi), v| (lo.min(v), hi.max(v)));
    let (left, right) = extent(points.iter().map(|q| q.x()).collect());
    let (top, bottom) = extent(points.iter().map(|q| q.y()).collect());
    let length = 50.0 * MM_PER_POINT;
    assert!((top + 1.25 + OVERLAP).abs() < 0.05 && (bottom - 1.25 - OVERLAP).abs() < 0.05, "as wide as drawn: {top} to {bottom}");
    assert!((left + 1.25).abs() < 0.05 && (right - length - 1.25).abs() < 0.05, "caps included: {left} to {right}");
    assert_eq!(read.warnings.len(), 0, "{:?}", read.warnings);
    let said: Vec<String> = outcome.diagnostics.iter().map(ToString::to_string).collect();
    assert_eq!(said, ["info SC-I0901: 1 shape is digitized automatically: it sews as a satin column."]);
}
