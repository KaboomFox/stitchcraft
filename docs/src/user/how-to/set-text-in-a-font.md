# Set text in an embroidery font

`stitch lettering` sews a text in an embroidery font: a font whose letters were digitized one by one, with
their satin columns and running stitches. It reads fonts in Ink/Stitch's format, where a font is a folder
with a `font.json` and a glyph file for each direction it sews in. Ink/Stitch's own fonts are in that
format, in its [embroidery-fonts](https://github.com/inkstitch/embroidery-fonts) repository, each with its
licence file.

## Sew a word

Name the text, the font's folder and the machine file:

```sh
stitch lettering "Hello" --font fonts/small_font -o hello.pes --preview hello.png
```

The command plans the text for the reference machine, a Brother PE800 with its 5 × 7 in hoop, or for the
machine `--profile` names. It prints the font, the scale, the size of the sewn text, its stitches and its
threads, and `--preview` draws what the file sews. `Hello` in Ink/Stitch Small Font is 20.2 mm wide and
5.4 mm tall, in 401 stitches.

A text of several lines has `\n` where each line ends:

```sh
stitch lettering "Stitch\nCraft" --font fonts/small_font --align center -o name.pes
```

## Choose the size

A font sews well between 2 heights, which its author set. `--scale` gives the size in percent of the
font's own, and `--height` gives it in millimetres, measured on the font's reference capital:

```sh
stitch lettering "Hello" --font fonts/small_font --height 8 -o hello.pes
```

Ink/Stitch Small Font sews from 3.6 to 15.2 mm tall, and Ink/Stitch Medium Font from 11.4 to 22.9 mm. A
size outside a font's range is set at the nearest size inside it, and `SC-W1106` names the range. For
smaller or larger letters, choose a font drawn for them.

## Space and align the text

`--letter-spacing`, `--word-spacing` and `--line-height` add millimetres between letters, words and lines,
or take them away when negative. They are millimetres at a scale of 100 percent and grow and shrink with
the text. `--align` takes `left`, `center`, `right`, `block` or `letterspacing`. A block spreads the words
of each line as wide as the widest line.

## What to check

- **A character the font lacks** sews as the font's default glyph, and `SC-W1107` names the character.
- **A text wider than the hoop** isn't written: `SC-E0701` gives its size and the hoop's. Set it smaller, on
  more lines, or for a larger hoop.
- **Threads between letters.** At small sizes the letters of a word in Ink/Stitch Small Font are joined by a
  stitch on the fabric. At larger sizes the needle jumps from letter to letter, and a machine that doesn't
  cut the thread, as the PE800, leaves those threads to cut by hand. The summary counts them.
- **The picture.** Look at it before you sew, since it draws the file's stitches as the machine sews them.

The settings are listed in the [command-line reference](../reference/cli.md#stitch-lettering), and
[fonts](../../design/fonts.md#lettering) describes how a text is laid out. Lettering along a path, a trim
after each letter and lines sewn back and forth come with a later step.
