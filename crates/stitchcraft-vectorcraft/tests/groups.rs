//! What a group's or a layer's StitchCraft effects and paints sew (`REQ-VC-006`), as VectorCraft draws them.
//!
//! VectorCraft runs a group's geometry effects on each path inside it, then paints what they draw with the
//! path's own paints and with the paints of the group and of the groups inside it, a group's paints over
//! what it holds unless its Contents row is moved up (`evaluate_container` in its `crates/effects`).

// Test code may unwrap, panic and index (clippy.toml allows it in tests, but not in their helpers).
#![allow(clippy::unwrap_used, clippy::panic, clippy::indexing_slicing)]

mod common;

use common::{RED, document, element, ids, line, params, read_ok, said, square};
use stitchcraft_core::{Code, Fix};
use stitchcraft_engine::design::{FillRule, Shape};
use stitchcraft_plan::Rgb;
use stitchcraft_vectorcraft::VectorCraft;

/// The primary colours, a channel each, as VectorCraft writes them from 0 to 1.
const R: [u8; 3] = [1, 0, 0];
const G: [u8; 3] = [0, 1, 0];
const B: [u8; 3] = [0, 0, 1];

/// 3 mm and 2 mm, in points.
const PT_3MM: f64 = 8.503_937;
const PT_2MM: f64 = 5.669_291;

/// The thread of colour `rgb`.
fn rgb([r, g, b]: [u8; 3]) -> Rgb {
    Rgb::new(r * 255, g * 255, b * 255)
}

/// A fill in `rgb`, as an appearance item.
fn fill([r, g, b]: [u8; 3]) -> String {
    format!(r#"{{"kind": "fill", "paint": {{"type": "solid", "color": {{"model": "rgb", "r": {r}, "g": {g}, "b": {b}}}}}}}"#)
}

/// A stroke `width` points wide in `rgb`, as an appearance item.
fn stroke(width: f64, [r, g, b]: [u8; 3]) -> String {
    format!(r#"{{"kind": "stroke", "width": {width}, "paint": {{"type": "solid", "color": {{"model": "rgb", "r": {r}, "g": {g}, "b": {b}}}}}}}"#)
}

/// A stroke of paint none, `width` points wide, which VectorCraft writes for an object without a stroke.
fn no_stroke(width: f64) -> String {
    format!(r#"{{"kind": "stroke", "width": {width}, "paint": {{"type": "none"}}}}"#)
}

/// StitchCraft's effect of `family`, with `params`.
fn effect(family: &str, params: &str) -> String {
    format!(r#"{{"id": "plugin.dev.stitchcraft.{family}", "params": {params}}}"#)
}

/// An appearance of `items` and `effects`.
fn looks(items: &[String], effects: &[String]) -> String {
    format!(r#"{{"items": [{}], "effects": [{}]}}"#, items.join(","), effects.join(","))
}

/// A group or a layer's appearance of `items` and `effects`, with its Contents row over the first `k` items,
/// which VectorCraft then paints under what it holds.
fn looks_over(items: &[String], effects: &[String], k: usize) -> String {
    format!(r#"{{"items": [{}], "effects": [{}], "contents_index": {k}}}"#, items.join(","), effects.join(","))
}

/// A group with `id`, `appearance` and `extra` fields in its kind (`"clip": true,`), holding `children`.
fn group(id: u32, appearance: &str, extra: &str, children: &[String]) -> String {
    format!(r#"{{"id": {id}, "appearance": {appearance}, "kind": {{"type": "group", {extra} "children": [{}]}}}}"#, children.join(","))
}

/// The thread element `id` sews in, and its stroke's width in millimetres to a thousandth (0 for a fill).
fn sews(read: &VectorCraft, id: &str) -> (Rgb, f64) {
    let element = element(read, id);
    let width = match element.shape {
        Shape::Stroke { width, .. } => (width.get() * 1000.0).round() / 1000.0,
        _ => 0.0,
    };
    (element.thread.color, width)
}

/// A tatami fill at 45° with rows 0.3 mm apart, as a StitchCraft Fill effect records it, red.
const TATAMI_45: &str = r#"{"items": [{"kind": "fill", "paint": {"type": "solid", "color": {"model": "rgb", "r": 1, "g": 0, "b": 0}}}],
    "effects": [{"id": "plugin.dev.stitchcraft.fill", "params": {"angle": 45, "row_spacing_mm": 0.3, "preview_level": "rows"}}]}"#;

#[test]
fn req_vc_006_a_graphic_style_s_effects_sew_each_object_it_is_applied_to() {
    // VectorCraft copies a style's appearance into each object it applies the style to, and links the
    // object to the style by its id.
    let styled = |id: u32, x: f64| square(id, x, 0.0, TATAMI_45).replacen(r#""appearance""#, r#""graphic_style": 1, "appearance""#, 1);
    let file = format!(
        r#"{{"format": "vectorcraft", "version": 4, "document": {{"version": 1, "artboards": [], "next_id": 99,
            "graphic_styles": [{{"name": "Tatami 45", "id": 1, "appearance": {TATAMI_45}}}],
            "layers": [{{"id": 1, "kind": {{"type": "layer", "children": [{}, {}]}}}}]}}}}"#,
        styled(2, 0.0),
        styled(3, 50.0)
    );
    let read = read_ok(file.as_bytes());
    assert_eq!(ids(&read), ["vc:2:fill", "vc:3:fill"]);
    for element in read.design.elements() {
        assert_eq!(params(element), [("angle", "45"), ("row_spacing_mm", "0.3")]);
        assert_eq!(element.thread.color, Rgb::new(255, 0, 0));
    }
    assert_eq!(said(&read), []);
}

#[test]
fn req_vc_006_a_group_s_effects_sew_each_path_inside_it() {
    // A style applied to a group gives the group the effect, and VectorCraft runs it on each path and
    // compound path inside, however deep, each in its own paint.
    let blue = r#"{"items": [{"kind": "fill", "paint": {"type": "solid", "color": {"model": "rgb", "r": 0, "g": 0, "b": 1}}}]}"#;
    let ring = r#"{"id": 4, "appearance": {"items": [{"kind": "fill", "paint": {"type": "solid", "color": {"model": "gray", "k": 1}}}]},
        "kind": {"type": "compound", "rule": "EvenOdd", "children": [
            {"id": 41, "kind": {"type": "path", "path": {"subpaths": [{"closed": true, "anchors": [{"p": [100, 0]}, {"p": [136, 0]}, {"p": [136, 36]}, {"p": [100, 36]}]}]}}},
            {"id": 42, "kind": {"type": "path", "path": {"subpaths": [{"closed": true, "anchors": [{"p": [110, 10]}, {"p": [126, 10]}, {"p": [126, 26]}, {"p": [110, 26]}]}]}}}]}}"#;
    let inner = format!(r#"{{"id": 5, "kind": {{"type": "group", "children": [{}]}}}}"#, square(6, 150.0, 0.0, blue));
    let group = format!(
        r#"{{"id": 2, "graphic_style": 1, "appearance": {{"effects": [{{"id": "plugin.dev.stitchcraft.fill", "params": {{"angle": 45}}}}]}},
            "kind": {{"type": "group", "children": [{}, {ring}, {inner}]}}}}"#,
        square(3, 0.0, 0.0, RED)
    );
    let read = read_ok(&document(4, &group));
    assert_eq!(ids(&read), ["vc:3:fill", "vc:4:fill", "vc:6:fill"]);
    assert!(read.design.elements().iter().all(|e| params(e) == [("angle", "45")]), "{:?}", read.design.elements());
    assert!(matches!(element(&read, "vc:4:fill").shape, Shape::Fill { rule: FillRule::EvenOdd, .. }));
    let colours: Vec<Rgb> = read.design.elements().iter().map(|e| e.thread.color).collect();
    assert_eq!(colours, [Rgb::new(255, 0, 0), Rgb::new(0, 0, 0), Rgb::new(0, 0, 255)]);
    assert_eq!(said(&read), []);
    // A layer's effects reach what it holds too, and a satin column takes its width from its own stroke.
    let stroked = r#"{"items": [{"kind": "stroke", "width": 8.503937, "paint": {"type": "solid", "color": {"model": "gray", "k": 1}}}]}"#;
    let line = r#"{"subpaths": [{"anchors": [{"p": [0, 0]}, {"p": [100, 0]}]}]}"#;
    let file = format!(
        r#"{{"format": "vectorcraft", "version": 4, "document": {{"layers": [{{"id": 1,
            "appearance": {{"effects": [{{"id": "plugin.dev.stitchcraft.satin", "params": {{"zigzag_spacing_mm": 0.5}}}}]}},
            "kind": {{"type": "layer", "children": [{{"id": 7, "appearance": {stroked}, "kind": {{"type": "path", "path": {line}}}}}]}}}}]}}}}"#
    );
    let read = read_ok(file.as_bytes());
    let satin = element(&read, "vc:7:satin");
    let Shape::Stroke { width, .. } = satin.shape else { panic!("{:?}", satin.shape) };
    assert!((width.get() - 3.0).abs() < 1e-6);
    assert_eq!(params(satin), [("satin_column", "true"), ("zigzag_spacing_mm", "0.5")]);
}

#[test]
fn req_vc_006_a_group_s_paints_over_what_it_holds_give_its_effects_their_thread_and_stroke() {
    // A graphic style applied to a group puts its paints there too, and VectorCraft paints them over what
    // the group holds: every object inside shows in the style's colour and stroke, whatever its own.
    let satin = group(
        2,
        &looks(&[stroke(PT_3MM, R)], &[effect("satin", "{}")]),
        "",
        &[line(3, "", &looks(&[stroke(1.0, B)], &[])), line(4, "", &looks(&[fill(G), no_stroke(2.2)], &[]))],
    );
    let fill = group(5, &looks(&[fill(R)], &[effect("fill", "{}")]), "", &[square(6, 100.0, 0.0, &looks(&[fill(B)], &[]))]);
    let read = read_ok(&document(4, &[satin, fill].join(",")));
    assert_eq!(ids(&read), ["vc:3:satin", "vc:4:satin", "vc:6:fill"]);
    assert_eq!(sews(&read, "vc:3:satin"), (rgb(R), 3.0));
    assert_eq!(sews(&read, "vc:4:satin"), (rgb(R), 3.0));
    assert_eq!(sews(&read, "vc:6:fill"), (rgb(R), 0.0));
}

#[test]
fn req_vc_006_a_group_s_paints_under_what_it_holds_come_after_the_object_s_own() {
    // With the Contents row over a group's paints, VectorCraft paints them under what it holds, and the
    // object's own paints show over them.
    let under = group(
        2,
        &looks_over(&[stroke(PT_3MM, R)], &[effect("satin", "{}")], 1),
        "",
        &[line(3, "", &looks(&[stroke(1.0, B)], &[])), line(4, "", "{}")],
    );
    // Red under the Contents row and green over it, over the line's own blue.
    let both = group(
        5,
        &looks_over(&[stroke(PT_3MM, R), stroke(PT_2MM, G)], &[effect("satin", "{}")], 1),
        "",
        &[line(6, "", &looks(&[stroke(1.0, B)], &[]))],
    );
    let read = read_ok(&document(4, &[under, both].join(",")));
    assert_eq!(sews(&read, "vc:3:satin"), (rgb(B), 0.353));
    assert_eq!(sews(&read, "vc:4:satin"), (rgb(R), 3.0));
    assert_eq!(sews(&read, "vc:6:satin"), (rgb(G), 2.0));
}

#[test]
fn req_vc_006_the_paints_over_the_stitches_give_the_thread_the_outermost_first() {
    // VectorCraft paints the stitches of a group's effect with the paints of that group and of the groups
    // inside it, each group's over what it holds. A group around it paints the object's shape.
    let own = looks(&[fill(G)], &[]);
    let blue = |id: u32, children: &[String]| group(id, &looks(&[fill(B)], &[]), "", children);
    let fill_effect = |id: u32, items: &[String], children: &[String]| group(id, &looks(items, &[effect("fill", "{}")]), "", children);
    let objects = [
        // Group 2's red over group 3's blue, over the square's own green.
        fill_effect(2, &[fill(R)], &[blue(3, &[square(4, 0.0, 0.0, &own)])]),
        // Group 5 doesn't paint, and group 6's blue is on top.
        fill_effect(5, &[], &[blue(6, &[square(7, 50.0, 0.0, &own)])]),
        // Group 8 paints the squares' shapes, and square 10's stitches show in its own green. Square 11
        // has no paint of its own, and shows in group 8's blue.
        blue(8, &[fill_effect(9, &[], &[square(10, 100.0, 0.0, &own), square(11, 150.0, 0.0, "{}")])]),
    ];
    let read = read_ok(&document(4, &objects.join(",")));
    assert_eq!(ids(&read), ["vc:4:fill", "vc:7:fill", "vc:10:fill", "vc:11:fill"]);
    let colours: Vec<Rgb> = read.design.elements().iter().map(|e| e.thread.color).collect();
    assert_eq!(colours, [rgb(R), rgb(B), rgb(G), rgb(B)]);
}

#[test]
fn req_vc_006_an_object_s_own_effects_take_its_own_paints_first() {
    // VectorCraft paints an object's own effects with its own paints alone, and a group's paints over the
    // object's shape. What the object doesn't paint, a stroke here, comes from the group.
    let objects = [group(
        2,
        &looks(&[stroke(PT_3MM, G)], &[]),
        "",
        &[
            line(3, "", &looks(&[], &[effect("satin", "{}")])),
            line(4, "", &looks(&[stroke(1.0, B)], &[effect("satin", "{}")])),
            line(5, "", &looks(&[fill(B), no_stroke(2.2)], &[effect("satin", "{}")])),
        ],
    )];
    let read = read_ok(&document(4, &objects.join(",")));
    assert_eq!(sews(&read, "vc:3:satin"), (rgb(G), 3.0));
    assert_eq!(sews(&read, "vc:4:satin"), (rgb(B), 0.353));
    assert_eq!(sews(&read, "vc:5:satin"), (rgb(G), 3.0));
    // The group's paints are named, as no element of their own, and nothing else is said.
    assert_eq!(said(&read), [(Code::VcFeatureIgnored, "vc:2".to_string())]);
    assert!(read.warnings[0].message.contains("give their thread and stroke"), "{}", read.warnings[0]);
}

#[test]
fn req_vc_006_a_group_s_effects_leave_out_what_nothing_paints() {
    // VectorCraft draws a group's effect on an object with paints, the object's or a group's, and draws
    // nothing where none paints it: nothing is sewn there, and nothing is said.
    let unpainted = looks(&[r#"{"kind": "fill", "paint": {"type": "none"}}"#.to_string(), no_stroke(1.0)], &[]);
    // A gradient without stops paints nothing either.
    let stopless = looks(&[r#"{"kind": "fill", "paint": {"type": "gradient", "gradient": {"kind": "Linear", "stops": []}}}"#.to_string()], &[]);
    let squares = [square(3, 0.0, 0.0, "{}"), square(4, 50.0, 0.0, &unpainted), square(5, 100.0, 0.0, RED), square(6, 150.0, 0.0, &stopless)];
    let objects = [group(2, &looks(&[], &[effect("fill", "{}")]), "", &squares)];
    let read = read_ok(&document(4, &objects.join(",")));
    assert_eq!(ids(&read), ["vc:5:fill"]);
    assert_eq!(said(&read), []);
}

#[test]
fn req_vc_006_a_clipping_object_takes_the_group_s_effects_and_only_its_own_paints() {
    // A clip group's first object is its clipping object, whatever its kind. VectorCraft runs the group's
    // effects on it too, and paints what they draw with its own paints alone, none after Make Clipping
    // Mask: its fills under the clipped art and its strokes over it.
    let style = looks(&[stroke(1.0, G)], &[effect("fill", r#"{"angle": 30}"#)]);
    let clip = |id: u32, clipping: String, member: String| group(id, &style, r#""clip": true,"#, &[clipping, member]);
    let compound = format!(
        r#"{{"id": 6, "appearance": {}, "kind": {{"type": "compound", "children": [
            {{"id": 61, "kind": {{"type": "path", "path": {{"subpaths": [{{"closed": true, "anchors": [{{"p": [50, 0]}}, {{"p": [86, 0]}}, {{"p": [86, 36]}}]}}]}}}}}}]}}}}"#,
        looks(&[r#"{"kind": "fill", "paint": {"type": "none"}}"#.to_string(), no_stroke(0.0)], &[])
    );
    let objects = [
        // Square 3 outlines the clip in blue, and sews by the group's effect in its own blue.
        clip(2, square(3, 0.0, 0.0, &looks(&[stroke(1.0, B)], &[])), square(4, 0.0, 0.0, "{}")),
        // Compound 6 paints nothing, as Make Clipping Mask leaves it, and sews nothing.
        clip(5, compound, square(7, 50.0, 0.0, "{}")),
    ];
    let read = read_ok(&document(4, &objects.join(",")));
    assert_eq!(ids(&read), ["vc:3:fill", "vc:4:fill", "vc:7:fill"]);
    assert!(read.design.elements().iter().all(|e| params(e) == [("angle", "30")]), "{:?}", read.design.elements());
    let colours: Vec<Rgb> = read.design.elements().iter().map(|e| e.thread.color).collect();
    assert_eq!(colours, [rgb(B), rgb(G), rgb(G)]);
    // What is said is said of the groups: their clipping, and their paints.
    assert!(read.warnings.iter().all(|w| w.element.as_ref().is_some_and(|e| ["vc:2", "vc:5"].contains(&e.as_str()))), "{:?}", read.warnings);
}

#[test]
fn req_vc_006_a_shaper_group_that_clips_sews_its_clipping_object() {
    // Make Clipping Mask on a Shaper group moves its top object under the transparent sources, as the
    // clipping object (`layer.clippingMask.toggle`).
    let sources = format!(r#"{{"id": 4, "opacity": 0, "kind": {{"type": "group", "children": [{}]}}}}"#, square(5, 0.0, 0.0, RED));
    let shaper = format!(
        r#"{{"id": 2, "shaper": {{"edges": []}}, "kind": {{"type": "group", "clip": true, "children": [{}, {sources}, {}]}}}}"#,
        square(3, 0.0, 0.0, RED),
        square(6, 0.0, 0.0, &looks(&[fill(B)], &[]))
    );
    let read = read_ok(&document(4, &shaper));
    assert_eq!(ids(&read), ["vc:3:fill", "vc:6:fill"]);
    assert_eq!(said(&read), [(Code::VcFeatureIgnored, "vc:2".to_string())]);
}

#[test]
fn diag_sc_w0807_a_path_stitchcraft_effects_reach_from_2_levels_sews_by_the_nearest() {
    // VectorCraft runs a group's effect on each path inside, then the path's own on what that drew: an
    // effect on another's stitches, which no machine sews. The nearest effects say how the path sews.
    let own = r#"{"items": [{"kind": "stroke", "width": 1, "paint": {"type": "solid", "color": {"model": "gray", "k": 0}}}],
        "effects": [{"id": "plugin.dev.stitchcraft.stroke", "params": {"running_stitch_length_mm": 2}}]}"#;
    let fill = |id: u32, children: &str| {
        format!(
            r#"{{"id": {id}, "appearance": {{"effects": [{{"id": "plugin.dev.stitchcraft.fill"}}]}}, "kind": {{"type": "group", "children": [{children}]}}}}"#
        )
    };
    let stroke_group = format!(
        r#"{{"id": 5, "appearance": {{"effects": [{{"id": "plugin.dev.stitchcraft.stroke"}}]}}, "kind": {{"type": "group", "children": [{}]}}}}"#,
        square(6, 100.0, 0.0, RED)
    );
    let objects = [fill(2, &square(3, 0.0, 0.0, own)), fill(4, &stroke_group)];
    let read = read_ok(&document(4, &objects.join(",")));
    assert_eq!(ids(&read), ["vc:3:stroke", "vc:6:stroke"]);
    assert_eq!(element(&read, "vc:3:stroke").params.get("running_stitch_length_mm"), Some("2"));
    let expected = [(Code::VcFeatureIgnored, "vc:3".to_string()), (Code::VcFeatureIgnored, "vc:6".to_string())];
    assert_eq!(said(&read), expected);
    assert_eq!(
        read.warnings[0].to_string(),
        "warning SC-W0807: The object gets StitchCraft effects from more than one level, its own or those of the groups and layers \
         around it, and VectorCraft runs each on the stitches of the one before: it is sewn as the nearest say."
    );
    assert_eq!(read.warnings[0].fix, Some(Fix::Hint("Keep StitchCraft's effects on the object or on one group around it.".to_string())));
}

#[test]
fn diag_sc_w0807_what_is_said_of_a_paint_is_said_once() {
    // A fill effect and a satin effect can take their thread from the same paint, a group's gradient
    // here: what is said of it is said once.
    let gradient = r#"{"kind": "fill", "paint": {"type": "gradient", "gradient": {"kind": "Linear", "stops": [
        {"color": {"model": "rgb", "r": 1, "g": 0, "b": 0}}, {"color": {"model": "rgb", "r": 0, "g": 0, "b": 1}}]}}}"#;
    let both = [effect("fill", "{}"), effect("satin", "{}")];
    let objects = [
        group(2, &looks(&[gradient.to_string()], &both), "", &[line(3, "", "{}")]),
        // Without paint, and in no group that paints, an object is sewn in black, and that is said once.
        square(4, 100.0, 0.0, &looks(&[], &both)),
    ];
    let read = read_ok(&document(4, &objects.join(",")));
    assert_eq!(ids(&read), ["vc:3:fill", "vc:3:satin", "vc:4:fill", "vc:4:satin"]);
    let colours: Vec<Rgb> = read.design.elements().iter().map(|e| e.thread.color).collect();
    assert_eq!(colours, [rgb(R), rgb(R), Rgb::new(0, 0, 0), Rgb::new(0, 0, 0)]);
    let w = Code::VcFeatureIgnored;
    assert_eq!(said(&read), [(w, "vc:2".to_string()), (w, "vc:2".to_string()), (w, "vc:4".to_string())], "{:#?}", read.warnings);
    assert!(read.warnings[1].message.contains("gradient") && read.warnings[2].message.contains("black"), "{:#?}", read.warnings);
}
