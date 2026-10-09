//! The StitchCraft engine (layer L2): from a host-independent design to a checked stitch plan.
//!
//! Pipeline: normalize → validate → generate (per element) → assemble → finalize → check. Generators are
//! pure, deterministic and budgeted; the engine never sees a host document or a file.
//!
//! Today it holds the engine's input model ([`design`]), the machine-checkpoint [`testsheets`], drawn in
//! code, and the parameter [`registry`] with the settings every stitch type shares ([`common`]); the
//! pipeline arrives step by step in roadmap milestone M3. Design: `docs/src/design/engine-pipeline.md`.
#![forbid(unsafe_code)]

pub mod common;
pub mod design;
pub mod registry;
pub mod testsheets;
