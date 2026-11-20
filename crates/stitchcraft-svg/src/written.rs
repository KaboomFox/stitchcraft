//! A design's elements written as an SVG file with Ink/Stitch's attributes: what `stitch expand -o
//! design.svg` writes (`REQ-CLI-025`), and the files of the Ink/Stitch differential's cases
//! (`stitchcraft_testkit::inkstitch`, `docs/src/design/conformance.md` › L3).
//!
//! **Why.** Automatic digitizing ends in ordinary elements whose settings are Ink/Stitch's parameters:
//! `satin_column`, `pull_compensation_mm`, the underlays, `bean_stitch_repeats`. Written with them, the
//! elements are a design Ink/Stitch sews with the same stitch types, and StitchCraft's SVG reader reads
//! each element's shape, colour and parameters as they were written (`REQ-PRM-003`, `REQ-SVG-009`).
//!
//! **Which settings** ([`Params`]). Automatic digitizing's own are left out, as no stitch type reads them in
//! either program (`REQ-AUTO-020`). An expanded design keeps every other setting, StitchCraft's own among
//! them: `digitized_as` (`REQ-AUTO-143`), `turn_stitches` and `underlays_first`, which Ink/Stitch passes over,
//! so it sews such a design with jumps where StitchCraft runs under later shapes, and without the cap. The
//! differential instead refuses an element with one of StitchCraft's own that a stitch type reads, as
//! Ink/Stitch would sew it otherwise, and leaves out `turn_stitches` (`REQ-FILL-TAT-014`): Ink/Stitch's
//! own shortest stitch, 0.1 mm, keeps a fill's turns anyway.
//!
//! **The file.** Each element is a `<path>`, in the order given, which is the order both programs sew them
//! in. A fill is painted with its thread's colour by its fill rule, and a stroke with its colour, width,
//! join and cap. A satin column drawn as one path keeps its stroke's width, which both sew as a column that
//! wide (`REQ-SAT-016`). Each parameter is an `inkstitch:` attribute of its own name. An element's name is
//! its `inkscape:label`, which the Layers and Objects panel shows.
//!
//! An element's start and end positions and its target point are Ink/Stitch's `starting_point`,
//! `ending_point` and `target_point` commands (`REQ-GEN-001`): a `<use>` of the command's symbol at the
//! position, and a connector from it to the element's path, in a group, as Ink/Stitch's Attach Commands draws
//! them. Ink/Stitch moves the commands of a clone's source with the clone only when they are in a group.
//!
//! An element's clips are clip paths (`REQ-GEN-005`), each a `<clipPath>` of a `<path>` for each of its
//! shapes, by its `clip-rule`, in user space. The first clips the element's path, and each one after it a
//! group round the path, as Inkscape clips a group, which both programs read (`REQ-SVG-011`). Ink/Stitch
//! takes a clip's shapes as one outline and its holes, whatever their rules (`DEV-SVG-016`).
//!
//! A fill painted with a linear gradient is painted with a `<linearGradient>` of its stops, its start and
//! end in user space, which both programs read (`REQ-SVG-014`). The reader takes the fill's thread from the
//! gradient's first stop that shows, so an element whose thread is another colour reads back in that one.
//!
//! An element's guide line and anchor line are Ink/Stitch's helpers (`REQ-RIP-012` to `REQ-RIP-014`): paths
//! whose `style` starts them with its guide-line or anchor-line marker, beside the element's path in the
//! group round it, which both programs look in. The guide keeps its stroke's width and join and its settings,
//! and the anchor keeps its path.
//!
//! An element the design's file draws as a clone of another ([`Cloned`], as TS-16's does) is a `<use>` of
//! that element's path under the clone's transform (`REQ-SVG-012`), in its place.
//!
//! The design's origin and stop position are Ink/Stitch's `origin` and `stop_position` commands
//! (`REQ-ASM-005`): a `<use>` of the command's symbol at the point, standing alone, as Ink/Stitch's Add
//! Commands draws them.
//!
//! The metadata gives Ink/Stitch's current file version ([`crate::version::CURRENT`]), so its updater
//! changes nothing, and each design setting that isn't StitchCraft's default, the turn on export
//! (`rotate_on_export`) among them, in degrees, and what the cap holds the design to when it keeps that
//! (`crate::settings`). One that is at its default stays unset, and Ink/Stitch takes its own default,
//! StitchCraft's too but for the shortest stitch, which StitchCraft leaves to the machine and Ink/Stitch
//! takes as 0.1 mm (`DEV-FIN-001`).
//!
//! Numbers are written as Rust writes a float, the shortest decimal that reads back as the same number, so
//! the reader reads every point and setting as it was. Lengths are in millimetres, one to the user unit,
//! and the view box starts at the origin, so the file keeps the design's coordinates. The page reaches
//! [`MARGIN`] past the design's right and bottom. What lies left of the origin or above it is off the page,
//! which a viewer leaves out, but both programs sew it.

use stitchcraft_core::{Point, Rect};
use stitchcraft_engine::auto::Held;
use stitchcraft_engine::design::{Cap, DesignSettings, Element, FillRule, Hints, Join, Path, SETTINGS, Segment, Shape, Turn};
use stitchcraft_engine::generators::tatami::TURN_STITCHES;
use stitchcraft_engine::registry::PARAMETERS;
use stitchcraft_engine::testsheets::Cloned;
use stitchcraft_params::{Origin, ParamGroup, ParamSpec};
use stitchcraft_plan::Rgb;

use crate::inkstitch::INKSTITCH_NS;
use crate::version::CURRENT;

/// How far the page reaches past the elements' paths, right and down, in millimetres: room for the widest
/// stroke.
pub const MARGIN: f64 = 10.0;

/// Which of the elements' settings the file keeps (see the module's docs).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Params {
    /// Every setting but automatic digitizing's own: an expanded design's.
    All,
    /// Those Ink/Stitch sews as StitchCraft does, for the differential: an element with one of StitchCraft's
    /// own that a stitch type reads is refused, and `turn_stitches` is left out.
    InkStitch,
}

/// How [`svg`] writes a file.
#[derive(Clone, Copy, Debug)]
pub struct Options<'c> {
    /// Which settings it keeps.
    pub params: Params,
    /// The elements it draws as clones of others.
    pub clones: &'c [Cloned],
    /// Whether each element is painted by a `style` attribute, as Inkscape paints a path, rather than by
    /// attributes of their own. Ink/Stitch's Auto-Route Running Stitch gives the elements it makes the style of
    /// the stroke each comes from, and nothing else of its paint.
    pub styled: bool,
}

impl Options<'_> {
    /// An expanded design's file: every setting but automatic digitizing's, no clones, painted by attributes.
    pub const EXPANDED: Options<'static> = Options { params: Params::All, clones: &[], styled: false };
}

/// `elements`, in order, as an SVG file that Ink/Stitch reads, with the design's `settings`, written as
/// `options` say (see the module's docs); or why they can't be: no element has a point, an element is a
/// clone of one that isn't there, or, for [`Params::InkStitch`], an element has a setting of StitchCraft's
/// own that a stitch type reads.
pub fn svg(elements: &[Element], settings: &DesignSettings, options: &Options) -> Result<String, String> {
    let Options { params, clones, styled } = *options;
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
    for (key, value) in settings.cap.iter().flat_map(Held::values) {
        out.push_str(&format!("    <inkstitch:{key}>{value}</inkstitch:{key}>\n"));
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
    let drawn = || elements.iter().enumerate().filter(|(_, element)| cloned(element).is_none());
    let clip_paths: String = drawn().map(|(index, element)| clip_paths(index, element)).collect();
    let gradients: String = drawn().map(|(index, element)| gradient(index, element)).collect();
    if !used.is_empty() || !clip_paths.is_empty() || !gradients.is_empty() {
        out.push_str("  <defs>\n");
        for name in &used {
            out.push_str(&format!("    <symbol id=\"inkstitch_{name}\"><circle r=\"1\"/></symbol>\n"));
        }
        out.push_str(&clip_paths);
        out.push_str(&gradients);
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
        let parameters = parameters(element, PARAMETERS, params)?;
        // The first clip on the path, each one after it on a group round it.
        let clip = |k: usize| format!(" clip-path=\"url(#e{index}-clip{k})\"");
        let groups = element.clips.len().saturating_sub(1);
        for k in (1..=groups).rev() {
            out.push_str(&format!("  <g{}>\n", clip(k)));
        }
        // A guide and an anchor lie beside the path, in the group round it: one of their own, or the innermost
        // clip's.
        let helpers = helpers(index, element);
        let wrapped = groups == 0 && !helpers.is_empty();
        if wrapped {
            out.push_str("  <g>\n");
        }
        let own = if element.clips.is_empty() { String::new() } else { clip(0) };
        let painted = painted(index, element, styled);
        let label = element.name.as_deref().map_or_else(String::new, |name| format!(" inkscape:label=\"{}\"", escaped(name)));
        out.push_str(&format!("  <path id=\"e{index}\"{label} {painted}{own}{parameters} d=\"{}\"/>\n", data(element.shape.path())));
        out.push_str(&helpers);
        out.push_str(&"  </g>\n".repeat(groups + usize::from(wrapped)));
        out.push_str(&commands(index, element));
    }
    for (name, at) in globals {
        out.push_str(&format!("  <use id=\"{name}\" xlink:href=\"#inkstitch_{name}\" x=\"{}\" y=\"{}\"/>\n", at.x(), at.y()));
    }
    out.push_str("</svg>\n");
    Ok(out)
}

/// The guide line and the anchor line of the element `index`, `element`, as Ink/Stitch draws its helpers: a
/// path each, its `style` starting it with Ink/Stitch's marker, with a stroke. The guide keeps its stroke's
/// width and join, and its settings as `inkstitch:` attributes.
fn helpers(index: usize, element: &Element) -> String {
    let mut out = String::new();
    if let Some(guide) = &element.hints.guide {
        let mut style = vec!["fill:none".to_string(), "stroke:#000000".to_string(), format!("stroke-width:{}", guide.width.get())];
        style.extend(match guide.join {
            Join::Miter { limit } => vec!["stroke-linejoin:miter".to_string(), format!("stroke-miterlimit:{limit}")],
            Join::Round => vec!["stroke-linejoin:round".to_string()],
            Join::Bevel => vec!["stroke-linejoin:bevel".to_string()],
        });
        style.push("marker-start:url(#inkstitch-guide-line-marker)".to_string());
        let settings: String = guide.params.iter().map(|(key, value)| format!(" inkstitch:{key}=\"{}\"", escaped(value))).collect();
        out.push_str(&format!("  <path id=\"e{index}-guide\" style=\"{}\"{settings} d=\"{}\"/>\n", style.join(";"), data(&guide.path)));
    }
    if let Some(anchor) = &element.hints.anchor {
        out.push_str(&format!(
            "  <path id=\"e{index}-anchor\" style=\"fill:none;stroke:#000000;stroke-width:0.1;marker-start:url(#inkstitch-anchor-line-marker)\" d=\"{}\"/>\n",
            data(anchor)
        ));
    }
    out
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

/// The linear gradient the element `index`, `element`, is painted with, as a `<linearGradient>` in user space
/// with a `<stop>` for each of its stops; nothing for an element without one.
fn gradient(index: usize, element: &Element) -> String {
    let Shape::Fill { gradient: Some(gradient), .. } = &element.shape else { return String::new() };
    let (start, end) = (gradient.start, gradient.end);
    let mut out = format!(
        "    <linearGradient id=\"e{index}-gradient\" gradientUnits=\"userSpaceOnUse\" x1=\"{}\" y1=\"{}\" x2=\"{}\" y2=\"{}\">\n",
        start.x(),
        start.y(),
        end.x(),
        end.y()
    );
    for stop in &gradient.stops {
        out.push_str(&format!("      <stop offset=\"{}\" stop-color=\"{}\" stop-opacity=\"{}\"/>\n", stop.offset, hex(stop.colour), stop.opacity));
    }
    out.push_str("    </linearGradient>\n");
    out
}

/// `colour` as SVG writes it: `#` and 6 hexadecimal digits.
fn hex(colour: Rgb) -> String {
    format!("#{:02x}{:02x}{:02x}", colour.r, colour.g, colour.b)
}

/// `rule` as SVG writes it.
const fn rule(rule: FillRule) -> &'static str {
    match rule {
        FillRule::NonZero => "nonzero",
        FillRule::EvenOdd => "evenodd",
    }
}

/// How the element `index`, `element`, is painted, as SVG's properties with their values: its fill, its
/// linear gradient's when it has one, or its stroke.
fn paint(index: usize, element: &Element) -> Vec<(&'static str, String)> {
    let colour = hex(element.thread.color);
    match &element.shape {
        Shape::Fill { rule: fill_rule, gradient, .. } => {
            let fill = if gradient.is_some() { format!("url(#e{index}-gradient)") } else { colour };
            vec![("fill", fill), ("fill-rule", rule(*fill_rule).to_string()), ("stroke", "none".to_string())]
        }
        Shape::Stroke { width, join, cap, .. } => {
            let mut properties = vec![("fill", "none".to_string()), ("stroke", colour), ("stroke-width", width.get().to_string())];
            match join {
                Join::Miter { limit } => properties.extend([("stroke-linejoin", "miter".to_string()), ("stroke-miterlimit", limit.to_string())]),
                Join::Round => properties.push(("stroke-linejoin", "round".to_string())),
                Join::Bevel => properties.push(("stroke-linejoin", "bevel".to_string())),
            }
            let cap = match cap {
                Cap::Butt => "butt",
                Cap::Round => "round",
                Cap::Square => "square",
            };
            properties.push(("stroke-linecap", cap.to_string()));
            properties
        }
    }
}

/// How the element `index`, `element`, is painted ([`paint`]), as attributes, or with `styled` as one
/// `style` attribute.
fn painted(index: usize, element: &Element, styled: bool) -> String {
    let properties = paint(index, element);
    if styled {
        let style: Vec<String> = properties.iter().map(|(name, value)| format!("{name}:{value}")).collect();
        format!("style=\"{}\"", style.join(";"))
    } else {
        properties.iter().map(|(name, value)| format!("{name}=\"{value}\"")).collect::<Vec<_>>().join(" ")
    }
}

/// `element`'s parameters as `inkstitch:` attributes, each with a space before it, but automatic
/// digitizing's own, by the parameters of `registry`, and those `params` leaves out; or, for
/// [`Params::InkStitch`], the first of StitchCraft's own that a stitch type reads.
fn parameters(element: &Element, registry: &[&ParamGroup], params: Params) -> Result<String, String> {
    let mut out = String::new();
    for (key, value) in element.params.iter() {
        if let Some(spec) = stitchcraft_params::find(registry, key) {
            if spec.applies_to.is_empty() || (params == Params::InkStitch && key == TURN_STITCHES) {
                continue;
            }
            if params == Params::InkStitch && own(spec) {
                return Err(format!("{}: `{key}` is StitchCraft's own, and Ink/Stitch would sew the element without it", element.id));
            }
        }
        out.push_str(&format!(" inkstitch:{key}=\"{}\"", escaped(value)));
    }
    Ok(out)
}

/// Whether `spec` is a setting of StitchCraft's own that a stitch type reads, which Ink/Stitch passes over
/// and sews otherwise.
fn own(spec: &ParamSpec) -> bool {
    spec.origin == Origin::StitchCraft && !spec.applies_to.is_empty() && spec.key != TURN_STITCHES
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
