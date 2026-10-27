//! Normalize (pipeline stage 1): each element's shape made ready for its generator.
//!
//! A [`Design`](crate::design::Design) keeps geometry exact: Bézier curves with their control points.
//! Generators want simpler, checked shapes: a stroke as polylines with its corners marked, a fill as
//! polygons with holes (`region`, M5.1), a satin as rails and rungs (M4.1). Normalizing works on one element at a
//! time, needing nothing of its neighbours (`docs/src/design/engine-pipeline.md` › Normalize). Where the
//! shapes of neighbours meet is measured here too (`near`), for where elements start and end. A narrow
//! part of a region is cut into triangles (`triangulation`, M13.4), whose shared edges cross it, for its
//! centre lines.

pub(crate) mod along;
pub(crate) mod centre_line;
#[cfg(test)]
pub(crate) mod fixture;
pub(crate) mod near;
pub(crate) mod offset;
pub mod region;
pub mod satin;
pub mod stroke;
#[cfg_attr(not(test), expect(dead_code, reason = "the centre lines of M13.4's second part are its first caller"))]
pub(crate) mod triangulation;
