//! StitchCraft foundations: the small, dependable pieces every other crate builds on.
//!
//! - [`units`]: lengths in millimetres and points that are always finite.
//! - [`math`]: transcendental functions that give the same bits on every platform.
//! - [`rng`]: the one seeded random number generator StitchCraft uses.
//!
//! This crate sits at layer L0 and depends on no other StitchCraft crate
//! (see `docs/src/design/architecture.md`).
#![forbid(unsafe_code)]

pub mod math;
pub mod rng;
pub mod units;

pub use rng::SplitMix64;
pub use units::{Mm, Point, UnitError};
