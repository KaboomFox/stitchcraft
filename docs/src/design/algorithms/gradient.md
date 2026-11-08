# Linear gradient fill

<!-- implements: crates/stitchcraft-engine/src/generators/gradient/** -->

Rows of running stitches across the fill, each in one of the colours of a linear gradient (P3, M10.12).
Thread can't mix colours, and the fill blends them by sharing out rows instead. Between 2 stops of the
gradient, the first stop's colour takes most rows near it and the second's most rows near the second stop,
in a fixed pattern. Seen from a step away, the 2 colours blend. Each colour's rows sew in that colour's
thread.

StitchCraft sews Ink/Stitch's linear gradient fill as Ink/Stitch sews it, read at `d59c9ab`:
`lib/stitches/linear_gradient_fill.py` for the rows and their colours, and `lib/elements/fill_stitch.py` for
the fill element's gradient, its underlay and its order of colours. What follows puts that behaviour in our
own words (ADR-0012), and the generator is written from these words. Ink/Stitch was also run as a black box
on gradients made for the purpose, with the cases below.

## Parameters

A gradient fill has one setting of its own, `stop_at_ending_point`, which it shares with the tartan fill.
It reads the tatami fill's `row_spacing_mm`, `max_stitch_length_mm`, `staggers`, `skip_last` and
`expand_mm`, its whole underlay, and `angle`, which turns the rows of a fill without a gradient and that
fill's underlay. It reads the running stitch's length and tolerance for the needle's path along the outline,
its random stitch lengths and `random_seed`. Ink/Stitch's gradient fill takes no pull compensation, end row
spacing, gap-filling rows or `underpath`, and StitchCraft's doesn't either.

| Key | Meaning | Default |
|---|---|---|
| `stop_at_ending_point` | whether the last 2 colours keep their stitches along the outline to the fill's end | off |

## The gradient

A linear gradient fill sews the gradient its shape is painted with. The gradient has a start and an end in
the design's millimetres, every transform applied, and stops
([data model](../data-model.md#engine-input-design-stitchcraft-engine)). The SVG and
VectorCraft adapters place it ([SVG input](../svg-input.md#gradients),
[VectorCraft integration](../vectorcraft-integration.md#a-gradients-geometry)). Each stop has an offset from 0
at the start to 1 at the end, a colour and an opacity. A stop of opacity 0 is transparent, and the rows it
would take are left out. Any other opacity sews its colour.

A fill without a linear gradient sews its own colour fading out, as below. One painted with a plain colour
is such a fill. `SC-W0329` says so, as Ink/Stitch warns of it.

## Rows

The fill's parts are grown by `expand_mm` and cut into pieces as a tatami fill's parts are
([fills](fills.md#5-parts)). Each piece gets rows of its own.

**The way along the gradient** points from its start to its end. Where the 2 are one point, the way is
along x, to the right. The rows lie square to it. Each runs the way along the gradient turned a quarter turn
clockwise, as the canvas shows it with y down: down the canvas when the gradient points right.

**Where the rows lie.** Measured along the gradient, the piece's outline lies between its nearest point and
its farthest. The rows start a longest stitch before the nearest and end a longest stitch past the farthest.
That length is cut into equal steps no longer than the row spacing, and a row lies at each end of each step:
row 0 at the near end, and the last row at the far end. Measured across the gradient, each row starts a row
spacing before the outline and ends a row spacing past it.

**Needle points.** Along each row, from its first end, row k's needle points lie the longest stitch apart.
The first lies k ÷ `staggers` of a stitch on, less its whole stitches. Each row's points then lie a
`staggers`-th of a stitch on from the row before's. A `staggers` of 0 counts as 1. The points stop short of
the row's last end. A row's first end is a needle point too when the first point lies more than half a row
spacing from it, and its last end when the last point does. A row with room for fewer than 2 such points is
its 2 ends. With `enable_random_stitch_length`, a row is a running stitch from its first end to its last at
random lengths, each row drawing from a stream of the element's generator that its number keys. Ink/Stitch
seeds each row's numbers itself, and they differ (`DEV-FILL-004`).

## Each stop's row

Each stop lies at a row. The gradient's start lies at row s, the row spacings from row 0 to the start along
the gradient, no more than the rows' whole length, rounded to the nearest whole number, halves to the even
one. Where that gives 0, s is minus the row spacings from the gradient's start to row 0, no more than the
gradient's length, rounded the same way. The steps between rows can be a little shorter than the row spacing.
Ink/Stitch counts in row spacings all the same, and so does StitchCraft. A stop at offset t lies at row s
plus the gradient's length times t, in row spacings and rounded.

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
3. **A blend of 2 rows or fewer** is no blend. The stop is passed over, and the next blend starts a row
   further on. That leaves a row out, and the next stop blends from the colour before the passed one.
   `SC-W0330` says so where the row left out crosses the piece. A gradient whose start and end are one point
   puts every stop at one row, and all but its first are passed over.
4. **After the last blend**, every row left takes the last stop's colour.

A blend lays its rows in a fixed pattern, and it can end a row or 2 before the stop's row, or past it. The
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
- The half ends at the first row of the first colour laid 2 rows short of half the blend or later. One row
  of the second colour follows it and ends the half.

The second half is the first half backwards, each row in the other colour. The first colour's runs shorten
towards the middle, and the second colour's lengthen after it. A blend of 12 rows, 1 for the first colour
and 2 for the second, is 1 1 1 2 1 2 1 2 1 2 2 2.

**Their order.** Each colour lists its rows as they come. The first stop's rows come from row 0. A blend's
rows of a colour follow, its first half's in their order and its second half's from its end back to the
middle. The rows after the last blend come last. Each colour's rows are routed in that order.

## Sewing

A gradient sews its parts in a tatami fill's order, nearest the needle first ([fills](fills.md#5-parts)).
Each part sews the tatami fill's underlay first, in the colour of the gradient's first stop, transparent or
not. Its rows run along the gradient, square to the top rows, unless `fill_underlay_angle` sets their angles.
A fill without a gradient sews its underlay as a tatami fill's, square to its `angle`.

**The colours** of each piece come in the order the stops first give them, the transparent ones left out.
The outline cuts each colour's rows, and each piece of row keeps the needle points inside it. A colour's
pieces of row are routed as a tatami fill's rows are routed ([routing](fills.md#3-routing-every-segment-once))
and sewn once. Between them the needle follows the piece's outline, never under the rows. The routes of a
piece all start where the needle was when its colours began, after its underlay, and head for the end the
next part or element offers. A colour that leaves the piece's rows out sews nothing.

**Ends left out.** The stitches before a colour's first needle point on a row are left out, unless the colour
is the piece's first and the fill has an underlay: then it runs on from the underlay. The stitches after its
last such point are left out too. With `stop_at_ending_point` on, the last 2 colours keep them, as Ink/Stitch
counts them. A colour with no needle point on a row before its last point keeps the stitches before it. One
with none after its first keeps all but its last point, as Ink/Stitch strips them.

**Several parts.** Each part sews its underlay and its colours. As Ink/Stitch sorts a gradient's groups, the
underlays and colours are then ordered by where their colour first comes among them. Each thread then sews
in one stretch.

**Threads and colour blocks.** Each colour of each piece is a colour block of its own thread, which
assembly trims after when the element's `trim_after` is on. The underlay ends no colour block
([several threads in one element](../engine-pipeline.md#several-threads-in-one-element)). A colour of the
element's own thread sews in it, named or not, and the others in threads of their colours alone, as a
tartan's do. The sewing order reads a gradient's threads from its paint and settings alone, before it is
sewn. The first stop's colour comes first when the fill has an underlay, then the colours of the stops that
aren't transparent, in their order.

**Stitches.** A needle point nearer the one before than the element's shortest stitch is left out, as
finalize would leave it out (`DEV-FIN-001`). The first colour sews on from the underlay in one thread. Its
run starts a shortest stitch from where the underlay ended, as does any run that sews on from the one before
in its thread. The
budget is charged for the rows and their needle points, the cuts, the colours' rows and the stitches, before
memory is taken for them.

A preview too costly to sew shows each colour's rows inside the pieces, without the stitches.

## How it compares with Ink/Stitch

The differential sews TS-17B's gradients and 6 files of its own in both programs (`REQ-FILL-GRD-009`).
Their colour blocks are the same, in the same threads, with the same trims and jumps, but where the
ledger explains otherwise. StitchCraft differs where the drawing and Ink/Stitch's sewing part:

- Ink/Stitch works out the underlay's angle from the gradient before it applies the gradient's
  `gradientTransform`. The underlay of a turned gradient then doesn't lie along it. StitchCraft's lies along
  the gradient as the file draws it (`DEV-FILL-031`).
- Where every stop's row lies before row 0, Ink/Stitch counts the rows the last blend's end lies before
  row 0. It sews that many rows from the far end back in the last colour and leaves the rest unsewn.
  StitchCraft sews every row after the last blend in the last stop's colour, as a viewer paints the fill
  past its last stop (`DEV-FILL-032`).

The SVG adapter places a gradient as a viewer does, which Ink/Stitch doesn't for 2 kinds of file
([SVG input](../svg-input.md#gradients)). Ink/Stitch counts the rows' steps in pixels. Under a gradient
across a square 40 mm a side, the rows span 46 mm, a whole number of row spacings. There Ink/Stitch's
rounding can give one step more, and its rows lie a little closer. TS-17B's gradients slant, which keeps
them off that edge.

## Test sheet

TS-17B's third row sews linear gradients in squares 40 mm a side, each trimmed after each colour
([machine testing](../../plan/machine-testing.md)). From left to right, red blends into blue up to the right
and deep gold through white into emerald green down to the right. The last blends tangerine into Prussian
blue down to the left, through a transparent white. Every stop's colour is one of Brother's threads, so both programs' files name each thread
as itself. Each gradient's own thread is its first stop's colour, unnamed, as the stops' threads are.

## Properties (conformance)

| Requirement | Property |
|---|---|
| `REQ-FILL-GRD-001` | The gradient is the fill's paint, its start, end and stops placed by the adapter, and a fill without one fades its own colour out along `angle`, with `SC-W0329` |
| `REQ-FILL-GRD-002` | The rows lie square to the gradient, in equal steps no longer than the row spacing, from a longest stitch before the piece to a longest stitch past it, each a row spacing past the outline at both ends |
| `REQ-FILL-GRD-003` | Each row's needle points lie the longest stitch apart, staggered row by row, its ends kept more than half a row spacing from them |
| `REQ-FILL-GRD-004` | Each stop's row is the gradient's start's row plus its offset's share of the gradient's length, in row spacings, rounded halves to even |
| `REQ-FILL-GRD-005` | The first stop takes the rows up to its own, each blend shares its rows in the fixed pattern, a blend of 2 rows or fewer passes its stop over with `SC-W0330`, and the rows after the last blend take the last colour |
| `REQ-FILL-GRD-006` | The underlay sews in the first stop's colour, along the gradient, then each colour in its own thread, routed along the outline from where the piece's colours began, without its stitches before its first row and after its last |
| `REQ-FILL-GRD-007` | `stop_at_ending_point` keeps the stitches of the last 2 colours along the outline to the end |
| `REQ-FILL-GRD-008` | Several parts sort their underlays and colours by colour, and the sewing order reads the threads before the fill is sewn |
| `REQ-FILL-GRD-009` | TS-17B's gradients and the differential's own sew in Ink/Stitch as in StitchCraft by every measure |
