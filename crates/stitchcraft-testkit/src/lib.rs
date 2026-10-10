//! Test support for StitchCraft: fixtures, property-test strategies, conformance metrics and invariant
//! assertions. A dev-dependency only (`cargo xtask layers` enforces it).
//!
//! - [`designs`]: small designs for the engine's cases, and a plan's shape at a glance.
//! - [`satins`]: satin columns sewn as an element is, and their needle points read back in pairs.
//! - [`plans`]: canonical stitch plans behind the format golden files.
//! - [`equivalence`]: when two plans make a machine do the same thing (round-trip tests).
//! - [`strategies`]: random plans for property tests, with fixed seeds on every PR.
//! - [`fuzz`]: what must hold for any bytes given to a reader — the bodies of the fuzz targets in `fuzz/`,
//!   run on every PR here and with coverage-guided inputs every night.
//!
//! Status: grows with roadmap milestones M1–M5. Design: `docs/src/design/conformance.md`.
#![forbid(unsafe_code)]
// Test support may unwrap and panic: a failing assertion is the point of a test.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

pub mod designs;
pub mod equivalence;
pub mod fuzz;
pub mod plans;
pub mod satins;
pub mod strategies;
