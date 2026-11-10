//! The font reader (roadmap step M14.1, `REQ-FONT-002` to `REQ-FONT-007`): fonts in Ink/Stitch's format,
//! drawn in code, read from their files. No font file of anyone's is used: the corpus case of a real font is
//! `REQ-FONT-008`'s, in the conformance suite.

// Test code may unwrap and index (clippy.toml allows it in tests).
#![allow(clippy::unwrap_used)]

use proptest::prelude::*;
use stitchcraft_core::units::MM_PER_SVG_PX;
use stitchcraft_core::{Budget, Code, Diagnostic};
use stitchcraft_svg::font::{self, Direction, Font, KEYS, LetterCase, Licence, Settings, TextDirection};

/// A glyph file's root: 100 mm square, a user unit a millimetre.
const ROOT: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" xmlns:inkscape="http://www.inkscape.org/namespaces/inkscape" xmlns:sodipodi="http://sodipodi.sourceforge.net/DTD/sodipodi-0.dtd" xmlns:inkstitch="http://inkstitch.org/namespace" width="100mm" height="100mm" viewBox="0 0 100 100">"#;

/// A line from (10, 20) to (30, 40).
const LINE: &str = r#"<path id="line" d="M 10,20 L 30,40" style="fill:none;stroke:#000000"/>"#;

/// The glyph layer of `label`, hidden as glyph files hide them, holding `body`.
fn layer(label: &str, body: &str) -> String {
    format!(r#"<g inkscape:groupmode="layer" inkscape:label="GlyphLayer-{label}" style="display:none">{body}</g>"#)
}

/// A glyph file holding `body`, with a baseline guide at Inkscape's `y`, up from the page's bottom, or none.
fn file(baseline: Option<f64>, body: &str) -> String {
    let guide = baseline.map(|y| format!(r#"<sodipodi:guide position="0,{y}" orientation="0,1" inkscape:label="baseline"/>"#)).unwrap_or_default();
    format!(r#"{ROOT}<sodipodi:namedview id="base">{guide}</sodipodi:namedview>{body}</svg>"#)
}

/// A glyph file of one glyph, `a`, a line, with its baseline 70 mm down.
fn one() -> String {
    file(Some(30.0), &layer("a", LINE))
}

/// The font of `files`, by path and text, read within `budget`.
fn read_within(files: &[(&str, &str)], budget: &Budget) -> Result<Font, Diagnostic> {
    let files: Vec<(&str, &[u8])> = files.iter().map(|(path, text)| (*path, text.as_bytes())).collect();
    font::read("test", &files, budget)
}

/// The font of `files`, which must read.
fn read(files: &[(&str, &str)]) -> Font {
    read_within(files, &Budget::DEFAULT).unwrap()
}

/// Why the font of `files` is refused.
fn refused(files: &[(&str, &str)]) -> Diagnostic {
    read_within(files, &Budget::DEFAULT).unwrap_err()
}

/// The warnings, as users read them.
fn said(font: &Font) -> Vec<String> {
    font.warnings.iter().map(ToString::to_string).collect()
}

/// The glyph names of `font`'s `direction`.
fn names(font: &Font, direction: Direction) -> Vec<String> {
    font.variant(direction).unwrap().glyphs.keys().cloned().collect()
}

/// `a` and `b` are within a nanometre.
#[track_caller]
fn near(a: f64, b: f64) {
    assert!((a - b).abs() < 1e-6, "{a} is not {b}");
}

#[test]
fn req_font_002_a_font_is_its_settings_and_a_glyph_file_for_each_direction() {
    let font = read(&[("font.json", r#"{"name": "Test"}"#), ("ltr.svg", &one()), ("rtl.svg", &one()), ("preview.png", "")]);
    assert_eq!(font.folder, "test");
    assert_eq!(font.settings.name, "Test");
    let directions: Vec<(Direction, Vec<String>)> = font.variants.iter().map(|v| (v.direction, v.files.clone())).collect();
    assert_eq!(directions, [(Direction::LeftToRight, vec!["ltr.svg".to_string()]), (Direction::RightToLeft, vec!["rtl.svg".to_string()])]);
    assert_eq!(said(&font), Vec::<String>::new());
    // Every direction, in Ink/Stitch's order, by its id and its arrow.
    let all: Vec<(&str, &str, Direction)> = Direction::ALL.iter().map(|d| (d.id(), d.arrow(), d.reversed())).collect();
    assert_eq!(
        all,
        [
            ("ltr", "→", Direction::RightToLeft),
            ("rtl", "←", Direction::LeftToRight),
            ("ttb", "↓", Direction::BottomToTop),
            ("btt", "↑", Direction::TopToBottom)
        ]
    );
    assert_eq!(
        (Direction::named("btt"), Direction::named("↓"), Direction::named("down")),
        (Some(Direction::BottomToTop), Some(Direction::TopToBottom), None)
    );
}

#[test]
fn req_font_002_a_direction_reads_its_file_then_its_folder_s_files_by_name() {
    let glyph = |label: &str| file(Some(30.0), &layer(label, LINE));
    let font = read(&[
        ("font.json", "{}"),
        ("ttb/b.svg", &glyph("b")),
        ("ttb/a.svg", &glyph("a")),
        ("ttb.svg", &glyph("t")),
        // Only the folder's own SVG files.
        ("ttb/deeper/c.svg", &glyph("c")),
        ("ttb/notes.txt", "not a glyph file"),
        ("ttbx.svg", &glyph("x")),
    ]);
    let ttb = font.variant(Direction::TopToBottom).unwrap();
    assert_eq!(ttb.files, ["ttb.svg", "ttb/a.svg", "ttb/b.svg"]);
    assert_eq!(names(&font, Direction::TopToBottom), ["a", "b", "t"]);
    assert_eq!(font.variants.len(), 1);
}

#[test]
fn req_font_002_an_older_font_names_its_files_by_arrows() {
    let font = read(&[("font.json", "{}"), ("→.svg", &one()), ("←.svg", &one()), ("rtl.svg", &file(None, &layer("r", LINE))), ("↓/x.svg", &one())]);
    let files: Vec<(Direction, Vec<String>)> = font.variants.iter().map(|v| (v.direction, v.files.clone())).collect();
    // An arrow's file counts only without the id's, and an arrow's folder not at all, as in Ink/Stitch.
    assert_eq!(files, [(Direction::LeftToRight, vec!["→.svg".to_string()]), (Direction::RightToLeft, vec!["rtl.svg".to_string()])]);
}

#[test]
fn req_font_002_the_licence_is_the_one_the_licence_file_names_first() {
    let licence = |name: &str, text: &str| read(&[("font.json", "{}"), ("ltr.svg", &one()), (name, text)]).licence;
    let ofl = licence("LICENSE", "Copyright 2017. This Font Software is licensed under the SIL Open Font License, Version 1.1.").unwrap();
    assert_eq!((ofl.path.as_str(), ofl.licence), ("LICENSE", Licence::Ofl));
    assert!(ofl.text.starts_with("Copyright 2017."));
    assert_eq!(licence("licence.md", "Licensed CC BY-NC-SA 4.0").map(|l| l.licence), Some(Licence::NonCommercial));
    assert_eq!(licence("README.md", "SIL Open Font License"), None);
    // The first by its path, in any case, and only in the folder itself.
    let font = read(&[("font.json", "{}"), ("ltr.svg", &one()), ("LICENSE", "CC0"), ("License.txt", "GPL"), ("ltr/LICENSE", "OFL")]);
    assert_eq!(font.licence.map(|l| (l.path, l.licence)), Some(("LICENSE".to_string(), Licence::PublicDomain)));
    // Bytes that are no UTF-8 are replaced.
    let glyphs = one();
    let files: [(&str, &[u8]); 3] = [("font.json", b"{}"), ("ltr.svg", glyphs.as_bytes()), ("LICENSE", b"CC BY \xff")];
    let font = font::read("test", &files, &Budget::DEFAULT).unwrap();
    assert_eq!(font.licence.map(|l| (l.licence, l.text)), Some((Licence::CcBy, "CC BY \u{fffd}".to_string())));
}

#[test]
fn req_font_002_font_json_may_start_with_a_byte_order_mark() {
    let font = read(&[("font.json", "\u{feff}{\"size\": 12}"), ("ltr.svg", &one())]);
    assert_eq!(font.settings.size, 12.0);
}

#[test]
fn diag_sc_e1101_a_font_that_can_t_be_read_says_why() {
    let no_json = refused(&[("ltr.svg", &one())]);
    assert_eq!(no_json.to_string(), "error SC-E1101: The font `test` has no `font.json`, which gives a font its settings.");
    assert_eq!(
        no_json.fix.map(|f| f.describe()).as_deref(),
        Some("Ink/Stitch's Extensions › Ink/Stitch › Font Management › Generate JSON writes one.")
    );
    let compressed = refused(&[("font.json.xz", ""), ("ltr.svg", &one())]);
    assert_eq!(compressed.message, "The font `test` has its `font.json` compressed with xz, which StitchCraft doesn't read.");
    assert_eq!(compressed.fix.map(|f| f.describe()).as_deref(), Some("Unpack it with `xz --decompress font.json.xz`."));
    let says = |files: &[(&str, &str)]| {
        let d = refused(files);
        assert_eq!(d.code, Code::FontUnreadable, "{d}");
        d.message
    };
    assert_eq!(
        says(&[("font.json", r#"{"name": "Bad","#), ("ltr.svg", &one())]),
        "The `font.json` of the font `test` doesn't read as JSON: EOF while parsing a value at line 1 column 15."
    );
    assert_eq!(says(&[("font.json", "[1, 2]"), ("ltr.svg", &one())]), "The `font.json` of the font `test` holds a list, not an object of settings.");
    assert_eq!(says(&[("font.json", "3"), ("ltr.svg", &one())]), "The `font.json` of the font `test` holds the number 3, not an object of settings.");
    assert_eq!(
        says(&[("font.json", "{}"), ("preview.png", "")]),
        "The font `test` has no glyph file StitchCraft reads: no `ltr.svg`, `rtl.svg`, `ttb.svg` or `btt.svg`, and no folder of them."
    );
    assert_eq!(
        says(&[("font.json", "{}"), ("ltr.svg", "<svg><g></svg>")]),
        "The glyph file `ltr.svg` of the font `test` doesn't read: The file is not well-formed XML: expected 'g' tag, not 'svg' at 1:9."
    );
    assert_eq!(
        says(&[("font.json", "{}"), ("ltr.svg", r#"<html xmlns="http://www.w3.org/1999/xhtml"/>"#)]),
        "The glyph file `ltr.svg` of the font `test` doesn't read: its root element is <html>, not SVG's <svg>."
    );
    assert_eq!(
        says(&[("font.json", "{}"), ("rtl.svg", &file(None, "").replace(r#"width="100mm""#, r#"width="-1mm""#))]),
        "The glyph file `rtl.svg` of the font `test` doesn't read: The drawing's size cannot be used: its width is zero or negative."
    );
    let gzip: [(&str, &[u8]); 2] = [("font.json", b"{}"), ("ltr.svg", b"\x1f\x8b\x08\x00")];
    assert_eq!(
        font::read("test", &gzip, &Budget::DEFAULT).unwrap_err().message,
        "The glyph file `ltr.svg` of the font `test` doesn't read: The file is compressed SVG (.svgz); StitchCraft reads plain SVG."
    );
    let entities = r#"<!DOCTYPE svg [<!ENTITY e SYSTEM "file:///etc/passwd">]><svg xmlns="http://www.w3.org/2000/svg">&e;</svg>"#;
    assert!(says(&[("font.json", "{}"), ("ltr.svg", entities)]).starts_with("The glyph file `ltr.svg` of the font `test` doesn't read: "));
    // Without a namespace, the root is still SVG's, as the SVG reader takes it, but only an <svg>.
    assert_eq!(read(&[("font.json", "{}"), ("ltr.svg", "<svg/>")]).variants.len(), 1);
    for root in ["<html/>", r#"<svg xmlns="http://www.w3.org/1999/xhtml"/>"#] {
        assert!(says(&[("font.json", "{}"), ("ltr.svg", root)]).ends_with("not SVG's <svg>."), "{root}");
    }
}

#[test]
fn diag_sc_w1103_a_compressed_glyph_file_is_named_and_left_out() {
    let font = read(&[("font.json", "{}"), ("ltr.svg.xz", ""), ("rtl.svg", &one()), ("rtl/more.svg.xz", "")]);
    assert_eq!(
        said(&font),
        [
            "warning SC-W1103: The glyph file `ltr.svg.xz` is compressed with xz, which StitchCraft doesn't read. Its glyphs are left out.",
            "warning SC-W1103: The glyph file `rtl/more.svg.xz` is compressed with xz, which StitchCraft doesn't read. Its glyphs are left out.",
            // Its left-to-right glyphs are left out, so it sews right to left by default.
            "info SC-I1105: `default_variant` is `ltr`, and the font `test` has no glyph file for it. It sews rtl by default, its first direction, as in Ink/Stitch.",
        ]
    );
    assert_eq!(font.warnings[0].fix.as_ref().map(|f| f.describe()).as_deref(), Some("Unpack it with `xz --decompress ltr.svg.xz`."));
    assert_eq!(font.variants.iter().map(|v| v.direction).collect::<Vec<_>>(), [Direction::RightToLeft]);
    // A font of compressed files alone has none to read.
    assert_eq!(refused(&[("font.json", "{}"), ("ltr.svg.xz", "")]).code, Code::FontUnreadable);
}

/// A `font.json` that sets every key to something other than its default.
const EVERY_KEY: &str = r#"{
    "name": "Every Key", "description": "All set.", "description_de": "Alles.", "keywords": ["satin", "tiny"],
    "font_license": "CC BY 4.0", "original_font": "Orig", "original_font_url": "https://example.org/orig",
    "text_direction": "rtl", "default_variant": "←", "letter_case": "upper", "default_glyph": "?",
    "size": 5.5, "min_scale": 0.5, "max_scale": 2, "leading": 120, "horiz_adv_x": {"a": 40, "b": 41.5},
    "horiz_adv_x_default": 45, "horiz_adv_x_space": 17, "kerning_pairs": {"a b": 3, "ab": 4}, "units_per_em": 128,
    "auto_satin": false, "reversible": false, "sortable": true, "combine_at_sort_indices": [0, 2],
    "scale_cross_stitch_pattern": true, "glyphs": ["a", "b"]
}"#;

#[test]
fn req_font_003_every_key_ink_stitch_reads_is_read() {
    let font = read(&[("font.json", EVERY_KEY), ("rtl.svg", &one())]);
    let s = &font.settings;
    assert_eq!(
        (s.name.as_str(), s.description.as_str(), s.descriptions.get("de").map(String::as_str), s.keywords.clone()),
        ("Every Key", "All set.", Some("Alles."), vec!["satin".to_string(), "tiny".to_string()])
    );
    assert_eq!((s.font_license.as_str(), s.original_font.as_str(), s.original_font_url.as_str()), ("CC BY 4.0", "Orig", "https://example.org/orig"));
    assert_eq!(
        (s.text_direction, s.default_variant, s.letter_case, s.default_glyph.as_str()),
        (TextDirection::RightToLeft, Direction::RightToLeft, LetterCase::Upper, "?")
    );
    assert_eq!((s.size, s.min_scale, s.max_scale, s.leading), (5.5, 0.5, 2.0, 120.0));
    assert_eq!(s.horiz_adv_x.iter().map(|(k, v)| (k.as_str(), *v)).collect::<Vec<_>>(), [("a", 40.0), ("b", 41.5)]);
    assert_eq!((s.horiz_adv_x_default, s.horiz_adv_x_space, s.units_per_em), (Some(45.0), 17.0, Some(128.0)));
    assert_eq!(s.kerning_pairs.iter().map(|(k, v)| (k.as_str(), *v)).collect::<Vec<_>>(), [("a b", 3.0), ("ab", 4.0)]);
    assert_eq!((s.auto_satin, s.reversible, s.sortable, s.scale_cross_stitch_pattern), (false, false, true, true));
    assert_eq!((s.combine_at_sort_indices.clone(), s.glyphs.clone()), (vec![0, 2], vec!["a".to_string(), "b".to_string()]));
    assert_eq!(said(&font), Vec::<String>::new());
    // The other words each key takes.
    let font = read(&[("font.json", r#"{"text_direction": "ltr", "letter_case": "lower", "default_variant": "ttb"}"#), ("ttb.svg", &one())]);
    assert_eq!(
        (font.settings.text_direction, font.settings.letter_case, font.settings.default_variant),
        (TextDirection::LeftToRight, LetterCase::Lower, Direction::TopToBottom)
    );
    assert_eq!(read(&[("font.json", r#"{"letter_case": ""}"#), ("ltr.svg", &one())]).settings.letter_case, LetterCase::AsTyped);
    let letter_case = KEYS.iter().find(|key| key.name == "letter_case").unwrap();
    assert_eq!((letter_case.value)(&font.settings), serde_json::Value::from("lower"));
    // Each key's value, as font.json writes it, is the one read: the arrow as the direction it names.
    let settings = read(&[("font.json", EVERY_KEY), ("rtl.svg", &one())]).settings;
    let object: serde_json::Value = serde_json::from_str(EVERY_KEY).unwrap();
    for key in KEYS {
        let given = if key.name == "default_variant" { serde_json::Value::from("rtl") } else { object[key.name].clone() };
        assert_eq!(numbers((key.value)(&settings)), numbers(given), "{}", key.name);
    }
}

/// `value` with each number a float, so that `2` and `2.0` compare equal.
fn numbers(value: serde_json::Value) -> serde_json::Value {
    use serde_json::Value;
    match value {
        Value::Number(n) => n.as_f64().map_or(Value::Null, Value::from),
        Value::Array(list) => Value::Array(list.into_iter().map(numbers).collect()),
        Value::Object(object) => Value::Object(object.into_iter().map(|(k, v)| (k, numbers(v))).collect()),
        other => other,
    }
}

#[test]
fn req_font_003_a_key_font_json_lacks_takes_ink_stitch_s_default() {
    let font = read(&[("font.json", "{}"), ("ltr.svg", &one())]);
    assert_eq!(font.settings, Settings::default());
    let s = Settings::default();
    assert_eq!(
        (s.font_license.as_str(), s.default_glyph.as_str(), s.text_direction, s.default_variant),
        ("SIL Open Font License v1.1", "\u{fffd}", TextDirection::LeftToRight, Direction::LeftToRight)
    );
    assert_eq!((s.size, s.min_scale, s.max_scale, s.leading, s.horiz_adv_x_space), (0.0, 1.0, 1.0, 100.0, 20.0));
    assert_eq!(
        (s.horiz_adv_x_default, s.units_per_em, s.auto_satin, s.reversible, s.sortable, s.scale_cross_stitch_pattern),
        (None, None, true, true, false, false)
    );
    assert_eq!(
        (s.letter_case, s.name.as_str(), s.keywords.len(), s.glyphs.len(), s.combine_at_sort_indices.len()),
        (LetterCase::AsTyped, "", 0, 0, 0)
    );
    // Each key once, and as font.json writes its default.
    let shown: Vec<String> = KEYS.iter().map(|key| format!("{} {}", key.name, font::shown(&(key.value)(&s)))).collect();
    assert_eq!(
        shown,
        [
            "name \"\"",
            "description \"\"",
            "font_license \"SIL Open Font License v1.1\"",
            "default_variant \"ltr\"",
            "text_direction \"ltr\"",
            "keywords []",
            "original_font \"\"",
            "original_font_url \"\"",
            "leading 100",
            "auto_satin true",
            "scale_cross_stitch_pattern false",
            "reversible true",
            "sortable false",
            "combine_at_sort_indices []",
            "letter_case \"\"",
            "default_glyph \"\u{fffd}\"",
            "size 0",
            "min_scale 1",
            "max_scale 1",
            "horiz_adv_x_default null",
            "horiz_adv_x_space 20",
            "units_per_em null",
            "horiz_adv_x {}",
            "kerning_pairs {}",
            "glyphs []",
        ]
    );
    assert!(KEYS.iter().all(|key| !key.takes.is_empty() && key.does.ends_with('.')));
    // A fraction stays, as font.json writes it, and so does a whole number too large for a float.
    assert_eq!(font::shown(&serde_json::Value::from(0.25)), "0.25");
    assert_eq!(font::shown(&serde_json::Value::from(u64::MAX)), "18446744073709551615");
    // Of 2 entries of one name, the later counts.
    assert_eq!(read(&[("font.json", r#"{"size": 1, "size": 2}"#), ("ltr.svg", &one())]).settings.size, 2.0);
}

#[test]
fn diag_sc_w1102_a_setting_of_another_type_keeps_its_default() {
    let json = r#"{"size": "12", "auto_satin": "false", "letter_case": "title", "default_variant": "down",
        "text_direction": 1, "horiz_adv_x_default": [], "keywords": "satin", "name": null, "description_fr": 3,
        "kerning_pairs": {"AV": "x", "VA": 2}, "glyphs": ["a", 1, "b"], "combine_at_sort_indices": [1, "2", 1.5],
        "description": "This description is far too long to be shown in a message as it is."}"#;
    let font = read(&[("font.json", json), ("ltr.svg", &one())]);
    assert_eq!(
        said(&font),
        [
            "warning SC-W1102: `auto_satin` takes `true` or `false`, and `font.json` gives the text \"false\". It is read as true, Ink/Stitch's default.",
            "warning SC-W1102: Entry 2 of `combine_at_sort_indices` is the text \"2\", which isn't an integer. It is left out.",
            "warning SC-W1102: Entry 3 of `combine_at_sort_indices` is the number 1.5, which isn't an integer. It is left out.",
            "warning SC-W1102: `default_variant` takes `ltr`, `rtl`, `ttb` or `btt`, or an older font's `→`, `←`, `↓` or `↑`, and `font.json` gives the text \"down\". It is read as \"ltr\", Ink/Stitch's default.",
            "warning SC-W1102: `description_fr` takes a text, and `font.json` gives the number 3. It is left out.",
            "warning SC-W1102: Entry 2 of `glyphs` is the number 1, which isn't a text. It is left out.",
            "warning SC-W1102: `horiz_adv_x_default` takes a number, or `null`, and `font.json` gives a list. It is read as null, Ink/Stitch's default.",
            "warning SC-W1102: The entry \"AV\" of `kerning_pairs` is the text \"x\", which isn't a number. It is left out.",
            "warning SC-W1102: `keywords` takes a list of texts, and `font.json` gives the text \"satin\". It is read as [], Ink/Stitch's default.",
            "warning SC-W1102: `letter_case` takes `\"\"`, `upper` or `lower`, and `font.json` gives the text \"title\". It is read as \"\", Ink/Stitch's default.",
            "warning SC-W1102: `name` takes a text, and `font.json` gives null. It is read as \"\", Ink/Stitch's default.",
            "warning SC-W1102: `size` takes a number, and `font.json` gives the text \"12\". It is read as 0, Ink/Stitch's default.",
            "warning SC-W1102: `text_direction` takes `ltr` or `rtl`, and `font.json` gives the number 1. It is read as \"ltr\", Ink/Stitch's default.",
        ]
    );
    let s = &font.settings;
    assert_eq!(
        (s.size, s.auto_satin, s.letter_case, s.default_variant, s.text_direction),
        (0.0, true, LetterCase::AsTyped, Direction::LeftToRight, TextDirection::LeftToRight)
    );
    assert_eq!((s.kerning_pairs.len(), s.glyphs.clone(), s.combine_at_sort_indices.clone()), (1, vec!["a".to_string(), "b".to_string()], vec![1]));
    // A long value is described by its type alone: 40 characters at most are shown.
    assert!(s.description.starts_with("This description"));
    let long = |text: &str| said(&read(&[("font.json", &format!(r#"{{"size": "{text}"}}"#)), ("ltr.svg", &one())]))[0].clone();
    assert!(long(&"x".repeat(38)).contains(&format!("gives the text \"{}\". ", "x".repeat(38))));
    assert!(long(&"x".repeat(39)).contains("gives the text. "), "{}", long(&"x".repeat(39)));
    // Objects and flags are described as such.
    let given = |value: &str| said(&read(&[("font.json", &format!(r#"{{"size": {value}}}"#)), ("ltr.svg", &one())]))[0].clone();
    assert!(given("{}").contains("gives an object."));
    assert!(given("true").contains("gives the flag true."));
    // A list or a table given something else keeps its default.
    let first = |json: &str| said(&read(&[("font.json", json), ("ltr.svg", &one())]))[0].clone();
    assert_eq!(
        first(r#"{"combine_at_sort_indices": 2}"#),
        "warning SC-W1102: `combine_at_sort_indices` takes a list of integers, and `font.json` gives the number 2. It is read as [], Ink/Stitch's default."
    );
    assert_eq!(
        first(r#"{"horiz_adv_x": [40]}"#),
        "warning SC-W1102: `horiz_adv_x` takes an object of numbers, and `font.json` gives a list. It is read as {}, Ink/Stitch's default."
    );
    assert_eq!(read(&[("font.json", r#"{"size": 1e2}"#), ("ltr.svg", &one())]).settings.size, 100.0);
}

#[test]
fn diag_sc_i1105_keys_the_format_doesn_t_have_change_nothing() {
    let font = read(&[("font.json", r#"{"rev,ersible": false, "colour": "red", "size": 3}"#), ("ltr.svg", &one())]);
    assert_eq!(
        said(&font),
        ["info SC-I1105: `font.json` has keys Ink/Stitch's lettering doesn't read, which change nothing: `colour`, `rev,ersible`."]
    );
    assert_eq!((font.settings.reversible, font.settings.size), (true, 3.0));
}

#[test]
fn req_font_004_glyphs_are_the_groups_labelled_so_wherever_they_lie() {
    let body = format!(
        r#"{}<g id="script" transform="translate(5,0)">{}<g><g id="deep" inkscape:label="GlyphLayer-c">{LINE}</g></g></g>{}<g inkscape:label="Glyph-d">{LINE}</g><path inkscape:label="GlyphLayer-e" d="M 0,0 L 1,1"/>"#,
        layer("a", LINE),
        layer("b", LINE),
        layer("f", &format!("{LINE}{}", layer("g", r#"<path d="M 60,20 L 70,40" style="fill:none;stroke:#000"/>"#)))
    );
    let font = read(&[("font.json", "{}"), ("ltr.svg", &file(Some(30.0), &body))]);
    assert_eq!(names(&font, Direction::LeftToRight), ["a", "b", "c", "f", "g"]);
    let ltr = font.variant(Direction::LeftToRight).unwrap();
    // The groups round a glyph move it.
    near(ltr.glyphs["b"].left, 15.0);
    near(ltr.glyphs["c"].left, 15.0);
    // A glyph inside a glyph is one of its own, and the paths of both are the outer one's.
    near(ltr.glyphs["g"].left, 60.0);
    near(ltr.glyphs["f"].width, 60.0);
}

#[test]
fn req_font_004_a_glyph_s_name_is_its_label_in_nfc() {
    // The ohm sign is the Greek capital omega in NFC, and an a with a combining diaeresis is one character.
    let font = read(&[("font.json", "{}"), ("ltr.svg", &file(Some(30.0), &(layer("\u{2126}", LINE) + &layer("a\u{308}", LINE))))]);
    assert_eq!(names(&font, Direction::LeftToRight), ["\u{e4}", "\u{3a9}"]);
    let ltr = font.variant(Direction::LeftToRight).unwrap();
    assert_eq!((ltr.glyphs["\u{3a9}"].label.as_str(), ltr.glyphs["\u{e4}"].label.as_str()), ("\u{2126}", "a\u{308}"));
    assert_eq!((ltr.glyphs["\u{3a9}"].name.as_str(), ltr.glyphs["\u{3a9}"].file.as_str()), ("\u{3a9}", "ltr.svg"));
}

#[test]
fn req_font_004_the_later_of_2_glyphs_of_one_name_counts() {
    let far = r#"<path id="far" d="M 50,20 L 60,40" style="fill:none;stroke:#000"/>"#;
    let font = read(&[
        ("font.json", "{}"),
        ("ltr.svg", &file(Some(30.0), &(layer("x", LINE) + &layer("x", far)))),
        ("ltr/more.svg", &file(Some(30.0), &(layer("\u{3a9}", LINE) + &layer("y", LINE)))),
        ("ltr/zz.svg", &file(Some(30.0), &layer("\u{2126}", far))),
    ]);
    assert_eq!(
        said(&font),
        [
            "info SC-I1105: The glyph `x` is drawn twice in the ltr direction, in `ltr.svg` and again in `ltr.svg`. The later counts, as in Ink/Stitch.",
            "info SC-I1105: The glyph `\u{3a9}` is drawn twice in the ltr direction, in `ltr/more.svg` and again in `ltr/zz.svg`. The later counts, as in Ink/Stitch.",
        ]
    );
    let ltr = font.variant(Direction::LeftToRight).unwrap();
    near(ltr.glyphs["x"].left, 50.0);
    assert_eq!((ltr.glyphs["\u{3a9}"].file.as_str(), ltr.glyphs["\u{3a9}"].label.as_str()), ("ltr/zz.svg", "\u{2126}"));
}

#[test]
fn req_font_004_each_glyph_is_a_design_of_its_own_read_by_the_svg_reader() {
    let satin = r#"<path id="column" d="M 0,0 L 10,0 M 0,5 L 10,5" style="fill:none;stroke:#ff0000" inkstitch:satin_column="True"/>"#;
    let old = file(Some(30.0), &(layer("a", LINE) + &layer("b", satin))).replace(
        "</sodipodi:namedview>",
        "</sodipodi:namedview><metadata><inkstitch:inkstitch_svg_version>2</inkstitch:inkstitch_svg_version></metadata>",
    );
    let font = read(&[("font.json", "{}"), ("ltr.svg", &old)]);
    let ltr = font.variant(Direction::LeftToRight).unwrap();
    let ids = |name: &str| ltr.glyphs[name].design.elements().iter().map(|e| e.id.as_str().to_string()).collect::<Vec<_>>();
    assert_eq!((ids("a"), ids("b")), (vec!["svg:line:stroke".to_string()], vec!["svg:column:stroke".to_string()]));
    // The column keeps its settings, and the file's version is updated as Ink/Stitch updates it.
    let column = &ltr.glyphs["b"].design.elements()[0];
    assert_eq!(column.params.get("satin_column").map(ToString::to_string).as_deref(), Some("True"));
    assert_eq!(ltr.glyphs["b"].warnings.iter().map(|w| w.code).collect::<Vec<_>>(), [Code::SvgInkStitchUpdated]);
    // The reader's notes are the glyph's, not the font's.
    assert!(font.warnings.is_empty());
}

#[test]
fn req_font_005_the_box_spans_the_points_a_glyph_s_paths_reach() {
    let glyph = |body: &str| {
        let font = read(&[("font.json", "{}"), ("ltr.svg", &file(Some(30.0), &layer("g", body)))]);
        let glyph = font.variant(Direction::LeftToRight).unwrap().glyphs["g"].clone();
        (glyph.left, glyph.width)
    };
    let stroke = r#"style="fill:none;stroke:#000""#;
    // A curve is bounded where it turns, not by its control points.
    let (left, width) = glyph(&format!(r#"<path d="M 20,0 C 0,0 0,40 20,40" {stroke}/>"#));
    near(left, 5.0);
    near(width, 15.0);
    // A move-to that draws nothing counts, alone or after a subpath.
    assert_eq!(glyph(&format!(r#"<path d="M 12,13" {stroke}/>"#)), (12.0, 0.0));
    assert_eq!(glyph(&format!(r#"<path d="M 10,20 L 30,20 M 50,60" {stroke}/>"#)), (10.0, 40.0));
    // Connectors don't count, and hidden paths do.
    let connector = format!(r##"<path d="M 90,90 L 95,95" inkscape:connection-end="#line" {stroke}/>"##);
    assert_eq!(glyph(&format!("{LINE}{connector}")), (10.0, 20.0));
    assert_eq!(glyph(&format!(r#"{LINE}<path d="M 80,80 L 85,85" inkscape:connection-end="" {stroke}/>"#)), (10.0, 75.0));
    assert_eq!(glyph(&format!(r#"{LINE}<path d="M 80,80 L 85,85" style="display:none;stroke:#000"/>"#)), (10.0, 75.0));
    // Each path's transforms and its groups'.
    assert_eq!(glyph(&format!(r#"<g transform="translate(5,0)"><path d="M 10,20 L 30,40" transform="scale(2)" {stroke}/></g>"#)), (25.0, 40.0));
    // A path whose transform doesn't read counts where it is drawn without it, as in Ink/Stitch's lettering,
    // and one its group flattens at the point it falls to.
    assert_eq!(glyph(&format!(r#"{LINE}<path d="M 80,20 L 90,40" transform="spin(9)" {stroke}/>"#)), (10.0, 80.0));
    assert_eq!(glyph(&format!(r#"{LINE}<g transform="scale(0)"><path d="M 80,20 L 90,40" {stroke}/></g>"#)), (0.0, 30.0));
    // The root's transform doesn't count, as the SVG reader doesn't apply it to the design.
    let moved = file(Some(30.0), &layer("g", LINE)).replace(r#"viewBox="0 0 100 100""#, r#"viewBox="0 0 100 100" transform="translate(50,0)""#);
    let font = read(&[("font.json", "{}"), ("ltr.svg", &moved)]);
    let g = &font.variant(Direction::LeftToRight).unwrap().glyphs["g"];
    assert_eq!((g.left, g.design.elements()[0].shape.path().subpaths[0].start.x()), (10.0, 10.0));
    // Other shapes and clones don't count.
    assert_eq!(glyph(&format!(r##"{LINE}<rect x="0" y="0" width="5" height="5"/><use x="90" y="0" href="#line"/>"##)), (10.0, 20.0));
    // A closed move-to counts as its point.
    assert_eq!(glyph(&format!(r#"<path d="M 10,20 Z" {stroke}/>"#)), (10.0, 0.0));
}

#[test]
fn req_font_005_the_box_is_in_millimetres_on_the_file_s_page() {
    // 1 user unit is a CSS pixel.
    let pixels = file(Some(30.0), &layer("a", LINE)).replace(r#"width="100mm" height="100mm""#, r#"width="100" height="100""#);
    let font = read(&[("font.json", "{}"), ("ltr.svg", &pixels)]);
    let a = &font.variant(Direction::LeftToRight).unwrap().glyphs["a"];
    near(a.left, 10.0 * MM_PER_SVG_PX);
    near(a.width, 20.0 * MM_PER_SVG_PX);
    near(a.baseline, 70.0 * MM_PER_SVG_PX);
    // The box and the design lie in one place.
    let first = a.design.elements()[0].shape.path().subpaths[0].start;
    near(first.x(), a.left);
}

#[test]
fn req_font_005_the_baseline_is_the_first_guide_labelled_so() {
    let baseline = |text: &str| {
        let font = read(&[("font.json", "{}"), ("ltr.svg", text)]);
        (font.variant(Direction::LeftToRight).unwrap().glyphs.get("a").map(|g| g.baseline), said(&font))
    };
    assert_eq!(baseline(&one()), (Some(70.0), vec![]));
    let two = one().replace("</sodipodi:namedview>", r#"<sodipodi:guide position="0,60" inkscape:label="baseline"/></sodipodi:namedview>"#);
    assert_eq!(baseline(&two).0, Some(70.0));
    let other = one().replace(r#"inkscape:label="baseline""#, r#"inkscape:label="caps""#);
    assert_eq!(
        baseline(&other),
        (
            Some(0.0),
            vec![
                "info SC-I1105: The glyph file `ltr.svg` has no guide labelled `baseline`. Its glyphs' baseline is the page's top, as in Ink/Stitch."
                    .to_string()
            ]
        )
    );
    // Parts that read as numbers count, with white space round them, and the guide's x doesn't matter.
    assert_eq!(baseline(&one().replace(r#"position="0,30""#, r#"position=" 12 ,, 30 ""#)).0, Some(70.0));
    // A guide of the second named view, or outside one, is none.
    let elsewhere = one().replace(r#"<sodipodi:namedview id="base">"#, r#"<sodipodi:namedview id="first"/><sodipodi:namedview id="base">"#);
    assert_eq!(baseline(&elsewhere).0, Some(0.0));
    // Without a viewBox, the page is the root's height in pixels tall, and 0 without a height in a unit, as
    // Ink/Stitch measures it.
    let sized = |height: &str| {
        let text = one().replace(r#"width="100mm" height="100mm" viewBox="0 0 100 100""#, &format!(r#"width="100" {height}"#));
        baseline(&text).0.unwrap() / MM_PER_SVG_PX
    };
    near(sized(""), -30.0);
    near(sized(r#"height="100%""#), -30.0);
    near(sized(r#"height="auto""#), -30.0);
    near(sized(r#"height="50mm""#), 50.0 / MM_PER_SVG_PX - 30.0);
    near(sized(r#"height="80""#), 50.0);
    // With a viewBox, its height counts, whatever the root's.
    let text = one().replace(r#"height="100mm" viewBox="0 0 100 100""#, r#"height="50mm" viewBox="0 0 100 100""#);
    near(baseline(&text).0.unwrap(), 35.0);
}

#[test]
fn diag_sc_w1104_a_glyph_without_a_box_is_left_out() {
    let stroke = r#"style="fill:none;stroke:#000""#;
    let body = [
        layer("a", LINE),
        layer("empty", &format!(r#"{LINE}<path d="" {stroke}/>"#)),
        layer("none", &format!(r#"<path {stroke}/>"#)),
        layer("broken", &format!(r#"<path d="L 5 5" {stroke}/>"#)),
        layer("rect", r#"<rect x="0" y="0" width="5" height="5"/>"#),
        layer("far", &format!(r#"<path d="M 1e308,0 L 1e308,1" transform="scale(10)" {stroke}/>"#)),
    ]
    .concat();
    let font = read(&[("font.json", "{}"), ("ltr.svg", &file(Some(30.0), &body))]);
    assert_eq!(
        said(&font),
        [
            "warning SC-W1104: The glyph `empty` in `ltr.svg` has a path that draws nothing, so it has no width. It is left out, as Ink/Stitch's lettering leaves it out.",
            "warning SC-W1104: The glyph `none` in `ltr.svg` has a path that draws nothing, so it has no width. It is left out, as Ink/Stitch's lettering leaves it out.",
            "warning SC-W1104: The glyph `broken` in `ltr.svg` has a path that draws nothing, so it has no width. It is left out, as Ink/Stitch's lettering leaves it out.",
            "warning SC-W1104: The glyph `rect` in `ltr.svg` has no path, so it has no width. It is left out, as Ink/Stitch's lettering leaves it out.",
            "warning SC-W1104: The glyph `far` in `ltr.svg` has a path that draws nothing, so it has no width. It is left out, as Ink/Stitch's lettering leaves it out.",
        ]
    );
    assert_eq!(
        font.warnings[0].fix.as_ref().map(|f| f.describe()).as_deref(),
        Some("Draw the glyph with paths, and delete the paths in its layer that draw nothing.")
    );
    assert_eq!(names(&font, Direction::LeftToRight), ["a"]);
    // A baseline guide whose place doesn't read leaves out every glyph of its file.
    for position in ["0,abc", "30", "1,2,3", "0,inf"] {
        let text = one().replace(r#"position="0,30""#, &format!(r#"position="{position}""#));
        let font = read(&[("font.json", "{}"), ("ltr.svg", &text), ("rtl.svg", &one())]);
        assert_eq!(
            said(&font),
            [format!(
                "warning SC-W1104: The guide labelled `baseline` in the glyph file `ltr.svg` is at \"{position}\", which isn't 2 numbers. Its glyph is left out, as Ink/Stitch's lettering leaves them out."
            )]
        );
        assert_eq!((names(&font, Direction::LeftToRight).len(), names(&font, Direction::RightToLeft)), (0, vec!["a".to_string()]));
    }
    let two = file(Some(30.0), &(layer("a", LINE) + &layer("b", LINE))).replace(r#"position="0,30""#, r#"position="none""#);
    assert!(said(&read(&[("font.json", "{}"), ("ltr.svg", &two)]))[0].contains("Its 2 glyphs are left out"));
}

/// A font with 2 glyphs, `a` and `b`, and the kerning and advances of `json`.
fn spaced(json: &str) -> Font {
    let b = r#"<path id="b" d="M 40,20 L 52,40" style="fill:none;stroke:#000"/>"#;
    read(&[("font.json", json), ("ltr.svg", &file(Some(30.0), &(layer("a", LINE) + &layer("b", b))))])
}

#[test]
fn req_font_006_kerning_is_the_spaced_pair_s_or_else_the_joined_one_s() {
    // `\u0062a` is `b` and `a` joined, escaped because the spelling check takes the pair for a typo.
    let font = spaced(r#"{"kerning_pairs": {"a b": 3, "ab": 4, "\u0062a": 5, "b a ": 6}}"#);
    near(font.kerning_mm("a", "b"), 3.0 * MM_PER_SVG_PX);
    near(font.kerning_mm("b", "a"), 5.0 * MM_PER_SVG_PX);
    assert_eq!((font.kerning_mm("a", "a"), font.kerning_mm("b", "c")), (0.0, 0.0));
}

#[test]
fn req_font_006_an_advance_is_the_glyph_s_own_or_the_font_s_or_its_right_edge() {
    let font = spaced(r#"{"horiz_adv_x": {"a": 40}, "horiz_adv_x_space": 17, "leading": 120, "size": 5, "min_scale": 0.5, "max_scale": 3}"#);
    let ltr = font.variant(Direction::LeftToRight).unwrap();
    near(font.advance_mm(&ltr.glyphs["a"]), 40.0 * MM_PER_SVG_PX);
    // The right edge, in millimetres on the page.
    near(font.advance_mm(&ltr.glyphs["b"]), 52.0);
    near(font.space_mm(), 17.0 * MM_PER_SVG_PX);
    near(font.leading_mm(), 120.0 * MM_PER_SVG_PX);
    assert_eq!(font.heights_mm(), (2.5, 15.0));
    let font = spaced(r#"{"horiz_adv_x_default": 45}"#);
    near(font.advance_mm(&font.variant(Direction::LeftToRight).unwrap().glyphs["b"]), 45.0 * MM_PER_SVG_PX);
}

#[test]
fn req_font_006_the_default_direction_is_default_variant_s_or_the_first() {
    let font = read(&[("font.json", r#"{"default_variant": "rtl"}"#), ("ltr.svg", &one()), ("rtl.svg", &one())]);
    assert_eq!(font.default_variant().map(|v| v.direction), Some(Direction::RightToLeft));
    let font = read(&[("font.json", r#"{"default_variant": "rtl"}"#), ("ttb.svg", &one()), ("btt.svg", &one())]);
    assert_eq!(font.default_variant().map(|v| v.direction), Some(Direction::TopToBottom));
    assert_eq!(
        said(&font),
        [
            "info SC-I1105: `default_variant` is `rtl`, and the font `test` has no glyph file for it. It sews ttb by default, its first direction, as in Ink/Stitch."
        ]
    );
    assert_eq!(font.variant(Direction::LeftToRight), None);
}

#[test]
fn req_font_007_reading_charges_one_meter_for_the_whole_font() {
    let least = |files: &[(&str, &str)]| {
        // The least work that reads the font.
        let (mut low, mut high) = (0_u64, 100_000_u64);
        assert!(read_within(files, &Budget { max_work: high, ..Budget::DEFAULT }).is_ok());
        while low + 1 < high {
            let mid = low + (high - low) / 2;
            if read_within(files, &Budget { max_work: mid, ..Budget::DEFAULT }).is_ok() { high = mid } else { low = mid }
        }
        high
    };
    let one_glyph = least(&[("font.json", "{}"), ("ltr.svg", &one())]);
    let two_files = least(&[("font.json", "{}"), ("ltr.svg", &one()), ("rtl.svg", &one())]);
    let two_glyphs = least(&[("font.json", "{}"), ("ltr.svg", &file(Some(30.0), &(layer("a", LINE) + &layer("b", LINE))))]);
    assert!(two_files > one_glyph && two_glyphs > one_glyph, "{one_glyph} {two_files} {two_glyphs}");
    // A larger font.json costs more.
    let big = format!(r#"{{"description": "{}"}}"#, "x".repeat(4000));
    assert!(least(&[("font.json", &big), ("ltr.svg", &one())]) >= one_glyph + 500);
    let ran_out = read_within(&[("font.json", "{}"), ("ltr.svg", &one())], &Budget { max_work: one_glyph - 1, ..Budget::DEFAULT }).unwrap_err();
    assert_eq!(ran_out.to_string(), format!("error SC-E0004: Reading the font needed more than the work budget of {} units.", one_glyph - 1));
    // Every budget short of it runs out somewhere along the way, and says so.
    for max_work in 0..one_glyph {
        let d = read_within(&[("font.json", "{}"), ("ltr.svg", &one())], &Budget { max_work, ..Budget::DEFAULT }).unwrap_err();
        assert_eq!(d.message, format!("Reading the font needed more than the work budget of {max_work} units."));
    }
    assert_eq!(
        read_within(&[("font.json", &big), ("ltr.svg", &one())], &Budget { max_work: 3, ..Budget::DEFAULT }).unwrap_err().code,
        Code::BudgetExhausted
    );
}

/// Bytes that may be a font's file: noise, or a valid file cut short or with a byte changed.
fn bytes(valid: String) -> impl Strategy<Value = Vec<u8>> {
    let len = valid.len();
    prop_oneof![
        prop::collection::vec(any::<u8>(), 0..64),
        (0..=len).prop_map({
            let valid = valid.clone();
            move |cut| valid.as_bytes()[..cut].to_vec()
        }),
        (0..len.max(1), any::<u8>()).prop_map(move |(at, byte)| {
            let mut bytes = valid.clone().into_bytes();
            if let Some(b) = bytes.get_mut(at) {
                *b = byte;
            }
            bytes
        }),
    ]
}

proptest! {
    #![proptest_config(stitchcraft_testkit::strategies::config(256))]

    /// Whatever the bytes of its files, a font reads or is refused with a code, and never panics.
    #[test]
    fn req_font_007_any_bytes_give_a_font_or_a_coded_error(
        json in bytes(EVERY_KEY.to_string()),
        glyphs in bytes(file(Some(30.0), &(layer("a", LINE) + &layer("b", r#"<path d="M 1,2 C 3,4 5,6 7,8 Z m 1,1" transform="rotate(30)" style="stroke:#000"/>"#)))),
    ) {
        let files: [(&str, &[u8]); 2] = [("font.json", &json), ("rtl/x.svg", &glyphs)];
        match font::read("noise", &files, &Budget::DEFAULT) {
            Ok(font) => {
                for glyph in font.variants.iter().flat_map(|variant| variant.glyphs.values()) {
                    prop_assert!(glyph.left.is_finite() && glyph.width >= 0.0 && glyph.baseline.is_finite());
                }
            }
            Err(d) => prop_assert!(matches!(d.code, Code::FontUnreadable | Code::BudgetExhausted), "{d}"),
        }
    }
}
