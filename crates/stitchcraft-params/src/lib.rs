//! The StitchCraft parameter registry (layer L0).
//!
//! A parameter is declared with [`params!`] beside the generator that uses it, once for each family of
//! stitch types that reads it. Typed structs, validation, the reference docs, the JSON Schema and
//! VectorCraft's plug-in manifests are made from that declaration, and later CLI help, SVG attributes and
//! property-test strategies will be too. Design: `docs/src/design/params.md`.
//!
//! - [`StitchType`]: the stitch types, by their Ink/Stitch method ids.
//! - [`ParamSpec`], [`Kind`], [`ParamGroup`]: what the registry knows about each parameter. A declaration
//!   for [`AUTOMATIC`] declares the settings of automatic digitizing, which no stitch type reads.
//! - [`find_for`]: the declaration of a key that a family reads (REQ-PRM-005). [`declarations`] gives
//!   every one, and [`find`] the first.
//! - [`ParamSet`]: an element's parameters as its design stores them (text), read into typed views by
//!   each declaration's `from_set`, or `from_set_as` for an element of one stitch type, with `SC-E0101`,
//!   `SC-W0102` and `SC-W0105` for what is wrong ([`value`], [`set`]).
//! - [`audit()`]: what REQ-PRM-001 asks of every declaration, checked by the registry's own test.
//!
//! The registry itself — the list of every declaration — lives in `stitchcraft-engine`, next to the
//! generators: this crate knows no stitch algorithm.
#![forbid(unsafe_code)]

pub mod audit;
pub mod kinds;
pub mod macros;
pub mod set;
pub mod spec;
pub mod stitch_type;
pub mod value;

pub use audit::audit;
pub use kinds::ParamKind;
pub use set::{ParamSet, Validated, declarations, find, find_for, read_param, unknown_keys};
pub use spec::{AUTOMATIC, ChoiceOption, Condition, Empty, Kind, MAX_LIST, MAX_TEXT, Origin, ParamGroup, ParamSpec, Stability};
pub use stitch_type::{Family, StitchType};
pub use stitchcraft_core::Diagnostic;
pub use value::{Value, normalized_angle, toggle};
