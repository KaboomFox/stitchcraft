//! The files the Ink/Stitch differential writes (`stitchcraft_testkit::inkstitch`, by the writer
//! `stitchcraft_svg::written`), read back by StitchCraft's reader, which reads each element's shape, colour,
//! parameters, hints and clips as they were written (`REQ-PRM-003`, `REQ-SVG-009`).

// Test code may unwrap and index (clippy.toml allows it in tests).
#![allow(clippy::unwrap_used, clippy::panic, clippy::indexing_slicing)]

use stitchcraft_core::{Budget, Mm};
use stitchcraft_engine::design::{Cap, Clip, FillRule, Hints, Join, Path, Segment, Shape, Subpath, Turn};
use stitchcraft_engine::generators::tatami::TURN_STITCHES;
use stitchcraft_engine::testsheets;
use stitchcraft_plan::Rgb;
use stitchcraft_svg::version::CURRENT;

use stitchcraft_engine::design::{DesignSettings, Element};
use stitchcraft_engine::testsheets::Cloned;
use stitchcraft_testkit::designs::{BLUE, RED, along, capped, fill, p, polylines, rings, widened};
use stitchcraft_testkit::inkstitch::{sewn_alike, svg, svg_styled, svg_with};

/// What a file keeps of an element: its shape, its thread's colour, its parameters, its hints and its clips.
type Kept = (Shape, Rgb, Vec<(String, String)>, Hints, Vec<Clip>);

/// What the file `text` keeps of each element, and the design settings StitchCraft reads from it, as
/// StitchCraft's SVG reader reads them, which says nothing of a file the test kit writes: the shapes,
/// colours, parameters, hints and clips (`REQ-PRM-003`).
fn read(text: &str) -> (Vec<Kept>, DesignSettings) {
    let read = stitchcraft_svg::read(text.as_bytes(), &Budget::DEFAULT).unwrap();
    assert!(read.warnings.is_empty(), "{:?}", read.warnings);
    let elements = read.design.elements();
    // Connectors, clip paths and guide and anchor lines are no elements of their own.
    let helpers = text.matches("marker-start:url(#inkstitch-").count();
    let others = text.matches("inkscape:connection-start").count() + text.matches("<path clip-rule").count() + helpers;
    let clones = text.matches(" xlink:href=\"#e").count();
    assert_eq!(elements.len(), text.matches("<path ").count() - others + clones, "an element for each path and each clone");
    (kept(elements), read.design.settings)
}

/// What a file should keep of `elements`: all but `turn_stitches`, which a file leaves out.
fn kept(elements: &[Element]) -> Vec<Kept> {
    let parameters =
        |element: &Element| element.params.iter().filter(|(k, _)| *k != TURN_STITCHES).map(|(k, v)| (k.to_string(), v.to_string())).collect();
    elements
        .iter()
        .map(|element| (element.shape.clone(), element.thread.color, parameters(element), element.hints.clone(), element.clips.clone()))
        .collect()
}

#[test]
fn req_prm_003_the_file_keeps_each_elements_shape_thread_and_parameters() {
    // A red square with a hole cut by the even-odd rule, and a blue curve 2 mm wide with round joins and
    // square caps, each with parameters.
    let square = fill(
        "square",
        &[&[(10.0, 10.0), (20.0, 10.0), (20.0, 20.0), (10.0, 20.0)], &[(13.0, 13.0), (17.0, 13.0), (17.0, 17.0)]],
        &RED,
        &[("fill_underlay", "false")],
    );
    let square = Element { shape: Shape::Fill { path: square.shape.path().clone(), rule: FillRule::EvenOdd, gradient: None }, ..square };
    let curve = Path {
        subpaths: vec![Subpath {
            start: p(30.0, 15.0),
            segments: vec![Segment::Quad(p(35.0, 10.0), p(40.0, 15.0)), Segment::Cubic(p(42.0, 17.0), p(44.0, 17.0), p(46.0, 15.0))],
            closed: false,
        }],
    };
    let line = capped(widened(along("curve", curve, &BLUE, &[("satin_column", "true"), ("pull_compensation_mm", "0.4")]), 2.0), Cap::Square);
    let Shape::Stroke { path, width, cap, .. } = line.shape.clone() else { panic!("{:?}", line.shape) };
    let line = Element { shape: Shape::Stroke { path, width, join: Join::Round, cap }, ..line };
    let written = [square, line];
    let text = svg(&written, &DesignSettings::default()).unwrap();
    assert!(text.contains(&format!("<inkstitch:inkstitch_svg_version>{CURRENT}</inkstitch:inkstitch_svg_version>")), "{text}");
    assert!(text.contains("viewBox=\"0 0 56 30\""), "the design's own coordinates: {text}");
    assert!(text.contains("stroke=\"#0000c8\" stroke-width=\"2\" stroke-linejoin=\"round\" stroke-linecap=\"square\""), "{text}");
    assert_eq!(read(&text), (kept(&written), DesignSettings::default()));
    // Painted by style attributes, as Inkscape paints, the file is read back the same.
    let styled = svg_styled(&written, &DesignSettings::default()).unwrap();
    assert!(styled.contains("style=\"fill:#c80000;fill-rule:evenodd;stroke:none\""), "{styled}");
    assert!(styled.contains("style=\"fill:none;stroke:#0000c8;stroke-width:2;stroke-linejoin:round;stroke-linecap:square\""), "{styled}");
    assert_eq!(read(&styled), (kept(&written), DesignSettings::default()));
}

#[test]
fn req_prm_003_an_element_s_hints_are_written_as_ink_stitch_s_commands() {
    // A fill that starts and ends at its corners and turns round a point off its middle, and a line with
    // a start alone: each hint a command tied to its element, which the reader places where it was.
    let square = fill("square", &[&[(10.0, 10.0), (20.0, 10.0), (20.0, 20.0), (10.0, 20.0)]], &RED, &[("fill_method", "circular_fill")]);
    let square =
        Element { hints: Hints { start: Some(p(10.0, 10.0)), end: Some(p(20.5, 20.25)), target: Some(p(12.0, 18.0)), ..Hints::default() }, ..square };
    let line = along("line", polylines(&[&[(30.0, 5.0), (40.0, 5.0)]]), &BLUE, &[]);
    let line = Element { hints: Hints { start: Some(p(31.0, 5.0)), ..Hints::default() }, ..line };
    let written = [square, line];
    let text = svg(&written, &DesignSettings::default()).unwrap();
    assert_eq!(text.matches("<symbol ").count(), 3, "a symbol for each command used: {text}");
    assert!(text.contains(r##"<use id="e0-ending_point" xlink:href="#inkstitch_ending_point" x="20.5" y="20.25"/>"##), "{text}");
    assert_eq!(read(&text).0, kept(&written));
}

#[test]
fn req_prm_003_an_element_s_clips_are_written_as_clip_paths() {
    // A fill cut to 2 clips, the second of 2 shapes, one of them by the even-odd rule: the first clip path
    // on its path, the second on a group round it, which the reader reads back as they were.
    let ring = |x: f64| rings(&[&[(x, 0.0), (x + 10.0, 0.0), (x + 10.0, 10.0), (x, 10.0)]]);
    let square = stitchcraft_testkit::designs::clipped(
        fill("square", &[&[(0.0, 0.0), (30.0, 0.0), (30.0, 30.0)]], &RED, &[]),
        &[&[(0.0, 0.0), (20.0, 0.0), (20.0, 20.0)]],
    );
    let mut square = square;
    square.clips.push(Clip {
        shapes: vec![
            stitchcraft_engine::design::ClipShape { path: ring(0.0), rule: FillRule::NonZero },
            stitchcraft_engine::design::ClipShape { path: ring(15.0), rule: FillRule::EvenOdd },
        ]
        .into(),
    });
    let text = svg(std::slice::from_ref(&square), &DesignSettings::default()).unwrap();
    assert!(text.contains("<g clip-path=\"url(#e0-clip1)\">\n  <path id=\"e0\" "), "{text}");
    assert!(text.contains(" clip-path=\"url(#e0-clip0)\""), "{text}");
    assert_eq!(text.matches("<path clip-rule=\"evenodd\"").count(), 1, "{text}");
    assert_eq!(read(&text).0, kept(&[square]));
}

#[test]
fn req_prm_003_a_fill_s_linear_gradient_is_written_as_one() {
    // A square under a gradient of 3 stops, its middle one transparent, in the thread of its first stop:
    // the reader places it where it was, by attributes or by style alike.
    let stops = [(0.0, RED.color, 1.0), (0.375, Rgb::new(240, 240, 240), 0.0), (1.0, BLUE.color, 0.5)]
        .map(|(offset, colour, opacity)| stitchcraft_engine::design::GradientStop { offset, colour, opacity })
        .to_vec();
    let gradient = stitchcraft_engine::design::Gradient { start: p(10.5, 20.0), end: p(30.25, 12.0), stops };
    let square = fill("square", &[&[(10.0, 10.0), (30.0, 10.0), (30.0, 30.0), (10.0, 30.0)]], &RED, &[("fill_method", "linear_gradient_fill")]);
    let square = Element { shape: Shape::Fill { path: square.shape.path().clone(), rule: FillRule::NonZero, gradient: Some(gradient) }, ..square };
    let text = svg(std::slice::from_ref(&square), &DesignSettings::default()).unwrap();
    assert!(text.contains(r#"<linearGradient id="e0-gradient" gradientUnits="userSpaceOnUse" x1="10.5" y1="20" x2="30.25" y2="12">"#), "{text}");
    assert!(text.contains(r##"<stop offset="0.375" stop-color="#f0f0f0" stop-opacity="0"/>"##), "{text}");
    assert!(text.contains(r##"fill="url(#e0-gradient)""##), "{text}");
    assert_eq!(read(&text).0, kept(std::slice::from_ref(&square)));
    let styled = svg_styled(std::slice::from_ref(&square), &DesignSettings::default()).unwrap();
    assert_eq!(read(&styled).0, kept(&[square]));
}

#[test]
fn what_lies_left_of_the_origin_or_above_it_keeps_its_coordinates() {
    let line = along("line", polylines(&[&[(-5.0, 2.0), (5.0, -3.0)]]), &RED, &[]);
    let text = svg(std::slice::from_ref(&line), &DesignSettings::default()).unwrap();
    assert!(text.contains("viewBox=\"0 0 15 12\""), "{text}");
    assert_eq!(read(&text).0, kept(&[line]));
    // A design wholly left of the origin and above it still has a page.
    let far = along("far", polylines(&[&[(-50.0, -40.0), (-30.0, -20.0)]]), &RED, &[]);
    let text = svg(std::slice::from_ref(&far), &DesignSettings::default()).unwrap();
    assert!(text.contains("viewBox=\"0 0 10 10\""), "{text}");
    assert_eq!(read(&text).0, kept(&[far]));
}

#[test]
fn design_settings_other_than_stitchcrafts_defaults_and_escaped_values_are_written() {
    let line = along("ring", rings(&[&[(0.0, 0.0), (4.0, 0.0), (4.0, 4.0)]]), &RED, &[("manual_stitch_pattern", "\"a\" & <b>")]);
    let settings = DesignSettings { min_stitch_len: Some(Mm::new(0.5).unwrap()), collapse_len: Mm::new(1.0).unwrap(), ..DesignSettings::default() };
    let text = svg(std::slice::from_ref(&line), &settings).unwrap();
    assert!(text.contains("d=\"M 0,0 L 4,0 L 4,4 Z\""), "{text}");
    assert!(text.contains("inkstitch:manual_stitch_pattern=\"&quot;a&quot; &amp; &lt;b&gt;\""), "{text}");
    assert!(text.contains("<inkstitch:min_stitch_len_mm>0.5</inkstitch:min_stitch_len_mm>"), "{text}");
    assert!(text.contains("<inkstitch:collapse_len_mm>1</inkstitch:collapse_len_mm>"), "{text}");
    // The narrowest satin stroke is StitchCraft's default, which Ink/Stitch's is too.
    assert!(!text.contains("min_satin_stroke_width_mm"), "{text}");
    assert!(text.contains("stroke-linejoin=\"miter\" stroke-miterlimit=\"5\" stroke-linecap=\"butt\""), "{text}");
    assert_eq!(read(&text), (kept(&[line]), settings));
}

#[test]
fn req_prm_003_the_design_s_origin_stop_position_and_turn_are_written_as_ink_stitch_keeps_them() {
    let line = along("ring", rings(&[&[(0.0, 0.0), (4.0, 0.0), (4.0, 4.0)]]), &RED, &[("stop_after", "true")]);
    let settings = DesignSettings { origin: Some(p(1.0, 2.0)), stop_position: Some(p(30.5, -4.0)), ..DesignSettings::default() };
    let text = svg(std::slice::from_ref(&line), &settings).unwrap();
    assert!(text.contains(r##"<use id="origin" xlink:href="#inkstitch_origin" x="1" y="2"/>"##), "{text}");
    assert!(text.contains(r##"<symbol id="inkstitch_stop_position">"##), "{text}");
    assert_eq!(read(&text), (kept(std::slice::from_ref(&line)), settings));
    // The turn on export is a design setting in the metadata, in degrees.
    for (turn, written) in [(Turn::LEFT, Some("-90")), (Turn::new(45.5).unwrap(), Some("45.5")), (Turn::NONE, None)] {
        let settings = DesignSettings { rotate_on_export: turn, ..DesignSettings::default() };
        let text = svg(std::slice::from_ref(&line), &settings).unwrap();
        match written {
            Some(degrees) => assert!(text.contains(&format!("<inkstitch:rotate_on_export>{degrees}</inkstitch:rotate_on_export>")), "{text}"),
            None => assert!(!text.contains("rotate_on_export"), "{text}"),
        }
        assert_eq!(read(&text).1, settings);
    }
}

#[test]
fn stitchcrafts_own_parameters_are_refused_but_automatic_digitizings_are_left_out() {
    let line = |params: &[(&str, &str)]| along("line", polylines(&[&[(0.0, 0.0), (4.0, 0.0)]]), &RED, params);
    let text = svg(&[line(&[("auto_overlap_mm", "0.6"), ("running_stitch_length_mm", "2")])], &DesignSettings::default()).unwrap();
    assert_eq!(read(&text).0, kept(&[line(&[("running_stitch_length_mm", "2")])]));
    // `underlays_first` is StitchCraft's own, and every stitch type reads it: Ink/Stitch would sew the
    // element's underlay just before its top stitches, whatever it says.
    let refused = svg(&[line(&[("running_stitch_length_mm", "2"), ("underlays_first", "true")])], &DesignSettings::default()).unwrap_err();
    assert_eq!(refused, "line: `underlays_first` is StitchCraft's own, and Ink/Stitch would sew the element without it");
    assert!(svg(&[line(&[("underlays_first", "false")])], &DesignSettings::default()).is_err(), "whatever its value");
    // So the elements both programs sew alike leave it out, and keep the others in their order.
    let (first, second, third) = (line(&[]), line(&[("underlays_first", "true")]), line(&[("auto_overlap_mm", "0.6")]));
    assert_eq!(sewn_alike(&[first.clone(), second, third.clone()]), [first, third]);
    // `turn_stitches` is StitchCraft's own too, and a fill reads it, but Ink/Stitch keeps a fill's turns
    // without it: the file leaves it out, whatever its value, and both programs sew the element.
    let square = |params: &[(&str, &str)]| fill("square", &[&[(0.0, 0.0), (4.0, 0.0), (4.0, 4.0), (0.0, 4.0)]], &RED, params);
    for value in ["true", "false"] {
        let turning = square(&[("turn_stitches", value), ("row_spacing_mm", "0.3")]);
        let text = svg(std::slice::from_ref(&turning), &DesignSettings::default()).unwrap();
        assert!(!text.contains("turn_stitches") && text.contains("inkstitch:row_spacing_mm=\"0.3\""), "{text}");
        assert_eq!(sewn_alike(std::slice::from_ref(&turning)), [turning]);
    }
    assert_eq!(svg(&[], &DesignSettings::default()).unwrap_err(), "no element has a point");
}

#[test]
fn req_prm_003_the_file_of_each_sheet_drawn_as_a_design_keeps_the_elements_it_sews() {
    // The TS-13 sheets' shapes as automatic digitizing makes them, each sheet's other elements as they are,
    // and TS-16's clone a clone in the file. TS-17C's crossing columns with their underlays first are left
    // out, as Ink/Stitch would sew them otherwise.
    let mut checked = 0;
    for sheet in testsheets::SHEETS.iter().filter(|sheet| sheet.designed()) {
        let (id, design, clones) = (sheet.id, sheet.design().unwrap().unwrap(), sheet.clones().unwrap().unwrap());
        let (digitized, _) = stitchcraft_testkit::inkstitch::digitized(&design, &Budget::DEFAULT);
        let alike = sewn_alike(&digitized);
        let left_out: Vec<&str> = digitized.iter().filter(|e| !alike.contains(e)).map(|e| e.id.as_str()).collect();
        let own = if id == "TS-17C" { &["ts17c:crossing-first:bar", "ts17c:crossing-first:post"][..] } else { &[] };
        assert_eq!(left_out, own, "{id}");
        let text = svg_with(&alike, &design.settings, &clones).unwrap();
        assert_eq!(text.matches("transform=\"matrix(").count(), clones.len(), "{id}");
        assert_eq!(read(&text), (kept(&alike), design.settings), "{id}");
        checked += usize::from(sheet.digitized());
    }
    assert!(checked >= 4, "the TS-13 sheets are drawn as designs");
}

#[test]
fn req_prm_003_an_element_drawn_as_a_clone_is_a_use_of_its_source() {
    // A square that starts and ends at its corners, and its clone turned a quarter right about (20, 0): its
    // corners and positions turned, and its rows, at 0°, turned to -90°, as the reader turns them.
    let square = fill("square", &[&[(0.0, 0.0), (10.0, 0.0), (10.0, 5.0), (0.0, 5.0)]], &RED, &[("row_spacing_mm", "0.4")]);
    let square = Element { hints: Hints { start: Some(p(10.0, 0.0)), end: Some(p(0.0, 5.0)), ..Hints::default() }, ..square };
    let map = [0.0, 1.0, -1.0, 0.0, 20.0, -20.0];
    let turned = |x: f64, y: f64| p(-y + 20.0, x - 20.0);
    let ring = vec![&[(20.0, -20.0), (20.0, -10.0), (15.0, -10.0), (15.0, -20.0)][..]];
    let clone = fill("clone", &ring, &RED, &[("angle", "-90"), ("row_spacing_mm", "0.4")]);
    let clone = Element { hints: Hints { start: Some(turned(10.0, 0.0)), end: Some(turned(0.0, 5.0)), ..Hints::default() }, ..clone };
    let cloned = Cloned { element: clone.id.clone(), of: square.id.clone(), map };
    let written = [square, clone];
    let text = svg_with(&written, &DesignSettings::default(), std::slice::from_ref(&cloned)).unwrap();
    assert!(text.contains("  <use id=\"e1\" xlink:href=\"#e0\" transform=\"matrix(0 1 -1 0 20 -20)\"/>\n"), "{text}");
    assert_eq!(text.matches("<g>").count(), 2, "the square's commands, each in a group: {text}");
    assert_eq!(read(&text).0, kept(&written));
    // A clone of an element the design doesn't have can't be written.
    let alone = Cloned { of: stitchcraft_core::ElementId::new("gone").unwrap(), ..cloned };
    assert_eq!(svg_with(&written, &DesignSettings::default(), &[alone]).unwrap_err(), "clone: a clone of gone, which the design doesn't have");
}
