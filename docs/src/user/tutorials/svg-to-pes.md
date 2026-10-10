# Turn an SVG into a PES file

In this tutorial you save a small design as SVG and plan it for a Brother machine. Then you look at what
it will sew, and sew it. The design is a red star inside a blue ring, 44 mm across.

## You need

- StitchCraft, [installed](../install.md).
- A text editor, or a drawing program that saves SVG, such as Inkscape.
- To sew it: the machine and materials of [your first sew-out](first-sew-out.md#you-need).

## 1. Save the design

Save this text as `badge.svg`:

```xml
{{#include ../../../fixtures/badge.svg}}
```

StitchCraft sews each shape's stroke as a line of running stitch, in the stroke's colour. It reads the
design's size from `width`, `height` and `viewBox`, and here one unit is 1 mm. Shapes are sewn in the
order the file lists them, and the star comes first.

## 2. Plan it

```console
{{#include ../reference/generated/plan-badge.txt}}
```

`stitch plan` writes the machine file and, with `--preview`, a picture of it. It plans for the
`brother-pe800-5x7` profile, the PE800 with its 5 × 7 in hoop, unless `--profile` names another. Each colour is a thread, and the
PES file shows it as the nearest Brother palette colour. The machine stops between the red and the blue
for you to change the thread.

## 3. Look at it

The preview shows the embroidery as it will sew:

<!-- shot: badge -->
![A red star inside a blue ring, as the badge will sew](../../images/generated/badge.png)
<!-- /shot -->

The simple style, from `stitch preview badge.pes -o badge-simple.png --style simple`, shows each needle
point:

<!-- shot: badge-simple -->
![The badge in the simple style: the needle points of the star and the ring, and grey dashes where the frame moves from the star to the ring after the colour change](../../images/generated/badge-simple.png)
<!-- /shot -->

The stitches are spread evenly along each straight edge, and every corner of the star has a needle point.
The grey dashes are the frame's move from the star to the ring, after the colour change.

## 4. Sew it

Load `badge.pes` and sew it as in [your first sew-out](first-sew-out.md#3-load-it-on-the-machine). Thread
red first, and blue when the machine stops.

## When StitchCraft warns

StitchCraft says what it changes or leaves out of a design. This one from StitchCraft's tests has a
filled rectangle and 2 lines that nearly touch:

```console
{{#include ../reference/generated/plan-strokes.txt}}
```

- `SC-W0011` names the fill. Fills are sewn from milestone M5, and until then the rest of the design is
  sewn without it.
- `SC-I0504` says a needle point too close to the one before was left out, where the lines nearly touch.

`stitch explain` with a code tells you more, and [diagnostic codes](../reference/diagnostics.md) lists
them all. `--report` writes the summary and the messages as JSON, for scripts.

## Next

[Stitch types](../stitches/README.md) shows what each stitch type does and the parameters that change it.
