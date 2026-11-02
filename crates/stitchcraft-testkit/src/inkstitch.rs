//! A design's elements written as an SVG file that Ink/Stitch reads, with their parameters, for the Ink/Stitch
//! differential's cases of test sheets (`docs/src/design/conformance.md` › L3).
//!
//! **Why.** Automatic digitizing ends in ordinary elements whose settings are Ink/Stitch's parameters:
//! `satin_column`, `pull_compensation_mm`, the underlays, `bean_stitch_repeats`. Written with them, the
//! elements are a design Ink/Stitch sews with the same stitch types, so the stitches StitchCraft makes of
//! its own choices can be checked against another program's (`REQ-AUTO-021`). StitchCraft's SVG reader
//! reads no element's parameters until M8, so the differential has StitchCraft sew the sheet itself, and
//! Ink/Stitch the file. The tests read the file back: its shapes and colours as StitchCraft's reader reads
//! them, and its parameters as XML.
//!
//! **The file.** Each element is a `<path>`, in the order given, which is the order both programs sew them
//! in. A fill is painted with its thread's colour by its fill rule, and a stroke with its colour, width,
//! join and cap. A satin column drawn as one path keeps its stroke's width, which both sew as a column that
//! wide (`REQ-SAT-016`). Each parameter is an `inkstitch:` attribute of its own name. One of StitchCraft's
//! own that a stitch type reads is refused, as Ink/Stitch would pass it over and sew otherwise. Automatic
//! digitizing's own are left out, as no stitch type reads them in either program (`REQ-AUTO-020`).
//!
//! The metadata gives Ink/Stitch's current file version ([`stitchcraft_svg::version::CURRENT`]), so its
//! updater changes nothing, and each design setting that isn't StitchCraft's default. One that is stays
//! unset, and Ink/Stitch takes its own default, StitchCraft's too but for the shortest stitch, which
//! StitchCraft leaves to the machine and Ink/Stitch takes as 0.1 mm (`DEV-FIN-001`).
//!
//! Lengths are in millimetres, one to the user unit, and the view box starts at the origin, so the file keeps
//! the design's coordinates. The page reaches [`MARGIN`] past the design's right and bottom. What lies left
//! of the origin or above it is off the page, which a viewer leaves out, but both programs sew it.

use stitchcraft_core::Rect;
use stitchcraft_engine::design::{Cap, DesignSettings, Element, FillRule, Join, Path, SETTINGS, Segment, Shape};
use stitchcraft_engine::registry::PARAMETERS;
use stitchcraft_params::{Origin, ParamGroup};
use stitchcraft_svg::inkstitch::INKSTITCH_NS;
use stitchcraft_svg::version::CURRENT;

/// How far the page reaches past the elements' paths, right and down, in millimetres: room for the widest
/// stroke.
pub const MARGIN: f64 = 10.0;

/// `elements`, in order, as an SVG file that Ink/Stitch reads, with the design's `settings` (see the
/// module's docs); or why they can't be: no element has a point, or an element has a parameter of
/// StitchCraft's own that a stitch type reads.
pub fn svg(elements: &[Element], settings: &DesignSettings) -> Result<String, String> {
    let Some(bounds) = Rect::around(elements.iter().flat_map(|element| element.shape.path().points())) else {
        return Err("no element has a point".to_string());
    };
    let (width, height) = ((bounds.max().x() + MARGIN).max(MARGIN), (bounds.max().y() + MARGIN).max(MARGIN));
    let mut out = String::from("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
    out.push_str(&format!("<svg xmlns=\"http://www.w3.org/2000/svg\" xmlns:inkstitch=\"{INKSTITCH_NS}\" "));
    out.push_str(&format!("width=\"{width}mm\" height=\"{height}mm\" viewBox=\"0 0 {width} {height}\">\n"));
    out.push_str("  <metadata>\n");
    out.push_str(&format!("    <inkstitch:inkstitch_svg_version>{CURRENT}</inkstitch:inkstitch_svg_version>\n"));
    let defaults = DesignSettings::default();
    for setting in SETTINGS.iter().filter(|setting| setting.get(settings) != setting.get(&defaults)) {
        out.push_str(&format!("    <inkstitch:{0}>{1}</inkstitch:{0}>\n", setting.name, setting.get(settings).get()));
    }
    out.push_str("  </metadata>\n");
    for (index, element) in elements.iter().enumerate() {
        let parameters = parameters(element, PARAMETERS)?;
        out.push_str(&format!("  <path id=\"e{index}\" {}{parameters} d=\"{}\"/>\n", paint(element), data(element.shape.path())));
    }
    out.push_str("</svg>\n");
    Ok(out)
}

/// How `element` is painted, as attributes: its fill, or its stroke.
fn paint(element: &Element) -> String {
    let colour = element.thread.color;
    let colour = format!("#{:02x}{:02x}{:02x}", colour.r, colour.g, colour.b);
    match &element.shape {
        Shape::Fill { rule, .. } => {
            let rule = match rule {
                FillRule::NonZero => "nonzero",
                FillRule::EvenOdd => "evenodd",
            };
            format!("fill=\"{colour}\" fill-rule=\"{rule}\" stroke=\"none\"")
        }
        Shape::Stroke { width, join, cap, .. } => {
            let join = match join {
                Join::Miter { limit } => format!("stroke-linejoin=\"miter\" stroke-miterlimit=\"{limit}\""),
                Join::Round => "stroke-linejoin=\"round\"".to_string(),
                Join::Bevel => "stroke-linejoin=\"bevel\"".to_string(),
            };
            let cap = match cap {
                Cap::Butt => "butt",
                Cap::Round => "round",
                Cap::Square => "square",
            };
            format!("fill=\"none\" stroke=\"{colour}\" stroke-width=\"{}\" {join} stroke-linecap=\"{cap}\"", width.get())
        }
    }
}

/// `element`'s parameters as `inkstitch:` attributes, each with a space before it, but automatic
/// digitizing's own, by the parameters of `registry`; or the first of StitchCraft's own that a stitch type
/// reads.
fn parameters(element: &Element, registry: &[&ParamGroup]) -> Result<String, String> {
    let mut out = String::new();
    for (key, value) in element.params.iter() {
        if let Some(spec) = stitchcraft_params::find(registry, key) {
            if spec.applies_to.is_empty() {
                continue;
            }
            if spec.origin == Origin::StitchCraft {
                return Err(format!("{}: `{key}` is StitchCraft's own, and Ink/Stitch would sew the element without it", element.id));
            }
        }
        out.push_str(&format!(" inkstitch:{key}=\"{}\"", escaped(value)));
    }
    Ok(out)
}

/// `text` with the characters that would end an attribute's value or begin markup in it written as
/// references.
fn escaped(text: &str) -> String {
    text.replace('&', "&amp;").replace('"', "&quot;").replace('<', "&lt;").replace('>', "&gt;")
}

/// `path` as SVG path data, in millimetres: also the outlines the Ink/Stitch differential draws its centre
/// lines over.
pub fn data(path: &Path) -> String {
    let mut out = Vec::new();
    for subpath in &path.subpaths {
        let at = |q: stitchcraft_core::Point| format!("{},{}", q.x(), q.y());
        out.push(format!("M {}", at(subpath.start)));
        for segment in &subpath.segments {
            out.push(match *segment {
                Segment::Line(end) => format!("L {}", at(end)),
                Segment::Quad(control, end) => format!("Q {} {}", at(control), at(end)),
                Segment::Cubic(first, second, end) => format!("C {} {} {}", at(first), at(second), at(end)),
            });
        }
        if subpath.closed {
            out.push("Z".to_string());
        }
    }
    out.join(" ")
}

#[cfg(test)]
mod tests {
    use stitchcraft_core::{Budget, Mm};
    use stitchcraft_engine::design::Subpath;
    use stitchcraft_engine::testsheets;
    use stitchcraft_params::ParamSpec;
    use stitchcraft_plan::Rgb;

    use super::*;
    use crate::designs::{BLUE, RED, along, capped, fill, p, polylines, rings, widened};

    /// What a file keeps of an element: its shape, its thread's colour and its parameters.
    type Kept = (Shape, Rgb, Vec<(String, String)>);

    /// What the file `text` keeps of each element, and the design settings StitchCraft reads from it: the
    /// shapes and colours as StitchCraft's SVG reader reads them, and the parameters as the `inkstitch:`
    /// attributes of each `<path>`, read as XML. The reader reads no element's parameters until M8, and
    /// says so once.
    fn read(text: &str) -> (Vec<Kept>, DesignSettings) {
        let read = stitchcraft_svg::read(text.as_bytes(), &Budget::DEFAULT).unwrap();
        assert!(read.warnings.len() <= 1 && read.warnings.iter().all(|w| w.message.contains("does not read yet")), "{:?}", read.warnings);
        let xml = roxmltree::Document::parse(text).unwrap();
        let attributes: Vec<Vec<(String, String)>> = xml
            .descendants()
            .filter(|node| node.has_tag_name("path"))
            .map(|node| {
                node.attributes().filter(|a| a.namespace() == Some(INKSTITCH_NS)).map(|a| (a.name().to_string(), a.value().to_string())).collect()
            })
            .collect();
        let elements = read.design.elements();
        assert_eq!(elements.len(), attributes.len(), "an element for each path");
        let kept = elements.iter().zip(attributes).map(|(element, params)| (element.shape.clone(), element.thread.color, params)).collect();
        (kept, read.design.settings)
    }

    /// What a file should keep of `elements`.
    fn kept(elements: &[Element]) -> Vec<Kept> {
        let parameters = |element: &Element| element.params.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect();
        elements.iter().map(|element| (element.shape.clone(), element.thread.color, parameters(element))).collect()
    }

    #[test]
    fn the_file_keeps_each_elements_shape_thread_and_parameters() {
        // A red square with a hole cut by the even-odd rule, and a blue curve 2 mm wide with round joins and
        // square caps, each with parameters.
        let square = fill(
            "square",
            &[&[(10.0, 10.0), (20.0, 10.0), (20.0, 20.0), (10.0, 20.0)], &[(13.0, 13.0), (17.0, 13.0), (17.0, 17.0)]],
            &RED,
            &[("fill_underlay", "false")],
        );
        let square = Element { shape: Shape::Fill { path: square.shape.path().clone(), rule: FillRule::EvenOdd }, ..square };
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
        assert_eq!(read(&text), (kept(&written), DesignSettings::default()));
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
        let settings =
            DesignSettings { min_stitch_len: Some(Mm::new(0.5).unwrap()), collapse_len: Mm::new(1.0).unwrap(), ..DesignSettings::default() };
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
    fn stitchcrafts_own_parameters_are_refused_but_automatic_digitizings_are_left_out() {
        let line = |params: &[(&str, &str)]| along("line", polylines(&[&[(0.0, 0.0), (4.0, 0.0)]]), &RED, params);
        let text = svg(&[line(&[("auto_overlap_mm", "0.6"), ("running_stitch_length_mm", "2")])], &DesignSettings::default()).unwrap();
        assert_eq!(read(&text).0, kept(&[line(&[("running_stitch_length_mm", "2")])]));
        // No parameter of StitchCraft's own is read by a stitch type yet: one in a registry of its own.
        let running = stitchcraft_params::find(PARAMETERS, "running_stitch_length_mm").unwrap();
        let own: &'static [ParamSpec] = Box::leak(Box::new([ParamSpec { key: "own_length_mm", origin: Origin::StitchCraft, ..*running }]));
        let registry = [&*Box::leak(Box::new(ParamGroup { specs: own, ..**PARAMETERS.first().unwrap() }))];
        assert_eq!(parameters(&line(&[("running_stitch_length_mm", "2")]), &registry), Ok(" inkstitch:running_stitch_length_mm=\"2\"".to_string()));
        let refused = parameters(&line(&[("own_length_mm", "2")]), &registry).unwrap_err();
        assert_eq!(refused, "line: `own_length_mm` is StitchCraft's own, and Ink/Stitch would sew the element without it");
        assert_eq!(svg(&[], &DesignSettings::default()).unwrap_err(), "no element has a point");
    }

    #[test]
    fn the_file_of_each_ts13_sheet_keeps_the_elements_automatic_digitizing_makes_of_it() {
        for sheet in testsheets::SHEETS.iter().filter(|sheet| sheet.digitized()) {
            let (id, design) = (sheet.id, sheet.design().unwrap().unwrap());
            let (digitized, _) = stitchcraft_engine::auto::digitized(&design, &Budget::DEFAULT);
            let text = svg(&digitized, &design.settings).unwrap();
            assert_eq!(read(&text), (kept(&digitized), design.settings), "{id}");
        }
    }
}
