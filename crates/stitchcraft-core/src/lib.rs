//! StitchCraft foundations: the small, dependable pieces every other crate builds on.
//!
//! - [`units`]: lengths in millimetres, sizes and points that are always finite.
//! - [`rect`]: bounds of designs and their parts.
//! - [`math`]: transcendental functions that give the same bits on every platform.
//! - [`rng`]: the one seeded random number generator StitchCraft uses.
//! - [`budget`]: work limits that guarantee every call finishes.
//! - [`diag`]: the diagnostics model and the registry of every diagnostic code.
//! - [`element`]: stable ids of the user's design elements.
//! - [`text`]: user-facing text taken from doc comments.
//!
//! This crate sits at layer L0 and depends on no other StitchCraft crate
//! (see `docs/src/design/architecture.md`).
#![forbid(unsafe_code)]

pub mod budget;
pub mod diag;
pub mod element;
pub mod math;
pub mod rect;
pub mod rng;
pub mod text;
pub mod units;

pub use budget::{Budget, Exhausted, Meter};
pub use diag::{Code, Diagnostic, Edit, Fix, Severity};
pub use element::{ElementId, ElementIdError};
pub use rect::Rect;
pub use rng::SplitMix64;
pub use units::{Mm, Point, Size, UnitError};
