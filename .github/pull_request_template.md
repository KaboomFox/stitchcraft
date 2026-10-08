## Roadmap step

<!-- e.g. M3.4 — and put the id at the start of the PR title. -->

## What and why

## Requirements and cases

<!-- REQ-… ids touched; conformance cases added. Requirements switched to `active`? -->

## Checklist

CI checks the mechanical rules (format, lints, no panics, no `unsafe`, determinism, layering, file size,
clean room, docs freshness, links, ids). Please confirm the rest:

- [ ] Requirements and failing cases came first; they pass now.
- [ ] No Ink/Stitch source code was consulted (clean room); algorithms cite public sources.
- [ ] User-facing docs updated in this PR; registry help text reads well for an embroiderer.
- [ ] Golden files changed? Then the `golden-change` label is set and `CHANGELOG.md` says why.
- [ ] Pictures should change? Shots refreshed (`docs:refresh-shots` label or `cargo xtask shots`).
- [ ] `ROADMAP.md` status updated if this completes a step.
- [ ] Affects what sews? Listed for the next machine checkpoint.
