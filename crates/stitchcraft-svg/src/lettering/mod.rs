//! Lettering: text set in an embroidery font (roadmap step M14.2, `docs/src/design/fonts.md` › Lettering,
//! `REQ-FONT-009` to `REQ-FONT-014`): the glyphs the text names, each placed, scaled and joined into one
//! design that every host plans as it plans any. M14.3 added the path a text is set along, the lines sewn
//! back and forth, the trims and the sorted colours (`REQ-FONT-015` to `REQ-FONT-019`).
//!
//! **Why here.** The font reader gives each glyph as a design with its box and its baseline
//! ([`crate::font`]), and a font's text is routed by the engine's routing ([`stitchcraft_engine::route`]).
//! Lettering needs both and nothing else, so it lies beside the reader, where the command line and, from
//! M14.4, the VectorCraft plug-in both find it.
//!
//! **How** ([`set`]):
//!
//! 1. The lines are laid out (the module `layout`), every other line in the glyphs of the direction back
//!    when the text sews back and forth: a glyph for each character, or for several where the font draws
//!    them as one, placed by the font's advances and kerning and the spacing asked for, and each line
//!    aligned.
//! 2. The scale asked for is kept within the heights the font sews at (`SC-W1106`), and every glyph's
//!    elements are moved to their place and scaled about the first line's start, with the widths of their
//!    strokes. Their settings stay as the font's author set them. Along a path (the module `along`), each
//!    glyph is moved to its seat on the path and turned the way the path runs under it.
//! 3. In a font whose `auto_satin` is on, the engine's routing joins the text's satin columns and strokes in
//!    their order and without trims, as Ink/Stitch's lettering does. Each piece of the route sews at the end
//!    of the glyph its element was in.
//! 4. The trims asked for are added, and the colours of a font that sorts are sorted (the module `order`).
//!
//! **What waits.** The 4 forms of an Arabic letter aren't chosen (`layout`).
//!
//! **Never crashes, bounded.** Any text and any settings give a design or a coded error. One meter of the
//! budget counts the layout and the placing: a unit per character, per glyph name tried for it, and per
//! element and point placed. Routing has the budget again, as a routing filter has.

mod along;
mod layout;
pub mod objects;
mod order;
pub mod pictures;

use std::collections::{BTreeMap, BTreeSet};

use stitchcraft_core::units::{Mm, Point};
use stitchcraft_core::{Budget, Code, Diagnostic, ElementId, Fix, Meter, Severity};
use stitchcraft_engine::design::{Clip, ClipShape, Design, DesignSettings, Element, Shape};
use stitchcraft_engine::route::{Options, Route, Tool, route};

use crate::font::{Direction, Font, Glyph};
pub use along::{Along, TextPosition, VerticalPosition};
use along::{Boxed, Seat, Track};
use layout::{Laid, Spot};
use order::Sewn;
pub use order::{ColourSort, Trim};

/// How the lines of a text are aligned: Ink/Stitch's `text_align`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Align {
    /// Each line starts at the left.
    #[default]
    Left,
    /// Each line is centred on the first line's start.
    Centre,
    /// Each line ends at the first line's start.
    Right,
    /// Each line's words are spread as wide as the widest line.
    Block,
    /// As [`Align::Block`], and the letters of a line of one word are spread.
    LetterSpacing,
}

impl Align {
    /// Every alignment, in the order Ink/Stitch lists them.
    pub const ALL: [Align; 5] = [Align::Left, Align::Centre, Align::Right, Align::Block, Align::LetterSpacing];

    /// Its name, as Ink/Stitch's batch lettering names it.
    pub const fn id(self) -> &'static str {
        match self {
            Align::Left => "left",
            Align::Centre => "center",
            Align::Right => "right",
            Align::Block => "block",
            Align::LetterSpacing => "letterspacing",
        }
    }

    /// The alignment `text` names.
    pub fn named(text: &str) -> Option<Align> {
        Align::ALL.into_iter().find(|align| align.id() == text)
    }
}

/// How a text is set: Ink/Stitch's lettering settings, by its names.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Lettering {
    /// `scale`: the size of the text, in percent of the font's own.
    pub scale: f64,
    /// `letter_spacing`: millimetres added between the letters of a word, before scaling.
    pub letter_spacing: f64,
    /// `word_spacing`: millimetres added between words, before scaling.
    pub word_spacing: f64,
    /// `line_height`: millimetres added between lines, before scaling.
    pub line_height: f64,
    /// `text_align`: how the lines are aligned.
    pub align: Align,
    /// `back_and_forth`: whether every other line sews in the direction back, in a font that draws one.
    pub back_and_forth: bool,
    /// `trim`: after which glyphs a trim is added.
    pub trim: Trim,
    /// `color_sort`: which elements of a font that sorts its colours sew sorted.
    pub color_sort: ColourSort,
}

impl Default for Lettering {
    fn default() -> Self {
        Lettering {
            scale: 100.0,
            letter_spacing: 0.0,
            word_spacing: 0.0,
            line_height: 0.0,
            align: Align::Left,
            back_and_forth: true,
            trim: Trim::Off,
            color_sort: ColourSort::Off,
        }
    }
}

impl Lettering {
    /// Takes the setting `name` from `value`, written as [`SETTINGS`] says each is: whether it is one of
    /// lettering's settings of a text off a path and `value` reads as it. A host that keeps the settings by
    /// name, as a VectorCraft effect's record does, reads them with it.
    pub fn take(&mut self, name: &str, value: &str) -> bool {
        let number = || value.trim().parse::<f64>().ok().filter(|number| number.is_finite());
        match name {
            "scale" => number().map(|scale| self.scale = scale).is_some(),
            "letter_spacing" => number().map(|mm| self.letter_spacing = mm).is_some(),
            "word_spacing" => number().map(|mm| self.word_spacing = mm).is_some(),
            "line_height" => number().map(|mm| self.line_height = mm).is_some(),
            "text_align" => Align::named(value).map(|align| self.align = align).is_some(),
            "back_and_forth" => value.parse::<bool>().map(|on| self.back_and_forth = on).is_ok(),
            "trim" => Trim::named(value).map(|trim| self.trim = trim).is_some(),
            "color_sort" => ColourSort::named(value).map(|sort| self.color_sort = sort).is_some(),
            _ => false,
        }
    }
}

/// A setting of lettering, as a host's help describes it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Setting {
    /// Its name in Ink/Stitch's lettering.
    pub name: &'static str,
    /// What it takes.
    pub takes: &'static str,
    /// Its default, as text.
    pub default: &'static str,
    /// What it does, as a sentence of help.
    pub does: &'static str,
}

/// `scale`.
pub const SCALE: Setting = Setting {
    name: "scale",
    takes: "PERCENT",
    default: "100",
    does: "The size of the text, in percent of the font's own size. A scale outside the font's `min_scale` to `max_scale` is set at the \
           nearest the font sews at, with a warning",
};

/// `letter_spacing`.
pub const LETTER_SPACING: Setting = Setting {
    name: "letter_spacing",
    takes: "MM",
    default: "0",
    does: "Millimetres added between the letters of a word, or taken away when negative, at a scale of 100 percent",
};

/// `word_spacing`.
pub const WORD_SPACING: Setting = Setting {
    name: "word_spacing",
    takes: "MM",
    default: "0",
    does: "Millimetres added between words, or taken away when negative, at a scale of 100 percent",
};

/// `line_height`.
pub const LINE_HEIGHT: Setting = Setting {
    name: "line_height",
    takes: "MM",
    default: "0",
    does: "Millimetres added between lines, or taken away when negative, at a scale of 100 percent",
};

/// `text_align`.
pub const TEXT_ALIGN: Setting = Setting {
    name: "text_align",
    takes: "left|center|right|block|letterspacing",
    default: "left",
    does: "How the lines are aligned: from the left, centred, to the right, their words spread as wide as the widest line (block), or \
           that with the letters of a one-word line spread too (letterspacing)",
};

/// `back_and_forth`.
pub const BACK_AND_FORTH: Setting = Setting {
    name: "back_and_forth",
    takes: "true|false",
    default: "true",
    does: "Whether every other line sews in the direction back, so that a line starts where the line before it ended. It needs a font            that draws its glyphs for both directions, and a text in another font sews every line the same way, with a warning",
};

/// `trim`.
pub const TRIM: Setting = Setting {
    name: "trim",
    takes: "off|line|word|glyph",
    default: "off",
    does: "Where the thread is trimmed: nowhere but where the font's glyphs trim it (off), after each line, after each word, or after            each letter (glyph)",
};

/// `color_sort`.
pub const COLOR_SORT: Setting = Setting {
    name: "color_sort",
    takes: "off|all|line|word",
    default: "off",
    does: "Which letters of a multicolour font sew one colour after another, in the order the font's author numbered its colours:            none (off), the whole text's, each line's or each word's. A font that doesn't sort its colours sews as it is, with a warning",
};

/// `text_position`.
pub const TEXT_POSITION: Setting = Setting {
    name: "text_position",
    takes: "left|center|right|stretch",
    default: "left",
    does: "Where the text lies along its path: from the path's start (left), centred on it, up to its end (right), or spread from its            start to its end (stretch)",
};

/// `vertical_text_position`.
pub const VERTICAL_TEXT_POSITION: Setting = Setting {
    name: "vertical_text_position",
    takes: "top|baseline|bottom",
    default: "top",
    does: "Which part of the text lies on its path: its lowest point, so that it stands on the path (top), its first line's baseline,            or its highest point, so that it hangs from the path (bottom)",
};

/// Every setting of lettering: those Ink/Stitch's batch lettering lists, in its order, then the one its lettering window has.
pub const SETTINGS: [Setting; 10] =
    [SCALE, COLOR_SORT, TRIM, TEXT_ALIGN, LETTER_SPACING, WORD_SPACING, LINE_HEIGHT, TEXT_POSITION, VERTICAL_TEXT_POSITION, BACK_AND_FORTH];

/// A glyph of a text set, where it lies.
#[derive(Clone, Debug, PartialEq)]
pub struct Placed {
    /// The glyph's name.
    pub name: String,
    /// The line it is in, from 0.
    pub line: usize,
    /// The word of the line it is in, from 0, in the order the words are laid out.
    pub word: usize,
    /// Its box's left edge, in millimetres from the first line's start. Along a path, the x of the point
    /// where its box's left edge meets its baseline, as the path gives it.
    pub left: f64,
    /// Its box's width, in millimetres.
    pub width: f64,
    /// Its baseline, in millimetres down from the first line's. Along a path, the y of the point where its
    /// box's left edge meets its baseline.
    pub baseline: f64,
    /// How far it is turned about that point, in degrees, clockwise as a picture shows it: 0 off a path.
    pub angle: f64,
}

/// A text set in a font.
#[derive(Clone, Debug, PartialEq)]
pub struct Set {
    /// The text as a design, its elements in the order they sew, in millimetres from the first line's start
    /// on its baseline.
    pub design: Design,
    /// Its glyphs, in the order they are laid out.
    pub glyphs: Vec<Placed>,
    /// The scale it is set at, in percent: the one asked for, within the font's range.
    pub scale: f64,
    /// What setting it said: of the scale, of characters the font lacks, of the glyphs sewn and of the route.
    pub warnings: Vec<Diagnostic>,
}

/// `text` set in `font` as `how` says, within `budget`. Refused with `SC-E1108` when no glyph is set, or when
/// the text can't be placed, and with `SC-E0004` when setting it needs more work than `budget` allows.
pub fn set(font: &Font, text: &str, how: &Lettering, budget: &Budget) -> Result<Set, Diagnostic> {
    set_on(font, text, how, None, budget)
}

/// `text` set in `font` as `how` says along the path of `along`, within `budget`: each glyph sits on the
/// path and is turned the way the path runs under it, and the text lies in the path's millimetres. Refused
/// as [`set`] refuses, and with `SC-E1108` when the path has no length.
pub fn set_along(font: &Font, text: &str, how: &Lettering, along: &Along, budget: &Budget) -> Result<Set, Diagnostic> {
    set_on(font, text, how, Some(along), budget)
}

/// [`set`], or [`set_along`] with a path.
fn set_on(font: &Font, text: &str, how: &Lettering, along: Option<&Along>, budget: &Budget) -> Result<Set, Diagnostic> {
    let mut meter = budget.meter();
    let Some(variant) = font.default_variant() else {
        return Err(unset(format!("The font `{}` has no glyphs.", font.folder)));
    };
    // The direction back, in a font that draws it and lets its lines sew back and forth.
    let back = font.variant(variant.direction.reversed()).filter(|_| font.settings.reversible);
    let odd = back.filter(|_| how.back_and_forth).unwrap_or(variant);
    let laid = layout::lay(font, [variant, odd], text, how, &mut meter).map_err(|e| e.diagnostic(budget, None))?;
    if laid.spots.is_empty() {
        return Err(unset(format!("None of the text's characters sews in the font `{}`: the text is empty, or only spaces.", font.folder)));
    }
    let mut warnings = Vec::new();
    let scale = scaled(font, how.scale, &mut warnings);
    if !laid.missing.is_empty() {
        warnings.push(missing(font, &laid.missing));
    }
    if how.back_and_forth && back.is_none() && laid.spots.iter().any(|spot| spot.line % 2 == 1) {
        warnings.push(one_way(font));
    }
    let sorts = how.color_sort != ColourSort::Off && font.settings.sortable;
    if how.color_sort != ColourSort::Off && !sorts {
        warnings.push(unsorted(font));
    }
    let used: BTreeMap<&str, &Glyph> = laid.spots.iter().map(|spot| (spot.glyph.name.as_str(), spot.glyph)).collect();
    warnings.extend(used.values().flat_map(|glyph| glyph.warnings.iter().cloned()));
    let factor = scale / 100.0;
    let seated =
        along.map(|along| seated(along, &laid, factor, &mut meter, &mut warnings)).transpose().map_err(|e| e.unwrap_or_else(|| exhausted(budget)))?;
    let seat = |index: usize| seated.as_ref().and_then(|(seats, lift)| Some((seats.get(index)?, *lift)));
    let glyphs = laid
        .spots
        .iter()
        .enumerate()
        .map(|(index, spot)| {
            let (left, baseline, angle) = match seat(index) {
                None => (spot.left * factor, spot.baseline * factor, 0.0),
                Some((seat, lift)) => {
                    let (x, y) = seat.place(0.0, (spot.baseline - lift) * factor);
                    (x, y, seat.angle())
                }
            };
            Placed { name: spot.glyph.name.clone(), line: spot.line, word: spot.word, left, width: spot.glyph.width * factor, baseline, angle }
        })
        .collect();
    // An rtl direction's glyphs are drawn to sew from the right, so a line in them sews from its end.
    let mut order: Vec<usize> = Vec::new();
    for (line, direction) in &laid.lines {
        if *direction == Direction::RightToLeft { order.extend(line.clone().rev()) } else { order.extend(line.clone()) }
    }
    let mut elements = Vec::new();
    for (at, index) in order.into_iter().enumerate() {
        let Some(spot) = laid.spots.get(index) else { continue };
        for element in spot.glyph.design.elements() {
            let element = placed(element, at, spot, factor, seat(index), &mut meter).map_err(|e| e.unwrap_or_else(|| exhausted(budget)))?;
            elements.push(Sewn { glyph: at, line: spot.line, word: spot.word, element });
        }
    }
    let settings = DesignSettings::default();
    if font.settings.auto_satin {
        elements = routed(elements, &settings, budget, &mut warnings);
    }
    order::trim(&mut elements, how.trim, sorts, &mut meter).map_err(|_| exhausted(budget))?;
    let scope = if sorts { how.color_sort } else { ColourSort::Off };
    let elements = order::sort(elements, scope, &font.settings.combine_at_sort_indices, &mut meter).map_err(|_| exhausted(budget))?;
    // The colours' numbers are lettering's to read: the planner has no setting of that name.
    let sewn = elements.into_iter().map(|mut sewn| {
        sewn.element.params.remove(order::COLOR_SORT_INDEX);
        sewn.element
    });
    let design = Design::new(sewn.collect(), settings).map_err(|e| unset(e.message))?;
    Ok(Set { design, glyphs, scale, warnings })
}

/// The seat of each spot of `laid` on the path of `along`, at the scale `factor`, and how far below the
/// first line's baseline the part of the text that lies on the path is, before the scale. Says in `warnings`
/// when a line is longer than the path. Charges `meter` for the path, and a unit for each glyph and each of
/// its points. `None` as the error when the budget runs out.
fn seated(
    along: &Along,
    laid: &Laid,
    factor: f64,
    meter: &mut Meter,
    warnings: &mut Vec<Diagnostic>,
) -> Result<(Vec<Seat>, f64), Option<Diagnostic>> {
    let track = Track::of(&along.path, meter).map_err(|_| None)?;
    let track = track
        .ok_or_else(|| Some(unset("The path to set the text along has no length: it has no subpath, or its first doesn't move.".to_string())))?;
    // The text's highest and lowest points, from the first line's baseline.
    let (mut top, mut bottom) = (f64::INFINITY, f64::NEG_INFINITY);
    for spot in &laid.spots {
        meter.charge(1).map_err(|_| None)?;
        for element in spot.glyph.design.elements() {
            let path = element.shape.path();
            meter.charge(u64::try_from(path.points().count()).unwrap_or(u64::MAX)).map_err(|_| None)?;
            for point in path.points() {
                let down = point.y() - spot.glyph.baseline + spot.baseline;
                (top, bottom) = (top.min(down), bottom.max(down));
            }
        }
    }
    let lift = match along.vertical {
        VerticalPosition::Top => bottom,
        VerticalPosition::Baseline => 0.0,
        VerticalPosition::Bottom => top,
    };
    let mut seats = Vec::with_capacity(laid.spots.len());
    let mut longest: Option<f64> = None;
    for (line, _) in &laid.lines {
        let spots = laid.spots.get(line.clone()).unwrap_or_default();
        let left = spots.iter().map(|spot| spot.left).fold(f64::INFINITY, f64::min);
        let boxed: Vec<Boxed> =
            spots.iter().map(|spot| Boxed { left: (spot.left - left) * factor, width: spot.glyph.width * factor, word: spot.word }).collect();
        let (line_seats, past) = along::seats(&track, along.position, &boxed);
        seats.extend(line_seats);
        if past {
            let width = boxed.iter().map(|glyph| glyph.left + glyph.width).fold(0.0, f64::max);
            longest = Some(longest.map_or(width, |longest| longest.max(width)));
        }
    }
    if let Some(width) = longest {
        warnings.push(past_the_path(width, track.length(), along.path.subpaths.first().is_some_and(|first| first.closed), along.position));
    }
    Ok((seats, lift))
}

/// `SC-W1109`: a line `width` millimetres wide is set along a path `length` millimetres long.
fn past_the_path(width: f64, length: f64, closed: bool, position: TextPosition) -> Diagnostic {
    let sewn = if position == TextPosition::Stretch {
        "Its letters are pushed together to fit, and overlap."
    } else if closed {
        "It goes round the closed path again, over its own start."
    } else {
        "Past the path's end it goes on straight, the way the path ends."
    };
    let message = format!("A line of the text is {width:.1} mm long, and its path is {length:.1} mm long. {sewn}");
    Diagnostic::new(Code::TextLongerThanPath, message)
        .with_fix(Fix::Hint("Draw a longer path, set the text at a smaller scale, or write a shorter text.".to_string()))
}

/// `SC-W1110`: `font` can't sew its lines back and forth.
fn one_way(font: &Font) -> Diagnostic {
    let why = if font.settings.reversible { "draws its glyphs for one direction" } else { "says in its `font.json` that it isn't reversible" };
    let message = format!(
        "The font `{}` {why}, so the text's lines aren't sewn back and forth: every line sews the same way, and the thread jumps from \
         each line's end to the next line's start.",
        font.folder
    );
    Diagnostic::new(Code::LetteringSettingNotApplied, message)
        .with_fix(Fix::Hint("Choose a font that sews in both directions, or cut the thread between the lines after sewing.".to_string()))
}

/// `SC-W1110`: `font` doesn't sort its colours.
fn unsorted(font: &Font) -> Diagnostic {
    let message = format!(
        "The font `{}` doesn't sort its colours (`sortable` in its `font.json`), so each letter sews its colours before the next.",
        font.folder
    );
    Diagnostic::new(Code::LetteringSettingNotApplied, message)
        .with_fix(Fix::Hint("Leave the colour sort off, or choose a multicolour font that sorts.".to_string()))
}

/// The scale `asked` for, in percent, within the range `font` sews at, with a warning when it lay outside.
fn scaled(font: &Font, asked: f64, warnings: &mut Vec<Diagnostic>) -> f64 {
    let (least, most) = (font.settings.min_scale * 100.0, font.settings.max_scale * 100.0);
    let scale = if asked < least {
        least
    } else if asked > most {
        most
    } else {
        return asked;
    };
    let (low, high) = font.heights_mm();
    let heights = if font.settings.size > 0.0 { format!(" ({low:.1} to {high:.1} mm tall)") } else { String::new() };
    let message = format!(
        "The scale {asked} % is outside the range the font `{}` sews at, {least:.0} to {most:.0} %{heights}. The text is set at {scale:.0} %.",
        font.folder
    );
    let fix = "Choose a scale within the range, or a font drawn for the size you want.";
    warnings.push(Diagnostic::new(Code::LetteringScaleOutOfRange, message).with_fix(Fix::Hint(fix.to_string())));
    scale
}

/// `SC-W1107`: the characters `missing` from `font`, which sew as its default glyph.
fn missing(font: &Font, missing: &BTreeSet<char>) -> Diagnostic {
    let named: Vec<String> = missing.iter().map(|c| format!("`{c}` (U+{:04X})", u32::from(*c))).collect();
    let default = &font.settings.default_glyph;
    let sewn = match font.default_variant().and_then(|variant| variant.glyphs.get(default)) {
        Some(_) => format!("sews as the font's default glyph, `{default}`"),
        None => format!("is left as a space, as the font has no glyph for its default glyph, `{default}`"),
    };
    let message = format!("The font `{}` has no glyph for {}. Each {sewn}.", font.folder, named.join(", "));
    Diagnostic::new(Code::CharacterNotInFont, message)
        .with_fix(Fix::Hint("Choose a font that has the characters, or write the text without them.".to_string()))
}

/// `element` of the glyph at `spot`, the `at`-th sewn, in its place and scaled by `factor`, charging `meter`:
/// where the layout puts it, or on `seat` of a path, with how far below the first line's baseline the path
/// lies. `None` as the error when the budget runs out.
fn placed(
    element: &Element,
    at: usize,
    spot: &Spot,
    factor: f64,
    seat: Option<(&Seat, f64)>,
    meter: &mut Meter,
) -> Result<Element, Option<Diagnostic>> {
    let points = element.shape.path().points().count();
    meter.charge(1 + u64::try_from(points).unwrap_or(u64::MAX)).map_err(|_| None)?;
    let (dx, dy) = (spot.left - spot.glyph.left, spot.baseline - spot.glyph.baseline);
    let far = || Some(unset("A glyph can't be placed: the scale or a spacing is no number, or the text is longer than 10 m.".to_string()));
    let point = |p: Point| {
        let (x, y) = match seat {
            None => ((p.x() + dx) * factor, (p.y() + dy) * factor),
            Some((seat, lift)) => seat.place((p.x() - spot.glyph.left) * factor, (p.y() + dy - lift) * factor),
        };
        Point::new(x, y).map_err(|_| far())
    };
    let id = ElementId::new(format!("{at}-{}", element.id))
        .map_err(|_| Some(unset(format!("The id `{}` of an element of the glyph `{}` is too long to number.", element.id, spot.glyph.name))))?;
    let mut placed = moved(element, point)?;
    if let Shape::Stroke { width, .. } = &mut placed.shape {
        *width = Mm::new(width.get() * factor).map_err(|_| far())?;
    }
    Ok(Element { id, ..placed })
}

/// `element` with each of its points where `point` puts it: those of its path, of a fill's gradient, of its
/// hints and of the areas it is cut to. Its id, its thread, its settings and a stroke's width stay. A host
/// places a text set with it, as the VectorCraft adapter turns a text the way its text object is turned.
pub fn moved<E>(element: &Element, point: impl Fn(Point) -> Result<Point, E>) -> Result<Element, E> {
    let shape = match &element.shape {
        Shape::Stroke { path, width, join, cap } => Shape::Stroke { path: path.map(&point)?, width: *width, join: *join, cap: *cap },
        Shape::Fill { path, rule, gradient } => {
            Shape::Fill { path: path.map(&point)?, rule: *rule, gradient: gradient.as_ref().map(|gradient| gradient.map(&point)).transpose()? }
        }
    };
    let shapes =
        |clip: &Clip| clip.shapes.iter().map(|shape| Ok(ClipShape { path: shape.path.map(&point)?, rule: shape.rule })).collect::<Result<_, E>>();
    let clips = element.clips.iter().map(|clip| Ok(Clip { shapes: shapes(clip)? })).collect::<Result<_, E>>()?;
    Ok(Element { shape, hints: element.hints.map(&point)?, clips, ..element.clone() })
}

/// `elements` routed as Ink/Stitch's lettering routes a font whose `auto_satin` is on: its satin columns and
/// strokes in their order, without trims. Each piece sews at the end of its element's glyph. A text without a
/// satin column stays as it is, and so does one whose route fails, which `warnings` then says. What the route
/// only notes, such as a fill it leaves out, isn't said.
fn routed(elements: Vec<Sewn>, settings: &DesignSettings, budget: &Budget, warnings: &mut Vec<Diagnostic>) -> Vec<Sewn> {
    let how = Route { tool: Tool::Satins, options: Options { preserve_order: true, trim: false, ..Tool::Satins.defaults() }, start: None, end: None };
    let (glyphs, given): (Vec<(usize, usize, usize)>, Vec<Element>) =
        elements.into_iter().map(|sewn| ((sewn.glyph, sewn.line, sewn.word), sewn.element)).unzip();
    let route = route(&given, &how, settings, budget);
    warnings.extend(route.diagnostics.into_iter().filter(|said| said.code != Code::RouteNothing && said.severity() != Severity::Info));
    let replaced: BTreeSet<usize> = route.replaced.into_iter().collect();
    let sewn = |index: usize, element: Element| {
        let (glyph, line, word) = glyphs.get(index).copied().unwrap_or_default();
        Sewn { glyph, line, word, element }
    };
    let kept = given.into_iter().enumerate().filter(|(index, _)| !replaced.contains(index));
    let mut all: Vec<Sewn> = kept.map(|(index, element)| sewn(index, element)).collect();
    all.extend(route.pieces.into_iter().map(|piece| sewn(piece.from, piece.element)));
    all.sort_by_key(|sewn| sewn.glyph);
    all
}

/// `SC-E1108`: the text isn't set, as `message` says.
fn unset(message: String) -> Diagnostic {
    Diagnostic::new(Code::LetteringUnset, message)
}

/// `SC-E0004`: placing the glyphs needed more than `budget`.
fn exhausted(budget: &Budget) -> Diagnostic {
    Diagnostic::new(Code::BudgetExhausted, format!("Setting the text needed more than the work budget of {} units.", budget.max_work))
        .with_fix(Fix::Hint("Set a shorter text, or set it with a larger budget.".to_string()))
}
