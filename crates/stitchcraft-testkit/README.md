# stitchcraft-testkit

Test support, used **only as a dev-dependency** (enforced by `cargo xtask layers`): the degenerate-shape
corpus, `proptest` strategies generated from the parameter registry, the conformance metrics (coverage,
containment, row spacing, furrows, topology) and invariant assertions.

**Status:** grows with M1–M5. Design: `docs/src/design/conformance.md`.

Tests may unwrap and panic; this crate opts out of the no-panic lints at its root.
