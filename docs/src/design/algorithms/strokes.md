# Stroke generators

<!-- implements: crates/stitchcraft-engine/src/generators/mod.rs, crates/stitchcraft-engine/src/generators/running/**, crates/stitchcraft-engine/src/generators/manual.rs, crates/stitchcraft-engine/src/generators/zigzag/**, crates/stitchcraft-engine/src/generators/passes.rs, crates/stitchcraft-engine/src/normalize/stroke.rs, crates/stitchcraft-engine/src/normalize/along.rs -->

Strokes follow a path. Shape: `StrokePath` (an ordered list of polylines, corners marked, from the
normalizer). Phase P1 (M3) unless noted.

A stroke starts where it is drawn and ends where it is drawn, as in Ink/Stitch. Its generator is given its
neighbours, as every generator is ([ADR 0014](../adr/0014-generators-see-their-neighbours.md)), and a
stroke sews the same whatever they are. A satin column or a tatami fill before a stroke ends near the
stroke's first point.

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
   stitch scaled below it joins its shorter neighbour. Where those stitches stray from the path by more
   than the tolerance left after flattening, as on a curve too tight for them, the span is spread again
   in one stitch more, then one more, as long as that takes no more stitches than step 5 would make of
   it, until they keep to it. A small circle is so sewn in even stitches as long as the tolerance left after
   flattening allows, where splitting the stray ones would halve them. With random length, each try draws its lengths again
   from where the span's draws began.
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
   deviation, among those at least the shortest stitch from both its ends, and repeat. A part longer than
   the longest length is split along the path as in step 4. So the stitches stray from the path by at
   most `running_stitch_tolerance_mm`, except where the path's details are smaller than the shortest
   stitch. There a point of the flattened path that is passed over lies nearer than the shortest stitch
   to a needle of its stitch, and the path strays by less than the shortest stitch plus a tenth of the
   tolerance.
6. **Random length.** With `enable_random_stitch_length`, each stitch length is drawn uniformly from
   `s × (1 ± jitter)`, the first of each span a random fraction of that (a random phase, below), then the
   span is rescaled to end on its corner. Each span draws from its own stream of the element's generator,
   keyed by its subpath's place in the path and its own place in the subpath, so a change to one span
   leaves the lengths of the others as they were. The generator is seeded with the element's
   `random_seed` (a setting every stitch type that varies at random shares), or with its id when that is
   empty. A seed replaces the id, as in Ink/Stitch, so elements alike given one seed sew alike, and the
   same element and seed always give the same stitches.

**When the rules disagree,** the shortest stitch wins (a shorter stitch hammers one spot and can break the
thread), then corners, then the tolerance. A part of the path that is a single point, shorter than the
shortest stitch, or lying all within it of its ends, is not stitched (`SC-W0401`). Only arithmetic and square roots are used, so
every platform places the same stitches.

**Compared with Ink/Stitch:** the same parameters, meanings and defaults; the placement differs in four
documented ways, `DEV-RUN-001` to `DEV-RUN-004` in the deviations ledger (`conformance/deviations.toml`),
and random length draws and places its lengths in other ways (`DEV-RUN-005`).

**Satin underlays** walk lines made point by point by the satin generator
([satin](satin.md#underlays)), and steps 2 to 5 place their stitches, with each walk's own length and
tolerance. Such a line is a polyline already, and each join that turns by more than 30° is a corner. A
line too small for a stitch of the shortest length is sewn as its 2 ends, as Ink/Stitch sews it, and
finalizing merges what is too short.

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
- A zigzag's turnaround takes no step of the list, as in Ink/Stitch ([zigzag stroke](#zigzag-stroke-p2-m7)).
- Repeats apply to running, ripple and zigzag strokes; bean stitch to those and to manual stitch (the
  [compatibility contract](../inkstitch-compat-contract.md) lists each parameter's stitch types).

### Random length

Ink/Stitch draws each stitch from `s × (1 ± jitter)` and starts each stretch between corners at a random
phase, so rows sewn side by side with the same settings do not line their holes up. One random number
gives a stitch both its length and its phase: the first stitch of a stretch is its drawn length times
that number. After a stitch that the tolerance made shorter, the next starts a new phase too. Ink/Stitch
does not rescale, so the last stitch before a corner is whatever is left. Each stretch draws from a
stream keyed by the seed and the stretch's place in its subpath, and the places start again in each
subpath, so subpaths drawn alike sew alike.

StitchCraft draws a stitch's length and its span's phase from 2 numbers. A phase starts only where a span
starts, and each span is rescaled to end on its corner (placement, step 6: no short leftover). Its streams
are keyed by the subpath's place as well, so subpaths drawn alike, such as lines side by side, start at
different phases, which is what the random phase is for (`DEV-RUN-005`).

### Properties (conformance)

- Stitches are spread evenly between corners, in more of them where they stray from the points of the
  flattened path by more than the tolerance left after flattening, as long as that takes no more than
  splitting. Measured straight, every stitch is at most
  `s × (1 + jitter)` + 1 µm for the longest length `s` and at least the shortest stitch, and a part too
  small for that is reported (`REQ-RUN-001`).
- Deviation from the source path ≤ tolerance, both ways, measured densely (`REQ-RUN-002`).
- Corners are penetration points unless that would make a stitch too short (`REQ-RUN-003`).
- Bean stitch sews each stitch `2b + 1` times, so a run of n stitches with b throughout has exactly
  `n × (2b + 1)`; the list runs on across repeats, each turnaround taking a step (`REQ-RUN-004`).
- Repeats alternate direction with no stitch in place; their parity decides the exit point
  (`REQ-RUN-005`).
- With random length the lengths vary. The same element and seed give the same stitches, and another
  seed gives others. A seed replaces the element's id, and elements given one seed draw the same lengths.
  Each span draws from its own stream, so a change to one span leaves the others' lengths as they were
  (`REQ-RUN-007`).

### Diagnostics

`SC-W0401` part of the path too small for the shortest stitch (skipped); `SC-W0402` stitch length below
twice the shortest stitch (raised), a stroke's or a satin underlay's, which the message names.

## Manual stitch

Every node of the path is a needle penetration, in order — for hand-placed stitches and imported stitch
files. A curve gives only its end node: its control points are not stitched and it is not flattened. When
the path is closed it comes back to its start, and a node where the needle already is sews no stitch.
Segments longer than `max_stitch_length_mm` (if set, a setting manual stitch shares with satin columns)
are split into the fewest equal parts no longer than it, but never into parts shorter than the shortest
stitch, and `SC-W0404` says how many stitches stay longer for that. Ink/Stitch splits a segment into the
fewest equal parts no longer than the setting, however short they are (`DEV-RUN-006`). A value of 0 means
"not set", as in Ink/Stitch, and so does one below 0, so every stitch is then sewn as drawn. Bean stitch
applies, and repeats do not. Lock stitches are added only when `force_lock_stitches` is set, as in
Ink/Stitch (read at `d59c9ab`).

Ink/Stitch's bean stitch takes a step of its list for every point it lists, and it lists some where the
needle already is. A node drawn twice is listed twice, and so is the end of each stitch Ink/Stitch
splits. Where a closed path came back to its start already, the start is listed once more. Its stitch
plan leaves out stitches shorter than its shortest stitch only after the bean stitch, and a node left out
for that keeps its step. StitchCraft gives each of these its step, sewing nothing, and a bean list then
falls on the same stitches in both tools.

The shortest stitch holds here too: a node closer than it to the needle point before it is left out, so
the stitch before runs on to the next node, and `SC-W0403` says how many. A part's last node is always
kept, leaving out the one before it instead. Ink/Stitch drops such points later, over the whole plan,
without a word, and only those 0.1 mm or less from the point before at its default (`DEV-RUN-006`). A part
that is one point, or too small for one stitch, is not stitched (`SC-W0401`).

Properties: needle points are the nodes in order plus only the documented splits, and `SC-W0404` names
a stitch kept longer than `max_stitch_length_mm` (`REQ-RUN-006`). Every stitch is at least as long as the
shortest stitch (`REQ-RUN-008`).

## Zigzag stroke (P2, M7)

A zigzag along the path, as wide as its stroke. The needle crosses the path from side to side, as
Ink/Stitch sews its zig-zag stitch (`stroke_method` `zigzag_stitch`, read at `d59c9ab`). It suits a light
border, a couched look or a stem too thin for a satin column.

**Parameters:** `zigzag_spacing_mm`, the distance along the path from one stitch to the next that crosses
the same way, a zig and a zag together, 0.4 mm by default; `stroke_pull_compensation_mm`, how much further
out than half the stroke's width each side's needle points go, one value for both sides or 2; and
`zigzag_angle`, which turns every stitch away from square to the path, from −89° to 89°. `repeats` and
`bean_stitch_repeats` apply. The width is the stroke's, read as a satin column drawn as one path reads it.

Strokes, satin columns and fills each read `zigzag_spacing_mm` their own way
([parameter registry](../params.md#naming-rules), rule 7). A stroke reads a list, as Ink/Stitch does.
Only its first value counts.

### Placement

Each part of the path, one per subpath, is sewn on its own, as a running stitch is.

1. **Spread.** The part is flattened within a tenth of a CSS pixel, as a satin column's or a fill's
   subpaths are, and `n + 1` points are spread along it at equal distances, its ends included. `n` is its
   length divided by half the spacing, rounded half to even, at least 2, and one more if that is odd. So
   the zigzag has a whole number of zigs and zags, and it starts and ends on the same side.
2. **Directions.** Point 0's direction is the line to point 1, and the last point's the line from the one
   before it. Every other point's direction is halfway between the line from the point before it and the
   line to the point after it, each taken with length 1, so a point at a corner leans evenly into both of
   its sides. The lines are those between the spread points, not the path's own.
3. **Across.** Each point moves square to its direction, turned by `zigzag_angle` (positive turns
   clockwise as the drawing shows it). The points numbered 0, 2, 4 and on move to the right of the way
   the path goes, by half the width plus the second pull compensation. Points 1, 3, 5 and on move to the
   left, by half the width plus the first. A pull compensation below minus half the width moves its
   points over to the other side. Each point whose direction has no length stays where it is, as on a
   turn straight back.
4. **Closed paths.** A stroke closes when one of its subpaths moves and the others, if any, draw nothing:
   single points, or segments of no length. That subpath's ends must lie nearer each other than 0.05 CSS
   pixels. Its last needle point then moves onto its first, and the zigzag ends where it started.
   Ink/Stitch counts a subpath of no length as a second line, and keeps such a zigzag open
   (`DEV-RUN-008`).
5. **Repeats and bean stitch.** Repeats sew the zigzag again, every other pass backwards, and each pass
   after the first starts with the needle point after the one the last pass ended at. Bean stitch then
   sews each stitch `2b + 1` times, with its list taken one after another along the stitches of all the
   passes, as for a running stitch ([repeats and bean stitch](#repeats-and-bean-stitch)). Ink/Stitch does
   not list the point a pass turns at twice in a zigzag, as it does in a running stitch, so that point
   takes no step of the bean list here.

The needle points come from arithmetic, square roots and the cosine and sine of the angle
(`stitchcraft_core::math`) alone, and every platform places them alike.

**Compared with Ink/Stitch.** Ink/Stitch spreads the points along a running stitch it sews first, at half
each spacing and within `running_stitch_tolerance_mm` of the path. StitchCraft spreads them along the
path, flattened as Ink/Stitch flattens it for that running stitch (`DEV-RUN-007`). At the default spacing
those running stitches are 0.2 mm long, chords of the path's curves, so the two lines and the points
spread along them differ by hundredths of a millimetre. A part whose length lies close to a rounding
boundary of its count of zigs and zags may have one zig and zag more or fewer. With several spacings,
Ink/Stitch's zigzag takes the first, and its running stitch takes half of each, one after another. A long
one cuts across a curve by up to `running_stitch_tolerance_mm`, and the zigzag follows it there, inside
the path. StitchCraft sews the first spacing along the path. A part of the path that is one point is not
stitched (`SC-W0401`), as for a running stitch (`DEV-RUN-004`).

### Properties (conformance)

- The needle points of each part, before repeats, are `n + 1` with `n` even, as step 1 counts them, and
  their places along the flattened path are `L / n` apart (`REQ-RUN-009`).
- Each needle point lies across its direction from its place on the path, turned by `zigzag_angle`:
  even points at half the width plus the second pull compensation to the right, odd points at half the
  width plus the first to the left (`REQ-RUN-010`).
- A closed stroke ends on its first needle point (`REQ-RUN-011`).
- Repeats and bean stitch sew the zigzag as step 5 says, with no step of the bean list for a turn
  (`REQ-RUN-012`).

### Diagnostics

`SC-W0401` for a part of the path that is one point. Finishing leaves out a needle point of each stitch
shorter than the shortest stitch (`SC-I0504`). At the default spacing, 0.4 mm, and a shortest stitch of
0.3 mm, that is every stitch of a straight zigzag narrower than 0.224 mm, and the zigzag sews as a line.
`SC-W0405` names a zigzag with stitches that short, and the width it needs. Ink/Stitch's shortest stitch
is 0.1 mm by default, and it sews such a zigzag as it is.

## Ripple stitch (P3, M10)

A series of copies of the stroke, each progressively scaled, offset or interpolated towards a target
point (or between two rails for "satin-guided ripple"), joined into one continuous running stitch.
Parameters: `line_count`, `min_line_dist_mm`, `staggers`, `skip_start`, `skip_end`, `flip_copies`,
`exponent`, `flip_exponent`, `reverse`, `grid_size_mm`, `grid_first`, `scale_axis`, `scale_start`,
`scale_end`, `rotate_ripples`, `join_style`, `satin_guide_pattern_position`, `reverse_rails`,
`swap_satin_rails`, plus running stitch parameters for each line. Design to be completed at M10; the
properties will include "lines never cross the target point" and "each line is within tolerance of its
interpolated curve".
