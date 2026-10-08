# stitchcraft-svg

Layer **L3**. Adapts SVG documents into the engine's `Design`: geometry, transforms, units from the
viewBox, colours, visibility, and — from M8 — Ink/Stitch's `inkstitch:*` parameters, command symbols and
clones. Also writes plans back as SVG for inspection.

**Status:** v1 (geometry) in M3.3, Ink/Stitch interoperability in M8. Design:
`docs/src/design/architecture.md#hosts-ports-and-adapters`, `docs/src/design/inkstitch-compat-contract.md`.

## Invariants

- Contains no stitch logic; only translation.
- Path data, including degenerate arcs, never fails the adapter (`REQ-SVG-002`); input is fuzzed.
- Parameters round-trip unchanged (`REQ-PRM-003`).
