# stitchcraft-engine

Layer **L2**. Turns a `Design` (host-independent elements with shapes, parameters and threads) into a
checked `StitchPlan`: normalize → validate → generate per element → assemble → finalize → check.

**Status:** the input model, `design::Design`, since M3.3 (`docs/src/design/data-model.md`); the
generators and the pipeline from M3.4. Design: `docs/src/design/engine-pipeline.md` and
`docs/src/design/algorithms/`.

## Invariants

- Knows no host, file format or renderer; never does I/O.
- Generators are pure functions of (shape, typed params, hints, seed, budget); output is deterministic.
- Never panics; every loop over data charges the budget; every fallback emits a coded diagnostic.
- Elements are generated independently from geometry-based hints, so results can be cached and planned in
  parallel without changing the output.
