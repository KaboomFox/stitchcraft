//! Lettering (roadmap step M14.2, `REQ-FONT-009` to `REQ-FONT-013`): text set in fonts drawn in code, its
//! layout checked on the glyphs' boxes. No font file of anyone's is used.

// Test code may unwrap and index (clippy.toml allows it in tests).
#![allow(clippy::unwrap_used)]

use proptest::prelude::*;
use stitchcraft_core::units::MM_PER_SVG_PX;
use stitchcraft_core::{Budget, Code, Diagnostic, Fix};
use stitchcraft_engine::design::Shape;
use stitchcraft_svg::font::{self, Font};
use stitchcraft_svg::lettering::{self, Align, ColourSort, Lettering, Placed, SETTINGS, Set, Trim};

/// A glyph file's root: 100 mm square, a user unit a millimetre.
const ROOT: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" xmlns:inkscape="http://www.inkscape.org/namespaces/inkscape" xmlns:sodipodi="http://sodipodi.sourceforge.net/DTD/sodipodi-0.dtd" xmlns:inkstitch="http://inkstitch.org/namespace" width="100mm" height="100mm" viewBox="0 0 100 100">"#;

/// The glyph of `label`: a stroke `id` from (`left`, 60) to (`left` + `width`, 70), on the baseline.
fn stroke(label: &str, id: &str, left: f64, width: f64) -> String {
    let path = format!(r#"<path id="{id}" d="M {left},60 L {},70" style="fill:none;stroke:#000000;stroke-width:0.4"/>"#, left + width);
    layer(label, &path)
}

/// The glyph layer of `label`, holding `body`.
fn layer(label: &str, body: &str) -> String {
    format!(r#"<g inkscape:groupmode="layer" inkscape:label="GlyphLayer-{label}" style="display:none">{body}</g>"#)
}

/// A satin column `id`, 2 mm wide and 10 mm tall, its left rail at `left`, standing on the baseline.
fn column(id: &str, left: f64) -> String {
    format!(
        r#"<path id="{id}" d="M {left},60 L {left},70 M {r},60 L {r},70" style="fill:none;stroke:#000000" inkstitch:satin_column="true"/>"#,
        r = left + 2.0
    )
}

/// A glyph file of `layers`, its baseline 70 mm down.
fn file(layers: &str) -> String {
    format!(
        r#"{ROOT}<sodipodi:namedview id="base"><sodipodi:guide position="0,30" orientation="0,1" inkscape:label="baseline"/></sodipodi:namedview>{layers}</svg>"#
    )
}

/// The glyphs of the test font: `a` 1 to 5 mm, `b` 2 to 8, `f` 0 to 3, the ligature `ff` 1 to 9, `?` 5 to 6,
/// `A` 0 to 7, `é` 0 to 2, and a form of `b` that is never chosen.
fn glyphs() -> String {
    [
        stroke("a", "pa", 1.0, 4.0),
        stroke("b", "pb", 2.0, 6.0),
        stroke("b.init", "pinit", 0.0, 9.0),
        stroke("f", "pf", 0.0, 3.0),
        stroke("ff", "pff", 1.0, 8.0),
        stroke("?", "pq", 5.0, 1.0),
        stroke("A", "pA", 0.0, 7.0),
        stroke("é", "pe", 0.0, 2.0),
    ]
    .concat()
}

/// `mm` in the CSS pixels `font.json` gives lengths in.
fn px(mm: f64) -> f64 {
    mm / MM_PER_SVG_PX
}

/// The settings of the test font, with `more` keys: 10 mm tall, sewn from 5 to 20 mm. `a` advances 6 mm, `b`
/// 8 and the others 5, a space 3 and a line 20. `a b` kerns 1 mm and `b a` 0.5.
fn json(more: &str) -> String {
    format!(
        r#"{{"name": "Test", "size": 10, "min_scale": 0.5, "max_scale": 2, "auto_satin": false, "default_glyph": "?", "leading": {}, "horiz_adv_x": {{"a": {}, "b": {}}}, "horiz_adv_x_default": {}, "horiz_adv_x_space": {}, "kerning_pairs": {{"a b": {}, "b a": {}}}{more}}}"#,
        px(20.0),
        px(6.0),
        px(8.0),
        px(5.0),
        px(3.0),
        px(1.0),
        px(0.5)
    )
}

/// The font of `json` and the glyph file `layers`, as `file`, which is `ltr.svg` or `rtl.svg`.
fn font_of(json: &str, file_name: &str, layers: &str) -> Font {
    let glyph_file = file(layers);
    font::read("test", &[("font.json", json.as_bytes()), (file_name, glyph_file.as_bytes())], &Budget::DEFAULT).unwrap()
}

/// The test font, with `more` keys in its settings.
fn test_font(more: &str) -> Font {
    font_of(&json(more), "ltr.svg", &glyphs())
}

/// `text` set in `font` as `how` says, which must set.
fn set(font: &Font, text: &str, how: &Lettering) -> Set {
    lettering::set(font, text, how, &Budget::DEFAULT).unwrap()
}

/// Why `text` isn't set in `font`.
fn refused(font: &Font, text: &str, how: &Lettering) -> Diagnostic {
    lettering::set(font, text, how, &Budget::DEFAULT).unwrap_err()
}

/// The names of the glyphs set.
fn names(set: &Set) -> Vec<&str> {
    set.glyphs.iter().map(|glyph| glyph.name.as_str()).collect()
}

/// `values` are `expected`, each within a nanometre.
#[track_caller]
fn near(values: &[f64], expected: &[f64]) {
    assert_eq!(values.len(), expected.len(), "{values:?} are not {expected:?}");
    assert!(values.iter().zip(expected).all(|(a, b)| (a - b).abs() < 1e-6), "{values:?} are not {expected:?}");
}

/// The left edges of the glyphs set.
fn lefts(set: &Set) -> Vec<f64> {
    set.glyphs.iter().map(|glyph| glyph.left).collect()
}

/// What setting said, as users read it.
fn said(set: &Set) -> Vec<String> {
    set.warnings.iter().map(ToString::to_string).collect()
}

/// The ids of the design's elements, in sewing order, each the glyph's number and the path's id: `0-pa` for
/// the reader's `0-svg:pa:stroke`.
fn ids(set: &Set) -> Vec<String> {
    let short = |id: &str| id.replacen("svg:", "", 1).rsplit_once(':').map_or(id.to_string(), |(path, _)| path.to_string());
    set.design.elements().iter().map(|element| short(element.id.as_str())).collect()
}

#[test]
fn req_font_009_each_character_sews_as_the_longest_glyph_that_names_it() {
    let font = test_font("");
    let plain = Lettering::default();
    assert_eq!(names(&set(&font, "ab", &plain)), ["a", "b"]);
    // A ligature sews for its letters, and the text goes on after it.
    assert_eq!(names(&set(&font, "ff", &plain)), ["ff"]);
    assert_eq!(names(&set(&font, "fffa", &plain)), ["ff", "f", "a"]);
    // The text is read in NFC, as glyphs are named.
    assert_eq!(names(&set(&font, "e\u{301}", &plain)), ["é"]);
    // A form of an Arabic letter is never chosen: its name's other characters are missing.
    let form = set(&font, "b.init", &plain);
    assert_eq!(names(&form), ["b", "?", "?", "?", "?", "?"]);
    assert_eq!(form.warnings.len(), 1);
    // The case the font forces.
    assert_eq!(names(&set(&test_font(r#", "letter_case": "upper""#), "a", &plain)), ["A"]);
    assert_eq!(names(&set(&test_font(r#", "letter_case": "lower""#), "A", &plain)), ["a"]);
    assert_eq!(names(&set(&font, "A", &plain)), ["A"]);
    // Lines end at line feeds and lose the spaces at their ends. Words lie between spaces.
    let lines = set(&font, "  a b \n\n b", &plain);
    let places: Vec<(&str, usize, usize)> = lines.glyphs.iter().map(|glyph| (glyph.name.as_str(), glyph.line, glyph.word)).collect();
    assert_eq!(places, [("a", 0, 0), ("b", 0, 1), ("b", 2, 0)]);
    near(&lefts(&lines), &[0.0, 8.0, 0.0]);
    assert_eq!(said(&lines), Vec::<String>::new());
}

#[test]
fn diag_sc_w1107_a_character_the_font_lacks_sews_as_its_default_glyph() {
    let font = test_font("");
    let missing = set(&font, "axby x", &Lettering::default());
    assert_eq!(names(&missing), ["a", "?", "b", "?", "?"]);
    // `?` lies its left edge after its origin, which is `a`'s advance after `a`'s, and advances 5 mm.
    near(&lefts(&missing), &[0.0, 10.0, 12.0, 23.0, 26.0]);
    assert_eq!(
        said(&missing),
        ["warning SC-W1107: The font `test` has no glyph for `x` (U+0078), `y` (U+0079). Each sews as the font's default glyph, `?`."]
    );
    assert_eq!(missing.warnings[0].code, Code::CharacterNotInFont);
    assert_eq!(missing.warnings[0].fix, Some(Fix::Hint("Choose a font that has the characters, or write the text without them.".to_string())));
    // A font without its default glyph leaves the character as a space, and the next glyph starts at the pen.
    let font = test_font(r##", "default_glyph": "#""##);
    let spaced = set(&font, "axb", &Lettering::default());
    assert_eq!(names(&spaced), ["a", "b"]);
    near(&lefts(&spaced), &[0.0, 8.0]);
    assert_eq!(
        said(&spaced),
        [
            "warning SC-W1107: The font `test` has no glyph for `x` (U+0078). Each is left as a space, as the font has no glyph for its default glyph, `#`."
        ]
    );
}

#[test]
fn req_font_010_glyphs_lie_by_their_advances_kerning_and_the_spacing_asked_for() {
    let font = test_font("");
    // `a` at the pen, its origin 1 mm before. `b`'s origin 6 - 1 mm after it, its box 2 mm on.
    near(&lefts(&set(&font, "ab", &Lettering::default())), &[0.0, 6.0]);
    near(&lefts(&set(&font, "ba", &Lettering::default())), &[0.0, 6.5]);
    // The pen after `ab` lies at 4 + 8, and a space is 3 mm.
    near(&lefts(&set(&font, "ab a", &Lettering::default())), &[0.0, 6.0, 15.0]);
    // An empty word between 2 spaces takes a space more.
    let double = set(&font, "a  b", &Lettering::default());
    near(&lefts(&double), &[0.0, 11.0]);
    assert_eq!(double.glyphs[1].word, 2);
    let spaced = Lettering { letter_spacing: 0.5, word_spacing: 2.0, line_height: 2.5, ..Lettering::default() };
    let wide = set(&font, "ab a\n\nba", &spaced);
    near(&lefts(&wide), &[0.0, 6.5, 17.5, 0.0, 7.0]);
    let baselines: Vec<f64> = wide.glyphs.iter().map(|glyph| glyph.baseline).collect();
    near(&baselines, &[0.0, 0.0, 0.0, 45.0, 45.0]);
    let widths: Vec<f64> = wide.glyphs.iter().map(|glyph| glyph.width).collect();
    near(&widths, &[4.0, 6.0, 4.0, 6.0, 4.0]);
    // Negative spacing closes the letters up.
    near(&lefts(&set(&font, "ab", &Lettering { letter_spacing: -1.0, ..Lettering::default() })), &[0.0, 5.0]);
    // The glyphs sew in the order they are laid out.
    assert_eq!(ids(&set(&font, "ab a", &Lettering::default())), ["0-pa", "1-pb", "2-pa"]);
}

#[test]
fn req_font_010_right_to_left_text_is_laid_in_reverse_and_an_rtl_direction_sews_from_the_end() {
    let font = test_font(r#", "text_direction": "rtl""#);
    let reversed = set(&font, "ab a", &Lettering::default());
    assert_eq!(names(&reversed), ["a", "b", "a"]);
    let words: Vec<usize> = reversed.glyphs.iter().map(|glyph| glyph.word).collect();
    assert_eq!(words, [0, 1, 1]);
    // `a`, a space, then `b` and `a`, kerned as `a b`: `a`'s origin 8 - 1 mm after `b`'s, at 6.
    near(&lefts(&reversed), &[0.0, 8.0, 14.0]);
    assert_eq!(ids(&reversed), ["0-pa", "1-pb", "2-pa"]);
    // A font whose default direction is rtl sews each line from its last glyph.
    let font = font_of(&json(""), "rtl.svg", &glyphs());
    let back = set(&font, "ab\nb a", &Lettering::default());
    assert_eq!(names(&back), ["a", "b", "b", "a"]);
    near(&lefts(&back), &[0.0, 6.0, 0.0, 9.0]);
    assert_eq!(ids(&back), ["0-pb", "1-pa", "2-pa", "3-pb"]);
}

#[test]
fn req_font_011_lines_align_by_the_width_of_their_glyphs_boxes() {
    let font = test_font("");
    let aligned = |text: &str, align: Align| lefts(&set(&font, text, &Lettering { align, ..Lettering::default() }));
    // `ab a` is 19 mm wide and `b` 6.
    near(&aligned("ab a\nb", Align::Left), &[0.0, 6.0, 15.0, 0.0]);
    near(&aligned("ab a\nb", Align::Centre), &[-9.5, -3.5, 5.5, -3.0]);
    near(&aligned("ab a\nb", Align::Right), &[-19.0, -13.0, -4.0, -6.0]);
    // A block: `ab a b` is 29 mm wide and `a a a` 20, so each of its 2 gaps takes 4.5 mm. A line of one word stays.
    near(&aligned("ab a b\na a a\nb", Align::Block), &[0.0, 6.0, 15.0, 23.0, 0.0, 12.5, 25.0, 0.0]);
    near(&aligned("ab a b\naba", Align::Block), &[0.0, 6.0, 15.0, 23.0, 0.0, 6.0, 12.5]);
    // With letter spacing, the 3 glyphs of `aba`, 16.5 mm wide, take 6.25 mm in each gap, and a line of one glyph stays.
    near(&aligned("ab a b\naba\nb", Align::LetterSpacing), &[0.0, 6.0, 15.0, 23.0, 0.0, 12.25, 25.0, 0.0]);
    near(&aligned("ab a b\na a a", Align::LetterSpacing), &[0.0, 6.0, 15.0, 23.0, 0.0, 12.5, 25.0]);
    // A line of 2 words is a block's, whatever its glyphs number: `ab a`, 19 mm wide, has one gap of 10 mm.
    near(&aligned("ab a b\nab a", Align::LetterSpacing), &[0.0, 6.0, 15.0, 23.0, 0.0, 6.0, 25.0]);
    // The width is from the leftmost box, which a negative spacing can put after the first glyph.
    let crossed = Lettering { letter_spacing: -10.0, align: Align::Right, ..Lettering::default() };
    near(&lefts(&set(&font, "ab", &crossed)), &[-8.0, -12.0]);
    // Every alignment by Ink/Stitch's name, and each setting by its own.
    let all: Vec<&str> = Align::ALL.iter().map(|align| align.id()).collect();
    assert_eq!(all, ["left", "center", "right", "block", "letterspacing"]);
    assert_eq!(
        (Align::named("center"), Align::named("letterspacing"), Align::named("centre")),
        (Some(Align::Centre), Some(Align::LetterSpacing), None)
    );
    assert_eq!(Align::default(), Align::Left);
    let settings: Vec<(&str, &str, &str)> = SETTINGS.iter().map(|setting| (setting.name, setting.takes, setting.default)).collect();
    assert_eq!(settings.len(), 10);
    assert_eq!(
        [settings[0], settings[3], settings[4], settings[5], settings[6]],
        [
            ("scale", "PERCENT", "100"),
            ("text_align", "left|center|right|block|letterspacing", "left"),
            ("letter_spacing", "MM", "0"),
            ("word_spacing", "MM", "0"),
            ("line_height", "MM", "0")
        ]
    );
    assert!(SETTINGS.iter().all(|setting| !setting.does.is_empty() && !setting.does.ends_with('.')));
    let plain = Lettering {
        scale: 100.0,
        letter_spacing: 0.0,
        word_spacing: 0.0,
        line_height: 0.0,
        align: Align::Left,
        back_and_forth: true,
        trim: Trim::Off,
        color_sort: ColourSort::Off,
    };
    assert_eq!(Lettering::default(), plain);
}

#[test]
fn req_font_012_every_element_is_placed_and_scaled_about_the_first_lines_start() {
    let fill = r#"<path id="fill" d="M 3,60 L 7,60 L 7,70 Z" style="fill:#ff0000;stroke:none"/>"#;
    let font = font_of(&json(""), "ltr.svg", &[glyphs(), layer("o", fill)].concat());
    let twice = set(&font, "ab\no", &Lettering { scale: 200.0, back_and_forth: false, ..Lettering::default() });
    assert_eq!(twice.scale, 200.0);
    assert_eq!(said(&twice), Vec::<String>::new());
    let expected = [
        Placed { name: "a".to_string(), line: 0, word: 0, left: 0.0, width: 8.0, baseline: 0.0, angle: 0.0 },
        Placed { name: "b".to_string(), line: 0, word: 0, left: 12.0, width: 12.0, baseline: 0.0, angle: 0.0 },
        Placed { name: "o".to_string(), line: 1, word: 0, left: 0.0, width: 8.0, baseline: 40.0, angle: 0.0 },
    ];
    assert_eq!(twice.glyphs, expected);
    let points = |index: usize| -> Vec<(f64, f64)> { twice.design.elements()[index].shape.path().points().map(|p| (p.x(), p.y())).collect() };
    assert_eq!(points(0), [(0.0, -20.0), (8.0, 0.0)]);
    assert_eq!(points(1), [(12.0, -20.0), (24.0, 0.0)]);
    assert_eq!(points(2), [(0.0, 20.0), (8.0, 20.0), (8.0, 40.0)]);
    assert_eq!(ids(&twice), ["0-pa", "1-pb", "2-fill"]);
    // A stroke's width scales, and its settings and its thread stay the glyph's.
    let glyph = &font.default_variant().unwrap().glyphs["a"].design.elements()[0];
    let placed = &twice.design.elements()[0];
    let (Shape::Stroke { width: drawn, join, cap, .. }, Shape::Stroke { width, join: placed_join, cap: placed_cap, .. }) =
        (&glyph.shape, &placed.shape)
    else {
        panic!("the glyph is a stroke")
    };
    assert!((drawn.get() - 0.4).abs() < 1e-9 && (width.get() - 0.8).abs() < 1e-9, "{drawn:?} {width:?}");
    assert_eq!((join, cap), (placed_join, placed_cap));
    assert_eq!((&placed.params, &placed.thread, placed.automatic), (&glyph.params, &glyph.thread, glyph.automatic));
    assert!(matches!(twice.design.elements()[2].shape, Shape::Fill { .. }));
    // At the font's own size nothing moves but to its place.
    let once = set(&font, "b", &Lettering::default());
    let b: Vec<(f64, f64)> = once.design.elements()[0].shape.path().points().map(|p| (p.x(), p.y())).collect();
    assert_eq!(b, [(0.0, -10.0), (6.0, 0.0)]);
}

#[test]
fn diag_sc_w1106_a_scale_outside_the_fonts_range_is_set_at_its_nearest() {
    let font = test_font("");
    let at = |scale: f64| set(&font, "a", &Lettering { scale, ..Lettering::default() });
    // The ends of the range are in it.
    for scale in [50.0, 100.0, 200.0] {
        assert_eq!((at(scale).scale, said(&at(scale))), (scale, Vec::new()));
    }
    let small = at(49.5);
    assert_eq!(small.scale, 50.0);
    near(&[small.glyphs[0].width], &[2.0]);
    assert_eq!(
        said(&small),
        [
            "warning SC-W1106: The scale 49.5 % is outside the range the font `test` sews at, 50 to 200 % (5.0 to 20.0 mm tall). The text is set at 50 %."
        ]
    );
    assert_eq!(small.warnings[0].code, Code::LetteringScaleOutOfRange);
    assert_eq!(small.warnings[0].fix, Some(Fix::Hint("Choose a scale within the range, or a font drawn for the size you want.".to_string())));
    let large = at(300.0);
    assert_eq!(large.scale, 200.0);
    assert_eq!(
        said(&large),
        [
            "warning SC-W1106: The scale 300 % is outside the range the font `test` sews at, 50 to 200 % (5.0 to 20.0 mm tall). The text is set at 200 %."
        ]
    );
    // A font without a size names its range in percent alone, and the warning comes before a missing character's.
    let unsized_font = font_of(r#"{"name": "Bare", "auto_satin": false}"#, "ltr.svg", &glyphs());
    let bare = set(&unsized_font, "ax", &Lettering { scale: 150.0, ..Lettering::default() });
    assert_eq!(bare.scale, 100.0);
    assert_eq!(bare.warnings.iter().map(|warning| warning.code).collect::<Vec<_>>(), [Code::LetteringScaleOutOfRange, Code::CharacterNotInFont]);
    assert_eq!(
        said(&bare)[0],
        "warning SC-W1106: The scale 150 % is outside the range the font `test` sews at, 100 to 100 %. The text is set at 100 %."
    );
}

#[test]
fn diag_sc_e1108_a_text_that_cannot_be_set_is_refused() {
    let font = test_font("");
    let plain = Lettering::default();
    let empty = "error SC-E1108: None of the text's characters sews in the font `test`: the text is empty, or only spaces.";
    assert_eq!(refused(&font, "", &plain).to_string(), empty);
    assert_eq!(refused(&font, "  \n ", &plain).to_string(), empty);
    let bare = Font { variants: Vec::new(), ..font.clone() };
    assert_eq!(refused(&bare, "a", &plain).to_string(), "error SC-E1108: The font `test` has no glyphs.");
    // Numbers that are none, and a text longer than a design may be.
    let unplaced = "error SC-E1108: A glyph can't be placed: the scale or a spacing is no number, or the text is longer than 10 m.";
    assert_eq!(refused(&font, "a", &Lettering { scale: f64::NAN, ..plain }).to_string(), unplaced);
    assert_eq!(refused(&font, "ab", &Lettering { letter_spacing: f64::INFINITY, ..plain }).to_string(), unplaced);
    assert_eq!(refused(&font, "ab", &Lettering { letter_spacing: 20_000.0, ..plain }).code, Code::LetteringUnset);
    // An element whose id leaves no room for its glyph's number.
    let long = "i".repeat(244);
    let font = font_of(&json(""), "ltr.svg", &stroke("a", &long, 0.0, 1.0));
    assert_eq!(
        refused(&font, "a", &plain).to_string(),
        format!("error SC-E1108: The id `svg:{long}:stroke` of an element of the glyph `a` is too long to number.")
    );
}

#[test]
fn req_font_012_setting_charges_the_budget_for_each_character_name_and_point() {
    let font = test_font("");
    let within = |work: u64| lettering::set(&font, "ab", &Lettering::default(), &Budget { max_work: work, ..Budget::DEFAULT });
    // `a`: a character and a name. `b`: a character and 2 names. Each glyph: an element of 2 points. Each
    // element once more for the trims and once for the colour sort.
    for work in 0..15 {
        assert_eq!(within(work).unwrap_err().code, Code::BudgetExhausted, "{work}");
    }
    assert_eq!(within(10).unwrap_err().to_string(), "error SC-E0004: Setting the text needed more than the work budget of 10 units.");
    assert_eq!(within(14).unwrap_err().fix, Some(Fix::Hint("Set a shorter text, or set it with a larger budget.".to_string())));
    assert_eq!(names(&within(15).unwrap()), ["a", "b"]);
}

#[test]
fn req_font_012_what_the_reader_said_of_a_glyph_is_said_once() {
    let odd = r#"<path id="odd" d="M 0,60 L 4,70" style="fill:none;stroke:#000000" inkstitch:no_such_setting="1"/>"#;
    let font = font_of(&json(""), "ltr.svg", &[glyphs(), layer("w", odd)].concat());
    let glyph = &font.default_variant().unwrap().glyphs["w"];
    assert_eq!(glyph.warnings.len(), 1, "{:?}", glyph.warnings);
    assert_eq!(set(&font, "waw", &Lettering::default()).warnings, glyph.warnings);
}

#[test]
fn req_font_012_an_elements_hints_move_and_scale_with_it() {
    use stitchcraft_core::Point;
    use stitchcraft_engine::design::{Design, Hints};
    // The glyph `a`, 1 to 5 mm across on a baseline 70 mm down, its stroke told to start at its top.
    let mut font = test_font("");
    let glyph = font.variants[0].glyphs.get_mut("a").unwrap();
    let mut elements = glyph.design.elements().to_vec();
    elements[0].hints = Hints { start: Some(Point::new(1.0, 60.0).unwrap()), ..Hints::default() };
    glyph.design = Design::new(elements, glyph.design.settings).unwrap();
    let twice = set(&font, "ba", &Lettering { scale: 200.0, ..Lettering::default() });
    // `a` lies at 6.5 mm, and its top 10 mm above the baseline, at twice the size.
    assert_eq!(twice.design.elements()[1].hints, Hints { start: Some(Point::new(13.0, -20.0).unwrap()), ..Hints::default() });
    assert_eq!(twice.design.elements()[0].hints, Hints::default());
}

/// A font of one glyph, `z`, of 2 satin columns, the right one drawn first.
fn backwards_font() -> Font {
    let layers = layer("z", &[column("right", 20.0), column("left", 0.0)].concat());
    font_of(r#"{"name": "Backwards", "auto_satin": true}"#, "ltr.svg", &layers)
}

#[test]
fn req_font_013_auto_satin_keeps_the_order_the_columns_are_drawn_in() {
    // A route without Preserve order starts at the leftmost end. Lettering's sews the right column first.
    let routed = set(&backwards_font(), "z", &Lettering::default());
    let order = ids(&routed);
    assert!(order.len() > 2, "{order:?}");
    assert_eq!((order.first().unwrap().as_str(), order.last().unwrap().as_str()), ("0-right", "0-left"));
    let mut columns = order;
    columns.dedup();
    assert_eq!(columns, ["0-right", "0-left"]);
}

/// A font of satin columns: `s` a column, `m` a fill and a column after it, and `a` a stroke.
fn satin_font(auto_satin: bool) -> Font {
    let fill = r#"<path id="fill" d="M 0,60 L 1,60 L 1,70 Z" style="fill:#ff0000;stroke:none"/>"#;
    let layers = [layer("s", &column("ps", 0.0)), layer("m", &[fill.to_string(), column("pm", 3.0)].concat()), stroke("a", "pa", 1.0, 4.0)].concat();
    font_of(&format!(r#"{{"name": "Satin", "auto_satin": {auto_satin}, "horiz_adv_x_default": {}}}"#, px(8.0)), "ltr.svg", &layers)
}

#[test]
fn req_font_013_auto_satin_routes_the_texts_columns_and_each_piece_sews_with_its_glyph() {
    let plain = Lettering::default();
    // Off, the elements sew as the glyphs draw them.
    let drawn = set(&satin_font(false), "sms", &plain);
    assert_eq!(ids(&drawn), ["0-ps", "1-fill", "1-pm", "2-ps"]);
    // On, the route's pieces replace the columns: each glyph's follow its fill, glyph after glyph.
    let routed = set(&satin_font(true), "sms", &plain);
    // Only what the reader said of the 2 glyphs' columns, which name no Ink/Stitch version: that the route leaves
    // a fill out isn't said.
    assert_eq!(routed.warnings, drawn.warnings);
    assert_eq!(routed.warnings.iter().map(|warning| warning.code.to_string()).collect::<Vec<_>>(), ["SC-W0802", "SC-W0802"]);
    let order = ids(&routed);
    // Each piece is named after its column, and a column makes several.
    let mut columns = order.clone();
    columns.dedup();
    assert_eq!(columns, ["0-ps", "1-fill", "1-pm", "2-ps"]);
    assert!(order.len() > 4 && ids(&drawn).len() == 4, "{order:?}");
    let full: Vec<&str> = routed.design.elements().iter().map(|element| element.id.as_str()).collect();
    assert!(!full.contains(&"0-svg:ps:stroke") && full.contains(&"1-svg:fill:fill"), "{full:?}");
    let fill = order.iter().position(|id| *id == "1-fill").unwrap();
    // The glyphs lie 8 mm apart, and no piece of one reaches the next.
    let glyph_of = |index: usize| {
        let bounds = stitchcraft_core::Rect::around(routed.design.elements()[index].shape.path().points()).unwrap();
        (bounds.center().x() / 8.0).floor()
    };
    let glyphs: Vec<f64> = (0..order.len()).map(glyph_of).collect();
    assert!(glyphs.windows(2).all(|pair| pair[0] <= pair[1]), "{glyphs:?}");
    assert!(fill > 0 && glyphs[fill - 1] == 0.0 && glyphs[fill] == 1.0, "{order:?} {glyphs:?}");
    assert_eq!(*glyphs.last().unwrap(), 2.0);
    // No trim is added: no element is set to trim that the glyphs don't set.
    assert!(routed.design.elements().iter().chain(drawn.design.elements()).all(|element| element.params.get("trim_after").is_none()));
    // A text without a column stays, and nothing is said of it.
    let strokes = set(&satin_font(true), "aa", &plain);
    assert_eq!((ids(&strokes), said(&strokes)), (vec!["0-pa".to_string(), "1-pa".to_string()], Vec::<String>::new()));
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(64))]

    #[test]
    fn req_font_012_no_text_and_no_setting_panics(
        text in "[ab f?x.\\n]{0,12}",
        numbers in proptest::array::uniform4(prop_oneof![any::<f64>(), -30.0..300.0_f64]),
        align in 0..5_usize,
        rtl in any::<bool>(),
    ) {
        let font = test_font(if rtl { r#", "text_direction": "rtl""# } else { "" });
        let [scale, letter_spacing, word_spacing, line_height] = numbers;
        let how = Lettering { scale, letter_spacing, word_spacing, line_height, align: Align::ALL[align], ..Lettering::default() };
        match lettering::set(&font, &text, &how, &Budget::DEFAULT) {
            Ok(set) => prop_assert!(!set.glyphs.is_empty() && set.design.elements().len() == set.glyphs.len()),
            Err(refusal) => prop_assert_eq!(refusal.code, Code::LetteringUnset),
        }
    }
}
