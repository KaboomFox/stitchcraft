# stitchcraft-render

Layer **L2**. Draws stitch plans as PNG images, in a realistic style (thread on fabric) and a simple style
(lines, needle holes, dashed travel, marks for trims, stops and lock stitches). Used by `stitch preview`,
the documentation images and the conformance suite. Design: `docs/src/design/rendering.md`.

## Invariants

- **What is shown is what sews** (`REQ-RND-001`): every position is rounded with the writers' own
  function (`Point::to_tenths`) before anything is drawn. A plan and its PES file read back give the
  same scene.
- **Deterministic** (`REQ-RND-002`): the same plan and settings give the same PNG bytes on every
  platform; golden files in `conformance/golden/render/` check it on Linux, macOS and Windows.
- Never crashes: every failure is a `RenderError` with a registered diagnostic code (`SC-E0005` for an
  image larger than 4,096 pixels on a side); drawing charges the caller's work budget.

## Structure

- `scene.rs` — the plan as the fabric will show it, in machine units: holes, how the thread arrived at
  each (sewn, loose, cut), trims and stops. Plain data, compared by tests.
- `raster.rs` — the two styles, drawn with tiny-skia (no SIMD) and encoded as PNG.
- `error.rs` — `RenderError` and its diagnostics.

## Dependencies

`stitchcraft-core`, `stitchcraft-plan`, `tiny-skia` (only this crate may use it; `cargo xtask layers`),
`thiserror`. Tests also use `stitchcraft-formats`, `stitchcraft-engine` and `stitchcraft-testkit`.
