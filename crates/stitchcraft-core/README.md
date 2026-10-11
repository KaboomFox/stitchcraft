# stitchcraft-core

Layer **L0**. The foundations every other crate builds on.

| Module | Purpose |
|---|---|
| `units` | `Mm` (a finite length in millimetres), `Size` and `Point` (finite, y down); host-unit conversions |
| `rect` | `Rect`, the bounds of a design or a part of it (never empty, never non-finite) |
| `math` | Transcendental functions through the pure-Rust `libm` crate, so results are identical on every platform |
| `exact` | `side` tells which side of a line a point lies on. Shewchuk's adaptive arithmetic makes the answer exact, and geometry built on it never turns a corner the wrong way |
| `rng` | `SplitMix64`, the only random number generator StitchCraft uses; seeds derived from element ids |
| `budget` | `Budget` and `Meter`: deterministic work limits; exhaustion becomes diagnostic `SC-E0004` |
| `diag` | `Diagnostic`, `Severity`, `Fix`, and the **registry**: every `Code` with its title and explanation |
| `element` | `ElementId`, the host's stable id for one object of the design |

The diagnostics registry is a single `registry!` block in `src/diag.rs`: an entry's doc comment *is* the
explanation users read (`stitch explain`, the diagnostics index), and tests reject entries without one.

## Invariants

- Every `Mm` and `Point` holds finite values; construction from untrusted numbers is checked.
- `math` and `rng` produce bit-identical results on Linux, macOS, Windows and wasm32; tests freeze
  reference values so a dependency change that alters them fails CI.
- `exact::side` gives the true side for any finite points, the same on every platform (tested against
  integer arithmetic).
- Diagnostic codes are unique, never reused, and their letter matches their severity (tested).
- A `Meter` never wraps around: once a limit is reached every further charge fails.
- No dependency on any other StitchCraft crate.

## Dependencies

`libm` (MIT) for deterministic math, `thiserror` for error types.
