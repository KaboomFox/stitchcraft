//! The kinds a [`params!`](crate::params) declaration names, and the Rust type each gives a generator.
//!
//! `row_spacing_mm: Length = …` in a declaration names [`Length`] here; the typed struct's field is then
//! `<Length as ParamKind>::Value`, an [`Mm`]. The same name picks the [`Kind`](crate::Kind) the spec
//! records, so a field's type and its validation cannot disagree.

use stitchcraft_core::Mm;

use crate::value::Value;

/// A kind of parameter, as a declaration names it.
pub trait ParamKind {
    /// The type generators receive.
    type Value;
    /// The typed value, if `value` is of this kind.
    fn take(value: Value) -> Option<Self::Value>;
}

macro_rules! kinds {
    ($($(#[doc = $doc:literal])+ $name:ident => $ty:ty, |$v:ident| $take:expr;)+) => {
        $(
            $(#[doc = $doc])+
            #[derive(Clone, Copy, Debug)]
            pub struct $name;

            impl ParamKind for $name {
                type Value = $ty;
                fn take($v: Value) -> Option<$ty> {
                    $take
                }
            }
        )+
    };
}

kinds! {
    /// A length in millimetres.
    Length => Mm, |v| if let Value::Length(Some(mm)) = v { Some(mm) } else { None };
    /// A length in millimetres that may be left empty (`None`).
    OptionalLength => Option<Mm>, |v| if let Value::Length(mm) = v { Some(mm) } else { None };
    /// An angle in degrees, in (−180, 180].
    Angle => f64, |v| if let Value::Angle(degrees) = v { Some(degrees) } else { None };
    /// A percentage.
    Percent => f64, |v| if let Value::Percent(percent) = v { Some(percent) } else { None };
    /// A number without a unit.
    Number => f64, |v| if let Value::Number(n) = v { Some(n) } else { None };
    /// A whole number.
    Count => u32, |v| if let Value::Count(n) = v { Some(n) } else { None };
    /// On or off.
    Toggle => bool, |v| if let Value::Toggle(on) = v { Some(on) } else { None };
    /// One option's id.
    Choice => &'static str, |v| if let Value::Choice(id) = v { Some(id) } else { None };
    /// A seed; `None` means "derive it from the element".
    Seed => Option<u64>, |v| if let Value::Seed(seed) = v { Some(seed) } else { None };
    /// Lengths in millimetres.
    LengthList => Vec<Mm>, |v| if let Value::Lengths(lengths) = v { Some(lengths) } else { None };
    /// A length in millimetres for each of 2 sides.
    LengthPair => [Mm; 2], |v| if let Value::LengthPair(pair) = v { Some(pair) } else { None };
    /// A percentage for each of 2 sides.
    PercentPair => [f64; 2], |v| if let Value::PercentPair(pair) = v { Some(pair) } else { None };
    /// Percentages.
    PercentList => Vec<f64>, |v| if let Value::Percents(percents) = v { Some(percents) } else { None };
    /// Whole numbers.
    CountList => Vec<u32>, |v| if let Value::Counts(counts) = v { Some(counts) } else { None };
    /// Text.
    Text => String, |v| if let Value::Text(text) = v { Some(text) } else { None };
}
