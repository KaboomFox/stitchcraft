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

ABI v2's exporter plug-ins ([P1](rfc-vectorcraft-abi-v2.md#p1-exporter-plug-ins)) would move the second
step into VectorCraft, as File › Export › PES.

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
  than its limit is cut square. Those pieces, joined into one area, are the fill's outline. A part of it too small to sew is
  named as a fill's is (`SC-W0303`). An area that would reach more than 10 m from the origin is left out
  (`SC-W0903`). No design lies there.

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

- **Overlaps.** Each shape extends 0.4 mm under the shapes sewn after it that it touches. Pull draws
  stitches in from the edges of their shape. With the overlap, the earlier shape's stitches reach under
  the later shape's edge, and no fabric shows between them.
- **Parts covered by a later fill.** Where a fill sewn later covers part of an earlier shape, the earlier shape is
  cut away under it, 0.4 mm inside its edge, and that area is sewn once. A trace that stacks its colours,
  as Inkscape's Trace Bitmap does with its Stack option and VectorCraft's Image Trace with its Overlapping
  method, then sews no more than an abutting one. A column or a line sewn later goes over the earlier
  shape, which is sewn whole under it. Cutting a fill round small letters adds stitches and can leave gaps
  ([Inkscape forum](https://inkscape.org/forums/embroidery/techniques-for-making-text-on-top-of-a-fill-more-crisp/)).
- **Specks.** A shape or a part of less than 1 mm² is too small to sew. It is merged into the shape around
  it, or into the shape it shares the longest edge with, so that no hole shows where it was. A speck with
  no neighbour is left out. A diagnostic counts the specks and names each element.
- **Background.** The lowest shape, when it covers the design's whole bounds within 0.5 mm on each side, is
  the picture's background. It is left out with a warning. With a StitchCraft effect, it sews as that effect
  says.

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
since M13.1). The narrowest satin column is the design's `min_satin_stroke_width_mm`. With `--auto` the
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

## With ABI v2

- **P1, exporter plug-ins.** File › Export › PES plans the document inside VectorCraft, objects without
  effects digitized automatically. The flow is then trace and export, both in VectorCraft.
- **P8, effects in filter objects.** A filter could set on each path the effect of the family its width
  chose. The user then adjusts it in that effect's dialog.
- **P4, budget classes.** The Auto effect's previews of large shapes get the heavy budget.

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
