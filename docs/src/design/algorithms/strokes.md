# Stroke generators

<!-- implements: crates/stitchcraft-engine/src/generators/mod.rs, crates/stitchcraft-engine/src/generators/running/**, crates/stitchcraft-engine/src/generators/manual.rs, crates/stitchcraft-engine/src/generators/zigzag/**, crates/stitchcraft-engine/src/generators/ripple/**, crates/stitchcraft-engine/src/generators/passes.rs, crates/stitchcraft-engine/src/normalize/stroke.rs, crates/stitchcraft-engine/src/normalize/along.rs -->

Strokes follow a path. Shape: `StrokePath` (an ordered list of polylines, corners marked, from the
normalizer). Phase P1 (M3) unless noted.

A stroke starts where it is drawn and ends where it is drawn, as in Ink/Stitch. Its generator is given its
neighbours, as every generator is ([ADR 0014](../adr/0014-generators-see-their-neighbours.md)), and a
stroke sews the same whatever they are, and whatever start and end positions its design gives it
(`REQ-GEN-001`). A satin column or a tatami fill before a stroke ends near the
stroke's first point.

A stroke cut to clips, its SVG clip paths or VectorCraft clipping objects, sews the pieces of its lines
inside every one (`REQ-GEN-005`). Its subpaths are cut where they cross a clip's outline, as Ink/Stitch
cuts them. A running stitch's or a zigzag
stroke's is flattened first as a shape's outline is, and a manual stitch's runs from node to node, with a
needle point at each cut. Each piece sews on its own, in order along its subpath, and a closed subpath that is cut
is cut where it starts too. Left whole, a subpath keeps its curves and sews as drawn. The clips leave the
width alone, so a zigzag stroke's zigzag ends square at a cut, half its width either side of the line.
Ink/Stitch closes each piece of a cut closed zigzag stroke, which StitchCraft sews open (`DEV-RUN-009`), and
sews a line that also touches its clip at a point whole, which StitchCraft cuts as any other
(`DEV-RUN-010`). Wholly outside its clips, a stroke isn't sewn (`SC-W0013`), and the element before it ends
nearest the first piece they leave. A ripple stitch is sewn whole until M10.11 cuts its lines
([ripple stitch](#not-sewn-yet)).

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

**A wide stroke's area**, which automatic digitizing fills ([image to
embroidery](../image-to-embroidery.md#stitch-types-by-width)), is made from the path segment by segment,
flattened within the shapes' tolerance: its joins go where 2 segments meet, and it bends smoothly inside
a curve. Its ends and corners are square to the chords there, and half the stroke's width turns the angle
such a chord makes with its curve into a shift of the corner. So a curve that ends the subpath, or meets
the next segment at a corner, is halved further at that end until the shift is within the tolerance too.
Where 2 segments meet smoothly nothing more is needed.

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
- The fills sewn as one path have repeats and bean stitch of their own, which Ink/Stitch declares for
  its fills apart from a stroke's. Bean stitch comes first, and each repeat then sews everything sewn so
  far again, the other way round, so `k` repeats sew 2^(k−1) passes ([fills](fills.md#stitches)).
- A circular fill's bean stitch leaves out the stitches that run between the pieces of its spiral, but
  each of those stitches takes a step of the list, as in Ink/Stitch
  ([circular fill](fills.md#circular-fill)). A tartan fill takes the fills' bean stitch for its runs
  alone, the needle's travel between them left out the same way, and its rows and its needle's travel
  take the running stitch's length, as a tatami fill's travel does ([tartan fill](tartan.md#a-colours-lines)).

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

A contour fill sews its path stretch by stretch, a ring or a turn of a spiral at a time, each a span of
its own, and keys each span's streams by its number in the fill ([fills](fills.md#stitches)).

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
Before Ink/Stitch's file version 3 a zigzag stroke read `pull_compensation_mm`, shared by its 2 sides, and
in a file of version 1 or 2 the SVG reader halves it into `stroke_pull_compensation_mm`, as Ink/Stitch's
update does (`REQ-SVG-007`).

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
`SC-W0405` names a zigzag with stitches that short, and the width it needs. It counts the stitches that
are sewn. A closed zigzag's last needle point moves onto its first before they are counted. That stitch
can be the only short one, as at a teardrop's tip. Ink/Stitch's shortest stitch is 0.1 mm by default, and
it sews such a zigzag as it is.

## Ripple stitch (M10.8)

A ripple stitch sews lines that spread out from the stroke: copies of its line that shrink towards a
point, or lines along a satin column from one of its rails to the other. All of them are sewn as one
running stitch. It suits light textures that fade to a point, such as a shell. Its lines may cross.
StitchCraft sews it as Ink/Stitch sews its ripple stitch (`stroke_method` `ripple_stitch`),
read at `d59c9ab` (`lib/stitches/ripple_stitch.py` and the ripple settings in `lib/elements/stroke.py`).

**Parameters:** `line_count`, `min_line_dist_mm`, `exponent`, `flip_exponent`, `skip_start`,
`skip_end`, `flip_copies`, `join_style`, `staggers` and `reverse` (`RippleParams`). Each line is sewn
with the running stitch's settings, its length and tolerance, random length and `random_seed`, and the
whole ripple with `repeats` and `bean_stitch_repeats`. Guided ripples arrive with M10.11
([not sewn yet](#not-sewn-yet)).

### The stroke's lines

The stroke's subpaths are flattened as a shape's outline is, within a tenth of a CSS pixel, and each one
that moves is a line. A subpath that is one point is not stitched (`SC-W0401`). Lines shorter together
than the shortest stitch sew nothing, and `SC-W0401` gives their length. The lines say which ripple the
stroke sews:

- **2 lines or more** are a satin column's rails and rungs, read by the satin column's own code
  ([satin](satin.md#recognizing-rails-and-rungs)), and the ripple runs along the column, its lines between
  the rails.
- **1 open line** ripples towards a target point.
- **1 closed line**, whose ends lie within 0.05 CSS pixels of each other, spirals in to the target point.

### The target point

Ink/Stitch's `target_point` command tied to the stroke gives the target point, read as the start and end
commands are read ([SVG input](../svg-input.md#inkstitch-objects)). In a VectorCraft document a path named
`stitch:target` grouped with the stroke gives it, at the middle of the box round its anchors
([VectorCraft integration](../vectorcraft-integration.md#commands-in-v1)). Without either, the target
point is the centroid of the convex hull of the stroke's lines, as GEOS finds a polygon's centroid. Where
the lines' points all lie in a line, the hull is the segment between the 2 farthest apart, and the target
point its middle. One point is its own hull.

### Places

Each line has a place `t`, from 0 to 1. Of `n` lines, line `k` is at `(k / (n − 1))` raised to the power
`exponent`, which is at least 0.1. With `flip_exponent`, line `k` is at 1 less the place of line
`n − 1 − k`. Above 1, the exponent packs the lines together towards the first line, and below 1 towards
the last. With 1 line, its place is 0, or 1 with `flip_exponent`.

- **Along a column.** Pairs of points are placed across the column every `running_stitch_tolerance_mm`,
  as a satin column places its pairs ([satin](satin.md#top-stitches-method-satin_column)), with no
  compensation and nothing at random. The rails are read with a satin column's defaults, cut into
  sections at their rungs. The second rail is turned where it runs against the first (`reverse_rails`
  automatic). The rails keep their order and their length. Line `k` goes through the point `t` of the way from each pair's point on
  the first rail to its point on the second. The first line lies along the first rail and the last along
  the second.
- **Towards a target point.** The line is first sewn as a running stitch with the stroke's stitch lengths
  and tolerance, without random length ([placement](#placement)). Its needle points are the outline. Line
  `k` goes through the point `t` of the way from each point of the outline to the target point: the
  outline shrunk towards the target point by `1 − t`. The first line is the outline itself, and a line at
  `t` 1 is the target point alone. Another line reaches the target point only where the outline does.

### How many lines

1. `line_count` gives the count. With `min_line_dist_mm`, the count is the shortest length among the
   column's pairs, or among the lines from the outline's points to the target point, divided by
   `min_line_dist_mm` and rounded up. The count then keeps the parity of `line_count`, odd or even, by
   being 1 less where it differs, so that the ripple ends on the same side however large the stroke is
   drawn. It is at least 1, and at most 1,000, or 999 where 1,000 would change its parity. `SC-W0407`
   says when it is cut, where Ink/Stitch sews every line (`DEV-RIP-002`). `line_count` itself stops at
   1,000. A pair or a line of length 0 counts as Ink/Stitch counts it: the shortest is taken from those
   after it, and is 0 when it is the last. So a column whose rails meet at its end takes the least count,
   whatever `min_line_dist_mm` says.
2. A closed line and the point join (`join_style` 1) add 1 line, since bending makes one line of 2.
3. A ripple along a column has at least 2 lines.

### Bent lines

A closed line's lines and the point join's bend into the next line. So do the lines along a column without
`flip_copies`. At the outline's point or the column's pair `i` of `m`, line `k` lies `i / (m − 1)` of the
way from its own point to line `k + 1`'s. It starts on its own start and ends on the next line's end. The
point join flips every other line, counted from the first of the count, towards a target point always
and along a column with `flip_copies` on. Such a line lies `1 − i / (m − 1)` of the way, so it starts on
the next line's start and ends on its own end. The last line bends into none and is left out, and the
bent lines are 1 fewer than the count. A closed line's lines, bent, run on from one to the next in a
spiral that ends at the target point.

### Skipping lines

`skip_start` leaves out the first lines and `skip_end` the last. Ink/Stitch leaves out none where they
add up to the number of lines or more, and so does StitchCraft. `SC-W0408` says so.

### Sewing the lines

#### One line at a time

Lines along a column or towards a target point with `flip_copies` on, the default, are sewn one at a time,
each as a running stitch of its own with the stroke's stitch lengths and tolerance. A line of 1 point,
from a column of 1 pair, is left out.

- **Flat join** (`join_style` 0, the default). Every other line, counted from the first sewn, is sewn
  from its end. The ripple goes back and forth. A straight running stitch joins each line's last
  needle point to the next line's first.
- **Point join** (`join_style` 1). The lines are bent, and every other line, counted from the first of
  the count, is sewn from its end. Each line then starts where the one before ended, and nothing joins
  them.
- **Staggers.** With `staggers` 0, a line is placed from the end it is sewn from, and a line of no length
  sews its one point. With any other value, line `i`, counted from the first sewn, is placed from its
  start, `((i / staggers) mod 1)` of the sum of the stitch lengths before it, so that the needle points of
  lines side by side don't line up. The line is lengthened back along its first segment by that much, or
  in proportion less where the segment is shorter than 0.1 CSS pixels, and sewn as a running stitch. The
  needle points before the line's start are left out, and the start is the first needle point. A needle
  point nearer the start than the shortest stitch is left out too. The line is then sewn from its end
  where it turns back. A line of no length sews its one point, but nothing at a phase of 0, as Ink/Stitch
  at `d59c9ab` sews it when run as a black box. `staggers` is 1 when a file sets none. Each line is then
  placed from its start. Ink/Stitch's settings window shows 0.
- With random length, each line draws from its own stream of the element's generator, keyed by its place
  among the lines sewn.

#### One run

A closed line's lines, and the others with `flip_copies` off, are sewn as one running stitch through all
their points in order. Each line is sewn from its start, and the needle goes straight from one line's end
to the next one's start.

#### The whole ripple

`reverse` sews the whole ripple backwards, from its last needle point. `repeats` and
`bean_stitch_repeats` apply as for a running stitch ([repeats and bean stitch](#repeats-and-bean-stitch)).
A line sewn one at a time that starts where the one before ended lists that needle point again, as
Ink/Stitch lists it, and it takes a step of the bean list each time. A ripple starts and ends where its
stitches do, whatever its neighbours. It offers the element before it its first subpath's start, as every
stroke does. Lines near the target point can be shorter than the shortest stitch. Finalize then leaves out
their needle points that lie too near the one before ([finalize](../engine-pipeline.md#5-finalize)).

The lines' points come from arithmetic, square roots and powers (`stitchcraft_core::math`) alone, and
every platform places them alike. Each point of each line costs a unit of work, and the points must fit
in the stitches the budget has left before they are made.

### Not sewn yet

M10.11 sews guided ripples: guide lines and satin guides, `scale_axis`, `scale_start`, `scale_end`,
`rotate_ripples`, `satin_guide_pattern_position`, the anchor line, `grid_size_mm`, `grid_first` and
`manual_pattern_placement`. The registry doesn't declare their settings yet. A stroke keeps them, and
`SC-W0105` names each. The SVG reader leaves a guide line or an anchor line out and names it (`SC-W0802`),
and the ripple sews as this section says. Ink/Stitch reads a column's rails with the element's
`reverse_rails`, `swap_satin_rails` and `push_compensation_mm`. StitchCraft reads them with the defaults,
automatic, false and 0, and `SC-W0406` names each one set otherwise. A ripple cut to clips is sewn whole,
and `SC-W0406` says so. Ink/Stitch cuts its stitches to the clips and routes the pieces.

### Compared with Ink/Stitch

The lines, their places and their count are Ink/Stitch's. The running stitches along each line, along the
joins and along the outline that a ripple towards a target point is made from place their needle points
as StitchCraft's running stitch does (`DEV-RUN-001` to `DEV-RUN-005`). So the outline's points differ a
little, and the copies made from them. With `min_line_dist_mm`, the count towards a target point is
measured from those points, and can differ by 2 where the shortest length falls near a whole number of
`min_line_dist_mm`. Ink/Stitch sews nothing of lines shorter together than 0.1 CSS pixels, and StitchCraft
nothing of lines shorter together than the shortest stitch (`DEV-RUN-004`).

Ink/Stitch starts a staggered line where its stitches have run the phase from the stretch's start,
measured along them. Where the stretch is the whole phase, that is the line's start, or near it where the
stitches cut a bend. Where the first segment is shorter than 0.1 CSS pixels, the stretch is shorter than
the phase, and Ink/Stitch leaves the rest of the phase of the line unsewn. StitchCraft starts every line at
its start (`DEV-RIP-001`).

### Properties (conformance)

- A stroke of 2 lines or more ripples along the column its rails and rungs make, a stroke of 1 open line
  towards its target point, and a stroke of 1 closed line in a spiral to it (`REQ-RIP-001`).
- The target point is the stroke's `target_point` command or `stitch:target` marker, or else the centroid
  of the convex hull of its lines (`REQ-RIP-002`).
- The count is `line_count`, or what `min_line_dist_mm` gives with the parity of `line_count` kept, 1 more
  for a closed line or the point join, at least 2 along a column and at most 1,000 (`REQ-RIP-003`).
- Line `k` lies at its place, `(k / (n − 1))^exponent`, or flipped, between the rails or towards the
  target point. A copy of the outline reaches the target point only at place 1, or where the outline does
  (`REQ-RIP-004`).
- `skip_start` and `skip_end` leave out lines at each end, and the ripple keeps every line when together
  they would leave out all of them (`REQ-RIP-005`).
- With `flip_copies`, the lines are sewn back and forth, joined flat by straight running stitches or at a
  point. Without it, and for a closed line, they are sewn as one running stitch (`REQ-RIP-006`).
- Each line's stitches keep within the running stitch's tolerance of the line through its points
  (`REQ-RIP-007`).
- Staggered lines start their needle points `((i / staggers) mod 1)` of the stitch lengths' sum before
  their start. A line of no length sews its one point, but nothing when staggered at a phase of 0
  (`REQ-RIP-008`).
- `reverse` sews the ripple backwards, then `repeats` and `bean_stitch_repeats` sew it as a running
  stitch's (`REQ-RIP-009`).
- What M10.11 sews is left out with a coded warning, and the ripple sews as this section says
  (`REQ-RIP-010`).
- TS-17C's ripples sew in Ink/Stitch as in StitchCraft (`REQ-RIP-011`).

### Diagnostics

`SC-W0401` for a subpath that is one point and for lines too short for the shortest stitch, `SC-W0406` for
what a ripple doesn't sew yet, `SC-W0407` for a count cut to 1,000 lines and `SC-W0408` for skips that
would leave out every line. A ripple along a column says what the satin column's reading of its rails and rungs
says (`SC-W0202`, `SC-W0203`, `SC-W0205`, `SC-W0207`, `SC-W0210`), and `SC-E0216` when no section of the
column has length.
