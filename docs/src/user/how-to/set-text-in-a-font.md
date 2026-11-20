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

Ink/Stitch Small Font sews from 3.6 to 15.2 mm tall, Ink/Stitch Medium Font from 11.4 to 22.9 mm, and
Barstitch Bold, whose strokes are half again as wide, from 11.5 to 29.9 mm. A
size outside a font's range is set at the nearest size inside it, and `SC-W1106` names the range. For
smaller or larger letters, choose a font drawn for them.

## Make the letters bolder

`--widen-percent` sews every satin column of the text wider, by that percent of its width on each side.
It takes 0 to 50 and is 0 unless you set it. In the Lettering effect's dialog it is **Widen columns**. Use
it where you want bold letters smaller than a bold font sews, such as Barstitch Bold under 12.5 mm:

```sh
stitch lettering "Hexmira" --font fonts/small_font --height 10 --widen-percent 20 -o hexmira.pes
```

At 20, Small Font's strokes sew about a fifth of the letters' height wide, as heavy as a bold font's. Running
stitches and fills sew as they are. The small counters close first: under 8 mm the holes of the A and the 4
fill with thread, so keep to 8 mm or taller, or widen less. Look at the preview before you
sew. A value outside 0 to 50 is set at the nearest, with `SC-W0102`.

## Space and align the text

`--letter-spacing`, `--word-spacing` and `--line-height` add millimetres between letters, words and lines,
or take them away when negative. They are millimetres at a scale of 100 percent and grow and shrink with
the text. `--align` takes `left`, `center`, `right`, `block` or `letterspacing`. A block spreads the words
of each line as wide as the widest line.

## Set the text along a path

Draw the path in an SVG file, give it a stroke, and name the file with `--path`:

```sh
stitch lettering "Hello" --font fonts/small_font --scale 300 --path arc.svg -o arc.pes
```

The text follows the path from its start, each letter turned the way the path runs under it. The letters
aren't bent. The text lies where the path lies on the file's page. In a file with several paths, label
the one to follow `batch lettering`, as Ink/Stitch's batch lettering asks. Without that label the file's
first path is taken, and the summary names it.

- `--text-position` takes `left`, `center`, `right` or `stretch`. The first 3 put the text at the path's
  start, at its middle or at its end. Stretched, its letters are spread from the start to the end.
- `--vertical-text-position` takes `top`, `baseline` or `bottom`. With `top` the text stands on the path,
  and with `bottom` it hangs from it. With `baseline` the letters' baseline is the path, and a `p` reaches
  below it.

A text longer than its path goes on straight past the path's end, and `SC-W1109` gives both lengths. Draw
the path as the text's baseline should run, in the direction the text reads. On a tight bend the letters'
tops spread apart or touch: look at the picture, and add `--letter-spacing` where they touch.

## Lines, trims and colours

- **Back and forth.** Ink/Stitch Small Font and Medium Font draw every letter twice, to sew from the left
  and from the right. The second line of a text then sews back from where the first ended, and the thread
  between the lines is short. This is on unless `--back-and-forth false` turns it off. Most fonts sew one
  way. In them every line sews from its start, the thread crosses the text between 2 lines, and `SC-W1110`
  says so.
- **Trims.** `--trim` takes `off`, `line`, `word` or `glyph`, and adds a trim after each line, each word or
  each letter. A font's own trims stay.
- **Colours.** A multicolour font sews each letter in its 2 or 3 threads before the next letter, with a
  thread change each time. `--color-sort all` sews the whole text's first colour, then its second. `line`
  and `word` do so for each line or each word. It needs a font that sorts its colours, as its `font.json`
  says with `sortable`.

## On a machine that doesn't cut the thread

The Brother PE800 cuts no thread between letters. Every jump leaves a thread on the fabric to cut by hand,
and the summary counts the jumps.

- **Leave `--trim` off.** A trim doesn't cut anything on this machine. It ends the stitching with a knot and
  jumps, where the letters of Small Font and Medium Font would have been joined by stitches under their
  columns. `Hello World` in Small Font at 100 percent has 3 jumps without trims and 10 with a trim after
  each letter.
- **Keep back and forth on,** and choose a font that sews both ways for a text of several lines. The
  thread between 2 lines is then as short as the lines are apart.
- **Sew small.** Up to about 10 mm tall, a word in Small Font sews without a jump. Above that the gaps
  between letters grow past 3 mm, and each becomes a thread to cut.
- **Sort the colours** of a multicolour font with `--color-sort all`: each thread change is a stop at the
  machine, and a sorted text has one for each colour.

## What to check

- **A character the font lacks** sews as the font's default glyph, and `SC-W1107` names the character.
- **A text wider than the hoop** isn't written: `SC-E0701` gives its size and the hoop's. Set it smaller, on
  more lines, or for a larger hoop.
- **Threads between letters.** At small sizes the letters of a word in Ink/Stitch Small Font are joined by a
  stitch on the fabric. At larger sizes the needle jumps from letter to letter, and a machine that doesn't
  cut the thread, as the PE800, leaves those threads to cut by hand. The summary counts them.
- **The picture.** Look at it before you sew, since it draws the file's stitches as the machine sews them.

The settings are listed in the [command-line reference](../reference/cli.md#stitch-lettering), and
[fonts](../../design/fonts.md#lettering) describes how a text is laid out and how it follows a path.
