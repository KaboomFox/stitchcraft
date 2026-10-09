//! The `params!` declaration.
//!
//! One declaration, next to the generator that uses the parameters, gives everything else: a typed
//! struct (each field named by its key), the [`ParamSpec`](crate::ParamSpec)s the registry lists, and
//! `from_set`, which reads a [`ParamSet`](crate::ParamSet) into the struct. Doc comments become help
//! text. It is a `macro_rules!` macro, so contributors read plain Rust and builds stay fast (ADR-0003,
//! `docs/src/design/adr/0003-parameter-registry.md`).
//!
//! ```
//! use stitchcraft_params::{params, ParamSet, StitchType};
//!
//! params! {
//!     /// Settings of an example stitch.
//!     pub struct ExampleParams for StitchType::ALL;
//!
//!     "Stitches" {
//!         /// Length of each stitch.
//!         example_length_mm: Length = "2.5", label "Stitch length", range (0.3, 12.0);
//!
//!         /// Where the stitches start.
//!         example_start: Choice = "left", label "Start", options ["left" => "Left", "right" => "Right"];
//!     }
//! }
//!
//! let mut set = ParamSet::new();
//! set.set("example_length_mm", "3");
//! let read = ExampleParams::from_set(&set).unwrap();
//! assert_eq!(read.params.example_length_mm.get(), 3.0);
//! assert_eq!(read.params.example_start, "left");
//! assert_eq!(ExampleParams::GROUP.specs.len(), 2);
//! ```
//!
//! Grammar, one parameter per line:
//!
//! ```text
//! /// help text
//! key: Kind = "default", label "Label" [, range (min, max)] [, options ["id" => "Label", …]]
//!     [, choices CONST] [, applies STITCH_TYPES] [, when other_key == "value"] [, origin ORIGIN]
//!     [, stability STABILITY];
//! ```
//!
//! `Kind` is one of the types in [`kinds`](crate::kinds). Numbers and lists need a `range`; choices need
//! `options`, or `choices` naming a `&[ChoiceOption]` constant that several parameters share. Other
//! combinations do not compile. `applies` overrides the struct's stitch types for
//! one parameter; `origin` defaults to [`Origin::InkStitch`](crate::Origin) and `stability` to
//! [`Stability::Stable`](crate::Stability).

/// Declares a group of parameters; see the [module documentation](crate::macros).
#[macro_export]
macro_rules! params {
    (
        $(#[doc = $struct_doc:literal])+
        $vis:vis struct $name:ident for $applies:expr;
        $(
            $group:literal {
                $(
                    $(#[doc = $doc:literal])+
                    $field:ident : $kind:ident = $default:literal, label $label:literal
                    $(, range ($min:expr, $max:expr))?
                    $(, options [$($id:literal => $option:literal),+ $(,)?])?
                    $(, choices $choices:path)?
                    $(, applies $field_applies:expr)?
                    $(, when $when_key:ident == $when_value:literal)?
                    $(, origin $origin:expr)?
                    $(, stability $stability:expr)?
                    ;
                )+
            }
        )+
    ) => {
        $(#[doc = $struct_doc])+
        #[derive(Clone, Debug, PartialEq)]
        $vis struct $name {
            $($(
                $(#[doc = $doc])+
                pub $field: <$crate::kinds::$kind as $crate::ParamKind>::Value,
            )+)+
        }

        impl $name {
            /// The parameters, in declaration order.
            pub const SPECS: &'static [$crate::ParamSpec] = &[
                $($(
                    $crate::ParamSpec {
                        key: stringify!($field),
                        label: $label,
                        help: concat!($($doc, "\n"),+),
                        kind: $crate::__param_kind!($kind; $($min, $max)?; $($($id => $option),+)?; $($choices)?),
                        default: $default,
                        group: $group,
                        applies_to: $crate::__param_or!([$($field_applies)?] [$applies]),
                        visible_when: $crate::__param_when!($($when_key == $when_value)?),
                        stability: $crate::__param_or!([$($stability)?] [$crate::Stability::Stable]),
                        origin: $crate::__param_or!([$($origin)?] [$crate::Origin::InkStitch]),
                    },
                )+)+
            ];

            /// The group, as the registry lists it.
            pub const GROUP: $crate::ParamGroup =
                $crate::ParamGroup { name: stringify!($name), help: concat!($($struct_doc, "\n"),+), applies_to: $applies, specs: Self::SPECS };

            /// Reads `set`: every parameter from its text, or its default. Errors (`SC-E0101`) mean the
            /// element cannot be stitched; warnings (`SC-W0102`) come with the parameters.
            pub fn from_set(set: &$crate::ParamSet) -> Result<$crate::Validated<Self>, Vec<$crate::Diagnostic>> {
                let mut problems = Vec::new();
                let mut specs = Self::SPECS.iter();
                $($(
                    let $field = $crate::read_param::<$crate::kinds::$kind>(set, specs.next(), &mut problems);
                )+)+
                match ($($($field,)+)+) {
                    ($($(Some($field),)+)+) => Ok($crate::Validated { params: Self { $($($field,)+)+ }, warnings: problems }),
                    _ => Err(problems),
                }
            }
        }
    };
}

/// The [`Kind`](crate::Kind) a declaration's kind name, range and options give. Combinations a kind
/// does not take do not match, so they do not compile.
#[doc(hidden)]
#[macro_export]
macro_rules! __param_kind {
    (Length; $min:expr, $max:expr; ;) => {
        $crate::Kind::Length { min: $min, max: $max, optional: false }
    };
    (OptionalLength; $min:expr, $max:expr; ;) => {
        $crate::Kind::Length { min: $min, max: $max, optional: true }
    };
    (Angle; ; ;) => {
        $crate::Kind::Angle
    };
    (Percent; $min:expr, $max:expr; ;) => {
        $crate::Kind::Percent { min: $min, max: $max }
    };
    (Count; $min:expr, $max:expr; ;) => {
        $crate::Kind::Count { min: $min, max: $max }
    };
    (Toggle; ; ;) => {
        $crate::Kind::Toggle
    };
    (Choice; ; $($id:literal => $option:literal),+;) => {
        $crate::Kind::Choice { options: &[$($crate::ChoiceOption { id: $id, label: $option }),+] }
    };
    (Choice; ; ; $choices:path) => {
        $crate::Kind::Choice { options: $choices }
    };
    (Seed; ; ;) => {
        $crate::Kind::Seed
    };
    (LengthList; $min:expr, $max:expr; ;) => {
        $crate::Kind::LengthList { min: $min, max: $max }
    };
    (CountList; $min:expr, $max:expr; ;) => {
        $crate::Kind::CountList { min: $min, max: $max }
    };
    (Text; ; ;) => {
        $crate::Kind::Text { max_bytes: $crate::MAX_TEXT }
    };
}

/// The first bracket's expression if there is one, else the second's.
#[doc(hidden)]
#[macro_export]
macro_rules! __param_or {
    ([] [$default:expr]) => {
        $default
    };
    ([$value:expr] [$default:expr]) => {
        $value
    };
}

/// A visibility condition, or none.
#[doc(hidden)]
#[macro_export]
macro_rules! __param_when {
    () => {
        None
    };
    ($key:ident == $value:literal) => {
        Some($crate::Condition { key: stringify!($key), equals: $value })
    };
}
