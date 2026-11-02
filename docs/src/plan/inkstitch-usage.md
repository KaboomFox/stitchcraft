# How people use Ink/Stitch

This review lists what Ink/Stitch's users make and ask for, with the roadmap step that covers each in
StitchCraft. The most frequent come first, as far as the sources show. It was made on 2026-10-19 to look
for major uses the roadmap lacked, after automatic digitizing of pictures turned out to be missing from
it. It changed the roadmap in 4 places ([changes](#changes-to-the-roadmap)).

## Sources

- Ink/Stitch's documentation and tutorials, at [inkstitch.org](https://inkstitch.org/docs/workflow/):
  the workflow, the Params dialog, the tools, lettering, thread colours, troubleshooting, and import and
  export.
- Its release notes from v3.0.0 (2023) to [v3.3.0](https://inkstitch.org/en/2026-07-31-v3.3.0) (July 2026),
  and its lettering web page, [Ink/Stitch Create](https://create.inkstitch.org/).
- Questions and answers in public, on Ink/Stitch's GitHub issues and discussions, the Inkscape forum's
  embroidery section and Reddit's r/Machine_Embroidery, and the titles of tutorial videos and how-to
  articles.

No survey of Ink/Stitch's users has published its results. The [survey of December
2024](https://inkstitch.org/en/2024-12-29-Survey) serves a doctoral thesis, and its answers are not
public. So the order below is how often a use came up in these sources. It measures what people write
about, which can differ from what they do most.

## What people make

| Use | What the sources show | StitchCraft |
|---|---|---|
| Shapes drawn by hand, their stitches set one by one | Ink/Stitch's [basic use](https://inkstitch.org/docs/basic-usage/) and [workflow](https://inkstitch.org/docs/workflow/) are to draw, set the stitches, and save a machine file. The [Params](https://inkstitch.org/docs/params/) dialog sets each element's stitches, with a preview | M6's effects in VectorCraft. Their dialogs show the parameters' names alone until M6.10 labels and groups them, with the richer parameters VectorCraft 0.14 adopted |
| Pictures made into embroidery | The workflow's way from a picture is Trace Bitmap, Simplify and Cleanup Document, with the stitches then set by hand. Tutorial videos trace [a cartoon](https://www.youtube.com/watch?v=46kfuQWOPW0) and [text and logos](https://www.youtube.com/watch?v=fx0Fn2tqtQ0). An article calls the result finicky ([Machine Embroidery Geek](https://www.machineembroiderygeek.com/?p=21980)). A traced logo showed yellow behind its black lettering and sewed too many stitches, and Ink/Stitch crashed when its letters were made satin ([Inkscape forum](https://inkscape.org/forums/embroidery/logo-help/)). A logo 3 inches wide came out muddled, with its letters filled where satin was needed. Its owner then paid a digitizer 5 dollars ([Reddit](https://www.reddit.com/r/Machine_Embroidery/comments/1iyrjkx/need_helpadvice/)) | M13 ([design](../design/image-to-embroidery.md)) |
| Lettering in embroidery fonts | [Lettering](https://inkstitch.org/docs/lettering/) has a menu of its own, with lettering along a path and batch lettering for lists of names. The [font library](https://inkstitch.org/fonts/font-library/) lists 93 fonts, each with the heights it sews at, and Ink/Stitch Create sets lettering on the web. The advice for small text is to use Ink/Stitch's fonts ([Reddit](https://www.reddit.com/r/Machine_Embroidery/comments/1iyrjkx/need_helpadvice/)). Text typed in an installed font fails as a fill whose border crosses itself ([Inkscape forum](https://inkscape.org/forums/questions/path855-error-fill-shape-is-not-valid-this-can-happen-if-the-border-crosses-over-itself/)) | M14, before 1.0. Large text in any font, made into paths with Type › Create Outlines in VectorCraft, sews by M13's rules |
| Patches and appliqué | Searches for tutorials find many patches, from [a beginners' guide](https://www.youtube.com/watch?v=-UDSsuB44Ao) to patch shapes with a merrow-like edge. Ink/Stitch has an [appliqué tutorial](https://inkstitch.org/tutorials/applique/) and the E-stitch for appliqué | Fills, satin borders, the E-stitch and stops sew patches today. An appliqué tool with placement and tack-down lines stays after 1.0 |
| Fewer colour changes and a good order | The FAQ's "Too many color changes" ([help](https://inkstitch.org/docs/faq/)), and issues such as [#1337](https://github.com/inkstitch/inkstitch/issues/1337), "Despite two colors I have 6 color changes", and [#378](https://github.com/inkstitch/inkstitch/issues/378), "reorder by color". The [routing tutorial](https://inkstitch.org/tutorials/routing/) leaves the order to the user. Its tools restack elements and route satin columns and running stitches, and Jump to Stroke turns a jump into a running stitch | M13.3 orders automatic elements, and with `--sort-colours` any design. M10 has the routing tools |
| The colours of real threads | Thread palettes installed into Inkscape, and [Apply Palette](https://inkstitch.org/docs/thread-color/) for the nearest thread. A [guide from the City of San Diego](https://www.sandiego.gov/sites/default/files/2025-09/embroidery_inkstitch-guide.pdf) picks Ink/Stitch's Brother palette before anything is drawn | The nearest colour of the PEC palette (M1.6), `stitch palette` for tracing (M13.7), thread catalogues (M11) |
| Bought designs opened, resized or moved to a machine | Requests to open a PES file to enlarge it ([#1716](https://github.com/inkstitch/inkstitch/issues/1716)) or to shrink it ([#2022](https://github.com/inkstitch/inkstitch/issues/2022)). Ink/Stitch opens machine files as manual stitches, and its maintainers warn that scaling changes the density | `stitch inspect`, `stitch preview` and `stitch convert` (M2). Opening a machine file in VectorCraft is added after 1.0 |
| Designs larger than the hoop | Requests to split a design for a small hoop ([#182](https://github.com/inkstitch/inkstitch/issues/182), [discussion 1341](https://github.com/inkstitch/inkstitch/discussions/1341)) | The hoop check (M1.3). Splitting stays after 1.0 |
| Checking a design before sewing | The [simulator](https://inkstitch.org/docs/visualize/), realistic previews and a density map | VectorCraft's previews (M6.3) and `stitch preview` |
| Worksheets for the machine's operator | [Print PDF](https://inkstitch.org/docs/print-pdf/), with the colour blocks, thread names and stitch counts | The report and the picture that `stitch plan` writes. Worksheets stay after 1.0 |
| Other stitch types | Ripple, contour, meander, circular, guided and gradient fills, tartan, cross stitch from pictures (v3.3.0), and tutorials on redwork, cutwork and free-standing lace | M7.6 to M7.9, and M10 |

## What people struggle with

| Trouble | Sources | StitchCraft |
|---|---|---|
| Installing and starting Ink/Stitch, above all on macOS | Issues such as [#4553](https://github.com/inkstitch/inkstitch/issues/4553), "v. 3.3.0 does not run on iMac/Sequoia", and [#600](https://github.com/inkstitch/inkstitch/issues/600), "Ink/Stitch is greyed out in Inkscape". A Params dialog and previews that never open ([Inkscape forum](https://inkscape.org/forums/embroidery/params-and-realistic-preview-wont-work-at-all/)) | The plug-ins are WebAssembly modules that VectorCraft loads, with no other program to install. Signed binaries come with 1.0 (M12) |
| Settings that are hard to choose | A reader found the Params settings hard to judge in advance ([Machine Embroidery Geek](https://www.machineembroiderygeek.com/?p=21646)). Another found Ink/Stitch "not beginner friendly" ([Reddit](https://www.reddit.com/r/Machine_Embroidery/comments/1kmf3tx/beginner_software/)) | M13 chooses the stitches, and VectorCraft previews them |
| Shapes that Ink/Stitch refuses | `Fill: Shape is not valid` for text and traced shapes whose borders cross ([EduTechWiki](https://edutechwiki.unige.ch/en/InkStitch_-_tips_and_trouble_shooting)) | A fill sews the region a viewer paints, crossings included (M5.1) |
| Long waits | An hour of "Rendering stitch-plan" on a complex drawing ([discussion 1834](https://github.com/inkstitch/inkstitch/discussions/1834)) | Budgets bound every loop over input, and `cargo xtask perf` guards the time of the hoop's whole field (NFR-PERF-1) |
| Small text that does not come out crisp | Lettering on a fill, in a logo 2 inches wide, on a PE800 ([Inkscape forum](https://inkscape.org/forums/embroidery/techniques-for-making-text-on-top-of-a-fill-more-crisp/)) | M14's fonts at the heights they sew at, and M13's rules for fills under columns |

## Changes to the roadmap

- **M13, image to embroidery.** It is new, and comes after M7.5 and before M7.6.
- **M10, more stitch types II.** It moves before M14 (2026-11-01): 35 of the 142 fonts in Ink/Stitch's
  library use its stitch types, cross stitch in 25 of them.
- **M14, lettering.** It moves from after 1.0 to after M8, whose SVG adapter reads the files of
  Ink/Stitch's fonts. Its last steps set the text in a picture again as lettering.
- **Colour sorting for any design.** `--sort-colours` gives a design with settings the order of M13.3. It
  was after 1.0.
- **Machine files opened in VectorCraft.** This is planned after 1.0. `stitch import design.pes -o
  design.vectorcraft` writes their stitches as manual stitches, with the same document writer as M8's
  `stitch import-inkstitch`.
