//! Test support for StitchCraft: fixtures, property-test strategies, conformance metrics and invariant
//! assertions. A dev-dependency only (`cargo xtask layers` enforces it).
//!
//! - [`plans`]: canonical stitch plans behind the format golden files.
//! - [`equivalence`]: when two plans make a machine do the same thing (round-trip tests).
//! - [`strategies`]: random plans for property tests, with fixed seeds on every PR.
//!
//! Status: grows with roadmap milestones M1–M5. Design: `docs/src/design/conformance.md`.
#![forbid(unsafe_code)]
// Test support may unwrap and panic: a failing assertion is the point of a test.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

pub mod equivalence;
pub mod plans;
pub mod strategies;
