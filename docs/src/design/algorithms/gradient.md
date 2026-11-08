# Linear gradient fill

<!-- implements: crates/stitchcraft-engine/src/generators/gradient/** -->

Rows of running stitches across the fill, each in one of the colours of a linear gradient (P3, M10.12).
Thread can't mix colours, so the fill blends them by sharing out rows. Between 2 stops of the gradient, the
first stop's colour takes most rows near it and the second's most rows near the second stop, in a fixed
pattern. Seen from a step away, the 2 colours blend. Each colour's rows sew in that colour's thread.

StitchCraft sews Ink/Stitch's linear gradient fill as Ink/Stitch sews it, read at `d59c9ab`:
`lib/stitches/linear_gradient_fill.py` for the rows and their colours, and the fill element's gradient, its
underlay and its order of colours in `lib/elements/fill_stitch.py`. This page describes them in our own
words (ADR-0012), and the code is written from it. Ink/Stitch was also run as a black box on gradients made
for the purpose, with the cases the page names.

## Parameters

A gradient fill has no settings of its own but `stop_at_ending_point`, which it shares with the tartan
fill. It reads the tatami fill's `row_spacing_mm`, `max_stitch_length_mm`, `staggers`, `skip_last` and
`expand_mm`, its whole underlay, and `angle`, which turns the rows of a fill without a gradient and that
fill's underlay. It reads the running stitch's length and tolerance for the needle's travel, its random
stitch lengths and `random_seed`. Ink/Stitch's gradient reads no pull compensation, end row spacing,
gap-filling rows or `underpath`, and neither does StitchCraft's.

| Key | Meaning | Default |
|---|---|---|
| `stop_at_ending_point` | whether the last 2 colours keep their travel to the fill's end | off |

## The gradient

The gradient is the fill's paint: a start and an end, which the adapters place in the design's millimetres
with every transform applied, and its stops ([data model](../data-model.md#engine-input-design-stitchcraft-engine)).
Each stop has an offset from 0 at the start to 1 at the end, a colour and an opacity. A stop of opacity 0 is
transparent, and the rows it would take are left out. Any other opacity sews its colour. The SVG and
VectorCraft adapters read it ([SVG input](../svg-input.md#gradients),
[VectorCraft integration](../vectorcraft-integration.md#a-gradients-geometry)).

A fill without a linear gradient, such as one painted with a plain colour, sews its own colour fading out,
as below, and `SC-W0329` says so, as Ink/Stitch warns of it.

## Rows

Each part of the fill is grown by `expand_mm` and cut into the pieces that leaves, as a tatami fill's parts
are. Each piece gets rows of its own.

**The way along the gradient** points from its start to its end. Where they are one point, it points
right, along x. The rows lie square to it. Each runs the way along the gradient turned a quarter turn
clockwise, as the canvas shows it with y down: down the canvas when the gradient points right.

**Where the rows lie.** Measured along the gradient, the piece's outline reaches from its nearest point to
its farthest. The rows reach a longest stitch further either way. That reach is cut into equal steps no
longer than the row spacing, and a row lies at each end of each step: row 0 at the near end, and the last
row at the far end. Measured across the gradient, each row reaches a row spacing past the outline at both
ends.

**Needle points.** Along each row, from its first end, row k's needle points lie the longest stitch apart.
The first lies k ÷ `staggers` of a stitch on, less its whole stitches, so each row's points lie a
`staggers`-th of a stitch on from the row before's. A `staggers` of 0 counts as 1. The points stop short of
the row's last end. A row's first end is a needle point too when the first point lies more than half a row
spacing from it, and its last end when the last point does. A row with room for fewer than 2 such points is
its 2 ends. With `enable_random_stitch_length`, a row is a running stitch from its first end to its last at
random lengths, each row drawing from a stream of the element's generator that its number keys. Ink/Stitch
seeds each row's numbers itself, and they differ (`DEV-FILL-004`).

## Each stop's row

Each stop lies at a row. The gradient's start lies at row s. s is how many row spacings the start lies
past row 0, along the gradient and no farther than the rows reach, rounded to the nearest whole number,
halves to the even one. Where that gives 0, s is minus the row spacings from the gradient's start to row 0,
along the gradient and no more than its length, rounded the same way. The steps between rows can be a
little shorter than the row spacing, and the count is in row spacings all the same, as Ink/Stitch counts
it. A stop at offset t lies at row s plus the gradient's length times t, in row spacings and rounded.

A fill without a gradient sews 2 stops: its own colour at row 0, and a transparent one a row past the last.
Its rows lie square to the way at `angle`, turned clockwise as the canvas shows it: at 30° the way points
right and 30° down. The colour fades out along it.

## The colours of the rows

The stops are taken in order, each with its row, and the rows go to their colours:

1. **The first stop** takes rows 0 to its own row, when its row is past row 0.
2. **Each stop after it** blends with the colour before it. The blend starts at the row after the last
   blend, or after the first stop's row, and its length is the rows from there to the stop's row. Where
   that row and the stop's row both lie before row 0, the stop blends nothing, and the next stop blends
   from it.
3. **A blend of 2 rows or fewer** is no blend. The stop is passed over: the next blend starts a row
   further on, which leaves that row out, and it blends from the colour before the passed stop. `SC-W0330`
   says so where the row left out crosses the piece. A gradient whose start and end are one point puts every
   stop at one row, so all but its first are passed over.
4. **After the last blend**, every row left takes the last stop's colour.

A blend lays its rows in a fixed pattern, so it can end a row or 2 before the stop's row, or past it. The
next blend starts where it ended. The blends never give row 0 to a colour. Only the first stop does. A row
past the last is left out, and so are the rows of a transparent stop. Stops of one colour share their rows.

**A blend's pattern.** Say the blend is T rows long, and n is the whole square root of T. Its first half is
laid one row after the other, in rounds 1 to n:

- Round i has i runs, each some rows of the first colour and then one row of the second. The round shares
  n − i rows of the first colour among its runs as evenly as they go, its first runs taking the rows left
  over.
- Half the rows past n², rounded down, go one each to the first runs laid, a row of the first colour more
  each.
- No run has more rows of the first colour than the last run of the round before.
- The half ends at the first row of the first colour laid 2 rows short of half the blend or later: one row
  of the second colour follows it and ends the half.

The second half is the first half backwards, each row in the other colour. So the first colour's runs
shorten towards the middle and the second colour's lengthen after it. A blend of 12 rows, 1 for the first
colour and 2 for the second, is 1 1 1 2 1 2 1 2 1 2 2 2.

**Their order.** Each colour lists its rows as they come: the first stop's rows from row 0, then each
blend's rows of its colour in the first half's order, then those of the second half from its end back to
the middle, then the rows after the last blend. Each colour's rows are routed in that order.

## Sewing

A gradient sews its parts in a tatami fill's order, nearest the needle first ([fills](fills.md#5-parts)).
Each part sews the tatami fill's underlay first, in the colour of the gradient's first stop, transparent or
not. Its rows run along the gradient, square to the top rows, unless `fill_underlay_angle` sets their angles.
A fill without a gradient sews its underlay as a tatami fill's, square to its `angle`.

**The colours** of each piece come in the order the stops first give them, the transparent ones left out.
Each colour's rows are cut where they leave the piece, each piece of row with the needle points inside it.
They are routed as a tatami fill's rows are routed ([routing](fills.md#3-routing-every-segment-once)) and
sewn once, the needle travelling along the piece's outline between them, never under the rows. Every
colour is routed from where the needle was when the piece's colours began, after its underlay, towards the
end the next part or element offers. A colour that leaves the piece's rows out sews nothing.

**Travel left out.** The travel before a colour's first needle point on a row is left out, unless the colour
is the piece's first and the fill has an underlay: then it runs on from the underlay. The travel after its
last such point is left out too. With `stop_at_ending_point` on, the last 2 colours keep it, as Ink/Stitch
counts them. A colour that sews no row point before its last keeps the travel before it, and one with no row
point after its first keeps all but its last point, as Ink/Stitch strips them.

**Several parts.** Each part sews its underlay and its colours in turn. Then, as Ink/Stitch sorts a
gradient's groups, the underlays and colours are ordered by where their colour first comes among them, so
that each thread sews in one stretch.

**Threads and colour blocks.** Each colour of each piece is a colour block of its own thread, which
assembly trims after when the element's `trim_after` is on. The underlay ends no colour block
([several threads in one element](../engine-pipeline.md#several-threads-in-one-element)). A colour of the
element's own thread sews in it, named or not, and the others in threads of their colours alone, as a
tartan's do. The sewing order reads a gradient's threads from its paint and settings alone, before it is
sewn: the first stop's colour for the underlay when it has one, then the colours of the stops that aren't
transparent, in their order.

**Stitches.** A needle point nearer the one before than the element's shortest stitch is left out, as
finalize would leave it out (`DEV-FIN-001`). The budget is charged for the rows and their needle points,
the cuts, the colours' rows and the stitches, before memory is taken for them.

A preview too costly to sew shows every colour's rows inside the pieces, without the stitches.

## How it compares with Ink/Stitch

The differential sews TS-17B's gradients and files of its own in both programs (`REQ-FILL-GRD-009`).
StitchCraft differs where the drawing and Ink/Stitch's sewing part:

- Ink/Stitch works out the underlay's angle from the gradient before it applies the gradient's
  `gradientTransform`, so the underlay of a turned gradient doesn't lie square to its rows. StitchCraft's
  lies along the gradient as the file draws it (`DEV-FILL-024`).
- Where every stop's row lies before row 0, Ink/Stitch takes the rows after the last blend from the far end of
  the rows back, as many as the last blend fell short of row 0, and leaves the rest unsewn. StitchCraft sews
  every row after the last blend in the last stop's colour, as a viewer paints the fill past its last stop
  (`DEV-FILL-024`).

The SVG adapter places a gradient as a viewer does, which Ink/Stitch doesn't for 2 kinds of file
([SVG input](../svg-input.md#gradients)).

## Test sheet

TS-17B's third row sews linear gradients in rectangles 40 mm wide and 30 mm tall, each trimmed after each
colour ([machine testing](../../plan/machine-testing.md)). One blends 2 stops across, one 3 stops at a
slant, and one 3 stops whose middle one is transparent. Every stop's colour is one of Brother's threads, so
both programs' files name each thread as itself.

## Properties (conformance)

| Requirement | Property |
|---|---|
| `REQ-FILL-GRD-001` | The gradient is the fill's paint, its start, end and stops placed by the adapter, and a fill without one fades its own colour out along `angle`, with `SC-W0329` |
| `REQ-FILL-GRD-002` | The rows lie square to the gradient, in equal steps no longer than the row spacing, from a longest stitch before the piece to a longest stitch past it, each a row spacing past the outline at both ends |
| `REQ-FILL-GRD-003` | Each row's needle points lie the longest stitch apart, staggered row by row, its ends kept more than half a row spacing from them |
| `REQ-FILL-GRD-004` | Each stop's row is the gradient's start's row plus its offset's share of the gradient's length, in row spacings, rounded halves to even |
| `REQ-FILL-GRD-005` | The first stop takes the rows up to its own, each blend shares its rows in the fixed pattern, a blend of 2 rows or fewer passes its stop over with `SC-W0330`, and the rows after the last blend take the last colour |
| `REQ-FILL-GRD-006` | The underlay sews in the first stop's colour, along the gradient, then each colour in its own thread, routed along the outline from where the piece's colours began, without its travel at either end |
| `REQ-FILL-GRD-007` | `stop_at_ending_point` keeps the travel of the last 2 colours to the end |
| `REQ-FILL-GRD-008` | Several parts sort their underlays and colours by colour, and the sewing order reads the threads before the fill is sewn |
| `REQ-FILL-GRD-009` | TS-17B's gradients and the differential's own sew in Ink/Stitch as in StitchCraft by every measure |
