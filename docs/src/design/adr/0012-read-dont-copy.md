# ADR-0012: Read Ink/Stitch's source, never copy it

**Status:** Accepted · 2026-10-08

## Context

[ADR-0001](0001-license-and-clean-room.md) made StitchCraft a strict clean room: nobody working on it
opened Ink/Stitch's source. What that protects is the MIT OR Apache-2.0 licence: a port of GPL-3.0 code
would be a derivative work, and VectorCraft, where StitchCraft may move
([ADR-0011](0011-movable-into-vectorcraft.md)), takes no GPL code. But copyright covers code, not
behaviour or algorithms: reading a program does not make a work derivative, copying it does.
VectorCraft draws the line in the same place. Its `AGENTS.md` (at `dcc0de7`) says "Never copy GPL/AGPL
code (Inkscape, lib2geom…)", and keeps its never-read rule for Adobe software, whose terms forbid
reverse engineering.

The strict rule has a cost. Ink/Stitch's documentation is silent on many details that decide whether a
file behaves the same in both tools (edge cases of parameters, how lists repeat, the font files), and
guessing them makes interoperability bugs.

## Decision

1. Ink/Stitch's source, like any GPL/AGPL code, may be read to understand its behaviour. Nothing from
   it is copied, pasted, transliterated or closely paraphrased into StitchCraft, and no GPL code, text or
   data file enters the repository.
2. Behaviour is written into our design docs in our own words, and code is written from those docs, as
   before. A design statement may link the Ink/Stitch source it rests on when that helps the next reader
   check it again; no link is required.
3. This replaces decision 2 of ADR-0001. Its other decisions stand: the licence, interoperability through
   Ink/Stitch's names and defaults, the things never copied, format facts from pyembroidery with
   attribution, and Ink/Stitch as a black-box oracle.

## Consequences

- Interoperability details can be checked against what Ink/Stitch actually does instead of inferred from
  its documentation.
- The risk moves from "never seen" to "never copied": code written right after reading another
  implementation tends to mirror its structure even when no line is copied. Writing the design doc first,
  in our own words, and implementing from it is the guard; the review checklist asks that nothing was
  copied.
- `cargo xtask cleanroom` still fails on GPL licence text and on pasted Python source. It no longer fails
  on Ink/Stitch file paths, so a design doc can link the source it read.

## Alternatives considered

- **Keep the strict clean room:** the cleanest provenance, but it leaves interoperability details to
  guesswork, and it is stricter than VectorCraft, the host whose licence sets the constraint. Rejected.
- **Require a pinned citation for everything learned from the source**, checked by the gate: traceable,
  but more process than VectorCraft asks for, and a link only helps where a statement may need checking
  again. Rejected: links are welcome, not required.
