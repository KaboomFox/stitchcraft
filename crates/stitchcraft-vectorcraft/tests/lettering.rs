//! Lettering in a VectorCraft document (roadmap step M14.4, `REQ-VC-026`): a text object with StitchCraft's
//! Lettering effect sews as lettering sets its text, in a font drawn in code. No font file of anyone's is
//! used.

// Test code may unwrap and index (clippy.toml allows it in tests). Its helpers take the JSON a case writes in
// place, and round a coordinate that is a few millimetres.
#![allow(clippy::unwrap_used, clippy::needless_pass_by_value, clippy::cast_possible_truncation)]

use std::rc::Rc;

use serde_json::{Value, json};
use stitchcraft_core::{Budget, Code, Diagnostic, Fix};
use stitchcraft_engine::design::{Reading, Shape};
use stitchcraft_plan::Rgb;
use stitchcraft_svg::font;
use stitchcraft_svg::font::Font;
use stitchcraft_svg::lettering::{self, Align, Lettering};
use stitchcraft_vectorcraft::lettering::{BuiltIn, Folder, Fonts};
use stitchcraft_vectorcraft::{VectorCraft, effect_id, read_in, read_with};

/// A glyph file, 100 mm square with a user unit a millimetre and its baseline 70 mm down. `a` is a stroke
/// from (1, 60) to (5, 70) and `b` one from (2, 60) to (8, 70), each in the colour given.
fn glyphs(a: &str, b: &str) -> String {
    let glyph = |label: &str, id: &str, left: f64, right: f64, colour: &str| {
        format!(
            r#"<g inkscape:groupmode="layer" inkscape:label="GlyphLayer-{label}"><path id="{id}" d="M {left},60 L {right},70" style="fill:none;stroke:{colour};stroke-width:0.4"/></g>"#
        )
    };
    format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" xmlns:inkscape="http://www.inkscape.org/namespaces/inkscape" xmlns:sodipodi="http://sodipodi.sourceforge.net/DTD/sodipodi-0.dtd" width="100mm" height="100mm" viewBox="0 0 100 100"><sodipodi:namedview id="base"><sodipodi:guide position="0,30" orientation="0,1" inkscape:label="baseline"/></sodipodi:namedview>{}{}</svg>"#,
        glyph("a", "pa", 1.0, 5.0, a),
        glyph("b", "pb", 2.0, 8.0, b)
    )
}

/// The settings of the test fonts: 10 mm tall, sewn from 5 to 20 mm.
const SETTINGS: &str = r#"{"name": "Test Font", "size": 10, "min_scale": 0.5, "max_scale": 2, "auto_satin": false, "leading": 75.6}"#;

/// A document of one layer that holds `objects`.
fn document(objects: Value) -> Vec<u8> {
    let layer = json!({"id": 1, "kind": {"type": "layer", "children": objects}});
    serde_json::to_vec(&json!({"format": "vectorcraft", "version": 4, "document": {"layers": [layer]}})).unwrap()
}

/// The text object `id` of `text` at (`x`, `y`) points, filled red, with `more` fields of its kind and the
/// Lettering effect with `params`.
fn text(id: u64, text: &str, x: f64, y: f64, more: Value, params: Value) -> Value {
    let red = json!({"type": "solid", "color": {"model": "rgb", "r": 1, "g": 0, "b": 0}});
    let mut kind = json!({"type": "text", "kind": {"kind": "point"}, "xf": [1, 0, 0, 1, x, y],
        "runs": [{"text": text, "style": {"font_family": "Source Sans 3", "size": 12, "fill": red}}]});
    for (key, value) in more.as_object().unwrap() {
        kind[key] = value.clone();
    }
    json!({"id": id, "name": "Title", "appearance": {"effects": [{"id": effect_id("lettering"), "params": params, "visible": true}]}, "kind": kind})
}

/// `values` kept for as long as the tests run, as a plug-in's fonts are kept in it.
fn kept<T>(values: Vec<T>) -> &'static [T] {
    Box::leak(values.into_boxed_slice())
}

/// The folder `name` of a font with the settings `json` and, when it draws any, its glyphs' file `ltr`.
fn folder(name: &'static str, json: &str, ltr: Option<String>) -> Folder {
    let mut files: Vec<(&'static str, &'static [u8])> = vec![("font.json", kept(json.as_bytes().to_vec()))];
    files.extend(ltr.map(|glyphs| ("ltr.svg", kept(glyphs.into_bytes()))));
    Folder { name, files: kept(files) }
}

/// The test font, black; a font of 2 colours, one of whose settings doesn't read and another isn't one; and a
/// font that draws nothing.
fn fonts() -> BuiltIn {
    let two = r#"{"name": "Two", "size": "big", "x": 1}"#;
    BuiltIn::new(kept(vec![
        folder("test", SETTINGS, Some(glyphs("#000000", "#000000"))),
        folder("two", two, Some(glyphs("#000000", "#0000ff"))),
        folder("broken", "{}", None),
    ]))
}

/// `bytes` read with [`fonts`].
fn read(bytes: &[u8]) -> VectorCraft {
    read_in(bytes, &Budget::DEFAULT, Reading::default(), &mut fonts()).unwrap()
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

/// The least work `bytes` reads within, with the fonts read whatever the budget.
fn least_work(bytes: &[u8]) -> u64 {
    let reads = |work: &u64| read_in(bytes, &Budget { max_work: *work, ..Budget::DEFAULT }, Reading::default(), &mut Unmetered(fonts())).is_ok();
    (0..2_000).find(reads).unwrap()
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

/// The points of the element `index` of `read`, in hundredths of a millimetre.
fn points(read: &VectorCraft, index: usize) -> Vec<(i64, i64)> {
    let round = |v: f64| (v * 100.0).round() as i64;
    read.design.elements()[index].shape.path().points().map(|p| (round(p.x()), round(p.y()))).collect()
}

#[test]
fn req_vc_026_a_text_with_the_lettering_effect_sews_as_lettering_sets_it() {
    // 72 points right and 144 down: an inch and 2.
    let read = read(&document(json!([text(
        7,
        "ab\nb",
        72.0,
        144.0,
        json!({}),
        json!({"font": "Test Font", "scale": 150, "text_align": "center", "back_and_forth": false})
    )])));
    assert_eq!(said(&read.warnings), Vec::<String>::new());
    let plain = glyphs("#000000", "#000000");
    let font = font::read("test", &[("font.json", SETTINGS.as_bytes()), ("ltr.svg", plain.as_bytes())], &Budget::DEFAULT).unwrap();
    let how = Lettering { scale: 150.0, align: Align::Centre, back_and_forth: false, ..Lettering::default() };
    let set = lettering::set(&font, "ab\nb", &how, &Budget::DEFAULT).unwrap();
    let moved = set.design.moved([25.4, 50.8]).unwrap();
    assert_eq!(read.design.elements().len(), 3);
    for (ours, theirs) in read.design.elements().iter().zip(moved.elements()) {
        // Each element is lettering's, moved to the text's origin, named as its object, in the text's red.
        assert_eq!(ours.shape, theirs.shape);
        assert_eq!(ours.params, theirs.params);
        assert_eq!(ours.id.to_string(), format!("vc:7:{}", theirs.id));
        assert_eq!(ours.name.as_deref(), Some("Title"));
        assert_eq!(ours.thread.color, Rgb::new(255, 0, 0));
        assert!(matches!(ours.shape, Shape::Stroke { .. }));
    }
    // The settings every lettering has are the effect's: read by name, each one.
    let params = json!({"font": "test", "scale": 100, "letter_spacing": 2, "word_spacing": 3, "line_height": 4, "text_align": "left",
        "back_and_forth": false, "trim": "word", "color_sort": "off", "widen_percent": 20, "_context": {"mode": "effect"}, "preview_level": 1});
    let spaced = self::read(&document(json!([text(7, "ab a\nb", 0.0, 0.0, json!({}), params)])));
    assert_eq!(said(&spaced.warnings), Vec::<String>::new());
    let how = Lettering {
        letter_spacing: 2.0,
        word_spacing: 3.0,
        line_height: 4.0,
        back_and_forth: false,
        trim: lettering::Trim::Word,
        widen_percent: 20.0,
        ..Lettering::default()
    };
    let set = lettering::set(&font, "ab a\nb", &how, &Budget::DEFAULT).unwrap();
    assert_eq!(spaced.design.elements().len(), 4);
    for (ours, theirs) in spaced.design.elements().iter().zip(set.design.elements()) {
        assert_eq!((&ours.shape, &ours.params), (&theirs.shape, &theirs.params));
    }
}

#[test]
fn req_vc_026_a_text_sews_turned_as_its_baseline_and_in_its_font_s_colours() {
    // A quarter turn clockwise, at twice the size: the baseline runs down, and the scale isn't used.
    let turned = read(&document(json!([text(7, "a", 72.0, 0.0, json!({"xf": [0, 2, -2, 0, 72, 0]}), json!({"font": "test"}))])));
    assert_eq!(said(&turned.warnings), Vec::<String>::new());
    // `a` runs from (0, -10) to (4, 0) as set: turned, from (10, 0) to (0, 4), then an inch right.
    assert_eq!(points(&turned, 0), [(3540, 0), (2540, 400)]);
    // A sixth of a turn at twice the size, and half a turn: each sews turned, and nothing is said.
    let (cos, sin) = (0.5_f64, 0.75_f64.sqrt());
    let sixth =
        read(&document(json!([text(7, "a", 0.0, 0.0, json!({"xf": [2.0 * cos, 2.0 * sin, -2.0 * sin, 2.0 * cos, 0, 0]}), json!({"font": "test"}))])));
    assert_eq!(said(&sixth.warnings), Vec::<String>::new());
    assert_eq!(points(&sixth, 0), [(866, -500), (200, 346)]);
    let half = read(&document(json!([text(7, "a", 0.0, 0.0, json!({"xf": [-1, 0, 0, -1, 0, 0]}), json!({"font": "test"}))])));
    assert_eq!((said(&half.warnings), points(&half, 0)), (Vec::new(), vec![(0, 1000), (-400, 0)]));
    // A thousand times the size, its second axis off square by less than a billionth of the size: upright.
    let large = read(&document(json!([text(7, "a", 0.0, 0.0, json!({"xf": [1000, 0, 1e-9, 1000, 0, 0]}), json!({"font": "test"}))])));
    assert_eq!(said(&large.warnings), Vec::<String>::new());
    // Mirrored, it sews turned only, and says so. A transform that holds no number leaves it upright.
    let mirrored = read(&document(json!([text(7, "a", 0.0, 0.0, json!({"xf": [1, 0, 0, -1, 0, 0]}), json!({"font": "test"}))])));
    assert_eq!(
        said(&mirrored.warnings),
        ["warning SC-W0807: Text \"Title\" is mirrored or slanted, which lettering doesn't follow: it is sewn turned as its baseline is."]
    );
    assert_eq!(points(&mirrored, 0), [(0, -1000), (400, 0)]);
    let slanted = read(&document(json!([text(7, "a", 0.0, 0.0, json!({"xf": [1, 0, 0.5, 1, 0, 0]}), json!({"font": "test"}))])));
    assert_eq!(slanted.warnings.len(), 1);
    let bare = read(&document(json!([text(7, "a", 0.0, 0.0, json!({"xf": [null, null, null, null, null, null]}), json!({"font": "test"}))])));
    assert_eq!((said(&bare.warnings), points(&bare, 0)), (Vec::new(), vec![(0, -1000), (400, 0)]));
    let flat = read(&document(json!([text(7, "a", 0.0, 0.0, json!({"xf": [0, 0, 0, 0, 72, 72]}), json!({"font": "test"}))])));
    assert_eq!(points(&flat, 0), [(2540, 1540), (2940, 2540)]);
    assert_eq!(flat.warnings.len(), 1);
    // A font of 2 colours keeps them, and a text without a fill keeps a font's one colour.
    let two = read(&document(json!([text(7, "ab", 0.0, 0.0, json!({}), json!({"font": "Two"}))])));
    let colours: Vec<Rgb> = two.design.elements().iter().map(|element| element.thread.color).collect();
    assert_eq!(colours, [Rgb::new(0, 0, 0), Rgb::new(0, 0, 255)]);
    let unfilled = read(&document(json!([text(7, "a", 0.0, 0.0, json!({"runs": [{"text": "a"}]}), json!({"font": "test"}))])));
    assert_eq!(unfilled.design.elements()[0].thread.color, Rgb::new(0, 0, 0));
    // An inline graphic is left out of the text, and said.
    let inline = read(&document(json!([text(7, "a", 0.0, 0.0, json!({"runs": [{"text": "a\u{FFFC}"}]}), json!({"font": "test"}))])));
    assert_eq!(said(&inline.warnings), ["warning SC-W0807: An inline graphic of text \"Title\" is not stitched."]);
    assert_eq!(inline.design.elements().len(), 1);
}

#[test]
fn req_vc_026_a_text_without_the_effect_is_left_out_and_a_setting_that_does_not_read_is_said() {
    let hint = "[Give it StitchCraft's Lettering effect to sew it in an embroidery font, or make paths of it with Type › Create Outlines.]";
    let mut plain = text(7, "ab", 0.0, 0.0, json!({}), json!({}));
    plain["appearance"]["effects"][0]["visible"] = json!(false);
    let hidden = read(&document(json!([plain.clone()])));
    assert_eq!(said(&hidden.warnings), [format!("warning SC-W0807: Text \"Title\" is not stitched. {hint}")]);
    assert!(hidden.design.elements().is_empty());
    plain["appearance"]["effects"] = json!([{"id": effect_id("fill"), "params": {}, "visible": true}]);
    assert_eq!(read(&document(json!([plain]))).warnings.len(), 1);
    // Without parameters every setting is lettering's default, and the font the default font, which isn't here.
    let bare = read(&document(json!([text(7, "ab", 0.0, 0.0, json!({}), Value::Null)])));
    assert!(said(&bare.warnings)[0].contains("names the font \"small_font\""), "{:?}", said(&bare.warnings));
    let odd = json!({"font": "test", "scale": "large", "text_align": 3, "trim": {}, "weight": 2});
    let read = read(&document(json!([text(7, "ab", 0.0, 0.0, json!({}), odd)])));
    let reset = "[Set the parameter again in the effect's dialog.]";
    assert_eq!(
        said(&read.warnings),
        ["scale", "text_align", "trim", "weight"].map(|key| format!(
            "warning SC-W0807: The parameter \"{key}\" of its StitchCraft Lettering effect does not read, and keeps its default. {reset}"
        ))
    );
    assert_eq!(read.design.elements().len(), 2);
}

#[test]
fn diag_sc_w1111_a_text_whose_font_is_not_here_or_that_lettering_does_not_set_is_left_out() {
    let choose =
        "[Choose a font that is here in the effect's dialog. For `stitch`, set `STITCHCRAFT_FONTS` to the folder that holds the font's folder.]";
    let asked = document(json!([text(7, "ab", 0.0, 0.0, json!({}), json!({"font": " Other "}))]));
    let read = self::read(&asked);
    assert_eq!(
        said(&read.warnings),
        [format!(
            "warning SC-W1111: Text \"Title\" is not stitched: its Lettering effect names the font \"Other\", which is not here. The fonts here are Test Font (`test`), Two (`two`), `broken`. {choose}"
        )]
    );
    assert!(read.design.elements().is_empty());
    // Read without fonts, as `read_with` reads.
    for none in [
        read_with(&asked, &Budget::DEFAULT, Reading::default()).unwrap(),
        read_in(&asked, &Budget::DEFAULT, Reading::default(), &mut BuiltIn::new(&[])).unwrap(),
    ] {
        assert_eq!(
            said(&none.warnings),
            [format!(
                "warning SC-W1111: Text \"Title\" is not stitched: its Lettering effect names the font \"Other\", which is not here. No embroidery font is here. {choose}"
            )]
        );
    }
    let broken = self::read(&document(json!([text(7, "ab", 0.0, 0.0, json!({}), json!({"font": "broken"}))])));
    assert!(said(&broken.warnings)[0].starts_with("warning SC-W1111: Text \"Title\" is not stitched: its font \"broken\" does not read. "));
    assert!(said(&broken.warnings)[0].ends_with("[Choose another font in the Lettering effect's dialog.]"), "{:?}", said(&broken.warnings));
    // Area type, type on a path, vertical type and a kind from a later VectorCraft are not set.
    let point = "[Write it as point type, a line at a time, or make paths of it with Type › Create Outlines.]";
    for (more, what) in [
        (json!({"kind": {"kind": "area", "frame": {"subpaths": []}}}), "area type"),
        (json!({"kind": {"kind": "onpath", "path": {"subpaths": []}, "start": 0}}), "type on a path"),
        (json!({"vertical": true}), "vertical type"),
        (json!({"kind": {"kind": "spiral"}}), "type of the kind \"spiral\""),
    ] {
        let read = self::read(&document(json!([text(7, "ab", 0.0, 0.0, more, json!({"font": "test"}))])));
        assert_eq!(
            said(&read.warnings),
            [format!("warning SC-W1111: Text \"Title\" is {what}, which lettering doesn't set, and is not stitched. {point}")]
        );
        assert!(read.design.elements().is_empty());
    }
    // A text none of whose characters the font draws: lettering's own words.
    let empty = self::read(&document(json!([text(7, "  ", 0.0, 0.0, json!({}), json!({"font": "test"}))])));
    assert!(said(&empty.warnings)[0].starts_with("warning SC-W1111: Text \"Title\" is not stitched: "), "{:?}", said(&empty.warnings));
    assert!(said(&empty.warnings)[0].ends_with("[Write a text with a character the font draws.]"));
    // A font says what its reader said once, however many texts name it.
    let twice = self::read(&document(json!([
        text(7, "a", 0.0, 0.0, json!({}), json!({"font": "two"})),
        text(8, "b", 0.0, 0.0, json!({}), json!({"font": "two"}))
    ])));
    assert_eq!(twice.design.elements().len(), 2);
    let once = self::read(&document(json!([text(7, "a", 0.0, 0.0, json!({}), json!({"font": "two"}))])));
    assert_eq!(twice.warnings, once.warnings);
    assert_eq!(once.warnings.iter().map(|said| said.code).collect::<Vec<_>>(), [Code::FontSettingDefault]);
    // What the reader notes for information isn't passed on: the font read alone says more.
    let coloured = glyphs("#000000", "#0000ff");
    let alone =
        font::read("two", &[("font.json", b"{\"name\": \"Two\", \"size\": \"big\", \"x\": 1}"), ("ltr.svg", coloured.as_bytes())], &Budget::DEFAULT);
    assert!(alone.unwrap().warnings.iter().any(|said| said.code == Code::FontReadAsInkStitch));
    // A message names at most 8 fonts, and counts the others.
    let folders = kept((0..10).map(|i| folder(format!("f{i}").leak(), &format!("{{\"name\": \"Font {i}\"}}"), None)).collect());
    let many = read_in(&asked, &Budget::DEFAULT, Reading::default(), &mut BuiltIn::new(folders)).unwrap();
    let listed =
        "The fonts here are Font 0 (`f0`), Font 1 (`f1`), Font 2 (`f2`), Font 3 (`f3`), Font 4 (`f4`), Font 5 (`f5`), Font 6 (`f6`), Font 7 (`f7`)";
    assert!(many.warnings[0].message.ends_with(&format!("{listed}, and 2 more.")), "{}", many.warnings[0].message);
    let nine = read_in(&asked, &Budget::DEFAULT, Reading::default(), &mut BuiltIn::new(&folders[..9])).unwrap();
    assert!(nine.warnings[0].message.ends_with(&format!("{listed}, and 1 more.")), "{}", nine.warnings[0].message);
    let eight = read_in(&asked, &Budget::DEFAULT, Reading::default(), &mut BuiltIn::new(&folders[..8])).unwrap();
    assert!(eight.warnings[0].message.ends_with(&format!("{listed}.")), "{}", eight.warnings[0].message);
}

#[test]
fn req_vc_026_lettering_is_read_within_the_budget_and_cut_by_the_clips_around_it() {
    let bytes = document(json!([text(7, "ab ab", 0.0, 0.0, json!({}), json!({"font": "test"}))]));
    let whole = read(&bytes);
    assert_eq!(whole.design.elements().len(), 4);
    let mut ran_out = 0;
    for work in 0..400 {
        let budget = Budget { max_work: work, ..Budget::DEFAULT };
        // Out of budget in the document's reader, the font's or lettering, nothing is read: no text is left out.
        match read_in(&bytes, &budget, Reading::default(), &mut fonts()) {
            Ok(read) => assert_eq!((said(&read.warnings), &read.design), (Vec::new(), &whole.design)),
            Err(error) => {
                assert_eq!(error.code, Code::BudgetExhausted);
                ran_out += 1;
            }
        }
    }
    assert!(ran_out > 10, "{ran_out}");
    // A long word costs lettering more than the document's reader, which counts a character and its element
    // where lettering also lays the character out and orders its element. Out of budget in lettering alone,
    // with the fonts read whatever the budget, nothing is read either.
    let long = document(json!([text(7, &"a".repeat(300), 0.0, 0.0, json!({}), json!({"font": "test"}))]));
    let (mut ran_out, mut sewn) = (0, 0);
    for work in (1_000..4_000).step_by(7) {
        let budget = Budget { max_work: work, ..Budget::DEFAULT };
        match read_in(&long, &budget, Reading::default(), &mut Unmetered(fonts())) {
            Ok(lettered) => {
                assert_eq!((said(&lettered.warnings), lettered.design.elements().len()), (Vec::new(), 300));
                sewn += 1;
            }
            Err(error) => {
                assert_eq!(error.code, Code::BudgetExhausted);
                ran_out += 1;
            }
        }
    }
    assert!(ran_out > 0 && sewn > 0, "{ran_out}, {sewn}");
    // In a clip group, the letters are cut by the clipping object, as a path's elements are.
    let square = json!({"subpaths": [{"closed": true, "anchors": [{"p": {"x": 0, "y": -40}}, {"p": {"x": 40, "y": -40}}, {"p": {"x": 40, "y": 0}}, {"p": {"x": 0, "y": 0}}]}]});
    let clip = json!({"id": 3, "kind": {"type": "path", "path": square}});
    let group =
        json!({"id": 2, "kind": {"type": "group", "clip": true, "children": [clip, text(7, "a", 0.0, 0.0, json!({}), json!({"font": "test"}))]}});
    let clipped = read(&document(json!([group])));
    assert_eq!(said(&clipped.warnings), Vec::<String>::new());
    assert_eq!(clipped.design.elements().len(), 1);
    assert_eq!(clipped.design.elements()[0].clips.len(), 1);
    // As a clip group's clipping object, a text is left out, with or without the effect.
    let as_clip = json!({"id": 2, "kind": {"type": "group", "clip": true, "children": [text(7, "a", 0.0, 0.0, json!({}), json!({"font": "test"}))]}});
    assert!(read(&document(json!([as_clip]))).design.elements().is_empty());
}

#[test]
fn req_vc_026_a_text_charges_the_budget_for_its_characters_its_elements_and_the_clips_that_cut_them() {
    // A square the text lies in clips it: 4 segments, which cost each element they cut 5 units.
    let square = json!({"subpaths": [{"closed": true, "anchors": [{"p": {"x": 0, "y": -40}}, {"p": {"x": 40, "y": -40}}, {"p": {"x": 40, "y": 0}}, {"p": {"x": 0, "y": 0}}]}]});
    let red = json!({"kind": "fill", "paint": {"type": "solid", "color": {"model": "rgb", "r": 1, "g": 0, "b": 0}}});
    let before = json!({"id": 9, "appearance": {"items": [red]}, "kind": {"type": "path", "path": square}});
    // A red square is cut by the clip too, before the text, in the document with the text left out as well.
    let clipped = |text: Value| {
        let clip = json!({"id": 3, "kind": {"type": "path", "path": square}});
        document(json!([{"id": 2, "kind": {"type": "group", "clip": true, "children": [clip, before, text]}}]))
    };
    let lettered = |letters: &str| text(7, letters, 0.0, 0.0, json!({}), json!({"font": "test"}));
    let mut bare = lettered("a");
    bare["appearance"]["effects"] = json!([]);
    let without = least_work(&clipped(bare));
    // Measured: the effect's record and its parameter, the text and its character, the letter's element with its
    // 2 points, and the clip's 5 for the element. A charge that changes changes the count.
    assert_eq!(least_work(&clipped(lettered("a"))) - without, 13);
    // A second letter costs its character 1, its element 1 and its 2 points, and the clip's 5 again.
    assert_eq!(least_work(&clipped(lettered("ab"))) - without, 22);
}
