# stitchcraft-testkit

Test support, used **only as a dev-dependency** (enforced by `cargo xtask layers`): the degenerate-shape
corpus, `proptest` strategies generated from the parameter registry, the conformance metrics (coverage,
containment, row spacing, furrows, topology) and invariant assertions.

**Now:** `plans` — the canonical stitch plans whose encodings are the format golden files
(`conformance/golden/formats/`); `equivalence` — what a machine does with a plan (needle-downs, cuts, pauses),
the oracle of the round-trip tests; `strategies` — random plans, with a fixed seed on every PR and
`PROPTEST_RNG_SEED` for fresh ones.

**Status:** grows with M1–M5. Design: `docs/src/design/conformance.md`.

Tests may unwrap and panic; this crate opts out of the no-panic lints at its root.
