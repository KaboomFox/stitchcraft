//! Text in pictures in a VectorCraft document (roadmap step M14.5, `REQ-VC-028`): what lies inside a path
//! named `stitch:text` sews as lettering of the name's text, in a font drawn in code. No font file of
//! anyone's is used.

// Test code may unwrap, panic and index (clippy.toml allows it in tests, but not in their helpers).
#![allow(clippy::unwrap_used, clippy::panic, clippy::indexing_slicing)]

mod common;

use common::{RED, document, ids};
use stitchcraft_core::units::MM_PER_POINT;
use stitchcraft_core::{Budget, Code};
use stitchcraft_engine::design::Reading;
use stitchcraft_plan::{Rgb, Thread};
use stitchcraft_svg::lettering::pictures::Found;
use stitchcraft_vectorcraft::lettering::{BuiltIn, Folder, Fonts, found};
use stitchcraft_vectorcraft::{VectorCraft, read_in, read_with};

/// A blue line 1 pt wide, as an appearance.
const OUTLINE: &str = r#"{"items": [{"kind": "stroke", "paint": {"type": "solid", "color": {"model": "rgb", "r": 0, "g": 0, "b": 1}}, "width": 1}]}"#;
/// A white fill, as an appearance.
const WHITE: &str = r#"{"items": [{"kind": "fill", "paint": {"type": "solid", "color": {"model": "gray", "k": 1}}}]}"#;

const SETTINGS: &[u8] = br#"{"name": "Test Font", "size": 10, "min_scale": 0.5, "max_scale": 4, "auto_satin": false}"#;
/// A glyph file whose `a` is a stroke 4 mm wide and 10 tall, on its baseline 70 mm down.
const GLYPHS: &[u8] = br##"<svg xmlns="http://www.w3.org/2000/svg" xmlns:inkscape="http://www.inkscape.org/namespaces/inkscape" xmlns:sodipodi="http://sodipodi.sourceforge.net/DTD/sodipodi-0.dtd" width="100mm" height="100mm" viewBox="0 0 100 100"><sodipodi:namedview id="base"><sodipodi:guide position="0,30" orientation="0,1" inkscape:label="baseline"/></sodipodi:namedview><g inkscape:groupmode="layer" inkscape:label="GlyphLayer-a"><path id="pa" d="M 0,60 L 4,70" style="fill:none;stroke:#000000;stroke-width:0.4"/></g></svg>"##;
const FILES: [(&str, &[u8]); 2] = [("font.json", SETTINGS), ("ltr.svg", GLYPHS)];
/// The plug-ins' default font's folder, holding the test font.
const FOLDERS: [Folder; 1] = [Folder { name: "small_font", files: &FILES }];

/// The closed path `id` round the box from (`x0`, `y0`) to (`x1`, `y1`), in millimetres, with `appearance`
/// and, if any, `name`.
fn boxed(id: u32, name: Option<&str>, [x0, y0, x1, y1]: [f64; 4], appearance: &str) -> String {
    let [x0, y0, x1, y1] = [x0, y0, x1, y1].map(|mm| mm / MM_PER_POINT);
    let name = name.map_or_else(String::new, |name| format!("\"name\": {}, ", serde_json::to_string(name).unwrap()));
    format!(
        r#"{{{name}"id": {id}, "appearance": {appearance}, "kind": {{"type": "path", "path": {{"subpaths": [{{"closed": true,
            "anchors": [{{"p": [{x0}, {y0}]}}, {{"p": [{x1}, {y0}]}}, {{"p": [{x1}, {y1}]}}, {{"p": [{x0}, {y1}]}}]}}]}}}}}}"#
    )
}

/// A white field, a red letter on it 8 mm wide and 20 tall from (10, 30), and `marker` round the letter.
fn picture(marker: &str) -> Vec<u8> {
    let objects = [boxed(2, None, [0.0, 0.0, 60.0, 80.0], WHITE), boxed(3, None, [10.0, 30.0, 18.0, 50.0], RED), marker.to_string()];
    document(4, &objects.join(","))
}

fn marked(name: &str) -> Vec<u8> {
    picture(&boxed(4, Some(name), [5.0, 25.0, 40.0, 60.0], OUTLINE))
}

fn said(read: &VectorCraft) -> Vec<String> {
    read.warnings.iter().map(ToString::to_string).collect()
}

#[test]
fn req_vc_028_what_lies_inside_a_path_named_stitch_text_sews_as_lettering() {
    for reading in [Reading::default(), Reading { auto: true, ..Reading::default() }] {
        let read = read_in(&marked("stitch:text a"), &Budget::DEFAULT, reading, &mut BuiltIn::new(&FOLDERS)).unwrap();
        // The traced letter and the marker are gone, and the lettering sews on the field.
        assert_eq!(ids(&read), ["vc:2:fill", "vc:4:stroke:0-svg:pa:stroke"]);
        let letter = &read.design.elements()[1];
        // As tall as the traced letter, where it stood, in its red, and never digitized automatically.
        let points: Vec<(f64, f64)> = letter.shape.path().points().map(|p| (p.x(), p.y())).collect();
        let expected = [(10.0, 30.0), (18.0, 50.0)];
        assert!(points.iter().zip(expected).all(|(a, b)| (a.0 - b.0).abs() < 1e-6 && (a.1 - b.1).abs() < 1e-6) && points.len() == 2, "{points:?}");
        assert_eq!((letter.thread.clone(), letter.automatic), (Thread::new(Rgb::new(255, 0, 0)), false));
        assert_eq!(read.design.elements()[0].automatic, reading.auto);
        assert_eq!(
            said(&read),
            [
                "info SC-I1112: The text marker `vc:4:stroke`: 1 traced outlines, 20.0 mm tall, are replaced by the text \"a\" in the font `small_font`, 20.0 mm tall."
            ]
        );
    }
    // The name is read as every marker's: in any case, after white space.
    let read = read_in(&marked("  Stitch:TEXT a"), &Budget::DEFAULT, Reading::default(), &mut BuiltIn::new(&FOLDERS)).unwrap();
    assert_eq!(ids(&read).len(), 2);
    // A font is named by its folder after the quoted text, as the Lettering effect names one.
    let read = read_in(&marked("stitch:text \"a\" font=SMALL_FONT"), &Budget::DEFAULT, Reading::default(), &mut BuiltIn::new(&FOLDERS)).unwrap();
    assert_eq!(ids(&read), ["vc:2:fill", "vc:4:stroke:0-svg:pa:stroke"]);
}

#[test]
fn req_vc_028_without_the_font_the_traced_shapes_sew_and_the_marker_never_does() {
    // Read without fonts, as a host that has none reads.
    let read = read_with(&marked("stitch:text a"), &Budget::DEFAULT, Reading::default()).unwrap();
    assert_eq!(ids(&read), ["vc:2:fill", "vc:3:fill"]);
    assert_eq!(
        said(&read),
        [
            "warning SC-W1113: The text marker `vc:4:stroke` is not used: The default embroidery font is not here, and no other is. What lies inside it \
          sews as it was traced."
        ]
    );
    let read = read_in(&marked("stitch:text \"a\" font=other"), &Budget::DEFAULT, Reading::default(), &mut BuiltIn::new(&FOLDERS)).unwrap();
    assert_eq!(ids(&read), ["vc:2:fill", "vc:3:fill"]);
    assert!(said(&read)[0].contains("The embroidery font `other` is not here. The fonts here are Test Font (`small_font`)."), "{:?}", said(&read));
    // With too little work allowed, the reading stops with the budget's error.
    let spent = read_in(&marked("stitch:text a"), &Budget { max_work: 60, ..Budget::DEFAULT }, Reading::default(), &mut BuiltIn::new(&FOLDERS));
    assert_eq!(spent.unwrap_err().code, Code::BudgetExhausted);
}

#[test]
fn req_vc_028_a_group_is_never_a_text_marker() {
    let group = format!(
        r#"{{"id": 5, "name": "stitch:text a", "kind": {{"type": "group", "children": [{}]}}}}"#,
        boxed(4, None, [5.0, 25.0, 40.0, 60.0], OUTLINE)
    );
    let read = read_in(&picture(&group), &Budget::DEFAULT, Reading::default(), &mut BuiltIn::new(&FOLDERS)).unwrap();
    assert_eq!(ids(&read), ["vc:2:fill", "vc:3:fill", "vc:4:stroke"]);
    assert_eq!(
        said(&read),
        ["warning SC-W0807: The group \"stitch:text a\" is named as a text marker, which a group can't be, and the name is ignored."]
    );
    assert_eq!(
        read.warnings[0].fix.as_ref().map(|fix| fix.describe()).as_deref(),
        Some("Name a shape drawn round the letters, or a line drawn through them, stitch:text and the text.")
    );
}

#[test]
fn req_vc_028_a_host_asks_its_fonts_for_a_markers_font() {
    let mut fonts = BuiltIn::new(&FOLDERS);
    assert!(matches!(found(&mut fonts, "", &Budget::DEFAULT), Found::Font(font) if font.folder == "small_font"));
    assert!(matches!(found(&mut fonts, "other", &Budget::DEFAULT), Found::Missing(names) if names == ["Test Font (`small_font`)"]));
    const BARE: [Folder; 1] = [Folder { name: "bare", files: &[] }];
    let mut bare = BuiltIn::new(&BARE);
    assert!(matches!(found(&mut bare, "bare", &Budget::DEFAULT), Found::Unread(unread) if unread.code == Code::FontUnreadable));
    assert_eq!(bare.names(), ["`bare`"]);
}
