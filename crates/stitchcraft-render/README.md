# stitchcraft-render

Layer **L2**. Renders stitch plans to images: a realistic thread look, a simple line look, and overlays for
jumps, trims and lock stitches. Used by `stitch preview`, the docs images and conformance diffs.

**Status:** planned for M2.6. Design: `docs/src/design/docs-pipeline.md` (images), `REQ-RND-001`.

## Invariants

- Renders the plan *after* quantization, so a preview is what will sew.
- Deterministic: identical bytes for identical input on every platform (docs images are compared exactly).
