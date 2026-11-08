# Route running stitches

A design drawn line by line jumps from the end of each line to the start of the next, and each jump
leaves a thread to trim. StitchCraft Auto-Route Running Stitch, a filter in VectorCraft, sews the same
lines in an order of its own, as Ink/Stitch's Auto-Route Running Stitch does. It cuts the lines where they
cross, so that the route can turn there. Where the route has to pass a stretch twice, it runs along it
first under the stitches it sews over it later. It jumps only where the lines lie apart.

## What routing changes

TS-17C shows it, below its satin letters. On the left, 6 lines are sewn as drawn. Along the bottom lies a
rail, with a post across it and a brace from the post down to the rail. A hook curves up from the rail, a
line that crosses itself lies above them, and a tick starts 0.5 mm below the rail. On the right, the same
lines are routed with Trim jump stitches on. Dashed lines are jumps:

<!-- shot: routing-lines -->
![TS-17C's network of 6 running stitches drawn as thin lines, twice: on the left as drawn, and on the right as Auto-Route Running Stitch routes them with Trim jump stitches on. A rail along the bottom, a post across it, a brace from the post down to the rail, a hook curving up from the rail, a line that crosses itself above them, and a tick beside the rail. Dashed lines are jumps, and orange rings mark lock stitches. As drawn, the machine jumps from each line to the next. Routed, it jumps only out to the line apart and back, and trims before each of those jumps. It reaches the tick, 0.5 mm off the rail, with a stitch.](../../images/generated/routing-lines.png)
<!-- /shot -->

On the left, the needle jumps from each line to the next. On the right, it sews along the rail and turns
up each branch, running out along it under and back on top. It jumps only to the line apart and back. The
lines on TS-17C are bean-stitched, and the runs under them are not, so they hide under the bolder stitches.
To sew the network as drawn beside it routed, write the sheet with `stitch testsheet TS-17C`, its letters
above.

## Route a selection in VectorCraft

Auto-Route Running Stitch is a heavy filter too, from VectorCraft 0.17. Install
`stitchcraft-route-runs.wasm` as you install the [satin columns' filter](route-satin-columns.md), and allow
it ([install](../install.md)). StitchCraft Stroke must be installed as well, since the route's lines have
its effect.

1. Give each line the stroke effect itself, with **Effect › Plug-ins › StitchCraft Stroke**, whatever its
   stroke method. A path that sews by its group's effect stays as it is, and so does a satin column.
2. Mark the route's start and end as for satin columns. A small path named `stitch:route-start` marks the
   start, and one named `stitch:route-end` the end. Without them, the route runs from the leftmost point of
   a line to the rightmost.
3. Select the lines and the markers, and choose **Object › Plug-ins › StitchCraft Auto-Route Running
   Stitch**.

The route replaces the lines in one step, which **Edit › Undo** takes back. Each stretch is a line of its
own with the stroke effect, named as Ink/Stitch names it, AutoRun 1 for a stretch on top or AutoRun
Underpath 2 for a run under the others. A stretch on top has the settings of the line it comes from, but an
even number of repeats is lowered by 1, so that it ends where the next one starts. Runs under the others
keep only the line's stitch length and tolerance, without its repeats or bean stitch. The stretches go
where the topmost line was, in the order they sew, and the lines and markers are gone.

## The options

The dialog offers Ink/Stitch's options:

- **Add nodes at intersections**, on at first: the lines are cut where they cross or touch, and where 2
  pass within 13.2 mm of each other, at their nearest points, so that the route can turn or jump there. Turn
  it off for lines you have drawn with a node of both wherever they meet. The route then meets them only
  there.
- **Preserve order of running stitches**: the lines are sewn in the order they are drawn, each joined to
  the next by its shortest jump. The route adds only the runs under the others it needs, and each line's
  stretches take its place.
- **Trim jump stitches**, off at first: the thread is trimmed after a stretch that a jump longer than
  0.75 mm follows. A run under the others right after a trim is left out, as the needle no longer needs it.

## When it refuses

The filter leaves the document as it was and says why (`SC-E0820`) when no selected path has the stroke
effect of its own, or when 2 objects of the selection mark the start, or 2 the end. A selection too large
for the work a filter may do stops with `SC-E0004`: route fewer lines at a time. Of more than 1,000 lines,
counting each subpath, none is cut where it meets another (`SC-W1004`), and the route meets them only at the
nodes they share.
