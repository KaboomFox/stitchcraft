use stitchcraft_core::{ElementId, Mm};
use stitchcraft_engine::auto::expand;
use stitchcraft_engine::design::{DesignSettings, Guide, Hints, Reading};
use stitchcraft_params::ParamSet;
use stitchcraft_plan::Thread;
use stitchcraft_plan::profiles::REFERENCE;

use super::*;
use crate::read_with;

/// A red square path of side 36 pt at (`x`, `y`) pt with `id`.
fn square(id: u32, x: f64, y: f64) -> String {
    let (r, b) = (x + 36.0, y + 36.0);
    format!(
        r#"{{"id": {id}, "appearance": {{"items": [{{"kind": "fill", "paint": {{"type": "solid", "color": {{"model": "rgb", "r": 1, "g": 0, "b": 0}}}}}}]}}, "kind": {{"type": "path", "path": {{"subpaths": [{{"closed": true, "anchors": [{{"p": [{x}, {y}]}}, {{"p": [{r}, {y}]}}, {{"p": [{r}, {b}]}}, {{"p": [{x}, {b}]}}]}}]}}}}}}"#
    )
}

/// A document of one layer holding `objects`, with `document` keys before its layers.
fn document(extra: &str, objects: &str) -> String {
    format!(
        r#"{{"format": "vectorcraft", "version": 4, "document": {{"version": 1, {extra}"layers": [{{"id": 1, "kind": {{"type": "layer", "children": [{objects}]}}}}]}}}}"#
    )
}

/// The traced group: a group of 2 red squares, and apart from it a blue marker of where a route starts, which
/// sews nothing.
fn traced(extra: &str) -> String {
    let blue = r#"{"id": 9, "name": "stitch:route-start", "appearance": {"items": [{"kind": "fill", "paint": {"type": "solid", "color": {"model": "rgb", "r": 0, "g": 0, "b": 1}}}]}, "kind": {"type": "path", "path": {"subpaths": [{"closed": true, "anchors": [{"p": [300, 300]}, {"p": [330, 300]}, {"p": [330, 330]}]}]}}}"#;
    document(extra, &format!(r#"{{"id": 3, "kind": {{"type": "group", "children": [{}, {}]}}}}, {blue}"#, square(4, 0.0, 0.0), square(5, 18.0, 18.0)))
}

/// `text` read as `stitch expand` reads it, expanded, written and read back as a design set by hand.
fn round_trip(text: &str) -> (Design, Design, String) {
    let original = read_with(text.as_bytes(), &Budget::DEFAULT, Reading { auto: true, ..Reading::default() }).unwrap().design;
    let (expanded, _) = expand(&original, REFERENCE, &Budget::DEFAULT).unwrap();
    let written = written(text.as_bytes(), &original, &expanded, REFERENCE, &Budget::DEFAULT).unwrap();
    assert!(written.warnings.is_empty(), "{:?}", written.warnings);
    let back = crate::read(&written.document, &Budget::DEFAULT).unwrap().design;
    (expanded, back, String::from_utf8(written.document).unwrap())
}

/// Whether `a` and `b` are the same points to within a nanometre.
fn near(a: &Path, b: &Path) -> bool {
    let (a, b): (Vec<Point>, Vec<Point>) = (a.points().collect(), b.points().collect());
    a.len() == b.len() && a.iter().zip(&b).all(|(p, q)| p.distance(*q) < 1e-9)
}

#[test]
fn req_cli_025_the_parts_go_in_a_group_over_the_traced_shapes_hidden_and_locked() {
    let text = traced(r#""title": "a \"quoted\" \\ title", "next_id": 12, "#);
    let (expanded, back, written) = round_trip(&text);
    // The group holding the automatic shapes is hidden and locked; the object apart keeps its bytes.
    assert!(written.contains(r#"{"visible": false, "locked": true, "id": 3, "kind": {"type": "group""#), "{written}");
    assert!(written.contains(r#"{"id": 9, "name": "stitch:route-start", "appearance""#));
    assert!(written.contains(&format!(r#""name":"{GROUP}""#)), "{written}");
    // The parts read back as they were expanded, in their order, each with its thread and settings.
    assert_eq!(back.elements().len(), expanded.elements().len());
    for (part, read) in expanded.elements().iter().zip(back.elements()) {
        assert_eq!((read.thread.color, read.name.as_deref()), (part.thread.color, part.name.as_deref()));
        let kept: ParamSet = read.params.iter().filter(|(key, _)| *key != SATIN_COLUMN).collect();
        let given: ParamSet = part.params.iter().filter(|(key, _)| *key != SATIN_COLUMN).collect();
        assert_eq!(kept, given, "{}", part.id);
        assert!(!read.automatic);
    }
    // And they sew as they were expanded, every stitch within a nanometre.
    let stitches = |design: &Design| {
        let plan = stitchcraft_engine::plan(design, REFERENCE, &Budget::DEFAULT).plan.unwrap();
        plan.stitches().map(|stitch| (stitch.at, stitch.kind, stitch.origin.role)).collect::<Vec<_>>()
    };
    let (want, got) = (stitches(&expanded), stitches(&back));
    assert_eq!(want.len(), got.len());
    assert!(want.iter().zip(&got).all(|(a, b)| a.0.distance(b.0) < 1e-9 && a.1 == b.1 && a.2 == b.2));
    // New ids come after the largest the document has, and the document counts past them.
    let ids = 12 + expanded.elements().len() as u64;
    assert!(written.contains(&format!(r#""next_id": {}, "#, ids + 1)), "{written}");
    // The design settings and the cap are kept with the Design Settings filter's data.
    let kept = crate::read(written.as_bytes(), &Budget::DEFAULT).unwrap();
    assert_eq!((kept.design.settings.cap, kept.machine.map(|machine| machine.id)), (expanded.settings.cap, Some(REFERENCE.id)));
}

#[test]
fn req_cli_025_what_a_document_keeps_already_is_replaced_and_its_other_data_stays() {
    let other = r#""dev.other": {"a": 1}"#;
    let filter = plugin_id(SETTINGS_FILTER);
    for extra in [
        String::new(),
        r#""unknown": {}, "#.to_string(),
        r#""unknown": {"pluginData": []}, "#.to_string(),
        format!(r#""unknown": {{"pluginData": {{{other}}}}}, "keywords": ["a", [1, [2]]], "#),
        format!(r#""unknown": {{"pluginData": {{"{filter}": {{"collapse_len_mm": 9}}, {other}}}}}, "#),
        r#""unknown": {"pluginData": {}}, "#.to_string(),
        r#""unknown": {"pluginData": 5}, "#.to_string(),
        format!(r#""unknown": {{"pluginData": {{"{filter}": "old"}}}}, "#),
    ] {
        let (expanded, _, written) = round_trip(&traced(&extra));
        let read = crate::read(written.as_bytes(), &Budget::DEFAULT).unwrap();
        assert_eq!(read.design.settings.cap, expanded.settings.cap, "{extra}");
        assert_eq!(read.design.settings.collapse_len, expanded.settings.collapse_len, "{extra}");
        assert!(read.warnings.is_empty(), "{extra}: {:?}", read.warnings);
        if extra.contains("dev.other") {
            assert!(written.contains(other), "{written}");
        }
        // A document without `next_id` is given one.
        assert!(written.contains(r#""next_id": "#), "{written}");
    }
}

#[test]
fn req_cli_025_a_top_object_s_own_visible_and_locked_are_set_and_not_repeated() {
    let text = document(
        "",
        &format!(r#"{{"visible": true, "id": 3, "locked": false, "kind": {{"type": "group", "children": [{}]}}}}"#, square(4, 0.0, 0.0)),
    );
    let (_, back, written) = round_trip(&text);
    assert!(written.contains(r#"{"visible": false, "id": 3, "locked": true, "kind""#), "{written}");
    assert!(!back.elements().is_empty());
}

#[test]
fn req_cli_025_a_part_with_hints_is_grouped_with_its_markers_and_a_thin_line_keeps_its_thread() {
    let at = |x: f64, y: f64| Point::new(x, y).unwrap();
    let path = |points: &[(f64, f64)]| Path {
        subpaths: vec![stitchcraft_engine::design::Subpath {
            start: at(points[0].0, points[0].1),
            segments: points[1..].iter().map(|&(x, y)| stitchcraft_engine::design::Segment::Line(at(x, y))).collect(),
            closed: false,
        }],
    };
    let hints = Hints {
        start: Some(at(1.0, 1.0)),
        end: Some(at(9.0, 9.0)),
        target: Some(at(5.0, 4.0)),
        guide: Some(Guide { path: path(&[(0.0, 5.0), (10.0, 6.0)]), width: Mm::ZERO, join: Join::Round, params: ParamSet::new() }),
        anchor: Some(path(&[(2.0, 0.0), (2.0, 10.0)])),
    };
    let fill = Element {
        id: ElementId::new("vc:4:fill").unwrap(),
        name: Some("Red fill (1)".to_string()),
        shape: Shape::Fill { path: path(&[(0.0, 0.0), (10.0, 0.0), (10.0, 10.0), (0.0, 10.0)]), rule: FillRule::EvenOdd, gradient: None },
        thread: Thread::new(stitchcraft_plan::Rgb::new(200, 0, 0)),
        params: [("fill_method", "guided_fill"), ("digitized_as", "part")].into_iter().collect(),
        automatic: false,
        lettered: None,
        hints: hints.clone(),
        clips: Vec::new(),
    };
    let thin = Element {
        id: ElementId::new("vc:4:stroke").unwrap(),
        shape: Shape::Stroke { path: path(&[(0.0, 20.0), (10.0, 20.0)]), width: Mm::ZERO, join: Join::Miter { limit: 4.0 }, cap: Cap::Square },
        thread: Thread::new(stitchcraft_plan::Rgb::new(0, 0, 200)),
        params: [("satin_column", "false"), ("bean_stitch_repeats", "2")].into_iter().collect(),
        hints: Hints::default(),
        ..fill.clone()
    };
    // Strokes with each corner and each end keep them.
    let styled = |id: &str, join: Join, cap: Cap| Element {
        id: ElementId::new(id).unwrap(),
        shape: Shape::Stroke { path: path(&[(0.0, 30.0), (5.0, 35.0), (10.0, 30.0)]), width: Mm::new(0.5).unwrap(), join, cap },
        ..thin.clone()
    };
    let styles = [(Join::Round, Cap::Round), (Join::Bevel, Cap::Butt), (Join::Miter { limit: 3.0 }, Cap::Square)];
    let strokes: Vec<Element> = styles.iter().enumerate().map(|(k, (join, cap))| styled(&format!("vc:4:s{k}"), *join, *cap)).collect();
    let settings = DesignSettings { origin: Some(at(5.0, 5.0)), stop_position: Some(at(0.0, 0.0)), ..DesignSettings::default() };
    let expanded = Design::new([fill, thin].into_iter().chain(strokes).collect(), settings).unwrap();
    // The traced document's origin and stop position were in its traced group.
    let marker = |id: u32, name: &str| {
        format!(r#"{{"id": {id}, "name": "{name}", "kind": {{"type": "path", "path": {{"subpaths": [{{"anchors": [{{"p": [0, 0]}}]}}]}}}}}}"#)
    };
    let text = document(
        "",
        &format!(
            r#"{{"id": 3, "kind": {{"type": "group", "children": [{}, {}, {}]}}}}"#,
            square(4, 0.0, 0.0),
            marker(7, "stitch:origin"),
            marker(8, "stitch:stop-position")
        ),
    );
    let original = crate::read(text.as_bytes(), &Budget::DEFAULT).unwrap().design;
    let written = written(text.as_bytes(), &original, &expanded, REFERENCE, &Budget::DEFAULT).unwrap();
    let back = crate::read(&written.document, &Budget::DEFAULT).unwrap();
    let elements = back.design.elements();
    assert_eq!(elements.len(), 5, "{:?}", back.warnings);
    for (element, (join, cap)) in elements[2..].iter().zip(styles) {
        assert!(matches!(element.shape, Shape::Stroke { join: j, cap: c, .. } if j == join && c == cap), "{:?}", element.shape);
    }
    let read = &elements[0].hints;
    let close = |a: Option<Point>, b: Option<Point>| a.zip(b).is_some_and(|(a, b)| a.distance(b) < 1e-9);
    assert!(close(read.start, hints.start) && close(read.end, hints.end) && close(read.target, hints.target), "{read:?}");
    assert!(near(&read.guide.as_ref().unwrap().path, &hints.guide.unwrap().path));
    assert!(near(read.anchor.as_ref().unwrap(), &hints.anchor.unwrap()));
    assert!(matches!(elements[0].shape, Shape::Fill { rule: FillRule::EvenOdd, .. }));
    // The line no wider than 0 keeps its thread.
    assert_eq!(elements[1].thread.color, stitchcraft_plan::Rgb::new(0, 0, 200));
    assert_eq!(elements[1].params.get("bean_stitch_repeats"), Some("2"));
    assert!(close(back.design.settings.origin, settings.origin) && close(back.design.settings.stop_position, settings.stop_position));
}

#[test]
fn diag_sc_w0922_a_part_cut_to_a_clip_is_written_whole_and_named() {
    let text = traced("");
    let original = read_with(text.as_bytes(), &Budget::DEFAULT, Reading { auto: true, ..Reading::default() }).unwrap().design;
    let (expanded, _) = expand(&original, REFERENCE, &Budget::DEFAULT).unwrap();
    let mut elements = expanded.elements().to_vec();
    let clip = stitchcraft_engine::design::Clip::default();
    elements[0].clips.push(clip);
    let clipped = Design::new(elements, expanded.settings).unwrap();
    let written = written(text.as_bytes(), &original, &clipped, REFERENCE, &Budget::DEFAULT).unwrap();
    assert_eq!(written.warnings.len(), 1);
    let name = clipped.elements()[0].name.clone().unwrap();
    assert_eq!(
        written.warnings[0].to_string(),
        format!(
            "warning SC-W0922: Expand couldn't keep everything. \"{name}\" is cut to a clipping shape, and the document has it whole: it sews \
             all of its shape there."
        )
    );
    let unnamed = Element { name: None, ..clipped.elements()[0].clone() };
    assert!(unclipped(&unnamed).message.contains(&format!("`{}` is cut", unnamed.id)));
}

#[test]
fn a_document_that_isn_t_json_as_vectorcraft_writes_it_is_an_error_and_a_compressed_one_stays_compressed() {
    use std::io::{Read, Write};
    let text = traced("");
    let original = read_with(text.as_bytes(), &Budget::DEFAULT, Reading { auto: true, ..Reading::default() }).unwrap().design;
    let (expanded, _) = expand(&original, REFERENCE, &Budget::DEFAULT).unwrap();
    let code = |bytes: &[u8]| written(bytes, &original, &expanded, REFERENCE, &Budget::DEFAULT).map(|_| ()).map_err(|said| said.code);
    for broken in [
        "[1]",
        r#"{"document": {"unknown": 5, "layers": [{"kind": {"children": []}}]}}"#,
        r#"{"document": {"layers": [}"#,
        r#"{"a": ["#,
        r#"{"document": {"layers": []}}"#,
        r#"{"layers": [{"kind": {"children": []}}]}"#,
        "\u{ff}",
    ] {
        assert_eq!(code(broken.as_bytes()), Err(Code::InternalCheckFailed), "{broken}");
    }
    let budget = Budget { max_work: 10, ..Budget::DEFAULT };
    assert_eq!(written(text.as_bytes(), &original, &expanded, REFERENCE, &budget).map(|_| ()).map_err(|said| said.code), Err(Code::BudgetExhausted));
    let tight = Budget { max_work: text.len() as u64 + 1, ..Budget::DEFAULT };
    assert_eq!(written(text.as_bytes(), &original, &expanded, REFERENCE, &tight).map(|_| ()).map_err(|said| said.code), Err(Code::BudgetExhausted));
    // A compressed document is written compressed, with the data after its JSON as it was.
    let mut encoder = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
    encoder.write_all(format!("{text}\n\0VCBLOBS\0data").as_bytes()).unwrap();
    let packed = written(&encoder.finish().unwrap(), &original, &expanded, REFERENCE, &Budget::DEFAULT).unwrap().document;
    let mut unpacked = Vec::new();
    flate2::read::GzDecoder::new(packed.as_slice()).read_to_end(&mut unpacked).unwrap();
    assert!(unpacked.ends_with(b"\n\0VCBLOBS\0data"));
    // An empty layer takes the group without a comma before it.
    let empty = document("", "");
    let none = Design::new(Vec::new(), DesignSettings::default()).unwrap();
    let out = written(empty.as_bytes(), &none, &none, REFERENCE, &Budget::DEFAULT).unwrap().document;
    assert!(
        String::from_utf8(out).unwrap().contains(&format!(r#""children": [{{"id":2,"kind":{{"children":[],"type":"group"}},"name":"{GROUP}"}}]"#))
    );
}

#[test]
fn a_setting_is_written_as_the_value_vectorcraft_s_dialog_holds() {
    assert_eq!(valued("true"), Value::Bool(true));
    assert_eq!(valued("false"), Value::Bool(false));
    assert_eq!(valued("0.4"), json!(0.4));
    assert_eq!(valued("3"), json!(3.0));
    assert_eq!(valued("007"), json!("007"));
    assert_eq!(valued("-1 0"), json!("-1 0"));
    assert_eq!(valued("part"), json!("part"));
    assert_eq!(object_of("vc:12:fill#2"), Some(12));
    assert_eq!((object_of("vc:#3:fill"), object_of("svg:path")), (None, None));
}

#[test]
fn the_scan_finds_the_layers_objects_their_ids_and_the_document_s_keys() {
    let text = r#"{"document": {"next_id": 12, "layers": [{"id": 1, "next_id": 3, "kind": {"children": [
        {"id": 3, "kind": {"children": [{"id": 4, "visible": true, "locked": true}, {"id": 5, "note": "six", "name": "five"}]}},
        {"locked": false, "id": 6, "visible": true, "a": ["x", 1, [2]]}
    ]}}, {"id": 2, "kind": {"children": []}}], "title": "a \"b\" \\"}}"#;
    let scanned = scan(text.as_bytes(), "f").unwrap();
    // The layers' 2 objects, each with the ids inside it, and the 2 layers' lists: no nested list is a layer's.
    let at = |what: &str| text.find(what).unwrap();
    let top = |k: usize| (scanned.tops[k].ids.clone(), scanned.tops[k].visible, scanned.tops[k].locked);
    assert_eq!(top(0), (vec![3, 4, 5], None, None));
    assert_eq!(top(1), (vec![6], Some((at(r#"true, "a""#), 4)), Some((at("false"), 5))));
    assert_eq!((scanned.tops[0].layer, scanned.tops[1].layer), (0, 0));
    assert_eq!(scanned.tops[0].names, ["five"]);
    assert_eq!((scanned.tops[0].open, scanned.tops[1].open), (at(r#"{"id": 3"#), at(r#"{"locked""#)));
    let layers: Vec<(usize, bool)> = scanned.layers.iter().map(|layer| (layer.close, layer.empty)).collect();
    assert_eq!(layers, [(at("]}}, {\"id\": 2"), false), (at("]}}], \"title\""), true)]);
    // The document's own `next_id`, not a layer's, and the largest id.
    assert_eq!((scanned.next_id, scanned.largest), (Some((at("12"), 2, 12)), 6));
    assert_eq!(scanned.spans.iter().find(|(kept, _)| *kept == 0).map(|(_, span)| span.0), Some(at(r#"{"next_id""#)));
    // A string or a number in a layer's list is in it, and a document of a token alone reads.
    let listed = |inside: &str| scan(format!(r#"{{"layers": [{{"kind": {{"children": [{inside}]}}}}]}}"#).as_bytes(), "f").unwrap().layers[0].empty;
    assert_eq!((listed(r#""a""#), listed("1"), listed("")), (false, false, true));
    assert!(scan(b"5", "f").is_some() && scan(b" 5", "f").is_some());
    // Brackets that don't match aren't JSON.
    for broken in [&b"{]"[..], b"[}", b"{\"a\": [}", b"[1"] {
        assert!(scan(broken, "f").is_none(), "{}", String::from_utf8_lossy(broken));
    }
    assert_eq!(string_end(br#""a\"b""#, 0), Some(5));
    assert_eq!(string_end(br#""a"#, 0), None);
}

#[test]
fn req_cli_025_the_group_goes_in_the_layer_of_the_first_hidden_object() {
    let first = r#"{"id": 1, "kind": {"type": "layer", "children": [{"id": 20, "name": "stitch:route-start", "kind": {"type": "path", "path": {"subpaths": []}}}]}}"#;
    let second = format!(r#"{{"id": 2, "kind": {{"type": "layer", "children": [{}]}}}}"#, square(4, 0.0, 0.0));
    let text = format!(r#"{{"format": "vectorcraft", "version": 4, "document": {{"version": 1, "layers": [{first}, {second}]}}}}"#);
    let (expanded, back, written) = round_trip(&text);
    assert_eq!(back.elements().len(), expanded.elements().len());
    let group = written.find(GROUP).unwrap();
    assert!(group > written.find(r#""id": 2, "kind""#).unwrap(), "{written}");
}

#[test]
fn req_cli_025_an_origin_is_written_only_in_place_of_a_marker_the_hidden_objects_held() {
    let at = Point::new(5.0, 5.0).unwrap();
    let marker = r#"{"id": 7, "name": "stitch:origin", "kind": {"type": "path", "path": {"subpaths": [{"anchors": [{"p": [0, 0]}]}]}}}"#;
    let grouped = format!(r#"{{"id": 3, "kind": {{"type": "group", "children": [{}, {marker}]}}}}"#, square(4, 0.0, 0.0));
    let apart = format!("{}, {marker}", square(4, 0.0, 0.0));
    // (the objects, whether the document has a marker, whether a hidden object holds it)
    for (objects, marked, held) in [(square(4, 0.0, 0.0), false, false), (apart, true, false), (grouped, true, true)] {
        let text = document("", &objects);
        let original = read_with(text.as_bytes(), &Budget::DEFAULT, Reading { auto: true, ..Reading::default() }).unwrap().design;
        let (expanded, _) = expand(&original, REFERENCE, &Budget::DEFAULT).unwrap();
        let expanded = Design::new(expanded.elements().to_vec(), DesignSettings { origin: Some(at), ..expanded.settings }).unwrap();
        // Nothing hidden: the document's own marker places the origin, and no other is added.
        let none = Design::new(Vec::new(), DesignSettings::default()).unwrap();
        let unhidden = written(text.as_bytes(), &none, &expanded, REFERENCE, &Budget::DEFAULT).unwrap();
        assert_eq!(String::from_utf8(unhidden.document).unwrap().matches("stitch:origin").count(), usize::from(marked));
        // A marker in a hidden object is written again, and one that stays shown isn't.
        let hidden = written(text.as_bytes(), &original, &expanded, REFERENCE, &Budget::DEFAULT).unwrap();
        let count = String::from_utf8(hidden.document.clone()).unwrap().matches("stitch:origin").count();
        assert_eq!(count, usize::from(marked) + usize::from(held), "{objects}");
        let back = crate::read(&hidden.document, &Budget::DEFAULT).unwrap();
        assert_eq!(back.design.settings.origin.is_some(), marked, "{:?}", back.warnings);
    }
}
