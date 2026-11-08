//! The StitchCraft engine (layer L2): from a host-independent design to a checked stitch plan.
//!
//! Pipeline: automatic digitizing → normalize → validate → generate (per element) → assemble → finalize →
//! check. Generators are pure, deterministic and budgeted; the engine never sees a host document or a file.
//!
//! [`plan`] is the entry point every host calls. It holds the engine's input model ([`design`]); the
//! pipeline stages: [`auto`], which chooses the stitches of the shapes without settings, [`normalize`],
//! [`generate`], which sends each element to its generator in
//! [`generators`], [`assemble`], with the [`locks`] it sews, and [`finalize`], which fits the plan to the
//! machine and checks it; the machine-checkpoint [`testsheets`], drawn in code; and the parameter
//! [`registry`] with the settings every stitch type shares ([`common`]). [`route`] joins satin columns into one
//! route, as a host's routing tools and lettering ask. `underlays_first` finds the runs of elements that sew
//! their underlays before their top stitches, which `plan` sews as 2 layers each. Finalize and the generators that
//! join pieces of their own leave out stitches shorter than the shortest by one rule, `thin`. Design:
//! `docs/src/design/engine-pipeline.md`.
#![forbid(unsafe_code)]

pub mod assemble;
pub mod auto;
pub mod common;
pub mod design;
pub mod finalize;
pub mod generate;
pub mod generators;
pub mod locks;
pub mod normalize;
mod pipeline;
pub mod registry;
pub mod route;
pub mod testsheets;
mod thin;
mod underlays_first;

pub use pipeline::{PlanOutcome, plan};
