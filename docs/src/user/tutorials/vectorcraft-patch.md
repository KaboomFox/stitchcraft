# A patch in VectorCraft

In this tutorial you draw a round patch in VectorCraft and choose how each part of it sews, with the
StitchCraft plug-ins showing the stitches as you go. Then you write the machine file and sew it. The patch
is 30 mm across. It has a blue disc of fill, with a white star of running stitch on it, inside a gold
satin border.

## You need

- VectorCraft, in a version the [compatibility page](../reference/compatibility.md) lists, with the
  StitchCraft plug-ins [installed](../install.md#the-vectorcraft-plug-ins).
- StitchCraft's command line, [installed](../install.md).
- To sew it: the machine and materials of [your first sew-out](first-sew-out.md#you-need).

Sizes here are in points, VectorCraft's own unit: 72 points make an inch, and about 2.8 a millimetre. A
field reads a size typed with its unit, such as `80 pt`, whichever unit it shows.

## 1. Draw the disc and fill it

Choose **File › New**, and make the document 108 pt wide and 108 pt high. Choose the Ellipse tool and
click near the artboard's top left corner. In the Ellipse dialog, make the circle 80 pt wide and 80 pt
high. Give it a blue fill and no stroke. A fill sews in the colour of its object's fill.

With the circle selected, choose **Effect › Plug-ins › StitchCraft Fill**:

<!-- shot: vectorcraft-plugins-menu -->
![VectorCraft's Effect menu, open at Plug-ins, which lists StitchCraft Fill, StitchCraft Satin and StitchCraft Stroke.](../../images/generated/vectorcraft-plugins-menu.png)
<!-- /shot -->

The dialog holds Ink/Stitch's [settings for a fill](../reference/params/fill.md) in sections, each with
its label and unit. Rest the pointer on a label for its help, and the button after a field opens its page.
Set **Angle** to 45, and choose **OK**. If the dialog is taller than your screen, see
[below](#dialogs-taller-than-the-screen).

<details>
<summary>The whole StitchCraft Fill dialog</summary>

<!-- shot: vectorcraft-fill-dialog -->
![The StitchCraft Fill dialog in VectorCraft, its settings in sections, from Fill, with Sew the fill and Fill method, to Longest stitch and Preview. Each row has a label, a field with its unit, and a button that opens the setting's help page. Under them are the Preview checkbox, checked, and the Cancel and OK buttons.](../../images/generated/vectorcraft-fill-dialog.png)
<!-- /shot -->

</details>

The circle now shows the rows the fill sews, at 45 degrees. VectorCraft lets a plug-in do only so much
work for a preview, and a larger fill, such as a disc more than about 30 mm across, shows its outline
instead of its rows. `stitch export --preview` shows every stitch, whatever the size.

## 2. Draw the star

Choose the Star tool and click in the middle of the disc. Make **Radius 1** 26 pt, **Radius 2** 11 pt and
**Points** 5. Give the star no fill and a white stroke. A line sews in the colour of its object's stroke.
Choose **Effect › Plug-ins › StitchCraft Stroke**, and **OK**. Its settings sew the line in running
stitch, 2.5 mm a stitch.

<details>
<summary>The whole StitchCraft Stroke dialog</summary>

<!-- shot: vectorcraft-stroke-dialog -->
![The StitchCraft Stroke dialog in VectorCraft, its settings in sections, from Stroke, with Method, to Shortest stitch and jump and Preview. Each row has a label, a field with its unit, and a button that opens the setting's help page. Under them are the Preview checkbox, checked, and the Cancel and OK buttons.](../../images/generated/vectorcraft-stroke-dialog.png)
<!-- /shot -->

</details>

## 3. Draw the border

A satin column is drawn as its 2 edges, its rails, joined into one compound path ([satin columns in
VectorCraft](../how-to/vectorcraft-columns-and-presets.md)). Lock the disc and the star first, and Select
All then takes only the rails: choose **Select › All**, then **Object › Lock › Selection**.

With the Ellipse tool, draw 2 more circles, one 84 pt across and one 70 pt across. Choose **Select › All**,
then **Object › Align › Horizontal Align Center** and **Object › Align › Vertical Align Center**. Join the
circles with **Object › Compound Path › Make**. Give the compound path no fill and a gold stroke, and
choose **Effect › Plug-ins › StitchCraft Satin**, then **OK**.

<details>
<summary>The whole StitchCraft Satin dialog</summary>

<!-- shot: vectorcraft-satin-dialog -->
![The StitchCraft Satin dialog in VectorCraft, its settings in sections, from Satin column, with Method and Zigzag spacing, through the 3 underlays to Longest stitch and Preview. Each row has a label, a field with its unit, and a button that opens the setting's help page. Under them are the Preview checkbox, checked, and the Cancel and OK buttons.](../../images/generated/vectorcraft-satin-dialog.png)
<!-- /shot -->

</details>

## 4. Centre it

Choose **Object › Unlock All** and **Select › All**. Align everything again with **Object › Align ›
Horizontal Align Center**, then **Vertical Align Center**. The patch is done, and VectorCraft shows the
stitches the plug-ins preview:

<!-- shot: vectorcraft-patch -->
![The patch in VectorCraft's canvas, as the plug-ins' previews show it. Rows at 45 degrees fill the blue disc, and a white star of running stitch lies on it, inside a gold satin border.](../../images/generated/vectorcraft-patch.png)
<!-- /shot -->

The objects sew in the order they were drawn. The disc sews first and the star next, and the border sews
last, over the edges of both.

## 5. Write the machine file

Save the document as `round-patch.vectorcraft`, and run `stitch export` on it:

```console
{{#include ../reference/generated/export-round-patch.txt}}
```

`round-patch.png` shows what the machine will sew:

<!-- shot: vectorcraft-patch-sewn -->
![The VectorCraft tutorial's patch as it will sew. The blue fill's rows run at 45 degrees, and a white star of running stitch lies on it, inside a gold satin border.](../../images/generated/vectorcraft-patch-sewn.png)
<!-- /shot -->

## 6. Sew it

Load `round-patch.pes` and sew it as in [your first sew-out](first-sew-out.md#3-load-it-on-the-machine).
Thread blue first, white when the machine stops, and gold when it stops again.

## Dialogs taller than the screen

From VectorCraft 0.14 the rows of a dialog taller than the window scroll, and a section's heading folds
its rows away. The satin's dialog is taller than most screens.

VectorCraft 0.12 and 0.13 don't scroll a plug-in's dialog, and list every setting under its Ink/Stitch
name. The fill's and the satin's dialogs are taller than most screens there, and the stroke's is taller
than the window VectorCraft opens with. Their first rows, or the OK button, can be out of sight. Make
VectorCraft's window as tall as the screen allows, and click a field you can see. Tab and Shift+Tab move to
the others, and Enter applies the dialog.

## Next

[Embroider a VectorCraft design](../how-to/embroider-a-vectorcraft-design.md) shows how to leave objects
out and mark where a shape starts and ends. It also explains what `stitch export` says.
