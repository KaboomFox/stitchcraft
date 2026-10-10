# Engine pipeline

<!-- implements: crates/stitchcraft-engine/src/normalize/mod.rs, crates/stitchcraft-engine/src/generate.rs, crates/stitchcraft-engine/src/assemble.rs, crates/stitchcraft-engine/src/finalize.rs, crates/stitchcraft-engine/src/pipeline.rs, apps/stitchcraft-cli/src/commands/plan.rs -->

From a `Design` to a checked `StitchPlan`. Each stage is a module in `stitchcraft-engine` with its
own tests; stages communicate only through the types in the [data model](data-model.md).

```text
Design ──▶ 1 normalize ──▶ 2 validate ──▶ 3 generate (per element) ──▶ 4 assemble ──▶ 5 finalize ──▶ 6 check ──▶ StitchPlan
              │               │                 │                         │              │              │
              └───────────────┴───── diagnostics accumulate at every stage ┴──────────────┴──────────────┘
```

## 1. Normalize

Per element, independent of the others:

- **Flatten curves** with the element's tolerance (a tenth of a CSS pixel for fills, as Ink/Stitch
  flattens them, and a tenth of `running_stitch_tolerance_mm` for strokes). Strokes are halved (de Casteljau) until each piece's
  control points lie within the tolerance of its chord (`stitchcraft_engine::normalize::stroke`), using
  only arithmetic and square roots so that every platform gets the same points. `kurbo`'s adaptive
  flattening is not used for this: in 0.13 it calls `powf` (`CubicBez::to_quads`) and `hypot`
  (`QuadBez::estimate_subdiv`) from the platform's maths library, whose last bits differ between
  systems ([determinism](determinism.md)). Corners, joins between segments that turn by more than 30°,
  are marked so running stitches land exactly on them; a curve's own bend is never a corner.
- **Regions:** a fill's subpaths are cut wherever they meet, and its fill rule says which faces between
  them are filled. The edges between filled and empty faces become outlines and holes, each turned one
  way, and parts too small to sew are left out with `SC-W0303`
  ([fills › Region](algorithms/fills.md#region), `stitchcraft_engine::normalize::region`).
- **Satins:** classify subpaths into rails and rungs ([satin](algorithms/satin.md#recognizing-rails-and-rungs)), or make
  them from a column's centre line ([single-path satin](algorithms/satin.md#single-path-satin)).
- **Strokes:** each subpath becomes its own path piece, in document order.
- **Commands** become hints (start/end/target points) on the element.

Adapters have already applied transforms and converted units, so normalization never sees host types.

## 2. Validate

- Parameters are checked by the registry: type, range, applicability, visibility conditions; the
  generator receives a typed struct or nothing.
- Shapes are checked against the stitch type. A satin needs a subpath longer than a point (`SC-E0201`),
  a fill needs a non-empty region, and a running stitch needs a path longer than the minimum stitch
  length.
- An element with an error diagnostic is skipped; the rest of the design still plans. A design whose
  every element was skipped yields `SC-E0010`.

## 3. Generate

Each element becomes stitch groups (`stitchcraft_engine::generate`): the needle points of each part of it
that a jump may separate from the next, in sewing order. A stroke gives one group per subpath, and a part
too small for a stitch gives none (`SC-W0401`). The element's own settings stay with the element, and
assembly reads them there: its thread, locks, trim and stop.

The stitch type picks the generator. A stroke whose `satin_column` setting is on is a satin column,
whatever its `stroke_method` says, as in Ink/Stitch. Its rails and rungs are recognized, what
recognition finds is reported, and its `satin_method` picks the generator: the satin stitch, sewn from
M4.2. A satin column also reads the running stitch's length (`running_stitch_length_mm`): the stitches
that join its underlays are no longer than the first value. For any other stroke the generator is its
`stroke_method`. `running_stitch` (the default) and
`manual_stitch` are sewn from M3. A satin column drawn as one path no wider than the design's
`min_satin_stroke_width` is sewn as a stroke, with `SC-W0212`, as in Ink/Stitch (`REQ-SAT-015`). A wider
one is sewn between rails made from its line from M4.9 (`REQ-SAT-016`). The other stroke and satin
methods, and fills, are skipped with `SC-W0011` until their milestones. A fill's region is built
already, so what it leaves out and how it will be sewn are said (`SC-W0303`, `SC-W0304`, `SC-I0306`,
`SC-W0307`). An element whose parameters are wrong (`SC-E0101`) is skipped too, and the rest of the
design still plans.

Each element is generated with the shortest stitch for it: the larger of the machine's (the profile's
`min_stitch`) and the element's `min_stitch_length_mm`, or the design's shortest stitch when the element
sets none. A satin column is given its longest stitch as well, the element's `max_stitch_length_mm`,
which its split stitches keep to.

Rules every generator follows:

- **Pure:** output depends only on (normalized shape, typed params, neighbours, seed, budget).
- **The needle before, the next element after.** Elements are generated in sewing order
  ([ADR 0014](adr/0014-generators-see-their-neighbours.md), `REQ-GEN-003`). Each is given the last
  needle point of the elements before it, whatever their thread, and what the next element offers to
  end near: its first point, or its shape when it starts at its own nearest point. The offer comes from
  the next element's shape and settings and the design's, never its stitches, so nothing waits for a
  later element. A
  satin column starts and ends by them ([satin](algorithms/satin.md#start-and-end)), and explicit
  start and end commands will override them (`REQ-GEN-001`).
- **Seeded randomness:** each element's generator is seeded from a hash of its id mixed with its
  `random_seed`, one of the settings every stitch type shares. The PRNG is SplitMix64 from
  `stitchcraft-core` ([determinism](determinism.md)).
- **Budgeted:** every inner loop charges work units. Each element has the budget's work to itself. One
  that runs out (`SC-E0004`) is skipped, and the others still plan. The stitch limit is for the whole
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
- A thread change starts a new colour block (`REQ-ASM-001`). Threads are compared by colour, as
  Ink/Stitch compares them: two names for one colour are one thread, and the block keeps the first.

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

An element that sews nothing, because it is skipped or too small for a stitch, has no place for its trim
or stop. Both are left out, as Ink/Stitch leaves them out, and `SC-W0505` says so.

A jump lands where sewing resumes, on the tie-in's first point or on the group's first point when it has
no tie-in, and the needle goes down there before the first stitch. The jump's own thread is the
machine's business: StitchCraft trims where an element's `trim_after` is set, as Ink/Stitch does.
Whether the reference machine also needs a trim on long jumps is test sheet TS-02's question at MC-1. Its
answer becomes a profile value.

Ink/Stitch (read at `d59c9ab`) joins groups the same way: one stitch from one group to the next within
the collapse length, locks and a jump beyond it. It sews that stitch as it is; StitchCraft's finalize
splits it if it is longer than the machine's longest stitch, which only a `min_jump_stitch_length_mm`
beyond that length can cause.

### Lock stitches (ties)

- A tie-in goes only at the start of a group that the needle jumps to, the design's first included. A
  tie-off goes only at the end of a group that a jump, trim, stop or thread change follows, or that ends
  the design. Groups sewn on from one to the next are sewn without locks between them (`REQ-LCK-001`).
- `ties` says which of those an element's groups get: both, the tie-in (before), the tie-off (after) or
  neither. `force_lock_stitches` adds the tie-off whatever `ties` says, never a tie-in. It also makes each
  group of the element end with a jump, and the tie-off is sewn there. That is Ink/Stitch's rule (read at
  `d59c9ab`).
- Manual stitch is sewn without locks unless `force_lock_stitches` is set, because its points are placed
  by hand.
- A group with fewer than two needle points is sewn without locks: it has no stitch to lock.
- [Lock stitches](algorithms/locks.md) specifies the lock itself: its shape (`lock_start`, `lock_end`),
  its size and its shortest stitch (0.2 mm).

### Origin and stop position

The plan is in hoop coordinates: the design's origin goes to the hoop's centre, (0, 0), where the needle
starts. The origin is the design's own (Ink/Stitch's origin command, read from M8) or else the centre of
the box around every point where the needle goes down (`REQ-ASM-005`). A design's stop position
(Ink/Stitch's stop position command) adds a jump to it before each `Stop`. At the stop position the frame
is clear of the needle for an appliqué or a check, and sewing resumes with a jump back.

### Commands

| Command | Effect | From |
|---|---|---|
| Start / end point | Entry/exit hints for the generator (`REQ-GEN-001`) | M8 |
| Target point | Centre for circular fills and ripple targets | M7, M10 |
| Trim after, stop after | `trim_after` and `stop_after` on the element's last group (see the connection table). The SVG adapter reads Ink/Stitch's trim and stop commands as these settings. | M3.8 (engine), M3 (adapter) |
| Ignore object / ignore layer | The adapter drops the element and says so in the report (`REQ-ASM-004`) | M3 |
| Origin | The design setting `origin` | M3.8 (engine), M8 (adapter) |
| Stop position | The design setting `stop_position` | M3.8 (engine), M8 (adapter) |

## 5. Finalize

Finalize fits the plan to the machine profile (`stitchcraft_engine::finalize`), and a plan that `plan`
returns can be written as it is. Generators already sew within the machine's limits. What is left to fit
comes from joining groups and from settings the machine cannot follow.

1. **The shortest stitch** of each needle point is its element's: the one its generator used, so finalize
   never thins what the generator spaced. That is the element's `min_stitch_length_mm`, else the design's
   `min_stitch_len`, and never shorter than the machine's (`profile.min_stitch`). Where one element's
   stitching runs straight on into the next, the stitch between them can be anything up to the collapse
   length, 0 included. Within each run of stitches, from where the needle lands to the next jump, trim or
   stop, a needle point less than its shortest stitch from the one before is left out. The stitch then
   runs on to the next point. The run's first and last points and lock points always stay, and the points
   before them are left out instead. A point where the needle already is would sew in place, and it is
   left out too. A stitch into or out of a lock point is a lock stitch, whose shortest is 0.2 mm
   (`REQ-PLAN-002`). `SC-I0504` says how many points were left out (`REQ-FIN-001`).
2. **The longest stitch.** A stitch longer than `profile.max_stitch` is split into the fewest equal parts
   no longer than it (`SC-I0703`, `REQ-FIN-001`). Such a stitch comes from a stitch placed by hand, from
   a custom lock's long step, or from a move sewn on under a `min_jump_stitch_length_mm` longer than the
   machine's longest stitch. Each part counts against the design's stitch budget.
3. **Colour limits:** colour changes and stops above what the profile's format records → `SC-E0601`.
4. **Hoop:** a design larger than the hoop → `SC-E0701`, with a rotate-to-fit fix when rotating 90° would
   fit. When a design's origin is far from its middle, a design smaller than the hoop can cross the
   hoop's edge. That is `SC-E0701` as well. So is a stop position past the edge, which the message names. The size is
   the stitches' own. For a design that cannot be sewn, `SC-E0701` is the only message about its size.
   Otherwise, larger than the comfort zone → `SC-W0702` (`REQ-FIN-002`).
5. The end is the plan's structure: the machine ends after the last block.

After an error at any step, `plan` returns no plan, and the error's diagnostic says why.

Splitting jumps longer than a format can encode in one record is *not* done here: it is the encoder's
job, because the limit is a property of the file format, not of the machine.

Ink/Stitch (read at `d59c9ab`) removes short stitches once, over the whole plan: an entry no farther than
its shortest stitch (the element's `min_stitch_length_mm`, else a global 0.1 mm) from the last one kept
is dropped. It keeps lock stitches, the stitch right after a jump, and stop, trim and colour-change
entries. A jump itself can be dropped. Nothing is split. StitchCraft uses the same shortest stitch for
each element as Ink/Stitch, but never one below the machine's (0.3 mm on the Brother). It keeps the ends
of each run and splits stitches the machine cannot sew: a deviation (`DEV-FIN-001`), because a file can
sew differently in the two tools.

## 6. Check

The plan invariant checker (`stitchcraft-plan::invariants`, the same code conformance level L0 runs)
validates the result. A violation is a bug in StitchCraft: `plan` returns no plan and reports
`SC-E0009 internal check failed` (`REQ-FIN-003`). The command line then does not write a machine file.
It writes a bug-report bundle that reproduces the failure (`REQ-CLI-002`). The conformance suite has a case for every invariant.

## 7. Incremental and parallel planning

- **Cache:** a group is keyed by (element shape hash, params hash, neighbours, seed, engine version). The
  VectorCraft live effects already cache per object; the CLI and future panels reuse the same key.
- **Parallelism (native only):** elements are generated in sewing order, each seeing its neighbours
  ([ADR 0014](adr/0014-generators-see-their-neighbours.md)). Reading parameters and recognizing shapes
  depend on the element alone. That work may run in parallel with `rayon` behind the CLI's `parallel`
  feature. Its results are joined in document order, and the output is then identical to a sequential run. The wasm
  plug-in stays single-threaded.

## 8. Outputs

| Output | Produced by |
|---|---|
| Machine file | `stitchcraft-formats` writer chosen by the profile or the output extension |
| Preview PNG | `stitchcraft-render`, from the *quantized* plan (`REQ-RND-001`) |
| Report JSON | CLI: stitch, jump, trim and colour counts, bounds, estimated sewing time, diagnostics |
| Plug-in preview geometry | `stitchcraft-vc-plugin` from a single group ([integration](vectorcraft-integration.md#live-effect-previews-within-v1-budgets)) |
