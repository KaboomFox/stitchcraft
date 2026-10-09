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

Each element is turned into a `StitchGroup`:

```rust,ignore
pub struct StitchGroup {
    pub element: ElementId,
    pub thread: Thread,
    pub stitches: Vec<(Point, Role)>,   // normal stitches only; no jumps or commands yet
    pub entry: Point,
    pub exit: Point,
    pub locks: LockSettings,
    pub trim_after: bool,
    pub stop_after: bool,
}
```

Rules every generator follows:

- **Pure:** output depends only on (normalized shape, typed params, hints, seed, budget).
- **Entry and exit hints, not neighbours' stitches.** The previous element's *geometry* suggests where
  this one should start (nearest point to its exit hint); the generator never waits for another
  element's stitches. This makes generation independent, cacheable and parallel, so large designs
  stay fast, and honours explicit
  start/end commands (`REQ-GEN-001`).
- **Seeded randomness:** seed = `random_seed` parameter if set, else a hash of the element id; the PRNG
  is SplitMix64 from `stitchcraft-core` ([determinism](determinism.md)).
- **Budgeted:** every inner loop charges work units; exhaustion returns `SC-E0004` and no stitches for
  this element.
- **Roles:** every stitch is tagged underlay, top, travel or lock, which previews colour-code and which
  later enables per-layer ordering.

Generators are listed in [algorithms](algorithms/README.md).

## 4. Plan assembly

Groups are joined into colour blocks in document order (host paint order, bottom first).

### Ordering

- Document order is kept: embroidery stacking follows the art's stacking. Reordering for fewer colour
  changes is an explicit, separate tool (P3) because it changes what covers what.
- A thread change starts a new colour block with a `ColorChange`.

### Connecting consecutive groups

With `d` the distance from the previous group's exit to the next group's entry:

| Condition | Connection |
|---|---|
| Same colour, `d ≤ min_jump_stitch_length` (if set) or `d ≤ collapse_len` (default 3.0 mm), no `force_lock_stitches` | Sewn directly (the jump "collapses" into stitches, split to the maximum stitch length) |
| Same colour, travel path inside the next region exists and the next element hides it | Travel stitches along that path (role `travel`) |
| Otherwise | Tie-off on the previous group → `Jump` → (`Trim` when `d` exceeds the profile's trim threshold or the element says `trim_after`) → tie-in on the next group |
| Colour change | Tie-off → `ColorChange` → tie-in |
| `stop_after` | Tie-off → `Stop` → tie-in when sewing resumes |

Ink/Stitch (read at `d59c9ab`) joins same-colour groups within the collapse length with one direct
stitch, and trims only when told to (`trim_after`, a trim command). StitchCraft splits that stitch to the
machine's longest stitch and also trims at the profile's threshold: both differences come from the
machine profile, and go into the deviations ledger with M3.8.

### Lock stitches (ties)

- `ties` selects where locks go: both, before (tie-in), after (tie-off) or neither.
- `lock_start` / `lock_end` choose a lock shape by id; `half_stitch` (stitch back and forth along the
  path by a fraction of the first stitch) is the default; other ids map to StitchCraft's own shape
  definitions with the same intent ([compatibility contract](inkstitch-compat-contract.md#method-identifiers)).
- Shapes are defined in a unit frame aligned with the path direction. Shapes made of back-and-forth
  steps are scaled by `lock_*_scale_mm` (an absolute size), shapes drawn as paths by
  `lock_*_scale_percent` (relative to their own size); the half stitch is sized from the first stitch.
  A custom lock (`lock_custom_start`, `lock_custom_end`) is either kind.
- `force_lock_stitches` adds locks even when the next group is close enough to collapse. In Ink/Stitch
  (read at `d59c9ab`) it also adds the tie-off when `ties` asks for none after, but never a tie-in.
- A group of fewer than two stitches gets no locks. Manual stitch gets none unless `force_lock_stitches`
  is set.
- Ink/Stitch's half stitch goes back and forth towards the first needle point at least 0.5 mm away, by
  fractions of that distance capped at 1.5 mm, and ignores both scale parameters; its tie-off leaves out
  its first point, which is the group's last stitch. Its settings window shows `lock_*_scale_mm` only for
  back-and-forth and custom locks and `lock_*_scale_percent` only for drawn shapes and custom locks;
  StitchCraft's parameter registry should show them the same way (M3.7).
- Lock stitches must be ≥ 0.2 mm long so the needle does not hit the same hole; shorter computed locks
  are scaled up, never dropped silently.

### Commands

| Command | Effect |
|---|---|
| Start / end point | Entry/exit hints for the generator |
| Target point | Centre for circular fills and ripple targets |
| Trim after, stop after | Flags on the group (see the connection table) |
| Ignore object / ignore layer | The adapter drops the element (and says so in the report) |
| Origin | The machine origin (encoders translate to it); default: centre of the design's bounding box |
| Stop position | A jump to this point before each `Stop`, so the frame moves out of the way |

## 5. Finalize

Against the machine profile:

1. **Split** `Normal` stitches longer than `profile.max_stitch` into equal parts.
2. **Merge** `Normal` stitches shorter than `min_stitch_len` (settings or profile, whichever is larger)
   into their neighbour, except lock stitches.
3. **Remove** consecutive duplicate positions.
4. **Hoop:** if the plan's bounds after translation to the origin exceed the hoop → `SC-E0701` (with a
   rotate-to-fit hint when rotating 90° would fit); if they exceed the comfort zone → `SC-W0702`.
5. **Colour limits:** colour changes above the format maximum → `SC-E0601`.
6. Append `End`.

Splitting jumps longer than a format can encode in one record is *not* done here: it is the encoder's
job, because the limit is a property of the file format, not of the machine.

Ink/Stitch (read at `d59c9ab`) removes short stitches once, over the whole plan: a stitch no longer than
the shortest stitch (the element's `min_stitch_length_mm`, else its global 0.1 mm) from the last one kept
is dropped, except lock stitches and the first stitch after a jump, stop, trim or colour change; nothing
is split. StitchCraft's floor is the machine's (0.3 mm on the Brother), short stitches merge into their
neighbour, and the running stitch already keeps the floor while it places stitches.

## 6. Check

The plan invariant checker (`stitchcraft-plan::invariants`, the same code conformance level L0 runs)
validates the result. A violation is a bug in StitchCraft: the CLI refuses to write the file, reports
`SC-E0009 internal check failed` with a bug-report bundle, and the conformance suite has a case for every
invariant.

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
