# Engine pipeline

<!-- implements: crates/stitchcraft-engine/src/normalize/mod.rs, crates/stitchcraft-engine/src/generate.rs, crates/stitchcraft-engine/src/generators/next.rs, crates/stitchcraft-engine/src/assemble.rs, crates/stitchcraft-engine/src/finalize.rs, crates/stitchcraft-engine/src/thin.rs, crates/stitchcraft-engine/src/pipeline.rs, apps/stitchcraft-cli/src/commands/plan.rs -->

From a `Design` to a checked `StitchPlan`. Each stage is a module in `stitchcraft-engine` with its
own tests; stages communicate only through the types in the [data model](data-model.md).

```text
Design ──▶ 0 auto ──▶ 1 normalize ──▶ 2 validate ──▶ 3 generate (per element) ──▶ 4 assemble ──▶ 5 finalize ──▶ 6 check ──▶ StitchPlan
              │            │               │                 │                         │              │              │
              └────────────┴───────────────┴───── diagnostics accumulate at every stage ┴──────────────┴──────────────┘
```

## 0. Automatic digitizing

Since M13.1, before anything else, each element the host marked automatic, a shape without settings read
with `--auto`, becomes the ordinary elements its width calls for, with the settings that say so
(`stitchcraft_engine::auto::digitized`, [image to embroidery](image-to-embroidery.md#stitch-types-by-width)).

- An area is sewn as it is, apart from its narrow parts: each becomes a running stitch along its centre
  line (M13.4), and the rest of the area the fill of its other parts.
- A line that runs is sewn as it is, tripled from 0.5 mm wide.
- A line sewn as a satin column becomes a column for each of its subpaths, or the fill of a subpath a
  column cannot follow (`SC-I0902`).
- A wider line becomes the fill of the area it paints.

Every stage after it sees only ordinary elements. A digitized shape sews, offers its neighbours what it
offers and is said of as the same shape set by hand would be. `SC-I0901` counts the shapes digitized. A
design without automatic elements passes through unchanged and uncopied. Each automatic element has the
budget's work to itself, naming its parts included, and one that needs more is left out with `SC-E0004`
(`REQ-GEN-002`). Digitizing doesn't add an error of its own (`REQ-AUTO-006`). What cannot be sewn as its
width says is sewn otherwise or left out with a warning.

The digitized elements are then sewn as neighbours, in an order of their own. What overlaps keeps its
paint order, and a line sews after the fill it borders. Then each thread sews as long as it can, the
nearest element next ([image to embroidery](image-to-embroidery.md#sewing-order), `SC-I0907`). Elements
with settings keep their order among themselves, unless `--sort-colours` is given. The switch brings a
design without automatic elements through this stage too.

## 1. Normalize

Per element, independent of the others:

- **Flatten curves** with the element's tolerance (a tenth of `running_stitch_tolerance_mm` for strokes,
  and a tenth of a CSS pixel for fills and satin columns, as Ink/Stitch flattens them). Curves are halved
  (de Casteljau) until each piece's control points lie within the tolerance of its chord
  (`stitchcraft_engine::normalize::stroke`), using only arithmetic and square roots so that every
  platform gets the same points. `kurbo`'s adaptive flattening is not used for this: in 0.13 it calls
  `powf` (`CubicBez::to_quads`) and `hypot` (`QuadBez::estimate_subdiv`) from the platform's maths
  library, whose last bits differ between systems ([determinism](determinism.md)). Corners, joins between
  segments that turn by more than 30°, are marked so running stitches land exactly on them; a curve's own
  bend is never a corner.
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

- Parameters are read by the registry as the element's stitch type reads them. Blank text takes the
  default, and a setting the stitch type does not use, or that its other settings hide, is kept unread
  ([parameters](params.md#the-parameters-an-element-reads-req-prm-004)). The rest are checked for type and range,
  and the generator receives a typed struct or nothing.
- Shapes are checked against the stitch type. A satin needs a subpath longer than a point (`SC-E0201`),
  a fill needs a non-empty region, and a running stitch needs a path longer than the minimum stitch
  length.
- An element with an error diagnostic is skipped; the rest of the design still plans. A design whose
  every element was skipped yields `SC-E0010`. `stitch plan` writes the machine file only when no error
  is said, as Ink/Stitch writes its file only when no element fails (`REQ-CLI-003`).

## 3. Generate

Each element becomes stitch groups (`stitchcraft_engine::generate`): the needle points of each part of it
that a jump may separate from the next, in sewing order. A stroke gives one group per subpath, and a part
too small for a stitch gives none (`SC-W0401`). The element's own settings stay with the element, and
assembly reads them there: its thread, locks, trim and stop.

The stitch type picks the generator. A stroke whose `satin_column` setting is on is a satin column,
whatever its `stroke_method` says, as in Ink/Stitch. Its rails and rungs are recognized, what
recognition finds is reported, and its `satin_method` picks the generator: the satin stitch, sewn from
M4.2, the E-stitch, from M7.3, the S-stitch, from M7.4, and the zigzag satin, from M7.5. A satin column also reads the running stitch's length (`running_stitch_length_mm`): the stitches
that join its underlays are no longer than the first value. For any other stroke the generator is its
`stroke_method`. `running_stitch` (the default) and `manual_stitch` are sewn from M3, and `zigzag_stitch`
from M7.2, as wide as the stroke. A satin column drawn as one path no wider than the design's
`min_satin_stroke_width` is sewn as a stroke, with `SC-W0212`, as in Ink/Stitch (`REQ-SAT-015`). Wider,
it is sewn between rails made from its line from M4.9 (`REQ-SAT-016`). A fill whose `fill` setting is
off sews nothing, and nothing is said of it (`REQ-FILL-003`). Another fill's region is built first, so
what it leaves out and how it will be sewn are said (`SC-W0303`, `SC-W0304`, `SC-I0306`, `SC-W0307`)
whatever comes next, and its `fill_method` picks the generator: the tatami fill, sewn from M5.4, which
reads the running stitch's length and tolerance for its travel between rows (the first of the lengths,
as a satin column does), under the rows from M5.5, and sews its underlay before its top rows from
M5.6. A fill set to Ink/Stitch's legacy fill is sewn as a tatami fill without underlay, with
`SC-W0314`: StitchCraft does not build that method (`REQ-FILL-005`). The other stroke, satin and
fill methods are skipped with `SC-W0011` until their milestones. An element whose parameters are wrong
(`SC-E0101`) is skipped too, and the rest of the design still plans.

Each element is generated with the shortest stitch for it: the larger of the machine's (the profile's
`min_stitch`) and the element's `min_stitch_length_mm`, or the design's shortest stitch when the element
sets none. A satin column is given its longest stitch as well, the element's `max_stitch_length_mm`,
which its split stitches keep to.

A preview that cannot afford a fill's stitches shows its top rows (`stitchcraft_engine::generate::rows`,
from M6.3). The fill's settings are read as for sewing it, and what they say is said the same, but only
the region is built and the rows laid over it: no needle point is placed and no route is searched.

Rules every generator follows:

- **Pure:** output depends only on (normalized shape, typed params, neighbours, seed, budget).
- **The needle before, the next element after.** Elements are generated in sewing order
  ([ADR 0014](adr/0014-generators-see-their-neighbours.md), `REQ-GEN-003`). Each is given the last needle
  point of the elements before it, whatever their thread, and what the next element offers to end near.
  That is its first point, or, when it starts at its own nearest point, its shape: a satin column's rails
  or a tatami fill's area. The offer comes from the next element's shape and settings and the design's,
  never its stitches, so nothing waits for a later element. A satin column and a tatami fill start and
  end by them ([satin](algorithms/satin.md#start-and-end), [fills](algorithms/fills.md#5-parts)), and
  explicit start and end commands will override them (`REQ-GEN-001`). Each ends near its next stitch, its
  point nearest the offer as Ink/Stitch's geometry library finds it, a point of one shape inside the
  other's area first (`generators::next`).
- **Seeded randomness:** each element's generator is seeded from its `random_seed`, one of the settings
  every stitch type shares. When that is empty, a hash of its id seeds it. A seed replaces the id, as in
  Ink/Stitch. The PRNG is SplitMix64 from `stitchcraft-core` ([determinism](determinism.md)).
- **Budgeted:** every inner loop charges work units. Each element has the budget's work to itself. One
  that runs out (`SC-E0004`) is skipped, and the others still plan. The stitch limit is for the whole
  design. Assembly and finalize share the budget's work once more, and running out there plans nothing.
  `SC-E0004` then says so for the whole design. A generator also spends the design's stitches on the
  needle points it makes ([budgets](data-model.md#budgets), `REQ-GEN-004`). Each element gets what the
  elements before it left, and the first that needs more ends planning, with `SC-E0004` naming it.
- **Roles:** every stitch is tagged by its role. Assembly tags the stitches of groups as top and the lock
  stitches as lock. The jumps between groups are travel. Previews draw the lock stitches apart. A
  generator hands assembly its needle points without roles, so an element's underlay, and the running
  stitches between its rows, are tagged top as well, until generators tag their own.

Generators are listed in [algorithms](algorithms/README.md).

## 4. Plan assembly

Groups are joined into colour blocks in the order the elements sew (host paint order, bottom first, or the
order stage 0 chose), by `stitchcraft_engine::assemble`.

### Ordering

- Assembly keeps the order the elements come in, so that embroidery stacking follows the art's stacking.
  That is paint order, unless stage 0 put the automatic elements, or with `--sort-colours` every element,
  in an order chosen for few thread changes, in which a shape still sews after every shape it is drawn
  over, where they overlap by more than rounding leaves.
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
machine's business. The plan trims where an element's `trim_after` is set, as Ink/Stitch's does. A PES
file also flags every jump as a trim, as pystitch writes Ink/Stitch's
([formats](formats.md#what-we-write-v1)), and whether a machine cuts at a jump is its own setting. At
MC-1 the reference machine cut the thread only at thread changes and stops.

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

The finished plan is in hoop coordinates: the design's origin goes to the hoop's centre, (0, 0), where the
needle starts. The origin is the design's own (Ink/Stitch's origin command, read from M8) or else the centre
of the box around every point where the needle goes down (`REQ-ASM-005`). Assembly builds the plan in the
design's coordinates, and finalize moves it once it has left out the points the machine cannot sew, so the
box is the finished plan's, as in Ink/Stitch. A design's stop position
(Ink/Stitch's stop position command) adds a jump to it before each `Stop`. At the stop position the frame
is clear of the needle for an appliqué or a check, and sewing resumes with a jump back.

### Commands

| Command | Effect | From |
|---|---|---|
| Start / end point | Entry/exit hints for the generator (`REQ-GEN-001`). The VectorCraft adapter reads them from markers named `stitch:start` and `stitch:end`, which it names until then | M8 |
| Target point | Centre for circular fills and ripple targets | M7, M10 |
| Trim after, stop after | `trim_after` and `stop_after` on the element's last group (see the connection table). The SVG adapter reads Ink/Stitch's trim and stop commands as these settings. | M3.8 (engine), M3 (adapter) |
| Ignore object / ignore layer | The adapter drops the element and says so in the report (`REQ-ASM-004`). The VectorCraft adapter reads them from the name `stitch:ignore` | M3 (SVG), M6 (VectorCraft) |
| Origin | The design setting `origin` | M3.8 (engine), M8 (adapter) |
| Stop position | The design setting `stop_position` | M3.8 (engine), M8 (adapter) |

## 5. Finalize

Finalize fits the plan to the machine profile (`stitchcraft_engine::finalize`), and a plan that `plan`
returns can be written as it is. Generators sew within the machine's limits where their settings let
them. What is left to fit comes from joining groups and from settings the machine cannot follow.

1. **The shortest stitch** of each needle point is its element's: the one its generator used, so finalize
   never thins what the generator spaced. That is the element's `min_stitch_length_mm`, else the design's
   `min_stitch_len`, and never shorter than the machine's (`profile.min_stitch`). Where one element's
   stitching runs straight on into the next, the stitch between them can be anything up to the collapse
   length, 0 included. Within each run of stitches, from where the needle lands to the next jump, trim or
   stop, a needle point less than its shortest stitch from the one before is left out. The stitch then
   runs on to the next point. The run's first and last points and lock points stay, and the points
   before them are left out instead. Elements sewn over each other can bring the last point or a lock
   point within the shortest stitch of one that stays before it, once the points between are left out.
   It is then left out itself, the points before it stay, and a trim or stop after it happens where the
   needle now is. So is a point where the needle already is, which would sew in place. Only the run's
   first point always stays. A stitch into or out of a lock point is a lock stitch, whose shortest is 0.2 mm
   (`REQ-PLAN-002`). `SC-I0504` says how many points were left out (`REQ-FIN-001`). The rule is one
   function, which a generator that joins pieces of its own also applies, as a tatami fill does to its
   rows and the travel between them, so that finalize finds nothing of its stitches to thin. A satin
   column joins its parts by rules of its own instead, since the end of a top stitch must not move
   ([satin](algorithms/satin.md#underlays)).
2. **The longest stitch.** A stitch longer than `profile.max_stitch` is split into the fewest equal parts
   no longer than it (`SC-I0703`, `REQ-FIN-001`). Such a stitch comes from a stitch placed by hand, a
   custom lock's long step, a satin column wider than the machine's longest stitch, a tatami fill whose
   `max_stitch_length_mm` is longer than it, or a move sewn on under a `min_jump_stitch_length_mm` that
   long. Each part counts against the design's stitch budget.
3. **The hoop's centre.** The fitted plan is moved so the design's origin is at the hoop's centre
   ([origin](#origin-and-stop-position), `REQ-ASM-005`).
4. **Colour limits:** colour changes and stops above what the profile's format records → `SC-E0601`.
5. **Hoop:** a design larger than the hoop → `SC-E0701`, with a rotate-to-fit fix when rotating 90° would
   fit. When a design's origin is far from its middle, a design smaller than the hoop can cross the
   hoop's edge. That is `SC-E0701` as well. So is a stop position past the edge, which the message names. The size is
   the stitches' own. For a design that cannot be sewn, `SC-E0701` is the only message about its size.
   Otherwise, larger than the comfort zone → `SC-W0702` (`REQ-FIN-002`).
6. The end is the plan's structure: the machine ends after the last block.

After an error at any step, `plan` returns no plan, and the error's diagnostic says why.

Splitting jumps longer than a format can encode in one record is *not* done here: it is the encoder's
job, because the limit is a property of the file format, not of the machine.

Ink/Stitch (read at `d59c9ab`) removes short stitches once, over the whole plan: an entry no farther than
its shortest stitch (the element's `min_stitch_length_mm`, else the design's `min_stitch_len_mm`, 0.1 mm
unless the file sets it) from the last one kept
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
| Report JSON | The CLI's, with the counts and size of the machine file read back as a machine reads it, its SHA-256, the threads and the diagnostics |
| Plug-in preview geometry | `stitchcraft-vc-plugin`, from one element's groups, its top rows or its path, by the preview's level ([integration](vectorcraft-integration.md#live-effect-previews-within-v1-budgets)) |
