# Changelog

All notable changes are listed here, newest first. Golden-file changes must be listed with their reason.

## Unreleased

### Added
- M1 (machine checkpoint MC-1 pending): budgets and the diagnostics registry (`stitch explain`); the
  stitch plan, its invariant checker and the `brother-200x200` profile with hoop and comfort-zone
  diagnostics; the Brother PEC palette with CIEDE2000 matching; PES v1 and DST writers; test sheets
  TS-01, TS-02 and TS-10A/B/C (`stitch testsheet`); `stitch profiles`; the conformance runner and its
  report (`cargo xtask conformance`), with requirements named by Rust tests (`req_<area>_<nnn>_…`);
  reference pages generated from the code (command line, profiles, formats, test sheets, diagnostic
  codes) and the first-sew-out tutorial with real output.

### Golden files
- Added `conformance/golden/formats/` (canonical plans `every-command` and `one-stitch`, PES and DST)
  and `conformance/golden/testsheets/` (the exact MC-1 files: TS-01 PES and DST, TS-02, TS-10A/B/C PES).
- M0 bootstrap: design documents, roadmap and machine-testing protocol; layered workspace with
  `stitchcraft-core` foundations (millimetre units, deterministic math, SplitMix64); `cargo xtask` gates
  (`ci`, `layers`, `docs --check`, `conformance --check`, `cleanroom`, `unsafe-audit`, `filesize`,
  `wasm`); GitHub Actions for CI, docs, VectorCraft compatibility discovery and nightly checks.
