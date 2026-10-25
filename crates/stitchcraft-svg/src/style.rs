//! Presentation properties: the part of CSS that decides whether an SVG element paints, in which
//! colours, and how its stroke is shaped.
//!
//! SVG elements take their colours and visibility from properties, written as presentation attributes
//! (`fill="red"`), in the rules of a style sheet ([`crate::sheet`]) or in the `style` attribute
//! (`style="fill:red"`), each winning over the one before, and `!important` over all three. Most
//! properties inherit, so the reader hands a [`Style`] from each element to its children, and each element
//! changes what it sets. A paint, colour, stroke width, join or miter limit that does not parse is
//! ignored, as CSS ignores an invalid declaration: the next declaration in that order wins, so StitchCraft
//! sees what an SVG viewer shows. The reader names a paint, stroke width, join or miter limit that the
//! element sets and that never parses ([`Declaration::unreadable`]). `display`, `visibility`, `opacity`,
//! `fill-opacity`, `stroke-opacity`, `fill-rule` and `paint-order` take the winning declaration as it is
//! written, as inkex takes them: a value that does not read there changes nothing, where a viewer lets the
//! declaration before it win.
//!
//! Colours are read as editors write them: keywords in any case (`currentcolor`), and an ICC colour
//! after the sRGB one (`#cd853f icc-color(…)`, from Inkscape's colour-managed picker) is left for the
//! sRGB one, as viewers without colour management do.
//!
//! The stroke's width and join matter to a satin column drawn as one path, its centre line: they give
//! the column its width and its corners ([`Style::join`]). Its cap matters to a stroke StitchCraft digitizes
//! automatically, whose stitches cover the line as it is drawn, caps included (`REQ-AUTO-003`).
//!
//! Read here: `fill`, `stroke`, `color` (for `currentColor`), `fill-rule`, `opacity`, `fill-opacity`,
//! `stroke-opacity`, `stroke-width`, `stroke-linejoin`, `stroke-miterlimit`, `stroke-linecap`, `visibility`, `display`,
//! `paint-order` and the marker properties, each of the three markers on its own.

use std::str::FromStr;

use roxmltree::Node;
use stitchcraft_core::{Exhausted, Meter};
use stitchcraft_engine::design::{Cap, FillRule, Join};
use stitchcraft_plan::Rgb;
use svgtypes::{Color, Length, LengthUnit, Number, PaintOrder, PaintOrderKind};

/// A `name: value` declaration as written, in a `style` attribute or a style sheet's rule.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Written<'a> {
    /// The property's name, as written.
    pub name: &'a str,
    /// Its value, without `!important`.
    pub value: &'a str,
    /// Whether it is marked `!important`.
    pub important: bool,
}

/// The properties one element sets itself: its `style` attribute's declarations and those of the style
/// sheet's rules that select it, in the order CSS gives them, then its presentation attributes.
pub struct Declared<'a, 'input> {
    node: Node<'a, 'input>,
    /// Every declaration of the `style` attribute and of the rules, the one that wins first: the important
    /// ones before the others, and of each, the `style` attribute's before the rules', the later before the
    /// earlier, and the more specific rule's before the less.
    cascade: Vec<(&'a str, &'a str)>,
}

impl<'a, 'input> Declared<'a, 'input> {
    /// What `node` declares, with the declarations of the style sheet's rules that select it, `rules`, in
    /// the order they apply: the most specific last, and of rules as specific, the later last. One unit of
    /// `meter` per declaration, the `style` attribute's and the rules'.
    pub fn of(node: Node<'a, 'input>, rules: &[&[Written<'a>]], meter: &mut Meter) -> Result<Self, Exhausted> {
        let inline = node.attribute("style").map(declarations).unwrap_or_default();
        let count = inline.len() + rules.iter().map(|rule| rule.len()).sum::<usize>();
        meter.charge(u64::try_from(count).unwrap_or(u64::MAX))?;
        let mut cascade = Vec::with_capacity(count);
        for important in [true, false] {
            let from_rules = rules.iter().rev().flat_map(|rule| rule.iter().rev());
            let all = inline.iter().rev().chain(from_rules).filter(|w| w.important == important);
            cascade.extend(all.map(|w| (w.name, w.value)));
        }
        Ok(Declared { node, cascade })
    }

    /// The declarations of `property`, the one that wins first; presentation attributes are not looked at.
    fn declared<'s>(&'s self, property: &'s str) -> impl Iterator<Item = &'a str> + 's {
        self.cascade.iter().filter(move |(name, _)| name.eq_ignore_ascii_case(property)).map(|(_, value)| *value)
    }

    /// The value `node` gives `property`: the declaration that wins, else the presentation attribute.
    pub fn get(&self, property: &str) -> Option<&'a str> {
        self.declared(property).next().or_else(|| self.node.attribute(property)).map(str::trim)
    }

    /// Whether `property` is set to something other than `none`: a clip path, mask or filter.
    pub fn uses(&self, property: &str) -> bool {
        self.get(property).is_some_and(names_something)
    }

    /// What the element declares for the paint `property` (`fill` or `stroke`).
    pub fn paint(&self, property: &str) -> Declaration<'a, Paint<'a>> {
        self.value(property, Paint::parse)
    }

    /// What the element declares for `stroke-width`: a length or percentage of 0 or more.
    pub fn stroke_width(&self) -> Declaration<'a, Length> {
        self.value("stroke-width", |v| Length::from_str(v).ok().filter(|l| l.number >= 0.0 && l.number.is_finite()))
    }

    /// What the element declares for `stroke-linejoin`: a join SVG names.
    pub fn line_join(&self) -> Declaration<'a, LineJoin> {
        self.value("stroke-linejoin", LineJoin::parse)
    }

    /// What the element declares for `stroke-linecap`: a cap SVG names, in any case.
    pub fn line_cap(&self) -> Declaration<'a, Cap> {
        self.value("stroke-linecap", |v| {
            [("butt", Cap::Butt), ("round", Cap::Round), ("square", Cap::Square)]
                .into_iter()
                .find(|(name, _)| v.eq_ignore_ascii_case(name))
                .map(|(_, cap)| cap)
        })
    }

    /// What the element declares for `stroke-miterlimit`: a number of 1 or more.
    pub fn miter_limit(&self) -> Declaration<'a, f64> {
        self.value("stroke-miterlimit", |v| Number::from_str(v).ok().map(|n| n.0).filter(|n| *n >= 1.0 && n.is_finite()))
    }

    /// The stroke's shape as the element declares it where it does not read: each property, the value that
    /// would win, and what a value must be.
    pub fn unreadable_stroke(&self) -> impl Iterator<Item = (&'static str, &'a str, &'static str)> {
        let declared = [
            ("stroke-width", self.stroke_width().unreadable(), "a length of 0 or more"),
            ("stroke-linejoin", self.line_join().unreadable(), "a join SVG names"),
            ("stroke-miterlimit", self.miter_limit().unreadable(), "a number of 1 or more"),
        ];
        declared.into_iter().filter_map(|(property, value, what)| Some((property, value?, what)))
    }

    /// What the element declares for `property`, read with `parse`: the first declaration that reads, in
    /// the order they win, else the presentation attribute if it reads. `inherit` keeps the inherited value.
    fn value<T>(&self, property: &str, parse: impl Fn(&'a str) -> Option<T>) -> Declaration<'a, T> {
        let mut unread = None;
        for value in self.declared(property).chain(self.node.attribute(property)).map(str::trim) {
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

    /// Every value the `style` attribute itself gives `property`, in order; style sheets and presentation
    /// attributes are not looked at. It reads the attribute again, which [`Declared::of`] paid for.
    pub fn in_style(&self, property: &str) -> Vec<&'a str> {
        let inline = self.node.attribute("style").map(declarations).unwrap_or_default();
        inline.into_iter().filter(|w| w.name.eq_ignore_ascii_case(property)).map(|w| w.value).collect()
    }

    /// The values `node` gives `marker-start`, `marker-mid` and `marker-end`. A `marker` declaration is a
    /// shorthand that sets all three where it stands in the cascade, and the declaration that wins sets
    /// each; a marker no declaration sets comes from its presentation attribute, or else from a `marker`
    /// attribute.
    pub fn markers(&self) -> [Option<&'a str>; 3] {
        MARKERS.map(|property| {
            let declared = self.cascade.iter().find(|(name, _)| name.eq_ignore_ascii_case(property) || name.eq_ignore_ascii_case("marker"));
            declared.map(|(_, value)| *value).or_else(|| self.node.attribute(property).or_else(|| self.node.attribute("marker"))).map(str::trim)
        })
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

impl<'a, T> Declaration<'a, T> {
    /// The value that does not read, when the element declares none that does.
    pub fn unreadable(self) -> Option<&'a str> {
        match self {
            Declaration::Unreadable(value) => Some(value),
            Declaration::Inherited | Declaration::Set(_) => None,
        }
    }
}

/// The joins `stroke-linejoin` names.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LineJoin {
    /// `miter`.
    Miter,
    /// `miter-clip` (SVG 2).
    MiterClip,
    /// `round`.
    Round,
    /// `bevel`.
    Bevel,
    /// `arcs` (SVG 2).
    Arcs,
}

impl LineJoin {
    /// The join `value` names, in any case.
    fn parse(value: &str) -> Option<LineJoin> {
        let names = [
            ("miter", LineJoin::Miter),
            ("miter-clip", LineJoin::MiterClip),
            ("round", LineJoin::Round),
            ("bevel", LineJoin::Bevel),
            ("arcs", LineJoin::Arcs),
        ];
        names.into_iter().find(|(name, _)| value.eq_ignore_ascii_case(name)).map(|(_, join)| join)
    }
}

/// The marker properties, one per place on a path.
const MARKERS: [&str; 3] = ["marker-start", "marker-mid", "marker-end"];

/// Whether a clip path, mask, filter or marker value names one: it is set to something other than `none`.
fn names_something(value: &str) -> bool {
    !value.is_empty() && !value.eq_ignore_ascii_case("none")
}

/// The `name: value` declarations of a `style` attribute or a rule's block, in order. It splits on `;`
/// outside quotes, parentheses and comments, takes `!important` off the value, skips a declaration without
/// a colon, and trims the comments and spaces around each name and value.
pub fn declarations<'s>(text: &'s str) -> Vec<Written<'s>> {
    let mut found = Vec::new();
    let mut add = |part: &'s str| {
        let Some((name, value)) = css_trim(part).split_once(':') else { return };
        let value = css_trim(value);
        let (value, important) = match value.rsplit_once('!') {
            Some((before, flag)) if css_trim(flag).eq_ignore_ascii_case("important") => (css_trim(before), true),
            _ => (value, false),
        };
        found.push(Written { name: css_trim(name), value, important });
    };
    let mut start = 0;
    for at in split_points(text, b';') {
        // `;` is ASCII, so both ends are character boundaries.
        add(text.get(start..at).unwrap_or_default());
        start = at + 1;
    }
    add(text.get(start..).unwrap_or_default());
    found
}

/// Where `text` has `separator` outside quotes, parentheses, brackets and comments: the byte offsets, in
/// order. A quote, parenthesis or comment left open runs to the end.
pub fn split_points(text: &str, separator: u8) -> Vec<usize> {
    let bytes = text.as_bytes();
    let (mut found, mut depth, mut i) = (Vec::new(), 0_u32, 0);
    while let Some(&byte) = bytes.get(i) {
        match byte {
            b'"' | b'\'' => i = string_end(bytes, i),
            b'/' if bytes.get(i + 1) == Some(&b'*') => i = comment_end(bytes, i),
            b'(' | b'[' => depth += 1,
            b')' | b']' => depth = depth.saturating_sub(1),
            _ if byte == separator && depth == 0 => found.push(i),
            _ => {}
        }
        i += 1;
    }
    found
}

/// The offset of the last byte of the string that opens at `at` (its closing quote, or the text's end):
/// a backslash escapes the byte after it.
pub fn string_end(bytes: &[u8], at: usize) -> usize {
    let quote = bytes.get(at).copied();
    let mut i = at + 1;
    while let Some(&byte) = bytes.get(i) {
        if byte == b'\\' {
            i += 1;
        } else if Some(byte) == quote {
            return i;
        }
        i += 1;
    }
    bytes.len().saturating_sub(1).max(at)
}

/// The offset of the last byte of the comment that opens at `at` (the `/` of its `*/`, or the text's end).
pub fn comment_end(bytes: &[u8], at: usize) -> usize {
    // The `*` of the `*/` comes after the `/*`: `/*/` opens a comment and does not close it.
    let mut i = at + 3;
    while let Some(&byte) = bytes.get(i) {
        if byte == b'/' && bytes.get(i - 1) == Some(&b'*') {
            return i;
        }
        i += 1;
    }
    bytes.len().saturating_sub(1).max(at)
}

/// `text` without the spaces and comments around it; those inside stay.
pub fn css_trim(text: &str) -> &str {
    let bytes = text.as_bytes();
    let mut start = 0;
    while let Some(&byte) = bytes.get(start) {
        if byte.is_ascii_whitespace() {
            start += 1;
        } else if byte == b'/' && bytes.get(start + 1) == Some(&b'*') {
            start = comment_end(bytes, start) + 1;
        } else {
            break;
        }
    }
    // The end is after the last byte outside a comment that is not a space.
    let (mut end, mut i) = (start, start);
    while let Some(&byte) = bytes.get(i) {
        if byte == b'/' && bytes.get(i + 1) == Some(&b'*') {
            i = comment_end(bytes, i) + 1;
            continue;
        }
        if byte == b'"' || byte == b'\'' {
            i = string_end(bytes, i);
        }
        if !byte.is_ascii_whitespace() {
            end = i + 1;
        }
        i += 1;
    }
    text.get(start..end).unwrap_or_default()
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
    /// The `stroke-width` property, applied in the user units of the shape it strokes.
    pub stroke_width: Length,
    /// The `stroke-linejoin` property, when anything sets it.
    pub line_join: Option<LineJoin>,
    /// The `stroke-miterlimit` property.
    pub miter_limit: f64,
    /// The `stroke-linecap` property.
    pub line_cap: Cap,
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
            stroke_width: Length::new_number(1.0),
            line_join: None,
            miter_limit: 4.0,
            line_cap: Cap::Butt,
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
        if let Declaration::Set(width) = declared.stroke_width() {
            style.stroke_width = width;
        }
        if let Declaration::Set(join) = declared.line_join() {
            style.line_join = Some(join);
        }
        if let Declaration::Set(cap) = declared.line_cap() {
            style.line_cap = cap;
        }
        if let Declaration::Set(limit) = declared.miter_limit() {
            style.miter_limit = limit;
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

    /// The join at the stroke's corners, as Ink/Stitch reads it: round and bevel as they say, a miter
    /// limited by `stroke-miterlimit` when the join is `miter`, and otherwise a miter limited at 5. SVG 2's
    /// `miter-clip` and `arcs` count as otherwise.
    pub fn join(&self) -> Join {
        match self.line_join {
            Some(LineJoin::Miter) => Join::Miter { limit: self.miter_limit },
            Some(LineJoin::Round) => Join::Round,
            Some(LineJoin::Bevel) => Join::Bevel,
            Some(LineJoin::MiterClip | LineJoin::Arcs) | None => Join::UNSET,
        }
    }

    /// The join at the stroke's corners as SVG draws it, for a line digitized automatically, whose stitches
    /// follow the drawing (`REQ-AUTO-004`): round and bevel as they say, and otherwise SVG's initial join, a
    /// miter limited by `stroke-miterlimit`. SVG 2's `miter-clip` and `arcs`, which few viewers draw, are
    /// miters too.
    pub fn painted_join(&self) -> Join {
        match self.line_join {
            Some(LineJoin::Round) => Join::Round,
            Some(LineJoin::Bevel) => Join::Bevel,
            Some(LineJoin::Miter | LineJoin::MiterClip | LineJoin::Arcs) | None => Join::Miter { limit: self.miter_limit },
        }
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

    /// What `node` declares, with the declarations of `rules`, read within the default budget.
    fn declared<'a, 'input>(node: Node<'a, 'input>, rules: &[&[Written<'a>]]) -> Declared<'a, 'input> {
        Declared::of(node, rules, &mut stitchcraft_core::Budget::DEFAULT.meter()).unwrap()
    }

    /// `check` on the style of the element with id `id` in `svg`, inherited through every ancestor.
    fn with_style(svg: &str, id: &str, check: impl FnOnce(Option<Style<'_>>)) {
        let doc = Document::parse(svg).unwrap();
        let node = doc.descendants().find(|n| n.attribute("id") == Some(id)).unwrap();
        let mut chain: Vec<_> = node.ancestors().filter(roxmltree::Node::is_element).collect();
        chain.reverse();
        check(chain.iter().try_fold(Style::default(), |style, n| style.child(&declared(*n, &[]))));
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
        let paint = |id| declared(doc.descendants().find(|n| n.attribute("id") == Some(id)).unwrap(), &[]).paint("fill");
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
        // The last declaration wins; quotes, parentheses and comments do not split.
        let pairs = |text| declarations(text).iter().map(|w| (w.name, w.value)).collect::<Vec<_>>();
        assert_eq!(pairs("fill:red;fill:url(#a;b) blue;font-family:'a;b'"), [("fill", "red"), ("fill", "url(#a;b) blue"), ("font-family", "'a;b'")]);
        assert_eq!(pairs(" ; nothing ; :x"), [("", "x")]);
        assert_eq!(pairs("/* a: b; */ fill /* c */ : /* d */ red /* e; */; stroke:blue"), [("fill", "red"), ("stroke", "blue")]);
        assert_eq!(pairs("font: 12px/2 serif; fill: red"), [("font", "12px/2 serif"), ("fill", "red")], "a `/` alone starts no comment");
        // A quote ends where it closes, an escaped quote does not close it, and a `!` that is not
        // `!important` stays in the value.
        assert_eq!(pairs(r#"font-family:'a;b';fill:red"#), [("font-family", "'a;b'"), ("fill", "red")]);
        assert_eq!(pairs(r#"font-family:"a\";b";fill:red"#), [("font-family", r#""a\";b""#), ("fill", "red")]);
        let flags = |text| declarations(text).iter().map(|w| (w.value, w.important)).collect::<Vec<_>>();
        assert_eq!(flags("x: a!b; y: c ! IMPORTANT; z: d!important"), [("a!b", false), ("c", true), ("d", true)]);
        // Something left open runs to the end.
        assert_eq!(pairs("fill:red /* open"), [("fill", "red")]);
        assert_eq!(pairs("font-family:'open; fill:red"), [("font-family", "'open; fill:red")]);
    }

    #[test]
    fn comments_are_trimmed_and_nothing_else() {
        assert_eq!(css_trim(" /* a */ x /* b */ y /* c */ "), "x /* b */ y");
        // A comment ends at the first `*/` after its `/*`, whatever it holds.
        assert_eq!(css_trim("/* a/b * c */x"), "x");
        assert_eq!(css_trim("/*/ x */y"), "y", "`/*/` opens a comment and does not close it");
        // A `/` or a `*` alone is no comment, and a comment's opening in a string is none either.
        for kept in ["/x", "a*b", "1/2 x", "'/*' x", r#""a /* b" c"#] {
            assert_eq!(css_trim(kept), kept);
        }
        assert_eq!(css_trim("x /* open"), "x");
        assert_eq!(css_trim("/* open"), "");
    }

    #[test]
    fn important_declarations_win_and_the_style_attribute_over_rules() {
        let doc = Document::parse(r#"<svg><path id="p" fill="red" style="fill:blue;stroke:lime !important;marker:none"/></svg>"#).unwrap();
        let node = doc.descendants().find(|n| n.attribute("id") == Some("p")).unwrap();
        // The rules in the order they apply: the second wins over the first.
        let first = declarations("fill:#000001;stroke:#000002 !important;marker-end:url(#m)");
        let second = declarations("fill:#000003 !important;stroke:#000004 !important");
        let both = declared(node, &[&first, &second]);
        assert_eq!(both.paint("fill"), Declaration::Set(Paint::Plain(Plain::Color(Rgb::new(0, 0, 3)))), "important over the style attribute");
        assert_eq!(both.get("stroke"), Some("lime"), "an important style attribute over important rules");
        assert_eq!(both.markers(), [Some("none"), Some("none"), Some("none")], "the style attribute's shorthand over the rule");
        assert_eq!(both.in_style("stroke"), ["lime"], "only the style attribute");
        let one = declared(node, &[&first]);
        assert_eq!(one.get("fill"), Some("blue"));
        assert_eq!(one.get("stroke"), Some("lime"));
        // A unit for each declaration: 3 in the style attribute, 3 and 2 in the rules.
        let mut meter = stitchcraft_core::Budget { max_stitches: 1, max_work: 8 }.meter();
        assert!(Declared::of(node, &[&first, &second], &mut meter).is_ok() && meter.work_left() == 0);
        let mut short = stitchcraft_core::Budget { max_stitches: 1, max_work: 7 }.meter();
        assert!(Declared::of(node, &[&first, &second], &mut short).is_err());
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
