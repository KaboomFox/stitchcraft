//! A design's elements written as an SVG file that Ink/Stitch reads, with their parameters, for the Ink/Stitch
//! differential's cases of test sheets (`docs/src/design/conformance.md` › L3).
//!
//! **Why.** Automatic digitizing ends in ordinary elements whose settings are Ink/Stitch's parameters:
//! `satin_column`, `pull_compensation_mm`, the underlays, `bean_stitch_repeats`. Written with them, the
//! elements are a design Ink/Stitch sews with the same stitch types, so the stitches StitchCraft makes of
//! its own choices can be checked against another program's (`REQ-AUTO-021`), and both programs sew the
//! same file. The tests read the file back with StitchCraft's SVG reader, which reads each element's
//! shape, colour and parameters as they were written (`REQ-PRM-003`, `REQ-SVG-009`).
//!
//! **The file.** Each element is a `<path>`, in the order given, which is the order both programs sew them
//! in. A fill is painted with its thread's colour by its fill rule, and a stroke with its colour, width,
//! join and cap. A satin column drawn as one path keeps its stroke's width, which both sew as a column that
//! wide (`REQ-SAT-016`). Each parameter is an `inkstitch:` attribute of its own name. One of StitchCraft's
//! own that a stitch type reads is refused, as Ink/Stitch would pass it over and sew otherwise: an element
//! that sets `underlays_first` sews its underlay in a run of them (`REQ-ASM-009`), and in Ink/Stitch just
//! before its top stitches. The differential writes a sheet's file of the elements both programs sew alike,
//! [`sewn_alike`], and leaves out the others, as TS-17C's crossing columns with their underlays first.
//! Automatic digitizing's own are left out, as no stitch type reads them in either program
//! (`REQ-AUTO-020`).
//!
//! An element's start and end positions and its target point are Ink/Stitch's `starting_point`,
//! `ending_point` and `target_point` commands (`REQ-GEN-001`): a `<use>` of the command's symbol at the
//! position, and a connector from it to the element's path, in a group, as Ink/Stitch's Attach Commands draws
//! them. Ink/Stitch moves the commands of a clone's source with the clone only when they are in a group.
//!
//! An element's clips are clip paths (`REQ-GEN-005`), each a `<clipPath>` of a `<path>` for each of its
//! shapes, by its `clip-rule`, in user space. The first clips the element's path, and each one after it a
//! group round the path, as Inkscape clips a group, which both programs read (`REQ-SVG-011`). Ink/Stitch
//! takes a clip's shapes as one outline and its holes, whatever their rules (`DEV-SVG-016`), so a case that
//! both sew gives each clip shapes that don't overlap.
//!
//! An element the design's file draws as a clone of another ([`Cloned`], as TS-16's does) is a `<use>` of
//! that element's path under the clone's transform (`REQ-SVG-012`), in its place. Both programs read it as
//! what the clone shows: the source's shape and positions moved, and its settings, the fill's angle turned
//! (`REQ-SVG-013`). The tests read it back as the element the design holds, so the clone's element is what
//! the file's clone is.
//!
//! The design's origin and stop position are Ink/Stitch's `origin` and `stop_position` commands
//! (`REQ-ASM-005`): a `<use>` of the command's symbol at the point, standing alone, as Ink/Stitch's Add
//! Commands draws them.
//!
//! The metadata gives Ink/Stitch's current file version ([`stitchcraft_svg::version::CURRENT`]), so its
//! updater changes nothing, and each design setting that isn't StitchCraft's default, the turn on export
//! (`rotate_on_export`) among them, in degrees. One that is stays
//! unset, and Ink/Stitch takes its own default, StitchCraft's too but for the shortest stitch, which
//! StitchCraft leaves to the machine and Ink/Stitch takes as 0.1 mm (`DEV-FIN-001`).
//!
//! Lengths are in millimetres, one to the user unit, and the view box starts at the origin, so the file keeps
//! the design's coordinates. The page reaches [`MARGIN`] past the design's right and bottom. What lies left
//! of the origin or above it is off the page, which a viewer leaves out, but both programs sew it.

use stitchcraft_core::{Point, Rect};
#[cfg(test)]
use stitchcraft_engine::design::Clip;
use stitchcraft_engine::design::{Cap, DesignSettings, Element, FillRule, Hints, Join, Path, SETTINGS, Segment, Shape, Turn};
use stitchcraft_engine::registry::PARAMETERS;
use stitchcraft_engine::testsheets::Cloned;
use stitchcraft_params::{Origin, ParamGroup, ParamSpec};
use stitchcraft_svg::inkstitch::INKSTITCH_NS;
use stitchcraft_svg::version::CURRENT;

/// How far the page reaches past the elements' paths, right and down, in millimetres: room for the widest
/// stroke.
pub const MARGIN: f64 = 10.0;

/// `elements`, in order, as an SVG file that Ink/Stitch reads, with the design's `settings` (see the
/// module's docs); or why they can't be: no element has a point, or an element has a parameter of
/// StitchCraft's own that a stitch type reads.
pub fn svg(elements: &[Element], settings: &DesignSettings) -> Result<String, String> {
    svg_with(elements, settings, &[])
}

/// [`svg`], with the elements `clones` names drawn as clones of others; or why they can't be, as for `svg`,
/// or a clone of an element that isn't there.
pub fn svg_with(elements: &[Element], settings: &DesignSettings, clones: &[Cloned]) -> Result<String, String> {
    let Some(bounds) = Rect::around(elements.iter().flat_map(|element| element.shape.path().points())) else {
        return Err("no element has a point".to_string());
    };
    let (width, height) = ((bounds.max().x() + MARGIN).max(MARGIN), (bounds.max().y() + MARGIN).max(MARGIN));
    let mut out = String::from("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
    out.push_str("<svg xmlns=\"http://www.w3.org/2000/svg\" xmlns:xlink=\"http://www.w3.org/1999/xlink\" ");
    out.push_str("xmlns:inkscape=\"http://www.inkscape.org/namespaces/inkscape\" ");
    out.push_str(&format!("xmlns:inkstitch=\"{INKSTITCH_NS}\" "));
    out.push_str(&format!("width=\"{width}mm\" height=\"{height}mm\" viewBox=\"0 0 {width} {height}\">\n"));
    out.push_str("  <metadata>\n");
    out.push_str(&format!("    <inkstitch:inkstitch_svg_version>{CURRENT}</inkstitch:inkstitch_svg_version>\n"));
    let defaults = DesignSettings::default();
    for setting in SETTINGS.iter().filter(|setting| setting.get(settings) != setting.get(&defaults)) {
        out.push_str(&format!("    <inkstitch:{0}>{1}</inkstitch:{0}>\n", setting.name, setting.get(settings).get()));
    }
    if settings.rotate_on_export != Turn::NONE {
        out.push_str(&format!("    <inkstitch:{0}>{1}</inkstitch:{0}>\n", Turn::NAME, settings.rotate_on_export.degrees()));
    }
    out.push_str("  </metadata>\n");
    // A symbol for each command the elements' hints and the design's origin and stop position are written as,
    // and the elements' clip paths.
    let globals: Vec<(&str, Point)> =
        [("origin", settings.origin), ("stop_position", settings.stop_position)].into_iter().filter_map(|(name, at)| Some((name, at?))).collect();
    let used: Vec<&str> = COMMANDS
        .iter()
        .filter(|(_, at)| elements.iter().any(|e| at(&e.hints).is_some()))
        .map(|(name, _)| *name)
        .chain(globals.iter().map(|(name, _)| *name))
        .collect();
    let cloned = |element: &Element| clones.iter().find(|clone| clone.element == element.id);
    let clip_paths: String =
        elements.iter().enumerate().filter(|(_, element)| cloned(element).is_none()).map(|(index, element)| clip_paths(index, element)).collect();
    if !used.is_empty() || !clip_paths.is_empty() {
        out.push_str("  <defs>\n");
        for name in &used {
            out.push_str(&format!("    <symbol id=\"inkstitch_{name}\"><circle r=\"1\"/></symbol>\n"));
        }
        out.push_str(&clip_paths);
        out.push_str("  </defs>\n");
    }
    for (index, element) in elements.iter().enumerate() {
        if let Some(clone) = cloned(element) {
            let Some(of) = elements.iter().position(|source| source.id == clone.of) else {
                return Err(format!("{}: a clone of {}, which the design doesn't have", element.id, clone.of));
            };
            let [a, b, c, d, e, f] = clone.map;
            out.push_str(&format!("  <use id=\"e{index}\" xlink:href=\"#e{of}\" transform=\"matrix({a} {b} {c} {d} {e} {f})\"/>\n"));
            continue;
        }
        let parameters = parameters(element, PARAMETERS)?;
        // The first clip on the path, each one after it on a group round it.
        let clip = |k: usize| format!(" clip-path=\"url(#e{index}-clip{k})\"");
        let groups = element.clips.len().saturating_sub(1);
        for k in (1..=groups).rev() {
            out.push_str(&format!("  <g{}>\n", clip(k)));
        }
        let own = if element.clips.is_empty() { String::new() } else { clip(0) };
        out.push_str(&format!("  <path id=\"e{index}\" {}{own}{parameters} d=\"{}\"/>\n", paint(element), data(element.shape.path())));
        out.push_str(&"  </g>\n".repeat(groups));
        out.push_str(&commands(index, element));
    }
    for (name, at) in globals {
        out.push_str(&format!("  <use id=\"{name}\" xlink:href=\"#inkstitch_{name}\" x=\"{}\" y=\"{}\"/>\n", at.x(), at.y()));
    }
    out.push_str("</svg>\n");
    Ok(out)
}

/// Which of an element's hints a command is written for.
type Hint = fn(&Hints) -> Option<Point>;

/// The commands `hints` are written as, each with the hint it is written for.
const COMMANDS: [(&str, Hint); 3] = [("starting_point", |h| h.start), ("ending_point", |h| h.end), ("target_point", |h| h.target)];

/// The commands of the element `index`, `element`: for each of its hints, a group of a connector from the
/// command's place to the element's first point, which ties it to the element, and a `<use>` of the command's
/// symbol there.
fn commands(index: usize, element: &Element) -> String {
    let Some(first) = element.shape.path().points().next() else { return String::new() };
    let mut out = String::new();
    for (name, at) in COMMANDS {
        let Some(at) = at(&element.hints) else { continue };
        let id = format!("e{index}-{name}");
        out.push_str(&format!(
            "  <g>\n    <path d=\"M {},{} L {},{}\" style=\"fill:none;stroke:#000;stroke-width:0.1\" inkscape:connector-type=\"polyline\" \
             inkscape:connection-start=\"#{id}\" inkscape:connection-end=\"#e{index}\"/>\n",
            at.x(),
            at.y(),
            first.x(),
            first.y()
        ));
        out.push_str(&format!("    <use id=\"{id}\" xlink:href=\"#inkstitch_{name}\" x=\"{}\" y=\"{}\"/>\n  </g>\n", at.x(), at.y()));
    }
    out
}

/// The clip paths of the element `index`, `element`: a `<clipPath>` for each of its clips, with a `<path>` for
/// each of the clip's shapes, filled by its rule.
fn clip_paths(index: usize, element: &Element) -> String {
    let mut out = String::new();
    for (k, clip) in element.clips.iter().enumerate() {
        out.push_str(&format!("    <clipPath id=\"e{index}-clip{k}\" clipPathUnits=\"userSpaceOnUse\">\n"));
        for shape in clip.shapes.iter() {
            out.push_str(&format!("      <path clip-rule=\"{}\" d=\"{}\"/>\n", rule(shape.rule), data(&shape.path)));
        }
        out.push_str("    </clipPath>\n");
    }
    out
}

/// `rule` as SVG writes it.
const fn rule(rule: FillRule) -> &'static str {
    match rule {
        FillRule::NonZero => "nonzero",
        FillRule::EvenOdd => "evenodd",
    }
}

/// How `element` is painted, as attributes: its fill, or its stroke.
fn paint(element: &Element) -> String {
    let colour = element.thread.color;
    let colour = format!("#{:02x}{:02x}{:02x}", colour.r, colour.g, colour.b);
    match &element.shape {
        Shape::Fill { rule: fill_rule, .. } => format!("fill=\"{colour}\" fill-rule=\"{}\" stroke=\"none\"", rule(*fill_rule)),
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
            if own(spec) {
                return Err(format!("{}: `{key}` is StitchCraft's own, and Ink/Stitch would sew the element without it", element.id));
            }
        }
        out.push_str(&format!(" inkstitch:{key}=\"{}\"", escaped(value)));
    }
    Ok(out)
}

/// Whether `spec` is a setting of StitchCraft's own that a stitch type reads, which Ink/Stitch passes over.
fn own(spec: &ParamSpec) -> bool {
    spec.origin == Origin::StitchCraft && !spec.applies_to.is_empty()
}

/// The elements of `elements` that Ink/Stitch sews as StitchCraft does, in their order: all but those that
/// set a setting of StitchCraft's own that a stitch type reads, which [`svg`] refuses. The differential
/// writes a sheet's file of these, so that both programs sew the same design.
pub fn sewn_alike(elements: &[Element]) -> Vec<Element> {
    let sets_own = |element: &Element| element.params.iter().any(|(key, _)| stitchcraft_params::find(PARAMETERS, key).is_some_and(own));
    elements.iter().filter(|element| !sets_own(element)).cloned().collect()
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
    use stitchcraft_plan::Rgb;

    use super::*;
    use crate::designs::{BLUE, RED, along, capped, fill, p, polylines, rings, widened};

    /// What a file keeps of an element: its shape, its thread's colour, its parameters, its hints and its clips.
    type Kept = (Shape, Rgb, Vec<(String, String)>, Hints, Vec<Clip>);

    /// What the file `text` keeps of each element, and the design settings StitchCraft reads from it, as
    /// StitchCraft's SVG reader reads them, which says nothing of a file the test kit writes: the shapes,
    /// colours, parameters, hints and clips (`REQ-PRM-003`).
    fn read(text: &str) -> (Vec<Kept>, DesignSettings) {
        let read = stitchcraft_svg::read(text.as_bytes(), &Budget::DEFAULT).unwrap();
        assert!(read.warnings.is_empty(), "{:?}", read.warnings);
        let elements = read.design.elements();
        let others = text.matches("inkscape:connection-start").count() + text.matches("<path clip-rule").count();
        let clones = text.matches(" xlink:href=\"#e").count();
        assert_eq!(elements.len(), text.matches("<path ").count() - others + clones, "an element for each path and each clone");
        (kept(elements), read.design.settings)
    }

    /// What a file should keep of `elements`.
    fn kept(elements: &[Element]) -> Vec<Kept> {
        let parameters = |element: &Element| element.params.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect();
        elements
            .iter()
            .map(|element| (element.shape.clone(), element.thread.color, parameters(element), element.hints, element.clips.clone()))
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
    fn req_prm_003_an_element_s_hints_are_written_as_ink_stitch_s_commands() {
        // A fill that starts and ends at its corners and turns round a point off its middle, and a line with
        // a start alone: each hint a command tied to its element, which the reader places where it was.
        let square = fill("square", &[&[(10.0, 10.0), (20.0, 10.0), (20.0, 20.0), (10.0, 20.0)]], &RED, &[("fill_method", "circular_fill")]);
        let square = Element { hints: Hints { start: Some(p(10.0, 10.0)), end: Some(p(20.5, 20.25)), target: Some(p(12.0, 18.0)) }, ..square };
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
        let square = crate::designs::clipped(
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
            let (digitized, _) = stitchcraft_engine::auto::digitized(&design, &Budget::DEFAULT);
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
        let square = Element { hints: Hints { start: Some(p(10.0, 0.0)), end: Some(p(0.0, 5.0)), target: None }, ..square };
        let map = [0.0, 1.0, -1.0, 0.0, 20.0, -20.0];
        let turned = |x: f64, y: f64| p(-y + 20.0, x - 20.0);
        let ring = vec![&[(20.0, -20.0), (20.0, -10.0), (15.0, -10.0), (15.0, -20.0)][..]];
        let clone = fill("clone", &ring, &RED, &[("angle", "-90"), ("row_spacing_mm", "0.4")]);
        let clone = Element { hints: Hints { start: Some(turned(10.0, 0.0)), end: Some(turned(0.0, 5.0)), target: None }, ..clone };
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
}
