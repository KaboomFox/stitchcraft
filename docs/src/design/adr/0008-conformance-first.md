# ADR-0008: Conformance-first development

**Status:** Accepted · 2026-10-08

## Context

The main complaint about Ink/Stitch's quality is testing: 52 test functions for 45k lines; issue #245
"automated testing" open since 2018; 62 error-related commits since 2024, none of which touched the tests.

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
