//! Presentation properties: the part of CSS that decides whether an SVG element paints, and in which
//! colours.
//!
//! SVG elements take their colours and visibility from properties, written as presentation attributes
//! (`fill="red"`) or in the `style` attribute (`style="fill:red"`), which wins. Most properties inherit,
//! so the reader hands a [`Style`] from each element to its children, and each element changes what it
//! sets. A value that does not parse is ignored, as CSS ignores an invalid declaration: the last valid
//! declaration in `style` wins, then a valid presentation attribute, so StitchCraft sees what an SVG
//! viewer shows. The reader names a paint the element sets that never parses ([`Declared::paint`]).
//!
//! Colours are read as editors write them: keywords in any case (`currentcolor`), and an ICC colour
//! after the sRGB one (`#cd853f icc-color(…)`, from Inkscape's colour-managed picker) is left for the
//! sRGB one, as viewers without colour management do.
//!
//! Read here: `fill`, `stroke`, `color` (for `currentColor`), `fill-rule`, `opacity`, `fill-opacity`,
//! `stroke-opacity`, `visibility`, `display`, `paint-order` and the marker properties, each of the three
//! markers on its own. Style sheets
//! (`<style>` elements) are not read; the document reader warns when a file has one.

use std::str::FromStr;

use roxmltree::Node;
use stitchcraft_engine::design::FillRule;
use stitchcraft_plan::Rgb;
use svgtypes::{Color, Length, LengthUnit, PaintOrder, PaintOrderKind};

/// The properties one element sets itself: its `style` attribute's declarations, then its presentation
/// attributes.
pub struct Declared<'a, 'input> {
    node: Node<'a, 'input>,
    style: Vec<(&'a str, &'a str)>,
}

impl<'a, 'input> Declared<'a, 'input> {
    /// What `node` declares.
    pub fn of(node: Node<'a, 'input>) -> Self {
        Declared { node, style: node.attribute("style").map(declarations).unwrap_or_default() }
    }

    /// The value `node` gives `property`: the last declaration in its `style` attribute, else the
    /// presentation attribute.
    pub fn get(&self, property: &str) -> Option<&'a str> {
        let declared = self.style.iter().rev().find(|(name, _)| name.eq_ignore_ascii_case(property)).map(|(_, value)| *value);
        declared.or_else(|| self.node.attribute(property)).map(str::trim)
    }

    /// Whether `property` is set to something other than `none`: a clip path, mask or filter.
    pub fn uses(&self, property: &str) -> bool {
        self.get(property).is_some_and(names_something)
    }

    /// What the element declares for the paint `property` (`fill` or `stroke`).
    pub fn paint(&self, property: &str) -> Declaration<'a, Paint<'a>> {
        self.value(property, Paint::parse)
    }

    /// What the element declares for `property`, read with `parse`: the last declaration in `style` that
    /// reads, else the presentation attribute if it reads. `inherit` keeps the inherited value.
    fn value<T>(&self, property: &str, parse: impl Fn(&'a str) -> Option<T>) -> Declaration<'a, T> {
        let in_style = self.style.iter().rev().filter(|(name, _)| name.eq_ignore_ascii_case(property)).map(|(_, value)| *value);
        let mut unread = None;
        for value in in_style.chain(self.node.attribute(property)).map(str::trim) {
            if value.eq_ignore_ascii_case("inherit") {
                return Declaration::Inherited;
            }
            match parse(value) {
                Some(parsed) => return Declaration::Set(parsed),
                None => {
                    unread.get_or_insert(value);
                }
            }
        }
        unread.map_or(Declaration::Inherited, Declaration::Unreadable)
    }

    /// Every value the `style` attribute itself gives `property`, in order; presentation attributes are
    /// not looked at.
    pub fn in_style<'s>(&'s self, property: &'s str) -> impl Iterator<Item = &'a str> + 's {
        self.style.iter().filter(move |(name, _)| name.eq_ignore_ascii_case(property)).map(|(_, value)| *value)
    }

    /// The values `node` gives `marker-start`, `marker-mid` and `marker-end`. In the `style` attribute the
    /// `marker` shorthand sets all three where it stands in the list, and a later declaration wins; a
    /// marker the `style` attribute does not set comes from its presentation attribute, or else from a
    /// `marker` attribute.
    pub fn markers(&self) -> [Option<&'a str>; 3] {
        let mut found = [None; 3];
        for (name, value) in &self.style {
            if name.eq_ignore_ascii_case("marker") {
                found = [Some(*value); 3];
            } else if let Some(slot) = MARKERS.iter().position(|m| name.eq_ignore_ascii_case(m)).and_then(|i| found.get_mut(i)) {
                *slot = Some(*value);
            }
        }
        for (slot, property) in found.iter_mut().zip(MARKERS) {
            if slot.is_none() {
                *slot = self.node.attribute(property).or_else(|| self.node.attribute("marker")).map(str::trim);
            }
        }
        found
    }
}

/// What an element declares for a property.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Declaration<'a, T> {
    /// Nothing, or `inherit`: the inherited value stays.
    Inherited,
    /// A value that reads.
    Set(T),
    /// Values none of which reads, the one that would win first: the inherited value stays, as in a
    /// viewer.
    Unreadable(&'a str),
}

/// The marker properties, one per place on a path.
const MARKERS: [&str; 3] = ["marker-start", "marker-mid", "marker-end"];

/// Whether a clip path, mask, filter or marker value names one: it is set to something other than `none`.
fn names_something(value: &str) -> bool {
    !value.is_empty() && !value.eq_ignore_ascii_case("none")
}

/// The `name: value` declarations of a `style` attribute, in order. It splits on `;` outside quotes and
/// parentheses, drops `!important`, and skips a declaration without a colon.
fn declarations<'s>(style: &'s str) -> Vec<(&'s str, &'s str)> {
    let mut found = Vec::new();
    let mut add = |part: &'s str| {
        if let Some((name, value)) = part.split_once(':') {
            let value = value.trim();
            let value = match value.rsplit_once('!') {
                Some((before, flag)) if flag.trim().eq_ignore_ascii_case("important") => before.trim(),
                _ => value,
            };
            found.push((name.trim(), value));
        }
    };
    let (mut depth, mut quote, mut start) = (0_u32, None, 0);
    for (i, byte) in style.bytes().enumerate() {
        match (quote, byte) {
            (Some(q), _) if byte == q => quote = None,
            (None, b'"' | b'\'') => quote = Some(byte),
            (None, b'(') => depth += 1,
            (None, b')') => depth = depth.saturating_sub(1),
            (None, b';') if depth == 0 => {
                // `;` is ASCII, so both ends are character boundaries.
                add(style.get(start..i).unwrap_or_default());
                start = i + 1;
            }
            _ => {}
        }
    }
    add(style.get(start..).unwrap_or_default());
    found
}

/// What a `fill` or `stroke` paints with, as declared. It is resolved per element: `currentColor` takes
/// the element's own `color`, and a paint server's colour comes from the document.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Paint<'a> {
    /// A paint that needs nothing from the document.
    Plain(Plain),
    /// A paint server (a gradient or a pattern) by id, and what to use when it cannot be used.
    Server {
        /// The id in `url(#id)`.
        id: &'a str,
        /// The paint written after the reference, if any.
        fallback: Option<Plain>,
    },
}

/// A paint that is not a server.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Plain {
    /// Nothing.
    None,
    /// A colour.
    Color(Rgb),
    /// The element's `color` property.
    CurrentColor,
}

impl Plain {
    /// The colour it paints with on an element whose `color` is `current`, if it paints.
    pub const fn colour(self, current: Rgb) -> Option<Rgb> {
        match self {
            Plain::None => None,
            Plain::Color(color) => Some(color),
            Plain::CurrentColor => Some(current),
        }
    }
}

impl<'a> Paint<'a> {
    /// The paint `value` declares (not `inherit`, which the caller handles), or `None` when it does not
    /// parse: a paint server reference with the plain paint after it, or a plain paint.
    fn parse(value: &'a str) -> Option<Paint<'a>> {
        let value = value.trim();
        let is_reference = value.get(..4).is_some_and(|start| start.eq_ignore_ascii_case("url("));
        if !is_reference {
            return plain(value).map(Paint::Plain);
        }
        // `url(#id)`, the function name in any case, the reference quoted or not.
        let end = value.find(')')?;
        let inside = value.get(4..end)?.trim();
        let unquoted = ['"', '\''].iter().find_map(|&q| inside.strip_prefix(q)?.strip_suffix(q)).unwrap_or(inside);
        let id = unquoted.trim().strip_prefix('#').filter(|id| !id.is_empty())?;
        let rest = value.get(end + 1..)?.trim();
        let fallback = if rest.is_empty() { None } else { Some(plain(rest)?) };
        Some(Paint::Server { id, fallback })
    }
}

/// A paint that is not a server, in any case: `none` (or `context-fill` and `context-stroke`, which
/// nothing here provides), `currentColor` or a colour.
fn plain(value: &str) -> Option<Plain> {
    let value = without_icc(value);
    let is = |keyword: &str| value.eq_ignore_ascii_case(keyword);
    if is("none") || is("context-fill") || is("context-stroke") {
        Some(Plain::None)
    } else if is("currentcolor") {
        Some(Plain::CurrentColor)
    } else {
        Color::from_str(value).ok().map(colour)
    }
}

/// `value` without an ICC colour after the sRGB colour (`#cd853f icc-color(…)`); a value that is only an
/// ICC colour stays as it is, and does not parse.
pub fn without_icc(value: &str) -> &str {
    let at = value.as_bytes().windows(10).position(|w| w.eq_ignore_ascii_case(b"icc-color("));
    match at.and_then(|i| value.get(..i)).map(str::trim_end) {
        Some(srgb) if !srgb.is_empty() => srgb,
        _ => value,
    }
}

/// A parsed colour as a paint: a fully transparent one paints nothing.
fn colour(color: Color) -> Plain {
    if color.alpha == 0 { Plain::None } else { Plain::Color(rgb(color)) }
}

/// The colour without its alpha: thread has no transparency.
pub fn rgb(color: Color) -> Rgb {
    Rgb::new(color.red, color.green, color.blue)
}

/// The inherited state that decides how an element paints.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Style<'a> {
    /// The `fill` property.
    pub fill: Paint<'a>,
    /// The `stroke` property.
    pub stroke: Paint<'a>,
    /// The `fill-rule` property.
    pub fill_rule: FillRule,
    /// The `color` property, which `currentColor` means.
    pub color: Rgb,
    /// `fill-opacity` is zero: the fill is invisible.
    pub fill_clear: bool,
    /// `stroke-opacity` is zero: the stroke is invisible.
    pub stroke_clear: bool,
    /// The `visibility` property is `visible`.
    pub visible: bool,
    /// This element or an ancestor has `opacity: 0`, which no descendant can undo.
    pub transparent: bool,
    /// `paint-order` puts the stroke before the fill, which a viewer shows and the reader notes: it sews
    /// the fill first.
    pub stroke_first: bool,
    /// Which of `marker-start`, `marker-mid` and `marker-end` name a marker: arrowheads and the like,
    /// drawn on top of the outline. Each inherits on its own.
    pub markers: [bool; 3],
}

impl Default for Style<'_> {
    /// SVG's initial values: a black fill, no stroke, visible.
    fn default() -> Self {
        Style {
            fill: Paint::Plain(Plain::Color(Rgb::new(0, 0, 0))),
            stroke: Paint::Plain(Plain::None),
            fill_rule: FillRule::NonZero,
            color: Rgb::new(0, 0, 0),
            fill_clear: false,
            stroke_clear: false,
            visible: true,
            transparent: false,
            stroke_first: false,
            markers: [false; 3],
        }
    }
}

impl<'a> Style<'a> {
    /// The style of an element that inherits this one and declares `declared`; `None` when the element
    /// has `display: none`, which hides it and everything inside it.
    pub fn child(&self, declared: &Declared<'a, '_>) -> Option<Style<'a>> {
        let get = |property: &str| declared.get(property);
        if get("display").is_some_and(|v| v.eq_ignore_ascii_case("none")) {
            return None;
        }
        let mut style = *self;
        if let Declaration::Set(paint) = declared.paint("fill") {
            style.fill = paint;
        }
        if let Declaration::Set(paint) = declared.paint("stroke") {
            style.stroke = paint;
        }
        if let Declaration::Set(color) = declared.value("color", |v| Color::from_str(without_icc(v)).ok()) {
            style.color = rgb(color);
        }
        match get("fill-rule") {
            Some(v) if v.eq_ignore_ascii_case("evenodd") => style.fill_rule = FillRule::EvenOdd,
            Some(v) if v.eq_ignore_ascii_case("nonzero") => style.fill_rule = FillRule::NonZero,
            _ => {}
        }
        if let Some(clear) = get("fill-opacity").and_then(zero) {
            style.fill_clear = clear;
        }
        if let Some(clear) = get("stroke-opacity").and_then(zero) {
            style.stroke_clear = clear;
        }
        style.transparent |= get("opacity").and_then(zero).unwrap_or(false);
        match get("visibility") {
            Some(v) if v.eq_ignore_ascii_case("visible") => style.visible = true,
            Some(v) if v.eq_ignore_ascii_case("hidden") || v.eq_ignore_ascii_case("collapse") => style.visible = false,
            _ => {}
        }
        if let Some(order) = get("paint-order").and_then(|v| PaintOrder::from_str(v).ok()) {
            // Whichever of the two comes first in the list paints first.
            let first = order.order.iter().find(|k| matches!(k, PaintOrderKind::Fill | PaintOrderKind::Stroke));
            style.stroke_first = first == Some(&PaintOrderKind::Stroke);
        }
        for (marked, value) in style.markers.iter_mut().zip(declared.markers()) {
            if let Some(value) = value.filter(|v| !v.eq_ignore_ascii_case("inherit")) {
                *marked = names_something(value);
            }
        }
        Some(style)
    }

    /// Whether anything this element paints can be seen.
    pub fn shown(&self) -> bool {
        self.visible && !self.transparent
    }

    /// Whether any marker property names a marker.
    pub fn has_markers(&self) -> bool {
        self.markers.contains(&true)
    }
}

/// Whether an opacity (a number, or a percentage) is zero or less; `None` when it does not parse.
fn zero(value: &str) -> Option<bool> {
    let length = Length::from_str(value).ok()?;
    matches!(length.unit, LengthUnit::None | LengthUnit::Percent).then_some(length.number <= 0.0)
}

#[cfg(test)]
mod tests {
    use roxmltree::Document;

    use super::*;

    const BLUE: Paint<'static> = Paint::Plain(Plain::Color(Rgb::new(0, 0, 255)));
    const NONE: Paint<'static> = Paint::Plain(Plain::None);

    /// `check` on the style of the element with id `id` in `svg`, inherited through every ancestor.
    fn with_style(svg: &str, id: &str, check: impl FnOnce(Option<Style<'_>>)) {
        let doc = Document::parse(svg).unwrap();
        let node = doc.descendants().find(|n| n.attribute("id") == Some(id)).unwrap();
        let mut chain: Vec<_> = node.ancestors().filter(roxmltree::Node::is_element).collect();
        chain.reverse();
        check(chain.iter().try_fold(Style::default(), |style, n| style.child(&Declared::of(*n))));
    }

    #[test]
    fn colours_are_read_as_editors_write_them() {
        assert_eq!(without_icc("#cd853f icc-color(sRGB-IEC61966-2.1, 0.8, 0.52, 0.25)"), "#cd853f");
        assert_eq!(without_icc("#cd853f ICC-COLOR(x, 1)"), "#cd853f");
        assert_eq!(without_icc("icc-color(x, 1)"), "icc-color(x, 1)", "an ICC colour alone stays, and does not parse");
        assert_eq!((plain("CurrentColor"), plain("NONE"), plain("Context-Fill")), (Some(Plain::CurrentColor), Some(Plain::None), Some(Plain::None)));
        assert_eq!(plain("#00000000"), Some(Plain::None), "a fully transparent colour paints nothing");
        let server = |id, fallback| Some(Paint::Server { id, fallback });
        assert_eq!(Paint::parse("url(#g) CURRENTCOLOR"), server("g", Some(Plain::CurrentColor)));
        assert_eq!(Paint::parse("URL( '#g' )"), server("g", None));
        assert_eq!(Paint::parse(r##"url("#g") none"##), server("g", Some(Plain::None)));
        for unreadable in ["url(#g) bogus", "url(#g", "url(g)", "url(#)", "url(\"#g')", "bogus"] {
            assert_eq!(Paint::parse(unreadable), None, "{unreadable}");
        }
    }

    #[test]
    fn a_declaration_that_does_not_read_gives_way_to_the_next() {
        let doc = Document::parse(
            r##"<svg><path id="a" style="fill:#00f;fill:bogus" fill="red"/><path id="b" style="fill:bogus" fill="worse"/>
               <path id="c" style="fill:inherit" fill="red"/><path id="d" style="fill:bogus" fill="inherit"/><path id="e"/></svg>"##,
        )
        .unwrap();
        let paint = |id| Declared::of(doc.descendants().find(|n| n.attribute("id") == Some(id)).unwrap()).paint("fill");
        assert_eq!(paint("a"), Declaration::Set(BLUE));
        assert_eq!(paint("b"), Declaration::Unreadable("bogus"), "the value that would win is named");
        assert_eq!((paint("c"), paint("d"), paint("e")), (Declaration::Inherited, Declaration::Inherited, Declaration::Inherited));
    }

    #[test]
    fn style_attributes_win_over_presentation_attributes() {
        with_style(r#"<svg><path id="p" fill="red" style="fill: blue ; stroke:#00ff00 !important"/></svg>"#, "p", |s| {
            let s = s.unwrap();
            assert_eq!((s.fill, s.stroke), (BLUE, Paint::Plain(Plain::Color(Rgb::new(0, 255, 0)))));
        });
        // The last declaration wins; quotes and parentheses do not split.
        let d = declarations("fill:red;fill:url(#a;b) blue;font-family:'a;b'");
        assert_eq!(d, [("fill", "red"), ("fill", "url(#a;b) blue"), ("font-family", "'a;b'")]);
        assert_eq!(declarations(" ; nothing ; :x"), [("", "x")]);
        // A quote ends where it closes; a `!` that is not `!important` stays in the value.
        assert_eq!(declarations("font-family:'a;b';fill:red"), [("font-family", "'a;b'"), ("fill", "red")]);
        assert_eq!(declarations("x: a!b; y: c ! IMPORTANT"), [("x", "a!b"), ("y", "c")]);
    }

    #[test]
    fn properties_inherit_and_children_override() {
        let svg = r##"<svg fill="#0000ff" color="red"><g stroke="currentColor" fill-rule="evenodd" visibility="hidden">
            <path id="a"/><path id="b" visibility="visible" fill="inherit" stroke="bogus"/></g></svg>"##;
        with_style(svg, "a", |a| {
            let a = a.unwrap();
            assert_eq!((a.fill, a.stroke), (BLUE, Paint::Plain(Plain::CurrentColor)));
            assert_eq!((a.fill_rule, a.color, a.visible), (FillRule::EvenOdd, Rgb::new(255, 0, 0), false));
        });
        with_style(
            r#"<svg fill-rule="evenodd" visibility="visible"><g fill-rule="bogus" visibility="bogus"><path id="p" fill-rule="nonzero"/><path id="q"/></g></svg>"#,
            "p",
            |p| {
                assert_eq!(p.unwrap().fill_rule, FillRule::NonZero);
            },
        );
        with_style(r#"<svg fill-rule="evenodd"><g fill-rule="bogus" visibility="bogus"><path id="q"/></g></svg>"#, "q", |q| {
            let q = q.unwrap();
            assert_eq!((q.fill_rule, q.visible), (FillRule::EvenOdd, true), "values that do not parse change nothing");
        });
        with_style(svg, "b", |b| {
            let b = b.unwrap();
            assert!(b.visible, "visibility can be undone by a child");
            assert_eq!((b.fill, b.stroke), (BLUE, Paint::Plain(Plain::CurrentColor)), "`inherit` and invalid values keep the inherited paint");
        });
        assert_eq!(Plain::CurrentColor.colour(Rgb::new(1, 2, 3)), Some(Rgb::new(1, 2, 3)));
        assert_eq!(Plain::None.colour(Rgb::new(1, 2, 3)), None);
    }

    #[test]
    fn hiding() {
        with_style(r#"<svg><g style="display:none"><path id="p"/></g></svg>"#, "p", |s| assert_eq!(s, None));
        with_style(r#"<svg><g opacity="0"><path id="p" opacity="1" visibility="visible"/></g></svg>"#, "p", |s| {
            assert!(!s.unwrap().shown(), "opacity 0 on a group hides its children for good");
        });
        with_style(r#"<svg><path id="p" fill="transparent" stroke="rgba(0,0,0,0)" fill-opacity="0%" stroke-opacity="0.5"/></svg>"#, "p", |s| {
            let s = s.unwrap();
            assert_eq!((s.fill, s.stroke, s.fill_clear, s.stroke_clear), (NONE, NONE, true, false));
            assert!(s.shown());
        });
        assert_eq!(zero("-1"), Some(true));
        assert_eq!(zero("2mm"), None);
        assert_eq!(zero("x"), None);
    }

    #[test]
    fn servers_paint_order_and_markers() {
        let svg = r#"<svg><path id="p" fill="url(#g) green" stroke="url(#h)" paint-order="stroke" marker-end="url(#m)"/></svg>"#;
        with_style(svg, "p", |s| {
            let s = s.unwrap();
            assert_eq!(s.fill, Paint::Server { id: "g", fallback: Some(Plain::Color(Rgb::new(0, 128, 0))) });
            assert_eq!(s.stroke, Paint::Server { id: "h", fallback: None });
            assert!(s.stroke_first && s.has_markers());
            assert_eq!(s.markers, [false, false, true]);
        });
        with_style(r#"<svg paint-order="markers"><g marker-end="url(#m)"><path id="p" marker-end="none"/></g></svg>"#, "p", |s| {
            let s = s.unwrap();
            assert!(!s.stroke_first && !s.has_markers());
        });
        // The shorthand sets all three where it stands; `inherit` keeps the parent's; presentation
        // attributes fill in what the style attribute leaves.
        let svg = r#"<svg><g marker-start="url(#a)"><path id="p" style="marker-end:url(#b);marker:none;marker-mid:url(#c)" marker-end="url(#d)"/>
            <path id="q" marker-start="inherit" marker="url(#e)"/></g></svg>"#;
        with_style(svg, "p", |s| assert_eq!(s.unwrap().markers, [false, true, false]));
        with_style(svg, "q", |s| assert_eq!(s.unwrap().markers, [true, true, true]));
    }
}
