//! Text in pictures (roadmap step M14.5, `REQ-AUTO-063` to `REQ-AUTO-066`): the shapes inside a marker are
//! replaced by lettering in a font drawn in code, checked on what is taken out, on where each letter's
//! points lie, on its thread and on what is said. No font file of anyone's is used.

// Test code may unwrap, panic and index (clippy.toml allows it in tests, but not in their helpers).
#![allow(clippy::unwrap_used, clippy::panic)]

use std::collections::BTreeSet;
use std::rc::Rc;

use stitchcraft_core::units::{MM_PER_SVG_PX, Mm, Point};
use stitchcraft_core::{Budget, Code, Diagnostic, ElementId};
use stitchcraft_engine::design::{Cap, Design, DesignSettings, Element, FillRule, Hints, Join, Path, Segment, Shape, Subpath};
use stitchcraft_engine::testsheets::pictures::word;
use stitchcraft_params::ParamSet;
use stitchcraft_plan::{Rgb, Thread};
use stitchcraft_svg::font::{self, Font};
use stitchcraft_svg::lettering::pictures::{self, APART, CLOSER, FIT, Found, MARKER, Marker};
use stitchcraft_testkit::designs::in_place;

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

/// The test font: `a` 4 mm wide and `b` 6, each 10 mm tall on the baseline, `p` 4 mm wide from 5 mm above the
/// baseline to 3 below it, all black, `c` as `a` in green, `e` a stroke 4 mm long on the baseline, and `i` a
/// stroke 10 mm tall of no width. `u` is a curve 10 mm tall whose control points lie 4 mm to the left of its
/// ends, and which itself reaches 3 mm to their left. The path of `c` has a label that reads as a marker's
/// name.
fn test_font() -> Rc<Font> {
    let layers = [
        glyph("a", "#000000", 4.0, 60.0, 70.0),
        glyph("b", "#000000", 6.0, 60.0, 70.0),
        glyph("p", "#000000", 4.0, 65.0, 73.0),
        glyph("c", "#00ff00", 4.0, 60.0, 70.0).replace("<path ", "<path inkscape:label=\"stitch:text c\" "),
        glyph("u", "#000000", 4.0, 60.0, 70.0).replace("M 0,60 L 4,70", "M 4,60 C 0,65 0,65 4,70"),
        glyph("e", "#000000", 4.0, 70.0, 70.0),
        glyph("i", "#000000", 0.0, 60.0, 70.0),
    ];
    font_of(&layers.concat(), "")
}

fn at((x, y): (f64, f64)) -> Point {
    Point::new(x, y).unwrap()
}

/// The subpath through `points`, closed or open.
fn subpath(points: &[(f64, f64)], closed: bool) -> Subpath {
    Subpath { start: at(points[0]), segments: points[1..].iter().map(|point| Segment::Line(at(*point))).collect(), closed }
}

/// The closed subpath round the box from (`x0`, `y0`) to (`x1`, `y1`).
fn rect(x0: f64, y0: f64, x1: f64, y1: f64) -> Subpath {
    subpath(&[(x0, y0), (x1, y0), (x1, y1), (x0, y1)], true)
}

const RED: Rgb = Rgb::new(200, 0, 0);
const WHITE: Rgb = Rgb::new(250, 250, 250);
const BLUE: Rgb = Rgb::new(0, 0, 200);

fn element(id: &str, name: Option<&str>, shape: Shape, colour: Rgb) -> Element {
    Element {
        id: ElementId::new(id).unwrap(),
        name: name.map(str::to_string),
        shape,
        thread: Thread::new(colour),
        params: ParamSet::new(),
        automatic: true,
        hints: Hints::default(),
        clips: Vec::new(),
    }
}

/// An automatic area of `subpaths`.
fn fill(id: &str, subpaths: Vec<Subpath>, colour: Rgb) -> Element {
    element(id, None, Shape::Fill { path: Path { subpaths }, rule: FillRule::EvenOdd, gradient: None }, colour)
}

/// A line of `subpaths`, `width` mm wide, named `name`.
fn line(id: &str, name: Option<&str>, subpaths: Vec<Subpath>, width: f64, colour: Rgb) -> Element {
    let shape = Shape::Stroke { path: Path { subpaths }, width: Mm::new(width).unwrap(), join: Join::Round, cap: Cap::Butt };
    element(id, name, shape, colour)
}

/// A marker round the box from (5, 25) to (`right`, 60), named `name`: a blue outline.
fn marker(name: &str, right: f64) -> Element {
    line("m", Some(name), vec![rect(5.0, 25.0, right, 60.0)], 0.3, BLUE)
}

/// 2 traced letters in red, each an element: boxes 20 mm tall from y 30, the first from x 10 to 18 and the
/// second from 22 to `right`.
fn letters(right: f64) -> Vec<Element> {
    vec![fill("l1", vec![rect(10.0, 30.0, 18.0, 50.0)], RED), fill("l2", vec![rect(22.0, 30.0, right, 50.0)], RED)]
}

fn design(elements: Vec<Element>) -> Design {
    Design::new(elements, DesignSettings::default()).unwrap()
}

/// `design` with its markers' texts set in the test font, and what was said.
fn replaced(design: Design) -> (Design, Vec<String>) {
    let font = test_font();
    let (design, said) = pictures::replace(design, &[], &mut |_, _| Found::Font(Rc::clone(&font)), &mut BTreeSet::new(), &Budget::DEFAULT).unwrap();
    (design, said.iter().map(ToString::to_string).collect())
}

fn ids(design: &Design) -> Vec<&str> {
    design.elements().iter().map(|element| element.id.as_str()).collect()
}

/// The points of the element `id`, x and y of each.
fn points(design: &Design, id: &str) -> Vec<(f64, f64)> {
    let element = design.elements().iter().find(|element| element.id.as_str() == id).unwrap_or_else(|| panic!("no {id} in {:?}", ids(design)));
    element.shape.path().points().map(|p| (p.x(), p.y())).collect()
}

/// `points` are `expected`, each within a nanometre.
#[track_caller]
fn near(points: &[(f64, f64)], expected: &[(f64, f64)]) {
    assert_eq!(points.len(), expected.len(), "{points:?} are not {expected:?}");
    assert!(points.iter().zip(expected).all(|(a, b)| (a.0 - b.0).abs() < 1e-6 && (a.1 - b.1).abs() < 1e-6), "{points:?} are not {expected:?}");
}

const A: &str = "m:0-svg:pa:stroke";
const B: &str = "m:1-svg:pb:stroke";

#[test]
fn req_auto_063_what_lies_wholly_inside_a_marker_is_taken_out_and_the_background_is_whole() {
    // A background traced round 2 letters, with an island of its own colour inside the second, as the
    // counter of a letter is. The letters are one compound path, the counter a hole in it.
    let ground = fill(
        "ground",
        vec![rect(0.0, 0.0, 50.0, 80.0), rect(10.0, 30.0, 18.0, 50.0), rect(22.0, 30.0, 32.0, 50.0), rect(25.0, 35.0, 28.0, 40.0)],
        WHITE,
    );
    let ink = fill("ink", vec![rect(10.0, 30.0, 18.0, 50.0), rect(22.0, 30.0, 32.0, 50.0), rect(25.0, 35.0, 28.0, 40.0)], RED);
    let star = fill("star", vec![rect(40.0, 70.0, 45.0, 75.0)], BLUE);
    let given = design(vec![ground, ink, marker("stitch:text ab", 40.0), star]);
    let (after, said) = replaced(given);
    // The letters' element is gone and so is the marker. The lettering sews after the background, where
    // the letters were, and before what was drawn over them.
    assert_eq!(ids(&after), ["ground", A, B, "star"]);
    // The background's holes are closed: its outline is all that is left, filled as it was.
    assert_eq!(after.elements()[0], fill("ground", vec![rect(0.0, 0.0, 50.0, 80.0)], WHITE));
    assert!(after.elements()[0].automatic && !after.elements()[1].automatic && after.elements()[1].name.is_none());
    near(&points(&after, "star"), &[(40.0, 70.0), (45.0, 70.0), (45.0, 75.0), (40.0, 75.0)]);
    // The red of the letters painted the most of what was taken: the font's black letters sew in it.
    assert!(after.elements()[1..3].iter().all(|letter| letter.thread == Thread::new(RED)));
    assert_eq!(
        said,
        ["info SC-I1112: The text marker `m`: 6 traced outlines, 20.0 mm tall, are replaced by the text \"ab\" in the font `test`, 20.0 mm tall."]
    );
}

#[test]
fn req_auto_063_a_shape_that_crosses_the_markers_edge_stays() {
    // A bar 60 % inside the marker, one 40 % inside, and a field the marker lies on.
    let bar = |id: &str, from: f64| {
        fill(id, vec![subpath(&[(from, 52.0), (from + 10.0, 52.0), (from + 5.0, 52.0), (from + 10.0, 53.0), (from, 53.0)], true)], RED)
    };
    let field = fill("field", vec![rect(0.0, 0.0, 60.0, 80.0)], WHITE);
    // A triangle with exactly half of its outline's points inside is not counted either.
    let half = fill("half", vec![subpath(&[(45.0, 56.0), (39.0, 56.0), (39.0, 58.0)], true)], RED);
    // A traced line of 2 strokes keeps the one outside, as wide as it was.
    let strokes = line("strokes", None, vec![subpath(&[(12.0, 40.0), (16.0, 40.0)], false), subpath(&[(50.0, 53.0), (55.0, 53.0)], false)], 1.0, RED);
    let mut elements = vec![field.clone(), bar("most", 34.5), bar("some", 37.5), half, strokes];
    elements.extend(letters(32.0));
    elements.push(marker("stitch:text ab", 40.0));
    let (after, said) = replaced(design(elements));
    assert_eq!(ids(&after), ["field", "most", "some", "half", "strokes", A, B]);
    assert_eq!(after.elements()[0], field);
    let kept = line("strokes", None, vec![subpath(&[(50.0, 53.0), (55.0, 53.0)], false)], 1.0, RED);
    assert_eq!(after.elements()[4], kept);
    assert_eq!(said.len(), 2, "{said:?}");
    assert_eq!(
        said[1],
        "warning SC-W1114: 1 shapes lie more than half inside the text marker `m` and cross its edge. They sew as they were traced, beside the \
         lettering."
    );
}

#[test]
fn req_auto_063_a_name_marks_text_and_an_object_drawn_twice_is_one_marker() {
    assert_eq!(MARKER, "stitch:text");
    assert_eq!(pictures::note("stitch:text SAMPLE PATCH"), Some("SAMPLE PATCH"));
    assert_eq!(pictures::note("  Stitch:TEXT\t ab  "), Some("ab"));
    assert_eq!(pictures::note("stitch:text"), Some(""));
    for other in ["stitch:texture ab", "stitch:tex", "stitch:guide", "a stitch:text ab", "", "stitch:textab"] {
        assert_eq!(pictures::note(other), None, "{other}");
    }
    let named = marker("stitch:text ab", 40.0);
    assert_eq!(Marker::of(&named), Some(Marker { label: "m".to_string(), shape: named.shape.clone(), note: "ab".to_string() }));
    assert_eq!(Marker::of(&fill("l", vec![rect(0.0, 0.0, 1.0, 1.0)], RED)), None);
    assert_eq!(Marker::of(&Element { name: Some("a label".to_string()), ..named.clone() }), None);
    // The fill and the stroke of one object are 2 elements of one name and one path: its text is set once.
    let filled = Element {
        id: ElementId::new("m:fill").unwrap(),
        shape: Shape::Fill { path: named.shape.path().clone(), rule: FillRule::NonZero, gradient: None },
        ..named.clone()
    };
    let mut elements = letters(32.0);
    elements.extend([filled, named]);
    let (after, said) = replaced(design(elements));
    assert_eq!(ids(&after), ["m:fill:0-svg:pa:stroke", "m:fill:1-svg:pb:stroke"]);
    assert_eq!(said.len(), 1, "{said:?}");
    // 2 markers of one text on unlike paths are 2 markers, and so are 2 texts on one path.
    let mut elements = letters(32.0);
    elements.extend([marker("stitch:text ab", 40.0), line("n", Some("stitch:text ab"), vec![rect(60.0, 25.0, 70.0, 60.0)], 0.3, BLUE)]);
    assert!(replaced(design(elements)).1[1].starts_with("warning SC-W1113: The text marker `n` is not used: no shape lies wholly inside it."));
    let mut elements = letters(32.0);
    elements.extend([marker("stitch:text ab", 40.0), Element { id: ElementId::new("n").unwrap(), ..marker("stitch:text ba", 40.0) }]);
    assert_eq!(replaced(design(elements)).1.len(), 2);
}

#[test]
fn req_auto_063_a_design_without_markers_is_as_it_was_and_its_settings_stay() {
    let mut given = design(letters(32.0));
    given.sort_colours = true;
    let mut fonts = |_: &str, _: &Budget| -> Found { panic!("no font is asked for") };
    let (after, said) = pictures::replace(given.clone(), &[], &mut fonts, &mut BTreeSet::new(), &Budget::DEFAULT).unwrap();
    assert!(after == given && said.is_empty());
    // A marker given beside the design is used as one in it, and the design's settings stay.
    let mut elements = letters(32.0);
    elements.push(marker("stitch:text ab", 40.0));
    let mut marked = design(elements);
    marked.sort_colours = true;
    marked.settings.collapse_len = Mm::new(1.5).unwrap();
    let more = Marker { label: "m".to_string(), shape: marker("", 40.0).shape, note: "ab".to_string() };
    let font = test_font();
    let mut fonts = |_: &str, _: &Budget| Found::Font(Rc::clone(&font));
    let (inside, _) = pictures::replace(marked.clone(), &[], &mut fonts, &mut BTreeSet::new(), &Budget::DEFAULT).unwrap();
    let (beside, _) =
        pictures::replace(design(letters(32.0)), std::slice::from_ref(&more), &mut fonts, &mut BTreeSet::new(), &Budget::DEFAULT).unwrap();
    assert!(inside.elements() == beside.elements() && inside.sort_colours && !beside.sort_colours && inside.settings == marked.settings);
    assert_eq!(ids(&inside), [A, B]);
}

#[test]
fn req_auto_064_the_lettering_is_as_tall_as_the_traced_letters_and_stands_where_they_stood() {
    // The traced letters' box is 22 mm wide and 20 tall, from (10, 30): the text at twice the font's size
    // is exactly as wide, so each glyph stands where the font sets it.
    let mut elements = letters(32.0);
    elements.push(marker("stitch:text ab", 40.0));
    let (after, said) = replaced(design(elements));
    near(&points(&after, A), &[(10.0, 30.0), (18.0, 50.0)]);
    near(&points(&after, B), &[(20.0, 30.0), (32.0, 50.0)]);
    assert_eq!(said.len(), 1, "{said:?}");
    // The widths of the glyphs' strokes are scaled with them.
    assert!(matches!(after.elements()[0].shape, Shape::Stroke { width, .. } if (width.get() - 0.8).abs() < 1e-9));
    // Traced 4 mm wider, the letters are spread by the 4 mm between them. A millimetre narrower, they
    // close up by it.
    for (right, b) in [(36.0, 24.0), (31.0, 19.0)] {
        let mut elements = letters(right);
        elements.push(marker("stitch:text ab", 40.0));
        let (after, said) = replaced(design(elements));
        near(&points(&after, A), &[(10.0, 30.0), (18.0, 50.0)]);
        near(&points(&after, B), &[(b, 30.0), (b + 12.0, 50.0)]);
        assert_eq!(said.len(), 1, "{said:?}");
    }
}

#[test]
fn req_auto_064_the_letters_spread_and_close_up_within_limits_and_are_centred() {
    assert_eq!((CLOSER, APART), (0.1, 0.5));
    // Half the letters' height further apart is exactly allowed, a tenth closer too: nothing is said.
    for (right, b) in [(42.0, 30.0), (30.0, 18.0)] {
        let mut elements = letters(right);
        elements.push(marker("stitch:text ab", 45.0));
        let (after, said) = replaced(design(elements));
        near(&points(&after, A), &[(10.0, 30.0), (18.0, 50.0)]);
        near(&points(&after, B)[..1], &[(b, 30.0)]);
        assert_eq!(said.len(), 1, "{said:?}");
    }
    // Traced 40 mm wide, the letters are spread by the 10 mm allowed, to 32 mm, and centred: 4 mm in.
    let mut elements = letters(50.0);
    elements.push(marker("stitch:text ab", 55.0));
    let (after, said) = replaced(design(elements));
    near(&points(&after, A), &[(14.0, 30.0), (22.0, 50.0)]);
    near(&points(&after, B), &[(34.0, 30.0), (46.0, 50.0)]);
    assert_eq!(said.len(), 2, "{said:?}");
    assert!(said[1].contains("is 20.0 mm tall and 32.0 mm wide, where the traced letters are 20.0 mm tall and 40.0 mm wide"), "{said:?}");
    // Traced 17.9 mm wide, they close up by the 2 mm allowed, to 20 mm, and reach 1.05 mm past each side.
    let mut elements = letters(27.9);
    elements.push(marker("stitch:text ab", 40.0));
    let (after, said) = replaced(design(elements));
    near(&points(&after, A), &[(8.95, 30.0), (16.95, 50.0)]);
    near(&points(&after, B), &[(16.95, 30.0), (28.95, 50.0)]);
    assert!(said.len() == 2 && said[1].starts_with("warning SC-W1115: "), "{said:?}");
    // A letter of no width stands in the middle of the traced letters.
    let mut elements = letters(32.0);
    elements.push(marker("stitch:text i", 40.0));
    let (after, said) = replaced(design(elements));
    near(&points(&after, "m:0-svg:pi:stroke"), &[(21.0, 30.0), (21.0, 50.0)]);
    assert_eq!(said.len(), 1, "{said:?}");
    // A single traced stroke has no width either: the letter is centred on it.
    let stroke = line("s", None, vec![subpath(&[(10.0, 30.0), (10.0, 50.0)], false)], 1.0, RED);
    let (after, said) = replaced(design(vec![stroke, marker("stitch:text a", 40.0)]));
    near(&points(&after, "m:0-svg:pa:stroke"), &[(6.0, 30.0), (14.0, 50.0)]);
    assert_eq!(said.len(), 1, "{said:?}");
    // One letter is never spread, and its width is not said.
    let mut elements = letters(32.0);
    elements.push(marker("stitch:text a", 40.0));
    let (after, said) = replaced(design(elements));
    near(&points(&after, "m:0-svg:pa:stroke"), &[(17.0, 30.0), (25.0, 50.0)]);
    assert_eq!(said.len(), 1, "{said:?}");
}

#[test]
fn req_auto_064_the_baseline_is_where_the_text_reaches_below_it_and_words_are_spread_as_letters() {
    // "ap" is 13 mm tall at the font's size, 3 of them below the baseline. Letters 26 mm tall give 200 %:
    // the baseline lies 6 mm above their bottom.
    let tall = vec![fill("l1", vec![rect(10.0, 30.0, 18.0, 56.0)], RED), fill("l2", vec![rect(20.0, 30.0, 28.0, 56.0)], RED)];
    let mut elements = tall;
    elements.push(marker("stitch:text ap", 40.0));
    let (after, said) = replaced(design(elements));
    near(&points(&after, A), &[(10.0, 30.0), (18.0, 50.0)]);
    near(&points(&after, "m:1-svg:pp:stroke"), &[(20.0, 40.0), (28.0, 56.0)]);
    assert_eq!(said.len(), 1, "{said:?}");
    // "a b" is 12 mm wide at the font's size: a, a space of 3 after its advance of 5, and b. Traced 30 mm
    // wide at 200 %, the gap between the words takes the 6 mm left.
    let mut elements = letters(40.0);
    elements.push(marker("stitch:text a b", 45.0));
    let (after, said) = replaced(design(elements));
    near(&points(&after, A), &[(10.0, 30.0), (18.0, 50.0)]);
    near(&points(&after, B), &[(28.0, 30.0), (40.0, 50.0)]);
    assert_eq!(said.len(), 1, "{said:?}");
}

#[test]
fn req_auto_064_letters_the_font_cannot_sew_as_tall_are_set_at_its_nearest_height() {
    // Letters 30 mm tall, where the font sews up to 20: set at 20 mm, centred on them.
    let mut elements = vec![fill("l1", vec![rect(10.0, 30.0, 18.0, 60.0)], RED), fill("l2", vec![rect(22.0, 30.0, 32.0, 60.0)], RED)];
    elements.push(line("m", Some("stitch:text ab"), vec![rect(5.0, 25.0, 40.0, 65.0)], 0.3, BLUE));
    let (after, said) = replaced(design(elements));
    near(&points(&after, A), &[(10.0, 35.0), (18.0, 55.0)]);
    assert_eq!(said.len(), 3, "{said:?}");
    assert_eq!(
        said[0],
        "warning SC-W1106: The scale 300 % is outside the range the font `test` sews at, 50 to 200 % (5.0 to 20.0 mm tall). The text is set at \
         200 %."
    );
    assert!(said[1].ends_with("30.0 mm tall, are replaced by the text \"ab\" in the font `test`, 20.0 mm tall."), "{said:?}");
    assert!(said[2].starts_with("warning SC-W1115: "), "{said:?}");
    // Letters 2 mm tall are set at the 5 mm the font sews from, and say so the same way.
    let mut elements = vec![fill("l1", vec![rect(10.0, 30.0, 10.8, 32.0)], RED), fill("l2", vec![rect(11.0, 30.0, 12.2, 32.0)], RED)];
    elements.push(marker("stitch:text ab", 40.0));
    let (after, said) = replaced(design(elements));
    near(&points(&after, A)[..1], &[(8.45, 28.5)]);
    assert!(
        said[0].starts_with("warning SC-W1106: The scale 20 % is outside") && said[2].contains("is 5.0 mm tall and 5.3 mm wide, where"),
        "{said:?}"
    );
    // The scale is found to a tenth of a percent.
    let mut elements = vec![fill("l1", vec![rect(10.0, 30.0, 18.0, 43.337)], RED)];
    elements.push(marker("stitch:text a", 40.0));
    let (after, _) = replaced(design(elements));
    near(&points(&after, "m:0-svg:pa:stroke"), &[(11.332, 29.9985), (16.668, 43.3385)]);
}

#[test]
fn req_auto_064_the_lettering_takes_the_thread_that_painted_the_most_of_what_was_taken() {
    // Traced lines count by their length and width: 20 mm of a line 2 mm wide outdoes a red area of 30 mm².
    let stroke = line("s", None, vec![subpath(&[(10.0, 30.0), (10.0, 50.0)], false)], 2.0, WHITE);
    let dot = fill("dot", vec![rect(20.0, 30.0, 26.0, 35.0)], RED);
    let (after, _) = replaced(design(vec![dot.clone(), stroke, marker("stitch:text ab", 40.0)]));
    assert!(after.elements().iter().all(|letter| letter.thread == Thread::new(WHITE)));
    assert_eq!(ids(&after), [A, B]);
    // A line 1.4 mm wide paints 28 mm², and the red wins. Of 2 threads that painted as much, the first.
    let thin = line("s", None, vec![subpath(&[(10.0, 30.0), (10.0, 50.0)], false)], 1.4, WHITE);
    let (after, _) = replaced(design(vec![dot.clone(), thin, marker("stitch:text ab", 40.0)]));
    assert!(after.elements().iter().all(|letter| letter.thread == Thread::new(RED)));
    let even = line("s", None, vec![subpath(&[(10.0, 30.0), (10.0, 50.0)], false)], 1.5, WHITE);
    let (after, _) = replaced(design(vec![even.clone(), dot.clone(), marker("stitch:text ab", 40.0)]));
    assert!(after.elements().iter().all(|letter| letter.thread == Thread::new(WHITE)));
    let (after, _) = replaced(design(vec![dot, even, marker("stitch:text ab", 40.0)]));
    assert!(after.elements().iter().all(|letter| letter.thread == Thread::new(RED)));
    // A hole counts against its thread: a white frame's hole, as large as the red letter in it, leaves
    // the white behind. A subpath of another element round it doesn't make a shape a hole.
    let frame = fill("frame", vec![rect(8.0, 28.0, 30.0, 52.0), rect(10.0, 30.0, 28.0, 50.0)], WHITE);
    let letter = fill("letter", vec![rect(10.0, 30.0, 28.0, 50.0)], RED);
    let (after, _) = replaced(design(vec![frame, letter, marker("stitch:text ab", 40.0)]));
    assert!(after.elements().iter().all(|letter| letter.thread == Thread::new(RED)));
    // A font of several threads keeps its own.
    let mut elements = letters(32.0);
    elements.push(marker("stitch:text ac", 40.0));
    let (after, _) = replaced(design(elements));
    assert_eq!([after.elements()[0].thread.color, after.elements()[1].thread.color], [Rgb::new(0, 0, 0), Rgb::new(0, 255, 0)]);
    // A name the font gave a glyph's path is not kept: the lettering is never read as a marker.
    assert!(after.elements().iter().all(|letter| letter.name.is_none()));
}

#[test]
fn req_auto_064_the_text_is_measured_by_what_it_sews_at_the_fonts_own_size() {
    // The curve of `u` sews from 1 mm right of its box's left edge to 4 mm: 3 mm wide, where its box is 4.
    // At 200 % it is 6 mm wide and centred on the traced letters, whose middle is at x = 21: its ends at 24,
    // its control points 8 mm to their left.
    let mut elements = letters(32.0);
    elements.push(marker("stitch:text u", 40.0));
    let (after, said) = replaced(design(elements));
    near(&points(&after, "m:0-svg:pu:stroke"), &[(24.0, 30.0), (16.0, 40.0), (16.0, 40.0), (24.0, 50.0)]);
    assert_eq!(said.len(), 1, "{said:?}");
    // A font that sews from twice its own size is measured at twice its size, and set as any other.
    let large = font_sewn(&[glyph("a", "#000000", 4.0, 60.0, 70.0), glyph("b", "#000000", 6.0, 60.0, 70.0)].concat(), 5.0, (2.0, 4.0), "");
    let mut elements = letters(32.0);
    elements.push(marker("stitch:text ab", 40.0));
    let (after, said) =
        pictures::replace(design(elements), &[], &mut |_, _| Found::Font(Rc::clone(&large)), &mut BTreeSet::new(), &Budget::DEFAULT).unwrap();
    near(&points(&after, A), &[(10.0, 30.0), (18.0, 50.0)]);
    near(&points(&after, B), &[(20.0, 30.0), (32.0, 50.0)]);
    assert_eq!(said.len(), 1, "{said:?}");
}

#[test]
fn req_auto_065_a_line_drawn_through_the_letters_marks_them_and_the_text_follows_it() {
    // A line drawn down the page, 30 mm wide. The letters' tops are to its left as it runs, which is to the
    // right on the page: the letters' boxes lie 20 mm across it, from 10 to 32 mm along it.
    let down = |name: &str| line("m", Some(name), vec![subpath(&[(20.0, 0.0), (20.0, 100.0)], false)], 30.0, BLUE);
    let turned: Vec<Element> = vec![fill("l1", vec![rect(10.0, 10.0, 30.0, 18.0)], RED), fill("l2", vec![rect(10.0, 22.0, 30.0, 32.0)], RED)];
    let mut elements = turned;
    elements.push(down("stitch:text ab"));
    let (after, said) = replaced(design(elements));
    assert_eq!(ids(&after), [A, B]);
    near(&points(&after, A), &[(30.0, 10.0), (10.0, 18.0)]);
    near(&points(&after, B), &[(30.0, 20.0), (10.0, 32.0)]);
    assert_eq!(
        said,
        ["info SC-I1112: The text marker `m`: 2 traced outlines, 20.0 mm tall, are replaced by the text \"ab\" in the font `test`, 20.0 mm tall."]
    );
    // What lies further from the line than half its width, or past an end of it, is not inside.
    let wide = fill("wide", vec![rect(4.9, 40.0, 30.0, 42.0)], RED);
    let edge = fill("edge", vec![rect(5.0, 44.0, 35.0, 46.0)], RED);
    let past = fill("past", vec![rect(15.0, 90.0, 25.0, 100.1)], RED);
    let ends = fill("ends", vec![rect(15.0, 0.0, 25.0, 4.0), rect(15.0, 96.0, 25.0, 100.0)], RED);
    let before = fill("before", vec![rect(15.0, -0.1, 25.0, 4.0)], RED);
    let mut elements = vec![wide, edge, past, ends, before];
    elements.push(down("stitch:text ab"));
    let (after, _) = replaced(design(elements));
    assert_eq!(ids(&after), ["wide", "past", A, B, "before"]);
    // A closed line is an area, whatever its width.
    let mut elements = letters(32.0);
    elements.push(line("m", Some("stitch:text ab"), vec![rect(5.0, 25.0, 40.0, 60.0)], 30.0, BLUE));
    assert_eq!(ids(&replaced(design(elements)).0), [A, B]);
}

#[test]
fn req_auto_065_on_an_arc_each_letter_is_turned_and_stands_between_the_traced_letters_edges() {
    // A quarter of a circle of radius 40 round (50, 50), drawn clockwise from its left to its top, as 2
    // cubics would trace it: here its polyline, a point each 3 degrees. The letters' tops are outside it.
    let on_arc = |radius: f64, degrees: f64| {
        let turn = stitchcraft_core::math::to_radians(180.0 + degrees);
        (50.0 + radius * stitchcraft_core::math::cos(turn), 50.0 + radius * stitchcraft_core::math::sin(turn))
    };
    let arc: Vec<(f64, f64)> = (0..=30).map(|step| on_arc(40.0, f64::from(step) * 3.0)).collect();
    // 2 traced letters, each a box bent with the arc: from radius 35 to 45, and from 20 to 26 and 28 to 38
    // degrees round, which is 11 mm along their feet, as wide as the text.
    let ring = |inner: f64, from: u32, to: u32| {
        let outer = (from..=to).map(|degree| on_arc(inner + 10.0, f64::from(degree)));
        let inside = (from..=to).rev().map(|degree| on_arc(inner, f64::from(degree)));
        subpath(&outer.chain(inside).collect::<Vec<_>>(), true)
    };
    let bent = |from: u32, to: u32| ring(35.0, from, to);
    let elements = vec![
        fill("l1", vec![bent(20, 26)], RED),
        fill("l2", vec![bent(28, 38)], RED),
        line("m", Some("stitch:text ab"), vec![subpath(&arc, false)], 12.0, BLUE),
    ];
    let (after, said) = replaced(design(elements));
    assert_eq!(ids(&after), [A, B]);
    assert_eq!(said.len(), 1, "{said:?}");
    let radius = |(x, y): (f64, f64)| stitchcraft_core::math::hypot(x - 50.0, y - 50.0);
    let round = |(x, y): (f64, f64)| stitchcraft_core::math::to_degrees(stitchcraft_core::math::atan2(y - 50.0, x - 50.0)) + 180.0;
    for (id, first, last) in [(A, 20.0, 27.0), (B, 27.0, 38.0)] {
        let [top, foot] = points(&after, id)[..] else { panic!("a glyph is one stroke") };
        // Each stroke runs from the top of its letter, on the traced letters' outer edge, to its foot on
        // their inner edge, within the 0.2 mm a chord of the arc lies inside it.
        assert!((radius(top) - 45.0).abs() < 0.2 && (radius(foot) - 35.0).abs() < 0.2, "{id}: {} {}", radius(top), radius(foot));
        // And between the traced letters' ends round the arc, in reading order.
        assert!(first - 1.0 < round(top) && round(top) < round(foot) && round(foot) < last + 1.0, "{id}: {} {}", round(top), round(foot));
    }
    // The foot of the last letter is on the baseline to a hundredth of a millimetre: the baseline goes on
    // past the text, and no glyph stands past its end on a straight line.
    assert!((radius(points(&after, B)[1]) - 35.0).abs() < 0.015, "{}", radius(points(&after, B)[1]));
    // Traced letters far apart: the text spreads by the 5 mm allowed and is centred between their ends,
    // at 45 degrees round, within half a degree.
    let apart = vec![
        fill("l1", vec![bent(20, 26)], RED),
        fill("l2", vec![bent(60, 70)], RED),
        line("m", Some("stitch:text ab"), vec![subpath(&arc, false)], 12.0, BLUE),
    ];
    let (wide, said) = replaced(design(apart));
    assert!(said.len() == 2 && said[1].starts_with("warning SC-W1115: "), "{said:?}");
    let middle = (round(points(&wide, A)[0]) + round(points(&wide, B)[1])) / 2.0;
    assert!((middle - 45.0).abs() < 0.5, "{middle}");
    // Letters well inside the line, from radius 22 to 32 under a stroke 40 mm wide: a millimetre of their
    // baseline is 1.8 mm of the marker's line, and the baseline is as long as the 4 letters need. The foot
    // of the last one is on it.
    let inner = vec![fill("l1", vec![ring(22.0, 20, 75)], RED), line("m", Some("stitch:text abab"), vec![subpath(&arc, false)], 40.0, BLUE)];
    let (tight, said) = replaced(design(inner));
    assert_eq!(said.len(), 1, "{said:?}");
    let foot = radius(points(&tight, "m:3-svg:pb:stroke")[1]);
    assert!((foot - 22.0).abs() < 0.015, "{foot}");
    // The text's ends are the traced letters' ends, within a degree: a glyph stands on the chord under it.
    let (start, end) = (round(points(&after, A)[0]), round(points(&after, B)[1]));
    assert!((start - 20.0).abs() < 1.0 && (end - 38.0).abs() < 1.0, "{start} {end}");
}

/// What `replace` says of `elements`, with the test font.
fn said_of(elements: Vec<Element>) -> Vec<String> {
    replaced(design(elements)).1
}

/// The design of 2 letters and the marker `mark`, what is left of it and what is said.
fn with_marker(mark: Element) -> (Vec<String>, Vec<String>) {
    let mut elements = letters(32.0);
    elements.push(mark);
    let (after, said) = replaced(design(elements));
    (ids(&after).into_iter().map(str::to_string).collect(), said)
}

const UNUSED_HINT: &str = "Name a shape drawn round the letters, or a line drawn through them, `stitch:text` and the text.";

#[test]
fn diag_sc_w1113_a_marker_that_cannot_be_used_leaves_the_traced_shapes_and_is_not_sewn() {
    let unused = |why: &str| vec![format!("warning SC-W1113: The text marker `m` is not used: {why} What lies inside it sews as it was traced.")];
    let kept = ["l1", "l2"];
    // No text.
    for name in ["stitch:text", "stitch:text  ", "stitch:text \"\"", "stitch:text \" \" font=test"] {
        assert_eq!(with_marker(marker(name, 40.0)), (kept.map(str::to_string).to_vec(), unused("it has no text.")), "{name}");
    }
    // No area: an outline of no height, and one of no width.
    let flat = |points: &[(f64, f64)]| line("m", Some("stitch:text ab"), vec![subpath(points, true)], 0.3, BLUE);
    for points in [&[(5.0, 25.0), (20.0, 25.0), (40.0, 25.0)][..], &[(5.0, 25.0), (5.0, 40.0), (5.0, 60.0)]] {
        assert_eq!(with_marker(flat(points)).1, unused("it encloses no area."), "{points:?}");
    }
    assert_eq!(with_marker(line("m", Some("stitch:text ab"), Vec::new(), 0.3, BLUE)).1, unused("it encloses no area."));
    // A line of no length.
    let dot = line("m", Some("stitch:text ab"), vec![subpath(&[(5.0, 25.0), (5.0, 25.0)], false)], 9.0, BLUE);
    assert_eq!(with_marker(dot).1, unused("its line has no length."));
    // Nothing wholly inside.
    assert_eq!(with_marker(marker("stitch:text ab", 9.0)).1, unused("no shape lies wholly inside it."));
    // A text the font draws no character of: what lettering says.
    assert_eq!(
        with_marker(marker("stitch:text zz", 40.0)).1,
        unused("None of the text's characters sews in the font `test`: the text is empty, or only spaces.")
    );
    // Shapes of no height: a traced line along the text's line.
    let level = vec![line("l", None, vec![subpath(&[(10.0, 40.0), (30.0, 40.0)], false)], 1.0, RED), marker("stitch:text ab", 40.0)];
    assert_eq!(said_of(level), unused("the shapes inside it, or its text in the font, have no height to measure."));
    // A text of no height in the font: a letter drawn along the baseline.
    assert_eq!(with_marker(marker("stitch:text e", 40.0)).1, unused("the shapes inside it, or its text in the font, have no height to measure."));
    // A name too long to number the letters by.
    let long = "m".repeat(ElementId::MAX_LEN - 3);
    let mut elements = letters(32.0);
    elements.push(Element { id: ElementId::new(long.clone()).unwrap(), ..marker("stitch:text ab", 40.0) });
    assert_eq!(
        said_of(elements)[0],
        format!(
            "warning SC-W1113: The text marker `{long}` is not used: its name is too long to number its letters by. What lies inside it sews as it was traced."
        )
    );
    // The hint says how a marker is made.
    let mut elements = letters(32.0);
    elements.push(marker("stitch:text", 40.0));
    let font = test_font();
    let (_, said) =
        pictures::replace(design(elements), &[], &mut |_, _| Found::Font(Rc::clone(&font)), &mut BTreeSet::new(), &Budget::DEFAULT).unwrap();
    assert_eq!((said[0].code, said[0].fix.as_ref().map(|fix| fix.describe()).as_deref()), (Code::TextMarkerNotUsed, Some(UNUSED_HINT)));
}

#[test]
fn diag_sc_w1113_a_font_that_is_not_there_or_does_not_read_is_named() {
    let said_with = |name: &str, found: &dyn Fn(&str) -> Found| {
        let mut elements = letters(32.0);
        elements.push(marker(name, 40.0));
        let (after, said) = pictures::replace(design(elements), &[], &mut |asked, _| found(asked), &mut BTreeSet::new(), &Budget::DEFAULT).unwrap();
        assert_eq!(ids(&after), ["l1", "l2"]);
        said.iter().map(ToString::to_string).collect::<Vec<_>>()
    };
    let unused = |why: &str| vec![format!("warning SC-W1113: The text marker `m` is not used: {why} What lies inside it sews as it was traced.")];
    assert_eq!(said_with("stitch:text ab", &|_| Found::Missing(Vec::new())), unused("The default embroidery font is not here, and no other is."));
    let listed = |asked: &str| {
        assert_eq!(asked, "other");
        Found::Missing(vec!["Test (`test`)".to_string(), "`more`".to_string()])
    };
    assert_eq!(
        said_with("stitch:text \"ab\" font=other", &listed),
        unused("The embroidery font `other` is not here. The fonts here are Test (`test`), `more`.")
    );
    let unread = |_: &str| Found::Unread(Diagnostic::new(Code::FontUnreadable, "It has no font.json."));
    assert_eq!(said_with("stitch:text ab", &unread), unused("The default embroidery font does not read. It has no font.json."));
}

#[test]
fn diag_sc_w1113_a_setting_that_does_not_read_is_named_and_the_others_are_used() {
    let mut asked = Vec::new();
    let font = test_font();
    let mut elements = letters(32.0);
    elements.push(marker("stitch:text \"a\"b\" trim=glyph font=test colour=red color_sort=maybe font trim=never color_sort=all", 40.0));
    let mut fonts = |name: &str, _: &Budget| {
        asked.push(name.to_string());
        Found::Font(Rc::clone(&font))
    };
    let (after, said) = pictures::replace(design(elements), &[], &mut fonts, &mut BTreeSet::new(), &Budget::DEFAULT).unwrap();
    assert_eq!(asked, ["test"]);
    // The text runs to the last quote: `a"b`, whose quote the font lacks.
    let said: Vec<String> = said.iter().map(ToString::to_string).collect();
    assert!(said[0].starts_with("warning SC-W1107: ") && said[2].contains("the text \"a\"b\" in the font"), "{said:?}");
    // The font doesn't sort its colours, which lettering says of the sorting asked for.
    assert!(said[1].starts_with("warning SC-W1110: "), "{said:?}");
    let unread: Vec<&str> = said.iter().map(String::as_str).filter(|said| said.contains("not used in full")).collect();
    let not_read = |setting: &str| {
        format!(
            "warning SC-W1113: The text marker `m` is not used in full: its setting `{setting}` does not read, and is left out. A marker reads \
             font, trim and color_sort, each as name=value after the quoted text."
        )
    };
    assert_eq!(unread, [not_read("colour=red"), not_read("color_sort=maybe"), not_read("font"), not_read("trim=never")]);
    // The trim asked for after each glyph is added.
    assert_eq!(after.elements()[0].params.get("trim_after"), Some("true"));
    // A note without quotes is all text, and so is one with a single quote.
    let (left, said) = with_marker(marker("stitch:text ab trim=glyph", 40.0));
    assert!(left.len() == 4 && said[1].contains("the text \"ab trim=glyph\""), "{left:?} {said:?}");
    assert!(with_marker(marker("stitch:text \"ab", 40.0)).1[1].contains("the text \"\"ab\""));
    // Without a trim asked for, none is added.
    let (after, _) = replaced(design({
        let mut elements = letters(32.0);
        elements.push(marker("stitch:text \"ab\"", 40.0));
        elements
    }));
    assert_eq!(after.elements()[0].params.get("trim_after"), None);
}

#[test]
fn diag_sc_i1112_the_replacement_is_said_once_a_marker() {
    let mut elements = letters(32.0);
    elements.push(marker("stitch:text ab", 40.0));
    let font = test_font();
    let (_, said) =
        pictures::replace(design(elements), &[], &mut |_, _| Found::Font(Rc::clone(&font)), &mut BTreeSet::new(), &Budget::DEFAULT).unwrap();
    assert_eq!(said.len(), 1);
    assert_eq!((said[0].code, said[0].fix.as_ref()), (Code::TextInPictureLettered, None));
    assert_eq!(
        said[0].to_string(),
        "info SC-I1112: The text marker `m`: 2 traced outlines, 20.0 mm tall, are replaced by the text \"ab\" in the font `test`, 20.0 mm tall."
    );
}

#[test]
fn diag_sc_w1114_shapes_that_cross_a_markers_edge_are_counted() {
    let poke = |id: &str, y: f64| fill(id, vec![rect(30.0, y, 41.0, y + 1.0)], RED);
    let mut elements = letters(32.0);
    elements.extend([poke("p1", 52.0), poke("p2", 55.0), marker("stitch:text ab", 40.0)]);
    let font = test_font();
    let (after, said) =
        pictures::replace(design(elements), &[], &mut |_, _| Found::Font(Rc::clone(&font)), &mut BTreeSet::new(), &Budget::DEFAULT).unwrap();
    assert_eq!(ids(&after), [A, B, "p1", "p2"]);
    assert_eq!(said[1].code, Code::ShapeCrossesTextMarker);
    assert_eq!(
        said[1].to_string(),
        "warning SC-W1114: 2 shapes lie more than half inside the text marker `m` and cross its edge. They sew as they were traced, beside the \
         lettering."
    );
    assert_eq!(
        said[1].fix.as_ref().map(|fix| fix.describe()).as_deref(),
        Some("Draw the marker round every letter of the text, and round nothing else.")
    );
}

#[test]
fn diag_sc_w1115_lettering_that_is_not_the_traced_letters_size_gives_both() {
    let mut elements = letters(50.0);
    elements.push(marker("stitch:text ab", 55.0));
    let font = test_font();
    let (_, said) =
        pictures::replace(design(elements), &[], &mut |_, _| Found::Font(Rc::clone(&font)), &mut BTreeSet::new(), &Budget::DEFAULT).unwrap();
    assert_eq!(said[1].code, Code::LetteringUnlikeTracedLetters);
    assert_eq!(
        said[1].to_string(),
        "warning SC-W1115: The lettering of the text marker `m` is 20.0 mm tall and 32.0 mm wide, where the traced letters are 20.0 mm tall and \
         40.0 mm wide. It is centred on them."
    );
    assert_eq!(
        said[1].fix.as_ref().map(|fix| fix.describe()).as_deref(),
        Some("Choose a font that sews at the letters' height and is about as wide, with font=NAME after the quoted text.")
    );
    // A width within a tenth of the letters' height of the traced letters' is not said: 2 mm here, where
    // the letters spread to 32 mm at most.
    assert_eq!(FIT, 0.1);
    for (right, count) in [(44.0, 1), (44.001, 2), (27.999, 2), (28.0, 1)] {
        let mut elements = letters(right);
        elements.push(marker("stitch:text ab", 55.0));
        assert_eq!(said_of(elements).len(), count, "{right}");
    }
}

#[test]
fn req_auto_066_a_fonts_own_warnings_are_said_once_and_work_is_bounded() {
    // A font whose settings name a glyph file it lacks warns of it when it is read.
    let warned = font_of(&glyph("a", "#000000", 4.0, 60.0, 70.0), r#", "leading": "tall""#);
    assert!(!warned.warnings.is_empty(), "the font should warn");
    let own: Vec<String> = warned.warnings.iter().map(ToString::to_string).collect();
    let two = || {
        let mut elements = letters(32.0);
        elements.extend([
            marker("stitch:text a", 20.0),
            Element { id: ElementId::new("n").unwrap(), ..line("n", Some("stitch:text a"), vec![rect(20.0, 25.0, 40.0, 60.0)], 0.3, BLUE) },
        ]);
        design(elements)
    };
    let mut said = BTreeSet::new();
    let (after, first) = pictures::replace(two(), &[], &mut |_, _| Found::Font(Rc::clone(&warned)), &mut said, &Budget::DEFAULT).unwrap();
    assert_eq!(ids(&after), ["m:0-svg:pa:stroke", "n:0-svg:pa:stroke"]);
    let first: Vec<String> = first.iter().map(ToString::to_string).collect();
    assert_eq!(first[..own.len()], own[..]);
    assert_eq!(first.len(), own.len() + 2, "{first:?}");
    assert_eq!(said, BTreeSet::from(["test".to_string()]));
    // Said already, they are not said again.
    let (_, again) = pictures::replace(two(), &[], &mut |_, _| Found::Font(Rc::clone(&warned)), &mut said, &Budget::DEFAULT).unwrap();
    assert_eq!(again.len(), 2);
    // With too little work allowed, the reading stops with the budget's error, whatever was reached.
    let font = test_font();
    let least = |design: &dyn Fn() -> Design| {
        let mut needed = 0;
        loop {
            let budget = Budget { max_work: needed, ..Budget::DEFAULT };
            match pictures::replace(design(), &[], &mut |_, _| Found::Font(Rc::clone(&font)), &mut BTreeSet::new(), &budget) {
                Ok(_) => return needed,
                Err(stopped) => assert_eq!(stopped.code, Code::BudgetExhausted, "at {needed}: {stopped}"),
            }
            needed += 1;
            assert!(needed < 5_000, "the work never fits");
        }
    };
    // The work of 2 areas, and of a line of 2 stretches through the letters, to the unit: every point
    // tested costs a unit per segment it is tested against.
    let bent = || {
        let mut elements = letters(32.0);
        elements.push(line("m", Some("stitch:text ab"), vec![subpath(&[(8.0, 40.0), (20.0, 40.0), (34.0, 40.0)], false)], 24.0, BLUE));
        design(elements)
    };
    assert_eq!((least(&two), least(&bent)), (131, 65));
    // A font whose reader ran out of budget stops the reading too.
    let spent = |_: &str, _: &Budget| Found::Unread(Diagnostic::new(Code::BudgetExhausted, "The budget ran out."));
    let stopped = pictures::replace(two(), &[], &mut { spent }, &mut BTreeSet::new(), &Budget::DEFAULT).unwrap_err();
    assert_eq!(stopped.code, Code::BudgetExhausted);
}

/// A font that draws the letters of MATCH, each a black stroke 8 mm wide and 10 tall that advances 9.5 mm:
/// 46 mm for the word, where the test sheets' block letters take 48.2 mm at that height.
fn match_font() -> Rc<Font> {
    let layers: String = ["M", "A", "T", "C", "H"].iter().map(|letter| glyph(letter, "#000000", 8.0, 60.0, 70.0)).collect();
    font_with(&layers, 9.5, "")
}

/// The box round the element `id`: its left, top, right and bottom.
fn box_of(design: &Design, id: &str) -> [f64; 4] {
    let points = points(design, id);
    let fold = |pick: fn(&(f64, f64)) -> f64, start: f64, best: fn(f64, f64) -> f64| points.iter().map(pick).fold(start, best);
    [
        fold(|p| p.0, f64::INFINITY, f64::min),
        fold(|p| p.1, f64::INFINITY, f64::min),
        fold(|p| p.0, f64::NEG_INFINITY, f64::max),
        fold(|p| p.1, f64::NEG_INFINITY, f64::max),
    ]
}

/// `elements` and a marker round the box of `letters` that says MATCH, its letters replaced in [`match_font`].
fn lettered(mut elements: Vec<Element>, letters: &Design) -> (Design, Vec<String>) {
    let bounds = letters.bounds().unwrap();
    let round = rect(bounds.min().x() - 1.0, bounds.min().y() - 1.0, bounds.max().x() + 1.0, bounds.max().y() + 1.0);
    elements.push(line("mark", Some("stitch:text MATCH"), vec![round], 0.3, BLUE));
    let font = match_font();
    let (after, said) =
        pictures::replace(design(elements), &[], &mut |_, _| Found::Font(Rc::clone(&font)), &mut BTreeSet::new(), &Budget::DEFAULT).unwrap();
    (after, said.iter().map(ToString::to_string).collect())
}

#[test]
fn req_auto_064_a_marked_word_of_ts_13b_sews_as_lettering_where_its_letters_stood() {
    // TS-13B's words: MATCH in block letters, each letter a shape, 15, 10 and 6 mm tall.
    for height in [15.0, 10.0, 6.0] {
        let traced = word((3.0, 4.0), height).unwrap();
        let before = design(traced.elements.clone());
        let (after, said) = lettered(traced.elements.clone(), &before);
        // No traced letter is left, and each letter of the text stands in its place.
        let letters: Vec<String> =
            ["M", "A", "T", "C", "H"].iter().enumerate().map(|(at, letter)| format!("mark:{at}-svg:p{letter}:stroke")).collect();
        assert_eq!(ids(&after), letters.iter().map(String::as_str).collect::<Vec<_>>());
        assert_eq!(said.len(), 1, "{height}: {said:?}");
        for (letter, shape) in letters.iter().zip(&traced.elements) {
            let [left, top, right, bottom] = box_of(&after, letter);
            let [was_left, was_top, was_right, was_bottom] = box_of(&before, shape.id.as_str());
            // As tall as the traced letters, on their baseline, to a thousandth of a millimetre: the scale
            // is found to a tenth of a percent.
            assert!((top - was_top).abs() < 0.001 * height && (bottom - was_bottom).abs() < 0.001 * height, "{letter} at {height}: {top} {bottom}");
            // And over its traced letter: its middle within the traced letter's box. A font's letters are
            // not as wide as a picture's, so nearer than that isn't asked: the spacing is spread evenly, and
            // each letter lies off by its own difference in width at most.
            let middle = (left + right) / 2.0;
            assert!(was_left < middle && middle < was_right, "{letter} at {height}: {middle} not in {was_left}..{was_right}");
        }
        // The text's ends are the traced word's ends, within a tenth of a millimetre.
        let (first, last) = (box_of(&after, &letters[0]), box_of(&after, &letters[4]));
        let whole = before.bounds().unwrap();
        assert!((first[0] - whole.min().x()).abs() < 0.1 && (last[2] - whole.max().x()).abs() < 0.1, "{height}: {first:?} {last:?}");
    }
}

#[test]
fn req_auto_064_the_lettered_patch_sews_its_field_whole_and_the_lettering_on_it() {
    // The lettered patch of the before and after sheet: the word 10 mm tall on a cream field with a red
    // border, both with cut corners, each lying whole under what is drawn on it.
    let traced = word((0.0, 0.0), 10.0).unwrap();
    let letters = design(traced.elements.clone());
    let bounds = letters.bounds().unwrap();
    let plate = |id: &str, margin: f64, colour: Rgb| {
        let (left, top, right, bottom) = (bounds.min().x() - margin, bounds.min().y() - margin, bounds.max().x() + margin, bounds.max().y() + margin);
        let cut = margin / 2.0;
        let ring = [
            (left + cut, top),
            (right - cut, top),
            (right, top + cut),
            (right, bottom - cut),
            (right - cut, bottom),
            (left + cut, bottom),
            (left, bottom - cut),
            (left, top + cut),
        ];
        fill(id, vec![subpath(&ring, true)], colour)
    };
    let (border, field) = (plate("border", 9.0, RED), plate("field", 6.0, WHITE));
    let mut elements = vec![border.clone(), field.clone()];
    elements.extend(traced.elements.clone());
    let (after, said) = lettered(elements, &letters);
    assert_eq!(said.len(), 1, "{said:?}");
    assert_eq!(ids(&after)[..2], ["border", "field"]);
    assert_eq!(after.elements().len(), 7);
    // The field and the border are as they were, and no traced letter sews.
    assert!(after.elements()[0] == border && after.elements()[1] == field);
    assert!(traced.elements.iter().all(|letter| !ids(&after).contains(&letter.id.as_str())));
    // The word was traced in black, and the lettering keeps it.
    assert!(after.elements()[2..].iter().all(|letter| letter.thread == traced.elements[0].thread && !letter.automatic));
    // It plans, and sews in 3 threads: the border, the field and, last, the lettering on them.
    let plan = in_place(&after).plan.unwrap();
    let threads: Vec<Rgb> = plan.blocks.iter().map(|block| block.thread.color).collect();
    assert_eq!(threads.len(), 3, "{threads:?}");
    assert_eq!(threads.last(), Some(&traced.elements[0].thread.color));
    // Every stitch of the lettering lies in the traced letters' box, within the millimetre a lock stitch or
    // a pull compensation may reach past a stroke's end.
    let last = plan.blocks.last().unwrap();
    assert!(last.stitches.len() > 20, "{}", last.stitches.len());
    let inside =
        |x: f64, y: f64| bounds.min().x() - 1.0 <= x && x <= bounds.max().x() + 1.0 && bounds.min().y() - 1.0 <= y && y <= bounds.max().y() + 1.0;
    assert!(last.stitches.iter().all(|stitch| inside(stitch.at.x(), stitch.at.y())));
}
