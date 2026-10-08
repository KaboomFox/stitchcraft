# stitchcraft-vectorcraft

Layer **L3**. Adapts VectorCraft data into the engine's `Design`: `.vectorcraft` documents (layers, groups,
paths, compound paths, paints, visibility, appearance effect records holding our parameters) and the JSON
objects VectorCraft's plug-in ABI v1 passes to live effects and filters.

**Status:** planned for M6.1. Design: `docs/src/design/vectorcraft-integration.md`; supported VectorCraft
versions are proven by `docs/src/design/compatibility-gate.md`.

## Invariants

- Depends on VectorCraft only through its documented file format and plug-in JSON, never its crates.
- Treats every file as untrusted: size and depth caps, typed errors, fuzzed.
- Contains no stitch logic.
