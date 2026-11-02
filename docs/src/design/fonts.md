# Fonts

Lettering sews text in a font. M14 sets text in embroidery fonts, digitized glyph by glyph in Ink/Stitch's
font format. M15 ports an ordinary TrueType or OpenType font automatically. It reads each glyph's outline at
the height the text is set and works out how the glyph sews ([roadmap](../plan/roadmap.md)). This page
records the research both milestones rest on, counted and read on 2026-11-02. The sources are listed at the
end.

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
less the kerning of its pair, and each word by the space's advance. Letter spacing, word spacing and line
height add millimetres of their own. Lines align left, centre or right, or are justified as a block. A trim
can follow each letter, word or line. Lines sew back and forth in a font with a variant for the other
direction, and a sortable font's colours sew in order. Batch lettering clamps the scale to the font's range.

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
`1505384`). Embroidery fonts in Ink/Stitch's format come from a repository of their own (M14.1). A ported
font is made on the user's computer from a font the user has.

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
- **Type in VectorCraft's plug-ins.** VectorCraft doesn't give plug-ins its type objects (its
  `docs/plugins.md` at `1505384`), and the exporter plug-in can't read font files. Proposing that VectorCraft
  give plug-ins type as glyph outlines is an addition to the [RFC](rfc-vectorcraft-abi-v2.md), for M9 or
  earlier.

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
