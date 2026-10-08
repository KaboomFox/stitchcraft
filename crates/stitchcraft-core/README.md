# stitchcraft-core

Layer **L0**. The foundations every other crate builds on.

| Module | Purpose |
|---|---|
| `units` | `Mm` (a finite length in millimetres) and `Point` (finite, y down); host-unit conversions |
| `math` | Transcendental functions through the pure-Rust `libm` crate, so results are identical on every platform |
| `rng` | `SplitMix64`, the only random number generator StitchCraft uses; seeds derived from element ids |

Planned here (M1.1): `Budget` (deterministic work limits) and the diagnostics model (`Diagnostic`, `Code`,
`Severity`) with its registry.

## Invariants

- Every `Mm` and `Point` holds finite values; construction from untrusted numbers is checked.
- `math` and `rng` produce bit-identical results on Linux, macOS, Windows and wasm32; tests freeze
  reference values so a dependency change that alters them fails CI.
- No dependency on any other StitchCraft crate.

## Dependencies

`libm` (MIT) for deterministic math, `thiserror` for error types.
