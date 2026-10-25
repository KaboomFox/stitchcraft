# Embroider a VectorCraft design

Draw the design in VectorCraft, and say how each object sews with the StitchCraft plug-ins, which show
its stitches as you work. Then write the machine file with VectorCraft's **File › Export › Export As**,
from VectorCraft 0.14, or with `stitch export` from the saved document. Both write the same file.

## Choose each object's stitches

With the [plug-ins installed](../install.md#the-vectorcraft-plug-ins), select an object and choose
**Effect › Plug-ins › StitchCraft Fill**, **StitchCraft Stroke** or **StitchCraft Satin**. The dialog
holds Ink/Stitch's settings for that kind of stitch, and the object shows its stitches in place of its
art. The thread is the colour of the object's fill for a fill, and of its stroke for a stroke or a satin
column.

From VectorCraft 0.14 each setting has a label and its unit, in the sections of the
[parameter reference](../reference/params.md). Rest the pointer on a label for its help, or press the
button after the field for its page in the reference. A setting shows only when it applies: a satin's
staggers show for staggered splits. VectorCraft before 0.14 lists every setting, each under its Ink/Stitch
name.

An object without a StitchCraft effect sews as Ink/Stitch sews a shape of an SVG file. Its fills are
filled, and its strokes are sewn as running stitch. With `stitch export --auto`, StitchCraft chooses its
stitches by width instead, as for a picture traced into shapes.

- A line up to 1 mm wide runs, or up to the narrowest satin stroke the document keeps
  ([below](#keep-the-designs-settings-and-machine-with-it)).
- A line up to 7 mm wide is a satin column as wide as drawn, its round or square caps included.
- A wider line, or a fill, is a tatami fill.

Objects are sewn in the order they are drawn, the bottom one first. [Satin columns and presets in VectorCraft](vectorcraft-columns-and-presets.md) shows
how to draw a column with 2 rails, and how to keep settings in graphic styles.

## Leave out what isn't to be sewn

Name an object or a layer `stitch:ignore`, by double-clicking its name in the Layers panel, and it is left
out with everything in it. Placement lines, notes and pictures to trace stay in the drawing this way. A
note can follow the name after a space, as in `stitch:ignore placement line`. Upper and lower case are
read alike. A hidden object is left out too.

VectorCraft's preview can't see names. An object named `stitch:ignore` that a StitchCraft effect sews, its
own or its group's, still shows its stitches there, and `stitch export` leaves it out.

## Write the machine file

With the [exporters installed](../install.md#the-vectorcraft-plug-ins), choose **File › Export › Export
As**, and **PES embroidery (StitchCraft)** or **DST embroidery (StitchCraft)** as the format. Once the file
is picked, a dialog shows 2 options:

- **Digitize automatically**, which chooses the stitches of the objects without a StitchCraft effect by
  their width, as `stitch export --auto` does.
- **Machine**, the machine and hoop the design is planned for: `document`, the one the document keeps
  ([below](#keep-the-designs-settings-and-machine-with-it)), or `brother-pe800-5x7` when it keeps none,
  unless another is chosen. `stitch profiles` lists them.

The design takes the file's name, which machines show. What `stitch export` would say about the design
shows with the export's warnings, one line for each message. When one of them is an error, nothing is
written, and VectorCraft says the first. VectorCraft lets an export run for a while, and stops one that
needs more: `stitch export` writes it.

Or save the document, and run `stitch export` on it. This patch has a fill, a satin border, a curve of
running stitch and 2 squares in a group that a graphic style's fill effect sews. Its placement line is
named `stitch:ignore placement line`:

```console
{{#include ../reference/generated/export-patch.txt}}
```

<!-- shot: patch -->
![The patch as it will sew. A red fill with rows at 45 degrees has a blue satin band and a green curve of running stitch under it. Under them, 2 gold squares with upright rows have the loose thread of the jump between them. The placement line and the start marker are not sewn.](../../images/generated/patch.png)
<!-- /shot -->

`stitch export` writes the file for the machine `--profile` names, or else the one the document keeps, or
else `brother-pe800-5x7`, and `--preview` adds a picture. It is [`stitch plan`](../reference/cli.md#stitch-plan) by another name, which
reads an SVG file the same way and takes the same options.

StitchCraft reads documents of up to 64 MiB. Pictures embedded to trace from can make one larger: place
them with **Link** checked in VectorCraft's Place dialog, and VectorCraft saves a small preview of each
in the document instead.

## Keep the design's settings and machine with it

From VectorCraft 0.16, a document can keep the machine it is for, and the 3 settings Ink/Stitch keeps
with a design for all of its objects. With the [plug-ins installed](../install.md#the-vectorcraft-plug-ins),
choose **Object › Plug-ins › StitchCraft Design Settings**, set them, and press OK:

- **Machine**, the machine and hoop the design is planned for, `brother-pe800-5x7` by default.
- **Shortest jump**, **Shortest stitch** and **Narrowest satin stroke**, in millimetres. Rest the pointer
  on a label for what each does.

The document saves them. The exporters and `stitch export` plan the design for the machine it keeps,
unless the export's **Machine** option or `--profile` names another. They use its settings as they use
an SVG file's design settings. Whatever is selected, the filter leaves every object as it is.

VectorCraft opens the dialog at the defaults each time, not at what the document keeps, so set all 4 when
you run it again. A live effect isn't given what the document keeps. The stitches shown as you work are
planned for `brother-pe800-5x7` with the default settings, and a longer shortest stitch shows only in the
machine file and in `stitch export --preview`. A setting that StitchCraft can't read keeps its default,
and `stitch export` says so (`SC-W0812`).

## Draw a test sheet

The [test sheets](../reference/test-sheets.md) drawn as designs, TS-02B to TS-09, can start from
VectorCraft 0.16 and later. With the [plug-ins installed](../install.md#the-vectorcraft-plug-ins), choose
**Object › Plug-ins › StitchCraft Test Sheet**, pick the sheet, and press OK. Rest the pointer on
**Sheet** for what each one tests. The sheet is drawn in the middle of the artboard, each shape with its
StitchCraft effect and settings, and shows its stitches. Write it as any design, with **File › Export ›
Export As**. Selected objects stay as they are, and the sheet goes above them, in their group or layer.

The sheet sews as `stitch testsheet` writes it, moved. A fill's rows lie where the document's origin puts
them, so they may fall a little elsewhere across a shape, and look the same. The threads keep their
colours but not their names in the Brother palette, and the machine file is the same. StitchCraft Stroke,
Satin and Fill must be installed: VectorCraft gives a new object only the effects of installed plug-ins.
TS-01, TS-02 and TS-10A to TS-10C are drawn stitch by stitch, and come from `stitch testsheet` alone.

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
