//! Text in pictures (roadmap step M14.5, `docs/src/design/image-to-embroidery.md` › Text in pictures,
//! `REQ-AUTO-063` to `REQ-AUTO-066`): the shapes inside an area marked `stitch:text` are taken out of a
//! design, and lettering of the marker's text is put where they were, as tall as they were.
//!
//! **Why.** Letters traced from a picture are shapes, and automatic digitizing sews them by the rules for
//! shapes: small ones come out ragged, and neighbours run together. An embroidery font is drawn for the
//! needle. Nothing here reads a picture, so a person says where the text is and what it says, with a marker.
//!
//! **Why here, on a design.** Both readers give a named shape as an element with its name, so one pass over
//! the design they read serves an SVG file, a VectorCraft document and a marker given on the command line.
//! It runs before anything is digitized: what is left of the traced shapes is then digitized as if the
//! letters had never been traced, with its overlaps and its base layer, and the lettering's elements keep
//! the settings the font's author gave them.
//!
//! **How** ([`replace`]), for each marker:
//!
//! 1. Every subpath of every element that lies wholly inside the marker is taken out. A traced picture
//!    keeps all the areas of one colour in one compound path, so the letters are subpaths, and so are the
//!    holes they leave in the colour round them: taking both out makes the background whole again.
//! 2. What was taken out is measured along and across the line the text follows (the module `frame`), and
//!    the text is set once at the font's own size to measure it too.
//! 3. The text is set again ([`super::set_along`]) at the scale that makes it as tall, on a line beside the
//!    marker's line that puts its box on the box of what was taken out, its letters spread or closed up to
//!    its width within limits.
//! 4. The lettering sews after the last element a subpath was taken from, in the thread that painted the
//!    most of what was taken out when the font has one thread.
//!
//! A marker that can't be used leaves the traced shapes as they are and says why (`SC-W1113`). Nothing is
//! taken out before the lettering is set, so a design is never left with a hole and no text.
//!
//! **Never crashes, bounded.** One meter of the budget counts a unit for each element, and for each
//! flattened point of the design a unit per segment of the marker it is tested against. Setting the text
//! has the budget again, as [`super::set`] says.

mod frame;
pub mod weight;

use std::collections::{BTreeMap, BTreeSet};
use std::rc::Rc;

use stitchcraft_core::units::Point;
use stitchcraft_core::{Budget, Code, Diagnostic, ElementId, Exhausted, Fix, Meter, Severity};
use stitchcraft_engine::design::{Design, Element, Path, Segment, Shape, Subpath};
use stitchcraft_engine::normalize::stroke::flatten;
use stitchcraft_plan::Thread;

use super::{Along, Lettering, Set, TextPosition, VerticalPosition, exhausted, scaled, set, set_along};
use crate::font::Font;
use frame::{At, Line};

/// The name that marks text in a picture: an element named `stitch:text` and a note, in any mix of upper and
/// lower case, is a marker.
pub const MARKER: &str = "stitch:text";

/// How near a curve its polyline lies when a shape is tested and measured, in millimetres: a tenth of the
/// shortest letter an embroidery font sews would still measure within 2 %.
const FLAT: f64 = 0.05;

/// How much closer than the font sets them the letters may be pushed to fit the traced letters' width, in
/// heights of the letters: a tenth, about what a font leaves between 2 letters, so strokes don't sew over
/// each other.
pub const CLOSER: f64 = 0.1;

/// How much further apart than the font sets them the letters may be spread to fit, in heights of the
/// letters: a half, past which a word reads as separate letters.
pub const APART: f64 = 0.5;

/// How far the lettering's width may be from the traced letters' before it is said, in heights of the
/// letters: a tenth, which is less than a stroke of a letter is wide.
pub const FIT: f64 = 0.1;

/// What a host has of a font a marker asks for.
#[derive(Clone, Debug)]
pub enum Found {
    /// The font.
    Font(Rc<Font>),
    /// The font is there and doesn't read: what its reader said.
    Unread(Diagnostic),
    /// No font of that name is there: the fonts that are, each as a person names it.
    Missing(Vec<String>),
}

/// A font that a marker which names none may be set in: one of the fonts a host holds for that, with what
/// the choice goes by (`REQ-AUTO-071`).
#[derive(Clone, Debug, PartialEq)]
pub struct Choice {
    /// The name the host finds the font by: its folder's.
    pub font: String,
    /// How bold it is: the width of its strokes in heights of its capital letters ([`weight::of_font`]).
    pub weight: f64,
    /// The least and the greatest height it sews at, in millimetres.
    pub heights: (f64, f64),
}

impl Choice {
    /// How far `height` lies outside the heights the font sews at, in millimetres: 0 within them.
    fn outside(&self, height: f64) -> f64 {
        (self.heights.0 - height).max(height - self.heights.1).max(0.0)
    }
}

/// Which of `choices` letters `height` mm tall are set in, their strokes `weight` heights wide: of the fonts
/// that sew nearest that height, which is at it when one sews at it, the nearest in weight, and the
/// earlier of 2 as near. Letters without a weight take the earliest. `None` of no choices.
pub fn chosen(choices: &[Choice], weight: Option<f64>, height: f64) -> Option<usize> {
    let mut best: Option<(usize, f64, f64)> = None;
    for (index, choice) in choices.iter().enumerate() {
        let (outside, unlike) = (choice.outside(height), weight.map_or(0.0, |weight| (choice.weight - weight).abs()));
        if best.is_none_or(|(_, least, like)| outside < least || (outside == least && unlike < like)) {
            best = Some((index, outside, unlike));
        }
    }
    best.map(|(index, ..)| index)
}

/// An area or a line that marks text in a picture.
#[derive(Clone, Debug, PartialEq)]
pub struct Marker {
    /// What messages call it, and what the ids of its letters start with: the id of its element.
    pub label: String,
    /// Its geometry. A fill, or a stroke whose first subpath is closed, is an area: what its outline
    /// encloses. A stroke whose first subpath is open is a line through the letters, as wide as the stroke.
    pub shape: Shape,
    /// What follows `stitch:text` in its name: the text, or the text between double quotes and settings
    /// after it, each `name=value`.
    pub note: String,
}

impl Marker {
    /// The marker `element` is, if its name says it is one.
    pub fn of(element: &Element) -> Option<Marker> {
        let note = note(element.name.as_deref()?)?;
        Some(Marker { label: element.id.to_string(), shape: element.shape.clone(), note: note.to_string() })
    }
}

/// The note of the marker `name` names: what follows [`MARKER`] and white space, without the white space at
/// its ends. `None` for a name that isn't a marker's.
pub fn note(name: &str) -> Option<&str> {
    let name = name.trim_start();
    if !name.get(..MARKER.len())?.eq_ignore_ascii_case(MARKER) {
        return None;
    }
    let rest = name.get(MARKER.len()..)?;
    (rest.is_empty() || rest.starts_with(char::is_whitespace)).then_some(rest.trim())
}

/// What a marker's note asks for.
#[derive(Clone, Debug, PartialEq)]
struct Asked {
    /// The text.
    text: String,
    /// The font's name: empty for the host's default font.
    font: String,
    /// How the text is set, as far as a marker says: its trims and its sorted colours.
    how: Lettering,
    /// The settings that don't read, as written.
    unread: Vec<String>,
}

impl Asked {
    /// What `note` asks for. A note that starts with a double quote and has a second holds the text between
    /// its first and its last, and settings after it: `font`, `trim` and `color_sort`.
    fn read(note: &str) -> Asked {
        let mut asked = Asked { text: note.to_string(), font: String::new(), how: Lettering::default(), unread: Vec::new() };
        let quoted = note.strip_prefix('"').and_then(|rest| Some((rest, rest.rfind('"')?)));
        let Some((rest, end)) = quoted else { return asked };
        asked.text = rest.get(..end).unwrap_or_default().to_string();
        for word in rest.get(end + 1..).unwrap_or_default().split_whitespace() {
            match word.split_once('=') {
                Some(("font", name)) => asked.font = name.to_string(),
                Some((name @ ("trim" | "color_sort"), value)) if asked.how.take(name, value) => {}
                _ => asked.unread.push(word.to_string()),
            }
        }
        asked
    }
}

/// Why a marker's text isn't set: a reason the user reads, or an error that ends the reading.
enum Stop {
    /// The marker isn't used, and its traced shapes stay: what follows "is not used: ".
    Unused(String),
    /// The budget ran out.
    Failed(Diagnostic),
}

/// `design` with the text of each of its markers, and of each of `more`, in the place of the shapes inside
/// the marker, and what was said. A marker of the design is an element whose name is [`MARKER`] and a note
/// ([`Marker::of`]), and is never sewn. `fonts` gives the font of a name, the empty name being the host's
/// default font. `said` holds the folders of the fonts whose own warnings were said already, and gains
/// those said here. A design without markers is returned as it is. Fails with `SC-E0004` when the work needs
/// more than `budget` allows.
pub fn replace(
    design: Design,
    more: &[Marker],
    fonts: &mut dyn FnMut(&str, &Budget) -> Found,
    said: &mut BTreeSet<String>,
    budget: &Budget,
) -> Result<(Design, Vec<Diagnostic>), Diagnostic> {
    replace_among(design, more, &mut |name, _, budget| fonts(name, budget), &[], said, budget)
}

/// As [`replace`], a marker that names no font set in the one of `choices` that is nearest in weight to
/// the letters it replaces, among those that sew at their height ([`chosen`], `REQ-AUTO-071`), with
/// `SC-I1118` saying which and why. Of fewer than 2 choices there is nothing to choose: such a marker is set
/// in the host's default font, and nothing is said of a choice. `fonts` is given the font's name and the marker's text, so that a host can
/// read a font only as far as the text needs it (`REQ-FONT-023`).
pub fn replace_among(
    design: Design,
    more: &[Marker],
    fonts: &mut dyn FnMut(&str, &str, &Budget) -> Found,
    choices: &[Choice],
    said: &mut BTreeSet<String>,
    budget: &Budget,
) -> Result<(Design, Vec<Diagnostic>), Diagnostic> {
    let mut meter = budget.meter();
    let mut markers: Vec<Marker> = Vec::new();
    let mut elements = Vec::with_capacity(design.elements().len());
    for element in design.elements() {
        meter.charge(1).map_err(|_| exhausted(budget))?;
        match Marker::of(element) {
            // An object with a fill and a stroke is 2 elements of one name and one path: one marker.
            Some(marker) if markers.iter().any(|known| known.note == marker.note && known.shape.path() == marker.shape.path()) => {}
            Some(marker) => markers.push(marker),
            None => elements.push(element.clone()),
        }
    }
    markers.extend(more.iter().cloned());
    if markers.is_empty() {
        return Ok((design, Vec::new()));
    }
    let mut warnings = Vec::new();
    for marker in &markers {
        let mut work = Work { fonts: &mut *fonts, choices, said: &mut *said, budget, meter: &mut meter, warnings: &mut warnings };
        match work.text(marker, &mut elements) {
            Ok(()) => {}
            Err(Stop::Unused(why)) => warnings.push(unused(marker, &why)),
            Err(Stop::Failed(failed)) => return Err(failed),
        }
    }
    let mut replaced = Design::new(elements, design.settings)?;
    replaced.sort_colours = design.sort_colours;
    Ok((replaced, warnings))
}

/// `SC-W1113`: the marker isn't used, for the reason `why`.
fn unused(marker: &Marker, why: &str) -> Diagnostic {
    let message = format!("The text marker `{}` is not used: {why} What lies inside it sews as it was traced.", marker.label);
    let fix = format!("Name a shape drawn round the letters, or a line drawn through them, `{MARKER}` and the text.");
    Diagnostic::new(Code::TextMarkerNotUsed, message).with_fix(Fix::Hint(fix))
}

/// `SC-I1118`: the text of `marker` is set in `choice`, chosen among `choices` for letters `height` mm tall
/// whose strokes are `bold` heights wide, and why.
fn chose(marker: &Marker, choices: &[Choice], choice: &Choice, bold: Option<f64>, height: f64) -> Diagnostic {
    let letters = match bold {
        Some(bold) => format!("its letters are {height:.1} mm tall, with strokes {bold:.2} of their height wide"),
        None => format!("its letters are {height:.1} mm tall, and their strokes have no width to measure"),
    };
    let sews = |font: &Choice| format!("from {:.1} to {:.1} mm", font.heights.0, font.heights.1);
    // The font nearest in weight whatever it sews at: the one a choice by weight alone would take.
    let nearest = bold.and_then(|bold| {
        choices.iter().fold(None, |best: Option<&Choice>, font| match best {
            Some(best) if (best.weight - bold).abs() <= (font.weight - bold).abs() => Some(best),
            _ => Some(font),
        })
    });
    let (why, fix) = match nearest.filter(|nearest| nearest.font != choice.font) {
        _ if choice.outside(height) > 0.0 => (
            format!(
                "No font here sews at that height. `{}` sews nearest to it, {}, with strokes {:.2} wide.",
                choice.font,
                sews(choice),
                choice.weight
            ),
            "Draw the picture so that its letters are as tall as a font sews, or name a font with font=NAME after the quoted text.".to_string(),
        ),
        Some(bolder) => (
            format!(
                "`{}` is the nearest in weight, with strokes {:.2} wide, and sews {}. So the text is set in `{}`, with strokes {:.2} wide, \
                 which sews at that height, {}.",
                bolder.font,
                bolder.weight,
                sews(bolder),
                choice.font,
                choice.weight,
                sews(choice)
            ),
            format!(
                "Leave the text unmarked to sew the letters as they were traced, which sews well from about 10 mm, or make the picture larger: \
                 `{}` sews from {:.1} mm.",
                bolder.font, bolder.heights.0
            ),
        ),
        None => (
            format!(
                "The text is set in `{}`, with strokes {:.2} wide: the nearest in weight of the fonts that sew at that height.",
                choice.font, choice.weight
            ),
            "Name another font with font=NAME after the quoted text.".to_string(),
        ),
    };
    let message = format!("The text marker `{}` names no font: {letters}. {why}", marker.label);
    Diagnostic::new(Code::FontChosenByWeight, message).with_fix(Fix::Hint(fix))
}

/// A marker's area, and the line its text follows.
struct Area {
    /// The outline of a marker that is an area, as closed polylines.
    rings: Vec<Vec<At>>,
    /// Half the width of a marker that is a line.
    band: Option<f64>,
    /// The line the text follows: the marker itself, or a level line along the top of its area.
    line: Line,
}

impl Area {
    /// The area of `marker`, flattened within `meter`.
    fn of(marker: &Marker, meter: &mut Meter) -> Result<Result<Area, &'static str>, Exhausted> {
        let pieces: Vec<Vec<At>> = flat(marker.shape.path(), meter)?;
        let open = marker.shape.path().subpaths.first().is_some_and(|first| !first.closed);
        if let (Shape::Stroke { width, .. }, true) = (&marker.shape, open) {
            let line = pieces.first().and_then(|first| Line::of(first));
            return Ok(line.map(|line| Area { rings: Vec::new(), band: Some(width.get() / 2.0), line }).ok_or("its line has no length."));
        }
        let rings = pieces;
        let Some([left, top, right, bottom]) = bounds(rings.iter().flatten().copied()) else { return Ok(Err("it encloses no area.")) };
        // Any level line serves: what is measured across it is placed across it again.
        let line = Line::of(&[(left, top), (right, top)]).filter(|_| bottom > top);
        Ok(line.map(|line| Area { rings, band: None, line }).ok_or("it encloses no area."))
    }

    /// Whether `p` lies in the area: inside the outline, or no further from the line than half its width and
    /// not past its ends.
    fn holds(&self, p: At) -> bool {
        match self.band {
            None => frame::inside(&self.rings, p),
            Some(half) => {
                let (along, across) = self.line.locate(p);
                across.abs() <= half && along >= 0.0 && along <= self.line.length()
            }
        }
    }

    /// What testing a point costs: a unit for each segment it is tested against.
    fn cost(&self) -> u64 {
        let segments = match self.band {
            None => self.rings.iter().map(Vec::len).sum(),
            Some(_) => self.line.segments(),
        };
        u64::try_from(segments).unwrap_or(u64::MAX)
    }
}

/// The subpaths of `path` as polylines within [`FLAT`] of them, one for each, charging `meter` as
/// [`flatten`] does.
fn flat(path: &Path, meter: &mut Meter) -> Result<Vec<Vec<At>>, Exhausted> {
    Ok(flatten(path, FLAT, meter)?.pieces.into_iter().map(|piece| piece.points.into_iter().map(|p| (p.x(), p.y())).collect()).collect())
}

/// The box round `points`: its left, top, right and bottom. `None` of no points.
fn bounds(points: impl Iterator<Item = At>) -> Option<[f64; 4]> {
    points.fold(None, |around: Option<[f64; 4]>, (x, y)| {
        Some(around.map_or([x, y, x, y], |[left, top, right, bottom]| [left.min(x), top.min(y), right.max(x), bottom.max(y)]))
    })
}

/// A subpath taken out of a design.
struct Taken {
    /// Its element, as an index into the design's elements.
    element: usize,
    /// Its place among its element's subpaths.
    subpath: usize,
    /// Its polyline.
    points: Vec<At>,
}

/// What setting markers' texts works with.
struct Work<'a> {
    fonts: &'a mut dyn FnMut(&str, &str, &Budget) -> Found,
    choices: &'a [Choice],
    said: &'a mut BTreeSet<String>,
    budget: &'a Budget,
    meter: &'a mut Meter,
    warnings: &'a mut Vec<Diagnostic>,
}

impl Work<'_> {
    /// The error of a meter that ran out.
    fn spent(&self) -> Stop {
        Stop::Failed(exhausted(self.budget))
    }

    /// Why `unset`, which setting the text returned, stops the marker.
    fn stopped(unset: Diagnostic) -> Stop {
        if unset.code == Code::BudgetExhausted { Stop::Failed(unset) } else { Stop::Unused(unset.message) }
    }

    /// Sets the text of `marker` in the place of what lies inside it in `elements`.
    fn text(&mut self, marker: &Marker, elements: &mut Vec<Element>) -> Result<(), Stop> {
        let asked = Asked::read(&marker.note);
        if asked.text.trim().is_empty() {
            return Err(Stop::Unused("it has no text.".to_string()));
        }
        let area = Area::of(marker, self.meter).map_err(|_| self.spent())?.map_err(|why| Stop::Unused(why.to_string()))?;
        let (taken, flats, crossing) = self.taken(&area, elements)?;
        if taken.is_empty() {
            return Err(Stop::Unused("no shape lies wholly inside it.".to_string()));
        }
        // What was taken out, along and across the line: from its start to its end, and from its bottom to
        // its top.
        let cost = u64::try_from(area.line.segments()).unwrap_or(u64::MAX);
        let mut held: Option<[f64; 4]> = None;
        for part in &taken {
            self.meter.charge(cost.saturating_mul(u64::try_from(part.points.len()).unwrap_or(u64::MAX))).map_err(|_| self.spent())?;
            held = bounds(held.iter().flat_map(|&[a0, c0, a1, c1]| [(a0, c0), (a1, c1)]).chain(part.points.iter().map(|&p| area.line.locate(p))));
        }
        let [start, bottom, end, top] = held.unwrap_or_default();
        let height = top - bottom;
        let thread = self.thread(&taken, &flats, elements)?;
        let (named, choice) = self.choice(marker, &asked, &taken, thread.as_ref(), elements, height)?;
        let font = self.font(&named, &asked.text)?;
        let trial = set(&font, &asked.text, &asked.how, self.budget).map_err(Self::stopped)?;
        let ink = self.ink(&trial)?;
        let per = 100.0 / trial.scale;
        let [left, up, right, down] = ink.map(|mm| mm * per);
        let own = down - up;
        if !(height > 0.0 && own > 0.0) {
            return Err(Stop::Unused("the shapes inside it, or its text in the font, have no height to measure.".to_string()));
        }
        // The scale that makes the text as tall, to a tenth of a percent, within what the font sews at.
        let wanted = (1000.0 * height / own).round() / 10.0;
        let mut ranged = Vec::new();
        let scale = scaled(&font, wanted, &mut ranged);
        let factor = scale / 100.0;
        let tall = own * factor;
        // The baseline: the text's box centred on the traced letters' across the line, and the baseline as
        // far above the box's bottom as the text reaches below it.
        let base = (top + bottom) / 2.0 - tall / 2.0 + down * factor;
        let room = frame::length(&area.line.beside(base, start, end));
        let natural = (right - left) * factor;
        let gaps = trial.glyphs.len().saturating_sub(1) as f64;
        let spread = if gaps > 0.0 { ((room - natural) / gaps).clamp(-CLOSER * height, APART * height) } else { 0.0 };
        let wide = natural + spread * gaps;
        // A millimetre along the baseline, in millimetres along the marker's line: 1 on a straight line.
        let step = if room > 0.0 { (end - start) / room } else { 1.0 };
        // Lettering along a path puts the leftmost glyph's box at the path's start. A box holds every path
        // of its glyph, sewn or not, so what the text sews may start further in.
        let boxes = trial.glyphs.iter().map(|glyph| glyph.left).fold(f64::INFINITY, f64::min) * per;
        let from = start + ((room - wide) / 2.0 - (left - boxes) * factor) * step;
        // The baseline goes on a letter's height past the text, so that it has a length whatever the text.
        let to = from + (wide + height) * step;
        // Every number here is finite, so every point is one, and the line has its 2 ends at least.
        let mut track = area.line.beside(base, from, to).into_iter().filter_map(|(x, y)| Point::new(x, y).ok());
        let first = track.next().unwrap_or(Point::from_tenths(0, 0));
        let path = Path { subpaths: vec![Subpath { start: first, segments: track.map(Segment::Line).collect(), closed: false }] };
        let how = Lettering { scale, letter_spacing: spread / factor, word_spacing: spread / factor, ..asked.how };
        let along = Along { path, position: TextPosition::Left, vertical: VerticalPosition::Baseline };
        let lettered = set_along(&font, &asked.text, &how, &along, self.budget).map_err(Self::stopped)?;
        let one = lettered.design.elements().first().map(|first| first.thread.color);
        let one_thread = lettered.design.elements().iter().all(|element| Some(element.thread.color) == one);
        let mut letters = Vec::with_capacity(lettered.design.elements().len());
        for element in lettered.design.elements() {
            let id = ElementId::new(format!("{}:{}", marker.label, element.id))
                .map_err(|_| Stop::Unused("its name is too long to number its letters by.".to_string()))?;
            let thread = thread.clone().filter(|_| one_thread).unwrap_or_else(|| element.thread.clone());
            letters.push(Element { id, name: None, thread, ..element.clone() });
        }
        // Everything is set: now the shapes go, and the lettering comes after the last element that lost
        // one.
        let mut gone: BTreeMap<usize, BTreeSet<usize>> = BTreeMap::new();
        for part in &taken {
            gone.entry(part.element).or_default().insert(part.subpath);
        }
        let mut kept = Vec::with_capacity(elements.len() + letters.len());
        let mut after = 0;
        for (index, element) in std::mem::take(elements).into_iter().enumerate() {
            let Some(subpaths) = gone.get(&index) else {
                kept.push(element);
                continue;
            };
            let path = element.shape.path();
            let left: Vec<Subpath> = path.subpaths.iter().enumerate().filter(|(at, _)| !subpaths.contains(at)).map(|(_, s)| s.clone()).collect();
            if !left.is_empty() {
                kept.push(Element { shape: with_path(&element.shape, Path { subpaths: left }), ..element });
            }
            after = kept.len();
        }
        kept.splice(after..after, letters);
        *elements = kept;
        if self.said.insert(font.folder.clone()) {
            self.warnings.extend(font.warnings.iter().filter(|said| said.severity() != Severity::Info).cloned());
        }
        self.warnings.extend(ranged);
        self.warnings.extend(lettered.warnings.iter().filter(|said| said.severity() != Severity::Info).cloned());
        let message = format!(
            "The text marker `{}`: {} traced outlines, {height:.1} mm tall, are replaced by the text \"{}\" in the font `{}`, {tall:.1} mm tall.",
            marker.label,
            taken.len(),
            asked.text,
            font.folder
        );
        self.warnings.push(Diagnostic::new(Code::TextInPictureLettered, message));
        self.warnings.extend(choice);
        if scale != wanted || (gaps > 0.0 && (room - wide).abs() > FIT * height) {
            let message = format!(
                "The lettering of the text marker `{}` is {tall:.1} mm tall and {wide:.1} mm wide, where the traced letters are {height:.1} mm \
                 tall and {room:.1} mm wide. It is centred on them.",
                marker.label
            );
            let fix = "Choose a font that sews at the letters' height and is about as wide, with font=NAME after the quoted text.";
            self.warnings.push(Diagnostic::new(Code::LetteringUnlikeTracedLetters, message).with_fix(Fix::Hint(fix.to_string())));
        }
        if crossing > 0 {
            let message = format!(
                "{crossing} shapes lie more than half inside the text marker `{}` and cross its edge. They sew as they were traced, beside \
                 the lettering.",
                marker.label
            );
            let fix = "Draw the marker round every letter of the text, and round nothing else.";
            self.warnings.push(Diagnostic::new(Code::ShapeCrossesTextMarker, message).with_fix(Fix::Hint(fix.to_string())));
        }
        for setting in &asked.unread {
            let message = format!(
                "The text marker `{}` is not used in full: its setting `{setting}` does not read, and is left out. A marker reads font, trim \
                 and color_sort, each as name=value after the quoted text.",
                marker.label
            );
            self.warnings.push(Diagnostic::new(Code::TextMarkerNotUsed, message));
        }
        Ok(())
    }

    /// The subpaths of `elements` wholly inside `area`, the polylines of every subpath of each element that
    /// has one of them, and how many subpaths lie more than half inside without lying wholly inside.
    #[allow(clippy::type_complexity)]
    fn taken(&mut self, area: &Area, elements: &[Element]) -> Result<(Vec<Taken>, BTreeMap<usize, Vec<Vec<At>>>, usize), Stop> {
        let (mut taken, mut flats, mut crossing) = (Vec::new(), BTreeMap::new(), 0);
        let cost = area.cost();
        for (element, shaped) in elements.iter().enumerate() {
            self.meter.charge(1).map_err(|_| self.spent())?;
            let pieces = flat(shaped.shape.path(), self.meter).map_err(|_| self.spent())?;
            let before = taken.len();
            for (subpath, points) in pieces.iter().enumerate() {
                self.meter.charge(cost.saturating_mul(u64::try_from(points.len()).unwrap_or(u64::MAX))).map_err(|_| self.spent())?;
                let held = points.iter().filter(|&&p| area.holds(p)).count();
                if held == points.len() {
                    taken.push(Taken { element, subpath, points: points.clone() });
                } else if 2 * held > points.len() {
                    crossing += 1;
                }
            }
            if taken.len() > before {
                flats.insert(element, pieces);
            }
        }
        Ok((taken, flats, crossing))
    }

    /// The name of the font the text of `marker` is set in, and what is said of a choice: the font `asked`
    /// names, or else the one of the choices nearest in weight to the letters among those that sew at
    /// their `height` ([`chosen`]), or else the host's default font. The letters are the subpaths of
    /// `taken` in `thread`, the thread that painted the most of what was taken out, so the holes they
    /// leave in the colour round them are not measured.
    fn choice(
        &mut self,
        marker: &Marker,
        asked: &Asked,
        taken: &[Taken],
        thread: Option<&Thread>,
        elements: &[Element],
        height: f64,
    ) -> Result<(String, Option<Diagnostic>), Stop> {
        if !asked.font.is_empty() || self.choices.len() < 2 || !(height > 0.0) {
            return Ok((asked.font.clone(), None));
        }
        let (mut rings, mut widths) = (Vec::new(), Vec::new());
        for part in taken {
            let Some(element) = elements.get(part.element).filter(|element| Some(&element.thread) == thread) else { continue };
            match &element.shape {
                Shape::Stroke { width, .. } => widths.push((width.get(), frame::length(&part.points))),
                Shape::Fill { .. } => rings.push(part.points.clone()),
            }
        }
        weight::samples(&rings, self.meter, &mut widths).map_err(|_| self.spent())?;
        let bold = weight::median(widths).map(|stroke| stroke / height);
        let Some(choice) = chosen(self.choices, bold, height).and_then(|index| self.choices.get(index)) else {
            return Ok((asked.font.clone(), None));
        };
        Ok((choice.font.clone(), Some(chose(marker, self.choices, choice, bold, height))))
    }

    /// The font `asked` names, as far as setting `text` needs it, or why the marker isn't used.
    fn font(&mut self, asked: &str, text: &str) -> Result<Rc<Font>, Stop> {
        let named = if asked.is_empty() { "The default embroidery font".to_string() } else { format!("The embroidery font `{asked}`") };
        match (self.fonts)(asked, text, self.budget) {
            Found::Font(font) => Ok(font),
            Found::Unread(unread) if unread.code == Code::BudgetExhausted => Err(Stop::Failed(unread)),
            Found::Unread(unread) => Err(Stop::Unused(format!("{named} does not read. {}", unread.message))),
            Found::Missing(names) if names.is_empty() => Err(Stop::Unused(format!("{named} is not here, and no other is."))),
            Found::Missing(names) => Err(Stop::Unused(format!("{named} is not here. The fonts here are {}.", names.join(", ")))),
        }
    }

    /// The box round what `trial` sews, from its first line's start on its baseline: its left, top, right
    /// and bottom.
    fn ink(&mut self, trial: &Set) -> Result<[f64; 4], Stop> {
        let mut ink: Option<[f64; 4]> = None;
        for element in trial.design.elements() {
            let pieces = flat(element.shape.path(), self.meter).map_err(|_| self.spent())?;
            ink = bounds(ink.iter().flat_map(|&[left, top, right, bottom]| [(left, top), (right, bottom)]).chain(pieces.into_iter().flatten()));
        }
        Ok(ink.unwrap_or_default())
    }

    /// The thread that painted the most of `taken`: of each thread, the areas its taken subpaths fill, less
    /// the areas of those that are holes, and the areas its taken lines cover. `flats` holds the polylines of
    /// every subpath of each element of `elements` that has one taken.
    fn thread(&mut self, taken: &[Taken], flats: &BTreeMap<usize, Vec<Vec<At>>>, elements: &[Element]) -> Result<Option<Thread>, Stop> {
        let mut painted: Vec<(Thread, f64)> = Vec::new();
        for part in taken {
            let (Some(element), Some(rings)) = (elements.get(part.element), flats.get(&part.element)) else { continue };
            let points: usize = rings.iter().map(Vec::len).sum();
            self.meter.charge(u64::try_from(points).unwrap_or(u64::MAX)).map_err(|_| self.spent())?;
            let size = match &element.shape {
                Shape::Stroke { width, .. } => frame::length(&part.points) * width.get(),
                Shape::Fill { .. } => {
                    // A subpath inside an odd number of its element's other subpaths is a hole.
                    let first = part.points.first().copied().unwrap_or_default();
                    let around =
                        rings.iter().enumerate().filter(|(at, ring)| *at != part.subpath && frame::inside(std::slice::from_ref(ring), first));
                    if around.count() % 2 == 1 { -frame::area(&part.points) } else { frame::area(&part.points) }
                }
            };
            match painted.iter_mut().find(|(thread, _)| *thread == element.thread) {
                Some((_, sum)) => *sum += size,
                None => painted.push((element.thread.clone(), size)),
            }
        }
        let mut most: Option<(Thread, f64)> = None;
        for (thread, sum) in painted {
            if most.as_ref().is_none_or(|(_, best)| sum > *best) {
                most = Some((thread, sum));
            }
        }
        Ok(most.map(|(thread, _)| thread))
    }
}

/// `shape` with `path` for its path.
fn with_path(shape: &Shape, path: Path) -> Shape {
    match shape {
        Shape::Stroke { width, join, cap, .. } => Shape::Stroke { path, width: *width, join: *join, cap: *cap },
        Shape::Fill { rule, gradient, .. } => Shape::Fill { path, rule: *rule, gradient: gradient.clone() },
    }
}
