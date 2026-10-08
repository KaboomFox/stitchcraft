# stitchcraft-params

Layer **L0** (may use `stitchcraft-core`). The parameter registry: every embroidery parameter is declared
once, next to its generator, with the `params!` macro; typed structs, validation, VectorCraft manifests, CLI
help, SVG attribute mapping, reference docs and property-test strategies are generated from it.

**Status:** planned for M3.1. Design: `docs/src/design/params.md`, `docs/src/design/adr/0003-parameter-registry.md`.

## Invariants

- Registry keys equal Ink/Stitch attribute names where the meaning matches, and defaults equal Ink/Stitch's
  (the interoperability contract in `conformance/inkstitch-params.toml`).
- Invalid values never fall back silently to defaults: they produce coded diagnostics.
- Knows no stitch algorithm.
