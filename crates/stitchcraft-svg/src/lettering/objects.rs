//! Text objects in automatic digitizing (roadmap step M14.7, `docs/src/design/fonts.md` › Text objects in
//! automatic digitizing, `REQ-AUTO-069` and `REQ-AUTO-070`): a text object of a drawing, which keeps its
//! characters and not their outlines, sews as lettering in the host's default embroidery font, at the
//! text's size and place.
//!
//! **Why an embroidery font.** A drawing names a typeface and holds no outline of a letter, and StitchCraft
//! reads no typeface's file yet (the roadmap's M15.2). Until it does, the letters of an embroidery font
//! stand in: they are drawn for the needle, so the word sews cleanly, and it stands where the drawing has
//! it. It doesn't look like the drawing's typeface, and a note says so for every text object (`SC-I1116`).
//!
//! **Why here.** Both readers find text objects, an SVG file's `<text>` and a VectorCraft document's point
//! type, and both have lettering below them. Each gives what it read as a [`TextObject`], and one function
//! sets it ([`letter`]), with lettering's own layout ([`super::set`]): there is no second one.
//!
//! **How tall.** A text's size is its font size, the height of the em box its typeface is drawn in, and no
//! letter is that tall. An embroidery font's size is the height of its letters. Without the typeface, how
//! tall it draws its capitals can't be known, and in most typefaces it is about 0.7 of the font size
//! ([`CAPITALS`]). So a line sews 0.7 of its font size tall, within the heights the embroidery font sews at
//! (`SC-W1106` outside them), and the note names the assumption.
//!
//! **Where.** Each line is set on its own, and starts, is centred or ends at its anchor on its baseline as
//! the drawing says. It is turned as the object's baseline is.
//!
//! **How long.** The embroidery font's letters are as wide as it draws them, and an embroidery font is
//! wider than most typefaces, so a line set at its height can reach past the place the drawing gives its
//! text, over the shapes beside it. A line is therefore fitted to the length its text most likely had: the
//! one the drawing states, or else half its font size for each character ([`WIDTH`]). Its gaps are narrowed
//! first, then it is set smaller, and never smaller than the font sews: there `SC-W1117` says that it is
//! still longer. A line that is shorter than its place isn't stretched.
//!
//! **All or nothing.** A text object whose font isn't there or doesn't read, or one of whose lines can't
//! be set, sews nothing, and `SC-W1111` says why. Nothing is left out without a word.
//!
//! **Never crashes, bounded.** One meter of the budget counts a unit for each text object and line, for
//! each byte of a line and for each element and point placed. Setting a line has the budget again, as
//! [`super::set`] says.

use std::collections::BTreeSet;

use stitchcraft_core::units::Point;
use stitchcraft_core::{Budget, Code, Diagnostic, ElementId, Exhausted, Fix, Meter, Severity};
use stitchcraft_engine::design::{Clip, Design, Element, within_working_limit};
use stitchcraft_plan::Thread;

use super::pictures::{CLOSER, Found};
use super::{Align, Lettering, Set, exhausted, moved, scaled, set};
use crate::font::Font;

/// How tall a line's letters sew, as a share of its font size: about the height of the capitals of most
/// typefaces, which lie between 0.65 and 0.75 of their font size.
pub const CAPITALS: f64 = 0.7;

/// How long a line of a text most likely was in its own typeface, for each of its characters, as a share
/// of its font size: an estimate for the proportional typefaces text is commonly set in, whose lines of
/// small letters measure 0.41 to 0.55 of the font size for each character and whose lines of capitals
/// 0.50 to 0.68 (`docs/src/design/fonts.md` › Text objects in automatic digitizing).
pub const WIDTH: f64 = 0.5;

/// The name that asks a host for its default embroidery font.
pub const DEFAULT_FONT: &str = "";

/// The embroidery font `text` sews in, asked of `fonts` within `budget`: the host's default font. Every
/// text object's font is chosen here and nowhere else.
fn font_of(_text: &TextObject, fonts: &mut dyn FnMut(&str, &Budget) -> Found, budget: &Budget) -> Found {
    fonts(DEFAULT_FONT, budget)
}

/// The most fonts a message names: a folder of fonts can hold hundreds.
const NAMED: usize = 8;

/// Where `stitch` finds a font, for a text that has none: what its hint starts with.
const FONTS_FOLDER: &str = "For `stitch`, set `STITCHCRAFT_FONTS` to the folder that holds the embroidery fonts' folders. ";

/// How to sew a text object's own letter shapes.
const TO_PATHS: &str = "To sew the letter shapes of its own typeface, convert the text to paths in the drawing program.";

/// Which point of a line its anchor is.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Anchor {
    /// The line starts at its anchor.
    #[default]
    Start,
    /// The line is centred on its anchor.
    Middle,
    /// The line ends at its anchor.
    End,
}

impl Anchor {
    /// The alignment that puts a line of lettering so about its origin.
    const fn align(self) -> Align {
        match self {
            Anchor::Start => Align::Left,
            Anchor::Middle => Align::Centre,
            Anchor::End => Align::Right,
        }
    }
}

/// A line of a text object.
#[derive(Clone, Debug, PartialEq)]
pub struct Line {
    /// Its characters.
    pub text: String,
    /// Its anchor, on its baseline, in the design's millimetres.
    pub at: Point,
    /// Which point of the line the anchor is.
    pub anchor: Anchor,
    /// Its font size, in millimetres.
    pub size: f64,
    /// The thread of its fill, which a font of one thread sews in. Without one, the font's own.
    pub thread: Option<Thread>,
    /// How long the drawing says the line is, in millimetres, when it says so, as an SVG file's
    /// `textLength` does. Without it the length is estimated ([`WIDTH`]).
    pub length: Option<f64>,
}

/// A text object a reader found: what a drawing keeps of a text, which is not its letters' outlines.
#[derive(Clone, Debug, PartialEq)]
pub struct TextObject {
    /// What messages call it, and what the ids of its letters start with, such as `svg:text1`.
    pub label: String,
    /// The name its elements take.
    pub name: Option<String>,
    /// The typeface the drawing names for it, for the note.
    pub typeface: Option<String>,
    /// Its lines, in the order they sew.
    pub lines: Vec<Line>,
    /// The cosine and the sine of the angle its baselines are turned by, clockwise as a picture shows it.
    pub turn: (f64, f64),
    /// The areas its elements are cut to.
    pub clips: Vec<Clip>,
}

/// A text object that waits for a host's fonts: a reader without fonts, as the SVG file's is, gives each
/// with its place in the stitching order.
#[derive(Clone, Debug, PartialEq)]
pub struct Waiting {
    /// How many of the design's elements sew before it.
    pub after: usize,
    /// The text object.
    pub text: TextObject,
}

/// `design` with the lettering of each of `texts` at its place in the stitching order ([`letter`]), and what
/// was said. `fonts` gives the font of a name, the empty name being the host's default font, and `said`
/// holds the folders of the fonts whose own warnings were said already. A design without texts is returned
/// as it is. Fails with `SC-E0004` when the work needs more than `budget` allows.
pub fn lettered(
    design: Design,
    texts: &[Waiting],
    fonts: &mut dyn FnMut(&str, &Budget) -> Found,
    said: &mut BTreeSet<String>,
    budget: &Budget,
) -> Result<(Design, Vec<Diagnostic>), Diagnostic> {
    if texts.is_empty() {
        return Ok((design, Vec::new()));
    }
    let mut meter = budget.meter();
    let mut warnings = Vec::new();
    let mut elements = Vec::with_capacity(design.elements().len());
    let mut waiting = texts.iter().peekable();
    for (index, element) in design.elements().iter().enumerate() {
        meter.charge(1).map_err(|_| exhausted(budget))?;
        while let Some(next) = waiting.next_if(|next| next.after <= index) {
            let (letters, notes) = letter(&next.text, fonts, said, budget)?;
            elements.extend(letters);
            warnings.extend(notes);
        }
        elements.push(element.clone());
    }
    for next in waiting {
        let (letters, notes) = letter(&next.text, fonts, said, budget)?;
        elements.extend(letters);
        warnings.extend(notes);
    }
    let mut with = Design::new(elements, design.settings)?;
    with.sort_colours = design.sort_colours;
    Ok((with, warnings))
}

/// The elements `text` sews, set a line at a time in the host's default font, which `fonts` gives for the
/// empty name, and what was said: `SC-I1116` and what the font and lettering warn of when it sews, and
/// `SC-W1111` with no element when it doesn't. A text without lines sews nothing and nothing is said, as a
/// viewer shows nothing of it. `said` holds the folders of the fonts whose own warnings were said already,
/// and gains the font's. Fails with `SC-E0004` when the work needs more than `budget` allows.
pub fn letter(
    text: &TextObject,
    fonts: &mut dyn FnMut(&str, &Budget) -> Found,
    said: &mut BTreeSet<String>,
    budget: &Budget,
) -> Result<(Vec<Element>, Vec<Diagnostic>), Diagnostic> {
    let mut meter = budget.meter();
    meter.charge(1).map_err(|_| exhausted(budget))?;
    if text.lines.is_empty() {
        return Ok((Vec::new(), Vec::new()));
    }
    let font = match font_of(text, fonts, budget) {
        Found::Font(font) => font,
        Found::Unread(unread) if unread.code == Code::BudgetExhausted => return Err(unread),
        Found::Unread(unread) => {
            let why = format!("the embroidery font it would sew in does not read. {}", unread.message);
            return Ok((Vec::new(), vec![unsewn(text, &why, FONTS_FOLDER)]));
        }
        Found::Missing(names) => return Ok((Vec::new(), vec![unsewn(text, &missing(&names), FONTS_FOLDER)])),
    };
    let mut warnings: Vec<Diagnostic> = Vec::new();
    if said.insert(font.folder.clone()) {
        warnings.extend(font.warnings.iter().filter(|said| said.severity() != Severity::Info).cloned());
    }
    let mut elements = Vec::new();
    let (mut least, mut most, mut longest) = (f64::INFINITY, 0.0_f64, 0.0_f64);
    let mut smaller: Vec<Smaller> = Vec::new();
    for (index, line) in text.lines.iter().enumerate() {
        let bytes = u64::try_from(line.text.len()).unwrap_or(u64::MAX);
        meter.charge(bytes.saturating_add(1)).map_err(|_| exhausted(budget))?;
        let mut said_here = Vec::new();
        let (tall, long) = match sewn_line(text, (index, line), &font, (&mut meter, budget), &mut elements, &mut said_here) {
            Ok(sewn) => {
                smaller.extend(sewn.smaller);
                (sewn.tall, sewn.long)
            }
            Err(Stop::Failed(failed)) => return Err(failed),
            Err(Stop::Unsewn(why)) => return Ok((Vec::new(), vec![unsewn(text, &why, "")])),
        };
        (least, most, longest) = (least.min(tall), most.max(tall), longest.max(long));
        // 2 lines of one size say the same of the font's range: once is enough.
        for note in said_here {
            if !warnings.contains(&note) {
                warnings.push(note);
            }
        }
    }
    warnings.push(stands_in(text, &font, (least, most), longest, &smaller));
    Ok((elements, warnings))
}

/// Why a line stops its text object.
enum Stop {
    /// The text object sews nothing, for this reason.
    Unsewn(String),
    /// The budget ran out.
    Failed(Diagnostic),
}

/// A line set smaller than [`CAPITALS`] of its font size, to fit its place.
#[derive(Clone, Copy)]
struct Smaller {
    /// The length it is fitted to, in millimetres.
    place: f64,
    /// Whether the drawing states that length. If not, it is estimated.
    stated: bool,
}

/// A line as it is sewn.
struct Sewn {
    /// How tall its letters sew, in millimetres. A font without a size has no height to give: 0 stands for
    /// it.
    tall: f64,
    /// How long it is along its baseline, from the first point its letters sew to the last, in millimetres.
    long: f64,
    /// What it is fitted to, when it is set smaller for that.
    smaller: Option<Smaller>,
}

/// How a line that is longer than its place is set to fit it.
struct Fit {
    /// The scale, in percent.
    scale: f64,
    /// The millimetres added between its letters and its words, before scaling: none, or fewer than none.
    spacing: f64,
    /// Whether the font sews no smaller, and the line stays longer than its place.
    smallest: bool,
}

/// How a line of `glyphs` glyphs, `natural` mm long as `font` sets it at `full` percent, is set to be no
/// longer than `place` mm: `None` when it is no longer as it is. Its gaps are narrowed first, by up to
/// [`CLOSER`] of its letters' height each, as traced letters' lettering is closed up. Then it is set
/// smaller, its gaps as narrow for its height, at the scale that makes it as long, cut to a tenth of a
/// percent, and no smaller than the font sews.
fn fit(font: &Font, full: f64, natural: f64, glyphs: usize, place: f64) -> Option<Fit> {
    let over = natural - place.max(0.0);
    if over <= 0.0 {
        return None;
    }
    let (own, factor) = (font.settings.size, full / 100.0);
    let gaps = glyphs.saturating_sub(1) as f64;
    // What narrowing every gap as far as it goes takes off a line, at the font's own size.
    let closed = CLOSER * own * gaps;
    if over <= closed * factor {
        return Some(Fit { scale: full, spacing: -over / gaps / factor, smallest: false });
    }
    // The line is longer than its place with its gaps closed, so what is left of it has a length.
    let fitted = (1000.0 * place.max(0.0) / (natural / factor - closed)).floor() / 10.0;
    let least = font.settings.min_scale * 100.0;
    let smallest = fitted < least;
    Some(Fit { scale: if smallest { least } else { fitted }, spacing: -CLOSER * own, smallest })
}

/// How long `set` is along its baseline, from the first point its elements sew to the last, in
/// millimetres: 0 of no point. A unit of work for each element and each point.
fn extent(set: &Set, meter: &mut Meter) -> Result<f64, Exhausted> {
    let (mut from, mut to) = (f64::INFINITY, f64::NEG_INFINITY);
    for element in set.design.elements() {
        let points = u64::try_from(element.shape.path().points().count()).unwrap_or(u64::MAX);
        meter.charge(points.saturating_add(1))?;
        (from, to) = element.shape.path().points().fold((from, to), |(from, to), p| (from.min(p.x()), to.max(p.x())));
    }
    Ok((to - from).max(0.0))
}

/// Sets the `index`-th line of `text`, `line`, in `font` and adds its elements to `elements`, each in its
/// place. The line is set at [`CAPITALS`] of its font size, and when it is then longer than its place, the
/// length the drawing states or the one estimated for it, it is fitted to that ([`fit`]). `warnings` gains
/// what setting it warned of, and `SC-W1117` for a line that stays longer than its place.
fn sewn_line(
    text: &TextObject,
    (index, line): (usize, &Line),
    font: &Font,
    (meter, budget): (&mut Meter, &Budget),
    elements: &mut Vec<Element>,
    warnings: &mut Vec<Diagnostic>,
) -> Result<Sewn, Stop> {
    let own = font.settings.size;
    // A font that states no size is set as it is drawn.
    let wanted = if own > 0.0 { (1000.0 * CAPITALS * line.size / own).round() / 10.0 } else { 100.0 };
    let full = scaled(font, wanted, warnings);
    let spent = |_| Stop::Failed(exhausted(budget));
    let unset = |unset: Diagnostic| {
        if unset.code == Code::BudgetExhausted {
            Stop::Failed(unset)
        } else {
            Stop::Unsewn(format!("its line {} is not set. {}", index + 1, unset.message))
        }
    };
    let plain = Lettering { scale: full, align: line.anchor.align(), ..Lettering::default() };
    let first = set(font, &line.text, &plain, budget).map_err(unset)?;
    let natural = extent(&first, meter).map_err(spent)?;
    let place = line.length.unwrap_or(WIDTH * line.size * line.text.chars().count() as f64);
    let (set, scale, long) = match fit(font, full, natural, first.glyphs.len(), place) {
        None => (first, full, natural),
        Some(fit) => {
            let closer = Lettering { scale: fit.scale, letter_spacing: fit.spacing, word_spacing: fit.spacing, ..plain };
            let fitted = set(font, &line.text, &closer, budget).map_err(unset)?;
            let long = extent(&fitted, meter).map_err(spent)?;
            if fit.smallest {
                warnings.push(longer(text, font, index, (long, own * fit.scale / 100.0), place));
            }
            (fitted, fit.scale, long)
        }
    };
    warnings.extend(set.warnings.iter().filter(|said| said.severity() != Severity::Info).cloned());
    let first = set.design.elements().first().map(|first| first.thread.color);
    let one_thread = set.design.elements().iter().all(|element| Some(element.thread.color) == first);
    let (cos, sin) = text.turn;
    for element in set.design.elements() {
        let points = u64::try_from(element.shape.path().points().count()).unwrap_or(u64::MAX);
        meter.charge(points.saturating_add(1)).map_err(spent)?;
        let far = || Stop::Unsewn("it lies beyond the design's reach.".to_string());
        let placed = moved(element, |p| {
            let turned = Point::new(cos * p.x() - sin * p.y() + line.at.x(), sin * p.x() + cos * p.y() + line.at.y());
            turned.ok().filter(|placed| within_working_limit(*placed)).ok_or_else(far)
        })?;
        let id = ElementId::new(format!("{}:{}:{}", text.label, index + 1, element.id))
            .map_err(|_| Stop::Unsewn("its name is too long to number its letters by.".to_string()))?;
        let thread = line.thread.clone().filter(|_| one_thread).unwrap_or_else(|| placed.thread.clone());
        let mut clips = placed.clips.clone();
        clips.extend(text.clips.iter().cloned());
        elements.push(Element { id, name: text.name.clone(), thread, clips, ..placed });
    }
    let smaller = (scale < full).then_some(Smaller { place, stated: line.length.is_some() });
    Ok(Sewn { tall: own * scale / 100.0, long, smaller })
}

/// `SC-W1117`: the `index`-th line of `text` is `long` mm long at `tall` mm, the smallest height `font`
/// sews at, and its place is `place` mm long.
fn longer(text: &TextObject, font: &Font, index: usize, (long, tall): (f64, f64), place: f64) -> Diagnostic {
    let message = format!(
        "Line {} of the text `{}` is {long:.1} mm long at {tall:.1} mm, the smallest height the font `{}` sews at, where its text most likely took \
         {place:.1} mm: it reaches past the text's place.",
        index + 1,
        text.label,
        font.folder
    );
    let fix = "Make the text larger or shorter in the drawing, or convert it to paths there.";
    about(text, Diagnostic::new(Code::LetteringLongerThanTextsPlace, message).with_fix(Fix::Hint(fix.to_string())))
}

/// What a text object without a font says: the fonts there are, if any.
fn missing(names: &[String]) -> String {
    let more = names.len().saturating_sub(NAMED);
    let listed = names.iter().take(NAMED).cloned().collect::<Vec<_>>().join(", ");
    match (names.is_empty(), more) {
        (true, _) => "no embroidery font is here to sew it in.".to_string(),
        (false, 0) => format!("the default embroidery font is not here. The fonts here are {listed}."),
        (false, more) => format!("the default embroidery font is not here. The fonts here are {listed}, and {more} more."),
    }
}

/// `text` with the element `diagnostic` is about, when its label is an element's id.
fn about(text: &TextObject, diagnostic: Diagnostic) -> Diagnostic {
    match ElementId::new(text.label.clone()) {
        Ok(id) => diagnostic.with_element(id),
        Err(_) => diagnostic,
    }
}

/// `SC-W1111`: `text` sews nothing, for the reason `why`. Its hint starts with `first`.
fn unsewn(text: &TextObject, why: &str, first: &str) -> Diagnostic {
    let message = format!("The text `{}` is not stitched: {why}", text.label);
    let fix = format!("{first}{TO_PATHS}");
    about(text, Diagnostic::new(Code::TextNotLettered, message).with_fix(Fix::Hint(fix)))
}

/// What `SC-I1116` says of `text` sewn in `font`, whose settings give no size: it can't be set at the text's.
fn unsized_note(text: &TextObject, font: &Font) -> String {
    format!(
        "The text `{}` is sewn in the embroidery font `{}`, as tall as the font is drawn: its settings give no size to set it at the text's by. \
         The typeface of the text itself can't be read yet.",
        text.label, font.folder
    )
}

/// `SC-I1116`: `text` sews in `font`, its lines from `least` to `most` millimetres tall and the longest
/// `longest` millimetres long, those of `smaller` set smaller to fit their places.
fn stands_in(text: &TextObject, font: &Font, (least, most): (f64, f64), longest: f64, smaller: &[Smaller]) -> Diagnostic {
    let tall = if most > least {
        format!("{least:.1} to {most:.1} mm tall")
    } else if most > 0.0 {
        format!("{most:.1} mm tall")
    } else {
        // A font whose settings give no size.
        return about(text, Diagnostic::new(Code::TextSewnInEmbroideryFont, unsized_note(text, font)).with_fix(Fix::Hint(TO_PATHS.to_string())));
    };
    let how = if smaller.iter().all(|line| line.stated) { "which the drawing states" } else { "estimated at half the font size for each character" };
    let why = match (smaller, text.lines.len()) {
        ([], _) => format!("{CAPITALS} of its font size, which is about how tall a typeface draws its capitals"),
        ([line], 1) => format!("made smaller than {CAPITALS} of its font size to fit the {:.1} mm its line most likely had, {how}", line.place),
        (some, lines) => {
            format!(
                "{} of its {lines} lines made smaller than {CAPITALS} of their font size to fit the length each most likely had, {how}",
                some.len()
            )
        }
    };
    let own = match &text.typeface {
        Some(typeface) => format!("Its own typeface, {typeface}, can't be read yet"),
        None => "Its own typeface can't be read yet".to_string(),
    };
    let message = format!(
        "The text `{}` is sewn in the embroidery font `{}`, {tall}: {why}. {own}, so the letters are the embroidery font's, and as wide as it \
         draws them: its longest line is {longest:.1} mm long.",
        text.label, font.folder
    );
    about(text, Diagnostic::new(Code::TextSewnInEmbroideryFont, message).with_fix(Fix::Hint(TO_PATHS.to_string())))
}
