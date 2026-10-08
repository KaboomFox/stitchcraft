//! Stitch plans, machine profiles and thread palettes (layer L1).
//!
//! A stitch plan is the ordered list of stitches, jumps, trims and stops for a design, in colour blocks.
//! The engine produces plans; machine formats encode and decode them; the invariant checker in this
//! crate validates every plan before it is written.
//!
//! - [`plan`]: the plan types; [`builder`]: building one with the needle tracked.
//! - [`invariants`]: the L0 rules every plan must satisfy (`REQ-PLAN-…`).
//! - [`profile`] and [`profiles`]: machine facts, and the built-in profiles.
//! - [`palette`] and [`thread`]: thread colours and nearest-colour matching.
//!
//! Design: `docs/src/design/data-model.md`.
#![forbid(unsafe_code)]

pub mod builder;
pub mod invariants;
pub mod palette;
pub mod plan;
pub mod profile;
pub mod profiles;
pub mod thread;

pub use builder::{PlanBuilder, PlanError};
pub use invariants::Violation;
pub use plan::{ColorBlock, ColorEntry, ElementRef, PlanStats, Provenance, Role, SewnStitch, Stitch, StitchKind, StitchPlan};
pub use profile::{FormatId, MachineProfile, PaletteId, TrimSupport};
pub use thread::{Rgb, Thread};
