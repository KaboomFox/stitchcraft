//! Reading parameter values: text as a design stores it, checked against the parameter's kind.
//!
//! Every host stores parameters as text — an SVG attribute, a command-line `key=value`, a value in a
//! VectorCraft effect record — so parsing happens once, here, the same way for all of them. A value is
//! accepted, clamped into range with `SC-W0102`, or rejected with `SC-E0101`; it never falls back to the
//! default in silence (REQ-PRM-002). Ink/Stitch strips every value of the white space round it before it
//! reads it, and so does this module, text included. A value of more than [`MAX_TEXT`] bytes does not
//! read, so reading one costs no more than that, and a message quotes only its start (`DEV-PRM-004`).
//!
//! Lengths accept a unit right after the number (`2.5mm`, `0.1in`, `7pt`; millimetres without one), which
//! Ink/Stitch does not (`DEV-PRM-001`). An optional length is empty as its kind says ([`Empty`]): most
//! count 0 as empty, as Ink/Stitch reads 0 as "not set" there, and a length below 0 as well, with
//! `SC-W0102`, where Ink/Stitch reads it as a length (`DEV-PRM-003`). Angles are normalized to (−180, 180],
//! each angle of a list too, and a seed may be any text: a number is used as it is, other text is hashed
//! (FNV-1a), so any seed a file stores works. A value for each of 2 sides is 1 value, for both, or 2
//! separated by a space, as Ink/Stitch writes them. An optional pair is empty only when its text is: 0 is a
//! value for a side, as in Ink/Stitch. A toggle reads Ink/Stitch's words for on, their opposites and `off`
//! ([`toggle`]).
//!
//! Spaces separate the values of a list or a pair, as in Ink/Stitch, which splits its lists at spaces and
//! reads one it cannot split so, such as `45,90`, as its default. A tab, or a run of spaces, counts as one
//! space (`DEV-PRM-001`). A comma separates nothing, and is no decimal point, so text with one does not
//! read, and `SC-E0101` comes with a hint. An angle of a list may end with one comma, which is left out:
//! Ink/Stitch reads a fill's underlay angles so, for older files.

use stitchcraft_core::rng::fnv1a64;
use stitchcraft_core::units::{MM_PER_INCH, MM_PER_POINT};
use stitchcraft_core::{Code, Diagnostic, Fix, Mm};

use crate::spec::{Empty, Kind, MAX_LIST, MAX_TEXT, ParamSpec};

/// What `SC-E0101` hints when the text it refuses has a comma.
const COMMA_HINT: &str = "Separate values with spaces, and write decimals with a point.";

/// How many characters of a value a message quotes.
const QUOTED: usize = 64;

/// The words for on, in any case: Ink/Stitch's, which reads any other text as off.
pub(crate) const ON: [&str; 5] = ["true", "yes", "y", "t", "1"];

/// The words for off, in any case: the opposites of the words for on, and `off`.
pub(crate) const OFF: [&str; 6] = ["false", "no", "n", "f", "0", "off"];

/// A parameter's value, read and checked.
#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    /// A length; `None` only for an optional length left empty.
    Length(Option<Mm>),
    /// An angle in degrees, in (−180, 180].
    Angle(f64),
    /// A percentage.
    Percent(f64),
    /// A number without a unit.
    Number(f64),
    /// A whole number.
    Count(u32),
    /// On or off.
    Toggle(bool),
    /// The chosen option's id.
    Choice(&'static str),
    /// A seed; `None` means "derive it from the element".
    Seed(Option<u64>),
    /// Lengths.
    Lengths(Vec<Mm>),
    /// A length for each of 2 sides; `None` only for an optional pair left empty.
    LengthPair(Option<[Mm; 2]>),
    /// A percentage for each of 2 sides; `None` only for an optional pair left empty.
    PercentPair(Option<[f64; 2]>),
    /// Percentages.
    Percents(Vec<f64>),
    /// Whole numbers.
    Counts(Vec<u32>),
    /// Angles in degrees, each in (−180, 180]; `None` only for an optional list left empty.
    Angles(Option<Vec<f64>>),
    /// Text.
    Text(String),
}

impl Value {
    /// The value as a design would store it.
    pub fn to_raw(&self) -> String {
        let join = |items: Vec<String>| items.join(" ");
        match self {
            Value::Length(Some(mm)) => mm.get().to_string(),
            Value::Length(None) | Value::Seed(None) | Value::LengthPair(None) | Value::PercentPair(None) | Value::Angles(None) => String::new(),
            Value::Angle(v) | Value::Percent(v) | Value::Number(v) => v.to_string(),
            Value::Count(n) => n.to_string(),
            Value::Toggle(on) => on.to_string(),
            Value::Choice(id) => (*id).to_string(),
            Value::Seed(Some(seed)) => seed.to_string(),
            Value::Lengths(lengths) => join(lengths.iter().map(|mm| mm.get().to_string()).collect()),
            Value::LengthPair(Some(pair)) => pair_text(pair.map(Mm::get)),
            Value::PercentPair(Some(pair)) => pair_text(*pair),
            Value::Percents(percents) => join(percents.iter().map(f64::to_string).collect()),
            Value::Counts(counts) => join(counts.iter().map(u32::to_string).collect()),
            Value::Angles(Some(angles)) => join(angles.iter().map(f64::to_string).collect()),
            Value::Text(text) => text.clone(),
        }
    }
}

impl Kind {
    /// `raw` read as a value of this kind, and whether it had to be clamped into range; `None` when it
    /// is not a value of this kind at all.
    /// A length below 0 that leaves an optional length empty counts as clamped, so that it is said.
    pub fn parse(self, raw: &str) -> Option<(Value, bool)> {
        if raw.len() > MAX_TEXT {
            return None;
        }
        let text = raw.trim();
        match self {
            Kind::Length { empty, .. } if empty != Empty::Never && text.is_empty() => Some((Value::Length(None), false)),
            Kind::Length { min, max, empty } => {
                let given = length(text)?;
                if empty == Empty::AtMostZero && given <= 0.0 {
                    return Some((Value::Length(None), given < 0.0));
                }
                let (mm, clamped) = clamp(given, min, max);
                Some((Value::Length(Some(Mm::new(mm).ok()?)), clamped))
            }
            Kind::Angle => Some((Value::Angle(angle(text)?), false)),
            Kind::Percent { min, max } => {
                let (percent, clamped) = clamp(percent(text)?, min, max);
                Some((Value::Percent(percent), clamped))
            }
            Kind::Number { min, max } => {
                let (n, clamped) = clamp(number(text)?, min, max);
                Some((Value::Number(n), clamped))
            }
            Kind::Count { min, max } => {
                let (count, clamped) = count(text, min, max)?;
                Some((Value::Count(count), clamped))
            }
            Kind::Toggle => toggle(text).map(|on| (Value::Toggle(on), false)),
            Kind::Choice { options } => options.iter().find(|o| o.id == text).map(|o| (Value::Choice(o.id), false)),
            Kind::Seed if text.is_empty() => Some((Value::Seed(None), false)),
            Kind::Seed => Some((Value::Seed(Some(text.parse().unwrap_or_else(|_| fnv1a64(text.as_bytes())))), false)),
            Kind::LengthList { min, max } => {
                let mut clamped = false;
                let mut lengths = Vec::new();
                for item in list(text)? {
                    let (mm, c) = clamp(length(item)?, min, max);
                    clamped |= c;
                    lengths.push(Mm::new(mm).ok()?);
                }
                Some((Value::Lengths(lengths), clamped))
            }
            Kind::LengthPair { optional: true, .. } if text.is_empty() => Some((Value::LengthPair(None), false)),
            Kind::LengthPair { min, max, .. } => {
                let ([a, b], clamped) = pair(text, length, min, max)?;
                Some((Value::LengthPair(Some([Mm::new(a).ok()?, Mm::new(b).ok()?])), clamped))
            }
            Kind::PercentPair { optional: true, .. } if text.is_empty() => Some((Value::PercentPair(None), false)),
            Kind::PercentPair { min, max, .. } => {
                let (sides, clamped) = pair(text, percent, min, max)?;
                Some((Value::PercentPair(Some(sides)), clamped))
            }
            Kind::PercentList { min, max } => {
                let mut clamped = false;
                let mut percents = Vec::new();
                for item in list(text)? {
                    let (value, c) = clamp(percent(item)?, min, max);
                    clamped |= c;
                    percents.push(value);
                }
                Some((Value::Percents(percents), clamped))
            }
            Kind::CountList { min, max } => {
                let mut clamped = false;
                let mut counts = Vec::new();
                for item in list(text)? {
                    let (n, c) = count(item, min, max)?;
                    clamped |= c;
                    counts.push(n);
                }
                Some((Value::Counts(counts), clamped))
            }
            Kind::AngleList { optional: true } if text.is_empty() => Some((Value::Angles(None), false)),
            Kind::AngleList { .. } => {
                let angles = list(text)?.into_iter().map(|item| angle(item.strip_suffix(',').unwrap_or(item))).collect::<Option<Vec<f64>>>()?;
                Some((Value::Angles(Some(angles)), false))
            }
            Kind::Text { max_bytes } => (text.len() <= max_bytes).then(|| (Value::Text(text.to_string()), false)),
        }
    }

    /// The accepted range, for clamping messages: `0.1 to 10 mm`.
    fn range(self) -> String {
        let unit = match self.unit() {
            "" => String::new(),
            "%" => "%".to_string(),
            unit => format!(" {unit}"),
        };
        match self {
            Kind::Length { min, max, .. }
            | Kind::Percent { min, max }
            | Kind::LengthList { min, max }
            | Kind::LengthPair { min, max, .. }
            | Kind::PercentPair { min, max, .. }
            | Kind::PercentList { min, max }
            | Kind::Number { min, max } => format!("{min} to {max}{unit}"),
            Kind::Count { min, max } | Kind::CountList { min, max } => format!("{min} to {max}"),
            _ => String::new(),
        }
    }
}

impl ParamSpec {
    /// `raw` read as this parameter's value: with `SC-W0102` when it was clamped into range, or left empty
    /// for being below 0, or `SC-E0101` when it is not a value of the parameter's kind, hinting at spaces
    /// and points when the value has a comma and the kind holds numbers. The diagnostics name the
    /// parameter, the value (its start, when long) and what is accepted; hosts add the element.
    pub fn read(&self, raw: &str) -> Result<(Value, Option<Diagnostic>), Diagnostic> {
        match self.kind.parse(raw) {
            Some((value, false)) => Ok((value, None)),
            Some((Value::Length(None), true)) => {
                let message = format!("`{}` is {}, below 0, so it is not set.", self.key, raw.trim());
                Ok((Value::Length(None), Some(Diagnostic::new(Code::ParamClamped, message))))
            }
            Some((value, true)) => {
                let unit = match self.kind.unit() {
                    "" => "",
                    "%" => "%",
                    _ => " mm",
                };
                let message = format!("`{}` is {}, outside {}; {}{unit} is used.", self.key, raw.trim(), self.kind.range(), value.to_raw());
                Ok((value, Some(Diagnostic::new(Code::ParamClamped, message))))
            }
            None => {
                let problem =
                    Diagnostic::new(Code::ParamInvalid, format!("`{}` is {}, but it must be {}.", self.key, quoted(raw), self.kind.describe()));
                Err(if self.kind.numeric() && raw.contains(',') { problem.with_fix(Fix::Hint(COMMA_HINT.to_string())) } else { problem })
            }
        }
    }
}

/// `text` read as a toggle: `Some(true)` for one of the words for on, Ink/Stitch's, `Some(false)` for one of
/// their opposites or `off`, in any case and with white space round it, and `None` for any other text.
/// Ink/Stitch reads such text as off, `on` too, and an element does not read it at all here (`DEV-PRM-001`).
pub fn toggle(text: &str) -> Option<bool> {
    let is = |words: &[&str]| words.iter().any(|word| text.trim().eq_ignore_ascii_case(word));
    if is(&ON) {
        Some(true)
    } else if is(&OFF) {
        Some(false)
    } else {
        None
    }
}

/// `raw` in quotes for a message: whole, or its first [`QUOTED`] characters and how long it is.
fn quoted(raw: &str) -> String {
    match raw.char_indices().nth(QUOTED) {
        Some((end, _)) => format!("\"{}…\" ({} bytes)", raw.get(..end).unwrap_or_default(), raw.len()),
        None => format!("\"{raw}\""),
    }
}

/// A finite number, written with nothing round it.
fn number(text: &str) -> Option<f64> {
    let value: f64 = text.parse().ok()?;
    value.is_finite().then_some(value)
}

/// An angle in degrees, from a number with an optional `°` or `deg`, normalized to (−180, 180].
fn angle(text: &str) -> Option<f64> {
    number(text.strip_suffix('°').or_else(|| text.strip_suffix("deg")).unwrap_or(text)).map(normalized_angle)
}

/// `degrees` turned by whole turns into (−180, 180], as an angle parameter holds it: an angle a generator
/// works out, such as a fill's angle plus 90°, then means what the same angle typed in does.
pub fn normalized_angle(degrees: f64) -> f64 {
    let turned = degrees.rem_euclid(360.0);
    if turned > 180.0 { turned - 360.0 } else { turned }
}

/// A percentage, from a number with an optional `%`.
fn percent(text: &str) -> Option<f64> {
    number(text.strip_suffix('%').unwrap_or(text))
}

/// A length in millimetres, from a number with an optional unit right after it.
fn length(text: &str) -> Option<f64> {
    let lower = text.to_ascii_lowercase();
    let (number_part, factor) = [("mm", 1.0), ("in", MM_PER_INCH), ("pt", MM_PER_POINT)]
        .iter()
        .find_map(|(unit, factor)| lower.strip_suffix(unit).map(|n| (n.to_string(), *factor)))
        .unwrap_or((lower.clone(), 1.0));
    let mm = number(&number_part)? * factor;
    mm.is_finite().then_some(mm)
}

/// A whole number clamped into `min..=max`, and whether it was.
fn count(text: &str, min: u32, max: u32) -> Option<(u32, bool)> {
    let n: i64 = text.parse().ok()?;
    let (lo, hi) = (i64::from(min), i64::from(max));
    let clamped = n.clamp(lo, hi);
    Some((u32::try_from(clamped).ok()?, clamped != n))
}

/// `value` clamped into `min..=max`, and whether it was.
fn clamp(value: f64, min: f64, max: f64) -> (f64, bool) {
    if value < min {
        (min, true)
    } else if value > max {
        (max, true)
    } else {
        (value, false)
    }
}

/// The values of a pair, each read by `read` and clamped into `min..=max`, and whether either was clamped:
/// 1 value, for both, or 2, separated by a space.
fn pair(text: &str, read: fn(&str) -> Option<f64>, min: f64, max: f64) -> Option<([f64; 2], bool)> {
    let [a, b] = match list(text)?.as_slice() {
        [both] => [*both, *both],
        [a, b] => [*a, *b],
        _ => return None,
    };
    let ((a, clamped_a), (b, clamped_b)) = (clamp(read(a)?, min, max), clamp(read(b)?, min, max));
    Some(([a, b], clamped_a || clamped_b))
}

/// A pair as a design stores it: 1 value when the two are the same, else both.
fn pair_text([a, b]: [f64; 2]) -> String {
    if a == b { a.to_string() } else { format!("{a} {b}") }
}

/// The items of a list: 1 to [`MAX_LIST`] of them, separated by spaces or tabs, a run of them counting as
/// one. It stops at the item past the most.
fn list(text: &str) -> Option<Vec<&str>> {
    let items: Vec<&str> = text.split_whitespace().take(MAX_LIST + 1).collect();
    (1..=MAX_LIST).contains(&items.len()).then_some(items)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::spec::{ChoiceOption, MAX_TEXT};

    const LENGTH: Kind = Kind::Length { min: 0.1, max: 10.0, empty: Empty::Never };

    fn mm(v: f64) -> Mm {
        Mm::new(v).unwrap()
    }

    #[test]
    fn lengths_take_units_and_are_clamped() {
        assert_eq!(LENGTH.parse("2.5"), Some((Value::Length(Some(mm(2.5))), false)));
        assert_eq!(LENGTH.parse(" 2.5mm "), Some((Value::Length(Some(mm(2.5))), false)));
        assert_eq!(LENGTH.parse("0.1in"), Some((Value::Length(Some(mm(0.1 * 25.4))), false)));
        assert_eq!(LENGTH.parse("18pt"), Some((Value::Length(Some(mm(18.0 * 25.4 / 72.0))), false)));
        assert_eq!(LENGTH.parse("25"), Some((Value::Length(Some(mm(10.0))), true)));
        // The limits themselves are inside the range.
        assert_eq!(LENGTH.parse("0.1"), Some((Value::Length(Some(mm(0.1))), false)));
        assert_eq!(LENGTH.parse("10"), Some((Value::Length(Some(mm(10.0))), false)));
        assert_eq!(LENGTH.parse("-1"), Some((Value::Length(Some(mm(0.1))), true)));
        for bad in ["", "big", "NaN", "inf", "1e999", "2.5 cm"] {
            assert_eq!(LENGTH.parse(bad), None, "{bad}");
        }
        let optional = Kind::Length { min: 0.0, max: 5.0, empty: Empty::AtMostZero };
        assert_eq!(optional.parse(" "), Some((Value::Length(None), false)));
        assert_eq!(optional.parse("2"), Some((Value::Length(Some(mm(2.0))), false)));
        assert_eq!(optional.parse("9"), Some((Value::Length(Some(mm(5.0))), true)));
        assert_eq!(optional.parse("big"), None);
    }

    #[test]
    fn an_optional_length_of_zero_or_less_is_empty_unless_zero_is_a_length() {
        // Ink/Stitch reads 0 as "not set": no maximum for manual stitch's longest stitch, the document's
        // setting for the shortest stitch and jump. Below 0 is no length either, and that is said: it counts
        // as clamped, where 0 does not.
        let optional = Kind::Length { min: 0.1, max: 25.0, empty: Empty::AtMostZero };
        for raw in ["0", " 0mm ", "0.0", "-0"] {
            assert_eq!(optional.parse(raw), Some((Value::Length(None), false)), "{raw}");
        }
        for raw in ["-1", "-0.5in", "-1e-9"] {
            assert_eq!(optional.parse(raw), Some((Value::Length(None), true)), "{raw}");
        }
        assert_eq!(optional.parse("0.05"), Some((Value::Length(Some(mm(0.1))), true)));
        // A required length is clamped as before.
        assert_eq!(LENGTH.parse("0"), Some((Value::Length(Some(mm(0.1))), true)));
        // Where 0 is a length, as a satin's shortest stitch to split is in Ink/Stitch, only empty text is
        // empty, and a length below 0 is raised to 0.
        let zero = Kind::Length { min: 0.0, max: 25.0, empty: Empty::Blank };
        assert_eq!(zero.parse(" "), Some((Value::Length(None), false)));
        assert_eq!(zero.parse("0"), Some((Value::Length(Some(mm(0.0))), false)));
        assert_eq!(zero.parse("-1"), Some((Value::Length(Some(mm(0.0))), true)));
        assert_eq!(zero.parse("x"), None);
    }

    #[test]
    fn angles_are_normalized() {
        for (raw, degrees) in [("0", 0.0), ("180", 180.0), ("-180", 180.0), ("190", -170.0), ("450", 90.0), ("-90deg", -90.0), ("45°", 45.0)] {
            assert_eq!(Kind::Angle.parse(raw), Some((Value::Angle(degrees), false)), "{raw}");
        }
        // An angle worked out means what the same angle typed in does: a fill's 135° plus 90°.
        assert_eq!(normalized_angle(135.0 + 90.0), -135.0);
        assert_eq!(normalized_angle(-540.0), 180.0);
    }

    #[test]
    fn a_list_of_angles_normalizes_each_and_may_be_empty_when_optional() {
        let angles = Kind::AngleList { optional: true };
        assert_eq!(angles.parse(" "), Some((Value::Angles(None), false)));
        assert_eq!(angles.parse("45"), Some((Value::Angles(Some(vec![45.0])), false)));
        assert_eq!(angles.parse("0, 90deg  270° -450,"), Some((Value::Angles(Some(vec![0.0, 90.0, -90.0, -90.0])), false)));
        for bad in ["45 x", "NaN", "inf 0", &"1 ".repeat(17), "0,90", "0,, 90", "45 ,"] {
            assert_eq!(angles.parse(bad), None, "{bad}");
        }
        // A list that needs a value has none when empty.
        assert_eq!(Kind::AngleList { optional: false }.parse(""), None);
        assert_eq!(Kind::AngleList { optional: false }.parse("30 -30"), Some((Value::Angles(Some(vec![30.0, -30.0])), false)));
        assert_eq!(Value::Angles(None).to_raw(), "");
        assert_eq!(Value::Angles(Some(vec![0.0, 90.0])).to_raw(), "0 90");
    }

    #[test]
    fn numbers_are_any_finite_value_in_range() {
        let staggers = Kind::Number { min: 0.01, max: 100.0 };
        assert_eq!(staggers.parse("4"), Some((Value::Number(4.0), false)));
        assert_eq!(staggers.parse(" 2.5 "), Some((Value::Number(2.5), false)));
        assert_eq!(staggers.parse("0"), Some((Value::Number(0.01), true)));
        for bad in ["", "four", "inf", "4 mm"] {
            assert_eq!(staggers.parse(bad), None, "{bad}");
        }
        assert_eq!(Value::Number(2.5).to_raw(), "2.5");
    }

    #[test]
    fn counts_toggles_choices_and_seeds() {
        let count = Kind::Count { min: 1, max: 20 };
        assert_eq!(count.parse("4"), Some((Value::Count(4), false)));
        assert_eq!(count.parse("-3"), Some((Value::Count(1), true)));
        assert_eq!(count.parse("99"), Some((Value::Count(20), true)));
        assert_eq!(count.parse("2.5"), None);
        assert_eq!(Kind::Toggle.parse("True"), Some((Value::Toggle(true), false)));
        assert_eq!(Kind::Toggle.parse("off"), Some((Value::Toggle(false), false)));
        assert_eq!(Kind::Toggle.parse("maybe"), None);
        let choice = Kind::Choice { options: &[ChoiceOption { id: "a", label: "A" }, ChoiceOption { id: "b", label: "B" }] };
        assert_eq!(choice.parse("b"), Some((Value::Choice("b"), false)));
        assert_eq!(choice.parse("c"), None);
        assert_eq!(Kind::Seed.parse(""), Some((Value::Seed(None), false)));
        assert_eq!(Kind::Seed.parse("42"), Some((Value::Seed(Some(42)), false)));
        // Text seeds hash with FNV-1a (published test vector: "a" → 0xaf63dc4c8601ec8c).
        assert_eq!(Kind::Seed.parse("a"), Some((Value::Seed(Some(0xaf63_dc4c_8601_ec8c)), false)));
    }

    #[test]
    fn lists_have_one_to_sixteen_values_each_in_range() {
        let lengths = Kind::LengthList { min: 0.3, max: 12.0 };
        assert_eq!(lengths.parse("2.5"), Some((Value::Lengths(vec![mm(2.5)]), false)));
        assert_eq!(lengths.parse("2.5 1\t0.1"), Some((Value::Lengths(vec![mm(2.5), mm(1.0), mm(0.3)]), true)));
        for comma in ["2.5, 1", "3,5", "2.5,"] {
            assert_eq!(lengths.parse(comma), None, "{comma}");
        }
        assert_eq!(lengths.parse(""), None);
        assert_eq!(lengths.parse(&"1 ".repeat(17)), None);
        let percents = Kind::PercentList { min: 0.0, max: 50.0 };
        assert_eq!(percents.parse("15"), Some((Value::Percents(vec![15.0]), false)));
        assert_eq!(percents.parse("10% 60 -5"), Some((Value::Percents(vec![10.0, 50.0, 0.0]), true)));
        assert_eq!(percents.parse("10, 60"), None);
        assert_eq!(percents.parse("10 x"), None);
        let counts = Kind::CountList { min: 0, max: 9 };
        assert_eq!(counts.parse("0 1 2"), Some((Value::Counts(vec![0, 1, 2]), false)));
        assert_eq!(counts.parse("0 99"), Some((Value::Counts(vec![0, 9]), true)));
        assert_eq!(counts.parse("1 x"), None);
        let text = Kind::Text { max_bytes: MAX_TEXT };
        assert_eq!(text.parse(" kept as is \t"), Some((Value::Text("kept as is".to_string()), false)));
        assert_eq!(text.parse(&"x".repeat(MAX_TEXT + 1)), None);
        assert_eq!(Kind::Text { max_bytes: 3 }.parse(" abc "), Some((Value::Text("abc".to_string()), false)), "stripped, then measured");
    }

    #[test]
    fn pairs_are_one_value_for_both_sides_or_one_for_each() {
        let lengths = Kind::LengthPair { min: -10.0, max: 10.0, optional: false };
        assert_eq!(lengths.parse("0.2"), Some((Value::LengthPair(Some([mm(0.2), mm(0.2)])), false)));
        assert_eq!(lengths.parse(" -0.2  0.4mm "), Some((Value::LengthPair(Some([mm(-0.2), mm(0.4)])), false)));
        assert_eq!(lengths.parse("0.1in 12"), Some((Value::LengthPair(Some([mm(0.1 * 25.4), mm(10.0)])), true)));
        assert_eq!(lengths.parse("0.1in,12"), None);
        assert_eq!(lengths.parse("-11 0"), Some((Value::LengthPair(Some([mm(-10.0), mm(0.0)])), true)));
        for bad in ["", "0.2 0.4 0.6", "0.2 x", "NaN"] {
            assert_eq!(lengths.parse(bad), None, "{bad}");
        }
        let percents = Kind::PercentPair { min: -100.0, max: 100.0, optional: false };
        assert_eq!(percents.parse("10"), Some((Value::PercentPair(Some([10.0, 10.0])), false)));
        assert_eq!(percents.parse("0 25%"), Some((Value::PercentPair(Some([0.0, 25.0])), false)));
        assert_eq!(percents.parse("150 -150"), Some((Value::PercentPair(Some([100.0, -100.0])), true)));
        assert_eq!(percents.parse("1 2 3"), None);
    }

    #[test]
    fn an_optional_pair_is_empty_only_when_its_text_is() {
        let lengths = Kind::LengthPair { min: -10.0, max: 10.0, optional: true };
        assert_eq!(lengths.parse(" "), Some((Value::LengthPair(None), false)));
        // 0 is a value, as in Ink/Stitch, unlike an optional length's.
        assert_eq!(lengths.parse("0"), Some((Value::LengthPair(Some([mm(0.0), mm(0.0)])), false)));
        assert_eq!(lengths.parse("0.2 x"), None);
        let percents = Kind::PercentPair { min: -100.0, max: 100.0, optional: true };
        assert_eq!(percents.parse(""), Some((Value::PercentPair(None), false)));
        assert_eq!(percents.parse("5 150"), Some((Value::PercentPair(Some([5.0, 100.0])), true)));
        assert_eq!(Value::LengthPair(None).to_raw(), "");
        assert_eq!(Value::PercentPair(None).to_raw(), "");
    }

    #[test]
    fn values_write_back_as_a_design_stores_them() {
        for raw in ["2.5", "true", "0 1 2", "", "kept", "-0.5", "0 0.5", "10 20", "15 7.5"] {
            let kind = match raw {
                "2.5" => LENGTH,
                "true" => Kind::Toggle,
                "0 1 2" => Kind::CountList { min: 0, max: 9 },
                "" => Kind::Seed,
                "-0.5" | "0 0.5" => Kind::LengthPair { min: -1.0, max: 1.0, optional: false },
                "10 20" => Kind::PercentPair { min: 0.0, max: 100.0, optional: false },
                "15 7.5" => Kind::PercentList { min: 0.0, max: 50.0 },
                _ => Kind::Text { max_bytes: MAX_TEXT },
            };
            assert_eq!(kind.parse(raw).unwrap().0.to_raw(), raw);
        }
    }
}
