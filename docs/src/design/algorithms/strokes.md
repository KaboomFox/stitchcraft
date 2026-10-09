# Stroke generators

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
6. **Random length** (M3.5). With `enable_random_stitch_length`, each stitch length is drawn uniformly
   from `s × (1 ± jitter)`, then the span is rescaled to end on its corner. The seed makes it repeatable.

**When the rules disagree,** the shortest stitch wins (a shorter stitch hammers one spot and can break the
thread), then corners, then the tolerance. A part of the path shorter than the shortest stitch, or lying
all within it of its ends, is not stitched (`SC-W0401`). Only arithmetic and square roots are used, so
every platform places the same stitches.

### Repeats and bean stitch

- `repeats = k` sews the path k times, alternating direction; odd k ends at the far end, even k returns
  to the start (useful for travel that must come back).
- `bean_stitch_repeats = b` replaces every stitch A→B with A→B→A→B… (2b+1 passes), producing the thick
  "triple stitch" line at b = 1. A list (`"1 0"`) alternates bean and plain stitches.

### Properties (conformance)

- Stitches are spread evenly between corners; every stitch, measured straight, is at most
  `s × (1 + jitter)` + 1 µm for the longest length `s` and at least the shortest stitch; a part too small
  for that is reported (`REQ-RUN-001`).
- Deviation from the source path ≤ tolerance, both ways, measured densely (`REQ-RUN-002`).
- Corners are penetration points unless that would make a stitch too short (`REQ-RUN-003`).
- Bean stitch: stitch count is exactly `n × (2b + 1)` for a span of n stitches (`REQ-RUN-004`).
- Repeats parity decides the exit point (`REQ-RUN-005`).

### Diagnostics

`SC-W0401` part of the path too small for the shortest stitch (skipped); `SC-W0402` stitch length below
twice the shortest stitch (raised).

## Manual stitch

Every node of the path is a needle penetration, in order — for hand-placed stitches and imported stitch
files. Segments longer than `max_stitch_length_mm` (if set) are split evenly. Bean stitch applies.
Property: output nodes equal input nodes plus only the documented splits (`REQ-RUN-006`).

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
