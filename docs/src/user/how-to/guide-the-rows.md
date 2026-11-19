# Make the rows of a fill follow a form

Automatic digitizing fills each broad shape of a picture with straight rows, all at one angle. Fur, a
petal, a leaf, a roof or a ribbon reads better when the rows bend with it, and no rule can know which way
that is. You can show it. Draw a path over the picture, name it `stitch:guide`, and each filled part that
the line crosses sews as a guided fill: its rows follow the line, bent where it bends.

Everything else is digitized as before. The guided parts still overlap their neighbours, keep the base
layer under them and sew in the order automatic digitizing chooses, and the cap on thickness still holds.
The guide itself is never sewn.

## What follows a guide

- **The filled parts it crosses.** A part follows the guide when a stretch of the line lies inside it. A
  line that crosses only a corner of a large part steers all of it, since the rows are copies of the line,
  lengthened straight on at its ends until they cover the part. Look at the preview, and draw the guide
  again where the rows go wrong.
- **Not the narrow parts.** A part sewn as a satin column or a running stitch keeps its stitches: a column's
  stitches already lie across its width, and a running stitch follows its middle.
- **Not the specks.** A part under 1 mm² stays in its area's fill and merges as before.
- **The guide drawn last** steers a part that 2 guides cross, the one you see on top.

Only the first piece of the line counts, from its start to its end. A closed line is opened where it closes,
and the width of its stroke doesn't matter.

## Draw a guide in VectorCraft

1. Trace the picture in VectorCraft (**Object › Image Trace › Make**).
2. With the Pen or the Pencil tool, draw a path across the shapes whose rows should follow it, the way the
   rows should run. Give it a thin stroke and no fill, so that it shows the picture under it.
3. In the Layers panel, name the line `stitch:guide`. A note after the name is yours, such as
   `stitch:guide left petal`.
4. Export it in StitchCraft's PES or DST format, with automatic digitizing on (**File › Export › Export
   As**).

Draw as many guides as the picture needs, all in the layer of the traced picture. VectorCraft's own preview
of the Auto effect still shows straight rows: the exported file and its preview picture follow the guides.

## Draw a guide in an SVG file

In an SVG file the guide is a path whose label is `stitch:guide`. In Inkscape, set it in **Object › Object
Properties**, as the label. Plan the file with `--auto`:

```sh
stitch plan traced.svg --auto -o traced.pes --preview traced-preview.png
```

## Give a guide from the command line

When `stitch plan` traces the picture itself, there is no document to draw a guide on. Give the line with
`--guide`, as 2 points or more, each `X,Y` in percent of the picture from its left and top:

```sh
stitch plan leaf.png -o leaf.pes --preview leaf-preview.png --guide '20,80,45,50,60,20'
```

That line runs from 20 % from the left and 80 % from the top, through the middle of the picture, to 60 %
from the left and 20 % from the top, joined by straight lines. A few points are enough: the guided fill
smooths its rows. Give `--guide` once for each guide. A bug-report bundle keeps the guides, and its
replay steers the same parts.

## Notices SC-I0918 and SC-W0919

For each guide, `SC-I0918` says how many filled parts of how many areas follow it, and how many narrow
parts it crosses that keep their stitches. If it counts more parts than you meant, draw the guide so that
it crosses fewer shapes.

`SC-W0919` says that a guide steers nothing: it crosses no filled part of a shape digitized automatically,
its line has no length, or the design has no such shape. The design then sews as without it.

## What it costs

Each area a guide steers sews one element more, so the rest of the design may sew in another order and
start or end its threads elsewhere. The thread may lie thicker in a few places, and the cap on thickness,
4 fills, applies to the whole design all the same.
