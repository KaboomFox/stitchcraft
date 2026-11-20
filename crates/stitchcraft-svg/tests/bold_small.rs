//! Bold letters below the height where the bold font sews (roadmap step M14.9, `REQ-FONT-024`,
//! `docs/src/design/image-to-embroidery.md` › Bold letters under the bold font's heights): lettering's
//! `widen_percent` widens every satin column of a text, and nothing else. The font is drawn in code. No font
//! file of anyone's is used. The automatic choice of a widening (`REQ-AUTO-140`, `REQ-AUTO-141`) waits for
//! the machine checkpoint, and its cases with it.

// Test code may unwrap, panic and index (clippy.toml allows it in tests, but not in their helpers).
#![allow(clippy::unwrap_used, clippy::panic, clippy::cast_possible_truncation)]

use std::rc::Rc;

use stitchcraft_core::units::MM_PER_SVG_PX;
use stitchcraft_core::{Budget, Code};
use stitchcraft_engine::design::Design;
use stitchcraft_plan::StitchKind;
use stitchcraft_svg::font::{self, Font};
use stitchcraft_svg::lettering::{self, Lettering, SETTINGS, WIDEN_PERCENT, WIDEST};
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
    // Today's width: 1 mm drawn and 0.2 mm of pull compensation on each side, which isn't scaled.
    let plain = lettering::set(&font, "a", &Lettering::default(), &Budget::DEFAULT).unwrap();
    assert!((sewn_width(&plain.design) - 1.4).abs() < 0.02, "{}", sewn_width(&plain.design));
    // widen_percent 20 adds a fifth of the column's width on each side: 1.0 + 0.4 + 0.4 mm.
    let mut how = Lettering::default();
    assert!(how.take("widen_percent", "20"), "widen_percent is one of lettering's settings");
    let wide = lettering::set(&font, "a", &how, &Budget::DEFAULT).unwrap();
    assert!((sewn_width(&wide.design) - 1.8).abs() < 0.02, "{}", sewn_width(&wide.design));
    // At twice the scale the widening is twice as wide, as the column is: 2.0 + 0.4 + 0.8 mm.
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
    // 50 % on each side: 1.0 + 0.4 + 1.0 mm.
    assert!((sewn_width(&set.design) - 2.4).abs() < 0.02, "{}", sewn_width(&set.design));
    assert!(set.warnings.iter().any(|w| w.message.contains("widen_percent") && w.message.contains("50")), "{:?}", set.warnings);
    // A negative widening is none.
    let mut how = Lettering::default();
    assert!(how.take("widen_percent", "-5"));
    let set = lettering::set(&font, "a", &how, &Budget::DEFAULT).unwrap();
    assert!((sewn_width(&set.design) - 1.4).abs() < 0.02, "{}", sewn_width(&set.design));
    assert!(!set.warnings.is_empty());
}

/// The font of [`columns`] with the `a`'s own `pull_compensation_percent` set to `own`.
fn with_own(own: &str) -> Rc<Font> {
    let font = columns("c");
    let mut font = (*font).clone();
    let variant = font.variants.first_mut().unwrap();
    let glyph = variant.glyphs.get_mut("a").unwrap();
    let elements: Vec<_> = glyph
        .design
        .elements()
        .iter()
        .cloned()
        .map(|mut element| {
            element.params.set("pull_compensation_percent", own);
            element
        })
        .collect();
    glyph.design = Design::new(elements, glyph.design.settings).unwrap();
    Rc::new(font)
}

/// The `pull_compensation_percent` each element of `a` set in `font` with `widen` gets.
fn percents(font: &Font, widen: &str) -> Vec<Option<String>> {
    let mut how = Lettering::default();
    assert!(how.take("widen_percent", widen));
    let set = lettering::set(font, "a", &how, &Budget::DEFAULT).unwrap();
    set.design.elements().iter().map(|e| e.params.get("pull_compensation_percent").map(str::to_string)).collect()
}

#[test]
fn req_font_024_widen_percent_adds_to_the_glyphs_own_pull_compensation_on_each_side() {
    // One value for both sides, 2 for each, and none, each gain the widening.
    assert_eq!(percents(&with_own("10"), "20"), [Some("30".to_string())]);
    assert_eq!(percents(&with_own("10 -5"), "20"), [Some("30 15".to_string())]);
    assert_eq!(percents(&with_own(""), "12.5"), [Some("12.5".to_string())]);
    assert_eq!(percents(&columns("c"), "7"), [Some("7".to_string())]);
    // At 0 a column keeps its own value, here none, and a running stitch never gains one.
    assert_eq!(percents(&columns("c"), "0"), [None]);
    let mut how = Lettering::default();
    how.take("widen_percent", "20");
    let set = lettering::set(&columns("c"), "r", &how, &Budget::DEFAULT).unwrap();
    assert!(set.design.elements().iter().all(|e| e.params.get("pull_compensation_percent").is_none()));
    // A value the planner reads as out of its range is widened from where the planner holds it.
    assert_eq!(percents(&with_own("150"), "20"), [Some("120".to_string())]);
    // A value that doesn't read is left for the planner to refuse with SC-E0101.
    assert_eq!(percents(&with_own("wide"), "20"), [Some("wide".to_string())]);
    let mut how = Lettering::default();
    how.take("widen_percent", "20");
    let set = lettering::set(&with_own("wide"), "a", &how, &Budget::DEFAULT).unwrap();
    let planned = in_place(&set.design);
    assert!(planned.diagnostics.iter().any(|d| d.code == Code::ParamInvalid), "{:?}", planned.diagnostics);
}

#[test]
fn req_font_024_widen_percent_0_sets_the_text_as_before_and_the_same_every_time() {
    let font = columns("c");
    let text = "ab ra\nbar";
    let plain = lettering::set(&font, text, &Lettering::default(), &Budget::DEFAULT).unwrap();
    let mut zero = Lettering::default();
    assert!(zero.take("widen_percent", "0"));
    assert_eq!(zero, Lettering::default());
    let same = lettering::set(&font, text, &zero, &Budget::DEFAULT).unwrap();
    assert_eq!(same, plain);
    assert!(plain.warnings.iter().all(|w| w.code != Code::ParamClamped), "{:?}", plain.warnings);
    // What it sews is the same, and a widened text is the same each time it is set.
    let sewn = |set: &lettering::Set| in_place(&set.design).plan.unwrap();
    assert_eq!(sewn(&same), sewn(&plain));
    let mut wide = Lettering::default();
    wide.take("widen_percent", "20");
    let once = lettering::set(&font, text, &wide, &Budget::DEFAULT).unwrap();
    let twice = lettering::set(&font, text, &wide, &Budget::DEFAULT).unwrap();
    assert_eq!(once, twice);
    assert_eq!(sewn(&once), sewn(&twice));
    assert_ne!(sewn(&once), sewn(&plain));
}

#[test]
fn req_font_024_widen_percent_at_its_ends_and_past_them() {
    let font = columns("c");
    let warned = |value: f64| {
        let how = Lettering { widen_percent: value, ..Lettering::default() };
        let set = lettering::set(&font, "a", &how, &Budget::DEFAULT).unwrap();
        let clamped: Vec<String> = set.warnings.iter().filter(|w| w.code == Code::ParamClamped).map(|w| w.message.clone()).collect();
        (sewn_width(&set.design), clamped)
    };
    // Each end is taken as it is, without a warning.
    let (width, said) = warned(WIDEST);
    assert!((width - 2.4).abs() < 0.02 && said.is_empty(), "{width} {said:?}");
    let (width, said) = warned(0.0);
    assert!((width - 1.4).abs() < 0.02 && said.is_empty(), "{width} {said:?}");
    // Past them it is held at the nearest, and the warning names the setting, the value and the range.
    assert_eq!(warned(50.5).1, ["`widen_percent` is 50.5, outside 0 to 50 %; 50 % is used."]);
    assert_eq!(warned(-0.5).1, ["`widen_percent` is -0.5, outside 0 to 50 %; 0 % is used."]);
    let (width, said) = warned(f64::NAN);
    assert!((width - 1.4).abs() < 0.02 && said.len() == 1, "{width} {said:?}");
    let (width, said) = warned(f64::INFINITY);
    assert!((width - 2.4).abs() < 0.02 && said.len() == 1, "{width} {said:?}");
    // A value that is no number isn't taken.
    let mut how = Lettering::default();
    assert!(!how.take("widen_percent", "wide") && !how.take("widen_percent", "inf"));
    assert_eq!(how, Lettering::default());
    // The setting is lettering's, listed last, with its help.
    assert_eq!(SETTINGS.last(), Some(&WIDEN_PERCENT));
    assert_eq!((WIDEN_PERCENT.name, WIDEN_PERCENT.takes, WIDEN_PERCENT.default), ("widen_percent", "PERCENT", "0"));
}
