# Satin columns and presets in VectorCraft

The StitchCraft plug-ins add their effects to VectorCraft's Effect › Plug-ins menu, and VectorCraft's own
commands do the rest of what a design needs.

## Make a satin column VectorCraft can show

A satin column drawn as one path sews as wide as its stroke. VectorCraft gives an effect the path but not
its stroke, so StitchCraft Satin leaves such a path as it is and says why (`SC-I0810`). The file still
sews it. To see its stitches in VectorCraft, draw the column with 2 rails:

1. Draw the column's 2 edges as 2 paths.
2. Select both and choose **Object › Compound Path › Make** (Cmd+8).
3. With the compound path selected, choose **Effect › Plug-ins › StitchCraft Satin**.

VectorCraft reverses the front path when both run the same way. The column turns it back
(`reverse_rails`, automatic), so the direction the rails were drawn in doesn't matter. A compound path
takes the appearance of the path at the back, so a StitchCraft Satin effect already on that path stays.

## Add a rung

Rungs set the angle of the stitches along the column. With the Pen tool, add a line across both rails
where the stitches should run. Select it with the column, and choose **Object › Compound Path › Make**. A
line drawn after the column lies in front of it, and the compound path takes the appearance of the column
behind, its effect included.

## Keep settings as a preset

A graphic style keeps an appearance, StitchCraft's effects included:

1. Select an object whose StitchCraft effect is set as you like it.
2. Choose **Window › Graphic Styles › New Graphic Style**.
3. Select other objects and click the style in the Graphic Styles panel.

Each object gets a copy of the effect, with the style's parameters, and sews with them. A style applied to
a group or a layer puts its effects on the group: they reach every path inside it, and each sews in its
own colour. Keep StitchCraft's effects at one level, though. A path that has its own StitchCraft effect
inside such a group, or sits inside 2 such groups, is drawn as one effect run on the stitches of the other.
It sews as the nearest effect says, and the command line warns about it (`SC-W0807`). Alt-clicking a style
adds its effects to the object's own, and an object with 2 fill effects sews twice.

## Turn a preview into paths

**Object › Expand Appearance** turns what an effect shows into ordinary objects, with strokes outlined.
Those objects are the shapes the preview draws, and they sew differently from the effect. Expand a copy for
a picture of the stitches, and keep the original to sew.

## Not in VectorCraft yet

- Making a line into a satin column, as Ink/Stitch's Convert Line to Satin does, needs more work than
  VectorCraft lets a plug-in do. Draw the 2 rails instead.
- Test sheets come from the command line: [`stitch testsheet`](../reference/test-sheets.md).
