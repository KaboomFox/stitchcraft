# Stroke generators

<!-- implements: crates/stitchcraft-engine/src/generators/mod.rs, crates/stitchcraft-engine/src/generators/running/**, crates/stitchcraft-engine/src/generators/manual.rs, crates/stitchcraft-engine/src/generators/passes.rs, crates/stitchcraft-engine/src/normalize/stroke.rs, crates/stitchcraft-engine/src/normalize/along.rs -->

Strokes follow a path. Shape: `StrokePath` (an ordered list of polylines, corners marked, from the
normalizer). Phase P1 (M3) unless noted.

## Running stitch

The workhorse: outlines, details, travel, underlay.

**Parameters:** `running_stitch_length_mm` (one length or a repeating pattern of lengths, `LengthList`),
`running_stitch_tolerance_mm`, `repeats`, `bean_stitch_repeats`, `enable_random_stitch_length`,
`random_stitch_length_jitter_percent`, `random_seed`.

### Placement

A stitch is the straight line between two needle points, so placement measures along the path but
checks straight. "The shortest stitch" is the one [Finalize](../engine-pipeline.md#5-finalize) enforces
(settings or profile, whichever is larger).

1. **Flatten.** The normalizer turns the path into polylines within a tenth of the tolerance (the other
   nine tenths are left for the stitches) and marks its corners: joins between segments where the path
   turns by more than 30°. A curve's own bend is never a corner, however tight.
2. **Split at corners.** Every corner gets a needle penetration, so sharp shapes stay sharp, unless it is
   closer than the shortest stitch, along the path, to the previous penetration or to the end: then the
   spans on either side of it are joined.
3. **Even spacing per span.** For a span of length `L` between corners and target length `s`, place
   `n = ceil(L / s − ε)` stitches spaced `L / n` apart along the path. Every stitch is at most `s` and the
   last one is never a short leftover — the classic even-division rule digitizers use. With several
   lengths (`"2.5 1.0"`) the lengths repeat along the path, carrying on from span to span; each span takes
   as many as it needs and scales them by one factor to end exactly on its corner. Lengths below twice
   the shortest stitch are raised to it (`SC-W0402`), so a span longer than the longest length scales its
   stitches by more than a half and keeps each at or above the shortest stitch; in a shorter span, a
   stitch scaled below it joins its shorter neighbour.
4. **Measure straight.** Where the path bends back on itself within less than the shortest stitch (a
   cusp, a tight loop, a curl at an end), two points far apart along it can be close in a straight line.
   Going along the needle points: one closer than the shortest stitch to the last one kept is dropped; a
   corner drops the points kept since the previous corner instead, and is dropped itself if that is not
   enough; the end drops whatever it takes, corners too. A drop lengthens the stitch over it, and one
   longer than the longest length is split where the path is half its length from its start (at most
   the longest length), as often as needed. When only the start is left and the end is too close to it
   (a small closed loop), the stitch goes by way of the point of the path farthest from both ends; if
   even that is closer than the shortest stitch, the part is not stitched (`SC-W0401`).
5. **Respect the curve.** If the chord between two consecutive needle points strays from the path by
   more than the tolerance left after flattening, split that stitch at the path's point of greatest
   deviation, among those that leave both parts within the lengths above, and repeat. This bounds the
   visual error by `running_stitch_tolerance_mm` on any curve whose details are larger than the shortest
   stitch.
6. **Random length.** With `enable_random_stitch_length`, each stitch length is drawn uniformly from
   `s × (1 ± jitter)`, the first of each span a random fraction of that (a random phase, below), then the
   span is rescaled to end on its corner. The element's own generator draws them, seeded with its
   `random_seed` (a setting every stitch type that varies at random shares), and the same element and
   seed always give the same stitches.

**When the rules disagree,** the shortest stitch wins (a shorter stitch hammers one spot and can break the
thread), then corners, then the tolerance. A part of the path that is a single point, shorter than the
shortest stitch, or lying all within it of its ends, is not stitched (`SC-W0401`). Only arithmetic and square roots are used, so
every platform places the same stitches.

**Compared with Ink/Stitch:** the same parameters, meanings and defaults; the placement differs in four
documented ways, `DEV-RUN-001` to `DEV-RUN-004` in the deviations ledger (`conformance/deviations.toml`).

### Repeats and bean stitch

- `repeats = k` sews the path k times, alternating direction; odd k ends at the far end, even k returns
  to the start (useful for travel that must come back). Each pass starts where the last one ended, so a
  turnaround is not a stitch in place.
- `bean_stitch_repeats = b` replaces every stitch A→B with A→B→A→B… (2b+1 passes), producing the thick
  "triple stitch" line at b = 1. A list (`"1 0"`) alternates bean and plain stitches.
- **Order, as in Ink/Stitch** (read at `d59c9ab`): bean stitch applies after repeats, to every stitch of
  every pass, with the list running on from pass to pass. Ink/Stitch repeats the turnaround point, so
  the turnaround takes one step of the list; StitchCraft counts that step too, without sewing it, so a
  file sews the same in both. With a two-value list, each stretch of the path then gets the same
  treatment on every pass.
- Repeats apply to running, ripple and zigzag strokes; bean stitch to those and to manual stitch (the
  [compatibility contract](../inkstitch-compat-contract.md) lists each parameter's stitch types).

### Random length

Ink/Stitch draws each stitch from `s × (1 ± jitter)` and starts each stretch between corners at a random
phase, its first stitch a random fraction of a drawn length, so rows sewn side by side with the same
settings do not line their holes up. It does not rescale, so the last stitch before a corner is whatever
is left. StitchCraft rescales each span to end on its corner (placement, step 6: no short leftover) and
keeps the random phase, for the same reason.

### Properties (conformance)

- Stitches are spread evenly between corners; every stitch, measured straight, is at most
  `s × (1 + jitter)` + 1 µm for the longest length `s` and at least the shortest stitch; a part too small
  for that is reported (`REQ-RUN-001`).
- Deviation from the source path ≤ tolerance, both ways, measured densely (`REQ-RUN-002`).
- Corners are penetration points unless that would make a stitch too short (`REQ-RUN-003`).
- Bean stitch sews each stitch `2b + 1` times, so a run of n stitches with b throughout has exactly
  `n × (2b + 1)`; the list runs on across repeats, each turnaround taking a step (`REQ-RUN-004`).
- Repeats alternate direction with no stitch in place; their parity decides the exit point
  (`REQ-RUN-005`).
- Random length: the same element and seed give the same stitches, another seed others, and the lengths
  vary (`REQ-RUN-007`).

### Diagnostics

`SC-W0401` part of the path too small for the shortest stitch (skipped); `SC-W0402` stitch length below
twice the shortest stitch (raised).

## Manual stitch

Every node of the path is a needle penetration, in order — for hand-placed stitches and imported stitch
files. A curve gives only its end node: its control points are not stitched and it is not flattened. A
node where the needle already is counts once, and a closed path comes back to its start. Segments longer
than `max_stitch_length_mm` (if set) are split into the fewest equal parts no longer than it, but never
into parts shorter than the shortest stitch. A value of 0 or less means "not set", as in Ink/Stitch, so
every stitch is then sewn as drawn. Bean stitch applies; repeats do not. Lock stitches are added
only when `force_lock_stitches` is set, as in Ink/Stitch (read at `d59c9ab`).

The shortest stitch holds here too: a node closer than it to the needle point before it is left out, so
the stitch before runs on to the next node, and `SC-W0403` says how many. A part's last node is always
kept, leaving out the one before it instead. Ink/Stitch drops such points later, over the whole plan,
without a word. A part that is a single point, or too small for one stitch, is not stitched (`SC-W0401`).

Properties: needle points are the nodes in order plus only the documented splits (`REQ-RUN-006`); no
stitch is shorter than the shortest stitch (`REQ-RUN-008`).

## Zigzag stroke (P2, M7)

A zigzag of width `stroke-width` (or the satin width parameters) centred on the path, with peak-to-peak
spacing `zigzag_spacing_mm`, optional `zigzag_angle` (slanted zigzag), and
`stroke_pull_compensation_mm`. Implemented as a satin on a centre line with no underlay (shares the satin
sampler), which keeps one code path for "two sides and alternation".

## Ripple stitch (P3, M10)

A series of copies of the stroke, each progressively scaled, offset or interpolated towards a target
point (or between two rails for "satin-guided ripple"), joined into one continuous running stitch.
Parameters: `line_count`, `min_line_dist_mm`, `staggers`, `skip_start`, `skip_end`, `flip_copies`,
`exponent`, `flip_exponent`, `reverse`, `grid_size_mm`, `grid_first`, `scale_axis`, `scale_start`,
`scale_end`, `rotate_ripples`, `join_style`, `satin_guide_pattern_position`, `reverse_rails`,
`swap_satin_rails`, plus running stitch parameters for each line. Design to be completed at M10; the
properties will include "lines never cross the target point" and "each line is within tolerance of its
interpolated curve".
