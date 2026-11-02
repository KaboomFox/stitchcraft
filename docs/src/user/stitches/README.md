# Stitch types

StitchCraft turns each element of a design into stitches of one type. In an SVG file an element is the
stroke or the fill of a shape, and each sews in its own thread colour.

| Stitch type | Sews | Available |
|---|---|---|
| [Running stitch](running.md) | strokes: outlines, details, lettering at small sizes | now |
| [Bean stitch](running.md#bean-stitch-and-repeats) | strokes, each stitch sewn 3 times or more | now |
| [Manual stitch](running.md#manual-stitch) | strokes whose nodes are the needle points | now |
| [Zigzag stroke](zigzag.md) | strokes as wide as their line: light borders, couched looks | now |
| [Satin column](satin-column.md) | wide strokes and borders | now |
| [E-stitch](e-stitch.md) | satin columns as appliqué edges: a spine along one rail, with spikes to the other | now |
| [S-stitch](s-stitch.md) | satin columns sewn one way at a time: a stitch across at each step, joined along the edges | now |
| [Zigzag satin](zigzag-satin.md) | satin columns sewn as a zigzag, every stitch slanting from one edge to the other | now |
| [Tatami fill](tatami-fill.md) | areas, in rows of running stitches, Ink/Stitch's legacy fill among them | now |
| [Contour fill](contour-fill.md) | areas, in rings that follow the outline inward, sewn as one line | now |
| [Meander fill](meander-fill.md) | areas, in one line that wanders along the sides of a tiling | now |

Every element also gets [lock stitches](locks.md) where its stitching starts and ends, and a trim or a
stop after it when it asks for one. A shape without settings can have its stitch type chosen by its width
instead ([automatic digitizing](automatic.md)). An element of a type that is not sewn yet is left out, and
`SC-W0011` names it. The [roadmap](../../plan/roadmap.md) says what each milestone adds.

## Setting parameters

A parameter belongs to one element. Ink/Stitch stores parameters in the SVG file as attributes, such as
`inkstitch:running_stitch_length_mm="1.5"`, and StitchCraft's parameters have the same names and
defaults.

`stitch plan` reads these attributes from each shape of an SVG file, and the VectorCraft plug-in sets them.
A shape with a fill and a stroke gives both the same settings, as in Ink/Stitch, and
`inkstitch:stroke_first` sews the stroke first. A setting of a stitch type StitchCraft doesn't sew yet is
kept, and `SC-W0105` names it. Settings on a group don't change what sews, as in Ink/Stitch, and
`SC-W0802` says so. Ink/Stitch's trim and stop commands are read too.

The pictures on these pages set parameters on the engine directly, to show what each one does.
