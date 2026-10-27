# Determinism

<!-- implements: crates/stitchcraft-core/src/math.rs, crates/stitchcraft-core/src/exact.rs, crates/stitchcraft-core/src/rng.rs, apps/stitchcraft-cli/src/commands/bug_report.rs, apps/stitchcraft-cli/src/commands/bug_report/** -->

**Same input, same version ⇒ byte-identical machine file and preview, on Linux, macOS, Windows and
wasm32.** This makes golden-file conformance possible, makes bug reports reproducible, makes caches
safe, and means the docs images regenerated in CI match the ones on a contributor's laptop.

## Sources of non-determinism and how we remove them

| Source | Rule | Enforced by |
|---|---|---|
| Hash map iteration order | No `HashMap`/`HashSet` in engine, formats, render, params or adapters' output paths; use `BTreeMap`, `BTreeSet`, `IndexMap` or sorted `Vec` | `clippy::disallowed_types` in `clippy.toml` |
| Platform `libm` (`sin`, `cos`, `atan2`, `exp`, `log`, `powf` and their kin differ by last bits across OSes, and `powi`'s precision varies even between calls) | Call transcendental functions only through `stitchcraft_core::math`, which uses the pure-Rust `libm` crate, and square by multiplying | `clippy::disallowed_methods` on `f64::sin` & co., and on the `f32` ones |
| Which side of a line a point lies on, or of a circle: rounded, the answer comes out wrong for some points nearly on the line or the circle | `stitchcraft_core::exact::side` gives it exactly for the [offset curves](algorithms/offset.md), and `exact::incircle` for the [centre lines](algorithms/centre-lines.md)' triangulation ([below](#exact-orientation)) | unit tests against exact integer arithmetic |
| Randomness | Only `stitchcraft_core::rng::SplitMix64`, seeded per element from `random_seed` or a hash of the element id, with the OS's randomness and the time kept out | `clippy::disallowed_types` on `RandomState`, `SystemTime` and `Instant`; `deny.toml` lets `rand` and `getrandom` in only through the property tests' proptest |
| Floating-point contraction (FMA) | Rust does not contract `a * b + c` implicitly, and we never call `mul_add` | `clippy::disallowed_methods` on `f64::mul_add` and `f32::mul_add`, everywhere |
| Parallelism | Library crates run on the caller's thread alone. The parallel planning that the [engine pipeline](engine-pipeline.md#7-incremental-and-parallel-planning) plans for the command line joins its results in document order | `clippy::disallowed_methods` on `std::thread::spawn` and its kin |
| Sorting ties | Every sort uses a total order with an explicit tie-breaker (`f64::total_cmp`, then index) | review checklist; property tests shuffle inputs |
| Quantization drift | Quantize absolute positions once, at encode time, with round-half-even (`Point::to_tenths`, shared by the writers and the previews); deltas are differences of quantized values | format conformance (`REQ-FMT-002`, `REQ-RND-001`) |
| Rasterization (SIMD code paths, platform trigonometry in curve flattening) | tiny-skia without its `simd` feature; previews draw only lines and circles; grid positions convert to `f32` exactly; pure-Rust PNG encoding without timestamps ([rendering](rendering.md)) | golden PNG files compared on all three operating systems (`REQ-RND-002`); `f32` transcendental methods are disallowed like the `f64` ones |
| Dependency upgrades | `Cargo.lock` committed; golden files re-blessed only in a PR that explains why | CI + review |

## Exact orientation

Whether a point lies left of a line, right of it or on it is the sign of a cross product. Rounded, that
sign can come out wrong for points nearly in line. Geometry built on a wrong sign tears, with a corner
turned the wrong way or two crossing segments taken to miss. `stitchcraft_core::exact::side` decides the
sign in plain floating point when the product is farther from zero than its largest possible rounding
error, and otherwise computes it exactly as a sum of floats that do not overlap, with the error-free
sums and products of J. R. Shewchuk's *Adaptive Precision Floating-Point Arithmetic and Fast Robust
Geometric Predicates* (1997). Where both of its products are 0, as for points level or upright with each
other, the cross product is exactly 0 without that sum, since far from underflow a product is 0 only where
a factor is. It uses only `+`, `−` and `×`, and every platform gets the same answer, which is the true one.

`stitchcraft_core::exact::incircle` decides in the same way whether a point lies inside the circle through 3
others. Its answer is the sign of a 4 × 4 determinant. That is computed in plain floating point when it lies
farther from zero than Shewchuk's bound on its error, and exactly otherwise. A Delaunay triangulation that
flips its edges by a rounded test can flip one edge back and forth forever, and one built on the exact test
stops.

## The seeded PRNG

`SplitMix64` (Steele, Lea & Flood, 2014) is tiny, fast, statistically adequate for jitter, and fully
specified by its 64-bit state update — so its output can be written into a conformance case and will
never change with a dependency upgrade. An element's generator starts from its `random_seed`, mixed once
by SplitMix64 itself, or, when it has none, from the FNV-1a hash of its id. A seed replaces the id, as in
Ink/Stitch, so elements given one seed draw the same numbers. A part of an element can draw from its own stream
of the element's generator (`SplitMix64::stream`). The part's place keys the stream, so a change to one
part leaves the numbers of the others as they were. A running stitch keys one for each span between
corners. The derivations are documented and tested with fixed vectors.

## Cross-platform check

`ci.yml` runs `cargo test` on Linux, macOS and Windows for every pull request. Among the tests,
xtask's `every_data_case_but_the_oracle_passes_here` runs the conformance suite's data cases. Each test
sheet and plan is made again and encoded, and its machine files must equal their golden files byte for
byte, so all three operating systems make the bytes the machine checkpoints sewed. The previews'
golden files (`REQ-RND-002`), the formats' canonical plans (`REQ-FMT-001`) and the frozen reference
values of `stitchcraft-core`'s tests are compared on all three as well. The oracle case reads the files
with pyembroidery, which only the Linux gates job installs.

WebAssembly is checked too, since VectorCraft runs the engine as WebAssembly. `cargo xtask wasm` checks
that the library crates and the plug-in build for `wasm32-unknown-unknown`, and the lints above keep the
library crates off what that target lacks. The `wasm-goldens` gate (`cargo xtask wasm --goldens`) builds
`stitch` for `wasm32-wasip1` and runs it under wasmtime on every test-sheet and plan case, each with the
command line the case stands for. Each machine file it writes must be its golden file byte for byte. WASI
gives `stitch` the files it reads and writes. Both are WebAssembly, and the engine's maths is its own, so
its arithmetic is the plug-in's.

## Bug reports

Because the same design, profile, format and version give the same plan everywhere, those four are all a
bug report needs, with the switches the design was read with. `--auto` decides how its shapes without
settings sew, and `--sort-colours` whether its shapes with settings sew in the order StitchCraft chooses.
`stitch bug-report` writes them into one JSON file, with the design itself in place of a path to it, and
each switch only when it was on. A bundle of a run without them is as it always was. The design is an SVG
file whole, or the part of a VectorCraft document its reader reads, which
reads as the whole document does. The file also records what came of them:

- a SHA-256 of each entry of the plan, from each coordinate's bits, its kind, role and element, and each
  block's thread
- the machine file's size and SHA-256, and the design name its label holds, which the replay uses as it is
- the first line of each diagnostic, and a panic's message if there was one, with the phase it came in
  (planning, drawing the picture or writing the files)

A replay plans the design again, and draws the picture again for a panic that came while drawing it. It
leaves the files unwritten. `stitch --version` gives the version a build will be released as. Builds from a
git checkout add their commit, which bundles record too. A replay reads the bundle's format first, and
refuses a format it does not know by its number, whatever else the file holds.

`stitch bug-report --replay` plans the design again through the same code as `stitch plan` and compares
each record (`REQ-CLI-001`). `stitch plan` writes a bundle by itself when it finds a bug in StitchCraft
(`REQ-CLI-002`). The user guide's [Report a bug](../user/how-to/report-a-bug.md) shows both.

The plan's digest is written out entry by entry rather than from the plan's `Debug` text, so that a
compiler that prints numbers differently cannot change it.

## What is *not* promised

- Output across StitchCraft versions: a new version may change stitches. Changes to golden files are
  reviewed and listed in the changelog.
- Wall-clock timings: budgets count work units precisely so that results never depend on speed.
