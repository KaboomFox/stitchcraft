# ADR-0005: Geometry stack

**Status:** Proposed — confirmed or revised by spike M0.7 · 2026-10-08

## Context

The engine needs curve flattening, polygon booleans and offsets with holes (fill rules, insets for
underlay and contour fill), spatial queries and graph algorithms — all pure Rust, wasm-compatible,
permissively licensed and robust on degenerate input.

## Decision (proposed)

| Need | Choice | Why |
|---|---|---|
| Curves, affine maps, flattening | `kurbo` 0.13 | VectorCraft uses it; mature; MIT/Apache |
| Booleans, offsets | `i_overlay` 9 | Pure Rust, integer-snapped robust overlay, fill rules, offsetting; MIT/Apache |
| Spatial index | `rstar` 0.13 | Nearest-neighbour queries for routing; MIT/Apache |
| Graphs | `petgraph` 0.8 + our own Hierholzer walk | Deterministic tie-breaking under our control; MIT/Apache |
| Math | `libm` 0.2 | Deterministic transcendental functions; MIT |

**Revised in M3.4:** strokes are not flattened with `kurbo`. Its `flatten` in 0.13 calls `powf` and
`hypot` from the platform's maths library, so the engine halves curves itself with arithmetic and square
roots ([engine pipeline › Normalize](../engine-pipeline.md#1-normalize)).

**Revised in M5.1:** a fill's region is not built with `i_overlay` but by the engine
([fills › Region](../algorithms/fills.md#region)). An integer-snapped overlay moves the drawing's points
onto its grid, and its loops cannot charge an element's work budget. The engine keeps every point the
drawing has and decides sides of lines exactly. It charges its work as it goes.

## Spike M0.7 acceptance criteria

1. Booleans and offsets of 10,000 random polygons with holes, plus the degenerate corpus: no panic, no
   hang, valid output (`i_overlay` validity + our own ring checks).
2. Identical output bytes on Linux, macOS, Windows and wasm32.
3. Offsetting a 150 mm region with 1,000 vertices inward 300 times (contour fill) in under 200 ms natively.
4. Build for `wasm32-unknown-unknown` adds under 300 KiB to the plug-in.

If a criterion fails: `geo`'s boolean/buffer algorithms are the first alternative; a small offsetter of
our own over `i_overlay` booleans the second.

## Consequences

Dependencies are wrapped behind `stitchcraft-engine::geometry` so a replacement touches one module.
