# Keep small parts of a picture

Automatic digitizing tidies a traced photo. It merges specks under 1 mm² into the patch beside them, and
thin ragged lines into their neighbours, so that the noise of the trace doesn't sew as hundreds of short
stitches. It can't tell noise from a part that matters: the light in an eye, the dot over a letter, a
whisker, a window. You can tell it. Draw a mark round the part, and every part of the picture that lies
wholly inside the mark sews in its own thread, by its width:

- A speck narrower than the narrowest satin stroke, 1 mm unless the design sets another, sews as a short
  running stitch along its middle. A wider one sews as a satin column or a fill.
- A thin line sews as a running stitch along its middle.

The rest of the picture sews by the same rules as before. A part that crosses the mark's edge is decided as
if there were no mark. The mark itself is never sewn.

## Mark a part in VectorCraft

1. Trace the picture in VectorCraft (**Object › Image Trace › Make**).
2. With the Rectangle or the Ellipse tool, draw a shape round the part: round all of it, a little larger
   than it. Give it a stroke and no fill, so that it shows the picture under it.
3. In the Layers panel, name the shape `stitch:keep`. A note after the name is yours, such as
   `stitch:keep left eye`.
4. Export it as for any traced picture, in StitchCraft's PES or DST format with automatic digitizing on
   (**File › Export › Export As**).

For a whisker or a twig, draw a path along it with the Pen tool instead, and give it a stroke as wide as
the part. The mark is the band the stroke covers, between the path's ends.

Name a shape. A group or a layer named `stitch:keep` is no mark, and the export says so.

## Mark a part in an SVG file

In an SVG file the mark is a shape whose label is `stitch:keep`. In Inkscape, set it in **Object › Object
Properties**, as the label. Plan the file with `--auto`:

```sh
stitch plan traced.svg --auto -o traced.pes
```

## Mark a part from the command line

When `stitch plan` traces the picture itself, there is no document to draw a mark on. Give the area with
`--keep`, as its left, top, width and height in percent of the picture:

```sh
stitch plan bird.png -o bird.pes --preview bird-preview.png --keep '62,18,6,5'
```

That marks the area that starts 62 % from the left and 18 % from the top, 6 % of the picture wide and
5 % of it tall. Read it off the picture by eye. Give `--keep` once for each part. A bug-report bundle
keeps the marks, and its replay sews the same parts.

## Notices SC-I0916 and SC-W0917

For each mark, `SC-I0916` says how many parts lie wholly inside it, and how many of them sew that would
have merged, as specks and as thin lines. A mark round an eye of a traced photo usually takes in a few
specks of the trace's noise too. Each sews as a short stitch, and may leave a jump thread to cut. If the
notice counts more specks than the part has, draw the mark tighter round the part.

`SC-W0917` says that a mark doesn't keep a part: no part lies wholly inside it, or the design has no shape
digitized automatically. Draw it round the whole part.

## What it costs

Each kept part is a shape more to sew, and the rest of the design may then sew in another order. The
thread may lie thicker somewhere else. The cap on thickness, 4 fills, applies to the whole design all the
same.
