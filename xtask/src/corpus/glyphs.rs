//! The glyphs of a glyph file, and each glyph as a design of its own.
//!
//! **Glyphs.** A glyph file of Ink/Stitch's fonts has a group for each glyph, labelled `GlyphLayer-` and the
//! glyph's name (`docs/src/design/fonts.md`). Most are layers of the document, hidden. Ink/Stitch's lettering
//! takes every group whose `inkscape:label` starts so, wherever it lies in the file (`lib/lettering/
//! font_variant.py`, read at `d59c9ab`), and so do [`glyphs`] and [`designs`]: 21,616 of them in the 142
//! fonts at `c7e3a05`, of which 3,030 lie inside other groups, in 11 fonts.
//!
//! **A glyph as a design.** The differential sews each glyph alone, its group shown and the other glyphs left
//! out ([`designs`]). The design is the glyph file with 2 kinds of edits, made to its text, so that
//! everything else reads exactly as it was written: its version, its definitions (the symbols of
//! Ink/Stitch's commands, clip paths), and the groups round the glyph with their transforms and styles.
//!
//! - Each drawn element (a shape, an image, text, a clone, a link, a switch or a group) beside the glyph's
//!   group or beside a group round it is left out, with all it holds. Other elements stay, such as `defs`,
//!   `style`, `metadata` and Inkscape's `namedview`.
//! - The glyph's group and the groups round it are shown: a `display` attribute is left out, and so is a
//!   `display` property of a `style` attribute, as lettering clears them. Their other properties, which the
//!   glyph's shapes may inherit, stay.
//!
//! A glyph file is untrusted input: [`designs`] reads it with a limit on its nodes, and an error says what is
//! wrong with it. Nothing panics.

use std::ops::Range;

/// What a glyph group's label starts with.
pub const PREFIX: &str = "GlyphLayer-";
/// The most nodes a glyph file may have.
const NODES: u32 = 20_000_000;
/// Inkscape's namespace, of `inkscape:label`.
const INKSCAPE: &str = "http://www.inkscape.org/namespaces/inkscape";
/// SVG's namespace.
const SVG: &str = "http://www.w3.org/2000/svg";
/// The drawn elements of SVG's namespace, which a design leaves out beside the glyph.
const DRAWN: &[&str] =
    &["a", "circle", "ellipse", "foreignObject", "g", "image", "line", "path", "polygon", "polyline", "rect", "svg", "switch", "text", "use"];

/// A glyph of a glyph file.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Glyph {
    /// Its name, its group's label after [`PREFIX`].
    pub name: String,
    /// The glyph file as a design of this glyph alone, when it was asked for.
    pub design: Option<String>,
}

/// The glyphs of the glyph file `text`, in the order of the file, each with its design when `wanted` says so
/// of its place in that order.
pub fn designs(text: &str, wanted: &dyn Fn(usize) -> bool) -> Result<Vec<Glyph>, String> {
    let mut glyphs = Vec::new();
    each_design(text, wanted, &mut |_, glyph| glyphs.push(glyph))?;
    Ok(glyphs)
}

/// Gives `each` the glyphs of the glyph file `text`, with their places in the order of the file, each with its
/// design when `wanted` says so of its place, one after another, so that a large file's designs are made as
/// they are taken and never held all at once.
pub fn each_design(text: &str, wanted: &dyn Fn(usize) -> bool, each: &mut dyn FnMut(usize, Glyph)) -> Result<(), String> {
    let options = roxmltree::ParsingOptions { allow_dtd: true, nodes_limit: NODES, ..roxmltree::ParsingOptions::default() };
    let document = roxmltree::Document::parse_with_options(text, options).map_err(|e| format!("the glyph file doesn't read: {e}"))?;
    let groups = document.descendants().filter(|node| node.has_tag_name((SVG, "g")) && name(node).is_some());
    for (index, group) in groups.enumerate() {
        let design = wanted(index).then(|| design(text, group));
        each(index, Glyph { name: name(&group).unwrap_or_default().to_string(), design });
    }
    Ok(())
}

/// The glyphs of the glyph file `text`, by name, in the order of the file.
pub fn glyphs(text: &str) -> Result<Vec<String>, String> {
    Ok(designs(text, &|_| false)?.into_iter().map(|glyph| glyph.name).collect())
}

/// The name of the glyph whose group `node` is, if it is one.
fn name<'a>(node: &roxmltree::Node<'a, '_>) -> Option<&'a str> {
    node.attribute((INKSCAPE, "label"))?.strip_prefix(PREFIX)
}

/// The glyph file `text` as a design of the glyph whose group is `glyph` alone.
fn design(text: &str, glyph: roxmltree::Node) -> String {
    let mut edits: Vec<(Range<usize>, String)> = Vec::new();
    let mut kept = glyph;
    while let Some(parent) = kept.parent_element() {
        edits.extend(shown(text, kept));
        let beside = parent.children().filter(|child| *child != kept && drawn(child));
        edits.extend(beside.map(|child| (child.range(), String::new())));
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
    out
}

/// Whether `node` is a drawn element of SVG's namespace.
fn drawn(node: &roxmltree::Node) -> bool {
    node.is_element() && node.tag_name().namespace() == Some(SVG) && DRAWN.contains(&node.tag_name().name())
}

/// The edits that show the element `node`: its `display` attribute left out, and the `display` property of
/// its `style` attribute.
fn shown(text: &str, node: roxmltree::Node) -> Vec<(Range<usize>, String)> {
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

    fn design_of(index: usize) -> String {
        designs(FILE, &|i| i == index).unwrap().into_iter().find_map(|glyph| glyph.design).unwrap()
    }

    #[test]
    fn every_group_labelled_as_a_glyph_is_one_wherever_it_lies() {
        assert_eq!(glyphs(FILE).unwrap(), ["A", "B", "ß", "ä"]);
        // Only the designs asked for are made.
        let made: Vec<bool> = designs(FILE, &|i| i % 2 == 1).unwrap().iter().map(|glyph| glyph.design.is_some()).collect();
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
        assert!(designs("<svg><g></svg>", &|_| true).unwrap_err().starts_with("the glyph file doesn't read: "));
        assert_eq!(glyphs(r#"<svg xmlns="http://www.w3.org/2000/svg"><g/></svg>"#).unwrap(), Vec::<String>::new());
        // Labelled so outside SVG's namespace, or not a group, it is no glyph.
        let others = r#"<svg xmlns="http://www.w3.org/2000/svg" xmlns:i="http://www.inkscape.org/namespaces/inkscape"><path i:label="GlyphLayer-x"/><g i:label=" GlyphLayer-y"/></svg>"#;
        assert_eq!(glyphs(others).unwrap(), Vec::<String>::new());
    }
}
