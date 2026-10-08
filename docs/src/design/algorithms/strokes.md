# Stroke generators

Strokes follow a path. Shape: `StrokePath` (an ordered list of polylines, corners marked, from the
normalizer). Phase P1 (M3) unless noted.

## Running stitch

The workhorse: outlines, details, travel, underlay.

**Parameters:** `running_stitch_length_mm` (one length or a repeating pattern of lengths, `LengthList`),
`running_stitch_tolerance_mm`, `repeats`, `bean_stitch_repeats`, `enable_random_stitch_length`,
`random_stitch_length_jitter_percent`, `random_seed`.

### Placement

1. **Split at corners.** Corners marked by the normalizer (turning angle > 30°) always receive a needle
   penetration, so sharp shapes stay sharp.
2. **Even spacing per span.** For a span of arc length `L` between corners and target length `s`,
   place `n = ceil(L / s − ε)` stitches spaced `L / n` apart along the arc. Every stitch is at most `s`
   and the last one is never a short leftover — the classic even-division rule digitizers use.
3. **Respect the curve.** If the chord between two consecutive needle points strays from the curve by
   more than the tolerance, split that stitch at the point of maximum deviation and repeat. This bounds
   the visual error by `running_stitch_tolerance_mm` on any curve.
4. **Patterns.** With several lengths (`"2.5 1.0"`) the lengths repeat along the path; each span still
   ends exactly on its corner by scaling its stitches proportionally.
5. **Random length.** With `enable_random_stitch_length`, each stitch length is drawn uniformly from
   `s × (1 ± jitter)`, then the span is rescaled to end on its corner. The seed makes it repeatable.

### Repeats and bean stitch

- `repeats = k` sews the path k times, alternating direction; odd k ends at the far end, even k returns
  to the start (useful for travel that must come back).
- `bean_stitch_repeats = b` replaces every stitch A→B with A→B→A→B… (2b+1 passes), producing the thick
  "triple stitch" line at b = 1. A list (`"1 0"`) alternates bean and plain stitches.

### Properties (conformance)

- Every top stitch ≤ `s × (1 + jitter)` + 1 µm, and ≥ the minimum stitch length unless the path is
  shorter than it (`REQ-RUN-001`).
- Maximum deviation from the source curve ≤ tolerance (measured densely) (`REQ-RUN-002`).
- Corners are penetration points (`REQ-RUN-003`).
- Bean stitch: stitch count is exactly `n × (2b + 1)` for a span of n stitches (`REQ-RUN-004`).
- Repeats parity decides the exit point (`REQ-RUN-005`).

### Diagnostics

`SC-W0401` path shorter than the minimum stitch length (skipped); `SC-W0402` stitch length below the
profile minimum (clamped).

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
