# ADR-0008: Conformance-first development

**Status:** Accepted · 2026-10-08

## Context

When behaviour is specified only by its implementation, nobody can say what "correct" means: a fix for
one input silently breaks another, and a regression looks like any other change. Stitch generators are
especially prone to this, because their output is thousands of coordinates no reviewer reads.

## Decision

Behaviour is specified as numbered requirements (`conformance/requirements.toml`) and proven by data-
driven cases (`conformance/cases/`) across five levels (invariants, formats, generator properties,
Ink/Stitch differential metrics, physical sew-outs). For each roadmap step, requirements and cases come
first, then the code that makes them pass. Gates per PR, nightly, milestone and release. Details:
[conformance](../conformance.md).

## Consequences

- "Done" is objective: requirements active and green, machine checkpoint signed off.
- Golden changes are explicit (`--bless`, label, changelog line).
- Up-front cost per feature; repaid by fewer regressions and reproducible bugs.
