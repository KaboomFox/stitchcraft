# Changelog

All notable changes are listed here, newest first. Golden-file changes must be listed with their reason.

## Unreleased

### Added
- M2.6: preview renderer (`stitchcraft-render`) in a realistic and a simple style, drawn from the
  positions a machine file makes (`REQ-RND-001`) and byte-identical on every platform (`REQ-RND-002`);
  diagnostic `SC-E0801` for previews larger than 4,096 pixels on a side. Writers and previews share one
  rounding function, `Point::to_tenths` in `stitchcraft-core`.
- M2.4: an independent reader, pinned pyembroidery, reads every golden machine file the way
  StitchCraft's reader does (conformance case `pyembroidery-oracle`, run in CI).
- M2.3: round-trip property tests for every writer/reader pair, judged by machine-visible behaviour
  (`stitchcraft-testkit::equivalence`), with fixed seeds per PR and fresh seeds nightly.
- M2.1–M2.2: PES/PEC and DST readers (any PES version's PEC block; trims inferred from DST jump runs)
  and `stitch inspect` for any machine file, with an optional profile check; diagnostic `SC-E0603`.
- M1 (machine checkpoint MC-1 pending): budgets and the diagnostics registry (`stitch explain`); the
  stitch plan, its invariant checker and the `brother-200x200` profile with hoop and comfort-zone
  diagnostics; the Brother PEC palette with CIEDE2000 matching; PES v1 and DST writers; test sheets
  TS-01, TS-02 and TS-10A/B/C (`stitch testsheet`); `stitch profiles`; the conformance runner and its
  report (`cargo xtask conformance`), with requirements named by Rust tests (`req_<area>_<nnn>_…`);
  reference pages generated from the code (command line, profiles, formats, test sheets, diagnostic
  codes) and the first-sew-out tutorial with real output.

### Fixed
- DST reading: a long jump followed by a long move back could be read as a trim (their split pieces
  cancelled exactly); trims are now runs of jumps of at most 1 mm, and the writer no longer writes
  zero-length jumps.

### Golden files
- Added `conformance/golden/render/`: previews of a sampler design (simple and realistic) and of TS-01
  (realistic). They pin how previews look; a change to them is a change users will see.
- Added `conformance/golden/formats/` (canonical plans `every-command` and `one-stitch`, PES and DST)
  and `conformance/golden/testsheets/` (the exact MC-1 files: TS-01 PES and DST, TS-02, TS-10A/B/C PES).
- M0 bootstrap: design documents, roadmap and machine-testing protocol; layered workspace with
  `stitchcraft-core` foundations (millimetre units, deterministic math, SplitMix64); `cargo xtask` gates
  (`ci`, `layers`, `docs --check`, `conformance --check`, `cleanroom`, `unsafe-audit`, `filesize`,
  `wasm`); GitHub Actions for CI, docs, VectorCraft compatibility discovery and nightly checks.
