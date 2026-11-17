//! Text objects of a VectorCraft document in automatic digitizing (roadmap step M14.7, `REQ-VC-029`): point
//! type that automatic digitizing reaches sews as lettering in the default embroidery font, drawn here in
//! code, at the text's size and place. No font file of anyone's is used.

// Test code may unwrap and index (clippy.toml allows it in tests). Its helpers take the JSON a case writes in
// place, and round a coordinate that is a few millimetres.
#![allow(clippy::unwrap_used, clippy::needless_pass_by_value, clippy::cast_possible_truncation)]

use std::collections::BTreeSet;
use std::rc::Rc;

use serde_json::{Value, json};
use stitchcraft_core::units::Point;
use stitchcraft_core::{Budget, Code, Diagnostic, Fix};
use stitchcraft_engine::design::{Element, Reading};
use stitchcraft_plan::{Rgb, Thread};
use stitchcraft_svg::font::Font;
use stitchcraft_svg::lettering::objects::{self, Anchor, Line, TextObject};
use stitchcraft_svg::lettering::pictures::Found;
use stitchcraft_vectorcraft::lettering::{BuiltIn, Folder, Fonts};
use stitchcraft_vectorcraft::{VectorCraft, effect_id, read_in, read_with};

/// A glyph file, 100 mm square with a user unit a millimetre and its baseline 70 mm down. `a` is a black
/// stroke from (1, 60) to (5, 70) and `b` one from (2, 60) to (8, 70).
fn glyphs() -> String {
    let glyph = |label: &str, id: &str, left: f64, right: f64| {
        format!(
            r##"<g inkscape:groupmode="layer" inkscape:label="GlyphLayer-{label}"><path id="{id}" d="M {left},60 L {right},70" style="fill:none;stroke:#000000;stroke-width:0.4"/></g>"##
        )
    };
    format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" xmlns:inkscape="http://www.inkscape.org/namespaces/inkscape" xmlns:sodipodi="http://sodipodi.sourceforge.net/DTD/sodipodi-0.dtd" width="100mm" height="100mm" viewBox="0 0 100 100"><sodipodi:namedview id="base"><sodipodi:guide position="0,30" orientation="0,1" inkscape:label="baseline"/></sodipodi:namedview>{}{}</svg>"#,
        glyph("a", "pa", 1.0, 5.0),
        glyph("b", "pb", 2.0, 8.0)
    )
}

/// The settings of the test font: 10 mm tall, sewn from 5 to 20 mm.
const SETTINGS: &str = r#"{"name": "Test Font", "size": 10, "min_scale": 0.5, "max_scale": 2, "auto_satin": false}"#;

/// A document of one layer that holds `objects`.
fn document(objects: Value) -> Vec<u8> {
    let layer = json!({"id": 1, "kind": {"type": "layer", "children": objects}});
    serde_json::to_vec(&json!({"format": "vectorcraft", "version": 4, "document": {"layers": [layer]}})).unwrap()
}

fn red() -> Value {
    json!({"type": "solid", "color": {"model": "rgb", "r": 1, "g": 0, "b": 0}})
}

/// The point type `id` of `text` at (`x`, `y`) points, 36 points in size and filled red, with `more` fields
/// of its kind and the effects `effects`, each by its short name.
fn text(id: u64, text: &str, x: f64, y: f64, more: Value, effects: &[&str]) -> Value {
    let mut kind = json!({"type": "text", "kind": {"kind": "point"}, "xf": [1, 0, 0, 1, x, y],
        "runs": [{"text": text, "style": {"font_family": "Source Sans 3", "size": 36, "fill": red()}}]});
    for (key, value) in more.as_object().unwrap() {
        kind[key] = value.clone();
    }
    let effects: Vec<Value> = effects.iter().map(|name| json!({"id": effect_id(name), "params": {}, "visible": true})).collect();
    json!({"id": id, "name": "Title", "appearance": {"effects": effects}, "kind": kind})
}

/// A group `id` with the effects `effects` and `more` fields of its kind, holding `children`.
fn group(id: u64, effects: &[&str], more: Value, children: Value) -> Value {
    let mut kind = json!({"type": "group", "children": children});
    for (key, value) in more.as_object().unwrap() {
        kind[key] = value.clone();
    }
    let effects: Vec<Value> = effects.iter().map(|name| json!({"id": effect_id(name), "params": {}, "visible": true})).collect();
    json!({"id": id, "appearance": {"effects": effects}, "kind": kind})
}

/// `values` kept for as long as the tests run, as a plug-in's fonts are kept in it.
fn kept<T>(values: Vec<T>) -> &'static [T] {
    Box::leak(values.into_boxed_slice())
}

/// The plug-in's default font, drawn here.
fn fonts() -> BuiltIn {
    let files: Vec<(&'static str, &'static [u8])> = vec![("font.json", kept(SETTINGS.as_bytes().to_vec())), ("ltr.svg", kept(glyphs().into_bytes()))];
    BuiltIn::new(kept(vec![Folder { name: "small_font", files: kept(files) }]))
}

const AUTO: Reading = Reading { auto: true, sort_colours: false };

/// `bytes` read with [`fonts`], as `reading` asks.
fn read(bytes: &[u8], reading: Reading) -> VectorCraft {
    read_in(bytes, &Budget::DEFAULT, reading, &mut fonts()).unwrap()
}

/// What `said` says, each with its hint.
fn said(said: &[Diagnostic]) -> Vec<String> {
    said.iter()
        .map(|d| match &d.fix {
            Some(Fix::Hint(hint)) => format!("{d} [{hint}]"),
            _ => d.to_string(),
        })
        .collect()
}

/// A line of `text` anchored at (`x`, `y`) mm, `size` mm its font size, in pure red.
fn line(text: &str, x: f64, y: f64, size: f64) -> Line {
    Line {
        text: text.to_string(),
        at: Point::new(x, y).unwrap(),
        anchor: Anchor::Start,
        size,
        thread: Some(Thread::new(Rgb::new(255, 0, 0))),
        length: None,
    }
}

/// What the text object `vc:7` of `lines`, turned by `turn`, sews in the test font.
fn expected(lines: Vec<Line>, turn: (f64, f64)) -> Vec<Element> {
    let font: Rc<Font> = fonts().font("", &Budget::DEFAULT).unwrap().unwrap();
    let text = TextObject { label: "vc:7".to_string(), name: Some("Title".to_string()), typeface: None, bold: false, lines, turn, clips: Vec::new() };
    objects::letter(&text, &mut |_, _| Found::Font(Rc::clone(&font)), &mut BTreeSet::new(), &Budget::DEFAULT).unwrap().0
}

/// The points of the element `index` of `read`, in hundredths of a millimetre.
fn points(read: &VectorCraft, index: usize) -> Vec<(i64, i64)> {
    let round = |v: f64| (v * 100.0).round() as i64;
    read.design.elements()[index].shape.path().points().map(|p| (round(p.x()), round(p.y()))).collect()
}

/// The elements of `read` are `expected`: each with its id, its name, its thread and its clips, and its
/// points within a nanometre.
#[track_caller]
fn same(read: &VectorCraft, expected: &[Element]) {
    let elements = read.design.elements();
    assert_eq!(elements.len(), expected.len());
    for (ours, theirs) in elements.iter().zip(expected) {
        assert_eq!(
            (&ours.id, &ours.name, &ours.thread, &ours.clips, &ours.params),
            (&theirs.id, &theirs.name, &theirs.thread, &theirs.clips, &theirs.params)
        );
        let (ours, theirs): (Vec<Point>, Vec<Point>) = (ours.shape.path().points().collect(), theirs.shape.path().points().collect());
        assert_eq!(ours.len(), theirs.len());
        assert!(ours.iter().zip(&theirs).all(|(a, b)| (a.x() - b.x()).abs() < 1e-6 && (a.y() - b.y()).abs() < 1e-6), "{ours:?} are not {theirs:?}");
    }
}

const NOTE: &str = "info SC-I1116: The text `vc:7` is sewn in the embroidery font `small_font`, 8.9 mm tall: 0.7 of its font size, which is about how \
                    tall a typeface draws its capitals. Its own typeface, Source Sans 3, can't be read yet, so the letters are the embroidery font's, \
                    and as wide as it draws them: its longest line is 10.7 mm long. [To sew the letter shapes of its own typeface, convert the text to paths in the drawing program.]";

const NOT_STITCHED: &str = "warning SC-W0807: Text \"Title\" is not stitched. [Give it StitchCraft's Lettering effect to sew it in an embroidery font, \
                            or make paths of it with Type › Create Outlines.]";

#[test]
fn req_vc_029_point_type_sews_as_lettering_at_its_size_and_place_under_auto() {
    // 72 points right and 144 down: an inch and 2. 36 points are 12.7 mm, whose 0.7 is 8.89 mm: 88.9 % of
    // the font's 10 mm.
    let bytes = document(json!([text(7, "ab", 72.0, 144.0, json!({}), &[])]));
    let read = self::read(&bytes, AUTO);
    assert_eq!(said(&read.warnings), [NOTE]);
    // `a` is 4 mm wide and 10 tall in the font, and starts where its line does.
    assert_eq!(points(&read, 0), [(2540, 5080 - 889), (2540 + 356, 5080)]);
    same(&read, &expected(vec![line("ab", 25.4, 50.8, 12.7)], (1.0, 0.0)));
    let first = &read.design.elements()[0];
    assert_eq!((first.id.as_str(), first.name.as_deref(), first.thread.color), ("vc:7:1:0-svg:pa:stroke", Some("Title"), Rgb::new(255, 0, 0)));
    // Without `auto` it is left out, as before.
    let read = self::read(&bytes, Reading::default());
    assert!(read.design.elements().is_empty());
    assert_eq!(said(&read.warnings), [NOT_STITCHED]);
}

#[test]
fn req_vc_029_the_auto_effect_reaches_a_text_as_it_reaches_a_path() {
    let sews = |objects: Value| {
        let read = read(&document(objects), Reading::default());
        (read.design.elements().len(), said(&read.warnings))
    };
    // Its own Auto effect, or a group's around it.
    assert_eq!(sews(json!([text(7, "ab", 72.0, 144.0, json!({}), &["auto"])])), (2, vec![NOTE.to_string()]));
    assert_eq!(sews(json!([group(3, &["auto"], json!({}), json!([text(7, "ab", 72.0, 144.0, json!({}), &[])]))])), (2, vec![NOTE.to_string()]));
    // Its own Auto effect counts before a stitch effect of a group around it.
    assert_eq!(sews(json!([group(3, &["satin"], json!({}), json!([text(7, "ab", 72.0, 144.0, json!({}), &["auto"])]))])).0, 2);
    // A stitch effect says how paths sew, and a text has none: with one of its own, or a group's around
    // it, it is left out as before, under `auto` too.
    let under_auto = |objects: Value| {
        let read = read(&document(objects), AUTO);
        (read.design.elements().len(), said(&read.warnings))
    };
    assert_eq!(under_auto(json!([text(7, "ab", 72.0, 144.0, json!({}), &["satin"])])), (0, vec![NOT_STITCHED.to_string()]));
    assert_eq!(
        under_auto(json!([group(3, &["satin"], json!({}), json!([text(7, "ab", 72.0, 144.0, json!({}), &[])]))])),
        (0, vec![NOT_STITCHED.to_string()])
    );
    // A hidden effect is none.
    let mut hidden = text(7, "ab", 72.0, 144.0, json!({}), &["satin"]);
    hidden["appearance"]["effects"][0]["visible"] = json!(false);
    assert_eq!(under_auto(json!([hidden])), (2, vec![NOTE.to_string()]));
}

#[test]
fn req_vc_029_a_text_with_the_lettering_effect_keeps_its_settings_and_is_not_lettered_twice() {
    let lettered = text(7, "ab", 72.0, 144.0, json!({}), &["lettering"]);
    let plain = read(&document(json!([lettered])), Reading::default());
    let auto = read(&document(json!([lettered])), AUTO);
    assert_eq!(plain.design.elements().len(), 2);
    assert_eq!(auto.design.elements(), plain.design.elements());
    // Nothing says an embroidery font stands in: the person chose it.
    assert!(auto.warnings.iter().all(|said| said.code != Code::TextSewnInEmbroideryFont), "{:?}", said(&auto.warnings));
    // Nor with the Auto effect beside the Lettering effect, or on a group around.
    let both = read(&document(json!([text(7, "ab", 72.0, 144.0, json!({}), &["lettering", "auto"])])), Reading::default());
    assert_eq!(both.design.elements(), plain.design.elements());
    let around = read(&document(json!([group(3, &["auto"], json!({}), json!([lettered]))])), Reading::default());
    assert_eq!(around.design.elements(), plain.design.elements());
}

#[test]
fn req_vc_029_lines_are_apart_by_the_leading_aligned_as_the_paragraph_and_turned_and_scaled_by_the_transform() {
    // 3 lines, the second empty: the third is 2 leadings down. Automatic leading is 1.2 times the size.
    let read = self::read(&document(json!([text(7, "ab\r\n \nb ", 72.0, 144.0, json!({}), &[])])), AUTO);
    let lines = vec![line("ab", 25.4, 50.8, 12.7), line("b", 25.4, 50.8 + 2.0 * 1.2 * 12.7, 12.7)];
    same(&read, &expected(lines, (1.0, 0.0)));
    assert_eq!(read.design.elements()[2].id.as_str(), "vc:7:2:0-svg:pb:stroke");
    // A leading of its own, in points.
    let mut led = text(7, "a\nb", 72.0, 144.0, json!({}), &[]);
    led["kind"]["runs"][0]["style"]["leading"] = json!(72);
    let read = self::read(&document(json!([led.clone()])), AUTO);
    same(&read, &expected(vec![line("a", 25.4, 50.8, 12.7), line("b", 25.4, 76.2, 12.7)], (1.0, 0.0)));
    // One that is no length is passed over.
    for none in [json!(0), json!(-5), json!("x")] {
        led["kind"]["runs"][0]["style"]["leading"] = none;
        let read = self::read(&document(json!([led.clone()])), AUTO);
        same(&read, &expected(vec![line("a", 25.4, 50.8, 12.7), line("b", 25.4, 50.8 + 1.2 * 12.7, 12.7)], (1.0, 0.0)));
    }
    // Centred and right-aligned paragraphs anchor each line at its middle and its end.
    for (justify, anchor) in [("Center", Anchor::Middle), ("Right", Anchor::End), ("Left", Anchor::Start), ("JustifyAll", Anchor::Start)] {
        let read = self::read(&document(json!([text(7, "ab", 72.0, 144.0, json!({"para": {"justify": justify}}), &[])])), AUTO);
        same(&read, &expected(vec![Line { anchor, ..line("ab", 25.4, 50.8, 12.7) }], (1.0, 0.0)));
    }
    // A quarter turn clockwise at half the size: the baseline runs down, the lines follow to its left, and
    // the letters are half as tall.
    let read = self::read(&document(json!([text(7, "a\nb", 72.0, 144.0, json!({"xf": [0, 0.5, -0.5, 0, 72, 144]}), &[])])), AUTO);
    let lines = vec![line("a", 25.4, 50.8, 6.35), line("b", 25.4 - 1.2 * 6.35, 50.8, 6.35)];
    same(&read, &expected(lines, (0.0, 1.0)));
    assert!(said(&read.warnings)[0].starts_with("warning SC-W1106: "), "{:?}", said(&read.warnings));
    // Twice the size.
    let read = self::read(&document(json!([text(7, "a", 72.0, 144.0, json!({"xf": [2, 0, 0, 2, 72, 144]}), &[])])), AUTO);
    same(&read, &expected(vec![line("a", 25.4, 50.8, 25.4)], (1.0, 0.0)));
    // A transform that flattens the text across its baseline leaves the size as it is written.
    let read = self::read(&document(json!([text(7, "a", 72.0, 144.0, json!({"xf": [1, 0, 0, 0, 72, 144]}), &[])])), AUTO);
    same(&read, &expected(vec![line("a", 25.4, 50.8, 12.7)], (1.0, 0.0)));
}

#[test]
fn req_vc_029_the_size_the_thread_and_the_typeface_are_those_of_the_first_run_with_characters() {
    let mut runs = text(7, "", 72.0, 144.0, json!({}), &[]);
    let blue = json!({"type": "solid", "color": {"model": "rgb", "r": 0, "g": 0, "b": 1}});
    runs["kind"]["runs"] = json!([
        {"text": "", "style": {"font_family": "Empty", "size": 99, "fill": red()}},
        {"text": "a", "style": {"font_family": " Georgia ", "size": 72, "fill": blue}},
        {"text": "b", "style": {"font_family": "Arial", "size": 36, "fill": red()}}
    ]);
    let read = self::read(&document(json!([runs.clone()])), AUTO);
    assert_eq!(points(&read, 0), [(2540, 5080 - 1778), (2540 + 711, 5080)]);
    assert!(read.design.elements().iter().all(|element| element.thread.color == Rgb::new(0, 0, 255)));
    assert!(said(&read.warnings)[0].contains("17.8 mm tall") && said(&read.warnings)[0].contains("Its own typeface, Georgia, can't"));
    // A size that doesn't read is said, and 12 points is used: 4.23 mm, whose 0.7 the font doesn't sew.
    for size in [json!("big"), json!(0), json!(-3), Value::Null] {
        runs["kind"]["runs"] = json!([{"text": "a", "style": {"size": size, "fill": red()}}]);
        let read = self::read(&document(json!([runs.clone()])), AUTO);
        let said = said(&read.warnings);
        assert_eq!(said[0], "warning SC-W0807: The size of text \"Title\" does not read, and 12 points is used.", "{size}");
        assert!(said[1].starts_with("warning SC-W1106: The scale 29.6 % is outside"), "{said:?}");
        assert!(said[2].contains("5.0 mm tall") && said[2].contains("Its own typeface can't be read yet"), "{said:?}");
    }
    // A text without a run sews nothing and says nothing but that.
    runs["kind"]["runs"] = json!([]);
    let read = self::read(&document(json!([runs])), AUTO);
    assert!(read.design.elements().is_empty());
    assert_eq!(said(&read.warnings), ["warning SC-W0807: The size of text \"Title\" does not read, and 12 points is used."]);
}

#[test]
fn diag_sc_w1111_type_lettering_does_not_set_and_text_without_a_font_are_left_out_under_auto() {
    let hint = "[Write it as point type, a line at a time, or make paths of it with Type › Create Outlines.]";
    for (more, what) in [
        (json!({"kind": {"kind": "area"}}), "area type"),
        (json!({"kind": {"kind": "onpath"}}), "type on a path"),
        (json!({"vertical": true}), "vertical type"),
    ] {
        let read = read(&document(json!([text(7, "ab", 72.0, 144.0, more, &[])])), AUTO);
        assert!(read.design.elements().is_empty(), "{what}");
        assert_eq!(
            said(&read.warnings),
            [format!("warning SC-W1111: Text \"Title\" is {what}, which lettering doesn't set, and is not stitched. {hint}")]
        );
    }
    // A reader without fonts, as `read_with` is.
    let bytes = document(json!([text(7, "ab", 72.0, 144.0, json!({}), &[])]));
    let read = read_with(&bytes, &Budget::DEFAULT, AUTO).unwrap();
    assert!(read.design.elements().is_empty());
    assert_eq!(
        said(&read.warnings),
        [
            "warning SC-W1111: The text `vc:7` is not stitched: no embroidery font is here to sew it in. [For `stitch`, set `STITCHCRAFT_FONTS` to the \
          folder that holds the embroidery fonts' folders. To sew the letter shapes of its own typeface, convert the text to paths in the drawing \
          program.]"
        ]
    );
    assert_eq!(read.warnings[0].code, Code::TextNotLettered);
    // A text moved beyond the design's reach.
    let read = self::read(&document(json!([text(7, "ab", 1e12, 144.0, json!({}), &[])])), AUTO);
    assert!(read.design.elements().is_empty());
    assert_eq!(
        said(&read.warnings),
        ["warning SC-W1111: The text `vc:7` is not stitched: it lies beyond the design's reach. [To sew the letter shapes of its own typeface, \
          convert the text to paths in the drawing program.]"]
    );
    // Lines so far apart that the last has no place at all.
    let mut apart = text(7, &format!("a{}b", "\n".repeat(40)), 72.0, 144.0, json!({}), &[]);
    apart["kind"]["runs"][0]["style"]["leading"] = json!(1e308);
    let read = self::read(&document(json!([apart])), AUTO);
    assert!(read.design.elements().is_empty());
    assert_eq!(
        said(&read.warnings),
        ["warning SC-W1111: Text \"Title\" is not stitched: it lies beyond the design's reach. [Move it onto the artboard.]"]
    );
}

#[test]
fn req_vc_029_the_lettering_is_cut_to_the_clips_around_it_and_inline_graphics_are_said() {
    let clipping = json!({"id": 4, "appearance": {"items": []}, "kind": {"type": "path", "clipping": true,
        "path": {"subpaths": [{"closed": true, "anchors": [{"p": [0, 0]}, {"p": [200, 0]}, {"p": [200, 200]}, {"p": [0, 200]}]}]}}});
    let clipped = group(3, &[], json!({"clip": true}), json!([clipping, text(7, "ab", 72.0, 144.0, json!({}), &[])]));
    let read = self::read(&document(json!([clipped])), AUTO);
    assert_eq!(read.design.elements().len(), 2);
    assert!(read.design.elements().iter().all(|element| element.clips.len() == 1));
    // An inline graphic is a character lettering has no glyph for: it is said and left out.
    let read = self::read(&document(json!([text(7, "a\u{FFFC}b", 72.0, 144.0, json!({}), &[])])), AUTO);
    same(&read, &expected(vec![line("ab", 25.4, 50.8, 12.7)], (1.0, 0.0)));
    assert_eq!(said(&read.warnings)[0], "warning SC-W0807: An inline graphic of text \"Title\" is not stitched.");
}

/// The test fonts, read whatever the budget, so that a case meters the document's reader alone.
struct Unmetered(BuiltIn);

impl Fonts for Unmetered {
    fn names(&self) -> Vec<String> {
        self.0.names()
    }

    fn font(&mut self, name: &str, _budget: &Budget) -> Option<Result<Rc<Font>, Diagnostic>> {
        self.0.font(name, &Budget::DEFAULT)
    }
}

#[test]
fn req_vc_029_reading_a_text_is_bounded() {
    let clipping = json!({"id": 4, "appearance": {"items": []}, "kind": {"type": "path", "clipping": true,
        "path": {"subpaths": [{"closed": true, "anchors": [{"p": [0, 0]}, {"p": [200, 0]}, {"p": [200, 200]}, {"p": [0, 200]}]}]}}});
    let with = |objects: Value| {
        let bytes = document(objects);
        let mut needed = 0;
        loop {
            match read_in(&bytes, &Budget { max_work: needed, ..Budget::DEFAULT }, AUTO, &mut Unmetered(fonts())) {
                Ok(read) => break (needed, read.design.elements().len()),
                Err(stopped) => assert_eq!(stopped.code, Code::BudgetExhausted, "at {needed}"),
            }
            needed += 1;
        }
    };
    let (one, sewn) = with(json!([text(7, "a", 72.0, 144.0, json!({}), &[])]));
    assert_eq!(sewn, 1);
    // A line more costs more: the reader a unit for the line and one for each of its bytes, and lettering
    // its own for the line and its letter, measured and placed.
    let (two, sewn) = with(json!([text(7, "a\na", 72.0, 144.0, json!({}), &[])]));
    assert!(two > one && sewn == 2, "{two} {one}");
    // Inside a clip, each letter is charged what holding the clip costs.
    let (clipped, sewn) = with(json!([group(3, &[], json!({"clip": true}), json!([clipping, text(7, "a\na", 72.0, 144.0, json!({}), &[])]))]));
    assert!(clipped > two && sewn == 2, "{clipped} {two}");
}

#[test]
fn req_vc_030_bold_point_type_sews_in_the_bold_font_held_where_it_sews_at_its_size() {
    // Both folders hold the test font. The table says the bold font sews from 12.51 mm: 36 points sew 8.89 mm
    // tall, and 72 points 17.78 mm.
    let files: Vec<(&'static str, &'static [u8])> = vec![("font.json", kept(SETTINGS.as_bytes().to_vec())), ("ltr.svg", kept(glyphs().into_bytes()))];
    let files = kept(files);
    let both = || BuiltIn::new(kept(vec![Folder { name: "small_font", files }, Folder { name: "barstitch_bold", files }]));
    let styled = |points: f64, style: Value| {
        let mut object = text(7, "ab", 72.0, 144.0, json!({}), &[]);
        object["kind"]["runs"][0]["style"]["size"] = json!(points);
        for (key, value) in style.as_object().unwrap() {
            object["kind"]["runs"][0]["style"][key] = value.clone();
        }
        let read = read_in(&document(json!([object])), &Budget::DEFAULT, AUTO, &mut both()).unwrap();
        said(&read.warnings).into_iter().filter(|said| said.contains("SC-I1118")).collect::<Vec<_>>()
    };
    let chosen = styled(72.0, json!({"font_style": "Bold"}));
    assert_eq!(chosen.len(), 1, "{chosen:?}");
    assert!(chosen[0].contains("is sewn in the bold embroidery font `barstitch_bold`"), "{}", chosen[0]);
    assert!(styled(72.0, json!({"fontAxes": {"wght": 700}}))[0].contains("`barstitch_bold`, with strokes 0.19"));
    let small = styled(36.0, json!({"font_style": "Bold Italic"}));
    assert!(small[0].contains("its letters sew 8.9 mm tall and `barstitch_bold` sews capital letters from 12.5 to 32.5 mm"), "{}", small[0]);
    assert!(styled(72.0, json!({"font_style": "SemiBold"})).is_empty());
    assert!(styled(72.0, json!({"fontAxes": {"wght": 699}})).is_empty());
    assert!(styled(72.0, json!({})).is_empty());
}
