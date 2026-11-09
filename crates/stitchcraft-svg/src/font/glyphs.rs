//! The glyphs of a glyph file: their groups, each glyph as a design of its own, its box, and the file's
//! baseline (`docs/src/design/fonts.md` › The reader).
//!
//! **Glyphs.** A glyph file of Ink/Stitch's fonts has a group for each glyph, labelled [`PREFIX`] and the
//! glyph's name. Most are layers of the document, hidden. Ink/Stitch's lettering takes every group whose
//! `inkscape:label` starts so, wherever it lies in the file (`lib/lettering/font_variant.py`, read at
//! `d59c9ab`), and so does [`groups`]: 21,616 of them in the 142 fonts at `c7e3a05`, of which 3,030 lie inside
//! other groups, in 11 fonts, and 13 inside other glyphs.
//!
//! **A glyph as a design** ([`cut`]). The glyph file with 2 kinds of edits, made to its text, so that
//! everything else reads exactly as it was written: its version, its definitions (the symbols of Ink/Stitch's
//! commands, clip paths), and the groups round the glyph with their transforms and styles.
//!
//! - Each drawn element (a shape, an image, text, a clone, a link, a switch or a group) beside the glyph's
//!   group or beside a group round it is left out, with all it holds. Other elements stay, such as `defs`,
//!   `style`, `metadata` and Inkscape's `namedview`.
//! - The glyph's group and the groups round it are shown: a `display` attribute is left out, and so is a
//!   `display` property of a `style` attribute, as lettering clears them. Their other properties, which the
//!   glyph's shapes may inherit, stay.
//!
//! The glyph differential sews each glyph so, in Ink/Stitch and in StitchCraft ([`each_glyph`]), and the
//! font reader reads each glyph's design so.
//!
//! **The box** ([`extent`]) is how Ink/Stitch's lettering measures a glyph (`lib/lettering/glyph.py`, read at
//! `d59c9ab`, and run as a black box on fonts drawn for it). It spans the points that the path data of the
//! `<path>` elements in the glyph's group reach, but connectors': each subpath's curves bounded where they
//! turn, and the place of a move-to that draws nothing, under each path's transforms and the root's. Hidden
//! paths and the paths of a glyph inside the glyph count. Other shapes and clones don't.
//!
//! **The baseline** ([`baseline`]) is the first guide labelled `baseline` among the guides of the file's
//! named view. Inkscape writes a guide's position with y measured up from the bottom of the page, so the
//! baseline lies the page's height in user units less that y below the top, mapped into millimetres as the
//! drawing is. The page's height is the viewBox's, or else the root's height in pixels, and 0 for a root
//! without one in a unit, as Ink/Stitch measures it: 2 files at `c7e3a05` have no height. Without such a
//! guide the baseline lies at the page's top, as Ink/Stitch puts it.

use std::ops::Range;
use std::str::FromStr;

use roxmltree::{Document, Node};
use stitchcraft_core::budget::{Exhausted, Meter};
use stitchcraft_core::{Budget, Diagnostic};
use svgtypes::{Length, LengthUnit};

use crate::BYTES_PER_UNIT;
use crate::document::root::{Page, page};
use crate::inkstitch::INKSCAPE_NS;
use crate::path;
use crate::text;
use crate::transform::{self, Affine};

/// What a glyph group's label starts with.
pub const PREFIX: &str = "GlyphLayer-";
/// SVG's namespace.
pub(crate) const SVG: &str = "http://www.w3.org/2000/svg";
/// Sodipodi's namespace, of Inkscape's named view and guides.
const SODIPODI: &str = "http://sodipodi.sourceforge.net/DTD/sodipodi-0.dtd";
/// The drawn elements of SVG's namespace, which a design leaves out beside the glyph.
const DRAWN: &[&str] =
    &["a", "circle", "ellipse", "foreignObject", "g", "image", "line", "path", "polygon", "polyline", "rect", "svg", "switch", "text", "use"];

/// The groups of glyphs in `doc`, in the order of the file, each with its label after [`PREFIX`].
pub(crate) fn groups<'a, 'input>(doc: &'a Document<'input>) -> impl Iterator<Item = (Node<'a, 'input>, &'a str)> {
    doc.descendants().filter(|node| node.has_tag_name((SVG, "g"))).filter_map(|node| Some((node, label(&node)?)))
}

/// The label after [`PREFIX`] of the glyph whose group `node` is, if it is one.
fn label<'a>(node: &Node<'a, '_>) -> Option<&'a str> {
    node.attribute((INKSCAPE_NS, "label"))?.strip_prefix(PREFIX)
}

/// A glyph of a glyph file, for the glyph differential.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Cut {
    /// Its label after [`PREFIX`], as the file writes it.
    pub label: String,
    /// The glyph file as a design of this glyph alone, when it was asked for.
    pub design: Option<String>,
}

/// Gives `each` the glyphs of the glyph file `bytes`, with their places in the order of the file, each with
/// its design when `wanted` says so of its place, one after another, so that a large file's designs are made
/// as they are taken and never held all at once. The glyph differential sews each glyph so. Fails as the SVG
/// reader does on a file it can't read, and with `SC-E0004` when the work runs past `budget`.
pub fn each_glyph(bytes: &[u8], budget: &Budget, wanted: &dyn Fn(usize) -> bool, each: &mut dyn FnMut(usize, Cut)) -> Result<(), Diagnostic> {
    let text = text::decode(bytes)?;
    let mut meter = budget.meter();
    let doc = crate::document::parse(&text)?;
    let exhausted = |_| super::exhausted(budget, "the glyph file");
    meter.charge(charge(text.len())).map_err(exhausted)?;
    for (index, (group, label)) in groups(&doc).enumerate() {
        meter.charge(1).map_err(exhausted)?;
        let design = if wanted(index) { Some(cut(&text, group, &mut meter).map_err(exhausted)?) } else { None };
        each(index, Cut { label: label.to_string(), design });
    }
    Ok(())
}

/// The work units of reading `bytes` bytes of text.
pub(crate) fn charge(bytes: usize) -> u64 {
    u64::try_from(bytes / BYTES_PER_UNIT).unwrap_or(u64::MAX).saturating_add(1)
}

/// The glyph file `text` as a design of the glyph whose group is `glyph` alone. Each edit costs a unit of
/// `meter`, and each 8 bytes of the design.
pub(crate) fn cut(text: &str, glyph: Node, meter: &mut Meter) -> Result<String, Exhausted> {
    let mut edits: Vec<(Range<usize>, String)> = Vec::new();
    let mut kept = glyph;
    while let Some(parent) = kept.parent_element() {
        edits.extend(shown(text, kept));
        for child in parent.children().filter(|child| *child != kept && drawn(child)) {
            meter.charge(1)?;
            edits.push((child.range(), String::new()));
        }
        kept = parent;
    }
    edits.sort_by_key(|(range, _)| range.start);
    let mut out = String::with_capacity(text.len());
    let mut at = 0;
    for (range, with) in edits {
        out.push_str(text.get(at..range.start).unwrap_or_default());
        out.push_str(&with);
        at = range.end;
    }
    out.push_str(text.get(at..).unwrap_or_default());
    meter.charge(charge(out.len()))?;
    Ok(out)
}

/// Whether `node` is a drawn element of SVG's namespace.
fn drawn(node: &Node) -> bool {
    node.is_element() && node.tag_name().namespace() == Some(SVG) && DRAWN.contains(&node.tag_name().name())
}

/// The edits that show the element `node`: its `display` attribute left out, and the `display` property of
/// its `style` attribute.
fn shown(text: &str, node: Node) -> Vec<(Range<usize>, String)> {
    let mut edits = Vec::new();
    for attribute in node.attributes().filter(|a| a.namespace().is_none()) {
        match attribute.name() {
            "display" => edits.push((attribute.range(), String::new())),
            "style" => {
                let value = attribute.range_value();
                let style = text.get(value.clone()).unwrap_or_default();
                let declarations: Vec<&str> = style
                    .split(';')
                    .filter(|d| !d.split(':').next().is_some_and(|property| property.trim().eq_ignore_ascii_case("display")))
                    .collect();
                if declarations.len() != style.split(';').count() {
                    edits.push((value, declarations.join(";")));
                }
            }
            _ => {}
        }
    }
    edits
}

/// Why a glyph has no box.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum NoBox {
    /// Its group has no `<path>` but connectors.
    NoPath,
    /// A `<path>` in its group reaches no point: its data is missing or empty, or draws nothing usable.
    Empty,
}

/// The least and greatest x of the glyph whose group is `group`, in millimetres, on `page`; or why it has
/// none. Each node looked at, and each segment of path data, costs a unit of `meter`.
pub(crate) fn extent(group: Node, page: &Page, meter: &mut Meter) -> Result<Result<(f64, f64), NoBox>, Exhausted> {
    let mut map = page.map;
    // The transforms of the groups round the glyph, outermost first, but the root's, which the reader
    // doesn't apply.
    let mut round: Vec<Node> = group.ancestors().skip(1).filter(|node| node.is_element() && node.parent_element().is_some()).collect();
    round.reverse();
    for node in round {
        meter.charge(1)?;
        map = map.after(own(node));
    }
    let (mut least, mut most, mut paths) = (f64::INFINITY, f64::NEG_INFINITY, 0_u32);
    let mut stack = vec![(group, map)];
    while let Some((node, outer)) = stack.pop() {
        meter.charge(1)?;
        let map = outer.after(own(node));
        if node.has_tag_name((SVG, "path")) && !node.attribute((INKSCAPE_NS, "connection-end")).is_some_and(|end| !end.is_empty()) {
            paths += 1;
            let outline = path::path_data(node.attribute("d").unwrap_or_default(), meter)?;
            let lone = outline.lone.iter().map(|&(x, y)| map.apply(x, y));
            let xs: Vec<f64> = path::reached(&path::mapped(&outline.subs, map)).into_iter().chain(lone).map(|(x, _)| x).collect();
            if xs.is_empty() || xs.iter().any(|x| !x.is_finite()) {
                return Ok(Err(NoBox::Empty));
            }
            least = xs.iter().fold(least, |least, x| least.min(*x));
            most = xs.iter().fold(most, |most, x| most.max(*x));
        }
        // Children in reverse, so that they come off the stack in the file's order.
        stack.extend(node.children().filter(Node::is_element).collect::<Vec<_>>().into_iter().rev().map(|child| (child, map)));
    }
    Ok(if paths == 0 { Err(NoBox::NoPath) } else { Ok((least, most)) })
}

/// The transform of the element `node` itself, or none when it has none or one that doesn't read. Ink/Stitch's
/// lettering measures a path whose transform doesn't read where it is drawn without it, run as a black box,
/// where the SVG reader leaves the element out of the design (`SC-W0804`).
fn own(node: Node) -> Affine {
    node.attribute("transform").and_then(|text| transform::parse(text).ok()).unwrap_or(Affine::IDENTITY)
}

/// The baseline of the glyph file whose root element is `root`, on `page`, in millimetres from the page's
/// top: `Ok(None)` without a baseline guide, and the guide's position when it isn't 2 numbers.
pub(crate) fn baseline<'a>(root: Node<'a, '_>, page: &Page) -> Result<Option<f64>, &'a str> {
    let view = root.children().find(|node| node.has_tag_name((SODIPODI, "namedview")));
    let guide = view
        .into_iter()
        .flat_map(|view| view.children())
        .find(|node| node.has_tag_name((SODIPODI, "guide")) && node.attribute((INKSCAPE_NS, "label")) == Some("baseline"));
    let Some(guide) = guide else { return Ok(None) };
    let position = guide.attribute("position").unwrap_or_default();
    // Ink/Stitch takes the parts between commas that read as numbers, and a point of 2 of them.
    let numbers: Vec<f64> = position.split(',').filter_map(|part| part.trim().parse::<f64>().ok()).collect();
    let [_, y] = numbers.as_slice() else { return Err(position) };
    let (_, below) = page.map.apply(0.0, height(root, page) - y);
    if below.is_finite() { Ok(Some(below)) } else { Err(position) }
}

/// The page's height in user units, as Ink/Stitch's lettering measures a guide's place on it, run as a black
/// box: the viewBox's height, or else the root's height in pixels, and 0 without one in a unit, as for no
/// height at all or a percentage.
fn height(root: Node, page: &Page) -> f64 {
    if root.attribute("viewBox").is_some_and(|view_box| !view_box.trim().is_empty()) {
        return page.viewport.1;
    }
    let length = root.attribute("height").and_then(|height| Length::from_str(height.trim()).ok());
    length.filter(|length| length.unit != LengthUnit::Percent).map_or(0.0, |length| transform::user_units(length, 0.0))
}

/// The page of the glyph file whose root element is `root`, as the SVG reader maps it.
pub(crate) fn page_of(root: Node) -> Result<Page, Diagnostic> {
    page(root, &mut Vec::new())
}

#[cfg(test)]
mod tests {
    use super::*;

    const FILE: &str = r##"<?xml version="1.0" encoding="UTF-8"?>
<svg xmlns="http://www.w3.org/2000/svg" xmlns:inkscape="http://www.inkscape.org/namespaces/inkscape" xmlns:xlink="http://www.w3.org/1999/xlink" width="20mm" height="20mm" viewBox="0 0 20 20">
  <metadata><inkstitch:inkstitch_svg_version xmlns:inkstitch="http://inkstitch.org/namespace">3</inkstitch:inkstitch_svg_version></metadata>
  <defs><symbol id="inkstitch_trim"><path d="M0 0 L1 1"/></symbol></defs>
  <g inkscape:groupmode="layer" inkscape:label="GlyphLayer-A" style="display:none;stroke:#000000"><path id="a" d="M1 1 L5 5"/><use xlink:href="#inkstitch_trim"/></g>
  <g inkscape:groupmode="layer" inkscape:label="GlyphLayer-B" style="display:inline"><path id="b" d="M2 2 L6 6"/></g>
  <path id="note" d="M0 0 L20 20"/>
  <g inkscape:label="Latin" transform="translate(1 2)" display="none" style="fill:none; display : none">
    <text>a note</text>
    <g inkscape:label="GlyphLayer-ß" style="display:none"><path id="sz" d="M3 3 L4 4"/></g>
    <g inkscape:label="GlyphLayer-ä"><path id="ae" d="M3 3 L5 5"/></g>
  </g>
  <g inkscape:groupmode="layer" inkscape:label="Construction"><path d="M0 1 L1 1"/></g>
</svg>"##;

    /// The glyphs of [`FILE`], each with its design when `wanted` says so.
    fn cuts(text: &str, wanted: &dyn Fn(usize) -> bool) -> Vec<Cut> {
        let mut out = Vec::new();
        each_glyph(text.as_bytes(), &Budget::DEFAULT, wanted, &mut |_, cut| out.push(cut)).unwrap();
        out
    }

    fn design_of(index: usize) -> String {
        cuts(FILE, &|i| i == index).into_iter().find_map(|cut| cut.design).unwrap()
    }

    #[test]
    fn every_group_labelled_as_a_glyph_is_one_wherever_it_lies() {
        let labels: Vec<String> = cuts(FILE, &|_| false).into_iter().map(|cut| cut.label).collect();
        assert_eq!(labels, ["A", "B", "ß", "ä"]);
        // Only the designs asked for are made.
        let made: Vec<bool> = cuts(FILE, &|i| i % 2 == 1).iter().map(|cut| cut.design.is_some()).collect();
        assert_eq!(made, [false, true, false, true]);
    }

    #[test]
    fn a_glyph_s_design_is_its_file_with_the_drawing_beside_it_left_out_and_the_glyph_shown() {
        let a = design_of(0);
        // Shown, with the style its shapes inherit kept.
        assert!(a.contains(r#"inkscape:label="GlyphLayer-A" style="stroke:#000000"><path id="a""#), "{a}");
        assert!(a.contains(r##"<use xlink:href="#inkstitch_trim"/>"##), "the glyph's commands stay: {a}");
        // Everything else drawn is left out, the definitions and the version kept.
        for gone in ["GlyphLayer-B", "id=\"note\"", "Latin", "Construction", "GlyphLayer-ß"] {
            assert!(!a.contains(gone), "{gone}: {a}");
        }
        for kept in ["<defs><symbol id=\"inkstitch_trim\">", "inkstitch_svg_version", "<?xml version=\"1.0\""] {
            assert!(a.contains(kept), "{kept}: {a}");
        }
        assert!(roxmltree::Document::parse(&a).is_ok(), "{a}");
    }

    #[test]
    fn a_glyph_inside_a_group_keeps_the_group_shown_with_its_transform_and_nothing_else_drawn_in_it() {
        let sz = design_of(2);
        assert!(sz.contains(r#"<g inkscape:label="Latin" transform="translate(1 2)"  style="fill:none">"#), "{sz}");
        assert!(sz.contains(r#"<g inkscape:label="GlyphLayer-ß" style=""><path id="sz""#), "{sz}");
        for gone in ["a note", "GlyphLayer-ä", "GlyphLayer-A", "GlyphLayer-B"] {
            assert!(!sz.contains(gone), "{gone}: {sz}");
        }
        // A glyph without a display of its own is left as it is.
        assert!(design_of(3).contains(r#"<g inkscape:label="GlyphLayer-ä"><path id="ae""#));
        // A shown glyph stays shown, its style unchanged when it sets no display.
        assert!(design_of(1).contains(r#"<g inkscape:groupmode="layer" inkscape:label="GlyphLayer-B" style=""><path id="b""#));
    }

    #[test]
    fn a_file_that_does_not_read_is_said_and_a_file_without_glyphs_has_none() {
        let refused = each_glyph(b"<svg><g></svg>", &Budget::DEFAULT, &|_| true, &mut |_, _| {}).unwrap_err();
        assert!(refused.message.starts_with("The file is not well-formed XML: "), "{refused}");
        assert_eq!(cuts(r#"<svg xmlns="http://www.w3.org/2000/svg"><g/></svg>"#, &|_| true), Vec::new());
        // Labelled so outside SVG's namespace, or not a group, it is no glyph.
        let others = r#"<svg xmlns="http://www.w3.org/2000/svg" xmlns:i="http://www.inkscape.org/namespaces/inkscape"><path i:label="GlyphLayer-x"/><g i:label=" GlyphLayer-y"/></svg>"#;
        assert_eq!(cuts(others, &|_| true), Vec::new());
    }

    #[test]
    fn only_svg_s_drawn_elements_beside_a_glyph_are_left_out() {
        let text = r#"<svg xmlns="http://www.w3.org/2000/svg" xmlns:inkscape="http://www.inkscape.org/namespaces/inkscape" xmlns:x="http://example.org/x"><x:path id="other"/><path id="drawn"/><g inkscape:label="GlyphLayer-A"><path id="a" d="M0 0 L1 1"/></g></svg>"#;
        let design = cuts(text, &|_| true).remove(0).design.unwrap();
        assert!(design.contains(r#"<x:path id="other"/>"#) && !design.contains(r#"id="drawn""#), "{design}");
    }

    #[test]
    fn a_unit_of_work_is_8_bytes_of_text_and_a_text_costs_1_more() {
        assert_eq!([0, 7, 8, 15, 16, 800].map(charge), [1, 1, 2, 2, 3, 101]);
    }

    #[test]
    fn cutting_a_glyph_charges_the_meter_and_stops_when_it_runs_out() {
        let budget = Budget { max_work: 3, ..Budget::DEFAULT };
        let refused = each_glyph(FILE.as_bytes(), &budget, &|_| true, &mut |_, _| {}).unwrap_err();
        assert_eq!(refused.code, stitchcraft_core::Code::BudgetExhausted);
    }
}
