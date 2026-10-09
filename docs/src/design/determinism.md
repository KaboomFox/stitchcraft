# Determinism

<!-- implements: crates/stitchcraft-core/src/math.rs, crates/stitchcraft-core/src/rng.rs -->

**Same input, same version ⇒ byte-identical machine file and preview, on Linux, macOS, Windows and
wasm32.** This makes golden-file conformance possible, makes bug reports reproducible, makes caches
safe, and means the docs images regenerated in CI match the ones on a contributor's laptop.

## Sources of non-determinism and how we remove them

| Source | Rule | Enforced by |
|---|---|---|
| Hash map iteration order | No `HashMap`/`HashSet` in engine, formats, render, params or adapters' output paths; use `BTreeMap`, `BTreeSet`, `IndexMap` or sorted `Vec` | `clippy::disallowed_types` in `clippy.toml` |
| Platform `libm` (`sin`, `cos`, `atan2`, `exp`, `powf` differ by last bits across OSes) | Call transcendental functions only through `stitchcraft_core::math`, which uses the pure-Rust `libm` crate | `clippy::disallowed_methods` on `f64::sin` & co. |
| Randomness | Only `stitchcraft_core::rng::SplitMix64`, seeded per element from `random_seed` or a hash of the element id; never the OS RNG, never time | `clippy::disallowed_methods`/types for `rand::thread_rng`, `SystemTime`, `Instant` in libraries |
| Floating-point contraction (FMA) | Rust does not contract `a * b + c` implicitly; we never call `mul_add` in quantization or comparisons | `clippy::disallowed_methods` on `f64::mul_add` in formats |
| Parallelism | Results joined in document order; no reductions whose order depends on scheduling | engine design; CI compares sequential vs parallel output |
| Sorting ties | Every sort uses a total order with an explicit tie-breaker (`f64::total_cmp`, then index) | review checklist; property tests shuffle inputs |
| Quantization drift | Quantize absolute positions once, at encode time, with round-half-even (`Point::to_tenths`, shared by the writers and the previews); deltas are differences of quantized values | format conformance (`REQ-FMT-002`, `REQ-RND-001`) |
| Rasterization (SIMD code paths, platform trigonometry in curve flattening) | tiny-skia without its `simd` feature; previews draw only lines and circles; grid positions convert to `f32` exactly; pure-Rust PNG encoding without timestamps ([rendering](rendering.md)) | golden PNG files compared on all three operating systems (`REQ-RND-002`); `f32` transcendental methods are disallowed like the `f64` ones |
| Dependency upgrades | `Cargo.lock` committed; golden files re-blessed only in a PR that explains why | CI + review |

## The seeded PRNG

`SplitMix64` (Steele, Lea & Flood, 2014) is tiny, fast, statistically adequate for jitter, and fully
specified by its 64-bit state update — so its output can be written into a conformance case and will
never change with a dependency upgrade. Element seeds are derived by FNV-1a hashing of the element id
and mixing in `random_seed`; the derivation is documented and tested with fixed vectors.

## Cross-platform check

From M2 (when there are plans to hash), `ci.yml` runs the conformance suite on `ubuntu-latest`,
`macos-latest` and `windows-latest` and, through `wasmtime`, on `wasm32-wasip1` (test-only; shipped code
targets `wasm32-unknown-unknown`). Until then the frozen reference values in `stitchcraft-core`'s tests run
on all three operating systems. Each run writes
`target/conformance/hashes.json` (one SHA-256 per output); a final job fails if any two platforms
disagree, and prints the first differing case.

## What is *not* promised

- Output across StitchCraft versions: a new version may change stitches. Changes to golden files are
  reviewed and listed in the changelog.
- Wall-clock timings: budgets count work units precisely so that results never depend on speed.
