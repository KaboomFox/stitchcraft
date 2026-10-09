# ADR-0010: Coded diagnostics with explanation pages

**Status:** Accepted · 2026-10-08

## Context

Users need to understand why a design will not sew well, and maintainers need to know which message a
user saw. A message without a code cannot be looked up, searched for in bug reports or tested on its
own, and an error path that no test ever renders can ship a broken message.

## Decision

Every user-facing problem is a `Diagnostic` with a stable code (`SC-E0201`), severity, element, location,
message and optional fix. A single registry holds codes and long explanations; docs pages and
`stitch explain` are generated from it; every code must have a conformance case that triggers it.
Details: [diagnostics](../diagnostics.md).

## Consequences

- Error paths are executed in CI; messages cannot rot unnoticed.
- Issue reports can cite a code; search engines find the explanation page.
