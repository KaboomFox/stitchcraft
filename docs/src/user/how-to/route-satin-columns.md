# Route satin columns

A design drawn column by column jumps from the end of each column to the start of the next, and each jump
leaves a thread to trim. StitchCraft Auto-Route Satin Columns, a filter in VectorCraft, sews the same
columns in an order of its own, as Ink/Stitch's Auto-Route Satin Columns does. It cuts each column where
the route needs. A run along a column's middle goes under the satin that later covers it, and the needle
jumps only where the columns lie apart.

## What routing changes

TS-17C shows it. Its top half is the word ROUTE in satin letters, sewn column by column. Its bottom half is
the same letters routed with the filter's options at their defaults. Dashed lines are jumps:

<!-- shot: routing-letters -->
![TS-17C's letters ROUTE drawn as thin lines, twice: at the top as drawn, and at the bottom as Auto-Route Satin Columns routes them. Dashed lines are jumps, which the reference machine leaves as threads to cut by hand, and orange rings mark lock stitches. The routed letters jump only from one letter to the next.](../../images/generated/routing-letters.png)
<!-- /shot -->

At the top, the needle jumps to each letter and to each arm of the E. At the bottom, it sews each letter in
one go, its strokes joined by runs hidden under the satin. It jumps only from one letter to the next.
`stitch testsheet TS-17C` writes the sheet, to sew both halves side by side.

## Route a selection in VectorCraft

The filter is a heavy one, from VectorCraft 0.17: install `stitchcraft-route-satins.wasm` with **Object ›
Plug-ins › Install Plug-in…** and allow it ([install](../install.md)). StitchCraft Satin and StitchCraft
Stroke must be installed too, since the route's pieces have their effects.

1. Give each column the satin effect itself, with **Effect › Plug-ins › StitchCraft Satin**. A column drawn
   with 2 rails and rungs, or drawn as one path as wide as its stroke, both route. A path that sews by its
   group's effect stays as it is.
2. If the needle should run between columns along lines of your own, give those lines **Effect ›
   Plug-ins › StitchCraft Stroke**.
3. To choose where the route starts and ends, draw a small path at each place and name them
   `stitch:route-start` and `stitch:route-end`. Without them, the route runs from the leftmost end of a
   column to the rightmost.
4. Select the columns, the strokes and the markers, and choose **Object › Plug-ins › StitchCraft Auto-Route
   Satin Columns**.

The route replaces the paths in one step, which **Edit › Undo** takes back. Each piece is an object of its
own, named as Ink/Stitch names it, such as AutoSatin 1 or AutoSatin Running Stitch 2. Each column's piece
has the satin effect with the column's settings, and each run the stroke effect. The pieces go where the
topmost path was, in the order they sew, and the paths and markers are gone.

## The options

The dialog offers Ink/Stitch's options:

- **Trim jump stitches**, on at first: the thread is trimmed after the last piece, and after a piece that a
  jump follows when the jump runs more than 1 mm outside the shapes it joins. A run right after a trim is
  left out, as the needle no longer needs it.
- **Preserve order of satin columns**: the paths are sewn in the order they are drawn, each joined to the
  next by its shortest jump. The route adds only the cuts and runs it needs, and each path's pieces take
  its place.
- **Keep original paths**: the paths and the markers stay as they are, and the route goes above them.

## When it refuses

The filter leaves the document as it was and says why (`SC-E0820`) when no selected path has the satin
effect of its own, or when 2 objects of the selection mark the start, or 2 the end. A selection too large
for the work a filter may do stops with `SC-E0004`: route fewer columns at a time.
