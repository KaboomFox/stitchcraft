# ADR-0005: Geometry stack

<!-- The status line, a label and its value, which the ADR index is checked against. -->
<!-- vale ai-tells.ColonUsage = NO -->
**Status:** Accepted · 2026-10-17, after spike M0.7 · proposed 2026-10-08
<!-- vale ai-tells.ColonUsage = YES -->

## Context

The engine needs curve flattening, polygon booleans and offsets with holes (fill rules, insets for
underlay and contour fill), spatial queries and graph algorithms. They must be pure Rust and build for
WebAssembly. Their licences must be permissive, and they must hold up on degenerate input.

## Decision

The engine does its own geometry, in Rust on `f64`, and depends on no geometry crate. `libm`, through
`stitchcraft_core::math`, gives it the few transcendental functions it needs, the same on every platform.

| Need | Proposed | Chosen | Why |
|---|---|---|---|
| Flattening curves | `kurbo` 0.13 | The engine halves each curve (`normalize::stroke`), since M3.4 | `kurbo`'s `flatten` calls the platform's `powf` and `hypot`, whose last bits differ from one platform to the next |
| A fill's region | `i_overlay` 9 | The engine's region code (`normalize::region`), since M5.1 | An integer-snapped overlay moves the drawing's points onto its grid, and its loops cannot charge an element's work budget. The engine keeps every point the drawing has and decides sides of lines exactly. It charges its work as it goes |
| Offsets: satin rails, insets, grown fills | `i_overlay` 9 | The engine's offsetter (`normalize::offset`), since M4 and M5.7 | Ink/Stitch offsets with shapely, whose curves are GEOS's. The engine builds the same curves, and checks them against shapely's |
| The nearest point or line | `rstar` 0.13 | Scans of one element's points at a time, charged to the work budget (`normalize::near`) | The hoop's whole field plans in about 30 ms with them (`REQ-FILL-TAT-009`), so an index would buy nothing yet |
| Routing a fill's rows | `petgraph` 0.8 with a walk of our own | The engine's own graph (`generators::tatami::graph`), since M5.4 | Ink/Stitch's route depends on the order in which its graph library lists a node's edges, and the engine's graph lists them in that order |
| Transcendental functions | `libm` 0.2 | `libm` 0.2, through `stitchcraft_core::math` | The same bits on every platform |

The geometry lives with the code that uses it, in `normalize` and in the generators, rather than behind one
`geometry` module. Each piece follows what Ink/Stitch does in its place, and its docs say what that is.

## Spike M0.7

The criteria were written for the crates proposed. The spike measured the engine's own geometry against
them.

| Criterion | Result |
|---|---|
| 1. Booleans and offsets of 10,000 random polygons with holes, and of a degenerate corpus, give valid output without panicking or hanging | **Met.** The property test `any_polygon_with_holes_grows_and_shrinks_into_valid_parts` grew and shrank 10,000 random polygons with holes, and every part it got was valid. It ran with `PROPTEST_CASES=10000`, where CI runs 256 each time. The offsets agree with shapely's for 1,050 random lines, and the grown and shrunk polygons for 330. The region code's tests take random rings, and rings with copies of themselves a rounding error apart. Every loop charges the work budget, so a plan that runs too long ends with `SC-E0004` |
| 2. The same output bytes on Linux, macOS, Windows and wasm32 | **Met.** The tests run on the 3 systems, and the `wasm-goldens` gate makes every golden file of the conformance cases with `stitch` built for WASI and run by wasmtime ([determinism](../determinism.md#cross-platform-check)): 16 files, byte for byte |
| 3. Offsetting a 150 mm region with 1,000 vertices inward 300 times in under 200 ms natively | **Met at M7.6**: 65 to 160 ms (below). At M0.7 it was not: 218 to 448 ms, against GEOS's 104 to 149 ms for the same parts and points. Contour fill is the first stitch type to need it, and `cargo xtask perf` keeps the offsetter within it (NFR-PERF-3) |
| 4. Building for wasm32 adds under 300 KiB to the plug-in | **Met for the geometry**: no geometry crate is linked, so none adds to the plug-in. The engine, linked into the plug-in by M6.2, adds 844 KiB. A stitch family's module is 1,014 KiB, and *hello*'s, with the ABI shim and the JSON reader alone, is 171 KiB (a release build with thin LTO, at `opt-level` 3, in the compatibility gate's contract job). The engine's own geometry is part of the 844 KiB and isn't measured apart. VectorCraft takes modules of up to 32 MiB |

Criterion 3, measured on a 2.1 GHz Xeon in an optimized build: a ring 150 mm across, of 1,000 points, round
or with 7 waves of 2 mm, shrunk 300 times in steps of 0.25 mm, and of 0.4 mm. shapely 2.2.0, with the GEOS
3.14.1 its wheels bundle, ran on the same machine, and its parts and points came out the same as the engine's.
M7.6 measured the engine again on a 2.1 GHz Xeon, with the `insets` example of the
[performance budget](../guardrails.md). By then the region code put the rings on its grid, and the 4 took
141 to 356 ms. Sorted lists in place of tree maps and sets brought them within the criterion, with the same
parts, points and work ([offset curves](../algorithms/offset.md#speed)).

| Ring | Step | Engine at M0.7 | Engine before M7.6 | Engine at M7.6 | GEOS |
|---|---|---|---|---|---|
| Round | 0.25 mm | 218 ms | 311 ms | 105 ms | 104 ms |
| Round | 0.4 mm | 135 ms | 141 ms | 65 ms | 61 ms |
| 7 waves | 0.25 mm | 448 ms | 356 ms | 160 ms | 149 ms |
| 7 waves | 0.4 mm | 261 ms | 206 ms | 96 ms | 115 ms |

## Consequences

- No geometry crate is there to vet, update or replace. The conformance suite and the shapely oracles
  (`conformance/oracle/`) check the engine's geometry, and a change to it is a change of the engine.
- CI makes the golden files again as WebAssembly. A plan the plug-in makes is one the command line makes.
- At M0.7 the offsetter was 2 to 3 times slower than GEOS on large regions, when a fill offset each part
  at most twice, for its inset underlay and its grown rows. Contour fill offsets hundreds of times, and
  from M7.6 the offsetter's times are close to GEOS's above.
