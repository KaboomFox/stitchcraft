//! Embroidery fonts in Ink/Stitch's format (roadmap step M14.1, `docs/src/design/fonts.md` › The reader,
//! ADR-0020): a font's folder read into its settings, its licence, its directions and their glyphs, each a
//! design of its own with the box and the baseline lettering places it by.
//!
//! **A font is the files of its folder**, which [`read`] takes as bytes. So the command line reads a folder
//! on disk, and the VectorCraft plug-in, which can't read files, reads fonts built into it. Nothing here
//! touches a file system.
//!
//! - **Settings**: `font.json` ([`Settings`], [`KEYS`]).
//! - **Directions**: for each of the 4 directions, its glyph files: `ltr.svg`, and the SVG files directly in a
//!   folder `ltr`, in the order of their names, or else an older font's `→.svg`; and so on for `rtl`, `ttb`
//!   and `btt` ([`Direction`]). Ink/Stitch takes a folder's files in the order the system lists them in.
//!   A file compressed with xz, as Ink/Stitch's own packages ship their fonts, is named (`SC-W1103`).
//! - **Glyphs**: every group of a glyph file labelled `GlyphLayer-` and a name ([`PREFIX`]). A glyph is found
//!   by its name, its label in Unicode's NFC, as Ink/Stitch's lettering finds it. Of 2 glyphs of one name in
//!   a direction, the later in the order of the files and of each file counts (`SC-I1105`).
//! - **Each glyph's design** is its glyph file with its group shown and the rest of the drawing left out, as
//!   the glyph differential sews it, read by the SVG reader with what it says of it ([`Glyph`]).
//! - **The licence**: the licence file, and the licence it names first ([`licence`]).
//!
//! **What lettering builds on.** The reader also reads what each key means where Ink/Stitch's lettering
//! reads more than the value: a pair's kerning, with an older font's pairs, a glyph's advance, with its
//! defaults, the heights the font sews at and its default direction ([`Font`]'s methods). Setting text is
//! M14.2's: matching characters to glyphs, the default glyph for a missing one, the spacing, the lines and
//! their alignment, and auto-satin.
//!
//! **Never crashes, bounded.** Any bytes give a font or a coded error. One meter of the budget counts the
//! whole read: a unit per 8 bytes of `font.json` and of each glyph file, per glyph group, per node and path
//! segment measured for a box, per edit and per 8 bytes of each glyph's design, and what the SVG reader
//! charges for each design. A font that needs more is refused with `SC-E0004`.

mod glyphs;
pub mod licence;
mod settings;

use std::collections::BTreeMap;

use stitchcraft_core::units::MM_PER_SVG_PX;
use stitchcraft_core::{Budget, Code, Diagnostic, Fix, Meter};
use stitchcraft_engine::design::{Design, Reading};
use unicode_normalization::UnicodeNormalization;

pub use glyphs::{Cut, PREFIX, each_glyph};
pub use licence::Licence;
pub use settings::{DESCRIPTION_IN, KEYS, Key, LetterCase, Settings, TextDirection, shown};

use crate::document;
use crate::text;
use glyphs::NoBox;

/// The name of a font's settings file.
pub const SETTINGS: &str = "font.json";

/// The ending of a file compressed with xz, as Ink/Stitch's packages compress their fonts' files.
const XZ: &str = ".xz";

/// A direction a font sews in: one of Ink/Stitch's font variants.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Direction {
    /// Left to right, `ltr`.
    LeftToRight,
    /// Right to left, `rtl`.
    RightToLeft,
    /// Top to bottom, `ttb`.
    TopToBottom,
    /// Bottom to top, `btt`.
    BottomToTop,
}

impl Direction {
    /// Every direction, in the order Ink/Stitch looks for their files.
    pub const ALL: [Direction; 4] = [Direction::LeftToRight, Direction::RightToLeft, Direction::TopToBottom, Direction::BottomToTop];

    /// Its id, which names its glyph file and its folder.
    pub const fn id(self) -> &'static str {
        match self {
            Direction::LeftToRight => "ltr",
            Direction::RightToLeft => "rtl",
            Direction::TopToBottom => "ttb",
            Direction::BottomToTop => "btt",
        }
    }

    /// The arrow an older font names its glyph file by.
    pub const fn arrow(self) -> &'static str {
        match self {
            Direction::LeftToRight => "→",
            Direction::RightToLeft => "←",
            Direction::TopToBottom => "↓",
            Direction::BottomToTop => "↑",
        }
    }

    /// The direction back, which lettering sews every other line in.
    pub const fn reversed(self) -> Direction {
        match self {
            Direction::LeftToRight => Direction::RightToLeft,
            Direction::RightToLeft => Direction::LeftToRight,
            Direction::TopToBottom => Direction::BottomToTop,
            Direction::BottomToTop => Direction::TopToBottom,
        }
    }

    /// The direction `text` names, by its id or its arrow.
    pub fn named(text: &str) -> Option<Direction> {
        Direction::ALL.into_iter().find(|d| d.id() == text || d.arrow() == text)
    }
}

/// A font in Ink/Stitch's format, read.
#[derive(Clone, Debug, PartialEq)]
pub struct Font {
    /// Its folder's name, which Ink/Stitch finds it by too.
    pub folder: String,
    /// Its settings, from `font.json`.
    pub settings: Settings,
    /// Its licence file, if it has one.
    pub licence: Option<LicenceFile>,
    /// The directions it has glyph files for, in the order of [`Direction::ALL`].
    pub variants: Vec<Variant>,
    /// What reading it said of the font, its files and its glyphs, in the order it read them. What the SVG
    /// reader said of each glyph's design is the glyph's.
    pub warnings: Vec<Diagnostic>,
}

/// A font's licence file.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LicenceFile {
    /// Its path in the font's folder.
    pub path: String,
    /// The licence it names first.
    pub licence: Licence,
    /// Its text, with any bytes that are no UTF-8 replaced.
    pub text: String,
}

/// A direction of a font, with its glyphs.
#[derive(Clone, Debug, PartialEq)]
pub struct Variant {
    /// The direction.
    pub direction: Direction,
    /// Its glyph files, by path in the font's folder, in the order they were read.
    pub files: Vec<String>,
    /// Its glyphs, by name.
    pub glyphs: BTreeMap<String, Glyph>,
}

/// A glyph of a font.
#[derive(Clone, Debug, PartialEq)]
pub struct Glyph {
    /// Its name, which lettering finds it by: its label in Unicode's NFC.
    pub name: String,
    /// Its label: its group's `inkscape:label` after `GlyphLayer-`, as the file writes it. Ink/Stitch's
    /// lettering looks a glyph's advance and kerning up by it.
    pub label: String,
    /// The glyph file it is drawn in, by path in the font's folder.
    pub file: String,
    /// Its elements: its glyph file with its group shown and the rest of the drawing left out, read by the
    /// SVG reader, in millimetres on the file's page.
    pub design: Design,
    /// What the SVG reader said of its design.
    pub warnings: Vec<Diagnostic>,
    /// Its box's left edge, in millimetres on the page: where lettering puts the glyph's origin across.
    pub left: f64,
    /// Its box's width, in millimetres.
    pub width: f64,
    /// Its file's baseline, in millimetres from the page's top: where lettering puts the glyph's origin down.
    pub baseline: f64,
}

impl Font {
    /// The glyphs of `direction`, if the font sews that way.
    pub fn variant(&self, direction: Direction) -> Option<&Variant> {
        self.variants.iter().find(|variant| variant.direction == direction)
    }

    /// The direction lettering sews in by default: `default_variant`'s, when the font has glyph files for
    /// it, and else its first.
    pub fn default_variant(&self) -> Option<&Variant> {
        self.variant(self.settings.default_variant).or_else(|| self.variants.first())
    }

    /// The kerning between the glyphs labelled `first` and `second`, `first` before `second` in
    /// `kerning_pairs`, in millimetres: the value of the 2 labels with a space between them, or else joined,
    /// as an older font writes them, or else 0.
    pub fn kerning_mm(&self, first: &str, second: &str) -> f64 {
        let pairs = &self.settings.kerning_pairs;
        pairs.get(&format!("{first} {second}")).or_else(|| pairs.get(&format!("{first}{second}"))).map_or(0.0, |k| k * MM_PER_SVG_PX)
    }

    /// The advance of `glyph`, in millimetres: `horiz_adv_x`'s value for its label, or else
    /// `horiz_adv_x_default`, or else its box's right edge.
    pub fn advance_mm(&self, glyph: &Glyph) -> f64 {
        let settings = &self.settings;
        match settings.horiz_adv_x.get(&glyph.label).copied().or(settings.horiz_adv_x_default) {
            Some(advance) => advance * MM_PER_SVG_PX,
            None => glyph.left + glyph.width,
        }
    }

    /// The advance of a space, `horiz_adv_x_space`, in millimetres.
    pub fn space_mm(&self) -> f64 {
        self.settings.horiz_adv_x_space * MM_PER_SVG_PX
    }

    /// The distance from one line's baseline to the next, `leading`, in millimetres.
    pub fn leading_mm(&self) -> f64 {
        self.settings.leading * MM_PER_SVG_PX
    }

    /// The least and the greatest height the font sews at, in millimetres: `size` times `min_scale` and
    /// `max_scale`.
    pub fn heights_mm(&self) -> (f64, f64) {
        let settings = &self.settings;
        (settings.size * settings.min_scale, settings.size * settings.max_scale)
    }
}

/// The font in the folder named `folder`, whose files are `files`, each by its path in the folder with `/`
/// between the path's parts, read within `budget`. Refused with `SC-E1101` when it has no `font.json` that
/// reads as an object, no glyph file it can read, or a glyph file the SVG reader refuses; and with
/// `SC-E0004` when reading it needs more work than `budget` allows.
pub fn read(folder: &str, files: &[(&str, &[u8])], budget: &Budget) -> Result<Font, Diagnostic> {
    let files: BTreeMap<&str, &[u8]> = files.iter().copied().collect();
    let mut meter = budget.meter();
    let mut warnings = Vec::new();
    let settings = read_settings(folder, &files, budget, &mut meter, &mut warnings)?;
    let licence = files.iter().find(|(path, _)| licence::is_licence_file(path)).map(|(path, bytes)| {
        let text = String::from_utf8_lossy(bytes).into_owned();
        LicenceFile { path: (*path).to_string(), licence: Licence::of(&text), text }
    });
    let mut variants = Vec::new();
    for direction in Direction::ALL {
        let paths = glyph_files(direction, &files, &mut warnings);
        if paths.is_empty() {
            continue;
        }
        let mut variant = Variant { direction, files: Vec::new(), glyphs: BTreeMap::new() };
        for path in paths {
            let bytes = files.get(path).copied().unwrap_or_default();
            read_glyph_file(folder, path, bytes, budget, &mut meter, &mut variant, &mut warnings)?;
            variant.files.push(path.to_string());
        }
        variants.push(variant);
    }
    if variants.is_empty() {
        let message = format!(
            "The font `{folder}` has no glyph file StitchCraft reads: no `ltr.svg`, `rtl.svg`, `ttb.svg` or `btt.svg`, and no folder of them."
        );
        return Err(unreadable(message, "A font in Ink/Stitch's format draws its glyphs in a file for each direction it sews in."));
    }
    let default = settings.default_variant;
    if let Some(first) = variants.first().filter(|_| !variants.iter().any(|variant| variant.direction == default)) {
        let message = format!(
            "`default_variant` is `{}`, and the font `{folder}` has no glyph file for it. It sews {} by default, its first direction, as in Ink/Stitch.",
            default.id(),
            first.direction.id()
        );
        warnings.push(Diagnostic::new(Code::FontReadAsInkStitch, message));
    }
    Ok(Font { folder: folder.to_string(), settings, licence, variants, warnings })
}

/// The settings of the font in `folder`, from its `font.json`.
fn read_settings(
    folder: &str,
    files: &BTreeMap<&str, &[u8]>,
    budget: &Budget,
    meter: &mut Meter,
    warnings: &mut Vec<Diagnostic>,
) -> Result<Settings, Diagnostic> {
    let generate = "Ink/Stitch's Extensions › Ink/Stitch › Font Management › Generate JSON writes one.";
    let Some(bytes) = files.get(SETTINGS) else {
        let compressed = files.contains_key(format!("{SETTINGS}{XZ}").as_str());
        let message = if compressed {
            format!("The font `{folder}` has its `{SETTINGS}` compressed with xz, which StitchCraft doesn't read.")
        } else {
            format!("The font `{folder}` has no `{SETTINGS}`, which gives a font its settings.")
        };
        return Err(unreadable(message, if compressed { "Unpack it with `xz --decompress font.json.xz`." } else { generate }));
    };
    meter.charge(glyphs::charge(bytes.len())).map_err(|_| exhausted(budget, "the font"))?;
    let bytes = bytes.strip_prefix(b"\xef\xbb\xbf").unwrap_or(bytes);
    let object = match serde_json::from_slice::<serde_json::Value>(bytes) {
        Ok(serde_json::Value::Object(object)) => object,
        Ok(other) => {
            let message = format!("The `{SETTINGS}` of the font `{folder}` holds {}, not an object of settings.", settings::described(&other));
            return Err(unreadable(message, generate));
        }
        Err(e) => return Err(unreadable(format!("The `{SETTINGS}` of the font `{folder}` doesn't read as JSON: {e}."), generate)),
    };
    let (settings, kept, unknown) = settings::read(&object);
    warnings.extend(kept.into_iter().map(|message| Diagnostic::new(Code::FontSettingDefault, message)));
    if !unknown.is_empty() {
        let names: Vec<String> = unknown.iter().map(|name| format!("`{name}`")).collect();
        let message = format!("`{SETTINGS}` has keys Ink/Stitch's lettering doesn't read, which change nothing: {}.", names.join(", "));
        warnings.push(Diagnostic::new(Code::FontReadAsInkStitch, message));
    }
    Ok(settings)
}

/// The glyph files of `direction` among `files`, in the order they are read: its file, the SVG files directly
/// in its folder in the order of their names, or else an older font's file. A compressed one is named in
/// `warnings`.
fn glyph_files<'f>(direction: Direction, files: &BTreeMap<&'f str, &[u8]>, warnings: &mut Vec<Diagnostic>) -> Vec<&'f str> {
    let id = direction.id();
    let of = |stem: &str| {
        let (file, folder) = (format!("{stem}.svg"), format!("{stem}/"));
        files
            .keys()
            .copied()
            .filter(|path| {
                let base = path.strip_suffix(XZ).unwrap_or(path);
                *base == file || base.strip_prefix(&folder).is_some_and(|name| !name.contains('/') && name.ends_with(".svg"))
            })
            .collect::<Vec<&str>>()
    };
    let mut found = of(id);
    if found.is_empty() {
        found = of(direction.arrow()).into_iter().filter(|path| !path.contains('/')).collect();
    }
    let (compressed, plain): (Vec<&str>, Vec<&str>) = found.into_iter().partition(|path| path.ends_with(XZ));
    for path in compressed {
        let message = format!("The glyph file `{path}` is compressed with xz, which StitchCraft doesn't read. Its glyphs are left out.");
        warnings.push(Diagnostic::new(Code::FontFileSkipped, message).with_fix(Fix::Hint(format!("Unpack it with `xz --decompress {path}`."))));
    }
    // The direction's own file first, then its folder's, as Ink/Stitch takes them.
    let (own, rest): (Vec<&str>, Vec<&str>) = plain.into_iter().partition(|path| !path.contains('/'));
    own.into_iter().chain(rest).collect()
}

/// Reads the glyph file at `path` of the font in `folder` into `variant`.
fn read_glyph_file(
    folder: &str,
    path: &str,
    bytes: &[u8],
    budget: &Budget,
    meter: &mut Meter,
    variant: &mut Variant,
    warnings: &mut Vec<Diagnostic>,
) -> Result<(), Diagnostic> {
    let refused = |d: Diagnostic| {
        let fix = "Open the file in Inkscape, check the glyphs drawn in it, and save it again.";
        unreadable(format!("The glyph file `{path}` of the font `{folder}` doesn't read: {}", d.message), fix)
    };
    let text = text::decode(bytes).map_err(&refused)?;
    text::check_entities(&text).map_err(&refused)?;
    meter.charge(glyphs::charge(text.len())).map_err(|_| exhausted(budget, "the font"))?;
    let doc = document::parse(&text).map_err(&refused)?;
    let root = doc.root_element();
    let (tag, namespace) = (root.tag_name().name(), root.tag_name().namespace());
    if tag != "svg" || namespace.is_some_and(|namespace| namespace != glyphs::SVG) {
        return Err(refused(Diagnostic::new(Code::SvgUnreadable, format!("its root element is <{tag}>, not SVG's <svg>."))));
    }
    let page = glyphs::page_of(root).map_err(&refused)?;
    let baseline = match glyphs::baseline(root, &page) {
        Ok(Some(baseline)) => baseline,
        Ok(None) => {
            let message =
                format!("The glyph file `{path}` has no guide labelled `baseline`. Its glyphs' baseline is the page's top, as in Ink/Stitch.");
            warnings.push(Diagnostic::new(Code::FontReadAsInkStitch, message));
            0.0
        }
        Err(position) => {
            let glyphs = match glyphs::groups(&doc).count() {
                1 => "Its glyph is".to_string(),
                count => format!("Its {count} glyphs are"),
            };
            let message = format!(
                "The guide labelled `baseline` in the glyph file `{path}` is at \"{position}\", which isn't 2 numbers. {glyphs} left out, as Ink/Stitch's lettering leaves them out."
            );
            warnings
                .push(Diagnostic::new(Code::GlyphLeftOut, message).with_fix(Fix::Hint("Place the baseline guide again in Inkscape.".to_string())));
            return Ok(());
        }
    };
    for (group, label) in glyphs::groups(&doc) {
        meter.charge(1).map_err(|_| exhausted(budget, "the font"))?;
        let name: String = label.nfc().collect();
        let (left, right) = match glyphs::extent(group, &page, meter).map_err(|_| exhausted(budget, "the font"))? {
            Ok(extent) => extent,
            Err(why) => {
                let what = match why {
                    NoBox::NoPath => "has no path",
                    NoBox::Empty => "has a path that draws nothing",
                };
                let message =
                    format!("The glyph `{label}` in `{path}` {what}, so it has no width. It is left out, as Ink/Stitch's lettering leaves it out.");
                let fix = "Draw the glyph with paths, and delete the paths in its layer that draw nothing.";
                warnings.push(Diagnostic::new(Code::GlyphLeftOut, message).with_fix(Fix::Hint(fix.to_string())));
                continue;
            }
        };
        let design = glyphs::cut(&text, group, meter).map_err(|_| exhausted(budget, "the font"))?;
        let svg = document::read_text(&design, budget, meter, Reading::default())
            .map_err(|d| if d.code == Code::BudgetExhausted { exhausted(budget, "the font") } else { refused(d) })?;
        let glyph = Glyph {
            name: name.clone(),
            label: label.to_string(),
            file: path.to_string(),
            design: svg.design,
            warnings: svg.warnings,
            left,
            width: right - left,
            baseline,
        };
        if let Some(earlier) = variant.glyphs.insert(name, glyph) {
            let message = format!(
                "The glyph `{}` is drawn twice in the {} direction, in `{}` and again in `{path}`. The later counts, as in Ink/Stitch.",
                earlier.label,
                variant.direction.id(),
                earlier.file
            );
            warnings.push(Diagnostic::new(Code::FontReadAsInkStitch, message));
        }
    }
    Ok(())
}

/// `SC-E1101`: the font isn't read, for `message`, with `fix`.
fn unreadable(message: String, fix: &str) -> Diagnostic {
    Diagnostic::new(Code::FontUnreadable, message).with_fix(Fix::Hint(fix.to_string()))
}

/// `SC-E0004` for `what` when reading it needs more work than `budget` allows.
fn exhausted(budget: &Budget, what: &str) -> Diagnostic {
    Diagnostic::new(Code::BudgetExhausted, format!("Reading {what} needed more than the work budget of {} units.", budget.max_work))
        .with_fix(Fix::Hint("Split the font's glyphs over more glyph files, or read it with a larger budget.".to_string()))
}
