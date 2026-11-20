# Expand an automatic design

StitchCraft digitizes a traced picture automatically. It chooses each shape's stitches again whenever the
picture, a mark, a guide or the Auto effect's settings change. Marks, guides and
names steer it. When none of them gives you what you want, expand the design. It becomes ordinary shapes,
each with its own StitchCraft effect, which you edit one by one, as other embroidery software leaves a
design it digitized.

## When to expand

Expand once the automatic design is close, and what is left is a change to a part, such as another angle
for a fill or a part you don't want. Steer the design first with
[marks](keep-small-parts.md), [guides](guide-the-rows.md) and the Auto effect's settings, since those stay
live and an expanded design doesn't follow them.

## Expand a design

```sh
stitch expand dog.vectorcraft -o dog-expanded.vectorcraft
```

The design is read as `stitch plan --auto` reads it, with the same `--set`, `--style`, `--text`, `--keep`
and `--guide`. A picture, a PNG or JPEG file, is traced first, as `stitch plan` traces it. To get an SVG
file for Inkscape and Ink/Stitch, name the output `.svg`.

Open the document in VectorCraft. At the top of its layer is a new group, Expanded embroidery, with a
path for each part, in the order they sew, each named as the notices name its shape:
`Red fill (3)`, `Deep Gold column (1)`. A piece of the base layer is named after its thread, such as
`Red base layer (1)`.

## What you get

- Each part has the effect and the settings it sewed with: the fill's angle, row spacing and underlay, a
  column's width and pull compensation, a line's repeats.
- Each part's `digitized_as` says it sews as automatic digitizing made it, and the design keeps the limits
  of how thick it sews. So the needle sews under later shapes between parts in place of a jump, and the
  cap thins the design where it sews too thick. Planned, the expanded design sews as the automatic one
  did, stitch for stitch.

Set a part's `digitized_as` to `no` to sew it as a shape you set by hand: the cap then leaves it alone,
and the thread jumps to it and from it.

## What changes

The expanded design no longer updates itself. Changing the picture, a mark, a guide or the Auto effect
changes nothing in it, and a part you edit sews as its own settings say. Editing one part can change the
part sewn after it, which starts nearest where it ends. The cap can thin other parts elsewhere, where the
edit changes how thick the design sews.

Lettering becomes the columns it sews, and you can't edit its text as text. `SC-W0922` names it, and any
shape that needed more work than the budget allows.

## Go back

The traced shapes are still in the document, hidden and locked, under the expanded group. To go back,
delete the expanded group, then show and unlock the traced group in the Layers panel. To compare, hide one
and show the other.
