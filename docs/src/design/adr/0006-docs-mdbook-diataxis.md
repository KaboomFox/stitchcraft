# ADR-0006: mdBook, Diátaxis, generated reference, CI-regenerated images

**Status:** Accepted · 2026-10-08

## Context

The docs must beat Ink/Stitch's and stay correct on every PR, including screenshots.

## Decision

- **mdBook** (Rust-native, fast, search and print built in, no Ruby/Node toolchain).
- **Diátaxis** structure: tutorials, how-to, reference, explanation; plus design, plan, contributing.
- **Generated reference** from registries, committed and checked for staleness.
- **Declared images** (`docs/shots.toml`) of four kinds — stitch renders, VectorCraft renders,
  VectorCraft UI screenshots, photos — regenerated and compared in CI, refreshed by a PR label.
- **Versioned publishing** (`/dev/`, `/vX.Y/`, `/latest/`).

Details: [docs pipeline](../docs-pipeline.md).

## Consequences

- Docs changes are reviewable like code; drift fails the build.
- UI screenshots depend on spike M0.8 (headless VectorCraft window in CI).

## Alternatives considered

- **Jekyll on GitHub Pages** (Ink/Stitch's choice): no checks, Ruby toolchain. Rejected.
- **Docusaurus / MkDocs:** capable, but bring Node or Python into a Rust-only repository.
