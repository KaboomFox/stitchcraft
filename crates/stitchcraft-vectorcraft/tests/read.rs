//! Reading `.vectorcraft` files (`REQ-VC-001`, `REQ-VC-004` to `REQ-VC-007`) and the adapter's diagnostics.

// Test code may unwrap, panic and index (clippy.toml allows it in tests, but not in their helpers).
#![allow(clippy::unwrap_used, clippy::panic, clippy::indexing_slicing)]

use std::io::Write;

use flate2::Compression;
use flate2::write::GzEncoder;
use stitchcraft_core::units::MM_PER_POINT;
use stitchcraft_core::{Budget, Code, Diagnostic, Fix, Mm, Point};
use stitchcraft_engine::design::{DesignSettings, Element, FillRule, Join, Segment, Shape};
use stitchcraft_engine::generate::generate;
use stitchcraft_engine::generators::Neighbours;
use stitchcraft_plan::Rgb;
use stitchcraft_plan::profiles::REFERENCE;
use stitchcraft_vectorcraft::{VectorCraft, read};

const FORMAT_1: &[u8] = include_bytes!("../../../conformance/fixtures/vectorcraft/drawing-format1.vectorcraft");
const FORMAT_4: &[u8] = include_bytes!("../../../conformance/fixtures/vectorcraft/drawing-format4.vectorcraft");
const FEATURES: &[u8] = include_bytes!("../../../conformance/fixtures/vectorcraft/features.vectorcraft");
const EFFECTS: &[u8] = include_bytes!("../../../conformance/fixtures/vectorcraft/effects.vectorcraft");
const RIBBONS_1: &[u8] = include_bytes!("../../../conformance/fixtures/vectorcraft/ribbons-format1.vectorcraft");
const RIBBONS_4: &[u8] = include_bytes!("../../../conformance/fixtures/vectorcraft/ribbons-format4.vectorcraft");

/// Millimetres in a point.
const MM: f64 = 25.4 / 72.0;

fn gzip(bytes: &[u8]) -> Vec<u8> {
    let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
    encoder.write_all(bytes).unwrap();
    encoder.finish().unwrap()
}

fn read_ok(bytes: &[u8]) -> VectorCraft {
    read(bytes, &Budget::DEFAULT).unwrap()
}

fn refusal(bytes: &[u8]) -> Diagnostic {
    read(bytes, &Budget::DEFAULT).unwrap_err()
}

fn ids(read: &VectorCraft) -> Vec<&str> {
    read.design.elements().iter().map(|e| e.id.as_str()).collect()
}

fn element<'a>(read: &'a VectorCraft, id: &str) -> &'a Element {
    read.design.elements().iter().find(|e| e.id.as_str() == id).unwrap_or_else(|| panic!("no {id} in {:?}", ids(read)))
}

/// Each warning as its code and the object it is about.
fn said(read: &VectorCraft) -> Vec<(Code, String)> {
    read.warnings.iter().map(|w| (w.code, w.element.as_ref().map_or(String::new(), |e| e.to_string()))).collect()
}

/// A document of one layer holding `objects`, in format `version`.
fn document(version: u32, objects: &str) -> Vec<u8> {
    format!(
        r#"{{"format": "vectorcraft", "version": {version}, "document": {{"version": 1, "artboards": [], "next_id": 99,
            "layers": [{{"id": 1, "kind": {{"type": "layer", "color": {{"Preset": 0}}, "children": [{objects}]}}}}]}}}}"#
    )
    .into_bytes()
}

/// A square path of side 36 pt at (`x`, `y`) pt, with `id` and `appearance`.
fn square(id: u32, x: f64, y: f64, appearance: &str) -> String {
    let (r, b) = (x + 36.0, y + 36.0);
    format!(
        r#"{{"id": {id}, "appearance": {appearance}, "kind": {{"type": "path", "path": {{"subpaths": [{{"closed": true,
            "anchors": [{{"p": [{x}, {y}]}}, {{"p": [{r}, {y}]}}, {{"p": [{r}, {b}]}}, {{"p": [{x}, {b}]}}]}}]}}}}}}"#
    )
}

const RED: &str = r#"{"items": [{"kind": "fill", "paint": {"type": "solid", "color": {"model": "rgb", "r": 1, "g": 0, "b": 0}}}]}"#;

fn p(x: f64, y: f64) -> Point {
    Point::new(x * MM, y * MM).unwrap()
}

fn close(a: Point, b: Point) -> bool {
    (a.x() - b.x()).abs() < 1e-9 && (a.y() - b.y()).abs() < 1e-9
}

#[test]
fn req_vc_001_formats_1_and_4_plain_and_compressed_read_the_same_drawing() {
    let first = read_ok(FORMAT_1);
    for bytes in [FORMAT_4.to_vec(), gzip(FORMAT_4), gzip(FORMAT_1)] {
        assert_eq!(read_ok(&bytes), first);
    }
    assert_eq!(ids(&first), ["vc:2:fill", "vc:2:stroke", "vc:3:fill", "vc:6:stroke"]);
    // The template layer is left out, and said.
    assert_eq!(said(&first), [(Code::VcNotPrinted, "vc:7".to_string())]);
}

#[test]
fn req_vc_001_points_are_millimetres_and_paints_are_threads() {
    let drawing = read_ok(FORMAT_4);
    let square = element(&drawing, "vc:2:fill");
    assert_eq!(square.name.as_deref(), Some("Square"));
    assert_eq!((square.thread.color, square.thread.name.as_deref()), (Rgb::new(255, 0, 0), Some("Red")));
    let Shape::Fill { path, rule: FillRule::NonZero } = &square.shape else { panic!("{:?}", square.shape) };
    let sub = &path.subpaths[0];
    assert!(sub.closed && close(sub.start, p(72.0, 72.0)));
    let corners = [p(144.0, 72.0), p(144.0, 144.0), p(72.0, 144.0), p(72.0, 72.0)];
    assert_eq!(sub.segments.len(), 4);
    for (segment, corner) in sub.segments.iter().zip(corners) {
        assert!(matches!(segment, Segment::Line(end) if close(*end, corner)), "{segment:?}");
    }
    let outline = element(&drawing, "vc:2:stroke");
    let Shape::Stroke { width, join: Join::Round, .. } = outline.shape else { panic!("{:?}", outline.shape) };
    assert!((width.get() - 1.0).abs() < 1e-6, "{width:?}");
    assert_eq!(outline.thread.color, Rgb::new(0, 0, 0));
    // A compound path is one shape of all its paths' subpaths, with its own rule and paint (CMYK cyan).
    let ring = element(&drawing, "vc:3:fill");
    let Shape::Fill { path, rule: FillRule::EvenOdd } = &ring.shape else { panic!("{:?}", ring.shape) };
    assert_eq!(path.subpaths.len(), 2);
    assert!(path.subpaths.iter().all(|s| s.closed && s.segments.len() == 4 && s.segments.iter().all(|g| matches!(g, Segment::Cubic(..)))));
    assert!(close(path.subpaths[0].start, p(324.0, 108.0)) && close(path.subpaths[1].start, p(306.0, 108.0)));
    assert_eq!(ring.thread.color, Rgb::new(0, 255, 255));
    // Handles at absolute positions; a missing handle leaves its end of the segment on the anchor.
    let wave = element(&drawing, "vc:6:stroke");
    let Shape::Stroke { path, width, join: Join::Miter { limit } } = &wave.shape else { panic!("{:?}", wave.shape) };
    assert!((width.get() - 0.5).abs() < 1e-6 && *limit == 4.0);
    let sub = &path.subpaths[0];
    assert!(!sub.closed && close(sub.start, p(72.0, 216.0)) && sub.segments.len() == 2);
    let Segment::Cubic(a, b, end) = sub.segments[0] else { panic!("{:?}", sub.segments[0]) };
    assert!(close(a, p(126.0, 162.0)) && close(b, p(162.0, 270.0)) && close(end, p(180.0, 216.0)));
    let Segment::Cubic(a, b, end) = sub.segments[1] else { panic!("{:?}", sub.segments[1]) };
    assert!(close(a, p(198.0, 162.0)) && close(b, p(234.0, 270.0)) && close(end, p(288.0, 216.0)));
    assert_eq!(wave.thread.color, Rgb::new(0, 0, 255));
}

#[test]
fn req_vc_001_one_handle_bends_a_segment_and_one_at_vectorcraft_s_tolerance_is_none() {
    // An out handle along x, then an in handle along y, each alone on its segment. Then an in handle
    // exactly VectorCraft's tolerance, a billionth of a point, from its anchor, and one a millionth away.
    let object = r#"{"id": 2, "appearance": {"items": [{"kind": "stroke", "width": 1, "paint": {"type": "solid", "color": {"model": "gray", "k": 1}}}]},
        "kind": {"type": "path", "path": {"subpaths": [{"anchors": [{"p": [0, 0], "out": [10, 0]}, {"p": [20, 0]},
        {"p": [40, 0], "in": [40, -5]}, {"p": [0, 30], "in": [1e-9, 30]}, {"p": [0, 60], "in": [1e-6, 60]}]}]}}}"#;
    let read = read_ok(&document(4, object));
    let Shape::Stroke { path, .. } = &element(&read, "vc:2:stroke").shape else { panic!() };
    let segments = &path.subpaths[0].segments;
    assert_eq!(segments.len(), 4);
    assert!(matches!(segments[0], Segment::Cubic(a, b, end) if close(a, p(10.0, 0.0)) && close(b, p(20.0, 0.0)) && close(end, p(20.0, 0.0))));
    assert!(matches!(segments[1], Segment::Cubic(a, b, end) if close(a, p(20.0, 0.0)) && close(b, p(40.0, -5.0)) && close(end, p(40.0, 0.0))));
    assert!(matches!(segments[2], Segment::Line(end) if close(end, p(0.0, 30.0))), "{:?}", segments[2]);
    assert!(
        matches!(segments[3], Segment::Cubic(a, b, end) if close(a, p(0.0, 30.0)) && b.x() > 0.0 && close(end, p(0.0, 60.0))),
        "{:?}",
        segments[3]
    );
}

#[test]
fn req_vc_001_coordinates_read_exactly_as_vectorcraft_reads_them() {
    // Coordinates as VectorCraft writes them, the shortest decimals that read back exactly. VectorCraft
    // reads them exactly, and a parser that is only nearly exact reads each a unit in the last place off.
    let xs = [180.179_334_388_384_18, -930.039_763_579_936_7, 474.475_323_503_054_37];
    let anchors: Vec<String> = xs.iter().map(|x| format!(r#"{{"p": [{x}, {x}]}}"#)).collect();
    let object = format!(
        r#"{{"id": 2, "appearance": {{"items": [{{"kind": "stroke", "width": 1, "paint": {{"type": "solid", "color": {{"model": "gray", "k": 1}}}}}}]}},
            "kind": {{"type": "path", "path": {{"subpaths": [{{"anchors": [{}]}}]}}}}}}"#,
        anchors.join(",")
    );
    let read = read_ok(&document(4, &object));
    let Shape::Stroke { path, .. } = &element(&read, "vc:2:stroke").shape else { panic!() };
    let exact = |x: f64| Point::new(x * MM_PER_POINT, x * MM_PER_POINT).unwrap();
    let sub = &path.subpaths[0];
    assert_eq!(sub.start, exact(xs[0]));
    assert_eq!(sub.segments, [Segment::Line(exact(xs[1])), Segment::Line(exact(xs[2]))]);
}

#[test]
fn req_vc_001_vectorcraft_s_own_example_reads_alike_in_formats_1_and_4() {
    // VectorCraft's `examples/ribbons.vectorcraft`, as 0.1.0 saved it (format 1) and as 0.11.0 did (format 4).
    let (old, new) = (read_ok(RIBBONS_1), read_ok(RIBBONS_4));
    assert_eq!(old, new);
    // Its background is a gradient, sewn in its first colour; the ribbons are blends in a clipping group,
    // computed as VectorCraft draws them, and the titles are text.
    assert_eq!(ids(&new), ["vc:2:fill"]);
    assert_eq!(element(&new, "vc:2:fill").thread.color, Rgb::new(27, 27, 47));
    let named: Vec<&str> = new.warnings.iter().map(|w| w.element.as_ref().map_or("", |e| e.as_str())).collect();
    assert_eq!(named, ["vc:2", "vc:15", "vc:5", "vc:8", "vc:11", "vc:12", "vc:13"]);
    assert!(new.warnings.iter().all(|w| w.code == Code::VcFeatureIgnored), "{:?}", new.warnings);
}

#[test]
fn req_vc_001_a_repeated_id_gets_the_next_free_suffix() {
    let bytes = document(4, &[square(5, 0.0, 0.0, RED), square(5, 50.0, 0.0, RED), square(5, 100.0, 0.0, RED)].join(","));
    assert_eq!(ids(&read_ok(&bytes)), ["vc:5:fill", "vc:5~2:fill", "vc:5~3:fill"]);
    // An object without an id is labelled by its place among the objects read, the layer being the first.
    let anonymous = square(9, 0.0, 0.0, RED).replacen(r#""id": 9, "#, "", 1);
    assert_eq!(ids(&read_ok(&document(4, &[anonymous.clone(), anonymous].join(",")))), ["vc:#2:fill", "vc:#3:fill"]);
}

#[test]
fn req_vc_001_reading_is_bounded_by_the_budget() {
    let small = Budget { max_stitches: 1, max_work: 50 };
    assert_eq!(read(FORMAT_4, &small).unwrap_err().code, Code::BudgetExhausted);
    let enough = Budget { max_stitches: 1, max_work: 2_000 };
    assert!(read(FORMAT_4, &enough).is_ok());
    // Each element costs its path again, so that many paints on a long path cost what they make: 200 fills
    // of 2,000 anchors are 400,000 segments, from 50 kB.
    let anchors: Vec<String> = (0..2_000).map(|i| format!(r#"{{"p": [{i}, {}]}}"#, i % 7)).collect();
    let fill = r#"{"kind": "fill", "paint": {"type": "solid", "color": {"model": "rgb", "r": 1, "g": 0, "b": 0}}}"#;
    let long = format!(
        r#"{{"id": 2, "appearance": {{"items": [{}]}}, "kind": {{"type": "path", "path": {{"subpaths": [{{"closed": true, "anchors": [{}]}}]}}}}}}"#,
        vec![fill; 200].join(","),
        anchors.join(",")
    );
    let bytes = document(4, &long);
    assert_eq!(read(&bytes, &Budget { max_stitches: 1, max_work: 100_000 }).unwrap_err().code, Code::BudgetExhausted);
    assert_eq!(read(&bytes, &Budget { max_stitches: 1, max_work: 500_000 }).unwrap().design.elements().len(), 200);
}

#[test]
fn req_vc_001_reading_costs_what_the_adapter_documents() {
    let gray = r#""paint": {"type": "solid", "color": {"model": "gray", "k": 1}}"#;
    // An object of 3 paints and 1 effect, an open subpath of 3 anchors and 3 elements of 2 segments: 5, 4
    // and 9 units. Then 1 paint and 1 StitchCraft effect of 2 parameters, a closed subpath of 2 anchors
    // and 1 element of 2 segments: 3, 3, 2 and 3. The layer is 1, and the JSON 1 and 1 for every 64 bytes.
    let painted = format!(
        r#"{{"id": 2, "appearance": {{"items": [{{"kind": "fill", {gray}}}, {{"kind": "fill", {gray}}},
            {{"kind": "stroke", "width": 1, {gray}, "effects": [{{"id": "x", "visible": false}}]}}]}},
            "kind": {{"type": "path", "path": {{"subpaths": [{{"anchors": [{{"p": [0, 0]}}, {{"p": [10, 0]}}, {{"p": [10, 10]}}]}}]}}}}}}"#
    );
    let effect = format!(
        r#"{{"id": 3, "appearance": {{"items": [{{"kind": "fill", {gray}}}], "effects": [{{"id": "plugin.dev.stitchcraft.fill", "params": {{"a": 1, "b": 2}}}}]}},
            "kind": {{"type": "path", "path": {{"subpaths": [{{"closed": true, "anchors": [{{"p": [0, 0]}}, {{"p": [5, 5]}}]}}]}}}}}}"#
    );
    let bytes = document(4, &[painted, effect].join(","));
    let reads = |work: u64| read(&bytes, &Budget { max_stitches: 1, max_work: work }).is_ok();
    let expected = (1 + bytes.len() / 64) as u64 + 1 + (5 + 4 + 9) + (3 + 3 + 2 + 3);
    assert!(reads(expected) && !reads(expected - 1), "{expected}");
}

#[test]
fn req_vc_004_effects_say_how_an_object_sews() {
    let read = read_ok(EFFECTS);
    assert_eq!(
        ids(&read),
        ["vc:50:fill", "vc:50:stroke", "vc:51:satin", "vc:52:fill", "vc:53:fill", "vc:54:fill", "vc:55:fill", "vc:55:fill.2", "vc:56:stroke"]
    );
    // A fill effect fills the shape in the fill's colour, a stroke effect stitches along it in the stroke's.
    let fill = element(&read, "vc:50:fill");
    assert!(matches!(fill.shape, Shape::Fill { .. }));
    assert_eq!(fill.thread.color, Rgb::new(255, 0, 0));
    // Its preview level is the plug-in's, and not a setting of the element.
    let params: Vec<(&str, &str)> = fill.params.iter().collect();
    assert_eq!(params, [("fill_method", "auto_fill"), ("fill_underlay", "true"), ("row_spacing_mm", "0.3")]);
    let stroke = element(&read, "vc:50:stroke");
    assert!(matches!(stroke.shape, Shape::Stroke { .. }));
    assert_eq!(stroke.thread.color, Rgb::new(0, 0, 0));
    assert_eq!(stroke.params.get("running_stitch_length_mm"), Some("2.5"));
    assert_eq!(stroke.params.get("satin_column"), Some("false"));
    // A satin effect makes a satin column of the path, as wide as its stroke, whatever its parameters say.
    let satin = element(&read, "vc:51:satin");
    let Shape::Stroke { width, join: Join::Bevel, .. } = satin.shape else { panic!("{:?}", satin.shape) };
    assert!((width.get() - 3.0).abs() < 1e-6);
    assert_eq!((satin.params.get("satin_column"), satin.params.get("zigzag_spacing_mm")), (Some("true"), Some("0.4")));
    // An effect turned off, or one that names no stitch family, leaves the object to its paints.
    assert!(element(&read, "vc:52:fill").params.iter().next().is_none());
    assert_eq!(element(&read, "vc:53:fill").thread.color, Rgb::new(255, 255, 0));
    // Values are written as the parameters read them.
    let values = element(&read, "vc:54:fill");
    let params: Vec<(&str, &str)> = values.params.iter().collect();
    assert_eq!(
        params,
        [("a_choice", "contour_fill"), ("a_count", "2"), ("a_float_count", "3"), ("a_list", "1 2.5"), ("a_small", "1e-7"), ("a_toggle", "false")]
    );
    assert_eq!(values.thread.color, Rgb::new(0, 0, 0));
    assert_eq!(element(&read, "vc:55:fill.2").params.get("angle"), Some("45"));
}

#[test]
fn req_vc_004_without_effects_an_object_sews_its_fills_then_its_strokes() {
    let gray = |k: f64| format!(r#""paint": {{"type": "solid", "color": {{"model": "gray", "k": {k}}}}}"#);
    let appearance = format!(
        r#"{{"items": [{{"kind": "stroke", "width": 1, {}}}, {{"kind": "fill", {}}}, {{"kind": "stroke", "width": 2, {}}}, {{"kind": "fill", {}}}]}}"#,
        gray(1.0),
        gray(0.0),
        gray(0.5),
        gray(0.25)
    );
    let read = read_ok(&document(4, &square(2, 0.0, 0.0, &appearance)));
    assert_eq!(ids(&read), ["vc:2:fill", "vc:2:fill.2", "vc:2:stroke", "vc:2:stroke.2"]);
    let colours: Vec<Rgb> = read.design.elements().iter().map(|e| e.thread.color).collect();
    assert_eq!(colours, [Rgb::new(255, 255, 255), Rgb::new(191, 191, 191), Rgb::new(0, 0, 0), Rgb::new(128, 128, 128)]);
}

#[test]
fn req_vc_004_an_effect_s_thread_is_the_first_paint_vectorcraft_draws() {
    // A gradient without stops and a stroke of width 0 draw nothing, so the thread is the next paint's.
    let blue = r#"{"kind": "fill", "paint": {"type": "solid", "color": {"model": "rgb", "r": 0, "g": 0, "b": 1}}}"#;
    let nothing = r#"{"kind": "fill", "paint": {"type": "gradient", "gradient": {"kind": "Linear", "stops": []}}},
        {"kind": "stroke", "width": 0, "paint": {"type": "solid", "color": {"model": "rgb", "r": 1, "g": 0, "b": 0}}}"#;
    // A record may leave its parameters out, which leaves them all at their defaults, without a word.
    let effects = r#"[{"id": "plugin.dev.stitchcraft.fill"}, {"id": "plugin.dev.stitchcraft.stroke", "params": {}}]"#;
    let object = square(2, 0.0, 0.0, &format!(r#"{{"items": [{nothing}, {blue}], "effects": {effects}}}"#));
    let read = read_ok(&document(4, &object));
    assert_eq!(ids(&read), ["vc:2:fill", "vc:2:stroke"]);
    assert!(read.design.elements().iter().all(|e| e.thread.color == Rgb::new(0, 0, 255)), "{:?}", read.design.elements());
    assert!(element(&read, "vc:2:fill").params.iter().next().is_none());
    assert_eq!(said(&read), []);
}

#[test]
fn req_vc_005_what_is_not_stitched_is_named() {
    let read = read_ok(FEATURES);
    assert_eq!(
        ids(&read),
        [
            "vc:19:fill",
            "vc:21:stroke",
            "vc:24:fill",
            "vc:28:fill",
            "vc:29:fill",
            "vc:30:fill",
            "vc:31:fill",
            "vc:32:fill",
            "vc:35:fill",
            "vc:36:fill",
            "vc:37:stroke",
            "vc:38:fill"
        ]
    );
    let saids = said(&read);
    let warned: Vec<(Code, &str)> = saids.iter().map(|(code, e)| (*code, e.as_str())).collect();
    let w = Code::VcFeatureIgnored;
    let expected = [
        (w, "vc:10"),
        (w, "vc:11"),
        (w, "vc:12"),
        (w, "vc:13"),
        (w, "vc:14"),
        (w, "vc:15"),
        (w, "vc:16"),
        (w, "vc:17"),
        (w, "vc:20"),
        (w, "vc:28"),
        (w, "vc:30"),
        (w, "vc:31"),
        (w, "vc:32"),
        (Code::VcNotPrinted, "vc:33"),
        (Code::VcNotPrinted, "vc:34"),
        (w, "vc:35"),
        (w, "vc:36"),
        (w, "vc:37"),
        (w, "vc:38"),
    ];
    assert_eq!(warned, expected, "{:#?}", read.warnings);
    // What VectorCraft doesn't draw is left out without a word: objects 25, 26 and 27.
    // A gradient of one colour is sewn in it without a word, a gradient of two in its first, a pattern in black.
    assert_eq!(element(&read, "vc:29:fill").thread.color, Rgb::new(0, 128, 0));
    assert_eq!(element(&read, "vc:28:fill").thread.color, Rgb::new(255, 0, 0));
    assert_eq!(element(&read, "vc:30:fill").thread.color, Rgb::new(0, 0, 0));
}

#[test]
fn req_vc_005_a_stroke_of_width_0_is_left_out_and_a_brush_is_named() {
    let gray = r#""paint": {"type": "solid", "color": {"model": "gray", "k": 1}}"#;
    let zero = square(2, 0.0, 0.0, &format!(r#"{{"items": [{{"kind": "stroke", "width": 0, {gray}}}]}}"#));
    let brushed = square(3, 50.0, 0.0, &format!(r#"{{"items": [{{"kind": "stroke", "width": 2, "brush": "Charcoal", {gray}}}]}}"#));
    let read = read_ok(&document(4, &[zero, brushed].join(",")));
    // VectorCraft draws no stroke of width 0, and nothing is said.
    assert_eq!(ids(&read), ["vc:3:stroke"]);
    assert_eq!(said(&read), [(Code::VcFeatureIgnored, "vc:3".to_string())]);
    assert!(read.warnings[0].message.contains("brush \"Charcoal\""), "{}", read.warnings[0]);
}

/// A tatami fill at 45° with rows 0.3 mm apart, as a StitchCraft Fill effect records it, red.
const TATAMI_45: &str = r#"{"items": [{"kind": "fill", "paint": {"type": "solid", "color": {"model": "rgb", "r": 1, "g": 0, "b": 0}}}],
    "effects": [{"id": "plugin.dev.stitchcraft.fill", "params": {"angle": 45, "row_spacing_mm": 0.3, "preview_level": "rows"}}]}"#;

/// The parameters of `element`, in order.
fn params(element: &Element) -> Vec<(&str, &str)> {
    element.params.iter().collect()
}

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
fn req_vc_006_a_path_without_paint_takes_its_thread_from_the_group_whose_effect_it_sews() {
    // A style made from a painted object paints the group it is applied to, and VectorCraft draws the paths
    // inside, as the effect changes them, in the group's paint. A clipping path is the clip's outline, and
    // VectorCraft draws no art of it: no effect reaches it.
    let green = r#"{"kind": "stroke", "width": 1, "paint": {"type": "solid", "color": {"model": "rgb", "r": 0, "g": 1, "b": 0}}}"#;
    let line = |id: u32, extra: &str| {
        format!(r#"{{"id": {id}, "kind": {{"type": "path", {extra} "path": {{"subpaths": [{{"anchors": [{{"p": [0, 0]}}, {{"p": [50, 0]}}]}}]}}}}}}"#)
    };
    let group = format!(
        r#"{{"id": 2, "appearance": {{"items": [{green}], "effects": [{{"id": "plugin.dev.stitchcraft.stroke"}}]}},
            "kind": {{"type": "group", "clip": true, "children": [{}, {}]}}}}"#,
        line(3, r#""clipping": true,"#),
        line(4, "")
    );
    let read = read_ok(&document(4, &group));
    assert_eq!(ids(&read), ["vc:4:stroke"]);
    assert_eq!(element(&read, "vc:4:stroke").thread.color, Rgb::new(0, 255, 0));
    // The group's clipping and its paint, which StitchCraft doesn't sew as an element of its own, are named.
    assert_eq!(said(&read), [(Code::VcFeatureIgnored, "vc:2".to_string()), (Code::VcFeatureIgnored, "vc:2".to_string())]);
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

/// A compound path as VectorCraft's Make Compound Path writes it, under `appearance`: one member path for
/// each of `members`, each the straight lines through its points.
fn compound(id: u32, appearance: &str, members: &[&[(f64, f64)]]) -> String {
    let members: Vec<String> = members
        .iter()
        .zip(1..)
        .map(|(points, k)| {
            let anchors: Vec<String> = points.iter().map(|(x, y)| format!(r#"{{"p": [{x}, {y}]}}"#)).collect();
            format!(r#"{{"id": {}, "kind": {{"type": "path", "path": {{"subpaths": [{{"anchors": [{}]}}]}}}}}}"#, id * 10 + k, anchors.join(","))
        })
        .collect();
    format!(r#"{{"id": {id}, "appearance": {appearance}, "kind": {{"type": "compound", "rule": "NonZero", "children": [{}]}}}}"#, members.join(","))
}

/// The needle points the engine sews element `id` of `read` with, and what it said.
fn sewn(read: &VectorCraft, id: &str) -> (Vec<Vec<Point>>, Vec<Diagnostic>) {
    let generation = generate(element(read, id), &DesignSettings::default(), REFERENCE, &Neighbours::default(), &Budget::DEFAULT);
    (generation.generated.unwrap().groups, generation.diagnostics)
}

#[test]
fn req_vc_007_a_column_made_of_2_paths_sews_whichever_way_vectorcraft_turns_them() {
    // Make Compound Path turns each path in front round when it turns the same way as the backmost, as 2
    // straight rails drawn the same way do: the satin turns its second rail back (`reverse_rails`).
    let satin = r#"{"items": [{"kind": "stroke", "width": 1, "paint": {"type": "solid", "color": {"model": "gray", "k": 0}}}],
        "effects": [{"id": "plugin.dev.stitchcraft.satin"}]}"#;
    let (back, front, turned) = (&[(0.0, 0.0), (100.0, 0.0)][..], &[(0.0, 20.0), (100.0, 20.0)][..], &[(100.0, 20.0), (0.0, 20.0)][..]);
    // A rung joined to the column, drawn either way.
    let (rung, rung_turned) = (&[(40.0, -5.0), (60.0, 25.0)][..], &[(60.0, 25.0), (40.0, -5.0)][..]);
    let objects = [
        compound(2, satin, &[back, front]),
        compound(3, satin, &[back, turned]),
        compound(4, satin, &[back, turned, rung]),
        compound(5, satin, &[back, front, rung_turned]),
    ];
    let read = read_ok(&document(4, &objects.join(",")));
    assert_eq!(ids(&read), ["vc:2:satin", "vc:3:satin", "vc:4:satin", "vc:5:satin"]);
    let [drawn, made, rung, rung_turned] = ["vc:2:satin", "vc:3:satin", "vc:4:satin", "vc:5:satin"].map(|id| sewn(&read, id));
    assert_eq!(made, drawn);
    assert_eq!(rung_turned, rung);
    assert_ne!(rung.0, drawn.0, "the rung pairs the rails' points");
    assert!(drawn.1.is_empty() && rung.1.is_empty(), "{:?} {:?}", drawn.1, rung.1);
}

#[test]
fn diag_sc_w0803_a_newer_format_is_read_as_the_newest_stitchcraft_reads() {
    let read = read_ok(&document(5, &square(2, 0.0, 0.0, RED)));
    assert_eq!(ids(&read), ["vc:2:fill"]);
    assert_eq!(said(&read), [(Code::VcNewerFormat, String::new())]);
    assert!(read.warnings[0].message.contains("format 5"), "{}", read.warnings[0]);
}

#[test]
fn diag_sc_e0806_what_is_not_a_vectorcraft_document_is_refused() {
    let cases: [(&[u8], &str); 9] = [
        (b"<svg/>", "JSON does not read"),
        (b"[1, 2]", "JSON does not read"),
        (br#"{"format": "inkscape", "version": 1, "document": {"layers": []}}"#, "names the format \"inkscape\""),
        (br#"{"version": 1, "document": {"layers": []}}"#, "names no format"),
        (br#"{"format": "vectorcraft", "document": {"layers": []}}"#, "format version"),
        (br#"{"format": "vectorcraft", "version": "4", "document": {"layers": []}}"#, "format version"),
        (br#"{"format": "vectorcraft", "version": 0, "document": {"layers": []}}"#, "format version"),
        (br#"{"format": "vectorcraft", "version": 4}"#, "no document"),
        (b"{\"format\": \"vectorcraft\", \"version\": 4, \"document\": {}}\n\0VCBLOBS", "NUL byte"),
    ];
    for (bytes, why) in cases {
        let refusal = refusal(bytes);
        assert_eq!(refusal.code, Code::VcUnreadable, "{refusal}");
        assert!(refusal.message.contains(why), "{refusal}");
    }
    // Groups nested past what the JSON reader allows.
    let deep =
        (0..50).fold(square(2, 0.0, 0.0, RED), |inner, i| format!(r#"{{"id": {}, "kind": {{"type": "group", "children": [{inner}]}}}}"#, 100 + i));
    let refusal = refusal(&document(4, &deep));
    assert!(refusal.code == Code::VcUnreadable && refusal.message.contains("recursion limit"), "{refusal}");
}

#[test]
fn diag_sc_w0807_text_images_and_live_objects_are_named_with_how_to_stitch_them() {
    let read = read_ok(FEATURES);
    let text = read.warnings.iter().find(|w| w.element.as_ref().is_some_and(|e| e.as_str() == "vc:10")).unwrap();
    assert_eq!(text.code, Code::VcFeatureIgnored);
    assert!(text.message.contains("Text \"Title\""), "{text}");
    assert!(matches!(&text.fix, Some(Fix::Hint(hint)) if hint.contains("Type › Create Outlines")), "{text}");
    // Each kind of object left out is named, with what makes paths of it in VectorCraft.
    let kinds = [
        ("text", "Text", "Type › Create Outlines"),
        ("image", "An image", "Trace it"),
        ("symbolinstance", "A symbol", "Break Link to Symbol"),
        ("blend", "A blend", "Object › Expand"),
        ("envelope", "An envelope", "Object › Expand"),
        ("mesh", "A gradient mesh", "Draw over it"),
        ("repeat", "A repeat", "Object › Expand"),
        ("placeddocument", "A placed document", "Object › Expand"),
        ("intertwine", "An Intertwine object", "Object › Intertwine › Release"),
    ];
    let objects: Vec<String> = kinds.iter().zip(10..).map(|((kind, ..), id)| format!(r#"{{"id": {id}, "kind": {{"type": "{kind}"}}}}"#)).collect();
    let read = read_ok(&document(4, &objects.join(",")));
    assert_eq!(read.warnings.len(), kinds.len());
    for (warning, (_, what, hint)) in read.warnings.iter().zip(kinds) {
        assert!(warning.message.starts_with(what), "{warning}");
        assert!(matches!(&warning.fix, Some(Fix::Hint(h)) if h.contains(hint)), "{warning}");
    }
    // What is said of a layer's clipping, paints and effects is said of a layer.
    let layer = br#"{"format": "vectorcraft", "version": 4, "document": {"layers": [{"id": 1, "name": "Cut",
        "appearance": {"items": [{"kind": "fill", "paint": {"type": "solid", "color": {"model": "gray", "k": 1}}}],
        "effects": [{"id": "stylize.dropShadow", "params": {}}]}, "kind": {"type": "layer", "clip": true, "children": []}}]}}"#;
    let read = read_ok(layer);
    assert_eq!(read.warnings.len(), 3);
    assert!(read.warnings.iter().all(|w| w.message.starts_with("A layer \"Cut\"")), "{:?}", read.warnings);
}

#[test]
fn diag_sc_w0807_what_stitchcraft_doesn_t_read_in_an_appearance_is_named() {
    let path = r#"{"subpaths": [{"closed": true, "anchors": [{"p": [0, 0]}, {"p": [36, 0]}, {"p": [36, 36]}]}]}"#;
    let object = |id: u32, rule: &str, appearance: &str| {
        format!(r#"{{"id": {id}, "appearance": {appearance}, "kind": {{"type": "path", "rule": "{rule}", "path": {path}}}}}"#)
    };
    let red = r#""paint": {"type": "solid", "color": {"model": "rgb", "r": 1, "g": 0, "b": 0}}"#;
    let objects = [
        r#"{"id": 2, "appearance": {"effects": [{"id": "plugin.dev.stitchcraft.lettering"}]}, "kind": {"type": "group", "children": []}}"#
            .to_string(),
        object(3, "Spiral", RED),
        object(4, "NonZero", &format!(r#"{{"items": [{{"kind": "fill", {red}, "effects": [{{"id": "plugin.dev.stitchcraft.stroke"}}]}}]}}"#)),
        object(5, "NonZero", &format!(r#"{{"items": [{{"kind": "shadow", {red}}}, {{"kind": "fill", {red}}}]}}"#)),
        object(6, "NonZero", r#"{"items": [{"kind": "fill", "paint": {"type": "gradient", "gradient": {"stops": [{"color": {"model": "hsv"}}]}}}]}"#),
        object(7, "EvenOdd", &format!(r#"{{"items": [{{"kind": "stroke", "width": 1, "join": "Wobbly", {red}}}]}}"#)),
        // A satin effect on an object without a stroke has nothing to take its width from.
        object(8, "NonZero", &format!(r#"{{"items": [{{"kind": "fill", {red}}}], "effects": [{{"id": "plugin.dev.stitchcraft.satin"}}]}}"#)),
    ];
    let read = read_ok(&document(4, &objects.join(",")));
    let messages: Vec<(&str, &str)> = read.warnings.iter().map(|w| (w.element.as_ref().map_or("", |e| e.as_str()), w.message.as_str())).collect();
    let expected = [
        ("vc:2", "The StitchCraft effect \"lettering\" stitches nothing, and is not used."),
        ("vc:3", "The fill rule \"Spiral\" is not one StitchCraft reads, and nonzero is used."),
        ("vc:4", "A StitchCraft effect on one fill or stroke of the appearance is not used."),
        ("vc:5", "An appearance item of the kind \"shadow\" is not stitched."),
        ("vc:6", "A gradient's first colour does not read, and it is sewn in black."),
        ("vc:7", "The stroke join \"Wobbly\" is not one StitchCraft reads, and a miter is used."),
    ];
    assert_eq!(messages, expected);
    assert!(read.warnings.iter().all(|w| w.code == Code::VcFeatureIgnored));
    assert!(matches!(element(&read, "vc:3:fill").shape, Shape::Fill { rule: FillRule::NonZero, .. }));
    assert_eq!(element(&read, "vc:6:fill").thread.color, Rgb::new(0, 0, 0));
    assert!(matches!(element(&read, "vc:7:stroke").shape, Shape::Stroke { join: Join::Miter { limit: 10.0 }, .. }));
    let Shape::Stroke { width, .. } = element(&read, "vc:8:satin").shape else { panic!() };
    assert_eq!((width, element(&read, "vc:8:satin").thread.color), (Mm::ZERO, Rgb::new(255, 0, 0)));
}

#[test]
fn diag_sc_w0808_geometry_that_cannot_be_used_is_left_out() {
    let null = r#"{"id": 2, "appearance": {"items": [{"kind": "fill", "paint": {"type": "solid", "color": {"model": "rgb", "r": 1, "g": 0, "b": 0}}}]},
        "kind": {"type": "path", "path": {"subpaths": [{"anchors": [{"p": [0, 0]}, {"p": [null, 5]}, {"p": [5, 5]}]}]}}}"#;
    let one_anchor = r#"{"id": 3, "appearance": {"items": [{"kind": "stroke", "width": 1, "paint": {"type": "solid", "color": {"model": "gray", "k": 1}}}]},
        "kind": {"type": "path", "path": {"subpaths": [{"anchors": [{"p": [0, 0]}]}]}}}"#;
    // Every anchor far away, and only the start of an open path.
    let far = square(4, 1.0e6, 0.0, RED);
    let far_start = r#"{"id": 6, "appearance": {"items": [{"kind": "stroke", "width": 1, "paint": {"type": "solid", "color": {"model": "gray", "k": 1}}}]},
        "kind": {"type": "path", "path": {"subpaths": [{"anchors": [{"p": [-1e6, 0]}, {"p": [5, 5]}]}]}}}"#;
    // A width VectorCraft wrote for one that wasn't a number; a negative width, which VectorCraft draws
    // nothing for, as for 0, so nothing is said.
    let bad_width = r#"{"id": 5, "appearance": {"items": [{"kind": "stroke", "width": null, "paint": {"type": "solid", "color": {"model": "gray", "k": 1}}},
        {"kind": "stroke", "width": -1, "paint": {"type": "solid", "color": {"model": "gray", "k": 1}}}]},
        "kind": {"type": "path", "path": {"subpaths": [{"anchors": [{"p": [0, 0]}, {"p": [5, 5]}]}]}}}"#;
    let read = read_ok(&document(4, &[null, one_anchor, &far, bad_width, far_start].join(",")));
    assert!(read.design.elements().is_empty(), "{:?}", ids(&read));
    let unusable = Code::VcGeometryUnusable;
    let expected = [(unusable, "vc:2"), (unusable, "vc:3"), (unusable, "vc:4"), (unusable, "vc:5"), (unusable, "vc:6")];
    assert_eq!(said(&read).iter().map(|(c, e)| (*c, e.as_str())).collect::<Vec<_>>(), expected);
    assert!(read.warnings[0].message.contains("not a number") && read.warnings[1].message.contains("draws nothing"));
    assert!(read.warnings[2].message.contains("10 m"), "{}", read.warnings[2]);
    assert!(read.warnings[3].message.contains("not a number, and the stroke is left out"), "{}", read.warnings[3]);
    assert!(read.warnings[4].message.contains("10 m"), "{}", read.warnings[4]);
    // Under a satin effect, a width that doesn't read is taken as 0, and said.
    let satin = r#"{"id": 6, "appearance": {"items": [{"kind": "stroke", "width": null, "paint": {"type": "solid", "color": {"model": "gray", "k": 1}}}],
        "effects": [{"id": "plugin.dev.stitchcraft.satin", "params": {}}]},
        "kind": {"type": "path", "path": {"subpaths": [{"anchors": [{"p": [0, 0]}, {"p": [5, 5]}]}]}}}"#;
    let read = read_ok(&document(4, satin));
    let Shape::Stroke { width, .. } = element(&read, "vc:6:satin").shape else { panic!() };
    assert_eq!(width, Mm::ZERO);
    assert_eq!(said(&read), [(unusable, "vc:6".to_string())]);
    assert!(read.warnings[0].message.contains("its effects take it as 0"), "{}", read.warnings[0]);
}

#[test]
fn diag_sc_i0809_layers_and_guides_vectorcraft_does_not_print_are_left_out() {
    let read = read_ok(FEATURES);
    let notes: Vec<&Diagnostic> = read.warnings.iter().filter(|w| w.code == Code::VcNotPrinted).collect();
    assert_eq!(notes.len(), 2);
    assert!(notes[0].message.contains("A layer \"Draft\" doesn't print"), "{}", notes[0]);
    assert!(notes[1].message.contains("guide"), "{}", notes[1]);
}
