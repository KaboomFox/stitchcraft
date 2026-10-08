# StitchCraft — instructions for contributors and agents

StitchCraft is a clean-room, MIT OR Apache-2.0 machine-embroidery engine in Rust, with plug-ins for
[VectorCraft](https://github.com/storytold/vectorcraft). This file is the contract for everyone who
changes the code — people and AI agents alike. `CLAUDE.md` points here; there is only one copy.

## Start every session here

1. Read `ROADMAP.md` (status) and pick the next step from `docs/src/plan/roadmap.md`, unless you were
   given a task.
2. Read the design pages that step touches, starting from `docs/src/design/README.md`. The technical
   design (`docs/src/design/tdd.md`) is the overview.
3. Work conformance-first: add requirements and failing cases, then the code that makes them pass, then
   the docs. A Rust test named `req_<area>_<nnn>_<what>` is a case for `REQ-<AREA>-<NNN>`. Finish with
   `cargo xtask ci`.

## Non-negotiables

- **Clean room.** Never open, copy or transliterate Ink/Stitch source code (GPL-3.0) or any GPL/AGPL
  code. Behaviour comes from our design docs, public documentation, published papers and format
  specifications. Ink/Stitch's parameter *names*, defaults and method ids are the interoperability
  contract and are used as-is (`docs/src/design/adr/0001-license-and-clean-room.md`).
- **Never crash.** No `unwrap`, `expect`, `panic!`, `unreachable!`, `todo!`, `unimplemented!` or
  `dbg!` outside tests; no unchecked indexing on data; return `Result` or a coded diagnostic. Every loop
  over input charges a `Budget`. Tests may unwrap.
- **No `unsafe`,** except the VectorCraft ABI shim `apps/stitchcraft-vc-plugin/src/abi.rs`, where every
  block has a `// SAFETY:` comment (`cargo xtask unsafe-audit`).
- **Deterministic.** No `HashMap`/`HashSet`, platform transcendental math, OS randomness or clocks in
  library crates; use `stitchcraft_core::{math, rng}` and ordered collections.
- **Layered.** A crate depends only on lower layers (`cargo xtask layers`; table in `xtask/src/layers.rs`).
- **One source of truth (DRY).** Parameters, diagnostics, profiles and formats live in their registries;
  docs, UI schemas, CLI help and test strategies are generated from them. Never hand-write a parameter's
  docs or duplicate a constant — import it.
- **Verify bugs before fixing them.** Reproduce with a failing test or conformance case first; a fix
  without a reproducing case is not done. When citing a bug in another project, cite evidence you
  checked (issue page, commit, code you ran).
- **No silent fallbacks.** A degraded result always emits a coded diagnostic.
- **Docs extend context.** Every crate has a `README.md` stating its purpose, invariants and
  dependencies; every module starts with `//!` docs explaining *why*. Update docs in the same PR as the
  behaviour.
- **One roadmap step per PR,** titled with its id (`M3.4: running stitch even spacing`).

## Commands

```sh
cargo xtask ci                     # every gate: fmt, clippy, tests, layers, filesize, cleanroom,
                                   # unsafe-audit, docs --check, shots --check, conformance (the whole
                                   # suite, with its report), wasm, deny, typos, book
cargo xtask layers                 # crate dependency rules
cargo xtask docs --check           # docs fresh, links and anchors valid, ids exist
cargo xtask docs                   # regenerate generated docs pages
cargo xtask shots --check          # docs images declared, reproducible and current
cargo xtask conformance            # run the suite; report in target/conformance/report.md
cargo xtask conformance --check    # requirements and cases consistent (no run)
cargo xtask conformance --bless ID # rewrite one data case's golden files, on purpose
cargo xtask cleanroom              # no GPL text or Ink/Stitch source paths in code and fixtures
cargo xtask unsafe-audit           # unsafe only in the ABI shim, always with SAFETY comments
cargo xtask filesize               # warn above 800 lines, fail above 1,500
cargo xtask wasm                   # library crates build for wasm32-unknown-unknown
```

Tools that are not installed locally (the wasm target, `cargo-deny`, `typos`, `mdbook`) are reported as
skipped by `cargo xtask ci`; CI installs them and requires them.

## Where code goes

See "Where does my code go?" in `docs/src/design/architecture.md`, and the playbooks in
`docs/src/contributing/` for new stitch types, parameters, formats and diagnostics.

## VectorCraft

The plug-in targets VectorCraft's WebAssembly ABI v1 (`docs/src/design/vectorcraft-integration.md`).
Do not depend on VectorCraft internals beyond that ABI and its documented `.vectorcraft` format; the
compatibility gate (`docs/src/design/compatibility-gate.md`) tests every VectorCraft release, pre-release,
`release` branch and `main`. Proposed VectorCraft changes go through the ABI v2 RFC, following
VectorCraft's own `AGENTS.md`.

## Machine testing

Changes that affect what sews are listed for the next machine checkpoint
(`docs/src/plan/machine-testing.md`). Profile values change only with a sew-out report that justifies them.
