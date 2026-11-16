//! Lettering: text set in an embroidery font (roadmap step M14.2, `docs/src/design/fonts.md` › Lettering,
//! `REQ-FONT-009` to `REQ-FONT-014`): the glyphs the text names, each placed, scaled and joined into one
//! design that every host plans as it plans any.
//!
//! **Why here.** The font reader gives each glyph as a design with its box and its baseline
//! ([`crate::font`]), and a font's text is routed by the engine's routing ([`stitchcraft_engine::route`]).
//! Lettering needs both and nothing else, so it lies beside the reader, where the command line and, from
//! M14.4, the VectorCraft plug-in both find it.
//!
//! **How** ([`set`]):
//!
//! 1. The lines are laid out (the module `layout`): a glyph for each character, or for several where the font draws
//!    them as one, placed by the font's advances and kerning and the spacing asked for, and each line
//!    aligned.
//! 2. The scale asked for is kept within the heights the font sews at (`SC-W1106`), and every glyph's
//!    elements are moved to their place and scaled about the first line's start, with the widths of their
//!    strokes. Their settings stay as the font's author set them.
//! 3. In a font whose `auto_satin` is on, the engine's routing joins the text's satin columns and strokes in
//!    their order and without trims, as Ink/Stitch's lettering does. Each piece of the route sews at the end
//!    of the glyph its element was in.
//!
//! **What waits.** The glyphs sew in the font's default direction, every line the same way, and no trim is
//! added between them: lines sewn back and forth, the trims and the sorted colours are M14.3's. The 4 forms of
//! an Arabic letter aren't chosen (`layout`).
//!
//! **Never crashes, bounded.** Any text and any settings give a design or a coded error. One meter of the
//! budget counts the layout and the placing: a unit per character, per glyph name tried for it, and per
//! element and point placed. Routing has the budget again, as a routing filter has.

mod layout;

use std::collections::{BTreeMap, BTreeSet};

use stitchcraft_core::units::{Mm, Point};
use stitchcraft_core::{Budget, Code, Diagnostic, ElementId, Fix, Meter, Severity};
use stitchcraft_engine::design::{Clip, ClipShape, Design, DesignSettings, Element, Shape};
use stitchcraft_engine::route::{Options, Route, Tool, route};

use crate::font::{Direction, Font, Glyph};
use layout::Spot;

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
}

impl Default for Lettering {
    fn default() -> Self {
        Lettering { scale: 100.0, letter_spacing: 0.0, word_spacing: 0.0, line_height: 0.0, align: Align::Left }
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

/// Every setting of lettering, in the order Ink/Stitch's batch lettering lists them.
pub const SETTINGS: [Setting; 5] = [SCALE, TEXT_ALIGN, LETTER_SPACING, WORD_SPACING, LINE_HEIGHT];

/// A glyph of a text set, where it lies.
#[derive(Clone, Debug, PartialEq)]
pub struct Placed {
    /// The glyph's name.
    pub name: String,
    /// The line it is in, from 0.
    pub line: usize,
    /// The word of the line it is in, from 0, in the order the words are laid out.
    pub word: usize,
    /// Its box's left edge, in millimetres from the first line's start.
    pub left: f64,
    /// Its box's width, in millimetres.
    pub width: f64,
    /// Its baseline, in millimetres down from the first line's.
    pub baseline: f64,
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
    let mut meter = budget.meter();
    let Some(variant) = font.default_variant() else {
        return Err(unset(format!("The font `{}` has no glyphs.", font.folder)));
    };
    let laid = layout::lay(font, variant, text, how, &mut meter).map_err(|e| e.diagnostic(budget, None))?;
    if laid.spots.is_empty() {
        return Err(unset(format!("None of the text's characters sews in the font `{}`: the text is empty, or only spaces.", font.folder)));
    }
    let mut warnings = Vec::new();
    let scale = scaled(font, how.scale, &mut warnings);
    if !laid.missing.is_empty() {
        warnings.push(missing(font, &laid.missing));
    }
    let used: BTreeMap<&str, &Glyph> = laid.spots.iter().map(|spot| (spot.glyph.name.as_str(), spot.glyph)).collect();
    warnings.extend(used.values().flat_map(|glyph| glyph.warnings.iter().cloned()));
    let factor = scale / 100.0;
    let glyphs = laid
        .spots
        .iter()
        .map(|spot| Placed {
            name: spot.glyph.name.clone(),
            line: spot.line,
            word: spot.word,
            left: spot.left * factor,
            width: spot.glyph.width * factor,
            baseline: spot.baseline * factor,
        })
        .collect();
    // An rtl direction's glyphs are drawn to sew from the right, so each of its lines sews from its end.
    let mut order: Vec<&Spot> = Vec::new();
    for line in &laid.lines {
        let spots = laid.spots.get(line.clone()).unwrap_or_default().iter();
        if variant.direction == Direction::RightToLeft { order.extend(spots.rev()) } else { order.extend(spots) }
    }
    let mut elements = Vec::new();
    for (at, spot) in order.iter().enumerate() {
        for element in spot.glyph.design.elements() {
            elements.push((at, placed(element, at, spot, factor, &mut meter).map_err(|e| e.unwrap_or_else(|| exhausted(budget)))?));
        }
    }
    let settings = DesignSettings::default();
    if font.settings.auto_satin {
        elements = routed(elements, &settings, budget, &mut warnings);
    }
    let design = Design::new(elements.into_iter().map(|(_, element)| element).collect(), settings).map_err(|e| unset(e.message))?;
    Ok(Set { design, glyphs, scale, warnings })
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

/// `element` of the glyph at `spot`, the `at`-th sewn, in its place and scaled by `factor`, charging `meter`.
/// `None` as the error when the budget runs out.
fn placed(element: &Element, at: usize, spot: &Spot, factor: f64, meter: &mut Meter) -> Result<Element, Option<Diagnostic>> {
    let points = element.shape.path().points().count();
    meter.charge(1 + u64::try_from(points).unwrap_or(u64::MAX)).map_err(|_| None)?;
    let (dx, dy) = (spot.left - spot.glyph.left, spot.baseline - spot.glyph.baseline);
    let far = || Some(unset("A glyph can't be placed: the scale or a spacing is no number, or the text is longer than 10 m.".to_string()));
    let point = |p: Point| Point::new((p.x() + dx) * factor, (p.y() + dy) * factor).map_err(|_| far());
    let shape = match &element.shape {
        Shape::Stroke { path, width, join, cap } => {
            Shape::Stroke { path: path.map(point)?, width: Mm::new(width.get() * factor).map_err(|_| far())?, join: *join, cap: *cap }
        }
        Shape::Fill { path, rule, gradient } => {
            Shape::Fill { path: path.map(point)?, rule: *rule, gradient: gradient.as_ref().map(|gradient| gradient.map(point)).transpose()? }
        }
    };
    let shapes = |clip: &Clip| {
        clip.shapes.iter().map(|shape| Ok(ClipShape { path: shape.path.map(point)?, rule: shape.rule })).collect::<Result<_, Option<Diagnostic>>>()
    };
    let clips = element.clips.iter().map(|clip| Ok(Clip { shapes: shapes(clip)? })).collect::<Result<_, Option<Diagnostic>>>()?;
    let id = ElementId::new(format!("{at}-{}", element.id))
        .map_err(|_| Some(unset(format!("The id `{}` of an element of the glyph `{}` is too long to number.", element.id, spot.glyph.name))))?;
    Ok(Element { id, shape, hints: element.hints.map(point)?, clips, ..element.clone() })
}

/// `elements`, each with the glyph it is of, routed as Ink/Stitch's lettering routes a font whose
/// `auto_satin` is on: its satin columns and strokes in their order, without trims. Each piece sews at the
/// end of its element's glyph. A text without a satin column stays as it is, and so does one whose route
/// fails, which `warnings` then says. What the route only notes, such as a fill it leaves out, isn't said.
fn routed(elements: Vec<(usize, Element)>, settings: &DesignSettings, budget: &Budget, warnings: &mut Vec<Diagnostic>) -> Vec<(usize, Element)> {
    let how = Route { tool: Tool::Satins, options: Options { preserve_order: true, trim: false, ..Tool::Satins.defaults() }, start: None, end: None };
    let (glyphs, given): (Vec<usize>, Vec<Element>) = elements.into_iter().unzip();
    let route = route(&given, &how, settings, budget);
    warnings.extend(route.diagnostics.into_iter().filter(|said| said.code != Code::RouteNothing && said.severity() != Severity::Info));
    let replaced: BTreeSet<usize> = route.replaced.into_iter().collect();
    let kept = given.into_iter().enumerate().filter(|(index, _)| !replaced.contains(index));
    let mut all: Vec<(usize, Element)> = kept.map(|(index, element)| (glyphs.get(index).copied().unwrap_or_default(), element)).collect();
    all.extend(route.pieces.into_iter().map(|piece| (glyphs.get(piece.from).copied().unwrap_or_default(), piece.element)));
    all.sort_by_key(|(glyph, _)| *glyph);
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
