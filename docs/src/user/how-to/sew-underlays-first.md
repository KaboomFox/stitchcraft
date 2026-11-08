# Sew underlays first where columns cross

Where 2 satin columns cross, the second column's underlay is sewn over the first column's top stitches.
The crossing then stacks top stitches, underlay and top stitches again. `underlays_first` sews the
underlays of elements next to each other in one thread before any of their top stitches, so that every
underlay lies under every top stitch. It is StitchCraft's own setting, off by default. Ink/Stitch sews a
file that sets it as if it didn't, each underlay just before its top stitches
([underlays first](../../design/engine-pipeline.md#underlays-first)).

## What it changes

Below, a bar and then a post, each with a contour and a zigzag underlay, sew as drawn and with their
underlays first. Their stitches lie 1.2 mm apart, wider than usual, to let the underlays show:

<!-- shot: underlays-first -->
| As drawn | Underlays first |
|:-:|:-:|
| ![2 red satin columns 5 mm wide and 16 mm long that cross, a bar and a post, each with a contour and a zigzag underlay, sewn with their stitches 1.2 mm apart so that the underlays show. As drawn, where they cross, the post's zigzag underlay lies on the bar's top stitches. With their underlays first, the bar's top stitches lie on both underlays there, under the post's, As drawn](../../images/generated/underlays-first-1.png) | ![2 red satin columns 5 mm wide and 16 mm long that cross, a bar and a post, each with a contour and a zigzag underlay, sewn with their stitches 1.2 mm apart so that the underlays show. As drawn, where they cross, the post's zigzag underlay lies on the bar's top stitches. With their underlays first, the bar's top stitches lie on both underlays there, under the post's, Underlays first](../../images/generated/underlays-first-2.png) |
<!-- /shot -->

On the left they sew as drawn: the bar's underlay and top stitches, then the post's. Where they cross, the
post's zigzag underlay lies on the bar's top stitches. On the right the bar's underlay and the post's sew
first, then the bar's top stitches and the post's. There the bar's top stitches lie on both underlays.
From above, sewn as dense as usual, the 2 look alike. Machine checkpoint MC-12 sews both on TS-17C, to see
which crossing lies flatter ([machine testing](../../plan/machine-testing.md)).

## Set it

In VectorCraft, every stitch family's dialog has the toggle **Underlays first**, in its **Sewing order**
section. Turn it on in the effect of each object that crosses or overlaps another. On a group's effect, it
applies to every path of the group. An SVG file keeps it as the attribute `inkstitch:underlays_first="true"`
on each shape, and `SC-W0802` names it on a group, where it doesn't apply.

Elements next to each other in sewing order form a run when each sets the toggle and they all sew in one
thread. A trim or a stop after an element ends its run. A run sews the underlays of its elements in their
order, then their top stitches in the same order. An element without underlay, such as a running stitch,
sews in the second pass. An element that joins no run sews as it does without the setting, and `SC-I0506`
names it.

## Travel and previews

Between the 2 passes the needle goes back from the run's last underlay to its first top stitch. It sews
straight there or jumps with locks, as between any 2 elements
([connecting groups](../../design/engine-pipeline.md#connecting-consecutive-groups)). So set the toggle on
elements that cross or overlap, not on elements apart. A run sews in one thread, and doesn't change
thread.

VectorCraft's live previews sew each object alone, its underlay just before its top stitches.
`stitch export --preview` draws the order that sews.
