# Machine testing protocol

<!-- implements: crates/stitchcraft-engine/src/testsheets/**, apps/stitchcraft-cli/src/commands/testsheet.rs -->

Automated tests prove the files are what we intend. Only a machine proves they sew well. This page is
the protocol for **machine checkpoints** (MC-1 … MC-7) on the reference machine: a Brother home
embroidery machine with a 200 × 200 mm (8 × 8 in) hoop, designs normally kept to about 150 mm (6 in).

## Roles

- **StitchCraft generates** each test sheet deterministically: `stitch testsheet TS-xx --profile
  brother-200x200 -o TS-xx.pes` prints the file's SHA-256, its size and counts, the threads the machine
  will ask for (with the Brother palette name it will show) and what to check after sewing. From M2.7 it
  also writes a picture of the expected result; until then, previews are drawn by the conformance tooling.
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
| Hoop | The 200 × 200 mm hoop, fabric drum-tight, design centred |
| Machine settings | Note the machine's speed and its own trim/jump settings (some Brother models trim automatically on long jumps) |

## Photographing and measuring

- **Front and back**, flat, in daylight or a daylight lamp, camera straight above.
- **A ruler in every photo**, along the top and left edges, millimetre side visible.
- **Close-ups** of anything odd (loops, gaps, puckers, bird's nests), with the ruler.
- Measure with a steel ruler or calipers; report in mm with one decimal.
- Rate each item 1–5 (5 = indistinguishable from expected) and add a sentence.

## Test sheets

| Sheet | What it checks | Measure / observe | First used |
|---|---|---|---|
| **TS-01** Orientation & scale | An asymmetric "F", a 100 mm cross with 10 mm ticks, 10 mm squares in the corners (120 × 120 mm) | F not mirrored or rotated; 100 mm line = 100.0 ± 0.5 mm in X and Y; squares square | MC-1 |
| **TS-02** Commands | Three colour blocks; a stop; rows of two dashes separated by jumps of 2, 5, 15, 40 mm; trims encoded as trim-flagged jumps on the left half (red) and as plain jumps only on the right (blue) (140 × 70 mm) | The machine stops for colours and the stop; which jumps were trimmed on each half; any loose loops | MC-1, MC-2 |
| **TS-02B** Commands, from elements | TS-02 drawn as a design and planned by the engine. Each red dash but the last sets a trim after it (`trim_after`). The blue ones set none, and the 2 mm gap between them is sewn across, because it is within the 3 mm collapse length. The green line stops halfway (`stop_after`). The stitching has a lock wherever it starts or ends (140 × 70 mm) | The same as TS-02, and whether the thread of each dash pulls out where it was cut | MC-2 |
| **TS-03** Running stitch | Lines at 1.5, 2.0, 2.5, 3.0 and 4.0 mm stitch length, bean 1× and 2×, circles at tolerances of 0.1, 0.2 and 0.5 mm, and 20 stitches placed by hand at 0.3, 0.4, 0.5, 0.7 and 1.0 mm (60 × 78 mm) | Even stitches, smooth curves, solid bean lines, and the shortest hand-placed stitch that sews cleanly | MC-2 |
| **TS-04** Lock stitches | Lines about 30 mm long with every lock shape but custom at both ends, trimmed after: the half stitch on first stitches of 1.5, 2.5 and 4 mm, back-and-forth at 0.5, 0.7 and 1.0 mm, drawn shapes at 70, 100 and 150 % (110 × 65 mm) | Pull each tail gently: does the lock stay, or does the thread pull out? Is the lock visible from the front? | MC-2 |
| **TS-05** Satin width ladder | Columns 1–10 mm wide at three spacings (0.3, 0.4, 0.5 mm) | Coverage, fabric showing between stitches, edges straight, long stitches loose | MC-3 |
| **TS-06** Satin underlays | The same 6 mm column with no underlay, centre walk, contour, zigzag, contour+zigzag | Edge sharpness, loft, puckering | MC-3 |
| **TS-07** Registration | A 60 mm tatami circle with a running-stitch outline; at three pull-compensation values | Gap or overlap between fill and outline at 12 points around the circle | MC-4 |
| **TS-08** Fill density & angle | 30 mm squares at spacing 0.25–0.6 mm and angles 0°, 45°, 90° | Coverage, stiffness, puckering, visible furrows | MC-4 |
| **TS-09** Fill underlay & travel | A shape with holes and a separated part; with and without underlay | Travel hidden? Trims between parts? Holes clean? | MC-4 |
| **TS-10A/B/C** Hoop size | Frames of 150 × 150 mm (A), 190 × 150 mm (B) and 150 × 190 mm (C), each with a centre cross and an "F" in the top-left corner | The machine accepts and shows each design; sews to size; landscape and portrait the right way round | MC-1, MC-5 |
| **TS-11** New stitch types | Samples of the milestone's new types | Per type, as listed on its expected-result sheet | MC-6 |
| **TS-12** Real design | A 140 mm design authored in VectorCraft with fill, satin, running stitch and three colours | Overall quality; registration between colours; time vs. estimate | MC-5, MC-7 |

## Checkpoint calendar

| Checkpoint | After | Sheets | Questions it must answer |
|---|---|---|---|
| MC-1 | M1 | TS-01, TS-02, TS-10A/B/C | Does the machine read our PES? Right size and orientation? Which trim encoding works? Are 150 and 190 mm designs accepted from PES v1? |
| MC-2 | M3 | TS-03, TS-04, TS-02B | Minimum stitch length and lock defaults? Does the machine trim where an element asks? |
| MC-3 | M4 | TS-05, TS-06 | Satin spacing, width limits, underlay defaults, pull compensation |
| MC-4 | M5 | TS-07, TS-08, TS-09 | Fill spacing, compensation, underlay and travel defaults |
| MC-5 | M6 | TS-12, TS-10 | Is the VectorCraft workflow complete and correct? |
| MC-6 | M7 | TS-11 | New stitch types acceptable? |
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
