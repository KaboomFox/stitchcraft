//! Reading parameter values: text as a design stores it, checked against the parameter's kind.
//!
//! Every host stores parameters as text — an SVG attribute, a command-line `key=value`, a value in a
//! VectorCraft effect record — so parsing happens once, here, the same way for all of them. A value is
//! accepted, clamped into range with `SC-W0102`, or rejected with `SC-E0101`; it never falls back to the
//! default in silence (REQ-PRM-002). Lengths accept a unit (`mm`, `in`, `pt`; millimetres without one);
//! an optional length of 0 or less counts as empty, because Ink/Stitch reads it as "not set".
//! angles are normalized to (−180, 180], and a seed may be any text: a number is used as it is, other
//! text is hashed (FNV-1a), so any seed a file stores works. A value for each of 2 sides is 1 value, for
//! both, or 2 separated by a space, as Ink/Stitch writes them.

use stitchcraft_core::units::{MM_PER_INCH, MM_PER_POINT};
use stitchcraft_core::{Code, Diagnostic, Mm};

use crate::spec::{Kind, MAX_LIST, ParamSpec};

/// A parameter's value, read and checked.
#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    /// A length; `None` only for an optional length left empty.
    Length(Option<Mm>),
    /// An angle in degrees, in (−180, 180].
    Angle(f64),
    /// A percentage.
    Percent(f64),
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
    /// A length for each of 2 sides.
    LengthPair([Mm; 2]),
    /// A percentage for each of 2 sides.
    PercentPair([f64; 2]),
    /// Whole numbers.
    Counts(Vec<u32>),
    /// Text.
    Text(String),
}

impl Value {
    /// The value as a design would store it.
    pub fn to_raw(&self) -> String {
        let join = |items: Vec<String>| items.join(" ");
        match self {
            Value::Length(Some(mm)) => mm.get().to_string(),
            Value::Length(None) | Value::Seed(None) => String::new(),
            Value::Angle(v) | Value::Percent(v) => v.to_string(),
            Value::Count(n) => n.to_string(),
            Value::Toggle(on) => on.to_string(),
            Value::Choice(id) => (*id).to_string(),
            Value::Seed(Some(seed)) => seed.to_string(),
            Value::Lengths(lengths) => join(lengths.iter().map(|mm| mm.get().to_string()).collect()),
            Value::LengthPair(pair) => pair_text(pair.map(Mm::get)),
            Value::PercentPair(pair) => pair_text(*pair),
            Value::Counts(counts) => join(counts.iter().map(u32::to_string).collect()),
            Value::Text(text) => text.clone(),
        }
    }
}

impl Kind {
    /// `raw` read as a value of this kind, and whether it had to be clamped into range; `None` when it
    /// is not a value of this kind at all.
    pub fn parse(self, raw: &str) -> Option<(Value, bool)> {
        let text = raw.trim();
        match self {
            Kind::Length { optional: true, .. } if text.is_empty() => Some((Value::Length(None), false)),
            Kind::Length { min, max, optional } => {
                let given = length(text)?;
                if optional && given <= 0.0 {
                    return Some((Value::Length(None), false));
                }
                let (mm, clamped) = clamp(given, min, max);
                Some((Value::Length(Some(Mm::new(mm).ok()?)), clamped))
            }
            Kind::Angle => {
                let degrees = number(text.strip_suffix('°').or_else(|| text.strip_suffix("deg")).unwrap_or(text))?.rem_euclid(360.0);
                Some((Value::Angle(if degrees > 180.0 { degrees - 360.0 } else { degrees }), false))
            }
            Kind::Percent { min, max } => {
                let (percent, clamped) = clamp(percent(text)?, min, max);
                Some((Value::Percent(percent), clamped))
            }
            Kind::Count { min, max } => {
                let (count, clamped) = count(text, min, max)?;
                Some((Value::Count(count), clamped))
            }
            Kind::Toggle => match text.to_ascii_lowercase().as_str() {
                "true" | "1" | "yes" | "on" => Some((Value::Toggle(true), false)),
                "false" | "0" | "no" | "off" => Some((Value::Toggle(false), false)),
                _ => None,
            },
            Kind::Choice { options } => options.iter().find(|o| o.id == text).map(|o| (Value::Choice(o.id), false)),
            Kind::Seed if text.is_empty() => Some((Value::Seed(None), false)),
            Kind::Seed => Some((Value::Seed(Some(text.parse().unwrap_or_else(|_| fnv1a(text)))), false)),
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
            Kind::LengthPair { min, max } => {
                let ([a, b], clamped) = pair(text, length, min, max)?;
                Some((Value::LengthPair([Mm::new(a).ok()?, Mm::new(b).ok()?]), clamped))
            }
            Kind::PercentPair { min, max } => {
                let (sides, clamped) = pair(text, percent, min, max)?;
                Some((Value::PercentPair(sides), clamped))
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
            Kind::Text { max_bytes } => (raw.len() <= max_bytes).then(|| (Value::Text(raw.to_string()), false)),
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
            | Kind::LengthPair { min, max }
            | Kind::PercentPair { min, max } => format!("{min} to {max}{unit}"),
            Kind::Count { min, max } | Kind::CountList { min, max } => format!("{min} to {max}"),
            _ => String::new(),
        }
    }
}

impl ParamSpec {
    /// `raw` read as this parameter's value: with `SC-W0102` when it was clamped into range, or
    /// `SC-E0101` when it is not a value of the parameter's kind. The diagnostics name the parameter,
    /// the value and what is accepted; hosts add the element.
    pub fn read(&self, raw: &str) -> Result<(Value, Option<Diagnostic>), Diagnostic> {
        match self.kind.parse(raw) {
            Some((value, false)) => Ok((value, None)),
            Some((value, true)) => {
                let unit = match self.kind.unit() {
                    "" => "",
                    "%" => "%",
                    _ => " mm",
                };
                let message = format!("`{}` is {}, outside {}; {}{unit} is used.", self.key, raw.trim(), self.kind.range(), value.to_raw());
                Ok((value, Some(Diagnostic::new(Code::ParamClamped, message))))
            }
            None => Err(Diagnostic::new(Code::ParamInvalid, format!("`{}` is \"{raw}\", but it must be {}.", self.key, self.kind.describe()))),
        }
    }
}

/// A finite number.
fn number(text: &str) -> Option<f64> {
    let value: f64 = text.trim().parse().ok()?;
    value.is_finite().then_some(value)
}

/// A percentage, from a number with an optional `%`.
fn percent(text: &str) -> Option<f64> {
    number(text.strip_suffix('%').unwrap_or(text))
}

/// A length in millimetres, from a number with an optional unit.
fn length(text: &str) -> Option<f64> {
    let lower = text.trim().to_ascii_lowercase();
    let (number_part, factor) = [("mm", 1.0), ("in", MM_PER_INCH), ("pt", MM_PER_POINT)]
        .iter()
        .find_map(|(unit, factor)| lower.strip_suffix(unit).map(|n| (n.to_string(), *factor)))
        .unwrap_or((lower.clone(), 1.0));
    let mm = number(&number_part)? * factor;
    mm.is_finite().then_some(mm)
}

/// A whole number clamped into `min..=max`, and whether it was.
fn count(text: &str, min: u32, max: u32) -> Option<(u32, bool)> {
    let n: i64 = text.trim().parse().ok()?;
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
/// 1 value, for both, or 2, separated by a space or a comma.
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

/// The items of a list: 1 to [`MAX_LIST`] of them, separated by spaces or commas.
fn list(text: &str) -> Option<Vec<&str>> {
    let items: Vec<&str> = text.split(|c: char| c.is_whitespace() || c == ',').filter(|s| !s.is_empty()).collect();
    (1..=MAX_LIST).contains(&items.len()).then_some(items)
}

/// The 64-bit FNV-1a hash of `text`: a fixed, documented way to turn any seed text into a number.
fn fnv1a(text: &str) -> u64 {
    text.bytes().fold(0xcbf2_9ce4_8422_2325, |hash, byte| (hash ^ u64::from(byte)).wrapping_mul(0x0100_0000_01b3))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::spec::{ChoiceOption, MAX_TEXT};

    const LENGTH: Kind = Kind::Length { min: 0.1, max: 10.0, optional: false };

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
        let optional = Kind::Length { min: 0.0, max: 5.0, optional: true };
        assert_eq!(optional.parse(" "), Some((Value::Length(None), false)));
        assert_eq!(optional.parse("2"), Some((Value::Length(Some(mm(2.0))), false)));
        assert_eq!(optional.parse("9"), Some((Value::Length(Some(mm(5.0))), true)));
        assert_eq!(optional.parse("big"), None);
    }

    #[test]
    fn an_optional_length_of_zero_or_less_is_empty() {
        // As in Ink/Stitch, where 0 and below mean "not set": no maximum for manual stitch's longest
        // stitch, the document's setting for the shortest stitch and jump. Not clamped, so no warning.
        let optional = Kind::Length { min: 0.1, max: 25.0, optional: true };
        for raw in ["0", "-1", " 0mm ", "-0.5in", "0.0"] {
            assert_eq!(optional.parse(raw), Some((Value::Length(None), false)), "{raw}");
        }
        assert_eq!(optional.parse("0.05"), Some((Value::Length(Some(mm(0.1))), true)));
        // A required length is clamped as before.
        assert_eq!(LENGTH.parse("0"), Some((Value::Length(Some(mm(0.1))), true)));
    }

    #[test]
    fn angles_are_normalized() {
        for (raw, degrees) in [("0", 0.0), ("180", 180.0), ("-180", 180.0), ("190", -170.0), ("450", 90.0), ("-90deg", -90.0), ("45°", 45.0)] {
            assert_eq!(Kind::Angle.parse(raw), Some((Value::Angle(degrees), false)), "{raw}");
        }
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
        assert_eq!(lengths.parse("2.5, 1 0.1"), Some((Value::Lengths(vec![mm(2.5), mm(1.0), mm(0.3)]), true)));
        assert_eq!(lengths.parse(""), None);
        assert_eq!(lengths.parse(&"1 ".repeat(17)), None);
        let counts = Kind::CountList { min: 0, max: 9 };
        assert_eq!(counts.parse("0 1 2"), Some((Value::Counts(vec![0, 1, 2]), false)));
        assert_eq!(counts.parse("0 99"), Some((Value::Counts(vec![0, 9]), true)));
        assert_eq!(counts.parse("1 x"), None);
        let text = Kind::Text { max_bytes: MAX_TEXT };
        assert_eq!(text.parse(" kept as is "), Some((Value::Text(" kept as is ".to_string()), false)));
        assert_eq!(text.parse(&"x".repeat(MAX_TEXT + 1)), None);
    }

    #[test]
    fn pairs_are_one_value_for_both_sides_or_one_for_each() {
        let lengths = Kind::LengthPair { min: -10.0, max: 10.0 };
        assert_eq!(lengths.parse("0.2"), Some((Value::LengthPair([mm(0.2), mm(0.2)]), false)));
        assert_eq!(lengths.parse(" -0.2  0.4mm "), Some((Value::LengthPair([mm(-0.2), mm(0.4)]), false)));
        assert_eq!(lengths.parse("0.1in,12"), Some((Value::LengthPair([mm(0.1 * 25.4), mm(10.0)]), true)));
        assert_eq!(lengths.parse("-11 0"), Some((Value::LengthPair([mm(-10.0), mm(0.0)]), true)));
        for bad in ["", "0.2 0.4 0.6", "0.2 x", "NaN"] {
            assert_eq!(lengths.parse(bad), None, "{bad}");
        }
        let percents = Kind::PercentPair { min: -100.0, max: 100.0 };
        assert_eq!(percents.parse("10"), Some((Value::PercentPair([10.0, 10.0]), false)));
        assert_eq!(percents.parse("0 25%"), Some((Value::PercentPair([0.0, 25.0]), false)));
        assert_eq!(percents.parse("150 -150"), Some((Value::PercentPair([100.0, -100.0]), true)));
        assert_eq!(percents.parse("1 2 3"), None);
    }

    #[test]
    fn values_write_back_as_a_design_stores_them() {
        for raw in ["2.5", "true", "0 1 2", "", "kept", "-0.5", "0 0.5", "10 20"] {
            let kind = match raw {
                "2.5" => LENGTH,
                "true" => Kind::Toggle,
                "0 1 2" => Kind::CountList { min: 0, max: 9 },
                "" => Kind::Seed,
                "-0.5" | "0 0.5" => Kind::LengthPair { min: -1.0, max: 1.0 },
                "10 20" => Kind::PercentPair { min: 0.0, max: 100.0 },
                _ => Kind::Text { max_bytes: MAX_TEXT },
            };
            assert_eq!(kind.parse(raw).unwrap().0.to_raw(), raw);
        }
    }
}
