# Guided fill

<!-- implements: crates/stitchcraft-engine/src/generators/guided/** -->

Rows that bend with a line the designer draws beside the fill (P3, M10.9 and M10.10). The guide line says
how the rows run, so that they can follow a shape that curves. `guided_fill_strategy` says how the rows are
made from it (`REQ-FILL-GDD-001`):

| Strategy | Its rows | Suits |
|---|---|---|
| `0`, copy, the default | Copies of the guide, moved across each part a row spacing at a time | A guide that bends gently, such as a leaf's stem |
| `1`, parallel offset | The guide's offset curves, which keep the row spacing round every bend | A band that turns, such as an arch or a ribbon |
| `2`, buffer | Rings round the guide, every subpath of it | A shape filled outward from a line or a dot inside it |

StitchCraft sews each as Ink/Stitch sews it (read at `d59c9ab`: `lib/stitches/guided_fill.py`,
`lib/stitches/utils/connect_geometries.py`, and the fill element's guided fill settings and its guide and
anchor lines in `lib/elements/fill_stitch.py` and `lib/elements/element.py`, described here in our own
words, ADR-0012). The copy strategy came with M10.9 and the other 2 with M10.10. The strategies differ in
[the guide](#the-guide-line) they take and in their rows. [Stitching](#the-stitches),
[cutting](#cutting-the-rows) and [sewing](#sewing-the-lines) are the same for all.

## The guide line

The guide line is a path grouped with the fill and marked as a guide, and is never stitched itself. In an
SVG file Ink/Stitch's guide-line marker starts it. Of the paths so marked in its group, the fill takes the
first that isn't a satin column, since Ink/Stitch's fill looks past satin guides, which steer ripple
stitches (`REQ-SVG-015`). VectorCraft documents name it `stitch:guide`, the marker a ripple stitch reads
too (`REQ-VC-023`). The copy and parallel offset strategies take the marker's first subpath, and a buffer
takes them all.

For the copy strategy, the engine makes that subpath one open line before it copies it
(`REQ-FILL-GDD-002`):

1. **Flattened** as a fill's outline is.
2. **Made one line.** A copy of a line that crosses itself crosses itself too, and its rows would cross
   each other. So the line is cut into pieces where it crosses or touches itself. A piece that comes back
   to where it began, a loop, is left out, and 2 pieces that meet end to end where no third meets them are
   joined. This is done up to 4 times, until one line is left. A line still in pieces after that, or one
   that runs back along itself, is replaced by the straight line from its first point to its last.
3. **Opened.** A closed line has no line between its ends to copy it by, so its last 2 points are left out:
   the end it closes on and the point before it.
4. **Lengthened** straight on at both ends, along the line from its first point to its last. Each end goes
   out by the larger of the distances from the 2 ends to the corner of the part's bounds farthest from each.
   Every copy then starts and ends outside the part, however far it is moved.

After step 2, a guide of no length, or one drawn there and back, starts and ends at one point. It doesn't
give a line to copy it by, and the fill is sewn as a tatami fill with its own settings, as Ink/Stitch sews a
guided fill without a guide. `SC-W0331` says why (`DEV-FILL-034`). Ink/Stitch moves
each lengthened end by up to 0.01 px at random, and StitchCraft doesn't (`DEV-FILL-035`). The other
strategies say `SC-W0331` for a guide whose points all lie at one place. Ink/Stitch stops with an error on
one with its parallel offset strategy (`DEV-FILL-034`), and sews rings round one with its buffer strategy
(`DEV-FILL-036`).

## The copies

The copies move a row spacing at a time. Without `guided_fill_angle` they move square to the line between
the guide's ends, turned a right angle clockwise from it as the drawing shows it. With it, they move at that
angle, clockwise from the x axis as drawn. Copy `k` lies `k` row spacings from the guide that way, so an
angle near the guide's own direction moves the copies mostly along it, and brings the rows closer together
than the row spacing (`REQ-FILL-GDD-003`).

Where the lengthened guide meets the part, the copies start from it, copy 0. Elsewhere they start from the
copy nearest the part's centroid. Its number counts the whole row spacings, rounded down, from the nearest
point of the lengthened guide to the centroid. The number is negative where the centroid lies behind the
guide as the copies move. From there the copies go on outward, one way and then the other, until one misses
the box round the part. The copies that meet the part are its rows, those that touch its outline among
them. With `smoothness_mm` each copy is smoothed first, as a contour fill's path is smoothed
(`REQ-FILL-GDD-004`).

The rows are then taken in the order of their first points. Of 2 rows, the one with the smaller x comes
first, and then the one with the smaller y. More rows are taken by how far along the diagonal of the box
round their first points each lies, from the box's corner of least x and y. Rows as far along keep the order
they were found in. That order counts each row's stagger and keys its random lengths.

## Parallel offset

A copy of a curved guide lies closer to the guide where the guide runs across the way the copies move, and
the rows crowd there. An offset curve stays the same distance from the guide all along it. On the inside of
a bend tighter than that distance the curve leaves out the loop a copy would make, and the row turns there
with a corner (`REQ-FILL-GDD-009`).

The guide is the first subpath, flattened. A closed guide that doesn't cross itself is offset as the ring it
is. Any other guide is lengthened as a copied guide is, step 4 above, and only then made one line where it
crosses itself, step 2. It isn't opened.

Row `k` is the guide's offset curve at `k` row spacings, with round corners of 16 segments to a quarter
circle, on the side that copies move to. Row 0 lies a ten-thousandth of a CSS pixel off the guide, so that
no offset is 0. The rows start from the row a copy would start from, with the centroid measured square to
the line between the guide's ends. From there they go outward one way and then the other while they meet
the box round the part. `guided_fill_angle` moves only copies. A ring's ends are one point, so the line between them has
no direction, and its rows start at row 0 or at the whole row spacings to the centroid, outward.

A ring is offset as shapely offsets one: as a line that ends where it starts. Shrunk, the curve closes where
the ring's first and last sides meet. Grown, it starts and ends a row's distance from the ring's first point
and leaves out the round corner between. An offset curve can fall into pieces where the guide comes back
beside itself, closer than the offset. Each piece that meets the part is a row.

The rows that [close inside the part](#rows-that-close-inside-the-part) are joined first. The rows are then
taken in the copies' order, by their first points.

## Buffer

A guide inside a shape is a middle the rows grow out from: each row runs round the guide at its own
distance, like the rings of a tree. Every subpath counts: the rings round 2 short lines grow apart and
then join into one (`REQ-FILL-GDD-010`).

Each subpath is flattened and simplified within a ten-thousandth of a CSS pixel, which leaves out only
points in line with their neighbours. The subpaths are then set in the order shapely calls normal. Each
runs from its lesser end, by x and then y, compared inward from both ends where the ends are one point. The
subpaths go greatest first, compared point by point the same way. That order says where each ring starts.

Row `k` is the boundary of the area within `(k + ½)` row spacings of the guide, the first a ten-thousandth
of a CSS pixel farther. The area has round ends and round corners of 16 segments to a quarter circle. Its
outlines run clockwise as shapely reads them, with y up, and its holes the other way. Each ring starts at
its point nearest the first point of the curve round the guide's first subpath, which is that subpath's
second point moved to its left. The rows start at the whole row spacings, rounded down, that the guide lies
from the part, 0 where they meet, and go outward only. They stop at the first boundary that misses the box
round the part. Every ring of a boundary that meets the part is a row, and the rows keep that order.

A subpath whose points all lie at one place is left out, where Ink/Stitch sews rings round the dot
(`DEV-FILL-036`). Should a boundary's edges not join into rings, the rows stop there and `SC-E0009` says so.
No design is known to do that.

## Rows that close inside the part

A row that meets the part's outline has its ends there, and the needle comes to it along the outline as
it comes to a tatami fill's row. A ring round a guide inside the shape has no ends. Sewn as it is, it would be
a stretch of its own that the needle jumps to. So a parallel offset's and a buffer's rings are spliced
into the rows round them, and the fill sews as one route (`REQ-FILL-GDD-011`).

**Sorted out.** The outline ring is the part's outline without its holes. A row that meets it is an outline
row. One that closes and starts inside the part is moved to start at the middle of its first stretch
outside the part. Every other row lies inside and is an inner ring. It is closed if it isn't, and one of
fewer than 4 points is first cut into thirds of its length. Without inner rings the rows stay as they are.
Without outline rows, the part's own rings take their place.

**Ring into ring.** The inner rings go smallest first, by area. Each goes into the first larger one that
covers it. Both are turned to run counter-clockwise as shapely reads it, and a walk goes along the inner
ring from its start, a CSS pixel at a time. Each step takes the point there and the point a row spacing on,
with their nearest points on the outer ring. Where both pairs lie less than 2½ row spacings apart, the
rings are joined. The outer ring is started at the nearest point of the second pair and loses its last row
spacing. The inner ring is started at its second point and loses its last row spacing too. It follows
the outer ring backwards. With `skip_last` the inner ring is first cut into stitches no longer than the longest
stitch, and its 2 ends are left out. A ring that the walk doesn't find a place for isn't sewn, and the rings
already joined into it go with it.

**Ring into row.** Each ring left is joined into the first outline row within a row spacing of it, with a
thousandth of a CSS pixel to spare. The walk goes along the ring as before and measures to the row's
stretches inside the part, until both points lie within 1½ row spacings. The row is opened there for a row
spacing, from the earlier of the 2 nearest points along it. The ring is started at the walk's first point
and loses its first row spacing. It goes in the gap. Should the joined row cross itself, the ring goes in the
other way round.

The ring is done when the row it joined meets the outline ring. A row that only meets a hole's ring isn't
enough. The ring, now part of that row, is joined on into the next row within a row spacing. A next row
that starts inside the part takes it as before. Any other is crossed with it into 2 rows where they come
nearest: each row runs to that place and goes on along the other, a row spacing apart. A ring with no row
near is tried again after the others, at most 5 more times, since another ring's join may bring a row near.
After that it is left out.

## The stitches

Row `i`, from 0 in the order the rows are taken, is smoothed again by `smoothness_mm` and stitched as
`stitch_position_method` says (`REQ-FILL-GDD-005`). It keeps its first point, and its needle points start
`i / staggers` of a longest stitch along it, with its whole turns left out. With 4 staggers, the second row
starts a quarter of a stitch on, the third half a stitch on, and the fifth where the first did. Staggers of
0 count as 1, where Ink/Stitch divides by them and stops (`DEV-FILL-033`). Each row is lengthened well past
the part. Its needle points keep their place inside it.

A row's corners are where Ink/Stitch's running stitch takes them. A join that turns by 45° or more is one,
unless it lies within twice `running_stitch_tolerance_mm` along the row of the turn before it. The gentler
joins where a copy meets its lengthened ends aren't corners. The stagger runs on through them.

- **Flexible**, the default, spreads a row's stitches evenly between its corners, at most the longest
  stitch each. The rows' needle points keep their stagger near the start of each row, and drift from it
  where a row's stitches are spread.
- **Flexible (stagger)** sews whole longest stitches from where the stagger starts the row, a needle point
  on each corner, and the last stitch whatever is left. The stagger then lasts the whole row long.

With random lengths on, the stitches are drawn as a running stitch draws them, from a stream of the
element's seed that the row's number keys, whatever the position method. Either way, a stitch that strays
from its row by more than the tolerance is split there, as a running stitch's is.

### Rows that close, and the anchor line

A buffer's stagger doesn't count the row's number. It counts the whole row spacings, rounded half to even,
that the row lies from the guide, so that rings at one distance start alike. Staggers of 0 count as 1 here
in Ink/Stitch too.

Where a closed row starts decides where its needle points start, and where the cut by the part splits it.
A row that closes without crossing itself, and starts inside the part or has an anchor line, is started
elsewhere before it is stitched (`REQ-FILL-GDD-012`):

- **Along the anchor line.** The anchor line is a path grouped with the fill and marked as one, as the
  guide is. It has Ink/Stitch's anchor-line marker, the first so marked that isn't a satin column, or the
  name `stitch:anchor`. Every subpath of it counts. Where it crosses or touches the row, the row is started at a
  crossing and cut at the others, and each piece is stitched on its own from where it starts. A buffer's
  rings, joined into one row, are so cut back into rings where an anchor line crosses them all, and each
  takes the stagger of its own distance. The needle points then line up along the anchor line, where
  without one each ring's start depends on where it was joined.
- **Outside the part.** A row the anchor line doesn't meet starts at the middle of its first stretch
  outside the part's outline ring, and is stitched from there. A row with no such stretch isn't stitched:
  its own points are its needle points, as in Ink/Stitch.

A row the anchor line started, and any stitched row that starts inside the part, is then started again at
the middle of its first stretch outside the outline ring, so that the cut starts there. A row that doesn't
close is closed for that, as shapely closes a line it rolls.

A ripple stitch reads the same anchor line, from the same field of the design's hints, and takes the ends
of its first subpath ([guided ripples](strokes.md#guided-ripples)).

## Cutting the rows

The stitched row is cut by the part (`REQ-FILL-GDD-006`). It is cut at every point where it crosses one of
the part's rings or touches one, but not where it runs along one. For such a stretch Ink/Stitch's geometry
library gives a line, and Ink/Stitch cuts at points only. A stretch between 2 cuts, or between a cut and the
row's end, is a line of the fill when the part contains its middle fifth, from 40 % to 60 % of its length.
That fifth lies nowhere outside the part, and partly inside it, off the rings. Each line runs the row's way
and keeps the row's needle points between its cuts.

## Sewing the lines

A guided fill sews its parts as a tatami fill sews them, nearest the needle first, each with the tatami
fill's underlay before its lines and grown by `expand_mm` for them (`REQ-FILL-GDD-007`). The shared part loop
does that ([section 5](fills.md#5-parts)). Its lines are routed as a circular fill's pieces are, each a row
of the tatami fill's route between its ends ([a circular fill's sewing](fills.md#sewing)). Each is sewn
once with all its needle points, from the end the route enters it by, and without its last with `skip_last`.
From line to line the needle goes as a tatami fill's does, under the lines not sewn yet with `underpath`
on. The lines it follows under them are turned from `guided_fill_angle`, or from 0 when it is empty, as
Ink/Stitch turns them.

With `bean_stitch_repeats`, the stitches shorter than the shortest stitch are left out first, so that each
stitch the bean stitch sews back over is one the machine sews. Then each stitch of the lines is sewn back and
forth, and the travel between them is not.

## Fallbacks

The rows can miss a part thinner than the row spacing between 2 of them. Such a part is sewn as a tatami
fill at the angle of the line between the ends of the guide's first subpath. Ink/Stitch falls back to one
where its rows meet a part but the cut keeps none of them. Where no copy or offset curve meets the part at
all, it stops with an error first (`DEV-FILL-034`), and with the buffer strategy it falls back there too.
The tatami fill's rows lie the guided fill's row spacing apart, in its longest stitch with its staggers and
`skip_last`. It has no end spacing, random lengths, pull compensation or rows that fill gaps, which
Ink/Stitch's fallback doesn't pass on. It doesn't keep the turns between its rows as stitches either:
`turn_stitches` is a tatami fill's own setting ([tatami fill](fills.md)). `SC-W0332` counts those parts, and `SC-W0305` those too thin for the
tatami rows too, which are sewn round their outline.

## Preview

A preview too costly to sew shows the segments the lines inside each part run along. A part that falls
back to a tatami fill shows its rows. The preview is given the element's random generator, so that random
lengths cut the lines where sewing would.

## Cost

Each point of the guide, of its rows and of their stitches charges a unit of work. An offset curve and a
buffer's boundary cost as the offset module's curves and areas do ([offsets](offset.md)). A parallel offset
costs far more than a copy, since each of its curves is built as a satin column's rail is, from the edge
of a stroke round the whole guide. A square of 100 mm with rows 0.25 mm apart along a guide of one curve charged
133 million units, a quarter of an element's default budget, against 1.6 million for its copies and 23
million for a buffer's rings. A guide of many more points on a fill that large can spend the budget, and
the element is then skipped with `SC-E0004`. Each step of a
walk that joins a ring charges a unit, and each nearest point a unit for each side it measures. Looking for where the
guide meets itself tests only the sides whose spans along x overlap, a unit for each pair. Testing a copy
against the part, and cutting a row by it, look only at the sides of the rings near each stitch. Sewing
charges a stitch for each needle point, and a preview's budget allows any number of them.

## Properties (conformance)

| Requirement | Property |
|---|---|
| `REQ-FILL-GDD-001` | Each strategy sews its rows. A fill without a usable guide sews its own tatami fill with `SC-W0331`. A part the rows miss falls back to a tatami fill with `SC-W0332` |
| `REQ-FILL-GDD-002` | The guide is its first subpath made one open line, lengthened at both ends past the part |
| `REQ-FILL-GDD-003` | The copies move square to the guide's ends or at `guided_fill_angle`, starting from the guide where it meets the part and elsewhere from the copy nearest the centroid |
| `REQ-FILL-GDD-004` | The copies that meet the part are its rows, smoothed, in the order of their first points |
| `REQ-FILL-GDD-005` | Each row is staggered by a `staggers`-th of a stitch from the one before, and stitched as `stitch_position_method` places its needle points |
| `REQ-FILL-GDD-006` | The rows are cut where they cross the rings, and the stretches whose middle fifth the part contains are the lines |
| `REQ-FILL-GDD-007` | The lines are routed as a tatami fill's rows after its underlay, the travel under them turned from the copy angle, and bean stitch leaves the travel out |
| `REQ-FILL-GDD-008` | TS-17B's leaf sews in Ink/Stitch as in StitchCraft by the differential's measures |
| `REQ-FILL-GDD-009` | Parallel offset's rows are the guide's offset curves, a row spacing apart, from the row a copy would start from |
| `REQ-FILL-GDD-010` | A buffer's rows are the rings round every subpath of the guide, half a row spacing and then whole ones out, from the guide's distance outward |
| `REQ-FILL-GDD-011` | Rows that close inside the part are joined into the rows round them, and on into a row that meets the outline |
| `REQ-FILL-GDD-012` | A closed row starts where the anchor line crosses it or outside the part, and a buffer's stagger counts the row's distance from the guide |
| `REQ-FILL-GDD-013` | TS-17B's fan and egg sew in Ink/Stitch as in StitchCraft by the differential's measures |
