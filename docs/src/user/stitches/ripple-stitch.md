# Ripple stitch

<!-- stitch-types: ripple_stitch -->

A ripple stitch sews lines that spread out from a stroke. Copies of its line shrink towards a point, or
lines run along a satin column from one of its edges to the other. Every line is a running stitch, and the
whole ripple sews as one run. Use it for light textures that fade to a point, such as a shell or the rings
round a flower's heart. Set an element's `stroke_method` to `ripple_stitch` for it.

In VectorCraft, set the StitchCraft Stroke effect's `stroke_method` to `ripple_stitch`. In an SVG file, the
line's settings are Ink/Stitch's attributes ([setting parameters](./#setting-parameters)).

TS-17C sews one ripple of each kind:

<!-- shot: ripple-kinds -->
![TS-17C's 3 ripple stitches in green. On the left, a rainbow: 7 arches sewn back and forth from the outer edge of a column drawn as 2 arches and a rung to its inner edge. In the middle, a wave and its copies, which shrink to a point 20 mm above its middle, each copy meeting the next at a point. On the right, a circle 22 mm across that spirals in to its middle.](../../images/generated/ripple-kinds.png)
<!-- /shot -->

## Kinds of ripple

The stroke's lines give the kind of ripple:

- **An open line** ripples towards its target point. Each copy of the line is the one before it shrunk
  towards the point, and the last copy shrinks to the point.
- **A closed line** spirals in to its target point, each turn a smaller copy of the line.
- **2 lines or more** are a satin column's rails and rungs, drawn as for a [satin column](satin-column.md).
  The first line lies on the first rail and the last on the second, and the lines between follow both.

A part of the path that is one point sews nothing, and `SC-W0401` names it.

## The target point

In an SVG file, Ink/Stitch's target point command tied to the stroke places the point the ripple shrinks
to. In a VectorCraft document, a small path named `stitch:target` grouped with the stroke places it, at the
middle of the box round its anchors. Without either, the target point is the middle of the area the line
spans, the centroid of its convex hull. A ripple along a column has no target point.

## How many lines

`line_count` is the number of lines, 10 by default, the stroke's own line and the target point among them.
With `min_line_dist_mm`, the count is the shortest distance from the line to the target point, or across
the column, divided by it. The count keeps the parity of `line_count`, odd or even, so that the ripple ends
on the same side however large it is drawn. A ripple has 1,000 lines at most (`SC-W0407`).

`exponent` spaces the lines. At 1 they lie evenly apart, and above 1 they crowd towards the first line.
`flip_exponent` crowds them towards the last line instead.

<!-- shot: ripple-lines -->
| Evenly spaced | Exponent 2 | Exponent 2, flipped |
|:-:|:-:|:-:|
| ![An arch 30 mm across sewn as a ripple stitch of 8 lines, its copies shrinking to the middle of the half disc it bounds, Evenly spaced](../../images/generated/ripple-lines-1.png) | ![An arch 30 mm across sewn as a ripple stitch of 8 lines, its copies shrinking to the middle of the half disc it bounds, Exponent 2](../../images/generated/ripple-lines-2.png) | ![An arch 30 mm across sewn as a ripple stitch of 8 lines, its copies shrinking to the middle of the half disc it bounds, Exponent 2, flipped](../../images/generated/ripple-lines-3.png) |
<!-- /shot -->

`skip_start` leaves out the first lines, such as the stroke's own line, and `skip_end` the last, such as
the lines that crowd round the target point. Skips that together would leave out every line leave out none
(`SC-W0408`).

## Joins

With `flip_copies` on, the default, every other line is sewn backwards, and the ripple goes back and
forth. `join_style` sets how each line meets the next. With the flat join, `0` and the default, a straight
run of stitches joins each line's end to the next line's start. With the point join, `1`, each line bends
into the next so that they meet at a point, and the ripple has a line more to bend.

With `flip_copies` off, every line is sewn the same way, and the needle goes straight from each line's end
to the next line's start. A spiral is always sewn so.

<!-- shot: ripple-joins -->
| Flat join | Point join | One way |
|:-:|:-:|:-:|
| ![An arch 30 mm across sewn as a ripple stitch of 8 lines, Flat join](../../images/generated/ripple-joins-1.png) | ![An arch 30 mm across sewn as a ripple stitch of 8 lines, Point join](../../images/generated/ripple-joins-2.png) | ![An arch 30 mm across sewn as a ripple stitch of 8 lines, One way](../../images/generated/ripple-joins-3.png) |
<!-- /shot -->

## Staggers

`staggers` moves where each line's needle points fall, so that the needle points of lines side by side
don't line up. Each line's needle points start a staggers-th of the stitch length further back than the
line's before. At 0, every line is placed from the end it is sewn from. The default, 1, places every
line's needle points from its start. Ink/Stitch's settings window shows 0, and it sews a file that sets
none as 1.

## Stitches, repeats and bean stitch

Each line is sewn with the running stitch's `running_stitch_length_mm` and `running_stitch_tolerance_mm`,
and with random stitch lengths when they are on. `reverse` sews the whole ripple backwards. `repeats` and
`bean_stitch_repeats` then sew it as they sew a [running stitch](running.md#bean-stitch-and-repeats).
Lines near the target point can be shorter than the shortest stitch the machine sews well, and StitchCraft
leaves their needle points out when it finishes the plan (`SC-I0504`).

## Not sewn yet

Guided ripples arrive with M10.11. Their copies of the line follow a guide line or a satin guide, and a
grid can cross them. A stroke keeps their settings, and `SC-W0105` names each. A ripple
cut to clip paths is sewn whole, and `SC-W0406` says so. A ripple along a column reads its rails with a
satin column's defaults, and `SC-W0406` names each setting of the column's rails that the stroke sets.

## Parameters

The reference lists each parameter with its range and default:

- [ripple stitch](../reference/params/ripple.md)
- [running stitch](../reference/params/running.md)
- [repeats and bean stitch](../reference/params/repeat.md)
- [the stroke method](../reference/params/stroke.md)
