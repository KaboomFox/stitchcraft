# Letter a picture

Text in a traced picture is shapes, and it sews as shapes: small letters come out ragged, and letters that
touch sew as one blob. You can have the text sewn in an embroidery font, whose letters were digitized one
by one for the needle. You mark where the text is and type what it says. StitchCraft takes the traced
letters out and closes the holes they left in the background. It sets your text where they were, as tall
as they were and in their colour.

The exporters hold 2 fonts, and StitchCraft chooses between them unless you name one
([which font](#which-font-the-text-is-set-in)). Ink/Stitch Small Font is light. Barstitch Bold is bold.
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
| `font` | The font, by its folder's name. The plug-ins hold `small_font` and `barstitch_bold`, and `stitch` has the fonts of its folder. Left out, the font is chosen by the letters' weight |
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
| `SC-I1118` | The font was chosen for you. The message gives the letters' height, how bold they are, and the font |

Letters smaller than the font sews are set at the font's smallest height, centred where they were. They
may then reach past the traced letters' place. Sew the design larger, or name a font that sews smaller.
Letters taller than the font sews are set at its greatest height.

The letters of an embroidery font are not as wide as a picture's. The gaps between them are widened or
narrowed so that the text is as wide as the traced text, by up to half the letters' height wider and a
tenth narrower. Beyond that the text is centred, and `SC-W1115` gives the 2 widths.

## How thick the letters sew

A font overlaps its columns where they join, so that the fabric doesn't show between them once the thread
pulls them in. Where 2 columns meet, their stitches lie on each other, with their underlay and the running
stitches between them. The cap on thread ([automatic digitizing](../stitches/automatic.md#how-thick-it-sews))
takes from marked text what other stitches cover: a column's underlay where another column covers it, and
the running stitches under a column. It never moves a column's stitches. The letters look as the font
draws them, and it leaves a column's underlay where no other column lies over it.

Letters on a field, such as a patch's word, sew thicker than letters on bare fabric: the field and its base
layer lie under every join. In the built-in fonts, Barstitch Bold's joins sew up to 4.7 fills alone and
6.1 on a field, and Small Font's up to 3.9 and 5.4. `SC-W0913` then names the join and the threads stacked
there.
It means the spot is thicker than the limit of 4 fills, not that the design is wrong. Sew a test piece of
the word first, slowly. To compare, sew the patch both ways once: with the word marked and with the
letters left traced. A lighter field under marked text may come as a setting, once a sew-out shows it is
needed.

Lettering you set yourself in a font is never changed. A design of your own lettering alone is measured,
and `SC-W0913` warns of a spot above the limit all the same.

## Which font the text is set in

A marker with `font=NAME` is set in that font. Without it, StitchCraft measures the traced letters: how
tall they are, and how wide their strokes are for that height. It then takes the font whose strokes are
nearest, among the fonts that sew letters that tall:

| Font | Folder | Its strokes, in heights of a capital letter | Sews capital letters |
|---|---|---|---|
| Ink/Stitch Small Font | `small_font` | 0.13 | from 3.8 to 16.2 mm |
| Barstitch Bold | `barstitch_bold` | 0.19 | from 12.5 to 32.5 mm |

So bold letters 15 mm tall are set in Barstitch Bold, and thin ones in Small Font. `SC-I1118` says which
font was taken and why. Bold letters under 12.5 mm are set in Small Font, since Barstitch Bold doesn't
sew that small, and the message says so. They look lighter than the picture.

`stitch` chooses the same way when its folder of fonts has both fonts. With one of them, or neither, the
text is set in `small_font`.

## Text that is still text

A marker is for letters that are shapes, traced from a picture or made into paths. Text that is still
a text object in the drawing sews without one, because automatic digitizing letters it by itself
([automatic digitizing](../stitches/automatic.md#text)).

## When to leave the letters traced

Bold block letters 10 mm tall and more sew well as traced shapes, and keep the look of the picture. From
10 to 12.5 mm that is the bolder choice, since only the light font sews there. Lettering helps most with
letters under about 8 mm, with letters on an arc, and wherever the tracing joined neighbours.
