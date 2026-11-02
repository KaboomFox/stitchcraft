//! The settings of automatic digitizing's rules (`REQ-AUTO-020`): each rule goes by the settings of the
//! shape it digitizes, StitchCraft's own parameters, which no stitch type reads. A shape that sets none takes
//! the defaults, the numbers the other requirements give.

// Test code may unwrap and index (clippy.toml allows it in tests).
#![allow(clippy::unwrap_used, clippy::panic, clippy::indexing_slicing)]

use stitchcraft_core::{Budget, Code, Diagnostic, Mm, Severity};
use stitchcraft_engine::auto::{
    AutoParams, Digitized, IGNORE_BACKGROUND, MAX_SATIN_WIDTH, MIN_AREA, MIN_TRIPLE_RUN_WIDTH, OVERLAP, Stitches, digitize, digitized,
};
use stitchcraft_engine::design::{Design, DesignSettings, Element, Shape};
use stitchcraft_engine::generators::passes::BEAN_STITCH_REPEATS;
use stitchcraft_engine::generators::pull::PULL_COMPENSATION;
use stitchcraft_engine::normalize::region;
use stitchcraft_engine::registry::PARAMETERS;
use stitchcraft_params::{Family, Origin, ParamSet, Stability, find, find_for};
use stitchcraft_testkit::designs::{BLUE, RED, along, automatic, fill, polylines, rules, widened};

/// `element` with the settings `params` added.
fn with(element: Element, params: &[(&str, &str)]) -> Element {
    let mut set: ParamSet = element.params.clone();
    for (key, value) in params {
        set.set(*key, *value);
    }
    Element { params: set, ..element }
}

/// What the automatic `element` sews as, in a design with the default settings.
fn made(element: &Element) -> Digitized {
    digitize(&automatic(element.clone()), &DesignSettings::default(), &mut Budget::DEFAULT.meter()).unwrap()
}

/// A red line `id` along `points`, `width` wide.
fn drawn(id: &str, points: &[(f64, f64)], width: f64) -> Element {
    widened(along(id, polylines(&[points]), &RED, &[]), width)
}

/// A fill `id` of the rectangle from `(x0, y0)` to `(x1, y1)`, sewn with `red` or else blue.
fn rectangle(id: &str, (x0, y0): (f64, f64), (x1, y1): (f64, f64), red: bool) -> Element {
    let thread = if red { RED } else { BLUE };
    fill(id, &[&[(x0, y0), (x1, y0), (x1, y1), (x0, y1)]], &thread, &[])
}

/// The setting `key` of `element`.
fn setting<'e>(element: &'e Element, key: &str) -> Option<&'e str> {
    element.params.iter().find(|(k, _)| *k == key).map(|(_, value)| value)
}

/// The least and greatest x the fill `element` sews over.
fn across(element: &Element) -> (f64, f64) {
    let Shape::Fill { path, rule } = &element.shape else { panic!("`{}` is not a fill: {:?}", element.id, element.shape) };
    let region = region::build(path, *rule, &mut Budget::DEFAULT.meter()).unwrap().region;
    let xs = region.parts.iter().flat_map(|part| part.outline.iter().map(|q| q.x()));
    xs.fold((f64::INFINITY, f64::NEG_INFINITY), |(low, high), x| (low.min(x), high.max(x)))
}

/// The elements `elements`, automatic, sew as, and what is said of them.
fn sewn(elements: Vec<Element>) -> (Vec<Element>, Vec<Diagnostic>) {
    let design = Design::new(elements.into_iter().map(automatic).collect(), DesignSettings::default()).unwrap();
    let (out, said) = digitized(&design, &Budget::DEFAULT);
    (out.into_owned(), said)
}

/// The messages of `said` under `code`.
fn messages(said: &[Diagnostic], code: Code) -> Vec<String> {
    said.iter().filter(|d| d.code == code).map(|d| d.message.clone()).collect()
}

#[test]
fn req_auto_020_the_rules_settings_are_stitchcraft_s_own_and_no_stitch_type_reads_them() {
    assert!(PARAMETERS.contains(&&AutoParams::GROUP) && AutoParams::GROUP.automatic());
    let defaults: Vec<(&str, &str)> = AutoParams::SPECS.iter().map(|spec| (spec.key, spec.default)).collect();
    assert_eq!(defaults, [(MAX_SATIN_WIDTH, "7"), (MIN_TRIPLE_RUN_WIDTH, "0.5"), (OVERLAP, "0.4"), (MIN_AREA, "1"), (IGNORE_BACKGROUND, "true")]);
    for spec in AutoParams::SPECS {
        assert_eq!((spec.origin, spec.stability), (Origin::StitchCraft, Stability::Experimental), "{}", spec.key);
        // Registered, and so known to every host, but no family's settings show it.
        assert!(find(PARAMETERS, spec.key).is_some(), "{}", spec.key);
        assert!(Family::ALL.iter().all(|family| find_for(PARAMETERS, spec.key, *family).is_none()), "{}", spec.key);
    }
    let read = rules();
    assert_eq!((read.auto_max_satin_width_mm, read.auto_min_triple_run_width_mm), (Mm::new(7.0).unwrap(), Mm::new(0.5).unwrap()));
    assert_eq!((read.auto_overlap_mm.get(), read.auto_min_area_mm2, read.auto_ignore_background), (0.4, 1.0, true));
}

#[test]
fn req_auto_020_a_line_is_a_satin_column_up_to_its_own_widest_satin_column() {
    let line = drawn("line", &[(0.0, 0.0), (30.0, 0.0)], 5.0);
    let settings = DesignSettings::default();
    for (widest, stitches) in [("7", Stitches::Satin), ("6", Stitches::Satin), ("5", Stitches::Satin), ("4.99", Stitches::FilledStroke)] {
        let element = with(line.clone(), &[(MAX_SATIN_WIDTH, widest)]);
        let own = stitchcraft_engine::auto::rules(&element.params).unwrap();
        assert_eq!(Stitches::of_stroke(Mm::new(5.0).unwrap(), &settings, &own), stitches, "{widest}");
        assert_eq!(made(&element).stitches, Some(stitches), "{widest}");
    }
    // A satin column has the overlap of the line it sews as its pull compensation.
    let column = made(&with(line, &[(OVERLAP, "0.6")]));
    assert_eq!(setting(&column.elements[0], PULL_COMPENSATION), Some("0.6"));
}

#[test]
fn req_auto_020_a_running_stitch_is_tripled_from_its_own_narrowest_triple_run() {
    let line = drawn("line", &[(0.0, 0.0), (30.0, 0.0)], 0.6);
    let tripled = |element: &Element| made(element).elements.iter().all(|part| setting(part, BEAN_STITCH_REPEATS) == Some("1"));
    assert!(tripled(&line));
    assert!(!tripled(&with(line.clone(), &[(MIN_TRIPLE_RUN_WIDTH, "0.8")])));
    assert!(tripled(&with(line, &[(MIN_TRIPLE_RUN_WIDTH, "0.6")])));
}

#[test]
fn req_auto_020_an_area_is_sorted_and_split_by_its_own_widest_satin_column() {
    // A band 5 mm wide sews as satin columns, and as a fill when its widest satin column is narrower.
    let band = rectangle("band", (0.0, 0.0), (30.0, 5.0), true);
    assert_eq!(made(&band).stitches, Some(Stitches::Satin));
    let filled = made(&with(band, &[(MAX_SATIN_WIDTH, "4")]));
    assert_eq!((filled.stitches, filled.elements.len()), (Some(Stitches::Fill), 1));
    // A square with a tab 3 mm wide: the tab is a narrow part, a column over the square's fill, unless its
    // widest satin column is narrower than the tab, which is then broad, filled with the square.
    let tabbed =
        fill("tabbed", &[&[(0.0, 0.0), (20.0, 0.0), (20.0, 8.5), (32.0, 8.5), (32.0, 11.5), (20.0, 11.5), (20.0, 20.0), (0.0, 20.0)]], &RED, &[]);
    assert_eq!(made(&tabbed).elements.len(), 2);
    assert_eq!(made(&with(tabbed, &[(MAX_SATIN_WIDTH, "2.5")])).elements.len(), 1);
}

#[test]
fn req_auto_020_an_area_reaches_its_own_overlap_under_the_shapes_sewn_after_it() {
    // 2 red squares one above the other, and a blue one beside both, sewn after them: each red square
    // reaches its own overlap under the blue one, and the lower red square under the upper one too.
    let lower = with(rectangle("lower", (0.0, 0.0), (10.0, 10.0), true), &[(OVERLAP, "0.8")]);
    let upper = rectangle("upper", (0.0, 10.0), (10.0, 20.0), true);
    let beside = rectangle("beside", (10.0, 0.0), (20.0, 20.0), false);
    let (out, said) = sewn(vec![lower, upper, beside]);
    let ids: Vec<&str> = out.iter().map(|element| element.id.as_str()).collect();
    assert_eq!(ids, ["lower", "upper", "beside"]);
    let reach = |id: &str| across(out.iter().find(|element| element.id.as_str() == id).unwrap()).1 - 10.0;
    assert!((reach("lower") - 0.8).abs() < 1e-6 && (reach("upper") - 0.4).abs() < 1e-6, "{} {}", reach("lower"), reach("upper"));
    assert_eq!(messages(&said, Code::AutoUnder), ["2 areas reach 0.4 to 0.8 mm under the shapes sewn after them that they touch."]);
}

#[test]
fn req_auto_020_an_area_s_specks_are_its_parts_smaller_than_its_own_smallest_area() {
    // A square with a part of 1.44 mm² far from it, which sews, a dot filled with the rest of the area
    // (REQ-AUTO-016), unless the area's smallest area is larger: then it is a speck that shares no edge with an
    // area, and is left out.
    let area =
        fill("area", &[&[(0.0, 0.0), (10.0, 0.0), (10.0, 10.0), (0.0, 10.0)], &[(30.0, 0.0), (31.2, 0.0), (31.2, 1.2), (30.0, 1.2)]], &RED, &[]);
    let (out, said) = sewn(vec![area.clone()]);
    assert!(messages(&said, Code::AutoSpeck).is_empty(), "{said:?}");
    assert_eq!(out.iter().map(|element| element.id.as_str()).collect::<Vec<_>>(), ["area"]);
    let (low, high) = across(&out[0]);
    assert!(low == 0.0 && (high - 31.2).abs() < 1e-9, "{low} {high}");
    let (out, said) = sewn(vec![with(area, &[(MIN_AREA, "2")])]);
    assert_eq!(
        messages(&said, Code::AutoSpeck),
        ["A part of this area, of 1.44 mm², is too small to sew and shares no edge with an automatic area, so it is left out."]
    );
    assert_eq!(across(&out[0]), (0.0, 10.0));
    // A part exactly as large as the smallest area sews: only a smaller one is a speck. A square 2 mm a side,
    // wider than a satin column of at most 1 mm, is filled with the rest of its area, and its 4 mm² are
    // exact, in binary too.
    let exact =
        fill("exact", &[&[(0.0, 0.0), (10.0, 0.0), (10.0, 10.0), (0.0, 10.0)], &[(30.0, 0.0), (32.0, 0.0), (32.0, 2.0), (30.0, 2.0)]], &RED, &[]);
    let (out, said) = sewn(vec![with(exact.clone(), &[(MAX_SATIN_WIDTH, "1"), (MIN_AREA, "4")])]);
    assert!(messages(&said, Code::AutoSpeck).is_empty(), "{said:?}");
    assert_eq!(across(&out[0]), (0.0, 32.0));
    let (out, said) = sewn(vec![with(exact, &[(MAX_SATIN_WIDTH, "1"), (MIN_AREA, "4.01")])]);
    assert_eq!(messages(&said, Code::AutoSpeck).len(), 1, "{said:?}");
    assert_eq!(across(&out[0]), (0.0, 10.0));
}

#[test]
fn req_auto_020_the_lowest_shape_sews_when_it_switches_the_background_off() {
    // A white square under a red one, covering the whole design: the background, left out unless it says not
    // to be.
    let ground = rectangle("ground", (0.0, 0.0), (60.0, 60.0), false);
    let badge = rectangle("badge", (20.0, 20.0), (40.0, 40.0), true);
    let (out, said) = sewn(vec![ground.clone(), badge.clone()]);
    assert_eq!((out.len(), messages(&said, Code::AutoBackground).len()), (1, 1));
    let (out, said) = sewn(vec![with(ground, &[(IGNORE_BACKGROUND, "false")]), badge]);
    assert_eq!((out.len(), messages(&said, Code::AutoBackground).len()), (2, 0));
}

#[test]
fn req_auto_020_a_value_out_of_range_is_its_nearest_and_one_that_doesnt_read_sews_nothing() {
    // An overlap of 5 mm is read as 2 mm, the most it takes, and said.
    let band = rectangle("band", (0.0, 0.0), (30.0, 5.0), true);
    let clamped = made(&with(band.clone(), &[(OVERLAP, "5")]));
    let [warning, ..] = clamped.diagnostics.as_slice() else { panic!("{:?}", clamped.diagnostics) };
    assert_eq!(
        (warning.code, warning.severity(), warning.element.as_ref().map(|id| id.as_str())),
        (Code::ParamClamped, Severity::Warning, Some("band"))
    );
    assert!(clamped.elements.iter().all(|element| setting(element, PULL_COMPENSATION) == Some("2")), "{:?}", clamped.elements);
    // A widest satin column that isn't a length: the shape sews nothing, and that is said, naming it.
    let unread = made(&with(band.clone(), &[(MAX_SATIN_WIDTH, "wide")]));
    assert_eq!((unread.elements.len(), unread.stitches), (0, None));
    let [error] = unread.diagnostics.as_slice() else { panic!("{:?}", unread.diagnostics) };
    assert_eq!((error.code, error.element.as_ref().map(|id| id.as_str())), (Code::ParamInvalid, Some("band")));
    // In a design, the others still sew.
    let (out, said) = sewn(vec![with(band, &[(MAX_SATIN_WIDTH, "wide")]), rectangle("other", (40.0, 0.0), (50.0, 10.0), true)]);
    assert_eq!(out.iter().map(|element| element.id.as_str()).collect::<Vec<_>>(), ["other"]);
    assert_eq!(said.iter().filter(|d| d.code == Code::ParamInvalid).count(), 1);
}
