//! `REQ-VC-022`: a linear gradient fill made in VectorCraft sews its object's linear gradient, placed where
//! VectorCraft draws it (`REQ-FILL-GRD-001`), in `stitch export` and in the fill's live effect alike. The
//! squares of [`common::square`] are 36 points a side, and a point is 25.4 / 72 mm.

// Test code may unwrap (clippy.toml allows it in tests).
#![allow(clippy::unwrap_used, clippy::panic)]

mod common;

use serde_json::{Value, json};
use stitchcraft_core::{Budget, Code, Point};
use stitchcraft_engine::design::{Gradient, Shape};
use stitchcraft_params::Family;
use stitchcraft_plan::Rgb;
use stitchcraft_vectorcraft::{VectorCraft, read_effect};

use common::{document, element, read_ok, said, square};

/// A point, in millimetres.
const PT: f64 = 25.4 / 72.0;
const RED: Rgb = Rgb::new(255, 0, 0);
const BLUE: Rgb = Rgb::new(0, 0, 255);

/// A stop at `offset` of the colour `rgb`, its components 0 to 1, with `more` fields.
fn stop(offset: f64, rgb: [f64; 3], more: &Value) -> Value {
    let mut stop = json!({"offset": offset, "color": {"model": "rgb", "r": rgb[0], "g": rgb[1], "b": rgb[2]}});
    if let (Some(stop), Value::Object(more)) = (stop.as_object_mut(), more.clone()) {
        stop.extend(more);
    }
    stop
}

/// Red to blue.
fn red_blue() -> Vec<Value> {
    vec![stop(0.0, [1.0, 0.0, 0.0], &json!({})), stop(1.0, [0.0, 0.0, 1.0], &json!({}))]
}

/// A gradient paint of `kind` through `stops`, with `more` fields: its `geom` or its `angle`.
fn paint(kind: &str, stops: &[Value], more: &Value) -> Value {
    let mut paint = json!({"type": "gradient", "gradient": {"kind": kind, "stops": stops}});
    if let (Some(paint), Value::Object(more)) = (paint.as_object_mut(), more.clone()) {
        paint.extend(more);
    }
    paint
}

/// The square `id` at (`x`, 0) pt, filled with `paint`, with StitchCraft's fill effect sewing it by `method`.
fn filled(id: u32, x: f64, paint: &Value, method: &str) -> String {
    let appearance =
        json!({"items": [{"kind": "fill", "paint": paint}], "effects": [{"id": "plugin.dev.stitchcraft.fill", "params": {"fill_method": method}}]});
    square(id, x, 0.0, &appearance.to_string())
}

/// The gradient and thread colour of the fill of the square `id`.
fn sewn(read: &VectorCraft, id: u32) -> (Option<Gradient>, Rgb) {
    let element = element(read, &format!("vc:{id}:fill"));
    let Shape::Fill { gradient, .. } = &element.shape else { panic!("not a fill: {element:?}") };
    (gradient.clone(), element.thread.color)
}

/// Each stop's offset, colour and opacity.
fn stops(gradient: &Gradient) -> Vec<(f64, Rgb, f64)> {
    gradient.stops.iter().map(|stop| (stop.offset, stop.colour, stop.opacity)).collect()
}

/// `point` is (`x`, `y`) pt, to within a micrometre.
#[track_caller]
fn at(point: Point, x: f64, y: f64) {
    assert!((point.x() - x * PT).abs() < 1e-3 && (point.y() - y * PT).abs() < 1e-3, "{point:?} is not ({x}, {y}) pt");
}

#[test]
fn req_vc_022_a_linear_gradient_fill_sews_its_object_s_gradient_where_vectorcraft_draws_it() {
    // Placed by its `geom`, in points; without one, fitted to the object's bounds at its angle, through their
    // middle: at 90°, from the square's bottom to its top.
    let placed = paint("Linear", &red_blue(), &json!({"geom": {"start": {"x": 0, "y": 6}, "end": [36, 6]}, "angle": 0}));
    let fitted = paint("Linear", &red_blue(), &json!({"angle": 90}));
    let read = read_ok(&document(4, &[filled(2, 0.0, &placed, "linear_gradient_fill"), filled(3, 50.0, &fitted, "linear_gradient_fill")].join(",")));
    assert!(read.warnings.is_empty(), "{:?}", read.warnings);
    let (gradient, thread) = sewn(&read, 2);
    let gradient = gradient.unwrap();
    at(gradient.start, 0.0, 6.0);
    at(gradient.end, 36.0, 6.0);
    assert_eq!((stops(&gradient), thread), (vec![(0.0, RED, 1.0), (1.0, BLUE, 1.0)], RED));
    let gradient = sewn(&read, 3).0.unwrap();
    at(gradient.start, 68.0, 36.0);
    at(gradient.end, 68.0, 0.0);
    // At 45° through a square, from corner to corner, as long as the square is across that way.
    let slanted = paint("Linear", &red_blue(), &json!({"angle": 45}));
    let gradient = sewn(&read_ok(&document(4, &filled(2, 0.0, &slanted, "linear_gradient_fill"))), 2).0.unwrap();
    at(gradient.start, 0.0, 36.0);
    at(gradient.end, 36.0, 0.0);
}

#[test]
fn req_vc_022_each_stop_is_kept_in_range_and_what_rows_can_t_follow_is_said() {
    // Offsets kept from 0 to 1 and never before the stop before, an opacity left out 1, a colour that
    // doesn't read black, and a midpoint off half way, each said.
    let written = [
        stop(-0.5, [1.0, 0.0, 0.0], &json!({"opacity": 0.5, "midpoint": 0.3})),
        json!({"offset": 0.8, "color": {"model": "spot"}}),
        stop(0.4, [0.0, 0.0, 1.0], &json!({"opacity": 2})),
    ];
    let read = read_ok(&document(4, &filled(2, 0.0, &paint("Linear", &written, &json!({})), "linear_gradient_fill")));
    let gradient = sewn(&read, 2).0.unwrap();
    assert_eq!(stops(&gradient), [(0.0, RED, 0.5), (0.8, Rgb::new(0, 0, 0), 1.0), (0.8, BLUE, 1.0)]);
    assert_eq!(said(&read), [(Code::VcFeatureIgnored, "vc:2".to_string()), (Code::VcFeatureIgnored, "vc:2".to_string())]);
    assert!(read.warnings[0].message.contains("sewn in black") && read.warnings[1].message.contains("midpoints"), "{:?}", read.warnings);
}

#[test]
fn req_vc_022_only_a_linear_gradient_fill_sews_a_linear_gradient() {
    // A tatami fill, a radial gradient, and a gradient of one stop: each sewn in its first colour, the 2
    // gradients of 2 colours said to be.
    let linear = paint("Linear", &red_blue(), &json!({}));
    let radial = paint("Radial", &red_blue(), &json!({}));
    let one = paint("Linear", &red_blue()[..1], &json!({}));
    let objects =
        [filled(2, 0.0, &linear, "tatami_fill"), filled(3, 50.0, &radial, "linear_gradient_fill"), filled(4, 100.0, &one, "linear_gradient_fill")];
    let read = read_ok(&document(4, &objects.join(",")));
    for id in [2, 3, 4] {
        assert_eq!(sewn(&read, id), (None, RED));
    }
    assert_eq!(said(&read), [(Code::VcFeatureIgnored, "vc:2".to_string()), (Code::VcFeatureIgnored, "vc:3".to_string())]);
    assert!(read.warnings.iter().all(|w| w.message.contains("A gradient is sewn in its first colour.")), "{:?}", read.warnings);
}

#[test]
fn req_vc_022_a_gradient_placed_too_far_away_is_said_and_sewn_without() {
    // More than 10 m from the origin: the fill is sewn in the gradient's first colour, and both are said. A
    // coordinate that isn't a number places no gradient either.
    let far = paint("Linear", &red_blue(), &json!({"geom": {"start": [0, 0], "end": [40000, 0]}}));
    let unplaced = paint("Linear", &red_blue(), &json!({"geom": {"start": [null, 0], "end": [36, 0]}}));
    let read = read_ok(&document(4, &[filled(2, 0.0, &far, "linear_gradient_fill"), filled(3, 50.0, &unplaced, "linear_gradient_fill")].join(",")));
    assert_eq!((sewn(&read, 2), sewn(&read, 3)), ((None, RED), (None, RED)));
    let messages: Vec<&str> = read.warnings.iter().map(|w| w.message.as_str()).collect();
    assert_eq!(
        messages,
        [
            "Its gradient reaches more than 10 m from the origin, and the fill is sewn without it.",
            "A gradient is sewn in its first colour.",
            "A gradient is sewn in its first colour."
        ]
    );
}

#[test]
fn req_vc_022_the_fill_effect_sews_the_topmost_fill_s_linear_gradient() {
    // The live effect is given the object's fills, bottom first, and sews the topmost that paints: the
    // gradient on top of a blue fill, the top fill painting nothing.
    let square = json!({"subpaths": [{"closed": true, "anchors": [{"p": [0, 0]}, {"p": [36, 0]}, {"p": [36, 36]}, {"p": [0, 36]}]}]});
    let blue = json!({"type": "solid", "color": {"model": "rgb", "r": 0, "g": 0, "b": 1}});
    let fills = json!([blue, paint("Linear", &red_blue(), &json!({"angle": 0})), null]);
    let input =
        json!({"objects": [{"type": "path", "path": square, "fillRule": "nonzero", "bounds": [0, 0, 36, 36], "fills": fills, "strokes": []}]});
    let input = serde_json::to_vec(&input).unwrap();
    let effect = |method: &str| {
        let params = json!({"fill_method": method, "_context": {"mode": "effect", "bounds": [0, 0, 36, 36], "item": "object"}});
        let read = read_effect(Family::Fill, &input, &serde_json::to_vec(&params).unwrap(), &Budget::DEFAULT).unwrap();
        let Shape::Fill { gradient, .. } = &read.element.shape else { panic!("not a fill") };
        gradient.clone()
    };
    let gradient = effect("linear_gradient_fill").unwrap();
    at(gradient.start, 0.0, 18.0);
    at(gradient.end, 36.0, 18.0);
    assert_eq!(stops(&gradient), [(0.0, RED, 1.0), (1.0, BLUE, 1.0)]);
    assert_eq!(effect("tatami_fill"), None);
}
