# ADR-0004: Determinism by construction

**Status:** Accepted · 2026-10-08

## Context

Golden-file conformance, reproducible bug reports, safe caching and CI-regenerated docs images all
require identical output across platforms and runs.

## Decision

Ordered collections only; transcendental math through `libm`; a specified PRNG (SplitMix64) with seeds
from element ids and `random_seed`; quantization of absolute positions once at encode time; total-order
sorts with tie-breakers; budgets in work units, not time; a CI job comparing output hashes across Linux,
macOS, Windows and wasm. Enforced by clippy `disallowed-types`/`disallowed-methods`. Details:
[determinism](../determinism.md).

## Consequences

- Slightly more verbose code (`BTreeMap`, `math::sin`); negligible performance cost.
- Byte-exact goldens are possible for formats and stitch renders.

## Alternatives considered

- **Tolerance-only comparisons everywhere:** hides real regressions and makes images flaky. Rejected
  except for VectorCraft UI screenshots, which we do not control.
