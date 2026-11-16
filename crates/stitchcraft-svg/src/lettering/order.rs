//! What lettering does to a text's elements once they are placed (roadmap step M14.3,
//! `docs/src/design/fonts.md` › Trims and › Sorted colours, `REQ-FONT-018` and `REQ-FONT-019`): the trims
//! after letters, words or lines, and the colours of a multicolour font sorted.
//!
//! **Why apart from the placing.** Both work on the order the elements sew in and on their settings, never
//! on where they lie, so their cases count trims and read the threads' sequence.
//!
//! - **Trims.** A trim is the setting `trim_after` on the last element a glyph sews, which the planner reads
//!   as it reads any element's. With the colours sorted, the last element of each of the glyph's colours gets
//!   one, as its colours no longer sew together.
//! - **Sorted colours.** A font's author numbers each element's colour with `color_sort_index`. The elements
//!   of a whole text, of each line or of each word sew in the order of those numbers, and those of one
//!   number in the order they sewed in before. An element without a number sews as number [`UNNUMBERED`].
//!   The elements of a number that `combine_at_sort_indices` lists become one element, the first's, with the
//!   subpaths of them all.

use stitchcraft_core::{Exhausted, Meter};
use stitchcraft_engine::design::{Element, Shape};

/// After which glyphs lettering adds a trim: Ink/Stitch's `trim`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Trim {
    /// After none: a glyph's own trims stay.
    #[default]
    Off,
    /// After each line but the last.
    Line,
    /// After each word but the text's last.
    Word,
    /// After each glyph.
    Glyph,
}

impl Trim {
    /// Every choice, in the order Ink/Stitch lists them.
    pub const ALL: [Trim; 4] = [Trim::Off, Trim::Line, Trim::Word, Trim::Glyph];

    /// Its name, as Ink/Stitch's batch lettering names it.
    pub const fn id(self) -> &'static str {
        match self {
            Trim::Off => "off",
            Trim::Line => "line",
            Trim::Word => "word",
            Trim::Glyph => "glyph",
        }
    }

    /// The choice `text` names.
    pub fn named(text: &str) -> Option<Trim> {
        Trim::ALL.into_iter().find(|trim| trim.id() == text)
    }
}

/// Which elements of a multicolour font sew sorted by colour: Ink/Stitch's `color-sort`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ColourSort {
    /// None: each glyph sews its colours before the next glyph.
    #[default]
    Off,
    /// The whole text's.
    All,
    /// Each line's.
    Line,
    /// Each word's.
    Word,
}

impl ColourSort {
    /// Every choice, in the order Ink/Stitch lists them.
    pub const ALL: [ColourSort; 4] = [ColourSort::Off, ColourSort::All, ColourSort::Line, ColourSort::Word];

    /// Its name, as Ink/Stitch's batch lettering names it.
    pub const fn id(self) -> &'static str {
        match self {
            ColourSort::Off => "off",
            ColourSort::All => "all",
            ColourSort::Line => "line",
            ColourSort::Word => "word",
        }
    }

    /// The choice `text` names.
    pub fn named(text: &str) -> Option<ColourSort> {
        ColourSort::ALL.into_iter().find(|sort| sort.id() == text)
    }
}

/// The setting a font's author numbers an element's colour with.
pub(super) const COLOR_SORT_INDEX: &str = "color_sort_index";

/// The number an element without `color_sort_index` sorts as, as in Ink/Stitch.
pub(super) const UNNUMBERED: i64 = 404;

/// The setting that trims the thread after an element.
const TRIM_AFTER: &str = "trim_after";

/// An element of a text, with the glyph it is of: that glyph's place among the glyphs sewn, its line and
/// its word in the line.
#[derive(Clone, Debug, PartialEq)]
pub(super) struct Sewn {
    /// The place of its glyph among the glyphs sewn.
    pub glyph: usize,
    /// Its glyph's line.
    pub line: usize,
    /// Its glyph's word in the line.
    pub word: usize,
    /// The element.
    pub element: Element,
}

/// The number `element`'s colour sorts by.
pub(super) fn sort_index(element: &Element) -> i64 {
    element.params.get(COLOR_SORT_INDEX).and_then(|text| text.trim().parse().ok()).unwrap_or(UNNUMBERED)
}

/// Adds the trims `trim` asks for to `elements`, which are in the order they sew, each glyph's together: on
/// the last element of each glyph a trim follows, or, with `by_colour`, on the last of each of that glyph's
/// colours. Charges `meter` a unit for each element.
pub(super) fn trim(elements: &mut [Sewn], trim: Trim, by_colour: bool, meter: &mut Meter) -> Result<(), Exhausted> {
    meter.charge(u64::try_from(elements.len()).unwrap_or(u64::MAX))?;
    // Going back from the text's end: the glyph sewn after the one at hand, and the colours of the glyph at
    // hand that have their trim.
    let mut after: Option<(usize, usize)> = None;
    let mut now: Option<(usize, usize, usize)> = None;
    let mut done: Vec<i64> = Vec::new();
    for sewn in elements.iter_mut().rev() {
        let this = (sewn.glyph, sewn.line, sewn.word);
        if now != Some(this) {
            after = now.map(|(_, line, word)| (line, word));
            now = Some(this);
            done.clear();
        }
        let ends = match trim {
            Trim::Off => false,
            Trim::Glyph => true,
            Trim::Word => after.is_some_and(|next| next != (sewn.line, sewn.word)),
            Trim::Line => after.is_some_and(|(line, _)| line != sewn.line),
        };
        let colour = if by_colour { sort_index(&sewn.element) } else { UNNUMBERED };
        if ends && !done.contains(&colour) {
            done.push(colour);
            sewn.element.params.set(TRIM_AFTER, "true");
        }
    }
    Ok(())
}

/// `elements`, which are in the order they sew, with the colours of each `scope` sorted: the elements of the
/// whole text, of a line or of a word in the order of their numbers, and those of one number as they were.
/// The elements of a number in `combine` become one. Charges `meter` a unit for each element and each
/// subpath joined.
pub(super) fn sort(elements: Vec<Sewn>, scope: ColourSort, combine: &[i64], meter: &mut Meter) -> Result<Vec<Sewn>, Exhausted> {
    meter.charge(u64::try_from(elements.len()).unwrap_or(u64::MAX))?;
    if scope == ColourSort::Off {
        return Ok(elements);
    }
    // Each element with the place of its scope among the scopes sewn, and its number.
    let mut keyed: Vec<((usize, i64), Sewn)> = Vec::with_capacity(elements.len());
    let mut scopes = 0;
    let mut before: Option<(usize, usize)> = None;
    for sewn in elements {
        let part = match scope {
            ColourSort::Off | ColourSort::All => (0, 0),
            ColourSort::Line => (sewn.line, 0),
            ColourSort::Word => (sewn.line, sewn.word),
        };
        if before.is_some_and(|last| last != part) {
            scopes += 1;
        }
        before = Some(part);
        keyed.push(((scopes, sort_index(&sewn.element)), sewn));
    }
    keyed.sort_by_key(|(key, _)| *key);
    let mut sorted: Vec<Sewn> = Vec::with_capacity(keyed.len());
    let mut last: Option<(usize, i64)> = None;
    for (key, sewn) in keyed {
        match sorted.last_mut().filter(|_| last == Some(key) && combine.contains(&key.1)) {
            Some(first) => join(&mut first.element, sewn.element, meter)?,
            None => sorted.push(sewn),
        }
        last = Some(key);
    }
    Ok(sorted)
}

/// Adds the subpaths of `more` to `first`, which then ends where it is told to when it is a tartan or a
/// linear gradient fill, as Ink/Stitch's lettering sets it.
fn join(first: &mut Element, more: Element, meter: &mut Meter) -> Result<(), Exhausted> {
    let (Shape::Stroke { path: added, .. } | Shape::Fill { path: added, .. }) = more.shape;
    meter.charge(u64::try_from(added.subpaths.len()).unwrap_or(u64::MAX))?;
    let (Shape::Stroke { path, .. } | Shape::Fill { path, .. }) = &mut first.shape;
    path.subpaths.extend(added.subpaths);
    if matches!(first.params.get("fill_method"), Some("tartan_fill" | "linear_gradient_fill")) {
        first.params.set("stop_at_ending_point", "true");
    }
    Ok(())
}
