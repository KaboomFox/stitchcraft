# Image to embroidery

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

Exporter plug-ins ([P1](rfc-vectorcraft-abi-v2.md#p1-exporter-plug-ins), in VectorCraft from 0.14.0)
would move the second step into VectorCraft, as File › Export › PES, once plug-in v2 uses them.

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

An element with settings sews as they say, whether or not the switch is given. So a picture can be
digitized automatically and one of its shapes then set by hand. A shape's fill gives its area and its
stroke gives its line, each with its thread, width and join, as for any element.

## Stitch types by width

An automatic element is measured at the size it sews, in millimetres. Its width at a place is the
diameter of the largest circle that fits inside it there. Its parts sew by their width:

| Width | Sewn as |
|---|---|
| Below the design's `min_satin_stroke_width_mm`, 1 mm unless the design sets it | A running stitch along the centre line, a triple run (bean stitch) from 0.5 mm wide |
| From that width to the widest satin column, 7 mm | Satin columns, whose rails are the outline |
| Above 7 mm | A tatami fill |

A stroke is a line as wide as it is drawn. Narrower than 1 mm it sews as a running stitch. Up to 7 mm it
sews as a satin column as wide as its stroke, as a satin column drawn as one path does (`REQ-SAT-016`).
Wider than that, the area it paints is filled.

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

The rules' values are StitchCraft's own parameters, registered as any other
([parameter registry](params.md)). The settings are the widest satin column, the overlap, the smallest
area and a switch that leaves the background out. Their defaults are the values on this page, 7 mm, 0.4 mm and 1 mm², and MC-8
confirms or changes them. The narrowest satin column is the design's `min_satin_stroke_width_mm`. The Auto
effect's dialog shows the settings, and with `--auto` they take their defaults.

## Previews in VectorCraft

The Auto effect is a live effect like the plug-ins of each stitch family
([plug-ins](vectorcraft-integration.md#plug-ins)). VectorCraft runs a group's effects on each path inside
it (`REQ-VC-006`), so the effect on a traced group previews every path's stitches, each sewn as its width
says. Each path gets a run of its own, with a live effect's whole budget. A live effect is given its path
alone. Its preview shows each shape's stitches without the
overlaps, the merged specks or the order, which `stitch export` adds. A preview that costs more than ABI
v1's budget shows less, with `SC-I0810`, as the other effects' previews do.

Tracing again replaces the group with a new one, without the effect. Apply the effect after the last
trace, or use `--auto`, which works without it.

## With ABI v2

Plug-in v2 (M9) uses what VectorCraft has merged of the RFC ([outcome](rfc-vectorcraft-abi-v2.md#outcome)).

- **P1, exporter plug-ins.** File › Export › PES plans the document inside VectorCraft, objects without
  effects digitized automatically. The flow is then trace and export, both in VectorCraft.
- **Apply Auto, a heavy filter (P4 and P8).** An object filter digitizes the selected traced shapes once
  and writes the result into the document. Each part becomes an object with its stitch family's effect and
  the settings the rules chose. A broad part is a path with the fill effect, a narrow part a compound path
  of its rails and rungs with the satin effect, and a line its centre line with the stroke effect. Their
  paths include the overlaps, and leave out what later fills cover. Specks are merged into their
  neighbours, and the background is left out. The filter returns the parts as new objects in sewing order and leaves the traced shapes out of
  its output, which replaces them in one undo step. Each part then previews with its own effect, the user
  adjusts it in that effect's dialog, and `stitch export` sews the parts as they are, without `--auto`.
- **Budgets.** Digitizing a whole picture takes more than a live effect gets. A filter may ask for the
  heavy budget, 1 G instructions and 300 s a run, and a live effect may not, as it runs whenever the
  document is drawn. So the Auto effect is the quick preview, within a live effect's budget, and Apply
  Auto's budget is measured when it is built, as the previews' were (M6.3).
- **P3, overlays (under review).** The Auto effect could draw its stitches over the traced shapes, whose
  geometry is then unchanged. Writing them costs less than writing geometry. The engine's work limits a
  preview more ([previews](vectorcraft-integration.md#live-effect-previews-within-v1-budgets)), so a large
  shape still shows its rows or an outline.

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
