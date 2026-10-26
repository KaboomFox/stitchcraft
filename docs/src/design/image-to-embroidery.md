# Image to embroidery

<!-- implements: crates/stitchcraft-engine/src/auto/** -->

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

An Image Trace object is a group of the traced paths, with the source image hidden beside them
(`imageTrace.make` in VectorCraft's `crates/engine`, `main` at `0842941`). `stitch export` already reads
those paths, and Expand is optional. Without `--auto` it sews each of them as Ink/Stitch sews an SVG
shape: every area as a tatami fill and every line as a running stitch, in paint order. In a test document
of a traced square and a strip 2.1 mm wide beside it, both sewed as tatami fills. With `--auto`, each
shape sews as this page describes. Once MC-8 confirms the rules on the machine, M13.8 makes automatic
digitizing the default for VectorCraft documents, and `--auto` is then needed for SVG files only.

VectorCraft's exporter plug-ins ([P1](rfc-vectorcraft-abi-v2.md#p1-exporter-plug-ins), 0.14) move the second
step into VectorCraft, as File › Export › Export As › PES, which StitchCraft's formats are exporters for since
M6.9. Their `auto` option is `--auto`.

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
- each path inside an object or a group that has the StitchCraft Auto effect (M13.7), unless the path has
  a StitchCraft effect of its own;
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

- No wider than `min_satin_stroke_width_mm` it sews as a running stitch along its path (`REQ-AUTO-002`).
  A satin column that narrow is too narrow to stitch across (`REQ-SAT-015`), so the bound is the same.
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

Until M13.4 and M13.5, a line no wider than `min_satin_stroke_width_mm` runs once, and its satin column
has a satin column's defaults. M13.4 triples the runs from 0.5 mm, and M13.5 gives columns their underlay
by width, strokes' columns as well as shapes'. M13.1 sews a shape's area as a tatami fill whatever its
width, and the rules below for the narrow parts of shapes follow in M13.4 to M13.6.

**Shapes in parts.** A shape wider than 7 mm in places and narrower in others is split. Its broad parts are
what an opening with a disc 7 mm across leaves: the shape shrunk by 3.5 mm and grown back by 3.5 mm, with
the polygon buffer of M5.7. The rest of the shape is its narrow parts. A narrow part shorter along its
centre line than twice its width is a corner that the opening rounded, and it goes back to the fill
beside it.

**Centre lines.** A narrow part's centre line, its width along it and the rungs across it come from a
constrained Delaunay triangulation of its outline, with points added along its long edges so that no
triangle is much longer than the part is wide:

- Each edge of a triangle that lies inside the part is a chord across it, as long as the part is wide
  there.
- A triangle with 2 inside edges is part of a strip, and its inside edges are the strip's rungs.
- A triangle with 3 inside edges is a junction, where 3 strips meet, and one with 1 is an end.

A strip between 2 ends or junctions sews as a satin column. Its rails are the 2 sides of the outline
between them, and some of its chords are its rungs. At a junction, the 2 strips that run on most nearly
straight sew as one column, and the third ends under it. A ring, such as a letter O, is a closed column
between its outer and inner outlines. The satin generator sews these columns as it sews columns drawn with
rails and rungs.

**Settings by width.** A column's underlay depends on its width. These are starting values, and MC-8
confirms or changes them:

| Column width | Underlay |
|---|---|
| Below 2 mm | Centre walk |
| 2 to 4 mm | Contour |
| Above 4 mm | Contour and zigzag |

Fills keep the tatami fill's defaults, underlay included.

## Neighbours

The shapes of a traced picture meet. VectorCraft's Abutting method traces each area with a hole where the
next one lies, and its Overlapping method, like Inkscape's Trace Bitmap with its Stack option, traces each
colour whole, over the colours under it. Sewn as drawn, either sews badly where shapes meet, and digitizing
mends it in 3 steps, in this order (`stitchcraft_engine::auto::neighbours`, M13.2):

1. **Background** (`REQ-AUTO-009`). The lowest shape is the picture's background when it is an automatic
   area, other shapes are drawn on it, and its outlines, its holes aside, cover the box around the design to
   within 0.5 mm of each side. The box is the one around the defining points of every shape's path, the
   background's own included, so a lowest shape that covers less of it, a disc in the corner of a square
   say, is part of the picture. The background is left out with `SC-W0906` before anything is digitized.
   With a StitchCraft effect, it sews as that effect says. Finding it has the work budget to itself, and a
   background that needs more is sewn, which `SC-W0906` says.
2. **Specks** (`REQ-AUTO-008`). An automatic area, or a part of one, smaller than 1 mm² is too small to sew:
   it would sew a knot. It is merged into the automatic area it shares the longest edge with, its own
   included: the area around it when it fills a hole, so that no hole shows where it was, or the rest of
   its own area when rounding left a hair between them. On a tie, it goes into the area sewn first. The
   edge 2 shapes share is how much of the area's rings lies within twice the shapes' tolerance of the
   speck. A speck that shares no edge with an automatic area is left out, and so is one whose neighbours
   need more work to find than the budget allows it: put back as drawn, it could overlap a speck merged in
   beside it. `SC-I0905` names each speck, by its area rounded to a hundredth of a square millimetre and
   kept under 1 mm², and where it went. A line filled because it is too wide for a column (M13.1) is an area
   too, so a part of it that small is a speck.
3. **Overlaps** (`REQ-AUTO-007`). Pull draws stitches in from the edges of their shape, so shapes that only
   meet show fabric between them. Each automatic area therefore reaches 0.4 mm under each area, satin
   column or filled line sewn after it, from where it touches that shape, and no further than that shape.
   Shapes touch where their edges meet within twice the shapes' tolerance, so a gap that rounding left
   between edges drawn as one closes too. Along an edge where a shape runs on near the area without
   touching it, the area doesn't reach, and fabric shows between them as drawn. A satin column covers the
   area between its rails as they are sewn, turned to run the same way. Where an area sewn after it covers
   part of it, it is cut away under that area, 0.4 mm inside the area's edge, and that part is sewn once: a
   stacked trace then sews the areas of an abutting one. A part that the areas sewn after it cover whole
   sews nothing, however near their edge it lies. A column or a line sewn after an area goes over it, and
   the area is sewn whole under it. Cutting a fill round small letters adds stitches and can leave gaps
   ([Inkscape forum](https://inkscape.org/forums/embroidery/techniques-for-making-text-on-top-of-a-fill-more-crisp/)).
   `SC-I0904` counts the areas that reach under others and those cut away, and names, in the order they are
   sewn, an area cut away whole, which sews nothing.

Only automatic areas change. A shape with settings is sewn as they say, and a satin column or a line
digitized from a stroke keeps its stitches: whether a column should reach under the fills beside it is
M13.5's, which makes columns from shapes. A fill that bounds no area is left to its fill, which says so.
The areas are taken from the last sewn to the first, so that each meets the shapes after it as they are
sewn. Each area has the work budget to itself for its specks and for its overlaps, and costs the work of the
shapes near it and a unit for each other shape it looks past, not the work of the whole design: what a
shape sewn after it paints is found once, with the work budget to itself. An area whose overlaps need more
keeps the region it has, and a speck that needs more is left out; `SC-I0904` and `SC-I0905` say so. A satin
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

The abutting badge's shapes, sewn as drawn, leave a tenth of their edges bare. Digitized, they overlap, and
98.7 % is covered. The stripes' edges are covered as drawn too, but by the running stitches the fills travel
on along them, under their rows, not by the rows: digitized, the rows themselves overlap. The stacked badge as
drawn sews its lower colours whole under the upper ones, which covers every edge with 217 more stitches,
6 %, sewn twice. Digitized, it sews the abutting badge's areas, within 9 stitches. Without pull, every edge
is covered as drawn and digitized alike, to within a hundredth. `tests/auto_pictures.rs` keeps the numbers
from falling back, and MC-8 tells whether 0.4 mm is the pull on the machine.

## Sewing order

Automatic elements sew in an order StitchCraft chooses, by 4 rules:

1. Shapes that overlap as drawn keep their paint order, the lower one first. Shapes that share an edge
   and nothing more, as abutting paths do, may sew in either order.
2. A column or a line that borders a fill sews after it, and covers its edge.
3. Within those rules, the next element has the thread that is sewing, as long as an element of that
   thread is free to sew. So the plan has few colour changes. Colours that the machine profile maps to one
   thread count as one.
4. Within a colour, the next element is the one nearest the needle, and each element starts and ends at
   its points nearest its neighbours (`REQ-GEN-003`).

The overlaps above follow this order. Elements with settings keep their places in paint order, and
automatic elements move around them within the rules. M10's routing tools, which order satin columns and
running stitches drawn by hand, start from this order.

## Settings

The rules' values become StitchCraft's own parameters, registered as any other
([parameter registry](params.md)), when the Auto effect's dialog lets a user set them (M13.7). The settings
are the widest satin column, the overlap, the smallest area and a switch that leaves the background out.
Their defaults are the values on this page, 7 mm, 0.4 mm and 1 mm², and MC-8 confirms or changes them.
Until then nothing can set them, and the rules use those values (`stitchcraft_engine::auto::SATIN_MAX_WIDTH`
since M13.1, and `OVERLAP` and `SMALLEST_AREA` since M13.2). The narrowest satin column is the design's `min_satin_stroke_width_mm`. With `--auto` the
settings take their defaults.

## Previews in VectorCraft

The Auto effect is a live effect like the plug-ins of each stitch family
([plug-ins](vectorcraft-integration.md#plug-ins)). VectorCraft runs a group's effects on each path inside
it (`REQ-VC-006`), so the effect on a traced group previews every path's stitches, each sewn as its width
says. A live effect is given its path alone. Its preview shows each shape's stitches without the
overlaps, the merged specks or the order, which `stitch export` adds. A preview that costs more than ABI
v1's budget shows less, with `SC-I0810`, as the other effects' previews do.

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

`stitch plan picture.png --auto` would leave VectorCraft out of the flow. Spike M13.9 chooses how it
traces:

- VectorCraft's command line, when it is installed, opens the picture and traces it
  (`vectorcraft-cli run --in picture.png --cmd imageTrace.make …`). This repeats no code.
- StitchCraft reads PNG and JPEG files and traces them itself. This works without VectorCraft.

## Testing

- Each rule gets its requirements and failing cases before its code ([ADR-0008](adr/0008-conformance-first.md)).
- The test pictures are drawn in code, as the test sheets are, and no logo or picture of anyone else's is
  used.
- Each test picture's automatic plan is compared with its plan in paint order: colour changes, jumps,
  trims, stitches, and how much of each shape and of each shared edge the stitches cover
  (`stitchcraft_testkit::metrics`).
- Level B of the [compatibility gate](compatibility-gate.md) gets a case in which VectorCraft's command
  line traces a picture and StitchCraft sews the document it saved, on every track.
- TS-13 sews the test pictures at MC-8 ([machine testing](../plan/machine-testing.md)).

Codes 0900 to 0999 are automatic digitizing's ([diagnostics](diagnostics.md)).
