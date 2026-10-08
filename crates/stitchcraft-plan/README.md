# stitchcraft-plan

Layer **L1**. The stitch plan — the engine's output and the formats' input — plus machine profiles,
thread palettes and the plan invariant checker that conformance level L0 runs on every plan.

**Status:** planned for M1.2–M1.3 and M1.6. Design: `docs/src/design/data-model.md`.

## Invariants

- A plan satisfies `REQ-PLAN-001` … `REQ-PLAN-007` (finite, inside the hoop, stitch length limits, block
  structure, locks around trims, colour limits, provenance) or it is never written to a file.
- Profiles are data; every value is backed by a machine-testing record.
- Generates no stitches and reads or writes no files.
