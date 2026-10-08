# ADR-0003: One parameter registry generates everything

**Status:** Accepted · 2026-10-08

## Context

Parameters appear in at least seven places: generators, validation, VectorCraft manifests, the CLI,
SVG attributes, documentation and property tests. Ink/Stitch declares them in code and documents them
again by hand; six code parameters have no docs entry ([F6](../inkstitch-analysis.md#f6--parameters-described-twice-by-hand)).

## Decision

Declare each parameter once, next to its generator, with a `params!` macro (doc comment = help text).
Generate typed structs, validation, manifests, CLI help, SVG mapping, docs pages, JSON Schema and
`proptest` strategies from it. Registry keys are Ink/Stitch attribute names where the meaning matches,
and defaults equal Ink/Stitch's. Details: [parameter registry](../params.md).

## Consequences

- Adding a parameter is a one-place change; docs and UIs cannot forget it.
- A `macro_rules!` macro keeps contributors in plain Rust; if it grows unwieldy (reassessed after M3),
  a derive macro can replace it without changing the registry's data model.
- VectorCraft v1's 64-parameter limit is checked by a test at PR time.

## Alternatives considered

- **Serde structs + hand-written docs:** the drift we are trying to avoid.
- **External schema file (YAML) generating Rust:** another language and a build step; doc comments
  next to code are where contributors look.
