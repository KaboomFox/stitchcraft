# stitchcraft-testkit

Test support, used **only as a dev-dependency** (enforced by `cargo xtask layers`): the degenerate-shape
corpus, `proptest` strategies generated from the parameter registry, the conformance metrics (coverage,
containment, row spacing, furrows, topology) and invariant assertions.

**Now:** `plans` — the canonical stitch plans whose encodings are the format golden files
(`conformance/golden/formats/`); `equivalence` — what a machine does with a plan (needle-downs, cuts, pauses),
the oracle of the round-trip tests; `strategies` — random plans, with a fixed seed on every PR and
`PROPTEST_RNG_SEED` for fresh ones; `fuzz` — the bodies of the fuzz targets in `fuzz/` (readers never
panic and respect their caps; anything read is written, read back from PES unchanged in what the machine
does, and previewed without a panic), run here on every PR and with coverage-guided inputs every night.

**Since M5.9:** `metrics` measures conformance cases the same way in every case. Its first metric is
coverage, the share of a fill's area that the thread of a plan's stitches covers, rasterized at 0.05 mm
with a 0.4 mm thread. `designs` builds the small designs the engine's cases plan.

**Status:** grows with M1–M5. Design: `docs/src/design/conformance.md`.

Tests may unwrap and panic; this crate opts out of the no-panic lints at its root.
