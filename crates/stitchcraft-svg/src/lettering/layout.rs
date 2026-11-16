//! The layout of a text's lines (`docs/src/design/fonts.md` › Lettering, `REQ-FONT-009` to `REQ-FONT-011`):
//! which glyph sews for each character, and where each glyph's box lies, before the scale.
//!
//! **Why apart from the placing.** The layout is numbers alone, the font's and the user's, so its cases
//! check boxes and no stitches. It follows Ink/Stitch's lettering, read at `d59c9ab` and described on the
//! design page, so that a text takes the room in StitchCraft that it takes there.
//!
//! - **Lines and words.** The text is read in Unicode's NFC, as glyphs are named. A line ends at a line
//!   feed and loses the spaces at its ends, and its words lie between its spaces.
//! - **Glyphs.** A word is in the case the font forces. At each character, the glyph with the longest name
//!   that the rest of the word starts with sews, so a font's ligature sews for its letters. A character
//!   without one sews as the font's default glyph, and is remembered. Without that glyph too, the pen moves
//!   on by a space. A name that ends in `.isol`, `.init`, `.medi` or `.fina` is a form of an Arabic letter,
//!   which Ink/Stitch chooses by the letter's neighbours. This step chooses none.
//! - **Places.** A word's first glyph has its box's left edge at the pen. Each later glyph's origin lies its
//!   predecessor's advance after that one's origin, less the pair's kerning, plus the letter spacing, and
//!   its box lies its own left edge after its origin. After a word the pen lies the last glyph's advance
//!   after its origin, plus a space's advance and the word spacing.
//! - **Right to left.** In a font whose `text_direction` is `rtl`, a line's words and each word's glyphs are
//!   laid in the reverse of their order in the text, and a pair's kerning is looked up with the glyph that
//!   comes first in the text first.
//! - **Back and forth.** The lines take their glyphs in turn from 2 directions of the font, which are the
//!   same one when the text sews one way: a font draws a glyph for each direction it sews in.
//! - **Lines.** Each line's baseline lies the font's leading plus the line height below the line before.
//! - **Alignment.** A line's width spans its glyphs' boxes. A centred line moves left by half of it, and a
//!   right-aligned one by all of it. A block's line moves each of its words right by its place in the line
//!   times an equal share of what the line lacks to the widest line. With letter spacing, a line of one word
//!   spreads its glyphs so.

use std::collections::BTreeSet;
use std::ops::{Bound, Range};

use stitchcraft_core::{Exhausted, Meter};
use unicode_normalization::UnicodeNormalization;

use super::{Align, Lettering};
use crate::font::{Direction, Font, Glyph, LetterCase, TextDirection, Variant};

/// The endings of the names of an Arabic letter's forms.
const FORMS: [&str; 4] = ["isol", "init", "medi", "fina"];

/// A glyph of a text, where the layout puts it, in the font's millimetres.
#[derive(Clone, Debug)]
pub(super) struct Spot<'f> {
    /// The glyph.
    pub glyph: &'f Glyph,
    /// Its line, from 0.
    pub line: usize,
    /// Its word in the line, from 0, as laid out.
    pub word: usize,
    /// Its box's left edge.
    pub left: f64,
    /// Its line's baseline, down from the first line's.
    pub baseline: f64,
}

/// A text laid out.
#[derive(Clone, Debug, Default)]
pub(super) struct Laid<'f> {
    /// Its glyphs, line by line, in the order they are laid out.
    pub spots: Vec<Spot<'f>>,
    /// Each line that has glyphs, as the range of its spots, with the direction its glyphs are drawn to sew
    /// in.
    pub lines: Vec<(Range<usize>, Direction)>,
    /// The characters the font has no glyph for.
    pub missing: BTreeSet<char>,
}

/// `text` laid out in `font` as `how` says, its lines in turn in the glyphs of the first and of the second of
/// `variants`, charging `meter` a unit for each character and for each glyph name tried for it.
pub(super) fn lay<'f>(font: &'f Font, variants: [&'f Variant; 2], text: &str, how: &Lettering, meter: &mut Meter) -> Result<Laid<'f>, Exhausted> {
    let text: String = text.nfc().collect();
    let rtl = font.settings.text_direction == TextDirection::RightToLeft;
    let mut laid = Laid::default();
    // Each line's spots, with its direction, the number of its words and its width.
    let mut lines: Vec<(Range<usize>, Direction, usize, f64)> = Vec::new();
    for (number, line) in text.lines().enumerate() {
        let [even, odd] = variants;
        let variant = if number % 2 == 1 { odd } else { even };
        let default = variant.glyphs.get(&font.settings.default_glyph);
        let baseline = number as f64 * (font.leading_mm() + how.line_height);
        let first = laid.spots.len();
        let mut words: Vec<&str> = line.trim().split(' ').collect();
        if rtl {
            words.reverse();
        }
        let mut pen = 0.0;
        for (index, word) in words.iter().enumerate() {
            let word = match font.settings.letter_case {
                LetterCase::Upper => word.to_uppercase(),
                LetterCase::Lower => word.to_lowercase(),
                LetterCase::AsTyped => (*word).to_string(),
            };
            let mut glyphs = matched(variant, &word, default, &mut laid.missing, meter)?;
            if rtl {
                glyphs.reverse();
            }
            // The origin of the glyph before in the word, and that glyph.
            let mut before: Option<(f64, &Glyph)> = None;
            for glyph in glyphs {
                let Some(glyph) = glyph else {
                    pen += font.space_mm();
                    before = None;
                    continue;
                };
                let left = match before {
                    None => pen,
                    Some((origin, last)) => {
                        let kerning = if rtl { font.kerning_mm(&glyph.label, &last.label) } else { font.kerning_mm(&last.label, &glyph.label) };
                        origin + font.advance_mm(last) - kerning + how.letter_spacing + glyph.left
                    }
                };
                let origin = left - glyph.left;
                pen = origin + font.advance_mm(glyph);
                before = Some((origin, glyph));
                laid.spots.push(Spot { glyph, line: number, word: index, left, baseline });
            }
            pen += font.space_mm() + how.word_spacing;
        }
        let spots = laid.spots.get(first..).unwrap_or_default();
        if !spots.is_empty() {
            let left = spots.iter().map(|spot| spot.left).fold(f64::INFINITY, f64::min);
            let right = spots.iter().map(|spot| spot.left + spot.glyph.width).fold(f64::NEG_INFINITY, f64::max);
            lines.push((first..laid.spots.len(), variant.direction, words.len(), right - left));
        }
    }
    let widest = lines.iter().map(|(.., width)| *width).fold(0.0, f64::max);
    for (range, _, words, width) in &lines {
        let Some(spots) = laid.spots.get_mut(range.clone()) else { continue };
        let spread = how.align == Align::LetterSpacing && *words == 1;
        let parts = if spread { spots.len() } else { *words };
        for (index, spot) in spots.iter_mut().enumerate() {
            spot.left += match how.align {
                Align::Left => 0.0,
                Align::Centre => -width / 2.0,
                Align::Right => -width,
                Align::Block | Align::LetterSpacing if parts > 1 => {
                    (widest - width) / (parts - 1) as f64 * if spread { index } else { spot.word } as f64
                }
                Align::Block | Align::LetterSpacing => 0.0,
            };
        }
    }
    laid.lines = lines.into_iter().map(|(range, direction, ..)| (range, direction)).collect();
    Ok(laid)
}

/// The glyphs of `word` in `variant`: at each character, the glyph with the longest name the rest of the
/// word starts with, or else `default`, the character then added to `missing`.
fn matched<'f>(
    variant: &'f Variant,
    word: &str,
    default: Option<&'f Glyph>,
    missing: &mut BTreeSet<char>,
    meter: &mut Meter,
) -> Result<Vec<Option<&'f Glyph>>, Exhausted> {
    let mut glyphs = Vec::new();
    let mut rest = word;
    while let Some(first) = rest.chars().next() {
        meter.charge(1)?;
        let from = first.to_string();
        let mut found: Option<&Glyph> = None;
        // The names that start with the character lie together, a shorter one before a longer that starts
        // with it, so the last that fits is the longest.
        for (name, glyph) in variant.glyphs.range::<str, _>((Bound::Included(from.as_str()), Bound::Unbounded)) {
            if !name.starts_with(first) {
                break;
            }
            meter.charge(1)?;
            let form = name.split_once('.').is_some_and(|(_, form)| FORMS.contains(&form));
            if rest.starts_with(name.as_str()) && !form {
                found = Some(glyph);
            }
        }
        match found {
            Some(glyph) => {
                glyphs.push(Some(glyph));
                rest = rest.strip_prefix(glyph.name.as_str()).unwrap_or_default();
            }
            None => {
                missing.insert(first);
                glyphs.push(default);
                let mut after = rest.chars();
                after.next();
                rest = after.as_str();
            }
        }
    }
    Ok(glyphs)
}
