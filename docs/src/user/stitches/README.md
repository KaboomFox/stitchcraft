# Stitch types

StitchCraft turns each element of a design into stitches of one type. In an SVG file an element is the
stroke or the fill of a shape, and each sews in its own thread colour.

| Stitch type | Sews | Available |
|---|---|---|
| [Running stitch](running.md) | strokes: outlines, details, lettering at small sizes | now |
| [Bean stitch](running.md#bean-stitch-and-repeats) | strokes, each stitch sewn 3 times or more | now |
| [Manual stitch](running.md#manual-stitch) | strokes whose nodes are the needle points | now |
| Satin column | wide strokes and borders | milestone M4 |
| Fill | areas | milestone M5 |

Every element also gets [lock stitches](locks.md) where its stitching starts and ends, and a trim or a
stop after it when it asks for one. An element of a type that is not sewn yet is left out, and `SC-W0011`
names it. The [roadmap](../../plan/roadmap.md) says what each milestone adds.

## Setting parameters

A parameter belongs to one element. Ink/Stitch stores parameters in the SVG file as attributes, such as
`inkstitch:running_stitch_length_mm="1.5"`, and StitchCraft's parameters have the same names and
defaults.

`stitch plan` does not read these attributes yet, and every element sews with the defaults. `SC-W0802`
says so when a file has them. The SVG reader reads them from milestone M8, and the VectorCraft plug-in
sets them from M6. Ink/Stitch's trim and stop commands are read today.

The pictures on these pages set parameters on the engine directly, to show what each one does.
