# Fonts

<!-- implements: crates/stitchcraft-svg/src/font/**, crates/stitchcraft-svg/src/lettering/**, apps/stitchcraft-cli/src/commands/lettering.rs, apps/stitchcraft-cli/src/commands/lettering/** -->

Lettering sews text in a font. M14 sets text in embroidery fonts, digitized glyph by glyph in Ink/Stitch's
font format. M15 ports an ordinary TrueType or OpenType font automatically. It reads each glyph's outline at
the height the text is set and works out how the glyph sews ([roadmap](../plan/roadmap.md)). This page
records the research both milestones rest on, counted and read on 2026-11-02. The sources are listed at the
end. Since M8.6, Ink/Stitch's fonts are a corpus of the differential, each glyph sewn in both programs
([the corpus](#the-corpus)).

## Ink/Stitch's fonts

Ink/Stitch's fonts are a repository of their own,
[inkstitch/embroidery-fonts](https://github.com/inkstitch/embroidery-fonts), which Ink/Stitch includes as its
`fonts` folder. At commit `c7e3a05` it has 142 fonts. Ink/Stitch's tutorial on making a font and its page on
the font tools document the format. The numbers below were counted from the font files.

- **A font is a folder.** `font.json` gives the font's settings, and a glyph file gives its glyphs for each
  direction it sews in. 137 fonts sew left to right (`ltr.svg`) and 17 right to left (`rtl.svg`), 12 of
  them both ways. One font also sews top to bottom and bottom to top. 5 large fonts split a direction into
  several files, in a folder of that name. The folder also has a licence file and a preview picture.
- **A glyph file** has a layer for each glyph, labelled `GlyphLayer-` followed by the character, and a guide
  named `baseline`. A layer can be named for several characters, as for a ligature. An Arabic letter has a
  layer for each of its forms.
- **The settings** name the font, its licence and the font it was made from, with that font's address.
  `size` is the height of a reference capital in millimetres, and `min_scale` and `max_scale` bound the
  heights the font sews at. `horiz_adv_x` gives each glyph's advance, with a default and the space's.
  `kerning_pairs` gives each pair's adjustment, which is subtracted from the advance. Further keys force
  upper or lower case, name the glyph sewn for a missing one, and say whether the font sews lines back and
  forth, sorts its colours or is routed by auto-satin.
- **Heights.** A font sews from `size` × `min_scale` to `size` × `max_scale`. The least heights range from
  2.8 mm (Glacial Tiny) to 144 mm and the greatest from 6.9 to 468 mm. Their medians are 20 and 45 mm. 21
  fonts are tagged `tiny`.
- **Stitch types.** 94 fonts sew satin columns, 73,422 of them. 119 fonts sew running stitches, 67 tatami
  fills and 25 cross stitch.

**How they are made.** 134 of the fonts name the font they were made from, 36 of them from Google Fonts. The
tutorial starts from a TrueType or OpenType file. FontForge writes it as an SVG font, and an Ink/Stitch tool
splits that into glyph layers at a chosen height. Each glyph is then digitized by hand, with its satin
columns and rungs, where it starts and ends, its trims and its settings. The tutorial calls each letter "a
small embroidery in itself" and ends with "real stitchouts are the ultimate test".

**Small sizes** are separate fonts. Pacificlo tiny sews Pacifico from 4.2 to 9.4 mm, and Pacificlo from 13.6
to 23.8 mm. Pacificlo tiny, Amitaclo small and Chicken Little KOR Small keep their larger fonts' columns with
lighter settings. Their satin spacing is 0.25 mm, without a zigzag underlay, and their pull compensation is
0.05 to 0.1 mm in place of 0.2 to 0.3 mm. Venezia small also sews satin columns where Venezia sews running
stitches. At their least heights, the median satin column of 4 small fonts is 0.47 to 0.61 mm wide, and a
tenth of their columns are narrower than 0.3 to 0.5 mm. The 4 are Ink/Stitch Small Font, Glacial Tiny,
Pacificlo tiny and Venezia small. Automatic digitizing sews a line narrower than 1 mm
(`min_satin_stroke_width_mm`) as a run, and compensates every satin column by 0.4 mm.

**Licences.** By their licence files, 102 fonts are under the OFL, 25 under CC BY-SA, 1 under CC BY, and 3
in the public domain or under CC0. 8 are under CC BY-NC-SA, 1 under CC Attribution-NoDerivatives and 1 under
the GPL-3.0. NickAinley, the 142nd, is under a free font's licence, with its authors' permission for
Ink/Stitch. The licence field of `font.json` disagrees with the licence file for 3 fonts. It names the OFL
for `precious`, whose licence file is the GPL-3.0 with a ban on selling the font. It names CC BY-SA for
`cogs_KOR`, under CC BY, and for `nick_ainley`. Ink/Stitch 3.3.0 renamed 14 fonts to respect the OFL's
Reserved Font Names.

**Lettering.** Ink/Stitch's lettering (`lib/lettering` at `d59c9ab`) advances each glyph by its own advance,
less the kerning of its pair, and each word by the space's advance. The user can widen the spacing of
letters, of words and of lines by a number of millimetres. Lines align left, centre or right, or are
justified as a block. A trim
can follow each letter, word or line. Lines sew back and forth in a font with a variant for the other
direction, and a sortable font's colours sew in order. Batch lettering clamps the scale to the font's range.
In a font whose `auto_satin` is on, Auto-Route Satin Columns routes the satin columns and strokes of the
whole text, their order kept and without trims, before the trims are added and the colours sorted. Each
piece goes to the end of the group its element was in. The key is on when `font.json` leaves it out, and
at `c7e3a05` every font sets it, Ink/Stitch Small Font and Medium Font on and the other 140 off.
StitchCraft's lettering routes such a font's text with M10.4's function ([routing tools](routing.md)).

## The corpus

M8.6 makes Ink/Stitch's fonts a corpus of the [differential](conformance.md#l3--inkstitch-differential-nightly).
`cargo xtask corpus` fetches the archive GitHub makes of `c7e3a05`, the commit
`conformance/inkstitch/fonts.toml` pins, and checks its SHA-256 before it unpacks it. The fonts stay in a
cache outside the repository: the folder `STITCHCRAFT_CORPUS` names, or `stitchcraft/corpus` in the user's
cache folder. No font file is committed. Outside the repository the gates leave them alone, where the
clean-room check would find the GPL in one font's licence. `cargo clean` keeps them, and every worktree
shares them.

**The fonts it takes.** A licence file names the font's own licence first, in a sentence such as "This font
has been adapted for Ink/Stitch and is licensed under the SIL Open Font License". It often names the licence
of the font it was made from after it, or quotes it. So of the licences a file names, the first is the
font's. The corpus takes the fonts under the OFL, CC0 or the public domain, CC BY and CC BY-SA, whose
adaptations may be shared for any use: 131 fonts, with 167 glyph files and 21,179 glyphs. The report names the
other 11, which are never sewn.

**A glyph as a design.** Ink/Stitch's lettering takes every group labelled `GlyphLayer-`, wherever it lies in
the file, and so does the differential: of the 21,616 glyphs of the 142 fonts, 3,030 lie inside other groups.
Each glyph is cut out of its glyph file as a design of its own. Its group is shown, as lettering shows it, and
every other drawn element is left out, in the groups round it too. The rest of the file stays as it was
written, its version and definitions with the symbols of Ink/Stitch's commands among them, and so do the
transforms and styles of the groups round the glyph. Both programs sew that file. StitchCraft plans it for the reference machine with a
hoop 400 mm a side, since a glyph file is drawn at its font's height and a glyph can be wider than the
reference hoop.

**The sample.** Every night the differential sews 4 glyphs of each font, 524 in all, and every Sunday all
21,179, in 6 parts. A font's glyphs are taken in the order of its files, sorted by path, and in each file in
its order. The sample takes the glyphs at the places 0, n/4, n/2 and 3n/4 of n, rounded down. It spreads over
each font, from its first glyph, and moves only when the pin does. Run from its checkout, Ink/Stitch logs at
its debug level, and a glyph took it 10 seconds, against 1.6 with the log turned off. On 2 cores the sample
then takes 12 minutes, and a twentieth of every glyph 22.

**What differs.** These counts are from runs on 3 November 2026, before M10.1 brought simple crosses. Of
the sample's 524 glyphs, 357 agree by every measure. 98 are of stitch types StitchCraft doesn't sew yet,
which the report lists, 66 of them in cross stitch and the rest in tartan fills, ripples and guided fills.
They join the comparison with M10's steps, the simple crosses with M10.1 and Jersey 15's double crosses
with M10.2. Sewn alone with M10.2, every one of Jersey 15's 162 glyphs agrees, the 2 programs' threads
overlapping by 1.000 in each. Ripples join with M10.8, and guided ripples with M10.11. Of the 324 glyphs of
the 3 Ondulamarif fonts, sewn alone on 8 November 2026, 144 agree and 180 differ as the ledger explains,
most of them by where each program's running stitch places the needle points of a ripple's lines. Medium
has 57 that agree and 51 excused, S 46 and 62, and XL 41 and 67. The sample's other 69 differ as the
ledger explains,
`conformance/inkstitch/glyphs.toml` naming the glyphs each excuse is for. A glyph may differ in more than
one way. A twentieth of every glyph, the seventh of 20 parts, holds 1,059 glyphs. Of them 699 agree, 238
are listed and 122 differ as the ledger explains. It found the last 3 rows of the table, which the sample
has none of, and a glyph neither program wrote a file of, which agreed: the U of Apex simple small, whose
satin column neither could sew then.

| Difference | Sample | Twentieth | Explained by |
|---|---|---|---|
| The threads named by other entries of Brother's palette, both files changing thread at the same blocks | 34 | 29 | `DEV-THREAD-001`, checked |
| Running stitches placed otherwise, more of them on tight curves, within 0.3 mm of each other | 14 | 28 | `DEV-RUN-002`, checked |
| Elements whose settings StitchCraft can't read, left out, such as a `repeats` of `0 2` in Neon blinking | 6 | 38 | `DEV-PRM-001` |
| Stitch patterns, which StitchCraft doesn't apply, in Initials XL and Mai en fleur | 5 | 6 | `DEV-SVG-026` |
| Random satin widths, and a negative width decrease raised to 0, in Magnolia bicolor and Cats | 4 | 7 | `DEV-SAT-002`, `DEV-PRM-002` |
| Meander tiles of StitchCraft's own, in Barstitch textured | 4 | 8 | `DEV-FILL-018` |
| Cooper Marif's fills in a viewBox of another shape than its page, which StitchCraft centres and Ink/Stitch doesn't | 4 | 6 | `DEV-SVG-025` |
| Satin stitches longer than the machine's 12 mm, split, in Hebrew simple rounded small, whose pull compensation is 10 mm | 2 | 1 | `DEV-FIN-001`, checked |
| Needle points nearer the one before than the machine's shortest stitch, 0.3 mm, left out, in Honoka's underlays of 0.2 mm stitches and DejaVu's full stop | 0 | 3 | `DEV-FIN-001`, checked |
| A needle point Ink/Stitch sews where a path ends with a move-to that draws nothing, 24 mm above Kum Tsoan relief's V | 0 | 1 | `DEV-SVG-011`, checked |
| A jump that goes nowhere in Ink/Stitch's file, which pystitch writes | 0 | 1 | `DEV-FMT-001`, checked |

A checked excuse counts only where the differential undoes the deviation in Ink/Stitch's file and the measure
then agrees, or where each program's thread lies within 0.3 mm of the other's. Without its 6 stitch patterns,
Ink/Stitch sews Initials XL's B in 9,804 stitches, against StitchCraft's 9,805 and its own 12,158. In a page
of its viewBox's shape, Cooper Marif's 7 overlaps by 0.999, against 0.573. Without its needle points nearer
the one before than 0.3 mm, Ink/Stitch sews Honoka's 服 in 1,889 stitches, against StitchCraft's 1,935 and its
own 3,101.

**Every glyph.** The first run of every glyph, on 3 November 2026 at `c31df0d4`, found 56 glyphs that differ
by more than the ledger explained and 11 that didn't sew, in 20 fonts. The 11 are glyphs of one empty path,
such as Egyptian's spaces and Noble's tilde. Ink/Stitch doesn't write a file of them and says it didn't
find an element to sew, and StitchCraft writes none with `SC-E0010`, so they agree now. `--font` sews the 20
fonts' 4,961 glyphs alone, and M8.6 sorted the 56 by their causes:

| Cause | Glyphs | What M8.6 did |
|---|---|---|
| Running stitch outlines whose needle points set the bounds the files are aligned by, round the fills of 22 of Western Light's glyphs and beside satin columns in 3 glyphs of other fonts | 25 | Excused by `DEV-RUN-002`, checked within 0.3 mm |
| Corners of 2 subpaths that meet but for rounding, 2e-15 to 7e-15 CSS pixels apart, so shapely repairs the shape, in 14 cross stitch glyphs of Egyptian's 2 sizes and in Jaquarda Bastarda 9's ® | 15 | Excused by `DEV-FILL-001`, for the glyphs named |
| A rung that rounding leaves a hair short of a rail, which Ink/Stitch takes for a rail, in 3 glyphs of AGS Garamond and in Apex simple's q in both its sizes | 5 | The V, which StitchCraft didn't sew, is fixed and agrees now. `DEV-SAT-013` excuses the other 4 |
| Fills that run along their own edges (`SC-I0313`), in Califragilistic | 3 | Excused by `DEV-FILL-001` |
| Cross stitch parts that reach equally far left after the repair, in Egyptian small's ǘ and ӳ | 2 | Excused by `DEV-FILL-029` |
| Satin rings that start apart, in Initials XL's comma and Alchemy's 🜂 | 2 | Excused by `DEV-SAT-009` |
| The second of 2 satin rings started past a rung, its stitches leaning up to 89°, in Emilio 20 tricolore's E | 1 | Fixed: it starts at its nearest node that no rung lies before |
| Travel under a tatami fill's rows, in Alchemy's ¯ | 1 | Excused by `DEV-FILL-007` |
| Jumps that go nowhere after lock stitches 2 satin columns force, in DejaVu's Ä | 1 | `DEV-FMT-001`, now undone within colour blocks too |
| Top stitches that Ink/Stitch's plan drops beside the underlay's last point, 0.4 mm off each tip of Manga Impact's & | 1 | Excused by `DEV-RUN-003` |

Run again on 8 November 2026 with these changes, the 20 fonts' glyphs give 4,644 that agree and 317 that
differ as the ledger explains. None differ otherwise or are known follow-up work, and every one sews.
The rung fix also lets StitchCraft sew the x and the U of Apex simple, in both its sizes, which neither
program sewed before. Ink/Stitch still calls their columns not stitchable, and `DEV-SAT-013` excuses them.
A scan of every glyph file's satin columns found 5 more fonts with columns the 2 fixes could change. Run
again too, their 562 glyphs agree, but for 2 that excuses made before M8.6 explain.

## Licences in the font repository

[ADR-0020](adr/0020-fonts-from-a-repository-of-their-own.md) sets which licences StitchCraft's
font repository takes and how a font's licence is checked when it joins. Read on 8 November 2026, each of
the 131 licence files the corpus takes at `c7e3a05` names an allowed licence first. 32 of them also point
at the terms of the font each was made from, or at a permission its author gave:

- 15 point at an original under the OFL or in the public domain, such as Magnolia Script, Linux Libertine
  and Apex Lake, or under DejaVu's licence.
- 7 point at an original that its author's own terms let anyone use, without a word on shared
  adaptations, and at its author's leave to use it for Ink/Stitch. Aventurina, Bluenesia Satin, Cherry for
  inkstitch, Cherry for Kaalleen, DinoMouse72, Learning curve and Monicha are these 7.
- 7 are the Emilio 20 fonts, whose original the files name as under CC BY-SA and "used and distributed with
  permission from the author".
- 3 draw animals of the Emoji Embroidery Project, whose author allowed their use in fonts for Ink/Stitch.
  The 3 are Animals Alphabet, Alphabet des animaux and Tieralphabet.

The ADR keeps the 7 of the second kind out until their authors allow sharing beyond Ink/Stitch, and a person
reads the 10 of the last 2 kinds before they join.

## The reader

`stitchcraft_svg::font::read` reads a font in Ink/Stitch's format from the files of its folder, as
Ink/Stitch's lettering reads it at `d59c9ab` (`lib/lettering/font.py`, `font_variant.py` and `glyph.py`),
run as a black box on fonts drawn for it ([ADR-0020](adr/0020-fonts-from-a-repository-of-their-own.md)). M14.2's lettering sets text with what it reads, as `REQ-FONT-002` to `REQ-FONT-008` say.

- **Settings.** `font.json` is UTF-8 text, with or without a byte order mark, and holds an object of
  settings. Each key that Ink/Stitch reads is read at Ink/Stitch's default where the file lacks it, as the
  [font settings](../user/reference/font-settings.md) page gives them. A value of another type keeps the
  default (`SC-W1102`), and a key Ink/Stitch doesn't read is ignored (`SC-I1105`). A font without a
  `font.json` that holds an object isn't read (`SC-E1101`). Ink/Stitch loads such a font with every setting
  at its default and leaves it out of its list of fonts, as it has no name.
- **Directions.** A direction's glyph files are `ltr.svg`, then the SVG files directly in a folder `ltr`
  by their names, and the same for `rtl`, `ttb` and `btt`. An older font names its files `→.svg`, `←.svg`,
  `↓.svg` and `↑.svg`, as Ink/Stitch Small Font's README describes them. Ink/Stitch takes a folder's files
  in the order the system lists them in, which differs from system to system. 2 fonts at `c7e3a05` draw a
  glyph in 2 files of a direction. Roman bicolor AGS draws 66 glyphs in its `mono.svg` and again in its
  `zbi.svg`, whose name sorts after it, and AGS Garamond's full stop is in both its files. A file
  compressed with xz isn't read (`SC-W1103`).
- **Glyphs.** Each group labelled `GlyphLayer-` and a name is a glyph, wherever it lies. Its name is its
  label in Unicode's NFC, so the ohm sign is the Greek omega, and of 2 glyphs of one name in a direction the
  later counts (`SC-I1105`). Its design is the one the glyph differential cuts and sews
  ([the corpus](#the-corpus)), read by the SVG reader with what the reader says of it.
- **The box.** A glyph's box spans the x of the points that its `<path>` elements reach, connectors'
  aside. A subpath reaches its ends and its curves' turning points, and a move-to that draws nothing
  reaches its place. Hidden paths count,
  and so does a glyph inside the glyph. Ink/Stitch's lettering places each glyph by its box's left edge,
  and advances a glyph without an advance of its own to its right edge. A path whose transform doesn't
  read counts where it lies without it, where the SVG reader leaves it out of the design (`SC-W0804`).
  Ink/Stitch leaves a glyph out when its group has no path, or when a path in it draws nothing, such as one
  of empty path data. StitchCraft leaves it out too (`SC-W1104`). 12 glyphs at `c7e3a05` are left out so,
  11 of them of one empty path.
- **The baseline.** A glyph file's baseline is its first guide labelled `baseline`. Inkscape writes the
  guide's y up from the page's bottom, and the baseline lies the page's height in user units less that y
  below the top. The page's height is the viewBox's, or else the root's height in pixels, and 0 when the
  root has no height in a unit, as 2 of Ink/Stitch's glyph files have none. Without the guide the
  baseline is the page's top (`SC-I1105`).
- **Lengths.** The reader gives every length in millimetres. The box and the baseline lie on the glyph
  file's page, as the glyph's design does, and the advances, the kerning, the space and the leading are
  converted from `font.json`'s CSS pixels. A pair's kerning is the value for its 2 labels with a space
  between them, or else joined, as older fonts write 195,269 of the 1,816,358 pairs at `c7e3a05`. A
  glyph's advance is its own, or else `horiz_adv_x_default`, or else its box's right edge.
- **The licence.** The licence file, and the licence it names first, as the corpus reads it.

**Measured against Ink/Stitch.** On 8 November 2026 both programs read the 142 fonts of `c7e3a05`. They
found the same 21,502 glyphs in the same directions, and the same default direction for each font. 21,132
glyphs have the same left edge, width and baseline within a micrometre. The numbers from Ink/Stitch have
6 digits, such as 11.3386 for StitchCraft's 11.338584. The other 370 differ for 2 causes:

- 303 are the glyphs of the 3 fonts whose glyph files draw a viewBox of another shape than the page, which
  StitchCraft centres and Ink/Stitch doesn't (`DEV-SVG-025`). Their boxes lie right of Ink/Stitch's, as their
  designs do: 30.0 mm in Califragilistic, 16.5 mm in Cooper Marif and 24.8 mm in InfiniPicto.
- 67 are the glyphs drawn in 2 files of a direction, which Ink/Stitch took in the system's order.

Ink/Stitch Small Font, under the OFL, is the corpus's case of a real font (`REQ-FONT-008`). Its 98
left-to-right and 89 right-to-left glyphs read with nothing said, and plan in 17,890 stitches for the
reference machine with a hoop 400 mm a side.

## Lettering

`stitchcraft_svg::lettering::set` sets a text in a font the reader read, and `stitch lettering` is its first
host (M14.2, `REQ-FONT-009` to `REQ-FONT-014`). `set_along` sets it along a path (M14.3, `REQ-FONT-015` to
`REQ-FONT-020`). Both follow Ink/Stitch's lettering, read at `d59c9ab` (`lib/lettering/font.py`,
`font_variant.py`, `lib/extensions/batch_lettering.py` and `lettering_along_path.py`) and run as a black box.
The text becomes a design, which every host plans as it plans any other.

- **Lines and words.** The text is read in Unicode's NFC, as glyphs are named. A line ends at a line feed
  and loses the spaces at its ends. Its words lie between its spaces, and between 2 spaces in a row lies an
  empty word. A word is set in the case `letter_case` forces.
- **Glyphs.** At each character of a word, the glyph with the longest name that the rest of the word starts
  with sews, and the text goes on after that name. A ligature's glyph, named by its letters, sews for them.
- **A missing character** sews as the glyph `default_glyph` names, and `SC-W1107` names the character. In a
  font without that glyph the pen moves on by a space, and the warning says that instead.
- **Places.** A word's first glyph has its box's left edge at the pen, which starts each line at 0. Each
  later glyph's origin lies its predecessor's advance after that glyph's origin. The pair's kerning is
  taken from that distance and the letter spacing added to it. Its box lies its own left edge after its origin. After a word the pen lies the
  last glyph's advance after its origin, plus a space's advance and the word spacing. Ink/Stitch adds a
  glyph's left edge to its place for every glyph but a word's first, which is the same rule told from the
  pen.
- **Lines.** Each line's baseline lies `leading` plus the line height below the line before. An empty line
  takes its height too.
- **Alignment.** A line's width spans its glyphs' boxes. Left-aligned lines stay. Centred, a line moves left by
  half its width, and right-aligned by all of it. The first line's start is then the text's centre or its
  right edge. A block moves each word of a line right by its place in the line times an equal share of
  what the line lacks to the widest line. Ink/Stitch's `letterspacing` does the same to the glyphs of a line
  of one word.
- **The scale.** The text is set at `scale` percent of the font's size, kept within `min_scale` and
  `max_scale`, as Ink/Stitch's batch lettering keeps it. Outside them the text is set at the nearer end, and
  `SC-W1106` names the range in percent and in millimetres of height. The elements are scaled about the first
  line's start on its baseline, each with its stroke's width. The spacings are scaled with the text, as
  Ink/Stitch scales the group it sets the text in: 1 mm of letter spacing at 50 percent is 0.5 mm. An
  element's stitch settings aren't scaled, in either program.
- **Right to left.** In a font whose `text_direction` is `rtl`, a line's words and each word's glyphs are
  laid in the reverse of their order in the text, and a pair's kerning is looked up with the glyph that
  comes first in the text first. The glyphs of an `rtl` direction are drawn to sew from the right, and a
  line in them sews from its last glyph to its first.
- **Back and forth.** A font's author can digitize each glyph twice, to sew from the left and from the
  right. With `back_and_forth` on, which is the default as in Ink/Stitch, the lines of a text in such a font
  take their glyphs from the font's default direction and from its reverse, one line from each. The lines are counted
  from 0, the empty ones too. A line then starts where the line before it ended, and the thread between
  them is short. The glyphs lie where they lie one way, within the difference between a glyph's 2 drawings.
  A font needs glyph files for both directions, and its `reversible` must not be off: 12 of the 142 fonts at
  `c7e3a05` have both. In another font every line sews the default way. Ink/Stitch does that in silence.
  StitchCraft says it with `SC-W1110` when the text has a line to sew back, since the thread then crosses the
  text from each line's end to the next line's start. A character that the reverse direction lacks sews
  as the default glyph, as any missing character does.
- **Trims.** `trim` chooses where lettering adds a trim, and `off` is the default, as in Ink/Stitch. With
  `glyph` a trim follows each glyph. With `word` it follows the last glyph sewn of each word, and with `line`
  of each line, but never the text's last glyph. A trim is the setting `trim_after` on the last element the
  glyph sews, which the planner reads as it reads any element's. A glyph's own trims stay with every choice.
  The planner joins or jumps between 2 letters without a trim as it does between any 2 elements, for the
  machine's profile.
- **Sorted colours.** A multicolour font sews each letter in several threads, and its author numbers each
  element's colour with `color_sort_index`. In a font whose `sortable` is on, `color_sort` sews the elements
  in the order of those numbers, and those of one number in the order they sewed in. `all` sorts the whole
  text's elements, and `line` and `word` sort each line's or each word's. An element without a number sorts as number 404,
  after the others, as in Ink/Stitch. The elements of a number that `combine_at_sort_indices` lists become
  one element, the first, with the subpaths of the others. A tartan or a linear gradient fill made so gets
  `stop_at_ending_point`, as Ink/Stitch sets it. Sorting moves no glyph. A glyph's colours no longer sew
  together once sorted, and a trim then follows the last element of each of them. In a font
  that doesn't sort, the text sews as it is drawn and `SC-W1110` says so. The numbers are lettering's to
  read, and it takes them off the design's elements, sorted or not: the planner has no setting of that name
  and would warn of it on every element.
- **Auto-satin.** In a font whose `auto_satin` is on, when the text has a satin column, the text's satin
  columns and strokes are routed before the trims are added and the colours sorted. They are routed by `stitchcraft_engine::route` as Auto-Route Satin Columns routes them with
  Preserve order on and trims off ([routing tools](routing.md)). The route's elements replace them. Each
  sews at the end of the glyph its element was in, after the glyph's fills, where Ink/Stitch puts it at the
  end of the group it was in. A text without a satin column sews as its glyphs draw it. The routing's
  warnings are passed on, and its notes aren't.
- **Bounded.** Any text, any path and any numbers give a design or `SC-E1108`, never a panic. The layout,
  the seats on a path, the placing, the trims and the sort charge one meter of the budget, and the routing
  has the budget again.

**The settings** are named as Ink/Stitch names them. Its batch lettering has `scale`, `color_sort`, `trim`,
`text_align`, `letter_spacing`, `word_spacing`, `line_height`, `text_position` and `vertical_text_position`,
and its lettering window has `back_and_forth`. `lettering::SETTINGS` declares each once with its help, and
the options of `stitch lettering` take their help from it ([command line](../user/reference/cli.md)).

### Along a path

`lettering::set_along` sets a text along a path. Each glyph is moved to a seat on the path and turned, and
the text lies in the path's millimetres.

- **The path** is the first subpath of the path given, followed within 0.01 mm. Ink/Stitch takes the first
  subpath too. A closed path is followed from its start, round to its start again.
- **A glyph is never bent.** It is a small embroidery, with satin columns whose stitches were set
  square to them, and it is moved and turned as a whole. The point where its box's left edge meets its
  baseline lies on the path. The glyph is turned about that point, the way the path runs from there to the
  point its box's width further along. Its baseline is then the chord of the path under it. On a straight
  stretch the glyph stands on the path. On a bend its ends touch the path, and across a sharp corner it
  bridges the corner. The glyphs before and after a sharp corner can overlap on the corner's inner side.
  Both programs leave them so: round the corner, or space the letters.
- **Along the path** the glyphs of a line keep the distances the layout gives them, measured from the
  line's leftmost box. `text_position` says where the line starts. With `left` it starts at the path's
  start. With `center` it is centred on the path's length, and with `right` it ends at the path's end.
- **Stretched.** With `stretch` the line starts at the path's start, and what the path is longer than the
  line is shared out equally: one share between 2 glyphs of a word, and 2 between 2 words.
- **Across the path.** `vertical_text_position` says which part of the text lies on the path. With `top`,
  the default, the text stands on the path by its lowest point, a descender's end if it has one. With
  `baseline` the first line's baseline lies on the path. With `bottom` the text hangs from the path by its
  highest point. The text's lowest and highest points are measured on the points of its glyphs' paths.
- **Lines.** Each later line lies `leading` plus `line_height` further across the path, at the places along
  the path it would have as the first line. On a straight path that is a parallel line. On a bend, the
  letters of a line on the bend's inner side lie closer together and can touch, and those on its outer side
  lie further apart. Lines sew back and forth along a path as they do off it.
- **A line longer than its path** is set whole. On an open path it goes on straight past the end, the way
  the path ends, and before the start when the line is centred or ends at the path's end. On a closed path
  it goes round again, over its own start. A stretched line has its letters pushed together to fit. Each
  time `SC-W1109` gives the line's length and the path's.
- **A path without length** has nothing to follow, and the text is refused with `SC-E1108`.
- **From the command line** `--path FILE` names an SVG file. The path is the one labelled `batch lettering`,
  the label Ink/Stitch's batch lettering looks for. In a file without that label it is the first path that
  paints, and the summary names the path taken. The text lies where the path lies on the file's page.

**How Ink/Stitch does each**, from running its batch lettering at `d59c9ab` on paths drawn for it:

- It seats and turns a glyph the same way, by the chord under its box. A word on an arc, on a line and
  across a right angle has the same size in both programs to 0.1 mm, in each position
  ([measured](#measured-against-inkstitchs-lettering-along-a-path)).
- A line longer than its path has every glyph past the end at the path's end, unturned, one over the other.
  A centred line that would start before the path's start is counted from the path's end. StitchCraft sets
  the line on and warns (`DEV-FONT-004`).
- A text of 2 lines comes out with its first line only. The code moves each later line onto a curve parallel
  to the path, which no file it wrote showed. Along a path Ink/Stitch sorts no colours (`DEV-FONT-005`).
- A closed path is followed from its start, as an open one.

**What M14.2 chose.**

- *Advances and kerning by the whole label.* Ink/Stitch's lettering looks a glyph's advance and kerning up
  by its label, and takes a label longer than 11 characters from its 12th character on. It looks the advance
  of Allegria 55's glyph `Inkscape_logo` up as `go`. StitchCraft looks it up by the whole label
  (`DEV-FONT-001`).
- *The 3 fonts whose glyph files are centred.* The reader's boxes of Califragilistic, Cooper Marif and
  InfiniPicto lie right of Ink/Stitch's (`DEV-SVG-025`), and the layout adds a glyph's left edge for every
  glyph but a word's first. Their words are set wider than in Ink/Stitch by that offset between the first
  2 letters of each word. No lettering case covers them, and the step leaves them so.
- *Arabic forms wait.* A glyph named with `.isol`, `.init`, `.medi` or `.fina` is a form of an Arabic
  letter, which Ink/Stitch chooses by the letter's neighbours. StitchCraft chooses none yet. The letter
  is then a missing character and is named so, which keeps the gap loud.
- *Where the code lies.* The layout is in `stitchcraft-svg` beside the reader, since it needs the reader's
  fonts and the engine's routing and nothing else. `Path::map` of the engine became public for it.
- *Design settings.* The text's design has the default design settings, as Ink/Stitch's batch lettering
  sets a text in an empty document. A glyph file's own settings aren't the text's.

**Measured against Ink/Stitch's lettering.** On 16 November 2026 both programs set words in Ink/Stitch
Small Font and Medium Font. Ink/Stitch at `d59c9ab` ran as a black box. Its batch lettering runs without
a window and wrote each text as an SVG file and as a PES file. StitchCraft wrote its PES file with `stitch lettering`
for the Brother PE800.

- *Places.* The 11 glyphs of `Stitch Craft` lie at the same places in both fonts within 0.0001 mm, the last
  digit Ink/Stitch writes. Ink/Stitch's places are the translations of its glyph groups. A tolerance of
  0.001 mm is a hundredth of a machine file's unit.
- *Stitches.* The table gives each file's size, stitches, jumps and trims, read back from the PES file.
  The sizes agree to the file's 0.1 mm, and the jumps and trims are the same. The stitch counts differ by
  1.5 percent at most. Each program leaves out needle points closer than the shortest stitch in its own
  way, which the glyph differential records for single glyphs.

| Font | Scale | Text | StitchCraft | Ink/Stitch |
|---|---|---|---|---|
| Small Font | 70 % | `Hello` | 14.2 × 3.8 mm, 297 stitches, 1 jump, 0 trims | 14.2 × 3.8 mm, 293, 1, 0 |
| Small Font | 100 % | `Hello` | 20.2 × 5.4 mm, 401, 1, 0 | 20.2 × 5.4 mm, 407, 1, 0 |
| Small Font | 300 % | `Hello` | 59.8 × 15.6 mm, 1,113, 5, 4 | 59.8 × 15.6 mm, 1,116, 5, 4 |
| Small Font | 100 % | `Stitch Craft` | 51.4 × 6.2 mm, 875, 4, 3 | 51.4 × 6.2 mm, 874, 4, 3 |
| Medium Font | 75 % | `Hello` | 44.0 × 11.8 mm, 792, 5, 4 | 44.0 × 11.8 mm, 800, 5, 4 |
| Medium Font | 150 % | `Hello` | 87.6 × 23.4 mm, 1,473, 5, 4 | 87.6 × 23.4 mm, 1,459, 5, 4 |

`Stitch Craft` in Small Font at 300 percent is 153.6 mm wide in both programs, wider than the PE800's hoop,
and `stitch lettering` refuses it with `SC-E0701`. The comparison was run by hand, and the 3 differences
the step leaves are in the ledger (`DEV-FONT-001` to `DEV-FONT-003`).

**What M14.3 chose.**

- *Trims by the glyphs' words.* Ink/Stitch finds a word's end by counting the glyphs against the text's
  characters, and a ligature, one glyph for 2 characters, shifts its trims. StitchCraft goes by the words the
  layout laid (`DEV-FONT-006`).
- *Lines across the path.* A later line is pushed across the path glyph by glyph, where Ink/Stitch's code
  means to follow a parallel curve. The parallel of a path with corners needs its joins and its loops cut,
  and Ink/Stitch's own batch lettering never showed a second line to compare with.
- *The warning for one direction.* `back_and_forth` is on by default, as in Ink/Stitch, and most fonts sew
  one way. A text of several lines in such a font is warned of each time. The thread across the text is
  what the owner of a machine without a cutter has to cut, and the warning is where she learns of it.
- *A fill's angle isn't turned.* A glyph's elements keep their settings on a path, as they do when scaled.
  A fill's `angle` is among them, and its rows keep their direction on the fabric while the glyph turns.
  Ink/Stitch Small Font and Medium Font have no fills. A font with fills along a bend waits for a later step.
- *No case in the nightly differential.* The differential runs one Ink/Stitch extension, the one that
  writes a stitch plan from a design, on files in the repository. Lettering needs another extension and a
  font, and fonts aren't in this repository (ADR-0020). A case would need the font corpus fetched in that
  job and a second runner. The comparison stays by hand, with the commands in the hand-off notes.

#### Measured against Ink/Stitch's lettering along a path

On 16 November 2026 both programs set texts in Ink/Stitch Small Font at 300 percent along paths, as in the
measurement above. The arc is half a circle of radius 50 mm, from its left end over the top. The line is
100 mm long, and the corner is 30 mm across and then 40 mm down. Each cell gives the file's size, stitches,
jumps and trims.

| Path | Text | Position | StitchCraft | Ink/Stitch |
|---|---|---|---|---|
| arc | `Hello` | left, top | 46.4 × 56.2 mm, 1,121, 5, 4 | 46.4 × 56.2 mm, 1,114, 5, 4 |
| arc | `Hello` | centre, baseline | 66.8 × 23.4 mm, 1,119, 5, 4 | 66.8 × 23.4 mm, 1,117, 5, 4 |
| arc | `Hello` | right, bottom | 36.2 × 47.8 mm, 1,122, 5, 4 | 36.2 × 47.8 mm, 1,121, 5, 4 |
| arc | `He llo` | stretch, top | 129.2 × 62.0 mm, 1,114, 5, 4 | 129.2 × 62.0 mm, 1,119, 5, 4 |
| line | `Help` | left, top | 51.6 × 19.4 mm, 1,037, 4, 3 | 51.6 × 19.4 mm, 1,041, 4, 3 |
| line | `Help` | left, bottom | 51.6 × 19.4 mm, 1,037, 4, 3 | 51.6 × 19.4 mm, 1,042, 4, 3 |
| corner | `Hello` | left, top | 46.0 × 47.0 mm, 1,134, 5, 4 | 46.0 × 47.0 mm, 1,115, 5, 4 |

The other settings were measured off a path. Magnolia bicolor is a font of 2 colours that sorts.

| Font | Scale | Text | Setting | StitchCraft | Ink/Stitch |
|---|---|---|---|---|---|
| Small Font | 300 % | `Hello`, `World` | back and forth | 70.4 × 39.4 mm, 2,415, 10, 9 | 70.4 × 39.4 mm, 2,426, 10, 9 |
| Small Font | 100 % | `Hi yo`, `We` | no trim | 21.4 × 14.0 mm, 625, 4, 3 | 21.4 × 14.0 mm, 628, 4, 3 |
| Small Font | 100 % | `Hi yo`, `We` | trim after lines | 625, 4, 3 | 628, 4, 3 |
| Small Font | 100 % | `Hi yo`, `We` | trim after words | 625, 4, 3 | 628, 4, 3 |
| Small Font | 100 % | `Hi yo`, `We` | trim after letters | 641, 6, 5 | 643, 6, 5 |
| Magnolia bicolor | 100 % | `Ab cd` | unsorted | 7 colour changes | 7 |
| Magnolia bicolor | 100 % | `Ab cd` | sorted, all | 1 colour change | 1 |
| Magnolia bicolor | 100 % | `Ab cd` | sorted by word | 3 colour changes | 3 |

A trim after each line or word changes nothing in that text: its words and lines lie more than 3 mm apart,
and the planner jumps between them with or without a trim. A trim after each letter cuts the 2 joins
that auto-satin made inside `Hi` and `yo`. Magnolia bicolor's text is 106.8 × 33.6 mm here and
107.0 × 34.2 mm in Ink/Stitch, with one jump fewer, sorted or not. The glyphs differ so before any sort,
as the glyph differential records, and the sort doesn't change it.

**Jumps between letters.** With auto-satin, the letters of a word are joined by the shortest way from one
to the next. As in Ink/Stitch, a way shorter than the design's `collapse_len`, 3 mm, sews as a stitch on
the fabric between the letters. Small Font is drawn for that, and at its small sizes a word
sews without a jump. At large sizes the gaps grow past 3 mm and become jumps, whose threads the PE800
leaves to cut by hand.


## Lettering in the place of a picture's text

From M14.5, `stitchcraft_svg::lettering::pictures` sets a text where a picture's traced letters were, at
their height and along their line, with [`set`](#lettering) and [`set_along`](#along-a-path) as they are.
[Text in pictures](image-to-embroidery.md#text-in-pictures) describes it. It asks a host for a font by
name, as a text object does, and the empty name is the default font.

## Text objects in automatic digitizing

M14.7 sews a text object that automatic digitizing meets (`REQ-AUTO-069`, `REQ-AUTO-070`). A text object
is what a drawing keeps of a text someone typed. It has the text's characters and the name of its
typeface, with its size and its place. An SVG file's `<text>` element is one, and so is a VectorCraft document's point type. Until
M14.7 automatic digitizing left both out with a warning. Text that was made into paths before it reached
StitchCraft is shapes, and [text in pictures](image-to-embroidery.md#text-in-pictures) handles those.

**An embroidery font is used in the typeface's place.** The outlines of a text object's letters aren't in
the drawing. They are in the typeface's font file, and StitchCraft doesn't read font files before
[M15.2](../plan/roadmap.md#m15--fonts-ported-automatically), nor does VectorCraft give
a plug-in a text's glyphs. So the text is set by [lettering](#lettering), in the host's default embroidery
font, which is Ink/Stitch Small Font where the fonts are Ink/Stitch's. The word sews cleanly, where the
drawing has it and about as tall. Its letters have the embroidery font's shapes and widths, not the
typeface's: a bold display face and a script both come out as Small Font's light upright letters. A note
says so for each text object (`SC-I1116`). Roadmap step M15.13 sews a text in its own typeface.

`stitchcraft_svg::lettering::objects` does it. Each reader gives what it read as a `TextObject`, which is
lines, and `letter` sets each line with [`set`](#lettering). There is no second layout.

**How tall.** A font size is the height of the box a typeface is drawn in, and no letter is that tall. An
embroidery font's `size` is the height of its letters. How tall a typeface draws its capitals is written
in its font file, which isn't read. In the 7 fonts measured below it is between 0.64 and 0.73 of the font size. So a line is set so
that the embroidery font's letters are 0.7 of its font size tall (`objects::CAPITALS`), to a tenth of a
percent. For those fonts the lettering is within 0.06 of the font size of the capitals it replaces,
which is 1.2 mm for a text 20 mm in size, unless its line is fitted to a shorter place. The scale stays within the heights the font sews at, and
outside them `SC-W1106` gives the height the text is set at. Small Font sews from 3.6 to 15.2 mm, so text
from 5.1 to 21.7 mm in font size, 14.6 to 61.5 points, sews at its height.

**Where.** These hold exactly:

- A line's baseline is the text's, and the lines are turned as the text's baseline is.
- A line starts, is centred or ends at its anchor, as the drawing aligns it.
- Each line of a text of several lines is placed on its own, where the drawing has it.

The note gives the height each text sewed at and why: 0.7 of its font size, or made smaller to fit a
length, which it names as stated or estimated. It also gives the longest line's length as sewn.

**How long.** The embroidery font's letters are as wide as it draws them, and Small Font is a wide font.
On a badge drawn for the step, a title of 8 characters at a font size of 18 mm was 98.7 mm long
at 12.6 mm tall, which is 0.69 of the font size for each character. The badge left the line 73 mm, and
the lettering ran over the badge's border. A design sewn so is wrong, and worse than one without its text.
So a line is fitted to the length its text most likely had in the drawing (`objects::fit`):

1. **That length** is the one the drawing states, where it states one, as an SVG file's `textLength`
   does. A VectorCraft document states none. VectorCraft measures a text object's box and doesn't save it:
   `cached_bounds` is marked `#[serde(skip)]` in `crates/doc/src/text.rs`, read at v0.24.1. Without
   a stated length it is estimated at half the font size for each character of the line, spaces included
   (`objects::WIDTH`).
2. **A line that is no longer than that** is left as it is. It isn't stretched.
3. **A longer line has its gaps narrowed first,** each by up to a tenth of the letters' height, the limit
   by which [lettering closes up to traced letters](image-to-embroidery.md#text-in-pictures).
4. **Then it is set smaller,** with its gaps as narrow for its height, at the scale that makes it as long
   as its place, cut to a tenth of a percent.
5. **It is never set smaller than the font sews.** A line that is still longer at the font's smallest
   height is set at that height, and `SC-W1117` gives its length and its place's.

The anchor and the baseline are the same through all of it, and a centred line is centred on its anchor
still. On the badge, "Camp
Fox" now sews 10.0 mm tall and 72.0 mm long, inside the badge.

The estimate is for the proportional typefaces text is commonly set in. It was measured on 7 fonts
installed on the build machine, with their own advances and kerning, as a line's length over its font size
and its count of characters. None of the fonts is in the repository.

| Font | Capitals' height | 4 lines of small letters | A line of capitals |
|---|---|---|---|
| FreeSans, with Helvetica's widths | 0.73 | 0.44 to 0.56 | 0.61 |
| FreeSerif, with Times's widths | 0.66 | 0.41 to 0.52 | 0.61 |
| Carlito, with Calibri's widths | 0.64 | 0.42 to 0.49 | 0.50 |
| Caladea, with Cambria's widths | 0.67 | 0.42 to 0.50 | 0.54 |
| DejaVu Sans | 0.73 | 0.52 to 0.62 | 0.61 |
| DejaVu Serif | 0.73 | 0.53 to 0.63 | 0.68 |
| Inter | 0.73 | 0.49 to 0.59 | 0.62 |

The lines were 4 short lines of 2 or 3 words and a pangram, and
the capitals "HAPPY BIRTHDAY". Half the font size lies in the middle of the small letters' range. It is
short for capitals and for wide faces, where a line comes out up to a quarter shorter than it was drawn,
and smaller with it. A line too short is inside its place, and a line too long is over its neighbours, so
the estimate errs short. The same fonts' capitals are 0.64 to 0.73 of the font size tall.

**The thread** is the text's fill colour when the font sews in one thread, as it is for a text with the
Lettering effect. A font of several threads keeps its own.

**What sews.** The letters keep the settings the font's author gave them. They aren't automatic elements,
so the rules for shapes don't touch them, and they sew at the text's place in the stitching order, cut to
the clips around it.

**What each reader reads.** [The SVG adapter](svg-input.md#text-objects) reads a `<text>` element, and
[the VectorCraft adapter](vectorcraft-integration.md#text-without-the-lettering-effect) reads point type.
Each page says what its reader leaves out.

**All or nothing.** A text object sews every line or none. It sews nothing when the host has no default
font, when the font doesn't read, when a line can't be set, or when its letters would lie beyond the
design's reach. `SC-W1111` then says why, and names the fonts the host has. A text object without a
visible character sews nothing and nothing is said, since a viewer doesn't show it either.

**No setting turns it off.** A design sews without its text when the text is hidden or deleted in the
drawing, as any shape is left out. A setting would be one more row in the Auto effect's dialog for
something the drawing already says.

**A text with the Lettering effect** (M14.4) keeps the font and the settings a person chose for it.
Automatic digitizing doesn't set it again, and `SC-I1116` isn't said of it.

## Fonts built into the plug-ins

A plug-in can't read files. The fonts that a VectorCraft document's texts sew in are built into the
exporters (M14.4, `REQ-FONT-021`, [lettering in VectorCraft](vectorcraft-integration.md#lettering)).

- **The build option.** The plug-in's build script reads `STITCHCRAFT_FONTS_DIR`, which names a folder
  of fonts: a checkout of the font repository's `fonts` folder, or the corpus's `src`. For each font that
  `apps/stitchcraft-vc-plugin/built-in-fonts.txt` lists, it includes the font's `font.json`, its glyph
  files and its licence file from where they lie. A preview picture and a README are left out. Without the option no
  font is built in, and everything builds, tests and runs. A case that needs a font built in says it is
  skipped. A font that the list names and the folder lacks stops the build, since a plug-in that silently
  lacked it would leave texts out.
- **Only the exporters contain them.** The modules are built from the same code, and the linker leaves out
  of each what its run doesn't use. Only an exporter's run uses the fonts.
- **Where `stitch` finds them.** In the folder `STITCHCRAFT_FONTS` names, where each folder that has a
  `font.json` is a font. A font is found by the same names there, so `stitch export` writes the file the
  exporter writes. The option `--fonts` and the user's data folder, which
  [ADR-0020](adr/0020-fonts-from-a-repository-of-their-own.md) plans, aren't built yet.
- **Where CI finds them.** `fonts.toml` doesn't pin the font repository yet. Until it does, the jobs that
  build the modules for the compatibility gate and for the browser demo take the fonts from the pinned
  corpus of Ink/Stitch's fonts, whose files the font repository copies whole (`.github/actions/fonts`, `build-in`).

**Which fonts.** The browser demo publishes the modules, and a published exporter shares the fonts in it.
So a font is listed only when its licence lets a copy be shared with the licence's text beside it and
asks for nothing more: the SIL Open Font License, and CC0 or the public domain. One font is listed:

| Font | Folder | Licence | Its files | What it adds to each exporter |
|---|---|---|---|---|
| Ink/Stitch Small Font, copyright 2017 Lex Neva | `small_font` | SIL Open Font License 1.1 | `font.json`, `ltr.svg`, `rtl.svg`, `LICENSE` | 829,061 bytes, as stored |

It is the default font: 5.08 mm tall at 100 percent, sewn from 70 to 300 percent, with glyphs for both
directions. Its lines sew back and forth. Its glyph files are text, which a web server sends
compressed. With the font, and with the SVG reader and lettering that read it, the demo's PES exporter
grew from 3.5 to 5.4 MB as stored, and from 1.2 to 1.7 MB as sent.

**What was checked,** on 16 November 2026, for Small Font at `c7e3a05`:

- Its `LICENSE` names the SIL Open Font License 1.1 first, with Lex Neva's copyright, and has the
  licence's whole text. It doesn't name a Reserved Font Name.
- The files are built in as they are, so the copy isn't a modified version.
- The OFL lets the font be bundled and redistributed with any software, when each copy has the copyright
  notice and the licence. The licence file is built into the module beside the glyph files. The demo's
  build also copies it to `fonts/small_font/LICENSE` on the site and lists it in `plugins.json`, and it
  fails for a font without a licence file.
- The test `req_font_021_…` reads every font built in, and fails for one whose licence file names
  another licence first.

**CC BY and CC BY-SA fonts** are left to the owner. Either licence requires a credit where a person who
gets the module can see it. The credit names the author and the licence, with the licence's address, and
says what was changed. The site would publish it beside the licence file. An export would have to say it
too, as `stitch lettering` is to print it (ADR-0020). CC BY-SA also requires that an adaptation be shared
under the same licence. A module with the font's files unchanged in it is a collection by the licence's own
terms, and a collection is no adaptation, and whether a sewn design is one is for the owner to decide before such a font
joins. Geneva Rounded, for one, is under CC BY-SA 2.5 and would add 716 KB.

## Fonts ported by other programs

| Program | With a TrueType or OpenType font | Reported results |
|---|---|---|
| Wilcom EmbroideryStudio | Converts an installed font as text is typed, or a whole font into an embroidery font. It cuts letters into column objects and sets their stitch angles. It finds where strokes overlap and the order they sew in. Turning strokes follow each stroke, and a complex fill sews at one angle. Options turn stitches parallel to slanted ends (K, X, Y), make serifs broader than 0.8 mm strokes of their own, cap sharp angles (A, V) and overlap strokes at T and O junctions. Converted letters can be edited and the font saved | Comparable to fonts digitized by hand, though "the quality may not be quite as good". Narrower serif fonts convert better than blocked ones |
| BERNINA Embroidery Software | Stitches text in the TrueType and OpenType fonts installed on the computer | "Not all TrueType fonts are suitable" |
| Hatch Embroidery | Generates stitches from an installed font's outlines | Test-stitch each, since not all suit embroidery. Its digitized fonts state ranges, such as 6 to 30 mm for a serif, 12 to 50 mm for a script and 4 to 6 mm for a block font in running stitch |
| Embird Font Engine | Fills letters with satin columns ("auto column") or fills, from about 5 mm to 30 cm tall. It replaces each satin stitch longer than 12 mm | Its maker claims lettering "almost as if it was digitized by a human digitizer" |
| SewWhat-Pro, Stitch Era | Convert text in installed fonts | A reviewer found SewWhat-Pro's results "abysmal". A test of Stitch Era found its TrueType lettering best at 8 to 10 mm, and smaller letters poor |
| Brother PE-Design 11, Embrilliance | Tools to digitize letters by hand. Embrilliance's BX fonts are digitized letters | None |

Trade articles on small lettering ask for capitals at least 6.4 mm tall, mixed case at least 5 mm and all
capitals at least 3.8 mm ([PI World](https://www.piworld.com/embroidery-digitizing-tips-on-creating-designs-with-letters/all)).
Its other minimums are 0.8 mm for a column's width, 0.9 mm across a counter and 0.5 mm between letters.
Another article puts small letters on a centre walk with pull compensation, and satin columns at least as
wide as the needle. It advises against serifs and strokes of varying width under about half an inch
([ASI](https://members.asicentral.com/news/how-to/may-2018/keep-your-small-letters-crisp/)).

**Published methods.** VSM's patent US 6,934,599 (priority 2002, expired 2023) describes porting a font
automatically. A glyph's contours are grouped and cut at sharp corners and at nodes of a thinned skeleton
until each part is simple. Each part sews as a straight or curved satin column or as a running stitch, its
stitches square to the skeleton. The parts are ordered by how they connect. Neighbouring columns are merged,
with jumps between the rest. M13's [centre lines](algorithms/centre-lines.md) and columns are a pipeline of
that kind. Brother's US 5,559,711 (expired 2014) chooses satin or another stitch by a region's roundness, its
area over its perimeter squared. StrokeStyles (Berio and others, 2022) splits glyphs of many scripts into
overlapping strokes, from the medial axis, the outline's convex and concave parts and 7 kinds of junction,
without training data. Balashova and others (2019) fit a model of strokes to existing fonts. A thesis of 1998
at National Taiwan Normal University digitized embroidery from medial axes, a neural network classifying the
junctions.

## Font files

| Crate | Licence | What it gives |
|---|---|---|
| skrifa 0.49 | MIT OR Apache-2.0 | Glyph outlines of TrueType and CFF fonts, unhinted at any size or hinted, at any point of a variable font's axes. VectorCraft's text uses 0.47 |
| read-fonts 0.46 | MIT OR Apache-2.0 | The tables that skrifa and harfrust read |
| harfrust 0.15 | MIT | HarfBuzz's shaping in Rust, with advances, kerning from `kern` and GPOS, ligatures, marks and Arabic forms. VectorCraft's text uses 0.13 |
| write-fonts 0.55 | MIT OR Apache-2.0 | Fonts built in code for tests, as VectorCraft's tests build theirs |
| ttf-parser 0.25 and rustybuzz 0.20 | MIT OR Apache-2.0, and MIT | The older pair, last released in November 2024 |
| fontdb 0.24 or fontique 0.12 | MIT, and Apache-2.0 OR MIT | Installed fonts found by family name, for the command line |

Each licence is on the allow-list of `deny.toml`. At the versions VectorCraft uses, skrifa, read-fonts and
harfrust forbid `unsafe` code and build without the standard library, and VectorCraft builds them for its
web app. A font file is never committed to StitchCraft. Tests build their fonts in code with write-fonts and
read a corpus downloaded at pinned commits.

- **Hinting** moves an outline's points onto a screen's pixels at small sizes, which means nothing to a
  needle. A port reads outlines unhinted, in font units, and scales them with StitchCraft's own arithmetic
  ([determinism](determinism.md)).
- **Composite glyphs**, such as most accented letters, are placed from their components. A port can
  digitize a base letter once for every glyph made from it.
- **Overlapping contours** are common in composites and in variable fonts, where each stroke is often a
  contour of its own. A viewer fills a glyph by the nonzero rule, and so does the region code of M5.1. Text
  typed in an installed font fails in Ink/Stitch as a fill whose border crosses itself
  ([how people use Ink/Stitch](../plan/inkstitch-usage.md)).
- **Variable fonts** are set at a named instance or at axis values. A weight axis can thicken a font's
  strokes at small heights, and an optical size axis gives the font's design for small text.

**Stroke widths.** In Source Sans 3 Regular, a stem is 83 units at a cap height of 660 (12.6 %). At capitals
8.0 mm tall its stems are 1 mm wide, the narrowest satin column of automatic digitizing. At 4.0 mm they are
0.5 mm wide, where a run is tripled. Inter Regular's stems are 1 mm wide at 7.8 mm, and Source Sans 3 Bold's
at 4.5 mm. Source Serif 4 Regular's stems are 1 mm wide at 7.1 mm and its hairlines at 14.9 mm. Between
those heights its letters mix satin columns and runs. A stem is 7 mm wide, where automatic digitizing fills,
at capitals 56 mm tall in Source Sans 3 Regular and 31 mm tall in its Bold.

**Dots.** At capitals 6 mm tall, Source Sans 3 Regular's i-dot covers 0.79 mm², its full stop 0.99 mm² and
each dot of an ä 0.65 mm². Inter Regular's i-dot covers 0.76 mm². M13 takes a part under 1 mm² for a speck,
and merges it into a neighbour or leaves it out.

## Licences of ported fonts

A font ported into an embroidery font is a modified version of the font. A design sewn in it is a document
made with the font.

- **OFL.** A "Modified Version" includes one made "by changing formats or by porting the Font Software to a
  new environment", and the licence's site counts a conversion to SVG as one. It is distributed under the
  OFL only, never sold by itself, and named without the font's Reserved Font Names. "The requirement for
  fonts to remain under this license does not apply to any document created using the Font Software."
- **UFL 1.0**, the Ubuntu font licence, counts a change of format as a modification too. A version changed
  substantially takes a new name, and another adds "derivative" to the name. The licence "does not affect
  any document created using the Font Software".
- **Apache-2.0** allows modified versions with the licence, its NOTICE file and a note of the changes.
- **Creative Commons.** BY and BY-SA allow adaptations with credit, under the same licence for SA.
  NoDerivatives doesn't allow sharing adaptations, and NonCommercial doesn't allow commercial use.
- **Proprietary fonts** follow their licences. Rare Bird's desktop licence doesn't allow derivative works,
  and requires a supplemental licence for "embroidering or sewing machines". KG Fonts sells an embroidery
  licence for embroidery font files.

A font's name table gives its copyright notice, trademark, licence description and licence URL (name IDs 0,
7, 13 and 14), which a port can read and report. Google Fonts' fonts are under the OFL, the UFL or
Apache-2.0, in a folder for each in its repository.

**Where fonts live.** VectorCraft's shared fonts are in
[storytold/craft-fonts](https://github.com/storytold/craft-fonts), an optional build input
(`CRAFT_FONTS_DIR`), with each font's licence text beside it (VectorCraft's `docs/development.md` at
`1505384`). Embroidery fonts in Ink/Stitch's format come from a repository of their own,
`KaboomFox/stitchcraft-fonts`, as [ADR-0020](adr/0020-fonts-from-a-repository-of-their-own.md) decides. A
ported font is made on the user's computer from a font the user has.

## Glyph rules

M13 digitizes a traced shape by its width at the size it sews ([image to embroidery](image-to-embroidery.md)).
A glyph is such a shape, and M13's rules do most of the work, from centre lines and junctions to rings and
the fills of broad parts. The table lists what glyphs need beyond them, with the step of M15 that adds it.

| Need | M13 today | Step |
|---|---|---|
| Every contour sews | A part under 1 mm² is a speck, and the lowest shape can be the background | M15.3 |
| Dots and marks | A dot's centre line has no length, and the dot is filled, or merged as a speck | M15.5 |
| Ends on the baseline, the x-height and the capitals' height | A column's end is flat when the edge lies within 20° of square to it, and pointed otherwise | M15.6 |
| Serifs | A serif is an arm with a column of its own, or a bump | M15.6 |
| Thin strokes | A part narrower than 1 mm runs along its centre line. Small fonts digitized by hand sew satin columns half as wide | M15.7 |
| Counters and the settings of small letters | Columns are compensated by 0.4 mm at every height, and a counter that closes isn't named. Small fonts digitized by hand compensate by 0.05 to 0.1 mm | M15.7 |
| Jumps inside a glyph | Columns sew longest first, and a jump joins each 2. TS-13B's digitized letters leave 37 jump threads | M15.8 |
| Joined letters and overlapping strokes | Contours are united before their centre lines are found | M15.9 |
| Heights | A shape is digitized at the size it sews | M15.10 |

**Correcting a glyph.** A ported font can be written in Ink/Stitch's format (M15.11), with its satin columns
drawn with their rails and rungs as Stroke to Satin draws them. A user then corrects a glyph in its layer, in
VectorCraft or in Inkscape with Ink/Stitch, and M14's lettering sews the corrected font. VectorCraft keeps
`inkstitch:*` attributes from SVG import to SVG export ([RFC](rfc-vectorcraft-abi-v2.md), P7). The writer
puts the baselines at 0, where a reader puts a baseline when the file lacks a `baseline` guide. Wilcom's
converted letters are corrected the same way, as letters of the saved font.

**Testing.** 36 of Ink/Stitch's fonts were digitized by hand from fonts on Google Fonts, all 36 under the OFL.
The corpus pairs each original with the Ink/Stitch font made from it. A port at that font's heights is
measured against it, by the share of each glyph's area sewn as satin columns, fills and runs, and by jumps,
coverage and stitches. TS-15 sews 3 ported fonts beside the fonts digitized by hand from them, at MC-10
([machine testing](../plan/machine-testing.md)).

## Decisions for the owner

- **The font repository and its licences, and the reader** (M14.1): decided. The owner accepted
  [ADR-0020](adr/0020-fonts-from-a-repository-of-their-own.md) on 2026-11-08, as it was written. The owner creates
  `KaboomFox/stitchcraft-fonts` before M14.2 pins it, and reads the licence files of the 10 fonts the ADR
  names before they join.
- **Spacing in millimetres of the text or of the font** (M14.2). `--letter-spacing 1` at a scale of 50
  percent adds 0.5 mm, as in Ink/Stitch, whose documents M14.4 will read. The recommendation, taken, is to
  keep Ink/Stitch's meaning. Millimetres as sewn are the other choice.
- **`--height`** (M14.2). `stitch lettering` takes the size as `--scale`, Ink/Stitch's setting, or as
  `--height` in millimetres over the font's `size`. The recommendation, taken, is to keep both.
- **Back and forth by default** (M14.3). `back_and_forth` is on unless it is turned off, as in Ink/Stitch.
  A text of several lines in a font drawn for one direction then gets a warning each time. The
  recommendation, taken, is to keep Ink/Stitch's default and the warning. Off by default is the other choice,
  with no warning and a long thread between the lines of every font.
- **A text longer than its path** (M14.3) is sewn on past the path's end, with a warning. The other choice
  is to refuse it, with no file written. The recommendation, taken, is to sew it: the picture shows by how much
  the text is too long.
- **Which fonts are built into the plug-ins** (M14.4). The recommendation, taken, is Ink/Stitch Small Font
  alone, under the OFL ([fonts built into the plug-ins](#fonts-built-into-the-plug-ins)). Each further
  font adds its glyph files to both exporters. Ink/Stitch Medium Font, also under the OFL, would add 773
  KB. A line of `built-in-fonts.txt` adds one. Fonts under CC BY or CC BY-SA wait for the owner's word on
  their credit.
- **The font's name typed, or chosen from a list** (M14.4). The Lettering effect's dialog takes the font's
  name as text. The recommendation, taken, is to keep the text field while one font is built in, since a
  document then sews the same in every build and from `stitch`. A list is easier with many fonts, and
  needs the same fonts in every build that opens the document.
- **Pinning the font repository** (M14.4). CI builds Small Font in from the corpus of Ink/Stitch's fonts
  until `fonts.toml` pins `KaboomFox/stitchcraft-fonts`. The recommendation is to pin it before a second
  font is built in.
- **Text objects sew in an embroidery font under automatic digitizing** (M14.7), until a typeface's
  outlines can be read (M15.13). The recommendation, taken, is to letter every text object automatic
  digitizing meets, with a note each time. The other choice leaves text out with a warning, as before, and
  a design then sews without its words.
- **How tall a text object's lettering is** (M14.7). The recommendation, taken, is 0.7 of the font size,
  about the capitals' height in most typefaces
  ([text objects in automatic digitizing](#text-objects-in-automatic-digitizing)). Small letters then come
  out a little taller than the drawing's, since an embroidery font's small letters are tall for its
  capitals. The other choice sets the whole font size as the letters' height, which sews every text about
  4 tenths larger than it is drawn.
- **Lettering longer than the drawn line** (M14.7): decided on 2026-11-16. A line is fitted to the length
  its text most likely had, a stated length or half the font size for each character
  ([text objects in automatic digitizing](#text-objects-in-automatic-digitizing)). Sewing every line at
  its full height, with its length in the note, was the other choice, and it sewed the badge's title over a
  badge's border. The estimate sews some lines smaller than they need be. M15.13 reads the real widths.
- **No setting turns text off in automatic digitizing** (M14.7). The recommendation, taken, is none: hiding
  the text in the drawing leaves it out. An `auto_text` setting is the other choice.
- **When M15 comes.** The roadmap places it after M14 and before M11 and 1.0. M15.1 and M15.2 can go ahead
  beside M14. Moving M15 after 1.0 brings 1.0 sooner.
- **Which ported fonts are written.** Text in any installed font can be sewn as a design, as Wilcom and Hatch
  sew it. The font's licence is reported. Writing a ported font is making a modified version of the font.
  The plan writes one for a font under the OFL, the UFL, Apache-2.0, CC0, CC BY or CC BY-SA, and for another
  font only when the user says they hold a licence that allows it. The owner can make that stricter or
  looser.
- **Ported fonts in the font repository.** Ports of OFL fonts, corrected by hand and sewn out, could join
  StitchCraft's font repository under new names, as Ink/Stitch's library grew from such fonts.
- **The narrowest satin column in lettering.** Automatic digitizing runs a line narrower than 1 mm, and
  Ink/Stitch's small fonts sew columns about 0.5 mm wide. TS-15 sews both, and MC-10 chooses.
- **Raising P9.** The RFC's [P9](rfc-vectorcraft-abi-v2.md#p9-type-as-glyph-outlines) proposes that
  VectorCraft give plug-ins type as glyph outlines, which its plans don't cover. Without it, the exporter
  plug-in can't sew type in the type's own font, since it can't read font files. From M14.4 it sews type in
  an embroidery font built into it. Whether to raise P9 on VectorCraft's own
  repository, and when, is the owner's choice.

## Sources

- Ink/Stitch's tutorial [Fonts for Ink/Stitch](https://inkstitch.org/tutorials/font-creation/)
- Ink/Stitch's [font tools](https://inkstitch.org/docs/font-tools/)
- Ink/Stitch's [lettering](https://inkstitch.org/docs/lettering/)
- Ink/Stitch's [satin tools](https://inkstitch.org/docs/satin-tools/) and [stroke tools](https://inkstitch.org/docs/stroke-tools/)
- The page of Ink/Stitch's [Small Font and Medium Font](https://inkstitch.org/fonts/ink_stitch)
- Ink/Stitch's release notes for [v3.3.0](https://inkstitch.org/en/2026-07-31-v3.3.0)
- The font files of [inkstitch/embroidery-fonts](https://github.com/inkstitch/embroidery-fonts) at `c7e3a05`
- Wilcom's [Converting TrueType fonts to embroidery](https://docs.wilcom.com/embroiderystudio/28/en/OnlineHelp/Lettering/lettering_custom/lettering_custom-2.htm)
  and [Convert TrueType fonts](https://docs.wilcom.com/embroiderystudio/28/en/OnlineHelp/Lettering/lettering_custom/lettering_custom-4.htm)
- Wilcom's [Experiment with conversion settings](https://docs.wilcom.com/embroiderystudio/27/en/OnlineHelp/Management/custom_fonts/custom_fonts-4.htm)
  and [Refine turning stroke options](https://docs.wilcom.com/embroiderystudio/28/en/OnlineHelp/Management/custom_fonts/custom_fonts-5.htm)
- BERNINA's [Convert fonts to embroidery](https://softwarehelp.bernina.com/en/MainHelp/PART_Lettering/special_lettering/special_lettering-4.htm)
- Hatch's [lettering sizes](https://hatchembroidery.com/resources/blog/choosing-the-right-size-embroidery-lettering-for-your-project)
  and its [note on TrueType fonts](https://help.hatchembroidery.com/portal/en/kb/hatch/tech-talk/articles/what-should-i-know-about-hatch-fonts-truetype-fonts-and-third-party-fonts)
- Embird's [Font Engine](https://embird.net/font-engine) and its [parameters](https://www.embird.net/sw/embird/tutorial/feparam/feparam.htm)
- Machine Embroidery Geek on [converting TTF fonts](https://www.machineembroiderygeek.com/?p=19511) and on
  [BX fonts](https://www.machineembroiderygeek.com/what-are-bx-fonts-for-machine-embroidery/)
- A shop's page on [Embrilliance StitchArtist Level 3](https://www.mulqueen.com/shop/Embroidery-Supplies/Software/Embrilliance-Software/p/Embrilliance-StitchArtist-Level-3.htm)
- EduTech Wiki on [Stitch Era's lettering](https://edutechwiki.unige.ch/en/Stitch_Era_-_lettering)
- Impressions on [alphabet and font options](https://impressionsmagazine.com/process-technique/understanding-alphabet-and-font-options/16203/)
- The patents [US 6,934,599](https://patents.google.com/patent/US6934599) and [US 5,559,711](https://patents.google.com/patent/US5559711)
- D. Berio and others, [StrokeStyles](https://doc.gold.ac.uk/autograff/post/papers/strokestyles_2022/), ACM
  Transactions on Graphics (2022)
- E. Balashova and others, [Learning a stroke-based representation for fonts](https://cris.tau.ac.il/en/publications/learning-a-stroke-based-representation-for-fonts/),
  Computer Graphics Forum 38 (2019)
- The thesis [Automatic embroidery digitizing based on skeletonization and backpropagation neural network](https://openaccess.library.uitm.edu.my/Record/ndltd-TW-087NTNU0395026),
  National Taiwan Normal University
- The crates' pages on crates.io on 2026-11-02, and the source of skrifa 0.47, read-fonts 0.44 and harfrust 0.13
- The [OFL](https://openfontlicense.org/open-font-license-official-text/), its pages on
  [Reserved Font Names](https://openfontlicense.org/ofl-reserved-font-names/) and on
  [webfonts](https://openfontlicense.org/webfonts-and-reserved-font-names/), and an
  [OFL FAQ](https://ctan.math.washington.edu/tex-archive/fonts/cm-unicode/doc/OFL-FAQ.txt)
- The [UFL 1.0](https://spdx.org/licenses/Ubuntu-font-1.0.html)
- [Rare Bird's desktop licence](https://fontspring.com/lic/rare-bird-font-foundry/desktop) and
  [KG Fonts' licences](https://kimberlygeswein.com//kg-font-licensing-information/)
- Google Fonts' [repository guide](https://googlefonts.github.io/gf-guide/googlefonts.html)
- Stroke widths and dots measured with fontTools on the OFL fonts that VectorCraft bundles, and satin widths
  measured on Ink/Stitch's font files
