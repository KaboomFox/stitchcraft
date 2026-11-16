# Letter in VectorCraft

A text object of a VectorCraft document sews in an embroidery font: a font whose letters were digitized
one by one for the needle. You type the text in VectorCraft, give it StitchCraft's Lettering effect, and
export. [Set text in an embroidery font](set-text-in-a-font.md) describes the fonts and lettering's
settings, which the command line shares.

## Letter a text

1. With the Type tool, click on the canvas and type the text. A click makes point type, which is the kind
   that sews. Press Enter for a new line.
2. With the text selected, choose **Effect › Plug-ins › StitchCraft Lettering**.
3. Leave **Font** empty for Ink/Stitch Small Font, and set **Size**. Choose OK.
4. Choose **File › Export › Export As**, and the PES or DST format of StitchCraft.

The text can still be retyped, and the effect's dialog opens again from the Appearance panel.

## What you see before the export

On the canvas the text keeps the look VectorCraft gives it, in its own font, and the embroidery font
isn't shown. VectorCraft gives an effect a text's outlines without its characters, and the effect can't
draw the letters that sew. The sewn text will differ from the text on the canvas in 3 ways:

- **Its letters** are the embroidery font's, whatever font the text has in VectorCraft.
- **Its size** is the effect's **Size**, in percent of the embroidery font's own height. The font size of
  the text in VectorCraft, and a text stretched by its handles, change nothing that sews.
- **Its width** is what the embroidery font's letters take. A line ends elsewhere than on the canvas.

The text sews from where its first line starts on the canvas, on its baseline, and turned as the text is
turned. To see what will sew, export and open the file's picture: `stitch preview text.pes -o text.png`
draws any machine file.

## Choose the size

Ink/Stitch Small Font is 5.08 mm tall at 100 percent, and sews from 70 to 300 percent:

| Size | Height |
|---|---|
| 70 % | 3.6 mm |
| 100 % | 5.1 mm |
| 150 % | 7.6 mm |
| 200 % | 10.2 mm |
| 300 % | 15.2 mm |

A size outside a font's range is set at the nearest size the font sews at, and the export says so
(`SC-W1106`).

## The other settings

Each is lettering's own, with the same meaning as on the command line:

- **Align** places each line from the left, centred or to the right of the point the text starts at.
- **Letter spacing**, **Word spacing** and **Line spacing** add millimetres, at a size of 100 percent.
- **Sew lines back and forth** sews every other line from its end, so that the thread between 2 lines is
  short. Small Font is drawn for both directions.
- **Sort colours** is for a font of several colours.
- **Trim** adds a trim after each line, word or letter. Leave it off for a machine that cuts no thread,
  such as the Brother PE800: there a trim only adds a jump to cut by hand.

A font that sews in one thread sews in the colour the text is filled with.

## Which fonts there are

An exporter has the fonts it was built with. The browser demo's exporters have 2, both under the SIL Open
Font License: Ink/Stitch Small Font, copyright 2017 Lex Neva, and Barstitch Bold, copyright 2017 The
Barlow Project Authors. Each licence is on the demo's site, at `fonts/small_font/LICENSE` and
`fonts/barstitch_bold/LICENSE` beside the page. Barstitch Bold is 23 mm tall at 100 percent and sews from
50 to 130 percent. Type `barstitch_bold` in **Font** for it. An empty **Font** is always Small Font: a
text object's font isn't chosen for you, as a [marked picture's](letter-a-picture.md#which-font-the-text-is-set-in) is. Exporters you build yourself have the fonts of the folder that
`STITCHCRAFT_FONTS_DIR` names when you build them, and none without it.

A font goes by its folder's name, `small_font`, or by its own, `Ink/Stitch Small Font`. When the export
doesn't find the font you named, it leaves the text out and says `SC-W1111`, with the fonts it has.

## What doesn't sew

- **Area type and type on a path.** Their lines break and bend by the widths of the letters on the canvas,
  which the embroidery font doesn't share. Write point type instead. `stitch lettering --path` sets a
  text along a path from the command line.
- **Vertical type.**
- **A text without the effect.** It is left out, and the export says so. Type › Create Outlines makes
  shapes of its letters instead, which sew as any shapes do.

## In the browser demo

The [browser demo](../../browser-demo.md) installs StitchCraft Lettering with the other plug-ins. Type a
text, apply the effect from **Effect › Plug-ins**, and export to PES: the file downloads.

## From the command line

`stitch export` and `stitch plan` sew a document's lettered texts too. They look for the font in the
folder that `STITCHCRAFT_FONTS` names, where each font is a folder of its own:

```sh
STITCHCRAFT_FONTS=fonts stitch export name.vectorcraft -o name.pes
```
