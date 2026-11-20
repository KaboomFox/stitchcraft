//! The thickness cap and lettering (roadmap step M13.50, `REQ-AUTO-119` to `REQ-AUTO-124`,
//! `docs/src/design/image-to-embroidery.md` › The cap and lettering): lettering that automatic digitizing
//! places is in the reach of the last pass and the cap, its columns stay whole, and a person's own lettering
//! is measured and warned of, never changed. The letters are drawn in code as a hand-digitized font draws
//! them, satin columns that overlap where they join, and no font file of anyone's is used.

// Test code may unwrap, panic and index (clippy.toml allows it in tests, but not in their helpers).
#![allow(clippy::unwrap_used, clippy::panic, clippy::cast_possible_truncation, clippy::cast_precision_loss)]

use std::collections::BTreeSet;
use std::rc::Rc;

use stitchcraft_core::units::{MM_PER_SVG_PX, Mm, Point};
use stitchcraft_core::{Budget, Code, ElementId, math};
use stitchcraft_engine::design::{Cap, Design, DesignSettings, Element, Hints, Join, Path, Segment, Shape, Subpath};
use stitchcraft_params::ParamSet;
use stitchcraft_plan::thickness::{Measure, Thickness};
use stitchcraft_plan::{Rgb, Role, StitchKind, StitchPlan, Thread};
use stitchcraft_svg::font::{self, Font};
use stitchcraft_svg::lettering::pictures::{self, Found};
use stitchcraft_svg::lettering::{self, Lettering};
use stitchcraft_testkit::designs::{automatic, fill, in_place};

/// A glyph file's root: 100 mm square, a user unit a millimetre.
const ROOT: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" xmlns:inkscape="http://www.inkscape.org/namespaces/inkscape" xmlns:sodipodi="http://sodipodi.sourceforge.net/DTD/sodipodi-0.dtd" xmlns:inkstitch="http://inkstitch.org/namespace" width="100mm" height="100mm" viewBox="0 0 100 100">"#;

/// How a column of the test font is set: a satin column with a contour underlay and 0.2 mm of pull
/// compensation, as hand-digitized fonts set theirs.
const COLUMN: &str = r#"inkstitch:satin_column="True" inkstitch:contour_underlay="True" inkstitch:pull_compensation_mm="0.2" style="fill:none;stroke:#000000;stroke-width:0.1""#;

/// A satin column of the test font from `a` to `b`, `width` mm wide: its 2 rails.
fn column(id: &str, (ax, ay): (f64, f64), (bx, by): (f64, f64), width: f64) -> String {
    let length = math::hypot(bx - ax, by - ay);
    let (nx, ny) = (-(by - ay) / length * width / 2.0, (bx - ax) / length * width / 2.0);
    format!(
        r#"<path id="{id}" d="M {},{} L {},{} M {},{} L {},{}" {COLUMN}/>"#,
        ax + nx,
        ay + ny,
        bx + nx,
        by + ny,
        ax - nx,
        ay - ny,
        bx - nx,
        by - ny
    )
}

/// The test font, 10 mm tall, its baseline 70 mm down, every glyph 9 mm wide:
/// - `t`: a running stitch under its bar, straight for 2.2 mm and then zigzagging, the bar, a column 1.6 mm wide across the top, and the
///   stem, as wide, whose end reaches 1.2 mm under the bar, sewn in that order. The zigzag steps 0.5 mm along
///   the bar, so that 2 of its stitches joined make 1 mm, shorter than the running stitch's longest: the cap
///   joins no 2 stitches into one longer than that (`REQ-AUTO-100`), and the lock that ties the running
///   stitch in cuts its straight part at 1.6 mm;
/// - `x`: 3 columns 1.6 mm wide and 10 long that cross at one point, 60° apart.
fn test_font() -> Rc<Font> {
    let zigzag = r#"<path id="run" d="M 0.4,60.8 L 2.6,60.8 L 3.1,61.1 L 3.6,60.5 L 4.1,61.1 L 4.6,60.5 L 5.1,61.1 L 5.6,60.5 L 6.1,61.1 L 6.6,60.5 L 7.1,61.1" style="fill:none;stroke:#000000;stroke-width:0.4"/>"#;
    let t = format!(
        r#"<g inkscape:groupmode="layer" inkscape:label="GlyphLayer-t" style="display:none">{zigzag}{}{}</g>"#,
        column("bar", (0.0, 60.8), (8.0, 60.8), 1.6),
        column("stem", (4.0, 60.4), (4.0, 70.0), 1.6)
    );
    let ray = |degrees: f64| {
        let (s, c) = math::sin_cos(degrees.to_radians());
        ((4.0 - 5.0 * c, 65.0 - 5.0 * s), (4.0 + 5.0 * c, 65.0 + 5.0 * s))
    };
    let x: String = [0.0, 60.0, 120.0]
        .iter()
        .enumerate()
        .map(|(k, degrees)| {
            let (a, b) = ray(*degrees);
            column(&format!("x{k}"), a, b, 1.6)
        })
        .collect();
    let x = format!(r#"<g inkscape:groupmode="layer" inkscape:label="GlyphLayer-x" style="display:none">{x}</g>"#);
    let px = |mm: f64| mm / MM_PER_SVG_PX;
    let json = format!(
        r#"{{"name": "Test", "size": 10, "min_scale": 0.5, "max_scale": 2.0, "auto_satin": false, "horiz_adv_x_default": {}, "horiz_adv_x_space": {}}}"#,
        px(9.0),
        px(3.0)
    );
    let file = format!(
        r#"{ROOT}<sodipodi:namedview id="base"><sodipodi:guide position="0,30" orientation="0,1" inkscape:label="baseline"/></sodipodi:namedview>{t}{x}</svg>"#
    );
    Rc::new(font::read("test", &[("font.json", json.as_bytes()), ("ltr.svg", file.as_bytes())], &Budget::DEFAULT).unwrap())
}

fn at(x: f64, y: f64) -> Point {
    Point::new(x, y).unwrap()
}

/// A patch traced from a picture with the letter `text` marked: a cream field 20 mm square, automatic, with
/// `field` among its settings, a traced letter 10 mm tall on it, and the marker round the letter. The field
/// isn't the picture's background: a patch's border lies under it in a real picture.
fn marked_patch(text: &str, field: &[(&str, &str)]) -> Design {
    let mut settings = vec![("auto_ignore_background", "false")];
    settings.extend_from_slice(field);
    let ground = automatic(fill("field", &[&[(0.0, 0.0), (20.0, 0.0), (20.0, 20.0), (0.0, 20.0)]], &Thread::new(Rgb::new(236, 226, 198)), &settings));
    let letter = automatic(fill("letter", &[&[(6.0, 5.0), (14.0, 5.0), (14.0, 15.0), (6.0, 15.0)]], &Thread::new(Rgb::new(20, 20, 20)), &[]));
    let ring = Subpath {
        start: at(5.0, 4.0),
        segments: vec![Segment::Line(at(15.0, 4.0)), Segment::Line(at(15.0, 16.0)), Segment::Line(at(5.0, 16.0))],
        closed: true,
    };
    let marker = Element {
        id: ElementId::new("m").unwrap(),
        name: Some(format!("stitch:text {text}")),
        shape: Shape::Stroke { path: Path { subpaths: vec![ring] }, width: Mm::new(0.3).unwrap(), join: Join::Round, cap: Cap::Butt },
        thread: Thread::new(Rgb::new(0, 0, 200)),
        params: ParamSet::new(),
        automatic: true,
        lettered: None,
        hints: Hints::default(),
        clips: Vec::new(),
    };
    let design = Design::new(vec![ground, letter, marker], DesignSettings::default()).unwrap();
    let font = test_font();
    let (design, _) = pictures::replace(design, &[], &mut |_, _| Found::Font(Rc::clone(&font)), &mut BTreeSet::new(), &Budget::DEFAULT).unwrap();
    design
}

/// The plan of `design`, with its stitches where the design draws them.
fn planned(design: &Design) -> StitchPlan {
    in_place(design).plan.unwrap()
}

/// The needle points of `plan`'s elements whose ids `of` picks, with their roles, by element.
fn points(plan: &StitchPlan, of: impl Fn(&str) -> bool) -> Vec<(String, Role, Point)> {
    plan.blocks
        .iter()
        .flat_map(|block| &block.stitches)
        .filter(|stitch| stitch.kind == StitchKind::Normal)
        .filter_map(|stitch| {
            let id = stitch.origin.element.and_then(|element| plan.element(element))?.to_string();
            of(&id).then_some((id, stitch.origin.role, stitch.at))
        })
        .collect()
}

/// Whether `id` names one of the lettering's elements: the marker's are named after it.
fn lettered(id: &str) -> bool {
    id.starts_with("m:")
}

/// The thickest spot of `plan`, in fills.
fn thickest(plan: &StitchPlan) -> f64 {
    Thickness::of(plan, &mut Budget::DEFAULT.meter()).unwrap().unwrap().most(Measure::Fills).value
}

/// The plan of the patch with the letter `text` marked, and its plan with the cap and the last pass off.
fn capped_and_not(text: &str) -> (StitchPlan, StitchPlan) {
    (planned(&marked_patch(text, &[])), planned(&marked_patch(text, &[("auto_thick_cap_fills", "50")])))
}

/// The top needle points of the lettering's satin columns, which every case here leaves as digitizing laid them.
fn column_tops(plan: &StitchPlan) -> BTreeSet<(String, i64, i64)> {
    points(plan, |id| lettered(id) && !id.contains("run"))
        .into_iter()
        .filter(|(_, role, _)| *role == Role::Top)
        .map(|(id, _, p)| (id, (p.x() * 1000.0).round() as i64, (p.y() * 1000.0).round() as i64))
        .collect()
}

#[test]
fn req_auto_119_lettering_that_digitizing_places_is_in_the_caps_reach() {
    let (capped, free) = capped_and_not("t");
    let (with, without) = (points(&capped, lettered).len(), points(&free, lettered).len());
    assert!(thickest(&free) > 3.25, "the letter's join sews above the cap: {}", thickest(&free));
    assert!(with < without, "the cap takes needle points of the lettering: {with} of {without} left");
    assert!(thickest(&capped) < thickest(&free), "{} against {}", thickest(&capped), thickest(&free));
}

#[test]
fn req_auto_120_a_satin_column_sewn_whole_tags_its_underlay() {
    let set = lettering::set(&test_font(), "t", &Lettering::default(), &Budget::DEFAULT).unwrap();
    let plan = planned(&set.design);
    let bar: Vec<Role> = points(&plan, |id| id.contains("bar")).into_iter().map(|(_, role, _)| role).collect();
    assert!(bar.contains(&Role::Top));
    assert!(bar.contains(&Role::Underlay), "the bar's contour underlay is tagged underlay: {bar:?}");
}

#[test]
fn req_auto_121_a_lettering_column_covers_as_its_own_pull_compensation_pulls_it() {
    // The running stitch under the bar turns at each needle point, and the bar, 1.6 mm wide, covers it once
    // pulled in by 0.2 mm on each side. Pulled in by the overlap of areas, 1 mm, it would cover nothing.
    // Beyond the stem, from 6 mm across the letter on, only the bar covers the running stitch.
    let (capped, free) = capped_and_not("t");
    let run = |plan: &StitchPlan| {
        points(plan, |id| lettered(id) && id.contains("run")).into_iter().filter(|(_, role, p)| *role == Role::Top && p.x() - 6.0 > 6.0).count()
    };
    assert!(run(&capped) < run(&free), "the bar hides its running stitch's turns: {} of {}", run(&capped), run(&free));
}

#[test]
fn req_auto_122_a_lettering_columns_underlay_goes_only_where_another_column_covers_it() {
    // The underlay that lies under no other column holds its column's edges up and stays: here, the stem's
    // below the bar, 1.2 mm under its top, and the bar's beyond the stem.
    let (capped, free) = capped_and_not("t");
    let underlay = |plan: &StitchPlan| -> BTreeSet<(String, i64, i64)> {
        points(plan, lettered)
            .into_iter()
            .filter(|(_, role, _)| *role == Role::Underlay)
            .map(|(id, _, p)| (id, (p.x() * 1000.0).round() as i64, (p.y() * 1000.0).round() as i64))
            .collect()
    };
    let (kept, all) = (underlay(&capped), underlay(&free));
    assert!(!all.is_empty(), "the columns' underlay is tagged (REQ-AUTO-120)");
    for (id, x, y) in all.difference(&kept) {
        let (x, y) = (*x as f64 / 1000.0 - 6.0, *y as f64 / 1000.0 - 5.0);
        // In the letter's own millimetres: the bar lies from y 0 to 1.6 and the stem from x 3.2 to 4.8.
        let under_the_other = if id.contains("bar") { (3.2..=4.8).contains(&x) } else { y <= 1.6 };
        assert!(under_the_other, "{id}'s underlay point at ({x:.2}, {y:.2}) lies under no other column");
    }
}

#[test]
fn req_auto_123_the_last_resort_leaves_lettering_columns_whole() {
    // 3 columns cross at one point, over the field: above the warning's limit, which the cap may meet on a
    // digitized shape by taking a column's stitch. A letter's columns stay as the font draws them.
    let (capped, free) = capped_and_not("x");
    assert!(thickest(&free) > 4.0, "the crossing sews above the warning's limit: {}", thickest(&free));
    let (kept, all) = (column_tops(&capped), column_tops(&free));
    assert_eq!(all.difference(&kept).count(), 0, "column points taken: {:?}", all.difference(&kept).collect::<Vec<_>>());
}

#[test]
fn req_auto_124_a_persons_own_lettering_is_warned_of_and_never_changed() {
    // Lettering set by a person, with no automatic shape in the design: its crossing sews above the warning's
    // limit, and it is told so, with nothing taken.
    let set = lettering::set(&test_font(), "x", &Lettering::default(), &Budget::DEFAULT).unwrap();
    let outcome = in_place(&set.design);
    let plan = outcome.plan.unwrap();
    assert!(thickest(&plan) > 4.0, "{}", thickest(&plan));
    assert!(outcome.diagnostics.iter().any(|said| said.code == Code::AutoThick), "{:?}", outcome.diagnostics);
    assert!(!outcome.diagnostics.iter().any(|said| said.code == Code::AutoThinned), "nothing is taken: {:?}", outcome.diagnostics);
}
