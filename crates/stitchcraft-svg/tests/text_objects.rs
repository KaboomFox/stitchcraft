//! Text objects in automatic digitizing (roadmap step M14.7, `REQ-AUTO-069`, `REQ-AUTO-070` and
//! `REQ-SVG-017`): a text object sews as lettering in a font drawn in code, checked on where each letter's
//! points lie, on its thread and on what is said, and an SVG file's `<text>` is read into the lines
//! lettering sets. No font file of anyone's is used.

// Test code may unwrap, panic and index (clippy.toml allows it in tests, but not in their helpers).
#![allow(clippy::unwrap_used, clippy::panic)]

use std::collections::BTreeSet;
use std::rc::Rc;

use stitchcraft_core::units::{MM_PER_SVG_PX, Point};
use stitchcraft_core::{Budget, Code, Diagnostic, ElementId, Fix};
use stitchcraft_engine::design::{
    Clip, ClipShape, Design, DesignSettings, Element, FillRule, Hints, Lettered, Path, Reading, Segment, Shape, Subpath,
};
use stitchcraft_params::ParamSet;
use stitchcraft_plan::{Rgb, Thread};
use stitchcraft_svg::font::{self, Font};
use stitchcraft_svg::lettering::objects::{self, Anchor, CAPITALS, Line, TextObject, Waiting};
use stitchcraft_svg::lettering::pictures::Found;

/// A glyph file's root: 100 mm square, a user unit a millimetre.
const ROOT: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" xmlns:inkscape="http://www.inkscape.org/namespaces/inkscape" xmlns:sodipodi="http://sodipodi.sourceforge.net/DTD/sodipodi-0.dtd" width="100mm" height="100mm" viewBox="0 0 100 100">"#;

/// The glyph `label`: a stroke of `colour` from (0, `top`) to (`width`, `bottom`), the baseline 70 mm down.
fn glyph(label: &str, colour: &str, width: f64, top: f64, bottom: f64) -> String {
    format!(
        r#"<g inkscape:groupmode="layer" inkscape:label="GlyphLayer-{label}" style="display:none"><path id="p{label}" d="M 0,{top} L {width},{bottom}" style="fill:none;stroke:{colour};stroke-width:0.4"/></g>"#
    )
}

/// The test font, its glyphs' layers `layers` and `more` keys in its settings: 10 mm tall, sewn from 5 to
/// 20 mm. Every glyph advances 5 mm and a space 3.
fn font_of(layers: &str, more: &str) -> Rc<Font> {
    font_with(layers, 5.0, more)
}

/// As [`font_of`], every glyph advancing `advance` mm.
fn font_with(layers: &str, advance: f64, more: &str) -> Rc<Font> {
    font_sewn(layers, advance, (0.5, 2.0), more)
}

/// As [`font_with`], sewn from `least` to `most` times its own size.
fn font_sewn(layers: &str, advance: f64, (least, most): (f64, f64), more: &str) -> Rc<Font> {
    let px = |mm: f64| mm / MM_PER_SVG_PX;
    let json = format!(
        r#"{{"name": "Test", "size": 10, "min_scale": {least}, "max_scale": {most}, "auto_satin": false, "horiz_adv_x_default": {}, "horiz_adv_x_space": {}{more}}}"#,
        px(advance),
        px(3.0)
    );
    let file = format!(
        r#"{ROOT}<sodipodi:namedview id="base"><sodipodi:guide position="0,30" orientation="0,1" inkscape:label="baseline"/></sodipodi:namedview>{layers}</svg>"#
    );
    Rc::new(font::read("test", &[("font.json", json.as_bytes()), ("ltr.svg", file.as_bytes())], &Budget::DEFAULT).unwrap())
}

/// The test font: `a` 4 mm wide and `b` 6, each 10 mm tall on the baseline and black, and `c` as `a` in
/// green. It is 10 mm tall, sewn from 5 to 20 mm, and every glyph advances 5 mm.
fn test_font() -> Rc<Font> {
    let layers = [glyph("a", "#000000", 4.0, 60.0, 70.0), glyph("b", "#000000", 6.0, 60.0, 70.0), glyph("c", "#00ff00", 4.0, 60.0, 70.0)];
    font_of(&layers.concat(), "")
}

const RED: Rgb = Rgb::new(200, 0, 0);
const BLUE: Rgb = Rgb::new(0, 0, 200);

fn at(x: f64, y: f64) -> Point {
    Point::new(x, y).unwrap()
}

/// A line of `text` anchored at (`x`, `y`), `size` mm its font size, red.
fn line(text: &str, x: f64, y: f64, size: f64) -> Line {
    Line { text: text.to_string(), at: at(x, y), anchor: Anchor::Start, size, thread: Some(Thread::new(RED)), length: None }
}

/// The text object `svg:t` of `lines`, level.
fn object(lines: Vec<Line>) -> TextObject {
    TextObject {
        label: "svg:t".to_string(),
        name: Some("Title".to_string()),
        typeface: Some("Georgia".to_string()),
        bold: false,
        lines,
        turn: (1.0, 0.0),
        clips: Vec::new(),
    }
}

/// What `text` sews in `font`, and what is said.
fn lettered_in(text: &TextObject, font: &Rc<Font>) -> (Vec<Element>, Vec<String>) {
    let (elements, said) = objects::letter(text, &mut |_, _| Found::Font(Rc::clone(font)), &mut BTreeSet::new(), &Budget::DEFAULT).unwrap();
    (elements, said.iter().map(ToString::to_string).collect())
}

/// What `text` sews in the test font, and what is said.
fn lettered(text: &TextObject) -> (Vec<Element>, Vec<String>) {
    lettered_in(text, &test_font())
}

fn ids(elements: &[Element]) -> Vec<&str> {
    elements.iter().map(|element| element.id.as_str()).collect()
}

/// The points of the element `id`, x and y of each.
fn points(elements: &[Element], id: &str) -> Vec<(f64, f64)> {
    let element = elements.iter().find(|element| element.id.as_str() == id).unwrap_or_else(|| panic!("no {id} in {:?}", ids(elements)));
    element.shape.path().points().map(|p| (p.x(), p.y())).collect()
}

/// `points` are `expected`, each within a nanometre.
#[track_caller]
fn near(points: &[(f64, f64)], expected: &[(f64, f64)]) {
    assert_eq!(points.len(), expected.len(), "{points:?} are not {expected:?}");
    assert!(points.iter().zip(expected).all(|(a, b)| (a.0 - b.0).abs() < 1e-6 && (a.1 - b.1).abs() < 1e-6), "{points:?} are not {expected:?}");
}

const A: &str = "svg:t:1:0-svg:pa:stroke";
const B: &str = "svg:t:1:1-svg:pb:stroke";

/// An automatic area round the box from (`x`, 0) to (`x` + 5, 5).
fn shape(id: &str, x: f64) -> Element {
    let corners = [(x + 5.0, 0.0), (x + 5.0, 5.0), (x, 5.0)].map(|(x, y)| Segment::Line(at(x, y)));
    let path = Path { subpaths: vec![Subpath { start: at(x, 0.0), segments: corners.to_vec(), closed: true }] };
    Element {
        id: ElementId::new(id).unwrap(),
        name: None,
        shape: Shape::Fill { path, rule: FillRule::NonZero, gradient: None },
        thread: Thread::new(BLUE),
        params: ParamSet::new(),
        automatic: true,
        lettered: None,
        hints: Hints::default(),
        clips: Vec::new(),
    }
}

/// The SVG file of `body`, 100 mm square with a user unit a millimetre, read with `auto`.
fn read(body: &str) -> stitchcraft_svg::Svg {
    read_as(body, true)
}

fn read_as(body: &str, auto: bool) -> stitchcraft_svg::Svg {
    let svg = format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" xmlns:inkscape="http://www.inkscape.org/namespaces/inkscape" width="100mm" height="100mm" viewBox="0 0 100 100">{body}</svg>"#
    );
    stitchcraft_svg::read_with(svg.as_bytes(), &Budget::DEFAULT, Reading { auto, ..Reading::default() }).unwrap()
}

/// The lines of the one text object of `body`: each line's text, anchor point, anchor and size.
fn lines(body: &str) -> Vec<(String, (f64, f64), Anchor, f64)> {
    let read = read(body);
    assert_eq!(read.texts.len(), 1, "{:?}", read.warnings);
    let round = |n: f64| (n * 1e6).round() / 1e6;
    read.texts[0].text.lines.iter().map(|line| (line.text.clone(), (round(line.at.x()), round(line.at.y())), line.anchor, round(line.size))).collect()
}

fn said(read: &stitchcraft_svg::Svg) -> Vec<String> {
    read.warnings.iter().map(ToString::to_string).collect()
}

#[test]
fn req_auto_069_a_line_sews_on_its_baseline_from_its_anchor_at_0_7_of_its_font_size() {
    // A font size of 20 mm sews 14 mm tall: 140 % of the font's 10 mm. The letters stand on the baseline at
    // y 30 from x 20, each as far along as the embroidery font advances, 5 mm at its own size.
    assert_eq!(CAPITALS, 0.7);
    let (elements, said) = lettered(&object(vec![line("ab", 20.0, 30.0, 20.0)]));
    assert_eq!(ids(&elements), [A, B]);
    near(&points(&elements, A), &[(20.0, 16.0), (25.6, 30.0)]);
    near(&points(&elements, B), &[(27.0, 16.0), (35.4, 30.0)]);
    // A font of one thread sews in the line's thread, and the elements have the object's name. They keep
    // the settings the font's author gave them, and are not automatic.
    assert!(elements.iter().all(|element| element.thread == Thread::new(RED) && element.name.as_deref() == Some("Title") && !element.automatic));
    // Digitizing placed them, so they are in the cap's reach (`REQ-AUTO-119`).
    assert!(elements.iter().all(|element| element.lettered == Some(Lettered::Digitized)));
    assert_eq!(
        said,
        ["info SC-I1116: The text `svg:t` is sewn in the embroidery font `test`, 14.0 mm tall: 0.7 of its font size, which is about how tall a \
          typeface draws its capitals. Its own typeface, Georgia, can't be read yet, so the letters are the embroidery font's, and as wide as it \
          draws them: its longest line is 15.4 mm long."]
    );
    // A size that needs a scale between tenths of a percent is set at the nearest tenth: 0.7 × 12.34 mm is
    // 86.38 % of the font's 10 mm, set at 86.4 %.
    let (elements, _) = lettered(&object(vec![line("a", 0.0, 0.0, 12.34)]));
    near(&points(&elements, A), &[(0.0, -8.64), (3.456, 0.0)]);
}

#[test]
fn req_auto_069_a_line_starts_is_centred_or_ends_at_its_anchor() {
    // What the 2 letters sew is 15.4 mm wide at 140 %.
    let anchored = |anchor: Anchor| lettered(&object(vec![Line { anchor, ..line("ab", 20.0, 30.0, 20.0) }])).0;
    near(&points(&anchored(Anchor::Start), A), &[(20.0, 16.0), (25.6, 30.0)]);
    let middle = anchored(Anchor::Middle);
    near(&points(&middle, A), &[(12.3, 16.0), (17.9, 30.0)]);
    near(&points(&middle, B), &[(19.3, 16.0), (27.7, 30.0)]);
    let end = anchored(Anchor::End);
    near(&points(&end, A), &[(4.6, 16.0), (10.2, 30.0)]);
    near(&points(&end, B), &[(11.6, 16.0), (20.0, 30.0)]);
    assert_eq!(Anchor::default(), Anchor::Start);
}

#[test]
fn req_auto_069_the_lines_are_turned_as_the_baseline_is_about_each_lines_anchor() {
    // A quarter turn clockwise as a picture shows it: the baseline runs down the page, and the letters'
    // tops lie to its right.
    let turned = TextObject { turn: (0.0, 1.0), ..object(vec![line("ab", 20.0, 30.0, 20.0), line("a", 50.0, 60.0, 20.0)]) };
    let (elements, _) = lettered(&turned);
    near(&points(&elements, A), &[(34.0, 30.0), (20.0, 35.6)]);
    near(&points(&elements, B), &[(34.0, 37.0), (20.0, 45.4)]);
    near(&points(&elements, "svg:t:2:0-svg:pa:stroke"), &[(64.0, 60.0), (50.0, 65.6)]);
    // Turned by the angle whose cosine is 0.6 and whose sine is 0.8.
    let (elements, _) = lettered(&TextObject { turn: (0.6, 0.8), ..object(vec![line("a", 20.0, 30.0, 20.0)]) });
    near(&points(&elements, A), &[(20.0 + 0.8 * 14.0, 30.0 - 0.6 * 14.0), (20.0 + 0.6 * 5.6, 30.0 + 0.8 * 5.6)]);
}

#[test]
fn req_auto_069_each_line_has_its_own_size_and_thread_and_its_letters_are_numbered_by_it() {
    let second = Line { thread: Some(Thread::new(BLUE)), ..line("b", 20.0, 50.0, 10.0) };
    let (elements, said) = lettered(&object(vec![line("ab", 20.0, 30.0, 20.0), second]));
    assert_eq!(ids(&elements), [A, B, "svg:t:2:0-svg:pb:stroke"]);
    near(&points(&elements, "svg:t:2:0-svg:pb:stroke"), &[(20.0, 43.0), (24.2, 50.0)]);
    assert_eq!(elements[2].thread, Thread::new(BLUE));
    assert_eq!(elements[0].thread, Thread::new(RED));
    // The note gives the least and the greatest height.
    assert!(said.len() == 1 && said[0].contains("`test`, 7.0 to 14.0 mm tall:"), "{said:?}");
    // And the length of the longest line, from its first letter's first point to its last letter's last.
    assert!(said[0].ends_with("its longest line is 15.4 mm long."), "{said:?}");
    // The other way round too.
    let (_, said) = lettered(&object(vec![line("b", 20.0, 50.0, 10.0), line("ab", 20.0, 30.0, 20.0)]));
    assert!(said[0].contains("`test`, 7.0 to 14.0 mm tall:") && said[0].ends_with("its longest line is 15.4 mm long."), "{said:?}");
    // A line of one letter is as long as the letter is wide, wherever it stands and however it is anchored.
    let (_, said) = lettered(&object(vec![Line { anchor: Anchor::End, ..line("b", 20.0, 50.0, 10.0) }]));
    assert!(said[0].ends_with("its longest line is 4.2 mm long."), "{said:?}");
}

#[test]
fn req_auto_069_a_font_of_several_threads_keeps_them_and_a_line_without_a_thread_keeps_the_fonts() {
    // `c` is green in the font: a text with it sews in the font's colours.
    let (elements, _) = lettered(&object(vec![line("ac", 20.0, 30.0, 20.0)]));
    let colours: Vec<Rgb> = elements.iter().map(|element| element.thread.color).collect();
    assert_eq!(colours, [Rgb::new(0, 0, 0), Rgb::new(0, 255, 0)]);
    // What lettering warns of is passed on: the font draws no `z`, which is left as a space.
    let (elements, said) = lettered(&object(vec![line("az", 20.0, 30.0, 20.0)]));
    assert_eq!(elements.len(), 1);
    assert_eq!(said.len(), 2, "{said:?}");
    assert!(said[0].starts_with("warning SC-W1107: The font `test` has no glyph for `z` (U+007A)."), "{said:?}");
    assert!(said[1].starts_with("info SC-I1116: "), "{said:?}");
    // A line without a thread of its own sews in the font's.
    let (elements, _) = lettered(&object(vec![Line { thread: None, ..line("ab", 20.0, 30.0, 20.0) }]));
    assert!(elements.iter().all(|element| element.thread.color == Rgb::new(0, 0, 0)));
}

#[test]
fn req_auto_069_a_size_the_font_does_not_sew_at_is_set_at_its_nearest_height_and_said_once() {
    // 0.7 of 100 mm is 70 mm, and the font sews up to 20: 2 lines of that size say so once.
    let (elements, said) = lettered(&object(vec![line("a", 0.0, 0.0, 100.0), line("a", 0.0, 50.0, 100.0)]));
    near(&points(&elements, A), &[(0.0, -20.0), (8.0, 0.0)]);
    assert_eq!(said.len(), 2, "{said:?}");
    assert!(said[0].starts_with("warning SC-W1106: The scale 700 % is outside the range the font `test` sews at, 50 to 200 %"), "{said:?}");
    assert!(said[1].starts_with("info SC-I1116: ") && said[1].contains("`test`, 20.0 mm tall:"), "{said:?}");
    // 0.7 of 2 mm is 1.4 mm, and the font sews from 5. So small a text is longer than its place too.
    let (elements, said) = lettered(&object(vec![line("a", 0.0, 0.0, 2.0)]));
    near(&points(&elements, A), &[(0.0, -5.0), (2.0, 0.0)]);
    assert_eq!(said.len(), 3, "{said:?}");
    assert!(said[0].starts_with("warning SC-W1106: The scale 14 % is outside"), "{said:?}");
    assert!(said[1].starts_with("warning SC-W1117: Line 1 of the text `svg:t` is 2.0 mm long at 5.0 mm"), "{said:?}");
    // The ends of the range are inside it: 50 % and 200 % say nothing.
    for (size, tall) in [(10.0 * 0.5 / 0.7, "5.0"), (10.0 * 2.0 / 0.7, "20.0")] {
        let (_, said) = lettered(&object(vec![line("a", 0.0, 0.0, size)]));
        assert!(said.len() == 1 && said[0].contains(&format!("`test`, {tall} mm tall:")), "{said:?}");
    }
}

#[test]
fn req_auto_069_a_font_whose_settings_give_no_size_is_set_as_it_is_drawn() {
    let file = format!(
        r#"{ROOT}<sodipodi:namedview id="base"><sodipodi:guide position="0,30" orientation="0,1" inkscape:label="baseline"/></sodipodi:namedview>{}</svg>"#,
        glyph("a", "#000000", 4.0, 60.0, 70.0)
    );
    let font = Rc::new(font::read("plain", &[("font.json", br#"{"name": "Plain"}"#), ("ltr.svg", file.as_bytes())], &Budget::DEFAULT).unwrap());
    assert_eq!(font.settings.size, 0.0);
    let (elements, said) = lettered_in(&object(vec![line("a", 20.0, 30.0, 50.0)]), &font);
    near(&points(&elements, A), &[(20.0, 20.0), (24.0, 30.0)]);
    assert_eq!(said.len(), 1, "{said:?}");
    assert_eq!(
        said.last().unwrap(),
        "info SC-I1116: The text `svg:t` is sewn in the embroidery font `plain`, as tall as the font is drawn: its settings give no size to set it \
         at the text's by. The typeface of the text itself can't be read yet."
    );
}

/// A font as the test font, but wide: `a` is 4 mm wide and advances 12 mm, at its size of 10 mm. 4 of them
/// are 40 mm long, and 37 with each gap a tenth of the height narrower.
fn wide_font() -> Rc<Font> {
    font_with(&glyph("a", "#000000", 4.0, 60.0, 70.0), 12.0, "")
}

/// The font size at which a line sews at the test fonts' own size, 10 mm tall.
const OWN: f64 = 10.0 / 0.7;

/// The `n`-th letter of the first line, from 0.
fn nth(n: usize) -> String {
    format!("svg:t:1:{n}-svg:pa:stroke")
}

/// What 4 letters of the wide font sew at the font's own size from (20, 30), their line `length` mm long
/// in the drawing, and what is said.
fn fitted_to(length: f64) -> (Vec<Element>, Vec<String>) {
    lettered_in(&object(vec![Line { length: Some(length), ..line("aaaa", 20.0, 30.0, OWN) }]), &wide_font())
}

#[test]
fn req_auto_069_a_line_no_longer_than_its_place_is_as_it_is_and_is_not_stretched() {
    assert_eq!(objects::WIDTH, 0.5);
    // 40 mm of lettering in a place of 40 mm, and in one of 90: each letter 12 mm after the one before.
    for length in [40.0, 90.0] {
        let (elements, said) = fitted_to(length);
        near(&points(&elements, &nth(0)), &[(20.0, 20.0), (24.0, 30.0)]);
        near(&points(&elements, &nth(1)), &[(32.0, 20.0), (36.0, 30.0)]);
        near(&points(&elements, &nth(3)), &[(56.0, 20.0), (60.0, 30.0)]);
        assert!(said.len() == 1 && said[0].contains("`test`, 10.0 mm tall: 0.7 of its font size, which"), "{said:?}");
        assert!(said[0].ends_with("its longest line is 40.0 mm long."), "{said:?}");
    }
    // One letter that is as long as its place has no gap to narrow, and is as it is.
    let (elements, said) = lettered_in(&object(vec![Line { length: Some(4.0), ..line("a", 20.0, 30.0, OWN) }]), &wide_font());
    near(&points(&elements, A), &[(20.0, 20.0), (24.0, 30.0)]);
    assert_eq!(said.len(), 1, "{said:?}");
    // Without a stated length the place is half the font size for each character: 4 letters of the test
    // font, 5 mm apart, are shorter than that at any size.
    let (elements, said) = lettered(&object(vec![line("abab", 20.0, 30.0, 20.0)]));
    near(&points(&elements, "svg:t:1:3-svg:pb:stroke"), &[(41.0, 16.0), (49.4, 30.0)]);
    assert!(said[0].contains("14.0 mm tall: 0.7 of its font size"), "{said:?}");
}

#[test]
fn req_auto_069_a_line_longer_than_its_place_has_its_gaps_narrowed_first() {
    // 2 mm too long: each of the 3 gaps is 2/3 mm narrower, and the letters are as tall.
    let (elements, said) = fitted_to(38.0);
    near(&points(&elements, &nth(0)), &[(20.0, 20.0), (24.0, 30.0)]);
    near(&points(&elements, &nth(1)), &[(20.0 + 12.0 - 2.0 / 3.0, 20.0), (24.0 + 12.0 - 2.0 / 3.0, 30.0)]);
    near(&points(&elements, &nth(3)), &[(54.0, 20.0), (58.0, 30.0)]);
    // The height is still 0.7 of the font size, and the note says so.
    assert!(said.len() == 1 && said[0].contains("`test`, 10.0 mm tall: 0.7 of its font size, which"), "{said:?}");
    assert!(said[0].ends_with("its longest line is 38.0 mm long."), "{said:?}");
    // 3 mm too long: each gap is a tenth of the height narrower, which is as far as gaps go.
    let (elements, said) = fitted_to(37.0);
    near(&points(&elements, &nth(1)), &[(31.0, 20.0), (35.0, 30.0)]);
    near(&points(&elements, &nth(3)), &[(53.0, 20.0), (57.0, 30.0)]);
    assert!(said.len() == 1 && said[0].contains("`test`, 10.0 mm tall: 0.7 of its font size, which"), "{said:?}");
    // At another height the gaps go as far for that height: at 14 mm, 1.4 mm each, of 56 mm.
    let tall = |length: f64| lettered_in(&object(vec![Line { length: Some(length), ..line("aaaa", 20.0, 30.0, 20.0) }]), &wide_font());
    let (elements, said) = tall(54.0);
    near(&points(&elements, &nth(1)), &[(20.0 + 16.8 - 2.0 / 3.0, 16.0), (25.6 + 16.8 - 2.0 / 3.0, 30.0)]);
    near(&points(&elements, &nth(3)), &[(68.4, 16.0), (74.0, 30.0)]);
    assert!(said[0].contains("`test`, 14.0 mm tall: 0.7 of its font size, which"), "{said:?}");
    // 3 mm too long at 14 mm is within the 4.2 mm its gaps give: they are 1 mm narrower each, as tall.
    let (elements, said) = tall(53.0);
    near(&points(&elements, &nth(3)), &[(67.4, 16.0), (73.0, 30.0)]);
    assert!(said[0].contains("`test`, 14.0 mm tall: 0.7 of its font size, which"), "{said:?}");
    // Words are closed up as letters are. 2 letters, a space of 3 mm and a letter are 31 mm long, and in a
    // place of 30 mm the line is shorter than that, no shorter than its place, and as tall.
    let (elements, said) = lettered_in(&object(vec![Line { length: Some(30.0), ..line("aa a", 20.0, 30.0, OWN) }]), &wide_font());
    let last = points(&elements, ids(&elements)[2]);
    assert!(last[1].0 < 51.0 - 1e-6 && last[1].0 > 50.0 - 1e-6 && (last[0].1 - 20.0).abs() < 1e-9, "{last:?}");
    assert!(said[0].contains("10.0 mm tall: 0.7 of its font size"), "{said:?}");
}

#[test]
fn req_auto_069_a_line_still_longer_is_set_smaller_until_it_fits_and_keeps_its_anchor() {
    // 30 mm for 40 mm of lettering: with its gaps as narrow as they go for its height, the line is 37 mm
    // for every 10 mm of height, so it is set at 30 / 37, cut to a tenth of a percent: 81 %.
    let (elements, said) = fitted_to(30.0);
    near(&points(&elements, &nth(0)), &[(20.0, 30.0 - 8.1), (20.0 + 3.24, 30.0)]);
    near(&points(&elements, &nth(1)), &[(20.0 + 0.81 * 11.0, 30.0 - 8.1), (20.0 + 0.81 * 11.0 + 3.24, 30.0)]);
    near(&points(&elements, &nth(3)), &[(20.0 + 0.81 * 33.0, 30.0 - 8.1), (20.0 + 0.81 * 37.0, 30.0)]);
    assert_eq!(
        said,
        [
            "info SC-I1116: The text `svg:t` is sewn in the embroidery font `test`, 8.1 mm tall: made smaller than 0.7 of its font size to fit the 30.0 mm its line most likely had, which the drawing states. Its own typeface, Georgia, can't be read yet, so the letters are the embroidery font's, and as wide as it draws them: its longest line is 30.0 mm long."
        ]
    );
    // Just under what narrowing the gaps reaches, the line is set a tenth of a percent smaller.
    let (elements, said) = fitted_to(36.99);
    near(&points(&elements, &nth(0)), &[(20.0, 30.0 - 9.99), (20.0 + 3.996, 30.0)]);
    assert!(said[0].contains("made smaller than 0.7 of its font size to fit the 37.0 mm"), "{said:?}");
    // Without a stated length the place is estimated: half of 20 mm for each of 4 characters, 40 mm, where
    // the lettering is 56 mm at 14 mm. It is set at 40 / 37, 108.1 %.
    let (elements, said) = lettered_in(&object(vec![line("aaaa", 20.0, 30.0, 20.0)]), &wide_font());
    near(&points(&elements, &nth(0)), &[(20.0, 30.0 - 10.81), (20.0 + 4.324, 30.0)]);
    near(&points(&elements, &nth(3)), &[(20.0 + 1.081 * 33.0, 30.0 - 10.81), (20.0 + 1.081 * 37.0, 30.0)]);
    assert!(
        said.len() == 1
            && said[0].contains(
                "`test`, 10.8 mm tall: made smaller than 0.7 of its font size to fit the 40.0 mm its line most likely had, estimated at half the font size for each character. Its own"
            ),
        "{said:?}"
    );
    // A centred line stays centred on its anchor, and one that ends there ends there.
    let anchored = |anchor: Anchor| lettered_in(&object(vec![Line { anchor, length: Some(30.0), ..line("aaaa", 20.0, 30.0, OWN) }]), &wide_font()).0;
    let middle = anchored(Anchor::Middle);
    near(&points(&middle, &nth(0)), &[(20.0 - 0.81 * 18.5, 30.0 - 8.1), (20.0 - 0.81 * 14.5, 30.0)]);
    near(&points(&middle, &nth(3)), &[(20.0 + 0.81 * 14.5, 30.0 - 8.1), (20.0 + 0.81 * 18.5, 30.0)]);
    near(&points(&anchored(Anchor::End), &nth(3)), &[(20.0 - 0.81 * 4.0, 30.0 - 8.1), (20.0, 30.0)]);
}

#[test]
fn req_auto_069_a_word_as_long_as_a_badges_stays_inside_the_badge() {
    // The badge drawn for the step: 8 characters at a font size of 18 mm, from 12 mm, on a badge whose
    // edge is 73 mm further. In a font this wide the lettering would be 113.4 mm long at 12.6 mm.
    let (elements, said) = lettered_in(&object(vec![line("aaaa aaa", 12.0, 32.0, 18.0)]), &wide_font());
    let right = elements.iter().flat_map(|element| element.shape.path().points()).map(|p| p.x()).fold(f64::MIN, f64::max);
    assert!(right <= 12.0 + 72.0 && right > 12.0 + 71.0, "{right}");
    assert!(said.len() == 1 && said[0].contains("to fit the 72.0 mm its line most likely had, estimated"), "{said:?}");
}

#[test]
fn req_auto_069_the_note_counts_the_lines_set_smaller_and_says_where_their_lengths_come_from() {
    let stated = |length: f64, y: f64| Line { length: Some(length), ..line("aaaa", 20.0, y, OWN) };
    let note = |lines: Vec<Line>| lettered_in(&object(lines), &wide_font()).1.pop().unwrap();
    // One of 2 lines.
    assert!(
        note(vec![stated(30.0, 30.0), stated(40.0, 50.0)]).contains(
            "`test`, 8.1 to 10.0 mm tall: 1 of its 2 lines made smaller than 0.7 of their font size to fit the length each most likely had, which the drawing states. Its own"
        )
    );
    // Both, one by a stated length and one by an estimated one.
    assert!(
        note(vec![stated(30.0, 30.0), line("aaaa", 20.0, 50.0, OWN)]).contains(
            "`test`, 7.7 to 8.1 mm tall: 2 of its 2 lines made smaller than 0.7 of their font size to fit the length each most likely had, estimated at half the font size for each character. Its own"
        )
    );
    // The one line of 2 that is set smaller.
    assert!(note(vec![stated(40.0, 30.0), stated(30.0, 50.0)]).contains("1 of its 2 lines made smaller"));
}

#[test]
fn diag_sc_w1117_a_line_longer_than_its_place_at_the_fonts_smallest_height_is_said() {
    // The font sews from 5 mm, where the 4 letters are 18.5 mm long with their gaps narrowed. A place of
    // 18.5 mm is fitted at the smallest height, and nothing is said but the note.
    let (elements, said) = fitted_to(18.5);
    near(&points(&elements, &nth(3)), &[(20.0 + 16.5, 25.0), (20.0 + 18.5, 30.0)]);
    assert!(said.len() == 1 && said[0].contains("`test`, 5.0 mm tall: made smaller"), "{said:?}");
    // A shorter place: the line is set at the smallest height all the same, and reaches past it.
    let (elements, said) = fitted_to(18.4);
    near(&points(&elements, &nth(0)), &[(20.0, 25.0), (22.0, 30.0)]);
    near(&points(&elements, &nth(3)), &[(20.0 + 16.5, 25.0), (20.0 + 18.5, 30.0)]);
    assert_eq!(
        said,
        [
            "warning SC-W1117: Line 1 of the text `svg:t` is 18.5 mm long at 5.0 mm, the smallest height the font `test` sews at, where its text most likely took 18.4 mm: it reaches past the text's place.",
            "info SC-I1116: The text `svg:t` is sewn in the embroidery font `test`, 5.0 mm tall: made smaller than 0.7 of its font size to fit the 18.4 mm its line most likely had, which the drawing states. Its own typeface, Georgia, can't be read yet, so the letters are the embroidery font's, and as wide as it draws them: its longest line is 18.5 mm long."
        ]
    );
    // The warning is about the text object, and names the second line as the second.
    let lines = vec![line("a", 20.0, 10.0, 20.0), Line { length: Some(1.0), ..line("aaaa", 20.0, 30.0, OWN) }];
    let (_, said) = objects::letter(&object(lines), &mut |_, _| Found::Font(wide_font()), &mut BTreeSet::new(), &Budget::DEFAULT).unwrap();
    assert_eq!(said[0].code, Code::LetteringLongerThanTextsPlace);
    assert_eq!(said[0].element.as_ref().map(ElementId::as_str), Some("svg:t"));
    assert!(said[0].message.starts_with("Line 2 of the text `svg:t` is 18.5 mm long"), "{}", said[0].message);
    let hint = "Make the text larger or shorter in the drawing, or convert it to paths there.";
    assert_eq!(said[0].fix, Some(Fix::Hint(hint.to_string())));
    // A length that is none leaves the line at the smallest height too.
    let (_, said) = fitted_to(-5.0);
    assert!(said[0].starts_with("warning SC-W1117: ") && said[0].contains("most likely took -5.0 mm"), "{said:?}");
}

#[test]
fn req_auto_069_the_lettering_is_cut_to_the_text_objects_clips_and_keeps_its_own() {
    let corners = [(50.0, 0.0), (50.0, 50.0)].map(|(x, y)| Segment::Line(at(x, y)));
    let path = Path { subpaths: vec![Subpath { start: at(0.0, 0.0), segments: corners.to_vec(), closed: true }] };
    let clip = Clip { shapes: vec![ClipShape { path, rule: FillRule::NonZero }].into() };
    let (elements, _) = lettered(&TextObject { clips: vec![clip.clone()], ..object(vec![line("ab", 20.0, 30.0, 20.0)]) });
    assert!(elements.len() == 2 && elements.iter().all(|element| element.clips == vec![clip.clone()]));
    let (elements, _) = lettered(&object(vec![line("ab", 20.0, 30.0, 20.0)]));
    assert!(elements.iter().all(|element| element.clips.is_empty()));
}

#[test]
fn req_auto_069_a_text_without_lines_sews_nothing_and_nothing_is_said() {
    // A viewer shows nothing of it either. The font isn't asked for.
    let mut asked = 0;
    let none = objects::letter(
        &object(Vec::new()),
        &mut |_, _| {
            asked += 1;
            Found::Missing(Vec::new())
        },
        &mut BTreeSet::new(),
        &Budget::DEFAULT,
    );
    assert_eq!(none.unwrap(), (Vec::new(), Vec::new()));
    assert_eq!(asked, 0);
}

#[test]
fn req_auto_069_the_default_font_is_asked_for_and_its_own_warnings_are_said_once() {
    // A font whose settings hold a key that isn't one warns of it when it is read.
    let font = font_of(&glyph("a", "#000000", 4.0, 60.0, 70.0), r#", "size": "big""#);
    assert!(!font.warnings.is_empty());
    let mut asked = Vec::new();
    let mut fonts = |name: &str, _: &Budget| {
        asked.push(name.to_string());
        Found::Font(Rc::clone(&font))
    };
    let mut told = BTreeSet::new();
    let text = object(vec![line("a", 0.0, 0.0, 10.0)]);
    let (_, first) = objects::letter(&text, &mut fonts, &mut told, &Budget::DEFAULT).unwrap();
    let (_, second) = objects::letter(&text, &mut fonts, &mut told, &Budget::DEFAULT).unwrap();
    assert_eq!(first.len(), font.warnings.len() + 1);
    assert_eq!(first[..font.warnings.len()], font.warnings[..]);
    assert!(second.len() == 1 && second[0].code == Code::TextSewnInEmbroideryFont);
    assert_eq!(asked, ["", ""]);
    assert_eq!(told, BTreeSet::from(["test".to_string()]));
}

#[test]
fn req_auto_069_texts_that_wait_sew_at_their_places_among_the_designs_elements() {
    let font = test_font();
    let design = || {
        let mut design = Design::new(vec![shape("s1", 0.0), shape("s2", 10.0)], DesignSettings::default()).unwrap();
        design.sort_colours = true;
        design
    };
    let text =
        |label: &str, after: usize| Waiting { after, text: TextObject { label: label.to_string(), ..object(vec![line("a", 20.0, 30.0, 20.0)]) } };
    let set = |texts: &[Waiting]| {
        let (design, said) =
            objects::lettered(design(), texts, &mut |_, _| Found::Font(Rc::clone(&font)), &mut BTreeSet::new(), &Budget::DEFAULT).unwrap();
        (design.elements().iter().map(|element| element.id.to_string()).collect::<Vec<_>>(), said, design)
    };
    // Before the first shape, twice between the 2, and after the last, also from further than the end.
    let (order, said, with) = set(&[text("t0", 0), text("t1", 1), text("u1", 1), text("t2", 2), text("t9", 9)]);
    let letter = |label: &str| format!("{label}:1:0-svg:pa:stroke");
    assert_eq!(order, [letter("t0"), "s1".to_string(), letter("t1"), letter("u1"), "s2".to_string(), letter("t2"), letter("t9")]);
    assert_eq!(said.iter().filter(|said| said.code == Code::TextSewnInEmbroideryFont).count(), 5);
    // The design's settings and its order of colours stay.
    assert!(with.sort_colours);
    assert_eq!(with.settings, design().settings);
    // A text whose place is before one already set sews next, not later.
    let (order, _, _) = set(&[text("t1", 1), text("t0", 0)]);
    assert_eq!(order, ["s1".to_string(), letter("t1"), letter("t0"), "s2".to_string()]);
    // 2 texts of one label would sew 2 elements of one id, which is no design: the reader's bug, said as one.
    let twice = [text("t1", 1), text("t1", 2)];
    let failed = objects::lettered(design(), &twice, &mut |_, _| Found::Font(Rc::clone(&font)), &mut BTreeSet::new(), &Budget::DEFAULT);
    assert_eq!(failed.unwrap_err().code, Code::InternalCheckFailed);
    // Without a text the design is as it was.
    let (order, said, with) = set(&[]);
    assert_eq!(order, ["s1", "s2"]);
    assert!(said.is_empty() && with == design());
}

/// What a text of one line says when its fonts are `found`.
fn said_with(found: &Found) -> Vec<String> {
    let text = object(vec![line("ab", 20.0, 30.0, 20.0)]);
    let (elements, said) = objects::letter(&text, &mut |_, _| found.clone(), &mut BTreeSet::new(), &Budget::DEFAULT).unwrap();
    assert!(elements.is_empty());
    said.iter().map(ToString::to_string).collect()
}

const HINT: &str = "For `stitch`, set `STITCHCRAFT_FONTS` to the folder that holds the embroidery fonts' folders. To sew the letter shapes of its own \
                    typeface, convert the text to paths in the drawing program.";

#[test]
fn diag_sc_w1111_a_text_object_without_a_font_sews_nothing_and_says_which_fonts_there_are() {
    assert_eq!(
        said_with(&Found::Missing(Vec::new())),
        ["warning SC-W1111: The text `svg:t` is not stitched: no embroidery font is here to sew it in."]
    );
    let names = |count: usize| (1..=count).map(|n| format!("f{n}")).collect::<Vec<_>>();
    assert_eq!(
        said_with(&Found::Missing(names(8))),
        ["warning SC-W1111: The text `svg:t` is not stitched: the default embroidery font is not here. The fonts here are f1, f2, f3, f4, f5, f6, \
          f7, f8."]
    );
    assert_eq!(
        said_with(&Found::Missing(names(10))),
        ["warning SC-W1111: The text `svg:t` is not stitched: the default embroidery font is not here. The fonts here are f1, f2, f3, f4, f5, f6, \
          f7, f8, and 2 more."]
    );
    let unread = Diagnostic::new(Code::FontUnreadable, "Its `font.json` is missing.");
    assert_eq!(
        said_with(&Found::Unread(unread)),
        ["warning SC-W1111: The text `svg:t` is not stitched: the embroidery font it would sew in does not read. Its `font.json` is missing."]
    );
    // The warning is about the text object, and says what to do.
    let text = object(vec![line("ab", 20.0, 30.0, 20.0)]);
    let (_, said) = objects::letter(&text, &mut |_, _| Found::Missing(Vec::new()), &mut BTreeSet::new(), &Budget::DEFAULT).unwrap();
    assert_eq!(said[0].code, Code::TextNotLettered);
    assert_eq!(said[0].element.as_ref().map(ElementId::as_str), Some("svg:t"));
    assert_eq!(said[0].fix, Some(Fix::Hint(HINT.to_string())));
}

#[test]
fn diag_sc_w1111_a_text_object_one_of_whose_lines_cannot_be_set_sews_nothing() {
    // The font draws no `z` and has no glyph for what it doesn't draw: the first line would sew, and
    // doesn't.
    let (elements, said) = lettered(&object(vec![line("ab", 20.0, 30.0, 20.0), line("z", 20.0, 50.0, 20.0)]));
    assert!(elements.is_empty());
    assert_eq!(said.len(), 1, "{said:?}");
    assert!(
        said[0].starts_with("warning SC-W1111: The text `svg:t` is not stitched: its line 2 is not set. None of the text's characters"),
        "{said:?}"
    );
    // A label too long to number the letters by.
    let long = TextObject { label: format!("svg:{}", "x".repeat(300)), ..object(vec![line("ab", 20.0, 30.0, 20.0)]) };
    let (elements, said) = lettered(&long);
    assert!(elements.is_empty());
    assert!(said.len() == 1 && said[0].ends_with("is not stitched: its name is too long to number its letters by."), "{said:?}");
    // A line so far out that its letters leave the design's reach.
    let reach = stitchcraft_engine::design::WORKING_LIMIT_MM;
    let (elements, said) = lettered(&object(vec![line("ab", reach - 1.0, 30.0, 20.0)]));
    assert!(elements.is_empty());
    assert_eq!(said, ["warning SC-W1111: The text `svg:t` is not stitched: it lies beyond the design's reach."]);
    // Such a text has a font: its hint says only how to sew its own letter shapes.
    let far = object(vec![line("ab", reach - 1.0, 30.0, 20.0)]);
    let (_, said) = objects::letter(&far, &mut |_, _| Found::Font(test_font()), &mut BTreeSet::new(), &Budget::DEFAULT).unwrap();
    let hint = "To sew the letter shapes of its own typeface, convert the text to paths in the drawing program.";
    assert_eq!(said[0].fix, Some(Fix::Hint(hint.to_string())));
    // A line that ends just inside the reach sews: `b` ends 15.4 mm from the line's start.
    let (elements, _) = lettered(&object(vec![line("ab", reach - 15.4, 30.0, 20.0)]));
    assert_eq!(elements.len(), 2);
}

#[test]
fn diag_sc_i1116_the_note_names_the_font_the_height_and_the_typeface_it_stands_in_for() {
    let text = TextObject { typeface: None, ..object(vec![line("ab", 20.0, 30.0, 20.0)]) };
    let (_, said) = objects::letter(&text, &mut |_, _| Found::Font(test_font()), &mut BTreeSet::new(), &Budget::DEFAULT).unwrap();
    assert_eq!(said.len(), 1);
    assert_eq!(said[0].code, Code::TextSewnInEmbroideryFont);
    assert_eq!(said[0].element.as_ref().map(ElementId::as_str), Some("svg:t"));
    assert_eq!(
        said[0].to_string(),
        "info SC-I1116: The text `svg:t` is sewn in the embroidery font `test`, 14.0 mm tall: 0.7 of its font size, which is about how tall a \
         typeface draws its capitals. Its own typeface can't be read yet, so the letters are the embroidery font's, and as wide as it draws them: its longest line is 15.4 mm long."
    );
    let hint = "To sew the letter shapes of its own typeface, convert the text to paths in the drawing program.";
    assert_eq!(said[0].fix, Some(Fix::Hint(hint.to_string())));
}

#[test]
fn req_auto_070_work_is_bounded_and_a_budget_that_runs_out_stops_with_its_error() {
    let font = test_font();
    let text = object(vec![line("ab", 20.0, 30.0, 20.0), line("ba", 20.0, 50.0, 20.0)]);
    let with = |budget: &Budget| objects::letter(&text, &mut |_, _| Found::Font(Rc::clone(&font)), &mut BTreeSet::new(), budget);
    let mut needed = 0;
    loop {
        match with(&Budget { max_work: needed, ..Budget::DEFAULT }) {
            Ok((elements, _)) => {
                assert_eq!(elements.len(), 4);
                break;
            }
            Err(stopped) => assert_eq!(stopped.code, Code::BudgetExhausted, "at {needed}"),
        }
        needed += 1;
    }
    // A unit for the object, one and its bytes for each line, and one and its points for each of 4 letters,
    // once to measure the line and once to place it.
    assert_eq!(needed, 1 + 2 * 3 + 2 * 4 * 3);
    // Among a design's elements, a unit more for each element.
    let design = || Design::new(vec![shape("s1", 0.0), shape("s2", 10.0)], DesignSettings::default()).unwrap();
    let waiting = [Waiting { after: 1, text: text.clone() }, Waiting { after: 2, text: TextObject { label: "svg:u".to_string(), ..text.clone() } }];
    let set = |budget: &Budget| objects::lettered(design(), &waiting, &mut |_, _| Found::Font(Rc::clone(&font)), &mut BTreeSet::new(), budget);
    assert_eq!(set(&Budget { max_work: needed - 1, ..Budget::DEFAULT }).unwrap_err().code, Code::BudgetExhausted);
    assert_eq!(set(&Budget { max_work: 1, ..Budget::DEFAULT }).unwrap_err().code, Code::BudgetExhausted);
    assert!(set(&Budget { max_work: needed, ..Budget::DEFAULT }).is_ok());
    // A text that sews after every element stops the work as one among them does.
    let last = [Waiting { after: 9, text: text.clone() }];
    let after = |max_work: u64| {
        objects::lettered(design(), &last, &mut |_, _| Found::Font(Rc::clone(&font)), &mut BTreeSet::new(), &Budget { max_work, ..Budget::DEFAULT })
    };
    assert_eq!(after(needed - 1).unwrap_err().code, Code::BudgetExhausted);
    assert!(after(needed).is_ok());
    // The elements are counted by themselves: 2 of them need 2 units beside a text that needs 1.
    let empty = [Waiting { after: 0, text: object(Vec::new()) }];
    let beside = |max_work: u64| {
        objects::lettered(design(), &empty, &mut |_, _| Found::Font(Rc::clone(&font)), &mut BTreeSet::new(), &Budget { max_work, ..Budget::DEFAULT })
    };
    assert_eq!(beside(1).unwrap_err().code, Code::BudgetExhausted);
    assert!(beside(2).is_ok());
    // A font whose reader ran out of budget stops the work too.
    let spent = |_: &str, _: &Budget| Found::Unread(Diagnostic::new(Code::BudgetExhausted, "The budget ran out."));
    let mut spent = spent;
    assert_eq!(objects::letter(&text, &mut spent, &mut BTreeSet::new(), &Budget::DEFAULT).unwrap_err().code, Code::BudgetExhausted);
}

const NOT_STITCHED: &str = "warning SC-W0802: `t` is text, which is not stitched; it is left out.";

#[test]
fn req_svg_017_a_text_element_is_read_under_auto_and_is_a_warning_without_it() {
    let body = r##"<rect id="r" width="5" height="5"/><text id="t" inkscape:label=" Title " x="10" y="20" font-size="10" font-family=" Georgia " fill="#c80000">Hi <tspan>there</tspan></text><rect id="s" x="10" width="5" height="5"/>"##;
    let read = read(body);
    assert!(read.warnings.is_empty(), "{:?}", read.warnings);
    assert_eq!(read.design.elements().len(), 2);
    let expected = TextObject {
        label: "svg:t".to_string(),
        name: Some("Title".to_string()),
        typeface: Some("Georgia".to_string()),
        bold: false,
        lines: vec![line("Hi there", 10.0, 20.0, 10.0)],
        turn: (1.0, 0.0),
        clips: Vec::new(),
    };
    // It sews between the 2 shapes it is drawn between.
    assert_eq!(read.texts, [Waiting { after: 1, text: expected }]);
    // Without `auto` it is left out, as before.
    let read = read_as(body, false);
    assert!(read.texts.is_empty());
    assert_eq!(said(&read), [NOT_STITCHED]);
    // Text an editor flows into a shape isn't read.
    let read = self::read(r#"<flowRoot id="f"><flowPara>Hi</flowPara></flowRoot>"#);
    assert!(read.texts.is_empty());
    assert_eq!(said(&read), ["warning SC-W0802: `f` is text, which is not stitched; it is left out."]);
    // A text nobody sees isn't read, and nothing is said of it.
    for hidden in [r#"display="none""#, r#"visibility="hidden""#, r#"opacity="0""#] {
        let read = self::read(&format!(r#"<text id="t" {hidden} x="1" y="2">Hi</text>"#));
        assert!(read.texts.is_empty() && read.warnings.is_empty(), "{hidden}");
    }
}

#[test]
fn req_svg_017_a_tspan_with_a_place_starts_a_line_and_dx_and_dy_move_its_start() {
    // As editors write lines: each `<tspan>` with its own `x` and `y`.
    let two = r#"<text id="t" x="10" y="20" font-size="10"><tspan x="10" y="20">one</tspan><tspan x="12" y="32">two</tspan></text>"#;
    assert_eq!(lines(two), [("one".to_string(), (10.0, 20.0), Anchor::Start, 10.0), ("two".to_string(), (12.0, 32.0), Anchor::Start, 10.0)]);
    // As people write them: an `x` and a `dy` from the line before. A `y` alone keeps the line's `x`, and
    // an `x` alone its `y`. A `<tspan>` without either goes on in its line, and so does what follows it.
    let by_hand = r#"<text id="t" x="10" y="20" dx="1" dy="2" font-size="10">one<tspan x="10" dy="12">two</tspan><tspan y="50" dx="-3">three <tspan>and</tspan></tspan> more<tspan x="5">four</tspan></text>"#;
    let read = lines(by_hand);
    let places: Vec<(&str, (f64, f64))> = read.iter().map(|(text, at, _, _)| (text.as_str(), *at)).collect();
    assert_eq!(places, [("one", (11.0, 22.0)), ("two", (10.0, 34.0)), ("three and more", (7.0, 50.0)), ("four", (5.0, 50.0))]);
    // Lengths with units are read, and a line without characters sews nothing.
    let units = r#"<text id="t" x="1" y="20" font-size="10"><tspan x="0" y="0"> </tspan><tspan x="2" y="3">a</tspan></text>"#;
    assert_eq!(lines(units), [("a".to_string(), (2.0, 3.0), Anchor::Start, 10.0)]);
    let units = r#"<text id="t" x="1cm" y="20mm" font-size="10">a</text>"#;
    assert_eq!(lines(units), [("a".to_string(), (37.795276, 75.590551), Anchor::Start, 10.0)]);
    // Without an `x` or a `y` a text starts at the origin.
    assert_eq!(lines(r#"<text id="t" font-size="10">a</text>"#), [("a".to_string(), (0.0, 0.0), Anchor::Start, 10.0)]);
}

#[test]
fn req_svg_017_white_space_is_collapsed_unless_the_file_preserves_it() {
    let body = "<text id=\"t\" x=\"1\" y=\"2\" font-size=\"10\">\n  Two\twords \n <tspan> and  </tspan>\n more\n</text>";
    assert_eq!(lines(body)[0].0, "Two words and more");
    let kept = "<text id=\"t\" xml:space=\"preserve\" x=\"1\" y=\"2\" font-size=\"10\"> Two\twords\n</text>";
    assert_eq!(lines(kept)[0].0, " Two words ");
    // The nearest `xml:space` counts, a group's too.
    let around = "<g xml:space=\"preserve\"><text id=\"t\" x=\"1\" y=\"2\" font-size=\"10\"> a  b</text></g>";
    assert_eq!(lines(around)[0].0, " a  b");
    let own = "<g xml:space=\"preserve\"><text id=\"t\" xml:space=\"default\" x=\"1\" y=\"2\" font-size=\"10\"> a  b</text></g>";
    assert_eq!(lines(own)[0].0, "a b");
}

#[test]
fn req_svg_017_the_font_size_is_the_nearest_length_times_the_shares_nearer() {
    let size = |outer: &str, text: &str, span: &str| {
        let body = format!(r#"<g {outer}><text id="t" x="1" y="2" {text}><tspan {span}>a</tspan></text></g>"#);
        lines(&body)[0].3
    };
    assert_eq!(size("", r#"font-size="10""#, ""), 10.0);
    assert_eq!(size("", r#"style="font-size:10px""#, ""), 10.0);
    // 10 mm are 37.8 user units, each a pixel.
    assert_eq!(size("", r#"font-size="10mm""#, ""), 37.795276);
    assert_eq!(size(r#"font-size="8""#, "", ""), 8.0);
    // The first visible character's element counts: the `<tspan>`'s size, not the text's.
    assert_eq!(size("", r#"font-size="10""#, r#"font-size="4""#), 4.0);
    // White space before it is no character a viewer shows.
    assert_eq!(lines(r#"<text id="t" x="1" y="2" font-size="10"> <tspan font-size="4">a</tspan></text>"#)[0].3, 4.0);
    // Shares: `em`, `ex` as half of one, and percent, each of the size around it.
    assert_eq!(size(r#"font-size="8""#, r#"font-size="2em""#, r#"font-size="50%""#), 8.0);
    assert_eq!(size(r#"font-size="8""#, r#"font-size="3em""#, r#"font-size="4ex""#), 48.0);
    assert_eq!(size(r#"font-size="8""#, r#"font-size="150%""#, ""), 12.0);
    // A word isn't read, and the size around it stays. `inherit` says as much.
    assert_eq!(size(r#"font-size="8""#, r#"font-size="larger""#, r#"font-size="inherit""#), 8.0);
    // With none, CSS's 16 pixels, and shares of it.
    assert_eq!(size("", "", ""), 16.0);
    assert_eq!(size("", r#"font-size="2em""#, ""), 32.0);
    // A text without a size shows nothing, and is no line.
    let read = read(r#"<text id="t" x="1" y="2" font-size="0">a</text>"#);
    assert!(read.texts.is_empty() && read.warnings.is_empty());
}

#[test]
fn req_svg_017_the_anchor_the_thread_and_the_typeface_are_those_of_a_lines_first_visible_character() {
    let body = r##"<g text-anchor="end" fill="#0000c8" font-family="Georgia"><text id="t" x="1" y="2" font-size="10">
        <tspan display="none" text-anchor="middle" fill="#00ff00">no</tspan><tspan visibility="hidden">no</tspan>
        <tspan fill="none">no</tspan>a<tspan x="1" y="9" text-anchor="MIDDLE" fill="#c80000" font-family="Arial">b</tspan>
        <tspan x="1" y="19" text-anchor="start" style="fill:none;stroke:#00ff00">c</tspan>
        <tspan x="1" y="29" text-anchor="inherit" fill-opacity="0" stroke="#c80000">d</tspan>
        <tspan x="1" y="39" fill="#c80000" stroke="#00ff00" stroke-opacity="0" text-anchor="End">e</tspan></text></g>"##;
    let read = read(body);
    assert!(read.warnings.is_empty(), "{:?}", read.warnings);
    let text = &read.texts[0].text;
    let lines: Vec<(&str, Anchor, Rgb)> =
        text.lines.iter().map(|line| (line.text.as_str(), line.anchor, line.thread.as_ref().unwrap().color)).collect();
    let green = Rgb::new(0, 255, 0);
    // What isn't shown isn't sewn. A line without a fill sews in its stroke's colour.
    assert_eq!(
        lines,
        [("a", Anchor::End, BLUE), ("b", Anchor::Middle, RED), ("c", Anchor::Start, green), ("d", Anchor::End, RED), ("e", Anchor::End, RED)]
    );
    // The note names the typeface of the first line.
    assert_eq!(text.typeface.as_deref(), Some("Georgia"));
    // A line with neither a fill nor a stroke shows nothing, and is no line: here the text has none left.
    let read = self::read(r#"<text id="t" x="1" y="2" font-size="10" fill="none">a</text>"#);
    assert!(read.texts.is_empty() && read.warnings.is_empty());
    let read = self::read(r#"<text id="t" x="1" y="2" font-size="10" fill-opacity="0" stroke="red" stroke-opacity="0">a</text>"#);
    assert!(read.texts.is_empty() && read.warnings.is_empty());
    // A fill that paints nothing, as a reference to nothing does, beside a stroke nobody sees.
    let read = self::read(r##"<text id="t" x="1" y="2" font-size="10" fill="url(#nothing)" stroke="red" stroke-opacity="0">a</text>"##);
    assert!(read.texts.is_empty() && read.warnings.is_empty(), "{:?}", read.warnings);
    // A text that names no typeface has none.
    let read = self::read(r#"<text id="t" x="1" y="2" font-size="10" font-family=" ">a</text>"#);
    assert_eq!(read.texts[0].text.typeface, None);
    assert_eq!(self::read(r#"<text id="t" x="1" y="2" font-size="10">a</text>"#).texts[0].text.typeface, None);
}

#[test]
fn req_svg_017_the_text_is_placed_sized_and_turned_by_its_transforms() {
    // Twice the size, moved: the anchor at 2 × (10, 20) + (5, 6), the font size 20.
    let scaled = r#"<g transform="translate(5 6)"><text id="t" transform="scale(2)" x="10" y="20" font-size="10">a</text></g>"#;
    assert_eq!(lines(scaled), [("a".to_string(), (25.0, 46.0), Anchor::Start, 20.0)]);
    let read = read(scaled);
    assert!(read.warnings.is_empty());
    assert_eq!(read.texts[0].text.turn, (1.0, 0.0));
    // A quarter turn clockwise, as a picture shows it: the baseline runs down.
    let read = self::read(r#"<text id="t" transform="rotate(90)" x="10" y="20" font-size="10">a</text>"#);
    let text = &read.texts[0].text;
    assert!(text.turn.0.abs() < 1e-12 && (text.turn.1 - 1.0).abs() < 1e-12, "{:?}", text.turn);
    assert!((text.lines[0].at.x() + 20.0).abs() < 1e-9 && (text.lines[0].at.y() - 10.0).abs() < 1e-9);
    assert!(read.warnings.is_empty(), "{:?}", read.warnings);
    // Moved, turned by the angle whose cosine is 0.6 and whose sine is 0.8, and twice the size.
    let read = self::read(r#"<text id="t" transform="matrix(1.2 1.6 -1.6 1.2 5 6)" x="10" y="20" font-size="10">a</text>"#);
    let text = &read.texts[0].text;
    assert!((text.turn.0 - 0.6).abs() < 1e-12 && (text.turn.1 - 0.8).abs() < 1e-12, "{:?}", text.turn);
    assert!((text.lines[0].size - 20.0).abs() < 1e-9);
    assert!((text.lines[0].at.x() + 15.0).abs() < 1e-9 && (text.lines[0].at.y() - 46.0).abs() < 1e-9, "{:?}", text.lines[0].at);
    assert!(read.warnings.is_empty(), "{:?}", read.warnings);
    // Stretched, mirrored or slanted: as tall as it is drawn, turned as its baseline is, and said.
    let odd = "warning SC-W0802: `t` is stretched, mirrored or slanted, which lettering doesn't follow: it is sewn as tall as it is drawn, turned as \
               its baseline is.";
    for (transform, size, turn) in [
        ("scale(3 2)", 20.0, (1.0, 0.0)),
        ("scale(1 -1)", 10.0, (1.0, 0.0)),
        ("scale(-1 1)", 10.0, (-1.0, 0.0)),
        ("matrix(0 1 1 0 0 0)", 10.0, (0.0, 1.0)),
        ("matrix(0 -1 -1 0 0 0)", 10.0, (0.0, -1.0)),
        ("skewX(45)", 10.0 * 2f64.sqrt(), (1.0, 0.0)),
    ] {
        let read = self::read(&format!(r#"<text id="t" transform="{transform}" x="10" y="20" font-size="10">a</text>"#));
        assert_eq!(said(&read), [odd], "{transform}");
        let text = &read.texts[0].text;
        assert!((text.lines[0].size - size).abs() < 1e-9, "{transform}: {}", text.lines[0].size);
        assert_eq!(text.turn, turn, "{transform}");
    }
}

#[test]
fn req_svg_017_what_places_letters_one_by_one_is_said_and_left_out() {
    let body = r#"<text id="t" x="10 15 20" y="20" rotate="10" textLength="50" font-size="10">a<tspan dx="1">b</tspan><tspan dy="1" rotate="5">c</tspan></text>"#;
    let read = read(body);
    assert_eq!(
        said(&read),
        ["warning SC-W0802: `t` places letters one by one (`rotate`, a `dx` or `dy` inside a line, a place for each letter), which \
          lettering doesn't follow: each line's letters stand as its font spaces them."]
    );
    // The first place of a list is the line's, and the letters are all in it.
    assert_eq!(lines(body), [("abc".to_string(), (10.0, 20.0), Anchor::Start, 10.0)]);
    // Each alone is said too.
    for (attributes, span, what) in [
        (r#"x="1,2""#, "", "a place for each letter"),
        (r#"dy="1 2""#, "", "a place for each letter"),
        (r#"rotate="5""#, "", "`rotate`"),
        ("", r#"dx="1""#, "a `dx` or `dy` inside a line"),
        ("", r#"dy="1""#, "a `dx` or `dy` inside a line"),
        ("", r#"rotate="1""#, "`rotate`"),
        ("", r#"x="1" y="2" rotate="1""#, "`rotate`"),
        ("", r#"textLength="5""#, "a `textLength` inside a line"),
    ] {
        let read = self::read(&format!(r#"<text id="t" {attributes} font-size="10">a<tspan {span}>b</tspan></text>"#));
        let said = said(&read);
        assert!(said.len() == 1 && said[0].contains(&format!("one by one ({what}), which")), "{attributes} {span}: {said:?}");
        assert_eq!(read.texts.len(), 1);
    }
    // Text on a path is left out, with what else an element that is no `<tspan>` and no link holds. The
    // rest sews.
    let on_a_path =
        r##"<text id="t" x="1" y="2" font-size="10">a<textPath href="#p">on <tspan>it</tspan></textPath><title>no</title><a href="x">b</a></text>"##;
    let read = self::read(on_a_path);
    assert_eq!(
        said(&read),
        ["warning SC-W0802: `t` holds text on a path (`<textPath>`), which lettering doesn't follow here; that text is left out."]
    );
    assert_eq!(read.texts[0].text.lines[0].text, "ab");
}

#[test]
fn req_svg_017_a_text_length_is_the_length_its_line_is_fitted_to() {
    let lengths = |body: &str| {
        let read = read(body);
        (read.texts[0].text.lines.iter().map(|line| line.length).collect::<Vec<_>>(), said(&read))
    };
    // A text of one line: its own, scaled as its transforms scale along the baseline.
    assert_eq!(lengths(r#"<text id="t" x="1" y="2" font-size="10" textLength="50">a</text>"#), (vec![Some(50.0)], vec![]));
    assert_eq!(lengths(r#"<text id="t" transform="scale(2 3)" x="1" y="2" font-size="10" textLength="50">a</text>"#).0, [Some(100.0)]);
    assert_eq!(lengths(r#"<text id="t" x="1" y="2" font-size="10" textLength="5cm">a</text>"#).0.len(), 1);
    // A `<tspan>` that starts a line states its line's, and its own counts before the text's.
    let two = r#"<text id="t" font-size="10"><tspan x="1" y="2" textLength="30">a</tspan><tspan x="1" y="9">b</tspan></text>"#;
    assert_eq!(lengths(two), (vec![Some(30.0), None], vec![]));
    let scaled =
        r#"<text id="t" transform="scale(2 3)" font-size="10"><tspan x="1" y="2" textLength="30">a</tspan><tspan x="1" y="9">b</tspan></text>"#;
    assert_eq!(lengths(scaled).0, [Some(60.0), None]);
    let own = r#"<text id="t" font-size="10" textLength="50"><tspan x="1" y="2" textLength="30">a</tspan></text>"#;
    assert_eq!(lengths(own), (vec![Some(30.0)], vec![]));
    let line = r#"<text id="t" font-size="10" textLength="50"><tspan x="1" y="2">a</tspan></text>"#;
    assert_eq!(lengths(line), (vec![Some(50.0)], vec![]));
    // The length of a text of several lines is no line's, which is said.
    let several = r#"<text id="t" font-size="10" textLength="50"><tspan x="1" y="2">a</tspan><tspan x="1" y="9">b</tspan></text>"#;
    let (read, said) = lengths(several);
    assert_eq!(read, [None, None]);
    assert!(said.len() == 1 && said[0].contains("one by one (a `textLength` of several lines), which"), "{said:?}");
    // One that is no length states nothing.
    for none in ["0", "-5", "long"] {
        assert_eq!(lengths(&format!(r#"<text id="t" x="1" y="2" font-size="10" textLength="{none}">a</text>"#)), (vec![None], vec![]), "{none}");
    }
}

#[test]
fn diag_sc_w1111_vertical_text_of_an_svg_file_is_not_stitched() {
    for mode in ["tb", "tb-rl", "vertical-rl", "vertical-lr", "sideways-rl", "sideways-lr", "Vertical-RL"] {
        let read = read(&format!(r#"<g writing-mode="{mode}"><text id="t" x="1" y="2" font-size="10">a</text></g>"#));
        assert!(read.texts.is_empty(), "{mode}");
        assert_eq!(said(&read), ["warning SC-W1111: `t` is vertical text, which lettering doesn't set, and is not stitched."], "{mode}");
        assert_eq!(read.warnings[0].element.as_ref().map(ElementId::as_str), Some("svg:t"));
    }
    // The text's own mode counts before a group's, and level text is read.
    for (outer, own) in [("tb", "lr"), ("tb", "horizontal-tb"), ("lr-tb", "inherit")] {
        let read = read(&format!(r#"<g writing-mode="{outer}"><text id="t" writing-mode="{own}" x="1" y="2" font-size="10">a</text></g>"#));
        assert!(read.texts.len() == 1 && read.warnings.is_empty(), "{outer} {own}");
    }
}

#[test]
fn req_svg_017_the_lettering_is_cut_to_the_clips_around_the_text_and_labels_are_unique() {
    let body = r##"<clipPath id="c"><rect width="50" height="50"/></clipPath><clipPath id="d"><rect width="5" height="5"/></clipPath>
        <g clip-path="url(#c)"><text id="t" x="1" y="2" font-size="10">a</text></g>
        <text id="t" x="1" y="2" font-size="10" clip-path="url(#d)" filter="url(#f)">b</text><text x="1" y="2" font-size="10">c</text>"##;
    let read = read(body);
    let labels: Vec<&str> = read.texts.iter().map(|waiting| waiting.text.label.as_str()).collect();
    assert_eq!(labels, ["svg:t", "svg:t~2", "svg:text@3"]);
    assert_eq!(read.texts[0].text.clips.len(), 1);
    assert!(read.texts[1].text.clips.is_empty() && read.texts[2].text.clips.is_empty());
    // A clip path of the text itself isn't applied, which is said, as its filter is.
    assert_eq!(
        said(&read),
        [
            "warning SC-W0802: `t` has filter effects (blurs, shadows and the like), which are not stitched.",
            "warning SC-W0802: The clip path of `t~2` itself is not applied to its lettering."
        ]
    );
}

#[test]
fn req_svg_017_a_text_beyond_the_designs_reach_is_left_out_and_reading_is_bounded() {
    let read = read(r#"<text id="t" x="1e300" y="1e300" font-size="10">a</text>"#);
    assert!(read.texts.is_empty());
    assert_eq!(said(&read), ["warning SC-W0804: `t` lies beyond the design's reach, so it is left out."]);
    // With too little work allowed, the reading stops with the budget's error, whatever was reached.
    let svg = r##"<svg xmlns="http://www.w3.org/2000/svg" width="100mm" height="100mm" viewBox="0 0 100 100"><g font-size="10"><text id="t" x="1" y="2" writing-mode="lr" clip-path="url(#c)">a<tspan x="1" y="9" fill="#c80000">b<tspan>c</tspan></tspan></text></g></svg>"##;
    let with = |max_work: u64| {
        stitchcraft_svg::read_with(svg.as_bytes(), &Budget { max_work, ..Budget::DEFAULT }, Reading { auto: true, ..Reading::default() })
    };
    let mut needed = 0;
    loop {
        match with(needed) {
            Ok(read) => {
                assert_eq!(read.texts[0].text.lines.len(), 2);
                break;
            }
            Err(stopped) => assert_eq!(stopped.code, Code::BudgetExhausted, "at {needed}"),
        }
        needed += 1;
    }
    assert!(needed > 20, "{needed}");
}

#[test]
fn req_auto_082_a_text_is_bold_as_its_first_line_s_font_weight_says() {
    let bold = |body: &str| {
        let read = read(body);
        assert_eq!(read.texts.len(), 1, "{:?}", read.warnings);
        read.texts[0].text.bold
    };
    assert!(bold(r#"<text x="10" y="50" font-size="20" font-weight="bold" fill="red">ab</text>"#));
    assert!(bold(r#"<g style="font-weight:700"><text x="10" y="50" font-size="20" fill="red">ab</text></g>"#));
    assert!(!bold(r#"<text x="10" y="50" font-size="20" font-weight="600" fill="red">ab</text>"#));
    assert!(!bold(r#"<text x="10" y="50" font-size="20" fill="red">ab</text>"#));
    // The first line that shows decides, as it does the typeface.
    assert!(!bold(
        r#"<text x="10" y="50" font-size="20" fill="red"><tspan x="10" y="50">ab</tspan><tspan x="10" y="80" font-weight="bold">ab</tspan></text>"#
    ));
    assert!(bold(
        r#"<text x="10" y="50" font-size="20" fill="red"><tspan x="10" y="50" font-size="0">ab</tspan><tspan x="10" y="80" font-weight="bold">ab</tspan></text>"#
    ));
}
