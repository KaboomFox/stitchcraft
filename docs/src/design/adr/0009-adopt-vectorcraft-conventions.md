# ADR-0009: Adopt VectorCraft's conventions; improve the ones that drift

**Status:** Accepted · 2026-10-08

## Context

StitchCraft lives next to VectorCraft and may one day be embedded in it. Contributors who know one
codebase should feel at home in the other, and an upstream PR is easier when our code already follows
their rules. We audited VectorCraft (`main` at `89ea46c`) for patterns worth following and checked
whether each one is actually enforced.

## Audit

| # | VectorCraft pattern | Evidence | Verdict | StitchCraft |
|---|---|---|---|---|
| 1 | **Never crash**: `unwrap`, `expect`, `panic!`, `todo!`, `unimplemented!`, `unreachable!` denied workspace-wide; documented alternatives | `Cargo.toml` `[workspace.lints.clippy]`; `docs/development.md` "Robustness" | Adopt | Same lints, plus `indexing_slicing` denied in parsers and `dbg_macro` |
| 2 | Panic guard at every entry point | `vectorcraft_engine::guard` | Adapt | Libraries never need it; the CLI wraps `main` and writes a bug-report bundle; the wasm plug-in cannot catch panics, so the lints are absolute |
| 3 | **Everything is a command** in a declarative registry reachable from UI, control channel and MCP | `crates/engine/src/cmd/*`, `cmd!` macro | Adopt the idea, improve the data | Everything user-visible is registered (parameters, diagnostics, profiles, formats, CLI commands); parameters are **typed schemas** that generate docs, where VectorCraft documents command parameters as free-form strings |
| 4 | **Layering enforced** by `cargo xtask layers` with an append-only table and a unit-tested rule engine | `xtask/src/layers.rs` | Adopt | Same model in `xtask/src/layers.rs` |
| 5 | One local gate, `cargo xtask ci`, with a step summary | `xtask/src/main.rs` (`cmd_ci`) | Adopt and extend | Same command — **and** the same gates run on every pull request in GitHub Actions. VectorCraft's workflows run the test suite on a pull request only when it changes Cargo manifests or FreeBSD packaging (`freebsd.yml`), relying on contributors running the gate locally; for external contributions we want required checks |
| 6 | `AGENTS.md` as the contract for contributors and agents | `AGENTS.md` | Adopt, keep DRY | `CLAUDE.md` is a pointer to `AGENTS.md`. VectorCraft keeps two copies that have drifted: `CLAUDE.md` (63 lines) lacks `AGENTS.md`'s (81 lines) contributor-credits section |
| 7 | Clean-room rules with an automated check | `AGENTS.md` says `cargo xtask cleanroom` runs in `cargo xtask ci` | Adopt and **implement** | At `89ea46c` there is no `cleanroom` subcommand in `xtask/src/main.rs` and `cmd_ci` does not run one — documentation and code drifted. Our `cargo xtask cleanroom` exists, runs in CI, and `cargo xtask docs --check` fails when docs mention a subcommand that does not exist |
| 8 | Asset attribution enforced (`ASSETS.md`, checked by VectorCraft's `xtask assets` step) | `xtask/src/assets.rs` | Adopt | Every committed image, font or fixture has a provenance row; generated images are attributed to their shot declaration |
| 9 | Untrusted input is fuzzed with property tests | `engine/tests/import_fuzz.rs`, `format/tests/prop_format.rs` | Adopt and extend | Property tests on every parser, plus coverage-guided `cargo-fuzz` nightly with a persisted corpus |
| 10 | `cargo xtask wasm` keeps crates building for the web | `xtask/src/main.rs` (`cmd_wasm`) | Adopt | Same, for L0–L3 and the plug-in |
| 11 | Shared corpora pinned by commit and SHA-256, never committed | `AGENTS.md`, craftrules `standards/test-corpora.md` | Adopt | `cargo xtask corpus` |
| 12 | Honest `ROADMAP.md`, updated in the same PR as the work | `ROADMAP.md`, `AGENTS.md` | Adopt | `ROADMAP.md` status table + [plan](../../plan/roadmap.md); the PR template asks for the update |
| 13 | One task id per commit (`M2.1: pen tool`) | `AGENTS.md` | Adopt | One roadmap step per PR, id in the title |
| 14 | PR template checklist | `.github/pull_request_template.md` | Adopt and automate | Most checklist items are CI checks; the template keeps the human ones |
| 15 | Rich doc comments on fields and modules | e.g. `crates/doc/src/node.rs` | Adopt and enforce | `missing_docs` denied in library crates |
| 16 | Dependencies explained in `Cargo.toml` comments | e.g. the `wasmi` features comment | Adopt | Same, plus `cargo-deny` |
| 17 | Headless UI tests that assert on what a frame shows | `crates/ui-egui/src/panels/transparency.rs` tests | Adopt (phase 3 panels) | Same style for any panel we contribute |
| 18 | Very large modules | `menus.rs` 3,752 lines, `canvas.rs` 2,941 lines | Do not adopt | Files warn at 800 and fail at 1,500 lines |
| 19 | Hand-taken screenshots in docs | `ASSETS.md` lists them as screenshots of the app | Do not adopt | Declared, regenerated and compared in CI ([docs pipeline](../docs-pipeline.md)) |
| 20 | Workspace dependency declared but unused (`egui_kittest`) | `Cargo.toml:74`; only named in `xtask/src/layers.rs` | Avoid | `cargo-machete` (or an xtask equivalent) flags unused declarations |
| 21 | `unsafe_code = "deny"`, example plug-in outside the workspace uses `unsafe` for the ABI | `Cargo.toml`, `crates/plugins/example` | Adopt and tighten | `forbid` everywhere; the plug-in's ABI shim is the single audited exception, checked by `cargo xtask unsafe-audit` |

## Decision

Follow VectorCraft's conventions by default (rows marked Adopt), with the improvements listed. When
VectorCraft changes a convention we follow, we re-evaluate here. When we improve on one (rows 5, 6, 7,
19, 20), we offer the improvement upstream where it is generic.

## Consequences

- VectorCraft contributors recognize the shape of the code, the gate and the rules.
- Upstreaming (RFC proposals, phase 3) meets their review standards from the start.
- We take on a little more CI (GitHub Actions on every PR), which external contributions need.
