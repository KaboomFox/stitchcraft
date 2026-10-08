# stitchcraft-plan

Layer **L1**. The stitch plan — the engine's output and the formats' input — plus machine profiles,
thread palettes and the plan invariant checker that conformance level L0 runs on every plan.

Design: `docs/src/design/data-model.md`.

| Module | Purpose |
|---|---|
| `plan` | `StitchPlan` (colour blocks of `Stitch` entries: `Normal`, `Jump`, `Trim`, `Stop`), `Provenance`, `PlanStats` |
| `builder` | `PlanBuilder`: appends entries with the needle tracked, so commands always happen where the needle is |
| `invariants` | `check(plan, profile)`: the L0 rules `REQ-PLAN-001/002/003/005/006`, each violation naming its requirement |
| `profile` | `MachineProfile`, `FormatId`, `TrimSupport`, `PaletteId`; profile validation and the hoop/comfort check (`SC-E0701`, `SC-W0702`) |
| `profiles` | The built-in profiles as data (`brother-200x200`) |
| `thread`, `palette` | `Rgb`, `Thread`; the Brother PEC palette; CIEDE2000 nearest-colour matching |

## Invariants

- Colour changes and the end are implied by the block structure, so they cannot be misplaced; encoders
  write exactly one colour change between blocks and exactly one end.
- A plan is checked (`invariants::check`) before it is written; a violation is a bug and is reported as
  `SC-E0009`, never written to a file.
- Profiles are data; every value names its evidence and changes only with a sew-out report.
- Colour matching is deterministic: CIEDE2000 through `stitchcraft_core::math`, ties to the lower index,
  tested against the Sharma (2005) reference pairs.
- Generates no stitches and reads or writes no files.

## Dependencies

`stitchcraft-core`; `thiserror`.
