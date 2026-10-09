//! The StitchCraft engine (layer L2): from a host-independent design to a checked stitch plan.
//!
//! Pipeline: normalize → validate → generate (per element) → assemble → finalize → check. Generators are
//! pure, deterministic and budgeted; the engine never sees a host document or a file.
//!
//! Today it holds the engine's input model ([`design`]), the first pipeline stages ([`normalize`] for
//! strokes; the running and manual stitches, with repeats and bean stitch, in [`generators`]; the
//! [`locks`] that plan assembly will sew), the machine-checkpoint [`testsheets`], drawn in code, and the
//! parameter [`registry`] with the settings every stitch type shares ([`common`]); the rest of the
//! pipeline arrives step by step in roadmap milestone M3. Design: `docs/src/design/engine-pipeline.md`.
#![forbid(unsafe_code)]

pub mod common;
pub mod design;
pub mod generators;
pub mod locks;
pub mod normalize;
pub mod registry;
pub mod testsheets;
