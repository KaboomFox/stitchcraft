# From a picture to a machine file

A picture of a few flat colours, such as a badge or a logo, becomes a machine file without a setting on any
of its shapes. VectorCraft traces the picture in your machine's threads, and StitchCraft chooses the stitches
of each traced shape by its width ([image to embroidery](../../design/image-to-embroidery.md)).

## You need

- StitchCraft's `stitch` command ([install](../install.md)).
- VectorCraft, for its Image Trace.
- A picture with a few flat colours, such as a PNG file, that fits your hoop once placed.

## 1. Write your machine's threads

```console
{{#include ../reference/generated/palette.txt}}
```

`stitch palette` writes your machine's threads as a GIMP palette, which VectorCraft loads as a swatch
library. Each colour is a thread's own, named as the machine names it. It writes the reference machine's
threads, the Brother PE800's, unless `--profile` names another machine (`stitch profiles` lists them).

## 2. Trace the picture in those threads

In VectorCraft:

1. Load the threads with **Window › Swatch Libraries › Other Library…**, and open `threads.gpl`.
2. Place the picture with **File › Place…**.
3. With the picture selected, open the **Image Trace** panel. Set **Mode** to Color, **Palette** to Document
   Library and **Library** to the threads. **Colors** is how many colours the picture has, its background
   included. With **Method** set to Abutting, the shapes share their edges. Then press **Trace**.
4. Keep the traced shapes, without the picture, with **Object › Image Trace › Expand**.
5. Save the document, here as `traced-badge.vectorcraft`.

Each traced shape is filled in one of your threads. Leave the shapes without StitchCraft's Fill, Stroke and
Satin effects: StitchCraft chooses the stitches of a shape without one, with `--auto` or with StitchCraft's
Auto effect ([below](#in-vectorcraft-the-auto-effect)).

## 3. Digitize it

```console
{{#include ../reference/generated/plan-traced-badge.txt}}
```

`--auto` digitizes each shape without settings by its width (`SC-I0901`). A line runs, a band as wide as a
satin column sews as satin columns, and a broader area sews as a tatami fill. The white round the badge was
traced lowest, and covers the whole picture. It is the picture's background, the fabric's own colour, and is
left out (`SC-W0906`). Each area reaches 0.4 mm under the shapes sewn after it that it touches
(`SC-I0904`), so that no fabric shows where they meet once the thread pulls in. The example plans
`traced-badge.svg`, a badge drawn as Image Trace traces one, and a document that VectorCraft saved plans the
same way.

<!-- shot: traced-badge -->
![The traced badge digitized automatically. A red disc with a blue square on it and a gold disc in the middle sew as tatami fills, with no white round them.](../../images/generated/traced-badge.png)
<!-- /shot -->

### In VectorCraft, the Auto effect

With the traced group selected, choose **Effect › Plug-ins › StitchCraft Auto**. Its dialog has the
settings of the rules, with the numbers above as their defaults. A smaller **Widest satin column** fills more of the
picture, and a larger **Smallest area** leaves out more specks ([reference](../reference/params/auto.md)).

<!-- shot: vectorcraft-auto-dialog -->
![The StitchCraft Auto dialog in VectorCraft. The settings of automatic digitizing's rules are in 2 sections, Stitches by width and Neighbours, from Widest satin column to Leave out the background, and Preview comes last. Each row has a label, a field with its unit or a checkbox, and a button that opens the setting's help page. Under them are the Preview checkbox, checked, and the Cancel and OK buttons.](../../images/generated/vectorcraft-auto-dialog.png)
<!-- /shot -->

Each traced shape then shows the stitches StitchCraft chooses for it, as far as VectorCraft lets a live
effect work them out: a fill's top rows, and a column's or a line's stitches. A shape that needs more,
such as a letter or a ring, shows its outline instead, and the machine file has every stitch whichever
shows. The document now says it is digitized automatically, so **File › Export › Export As** writes the
machine file without **Digitize automatically**, as does `stitch export` without `--auto`
([how](../how-to/embroider-a-vectorcraft-design.md)). Trace again and the group is new, without the
effect: apply it after the last trace.

## 4. Sew it

Load `traced-badge.pes` and sew it as in [your first sew-out](first-sew-out.md#3-load-it-on-the-machine).
Thread each colour as the machine names it, when it stops for it.

## Next

- [Image to embroidery](../../design/image-to-embroidery.md) says how each shape's stitches are chosen, and
  what is done where shapes meet.
- [Fit a design to the hoop](../how-to/fit-the-hoop.md), when the traced picture is larger than your hoop.
