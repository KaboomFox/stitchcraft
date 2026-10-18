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

Apply the effect after joining the rails. VectorCraft 0.10.0 and later keep the appearance of the path at
the back, and earlier versions that of the path in front, which may not be the one with the effect. From
0.10.0, VectorCraft may also reverse the front path, and always does for 2 straight rails. The column
turns it back (`reverse_rails`, automatic). A reversed ring also starts somewhere else, so of 2 rings the
second starts at its node nearest the first's start. The stitches are the same whatever node the rails
were drawn from and whichever way they run.

## Add a rung

Rungs set the angle of the stitches along the column. With the Pen tool, add a line across both rails
where the stitches should run. Select it with the column, and choose **Object › Compound Path › Make**.
From VectorCraft 0.10.0 the column, behind the line, keeps its effect. Before 0.10.0 the compound path takes
the line's appearance: apply StitchCraft Satin to it again.

## Keep settings as a preset

A graphic style keeps an appearance, StitchCraft's effects included:

1. Select an object whose StitchCraft effect is set as you like it.
2. Choose **Window › Graphic Styles › New Graphic Style**.
3. Select other objects and click the style in the Graphic Styles panel.

Each object gets a copy of the effect, with the style's parameters, and sews with them. A style applied
to a group or a layer puts its effects on the group, and every path inside it sews with them. The style's
fills and strokes go on the group too, and VectorCraft paints them over the group's contents. Each path
inside sews in the colour on top, the style's fill for a fill effect and the style's stroke for a satin
column, which follows its width. Where the style has no paint of that kind, or lets the path show through
it, the path's own paint of that kind gives the colour, and with none on either, the topmost paint of the
other kind does. A path without paint under a style without any is invisible and isn't sewn. To show the
paths' own colours over the style's, drag the Contents row above the style's fills and strokes in the
Appearance panel. Keep StitchCraft's effects at one level, though. A path that has its own StitchCraft
effect inside such a group, or sits inside 2 such groups, is drawn as one effect run on the stitches of
the other. It sews as the nearest effect says, and the command line warns about it (`SC-W0807`).
Alt-clicking a style adds its effects to the object's own, and an object with 2 fill effects sews twice.

## Turn a preview into paths

**Object › Expand Appearance** turns what an effect shows into ordinary objects, with strokes outlined.
Those objects are the shapes the preview draws, and they sew differently from the effect. Expand a copy
for a picture of the stitches, and keep the original to sew.

## Not in VectorCraft yet

- Making a line into a satin column, as Ink/Stitch's Convert Line to Satin does, needs more work than
  VectorCraft lets a plug-in do. Draw the 2 rails instead.
- Test sheets come from the command line: [`stitch testsheet`](../reference/test-sheets.md).
