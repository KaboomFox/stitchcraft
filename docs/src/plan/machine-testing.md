# Machine testing protocol

<!-- implements: crates/stitchcraft-engine/src/testsheets/**, apps/stitchcraft-cli/src/commands/testsheet.rs -->

Automated tests prove the files are what we intend. Only a machine proves they sew well. This page is
the protocol for **machine checkpoints** (MC-1 … MC-9) on the reference machine: a Brother PE800, whose
5 × 7 in hoop sews at most 130 × 180 mm. Its 4 × 4 in and small hoops have profiles too, and the TS-10
sheets test them ([ADR 0013](../design/adr/0013-brother-pe800-reference-machine.md)).

## Roles

- **StitchCraft generates** each test sheet deterministically: `stitch testsheet TS-xx -o TS-xx.pes`
  checks it against the profile of the hoop it is for. It prints that profile, the file's SHA-256, its
  size and counts, the threads in the order they are sewn (with the Brother palette name the machine
  shows for each) and what to check after sewing. Its size and counts are the file's, read back as the
  machine reads it. `stitch preview TS-xx.pes -o TS-xx.png` draws a picture of what the file sews.
  `stitch testsheet --list` lists the sheets. The MC-1 sheets are drawn stitch by stitch, to test the
  machine and the file formats; from MC-2, sheets are drawn as designs and planned by the engine, so
  sewing them tests its stitches, locks and plan assembly too.
- **The tester sews** it with the standard setup below, photographs it, measures it, and files a
  **Sew-out report** issue (the form asks for everything on this page).
- **A maintainer turns the report into changes**: profile values, parameter defaults, new conformance
  cases, or bugs. Every profile change links its report.

## Standard setup (keep it constant)

Record anything that differs in the report.

| Item | Standard |
|---|---|
| Fabric | Medium-weight woven cotton (quilting cotton or twill), pressed |
| Stabilizer | One layer of medium tear-away; cut-away for knits (TS-09 only) |
| Needle | 75/11 embroidery needle, fresh at each checkpoint |
| Top thread | 40 wt polyester embroidery thread; colours as listed on the expected-result sheet (any brand) |
| Bobbin | 60 wt (90 wt) bobbin thread, white |
| Hoop | The 5 × 7 in hoop, unless the sheet names another; fabric drum-tight, design centred |
| Machine settings | Note the machine's speed and its own trim/jump settings (some Brother models trim automatically on long jumps) |
| Starting | Hold the thread's tail for the first few stitches: at MC-1 a tail left loose pulled the first stitches out |
| Jump threads | The PE800 cuts the thread at thread changes and stops, but at no jump (MC-1). At each thread change and stop, snip the jump threads sewn so far, and snip the rest before any pull test. The sheets never sew over a jump thread that is still there |

## Photographing and measuring

- **Front and back**, flat, in daylight or a daylight lamp, camera straight above.
- **A ruler in every photo**, along the top and left edges, millimetre side visible.
- **Close-ups** of anything odd (loops, gaps, puckers, bird's nests), with the ruler.
- Measure with a steel ruler or calipers; report in mm with one decimal.
- Rate each item 1–5 (5 = indistinguishable from expected) and add a sentence.

## Test sheets

`stitch testsheet` writes each sheet. From M6.12, VectorCraft's Test Sheet filter draws the sheets drawn as
designs, TS-02B to TS-09, into a document, where they sew as `stitch testsheet` writes them, moved to the
middle of the artboard (`REQ-VC-015`). A checkpoint can so sew a sheet through VectorCraft's workflow.

| Sheet | What it checks | Measure / observe | First used |
|---|---|---|---|
| **TS-01** Orientation & scale | An asymmetric "F", a 100 mm cross with 10 mm ticks, 10 mm squares in the corners (120 × 120 mm) | F not mirrored or rotated; 100 mm line = 100.0 ± 0.5 mm in X and Y; squares square | MC-1 |
| **TS-02** Commands | 3 colour blocks, a stop, and rows of 2 dashes separated by jumps of 2, 5, 15 and 30 mm. The left half (red) has a trim before every jump, the right half (blue) none; PES flags every jump as a trim (REQ-FMT-010), so in a PES file the halves differ only in the plan (120 × 70 mm) | The machine stops for colours and the stop; every dash sews; which jumps were cut on each half; any loose loops | MC-1, MC-2 |
| **TS-02B** Commands, from elements | TS-02 drawn as a design and planned by the engine. Each red dash but the last sets a trim after it (`trim_after`). The blue ones set none, and the 2 mm gap between them is sewn across, because it is within the 3 mm collapse length. The green line stops halfway (`stop_after`). The stitching has a lock wherever it starts or ends (120 × 70 mm) | The machine stops for colours and the stop; does any dash pull out while the machine goes on, uncut, to the next? Once the jump threads are snipped, does each dash hold? | MC-2 |
| **TS-03** Running stitch | Lines at 1.5, 2.0, 2.5, 3.0 and 4.0 mm stitch length, bean 1× and 2×, circles at tolerances of 0.1, 0.2 and 0.5 mm, and 20 stitches placed by hand at 0.3, 0.4, 0.5, 0.7 and 1.0 mm (60 × 78 mm). The long lines alternate between left to right and right to left. The circles are sewn right to left, and each hand-placed line starts below where the last one ended | Even stitches, smooth curves, solid bean lines, and the shortest hand-placed stitch that sews cleanly: 0.3 mm or less if the 0.3 mm line is clean, since the sheet has no shorter line | MC-2 |
| **TS-04** Lock stitches | Lines 30 mm long with every lock shape but custom at both ends, trimmed after: the half stitch on first stitches of 1, 1.5 and 2.5 mm, back-and-forth at 0.5, 0.7 and 1.0 mm, drawn shapes at 70, 100 and 150 % (110 × 65 mm). The rows alternate between left to right and right to left | Snip the jump threads first. Pull each tail gently: does the lock stay, or does the thread pull out? Is the lock visible from the front? | MC-2 |
| **TS-05** Satin width ladder | 3 rows at zigzag spacings of 0.3, 0.4 and 0.5 mm, each of 10 columns 20 mm tall and 1 to 10 mm wide, with no underlay (91 × 76 mm). Each column is sewn as drawn and trimmed after; the rows alternate between left to right and right to left, and the columns between down and up, each from its corner nearest the one before | Coverage, fabric showing between stitches, edges straight, long stitches loose, how much narrower than drawn | MC-3 |
| **TS-06** Satin underlays | The same column, 6 mm wide and 30 mm tall, with no underlay, a centre walk, a contour, a zigzag, and a contour with a zigzag (62 × 30 mm). Each column is sewn as drawn and trimmed after, alternately down and up from its corner nearest the one before, the centre walk first | Edge sharpness, loft, puckering, underlay showing | MC-3 |
| **TS-07** Registration | 3 circles 60 mm across, filled in red with the default underlay at pull compensations of 0, 0.2 and 0.4 mm, then outlined in black with a running stitch along their edges (126.2 × 126 mm). Each fill and outline is trimmed after, and each outline starts where the jump to it arrives | Gap or overlap between fill and outline at 12 points around each circle, how much narrower than drawn each fill sews, puckering | MC-4 |
| **TS-08** Fill density & angle | 15 squares 30 mm wide, with the default underlay: rows at spacings of 0.25, 0.3, 0.4, 0.5 and 0.6 mm, top to bottom, and columns at angles of 0°, 45° and 90° (100 × 170 mm). Each square is trimmed after | Coverage, stiffness, puckering, visible furrows. At 0.25 mm, below the Brother's shortest stitch of 0.3 mm, a sliver at each row's start in the 0° and 90° squares, where the rows meet the edges square on (`DEV-FIN-001`). The 45° square's rows end 0.35 mm apart along its edges and keep their starts. If the machine sews 0.25 mm stitches cleanly, the report can lower the profile's shortest stitch | MC-4 |
| **TS-09** Fill underlay & travel | A rectangle 50 × 30 mm with a square hole and a round hole, and a bar 6 mm to its right, all one fill, sewn 3 times (66 × 114 mm). The top copy has no underlay. The others have the default underlay, and the bottom one 2 gap-filling rows too. Each copy is trimmed after | Travel hidden? Does the fill hold at both ends of the jump to the bar? Holes clean? A gap where the rows round a hole meet? | MC-4 |
| **TS-10A/B/C** Hoop size | Frames of 100 × 100 mm (A, the 4 × 4 in hoop), 130 × 180 mm (B, the 5 × 7 in) and 20 × 60 mm (C, the small hoop), each as large as its hoop's field, with a centre cross and an "F" in the top-left corner | The machine takes each design in its hoop without asking for a larger one, and sews it to size. The small hoop's field is the right way round | MC-1, MC-5 |
| **TS-11** New stitch types | Samples of the milestone's new types. So far zigzag strokes (M7.2), straight and curved, at spacings of 0.4, 1 and 2 mm and widths of 1, 2 and 4 mm, and one at 45°. E-stitches (M7.3) 3 and 5 mm wide at spacings of 2 and 3 mm, along a straight edge and round a circle, on appliqué fabric. S-stitches (M7.4) 6 and 10 mm wide at spacings of 0.4 and 0.6 mm, straight and curved, the 10 mm ones split at 3 mm by the default and the staggered methods. Zigzag satins (M7.5) 4 and 8 mm wide at spacings of 0.4 and 1 mm, straight and curved, beside satin columns of the same widths and spacings, and one 8 mm wide at 1 mm over a contour underlay | Per type, as listed on its expected-result sheet. A zigzag sews narrower than drawn, and the report says by how much, whether its sides are even round curves and which is the narrowest width that sews cleanly. The E-stitch's spine should cover the appliqué's raw edge, and its spikes hold it without puckering. The S-stitch should cover its band at 0.4 mm with its steps along the edges even, and the report says whether the default method's splits leave a furrow along the band that the staggered ones do not. The zigzag satin should cover its band at 0.4 mm as its satin column does, and the report says whether it sews narrower than the satin column beside it. Its contour underlay should not show past the zigzag at either end | MC-6 |
| **TS-12** Real design | A design up to 130 × 180 mm authored in VectorCraft with fill, satin, running stitch and 3 colours, its PES file written by VectorCraft's File › Export › Export As (M6.9) | Overall quality, registration between colours, and time against the estimate. Satin columns start and end at their nearest points: is the way the needle takes under them hidden? Satin columns drawn as one path: are they as wide as their strokes, and do their corners sew clean? A graphic style applied to a group: do the paths inside sew as VectorCraft shows them, with the style's settings and colour? A satin border of 2 circles joined by Make Compound Path: do its stitches go straight across all the way round? A satin column that Stroke to Satin made of a line (M6.12): does it sew as the line did with the satin effect? | MC-5, MC-7 |
| **TS-13** Pictures digitized automatically (M13) | Test pictures drawn in code and digitized with `--auto`, each beside the same shapes sewn in paint order. A badge is a broad fill inside a ring. Block letters are 4, 6, 10 and 15 mm tall. A cartoon face has black outlines 1, 2 and 4 mm wide. Lines are 0.3, 0.6 and 0.9 mm wide, and specks lie round a disc | Fabric showing between colours, on both halves, and on the digitized half whether each area's 0.4 mm overlap under the next closes the gap or shows as a ridge (M13.2). Whether a stacked trace's digitized half, its covered parts cut away, sews as flat as the abutting one, and what became of the specks round the disc (M13.2). Where a shape runs on beside an area without touching it, whether the fabric that shows there as drawn reads as a gap (M13.2). The smallest letters that read. Whether each outline's columns cover the edges of the fills beside them, and whether the triple runs show as wide as their lines. A column's square end past the end of a line with round caps (M13.1), and how far it shows past the round end. Small traced rings and hairpins, filled because no satin column can follow them (M13.1), and how solid they sew beside their columns' neighbours. Colour changes, trims and time against the half in paint order | MC-8 |
| **TS-14** Lettering (M14) | 3 embroidery fonts, a block, a script and a small one, each at the least and the greatest heights it sews at, and a name along an arc | Whether every glyph reads at its least height, satin coverage at its greatest, and whether the locks of small glyphs hold after the trims | MC-9 |

## Checkpoint calendar

| Checkpoint | After | Sheets | Questions it must answer |
|---|---|---|---|
| MC-1 | M1 | TS-01, TS-02, TS-10A/B/C | Does the machine read our PES? Right size and orientation? Which trim encoding works? Does each hoop take a design as large as its profile allows, from PES v1? |
| MC-2 | M3 | TS-03, TS-04, TS-02B | Minimum stitch length and lock defaults? Do the locks hold where the machine cuts the thread, and where a jump thread is snipped? |
| MC-3 | M4 | TS-05, TS-06 | Satin spacing, width limits, underlay defaults, pull compensation |
| MC-4 | M5 | TS-07, TS-08, TS-09 | Fill spacing, pull compensation, gap-filling rows, underlay and travel defaults |
| MC-5 | M6 | TS-12, TS-10 | Is the VectorCraft workflow complete and correct? Does a test sheet drawn by the Test Sheet filter sew as `stitch testsheet` writes it? |
| MC-8 | M13.7 | TS-13 | Do pictures digitized automatically sew better than the same shapes in paint order? Are the widest satin column, the overlap and the underlays by width right? |
| MC-6 | M7 | TS-11 | New stitch types acceptable? |
| MC-9 | M14 | TS-14 | Do the fonts sew legibly across the heights they claim? |
| MC-7 | M12 | all | Release regression |

## Report template

The GitHub issue form *Sew-out report* asks for these fields; this is the same template for notes kept
elsewhere:

```markdown
### Sew-out report — TS-__ — MC-__

- StitchCraft version / commit:
- File SHA-256 (from `stitch testsheet`):
- Machine model and firmware:            Hoop:
- Fabric / stabilizer / needle / thread / bobbin: (standard? if not, what)
- Machine speed and trim/jump settings:

| Check | Expected | Measured / observed | Rating 1–5 |
|---|---|---|---|
|  |  |  |  |

Problems seen (loops, gaps, puckers, thread breaks, bird's nests, skipped trims):

Photos attached: front ☐ back ☐ close-ups ☐ ruler visible ☐
```

## From report to change

1. A maintainer reads the report and labels it `sewout:pass`, `sewout:tune` or `sewout:bug`.
2. **Tune:** the profile or default changes in a PR that links the report; a conformance case pins the
   new value; the docs' parameter pages show the evidence link.
3. **Bug:** a failing conformance case reproducing it comes first, then the fix.
4. **Pass:** the checkpoint row in `ROADMAP.md` is ticked with the report link.
