//! Parameter values in VectorCraft's JSON as the text the parameters read.
//!
//! **Why here, once.** An effect record in a document and the parameters VectorCraft gives a live effect
//! hold the same JSON values, which designs store as text (`stitchcraft_params::ParamSet`). The reader and
//! the live effect write them the same way, so an object sews the same from a file as in its preview.

use serde_json::{Number, Value};

use crate::PREVIEW_LEVEL;

/// Whether `key`, a parameter of a StitchCraft effect, is a setting of the element it sews: not one of
/// VectorCraft's own, which start with `_` (such as the `_context` a live effect is given), nor the
/// plug-in's [`PREVIEW_LEVEL`].
pub(crate) fn sews(key: &str) -> bool {
    !key.starts_with('_') && key != PREVIEW_LEVEL
}

/// A parameter's value as text, as the parameters read it: a number as a decimal, a whole number without
/// a point; a toggle as `true` or `false`; a choice as its id; a list as its values with spaces between.
/// `None` for anything else.
pub fn text(value: &Value) -> Option<String> {
    match value {
        Value::Bool(on) => Some(on.to_string()),
        Value::Number(n) => Some(number(n)),
        Value::String(s) => Some(s.clone()),
        Value::Array(items) => {
            let scalar = |item: &Value| if matches!(item, Value::Array(_)) { None } else { text(item) };
            items.iter().map(scalar).collect::<Option<Vec<_>>>().map(|values| values.join(" "))
        }
        Value::Null | Value::Object(_) => None,
    }
}

/// A JSON number as the parameters read it: whole numbers without a point, so that a count reads.
fn number(n: &Number) -> String {
    match n.as_f64() {
        // Below 2^53 a float with no fraction is the whole number it shows, and an integer is written so
        // either way. From 2^53, where floats skip whole numbers, a number is written as the file has it.
        Some(f) if f.fract() == 0.0 && f.abs() < 9_007_199_254_740_992.0 => format!("{f:.0}"),
        _ => n.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_number_is_written_as_the_parameters_read_it() {
        let text = |json: &str| number(&serde_json::from_str::<Number>(json).unwrap());
        assert_eq!(text("3.0"), "3");
        assert_eq!(text("-2.0"), "-2");
        assert_eq!(text("0.25"), "0.25");
        assert_eq!(text("1e-7"), "1e-7");
        assert_eq!(text("9007199254740992.0"), "9007199254740992.0");
        assert_eq!(text("-9007199254740993"), "-9007199254740993");
        assert_eq!(text("18446744073709551615"), "18446744073709551615");
    }
}
