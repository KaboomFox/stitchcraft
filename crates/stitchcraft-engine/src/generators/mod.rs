//! Generators (pipeline stage 3): one module per stitch type, each turning a normalized shape and its
//! typed parameters into stitches.
//!
//! Every generator is pure: its stitches depend only on the shape, its parameters, its hints, its seed
//! and its budget. It charges the budget in every loop, and it reports what it changed or left out with a
//! coded diagnostic (`docs/src/design/engine-pipeline.md` › Generate). Until plan assembly arrives (M3.8)
//! with the `Generator` trait and the table that sends each element to its generator, each generator is
//! a function.

pub mod running;
