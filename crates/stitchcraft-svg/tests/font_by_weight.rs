//! The font chosen by the letters' weight (roadmap step M14.8, `REQ-AUTO-071` and `REQ-AUTO-072`): a text
//! marker that names no font is set in the font nearest in weight to the traced letters it replaces, among
//! those that sew at their height, and a note says which and why. The fonts are drawn in code. No font file
//! of anyone's is used.

// Test code may unwrap, panic and index (clippy.toml allows it in tests, but not in their helpers).
#![allow(clippy::unwrap_used, clippy::panic)]

use std::collections::BTreeSet;
use std::rc::Rc;

use stitchcraft_core::units::{MM_PER_SVG_PX, Mm, Point};
use stitchcraft_core::{Budget, Code, Diagnostic, ElementId, Fix};
use stitchcraft_engine::design::{Cap, Design, DesignSettings, Element, FillRule, Hints, Join, Path, Segment, Shape, Subpath};
use stitchcraft_engine::testsheets::pictures::word;
use stitchcraft_params::ParamSet;
use stitchcraft_plan::{Rgb, Thread};
use stitchcraft_svg::font::{self, Font};
use stitchcraft_svg::lettering::pictures::{self, Choice, Found, Marker, chosen, weight};

/// A glyph file's root: 100 mm square, a user unit a millimetre.
const ROOT: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" xmlns:inkscape="http://www.inkscape.org/namespaces/inkscape" xmlns:sodipodi="http://sodipodi.sourceforge.net/DTD/sodipodi-0.dtd" xmlns:inkstitch="http://inkstitch.org/namespace" width="100mm" height="100mm" viewBox="0 0 100 100">"#;

/// A font named `folder` of the glyph layers `layers`: 10 mm tall, sewn from 5 to 20 mm, every glyph
/// advancing 5 mm.
fn font_of(folder: &str, layers: &str) -> Rc<Font> {
    let json = format!(r#"{{"name": "Test", "size": 10, "min_scale": 0.5, "max_scale": 2, "auto_satin": false, "horiz_adv_x_default": {}}}"#, 5.0 / MM_PER_SVG_PX);
    let file = format!(
        r#"{ROOT}<sodipodi:namedview id="base"><sodipodi:guide position="0,30" orientation="0,1" inkscape:label="baseline"/></sodipodi:namedview>{layers}</svg>"#
    );
    Rc::new(font::read(folder, &[("font.json", json.as_bytes()), ("ltr.svg", file.as_bytes())], &Budget::DEFAULT).unwrap())
}

/// The glyph `label`, drawn by the path data `d` with the attributes `more`.
fn glyph(label: &str, d: &str, more: &str) -> String {
    format!(
        r#"<g inkscape:groupmode="layer" inkscape:label="GlyphLayer-{label}" style="display:none"><path id="p{label}" d="{d}" style="fill:none;stroke:#000000;stroke-width:0.4" {more}/></g>"#
    )
}

/// A font whose `a` and `b` are strokes 10 mm tall on the baseline.
fn plain(folder: &str) -> Rc<Font> {
    font_of(folder, &[glyph("a", "M 0,60 L 4,70", ""), glyph("b", "M 0,60 L 4,70", "")].concat())
}

fn at((x, y): (f64, f64)) -> Point {
    Point::new(x, y).unwrap()
}

/// The closed subpath round the box from (`x0`, `y0`) to (`x1`, `y1`).
fn rect(x0: f64, y0: f64, x1: f64, y1: f64) -> Subpath {
    let corners = [(x1, y0), (x1, y1), (x0, y1)];
    Subpath { start: at((x0, y0)), segments: corners.iter().map(|corner| Segment::Line(at(*corner))).collect(), closed: true }
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

/// A marker named `name` round the box from (5, 25) to (95, 25 + 10 + `tall`).
fn marker(name: &str, tall: f64) -> Element {
    line("marker", Some(name), vec![rect(5.0, 25.0, 95.0, 35.0 + tall)], 0.3, BLUE)
}

/// 2 traced letters in red, bars `tall` mm tall from y 30 whose strokes are `wide` mm wide, the first from
/// x 10 and the second from x 40, with a marker named `name` round them.
fn bars(name: &str, wide: f64, tall: f64) -> Design {
    let letters = [fill("l1", vec![rect(10.0, 30.0, 10.0 + wide, 30.0 + tall)], RED), fill("l2", vec![rect(40.0, 30.0, 40.0 + wide, 30.0 + tall)], RED)];
    Design::new(letters.into_iter().chain([marker(name, tall)]).collect(), DesignSettings::default()).unwrap()
}

/// A light font that sews from 5 to 20 mm, and a bold one that sews from 12 to 30.
fn choices() -> Vec<Choice> {
    vec![
        Choice { font: "light".to_string(), weight: 0.14, heights: (5.0, 20.0) },
        Choice { font: "bold".to_string(), weight: 0.25, heights: (12.0, 30.0) },
    ]
}

/// `design` with its markers' texts set, a marker that names no font choosing among `choices`: the names
/// the fonts were asked for by, the font each lettered element's id shows aside, and what was said of the
/// choice.
fn chosen_for(design: Design, choices: &[Choice]) -> (Vec<String>, Vec<Diagnostic>) {
    let mut asked = Vec::new();
    let mut fonts = |name: &str, _: &Budget| {
        asked.push(name.to_string());
        Found::Font(plain(if name.is_empty() { "default" } else { name }))
    };
    let (_, said) = pictures::replace_among(design, &[], &mut fonts, choices, &mut BTreeSet::new(), &Budget::DEFAULT).unwrap();
    (asked, said.into_iter().filter(|said| said.code == Code::FontChosenByWeight).collect())
}

/// The hint of what `said` proposes.
fn hint(said: &Diagnostic) -> &str {
    match &said.fix {
        Some(Fix::Hint(hint)) => hint,
        other => panic!("{other:?}"),
    }
}

#[test]
fn req_auto_071_a_font_is_as_bold_as_its_capital_letters_satin_columns_are_wide() {
    let satin = r#"inkstitch:satin_column="true""#;
    // `A` is a column 2 mm wide and 10 tall drawn by its rails, `B` one 3 mm wide drawn by its centre line,
    // and `C` one 2 mm wide and 8 tall. `D` is a stroke that is no column, 12 mm tall, and `a` is no capital.
    let layers = [
        glyph("A", "M 0,60 L 0,70 M 2,60 L 2,70", satin),
        glyph("B", "M 0,60 L 0,70", satin).replace("stroke-width:0.4", "stroke-width:3"),
        glyph("C", "M 0,62 L 0,70 M 2,62 L 2,70", satin),
        glyph("D", "M 0,58 L 0,70", ""),
        glyph("a", "M 0,60 L 0,70 M 9,60 L 9,70", satin),
    ];
    let measured = weight::of_font(&font_of("columns", &layers.concat()), &Budget::DEFAULT).unwrap().unwrap();
    // Of the columns' outlines, 28 + 20 + 16 mm are 2 mm from the other rail and 20 mm are 3: the median
    // is 2. The heights are 8, 10, 10 and 12 mm: the median is 10.
    assert!((measured.stroke - 2.0).abs() < 1e-9 && (measured.height - 10.0).abs() < 1e-9, "{measured:?}");
    assert!((measured.ratio() - 0.2).abs() < 1e-9);
    // A font that draws no capital letter as a satin column has no weight, and neither has one without glyphs
    // for them.
    assert_eq!(weight::of_font(&font_of("strokes", &glyph("D", "M 0,58 L 0,70", "")), &Budget::DEFAULT).unwrap(), None);
    assert_eq!(weight::of_font(&plain("small"), &Budget::DEFAULT).unwrap(), None);
    // Measuring is metered: a budget too small for it ends with SC-E0004.
    let font = font_of("columns", &layers.concat());
    for allowed in [0, 1, 3, 10, 40, 60, 80] {
        let failed = weight::of_font(&font, &Budget { max_work: allowed, ..Budget::DEFAULT }).unwrap_err();
        assert_eq!(failed.code, Code::BudgetExhausted, "{allowed}");
    }
}

#[test]
fn req_auto_071_traced_letters_are_as_bold_as_their_strokes_are_wide_in_their_height() {
    // Bars 4 mm wide and 16 tall: strokes a quarter of their height.
    let (_, said) = chosen_for(bars("stitch:text ab", 4.0, 16.0), &choices());
    assert!(said[0].message.contains("its letters are 16.0 mm tall, with strokes 0.25 of their height wide."), "{}", said[0]);
    // The block letters of the test sheets, drawn with strokes 18 % of their height wide, measure so, the
    // hole of the A and the slanted strokes of the M with them.
    let mut letters = word((10.0, 30.0), 15.0).unwrap().elements;
    letters.push(marker("stitch:text ab", 15.0));
    let (_, said) = chosen_for(Design::new(letters, DesignSettings::default()).unwrap(), &choices());
    assert!(said[0].message.contains("its letters are 15.0 mm tall, with strokes 0.18 of their height wide."), "{}", said[0]);
    // A traced picture keeps a colour's areas in one path, and the letters leave holes in the colour round
    // them. The holes are the background's, and aren't measured: a wide gap between 2 letters is no stroke.
    let ground = fill("g", vec![rect(0.0, 0.0, 100.0, 100.0), rect(10.0, 30.0, 12.4, 46.0), rect(40.0, 30.0, 42.4, 46.0)], WHITE);
    let both = fill("l", vec![rect(10.0, 30.0, 12.4, 46.0), rect(40.0, 30.0, 42.4, 46.0)], RED);
    let traced = Design::new(vec![ground, both, marker("stitch:text ab", 16.0)], DesignSettings::default()).unwrap();
    let (_, said) = chosen_for(traced, &choices());
    assert!(said[0].message.contains("with strokes 0.15 of their height wide."), "{}", said[0]);
    // Letters drawn as lines are as bold as the lines are wide.
    let drawn = [(10.0, 0.0), (40.0, 0.0)].map(|(x, _)| Subpath { start: at((x, 30.0)), segments: vec![Segment::Line(at((x, 46.0)))], closed: false });
    let lines = line("l", None, drawn.to_vec(), 3.2, RED);
    let (_, said) = chosen_for(Design::new(vec![lines, marker("stitch:text ab", 16.0)], DesignSettings::default()).unwrap(), &choices());
    assert!(said[0].message.contains("its letters are 16.0 mm tall, with strokes 0.20 of their height wide."), "{}", said[0]);
}

#[test]
fn req_auto_072_the_font_nearest_in_weight_that_sews_at_the_height_is_chosen() {
    let choices = choices();
    // At a height both sew, the nearer in weight: 0.195 lies halfway, and the earlier font takes a tie.
    assert_eq!(chosen(&choices, Some(0.19), 15.0), Some(0));
    assert_eq!(chosen(&choices, Some(0.20), 15.0), Some(1));
    assert_eq!(chosen(&[choices[0].clone(), Choice { weight: 0.24, ..choices[1].clone() }], Some(0.19), 15.0), Some(0));
    assert_eq!(chosen(&[choices[1].clone(), Choice { weight: 0.36, ..choices[0].clone() }], Some(0.305), 15.0), Some(0));
    assert_eq!(chosen(&choices, Some(0.5), 12.0), Some(1));
    assert_eq!(chosen(&choices, Some(0.5), 20.0), Some(1));
    assert_eq!(chosen(&choices, Some(0.01), 20.0), Some(0));
    // At a height one sews, that one, however bold the letters.
    assert_eq!(chosen(&choices, Some(0.5), 11.9), Some(0));
    assert_eq!(chosen(&choices, Some(0.5), 5.0), Some(0));
    assert_eq!(chosen(&choices, Some(0.01), 20.1), Some(1));
    assert_eq!(chosen(&choices, Some(0.01), 30.0), Some(1));
    // At a height none sews, the one that sews nearest to it.
    assert_eq!(chosen(&choices, Some(0.5), 4.0), Some(0));
    assert_eq!(chosen(&choices, Some(0.01), 31.0), Some(1));
    // Letters without a weight take the first font that sews at their height.
    assert_eq!(chosen(&choices, None, 15.0), Some(0));
    assert_eq!(chosen(&choices, None, 25.0), Some(1));
    assert_eq!(chosen(&[], Some(0.2), 15.0), None);
}

#[test]
fn diag_sc_i1117_names_the_font_chosen_and_says_why() {
    let choices = choices();
    // Bold letters at a height the bold font sews: it is asked for by its name.
    let (asked, said) = chosen_for(bars("stitch:text ab", 4.0, 16.0), &choices);
    assert_eq!(asked, ["bold"]);
    assert_eq!(
        said.iter().map(ToString::to_string).collect::<Vec<_>>(),
        ["info SC-I1117: The text marker `marker` names no font: its letters are 16.0 mm tall, with strokes 0.25 of their height wide. The text is \
          set in `bold`, with strokes 0.25 wide: the nearest in weight of the fonts that sew at that height."]
    );
    assert_eq!(hint(&said[0]), "Name another font with font=NAME after the quoted text.");
    // Light letters at the same height take the light font.
    let (asked, said) = chosen_for(bars("stitch:text ab", 2.0, 16.0), &choices);
    assert_eq!(asked, ["light"]);
    assert!(said[0].message.ends_with("The text is set in `light`, with strokes 0.14 wide: the nearest in weight of the fonts that sew at that height."));
    // Bold letters shorter than the bold font sews are set in the light one, and the note says from what
    // height the bold one sews and how to keep the traced letters.
    let (asked, said) = chosen_for(bars("stitch:text ab", 2.5, 10.0), &choices);
    assert_eq!(asked, ["light"]);
    assert_eq!(
        said[0].message,
        "The text marker `marker` names no font: its letters are 10.0 mm tall, with strokes 0.25 of their height wide. `bold` is the nearest in \
         weight, with strokes 0.25 wide, and sews from 12.0 to 30.0 mm. So the text is set in `light`, with strokes 0.14 wide, which sews at \
         that height, from 5.0 to 20.0 mm."
    );
    assert_eq!(
        hint(&said[0]),
        "Leave the text unmarked to sew the letters as they were traced, which sews well from about 10 mm, or make the picture larger: `bold` \
         sews from 12.0 mm."
    );
    // Letters no font sews as tall take the font that sews nearest their height.
    let (asked, said) = chosen_for(bars("stitch:text ab", 1.0, 4.0), &choices);
    assert_eq!(asked, ["light"]);
    assert!(
        said[0].message.ends_with(
            "with strokes 0.25 of their height wide. No font here sews at that height. `light` sews nearest to it, from 5.0 to 20.0 mm, with \
             strokes 0.14 wide."
        ),
        "{}",
        said[0]
    );
    assert!(hint(&said[0]).starts_with("Draw the picture so that its letters are as tall as a font sews, or name a font"));
}

#[test]
fn req_auto_072_a_named_font_wins_and_no_choice_leaves_the_default() {
    // A marker that names a font is set in it, whatever the letters weigh, and nothing is said of a choice.
    let (asked, said) = chosen_for(bars(r#"stitch:text "ab" font=light"#, 4.0, 16.0), &choices());
    assert_eq!(asked, ["light"]);
    assert!(said.is_empty(), "{said:?}");
    // A host without choices sets the text in its default font, as before, and says nothing of a choice.
    let (asked, said) = chosen_for(bars("stitch:text ab", 4.0, 16.0), &[]);
    assert_eq!(asked, [""]);
    assert!(said.is_empty(), "{said:?}");
    // `replace` is the pass without choices.
    let mut asked = Vec::new();
    let mut fonts = |name: &str, _: &Budget| {
        asked.push(name.to_string());
        Found::Font(plain("default"))
    };
    let (_, said) = pictures::replace(bars("stitch:text ab", 4.0, 16.0), &[], &mut fonts, &mut BTreeSet::new(), &Budget::DEFAULT).unwrap();
    assert!(asked == [""] && said.iter().all(|said| said.code != Code::FontChosenByWeight));
    // A marker given beside the design chooses as one in it does.
    let design = bars("stitch:text ab", 4.0, 16.0);
    let beside = Marker { label: "text-1".to_string(), shape: marker("", 16.0).shape, note: "ab".to_string() };
    let letters: Vec<Element> = design.elements().iter().filter(|element| element.name.is_none()).cloned().collect();
    let mut fonts = |name: &str, _: &Budget| Found::Font(plain(name));
    let alone = Design::new(letters, DesignSettings::default()).unwrap();
    let (after, said) = pictures::replace_among(alone, &[beside], &mut fonts, &choices(), &mut BTreeSet::new(), &Budget::DEFAULT).unwrap();
    assert!(said.iter().any(|said| said.code == Code::FontChosenByWeight && said.message.contains("set in `bold`")));
    assert!(after.elements().iter().all(|element| element.id.to_string().starts_with("text-1:")));
}

#[test]
fn req_auto_072_choosing_stops_with_the_budget() {
    // Every amount of work short of what the pass needs ends with SC-E0004, and never with a panic or a
    // design half changed.
    let mut fonts = |name: &str, _: &Budget| Found::Font(plain(name));
    let mut least = None;
    for allowed in 0..4_000 {
        let budget = Budget { max_work: allowed, ..Budget::DEFAULT };
        match pictures::replace_among(bars("stitch:text ab", 4.0, 16.0), &[], &mut fonts, &choices(), &mut BTreeSet::new(), &budget) {
            Ok(_) => {
                least = Some(allowed);
                break;
            }
            Err(failed) => assert_eq!(failed.code, Code::BudgetExhausted, "{allowed}"),
        }
    }
    assert!(least.is_some_and(|least| least > 100), "{least:?}");
}
