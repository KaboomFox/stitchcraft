# ADR-0001: MIT OR Apache-2.0, clean-room from Ink/Stitch

**Status:** Accepted · 2026-10-08 · decision 2 replaced by [ADR-0012](0012-read-dont-copy.md)

## Context

Ink/Stitch, the behaviour reference, is GPL-3.0. VectorCraft, the host, is MIT OR Apache-2.0 and its
`AGENTS.md` forbids GPL/AGPL code in its tree. A port of Ink/Stitch's Python to Rust would be a
derivative work and would have to be GPL-3.0: it could still run as a separately distributed plug-in,
but it could never be contributed to VectorCraft or embedded in it, and every ArtCraft-family project
around it is permissively licensed.

## Decision

1. StitchCraft is licensed **MIT OR Apache-2.0**, like VectorCraft.
2. It is a **clean-room** implementation. The design documents in `docs/src/design/` describe behaviour
   — inputs, outputs, parameters, published algorithms — in our own words. Implementers work from these
   documents, public embroidery knowledge, published papers and Ink/Stitch's public user documentation,
   and **never open, copy or transliterate Ink/Stitch source code**. *Replaced by
   [ADR-0012](0012-read-dont-copy.md): the source may be read, never copied.*
3. **Interoperability is preserved:** parameter names, their meanings and defaults, method identifiers
   and command names match Ink/Stitch's SVG attributes, because reading and writing the same files is the
   point ([compatibility contract](../inkstitch-compat-contract.md)).
4. Not copied, ever: Ink/Stitch code, docs text, parameter descriptions, lock-stitch shape data, meander
   tiles, fonts, palettes compiled by Ink/Stitch, images.
5. Format facts may be taken from MIT-licensed pyembroidery/pystitch with attribution in `NOTICE`;
   prefer public specifications where they exist.
6. Running Ink/Stitch as a black-box oracle for metric comparisons (conformance L3) is allowed: it
   uses the program, it does not copy it.

## Consequences

- Upstreaming to VectorCraft and embedding the engine anywhere stays possible.
- We cannot take shortcuts by translating Ink/Stitch's algorithms; we design them, which the quality goals
  require anyway.
- Our documents describe Ink/Stitch only by its public behaviour, documentation and file format, never
  by its code. *(ADR-0012: they may describe behaviour its source shows, in our own words.)*
- `cargo xtask cleanroom` fails on GPL licence text and on Ink/Stitch source paths anywhere in the
  repository, documents included; the review checklist asks about provenance. *(ADR-0012: source paths
  are allowed, so documents can link what they read.)*

## Alternatives considered

- **GPL-3.0 port:** fastest path to parity and the most faithful behaviour, but closes the door on
  VectorCraft and contradicts the ecosystem's licensing. Rejected.
- **Contribute Rust to Ink/Stitch:** different host, different language, different goals. Out of scope.
