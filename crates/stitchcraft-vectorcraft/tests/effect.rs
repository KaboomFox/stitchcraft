//! What a live effect reads of the object VectorCraft gives it (`REQ-VC-010`). From 0.14 VectorCraft gives
//! the object's strokes beside its path, and a stroke or a satin effect follows the topmost that paints, as
//! `stitch export` follows the topmost stroke VectorCraft draws. An effect on one of the object's fills or
//! strokes, which `stitch export` doesn't use, leaves the object as it is.

// Test code may unwrap (clippy.toml allows it in tests).
#![allow(clippy::unwrap_used, clippy::panic)]

mod common;

use serde_json::{Value, json};
use stitchcraft_core::Budget;
use stitchcraft_engine::design::{Cap, Join, Shape};
use stitchcraft_params::Family;
use stitchcraft_vectorcraft::{effect_error, read_effect};

use common::{document, element, line, read_ok};

/// 3 mm, in points.
const THREE_MM: f64 = 3.0 * 72.0 / 25.4;

/// The line [`common::line`] draws, 50 points long, as VectorCraft gives a live effect its path.
fn path() -> Value {
    json!({"subpaths": [{"anchors": [{"p": [0, 0]}, {"p": [50, 0]}]}]})
}

/// VectorCraft's input for one object along [`path`], with `extra` fields: from 0.14, its `fills` and
/// `strokes`.
fn given(extra: Value) -> Vec<u8> {
    let mut object = json!({"type": "path", "path": path(), "fillRule": "nonzero", "bounds": [0, 0, 50, 0]});
    if let (Some(object), Value::Object(extra)) = (object.as_object_mut(), extra) {
        object.extend(extra);
    }
    serde_json::to_vec(&json!({"objects": [object]})).unwrap()
}

/// VectorCraft's input for one object along [`path`] with no fills and `strokes`, bottom first.
fn stroked(strokes: &Value) -> Vec<u8> {
    given(json!({"fills": [], "strokes": strokes}))
}

/// The parameters of an effect that `item` says where it sits: `"object"`, or one of its paints.
fn on(item: &Value) -> Vec<u8> {
    serde_json::to_vec(&json!({"_context": {"mode": "effect", "bounds": [0, 0, 50, 0], "item": item}})).unwrap()
}

/// A plain blue, as VectorCraft gives a paint.
fn blue() -> Value {
    json!({"type": "solid", "color": {"model": "rgb", "r": 0, "g": 0, "b": 1}})
}

/// A stroke as VectorCraft 0.15 gives one: its paint, its width in points, its join and cap as SVG names
/// them, and its miter limit.
fn stroke(paint: &Value, width: f64, join: &str, cap: &str, limit: f64) -> Value {
    json!({"paint": paint, "width": width, "join": join, "cap": cap, "miterLimit": limit})
}

/// The element `stitch export` sews of a line with the satin effect and the stroke `item`, a stroke item of a
/// document's appearance, as JSON.
fn exported(item: &str) -> Shape {
    let appearance = format!(r#"{{"items": [{item}], "effects": [{{"id": "plugin.dev.stitchcraft.satin"}}]}}"#);
    element(&read_ok(&document(4, &line(2, "", &appearance))), "vc:2:satin").shape.clone()
}

/// The style of a stroke element: its width in millimetres, join and cap.
fn style(shape: &Shape) -> (f64, Join, Cap) {
    let Shape::Stroke { width, join, cap, .. } = shape else { panic!("not a stroke: {shape:?}") };
    (width.get(), *join, *cap)
}

#[test]
fn req_vc_010_a_stroke_or_satin_effect_follows_the_topmost_stroke_that_paints() {
    let strokes = json!([
        stroke(&blue(), 10.0, "round", "square", 10.0),
        stroke(&blue(), THREE_MM, "bevel", "round", 10.0),
        // Above it, strokes that paint nothing: no paint, a gradient without stops, no width.
        stroke(&Value::Null, 20.0, "miter", "butt", 10.0),
        stroke(&json!({"type": "gradient", "gradient": {"kind": "Linear", "stops": []}}), 20.0, "miter", "butt", 10.0),
        stroke(&blue(), 0.0, "miter", "butt", 10.0),
    ]);
    for family in [Family::Stroke, Family::Satin] {
        let effect = read_effect(family, &stroked(&strokes), &on(&json!("object")), &Budget::DEFAULT).unwrap();
        let (width, join, cap) = style(&effect.element.shape);
        assert!((width - 3.0).abs() < 1e-9, "{family:?}: {width}");
        assert_eq!((join, cap, effect.stroked), (Join::Bevel, Cap::Round, true), "{family:?}");
    }
}

#[test]
fn req_vc_010_a_live_effect_follows_a_stroke_as_stitch_export_does() {
    // A mitred, square-capped stroke 3 mm wide: in a document, and given a live effect on the object.
    let item = format!(r#"{{"kind": "stroke", "width": {THREE_MM}, "join": "Miter", "miter_limit": 4, "cap": "Square", "paint": {}}}"#, blue());
    let effect =
        read_effect(Family::Satin, &stroked(&json!([stroke(&blue(), THREE_MM, "miter", "square", 4.0)])), &on(&json!("object")), &Budget::DEFAULT)
            .unwrap();
    assert_eq!(effect.element.shape, exported(&item));
    assert_eq!(style(&effect.element.shape), (3.0, Join::Miter { limit: 4.0 }, Cap::Square));
    // Round, with a round cap.
    let item = format!(r#"{{"kind": "stroke", "width": {THREE_MM}, "join": "Round", "cap": "Round", "paint": {}}}"#, blue());
    let effect =
        read_effect(Family::Satin, &stroked(&json!([stroke(&blue(), THREE_MM, "round", "round", 10.0)])), &on(&json!("object")), &Budget::DEFAULT)
            .unwrap();
    assert_eq!(effect.element.shape, exported(&item));
}

#[test]
fn req_vc_010_a_stroke_from_vectorcraft_0_14_has_the_join_and_cap_of_a_file_that_leaves_them_out() {
    // 0.14 gives a stroke's paint and width alone, and no `item`.
    let effect = read_effect(Family::Satin, &stroked(&json!([{"paint": blue(), "width": THREE_MM}])), b"{}", &Budget::DEFAULT).unwrap();
    let item = format!(r#"{{"kind": "stroke", "width": {THREE_MM}, "paint": {}}}"#, blue());
    assert_eq!(effect.element.shape, exported(&item));
    assert_eq!(style(&effect.element.shape), (3.0, Join::Miter { limit: 10.0 }, Cap::Butt));
    assert!(effect.stroked);
}

#[test]
fn req_vc_010_given_no_stroke_that_paints_an_effect_has_the_stand_in_stroke() {
    // Up to 0.13 VectorCraft gives no paints at all.
    let stand_in = read_effect(Family::Satin, &given(json!({})), b"{}", &Budget::DEFAULT).unwrap();
    assert!(!stand_in.stroked);
    // From 0.14, an object without a stroke, or whose strokes paint nothing: VectorCraft writes a stroke of
    // no paint, with a width, for an object without one.
    for strokes in [json!([]), json!([stroke(&Value::Null, THREE_MM, "round", "round", 10.0)])] {
        let effect = read_effect(Family::Satin, &stroked(&strokes), &on(&json!("object")), &Budget::DEFAULT).unwrap();
        assert_eq!((&effect.element.shape, effect.stroked), (&stand_in.element.shape, false), "{strokes}");
    }
}

#[test]
fn req_vc_010_an_effect_on_one_of_the_object_s_paints_leaves_it_as_it_is() {
    let strokes = json!([stroke(&blue(), THREE_MM, "miter", "butt", 10.0)]);
    for family in Family::ALL {
        for item in [json!({"fill": 0}), json!({"stroke": 0})] {
            let refused = read_effect(*family, &stroked(&strokes), &on(&item), &Budget::DEFAULT).unwrap_err();
            let error: Value = serde_json::from_slice(&effect_error(&refused)).unwrap();
            assert_eq!(
                error["error"],
                "info SC-I0810: This effect is on one of the object's fills or strokes, which `stitch export` doesn't use, so the live effect \
                 leaves the object as it is. Apply StitchCraft's effects to the object itself.",
                "{family:?} on {item}"
            );
        }
        // On the object, it reads it.
        assert!(read_effect(*family, &stroked(&strokes), &on(&json!("object")), &Budget::DEFAULT).is_ok(), "{family:?}");
    }
}
