//! The StitchCraft engine (layer L2): from a host-independent design to a checked stitch plan.
//!
//! Pipeline: normalize → validate → generate (per element) → assemble → finalize → check. Generators are
//! pure, deterministic and budgeted; the engine never sees a host document or a file.
//!
//! [`plan`] is the entry point every host calls. Today it holds the engine's input model ([`design`]), the
//! pipeline stages up to plan assembly ([`normalize`] for strokes; [`generate`], which sends each element
//! to its generator in [`generators`]: running and manual stitches, with repeats and bean stitch; and
//! [`assemble`], with the [`locks`] it sews), the machine-checkpoint [`testsheets`], drawn in code, and the
//! parameter [`registry`] with the settings every stitch type shares ([`common`]); finalizing and the plan
//! check arrive in roadmap step M3.9. Design: `docs/src/design/engine-pipeline.md`.
#![forbid(unsafe_code)]

pub mod assemble;
pub mod common;
pub mod design;
pub mod generate;
pub mod generators;
pub mod locks;
pub mod normalize;
mod pipeline;
pub mod registry;
pub mod testsheets;

pub use pipeline::{PlanOutcome, plan};
