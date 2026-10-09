//! Reading an SVG document into a [`Design`]: which elements draw, where, and in which colour.
//!
//! The reader walks the document once, in order. Document order is SVG's paint order (bottom first) and
//! so the stitching order. Each element gets its transform and inherited [`Style`] from its parent. Every
//! shape that paints becomes one design element per paint, in the element's `paint-order`: its fill (an
//! area) and its stroke (an outline), each sewn in its paint's colour.
//!
//! **Ids.** A design element is `svg:<label>:fill` or `svg:<label>:stroke`. The label is the SVG
//! element's `id`, or `<tag>@<n>` (the n-th `<tag>` in the file) when it has none. A repeated id gets
//! `~2`, `~3`, … so that ids stay unique. The reader's own diagnostics name the SVG element:
//! `svg:<label>`.
//!
//! **Nothing is dropped silently.** `SC-W0802` reports SVG features that StitchCraft does not stitch:
//! text, images, clones, nested `<svg>`, style sheets, clipping, masks, filters, markers, gradients,
//! patterns and Ink/Stitch's settings. `SC-W0804` reports geometry that cannot be used, and `SC-E0801` a
//! file that cannot be read at all. What a viewer would not show either is left out without a word:
//! `display: none`, hidden or fully transparent elements, and the insides of `<defs>`.
//!
//! **Limits keep any file cheap.** A file may be at most [`MAX_BYTES`] long with a million XML nodes,
//! and may not declare entities, whose expansion could need far more memory than the file. Work is
//! charged to the budget: one unit per XML node, per path segment, and per 8 bytes of a transform or
//! point list.

use std::collections::{BTreeMap, BTreeSet};
use std::str::FromStr;

use roxmltree::{Document, Node, NodeId, ParsingOptions};
use stitchcraft_core::{Budget, Code, Diagnostic, ElementId, Fix, Meter};
use stitchcraft_engine::design::{Design, DesignSettings, Element, Shape};
use stitchcraft_params::ParamSet;
use stitchcraft_plan::{Rgb, Thread};
use svgtypes::{AspectRatio, Color, Length, LengthUnit, ViewBox};

use crate::path::{self, Seg, Sub};
use crate::style::{Declared, Paint, Style, rgb};
use crate::transform::{self, Affine, user_units};

/// The largest SVG file StitchCraft reads, in bytes (64 MiB). The largest file in Ink/Stitch's font
/// library is 14.5 MB; a file much larger than that usually carries embedded images, which are not
/// stitched anyway. Memory stays bounded by this and by the XML node limit.
pub const MAX_BYTES: usize = 64 << 20;

/// The most XML nodes a file may have.
const MAX_NODES: u32 = 1_000_000;

/// A transform or point list costs one work unit per this many bytes, about one per number pair.
const BYTES_PER_UNIT: usize = 8;

/// How many `href`s a gradient may follow to find its colour stops.
const MAX_HOPS: u32 = 16;

const SVG_NS: &str = "http://www.w3.org/2000/svg";
const INKSCAPE_NS: &str = "http://www.inkscape.org/namespaces/inkscape";
const XLINK_NS: &str = "http://www.w3.org/1999/xlink";
const INKSTITCH_NS: &str = "http://inkstitch.org/namespace";

/// What an SVG file holds for StitchCraft.
#[derive(Clone, Debug, PartialEq)]
pub struct Svg {
    /// Every shape that paints, in stitching order.
    pub design: Design,
    /// What was left out or simplified, in document order.
    pub warnings: Vec<Diagnostic>,
}

/// The design in the SVG file `bytes`, read within `budget`. Fails with `SC-E0801` when the file cannot
/// be read at all, and with `SC-E0004` when reading it needs more work than the budget allows.
pub fn read(bytes: &[u8], budget: &Budget) -> Result<Svg, Diagnostic> {
    let text = text(bytes)?;
    if text.contains("<!ENTITY") {
        return Err(unreadable("The file declares XML entities, which StitchCraft does not read."));
    }
    // A DTD without entities (`<!DOCTYPE svg PUBLIC …>`, common in older files) adds nothing, so it is
    // allowed; roxmltree never fetches external DTDs.
    let options = ParsingOptions { allow_dtd: true, nodes_limit: MAX_NODES, ..ParsingOptions::default() };
    let doc = Document::parse_with_options(text, options).map_err(|e| match e {
        roxmltree::Error::NodesLimitReached => unreadable(format!("The file has more than {MAX_NODES} XML nodes, more than StitchCraft reads.")),
        e => unreadable(format!("The file is not well-formed XML: {e}.")),
    })?;
    Reader::new(budget).read(&doc)
}

/// The file's text, or why it is not text StitchCraft can read.
fn text(bytes: &[u8]) -> Result<&str, Diagnostic> {
    if bytes.len() > MAX_BYTES {
        let size = bytes.len().div_ceil(1 << 20);
        return Err(unreadable(format!("The file is {size} MB; StitchCraft reads SVG files of up to {} MB.", MAX_BYTES >> 20)));
    }
    if bytes.starts_with(&[0x1f, 0x8b]) {
        return Err(unreadable("The file is compressed SVG (.svgz); StitchCraft reads plain SVG."));
    }
    if bytes.starts_with(&[0xff, 0xfe]) || bytes.starts_with(&[0xfe, 0xff]) {
        return Err(unreadable("The file is UTF-16 text; StitchCraft reads UTF-8."));
    }
    let bytes = bytes.strip_prefix(b"\xef\xbb\xbf").unwrap_or(bytes);
    std::str::from_utf8(bytes).map_err(|e| unreadable(format!("The file is not UTF-8 text: byte {} starts no character.", e.valid_up_to())))
}

/// `SC-E0801` with `message`.
fn unreadable(message: impl Into<String>) -> Diagnostic {
    Diagnostic::new(Code::SvgUnreadable, message).with_fix(Fix::Hint("Open the file in a vector editor and save it again as plain SVG.".to_string()))
}

/// What an element's children are drawn with.
#[derive(Clone, Copy, Debug)]
enum Context<'a> {
    /// Nothing inside is drawn: a leaf, `<defs>`, `display: none`, a branch a `<switch>` did not choose.
    Hidden,
    /// Every child is drawn with this transform (user units to millimetres) and style.
    Shown { map: Affine, style: Style<'a> },
    /// Only the child a `<switch>` chose is drawn.
    Switch { chosen: Option<NodeId>, map: Affine, style: Style<'a> },
}

/// The elements the reader draws, or reports; any other element draws nothing, in a viewer too.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Kind {
    Group,
    Switch,
    Shape(Geometry),
    /// Drawn by a viewer but not stitched: what it is, and how to make it stitchable.
    Unsupported(&'static str, Option<&'static str>),
}

/// The basic shapes and paths.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Geometry {
    Path,
    Rect,
    Circle,
    Ellipse,
    Line,
    Polyline,
    Polygon,
}

impl Kind {
    fn of(tag: &str) -> Option<Kind> {
        Some(match tag {
            "g" | "a" => Kind::Group,
            "switch" => Kind::Switch,
            "path" => Kind::Shape(Geometry::Path),
            "rect" => Kind::Shape(Geometry::Rect),
            "circle" => Kind::Shape(Geometry::Circle),
            "ellipse" => Kind::Shape(Geometry::Ellipse),
            "line" => Kind::Shape(Geometry::Line),
            "polyline" => Kind::Shape(Geometry::Polyline),
            "polygon" => Kind::Shape(Geometry::Polygon),
            "text" | "flowRoot" => {
                Kind::Unsupported("is text, which is not stitched", Some("Convert the text to paths first (in Inkscape: Path › Object to Path)."))
            }
            "image" => {
                Kind::Unsupported("is a raster image, which is not stitched", Some("Trace it into paths first (in Inkscape: Path › Trace Bitmap)."))
            }
            "use" => Kind::Unsupported(
                "is a clone (`<use>`), which is not stitched yet",
                Some("Unlink the clone first (in Inkscape: Edit › Clone › Unlink Clone)."),
            ),
            "svg" => Kind::Unsupported("is a nested `<svg>` element, which is not read yet", None),
            "foreignObject" => Kind::Unsupported("holds content that is not SVG (`<foreignObject>`), which is not stitched", None),
            _ => return None,
        })
    }
}

/// Which way a length attribute runs, for percentages (SVG 1.1 § 7.10).
#[derive(Clone, Copy)]
enum Axis {
    X,
    Y,
    /// Radii: percentages of the viewport's diagonal over √2.
    Other,
}

struct Reader<'b, 'a, 'input> {
    budget: &'b Budget,
    meter: Meter,
    /// The root element, once reading has started.
    root: Option<Node<'a, 'input>>,
    /// The root element's namespace: SVG's, or none in a file that declares none.
    ns: Option<&'a str>,
    /// The root viewport in user units: what percentages are of.
    viewport: (f64, f64),
    elements: Vec<Element>,
    warnings: Vec<Diagnostic>,
    /// Labels given out, and the next suffix to try for each repeated one.
    taken: BTreeSet<String>,
    suffixes: BTreeMap<String, u32>,
    /// Elements by id, the first of each, for paint servers; built when first needed.
    ids: Option<BTreeMap<&'a str, Node<'a, 'input>>>,
    /// Whether the file-wide notes (a style sheet, Ink/Stitch settings) have been given.
    noted_style_sheet: bool,
    noted_inkstitch: bool,
}

impl<'b, 'a, 'input> Reader<'b, 'a, 'input> {
    fn new(budget: &'b Budget) -> Self {
        Reader {
            budget,
            meter: budget.meter(),
            root: None,
            ns: None,
            viewport: (0.0, 0.0),
            elements: Vec::new(),
            warnings: Vec::new(),
            taken: BTreeSet::new(),
            suffixes: BTreeMap::new(),
            ids: None,
            noted_style_sheet: false,
            noted_inkstitch: false,
        }
    }

    fn read(mut self, doc: &'a Document<'input>) -> Result<Svg, Diagnostic> {
        let root = doc.root_element();
        let (tag, ns) = (root.tag_name().name(), root.tag_name().namespace());
        if tag != "svg" {
            return Err(unreadable(format!("The file's root element is <{tag}>, not <svg>.")));
        }
        if let Some(other) = ns.filter(|ns| *ns != SVG_NS) {
            return Err(unreadable(format!("The file's <svg> element is in the namespace {other}, not SVG's.")));
        }
        (self.root, self.ns) = (Some(root), ns);
        let map = self.root_map(root)?;
        if root.has_attribute("transform") {
            self.note(None, "The root `<svg>` element's transform is not applied.".to_string(), None);
        }
        self.notice(root);
        let root_context = match Style::default().child(&Declared::of(root)) {
            Some(style) => Context::Shown { map, style },
            None => Context::Hidden,
        };
        // Document order, with the ancestors' contexts on a stack: a node's parent is the nearest entry
        // still on it once the finished subtrees above it are popped.
        let mut stack = vec![(root.id(), root_context)];
        let mut counts: BTreeMap<&str, u32> = BTreeMap::from([("svg", 1)]);
        for node in root.descendants().skip(1) {
            self.charge(1)?;
            if !node.is_element() {
                continue;
            }
            let parent = node.parent().map(|p| p.id());
            while stack.last().is_some_and(|(id, _)| Some(*id) != parent) {
                stack.pop();
            }
            let Some(&(_, context)) = stack.last() else { continue };
            self.notice(node);
            let tag = node.tag_name().name();
            let n = if self.is_svg(node) {
                let count = counts.entry(tag).or_insert(0);
                *count += 1;
                *count
            } else {
                0
            };
            let own = match context {
                Context::Hidden => Context::Hidden,
                Context::Switch { chosen, .. } if chosen != Some(node.id()) => Context::Hidden,
                Context::Shown { map, style } | Context::Switch { map, style, .. } => self.visit(node, n, map, &style)?,
            };
            if node.has_children() {
                stack.push((node.id(), own));
            }
        }
        let design = Design::new(self.elements, DesignSettings::default())?;
        Ok(Svg { design, warnings: self.warnings })
    }

    /// The map from the root's user units to millimetres, from its size, viewBox and aspect ratio.
    fn root_map(&mut self, root: Node<'a, 'input>) -> Result<Affine, Diagnostic> {
        let length = |name: &str| match root.attribute(name).map(str::trim) {
            None | Some("auto") => Ok(None),
            Some(v) => Length::from_str(v).map(Some).map_err(|_| unreadable(format!("The root element's {name}, \"{v}\", is not a length."))),
        };
        let (width, height) = (length("width")?, length("height")?);
        let view_box = match root.attribute("viewBox") {
            None => None,
            Some(v) => Some(ViewBox::from_str(v).map_err(|_| unreadable(format!("The root element's viewBox, \"{v}\", is not four numbers.")))?),
        };
        let aspect = match root.attribute("preserveAspectRatio") {
            None => AspectRatio::default(),
            Some(v) => AspectRatio::from_str(v).map_err(|_| unreadable(format!("The root element's preserveAspectRatio, \"{v}\", is not one.")))?,
        };
        // Without a viewBox or an absolute size, percentages are of a viewer's default size, 300 × 150.
        let pixels = |length: Option<Length>, default: f64| match length {
            Some(l) if l.unit != LengthUnit::Percent => user_units(l, 0.0),
            _ => default,
        };
        self.viewport = view_box.map_or_else(|| (pixels(width, 300.0), pixels(height, 150.0)), |vb| (vb.w, vb.h));
        transform::root_to_mm(width, height, view_box, aspect).map_err(|reason| unreadable(format!("The drawing's size cannot be used: {reason}.")))
    }

    /// File-wide notes: a style sheet, and Ink/Stitch settings, wherever they are.
    fn notice(&mut self, node: Node<'a, 'input>) {
        if !self.noted_style_sheet
            && self.is_svg(node)
            && node.tag_name().name() == "style"
            && node.children().any(|c| c.text().is_some_and(|t| !t.trim().is_empty()))
        {
            self.noted_style_sheet = true;
            let hint = "Save the file with inline styles or presentation attributes instead of a style sheet.";
            self.note(
                None,
                "The file has a style sheet (`<style>`), which StitchCraft does not read: colours it sets are not used.".to_string(),
                Some(hint),
            );
        }
        if !self.noted_inkstitch && node.attributes().any(|a| a.namespace() == Some(INKSTITCH_NS)) {
            self.noted_inkstitch = true;
            self.note(
                None,
                "The file has Ink/Stitch embroidery settings, which this version does not read yet: every element uses the default settings."
                    .to_string(),
                None,
            );
        }
    }

    /// Draws or reports one element whose parent is drawn; returns what its children are drawn with.
    fn visit(&mut self, node: Node<'a, 'input>, n: u32, map: Affine, inherited: &Style<'a>) -> Result<Context<'a>, Diagnostic> {
        let tag = node.tag_name().name();
        let Some(kind) = Kind::of(tag).filter(|_| self.is_svg(node)) else { return Ok(Context::Hidden) };
        let declared = Declared::of(node);
        let Some(style) = inherited.child(&declared) else { return Ok(Context::Hidden) };
        let label = label(node, tag, n);
        let map = match node.attribute("transform") {
            None => map,
            Some(text) => {
                self.charge_text(text)?;
                match transform::parse(text) {
                    Ok(own) => map.after(own),
                    Err(reason) => {
                        self.unusable(
                            &label,
                            format!("The transform of `{label}` cannot be used ({reason}), so it is left out with everything inside it."),
                        );
                        return Ok(Context::Hidden);
                    }
                }
            }
        };
        let effects = |reader: &mut Self, label: &str| {
            for (property, what) in [
                ("clip-path", "is clipped (`clip-path`); clipping is not applied, so all of it is stitched"),
                ("mask", "has a mask; masks are not applied, so all of it is stitched"),
                ("filter", "has filter effects (blurs, shadows and the like), which are not stitched"),
            ] {
                if declared.uses(property) {
                    reader.note(Some(label), format!("`{label}` {what}."), None);
                }
            }
        };
        match kind {
            Kind::Group => {
                effects(self, &label);
                Ok(Context::Shown { map, style })
            }
            Kind::Switch => {
                effects(self, &label);
                Ok(Context::Switch { chosen: self.chosen(node).map(|c| c.id()), map, style })
            }
            Kind::Shape(geometry) => {
                if style.shown() {
                    effects(self, &label);
                    self.shape(node, geometry, label, n, map, &style)?;
                }
                Ok(Context::Hidden)
            }
            Kind::Unsupported(what, hint) => {
                if style.shown() {
                    self.note(Some(&label), format!("`{label}` {what}; it is left out."), hint);
                }
                Ok(Context::Hidden)
            }
        }
    }

    /// The child a `<switch>` draws: the first SVG element whose conditions hold. StitchCraft implements
    /// no extensions, so `requiredExtensions` fails; it has no user language, so `systemLanguage` passes,
    /// and `requiredFeatures`, dropped in SVG 2, passes as in today's viewers.
    fn chosen(&self, node: Node<'a, 'input>) -> Option<Node<'a, 'input>> {
        node.children().find(|c| self.is_svg(*c) && !c.has_attribute("requiredExtensions"))
    }

    /// The design elements of a shape that is shown: one per paint it paints with.
    fn shape(&mut self, node: Node<'a, 'input>, geometry: Geometry, label: String, n: u32, map: Affine, style: &Style<'a>) -> Result<(), Diagnostic> {
        let label = self.claim(label, node.tag_name().name(), n)?;
        // A line has no inside to fill.
        let fill = if geometry == Geometry::Line || style.fill_clear { None } else { self.colour(&label, "fill", style.fill, style.color)? };
        let stroke = if style.stroke_clear { None } else { self.colour(&label, "stroke", style.stroke, style.color)? };
        if fill.is_none() && stroke.is_none() {
            return Ok(());
        }
        if style.markers && matches!(geometry, Geometry::Path | Geometry::Line | Geometry::Polyline | Geometry::Polygon) {
            self.note(Some(&label), format!("`{label}` has markers (arrowheads and the like), which are not stitched."), None);
        }
        let Some(subs) = self.outline(node, geometry, &label)? else { return Ok(()) };
        let path = match path::to_mm(&subs, map) {
            Ok(path) => path,
            Err(reason) => {
                self.unusable(&label, format!("`{label}` cannot be stitched: {reason}. It is left out."));
                return Ok(());
            }
        };
        let mut points = path.points();
        let first = points.next();
        if points.all(|p| Some(p) == first) {
            self.unusable(&label, format!("`{label}` draws nothing: all of its points are in the same place. It is left out."));
            return Ok(());
        }
        let name = node.attribute((INKSCAPE_NS, "label")).map(str::trim).filter(|s| !s.is_empty()).map(str::to_string);
        let paints = if style.stroke_first { [("stroke", stroke), ("fill", fill)] } else { [("fill", fill), ("stroke", stroke)] };
        for (part, colour) in paints {
            let Some(colour) = colour else { continue };
            let id = ElementId::new(format!("svg:{label}:{part}"))
                .map_err(|e| Diagnostic::new(Code::InternalCheckFailed, format!("The SVG reader made an element id that cannot be used: {e}.")))?;
            let shape = if part == "fill" { Shape::Fill { path: path.clone(), rule: style.fill_rule } } else { Shape::Stroke(path.clone()) };
            self.elements.push(Element { id, name: name.clone(), shape, thread: Thread::new(colour), params: ParamSet::new() });
        }
        Ok(())
    }

    /// The outline of a shape element in user units; `None`, with a warning, when it draws nothing.
    fn outline(&mut self, node: Node<'a, 'input>, geometry: Geometry, label: &str) -> Result<Option<Vec<Sub>>, Diagnostic> {
        let length = |name, axis| self.length(node, name, axis);
        let mut error = None;
        let subs = match geometry {
            Geometry::Path => {
                let outline = path::path_data(node.attribute("d").unwrap_or_default(), &mut self.meter).map_err(|_| self.exhausted())?;
                error = outline.error;
                outline.subs
            }
            Geometry::Rect => {
                let (x, y) = (length("x", Axis::X).unwrap_or(0.0), length("y", Axis::Y).unwrap_or(0.0));
                let (width, height) = (length("width", Axis::X).unwrap_or(0.0), length("height", Axis::Y).unwrap_or(0.0));
                // A radius not given (or negative, or `auto`) is the other one; neither means square corners.
                let (rx, ry) = (length("rx", Axis::X).filter(|r| *r >= 0.0), length("ry", Axis::Y).filter(|r| *r >= 0.0));
                let (rx, ry) = (rx.or(ry).unwrap_or(0.0), ry.or(rx).unwrap_or(0.0));
                if width > 0.0 && height > 0.0 { path::rect(x, y, width, height, rx, ry) } else { Vec::new() }
            }
            Geometry::Circle => {
                let r = length("r", Axis::Other).unwrap_or(0.0);
                let (cx, cy) = (length("cx", Axis::X).unwrap_or(0.0), length("cy", Axis::Y).unwrap_or(0.0));
                if r > 0.0 { path::ellipse(cx, cy, r, r) } else { Vec::new() }
            }
            Geometry::Ellipse => {
                let (rx, ry) = (length("rx", Axis::X).filter(|r| *r >= 0.0), length("ry", Axis::Y).filter(|r| *r >= 0.0));
                let (rx, ry) = (rx.or(ry).unwrap_or(0.0), ry.or(rx).unwrap_or(0.0));
                let (cx, cy) = (length("cx", Axis::X).unwrap_or(0.0), length("cy", Axis::Y).unwrap_or(0.0));
                if rx > 0.0 && ry > 0.0 { path::ellipse(cx, cy, rx, ry) } else { Vec::new() }
            }
            Geometry::Line => {
                let start = (length("x1", Axis::X).unwrap_or(0.0), length("y1", Axis::Y).unwrap_or(0.0));
                let end = (length("x2", Axis::X).unwrap_or(0.0), length("y2", Axis::Y).unwrap_or(0.0));
                vec![Sub { start, segs: vec![Seg::Line(end)], closed: false }]
            }
            Geometry::Polyline | Geometry::Polygon => {
                let points = node.attribute("points").unwrap_or_default();
                self.charge_text(points)?;
                path::polyline(points, geometry == Geometry::Polygon)
            }
        };
        match (error, subs.is_empty()) {
            (Some(error), true) => {
                self.unusable(label, format!("The path data of `{label}` has an error before it draws anything ({error}). It is left out."));
            }
            (Some(error), false) => {
                self.unusable(label, format!("The path data of `{label}` has an error ({error}); it is stitched up to the error."));
            }
            (None, true) => self.unusable(label, format!("`{label}` draws nothing: its size or its data is empty. It is left out.")),
            (None, false) => {}
        }
        Ok((!subs.is_empty()).then_some(subs))
    }

    /// The length attribute `name` in user units; `None` when it is missing or not a length.
    fn length(&self, node: Node<'a, 'input>, name: &str, axis: Axis) -> Option<f64> {
        let length = Length::from_str(node.attribute(name)?).ok()?;
        let (w, h) = self.viewport;
        let reference = match axis {
            Axis::X => w,
            Axis::Y => h,
            Axis::Other => ((w * w + h * h) / 2.0).sqrt(),
        };
        Some(user_units(length, reference))
    }

    /// The colour `label`'s `part` is sewn in, if it paints. A paint server is replaced by a colour, with
    /// a warning: a gradient by its first colour, a pattern by the fallback colour written after it.
    fn colour(&mut self, label: &str, part: &str, paint: Paint<'a>, current: Rgb) -> Result<Option<Rgb>, Diagnostic> {
        let (id, fallback) = match paint {
            Paint::Plain(plain) => return Ok(plain.colour(current)),
            Paint::Server { id, fallback } => (id, fallback.and_then(|f| f.colour(current))),
        };
        // A reference to nothing paints the fallback, or nothing, as in a viewer.
        let Some(server) = self.by_id(id)? else { return Ok(fallback) };
        let kind = server.tag_name().name();
        if kind == "linearGradient" || kind == "radialGradient" {
            let colour = self.first_stop(server)?;
            if let Some(colour) = colour {
                self.note(
                    Some(label),
                    format!("The {part} of `{label}` is a gradient; it is stitched in the gradient's first colour, {colour}."),
                    None,
                );
            }
            // A gradient without stops paints nothing, in a viewer too.
            return Ok(colour);
        }
        let message = match fallback {
            Some(colour) => {
                format!("The {part} of `{label}` is a `<{kind}>`, which is not stitched; it is stitched in the fallback colour, {colour}.")
            }
            None => format!("The {part} of `{label}` is a `<{kind}>`, which is not stitched; the {part} is left out."),
        };
        self.note(Some(label), message, Some("Give the shape a plain colour."));
        Ok(fallback)
    }

    /// The colour of a gradient's first stop, following `href`s to the gradient that has the stops.
    fn first_stop(&mut self, gradient: Node<'a, 'input>) -> Result<Option<Rgb>, Diagnostic> {
        let mut current = gradient;
        for _ in 0..MAX_HOPS {
            for child in current.children() {
                self.charge(1)?;
                if self.is_svg(child) && child.tag_name().name() == "stop" {
                    return self.stop_colour(child).map(Some);
                }
            }
            let href = current.attribute((XLINK_NS, "href")).or_else(|| current.attribute("href"));
            let Some(next) = href.and_then(|h| h.trim().strip_prefix('#')) else { return Ok(None) };
            match self.by_id(next)? {
                Some(next) if matches!(next.tag_name().name(), "linearGradient" | "radialGradient") => current = next,
                _ => return Ok(None),
            }
        }
        Ok(None)
    }

    /// A stop's `stop-color`: black when it says nothing usable, its `color` for `currentColor`.
    fn stop_colour(&mut self, stop: Node<'a, 'input>) -> Result<Rgb, Diagnostic> {
        let black = Rgb::new(0, 0, 0);
        let value = Declared::of(stop).get("stop-color").unwrap_or("black");
        if !value.eq_ignore_ascii_case("currentColor") {
            return Ok(Color::from_str(value).map_or(black, rgb));
        }
        // `color` inherits: the nearest element from the stop up that sets it.
        for node in stop.ancestors() {
            self.charge(1)?;
            if let Some(color) = Declared::of(node).get("color").and_then(|v| Color::from_str(v).ok()) {
                return Ok(rgb(color));
            }
        }
        Ok(black)
    }

    /// The first element with `id`, as `getElementById` finds it.
    fn by_id(&mut self, id: &str) -> Result<Option<Node<'a, 'input>>, Diagnostic> {
        if self.ids.is_none() {
            let mut ids = BTreeMap::new();
            if let Some(root) = self.root {
                for node in root.descendants() {
                    self.charge(1)?;
                    if let Some(id) = node.attribute("id") {
                        ids.entry(id).or_insert(node);
                    }
                }
            }
            self.ids = Some(ids);
        }
        Ok(self.ids.as_ref().and_then(|ids| ids.get(id).copied()))
    }

    /// A unique label for a shape's design elements: `label`, or `label~2`, `label~3`, … when it is taken.
    fn claim(&mut self, label: String, tag: &str, n: u32) -> Result<String, Diagnostic> {
        let unique = self.unique(label)?;
        // The suffix may make a long id too long; the element's place in the file is always short.
        if ElementId::new(format!("svg:{unique}:stroke")).is_ok() { Ok(unique) } else { self.unique(format!("{tag}@{n}")) }
    }

    fn unique(&mut self, label: String) -> Result<String, Diagnostic> {
        if self.taken.insert(label.clone()) {
            return Ok(label);
        }
        let mut k = self.suffixes.get(&label).copied().unwrap_or(1);
        loop {
            self.charge(1)?;
            k += 1;
            let candidate = format!("{label}~{k}");
            if self.taken.insert(candidate.clone()) {
                self.suffixes.insert(label, k);
                return Ok(candidate);
            }
        }
    }

    fn is_svg(&self, node: Node<'a, 'input>) -> bool {
        node.is_element() && node.tag_name().namespace() == self.ns
    }

    /// `SC-W0802` (a feature that is not stitched), about `label` when it concerns one element.
    fn note(&mut self, label: Option<&str>, message: String, hint: Option<&str>) {
        self.warn(Code::SvgFeatureIgnored, label, message, hint);
    }

    /// `SC-W0804` (geometry that cannot be used) about `label`.
    fn unusable(&mut self, label: &str, message: String) {
        self.warn(Code::SvgGeometryUnusable, Some(label), message, None);
    }

    fn warn(&mut self, code: Code, label: Option<&str>, message: String, hint: Option<&str>) {
        let mut warning = Diagnostic::new(code, message);
        if let Some(id) = label.and_then(|l| ElementId::new(format!("svg:{l}")).ok()) {
            warning = warning.with_element(id);
        }
        if let Some(hint) = hint {
            warning = warning.with_fix(Fix::Hint(hint.to_string()));
        }
        self.warnings.push(warning);
    }

    fn charge(&mut self, units: u64) -> Result<(), Diagnostic> {
        self.meter.charge(units).map_err(|_| self.exhausted())
    }

    /// Charges for reading a transform or point list.
    fn charge_text(&mut self, text: &str) -> Result<(), Diagnostic> {
        self.charge(1 + (text.len() / BYTES_PER_UNIT) as u64)
    }

    /// `SC-E0004` for a file that needs more work than the budget allows.
    fn exhausted(&self) -> Diagnostic {
        Diagnostic::new(Code::BudgetExhausted, format!("Reading the SVG file needed more than the work budget of {} units.", self.budget.max_work))
            .with_fix(Fix::Hint("Simplify the drawing (fewer nodes), or split it into several files.".to_string()))
    }
}

/// What the reader calls `node` in ids and messages: its `id`, or `<tag>@<n>` (the n-th `<tag>` in the
/// file) when it has none, or one too long for an element id.
fn label(node: Node<'_, '_>, tag: &str, n: u32) -> String {
    match node.attribute("id") {
        Some(id) if ElementId::new(format!("svg:{id}:stroke")).is_ok() => id.to_string(),
        _ => format!("{tag}@{n}"),
    }
}
