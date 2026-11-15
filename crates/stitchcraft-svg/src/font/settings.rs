//! A font's settings: its `font.json` (roadmap step M14.1, `docs/src/design/fonts.md` › The reader).
//!
//! **One table.** [`KEYS`] lists each key that Ink/Stitch's lettering and its font tools read, read at
//! `d59c9ab` (`lib/lettering/font.py` and `lib/extensions/lettering_generate_json.py`). Each entry has the
//! key's name, what it takes and does, how it is read into [`Settings`], and its value there. The font
//! settings page of the user's guide is written from the table, each key at [`Settings::default`], which
//! holds Ink/Stitch's defaults. The names and the defaults are the interoperability contract, used as they
//! are.
//!
//! **Read as Ink/Stitch reads it.** The file is UTF-8 text, with or without a byte order mark, and holds a
//! JSON object. A key the object lacks keeps its default. A value of another type, or a word the key
//! doesn't take, keeps the default too, and `SC-W1102` names it. So does an entry of a list or a table that
//! isn't of its type, which is left out. A key `description_` and a language code is the description in
//! that language, as Ink/Stitch looks it up. Another key changes nothing, as in Ink/Stitch, and `SC-I1105`
//! names it. Of 2 entries of one name in an object the later counts, as Python's reader takes it.
//!
//! **Units.** `size` is in millimetres. The other lengths are in the glyph files' CSS pixels, 1/96 inch, in
//! which Ink/Stitch's lettering adds them up. [`super::Font`] gives them in millimetres.
//!
//! **Bounded.** Reading the file costs a unit of work per 8 bytes, which bounds its entries too.

use std::collections::BTreeMap;

use serde_json::Value;

use super::Direction;

/// Which way the font's script runs: Ink/Stitch's `text_direction`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TextDirection {
    /// Left to right, `ltr`.
    #[default]
    LeftToRight,
    /// Right to left, `rtl`: lettering reverses each line's characters before it sets them.
    RightToLeft,
}

/// The case text is set in: Ink/Stitch's `letter_case`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum LetterCase {
    /// As typed, `""`.
    #[default]
    AsTyped,
    /// In capitals, `upper`.
    Upper,
    /// In small letters, `lower`.
    Lower,
}

/// A font's settings, as its `font.json` gives them, each at Ink/Stitch's default where the file lacks it.
/// Each field is the key of its name in [`KEYS`].
#[derive(Clone, Debug, PartialEq)]
pub struct Settings {
    /// `name`.
    pub name: String,
    /// `description`.
    pub description: String,
    /// The keys `description_` and a language code: the description in each language, by its code.
    pub descriptions: BTreeMap<String, String>,
    /// `keywords`.
    pub keywords: Vec<String>,
    /// `font_license`.
    pub font_license: String,
    /// `original_font`.
    pub original_font: String,
    /// `original_font_url`.
    pub original_font_url: String,
    /// `text_direction`.
    pub text_direction: TextDirection,
    /// `default_variant`.
    pub default_variant: Direction,
    /// `letter_case`.
    pub letter_case: LetterCase,
    /// `default_glyph`.
    pub default_glyph: String,
    /// `size`, in millimetres.
    pub size: f64,
    /// `min_scale`.
    pub min_scale: f64,
    /// `max_scale`.
    pub max_scale: f64,
    /// `leading`, in CSS pixels.
    pub leading: f64,
    /// `horiz_adv_x`, in CSS pixels, by glyph label.
    pub horiz_adv_x: BTreeMap<String, f64>,
    /// `horiz_adv_x_default`, in CSS pixels.
    pub horiz_adv_x_default: Option<f64>,
    /// `horiz_adv_x_space`, in CSS pixels.
    pub horiz_adv_x_space: f64,
    /// `kerning_pairs`, in CSS pixels, by pair.
    pub kerning_pairs: BTreeMap<String, f64>,
    /// `units_per_em`, in CSS pixels.
    pub units_per_em: Option<f64>,
    /// `auto_satin`.
    pub auto_satin: bool,
    /// `reversible`.
    pub reversible: bool,
    /// `sortable`.
    pub sortable: bool,
    /// `combine_at_sort_indices`.
    pub combine_at_sort_indices: Vec<i64>,
    /// `scale_cross_stitch_pattern`.
    pub scale_cross_stitch_pattern: bool,
    /// `glyphs`.
    pub glyphs: Vec<String>,
}

impl Default for Settings {
    /// Ink/Stitch's defaults, read at `d59c9ab` (`lib/lettering/font.py`).
    fn default() -> Self {
        Settings {
            name: String::new(),
            description: String::new(),
            descriptions: BTreeMap::new(),
            keywords: Vec::new(),
            font_license: "SIL Open Font License v1.1".to_string(),
            original_font: String::new(),
            original_font_url: String::new(),
            text_direction: TextDirection::LeftToRight,
            default_variant: Direction::LeftToRight,
            letter_case: LetterCase::AsTyped,
            default_glyph: "\u{fffd}".to_string(),
            size: 0.0,
            min_scale: 1.0,
            max_scale: 1.0,
            leading: 100.0,
            horiz_adv_x: BTreeMap::new(),
            horiz_adv_x_default: None,
            horiz_adv_x_space: 20.0,
            kerning_pairs: BTreeMap::new(),
            units_per_em: None,
            auto_satin: true,
            reversible: true,
            sortable: false,
            combine_at_sort_indices: Vec::new(),
            scale_cross_stitch_pattern: false,
            glyphs: Vec::new(),
        }
    }
}

/// A key of `font.json`.
#[derive(Clone, Copy, Debug)]
pub struct Key {
    /// Its name in the file.
    pub name: &'static str,
    /// What it takes, for people.
    pub takes: &'static str,
    /// What it does, for people.
    pub does: &'static str,
    /// Its value in `settings`, as `font.json` writes it.
    pub value: fn(&Settings) -> Value,
    /// Reads the file's value into `settings`, saying of entries it leaves out, or says why it doesn't.
    read: fn(&mut Settings, &Value, &mut LeftOut) -> Result<(), String>,
}

const TEXT: &str = "a text";
const NUMBER: &str = "a number";
const YES_OR_NO: &str = "`true` or `false`";
const TEXTS: &str = "a list of texts";

/// Every key of `font.json` the reader reads, in the order Ink/Stitch's Generate JSON writes them.
pub const KEYS: [Key; 25] = [
    Key {
        name: "name",
        takes: TEXT,
        does: "The font's name, which lists of fonts show. Ink/Stitch's lists leave out a font without one.",
        value: |s| Value::from(s.name.as_str()),
        read: |s, v, _| text(v).map(|t| s.name = t),
    },
    Key {
        name: "description",
        takes: TEXT,
        does: "What the font is like. A key `description_` and a language code, such as `description_de`, gives it in that language.",
        value: |s| Value::from(s.description.as_str()),
        read: |s, v, _| text(v).map(|t| s.description = t),
    },
    Key {
        name: "font_license",
        takes: TEXT,
        does: "The name of the font's licence, as its author wrote it. The font's licence file is what grants the rights.",
        value: |s| Value::from(s.font_license.as_str()),
        read: |s, v, _| text(v).map(|t| s.font_license = t),
    },
    Key {
        name: "default_variant",
        takes: "`ltr`, `rtl`, `ttb` or `btt`, or an older font's `→`, `←`, `↓` or `↑`",
        does: "The direction lettering sews in unless it is told another. A font without glyph files for it sews in its first direction.",
        value: |s| Value::from(s.default_variant.id()),
        read: |s, v, _| direction(v).map(|d| s.default_variant = d),
    },
    Key {
        name: "text_direction",
        takes: "`ltr` or `rtl`",
        does: "Which way the font's script runs. Lettering reverses each line of a right-to-left font before it sets the glyphs.",
        value: |s| Value::from(if s.text_direction == TextDirection::RightToLeft { "rtl" } else { "ltr" }),
        read: |s, v, _| {
            word(v, &["ltr", "rtl"]).map(|w| s.text_direction = if w == "rtl" { TextDirection::RightToLeft } else { TextDirection::LeftToRight })
        },
    },
    Key {
        name: "keywords",
        takes: TEXTS,
        does: "The categories lists of fonts sort the font into, such as `satin`, `sans_serif` and `tiny`.",
        value: |s| Value::from(s.keywords.clone()),
        read: |s, v, left_out| texts(v, left_out).map(|t| s.keywords = t),
    },
    Key {
        name: "original_font",
        takes: TEXT,
        does: "The font the embroidery font was made from.",
        value: |s| Value::from(s.original_font.as_str()),
        read: |s, v, _| text(v).map(|t| s.original_font = t),
    },
    Key {
        name: "original_font_url",
        takes: TEXT,
        does: "Where that font is published.",
        value: |s| Value::from(s.original_font_url.as_str()),
        read: |s, v, _| text(v).map(|t| s.original_font_url = t),
    },
    Key {
        name: "leading",
        takes: NUMBER,
        does: "The distance from one line's baseline to the next, in the glyph files' pixels.",
        value: |s| Value::from(s.leading),
        read: |s, v, _| number(v).map(|n| s.leading = n),
    },
    Key {
        name: "auto_satin",
        takes: YES_OR_NO,
        does: "Whether lettering routes the text's satin columns and running stitches with auto-satin, in their order.",
        value: |s| Value::from(s.auto_satin),
        read: |s, v, _| flag(v).map(|f| s.auto_satin = f),
    },
    Key {
        name: "scale_cross_stitch_pattern",
        takes: YES_OR_NO,
        does: "Whether lettering scales the grid of the font's cross stitch with the text.",
        value: |s| Value::from(s.scale_cross_stitch_pattern),
        read: |s, v, _| flag(v).map(|f| s.scale_cross_stitch_pattern = f),
    },
    Key {
        name: "reversible",
        takes: YES_OR_NO,
        does: "Whether lettering may sew every other line in the reversed direction, back and forth.",
        value: |s| Value::from(s.reversible),
        read: |s, v, _| flag(v).map(|f| s.reversible = f),
    },
    Key {
        name: "sortable",
        takes: YES_OR_NO,
        does: "Whether lettering may sort a multicolour font's colours, by its elements' `color_sort_index`.",
        value: |s| Value::from(s.sortable),
        read: |s, v, _| flag(v).map(|f| s.sortable = f),
    },
    Key {
        name: "combine_at_sort_indices",
        takes: "a list of integers",
        does: "The colour sort indices whose elements lettering joins into one element when it sorts the colours.",
        value: |s| Value::from(s.combine_at_sort_indices.clone()),
        read: |s, v, left_out| integers(v, left_out).map(|i| s.combine_at_sort_indices = i),
    },
    Key {
        name: "letter_case",
        takes: "`\"\"`, `upper` or `lower`",
        does: "Text set in capitals, in small letters, or as typed.",
        value: |s| {
            Value::from(match s.letter_case {
                LetterCase::AsTyped => "",
                LetterCase::Upper => "upper",
                LetterCase::Lower => "lower",
            })
        },
        read: |s, v, _| {
            word(v, &["", "upper", "lower"]).map(|w| {
                s.letter_case = match w {
                    "upper" => LetterCase::Upper,
                    "lower" => LetterCase::Lower,
                    _ => LetterCase::AsTyped,
                };
            })
        },
    },
    Key {
        name: "default_glyph",
        takes: TEXT,
        does: "The glyph lettering sews for a character the font lacks. A space leaves a space.",
        value: |s| Value::from(s.default_glyph.as_str()),
        read: |s, v, _| text(v).map(|t| s.default_glyph = t),
    },
    Key {
        name: "size",
        takes: NUMBER,
        does: "The height of the font's reference capital at a scale of 100 %, in millimetres.",
        value: |s| Value::from(s.size),
        read: |s, v, _| number(v).map(|n| s.size = n),
    },
    Key {
        name: "min_scale",
        takes: NUMBER,
        does: "The least scale the font sews at, as a share of `size`: 0.5 is half as tall.",
        value: |s| Value::from(s.min_scale),
        read: |s, v, _| number(v).map(|n| s.min_scale = n),
    },
    Key {
        name: "max_scale",
        takes: NUMBER,
        does: "The greatest scale the font sews at, as a share of `size`.",
        value: |s| Value::from(s.max_scale),
        read: |s, v, _| number(v).map(|n| s.max_scale = n),
    },
    Key {
        name: "horiz_adv_x_default",
        takes: "a number, or `null`",
        does: "The advance of a glyph `horiz_adv_x` gives none, in pixels. With `null`, a glyph's advance is its right edge.",
        value: |s| s.horiz_adv_x_default.map_or(Value::Null, Value::from),
        read: |s, v, _| optional_number(v).map(|n| s.horiz_adv_x_default = n),
    },
    Key {
        name: "horiz_adv_x_space",
        takes: NUMBER,
        does: "The advance of a space, in pixels.",
        value: |s| Value::from(s.horiz_adv_x_space),
        read: |s, v, _| number(v).map(|n| s.horiz_adv_x_space = n),
    },
    Key {
        name: "units_per_em",
        takes: "a number, or `null`",
        does: "The em of the font the glyphs were made from, in pixels. Ink/Stitch's font tools keep it, and lettering doesn't use it.",
        value: |s| s.units_per_em.map_or(Value::Null, Value::from),
        read: |s, v, _| optional_number(v).map(|n| s.units_per_em = n),
    },
    Key {
        name: "horiz_adv_x",
        takes: "an object of numbers",
        does: "The advance of each glyph that has one of its own, by the glyph's label, in pixels.",
        value: |s| table_value(&s.horiz_adv_x),
        read: |s, v, left_out| table(v, left_out).map(|t| s.horiz_adv_x = t),
    },
    Key {
        name: "kerning_pairs",
        takes: "an object of numbers",
        does: "The kerning of each pair of glyphs, by their labels with a space between them, or joined in an older font, in pixels. It is taken off the second glyph's place.",
        value: |s| table_value(&s.kerning_pairs),
        read: |s, v, left_out| table(v, left_out).map(|t| s.kerning_pairs = t),
    },
    Key {
        name: "glyphs",
        takes: TEXTS,
        does: "The glyphs the font has, by their labels, as Ink/Stitch's font tools list them. Ink/Stitch's lettering offers the fonts that have every character typed.",
        value: |s| Value::from(s.glyphs.clone()),
        read: |s, v, left_out| texts(v, left_out).map(|t| s.glyphs = t),
    },
];

/// What the prefix of a description in a language is.
pub const DESCRIPTION_IN: &str = "description_";

/// What reading the object `object` of `font.json` gives: the settings, the messages about values read at
/// their defaults and about entries left out, for `SC-W1102`, and the keys the format doesn't have, for
/// `SC-I1105`.
pub(super) fn read(object: &serde_json::Map<String, Value>) -> (Settings, Vec<String>, Vec<String>) {
    let mut settings = Settings::default();
    let (mut kept, mut unknown) = (Vec::new(), Vec::new());
    for (name, value) in object {
        if let Some(language) = name.strip_prefix(DESCRIPTION_IN) {
            match text(value) {
                Ok(description) => {
                    settings.descriptions.insert(language.to_string(), description);
                }
                Err(given) => kept.push(format!("`{name}` takes {TEXT}, and `font.json` gives {given}. It is left out.")),
            }
            continue;
        }
        let Some(key) = KEYS.iter().find(|key| key.name == name) else {
            unknown.push(name.clone());
            continue;
        };
        let mut left_out = Vec::new();
        if let Err(given) = (key.read)(&mut settings, value, &mut left_out) {
            let default = shown(&(key.value)(&Settings::default()));
            kept.push(format!("`{name}` takes {}, and `font.json` gives {given}. It is read as {default}, Ink/Stitch's default.", key.takes));
        }
        kept.extend(
            left_out.into_iter().map(|(entry, given, wanted)| format!("{entry} of `{name}` is {given}, which isn't {wanted}. It is left out.")),
        );
    }
    (settings, kept, unknown)
}

/// An entry left out of a list or a table: which it is, what it is, and what the key's entries are.
type LeftOut = Vec<(String, String, &'static str)>;

/// `value` as `font.json` writes it, a whole number without a fraction, as Ink/Stitch writes its defaults.
pub fn shown(value: &Value) -> String {
    match value.as_f64() {
        Some(number) if value.is_f64() => number.to_string(),
        _ => value.to_string(),
    }
}

/// What a JSON value is, for a message: its type, with the value when it is short.
pub(super) fn described(value: &Value) -> String {
    let kind = match value {
        Value::Null => return "null".to_string(),
        Value::Bool(_) => "the flag",
        Value::Number(_) => "the number",
        Value::String(_) => "the text",
        Value::Array(_) => return "a list".to_string(),
        Value::Object(_) => return "an object".to_string(),
    };
    let text = shown(value);
    if text.chars().count() <= 40 { format!("{kind} {text}") } else { kind.to_string() }
}

fn text(value: &Value) -> Result<String, String> {
    value.as_str().map(str::to_string).ok_or_else(|| described(value))
}

fn number(value: &Value) -> Result<f64, String> {
    value.as_f64().ok_or_else(|| described(value))
}

fn optional_number(value: &Value) -> Result<Option<f64>, String> {
    if value.is_null() { Ok(None) } else { number(value).map(Some) }
}

fn flag(value: &Value) -> Result<bool, String> {
    value.as_bool().ok_or_else(|| described(value))
}

/// The text `value` holds when it is one of `words`.
fn word<'w>(value: &Value, words: &[&'w str]) -> Result<&'w str, String> {
    value.as_str().and_then(|text| words.iter().find(|word| **word == text).copied()).ok_or_else(|| described(value))
}

/// The direction `value` names, by its id or an older font's arrow.
fn direction(value: &Value) -> Result<Direction, String> {
    value.as_str().and_then(Direction::named).ok_or_else(|| described(value))
}

/// The texts of the list `value`, each entry of another type in `left_out`.
fn texts(value: &Value, left_out: &mut LeftOut) -> Result<Vec<String>, String> {
    let list = value.as_array().ok_or_else(|| described(value))?;
    let mut kept = Vec::new();
    for (place, entry) in (1..).zip(list) {
        match entry.as_str() {
            Some(text) => kept.push(text.to_string()),
            None => left_out.push((format!("Entry {place}"), described(entry), TEXT)),
        }
    }
    Ok(kept)
}

/// The integers of the list `value`, each entry of another kind in `left_out`.
fn integers(value: &Value, left_out: &mut LeftOut) -> Result<Vec<i64>, String> {
    let list = value.as_array().ok_or_else(|| described(value))?;
    let mut kept = Vec::new();
    for (place, entry) in (1..).zip(list) {
        match entry.as_i64() {
            Some(integer) => kept.push(integer),
            None => left_out.push((format!("Entry {place}"), described(entry), "an integer")),
        }
    }
    Ok(kept)
}

/// The numbers of the object `value`, by name, each entry of another type in `left_out`.
fn table(value: &Value, left_out: &mut LeftOut) -> Result<BTreeMap<String, f64>, String> {
    let object = value.as_object().ok_or_else(|| described(value))?;
    let mut kept = BTreeMap::new();
    for (name, entry) in object {
        match entry.as_f64() {
            Some(number) => {
                kept.insert(name.clone(), number);
            }
            None => left_out.push((format!("The entry {}", Value::from(name.as_str())), described(entry), NUMBER)),
        }
    }
    Ok(kept)
}

/// A table of numbers, as `font.json` writes it.
fn table_value(table: &BTreeMap<String, f64>) -> Value {
    Value::Object(table.iter().map(|(name, number)| (name.clone(), Value::from(*number))).collect())
}
