//! The StitchCraft engine (layer L2): from a host-independent design to a checked stitch plan.
//!
//! Pipeline: normalize → validate → generate (per element) → assemble → finalize → check. Generators are
//! pure, deterministic and budgeted; the engine never sees a host document or a file.
//!
//! Status: planned from roadmap milestone M3. Design: `docs/src/design/engine-pipeline.md`.
#![forbid(unsafe_code)]
