# Letter a picture

Text in a traced picture is shapes, and it sews as shapes: small letters come out ragged, and letters that
touch sew as one blob. You can have the text sewn in an embroidery font, whose letters were digitized one
by one for the needle. You mark where the text is and type what it says. StitchCraft takes the traced
letters out and closes the holes they left in the background. It sets your text where they were, as tall
as they were and in their colour.

The font is Ink/Stitch Small Font unless you name another. It sews letters from 3.6 to 15.2 mm tall.
[Set text in an embroidery font](set-text-in-a-font.md) describes the fonts.

## Mark the text in VectorCraft

1. Trace the picture with **Object › Image Trace › Make**, as for any picture.
2. With the Rectangle tool, draw a rectangle round the text: round every letter of it, and round nothing
   else. Give it a stroke, so that it is an object that paints.
3. In the Layers panel, name the rectangle `stitch:text` and the text, such as `stitch:text MATCH`.
4. Choose **File › Export › Export As**, and the PES or DST format of StitchCraft, with automatic
   digitizing on.

The rectangle isn't sewn. The canvas keeps showing the traced letters, and the lettering is in the
exported file. To see it, draw the file: `stitch preview patch.pes -o patch.png`.

One marker has one line of text. Mark 2 lines with 2 rectangles.

## Text on an arc

For text that follows a curve, as round a badge, a path takes the place of the rectangle:

1. With the Pen tool, draw a path through the middle of the letters, from the first letter to the last,
   in the direction the text reads. On the lower half of a round badge that is still from left to right.
2. Give the path a stroke wide enough to cover the letters from top to bottom, and no wider: it must not
   reach a star or a border beside the text.
3. Name it `stitch:text` and the text.

The text follows the path, each letter turned with it. Its tops are to the left of the way you drew the
path. A path drawn the other way sets the text upside down.

## Mark the text from the command line

`stitch plan` and `stitch export` trace a picture themselves, and no document opens to draw on. Give the
text's area with `--text`, as its left, top, width and height in percent of the picture, then a colon and
the text:

```sh
stitch plan patch.png -o patch.pes --preview patch.png --text '20,35,60,30:MATCH'
```

That marks the area that starts 20 % from the left and 35 % from the top, 60 % of the picture wide and
30 % of it tall. The area must have every letter inside it and nothing else. Read it off the picture by
eye and leave some room. Give `--text` once for each line of text.

`--text` marks level text. For text on an arc, write the traced document with `--traced traced.vectorcraft`,
add the path to it in VectorCraft, and plan the document.

`stitch` finds its fonts in the folder the `STITCHCRAFT_FONTS` variable names, each font a folder in it.

## An SVG file

In an SVG file the marker is a shape whose label is `stitch:text` and the text. In Inkscape, set it in
**Object › Object Properties**, as the label. Plan the file with `--auto`:

```sh
stitch plan traced.svg --auto -o traced.pes
```

## Another font, and trims

Put the text in double quotes, and settings after it:

```text
stitch:text "MATCH" font=geneva_simple trim=word
```

| Setting | What it does |
|---|---|
| `font` | The font, by its folder's name. The plug-ins hold Ink/Stitch Small Font, and `stitch` has the fonts of its folder |
| `trim` | A trim after each `line`, `word` or `glyph`, or `off` |
| `color_sort` | In a font of several colours, sorts them over `all` the text, each `line` or each `word`, or `off` |

The size and the spacing are found from the traced letters, so a marker doesn't set them.

## What you are told

| Code | When |
|---|---|
| `SC-I1112` | Text was replaced. The message counts the traced outlines and gives their height, the text, the font and the lettering's height |
| `SC-W1113` | A marker wasn't used, and why. The traced letters then sew as before |
| `SC-W1114` | Shapes cross the marker's edge. They sew as traced, beside the lettering. Make the marker larger to take them in |
| `SC-W1115` | The lettering isn't the traced letters' size, with both sizes |

Letters smaller than the font sews are set at the font's smallest height, 3.6 mm in Small Font, centred
where they were. They may then reach past the traced letters' place. Sew the design larger, or name a font
that sews smaller. Letters taller than 15.2 mm are set at 15.2 mm.

The letters of an embroidery font are not as wide as a picture's. The gaps between them are widened or
narrowed so that the text is as wide as the traced text, by up to half the letters' height wider and a
tenth narrower. Beyond that the text is centred, and `SC-W1115` gives the 2 widths.

## When to leave the letters traced

Small Font's strokes are light. Bold block letters 10 mm tall and more sew well as traced shapes, and
keep the look of the picture. Lettering helps most with letters under about 8 mm, with letters on an arc,
and wherever the tracing joined neighbours.
