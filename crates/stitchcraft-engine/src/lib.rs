//! The StitchCraft engine (layer L2): from a host-independent design to a checked stitch plan.
//!
//! Pipeline: normalize → validate → generate (per element) → assemble → finalize → check. Generators are
//! pure, deterministic and budgeted; the engine never sees a host document or a file.
//!
//! [`plan`] is the entry point every host calls. It holds the engine's input model ([`design`]); the
//! pipeline stages: [`normalize`], [`generate`], which sends each element to its generator in
//! [`generators`], [`assemble`], with the [`locks`] it sews, and [`finalize`], which fits the plan to the
//! machine and checks it; the machine-checkpoint [`testsheets`], drawn in code; and the parameter
//! [`registry`] with the settings every stitch type shares ([`common`]). Finalize and the generators that
//! join pieces of their own leave out stitches shorter than the shortest by one rule, `thin`. Design:
//! `docs/src/design/engine-pipeline.md`.
#![forbid(unsafe_code)]

pub mod assemble;
pub mod common;
pub mod design;
pub mod finalize;
pub mod generate;
pub mod generators;
pub mod locks;
pub mod normalize;
mod pipeline;
pub mod registry;
pub mod testsheets;
mod thin;

pub use pipeline::{PlanOutcome, plan};
