# stitchcraft-engine

Layer **L2**. Turns a `Design` (host-independent elements with shapes, parameters and threads) into a
checked `StitchPlan`: normalize → validate → generate per element → assemble → finalize → check.

**Status:** the input model, `design::Design`, since M3.3 (`docs/src/design/data-model.md`); stroke
normalization (`normalize::stroke`) and the running stitch (`generators::running`) since M3.4, with its
repeats, bean stitch (`generators::passes`) and random length since M3.5; manual stitch
(`generators::manual`) since M3.6. Lock stitches (`locks`) of each shape, at either end of a group, are
here since M3.7. Since M3.8 the entry point, `plan`, sends each element to its generator (`generate`) and
joins the groups into one plan (`assemble`). Since M3.9 it fits the plan to the machine and checks it
(`finalize`), and a plan that `plan` returns can be written as it is. Since M4.1 a satin column's path
is read as its rails and rungs (`normalize::satin`), as Ink/Stitch reads it. Since M4.2 its top stitches
are placed as Ink/Stitch places them (`generators::satin`), since M4.3 compensated and varied at random
as Ink/Stitch does, since M4.4 inset where they crowd on curves, and since M4.5 split where they are
long. Since M4.6 its underlays come first, as Ink/Stitch sews them, and since M4.7 it starts and ends
at its nearest points. Since M4.8 a column drawn as one path and too narrow to stitch across is sewn as a
stroke, and since M4.9 a wider one is sewn between rails made from its centre line
(`normalize::centre_line`), offset as shapely offsets them (`normalize::offset`). Design:
`docs/src/design/engine-pipeline.md` and
`docs/src/design/algorithms/`.

## Invariants

- Knows no host, file format or renderer; never does I/O.
- Generators are pure functions of (shape, typed params, hints, seed, budget); output is deterministic.
- Never panics; every loop over data charges the budget; every fallback emits a coded diagnostic.
- Elements are generated independently from geometry-based hints, so results can be cached and planned in
  parallel without changing the output.

## Dependencies

`stitchcraft-core`, `stitchcraft-params`, `stitchcraft-plan`; `thiserror`. Tests also use `proptest` and
`stitchcraft-testkit`. Curves are flattened here, not with `kurbo`, so every platform gets the same
points (`docs/src/design/engine-pipeline.md` › Normalize).
