# Engine pipeline

From a `Design` to a checked `StitchPlan`. Each stage is a module in `stitchcraft-engine` with its
own tests; stages communicate only through the types in the [data model](data-model.md).

```text
Design ──▶ 1 normalize ──▶ 2 validate ──▶ 3 generate (per element) ──▶ 4 assemble ──▶ 5 finalize ──▶ 6 check ──▶ StitchPlan
              │               │                 │                         │              │              │
              └───────────────┴───── diagnostics accumulate at every stage ┴──────────────┴──────────────┘
```

## 1. Normalize

Per element, independent of the others:

- **Flatten curves** with the element's tolerance (default 0.1 mm for fills, a tenth of
  `running_stitch_tolerance_mm` for strokes). Strokes are halved (de Casteljau) until each piece's
  control points lie within the tolerance of its chord (`stitchcraft_engine::normalize::stroke`), using
  only arithmetic and square roots so that every platform gets the same points. `kurbo`'s adaptive
  flattening is not used for this: in 0.13 it calls `powf` (`CubicBez::to_quads`) and `hypot`
  (`QuadBez::estimate_subdiv`) from the platform's maths library, whose last bits differ between
  systems ([determinism](determinism.md)). Corners, joins between segments that turn by more than 30°,
  are marked so running stitches land exactly on them; a curve's own bend is never a corner.
- **Regions:** resolve the fill rule into valid polygons with holes (`i_overlay`), snap to a 1 µm grid
  first so near-coincident vertices become coincident, drop rings under the minimum area (`SC-W0303`),
  orient rings explicitly.
- **Satins:** classify subpaths into rails and rungs ([satin](algorithms/satin.md#recognizing-rails-and-rungs)).
- **Strokes:** each subpath becomes its own path piece, in document order.
- **Commands** become hints (start/end/target points) on the element.

Adapters have already applied transforms and converted units, so normalization never sees host types.

## 2. Validate

- Parameters are checked by the registry: type, range, applicability, visibility conditions; the
  generator receives a typed struct or nothing.
- Shapes are checked against the stitch type: a satin needs two non-crossing rails; a fill needs a
  non-empty region; a running stitch needs a path longer than the minimum stitch length.
- An element with an error diagnostic is skipped; the rest of the design still plans. A design whose
  every element was skipped yields `SC-E0010`.

## 3. Generate

Each element becomes stitch groups (`stitchcraft_engine::generate`): the needle points of each part of it
that a jump may separate from the next, in sewing order. A stroke gives one group per subpath; a part too
small for a stitch gives none (`SC-W0401`). The element's own settings — its thread, locks, trim and stop —
stay with the element, and assembly reads them there.

The stitch type picks the generator. For a stroke it is `stroke_method`: `running_stitch` (the default)
and `manual_stitch` are sewn from M3; the other stroke methods, satins and fills are skipped with
`SC-W0011` until their milestones. An element whose parameters are wrong (`SC-E0101`) is skipped too, and
the rest of the design still plans.

Each element is generated with the shortest stitch for it: the larger of the machine's (the profile's
`min_stitch`) and the element's `min_stitch_length_mm`, or the design's shortest stitch when the element
sets none.

Rules every generator follows:

- **Pure:** output depends only on (normalized shape, typed params, hints, seed, budget).
- **Entry and exit hints, not neighbours' stitches.** The previous element's *geometry* suggests where
  this one should start (nearest point to its exit hint); the generator never waits for another
  element's stitches. This makes generation independent, cacheable and parallel, so large designs
  stay fast, and honours explicit
  start/end commands (`REQ-GEN-001`).
- **Seeded randomness:** seed = `random_seed` parameter if set, else a hash of the element id; the PRNG
  is SplitMix64 from `stitchcraft-core` ([determinism](determinism.md)).
- **Budgeted:** every inner loop charges work units. Each element has the budget's work to itself, so
  one that runs out (`SC-E0004`) is skipped and the others still plan; the stitch limit is for the whole
  design.
- **Roles:** every stitch is tagged underlay, top, travel or lock, which previews colour-code and which
  later enables per-layer ordering.

Generators are listed in [algorithms](algorithms/README.md).

## 4. Plan assembly

Groups are joined into colour blocks in document order (host paint order, bottom first), by
`stitchcraft_engine::assemble`.

### Ordering

- Document order is kept: embroidery stacking follows the art's stacking. Reordering for fewer colour
  changes is an explicit, separate tool (P3) because it changes what covers what.
- A thread change starts a new colour block (`REQ-ASM-001`).

### Connecting consecutive groups

The needle sews straight on from one group to the next only when nothing separates them. With `d` the
distance from where the needle is to the next group's first needle point:

| Between two groups | Connection |
|---|---|
| Same thread, `d` no more than the earlier element's `min_jump_stitch_length_mm` if it sets one, else the design's collapse length (3 mm), and the earlier element does not force locks | **Sewn on:** the next group's first stitch starts where the needle is; finalize splits it if it is longer than the machine's longest stitch |
| Same thread, otherwise | Tie-off → `Jump` → tie-in (`REQ-ASM-002`) |
| After an element's last group, when it says `trim_after` | Tie-off → `Trim` → `Jump` → tie-in (`REQ-ASM-003`) |
| After an element's last group, when it says `stop_after` | Tie-off → `Jump` to the stop position, if the design has one → `Stop` → `Jump` → tie-in (`REQ-ASM-003`, `REQ-ASM-005`) |
| Thread change | Tie-off → a new colour block → `Jump` → tie-in |
| Before the design's first group | `Jump` → tie-in |
| After the design's last group | Tie-off |

A jump lands where sewing resumes, on the tie-in's first point or on the group's first point when it has
no tie-in, and the needle goes down there before the first stitch. The jump's own thread is the
machine's business: StitchCraft trims where an element asks for it (`trim_after`), as Ink/Stitch does.
Whether the reference machine also needs a trim on long jumps is test sheet TS-02's question at MC-1; its
answer becomes a profile value.

Ink/Stitch (read at `d59c9ab`) joins groups the same way: one stitch from one group to the next within
the collapse length, locks and a jump beyond it. It sews that stitch as it is; StitchCraft's finalize
splits it if it is longer than the machine's longest stitch, which only a `min_jump_stitch_length_mm`
beyond that length can cause.

### Lock stitches (ties)

- A tie-in goes only at the start of a group that the needle jumps to, the design's first included; a
  tie-off only at the end of a group that a jump, trim, stop or thread change follows, or that ends the
  design. Groups sewn on from one to the next get none between them (`REQ-LCK-001`).
- `ties` says which of those an element's groups get: both, the tie-in (before), the tie-off (after) or
  neither. `force_lock_stitches` adds the tie-off whatever `ties` says, never a tie-in, and makes every
  group of the element end with a jump, so the tie-off is sewn. That is Ink/Stitch's rule (read at
  `d59c9ab`).
- Manual stitch gets no locks unless `force_lock_stitches` is set: its points are placed by hand.
- A group with fewer than two needle points gets no locks: there is no stitch to lock.
- The lock itself — its shape (`lock_start`, `lock_end`), its size and the 0.2 mm shortest lock stitch —
  is specified in [Lock stitches](algorithms/locks.md).

### Origin and stop position

The plan is in hoop coordinates: the design's origin goes to the hoop's centre, (0, 0), where the needle
starts. The origin is the design's own (Ink/Stitch's origin command, read from M8) or else the centre of
the box around every point where the needle goes down (`REQ-ASM-005`). A design's stop position
(Ink/Stitch's stop position command) adds a jump to it before each `Stop`, so the frame moves out of the
way for an appliqué or a check, and sewing resumes with a jump back.

### Commands

| Command | Effect | From |
|---|---|---|
| Start / end point | Entry/exit hints for the generator (`REQ-GEN-001`) | M8 |
| Target point | Centre for circular fills and ripple targets | M7, M10 |
| Trim after, stop after | `trim_after` and `stop_after` on the element's last group (see the connection table) | M3.8 |
| Ignore object / ignore layer | The adapter drops the element and says so in the report (`REQ-ASM-004`) | M8 |
| Origin | The design setting `origin` | M3.8 (engine), M8 (adapter) |
| Stop position | The design setting `stop_position` | M3.8 (engine), M8 (adapter) |

## 5. Finalize

Against the machine profile (`stitchcraft_engine::finalize`), so that a plan `plan` returns can be written
as it is. Generators keep their own stitches within the machine's limits; what is left comes from joining
things up and from settings the machine cannot follow.

1. **The shortest stitch** is the machine's (`profile.min_stitch`), or the design's `min_stitch_len` when
   that is longer. Where one element's stitching runs straight on into the next, the stitch between them
   can be anything up to the collapse length, 0 included. Within each run of stitches — from where the
   needle lands to the next jump, trim or stop — a needle point less than the shortest stitch from the one
   before is left out, so the stitch runs on to the next. The run's first and last points and lock points
   always stay, and leave out the points before them instead. A stitch into or out of a lock point is a
   lock stitch, whose shortest is 0.2 mm (`REQ-PLAN-002`). `SC-I0504` says how many points were left out
   (`REQ-FIN-001`).
2. **The longest stitch.** A stitch longer than `profile.max_stitch` is split into the fewest equal parts
   no longer than it (`SC-I0703`): a stitch placed by hand, a custom lock's long step, or a move sewn on
   under a `min_jump_stitch_length_mm` longer than the machine's longest stitch (`REQ-FIN-001`).
3. **Colour limits:** colour changes and stops above what the profile's format records → `SC-E0601`.
4. **Hoop:** a design larger than the hoop → `SC-E0701` (with a rotate-to-fit fix when rotating 90° would
   fit), and so is one that would fit but reaches past the hoop's edge because its origin is far from its
   middle; larger than the comfort zone → `SC-W0702` (`REQ-FIN-002`).
5. The end is the plan's structure: the machine ends after the last block.

An error at any step leaves no plan, and its diagnostic says why.

Splitting jumps longer than a format can encode in one record is *not* done here: it is the encoder's
job, because the limit is a property of the file format, not of the machine.

Ink/Stitch (read at `d59c9ab`) removes short stitches once, over the whole plan: a stitch no longer than
the shortest stitch (the element's `min_stitch_length_mm`, else its global 0.1 mm) from the last one kept
is dropped, except lock stitches and the first stitch after a jump, stop, trim or colour change; nothing
is split. StitchCraft's floor is the machine's (0.3 mm on the Brother), it keeps the ends of each run, and
it splits stitches the machine cannot sew: a deviation (`DEV-FIN-001`), because a file can sew differently
in the two tools.

## 6. Check

The plan invariant checker (`stitchcraft-plan::invariants`, the same code conformance level L0 runs)
validates the result. A violation is a bug in StitchCraft: `plan` returns no plan and reports
`SC-E0009 internal check failed` (`REQ-FIN-003`); the command line writes nothing, and from M3.10 a
bug-report bundle. The conformance suite has a case for every invariant.

## 7. Incremental and parallel planning

- **Cache:** a group is keyed by (element shape hash, params hash, hints, seed, engine version). The
  VectorCraft live effects already cache per object; the CLI and future panels reuse the same key.
- **Parallelism (native only):** generation fans out over elements with `rayon` behind the CLI's
  `parallel` feature; results are joined in document order, so output is identical to a sequential run.
  The wasm plug-in stays single-threaded.

## 8. Outputs

| Output | Produced by |
|---|---|
| Machine file | `stitchcraft-formats` writer chosen by the profile or the output extension |
| Preview PNG | `stitchcraft-render`, from the *quantized* plan (`REQ-RND-001`) |
| Report JSON | CLI: stitch, jump, trim and colour counts, bounds, estimated sewing time, diagnostics |
| Plug-in preview geometry | `stitchcraft-vc-plugin` from a single group ([integration](vectorcraft-integration.md#live-effect-previews-within-v1-budgets)) |
