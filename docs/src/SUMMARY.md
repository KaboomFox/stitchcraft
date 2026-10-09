# Summary

[Introduction](README.md)

# User guide

- [User guide](user/README.md)
  - [Your first sew-out on a Brother](user/tutorials/first-sew-out.md)
  - [How-to guides](user/how-to/README.md)
  - [Embroidery basics](user/explanation/embroidery-basics.md)
  - [Reference](user/reference/README.md)
    - [Command line](user/reference/cli.md)
    - [Machine profiles](user/reference/profiles.md)
    - [File formats](user/reference/formats.md)
    - [Test sheets](user/reference/test-sheets.md)
    - [Diagnostic codes](user/reference/diagnostics.md)
    - [Parameters](user/reference/params.md)
      - [Common parameters](user/reference/params/common.md)
    - [Glossary](user/reference/glossary.md)
    - [VectorCraft compatibility](user/reference/compatibility.md)

# Design

- [Design documents](design/README.md)
  - [Technical design document](design/tdd.md)
  - [Architecture](design/architecture.md)
  - [Data model](design/data-model.md)
  - [Parameter registry](design/params.md)
  - [Diagnostics](design/diagnostics.md)
  - [Engine pipeline](design/engine-pipeline.md)
  - [Stitch generators](design/algorithms/README.md)
    - [Strokes](design/algorithms/strokes.md)
    - [Satin](design/algorithms/satin.md)
    - [Fills](design/algorithms/fills.md)
  - [Machine formats](design/formats.md)
  - [Rendering previews](design/rendering.md)
  - [VectorCraft integration](design/vectorcraft-integration.md)
  - [RFC: VectorCraft plug-in ABI v2](design/rfc-vectorcraft-abi-v2.md)
  - [VectorCraft compatibility gate](design/compatibility-gate.md)
  - [Conformance testing](design/conformance.md)
  - [Determinism](design/determinism.md)
  - [Guardrails](design/guardrails.md)
  - [Documentation pipeline](design/docs-pipeline.md)
  - [Ink/Stitch compatibility contract](design/inkstitch-compat-contract.md)
  - [Decision records](design/adr/README.md)
    - [0001 Licence and clean room](design/adr/0001-license-and-clean-room.md)
    - [0002 Host-agnostic engine, plug-in first](design/adr/0002-host-agnostic-engine-plugin-first.md)
    - [0003 Parameter registry](design/adr/0003-parameter-registry.md)
    - [0004 Determinism](design/adr/0004-determinism.md)
    - [0005 Geometry stack](design/adr/0005-geometry-stack.md)
    - [0006 Docs: mdBook and Diátaxis](design/adr/0006-docs-mdbook-diataxis.md)
    - [0007 PES first, Brother profile](design/adr/0007-pes-first-brother-profile.md)
    - [0008 Conformance first](design/adr/0008-conformance-first.md)
    - [0009 VectorCraft conventions](design/adr/0009-adopt-vectorcraft-conventions.md)
    - [0010 Coded diagnostics](design/adr/0010-diagnostics-with-codes.md)
    - [0011 Movable into VectorCraft](design/adr/0011-movable-into-vectorcraft.md)
    - [0012 Read, never copy](design/adr/0012-read-dont-copy.md)

# Plan

- [Roadmap](plan/roadmap.md)
- [Machine testing](plan/machine-testing.md)

# Contributing

- [Contributing](contributing/README.md)
  - [New stitch type](contributing/playbook-new-stitch-type.md)
  - [New parameter](contributing/playbook-new-param.md)
  - [New format](contributing/playbook-new-format.md)
  - [New diagnostic](contributing/playbook-new-diagnostic.md)
  - [Review checklist](contributing/review-checklist.md)
