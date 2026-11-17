//! A `<text>` element under automatic digitizing (roadmap step M14.7, `REQ-SVG-017`,
//! `docs/src/design/svg-input.md` › Text): what is read of it, for lettering to sew in an embroidery font
//! ([`crate::lettering::objects`]).
//!
//! **Why only this much.** An SVG file keeps a text's characters and names its typeface, and a viewer lays
//! the characters out with the typeface's widths. The reader has no typeface, and the embroidery font that
//! stands in has widths of its own. So it reads what doesn't depend on a width: where each line starts on
//! its baseline, how it is anchored there, its font size, its fill and how the element is turned. What
//! places single letters by the typeface's widths is said and left out.
//!
//! **Lines.** The `<text>` element starts a line at its `x` and `y`. A `<tspan>` with an `x` or a `y`
//! starts a new one, as editors write the lines of a text, and a `dx` or `dy` there moves the line's start.
//! A `<tspan>` without them goes on in the line it is in. A line has the font size, the fill and the anchor
//! of the element its first visible character is in.
//!
//! **The reader has no fonts.** It gives each text object with its place in the stitching order
//! ([`Waiting`]), and a host with fonts sets them ([`crate::lettering::objects::lettered`]).
//!
//! **Bounded.** A unit of work for each node inside the element, and for each element between a run of
//! characters and the root, each time one of its properties is looked for.

use std::collections::BTreeSet;
use std::str::FromStr;

use roxmltree::Node;
use stitchcraft_core::units::Point;
use stitchcraft_core::{Code, Diagnostic, math};
use stitchcraft_engine::design::within_working_limit;
use stitchcraft_plan::Thread;
use svgtypes::{Length, LengthUnit};

use super::{Axis, Reader};
use crate::inkstitch::INKSCAPE_NS;
use crate::lettering::objects::{self, Anchor, Line, TextObject, Waiting};
use crate::style::{Paint, Plain, Style};
use crate::transform::{Affine, user_units};

/// The font size of a text nothing gives one: CSS's `medium`, 16 pixels.
const MEDIUM: f64 = 16.0;

/// The namespace of `xml:space`.
const XML_NS: &str = "http://www.w3.org/XML/1998/namespace";

/// How a vertical text or one nobody sees is stitched.
const TO_PATHS: &str = "Convert the text to paths first (in Inkscape: Path › Object to Path).";

/// A line as it is read: its characters as the file has them, and where it starts in user units.
struct Draft<'a> {
    raw: String,
    x: f64,
    y: f64,
    /// How long its `textLength` says it is, in user units.
    length: Option<f64>,
    /// What it takes from its first visible character.
    first: Option<First<'a>>,
}

/// What a line takes from its first visible character: its font size, its anchor, its fill's thread, its
/// typeface and whether its weight is bold.
type First<'a> = (f64, Anchor, Option<Thread>, Option<&'a str>, bool);

impl<'b, 'a, 'input> Reader<'b, 'a, 'input> {
    /// The `<text>` element `node`, `label`, drawn with `style` under `map` inside the clips of the chain
    /// from `clipped`: read as a text object that waits for a host's fonts. Vertical text is left out with
    /// `SC-W1111`, and what is left out of a text that is read is said with `SC-W0802`.
    pub(super) fn text(
        &mut self,
        node: Node<'a, 'input>,
        label: &str,
        map: Affine,
        style: &Style<'a>,
        clipped: Option<usize>,
    ) -> Result<(), Diagnostic> {
        let mode = self.inherited(node, "writing-mode")?.first().copied().unwrap_or_default();
        if ["tb", "tb-rl", "vertical-rl", "vertical-lr", "sideways-rl", "sideways-lr"].iter().any(|vertical| mode.eq_ignore_ascii_case(vertical)) {
            let message = format!("`{label}` is vertical text, which lettering doesn't set, and is not stitched.");
            self.warn(Code::TextNotLettered, Some(label), message, Some(TO_PATHS));
            return Ok(());
        }
        let preserve = node.ancestors().find_map(|at| at.attribute((XML_NS, "space"))) == Some("preserve");
        let mut left_out: BTreeSet<&'static str> = BTreeSet::new();
        let (x, y) = self.start(node, (0.0, 0.0), &mut left_out);
        // The length of the whole text, which is its line's when it has one line.
        let whole = self.stated(node);
        let mut drafts = vec![Draft { raw: String::new(), x, y, length: None, first: None }];
        let mut on_a_path = false;
        for inside in node.descendants().skip(1) {
            self.charge(1)?;
            if inside.is_element() {
                let positioned = inside.has_attribute("x") || inside.has_attribute("y");
                match inside.tag_name().name() {
                    "tspan" if positioned => {
                        let from = drafts.last().map_or((x, y), |last| (last.x, last.y));
                        let (x, y) = self.start(inside, from, &mut left_out);
                        drafts.push(Draft { raw: String::new(), x, y, length: self.stated(inside), first: None });
                    }
                    "tspan" => {
                        if inside.has_attribute("textLength") {
                            left_out.insert("a `textLength` inside a line");
                        }
                        if inside.has_attribute("dx") || inside.has_attribute("dy") {
                            left_out.insert("a `dx` or `dy` inside a line");
                        }
                        if inside.has_attribute("rotate") {
                            left_out.insert("`rotate`");
                        }
                    }
                    "textPath" => on_a_path = true,
                    _ => {}
                }
                continue;
            }
            let Some(characters) = inside.text().filter(|_| inside.is_text()) else { continue };
            let Some(shown) = self.shown_in(inside, node, style)? else { continue };
            // Characters with neither a fill nor a stroke are in the file and not in the picture.
            let unpainted = |paint: Paint<'a>, clear: bool| clear || paint == Paint::Plain(Plain::None);
            if unpainted(shown.fill, shown.fill_clear) && unpainted(shown.stroke, shown.stroke_clear) {
                continue;
            }
            let Some(draft) = drafts.last_mut() else { continue };
            draft.raw.push_str(characters);
            if draft.first.is_none() && !characters.trim().is_empty() {
                let parent = inside.parent().unwrap_or(node);
                let size = self.font_size(parent)?;
                let anchor = match self.inherited(parent, "text-anchor")?.first().copied().unwrap_or_default() {
                    middle if middle.eq_ignore_ascii_case("middle") => Anchor::Middle,
                    end if end.eq_ignore_ascii_case("end") => Anchor::End,
                    _ => Anchor::Start,
                };
                let typeface = self.inherited(parent, "font-family")?.first().copied();
                let bold = self.inherited(parent, "font-weight")?.first().is_some_and(|weight| objects::bold_weight(weight));
                let fill = if shown.fill_clear { None } else { self.colour(label, "fill", shown.fill, shown.color, false)? };
                let stroke = if shown.stroke_clear { None } else { self.colour(label, "stroke", shown.stroke, shown.color, false)? };
                let thread = fill.or(stroke).map(|painted| Thread::new(painted.colour));
                draft.first = Some((size, anchor, thread, typeface, bold));
            }
        }
        if on_a_path {
            let message = format!("`{label}` holds text on a path (`<textPath>`), which lettering doesn't follow here; that text is left out.");
            self.note(Some(label), message, Some(TO_PATHS));
        }
        // Where the element's axes go: along its baselines and down across them.
        let (ox, oy) = map.apply(0.0, 0.0);
        let (ax, ay) = map.apply(1.0, 0.0);
        let (dx, dy) = map.apply(0.0, 1.0);
        let (a, b, c, d) = (ax - ox, ay - oy, dx - ox, dy - oy);
        let (along, across) = (math::hypot(a, b), math::hypot(c, d));
        // The map doesn't flatten the element, or it would not be drawn: its baseline has a length.
        let turn = (a / along, b / along);
        let (mut typeface, mut bold) = (None, false);
        let mut lines = Vec::new();
        for draft in drafts {
            let Some((size, anchor, thread, family, heavy)) = draft.first else { continue };
            let text = if preserve { draft.raw.replace(['\n', '\r', '\t'], " ") } else { draft.raw.split_whitespace().collect::<Vec<_>>().join(" ") };
            let size = size * across;
            // A viewer shows nothing of a text without a size, or without a fill and a stroke.
            if size.partial_cmp(&0.0) != Some(std::cmp::Ordering::Greater) || thread.is_none() {
                continue;
            }
            let (px, py) = map.apply(draft.x, draft.y);
            let Some(at) = Point::new(px, py).ok().filter(|at| within_working_limit(*at)) else {
                self.unusable(label, format!("`{label}` lies beyond the design's reach, so it is left out."));
                return Ok(());
            };
            typeface = typeface.or(family);
            bold = bold || (lines.is_empty() && heavy);
            lines.push(Line { text, at, anchor, size, thread, length: draft.length.map(|length| length * along) });
        }
        match (lines.as_mut_slice(), whole) {
            ([line], Some(whole)) => line.length = line.length.or(Some(whole * along)),
            ([_, _, ..], Some(_)) => {
                left_out.insert("a `textLength` of several lines");
            }
            _ => {}
        }
        if !left_out.is_empty() {
            let listed = left_out.into_iter().collect::<Vec<_>>().join(", ");
            let message = format!(
                "`{label}` places letters one by one ({listed}), which lettering doesn't follow: each line's letters stand as its font spaces them."
            );
            self.note(Some(label), message, None);
        }
        if lines.is_empty() {
            return Ok(());
        }
        if !upright(map) {
            let message = format!(
                "`{label}` is stretched, mirrored or slanted, which lettering doesn't follow: it is sewn as tall as it is drawn, turned as its baseline is."
            );
            self.note(Some(label), message, None);
        }
        if self.declared(node)?.uses("clip-path") {
            self.note(Some(label), format!("The clip path of `{label}` itself is not applied to its lettering."), None);
        }
        let clips = self.chain(clipped)?;
        let name = node.attribute((INKSCAPE_NS, "label")).map(str::trim).filter(|s| !s.is_empty()).map(str::to_string);
        let typeface = typeface.map(|family| family.trim().to_string()).filter(|family| !family.is_empty());
        let text = TextObject { label: format!("svg:{label}"), name, typeface, bold, lines, turn, clips };
        self.texts.push(Waiting { after: self.elements.len(), text });
        Ok(())
    }

    /// Where a line that `node` starts begins, in user units: its `x` and `y`, each `from`'s when it has
    /// none, moved by its `dx` and `dy`. Of a list of numbers, which places letters one by one, the first
    /// counts, and `left_out` gains what isn't followed.
    fn start(&self, node: Node<'a, 'input>, from: (f64, f64), left_out: &mut BTreeSet<&'static str>) -> (f64, f64) {
        let mut first = |name: &str, axis: Axis| {
            let mut numbers = node.attribute(name).unwrap_or_default().split([',', ' ', '\t', '\n', '\r']).filter(|part| !part.is_empty());
            let first = numbers.next().and_then(|text| Length::from_str(text).ok()).map(|length| user_units(length, self.reference(axis)));
            if numbers.next().is_some() {
                left_out.insert("a place for each letter");
            }
            first
        };
        let x = first("x", Axis::X).unwrap_or(from.0) + first("dx", Axis::X).unwrap_or(0.0);
        let y = first("y", Axis::Y).unwrap_or(from.1) + first("dy", Axis::Y).unwrap_or(0.0);
        if node.has_attribute("rotate") {
            left_out.insert("`rotate`");
        }
        (x, y)
    }

    /// How long the `textLength` of `node` says its text is, in user units: `None` without one, and for one
    /// that is no length a text can have.
    fn stated(&self, node: Node<'a, 'input>) -> Option<f64> {
        self.length(node, "textLength", Axis::X).ok().flatten().filter(|length| *length > 0.0)
    }

    /// The style the characters `inside` are drawn with, in the `<text>` element `text` drawn with `style`:
    /// `None` when a viewer doesn't show them, or shows them on a path. Only a `<tspan>` or a link lies
    /// between characters a viewer shows in their line and their `<text>` element.
    fn shown_in(&mut self, inside: Node<'a, 'input>, text: Node<'a, 'input>, style: &Style<'a>) -> Result<Option<Style<'a>>, Diagnostic> {
        let between: Vec<Node<'a, 'input>> = inside.ancestors().skip(1).take_while(|at| *at != text).collect();
        let mut shown = *style;
        for element in between.into_iter().rev() {
            self.charge(1)?;
            if !matches!(element.tag_name().name(), "tspan" | "a") {
                return Ok(None);
            }
            let declared = self.declared(element)?;
            let Some(own) = shown.child(&declared) else { return Ok(None) };
            shown = own;
        }
        Ok(shown.shown().then_some(shown))
    }

    /// What `node` and the elements around it declare for the inherited `property`, the nearest first.
    fn inherited(&mut self, node: Node<'a, 'input>, property: &str) -> Result<Vec<&'a str>, Diagnostic> {
        let mut values = Vec::new();
        for at in node.ancestors().filter(Node::is_element) {
            self.charge(1)?;
            values.extend(self.declared(at)?.get(property).map(str::trim).filter(|value| !value.eq_ignore_ascii_case("inherit")));
        }
        Ok(values)
    }

    /// The font size of `node`, in user units: the nearest `font-size` that is a length, times those nearer
    /// that are a share of it (`em`, `ex` as half an `em`, and `%`). One that is a word, such as `larger`,
    /// isn't read, and the size around it stays. With none, CSS's `medium`.
    fn font_size(&mut self, node: Node<'a, 'input>) -> Result<f64, Diagnostic> {
        let mut share = 1.0;
        for value in self.inherited(node, "font-size")? {
            let Ok(length) = Length::from_str(value) else { continue };
            match length.unit {
                LengthUnit::Em => share *= length.number,
                LengthUnit::Ex => share *= length.number / 2.0,
                LengthUnit::Percent => share *= length.number / 100.0,
                _ => return Ok(share * user_units(length, 0.0)),
            }
        }
        Ok(share * MEDIUM)
    }
}

/// Whether `map` draws letters upright and as wide as they are tall: it scales both axes alike and turns
/// them as one, without a mirror or a skew.
fn upright(map: Affine) -> bool {
    map.stretches_evenly() && !map.mirrors()
}
