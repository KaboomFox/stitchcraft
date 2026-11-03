# Cross stitch

<!-- implements: crates/stitchcraft-engine/src/generators/cross/** -->

A cross in each cell of a grid laid over the fill, where the fill covers enough of the cell (P3, M10.1).
Cross stitch is counted work: hand embroiderers sew it on even-weave fabric, every cross the same size, in
rows and columns. A shape is filled by choosing cells of the grid, so its edges step along the grid's
lines. StitchCraft sews Ink/Stitch's simple crosses, plain and flipped, as Ink/Stitch sews them (read at
`d59c9ab`, `lib/stitches/cross_stitch.py`, `lib/stitches/utils/cross_stitch.py` and the fill element's
cross stitch settings in `lib/elements/fill_stitch.py`, described here in our own words, ADR-0012). The
other crosses, half, upright, double and Smyrna, arrive with M10.2, and the turned grid
(`cross_rotation`) with it.

24 of the 25 cross-stitch fonts in Ink/Stitch's font library sew simple crosses only, on grids of 1.5 to 3
mm with 2 or 4 threads. They draw each letter as rectangles of whole cells, grown by 0.1 mm, on a grid
that starts at each rectangle's corner (`canvas_grid_origin` off).

## Parameters

| Key | Meaning | Default |
|---|---|---|
| `cross_stitch_method` | `simple_cross`, or `simple_cross_flipped` with the `/` diagonal under; the 10 other methods of M10.2 skip the element (`SC-W0011`) | `simple_cross` |
| `cross_thread_count` | passes on each half of each diagonal, 1 to 20; an odd count sews the even count below it, 2 at least | 4 |
| `pattern_size_mm` | the cells' width and height, 1 length or 2 | 3 |
| `canvas_grid_origin` | the grid's lines through the canvas's origin, or through each part's top left corner | on |
| `cross_offset_mm` | the grid moved left by the first length and down by the second, 1 length or 2 | 0 |
| `fill_coverage` | how much of a cell, in percent, the fill must cover for a cross | 50 |
| `max_cross_stitch_length_mm` | the longest stitch; a longer one is split | 11 |
| `expand_mm` | each part grown, or shrunk where negative, before its cells are chosen | 0 |

The pattern size and the offset are 1 length, for both ways, or 2 separated by a space. The registry's
`LengthPair` kind reads exactly that, as it reads a satin's pull compensation for each of its sides, so they
take it. Ink/Stitch reads more than 2 values as its first 2, which StitchCraft refuses (`DEV-PRM-004`). It
reads a pattern size below 0.1 mm as 0.1, an offset below 0 as 0 and a coverage below 1 % as 1 %, and
StitchCraft clamps them so too, with `SC-W0102`. Its coverage is a whole number, and its fonts write it as
`50.0` too, which it sews at its default of 50. StitchCraft reads it as a percentage, so `50.0` is 50
here too (`DEV-PRM-001`).

## The grid

Each part of the fill's area is grown by `expand_mm` first. Grown parts that meet are one part, as Ink/Stitch
grows the whole shape at once. Shrunk, each part is shrunk alone, and a part shrunk away is sewn at its own
size, with `SC-W0309`, as every fill's part is. Ink/Stitch keeps the shape as drawn only when nothing at all
is left of it, and otherwise sews no cross in such a part (`DEV-FILL-024`).

The grid's cells are `pattern_size_mm` wide and tall. Its lines pass through the canvas's origin, so that
cross stitch fills side by side share one grid and their crosses line up. With `canvas_grid_origin` off,
they pass through the top left corner of the part's bounds instead, and the part sews the same wherever it
lies. `cross_offset_mm` moves the lines left by its first length and down by its second, as Ink/Stitch moves
them. A move by a whole cell leaves the grid where it was.

**The canvas's origin.** In an SVG file it is the origin of the page, where Ink/Stitch puts it, and the SVG
reader puts the page's top left corner at the design's origin. In a VectorCraft document it is the origin of
the document's coordinates, which the VectorCraft adapter keeps as the design's origin
([coordinates](../vectorcraft-integration.md#reading-vectorcraft-files-stitchcraft-vectorcraft)). A new document's first
artboard has its top left corner there. The grid so stays put when artboards move, a fill and its neighbours
on other artboards share it, and a document exported to SVG keeps it where its page puts the origin.

Every cell the part's bounds reach is looked at. Ink/Stitch starts at the grid's first line at or above the
part's top, moved down by the offset's second length, and never looks at the row above it. So where the
part's top lies less far below a line than that length, Ink/Stitch loses the row of crosses it covers there.
A 3 mm grid moved 2 mm down over a rectangle whose top lies 0.2 mm below a line leaves out a row the
rectangle covers 60 % of (`DEV-FILL-025`).

## Coverage

A cell sews a cross when the part covers `fill_coverage` percent of it or more, less a ten-thousandth of a
percent of slack, as Ink/Stitch decides it. A cell covered exactly half sews at 50, however the arithmetic
rounds. The covered area is worked out exactly: row by row, each of the part's rings is clipped to the row's
strip, as Sutherland and Hodgman clip a polygon by each of the strip's edges. By Green's theorem, the
area of the clipped rings left of a line down the strip is a sum over their sides, of the area between each
side and the strip's top edge, so each side's share falls to the columns it crosses. The holes run the
other way round, and take their area away. A part that covers no cell so much sews nothing, and
`SC-W0319` says so, as Ink/Stitch warns of it.

Every cell the bounds reach costs a unit of work before any is looked at. A row of cells, held while they are
looked at, and each cell that sews a cross must fit in the stitches left (`REQ-GEN-004`). A grid of 0.1 mm
over a part 1 m wide reaches 10⁸ cells, and ends with `SC-E0004` before it takes memory.

## A cross

Each cross is sewn as a loop from one of its corners back to it. Its `\` diagonal goes under and its `/`
diagonal over, as the drawing shows them, and between the 2 the needle passes through the cell's middle,
where the second diagonal covers it. Each half of each diagonal takes `cross_thread_count` passes of the
thread, the needle going back and forth along it, so the count is even. An odd count sews the even count
below it, as in Ink/Stitch, and 1 sews 2, with `SC-W0320`. Ink/Stitch sews a stitch from a corner of each
cell to its middle for 1, and no cross (`DEV-FILL-027`).

From a corner of its `/` diagonal, the loop runs in to the middle along that diagonal, which it sews last.
It then sews the `\` diagonal back and forth from the corner on the start's side, the top or the bottom,
returns to the middle, and sews the `/` diagonal from its far end back to the start. Nothing of the loop
lies over the `/` diagonal. From a corner of its `\` diagonal, the loop sews that diagonal back and forth
first, ending at its far end, goes in to the middle, sews the `/` diagonal from its top, and returns through
the middle to where it started. That last stitch lies over the `/` diagonal.

`simple_cross_flipped` sews the `/` diagonal first and the `\` diagonal over it, on the same grid. A flipped
cross is a plain cross turned a quarter turn, so the part, its grid, the needle and the end are turned, sewn
as plain crosses, and turned back, exactly. Ink/Stitch turns the part alone and lays its grid afresh over
it. Its flipped cells are then the pattern size's second length wide and its first tall, the offset's
second length moves them right and its first down, and with `canvas_grid_origin` off its grid starts at the
part's bottom left corner (`DEV-FILL-026`). On square cells, without an offset and on the canvas's grid,
both lay the same crosses.

## Sewing the crosses

Crosses that share a corner, side by side or corner to corner, form a group, and each group is one run, from
a corner back to it. The run starts with a cross at its corner, one whose `/` diagonal ends there if it has
one, and goes round its loop. At each corner it reaches, before going on, it sews the loop of each cross not
sewn yet whose `/` diagonal ends there: the cross below and to the left of the corner first, then the one
above and to the right, each sewn the same way as the run goes along it. Such a loop starts along its own
`/` diagonal, so nothing of it lies on top. Where the run has been round and crosses are left, which only
their `\` diagonals' ends join to it, it takes the first corner along it where such a cross has one, the cross
below and to the right first. It sews that loop there, ending on top, and goes on as before. Every corner of
a cross sewn lies on the run, so every cross of the group is sewn.

The groups go in this order:

- The group with the corner nearest the end is sewn last, from that corner, so the part ends there. The end
  is the next part's point nearest this one, or for the last part the next element's offer, or the end
  position.
- The others go first, each the group with the corner nearest the needle, from that corner, the needle being
  where the run before it ended. With no needle, the first is the group of the first cross, row by row from
  the top, from that cross's top right corner.
- A part of one group with both a needle and an end first travels from its corner nearest the needle to its
  corner nearest the end, corner to corner through the middles of the fewest crosses, and sews from there.
  The crosses sewn later cover the travel.

Each run is a group of stitches of its own, which assembly joins to the next with a stitch when it starts
within the element's jump length, and with a jump otherwise, as Ink/Stitch's stitch plan joins its runs. The
parts are sewn nearest the needle first, as a tatami fill's are, and a fill with a start position starts its
first part nearest it and each part after nearest where the part before ended (`DEV-FILL-023`).

**How it compares with Ink/Stitch.** Ink/Stitch builds the same runs, of the same loops. It takes its
groups, and the crosses that meet at a corner, in the order of a Python set of its objects, which follows
where they lie in memory, where StitchCraft's order is fixed. Where the needle and the end lie nearest one
group of several, it sews another group first, from that group's corner nearest the end. And with neither, it
starts each group at the first corner it finds of one cross alone (`DEV-FILL-028`). The crosses and their
stitches are the same in both, so the measures of the differential agree.

## Stitches

Each place of a run is a needle point. A stitch longer than `max_cross_stitch_length_mm` is split into the
fewest equal stitches no longer, as Ink/Stitch's geometry library splits it, and a stitch exactly as long is
not split. A longest stitch below twice the shortest stitch is raised to it, with `SC-W0402`, as a fill's
is, so no piece of a split stitch is shorter than the shortest stitch. A cross's shortest stitches run from
its corners to its middle, half its diagonal. On a grid so fine that they are shorter than the shortest
stitch, `SC-W0321` says so, and a needle point nearer the one before than the shortest stitch is left out,
as finalize would leave it out.

A cross stitch fill has no underlay, no pull compensation and no random stitch lengths, as in Ink/Stitch.
A preview too costly to sew shows each cross's 2 diagonals.

## Test sheet

TS-17A sews squares 24 mm wide on grids of 2 and 3 mm, with 2 and 4 threads, plain and flipped, and a K
whose slanted edges cut their cells in half, with the fonts' 2 threads and 0.1 mm of growth
([machine testing](../../plan/machine-testing.md)). Every sample lies on the canvas's grid, its corner a
multiple of 6 mm from the origin, so that the squares sew whole cells. VectorCraft's Test Sheet filter moves
the sheet by whole multiples of 6 mm, which keep them. The differential compares the sheet with Ink/Stitch's
sewing of it (`REQ-FILL-CRS-007`).

## Properties (conformance)

| Requirement | Property |
|---|---|
| `REQ-FILL-CRS-001` | The grid's cells are the pattern size, 1 length or 2; its lines pass through the canvas's origin, or each part's top left corner, moved left and down by the offset; every cell the part reaches is looked at |
| `REQ-FILL-CRS-002` | A cell sews a cross at `fill_coverage` percent covered, less a ten-thousandth of a percent; a part that covers none so much sews nothing, with `SC-W0319` |
| `REQ-FILL-CRS-003` | Each cross is a loop from a corner back to it, `\` under `/`, or `/` under `\` flipped on the same grid; each half diagonal takes the even thread count, 2 at least, with `SC-W0320` for an odd one |
| `REQ-FILL-CRS-004` | Crosses that share a corner sew in one run; loops join at the corners of their top diagonals first; the group nearest the end goes last, the others nearest the needle first; one group travels from the needle to the end first |
| `REQ-FILL-CRS-005` | A stitch longer than the longest is split into equal stitches, one as long is not; a grid too fine for the shortest stitch says so, with `SC-W0321` |
| `REQ-FILL-CRS-006` | Parts grown into each other are one; a part shrunk away is sewn at its size, with `SC-W0309`; a fine grid over a large part runs out of the budget before it takes memory; M10.2's methods skip the element, with `SC-W0011` |
| `REQ-FILL-CRS-007` | TS-17A's simple crosses sew in Ink/Stitch as in StitchCraft by the differential's measures |
