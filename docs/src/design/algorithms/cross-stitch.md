# Cross stitch

<!-- implements: crates/stitchcraft-engine/src/generators/cross/** -->

A cross in each cell of a grid laid over the fill, where the fill covers enough of the cell (P3, M10.1 and
M10.2). Cross stitch is counted work. Hand embroiderers sew it on even-weave fabric, every cross the same
size and in rows and columns. A shape is filled by choosing cells of the grid, and its edges step along the
grid's lines. StitchCraft sews Ink/Stitch's 12 cross stitch methods as Ink/Stitch sews them (read at
`d59c9ab`, `lib/stitches/cross_stitch.py`, `lib/stitches/cross_stitch_half.py`,
`lib/stitches/utils/cross_stitch.py` and the fill element's cross stitch settings in
`lib/elements/fill_stitch.py`, described here in our own words, ADR-0012). Simple, upright, dense upright,
double and Smyrna crosses are full crosses, a stack of bars through each cell's middle, sewn in runs of
loops. Half crosses sew one diagonal in each cell, in rows.

24 of the 25 cross-stitch fonts in Ink/Stitch's font library sew simple crosses only, on grids of 1.5 to 3
mm with 2 or 4 threads. They draw each letter as rectangles of whole cells, grown by 0.1 mm, on a grid
that starts at each rectangle's corner (`canvas_grid_origin` off). Jersey 15, the 25th, sews its 238
elements as double crosses of 2 threads, grown so and on such a grid, of the default 3 mm cells.

## Parameters

| Key | Meaning | Default |
|---|---|---|
| `cross_stitch_method` | `simple_cross`, `upright_cross`, `dense_upright_cross`, `double_cross`, `smyrna_cross` or `half_cross`, each also as its flipped variant, `_flipped` after its name | `simple_cross` |
| `cross_thread_count` | passes on each half of each bar of a full cross, 1 to 20, or along each stitch of a half cross's rows. A full cross sews an odd count as the even count below it, 2 at least, and a half cross an even count as the odd count above it | 4 |
| `pattern_size_mm` | the cells' width and height, 1 length or 2 | 3 |
| `canvas_grid_origin` | the grid's lines through the canvas's origin, or through each part's top left corner | on |
| `cross_offset_mm` | the grid moved left by the first length and down by the second, 1 length or 2 | 0 |
| `cross_rotation` | the grid turned by this angle, clockwise as drawn for a positive one | 0 |
| `fill_coverage` | how much of a cell, in percent, the fill must cover for a cross | 50 |
| `max_cross_stitch_length_mm` | the longest stitch, past which a stitch is split, and the length of a half cross's travel stitches | 11 |
| `expand_mm` | each part grown, or shrunk where negative, before its cells are chosen | 0 |

The pattern size and the offset are 1 length, for both ways, or 2 separated by a space. The registry's
`LengthPair` kind reads exactly that, as it reads a satin's pull compensation for each of its sides, and they
are of that kind. Ink/Stitch takes the first 2 of more values, where StitchCraft leaves the element out with
`SC-E0101` (`DEV-PRM-004`). Ink/Stitch reads a pattern size below 0.1 mm as 0.1, an offset below 0 as 0 and a
coverage below 1 % as 1 %, and StitchCraft clamps them so too, with `SC-W0102`. Its coverage is a whole
number, and its fonts write it as `50.0` too, which it sews at its default of 50. StitchCraft reads it as a
percentage, and `50.0` is 50 here too (`DEV-PRM-001`). The rotation is an angle of the registry's `Angle`
kind, read in (−180°, 180°], so a whole turn is no turn.

## The grid

Each part of the fill's area is grown by `expand_mm` first. Grown parts that meet are one part, as Ink/Stitch
grows the whole shape at once. Shrunk, each part is shrunk alone, and a part shrunk away is sewn at its own
size, with `SC-W0309`, as every fill's part is. Ink/Stitch keeps the shape as drawn only when nothing at all
is left of it, and otherwise sews no cross in such a part (`DEV-FILL-024`).

The grid's cells are `pattern_size_mm` wide and tall. Its lines pass through the canvas's origin, so that
2 fills of cross stitch that lie side by side share one grid and their crosses line up. With
`canvas_grid_origin` off, they pass through the top left corner of the part's bounds instead, and the
part sews the same wherever it lies. `cross_offset_mm` moves the lines left by its first length and down
by its second, as Ink/Stitch moves them. A move by a whole cell leaves the grid where it was.

**The canvas's origin.** In an SVG file it is the origin of the page, where Ink/Stitch puts it, and the SVG
reader puts the page's top left corner at the design's origin. In a VectorCraft document it is the origin of
the document's coordinates, which the VectorCraft adapter keeps as the design's origin
([coordinates](../vectorcraft-integration.md#reading-vectorcraft-files-stitchcraft-vectorcraft)). A new document's first
artboard has its top left corner there. Moving an artboard then leaves the grid where it is, a fill and its
neighbours on other artboards share it, and a document exported to SVG keeps it where its page puts the
origin.

Every cell within the part's bounds is looked at. Ink/Stitch starts at the grid's first line at or above the
part's top, moved down by the offset's second length, and never looks at the row above it. Where the part's
top lies less far below a line than that length, Ink/Stitch loses the row of crosses the part covers there. A
3 mm grid moved 2 mm down over a rectangle whose top lies 0.2 mm below a line leaves out a row the rectangle
covers 60 % of (`DEV-FILL-025`).

### The turned grid

`cross_rotation` turns the grid. The part is turned the other way, the grid is laid over the part so turned
as above, the crosses are sewn there, and their stitches are turned back. On the canvas's grid the turn is
about the point (x, y) that the offset's lengths name, as Ink/Stitch turns it, where the grid's own lines
pass through (−x, y). Fills side by side with the same turn and offset then share one turned grid. With
`canvas_grid_origin` off the grid starts at the turned part's top left corner, so the point the grid turns
about changes nothing but rounding. StitchCraft turns such a grid about the canvas's origin. Ink/Stitch turns
it about the part's bottom left corner, or about the point that a file's `inkscape:transform-center-x` and
`-y` name.

That is how lettering along a path turns a glyph of a cross-stitch font (M14.3). Ink/Stitch's lettering
turns each glyph to follow the path and sets its elements' `cross_rotation` to the same angle, so a letter
drawn as whole cells keeps its crosses square to its sides. A whole number of quarter turns turns each
point exactly, and a whole turn turns nothing.

## Coverage

A cell sews a cross when the part covers `fill_coverage` percent of it or more. Ink/Stitch allows a
ten-thousandth of a percent of slack, and StitchCraft allows the same. A cell covered exactly half sews at 50,
however the arithmetic rounds. The covered area is worked out exactly, row by row. Each of the part's rings is clipped to the row's
strip, as Sutherland and Hodgman clip a polygon by each of the strip's edges. By Green's theorem, the area of
the clipped rings left of a line down the strip is a sum over their sides, of the area between each side and
the strip's top edge. Each side's share then falls to the columns it crosses. The holes run the other way
round, and take their area away. A part that covers no cell so much sews nothing, and `SC-W0319` says so, as
Ink/Stitch warns of it.

Dense upright crosses look at a second grid too, its lines half a cell left of and above the first grid's,
and a cell of it sews a cross by the same rule.

Every cell within the bounds costs a unit of work before any is looked at. A row of cells, held while they are
looked at, and each cell that sews a cross must fit in the stitches left (`REQ-GEN-004`). A grid of 0.1 mm
over a part 1 m wide holds 10⁸ cells, and ends with `SC-E0004` before it takes memory.

## Full crosses

A full cross is a stack of bars through its cell's middle, sewn from the bottom up:

| Method | Bars, from the bottom up | Good joins | Bad joins |
|---|---|---|---|
| simple | `\`, `/` | the corners of `/` | the corners of `\` |
| upright | the bar across, the bar down | the middles of the top and bottom sides | the middles of the left and right sides |
| double | `\`, the bar across, the bar down, `/` | the corners of `/` | the corners of `\` |
| Smyrna | `\`, `/`, the bar across, the bar down | the middles of the top and bottom sides | the 4 corners |

The bar across runs from the middle of the cell's left side to the middle of its right side, and the bar down
from the middle of its top side to the middle of its bottom side. A dense upright cross adds upright crosses
on the second grid. Each has its middle on a corner of the first grid, and its arms end at the middles of
the sides of the first grid's cells, where the first grid's arms end too.

A cross joins the run that sews it at its joins. Its good joins are the ends of its top bar. A loop from one
of them goes in to the middle along the top bar, which covers that stitch later. The loop then leaves
nothing on top. From a bad one, the loop ends with a stitch from the middle back to it, on top of the bars.

### A loop

Each cross is sewn as a loop from one of its joins back to it. Each half of each bar takes
`cross_thread_count` passes of the thread, the needle going back and forth along the bar, and the count is
even. An odd count sews the even count below it, as in Ink/Stitch, and 1 sews 2, with `SC-W0320`. Ink/Stitch
sews a stitch from a corner of each cell to its middle for 1, and no cross (`DEV-FILL-027`). With n the count's
half:

- From an end of the top bar, the loop goes in to the middle along the top bar. It then sews the bars below
  the top one from the bottom up, each n times back and forth from its first end and back to the middle, and
  ends with the top bar n times back and forth from its far end to the start.
- From an end of the bottom bar, the loop sews that bar back and forth from the start, ending at its far end,
  and goes to the middle. It sews the bars above it from the bottom up as above, and ends with the stitch from
  the middle to the start.
- From an end of a bar between, a Smyrna cross's `/` diagonal, the loop goes in to the middle along that bar
  and sews the bars from the bottom up. That bar goes from the middle to its far end and back and forth 1 time
  fewer, ending there. The loop ends with the stitch from the middle to the start.

A bar's first end is the one the needle goes to first from the middle. For the `\` diagonal it is the top
end, or the bottom end in a loop from the bottom left corner. For the `/` diagonal it is the top end, but a
Smyrna cross's bottom end, unless its loop starts at the bottom right corner. The bar across starts from its
left end. The bar down starts from its top end, but a Smyrna cross's from its bottom end in a loop from the
top left corner. So each loop is the one Ink/Stitch sews from that join, needle point for needle point.

**Flipped crosses** lie on the same grid. A flipped cross is a plain cross turned a quarter turn, each bar in
the place of the bar a quarter turn from it, so that a simple cross's `/` diagonal goes under its `\` one and
an upright cross's bar down under its bar across. The part, its grid, the needle and the end are turned a
quarter turn, sewn as plain crosses, and turned back, exactly. Ink/Stitch turns the part alone and lays its
grid afresh over it. Its flipped cells are then the pattern size's second length wide and its first tall, the
offset's second length moves them right and its first down, and with `canvas_grid_origin` off its grid starts
at the part's bottom left corner (`DEV-FILL-026`). On square cells, without an offset and on the canvas's
grid, both lay the same crosses.

### Sewing the crosses

Crosses that share a join form a group: simple, double and Smyrna crosses side by side or corner to corner,
upright crosses side by side, and the 2 grids' dense upright crosses through the ends of the arms they share.
Each group is one run, from a join back to it. The run starts with a cross at its join, one whose good join it
is if it has one, and goes round its loop. At each place along the run, before going on, it sews the loop of
each cross not sewn yet whose good join lies there. Of several, the cross lower down goes first, and of 2
level with each other the one further right, each loop sewn the same way as the run goes along it. Such a loop
leaves nothing on top. Where the run has been round and crosses are left, which only their bad joins join to
it, it takes the first place along it where such a cross has one, in the same order. It sews that loop there,
ending on top, and goes on as before. Every join of a cross sewn lies on the run, and every cross of the group
is sewn.

The groups go in this order:

- The group with the join nearest the end is sewn last, from that join, and the part ends there. The end is
  the next part's point nearest this one, or for the last part the next element's offer, or the end position.
- The others go first, each the group with the join nearest the needle, from that join, the needle being
  where the run before it ended. With no needle, the first is the group of the first cross, row by row from
  the top, from that cross's first good join clockwise from its top left: a simple or double cross's top right
  corner, an upright or Smyrna cross's top middle.
- A part of one group with both a needle and an end first runs from its join nearest the needle to its join
  nearest the end through the middles of the fewest crosses. Its loops start from there, and the crosses sewn
  later cover these stitches.

Each run is a group of stitches of its own, which assembly joins to the next with a stitch when it starts
within the element's jump length, and with a jump otherwise, as Ink/Stitch's stitch plan joins its runs. The
parts are sewn nearest the needle first, as a tatami fill's are, and a fill with a start position starts its
first part nearest it and each part after nearest where the part before ended (`DEV-FILL-023`).

## Half crosses

A half cross sews one diagonal in each cell, `\`, or `/` for `half_cross_flipped`, on the same grid. The
needle's way from cell to cell lies bare. So the diagonals that run on from cell to cell are joined into
rows, which are routed as a tatami fill's rows, as Ink/Stitch routes its half crosses (read at `d59c9ab`,
`lib/stitches/cross_stitch_half.py`):

1. **Pieces.** Cells that share a side or a corner are one piece, and each piece is sewn on its own. The
   pieces nearest the needle go first, as a fill's parts do, each ending at its point nearest the next.
2. **Outline.** A piece's rings are the sides of its cells that no other cell of it shares, joined end to
   end, the piece on their left as a fill's rings have it. Where 2 cells of the piece meet only at a corner,
   the ring runs through the corner from the one to the other, as Ink/Stitch's union of the cells, grown by a
   hundred-thousandth of a CSS pixel, joins them. Each ring starts at its top left corner.
3. **Rows.** Each cell's diagonal is split into the fewest equal stitches no longer than
   `max_cross_stitch_length_mm`, and diagonals that run on from cell to cell are one row. A `\` row runs from
   its top end and a `/` row from its bottom end, the rows in the order of their first cells, row by row
   from the top.
4. **Route.** The rows are routed as a tatami fill's rows with `underpath` off (`REQ-FILL-TAT-001`), each sewn
   once. Between them the needle is sent along the rings in running stitches of `max_cross_stitch_length_mm`,
   within 0.2 CSS pixels of the way, as in Ink/Stitch's half crosses. Those stitches lie on the outline of the
   cells, where nothing covers them.
5. **Passes.** Each stitch of a row is sewn there and back again until it lies an odd number of times, as
   bean stitch sews it, and the travel lies once. Ink/Stitch takes half of `cross_thread_count` as the bean
   stitch's repeats. So an odd count is the number of times, and an even count sews the odd count above it,
   4 sewing 5, with `SC-W0322`.

The tatami fill's route differs from Ink/Stitch's where its own ledger entries say (`DEV-FILL-005`,
`DEV-FILL-006`, `DEV-FILL-011`). Ink/Stitch's grown union rounds each outer corner of the outline by its
hundred-thousandth of a pixel. Its way between rows then cuts such a corner by up to its tolerance, 0.05 mm.
Where no row joins a hole's ring to another ring, Ink/Stitch would leave out the rows its walk finds no way
to, where StitchCraft's needle goes to them along the rows.

## Stitches

Each place of a run is a needle point. A stitch longer than `max_cross_stitch_length_mm` is split into the
fewest equal stitches no longer, as Ink/Stitch's geometry library splits it, and a stitch exactly as long is
not split. A longest stitch below twice the shortest stitch is raised to it, with `SC-W0402`, as a fill's
is, and every piece of a split stitch is at least the shortest stitch. A needle point nearer the one before
than the shortest stitch is left out, as finalize would leave it out. On a grid so fine that the method's
shortest stitches are shorter than the shortest stitch, `SC-W0321` says so:

- a simple cross's run from a corner to its middle, half its diagonal,
- an upright, dense upright, double or Smyrna cross's run from the middle of a side to its middle, half
  the cell's shorter side,
- a half cross's stitches between rows, each a side of a cell.

A cross stitch fill sews its crosses alone, without the underlay, pull compensation or random stitch lengths
of a tatami fill, as in Ink/Stitch. A preview too costly to sew shows each cross's bars, or each half cross's
diagonal.

## How it compares with Ink/Stitch

The crosses, their loops and the stitches of each are the same in both, and the measures of the differential
agree. These differ:

- **Orders.** Ink/Stitch takes its groups, and the crosses that meet at a place, in the order of a Python set
  of its objects, which follows where they lie in memory, where StitchCraft's order is fixed. Where the needle
  and the end lie nearest one group of several, it sews another group first, from that group's join nearest
  the end. And with neither, it starts each group at the first good join it finds of one cross alone
  (`DEV-FILL-028`).
- **Cells covered exactly half.** On a file whose `viewBox` is in millimetres, Ink/Stitch writes the drawing's
  scale with 6 decimals, 3.779528 pixels to the millimetre, against 3.7795275… for its grid. Its drawing is
  then a ten-millionth larger than its grid, about the page's origin. A cell that a part covers exactly half
  is then covered less than the slack allows on the part's left and top sides, some 13 mm or more from the
  origin, and more on the right and bottom sides. So a dense upright square drawn on the grid loses the second grid's
  crosses along its left and top sides in Ink/Stitch, and keeps those along its right and bottom sides.
  StitchCraft reads the drawing exactly, and sews all 4 sides. Grown by 0.1 mm, as Ink/Stitch's fonts grow
  their letters, the cells along every side are covered more than half, and both sew them.

## Test sheet

TS-17A sews squares 24 mm wide on grids of 2 and 3 mm, with 2 and 4 threads, plain and flipped, and a K
whose slanted edges cut their cells in half, with the fonts' 2 threads and 0.1 mm of growth. A third row sews
upright, dense upright, double and Smyrna crosses on the 3 mm grid, and a fourth half crosses, plain and
flipped, and a square turned 30° with its grid ([machine testing](../../plan/machine-testing.md)). Every sample
but the turned one lies on the canvas's grid, its corner a multiple of 6 mm from the origin, so that the
squares sew whole cells. VectorCraft's Test Sheet filter moves the sheet by whole multiples of 6 mm, which
keep them, and the turned square's grid starts at its own corner. The differential compares the sheet with
Ink/Stitch's sewing of it (`REQ-FILL-CRS-007`, `REQ-FILL-CRS-012`), and the case `inkstitch-cross-methods`
adds each method's flipped variant, turned grids, and half crosses in pieces, round a hole and through a
corner.

## Properties (conformance)

| Requirement | Property |
|---|---|
| `REQ-FILL-CRS-001` | The grid's cells are the pattern size, 1 length or 2. Its lines pass through the canvas's origin, or each part's top left corner, moved left and down by the offset. Every cell within the part's bounds is looked at |
| `REQ-FILL-CRS-002` | A cell sews a cross at `fill_coverage` percent covered, less a ten-thousandth of a percent, and a part that covers none so much sews nothing, with `SC-W0319` |
| `REQ-FILL-CRS-003` | Each simple cross is a loop from a corner back to it, `\` under `/`, or `/` under `\` flipped on the same grid. Each half diagonal takes the even thread count, 2 at least, with `SC-W0320` for an odd one |
| `REQ-FILL-CRS-004` | Crosses that share a corner sew in one run, and loops join at the corners of their top diagonals first. The group nearest the end goes last and the others nearest the needle first. One group runs from the needle to the end first |
| `REQ-FILL-CRS-005` | A stitch longer than the longest is split into equal stitches, and one as long is not. A grid too fine for the shortest stitch says so, with `SC-W0321` |
| `REQ-FILL-CRS-006` | Parts grown into each other are one, and a part shrunk away is sewn at its size, with `SC-W0309`. A fine grid over a large part runs out of the budget before it takes memory |
| `REQ-FILL-CRS-007` | TS-17A's simple crosses sew in Ink/Stitch as in StitchCraft by the differential's measures |
| `REQ-FILL-CRS-008` | Upright crosses sew the bar across under the bar down, and dense ones add the second grid's. They join at the middles of their sides, the 2 grids through their arms' ends |
| `REQ-FILL-CRS-009` | Double crosses sew an upright cross between their diagonals, and Smyrna crosses one over them, each loop as the table of joins gives it |
| `REQ-FILL-CRS-010` | Half crosses sew one diagonal in each cell, in rows routed as a tatami fill's, each stitch an odd number of times, with `SC-W0322` for an even count |
| `REQ-FILL-CRS-011` | `cross_rotation` turns the grid, about the offset's point on the canvas's grid, and a part turned with its grid sews its crosses turned |
| `REQ-FILL-CRS-012` | TS-17A's other samples and the methods' case sew in Ink/Stitch as in StitchCraft by the differential's measures |
