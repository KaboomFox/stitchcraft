//! The StitchCraft parameter registry (layer L0).
//!
//! Every embroidery parameter is declared once, beside the generator that uses it, with [`params!`];
//! everything else that needs to know about parameters — typed structs, validation, reference docs, the
//! JSON Schema, and later VectorCraft plug-in manifests, CLI help, SVG attributes and property-test
//! strategies — is generated from that declaration. Design: `docs/src/design/params.md`.
//!
//! - [`StitchType`]: the stitch types, by their Ink/Stitch method ids.
//! - [`ParamSpec`], [`Kind`], [`ParamGroup`]: what the registry knows about each parameter.
//! - [`ParamSet`]: an element's parameters as its design stores them (text), read into typed views by
//!   each declaration's `from_set`, with `SC-E0101`, `SC-W0102` and `SC-W0105` for what is wrong
//!   ([`value`], [`set`]).
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
pub use set::{ParamSet, Validated, find, read_param, unknown_keys};
pub use spec::{ChoiceOption, Condition, Kind, MAX_LIST, MAX_TEXT, Origin, ParamGroup, ParamSpec, Stability};
pub use stitch_type::{Family, StitchType};
pub use stitchcraft_core::Diagnostic;
pub use value::Value;
