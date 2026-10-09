//! Presentation properties: the part of CSS that decides whether an SVG element paints, and in which
//! colours.
//!
//! SVG elements take their colours and visibility from properties, written as presentation attributes
//! (`fill="red"`) or in the `style` attribute (`style="fill:red"`), which wins. Most properties inherit,
//! so the reader hands a [`Style`] from each element to its children, and each element changes what it
//! sets. A value that does not parse is ignored, as CSS ignores an invalid declaration, so StitchCraft
//! sees what an SVG viewer shows.
//!
//! Read here: `fill`, `stroke`, `color` (for `currentColor`), `fill-rule`, `opacity`, `fill-opacity`,
//! `stroke-opacity`, `visibility`, `display`, `paint-order` and the marker properties. Style sheets
//! (`<style>` elements) are not read; the document reader warns when a file has one.

use std::str::FromStr;

use roxmltree::Node;
use stitchcraft_engine::design::FillRule;
use stitchcraft_plan::Rgb;
use svgtypes::{Color, Length, LengthUnit, PaintFallback, PaintOrder, PaintOrderKind};

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

    /// Whether `property` is set to something other than `none`: a clip path, mask, filter or marker.
    pub fn uses(&self, property: &str) -> bool {
        self.get(property).is_some_and(|value| !value.is_empty() && !value.eq_ignore_ascii_case("none"))
    }
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
    /// The paint `value` declares, or `None` when it does not parse (the declaration is then ignored) or
    /// says `inherit` (the inherited paint stays).
    fn parse(value: &'a str) -> Option<Paint<'a>> {
        Some(match svgtypes::Paint::from_str(value).ok()? {
            svgtypes::Paint::None | svgtypes::Paint::ContextFill | svgtypes::Paint::ContextStroke => Paint::Plain(Plain::None),
            svgtypes::Paint::Inherit => return None,
            svgtypes::Paint::CurrentColor => Paint::Plain(Plain::CurrentColor),
            svgtypes::Paint::Color(color) => Paint::Plain(colour(color)),
            svgtypes::Paint::FuncIRI(id, fallback) => Paint::Server {
                id,
                fallback: fallback.map(|f| match f {
                    PaintFallback::None => Plain::None,
                    PaintFallback::CurrentColor => Plain::CurrentColor,
                    PaintFallback::Color(color) => colour(color),
                }),
            },
        })
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
    /// `paint-order` puts the stroke before the fill.
    pub stroke_first: bool,
    /// A marker property names a marker: arrowheads and the like, drawn on top of the outline.
    pub markers: bool,
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
            markers: false,
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
        if let Some(paint) = get("fill").and_then(Paint::parse) {
            style.fill = paint;
        }
        if let Some(paint) = get("stroke").and_then(Paint::parse) {
            style.stroke = paint;
        }
        if let Some(color) = get("color").and_then(|v| Color::from_str(v).ok()) {
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
        for property in ["marker", "marker-start", "marker-mid", "marker-end"] {
            if let Some(value) = get(property) {
                style.markers = !value.is_empty() && !value.eq_ignore_ascii_case("none");
            }
        }
        Some(style)
    }

    /// Whether anything this element paints can be seen.
    pub fn shown(&self) -> bool {
        self.visible && !self.transparent
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
            assert!(s.stroke_first && s.markers);
        });
        with_style(r#"<svg paint-order="markers"><g marker-end="url(#m)"><path id="p" marker-end="none"/></g></svg>"#, "p", |s| {
            let s = s.unwrap();
            assert!(!s.stroke_first && !s.markers);
        });
    }
}
