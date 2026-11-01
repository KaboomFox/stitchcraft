# Image to embroidery

<!-- implements: crates/stitchcraft-engine/src/auto/**, apps/stitchcraft-cli/src/commands/palette.rs, apps/stitchcraft-cli/src/trace.rs -->

Automatic digitizing makes a design that sews well from a picture of flat colours, such as a logo, clip
art or a drawing, once it is traced into shapes. Settings on the shapes are optional. StitchCraft chooses
the stitches of each shape by its width at the size it sews. Neighbouring shapes overlap where they meet.
Specks and the background are left out, and the sewing is ordered for few colour changes and short jumps.
Milestone M13 of the [roadmap](../plan/roadmap.md) builds it, after M7.5 and before M7.6. Photographs
sewn in blended colours need another technique, outside the roadmap.

## The flow

A picture becomes a machine file in 2 steps:

1. In VectorCraft, place the picture, scale it to the size it will sew, and trace it with Object › Image
   Trace › Make.
2. Run `stitch export picture.vectorcraft --auto -o picture.pes --preview picture.png`.

With VectorCraft installed, `stitch export picture.png -o picture.pes` takes both steps itself
([below](#pictures-from-the-command-line)).

An Image Trace object is a group of the traced paths, with the source image hidden beside them
(`imageTrace.make` in VectorCraft's `crates/engine`, `main` at `0842941`). `stitch export` already reads
those paths, and Expand is optional. Without `--auto` it sews each of them as Ink/Stitch sews an SVG
shape: every area as a tatami fill and every line as a running stitch, in paint order. In a test document
of a traced square and a strip 2.1 mm wide beside it, both sewed as tatami fills. With `--auto`, each
shape sews as this page describes. Once MC-8 confirms the rules on the machine, M13.8 makes automatic
digitizing the default for VectorCraft documents, and `--auto` is then needed for SVG files only.

VectorCraft's exporter plug-ins ([P1](rfc-vectorcraft-abi-v2.md#p1-exporter-plug-ins), 0.14) move the second
step into VectorCraft, as File › Export › Export As › PES, which StitchCraft's formats are exporters for since
M6.9. Their `auto` option is `--auto`. Since M13.7, StitchCraft's Auto effect on the traced group says it in
the document itself, with the rules' settings in its dialog, and previews each path's stitches: the
document then sews digitized automatically from either step, without the switch.

An SVG file traced elsewhere, such as by Inkscape's Trace Bitmap, sews the same way with
`stitch plan picture.svg --auto`. Without the switch, an SVG file's shapes sew as Ink/Stitch sews them,
since the file may be one of Ink/Stitch's.

## What VectorCraft does and what StitchCraft does

The 2 programs split the work. VectorCraft's Image Trace (its `crates/trace`) already does what a picture
needs before it is embroidered:

- Its colour modes reduce the picture to a few flat colours. Its Document Library palette traces with a
  swatch library's colours exactly.
- Its Noise setting leaves out areas smaller than a number of pixels, and Ignore White leaves out white
  areas.
- Its Abutting method traces each colour's areas as paths that share their edges with their neighbours.
- Create Strokes traces lines up to its Stroke width as stroked centre lines.
- The paths are smooth curves over the steps of the pixels.

StitchCraft links none of VectorCraft's crates ([integration](vectorcraft-integration.md)), and a tracer of
its own would repeat VectorCraft's. StitchCraft does the embroidery. It chooses the stitches of each shape,
the overlaps between shapes and the order they sew in.

`stitch palette` (M13.7) writes a machine profile's thread palette as a GIMP palette file (`.gpl`).
VectorCraft loads that file as a swatch library (Window › Swatch Libraries › Other Library…), and Image
Trace's Document Library palette then traces the picture in the threads' colours.

## Automatic elements

An automatic element is a shape whose stitches StitchCraft chooses. The readers make one of:

- each object of a VectorCraft document that has no StitchCraft effect, with `--auto`;
- each path with the StitchCraft Auto effect, on the path itself or on a group or a layer around it, with
  or without `--auto` (M13.7, `REQ-VC-016`). A path nearer to a StitchCraft stitch effect, its own or a
  group's, sews as that says. The effect's parameters are the shape's settings of the rules
  ([below](#settings)). A stitch effect says how an object sews, so an Auto effect beside one isn't used,
  and `SC-W0807` says so;
- each SVG element without `inkstitch:` settings, with `--auto`.

An element with settings is not digitized automatically, whether or not the switch is given: it sews as
its settings say, as far as its reader reads them. So a picture can be digitized automatically and one of
its shapes then set by hand. (The SVG reader reads no `inkstitch:` settings but `ignore_object` until M8,
and says so with `SC-W0802`, so such a shape sews with the defaults until then.) A shape's fill gives its
area and its stroke gives its line, each with its thread, width, join and cap, as for any element. A trim
or a stop that an Ink/Stitch command asks for is not a setting of the stitches, and an automatic element
keeps it. An element sewn as several elements keeps it for the last of them.

The readers mark the element (`Element::automatic`), and they read the switch from `Reading`, which every
host takes. Before anything is generated, the engine turns each automatic element into ordinary elements
whose settings say what to sew (`stitchcraft_engine::auto`). The generators, the assembly and every host
then treat them as shapes set by hand. `SC-I0901` counts the shapes digitized, by the stitches they sew
as. An automatic line sewn across its width follows the drawing, and the reader names what the drawing
draws across the line that its stitches don't follow (`REQ-AUTO-005`). Of a VectorCraft document, that is
the line's alignment inside or outside a closed path and its width profile (`SC-W0807`). Of an SVG file,
it is transforms that stretch the line more one way than the other (`SC-W0802`). Under such transforms a
viewer draws it wider one way, and its stitches are as wide as the transforms stretch it on average. Its caps are
followed (below), and so are its corners, joined as the drawing joins them. An SVG join that is not set
is SVG's miter, limited by `stroke-miterlimit`, where Ink/Stitch reads a miter limited at 5.

Digitizing doesn't add an error of its own (`REQ-AUTO-006`). A part that its width's stitches cannot sew
is sewn otherwise, or left out with a warning, and a shape that paints nothing sews as it would without
the switch. One odd shape among hundreds never keeps a design from being written.

## Stitch types by width

An automatic element is measured at the size it sews, in millimetres, to the micrometre (`REQ-AUTO-002`).
A width written in another unit that converts to a bound give or take a rounding error, such as 7 mm in
CSS pixels, is that bound. Its width at a place is the diameter of the largest circle that fits inside it
there. Its parts sew by their width:

| Width | Sewn as |
|---|---|
| Up to the design's `min_satin_stroke_width_mm`, 1 mm unless the design sets it | A running stitch along the centre line, a triple run (bean stitch) from 0.5 mm wide |
| Wider, up to the widest satin column, 7 mm | Satin columns, whose rails are the outline |
| Above 7 mm | A tatami fill |

A stroke is a line as wide as it is drawn (M13.1):

- No wider than `min_satin_stroke_width_mm` it sews as a running stitch along its path (`REQ-AUTO-002`),
  tripled from 0.5 mm wide (below). A satin column that narrow is too narrow to stitch across
  (`REQ-SAT-015`), so the bound is the same.
- Up to 7 mm it sews as a satin column as wide as its stroke, with its path as the column's centre line,
  as a satin column drawn as one path does (`REQ-SAT-016`, `REQ-AUTO-003`). A centre line is one subpath,
  so a stroke of several sews a column for each, in order. The second and later columns are named after
  the stroke with `#2`, `#3` and so on. A traced line's ends are round or square, and its caps are part of
  the line drawn. An open end with a round or square cap reaches half the stroke's width past the path's
  end, straight on, as far as the cap does. A round cap's end is then square, which covers its corners too.
  A column cannot follow a subpath that turns more tightly than the stroke is wide, such as a small ring
  traced from a dot. Nor can it follow a subpath shorter than a CSS pixel. Its rails would fold over
  themselves, or vanish. The rails the satin column would make tell whether it can follow a subpath. A
  subpath it cannot follow sews as the area it paints, filled as a wider line is (`SC-I0902`). So does a
  dot that a round or square cap draws. A dot with a butt cap draws nothing, and is left out (`SC-W0401`).
- Wider than that, the area it paints is filled (`REQ-AUTO-004`). That area is each segment widened by
  half the stroke's width to each side, with its corners joined as the stroke's join joins them and its
  open ends capped with its cap. It is the stroke's area within the shapes' tolerance. A miter longer
  than its limit is cut square. Those pieces, joined into one area, are the fill's outline. A part of it smaller than 1 mm²
  is a speck, merged into a neighbour or left out as specks are ([Neighbours](#neighbours)). An area that would reach more
  than 10 m from the origin is left out (`SC-W0903`). No design lies there.

  The pieces are not handed to the area code one by one, which would cost many times the work of their
  outline. Each open subpath is one ring instead. It follows the right side forward, the end cap, the left
  side back and the start cap. The outer side goes round each join, and the inner side goes in to the
  path's point and out again. A closed subpath is 2 rings, one for each side. Such rings wind round each
  point as many times as the pieces cover it. The nonzero rule then fills exactly the pieces' union. A line
  of more than 32 chords is cut into lengths of 32 chords. Each length is a ring made into its area alone,
  and the areas are then joined. A 10 mm line a metre long fills within the default budget that way.

**Triple runs** (M13.4, `REQ-AUTO-015`). A running stitch that automatic digitizing makes is a triple run
from 0.5 mm wide: each stitch is sewn 3 times, bean stitch 1. A running stitch sewn once is as fine as the
thread, and a line drawn half a millimetre wide or wider sews bolder tripled. A stroke is as wide as it is
drawn, and a narrow part (below) as wide as the circles that fit inside it along the middle. Widths are
compared to the micrometre, and MC-8 confirms or changes the bound.

Every automatic satin column, a stroke's as well as a shape's, has the settings of its width (M13.5,
[below](#settings-by-width)). A shape's area is a tatami fill apart from its narrow parts (M13.4), its
parts as wide as a satin column (M13.5) and the narrow parts of its broad ones (M13.6).

**Narrow parts** (M13.4, `REQ-AUTO-013`). A part of a shape that is narrower than
`min_satin_stroke_width_mm` everywhere sews as a running stitch along its centre line, as a stroke that
narrow does. No circle that wide fits inside such a part: shrunk by half the width with the polygon buffer
of M5.7, nothing of it is left. The width is taken half a micrometre wider, so that a part as wide as the
bound, give or take a rounding error, is narrow, as a stroke is. Before that, a part smaller than 1 mm² is a
speck ([Neighbours](#neighbours)), and stays in the area. So does a part whose centre line has no length, a
dot.

- **What sews.** The rest of the shape is a tatami fill, and each narrow part's line sews after it, in the
  order of the parts. It is named after the shape with `#2`, `#3` and so on, or with the shape's own name
  when nothing of it is filled. A trim or a stop after the shape follows its last part.
- **Its width.** A part's width at a point of its centre line is the diameter of the largest circle centred
  there that fits inside it. Averaged along the line, it says whether the part is a triple run
  ([centre lines](algorithms/centre-lines.md#width-along-the-line)). A strip as wide all along as a stroke
  is as wide as the stroke.
- **To its neighbours** a narrow part's line paints the part. It sews after a fill it borders, and the
  areas sewn before it reach under it as under a satin column ([Neighbours](#neighbours)).
- **As drawn.** A shape's narrow parts are found as it is drawn, before its neighbours. A part that only
  the areas sewn after it leave narrow is cut away round them and filled with the rest of its shape. A
  stacked trace's dark outline under the colours inside it is often such a part. M13.6 keeps finding
  parts as drawn. An abutting trace draws such an outline as the ring it shows, which sews as lines and
  columns. Finding parts again after the cut would digitize each shape a second time, after its
  neighbours.
- `SC-I0901` counts a shape whose every part runs as a running stitch, and any other as a tatami fill.
  Finding a part's width and centre line is part of the work of digitizing its shape. A part whose width
  or centre line can't be found, a bug, stays in the fill, and `SC-E0009` names its shape.

**Parts as wide as a satin column** (M13.5, `REQ-AUTO-016` to `REQ-AUTO-018`). Filled, a part as wide as a
satin column sews rows no longer than it is wide wherever they cross it. A digitizer sews it as satin
columns along its strips and round its rings. A part that isn't narrow, and is nowhere wider than 7 mm, is
as wide as a satin column. Shrunk by 3.5 mm, taken half a micrometre wider, nothing of it is left. Such a
part sews a satin column along each of its centre lines, between its own outlines
([centre lines](algorithms/centre-lines.md#columns)):

- **Its columns.** Each line sews one column over the triangles round it. A line is cut where it turns by
  more than 60°, so that the column before a corner covers it and the one after ends under it, as an arm
  of a T ends under its bar. A branch of the line is a column of its own however short, as each arm of a
  t's crossbar is. A branch to the second sharp corner of an end is a bump instead, sewn with the column
  beside it. A column's rails are the part's outline on either side, and its rungs lie square to it.
- **Their ends.** A column that ends where another runs on ends under it: it sews first, and is 0.4 mm
  longer at that end, by its push compensation, so that no fabric shows where they meet. Otherwise a column
  ends at its farthest point along its line, flat along the end's edge when that lies within 20° of square
  to the column, and at a point otherwise.
- **Their order.** A column that ends under another sews before it. Then the longest sews first, and each
  next column is the one with an end nearest where the last one ended, sewn from that end. A ring starts
  at its rung nearest. Where the columns that end under each other go round, as a pinwheel's bars do, the
  first in that order sews first, and the last ends on top of it.
- **Their width.** A column is as wide as the circles that fit inside the part along its line, as a narrow
  part is. One no wider than the design's narrowest satin stroke runs along its line as a triple run.
- **What sews.** The columns sew after the rest of the shape's area, named as a narrow part's lines are,
  and paint the part to its neighbours. `SC-I0901` counts a shape as satin columns when none of it is
  filled and one of its parts sews a satin column.
- **Drawn.** Each column is drawn with its rails and rungs, as the Stroke to Satin filter draws one, and
  read back it must give them. A part whose triangles don't make a column round one of its lines, or one
  of whose columns is read otherwise, stays in the fill, and `SC-I0902` says so. A dot, whose line has no
  length, stays in the fill too. A rung added to tell a column's rails from its rungs goes across an end
  that doesn't reach under another column where one can take it ([satin
  columns](algorithms/satin.md#drawn-with-its-rails-and-rungs)).

**Shapes in parts** (M13.6, `REQ-AUTO-019`). A shape wider than 7 mm in places and narrower in others sews
in parts. Its broad parts are a tatami fill, and its narrow parts sew over the fill as lines and columns,
as a digitizer sews a balloon's string over the balloon:

- **Its broad parts** are what an opening with a disc 7 mm across leaves: the shape shrunk by 3.5 mm and
  grown back by 3.5 mm, with the polygon buffer of M5.7, each taken half a micrometre wider, as the bound
  is. An opening finds them without the shape's centre lines.
- **Its narrow parts** are the rest of the shape, cut where they meet the broad parts. The opening rounds
  each convex corner of a broad part, and leaves the corner as a narrow part. A narrow part that reaches
  less than 1 mm² beyond 3.5 mm from the broad parts is such a corner, or a stub too short for a column,
  and stays in the fill, as a speck does. Length along its centre line tells no corner. The piece a
  square corner leaves has a horn along each side of the corner, and its centre line runs 6 mm along
  them, less than half a millimetre wide.
- **The cut.** Where a narrow part meets a broad one, the opening leaves an arc, and a column would end
  there in a point at one of its corners. Each narrow part is cut square there instead, at a chord across
  it square to the way its sides run on, from the end of the arc that lies farther along that way. What
  lies past the chord goes to the fill. A column ends flat along the cut, however long it is, so that a
  star's points end along the star's middle.
- **What sews.** Each narrow part sews as a part of its own. One narrower than
  `min_satin_stroke_width_mm` everywhere runs along its centre line, and one no wider than 7 mm sews as
  satin columns. Any other stays in the fill. The fill is the shape without its narrow parts. It sews
  first, and reaches 0.4 mm under the lines and columns sewn over it ([Neighbours](#neighbours)).

**Centre lines.** A narrow part's centre line, its width along it and the rungs across it come from a
constrained Delaunay triangulation of its outline, with points added along its long edges so that no
triangle is much longer than the part is wide:

- Each edge of a triangle that lies inside the part is a chord across it, as long as the part is wide
  there.
- A triangle with 2 inside edges is part of a strip, and its inside edges are the strip's rungs.
- A triangle with 3 inside edges is a junction, where 3 strips meet, and one with 1 is an end.

The centre line joins the middles of the chords (M13.4, `REQ-AUTO-014`). A round end stops it about half
its width from the end, on its middle, and a sharp point draws it on to the point. The small bumps of a traced outline, and
the corner past a sharp turn, would branch it, and are left out. At each junction left, the 2 branches that
run on most nearly straight are one line and the third ends there, and a ring has a closed line. No piece
of the line is sewn twice. A part's lines sew one after another, the longest first, each from its end
nearest where the last one ended. The [centre lines](algorithms/centre-lines.md) page has the rules in
full.

A part as wide as a satin column sews a column along each of its lines (M13.5, above). Its rails are the
part's outline on either side, and its rungs come from the triangles across it. A junction joins the 2
lines that run on most nearly straight, and the column of the third ends under theirs. A ring, such as a
letter O, is a closed column between its outer and inner outlines. The satin generator sews these columns
as it sews columns drawn with rails and rungs.

### Settings by width

Every automatic satin column, a stroke's or a part's, has the underlay of its width (`REQ-AUTO-018`),
compared to the micrometre. These are starting values, and MC-8 confirms or changes them:

| Column width | Underlay |
|---|---|
| Below 2 mm | Centre walk |
| 2 to 4 mm | Contour |
| Above 4 mm | Contour and zigzag |

A stroke's column is as wide as the stroke, and a part's as its width along its line. Each has a pull
compensation of 0.4 mm, the overlap an area reaches under the shapes sewn after it: the thread pulls a
column in from its rails as it pulls an area in from its edges, and stitches that reach that far past
the rails meet the shapes beside them with no fabric between. A column ends at its rails' ends rather
than at the point nearest the next shape, which would join its ends to the next shape with a straight run
across it. Fills keep the tatami fill's defaults, underlay included.

## Neighbours

The shapes of a traced picture meet. VectorCraft's Abutting method traces each area with a hole where the
next one lies, and its Overlapping method, like Inkscape's Trace Bitmap with its Stack option, traces each
colour whole, over the colours under it. Sewn as drawn, either sews badly where shapes meet, and digitizing
mends it in 3 steps, in this order (`stitchcraft_engine::auto::neighbours`, M13.2).

1. **Background** (`REQ-AUTO-009`). The lowest shape is the picture's background when it is an automatic
   area with a shape on another path drawn on it, and its outlines cover the box around the design to within
   0.5 mm of each side, its holes and notches aside. The box is the one around the defining points of every
   shape's path, the background's own included. So a disc in the corner of a square, lowest but covering
   less of the box, is part of the picture. A shape on its own path is the same object's: the readers make a
   fill and its stroke 2 elements on one path, and a design of one shape with a stroke sews it. Where a
   shape of a tightly cropped picture touches its edge, the Abutting method cuts a notch in the background
   instead of a hole. A part of the box its outlines leave is a notch when the areas drawn on it fill it, to
   within twice the shapes' tolerance, and it runs along the outlines at least as far as along the box's
   sides, measured 0.5 mm inside them. A part that runs further along the sides is no notch. A disc or a
   rounded square that reaches the sides leaves such parts in its corners, and sews as the picture, though
   the shapes in its corners fill the rest. The cost of this rule is a background cut away along a whole
   side, by a bar down the side say, or cut in two by a band from side to side, which is sewn. The
   background is left out with `SC-W0906` before anything is digitized. With a StitchCraft effect, it sews
   as that effect says. Finding it has the work budget to itself, and a background that needs more is sewn,
   which `SC-W0906` says.
2. **Specks** (`REQ-AUTO-008`). An automatic area, or a part of one, smaller than 1 mm² is too small to sew:
   it would sew a knot. It is merged into the automatic area it shares the longest edge with, which may be
   its own. A speck that fills a hole goes into the area around it, and no hole shows where it was. One that
   rounding left a hair from the rest of its own area rejoins that. On a tie, it goes into the area drawn
   first. The edge 2 shapes share is how much of the area's rings lies within twice the shapes' tolerance
   of the speck. A speck that doesn't share an edge with an automatic area is left out, and so is one whose
   neighbours need more work to find than the budget allows it: put back as drawn, it could overlap a speck
   merged in beside it. `SC-I0905` names each speck, by its area rounded to a hundredth of a square millimetre and
   kept under 1 mm², and where it went. A line filled because it is too wide for a column (M13.1) is an area
   too, so a part of it that small is a speck.
3. **Overlaps** (`REQ-AUTO-007`). Pull draws stitches in from the edges of their shape, so shapes that only
   meet show fabric between them. Each automatic area reaches 0.4 mm under each area, satin
   column, filled line or narrow part's line sewn after it, from where it touches that shape, and no
   further than that shape.
   Shapes touch where their edges meet within twice the shapes' tolerance, so a gap that rounding left
   between edges drawn as one closes too. Along an edge where a shape runs on near the area without
   touching it, the area doesn't reach, and fabric shows between them as drawn. A satin column covers the
   area between its rails as they are sewn, turned to run the same way. Where an area sewn after it covers
   part of it, it is cut away under that area, 0.4 mm inside the area's edge, and that part is sewn once: a
   stacked trace then sews the areas of an abutting one. A part that the areas sewn after it cover whole
   sews nothing, however near their edge it lies. So does an area they cover whole. A trim or a stop after
   such an area has no place, as after any element that sews nothing (`SC-W0505`). `SC-I0901` counts only
   the shapes that sew. A narrow part's line covers its part, and isn't cut away.
   It sews nothing when the areas sewn after it cover the part whole, and sews whole when they cover the
   part in places. A column or a line sewn after an area goes over it, and
   the area is sewn whole under it. Cutting a fill round small letters adds stitches and can leave gaps
   ([Inkscape forum](https://inkscape.org/forums/embroidery/techniques-for-making-text-on-top-of-a-fill-more-crisp/)).
   `SC-I0904` counts the areas that reach under others and those cut away. It also names each area cut away
   whole, which sews nothing. The areas are named in the order they are sewn.

Only automatic areas change. A shape with settings is sewn as they say, and a satin column or a line
digitized from a stroke or a part keeps its stitches: a column is sewn past its rails by its pull
compensation, 0.4 mm, as an area is grown under the shapes sewn after it ([Settings by
width](#settings-by-width)). A fill that bounds no area is left to its fill, which says so.
The areas are taken from the last sewn to the first, so that each meets the shapes after it as they are
sewn. Each area has the work budget to itself for its specks and for its overlaps, and costs the work of the
shapes near it and a unit for each other shape it looks past, not the work of the whole design: what a
shape sewn after it paints is found once, with the work budget to itself. An area whose overlaps need more
keeps the region it has, and a speck that needs more is left out. `SC-I0904` and `SC-I0905` say so. A satin
column whose area needs more is not reached under, which `SC-I0904` says too. The regions are combined by
one function, which cuts the rings of both where they meet and keeps the faces by their winding numbers
(`normalize::region::ops`).

**Measured.** The test pictures ([Testing](#testing)) are sewn as drawn, each shape a tatami fill, and
digitized, and the edges where their shapes meet are measured with the thread pulled in by 0.4 mm, the
pull the overlap is there to make up for: the share of the edges that the top stitches' thread covers
once each is pulled in where the thread turns (edge coverage, `stitchcraft_testkit::metrics`).

| Test picture | As drawn | Digitized | Stitches as drawn | Stitches digitized |
|---|---|---|---|---|
| Stripes, 3 abutting | 98.9 % | 100 % | 2,994 | 3,221 |
| Badge, abutting | 89.1 % | 98.7 % | 3,761 | 3,773 |
| Badge, stacked | 100 % | 97.2 % | 3,981 | 3,764 |
| Badges in a row, abutting | 94.2 % | 99.8 % | 4,947 | 4,918 |
| Badges in a row, stacked | 100 % | 99.9 % | 4,872 | 4,934 |

The abutting badge's shapes, sewn as drawn, leave a tenth of their edges bare. Digitized, they overlap, and
98.7 % is covered. The stripes' edges are covered as drawn too, but by the running stitches the fills travel
on along them, under their rows, not by the rows: digitized, the rows themselves overlap. The stacked badge as
drawn sews its lower colours whole under the upper ones, which covers its edges with 217 more stitches,
6 %, sewn twice. Digitized, it sews the abutting badge's areas, within 9 stitches. The row of badges, each
the badge at three fifths of its size and sewn in the order below, covers its edges as the badge does, and
stacked it sews within 16 stitches of the abutting row. Without pull, every edge is covered as drawn and
digitized alike, to within a hundredth. `tests/auto_pictures.rs` keeps the numbers
from falling back, and MC-8 tells whether 0.4 mm is the pull on the machine.

## Sewing order

Automatic elements sew in an order StitchCraft chooses (`stitchcraft_engine::auto::neighbours::order`,
M13.3). The order is chosen after specks are merged, so that a speck merged into an area is no element of
its own, and before areas reach under their neighbours, which follows it. VectorCraft's Auto Grouping
draws each shape together with the shapes inside it. Sewn in paint order, a picture traced with it changes
thread at nearly every shape, and each change is a stop to rethread a home machine. The order comes from 4
rules.

1. **What overlaps keeps its paint order** (`REQ-AUTO-010`). Of 2 shapes whose paint overlaps, the lower
   sews first, or it would cover the upper one. An area paints its region, and a satin column the area
   between its rails as they are sewn. Lines paint the areas their strokes paint as drawn, and a narrow
   part's line the part. Shapes that only meet, as
   abutting paths do, may sew in either order. Rounding leaves a sliver no wider than twice the shapes'
   tolerance between edges drawn as one, and shapes overlap only where their overlap is wider. Where 2 lines
   0.1 mm wide cross, they overlap.
2. **A line after the fill it borders.** A satin column or a line that touches a fill without overlapping
   it sews after the fill and covers its edge, and the fill reaches under a column
   ([Neighbours](#neighbours)). Rule 1 comes first. A line that rule 1 already sews before the fill, through
   the shapes it overlaps, keeps its place.
3. **Few thread changes.** The next element has the thread that is sewing, as long as an element of that
   thread is free to sew: every element that must sew before it has. Otherwise the thread changes to that
   of the first free element in paint order. Threads are the same when their colours are, as plan assembly
   counts thread changes. A PES file gives different colours different palette entries while they last
   (`REQ-THREAD-002`), so 2 colours are one thread to the machine only past the palette's 64 entries.
4. **The nearest next.** Of the free elements of that thread, the next is the one whose box lies nearest
   the box of the element sewn last. On a tie it is the one whose box's centre lies nearest, and then the
   one drawn first. Each element then starts and ends at its points nearest its neighbours
   (`REQ-GEN-003`).

`tests/auto_order.rs` checks rules 1 to 3 together on random designs of squares and lines, wherever the
shapes fall.

Elements with settings keep their order among themselves, and automatic elements move round them within
the rules. The parts of a shape sewn as several elements, such as a line's satin columns, keep their order
too, so that a trim after the shape still comes after its last part. `--sort-colours` lets the elements
with settings move as well (`REQ-AUTO-011`), whether or not `--auto` is given. It is for a design drawn in
an order that suits drawing it rather than sewing it. The switch is the command line's. VectorCraft's
exporters order the automatic elements when their `auto` option is on, and sew the elements with settings
as drawn.
An element with a stop after it keeps its place: the elements drawn before it sew before it, and those
drawn after it after it. A user sets a stop for a reason, such as placing appliqué fabric, and it comes
where it was drawn. `SC-I0907` says when the order isn't paint order, with the thread changes of each.
M10's routing tools, which order satin columns and running stitches drawn by hand, start from this order.

Shapes are near when their boxes come within twice the shapes' tolerance of each other. An automatic area's
box is round its region, and a narrow part's line's round the part. Any other shape's box is round its
path's points, grown for a stroke by half its width times √(L² + 1) for a miter limit L. That holds the
corners of a miter clipped at its limit, as a satin column's rails clip it. Other strokes grow by half their
width times √2, which holds a square cap's corners. Rule 4 measures with the same boxes. The order finds a
shape's paint only when the shape is near an element that moves.

**Work** (`REQ-AUTO-012`). Each element has the work budget to itself to find how it lies beside the
shapes drawn after it. It costs a unit for each of them, and the work of those near it. One that needs
more keeps its place in paint order as an element with a stop after it does: nothing moves across it.
An element after it that one before it frees still waits its turn after it, and each element sews once. A
shape whose paint can't be found within the budget keeps its place beside the shapes near it. Choosing the
order has the budget once more, and costs a unit for each free element looked at each time an element is
placed. An order that needs more sews the elements not placed by then in paint order, after the others.
`SC-I0907` says each. `SC-I0904` says it of a satin column instead, whose area the areas sewn before it
then don't reach under either. A fill's generator needs the work of its region too, and says it of a fill
(`SC-E0004`).

**Measured.** The test pictures, sewn as drawn and digitized as [Neighbours](#neighbours) sews them.

| Test picture | Thread changes as drawn | Digitized | Jumps as drawn | Digitized |
|---|---|---|---|---|
| Stripes, 3 abutting | 2 | 2 | 3 | 3 |
| Badge, abutting | 2 | 2 | 3 | 3 |
| Badge, stacked | 2 | 2 | 3 | 3 |
| Badges in a row, abutting | 8 | 2 | 9 | 7 |
| Badges in a row, stacked | 8 | 2 | 9 | 7 |

The stripes and the badge on its own have one shape of each colour, which no order sews with fewer
changes, and digitizing keeps their order. The row of 3 badges, each 30 mm across, is drawn badge by badge,
as Auto Grouping draws it. It changes thread 8 times as drawn. Digitized, each thread sews its 3 shapes one
after another, and the row changes thread twice. Its red discs touch, and the needle sews on from one to
the next without a jump. The row jumps 7 times instead of 9. `tests/auto_pictures.rs` keeps the numbers
from falling back.

## Settings

The rules' values are StitchCraft's own parameters, registered as any other
([parameter registry](params.md), [reference](../user/reference/params/auto.md)), since M13.7
(`REQ-AUTO-020`). Each rule goes by the settings of the shape it digitizes (`stitchcraft_engine::auto::AutoParams`):

| Setting | Key | Default | Rules |
|---|---|---|---|
| Widest satin column | `auto_max_satin_width_mm` | 7 mm | a line's stitches, a part's columns, the split of a part into broad and narrow parts |
| Narrowest triple run | `auto_min_triple_run_width_mm` | 0.5 mm | a running stitch tripled |
| Overlap | `auto_overlap_mm` | 0.4 mm | an area reaching under its neighbours and cut away under them, a satin column's pull compensation |
| Smallest area | `auto_min_area_mm2` | 1 mm² | specks, and the narrow parts too small to cut from a broad part |
| Leave out the background | `auto_ignore_background` | on | the background, read of the lowest shape |

The defaults are the values on this page, and MC-8 confirms or changes them. A shape that sets none of the
settings takes the defaults, and an automatic shape with a setting that doesn't read sews nothing
(`SC-E0101`). Each area reaches its own overlap under the shapes sewn after it, and its specks are its own
parts smaller than its smallest area, wherever they go. The narrowest satin column is the design's
`min_satin_stroke_width_mm`, one value for the whole design, and the underlay of a column's width (above)
is no setting.

An automatic element reads the settings before it is sewn as any stitch type, and the ordinary elements it
becomes don't ([parameter registry](params.md#settings-of-automatic-digitizing)). They are in no stitch
family's dialog, and a shape that sets them is still automatic. VectorCraft's Auto effect (M13.7) sets them
from its dialog for the paths it is on and those inside the group it is on, and with `--auto` every other
shape takes the defaults.

## Previews in VectorCraft

The Auto effect is a live effect like the plug-ins of each stitch family
([plug-ins](vectorcraft-integration.md#plug-ins)). VectorCraft runs a group's effects on each path inside
it (`REQ-VC-006`), so the effect on a traced group previews every path's stitches, each sewn as its width
says. A live effect is given its path and its paints alone. Like `stitch export`, it reads the path as an
area when a fill paints it, and as a line along its topmost stroke. Its preview shows each shape's
stitches without the overlaps, the merged specks or the order, which `stitch export` adds. VectorCraft
draws what a live effect returns with every paint of its object. The stitches come back as ribbons that
the fill fills when a fill paints the object, and as lines that its stroke draws otherwise.

Digitizing is most of a preview's work, from about 1,450 instructions a unit to 2,260 in VectorCraft
0.18.0's host, and a live effect has 50 million ([live-effect previews](vectorcraft-integration.md#live-effect-previews-within-v1-budgets)).
A square's takes 12,900 units, an L's 21,000 and a ring's 31,800. The auto level digitizes within eight
tenths of the budget, then shows each element made as auto shows its family's: a fill's top rows, and a
satin column's and a running stitch's every stitch. When that costs too much it shows the outline, as a
larger square, an L, a ring and most traced letters do. Exact shows every stitch, and outline the path,
hatched across a fill. A preview that costs more than the level allows shows less, with `SC-I0810`, as the
other effects' previews do. The command line's preview always shows every stitch.

Tracing again replaces the group with a new one, without the effect. Apply the effect after the last
trace, or use `--auto`, which works without it.

## With VectorCraft's newer plug-in features

- **Exporters** (0.14, [P1](rfc-vectorcraft-abi-v2.md#p1-exporter-plug-ins)). File › Export › Export As › PES
  plans the document inside VectorCraft, and its `auto` option digitizes the objects without effects
  (M6.9). The flow is then trace and export, both in VectorCraft.
- **Effects in filter objects** (0.15, [P8](rfc-vectorcraft-abi-v2.md#p8-effects-and-stroke-style-in-filter-objects)).
  A filter could set on each path the effect of the family its width chose. The user then adjusts it in
  that effect's dialog.
- **Budget classes** ([P4](rfc-vectorcraft-abi-v2.md#p4-budget-classes-for-live-effects), still proposed for
  live effects). The Auto effect's previews of large shapes would get the heavy budget.

## Text in pictures

Text traced from a picture is shapes, and the width rules sew its letters' strokes as satin columns and
running stitches. TS-13 sews block letters from 4 to 15 mm tall to find the smallest that reads well.
Smaller letters need lettering in an embroidery font, made to sew at small sizes. M14.5 replaces the
traced letters inside a marked area with lettering of the marker's text, as tall as they were. Spike M14.6
looks for a way to find and read the text without a marker.

## Pictures from the command line

`stitch plan picture.png` and `stitch export picture.png` trace the picture with VectorCraft's command line,
as spike M13.9 decided ([ADR-0015](adr/0015-pictures-traced-by-vectorcraft.md)) and M13.10 built
(`REQ-CLI-012`, `REQ-CLI-013`). A design whose extension is `.png`, `.jpg` or `.jpeg`, in any case, is a
picture. VectorCraft publishes `vectorcraft-cli` with each release, and `stitch` runs the one
`--vectorcraft-cli` names, or else the one on the `PATH`:

```sh
vectorcraft-cli run --commands - --export FOLDER/traced.vectorcraft
```

It gives it 4 steps on standard input, where a picture of any size fits. They are the compatibility gate's
`trace` case's:

1. `swatch.library.load` loads the threads `stitch palette` writes for the machine as a swatch library.
2. `file.place` places the picture's bytes, fitted inside a square `--size` millimetres across, or else
   inside the machine's hoop 0.5 mm in from each side, its aspect kept.
3. `imageTrace.make` traces it in colour with the library's colours as they are, at most `--colours` of
   them, by the Abutting method, so that the shapes share their edges.
4. `imageTrace.expand` keeps the paths, without the picture.

VectorCraft saves the document into a folder of the command's own beside the machine file, named as
`stitch`'s temporary files are, and the folder is removed once the document is read. It runs with
`VECTORCRAFT_NO_PREFS` set, so that it neither reads nor writes the user's VectorCraft preferences, and a
picture traces the same for everyone. The document is then the design. It is read with `--auto`, planned for
the machine `--profile` names or else the reference machine, and named after the picture. `--traced` writes
it beside the machine file, to correct the trace in VectorCraft. A bug-report bundle keeps it too, so that a
replay doesn't need VectorCraft.

**The picture's size.** VectorCraft places a picture at its own resolution, or 72 pixels to the inch when
its file says none, so a logo a few hundred pixels across would sew larger than any hoop. `stitch` fits the
picture into the machine's hoop unless `--size` gives the length of its longer side. The size is the
picture's, its background included, and the shapes left once the background is out sew smaller. Fitted to
the hoop, the picture keeps 0.5 mm from each side: a shape along its edge sews as a satin column, whose
stitches reach 0.4 mm past its rails, its pull compensation, from rails that may lie up to the tolerance
shapes meet within outside the shape. Fitted to the hoop itself, a picture whose shapes reach its sides
would sew past the hoop, which `SC-E0701` refuses.

**How many colours.** Image Trace keeps at most `--colours` of the library's colours, 6 unless it says
otherwise, as in its own panel. The count takes in the background's colour. A picture with smooth edges, or a
JPEG file, has colours between its shapes' colours, which may trace as threads of their own. A `--colours`
equal to the number of colours the picture has keeps them out. A badge in 3 colours on white traces in 4.

**When VectorCraft can't trace.** A command line that can't be started, missing from the `PATH` or from
the path named, is `SC-E0815`, which says how to install it, or to trace the picture in VectorCraft. One
that fails or doesn't save the document is `SC-E0816`, which ends with what it said. Then nothing is written but
the report.

StitchCraft traces nothing itself. A tracer of its own would repeat VectorCraft's, which a harness
measures. It would also trace a picture otherwise than VectorCraft does, and the command line would then
sew it unlike the tutorial's flow.

## Testing

- Each rule gets its requirements and failing cases before its code ([ADR-0008](adr/0008-conformance-first.md)).
- The test pictures are drawn in code, as the test sheets are (`stitchcraft_engine::testsheets::pictures`),
  and no logo or picture of anyone else's is used.
- Each test picture's automatic plan is compared with its plan in paint order: colour changes, jumps,
  trims, stitches, and how much of each shape and of each shared edge the stitches cover
  (`stitchcraft_testkit::metrics`). The cases measure the pictures where `stitchcraft_testkit::pictures`
  places them.
- Level B of the [compatibility gate](compatibility-gate.md) has a case, `trace`, in which VectorCraft's
  command line traces a picture drawn in the threads `stitch palette` writes, and StitchCraft sews the
  document it saved, on every track. In its case `picture`, `stitch plan` traces the picture itself.
- `stitch plan`'s own tests run it with a stand-in for VectorCraft's command line, the test program itself,
  which saves a document VectorCraft 0.18.0 traced (`apps/stitchcraft-cli/tests/picture.rs`).
- TS-13 sews the test pictures at MC-8 ([machine testing](../plan/machine-testing.md)), each twice. A
  sheet's top half is digitized automatically, and its bottom half is sewn as drawn, in paint order. The
  bottom half sews first and stops, so that nothing digitized moves into it, and each half's thread
  changes, jumps and time can be read. TS-13A sews the pictures of neighbours and sewing order, TS-13B
  block letters and lines traced as areas, and TS-13C a face drawn with lines and shapes in parts
  ([test sheets](../user/reference/test-sheets.md)). The PE800 cuts no jump, and the sew-out reports where a
  shape sews down the thread of a jump before it.
- `tests/auto_sheets.rs` answers the TS-13 checks that the sheets' own stitches can, moved to where each sheet
  draws its shapes (`stitchcraft_testkit::pictures::sheet`). Where the bar runs on beside the square, each
  ends its rows at its own edge. The specks sew as the disc, and no shape sews over one drawn over it. A line
  traced as an area runs to within its width of its ends, and its branch from the line it meets. The smile's
  column ends where its round caps end, the columns' rungs lie within 10° of square to their strokes, and the
  face's columns cover the edges of the areas under them. The sew-out shows what only the machine can: whether
  fabric or a ridge shows once the thread pulls in, and how small a word still reads.
- The same cases count the jump threads each half leaves on the PE800, snipped at each thread change and
  stop, and how many a later stitch of their colour sews down (`stitchcraft_testkit::stitches`). Digitized,
  the letters sew as columns with a jump between each 2:

  | Sheet | As drawn: jump threads | sewn down | Digitized: jump threads | sewn down |
  |---|---:|---:|---:|---:|
  | TS-13A | 8 | 0 | 11 | 3 |
  | TS-13B | 6 | 0 | 37 | 16 |
  | TS-13C | 9 | 4 | 6 | 1 |

Codes 0900 to 0999 are automatic digitizing's ([diagnostics](diagnostics.md)).
