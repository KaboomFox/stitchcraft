# Embroider a VectorCraft design

Draw the design in VectorCraft, and say how each object sews with the StitchCraft plug-ins, which show
its stitches as you work. Then `stitch export` writes the machine file from the saved document.
VectorCraft's own File › Export can't write embroidery files: its plug-ins can't add file formats yet.

## Choose each object's stitches

With the [plug-ins installed](../install.md#the-vectorcraft-plug-ins), select an object and choose
**Effect › Plug-ins › StitchCraft Fill**, **StitchCraft Stroke** or **StitchCraft Satin**. The dialog
holds Ink/Stitch's settings for that kind of stitch, and the object shows its stitches in place of its
art. The thread is the colour of the object's fill for a fill, and of its stroke for a stroke or a satin
column.

An object without a StitchCraft effect sews as Ink/Stitch sews a shape of an SVG file. Its fills are
filled, and its strokes are sewn as running stitch. Objects are sewn in the order they are drawn, the
bottom one first. [Satin columns and presets in VectorCraft](vectorcraft-columns-and-presets.md) shows
how to draw a column with 2 rails, and how to keep settings in graphic styles.

## Leave out what isn't to be sewn

Name an object or a layer `stitch:ignore`, by double-clicking its name in the Layers panel, and it is left
out with everything in it. Placement lines, notes and pictures to trace stay in the drawing this way. A
note can follow the name after a space, as in `stitch:ignore placement line`. Upper and lower case are
read alike. A hidden object is left out too.

VectorCraft's preview can't see names. An object named `stitch:ignore` that a StitchCraft effect sews, its
own or its group's, still shows its stitches there, and `stitch export` leaves it out.

## Write the machine file

Save the document, and run `stitch export` on it. This patch has a fill, a satin border, a curve of
running stitch and 2 squares in a group that a graphic style's fill effect sews. Its placement line is
named `stitch:ignore placement line`:

```console
{{#include ../reference/generated/export-patch.txt}}
```

<!-- shot: patch -->
![The patch as it will sew. A red fill with rows at 45 degrees has a blue satin band and a green curve of running stitch under it. Under them, 2 gold squares with upright rows have the loose thread of the jump between them. The placement line and the start marker are not sewn.](../../images/generated/patch.png)
<!-- /shot -->

`stitch export` writes the file for the machine `--profile` names, `brother-pe800-5x7` by default, and
`--preview` adds a picture. It is [`stitch plan`](../reference/cli.md#stitch-plan) by another name, which
reads an SVG file the same way and takes the same options.

StitchCraft reads documents of up to 64 MiB. Pictures embedded to trace from can make one larger: place
them with **Link** checked in VectorCraft's Place dialog, and VectorCraft saves a small preview of each
in the document instead.

## Read what it says

Each line under the summary names an object by its id in the document, such as `vc:10`, and says what
was done with it. In the patch, the placement line is left out, as its name asks (`SC-I0811`). The start
marker isn't applied yet (below). The group's own gold fill, which the graphic style put on it, isn't
sewn as a shape of its own, and gives the squares their thread. Other messages name what isn't sewn as it
is drawn, with the VectorCraft command that makes it sewable, such as **Type › Create Outlines** for text
(`SC-W0807`). The [diagnostic codes](../reference/diagnostics.md) page explains each code, as
`stitch explain SC-W0807` does.

## Start and end markers

A small path named `stitch:start` or `stitch:end`, grouped with a fill or a satin column, marks where the
fill or the column is to start or end, as Ink/Stitch's starting and ending positions do. A marker is
never sewn, and a group or a layer is never a marker. StitchCraft doesn't start or end a shape at one
yet, and chooses where, as the patch's start marker shows.

## When it sews wrong

`stitch bug-report design.vectorcraft --says "what is wrong"` writes a bundle that includes the document.
[Report a bug](report-a-bug.md) says what to do with it.
