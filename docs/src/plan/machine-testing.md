# Machine testing protocol

<!-- implements: crates/stitchcraft-engine/src/testsheets/**, apps/stitchcraft-cli/src/commands/testsheet.rs -->

Automated tests prove the files are what we intend. Only a machine proves they sew well. This page is
the protocol for **machine checkpoints** (MC-1 … MC-12) on the reference machine: a Brother PE800, whose
5 × 7 in hoop sews at most 130 × 180 mm. Its 4 × 4 in and small hoops have profiles too, and the TS-10
sheets test them ([ADR 0013](../design/adr/0013-brother-pe800-reference-machine.md)).

## Roles

- **StitchCraft generates** each test sheet deterministically: `stitch testsheet TS-xx -o TS-xx.pes`
  checks it against the profile of the hoop it is for. It prints that profile, the file's SHA-256, its
  size and counts, the threads in the order they are sewn (with the Brother palette name the machine
  shows for each) and what to check after sewing. Its size and counts are the file's, read back as the
  machine reads it. `stitch preview TS-xx.pes -o TS-xx.png` draws a picture of what the file sews on the
  reference machine, with the jump threads it leaves.
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
| Jump threads | The PE800 cuts the thread at thread changes and stops, but at no jump (MC-1). At each thread change and stop, snip the jump threads sewn so far, and snip the rest before any pull test. The sheets never sew over a jump thread that is still there, but TS-13, which sews its pictures as any design: which jump threads they sew down is one of its checks |

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
| **TS-11A** New strokes and satins | M7's new strokes and satins, a band to a thread, each sample trimmed after (127 × 144 mm). Most are hooks, straight and then turning a quarter circle, in rows that turn back on each other. Zigzag strokes (M7.2) at spacings of 0.4, 1 and 2 mm and widths of 1, 2 and 4 mm, and a line 4 mm wide whose stitches turn 45°. E-stitches (M7.3) 3 and 5 mm wide at spacings of 2 and 3 mm, along the sides of a square and round 4 circles, on appliqué fabric laid at one stop and trimmed at the next. S-stitches (M7.4) 6 and 10 mm wide at spacings of 0.4 and 0.6 mm, the 10 mm ones split at 3 mm by the default and the staggered methods. Zigzag satins (M7.5) 4 and 8 mm wide at spacings of 0.4 and 1 mm, each beside a satin column of the same width and spacing, and one 8 mm wide at 1 mm over a contour underlay | Per type, as listed on its expected-result sheet. A zigzag sews narrower than drawn, and the report says by how much, whether its sides are even round curves and which is the narrowest width that sews cleanly. The E-stitch's spine should cover the appliqué's raw edge, and its spikes hold it without puckering. The S-stitch should cover its band at 0.4 mm with its steps along the edges even, and the report says whether the default method's splits leave a furrow along the band that the staggered ones do not. The zigzag satin should cover its band at 0.4 mm as its satin column does, and the report says whether it sews narrower than the satin column beside it. Its contour underlay should not show past the zigzag at either end | MC-6 |
| **TS-11B** New fills | Contour fills (M7.7) and circular fills (M7.9), each over the default underlay and trimmed after, a row to a thread (125 × 166 mm). Top row: discs 36 mm across with rings 0.25 mm apart, sewn inner to outer round a hole 12 mm across set off its middle, as a spiral and as a double spiral. Middle row: stars 40 mm across with rings 0.4 mm apart and round, mitred and bevelled corners. Third row: 2 squares joined by a neck 3 mm wide, where the rings part. Beside them, a rectangle whose rings alternate their direction and a leaf smoothed by 0.5 mm. Bottom row, circular fills: the disc round its hole and the star again, and a rectangle whose circles fade from 0.25 mm apart at its middle to 1 mm at its corners | Fabric showing between the rings, puckering, and whether the discs sew 36 mm across. Whether inner to outer's steps show as a line across the rings, and whether the alternating rectangle looks calmer. A knot or a bump where the spirals end. Whether the mitred star's rings reach into its points where the round and bevelled ones leave a gap at each tip, and which looks best. The neck should fill, and the smoothed leaf's line should run without kinks. Whether the circular fills' rings look round and even, whether the travel between pieces of a spiral shows on the star's points or round the hole, and whether the fading rectangle is denser at its middle. The time the sheet takes | MC-6 |
| **TS-11C** Meander fills | Meander fills (M7.8), each over the default underlay and trimmed after, a row to a thread (123 × 120 mm). The top row's discs, 36 mm across, are in hexagons, squares and triangles 2 mm a side. The middle row's discs are in octagons and squares, in hexagons and triangles, and in hexagons with squares and triangles. In the bottom row a star 40 mm across is in squares and triangles. Beside it a rectangle in hexagons twice as large sews as a zigzag 2.5 mm wide kept inside it, and a leaf in hexagons half as large is turned by 30° and smoothed by 0.5 mm | Whether each tiling shows in the stitches and the line looks even, or bare patches show where it didn't reach, and which tile looks best at which size. Does the underlay show between the line's turns, or is the fill too light to hide the fabric? The zigzag should stay inside the rectangle and lie flat. On the smoothed leaf, a line without kinks and 1 mm sides sewn cleanly. Puckering where the line turns | MC-6 |
| **TS-12** Real design | A design up to 130 × 180 mm authored in VectorCraft with fill, satin, running stitch and 3 colours, its PES file written by VectorCraft's File › Export › Export As (M6.9) | Overall quality, registration between colours, and time against the estimate. Satin columns start and end at their nearest points: is the way the needle takes under them hidden? Satin columns drawn as one path: are they as wide as their strokes, and do their corners sew clean? A graphic style applied to a group: do the paths inside sew as VectorCraft shows them, with the style's settings and colour? A satin border of 2 circles joined by Make Compound Path: do its stitches go straight across all the way round? A satin column that Stroke to Satin made of a line (M6.12): does it sew as the line did with the satin effect? A fill and a line cut by Make Clipping Mask (M8.7): do they sew only inside the clipping shape, cut where VectorCraft shows them cut? | MC-5, MC-7 |
| **TS-13** Pictures digitized automatically (M13) | Test pictures drawn in code (`stitchcraft_engine::testsheets::pictures`), digitized automatically in the top half of each sheet and sewn as drawn, in paint order, in the bottom half, which sews first and stops. TS-13A has a badge traced abutting and one traced stacked, specks round a disc, a row of badges drawn badge by badge, and a bar that runs on beside a square without touching it (126 × 168 mm). TS-13B has the word MATCH in block letters 15, 10, 6 and 4 mm tall, and windows whose lines, traced as areas, are 0.3, 0.6 and 0.9 mm wide (110 × 148 mm). TS-13C has a cartoon face drawn with black lines 1, 2 and 4 mm wide, a star, and a balloon on its string (111 × 108 mm). TS-13D has an emblem 42 mm wide, a navy square with a white disc, a red star on the disc and 3 gold bars, traced abutting and traced stacked (89 × 100 mm). TS-13E has a button 23 mm across, a yellow disc with 7 orange dots in a black rim, traced abutting and traced stacked (24 × 108 mm) | Fabric showing between colours, on both halves, and on the digitized half whether each area's 1 mm overlap under the next covers the gap or shows as a ridge (M13.2, M13.16), and whether the base layer under the digitized half shows beside a shape's edge or through a fill (M13.16). Whether a stacked trace's digitized half, its covered parts cut away, sews as flat as the abutting one, and what became of the specks round the disc (M13.2). Where a shape runs on beside an area without touching it, whether the fabric that shows there as drawn reads as a gap (M13.2). The smallest letters that read. Whether each outline's columns cover the edges of the fills beside them, and whether the triple runs show as wide as their lines. A column's square end past the end of a line with round caps (M13.1), and how far it shows past the round end. Small traced rings and hairpins, filled because no satin column can follow them (M13.1), and how solid they sew beside their columns' neighbours. Colour changes, jumps, trims and time against the half in paint order, on a row of badges drawn badge by badge too, as Auto Grouping draws them (M13.3). Whether any shape covers one drawn over it, which the order must never let happen, and whether each line sewn after the fill it borders covers the fill's edge (M13.3). Lines traced as areas, 0.3, 0.6 and 0.9 mm wide, sewn along their middles (M13.4): whether each reaches as far as the line drawn, whether the 0.6 and 0.9 mm lines' triple runs show as wide as they are drawn and the 0.3 mm line's single run doesn't look thin, and whether the fills beside a line, reaching under it, leave fabric showing along it. Whether a junction's third branch, which ends at the line it meets, leaves a gap or a knot there (M13.4). Letters and outlines as wide as a satin column, sewn as satin columns between their own edges (M13.5): whether each column's stitches lie square to its stroke, round a ring and through a bend. Where a column ends under another, 0.4 mm longer, does fabric show, or a ridge? A slanted terminal should end flat along its edge as drawn, and a round one at its farthest point. Whether a crossbar sewn as a column of its own across a stem reads as one stroke. The underlays by width should hold the columns up, and 0.4 mm of pull compensation keep them as wide as drawn. The stitches that lean where strokes meet at sharp angles, as in an M or a W (`SC-W0208`), and the jumps between a letter's columns. Shapes broad in places and narrow in others (M13.6). The star's points and the balloon's string sew as columns over the fill of the star's middle and of the balloon. Do their flat ends cover the fill's edge, or leave fabric or a ridge there? Do each point's stitches lie square to it from its tip to its cut? Pictures traced in layers (M13.12). Whether the stacked emblem, its layers flattened, sews as the abutting one beside it, and how stiff it sews as drawn on the bottom half, each colour whole under the colours over it. Which sews better between the bars, the navy filled whole under the white disc and the bars or the navy sewn round them as drawn? Whether the gold bars keep their height, 2.1 mm as drawn, and their square ends. Which sews better between the dots, the button's yellow filled whole under them or sewn round them as drawn, and does the rim's column cover the yellow's edge? Whether fabric or a ridge shows round the bars, the disc, the star and the dots. Small discs no wider than a satin column are dots, filled (M13.12), as the gold discs of the row of badges and the button's dots are. Do they sew round, and does fabric show at their edges? Which jump threads a shape sews down, since the PE800 cuts none | MC-8 |
| **TS-13F** How thick a design may sew (M13.30) | A ladder of squares 15 mm a side, drawn as plain fills and not digitized (112 × 57 mm with its outlines). Top row: 1, 2, 3, 4, 5 and 6 fills on each other, left to right, each fill with the default rows of 0.25 mm, its angle turned 30° from the one under it. Middle row: the same squares under black satin outlines 2 mm wide, which sew last. Bottom row: 3 blocks 12 mm a side, each one fill cut into patches 1.5, 2 and 3 mm across that reach 1 mm under the patch sewn after them in their row and the one below. Every square and block lies on one open layer of rows, as the base layer of a digitized design sews by default, and no fill has an underlay of its own | The first square that breaks or shreds thread, or where the needle thumps, bare and under the outline. Puckers round each square, how stiff each is, and its thickness by calipers. Which block of patches is the stiffest, and whether the base layer shows between patches | MC-8 |
| **TS-14** Lettering (M14) | 3 embroidery fonts, a block, a script and a small one, each at the least and the greatest heights it sews at, and a name along an arc | Whether every glyph reads at its least height, satin coverage at its greatest, and whether the locks of small glyphs hold after the trims | MC-9 |
| **TS-15** Fonts ported automatically (M15) | 3 fonts from Google Fonts, a sans, a serif and a script, each ported at 3 heights beside the font Ink/Stitch digitized by hand from it, at the same heights. A word in satin columns 0.4, 0.6, 0.8 and 1 mm wide beside the same word in triple runs. `stitch testsheet` draws it with the fonts of the corpus | Whether every ported glyph reads at the least height, with its dots, serifs and counters. The jump threads inside letters and between them. Whether the ported letters read as well as the letters digitized by hand beside them, and the narrowest satin column that sews a letter cleanly | MC-10 |
| **TS-16** An Ink/Stitch file (M8) | A design written as Ink/Stitch writes a file, which `stitch plan` reads, each step of M8 drawing its part. Its fill and its satin column start and end at Ink/Stitch's commands. From M8.4 a red fill 40 × 30 mm sews from its top-right corner to its bottom-left one, and a blue satin column 40 mm long starts in its middle and ends three quarters along. From M8.5 a green fill is cut by its clip path, a circle, to a quarter of a disc, and a black zigzag running stitch by a U-shaped one into 2 pieces. From M8.8 the origin lies off the design's middle, on the blue column's top edge 10 mm in from its left end, and a stop after the blue column sends the frame to a stop position 20 mm right of the fill and 10 mm above it. From M8.9 the file turns the design right on export, about its origin, so its first 4 parts sew 100 mm wide and 40 mm tall. From M8.10 a clone of the red fill, turned 45° right, sews after them, in red, below the fill once the design is turned: its rows turn with it, and it starts and ends at the corners turned from the fill's. From M8.13 2 black lines 40 mm long sew last, above the rest once the design is turned, each locked at both ends with a custom lock drawn as a path and trimmed after: straight lines and a curve at 100 %, and lines and an arc at 150 %. It is sewn beside the PES file the differential's Ink/Stitch writes of it | Where each element starts and ends, against its commands. Whether the clipped edges sew where the clip path cuts them, and the clone's rows lie at its turned angle. Do the drawn locks hold when the tail is pulled? Whether the machine puts the origin or the design's middle at the hoop's centre, where the frame waits at the stop, and whether the design sews turned right about its origin. Every difference between the 2 sew-outs | MC-11 |
| **TS-17A** Cross stitch (M10) | Cross stitch fills (M10.1, M10.2), drawn before MC-12, each trimmed after, a row to a thread, every sample but the turned square on the canvas's grid. From M10.1, in red, squares 24 mm wide of simple crosses on grids of 2 and 3 mm, with 2 and 4 threads. Below them in blue, the 2 mm square with 2 threads and the 3 mm one with 4, flipped, and a K 24 mm tall drawn as Ink/Stitch's cross-stitch fonts draw their letters, on a 2 mm grid with 2 threads, grown by 0.1 mm, its slanted edges cutting their cells in half. From M10.2, in green, squares of upright, dense upright, double and Smyrna crosses on the 3 mm grid, the double crosses with 2 threads as Jersey 15 sews them. Below them in black, squares of half crosses with 3 threads, plain and flipped, and a square 18 mm wide turned 30° with its grid, as lettering along a path turns a glyph's (114 × 114 mm) | Whether every cross's top diagonal lies the same way, and whether the stitches through the cells' middles show. Fabric between the crosses at 2 and 4 threads, and the grid that reads best at the letter's size. How the layers of the double and Smyrna crosses look, and whether the half crosses' travel stitches show on their edges. Gaps between the dense upright crosses, and the turned square's stepped edges. The time the sheet takes | MC-12 |
| **TS-17B** New fills (M10) | Tartan, guided and linear gradient fills (M10.6, M10.7, M10.9, M10.10, M10.12), drawn before MC-12, each trimmed after. From M10.6, 5 tartans 40 mm square, 3 to a row, each underlay in a thread of its own. Their setts are mirrored at the ends or repeated, and one has a weft that differs from its warp. Another has stripes 0.5 and 0.8 mm wide sewn as runs, and one is turned 30° and scaled by 150 %. From M10.7, a herringbone 3 mm wide of blue, white, red and black ends the second row. From M10.12, 3 squares 40 mm a side in a third row, in linear gradients of 2 and 3 stops at slants of their own, the last one's middle stop transparent, each underlay in its first colour. From M10.9, the fourth row starts with a leaf 48 mm from tip to tip, from the bottom left to the top right of a 40 mm square, in emerald green over its underlay. Its guided fill copies an S-shaped guide from tip to tip, 0.25 mm apart. From M10.10, beside it, a fan in yellow whose rows are the offset curves of an arc, and an egg in flesh pink whose rows are rings round a short line, started along an anchor line. From M10.9 only `stitch testsheet` draws the sheet: the leaf's guide is a marker the Test Sheet filter would have to group with it (130 × 172 mm) | Whether a tartan's warp and weft read as a weave and its runs lie flat on its fills, and how its herringbone turns. The spacing of the guided rows round the guide's bends, and where the copied rows crowd. The spacing of the fan's offset rows from its inner arc to its outer, and where the egg's rings show their joins or their starts along the anchor line. Whether a gradient's colours blend with no step between its sections, and whether the transparent stop leaves a clean gap. Colour changes, jumps and time | MC-12 |
| **TS-17C** Ripples and routing (M10) | Ripple stitches (M10.8, M10.11), drawn before MC-12, along a column of rails and rungs, from an open line towards a target point and from a closed line spiralling in. Ripples along a guide line and closed lines on a satin guide, beside a ripple with a grid. The routing tools (M10.4, M10.13): satin letters as drawn beside the same letters after auto-satin routing, and a network of running stitches as drawn beside it after auto-run routing. Drawn from M10.4 with its part, the word ROUTE in satin letters 20 mm tall with a centre walk underlay, as drawn above and routed 30 mm below. From M10.13, below them in red, 6 bean-stitched running stitches that cross, branch and pass near each other, as drawn on the left and routed with trims on the right. From M10.15, below the network in red, a bar and a post that cross at their middles, satin columns 5 mm wide and 30 mm long with a contour and a zigzag underlay, as drawn on the left and with their underlays first on the right. From M10.8, in green below the crossing columns, a ripple of 7 lines sewn back and forth along a column between 2 arches, drawn with rails and a rung. Beside it, the 8 copies of a wave shrink to a target point 20 mm above it and meet at points. On the right, a circle spirals in to its middle. From M10.11, at the bottom in green, 5 guided ripples. 8 copies of a wave follow an arch, shrinking to 30 % as they turn with it. Copies of a circle 5 mm across follow 2 curved rails, centred between them. A column of 5 lines has a grid across it 3 mm apart, sewn first, and copies of a thin diamond stand across a ladder at its rungs, an anchor line across its middle. Last, 7 copies of a zigzag along a straight guide, sewn through their nodes, are cut to a circle. 2 satin columns of one thread that cross, as drawn beside the same 2 with their underlays first (M10.15) | Whether the ripples' lines and the guided copies keep their spacing, how their joins look, and if the grid lies flat across its column with the clipped zigzag's pieces joined by no visible thread. Jumps, trims and time on the routed halves against the halves as drawn, and whether the runs under the satins show, and under the bean stitches of the lines. Whether the crossing columns, their underlays sewn first, lie flatter than as drawn. The height of each crossing above the fabric, with calipers. Whether the bar's satin bulges or its underlay shows where the post crosses it. How the needle goes back from the post's underlay to the bar | MC-12 |

## Checkpoint calendar

| Checkpoint | After | Sheets | Questions it must answer |
|---|---|---|---|
| MC-1 | M1 | TS-01, TS-02, TS-10A/B/C | Does the machine read our PES? Right size and orientation? Which trim encoding works? Does each hoop take a design as large as its profile allows, from PES v1? |
| MC-2 | M3 | TS-03, TS-04, TS-02B | Minimum stitch length and lock defaults? Do the locks hold where the machine cuts the thread, and where a jump thread is snipped? |
| MC-3 | M4 | TS-05, TS-06 | Satin spacing, width limits, underlay defaults, pull compensation |
| MC-4 | M5 | TS-07, TS-08, TS-09 | Fill spacing, pull compensation, gap-filling rows, underlay and travel defaults |
| MC-5 | M6 | TS-12, TS-10 | Is the VectorCraft workflow complete and correct? Does a test sheet drawn by the Test Sheet filter sew as `stitch testsheet` writes it? |
| MC-8 | M13.12 | TS-13, from `stitch testsheet` or drawn in VectorCraft by the Test Sheet filter with the Auto effect | Do pictures digitized automatically sew better than the same shapes in paint order? Are the widest satin column, the narrowest triple run, the overlap, the base layer, the underlays by width and the columns' pull compensation right? |
| MC-6 | M7 | TS-11A and TS-11B, from `stitch testsheet` or drawn in VectorCraft by the Test Sheet filter, and TS-11C from `stitch testsheet` | New stitch types acceptable? Are their defaults right? |
| MC-11 | M8 | TS-16, from `stitch testsheet`, beside Ink/Stitch's machine file of it, which the differential keeps (`target/conformance/differential/ts-16.inkstitch.pes`) | Does an Ink/Stitch file sew from StitchCraft's machine file as it does from Ink/Stitch's? |
| MC-12 | M10 | TS-17A, TS-17B, TS-17C, from `stitch testsheet` or drawn in VectorCraft by the Test Sheet filter, but TS-17B and TS-17C only from `stitch testsheet`. TS-17B's guided leaf, fan and egg follow guides that a filter can't group with them (M10.9, M10.10). A document's coordinates round the points where TS-17C's routed letters are cut, and a piece of its T then sews its first stitch 0.05 mm off. In M10.12 the filter drew TS-17B's linear gradients as gradient paints, which sew as the engine sews them | New stitch types acceptable, and their defaults right? Do ripple stitches keep their lines apart and their joins neat, and do guided copies follow their guides? Are there fewer jump threads on the routed designs than on the same elements as drawn? With their underlays first, do satin columns that cross lie flatter? Does each linear gradient blend its colours with no step between its stops? From M10.9, does a guided fill's rows bend with its guide and keep their spacing round its bends? From M10.10, do offset rows keep their spacing where copies would crowd, and do a buffer's rings hide their joins and their starts? |
| MC-9 | M14 | TS-14 | Do the fonts sew legibly across the heights they claim? |
| MC-10 | M15 | TS-15 | Do fonts ported automatically sew as legibly as fonts digitized by hand, from the least height of their range? |
| MC-7 | M12 | all | Release regression |

Before MC-8, cases answer the TS-13 checks that its stitches can, and count the jump threads that later
stitches sew down ([image to embroidery › Testing](../design/image-to-embroidery.md#testing)). The sew-out
answers the rest, and shows whether those threads show. On 2026-11-01 TS-13B's digitized T 4 mm tall
gained the arm of its bar that it had lost, and the leg of its A 6 mm tall that sewed as a line became a
column like the other. M13.12 then changed what the digitized halves of TS-13A and TS-13B sew, as the
pictures' layers are flattened before they are digitized. It also added TS-13D. A TS-13 file written before
then sews as it did: write the sheet again before sewing it, and check that its stitches number as the
[test-sheet reference](../user/reference/test-sheets.md) says. On 2026-11-08 a narrow part's column
stopped running along its cut into one of the cut's corners, where it sewed a fan of stitches and left
fabric bare beside it. The column now ends along the whole cut. TS-13C's digitized star then sews its
right-hand point from its tip to its cut. At MC-8, look at each point's end along its cut for fabric
showing at the cut's corners. Later that day M13.15 measured the column's width inside its own outline,
and made an open column's rail cut across the outline's slivers narrower than a thread. TS-13B's word
4 mm tall then sews the outer legs of its M as satin columns, 1.05 mm wide where the M's middle strokes meet
them. Do they sew as cleanly as the triple runs beside them, and does the M keep its shape?

Lettering sews from M14.2, before TS-14 is drawn. Until then, sew a word in Ink/Stitch Small Font at the
ends of its range on the PE800, from a copy of Ink/Stitch's font folder:
`stitch lettering "Stitch" --font small_font --scale 70 -o small.pes` for its least height, 3.6 mm, and
`--scale 300` for its greatest, 15.2 mm.

- At 3.6 mm, does every letter read, and do the stitches that join the letters on the fabric show?
- At 15.2 mm, do the satin columns cover, and how long do the jump threads between the letters take to cut?

From M14.3, sew 2 more on the PE800. `stitch lettering "Hello\nWorld" --font small_font --scale 200 -o lines.pes`
sews its second line back from where the first ended. With `--path arc.svg --scale 300`, `Hello` follows an
arc drawn in an SVG file.

- Between the 2 lines, is the thread as short as the picture draws it, and does the second line, sewn from
  the right, look like the first?
- On the arc, do the letters stand evenly, and do the columns of a turned letter cover as those of an
  upright one?

From M14.4, sew a name typed in VectorCraft on the PE800. Type 2 lines as point type, apply
**Effect › Plug-ins › StitchCraft Lettering** with Size 150, and export to PES.

- Does the text sew where it stands in the document, beside a shape drawn next to it?
- Is 7.6 mm the height you expected from the dialog, without a picture of the font on the canvas?

From M14.5, sew a lettered patch twice on the PE800. The first has its word traced as shapes. The second
is the same picture planned with `--text` round the word, in Ink/Stitch Small Font
([letter a picture](../user/how-to/letter-a-picture.md)).

- Which word reads better at 10 mm, and which at 6 mm and below?
- Is the field whole under the lettering, with no fabric showing round a letter?

From M14.7, sew a drawn badge with a word typed on it, as text, on the PE800. Plan the SVG file with
`--auto` and Ink/Stitch Small Font in the font folder.

- Does the word stand where the drawing has it, and is its height the one the note gave?
- Is Small Font's word, beside the drawing's typeface, a word you would sew, or would you rather have had
  the text left out?
- Does the lettering stand where the picture has the word, and is it as tall?
- Are Small Font's strokes too light beside the drawn letters?

From M14.8, sew the word MATCH 3 times at 15 mm on the PE800: traced as shapes, in Ink/Stitch Small Font
and in Barstitch Bold. `stitch lettering "MATCH" --font FOLDER --height 15` sets each font.

- Which font's strokes look like the traced letters'?
- Do Barstitch Bold's wide columns pull the fabric in, or sew too thick where 2 strokes meet?
- At 10 mm, where only Small Font sews, is the word too light beside the traced one?

MC-8 also sews one of the owner's traced dogs, `docs/fixtures/traced-dog-running-9.vectorcraft`, 100 mm
tall: `stitch plan docs/fixtures/traced-dog-running-9.vectorcraft --auto -o dog.pes`. Does the fur read as
fur, the face and the eyes as hers, and do its 83 jump threads, which the PE800 doesn't cut, take long to
snip? 3 rows of its fills show beside thin lines of fur of another colour (M13.18): do they show?

M13.27 changed what the dog sews: 2 blobs of its fur that sewed as satin columns are fills now, as a small
star is ([which stitch a shape gets](../design/image-to-embroidery.md#which-stitch-a-shape-gets)). Write
the dog again before sewing. MC-8 also sews the demo's badge, `web/templates/alpine-night.svg`, twice:
`stitch plan web/templates/alpine-night.svg --auto -o badge.pes`, and the same with
`--set auto_fill_outline=false`.

- Do the 2 stars, 5.3 and 4 mm across, read as stars of 4 points when they are filled?
- With the outline, are the stars' points sharper, and is the run round the moon and the mountains on the
  fill's edge or beside it?

M13.31 moved the 2 buttons of TS-13D to a sheet of their own, TS-13E, so that VectorCraft exports each
sheet with room in its budget ([compatibility gate](../design/compatibility-gate.md#level-a--abi-contract-fast-minutes)).
No rule changed. A TS-13D file written before then holds the buttons too: write TS-13D and TS-13E again
before sewing.

M13.28 changed what every automatic design sews: each fill sews an outline now
([an outline round a fill](../design/image-to-embroidery.md#an-outline-round-a-fill)). Write the badge, the
dog and TS-13A to TS-13D again before sewing.

- Does the moon's outline, 18 stitches round a circle 9.6 mm across, read as round?
- M13.17 runs the thread under shapes sewn later in place of 13 of the dog's jumps, and of one of the
  badge's ([runs in place of jumps](../design/image-to-embroidery.md#runs-in-place-of-jumps)). Write the
  dog, the badge and TS-13B again before sewing. Does a run show through the shape over it, as a ridge or
  as a darker line under a light thread? Does one peek out at the edge of a fill?
- On the dog, do the outlines sharpen the fur's edges or draw lines that the photo doesn't have?
- An outline sews through a stretch of its edge under a later shape. Does that run show through the shape
  that covers it, or raise it?

M13.22 changed the order that the digitized halves of TS-13A, TS-13C and TS-13D sew in, and the dog's:
write them again before sewing. The elements now sew in the order that changes thread least
([sewing order](../design/image-to-embroidery.md#sewing-order)), which MC-8 checks:

- The dog changes thread 12 times, where it changed 28 times. Count the rethreads at the machine: do they
  match, and is each thread's turn as long as the plan says?
- Some satin columns now sew before the fill beside them and reach 1 mm under it, where every column sewed
  after its fill. Along the dog's columns of fur, does the fill's edge show on the column, or fabric
  between them?
- A thread that sews once jumps where it changed before. Are the dog's long jump threads, some across the
  whole design, in the way of the stitches sewn after them, or sewn down by them?

M13.21 changed what the digitized halves of TS-13A and TS-13C sew, and what the dog sews. Automatic digitizing now chooses a part's stitches from its geometry
([which stitch a shape gets](../design/image-to-embroidery.md#which-stitch-a-shape-gets)), and its bounds
are StitchCraft's own values, which MC-8 confirms or changes:

- On TS-13A, the 2 large badges' blue squares are fills, whole under their gold discs, where they were satin
  columns round the discs with stitches of 8 mm at the corners. The 3 small badges' squares are columns
  still. Which sews flatter, and does the gold disc lie flat on the fill?
- On TS-13C, the fill of the star's middle goes on into each point as far as a disc 4 mm across fits. The
  tips sew as columns. Does a point read as well as it did as one column from its cut 6.6 mm wide?
- Sew the dog twice, as planned and with **Fill uneven parts** off (`--set auto_fill_uneven=false`), which
  sews its ragged patches as columns again. As planned it changes thread 12 times and jumps 108 times,
  and with the switch off 10 and 132 times. Which reads better, and which takes longer at the machine?
- A blob sews as a small fill, where a column bent into its corners. Compare the dog's nostrils and the
  buttons' dots of TS-13D with a satin dot drawn by hand. Does a fill 1 to 2 mm across sew cleanly without
  an underlay?
- The longest satin stitch between a column's rails is 7 mm. Is that the right limit on the fabric sewn?

M13.16 changed what the digitized half of every TS-13 sheet sews, and what the dog sews. A design digitized automatically now sews a base layer first, an open underlay under all of
it, and its fills sew no underlay of their own. Its neighbours overlap by 1 mm where they overlapped by
0.4 mm. Its fills turn from row to row in a stitch 0.3 mm long at the outline
([covering the fabric](../design/image-to-embroidery.md#covering-the-fabric)). Whether an underlay belongs
under dense fills is disputed among digitizers, and no source gives an overlap in millimetres. Both
defaults are StitchCraft's own, set from measurements on pictures, and MC-8 confirms them on fabric or
changes them:

- Sew the dog on black fabric twice, as planned and with the base layer off. The Auto effect's dialog in
  VectorCraft turns it off (`auto_base_layer`), and each fill then sews its own underlay.
- On black, does fabric show between patches, with the base layer and without it?
- Since M13.21 the top thread of this dog has 0 slivers, and leaves less than 0.001 % of the design bare. The
  measure passes slivers up to 0.4 mm² over the base layer or within 0.4 mm of the design's outline, and 0.5 % bare
  in all (`auto_base_sliver_mm2`, `auto_base_bare_percent`). Both limits are StitchCraft's own, set from
  the 4 traced dogs. With the base layer on, can a sliver be found by eye, and does the base layer's
  thread hide it? The running dog in 6 threads has the largest, one sliver of 0.26 mm²: sew it on black
  too when the first dog leaves the question open.
- Does the base layer's thread show beside a patch's edge, or through a lighter fill sewn over it? The
  base layer lies 0.4 mm inside the design's outline, a value of StitchCraft's own, and the measure doesn't
  model how far real thread pulls an edge in. By the measure, 0.23 mm² of base thread lies uncovered
  within 0.4 mm of this dog's outline on another colour, the nearest 0.18 mm from it, against 0.02 mm²
  with the base layer 0.6 mm inside. The dog on turf in 9 threads has the most, 0.70 mm² against 0.07 mm²
  ([the numbers](../design/image-to-embroidery.md#settings-of-the-cover)). Look along the outline on
  black fabric. Where base thread shows there, sew again with `auto_base_offset_mm` at −0.6 mm.
- Which dog lies flatter and softer, and which puckers less?
- Does the overlap of 1 mm show as a ridge where 2 patches meet?
- Do the fills' edges sew cleanly where the rows turn, or does the thread pile up or break there?

M13.29 measures how thick a design sews and changes nothing that sews
([how thick a design sews](../design/image-to-embroidery.md#how-thick-a-design-sews)). Its limits, 3 and
6 fills of thread over one spot and 15 and 30 needle points per mm², are starting values from public
advice, which no sew-out has confirmed. MC-8 confirms them or changes them with a ladder of squares,
test sheet TS-13F, which `stitch testsheet TS-13F` writes and the Test Sheet filter draws:

- **Top row:** squares 15 mm a side of 1, 2, 3, 4, 5 and 6 fills on each other, each fill with the default
  rows of 0.25 mm and stitches of 3 mm, its angle turned 30° from the one under it, over one base layer.
  They measure 1 to 6 fills.
- **Middle row:** the same squares with a satin outline 2 mm wide on top, sewn after every fill of the
  sheet in black.
- **Bottom row:** 3 blocks 12 mm a side, each one fill cut into patches 1.5, 2 and 3 mm across that reach
  1 mm under the patch sewn after them in their row and the one below, as automatic digitizing cuts a
  traced photo.
- Not on the sheet: the top row with rows 0.4 mm apart, the fill that the sources call standard, which
  measures 0.6 to 3.8 fills. Draw it in VectorCraft as plain fills when the sheet leaves the limit open.
- Sew on medium cotton twill with one layer of medium cut-away, a 75/11 needle and 40-weight polyester,
  at the PE800's usual speed. Sew it again on a knit T-shirt when time allows: the limit should be lower
  there.
- For each square, count the thread breaks and the places where the thread shreds. Listen for a thumping
  needle. Look at the needle for a bend or a burr, and at the fabric round the square for puckers. Bend
  the square, and measure how thick it is with calipers.
- The first square that breaks thread or thumps sets the failing limit, and the square before it, less
  one, sets the warning. Then sew the running dog in 9 threads, whose thickest spot has 7.0 fills, and
  compare where it gives trouble with its heat map (`stitch preview --style thickness`).

M13.30 changes what sews where areas overlap
([less thread where areas overlap](../design/image-to-embroidery.md#less-thread-where-areas-overlap)).
At MC-8, look at the edges between 2 fills of the digitized halves and of the traced dog:

- Along an edge that runs the way the rows do, the lower fill reaches 0.4 mm under the upper one, where
  it reached 1 mm. Does fabric or the base layer show along such an edge?
- Where 3 fills meet, one overlap lies under the last fill, where 2 lay. Does a spot of the base layer
  show at the corner, and is the corner flatter than it was?
- Under a part of a fill narrower than 3 mm, the fills beside it reach 0.5 mm. Does fabric show beside
  such a part?
- The base layer is one layer of rows across the fills' rows, where it was 3 layers. Do the fills lie
  flat, and do they pucker more than with `auto_base_rows` set to `lattice`? TS-13F doesn't compare the 2:
  each of its squares lies on the one layer. Export one traced picture twice, as planned and with
  `auto_base_rows` set to `lattice` on its shapes, and sew both.
- A small ragged part that sewed as a short bent satin column is a fill now. Does the fur of the traced
  dog read as well without those columns?

M13.35 takes thread away where one shape piles it up by itself
([the thickest spot](../design/image-to-embroidery.md#the-thickest-spot)). At MC-8, after TS-13F's ladder:

- On the inside of a satin column's bend, half the needle points stop a quarter or half of the column
  short of the rail. Does the inside of TS-13A's blue strip and of the lettered patch's M still look
  smooth, or does it look ragged?
- Locks are 4 stitches of 0.3 mm. Cut the jump threads of the traced pear's stem and of TS-13B's letters
  by hand. Does an end pull out?
- At a spot still above 4 fills, the last pass keeps one back and forth of a lock, and moves row ends
  back under the shapes sewn after them. Look where TS-13D's white ring meets the star, and at the traced
  dogs' seams. Is fabric showing between 2 shapes there? Is a row's end showing past its neighbour?
- A satin column narrower than 2 mm walks its middle once, where it walked there and back. Do the
  lettered patch's letters and TS-13B's 6 mm and 10 mm words still stand up off the fabric, with sharp
  edges?
- An outline no longer sews through a stretch longer than 12 mm under a fill sewn after it. On the traced
  dogs, cut the jump threads this leaves. Does the edge of each outline still look whole where it meets
  the next patch?

M13.35's cap takes needle points out of the plan where a spot still sews above 4 fills, the setting
`auto_thick_cap_fills` ([the cap at the end of planning](../design/image-to-embroidery.md#the-cap-at-the-end-of-planning)).
TS-13F's ladder shows where the needle starts to struggle. At MC-8, after it:

- At how many fills did the ladder's needle start to thump or break the thread? That sets the cap's
  value and the limits of `SC-W0913`.
- The cap took a base layer's and a fill's turns under later shapes on the traced dogs and on TS-13C's
  face. Is fabric showing where 2 patches meet there?
- Where a satin column's points crowd in a bend, the cap may take one, and the stitch left crosses part
  of the column only, 14° from square on TS-13C's face outline. Can you see it on the outline's inner
  bend, and does the column still look smooth?

M13.36 keeps a traced letter's counters open and its edges whole
([holes and edges of columns](../design/image-to-embroidery.md#holes-and-edges-of-columns)). At MC-8, on
the traced lettered patch (kit item 14) beside the patch with its word as text (item 15):

- The A's counter has no pull compensation along it. Does the cream field show through it as a
  triangle, as on item 15? Measure it across at its base, and say whether the A reads as an A from a
  metre away.
- A straight column moves a crowded point no farther than its edge and a quarter of a thread. Run a
  finger along the M's stems and the A's legs: are their edges smooth, without the notches of MC-8's
  first sew-out?
- Where the A's legs meet its crossbar, and the M's stems meet its diagonals, does fabric or field show
  between the columns?
- TS-13B's 6 mm word has 2 more jump threads. Cut them by hand. Does an end pull out?

M13.37 sews a traced letter's straight strokes each as a column of its own
([letters sewn as strokes](../design/image-to-embroidery.md#letters-sewn-as-strokes)). At MC-8, compare the
traced lettered patch (kit item 14) with the patch with its word as text (item 15):

- Do the M's stems run top to bottom with level stitches, and its middle strokes stop against them, as on
  item 15? Does the A's apex split upright, and does its crossbar stop against both legs?
- Where a stroke stops against another, as the T's stem under its bar, the H's crossbar against its stems
  and the M's middle strokes against its stems, does fabric show where they meet after sewing? The stroke
  that stops ends on the other's side, and the other's pull compensation is what reaches past that side. If
  fabric shows, the stopping stroke reaches 0.4 mm into the other again, in a step of its own.
- TS-13D's emblems sew their white frames as columns along their sides, where they were filled. Do the
  frames' corners sew cleanly, and is the stacked emblem's white ring, 4.23 fills at its thickest, stiff?

M13.18 merges the thin lines of a traced photo into the patches beside them
([neighbours](../design/image-to-embroidery.md#neighbours)). It changes what the traced dogs sew, and no
test sheet. At MC-8, sew the running dog in 9 threads from `main` before M13.18 and from after it, side by
side:

- A ragged strand of fur narrower than 1 mm now sews as part of the patch beside it, in that patch's
  thread. Does the dog still read as the photo does, its eyes, nose and the white blaze on its face clear?
  Which version looks more like fur?
- The dogs leave 15 to 32 % fewer jump threads. Count the ones you cut on each.
- The dog on turf in 9 threads sews 0.31 fills more at its thickest spot, 4.8 fills, where a satin column or a fill lies
  over 3 or 4 other fills. Does the needle thump there, or the thread fray?

M13.39 keeps the parts a person marks ([marks that keep parts](../design/image-to-embroidery.md#marks-that-keep-parts)).
Without a mark nothing sews differently. At MC-8, sew a traced dog with a `stitch:keep` mark round each eye
and its nose, beside the same dog without marks:

- A marked speck sews as a short running stitch, 1 to 2 mm long. Does the light in an eye show as a dot,
  and does its short stitch hold, or pull out once its jump threads are cut?
- A marked part sews in its own thread over the fur round it. Are its edges whole, and does fabric show
  round it?
- The rest of the dog sews in another order. Feel for thick spots away from the marks.

M13.40 sews a detail after the fills round it where that costs no thread change, when the design has
`auto_details_last` on ([details last](../design/image-to-embroidery.md#details-last)). It landed off,
M13.45 turned it on by default, and M13.48 turned it off again. At MC-8, sew a traced dog with it off and on,
side by side:

- Its eyes and nose sew after the fur round them when it is on. Are their edges whole, without fur rows
  lying over them?
- Measure the dark parts of the face across, and compare them with the trace. Are they smaller with it
  off?
- Feel for thick spots away from the face: on, a moved fill can lay more thread elsewhere.

M13.45 turns on by default both M13.40's details last and M13.44's merging by likeness: a thin line that is
a whole part of its colour sews as a line where merging it would take the picture under 0.85 of its likeness
([merging by likeness](../design/image-to-embroidery.md#merging-by-likeness)). Of the repository's
pictures only the traced dogs and the marked patch drawn abutting sew differently. At MC-8, sew the dog on turf and the running dog in 6 threads from `main` before
M13.45 and from after it, side by side:

- The lines it keeps sew as running stitches over the patches. Do they read as the strands of fur and the
  pale strip above the nose of the photo, or as stray thread?
- It leaves 4 jump threads more. Cut them by hand, and say whether an end pulls out.
- The cap limits every spot to 4 fills in both. Feel the areas at 3 fills or more: 137 mm² before and
  126 mm² after. Is either stiffer?
- On the running dog in 6 threads, the grey ring round the tan patch on its chest sews before the patch, so
  the tan lies on top. Is its edge whole, and does it read as tan from a metre away?

M13.48 turns details last off by default again and keeps merging by likeness on. Of the repository's
pictures only the traced dogs sew differently from M13.45. At MC-8, sew the dog on turf in 9 threads and the
running dog in 6 threads from `main` after M13.48:

- They leave 4 and 7 jump threads fewer than with M13.45. Count the cuts by hand on each.
- Do the eyes, the nose and the tan chest patch keep their edges with the fur sewn after them?

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
