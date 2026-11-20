//! Bold lettering below the height where the bold font sews (roadmap step M14.9, `REQ-FONT-024`,
//! `REQ-AUTO-140` and `REQ-AUTO-141`, `docs/src/design/image-to-embroidery.md` › Bold letters under the bold
//! font's heights): lettering's `widen_percent` widens every satin column of a text, and bold letters or bold
//! text that the bold font can't sew at their height are set in the lighter font, widened. The fonts are
//! drawn in code. No font file of anyone's is used.

// Test code may unwrap, panic and index (clippy.toml allows it in tests, but not in their helpers).
#![allow(clippy::unwrap_used, clippy::panic)]

use std::collections::BTreeSet;
use std::rc::Rc;

use stitchcraft_core::units::{MM_PER_SVG_PX, Mm, Point};
use stitchcraft_core::{Budget, Code, Diagnostic, ElementId};
use stitchcraft_engine::design::{Cap, Design, DesignSettings, Element, FillRule, Hints, Join, Path, Segment, Shape, Subpath};
use stitchcraft_params::ParamSet;
use stitchcraft_plan::{Rgb, StitchKind, Thread};
use stitchcraft_svg::font::{self, Font};
use stitchcraft_svg::lettering::objects::{self, Anchor, Line, TextObject};
use stitchcraft_svg::lettering::pictures::{self, Choice, Found};
use stitchcraft_svg::lettering::{self, Lettering};
use stitchcraft_testkit::designs::in_place;

/// A glyph file's root: 100 mm square, a user unit a millimetre.
const ROOT: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" xmlns:inkscape="http://www.inkscape.org/namespaces/inkscape" xmlns:sodipodi="http://sodipodi.sourceforge.net/DTD/sodipodi-0.dtd" xmlns:inkstitch="http://inkstitch.org/namespace" width="100mm" height="100mm" viewBox="0 0 100 100">"#;

/// A font named `folder`, 10 mm tall and sewn from 5 to 20 mm, every glyph advancing 5 mm. Its `a` and `b`
/// are each an upright satin column 1 mm wide and 10 mm tall on the baseline, with 0.2 mm of pull
/// compensation and no underlay, and its `r` is a running stitch beside it.
fn columns(folder: &str) -> Rc<Font> {
    let satin = r#"inkstitch:satin_column="True" inkstitch:pull_compensation_mm="0.2" inkstitch:contour_underlay="False" inkstitch:zigzag_underlay="False" inkstitch:center_walk_underlay="False""#;
    let glyph = |label: &str, d: &str, more: &str| {
        format!(
            r#"<g inkscape:groupmode="layer" inkscape:label="GlyphLayer-{label}" style="display:none"><path id="p{label}" d="{d}" style="fill:none;stroke:#000000;stroke-width:0.4" {more}/></g>"#
        )
    };
    let layers =
        [glyph("a", "M 1,60 L 1,70 M 2,60 L 2,70", satin), glyph("b", "M 1,60 L 1,70 M 2,60 L 2,70", satin), glyph("r", "M 1,60 L 1,70", "")];
    let json = format!(
        r#"{{"name": "Test", "size": 10, "min_scale": 0.5, "max_scale": 2, "auto_satin": false, "horiz_adv_x_default": {}}}"#,
        5.0 / MM_PER_SVG_PX
    );
    let file = format!(
        r#"{ROOT}<sodipodi:namedview id="base"><sodipodi:guide position="0,30" orientation="0,1" inkscape:label="baseline"/></sodipodi:namedview>{}</svg>"#,
        layers.concat()
    );
    Rc::new(font::read(folder, &[("font.json", json.as_bytes()), ("ltr.svg", file.as_bytes())], &Budget::DEFAULT).unwrap())
}

fn at((x, y): (f64, f64)) -> Point {
    Point::new(x, y).unwrap()
}

/// How wide the satin column of a design of one letter `a` sews: the needle points' extent across it.
fn sewn_width(design: &Design) -> f64 {
    let plan = in_place(design).plan.unwrap();
    let xs: Vec<f64> = plan.blocks.iter().flat_map(|b| &b.stitches).filter(|s| s.kind == StitchKind::Normal).map(|s| s.at.x()).collect();
    xs.iter().copied().fold(f64::NEG_INFINITY, f64::max) - xs.iter().copied().fold(f64::INFINITY, f64::min)
}

/// The needle points of the running stitch `r` set with `how`.
fn running(how: &Lettering) -> Vec<(i64, i64)> {
    let set = lettering::set(&columns("c"), "r", how, &Budget::DEFAULT).unwrap();
    let plan = in_place(&set.design).plan.unwrap();
    plan.stitches()
        .filter(|s| s.kind == StitchKind::Normal)
        .map(|s| ((s.at.x() * 1000.0).round() as i64, (s.at.y() * 1000.0).round() as i64))
        .collect()
}

#[test]
fn req_font_024_widen_percent_widens_each_satin_column_on_both_sides() {
    let font = columns("c");
    // Today's width: 1 mm drawn and 0.2 mm of pull compensation on each side.
    let plain = lettering::set(&font, "a", &Lettering::default(), &Budget::DEFAULT).unwrap();
    assert!((sewn_width(&plain.design) - 1.4).abs() < 0.02, "{}", sewn_width(&plain.design));
    // widen_percent 20 adds a fifth of the column's width on each side: 1.0 × 1.4 + 0.4 mm.
    let mut how = Lettering::default();
    assert!(how.take("widen_percent", "20"), "widen_percent is one of lettering's settings");
    let wide = lettering::set(&font, "a", &how, &Budget::DEFAULT).unwrap();
    assert!((sewn_width(&wide.design) - 1.8).abs() < 0.02, "{}", sewn_width(&wide.design));
    // At twice the scale the widening is twice as wide, as the column is: 2.0 × 1.4 + 0.4 mm.
    how.scale = 200.0;
    let tall = lettering::set(&font, "a", &how, &Budget::DEFAULT).unwrap();
    assert!((sewn_width(&tall.design) - 3.2).abs() < 0.02, "{}", sewn_width(&tall.design));
    // A running stitch is not widened.
    let mut widened = Lettering::default();
    widened.take("widen_percent", "20");
    assert_eq!(running(&widened), running(&Lettering::default()));
}

#[test]
fn req_font_024_widen_percent_outside_0_to_50_is_set_at_the_nearest_with_a_warning() {
    let font = columns("c");
    let mut how = Lettering::default();
    assert!(how.take("widen_percent", "80"));
    let set = lettering::set(&font, "a", &how, &Budget::DEFAULT).unwrap();
    // 50 % on each side: 1.0 × 2.0 + 0.4 mm.
    assert!((sewn_width(&set.design) - 2.4).abs() < 0.02, "{}", sewn_width(&set.design));
    assert!(set.warnings.iter().any(|w| w.message.contains("widen_percent") && w.message.contains("50")), "{:?}", set.warnings);
    // A negative widening is none.
    let mut how = Lettering::default();
    assert!(how.take("widen_percent", "-5"));
    let set = lettering::set(&font, "a", &how, &Budget::DEFAULT).unwrap();
    assert!((sewn_width(&set.design) - 1.4).abs() < 0.02, "{}", sewn_width(&set.design));
    assert!(!set.warnings.is_empty());
}

/// The closed subpath round the box from (`x0`, `y0`) to (`x1`, `y1`).
fn rect(x0: f64, y0: f64, x1: f64, y1: f64) -> Subpath {
    let corners = [(x1, y0), (x1, y1), (x0, y1)];
    Subpath { start: at((x0, y0)), segments: corners.iter().map(|corner| Segment::Line(at(*corner))).collect(), closed: true }
}

fn element(id: &str, name: Option<&str>, shape: Shape, colour: Rgb) -> Element {
    Element {
        id: ElementId::new(id).unwrap(),
        name: name.map(str::to_string),
        shape,
        thread: Thread::new(colour),
        params: ParamSet::new(),
        automatic: true,
        lettered: None,
        hints: Hints::default(),
        clips: Vec::new(),
    }
}

/// 2 traced letters, bars `tall` mm tall whose strokes are `wide` mm wide, with a marker `stitch:text ab` round
/// them.
fn bars(wide: f64, tall: f64) -> Design {
    let red = Rgb::new(200, 0, 0);
    let fill = |id: &str, x: f64| {
        element(
            id,
            None,
            Shape::Fill { path: Path { subpaths: vec![rect(x, 30.0, x + wide, 30.0 + tall)] }, rule: FillRule::EvenOdd, gradient: None },
            red,
        )
    };
    let shape = Shape::Stroke {
        path: Path { subpaths: vec![rect(5.0, 25.0, 95.0, 35.0 + tall)] },
        width: Mm::new(0.3).unwrap(),
        join: Join::Round,
        cap: Cap::Butt,
    };
    let marker = element("marker", Some("stitch:text ab"), shape, Rgb::new(0, 0, 200));
    Design::new(vec![fill("l1", 10.0), fill("l2", 40.0), marker], DesignSettings::default()).unwrap()
}

/// A light font, as heavy as Small Font and sewn from 3.8 to 16.2 mm, and a bold one that sews from 12.5 mm.
fn choices() -> Vec<Choice> {
    vec![
        Choice { font: "light".to_string(), weight: 0.13, heights: (3.8, 16.2) },
        Choice { font: "bold".to_string(), weight: 0.19, heights: (12.5, 32.5) },
    ]
}

/// The marked letters set among `choices()`: the widening each lettered satin column carries, and the notes
/// on the choice.
fn widened(design: Design) -> (Vec<Option<String>>, Vec<Diagnostic>) {
    let mut fonts = |name: &str, _: &str, _: &Budget| Found::Font(columns(if name.is_empty() { "default" } else { name }));
    let (design, said) = pictures::replace_among(design, &[], &mut fonts, &choices(), &mut BTreeSet::new(), &Budget::DEFAULT).unwrap();
    let percent = design
        .elements()
        .iter()
        .filter(|e| e.params.get("satin_column").is_some_and(|v| v.eq_ignore_ascii_case("true")))
        .map(|e| e.params.get("pull_compensation_percent").map(str::to_string))
        .collect();
    (percent, said.into_iter().filter(|said| said.code == Code::FontChosenByWeight).collect())
}

#[test]
fn req_auto_140_bold_letters_under_the_bold_fonts_heights_are_set_in_the_light_font_widened() {
    // Letters 10 mm tall with strokes 0.18 of their height: bolder than the light font, and under the bold
    // font's 12.5 mm. 50 × (0.18 / 0.13 − 1) is 19.2: the columns are widened by 19 % on each side.
    let (percent, said) = widened(bars(1.8, 10.0));
    assert_eq!(percent, [Some("19".to_string()), Some("19".to_string())]);
    assert!(said[0].message.contains("`light`") && said[0].message.contains("widened by 19 %"), "{}", said[0].message);
    // Letters bolder still are widened by 20 % at most.
    let (percent, said) = widened(bars(3.0, 10.0));
    assert_eq!(percent, [Some("20".to_string()), Some("20".to_string())]);
    assert!(said[0].message.contains("widened by 20 %"), "{}", said[0].message);
    // From 8 mm: at 8 mm they are widened.
    let (percent, _) = widened(bars(1.44, 8.0));
    assert_eq!(percent, [Some("19".to_string()), Some("19".to_string())]);
}

#[test]
fn req_auto_140_letters_under_8_mm_or_near_the_fonts_weight_are_not_widened() {
    // Under 8 mm a widened light font closes its counters: the letters are set as they are today, and the
    // note says why.
    let (percent, said) = widened(bars(1.4, 7.9));
    assert_eq!(percent, [None, None]);
    assert!(said[0].message.contains("8.0 mm"), "{}", said[0].message);
    // Letters within 4 % of a widening of the font's weight are set in it unwidened: 50 × (0.14 / 0.13 − 1)
    // is 3.8.
    let (percent, said) = widened(bars(1.4, 10.0));
    assert_eq!(percent, [None, None]);
    assert!(!said[0].message.contains("widened"), "{}", said[0].message);
    // Letters the bold font sews are set in it, unwidened, as before.
    let (percent, said) = widened(bars(2.6, 14.0));
    assert_eq!(percent, [None, None]);
    assert!(said[0].message.contains("`bold`"), "{}", said[0].message);
}

/// A bold text object of one line `ab`, `size` mm of font size.
fn bold_text(size: f64) -> TextObject {
    TextObject {
        label: "svg:t".to_string(),
        name: None,
        typeface: Some("Lakeside Sans".to_string()),
        bold: true,
        lines: vec![Line {
            text: "ab".to_string(),
            at: at((10.0, 20.0)),
            anchor: Anchor::Start,
            size,
            thread: Some(Thread::new(Rgb::new(200, 0, 0))),
            length: None,
        }],
        turn: (1.0, 0.0),
        clips: Vec::new(),
    }
}

/// What `text` sews among `choices()`: the widening each satin column carries, and the notes on the choice.
fn text_widened(text: &TextObject) -> (Vec<Option<String>>, Vec<Diagnostic>) {
    let mut fonts = |name: &str, _: &str, _: &Budget| Found::Font(columns(if name.is_empty() { "default" } else { name }));
    let (elements, said) = objects::letter_among(text, &mut fonts, &choices(), &mut BTreeSet::new(), &Budget::DEFAULT).unwrap();
    let percent = elements
        .iter()
        .filter(|e| e.params.get("satin_column").is_some_and(|v| v.eq_ignore_ascii_case("true")))
        .map(|e| e.params.get("pull_compensation_percent").map(str::to_string))
        .collect();
    (percent, said.into_iter().filter(|said| said.code == Code::FontChosenByWeight).collect())
}

#[test]
fn req_auto_141_a_bold_text_under_the_bold_fonts_heights_sews_in_the_default_font_widened() {
    // 15 mm of font size sew 10.5 mm tall, under the bold font's 12.5 mm and from 8 mm: the default font,
    // widened toward the bold font's weight, 50 × (0.19 / 0.13 − 1) = 23, by 20 % at most.
    let (percent, said) = text_widened(&bold_text(15.0));
    assert_eq!(percent, [Some("20".to_string()), Some("20".to_string())]);
    assert!(said[0].message.contains("widened by 20 %"), "{}", said[0].message);
    // 10 mm of font size sew 7 mm tall: under 8 mm, not widened, and the note says so.
    let (percent, said) = text_widened(&bold_text(10.0));
    assert_eq!(percent, [None, None]);
    assert!(said[0].message.contains("8.0 mm"), "{}", said[0].message);
    // Where the bold font sews, it is used, unwidened.
    let (percent, _) = text_widened(&bold_text(20.0));
    assert_eq!(percent, [None, None]);
}
