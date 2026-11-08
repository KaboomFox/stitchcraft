# Guided fill

<!-- implements: crates/stitchcraft-engine/src/generators/guided/** -->

Rows that bend with a line the designer draws beside the fill (P3, M10.9). The guide line says how the
rows run, so that they can follow a shape that curves, and the fill copies it across each part a row spacing
at a time. StitchCraft sews
Ink/Stitch's copy strategy as Ink/Stitch sews it (read at `d59c9ab`, `lib/stitches/guided_fill.py` and the
fill element's guided fill settings in `lib/elements/fill_stitch.py`, described here in our own words,
ADR-0012). Ink/Stitch's parallel offset and buffer strategies come with M10.10. Until then a fill set to one
of them is skipped, and `SC-W0011` names the strategy (`REQ-FILL-GDD-001`).

## The guide line

A guide line is a path grouped with the fill and marked as one, which is never stitched itself. An SVG
file marks it with Ink/Stitch's guide-line marker. A fill takes the first such path of its group that is
no satin column, as Ink/Stitch's fill looks past satin guides, which steer ripple stitches
(`REQ-SVG-015`). A VectorCraft document names it `stitch:guide`, the marker a ripple stitch reads too
(`REQ-VC-023`). The fill copies the marker's first subpath.

The engine makes that subpath one open line before it copies it (`REQ-FILL-GDD-002`):

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

A guide that starts and ends at one point after step 2, such as a guide of no length or one drawn there and
back, gives no line to copy it by. The fill is then sewn as a tatami fill with its own settings, as
Ink/Stitch sews a guided fill without a guide, and `SC-W0331` says why (`DEV-FILL-034`). Ink/Stitch moves
each lengthened end by up to 0.01 px at random, and StitchCraft doesn't (`DEV-FILL-035`).

## The copies

The copies move a row spacing at a time. Without `guided_fill_angle` they move square to the line between
the guide's ends, turned a right angle clockwise from it as the drawing shows it. With it, they move at that
angle, clockwise from the x axis as drawn. Copy `k` lies `k` row spacings from the guide that way, so an
angle near the guide's own direction moves the copies mostly along it, and brings the rows closer together
than the row spacing (`REQ-FILL-GDD-003`).

Where the lengthened guide meets the part, the copies start from it, copy 0. Elsewhere they start from the
copy that lies nearest the part's centroid: its number is the distance from the centroid to the nearest point
of the lengthened guide in whole row spacings, rounded down, and negative where the centroid lies behind the
guide as the copies move. From there the copies go on outward, one way and then the other, until one misses
the box round the part. Each copy that meets the part, its outline included, is a row. With `smoothness_mm`
each copy is smoothed first, as a contour fill's path is smoothed (`REQ-FILL-GDD-004`).

The rows are then taken in the order of their first points. Of 2 rows, the one with the smaller x comes
first, and then the one with the smaller y. More rows are taken by how far along the diagonal of the box
round their first points each lies, from the box's corner of least x and y. Rows as far along keep the order
they were found in. That order counts each row's stagger and keys its random lengths.

## The stitches

Row `i`, from 0 in the order the rows are taken, is smoothed again by `smoothness_mm` and stitched as
`stitch_position_method` says (`REQ-FILL-GDD-005`). It keeps its first point, and its needle points start
`i / staggers` of a longest stitch along it, with its whole turns left out. With 4 staggers, the second row
starts a quarter of a stitch on, the third half a stitch on, and the fifth where the first did. Staggers of
0 count as 1, where Ink/Stitch divides by them and stops (`DEV-FILL-033`). Each row is lengthened well past
the part, so its needle points keep their place inside it.

A row's corners are where Ink/Stitch's running stitch takes them: joins that turn by 45° or more, at least
twice `running_stitch_tolerance_mm` along the row from the last such turn. The gentler joins where a copy
meets its lengthened ends are no corners, so the stagger runs on through them.

- **Flexible**, the default, spreads a row's stitches evenly between its corners, at most the longest
  stitch each. The rows' needle points keep their stagger near the start of each row, and drift from it
  where a row's stitches are spread.
- **Flexible (stagger)** sews whole longest stitches from where the stagger starts the row, a needle point
  on each corner, and the last stitch whatever is left. The stagger then holds the whole row long.

With random lengths on, the stitches are drawn as a running stitch draws them, from a stream of the
element's seed that the row's number keys, and the position method changes nothing. A stitch that strays
from its row by more than the tolerance is split there, as a running stitch's is.

## Cutting the rows

The stitched row is cut by the part (`REQ-FILL-GDD-006`). It is cut at every point where it crosses one of
the part's rings or touches one, but not where it runs along one: Ink/Stitch's geometry library gives such a
stretch as a line, and Ink/Stitch cuts at points only. A stretch between 2 cuts, or between a cut and the
row's end, is a line of the fill when the part holds its middle fifth, from 40 % to 60 % of its length. None
of that fifth may lie outside the part, and some of it must lie inside, off the rings. Each line runs the
row's way and keeps the row's needle points between its cuts.

## Sewing the lines

A guided fill sews its parts as a tatami fill sews them, nearest the needle first, each with the tatami
fill's underlay before its lines and grown by `expand_mm` for them (`REQ-FILL-GDD-007`). The shared part loop
does that ([section 5](fills.md#5-parts)). Its lines are routed as a circular fill's pieces are, each a row
of the tatami fill's route between its ends ([a circular fill's sewing](fills.md#sewing)). Each is sewn
once with all its needle points, from the end the route enters it by, and without its last with `skip_last`.
The needle travels between lines as a tatami fill's does, under the lines not sewn yet with `underpath` on.
The lines under them are turned from `guided_fill_angle`, or from 0 when it is empty, as Ink/Stitch turns
them.

With `bean_stitch_repeats`, the stitches shorter than the shortest stitch are left out first, so that each
stitch the bean stitch sews back over is one the machine sews. Then each stitch of the lines is sewn back and
forth, and the travel between them is not.

## Fallbacks

A part no copy of the guide meets, one thinner than the row spacing between 2 copies, is sewn as a tatami
fill at the angle of the line between the guide's ends, as Ink/Stitch falls back to one there. Its rows lie
the guided fill's row spacing apart, in its longest stitch with its staggers and `skip_last`. It has no end
spacing, random lengths, pull compensation or rows that fill gaps, which Ink/Stitch's fallback doesn't pass
on. `SC-W0332` counts those parts, and `SC-W0305` those too thin for the tatami rows too, which are sewn
round their outline.

## Preview

A preview too costly to sew shows the lines inside each part, as the segments they run along, or the tatami
rows a part falls back to. It is given the element's random generator, so that random lengths cut the lines
where sewing would.

## Cost

Each point of the guide, of its copies and of their stitches charges a unit of work. Looking for where the
guide meets itself tests only the sides whose spans along x overlap, a unit for each pair. Testing a copy
against the part, and cutting a row by it, look only at the sides of the rings near each stitch. A preview's
budget holds any number of needle points, and sewing charges a stitch for each.

## Properties (conformance)

| Requirement | Property |
|---|---|
| `REQ-FILL-GDD-001` | The copy strategy sews, the others are skipped with `SC-W0011`, a fill without a usable guide sews its own tatami fill with `SC-W0331`, and a part no copy meets falls back to a tatami fill with `SC-W0332` |
| `REQ-FILL-GDD-002` | The guide is its first subpath made one open line, lengthened at both ends past the part |
| `REQ-FILL-GDD-003` | The copies move square to the guide's ends, clockwise as drawn, or at `guided_fill_angle`, from the guide where it meets the part or from the copy nearest the centroid |
| `REQ-FILL-GDD-004` | The copies that meet the part are its rows, smoothed, in the order of their first points |
| `REQ-FILL-GDD-005` | Each row is staggered by a `staggers`-th of a stitch from the one before, and stitched as `stitch_position_method` places its needle points |
| `REQ-FILL-GDD-006` | The rows are cut where they cross the rings, and the stretches whose middle fifth the part holds are the lines |
| `REQ-FILL-GDD-007` | The lines are routed as a tatami fill's rows after its underlay, the travel under them turned from the copy angle, and bean stitch leaves the travel out |
| `REQ-FILL-GDD-008` | TS-17B's leaf sews in Ink/Stitch as in StitchCraft by the differential's measures |
