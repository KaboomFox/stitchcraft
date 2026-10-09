# stitchcraft-svg

Layer **L3**. Reads SVG documents into the engine's `Design`: `stitchcraft_svg::read(bytes, budget)`
returns the design and warnings about what was left out or simplified. Ink/Stitch's other `inkstitch:*`
settings, its remaining commands and clones follow in M8, and so does writing plans back as SVG for
inspection.

**Status:** v1 (M3.3) reads paths and the basic shapes, groups and `<switch>`, transforms, the root's
size and viewBox (so positions come out in millimetres), fill and stroke colours with `currentColor` and
`paint-order`, and everything that hides an element. Ink/Stitch's own objects are not stitched
(`src/inkstitch.rs`, `REQ-SVG-003`):
- command symbols and the connectors that tie them to objects;
- lines drawn with Inkscape's connector tool;
- guide, anchor-line and pattern helper paths.

Its trim and stop commands set `trim_after` and `stop_after`. Its ignore commands and `ignore_object`
setting leave objects and layers out and list them (`SC-I0805`, `REQ-ASM-004`).

Design: `docs/src/design/data-model.md` (the `Design`), `docs/src/design/architecture.md#hosts-ports-and-adapters`.

## Invariants

- Contains no stitch logic; only translation.
- Never fails on content: any path data, degenerate arcs included, is read the way SVG viewers draw it
  (`REQ-SVG-002`); the reader is fuzzed nightly (`fuzz/fuzz_targets/read_svg.rs`). Only a file that is not
  SVG at all is refused (`SC-E0801`).
- Nothing is dropped silently: `SC-W0802` for SVG features it does not stitch, `SC-W0804` for geometry
  it cannot use, `SC-I0805` for what the file itself asks to leave out, unless a viewer would not show it
  either. Ink/Stitch's command symbols and their connectors are its controls, not drawing, so they are
  left out without a word; a command that is not applied is noted.
- Positions are exact to within a micrometre (`REQ-SVG-001`) and the same on every platform: transforms
  and arcs use `stitchcraft_core::math`, never the platform's trigonometry.
- Work is bounded. A file is at most 64 MiB with its entities expanded (`src/text.rs`) and a million XML
  nodes, and reading charges the budget two units per XML node: one to find ids and Ink/Stitch's
  commands, one to read it.
- Parameters will round-trip unchanged (`REQ-PRM-003`, M8).

## Dependencies

`stitchcraft-core`, `stitchcraft-params`, `stitchcraft-plan`, `stitchcraft-engine`; `roxmltree` (the XML
tree) and `svgtypes` (SVG's micro-syntaxes), the versions VectorCraft's lockfile carries, and only here
(`cargo xtask layers`).
