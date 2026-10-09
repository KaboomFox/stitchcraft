//! Normalize (pipeline stage 1): each element's shape made ready for its generator.
//!
//! A [`Design`](crate::design::Design) keeps geometry exact: Bézier curves with their control points.
//! Generators want simpler, checked shapes: a stroke as polylines with its corners marked, a fill as
//! polygons with holes (M5.1), a satin as rails and rungs (M4.1). Normalizing works on one element at a
//! time, so elements stay independent and can be planned in any order
//! (`docs/src/design/engine-pipeline.md` › Normalize).

pub mod stroke;
