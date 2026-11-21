# stitchcraft-testkit

Test support, used **only as a dev-dependency** (enforced by `cargo xtask layers`): the degenerate-shape
corpus, `proptest` strategies of random plans and paths, the conformance metrics (coverage, containment,
row spacing, furrows, topology) and invariant assertions. Strategies made from the parameter registry are
planned, in no step yet (`docs/src/design/params.md`).

**Now:**

- `plans`: the canonical stitch plans whose encodings are the format golden files
  (`conformance/golden/formats/`).
- `equivalence`: what a machine does with a plan (needle-downs, cuts, pauses), and the cuts each format
  adds (`format_events`, with `as_pes` for the trim a PES file puts before every jump). It is the oracle
  of the round-trip tests.
- `strategies`: random plans, with a fixed seed on every PR and `PROPTEST_RNG_SEED` for fresh ones.
- `fuzz`: the bodies of the fuzz targets in `fuzz/`, run here on every PR and with coverage-guided inputs
  every night. Readers never panic and keep to their caps. Anything read is written in every format and
  reads back to the plan its format makes of it. Its previews never panic.
- `temp`: a folder for the files one test writes. It is removed with its contents when the test ends,
  passed or failed, and test runs leave the system's temporary folder as they found it.

**Since M5.9:** `metrics` measures conformance cases the same way in every case. Its first metric is
coverage, the share of a fill's area that the thread of a plan's stitches covers, rasterized at 0.05 mm
with a 0.4 mm thread. `designs` builds the small designs the engine's cases plan.

**Since M13:** `pictures` places the test pictures of automatic digitizing, and the TS-13 sheets' stitches,
where the cases measure them: the traced pear and dogs of `docs/fixtures/`, and a lettered patch drawn in
code. `stitches` reads back each element's needle points, a satin column's rungs and the jump threads the
PE800 leaves. `satins` sews straight satin columns for the engine's satin cases and reads their needle points
back as pairs across them. `strokes` holds the strokes of the traced word's M and A, as a person draws them,
for the cases of letters sewn as strokes (`REQ-AUTO-087`). `transformed` mirrors, turns and redraws a design,
for the cases that its stitches change only by the same change. `inkstitch` gives the Ink/Stitch
differential `stitch expand`'s SVG writer (`stitchcraft_svg::written`), with Ink/Stitch's settings alone, so
that a test sheet sews in both programs from one file.

**Since M7:** `degenerate` holds the degenerate corpus, which the engine's `tests/degenerate.rs` runs
through every stitch type. Among its shapes are points and lines of no length, rings of no area, hair-thin
and self-touching shapes, rings twice over, rails that cross, a square near the edge of where a design may
lie, and a path of 10⁶ points. Its `random` strategy draws paths on a coarse grid, where points coincide and lie in line often.

**Since M6.15:** the example `sheet` measures the standard pictures of the before and after sheet with
the `fabric` module. It draws each one too, for `web/sheet.py` (`docs/src/design/before-after-sheet.md`).

**Since M13.29:** `fabric::thickness` reads how thick a plan sews from the plan's own measure
(`stitchcraft_plan::thickness`), which gives the grid's pixel and the thread's width to `metrics` too. The
example `sheet` writes the thickest spot, the area at 3 fills or more and the most needle points.

**Status:** grows with every milestone. Design: `docs/src/design/conformance.md`.

## Dependencies

Every StitchCraft library: `stitchcraft-core`, `stitchcraft-params`, `stitchcraft-plan`,
`stitchcraft-engine`, `stitchcraft-formats`, `stitchcraft-render`, `stitchcraft-svg` and
`stitchcraft-vectorcraft`, which its cases plan, write, draw and read. `proptest` for the strategies, and
`flate2` for the compressed documents the fuzz bodies write (`cargo xtask layers` keeps `flate2` here and in
the VectorCraft adapter). It is only ever a dev-dependency, and the libraries it depends on use it in their tests.

Tests may unwrap and panic; this crate opts out of the no-panic lints at its root.
