# Design documents

These pages are the specification StitchCraft is built from. They describe behaviour and algorithms in
our own words and never carry Ink/Stitch's code ([ADR-0012](adr/0012-read-dont-copy.md)).

## Reading order

1. [Technical design document](tdd.md) — the whole system in 25 minutes.
2. [Architecture](architecture.md) — crates, layers, where code goes.
3. [Data model](data-model.md), [parameter registry](params.md), [diagnostics](diagnostics.md).
4. [Engine pipeline](engine-pipeline.md) and the [stitch generators](algorithms/README.md).
5. [Machine formats](formats.md) and [rendering previews](rendering.md).
6. [VectorCraft integration](vectorcraft-integration.md), the [ABI v2 RFC](rfc-vectorcraft-abi-v2.md)
   and the [compatibility gate](compatibility-gate.md).
7. Quality: [conformance](conformance.md), [determinism](determinism.md), [guardrails](guardrails.md),
   [docs pipeline](docs-pipeline.md).
8. File compatibility: the [Ink/Stitch compatibility contract](inkstitch-compat-contract.md).
9. Decisions: [ADRs](adr/README.md).

## Keeping these pages true

- A design page changes in the same PR as the behaviour it describes.
- Pages cite requirement ids (`REQ-…`) and diagnostic codes (`SC-…`); the docs check fails on unknown ids.
- Facts about Ink/Stitch and VectorCraft name the commit they were checked against.
