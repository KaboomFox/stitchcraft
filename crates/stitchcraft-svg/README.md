# stitchcraft-svg

Layer **L3**. Reads SVG documents into the engine's `Design`: `stitchcraft_svg::read(bytes, budget)`
returns the design and warnings about what was left out or simplified. Ink/Stitch's `inkstitch:*`
settings, command symbols and clones follow in M8, and so does writing plans back as SVG for inspection.

**Status:** v1 (M3.3) reads paths and the basic shapes, groups and `<switch>`, transforms, the root's
size and viewBox (so positions come out in millimetres), fill and stroke colours with `currentColor` and
`paint-order`, and everything that hides an element. Design: `docs/src/design/data-model.md` (the
`Design`), `docs/src/design/architecture.md#hosts-ports-and-adapters`.

## Invariants

- Contains no stitch logic; only translation.
- Never fails on content: any path data, degenerate arcs included, is read the way SVG viewers draw it
  (`REQ-SVG-002`); the reader is fuzzed nightly (`fuzz/fuzz_targets/read_svg.rs`). Only a file that is not
  SVG at all is refused (`SC-E0801`).
- Nothing is dropped silently: `SC-W0802` for SVG features it does not stitch, `SC-W0804` for geometry
  it cannot use, unless a viewer would not show it either.
- Positions are exact to within a micrometre (`REQ-SVG-001`) and the same on every platform: transforms
  and arcs use `stitchcraft_core::math`, never the platform's trigonometry.
- Work is bounded: 64 MiB per file, a million XML nodes, no entity declarations, and the budget.
- Parameters will round-trip unchanged (`REQ-PRM-003`, M8).

## Dependencies

`stitchcraft-core`, `stitchcraft-params`, `stitchcraft-plan`, `stitchcraft-engine`; `roxmltree` (the XML
tree) and `svgtypes` (SVG's micro-syntaxes), the versions VectorCraft's lockfile carries, and only here
(`cargo xtask layers`).
