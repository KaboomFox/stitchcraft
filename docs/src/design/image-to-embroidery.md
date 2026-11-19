# Image to embroidery

<!-- implements: crates/stitchcraft-engine/src/auto/**, crates/stitchcraft-engine/src/moves.rs, crates/stitchcraft-engine/src/moves/**, crates/stitchcraft-plan/src/thickness/**, apps/stitchcraft-cli/src/commands/palette.rs, apps/stitchcraft-cli/src/trace.rs, apps/stitchcraft-cutout/src/**, apps/stitchcraft-cli/src/subject.rs, apps/stitchcraft-cli/src/subject/**, apps/stitchcraft-cli/src/commands/cutout.rs, crates/stitchcraft-svg/src/lettering/pictures.rs, crates/stitchcraft-svg/src/lettering/pictures/**, apps/stitchcraft-cli/src/text.rs -->

Automatic digitizing makes a design that sews well from a picture of flat colours, such as a logo, clip
art or a drawing, once it is traced into shapes. Settings on the shapes are optional. StitchCraft chooses
the stitches of each shape by its width at the size it sews. A colour drawn under others sews what shows of
it, and neighbouring shapes overlap where they meet. Specks and the background are left out, and the
sewing is ordered for few colour changes and short jumps. Milestone M13 of the [roadmap](../plan/roadmap.md)
builds it, after M7.5. Photographs sewn in blended colours need another technique, outside the roadmap.

## The flow

A picture becomes a machine file in 2 steps:

1. In VectorCraft, place the picture, scale it to the size it will sew, and trace it with Object › Image
   Trace › Make.
2. Run `stitch export picture.vectorcraft --auto -o picture.pes --preview picture.png`.

With VectorCraft installed, `stitch export picture.png -o picture.pes` takes both steps itself
([below](#pictures-from-the-command-line)).

An Image Trace object is a group of the traced paths, with the source image hidden beside them
(`imageTrace.make` in VectorCraft's `crates/engine`, `main` at `0842941`). `stitch export` already reads
those paths, and Expand is optional. Without `--auto` it sews each of them as Ink/Stitch sews an SVG
shape: every area as a tatami fill and every line as a running stitch, in paint order. In a test document
of a traced square and a strip 2.1 mm wide beside it, both sewed as tatami fills. With `--auto`, each
shape sews as this page describes. Once MC-8 confirms the rules on the machine, M13.8 makes automatic
digitizing the default for VectorCraft documents, and `--auto` is then needed for SVG files only.

VectorCraft's exporter plug-ins ([P1](rfc-vectorcraft-abi-v2.md#p1-exporter-plug-ins), 0.14) move the second
step into VectorCraft, as File › Export › Export As › PES, which StitchCraft's formats are exporters for since
M6.9. Their `auto` option is `--auto`. Since M13.7, StitchCraft's Auto effect on the traced group says it in
the document itself, with the rules' settings in its dialog, and previews each path's stitches: the
document then sews digitized automatically from either step, without the switch.

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
- each path with the StitchCraft Auto effect, on the path itself or on a group or a layer around it, with
  or without `--auto` (M13.7, `REQ-VC-016`). A path nearer to a StitchCraft stitch effect, its own or a
  group's, sews as that says. The effect's parameters are the shape's settings of the rules
  ([below](#settings)). A stitch effect says how an object sews, so an Auto effect beside one isn't used,
  and `SC-W0807` says so;
- each SVG element without `inkstitch:` settings, with `--auto`.

An element with settings is not digitized automatically, whether or not the switch is given: it sews as
its settings say, as far as its reader reads them. So a picture can be digitized automatically and one of
its shapes then set by hand. A shape's fill gives its area and its stroke gives its line, each with its
thread, width, join and cap, as for any element. A trim or a stop that an Ink/Stitch command asks for is
not a setting of the stitches, and an automatic element keeps it. An element sewn as several elements
keeps it for the last of them. Start and end positions, from Ink/Stitch's commands or VectorCraft's
markers, and a target point, are not kept: automatic digitizing places where its parts start and end, and
the readers say the commands and markers aren't used (`REQ-GEN-001`). The elements it makes have none. An
automatic element's clip paths are applied before it is digitized, where an ordinary element's are
applied as it is generated: the shape digitized is what its clips leave of it, and its columns follow
what shows (`REQ-GEN-005`, `auto/clips.rs`). A line's cut end takes the line's cap. A shape wholly
outside its clips is left out (`SC-W0013`).

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

## Which stitch a shape gets

Automatic digitizing chooses the stitches of each part of a shape from the part's geometry alone, by the
first rule of one table that the part matches (M13.21, `REQ-AUTO-030`). The picture's subject
is no input of a rule, and a flower and a dog go by the same table. A part is a connected piece of what
shows of an area, once the layers are flattened and the specks are merged ([Neighbours](#neighbours)).

Width alone chose the stitch before. A blob as wide as a satin column then sewed as a column bent into its
corners, and a ragged patch of a traced photo as several columns that fan where they meet. On the traced
running dog, 62 of 107 shapes sewed so.

### The measures

- **Widest.** The diameter of the largest circle that fits inside the part. The rules use one fact about it, whether a
  circle as wide as a bound fits.
- **Width.** The diameter of the circles that fit inside the part centred on its centre lines, averaged
  along them ([width along the line](algorithms/centre-lines.md#width-along-the-line)).
- **Slenderness.** The part's area divided by its width squared, which says how long it is for its width. A
  rectangle 3 by 9 mm measures 3, a disc 0.8, and a letter drawn with an even stroke 6 or more.
- **Unevenness.** How much the width varies along the lines: its standard deviation divided by the width.
  The letters of a drawn patch measure 0.11 to 0.23, and the ragged patches of 4 traced photos 0.39 to
  0.52 at the median.
- **Lines.** How many centre lines the part has. A part of one line has no branch.
- **Reach.** How far the part lies from one of its centre lines on both sides of the line, in radii of the
  widest circle that fits inside it centred on its lines. A column has 2 rails, the part's outline on either
  side of its line. Each rail is measured by its farthest corner beside the line, and a column by the
  nearer of its rails. A part measures what its farthest column does. However its width tapers, a column
  measures 1, and a star measures 1.6 to 2.6.

### The rules

The [rules' table](../user/reference/auto-rules.md) is generated from the code's own
(`stitchcraft_engine::auto::rules::RULES`), with each bound's default, so that the table, the notice a
shape gives and the tests at each bound can't drift apart. The rules are tried in this order. First a shape whose
setting names a stitch sews so. Then a speck merges into a neighbour and a line runs. A ragged thin line has
merged into its neighbour before the rules are tried ([Neighbours](#neighbours)). Areas are filled next,
with compact parts and uneven parts after them, and then a part with arms. Any other part sews as satin
columns, unless its columns are refused.
One function compares every bound (`auto::rules::choose`), a length to the micrometre and a ratio to its
third decimal. A width written in another unit that converts to a bound give or take a rounding error,
such as 7 mm in CSS pixels, is that bound.

**A compact part is filled** (`REQ-AUTO-031`) because a blob has no way to run. Its centre line is what its
corners leave of its middle, so a column along that line sews stitches of unlike lengths round a point. A
fill sews rows across the blob at the design's angle. On a part under 7 mm wide a row is a stitch or 2.
The fill then looks like a satin column without the fan. A dot is the smallest compact part.

**An uneven part is filled** (`REQ-AUTO-032`) for what its columns would do: a part of several lines sews a column along each, and
the columns meet at its junctions. Where the strokes are even, as a letter's are, the columns meet cleanly.
Where the width swells and shrinks, each column fans at its ends and the outline sews scalloped. A part of
one line isn't held to its evenness: a leaf and a tapering stroke narrow smoothly, and one satin column
sews each well. A piece of an uneven part that is narrower than the narrowest satin stroke leaves the
fill, cut as a narrow part of a broad one is ([Shapes in parts](#stitch-types-by-width)), and sews as a line.
The measure is of the whole part, so a letter of a thick and a thin stroke is uneven too, though each
stroke is even. A design drawn in such strokes turns the rule off, which the `logo` style does.

**A part with arms is filled** (`REQ-AUTO-050`, M13.27). A star of 4 points 5 mm across has 2 axes, and
its centre lines keep one of them. The points of the other axis are short for the star's middle, so they
are left out of the lines as bumps, as the corner of a sharp turn is
([centre lines](algorithms/centre-lines.md#strips-junctions-and-ends)). Such a star counted
one line, 0.97 mm wide and 7.5 times as slender, which is neither compact nor uneven. It sewed as one satin
column from point to point, with its 2 other points as bumps on the column's sides. This measure tells
such a part from a column. The points left out lie on both rails, far from the line. A column's rails lie
no farther from its line than the radius of its widest circle. A part that measures more than 1.4 is
filled. The corner of a sharp turn lies on one rail only. A letter V measures under 1, and keeps its
columns. The rule comes after the uneven one, so a part that was filled before is filled by the same rule as before,
and only columns moved.

2 other measures were tried first. Counting the points left out as centre lines makes a star uneven, but a
letter V 4 mm tall varies by 0.29 of its width, 0.01 under that rule's bound. Measured on one side alone,
or as the longest stitch for the line's length, a sharp corner counts as a star's point does. The
rule has 2 limits. A star of 3 points measures 0.87, since its third point lies on one rail as a corner
does, and stays a column. A star nowhere wider than the narrowest satin stroke is a line before its
columns are looked for.

**Holes** (`REQ-AUTO-033`). A part that is filled is filled whole under the areas that fill its holes, and
they sew on it ([Layers](#neighbours)). A dog's nose sews as one fill with the nostrils on it, where it sewed
as 4 columns round them. A hole that nothing fills is left open, and a part sewn as columns keeps its holes,
as a ring or a letter's counter does.

**What leaves a fill** (`REQ-AUTO-034`). A piece of an area leaves the fill only where it is narrower than
the narrowest piece of a fill, and only when the rules make it a line or a column. A balloon's string
leaves the balloon, and a wing 6 mm wide stays in the fill.

**Refused columns** (`REQ-AUTO-035`). A satin stitch longer than the widest satin column lies loose, and a
column no wider than that on average still sews one where its rails part, at a corner or round a bend. A
column's longest stitch lies between the 2 points as far along its rails, as shares of their lengths from
one rung to the next, at a corner of either rail. A part with a column that would sew a longer one is
filled, and `SC-I0902` gives the stitch's length. The widest satin column is the longest satin stitch a
machine sews well, so a shape may set it lower and never higher. It is the same for every machine profile
so far, and becomes a value of the profile with the step that splits the stitches of a wider column.

### Seeing and changing the choice

`SC-I0912` says, for each part of a shape, which rule it sewed by and the measures that decided it
(`REQ-AUTO-036`). The settings come in 3 tiers ([Settings](#settings)):

- **A shape** names its stitch with `auto_stitch`. A part that can't sew so goes by the table.
- **A design** leans the rules towards satin columns or fills with `auto_prefer`, which moves 3 bounds
  together. `auto_fill_uneven` turns the filling of uneven parts off, and `auto_fill_angle` gives every
  fill one angle.
- **The bounds themselves** are advanced settings. No dialog shows them, and `stitch plan --set` sets one
  on every shape digitized automatically, unless the shape sets it.

A style is a named set of values of these settings, and no rule of its own: `stitch plan --style logo` for
flat art drawn in strokes, and `--style photo` for a traced photo, which is the defaults.

### The grounds of the bounds

The narrowest and the widest satin column are established: digitizing manuals put the narrowest satin at 1
to 1.5 mm, and split a satin stitch at 7 mm to keep its look. The least slenderness, the most a width
varies and the narrowest piece of a fill are StitchCraft's own. Their values come from measurements of 9
pictures, which are 2 traced dogs, 2 traced photos, a pear, 3 emblems and a lettered patch, whose letters all kept
their columns.
Between a slenderness of 2 and 4 the running dog's result moves by 7 shapes of 107. The bound of a
part with arms, 1.4, lies between what was measured on 2026-11-15. Drawn bands, leaves, crosses and letters measure at
most 1.0, the lettered patch's letters at most 1.19, and the dogs' parts that sew as columns at most 1.23.
Drawn stars of 4, 5 and 6 points measure 1.61 to 2.62, and 5 blobs of the dogs' fur 1.51 to 2.11. MC-8 confirms the
defaults or changes them.

### What it costs

Measured on the 4 traced dogs on 2026-11-14, against the step before (`tests/auto_photos.rs` records the
numbers):

| Traced dog | Shapes sewn as satin columns | As fills | Jumps | Thread changes | Slivers | Bare |
|---|---:|---:|---:|---:|---:|---:|
| On turf, 6 threads | 64 to 27 | 4 to 45 | 124 to 82 | 10 to 12 | 4 to 0 | 0.204 to 0.020 % |
| On turf, 9 threads | 73 to 31 | 3 to 49 | 143 to 94 | 12 to 22 | 10 to 1 | 0.346 to 0.097 % |
| Running, 6 threads | 59 to 21 | 6 to 47 | 113 to 81 | 12 to 16 | 3 to 1 | 0.107 to 0.026 % |
| Running, 9 threads | 79 to 29 | 7 to 60 | 135 to 112 | 18 to 28 | 2 to 0 | 0.073 to 0.001 % |

The satin stitches that lean along their column (`SC-W0208`), which a fan sews, fell from 26 to 34 a dog to
4 to 8. The thread changes rose with M13.21, and each is a rethread by hand on a machine with one needle.
A design of many small fills has many lines, each of which sews after the fills it borders. M13.22
searches for the order that changes thread least, and the dogs changed thread 8, 12, 8 and 12 times
([Sewing order](#sewing-order)). Since M13.30 the second changes thread 13 times. With `auto_fill_uneven` off they change thread 6, 9, 7 and 10 times and
jump 113, 135, 105 and 132 times, where they jump 84, 91, 79 and 108 times as planned. The switch trades
thread changes for jump threads.

M13.27's rule for a part with arms fills 5 parts of the dogs that sewed as columns, blobs of fur 2 to 13 mm²
large, and the 2 stars of the lettered patch. Measured on 2026-11-15 against the step before, the thread
changes stay as they were:

| Picture | Parts that moved | Stitches | Jumps |
|---|---:|---:|---:|
| Dog on turf, 6 threads | 2 | 10464 to 10420 | 84 to 79 |
| Dog on turf, 9 threads | 0 | 11158 | 91 |
| Running dog, 6 threads | 1 | 11331 to 11330 | 79 |
| Running dog, 9 threads | 2 | 13108 to 13115 | 108 to 106 |
| Lettered patch | 2 | 25333 to 25340 | 37 |
| Alpine Night, the demo's badge | 2 | 5419 to 5443 | 9 |

## Stitch types by width

An automatic element is measured at the size it sews, in millimetres (`REQ-AUTO-002`). Its width at a place
is the diameter of the largest circle that fits inside it there. This section says how each stitch is
made once the [rules](#which-stitch-a-shape-gets) have chosen it.

A stroke is a line as wide as it is drawn (M13.1):

- No wider than `min_satin_stroke_width_mm` it sews as a running stitch along its path (`REQ-AUTO-002`),
  tripled from 0.5 mm wide (below). A satin column that narrow is too narrow to stitch across
  (`REQ-SAT-015`), so the bound is the same.
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
  than its limit is cut square. Those pieces, joined into one area, are the fill's outline. A part of it smaller than 1 mm²
  is a speck, merged into a neighbour or left out as specks are ([Neighbours](#neighbours)). An area that would reach more
  than 10 m from the origin is left out (`SC-W0903`). No design lies there.

  The pieces are not handed to the area code one by one, which would cost many times the work of their
  outline. Each open subpath is one ring instead. It follows the right side forward, the end cap, the left
  side back and the start cap. The outer side goes round each join, and the inner side goes in to the
  path's point and out again. A closed subpath is 2 rings, one for each side. Such rings wind round each
  point as many times as the pieces cover it. The nonzero rule then fills exactly the pieces' union. A line
  of more than 32 chords is cut into lengths of 32 chords. Each length is a ring made into its area alone,
  and the areas are then joined. A 10 mm line a metre long fills within the default budget that way.

**Triple runs** (M13.4, `REQ-AUTO-015`). A running stitch that automatic digitizing makes is a triple run
from 0.5 mm wide: each stitch is sewn 3 times, bean stitch 1. A running stitch sewn once is as fine as the
thread, and a line drawn half a millimetre wide or wider sews bolder tripled. A stroke is as wide as it is
drawn, and a narrow part (below) as wide as the circles that fit inside it along the middle. Widths are
compared to the micrometre, and MC-8 confirms or changes the bound.

Every automatic satin column, a stroke's as well as a shape's, has the settings of its width (M13.5,
[below](#settings-by-width)). A shape's area is a tatami fill apart from its narrow parts (M13.4), its
parts as wide as a satin column (M13.5) and the narrow parts of its broad ones (M13.6).

**Narrow parts** (M13.4, `REQ-AUTO-013`). A part of a shape that is narrower than
`min_satin_stroke_width_mm` everywhere sews as a running stitch along its centre line, as a stroke that
narrow does. No circle that wide fits inside such a part: shrunk by half the width with the polygon buffer
of M5.7, nothing of it is left. The width is taken half a micrometre wider, so that a part as wide as the
bound, give or take a rounding error, is narrow, as a stroke is. Before that, a part smaller than 1 mm² is a
speck ([Neighbours](#neighbours)), and stays in the area. So does a part whose centre line has no length, a
dot.

- **What sews.** The rest of the shape is a tatami fill, and each narrow part's line sews after it, in the
  order of the parts. It is named after the shape with `#2`, `#3` and so on, or with the shape's own name
  when nothing of it is filled. A trim or a stop after the shape follows its last part.
- **Its width.** A part's width at a point of its centre line is the diameter of the largest circle centred
  there that fits inside it. Averaged along the line, leaving out the piece that runs on into a tip's
  corner, it says whether the part is a triple run
  ([centre lines](algorithms/centre-lines.md#width-along-the-line)). A strip as wide all along as a stroke
  is as wide as the stroke.
- **To its neighbours** a narrow part's line paints the part. It sews after a fill it borders. An area
  sewn before it reaches under it, and is cut away under the part where the part covers it, as under an
  area ([Neighbours](#neighbours)).
- **Once the layers are flattened.** A shape's parts are found in what it sews once the layers are
  flattened ([Neighbours](#neighbours)): what shows of it, with the holes its details fill closed. A
  stacked trace's dark outline under the colours inside it shows as a ring, and sews as lines and columns,
  as an abutting trace's outline does. A broad area's holes under its details are closed, so the strips of
  it between the details are part of its fill. The parts are found before specks merge and before each
  area reaches under its neighbours.
- `SC-I0901` counts a shape whose every part runs as a running stitch, and any other as a tatami fill.
  Finding a part's width and centre line is part of the work of digitizing its shape. A part whose width
  or centre line can't be found, a bug, stays in the fill, and `SC-E0009` names its shape.

**Parts as wide as a satin column** (M13.5, `REQ-AUTO-016` to `REQ-AUTO-018`). Filled, a part as wide as a
satin column sews rows no longer than it is wide wherever they cross it. A digitizer sews it as satin
columns along its strips and round its rings. A part that isn't narrow, and is nowhere wider than 7 mm, is
as wide as a satin column. Shrunk by 3.5 mm, taken half a micrometre wider, nothing of it is left. Such a
part sews a satin column along each of its centre lines, between its own outlines
([centre lines](algorithms/centre-lines.md#columns)):

- **Its columns.** Each line sews one column over the triangles round it. A line is cut where it turns by
  more than 60°, so that the column before a corner covers it and the one after ends under it, as an arm
  of a T ends under its bar. A branch of the line is a column of its own however short, as each arm of a
  t's crossbar is. A branch to the second sharp corner of an end is a bump instead, sewn with the column
  beside it. So is a tooth, a step of a traced outline as narrow as the narrowest satin stroke somewhere
  and no longer than its junction is wide. A column of its own, it would sew a short line beside the column
  and cut that column in 2. A column's rails are the part's outline on either side, and its rungs lie
  square to it.
- **Their ends.** A column that ends where another runs on ends under it: it sews first, and is 0.4 mm
  longer at that end, its pull compensation, by its push compensation, so that no fabric shows where they
  meet. Otherwise a column
  ends at its farthest point along its line, flat along the end's edge when that lies within 20° of square
  to the column, and at a point otherwise.
- **Their order.** A column that ends under another sews before it. Then the longest sews first, and each
  next column is the one with an end nearest where the last one ended, sewn from that end. A ring starts
  at its rung nearest. Where the columns that end under each other go round, as a pinwheel's bars do, the
  first in that order sews first, and the last ends on top of it.
- **Their width.** A column is as wide as the circles that fit inside the part along its line, as a narrow
  part is, without the piece of its line into a tip's corner. One no wider than the design's narrowest
  satin stroke runs along its line as a triple run, unless the column is wider than that stroke somewhere
  along its line, but at its 2 ends, or reaches farther than that from its line beside it. Both are
  measured inside the column's own outline, the triangles round its line. A running stitch covers a
  thread's width along its line, so where the column is wider, the fabric beside the line shows unless an
  area sewn before it reaches under it. Measured on average alone, the strands of fur in the owner's traced
  dog that widened at an end ran, and so did its small blobs, each beside other lines, and they left fabric
  bare. So did a column whose line ran along one side of a bump 1.4 mm across: the bump's triangles were the
  column's, far from its line. Where 2 strokes meet, the part is wider than either, by where
  the junction's triangle has its corners. Measured inside the part, the circle at the junction of a T in
  strokes 0.9 mm wide was up to 1.13 mm, and the T's bar sewed as a satin column one way round and as a
  triple run mirrored. A column's own outline crosses to the other column there. A line's end lies where it
  meets another line or runs into a corner, and measures that. On TS-13B, the outer legs of the M 4 mm
  tall, 0.81 mm wide on average, are 1.05 mm wide where the M's middle strokes meet them, inside their own
  outlines, and sew as satin columns.
- **A dot.** A part whose every line is shorter than half its width along it is a dot, and stays in the
  fill, as a part whose line has no length does. A disc or a square has no way to run, and its lines are
  only what its triangles leave of its middle. Its corners lie on one circle, where the triangulation
  ties, and how each tie breaks depends on where the part lies and how it is drawn. Without the rule, of 2
  dots alike one could sew a column across and the other a fill. A disc's or a square's lines are 0.4
  times as long as it is wide, and discs and squares no wider than a satin column are fills wherever they
  lie. A rectangle 3 mm wide is a column from 3.5 mm long.
- **What sews.** The columns sew after the rest of the shape's area, named as a narrow part's lines are,
  and paint the part to its neighbours. `SC-I0901` counts a shape as satin columns when none of it is
  filled and one of its parts sews a satin column.
- **Drawn.** Each column is drawn with its rails and rungs, as the Stroke to Satin filter draws one, and
  read back it must give them. A part whose triangles don't make a column round one of its lines, or one
  of whose columns is read otherwise, stays in the fill, and `SC-I0902` says so. A rung added to tell a
  column's rails from its rungs goes across an end that doesn't reach under another column where one can
  take it ([satin columns](algorithms/satin.md#drawn-with-its-rails-and-rungs)).

**Shapes in parts** (M13.6, `REQ-AUTO-019`). A shape wider than 7 mm in places and narrower in others sews
in parts. Its broad parts are a tatami fill, and its narrow parts sew over the fill as lines and columns,
as a digitizer sews a balloon's string over the balloon:

- **Its broad parts** are what an opening with a disc 7 mm across leaves: the shape shrunk by 3.5 mm and
  grown back by 3.5 mm, with the polygon buffer of M5.7, each taken half a micrometre wider, as the bound
  is. An opening finds them without the shape's centre lines.
- **Its narrow parts** are the rest of the shape, cut where they meet the broad parts. The opening rounds
  each convex corner of a broad part, and leaves the corner as a narrow part. A narrow part that reaches
  less than 1 mm² beyond 3.5 mm from the broad parts is such a corner, or a stub too short for a column,
  and stays in the fill, as a speck does. Length along its centre line tells no corner. The piece a
  square corner leaves has a horn along each side of the corner, and its centre line runs 6 mm along
  them, less than half a millimetre wide.
- **The cut.** Where a narrow part meets a broad one, the opening leaves an arc, and a column would end
  there in a point at one of its corners. Each narrow part is cut square there instead, at a chord across
  it square to the way its sides run on, from the end of the arc that lies farther along that way. What
  lies past the chord goes to the fill. A chord that would cross the outline of the piece or of the part
  is left out, and the arc stays. That holds a hair from the chord's end too. Where an arc ended
  2.5e-10 mm before a corner of a traced cat's ear, the chord crossed the edge after the corner, and the
  piece had no triangulation (M13.23). A column ends flat along the cut, however long it is, so that a
  star's points end along the star's middle. The triangles that touch the cut are the column's end, and
  its line stops where they start ([centre lines](algorithms/centre-lines.md#columns)).
- **What sews.** Each narrow part sews as a part of its own. One narrower than
  `min_satin_stroke_width_mm` everywhere runs along its centre line, and one no wider than 7 mm sews as
  satin columns. Any other stays in the fill. The fill is the shape without its narrow parts. It sews
  first, and reaches the overlap, 1 mm, under the lines and columns sewn over it ([Neighbours](#neighbours)).

**Centre lines.** A narrow part's centre line, its width along it and the rungs across it come from a
constrained Delaunay triangulation of its outline, with points added along its long edges so that no
triangle is much longer than the part is wide:

- Each edge of a triangle that lies inside the part is a chord across it, as long as the part is wide
  there.
- A triangle with 2 inside edges is part of a strip, and its inside edges are the strip's rungs.
- A triangle with 3 inside edges is a junction, where 3 strips meet, and one with 1 is an end.

The centre line joins the middles of the chords (M13.4, `REQ-AUTO-014`). A round end stops it about half
its width from the end, on its middle, and a sharp point draws it on to the point. The small bumps of a traced outline, and
the corner past a sharp turn, would branch it, and are left out. A branch longer than its junction is
wide is an arm, and has a line, as each arm of a small T's bar has. At each junction left, the 2 branches that
run on most nearly straight are one line and the third ends there, and a ring has a closed line. No piece
of the line is sewn twice. A part's lines sew one after another, the longest first, each from its end
nearest where the last one ended. The [centre lines](algorithms/centre-lines.md) page has the rules in
full.

A part as wide as a satin column sews a column along each of its lines (M13.5, above). Its rails are the
part's outline on either side, and its rungs come from the triangles across it. A junction joins the 2
lines that run on most nearly straight, and the column of the third ends under theirs. A ring, such as a
letter O, is a closed column between its outer and inner outlines. The satin generator sews these columns
as it sews columns drawn with rails and rungs.

### Settings by width

Every automatic satin column, a stroke's or a part's, has the underlay of its width (`REQ-AUTO-018`),
compared to the micrometre. These are starting values, and MC-8 confirms or changes them:

| Column width | Underlay |
|---|---|
| Below 2 mm | Centre walk |
| 2 to 4 mm | Contour |
| Above 4 mm | Contour and zigzag |

A stroke's column is as wide as the stroke, and a part's as its width along its line. Each has a pull
compensation of 0.4 mm, a setting of its own (`auto_pull_compensation_mm`): the thread pulls a column in
from its rails, and stitches that reach that far past them sew the column as wide as drawn. Past a rail
that a shape sewn after the column runs along, the column sews by the overlap instead
([Neighbours that overlap](#neighbours-that-overlap)). A column ends at its rails' ends rather
than at the point nearest the next shape, which would join its ends to the next shape with a straight run
across it. Fills keep the tatami fill's defaults, underlay included.

### Holes and edges of columns

A letter traced as an area is cut into columns, and the reader has to see its counters and straight
edges. On MC-8's traced lettered patch (2026-11-17) the owner found the A black all over, and notches
along the letters' edges. 2 rules keep them (M13.36):

- **A hole shows what lies under it** (`REQ-AUTO-084`). A column's pull compensation reaches 0.4 mm past each rail, and
  the 3 columns round the A's counter, a triangle 1.4 mm across inside, sewed over all but 7 % of it. It
  was no better at M13.30 or at M14.5's merge, 3 %. A hole that a disc 0.8 mm across fits in, and whose
  box no shape drawn after the area meets, keeps its columns' stitches on its edge: a rail along it has no
  pull compensation on that side, the other rail keeps its own. Each edge's thread then spreads half a
  thread into the hole, and the hole keeps (1 - 0.4 / 1.4)² of itself, half of the 10 mm A's counter. A
  smaller hole closes under its edges' threads anyway, and a later shape in the hole's box may be a detail
  that the area reaches under (`REQ-AUTO-029`). A filled area keeps a hole as its rows stop at its edge.
- **An even column keeps its edge** (`REQ-AUTO-085`). Beside a junction a straight stem's other rail runs
  round the junction's corner, and the stem's outer points lie 0.3 mm apart, closer than the 0.35 mm at
  which a point moves in ([The thickest spot](#the-thickest-spot)). Every other point then moved in by a
  quarter of the column, 0.25 mm past its edge, and left a notch: 6 on the M's stems, 0.65 mm apart. A
  column whose stitches pair no more than 1.5 mm of one rail with a millimetre of the other moves its
  first crowded point no farther than its pull compensation and 0.1 mm, a quarter of a thread, past its
  edge, and the point's thread still covers the edge. So does a column along a hole kept open, held to the
  hole's edge. A column that fans, at a bend or where it ends on the slant under another, keeps the
  quarter: held there too, TS-13A rose from 4.28 to 4.57 fills and the badges from 4.40 to 4.58.

Where 2 columns of one shape meet, the one that ends under the other reaches its pull compensation past
the chord between them. No join of the patch's letters or TS-13B's words opened a gap a thread wide, on
`main` or since: the gaps the owner saw were the notches.

Measured with the cases' plans (`tests/auto_letters.rs`, `fabric::shown`), before (`main` at M13.35)
and after. The thickest spot and the jump threads include the start rule of
[a narrow column](#a-narrow-column-walks-once):

| Design | A's counter showing | Notches in letters | Thickest, fills | At 3 fills or more, mm² | Jump threads |
|---|---:|---:|---:|---:|---:|
| Lettered patch | 7 % → 62 % | 6 → 0 | 4.33 → 4.20 | 52.0 → 43.3 | 8 → 8 |
| TS-13B's word, 15 mm | 25 % → 73 % | 14 → 5 | 3.88 → 3.63 | 4.5 → 1.0 | 7 → 7 |
| TS-13B's word, 10 mm | 7 % → 62 % | 6 → 0 | 3.63 → 3.28 | 0 → 0 | 8 → 8 |
| TS-13B's word, 6 mm | 0 → 41 % | 0 → 0 | 3.78 → 3.44 | 0 → 0 | 3 → 5 |

The 15 mm word's M and C keep 5 notches where their points fan. The 4 mm word runs along its letters'
middles, and its A's counter, 0.56 mm across inside, isn't kept open.

### Letters sewn as strokes

Block letters and a logo's bars are drawn in strokes: straight bands of nearly even width that meet at
joins. An embroidery font sews each stroke as a satin column of its own, its stitches across it. The
columns along a part's centre lines run on round the corners where strokes meet. An M's middle strokes
took in the corners of its stems there, their rails ran on unevenly, and their stitches leaned up to 56°. TS-13B said so 6
times (SC-W0208). A part that reads as strokes now sews a column along each (`REQ-AUTO-087`), and where
2 strokes meet one runs through and the other stops (`REQ-AUTO-088`). The code is
`normalize/strips/strokes.rs` and its joins `strokes/joins.rs`, called from `auto/columns.rs`.

**Reading the strokes.** Each ring of the part is cut into straight runs, from its sharpest corner: a run's
points lie within 0.05 mm of the line between its ends. 2 runs are a stroke's sides when all of these hold.
They run opposite ways within 5°, and lie as far apart at both ends within a tenth. They are at least the
narrowest satin stroke and at most the widest satin column apart, and overlap along each other by half that
width. The longer is as long as the width and the shorter half as long. The part fills the band between
them but for 2 % of it. Sides on
the same 2 lines, within 2° and a tenth of the width, are one stroke, as an E's stem is between its arms.

**Joins.** 2 strokes meet where their bands, a width longer at each end, share more of the part than
0.02 of their widths' product (0.01 mm² at least). A stroke runs through when the part goes on past the
join by half its width both ways along it. Of 2 that both do, as in a cross, the longer runs through and
the other loses its band, narrowed on each side by its stitches' reach across, and sews in 2 columns. Of 2
that end there, the one whose end lies more nearly square to it runs through, by more than 0.15 of its
width. An M's stem has a square top and runs through, and its middle stroke, cut on the slant, stops. Alike, 2 strokes
whose lines meet between 15° and 60° are cut along the line that halves the angle between them, as at the
top of an A and the bottom of an M's V, and each reaches max(0, reach / 2 - reach cos) past that line.
There, reach is the stroke's pull compensation and cos the cosine of its line to the halving line. At a blunter
join, as at an L's corner, the longer runs through. Cut along a halving line there, each would end in a
point at the corner's outside, and the 2 tips and their pull compensation piled 4.34 fills on TS-13D's
white frame, where the columns along their centre lines laid 4.17.

**Where a stroke stops.** A stroke that stops keeps its side of the other's near side, moved into the
other by reach (1 - cos), the cosine of the angle between the 2 lines: its own stitches' pull compensation
already reaches the rest across the line where they meet. That stub is part of the stroke's end when it
lies within the flat end's 20° along the stroke, and it does for a stroke wider than reach / tan 20°,
1.1 mm at the default 0.4 mm. A wider stroke then ends on the other's side, and the overlap is the other's
own pull compensation, whose stitches run along the stopping stroke at a square join and reach 0.4 mm past
that side. Restored as rails of the stroke, the stub made TS-13D 4.55 fills thick where it is 4.23, the lettered
patch 4.09 where it is 4.00 and TS-13B 3.90 where it is 3.85, and no measure showed a gap either way, so
the stub is left out until a sew-out shows fabric where 2 strokes meet (MC-8). The stroke that stops sews first,
and ends under the other without a push compensation.

**Columns.** Each stroke's piece is the part inside its band, cut so. The pieces must cover the part but for
1 % of it. A piece's rails are its 2 sides from its end least far along the stroke to its end farthest. An
end whose points lie within tan 20° of the width along the stroke is flat, and any other ends in a point,
as a column ends along an edge within 20° of square (`REQ-AUTO-016`). A rung lies square to the stroke at
every corner of either rail. A flat end on the slant gets one more, past its corner farther in by twice
the corners' spread along the stroke, so that the stitches between pair no more than 1.5 of one rail with
1 of the other. A rung 0.1 mm past the corner notched the A's feet. The middle line has a point every half
width, so that a stroke's thin tips measure as the centre lines' do. A piece's points within 0.001 mm of a
corner of the part are that corner, so that a rail runs along a hole's edge where the hole does
(`REQ-AUTO-084`): without that the A's counter fell to 7 %.

**When a part keeps its centre lines' columns.** The rule runs only on a part that wasn't cut from a broad
one and reads as 2 strokes or more that meet. Its centre lines must give 2 columns or more, none of them a
ring. Its columns are kept only when none of them runs (`REQ-AUTO-016`). Strokes are straight only:
a curved stroke, as a C's or an S's, or a D's bowl, keeps today's columns, and curved strokes are a
candidate for a later step.

**Measuring the lean.** A stitch leans by the angle between it and the square to its stroke, over the zigzag
top stitches 0.4 mm long or more, leaving out the 2 at each end of a column. Those follow the column's
end. A flat end 19° on the slant measured 31° with the zigzag's own 10° at 6 mm, which says nothing of
how the column lies along its stroke.

Measured with the cases' plans before (`main` at M13.36) and after, built at opt-level 1. Small Font's M at
10 mm measures 3.71 fills and its A 3.31, alike:

| Design | M's most lean | A's most lean | Thickest, fills | At 3 fills or more, mm² | Jump threads |
|---|---:|---:|---:|---:|---:|
| Lettered patch | 54.5° → 16.7° | 28.0° → 25.2° | 4.20 → 4.00 | 43.25 → 40.75 | 8 → 8 |
| TS-13B's word, 15 mm | 51.4° → 18.9° | 31.6° → 24.1° | 3.63 → 3.18 | 1.0 → 0 | 7 → 7 |
| TS-13B's word, 10 mm | 54.5° → 16.7° | 28.0° → 25.2° | 3.26 → 3.13 | 0 → 0 | 8 → 8 |
| TS-13B's word, 6 mm | 56.2° → 18.9° | 29.5° → 21.8° | 3.44 → 3.37 | 0 → 0 | 5 → 6 |
| TS-13B's digitized half | | | 3.99 → 3.85 | 12.25 → 4.25 | 44 → 46 |
| TS-13D's digitized half | | | 4.17 → 4.23 | 67.75 → 51.25 | 12 → 12 |

TS-13D's one rise sits at the stacked emblem's white ring, not on a stroke. The white frame now sews as
strokes, with one bar through at each corner, and the ring's column starts elsewhere. Its area at 3 fills or more
fell by 16.5 mm². The emblems' white frames, refused as columns before, sew as strokes, and TS-13B's M no
longer says that its stitches lean. The 4 mm word runs along its letters' middles, as before. Every other
test picture sews as it did. Taking the strokes from the centre lines instead fails: the M's middle stroke
is free for only 1.5 widths, too short to fit a line to.

## Neighbours

The shapes of a traced picture meet. VectorCraft's Abutting method traces each area with a hole where the
next one lies, and its Overlapping method, like Inkscape's Trace Bitmap with its Stack option, traces each
colour whole, over the colours under it. Sewn as drawn, either sews badly where shapes meet, and digitizing
mends it in 5 steps, in this order (`stitchcraft_engine::auto::neighbours`, M13.2, M13.12 and M13.18).

1. **Background** (`REQ-AUTO-009`). The lowest shape is the picture's background when it is an automatic
   area with a shape on another path drawn on it, and its outlines cover the box around the design to within
   0.5 mm of each side, its holes and notches aside. The box is the one around the defining points of every
   shape's path, the background's own included. So a disc in the corner of a square, lowest but covering
   less of the box, is part of the picture. A shape on its own path is the same object's: the readers make a
   fill and its stroke 2 elements on one path, and a design of one shape with a stroke sews it. Where a
   shape of a tightly cropped picture touches its edge, the Abutting method cuts a notch in the background
   instead of a hole. A part of the box its outlines leave is a notch when the areas drawn on it fill it, to
   within twice the shapes' tolerance, and it runs along the outlines at least as far as along the box's
   sides, measured 0.5 mm inside them. A part that runs further along the sides is no notch. A disc or a
   rounded square that reaches the sides leaves such parts in its corners, and sews as the picture, though
   the shapes in its corners fill the rest. The cost of this rule is a background cut away along a whole
   side, by a bar down the side say, or cut in two by a band from side to side, which is sewn. The
   background is left out with `SC-W0906` before anything is digitized. With a StitchCraft effect, it sews
   as that effect says. Finding it has the work budget to itself, and a background that needs more is sewn,
   which `SC-W0906` says. What shows of the background inside the picture once the layers are flattened
   (step 2) is sewn: its parts that come no nearer than 0.5 mm to the box's sides. A trace that stacks its
   colours draws the background whole under the picture, a white disc on a badge included, which a trace
   whose colours abut draws as a shape of its own. Such a part sews as the background's own area, and
   `SC-W0906` counts it.
2. **Layers** (`REQ-AUTO-023`, M13.12). Digitized as drawn, a stacked colour's stitches would be chosen for
   its whole shape, though most of it is hidden. Taken as it shows, a broad area with details on it splits
   into narrow columns between them, traced either way, as a flower's middle does round its dots. A
   digitizer fills such an area whole and sews the details on top. So before anything is digitized, the
   layers are flattened, in 3 parts (`auto::neighbours::layers`).

   - **What shows.** The automatic areas drawn after an area are taken from it where they overlap it by
     more than twice the shapes' tolerance, as the order measures an overlap ([Sewing
     order](#sewing-order)). Rounding leaves hairlines where edges drawn as one meet, as round a disc drawn
     a hair larger than the disc on it. A part of what is left that is narrower than twice the shapes'
     tolerance everywhere, measured as a narrow part is, is such a hairline and is dropped. An area that
     they cover whole sews nothing (`SC-I0904`), and so does one of which only hairlines are left.
   - **Details.** Where what shows of an area is wider than its widest satin column somewhere
     (`auto_max_satin_width_mm`), the area is digitized whole under its details, the automatic areas that
     fill one of its holes. They fill the hole when what they leave of it is no wider than twice the shapes'
     tolerance, or smaller than the area's smallest area. The hole is closed, and the area's stitches are
     chosen by its width with the hole closed ([Stitch types by width](#stitch-types-by-width)). An area,
     or a part of one, no wider than a satin column everywhere, such as a letter or an outline, keeps its
     holes, and the shapes in them sew beside it. Closed, it would be a broad fill under them. Specks in a
     hole aren't details. When only specks fill a hole, the hole is left open, and the specks merge into the
     area round them (step 4), which closes it. A hole is filled from the parts whose boxes meet its own, and costs
     the work of those parts alone.
   - **Moved back.** The area sews before its details, so that they lie on it. It moves back in paint
     order past them, past the automatic areas whose part that shows doesn't overlap it, and past the
     other shapes whose boxes don't come within twice the shapes' tolerance of its box. It never moves
     across a stop, nor when it has a stop after it. The areas move the largest first, so that an area
     moves before the areas inside it. Once all have moved, a hole whose details don't all sew after its
     area is left open: sewn after them, the area would cover them. The area couldn't move back past them,
     or another area that has one of them for a detail moved back past it. On a red square with a white
     ring on it and the red showing again as a bullseye in the ring's hole, the ring moves back before the
     red, the bullseye being its detail, and the red's hole is opened again round the ring.

   The overlaps (step 5) then cut the area away under its details, the overlap inside their edges. So a trace
   that stacks its colours sews as one whose colours abut. An area whose region changed is given its new
   outline, and one whose region is what it was keeps its path as drawn. `SC-I0908` counts the areas that
   sew only what shows of them and those digitized whole under their details.

   Each area has the work budget to itself at each step of the flattening, and again when it is given
   what it sews. It costs a unit for each shape and each part it looks past, and the work of those near
   it. An area that needs more work for a step does without it. One that needs more to find what shows of
   it is sewn as it is drawn, in its place, but for the background, which is left out whole. The other
   areas' holes are found without it. When finding an area's details needs more work, its holes are kept
   open. When its move does, it keeps its place, and the holes whose details then sew before it are opened
   again. When giving it its region does, it is sewn as it is drawn, and isn't counted as digitized whole.
   `SC-I0908` names each. `SC-E0009` names an area whose region the region code fails to join, a bug,
   which is treated the same way.

   The test pictures ([Testing](#testing)) sew alike traced either way. An emblem 60 mm wide, a navy
   square with a white disc, a red star on the disc and 3 gold bars, on white, sews the same shapes with
   each thread. The navy square is one tatami fill of 1,848.5 mm², sewn first. The white disc is one fill
   of 380.7 mm² as its layers are flattened, sewn after it: the disc's 531 mm² less the star, which it then reaches under. The bars
   sew as satin columns 3.00 mm wide, as drawn, and the star as one fill. The stitches of the 2 traces
   differ by about 1 %, because the traces draw their colours in different orders. A button, a yellow disc
   24 mm across with 7 orange dots in a black rim 2.5 mm wide, sews the same shapes with each thread either
   way, and as many stitches, 1,925, 1,845 of them in the same places. Its yellow
   is one fill of 445.9 mm² under the dots, each dot a fill too, and its rim a satin column 2.50 mm wide.
   `tests/auto_pictures.rs` keeps the pictures sewing so.
3. **Thin lines** (`REQ-AUTO-090`, M13.18). A trace of a photo draws soft shading and the strands of fur as
   thin lines between patches. Each sewed as a running stitch between 2 fills, which ended their rows on it:
   the traced dogs sewed 15 to 34 shapes so, and those lines caused most of their stitches astray, their
   slivers and their jump threads. A digitizer sews such a strand as part of the patch it lies in. So once the
   layers are flattened, and before the stitches of any area are chosen, a part of an automatic area that is
   narrower than the narrowest satin stroke everywhere, and is no speck, is a thin line when its width varies
   along its centre lines by `auto_min_line_unevenness` of its width or more, measured as a column's
   unevenness is ([The measures](#the-measures)). It merges into the automatic area it shares the longest
   edge with, which may be the rest of its own area, the edge measured as a speck's is (step 4). Every area
   gives up its thin lines first. A line then goes into what its neighbours keep. On a tie it goes into the
   area that sews first. A thin line that doesn't touch an automatic area sews as a line. The thin lines of a
   shape that turns `auto_fill_uneven` off sew as lines, as with the `logo` style, and so do those of a shape
   that names its stitch. An area
   whose every part merges sews nothing. `SC-I0915` names each line merged, by its area, and where it went
   (`auto::neighbours::lines`). The bound of 0.05 is StitchCraft's own. The dogs' 80 thin lines vary by 0.09
   to 0.46 of their width, and TS-13B's window lines, drawn with an even pen 0.3 and 0.6 mm wide, by 0.01.
   The rule goes by the line's width and how it varies, never by what the picture shows. Each area has the
   work budget to itself to find its thin lines, and each line to find where it goes. One that needs more
   keeps its lines, and `SC-I0915` says so. The measure of thread astray paints the picture with each line
   merged in its neighbour's colour (`auto::as_merged`), since the neighbour's thread covers it by the rule.

   Lines merge only while the picture keeps its likeness (`REQ-AUTO-095`, M13.44,
   `auto::neighbours::lines::parts`). A line that doesn't touch another piece of its colour is a part of the
   picture by itself, as [the likeness measure](#likeness) counts parts, and merged into a neighbour of
   another colour it takes its colour away, as with a twig or the pale rim of a petal. The
   lines of one colour that touch each other are one group. A group that touches a piece of its colour that
   is no speck merges freely, since its colour still shows there. Any other group that is a part costs the
   picture's likeness 1 of its colour's parts, divided by the number of colours. The groups that share an
   edge with an area merge the cheapest first, those of the colour with the most parts, and of 2 as cheap
   the smaller, while their cost stays within `auto_min_likeness` of the picture's likeness. The rest sew
   as lines, and `SC-I0915` says so. A part is defined once, by `auto::is_part`, and the measure imports it.
   At 0, the default, nothing is measured and every line merges. The setting stays off until M13.35's cap
   on thickness lands ([Merging by likeness](#merging-by-likeness)).
4. **Specks** (`REQ-AUTO-008`). An automatic area, or a part of one, smaller than 1 mm² is too small to sew:
   it would sew a knot. It is merged into the automatic area it shares the longest edge with, which may be
   its own. A speck that fills a hole goes into the area around it, and no hole shows where it was. One that
   rounding left a hair from the rest of its own area rejoins that. On a tie, it goes into the area drawn
   first. The edge 2 shapes share is how much of the area's rings lies within twice the shapes' tolerance
   of the speck. A speck that doesn't share an edge with an automatic area is left out, and so is one whose
   neighbours need more work to find than the budget allows it: put back as drawn, it could overlap a speck
   merged in beside it. `SC-I0905` names each speck, by its area rounded to a hundredth of a square millimetre and
   kept under 1 mm², and where it went. A line filled because it is too wide for a column (M13.1) is an area
   too, so a part of it that small is a speck.
5. **Overlaps** (`REQ-AUTO-007`). Pull draws stitches in from the edges of their shape, so shapes that only
   meet show fabric between them. Each automatic area reaches its overlap, 1 mm, under each area, satin
   column, filled line or part's line or column sewn after it, from where it touches that shape, and no
   further than that shape as it is sewn. Under an area it reaches that far along its rows and less across
   them, half as far under a narrow part, and not where an area sewn before it already sews
   ([Less thread where areas overlap](#less-thread-where-areas-overlap)). The areas are taken from the last sewn to the first, so each
shape is taken as it is sewn: across a line narrower than the overlap, an area reaches on under the area beyond it.
   Shapes touch where their edges meet within twice the shapes' tolerance, so a gap that rounding left
   between edges drawn as one closes too. Along an edge where a shape runs on near the area without
   touching it, the area doesn't reach, and fabric shows between them as drawn, but for the overlap round
   the end of where they touch. A satin column covers the
   area between its rails as they are sewn, turned to run the same way. Where an area or a part's line or
   column sewn after the area covers part of it, as the details on an area digitized whole do, the area is
   cut away under that shape, the overlap inside the shape's edge, and that part is sewn once. Sewn whole, it
   would sew twice there, stiff and thick. A part of the area that such shapes cover whole sews nothing,
   however near their edge it lies. So does an area they cover whole. A trim or a stop after
   such an area has no place, as after any element that sews nothing (`SC-W0505`). `SC-I0901` counts only
   the shapes that sew. A part's line or column covers its part, and isn't cut away itself. It sews
   nothing when the areas, and the lines and columns of other shapes' parts, sewn after it cover the part
   whole, and it sews whole when they cover the part in places. A satin column or a line digitized from a
   stroke and sewn after an area goes over it, and the area is sewn whole under it. Cutting a fill round
   small letters adds stitches and can leave gaps
   ([Inkscape forum](https://inkscape.org/forums/embroidery/techniques-for-making-text-on-top-of-a-fill-more-crisp/)).
   `SC-I0904` counts the areas that reach under others and those cut away. It also names each area cut away
   whole, which sews nothing. The areas are named in the order they are sewn.

Only automatic areas change. A shape with settings is sewn as they say, and a satin column or a line
digitized from a stroke or a part keeps its stitches: a column is sewn past its rails by its pull
compensation, 0.4 mm ([Settings by width](#settings-by-width)), and by the overlap past a rail that a shape
sewn after it runs along ([Neighbours that overlap](#neighbours-that-overlap)). A fill that bounds no area is left to its fill, which says so.
The areas are taken from the last sewn to the first, so that each meets the shapes after it as they are
sewn. Each area has the work budget to itself for its specks and for its overlaps, and costs the work of the
shapes near it and a unit for each other shape it looks past, not the work of the whole design: what a
shape sewn after it paints is found once, with the work budget to itself. An area whose overlaps need more
keeps the region it has, and a speck that needs more is left out. `SC-I0904` and `SC-I0905` say so. A satin
column whose area needs more is not reached under, which `SC-I0904` says too. The regions are combined by
one function, which cuts the rings of both where they meet and keeps the faces by their winding numbers
(`normalize::region::ops`).

**Measured.** The test pictures ([Testing](#testing)) are sewn as drawn, each shape a tatami fill, and
digitized, and the edges where their shapes meet are measured with the thread pulled in by 0.4 mm, the
pull the overlap is there to make up for: the share of the edges that the top stitches' thread covers
once each is pulled in where the thread turns (edge coverage, `stitchcraft_testkit::metrics`).

| Test picture | As drawn | Digitized | Stitches as drawn | Stitches digitized |
|---|---|---|---|---|
| Stripes, 3 abutting | 98.9 % | 100 % | 2,994 | 3,221 |
| Badge, abutting | 89.1 % | 97.2 % | 3,761 | 3,764 |
| Badge, stacked | 100 % | 97.2 % | 3,981 | 3,764 |
| Badges in a row, abutting | 94.2 % | 99.9 % | 4,947 | 4,867 |
| Badges in a row, stacked | 100 % | 99.9 % | 4,872 | 4,867 |

The abutting badge's shapes, sewn as drawn, leave a tenth of their edges bare. Digitized, they overlap, and
97.2 % is covered. The stripes' edges are covered as drawn too, but by the running stitches the fills travel
on along them, under their rows, not by the rows: digitized, the rows themselves overlap. The stacked badge as
drawn sews its lower colours whole under the upper ones, which covers its edges with 217 more stitches,
6 %, sewn twice. Digitized, it sews the abutting badge's stitches, every one. The row of badges, each
the badge at three fifths of its size and sewn in the order below, covers its edges as the badge does, and
stacked it sews the abutting row's stitches. Without pull, every edge is covered as drawn and
digitized alike, to within a hundredth. `tests/auto_pictures.rs` keeps the numbers
from falling back, and MC-8 tells whether 0.4 mm is the pull on the machine.

### An outline round a fill

A tatami fill's rows end at its edge, and the thread pulls each row's end in. On a small shape with points,
such as a star 5 mm across, the ends of the rows are most of what shows, and the points sew blunt. A
digitizer runs a stitch round such a shape after filling it, which draws the edge as one line.
`auto_fill_outline` does that for a design (M13.27, `REQ-AUTO-051`), read of its lowest automatic
shape as the base layer's settings are.

Each automatic area then sews a running stitch along its edge, in its own thread, in an element of its own
straight after the area, named after it with `#2`. The edge is the area's as the layers and the specks
leave it, before the area reaches under its neighbours: what shows of it. Where a shape sewn after the area
touches the edge, the area lies under that shape by the overlap, and a run along the edge would lie under
that shape's stitches and thicken them. So the outline leaves out the edge within `auto::TOUCHING` of each
shape sewn after the area, and of each other element of the area's own shape, whose columns and lines are
the same shape going on. A line digitized from a stroke goes over an area, and hides no edge. The edge is
cut into pieces no longer than 0.5 mm, and a piece shows when its middle lies in none of those shapes. The
pieces that show in a row are one line, a ring where the whole edge shows. A line shorter than 3 mm is left
out, since 2 stitches beside a neighbour's edge sew a knot.

The setting is on by default since M13.28. M13.27 left it off, for the jumps it cost a traced photo.
Shown the badge's stars with and without it, the owner said that outlines were better all along. The Auto
effect's dialog has a row for it ([Settings](#settings)).

An outline's stitches follow a round edge. A running stitch keeps to its path within its tolerance, which
is 0.2 mm unless a shape sets it. Stitches 2.5 mm long stray 0.16 mm from a circle 9.6 mm across, so the
badge's moon was outlined with 12 sides. An outline has a tolerance of `auto::OUTLINE_TOLERANCE`,
0.1 mm, unless its shape sets `running_stitch_tolerance_mm`. Its stitches are then shorter where the edge
bends, never shorter than the machine's shortest stitch, and the moon has 18 sides.

An outline is sewn with as few jumps as its lines allow (`REQ-AUTO-052`). A traced area's edge is hidden
wherever a neighbour sewn later touches it, often for a few millimetres, and each hidden stretch parted 2
lines by a jump. The outline now sews through a hidden stretch of `BRIDGE`, 25 mm, or less, along the edge
and under the shape that covers it, at a stitch for each 2.5 mm. The longest hidden stretch of a ring stays
out, because sewing it would join nothing. Under a fill sewn after the area the limit is 12 mm
([the outline under a later fill](#the-outline-under-a-later-fill)). Of an outline's lines, the one with an end nearest where the
next element starts sews last and ends there. An open line is turned round for that if need be, and a ring
starts at its corner nearest that place.

What the outline costs, measured on 2026-11-15 with the setting off and on:

| Picture | Stitches | Jumps | Thread changes |
|---|---:|---:|---:|
| Alpine Night, the demo's badge | 5443 to 5575 | 9 | 3 |
| Pear | 5096 to 5517 | 4 | 2 |
| Lettered patch | 25340 to 25944 | 37 to 38 | 4 |
| Dog on turf, 6 threads | 10420 to 11400 | 79 to 84 | 8 |
| Dog on turf, 9 threads | 11158 to 12261 | 91 to 97 | 12 |
| Running dog, 6 threads | 11330 to 12479 | 79 to 82 | 8 |
| Running dog, 9 threads | 13115 to 14417 | 106 to 109 | 12 |

An outline sews in its fill's thread straight after the fill, and doesn't change the thread. Before the 2
wins the traced dogs' outlines brought 19 to 30 jumps each, and they bring 3 to 6 now. Sewing through every
hidden stretch, whatever its length, brings 0 to 3, for 400 more stitches on each dog: 25 mm is where the
stitches spent stop buying jumps. The jumps that are left come from a fill joined from 2 parts of one
shape, whose 2 outlines each leave out the edge between the parts, from a ring and a line of one outline
that lie far apart, and from hidden stretches longer than 25 mm. Each jump is a thread that a machine with
one needle leaves to cut by hand, until M13.17 of the [roadmap](../plan/roadmap.md) sews those moves under
the shapes that cover them.

## Covering the fabric

The fabric's colour can't be chosen. It may be black, so a design has to cover the fabric by itself. The
measure of bare fabric ([Sewing order](#sewing-order), `REQ-AUTO-025`) passes a patch under 1 mm², and its
disc needs 0.45 mm of bare fabric on the grid, so a sliver 0.4 mm wide between 2 shapes passes too. On the
traced dog such slivers lay between strands of fur and beside the nose, and on dark fabric each was a dark
line. M13.16 of the [roadmap](../plan/roadmap.md) made the measure stricter, and built one underlay under
the whole design and a wider overlap between neighbours to meet it (`REQ-AUTO-027` to `REQ-AUTO-029`,
`REQ-RND-003`).

### The stricter measure

`stitchcraft_testkit::fabric::cover` paints the picture as the measure of bare fabric does (`REQ-AUTO-027`).

- **The design** is what the picture paints, with the slits between its shapes that are narrower than a
  thread: the painted places closed with a disc 0.4 mm across. A hole a thread wide or wider stays fabric.
- **Its edge**, the outermost 0.2 mm of the design, isn't measured. A fill's first row lies up to a row's
  spacing inside the outline, and beyond the outline is fabric anyway.
- **Bare** is the part of the rest that the top stitches don't cover, and **open** the part that the thread of every role leaves uncovered. The
  base layer is underlay: it shows in a bare place, and it doesn't make the place covered.
- **A gap** is a place where a disc 0.35 mm across fits on bare places, 7 pixels of the grid. A strip of
  fabric 0.4 mm wide holds 7 or 8 pixels by where it lies, so this disc fits in every such strip, and the
  9 pixels of the older measure's disc fit in none.
- **The base layer lies under** a place inside the shape that one of its pieces fills
  ([below](#one-underlay-under-the-whole-design)).
- **The rim** is the band along the design's outline that the base layer's offset leaves without a base
  layer, 0.4 mm wide by default. A part of the design narrower than twice that is all rim.
- **A sliver** is a gap no larger than `auto_base_sliver_mm2`, 0.4 mm², whose every place has the base
  layer under it or lies in the rim: its part without the base layer lies wholly within that band. Over the base layer its thread shows in place of the fabric. In the
  rim a sliver lies at the outline and reads as part of it.

A picture passes when no fabric shows inside its design, counting the base layer:

- Every gap is a sliver. Fabric shows in a larger gap, and in one with a place further inside than the rim
  that the base layer isn't under.
- With a base layer, the top stitches leave no more than `auto_base_bare_percent`, 0.5 %, of the design
  bare: the slivers and the places narrower than a thread together.

A design without a base layer has nothing under a gap. It passes with no gap at all and with no more than
0.15 % of its design bare, twice what a picture without gaps leaves. A design that turns `auto_base_layer`
off is told that fabric may show (`SC-I0911`). `Cover::faults` gives the verdict, by the settings of the
design.

Measured on 2026-11-10 at `8b8b23bd`, before the step, when a picture passed with no gap and 0.15 % bare:

| Picture, digitized | Design | Bare | Share bare | Gaps |
|---|---:|---:|---:|---:|
| Traced pear | 2,252 mm² | 1.87 mm² | 0.083 % | 0 |
| Traced dog on turf, 6 threads | 1,639 mm² | 8.47 mm² | 0.517 % | 12 |
| Traced dog on turf, 9 threads | 1,639 mm² | 12.67 mm² | 0.773 % | 17 |
| Traced running dog, 6 threads | 1,944 mm² | 5.35 mm² | 0.275 % | 7 |
| Traced running dog, 9 threads | 1,944 mm² | 6.26 mm² | 0.322 % | 8 |
| Stripes, 3 abutting | 1,800 mm² | 5.91 mm² | 0.328 % | 0 |

The stripes failed on their bare share alone. Of the red stripe's 160 rows only 93 had a needle point on
its outer side, and the others turned 0.8 or 1 mm short of it, which left notches narrower than a thread
along the 2 outer sides. A turn between 2 rows was a stitch shorter than the machine's shortest and was
dropped. Automatic fills now keep their turns as stitches
([tatami fill](algorithms/fills.md), `REQ-FILL-TAT-014`).

As built, measured on 2026-11-14:

| Picture, digitized | Bare | Share bare | Slivers | Of them reaching into the rim | Largest sliver |
|---|---:|---:|---:|---:|---:|
| Traced pear | 0 | 0 % | 0 | 0 | |
| Traced dog on turf, 6 threads | 0.33 mm² | 0.020 % | 0 | 0 | |
| Traced dog on turf, 9 threads | 1.59 mm² | 0.097 % | 1 | 0 | 0.17 mm² |
| Traced running dog, 6 threads | 0.51 mm² | 0.026 % | 1 | 0 | 0.26 mm² |
| Traced running dog, 9 threads | 0.02 mm² | 0.001 % | 0 | 0 | |

No fabric shows on any of them. The test pictures, the stripes among them, and the dogs' pieces don't leave a
gap. `tests/auto_cover.rs` holds their cases. The whole dogs are cases of `tests/auto_photos.rs`, which
runs in the release build. That file records each dog's row of this table, and its case fails when a
number changes. A dog whose slivers grow then shows before it comes to a limit.

These are the numbers since M13.21, which fills the dogs' ragged patches
([which stitch a shape gets](#which-stitch-a-shape-gets)). The limits were set at M13.16, when the dogs left
19 slivers and 0.07 to 0.35 % of their designs bare. Those slivers lay beside thin strands of fur. A strand
that sews as a running stitch is as wide as
its thread and can't reach under a neighbour, so the overlap can't close the fabric beside it. A thin
column's end sewn further under its neighbour closed 4 of them and laid 19 more stitches astray, so it
isn't built ([below](#neighbours-that-overlap)). The owner's decision of 2026-11-14 leaves a gap this small
to the base layer. 3 of the 19 reached into the rim, each by no more than 0.42 mm from the outline. Both limits are StitchCraft's own, set from the dogs: their slivers were 0.09 to
0.335 mm², and the dog on turf in 9 threads left the most bare. They are settings
([Settings](#settings)), and MC-8 confirms them on black fabric or changes them.

### One underlay under the whole design

Ink/Stitch has this as its Knockdown Fill tool. Its documentation says that the tool can make a global underlay
and that the offset is then usually negative ([fill tools](https://inkstitch.org/docs/fill-tools/) as read
on 2026-11-10). What it does, read
in `lib/extensions/knockdown_fill.py` at `d59c9ab` and told in our words
([ADR-0012](adr/0012-read-dont-copy.md)):

- **Outlines.** It takes the area each selected element sews. For a fill that is its shape, grown by its
  `expand`, and for a satin column the area between its rails as sewn, its pull compensation included. A
  running stitch counts as a band 0.3 mm wide along its path, and a ripple as a band of that width along each
  line of its stitches. A zigzag stroke counts as a band as wide as it sews, and a clone as what its elements make.
- **The outer shape.** It unites the outlines and offsets the union by the offset asked for, outwards when
  positive and inwards when negative. The join style, round or mitre or bevel, shapes the corners that the
  offset makes, with a mitre limit of 5 unless another is given, and never under 0.1. It then drops points
  that lie within 0.3 px, 0.08 mm, of the line between their neighbours.
- **Holes.** Each piece of that shape becomes one path. With "keep holes", on by default, the path has the
  piece's holes, and otherwise only its outer ring.
- **Place.** The paths go before the first selected element in drawing order, so they sew first.
- **Stitches.** Each path is a fill whose row spacing follows the stitch length asked for, 3 mm by default:
  `row_spacing_mm` is the length times √3/2, written to 4 decimals, with `max_stitch_length_mm` the length
  and `staggers` 2. Its underlay sews at `fill_underlay_angle` "60 -60" with the same length and spacing, and
  `underpath` and `underlay_underpath` are off. Rows at 0°, 60° and −60° that far apart and staggered over 2
  rows put the needle points on a lattice of triangles as long a side as the stitch.
- It can also frame the selection with a rectangle or a circle that the selection is cut out of, for
  embossing. StitchCraft doesn't take that.

Run as a black box at `d59c9ab` on small shapes drawn for it, it gives these numbers, which StitchCraft's
cases match where they share a setting:

| Shapes | Options | What it makes |
|---|---|---|
| 2 squares 20 mm a side that meet | offset 0 | one rectangle 40 by 20 mm |
| the same | offset −1 mm | one rectangle 38 by 18 mm, its 4 corners sharp |
| the same | offset 1 mm, round | 42 by 22 mm, each corner rounded through 1 or 2 more points |
| the same | offset 1 mm, mitre and bevel | 42 by 22 mm with 4 sharp corners, and with each corner cut 1 mm back |
| an L with arms 10 mm wide and its inner corner at (20, 20) | offset −1 mm | that corner at (19, 19) with a mitre, cut from (19, 20) to (20, 19) with a bevel, and rounded through (19.29, 19.29) |
| a square 30 mm a side with a hole 6 mm a side | offset −1 mm | a square 28 mm a side with a hole 8 mm a side, and no hole without "keep holes" |
| the same with a hole 1.5 mm a side | offset 1 mm | no hole, which the offset closed |
| 2 squares 0.3 mm apart | offset 0 and 0.2 mm | 2 paths, and one path |
| a satin column 4 mm wide, compensation 0.4 mm | offset 0 | a band 4.8 mm wide |
| a rectangle 40 by 20 mm, knocked down | stitch length 3, 2 and 1.5 mm | 624, 1,006 and 1,618 stitches |
| the running dog in 9 threads, 107 fills | offset 0 | one piece of 1,940 mm² with an outline of 294 mm and no hole |
| the same | offset −0.6 mm, stitch length 3, 2 and 1.5 mm | 1,767 mm² in 1,418, 2,372 and 3,553 stitches |

StitchCraft's base layer (`REQ-AUTO-028`) is that fill, made by automatic digitizing itself:

1. **When.** A design with automatic shapes gets one, unless `auto_base_layer` is off. Shapes with settings
   are sewn as they say and aren't part of it.
2. **The outer shape** is the union of what is painted by every automatic element that sews, once the
   background is left out, the layers are flattened and the specks are merged ([Neighbours](#neighbours)). An area paints its
   region and a line the area its stroke covers. The union is closed by half a thread, grown by 0.2 mm and
   shrunk by as much with round joins, so that a slit narrower than a thread between 2 shapes is part of
   it, as it is part of the design that the measure reads.
3. **Offset and holes.** The shape is offset by `auto_base_offset_mm`, −0.4 mm, with round joins. Ink/Stitch's
   join styles differ only at the corners an offset makes, and a setting can follow when one is asked for.
   Its holes are kept unless `auto_base_keep_holes` is off. A piece smaller than the smallest area is left
   out, and so is one with no room for a row of the base layer's stitches. A part of the design narrower than twice the inset has no base layer. It sews as a line or a
   column, and a column still sews the underlay of its width, which shapes its edges.
4. **Stitches.** Each piece is a tatami fill with the parameters Ink/Stitch's tool writes, at the stitch
   length `auto_base_stitch_length_mm`, 3 mm. StitchCraft's tatami fill has each of them, so no stitch code
   is new, and the nightly differential can sew Ink/Stitch's own knockdown path beside it.
5. **Underlay all through.** Every stitch of the base layer has the underlay's role, its top rows too. The
   measures go by a stitch's role. They take a row that has the top's role for cover, and they don't find it
   astray where it shows.
6. **Thread.** It is the thread that paints the largest area of the picture, the one painted first of 2
   as large. On the pear, the dogs and the badges that thread is also the first one sewn, and the base
   layer doesn't add a thread change. The digitized halves of TS-13A, C and D start in another thread and change
   thread once more.
7. **Order.** It is the element `stitchcraft:base`, its pieces after the first named `#2` and so on. It sews
   before every automatic element, at the place of the first one, and the order of
   [Sewing order](#sewing-order) leaves it there.
8. **The fills on it** sew no underlay of their own: digitizing turns their `fill_underlay` off. On the
   running dog in 9 threads the fills' own underlays are 601 of today's 11,996 stitches.
9. **Said.** `SC-I0909` says the base layer's area, pieces and thread. A base layer whose shape needs more
   work than the budget allows is left out with `SC-W0910`, and every fill then keeps its underlay. A
   design that turns the base layer off is told that fabric may show (`SC-I0911`). That is information and
   no warning, since the design sews as its settings ask. M13.21's specification planned that code for
   the message that says which rule chose a shape's stitches, and it takes the next free one.

What it costs on the running dog in 9 threads, by Ink/Stitch's counts above at an offset of −0.6 mm, and
how much of a gap 3 directions of thread 0.4 mm wide would hide, as it sewed before M13.30 gave it one
layer ([Less thread where areas overlap](#less-thread-where-areas-overlap)):

| Stitch length | Row spacing | Stitches | Less the fills' own underlays | At 600 stitches a minute | Of a gap, hidden |
|---|---:|---:|---:|---:|---:|
| 3 mm | 2.60 mm | 1,418 | 817 more, 6.8 % | 2.4 minutes | 39 % |
| 2 mm | 1.73 mm | 2,372 | 1,771 more, 14.8 % | 4.0 minutes | 55 % |
| 1.5 mm | 1.30 mm | 3,553 | 2,952 more, 24.6 % | 5.9 minutes | 67 % |

The plan has no sewing speed, and 600 stitches a minute is an assumption. A base layer this open doesn't
cover a gap. It puts thread of one colour under it, which shows less than black fabric does. The overlap is
what covers the gaps, and a sliver is what it leaves to the base layer.

As built, at the default stitch length of 3 mm, against the same picture with the base layer off, whose
fills sew their own underlays:

| Picture, digitized | Base layer's stitches | Stitches with it | Without it | Difference | At 600 stitches a minute |
|---|---:|---:|---:|---:|---:|
| Traced pear | 1,156 | 5,096 | 5,295 | 199 fewer, 3.8 % | 0.3 minutes less |
| Traced dog on turf, 6 threads | 1,201 | 10,463 | 11,921 | 1,458 fewer, 12.2 % | 2.4 minutes less |
| Traced dog on turf, 9 threads | 1,188 | 11,185 | 12,850 | 1,665 fewer, 13.0 % | 2.8 minutes less |
| Traced running dog, 6 threads | 1,322 | 11,297 | 13,015 | 1,718 fewer, 13.2 % | 2.9 minutes less |
| Traced running dog, 9 threads | 1,313 | 13,071 | 15,183 | 2,112 fewer, 13.9 % | 3.5 minutes less |

Every picture sews fewer stitches on one base layer than with an underlay under each fill. Before M13.21
the dogs sewed 675 to 1,027 more with it: most of their patches were satin columns then, which sew an underlay
with or without the base layer.

### Neighbours that overlap

Digitizing guides extend the colour sewn first under its neighbour by 1 to 1.5 mm on large areas and about
1 mm on small ones. mySewnet's manual says that compensation doesn't prevent gaps between neighbours, and
that overlap does. The owner brought both to this step on 2026-11-09. Before it an area reached 0.4 mm under the
shapes sewn after it, and a satin column sewed 0.4 mm past both rails, whatever lay beside it
([Neighbours](#neighbours)). `REQ-AUTO-029` changed 3 things:

1. **`auto_overlap_mm` is 1 mm.** An area reaches that far under each shape sewn after it, from where it
   touches it and no further than that shape, as today.
2. **A satin column sews under its neighbour too, by the rail.** Along a rail where a shape sewn after the column runs for
   more than half the rail's length, the column sews past that rail by the overlap. Ink/Stitch's
   `pull_compensation_mm` takes one value for each rail, and Ink/Stitch can sew such a column.
3. **Pull compensation is a setting of its own,** `auto_pull_compensation_mm`, 0.4 mm. A rail with no shape
   sewn after it beside it gets that. A column then doesn't grow wider over a colour sewn before it or over
   fabric.

An area is cut away under a shape sewn after it that covers part of it, the overlap inside that shape's
edge. A shape narrower than twice the overlap then lies on the area whole.

A column's end under another column stays as long as it was: it reaches its pull compensation under that
column, not the overlap. Sewn 1 mm longer, the ends of thin columns laid 19 more stitches astray on the
dogs, and the end of a column 4.4 mm wide skewed its last stitches and left 3.2 mm² bare (the piece
`blaze` of `tests/auto_cover.rs`). By the owner's decision of 2026-11-14, the small gaps at such ends
are left to the base layer.

Measured before the step, the setting as it then was bounded what the overlap could do. Raised, it also
widened every satin column on both sides, which laid stitches astray, so these are not the step's numbers:

| Picture, digitized | 0.4 mm, today | 1 mm | 1.5 mm |
|---|---|---|---|
| Traced pear | no gap, 0.083 % bare | no gap, 0.074 % | no gap, 0.072 % |
| Traced dog on turf, 6 threads | 12 gaps, 0.517 % | no gap, 0.049 % | 2 gaps, 0.074 % |
| Traced dog on turf, 9 threads | 17 gaps, 0.773 % | 2 gaps, 0.136 % | no gap, 0.046 % |
| Traced running dog, 6 threads | 7 gaps, 0.275 % | 3 gaps, 0.215 % | no gap, 0.057 % |
| Traced running dog, 9 threads | 8 gaps, 0.322 % | one gap, 0.070 % | no gap, 0.041 % |

At 1 mm the pictures sewed 2.2 to 4.7 % more stitches, and at 1.5 mm 3.9 to 9.5 % more. At 1 mm one gap of
the running dog in 6 threads was new, 2.0 mm² at (40.9, 42.4): the skewed end of the blaze's column.

### Less thread where areas overlap

An overlap is thread under thread. M13.29 measured that the overlaps cause most of the thread where a
traced picture sews thick ([How thick a design sews](#how-thick-a-design-sews)), and the owner's first
concern on 2026-11-16 was that thickness. `REQ-AUTO-067` (M13.30) takes away the part of an overlap that
the pull doesn't ask for (`auto::neighbours::under`):

1. **The overlap follows the rows.** The thread pulls a row in at its ends and not sideways. So an area
   reaches its overlap, 1 mm, along its rows, and its pull compensation, 0.4 mm, across them. What it sews
   under a shape is every point of that shape within an ellipse with those half axes round a point where the
   2 touch. Across its rows the overlap is never wider than along them, and without pull compensation
   it is 0.01 mm wide.
2. **Half as far under a narrow part.** Under a part of an area that is narrower than 3 overlaps, the
   overlap is half: 0.5 mm along the rows, and across them no more than that. A part is narrow where no disc 3
   overlaps across fits on the area. The areas on both sides of a part just under 3 mm wide then leave a
   third of it free, where 2 whole overlaps filled a part 2 mm wide between them. This is the owner's idea of
   2026-11-16, and digitizers work so by hand: one guide extends the lower colour 1.5 mm under bold areas,
   1 mm under fine details and 0.3 to 0.5 mm under delicate lettering
   ([Stitch Era's magic wand, a retailer's article](https://embroideryhoopstore.com/blogs/articles/stitch-era-magic-wand-digitizing-that-actually-stitches-clean-vector-vs-bitmap-underlay-and-the-1-5-mm-overlap-fix)).
3. **One overlap under a shape.** Where 2 areas would both sew under a shape sewn after them, only the
   area sewn first sews there. This is the owner's idea too: one overlap then lies under the shape at that
   place, where 2 lay at the corner of 3 shapes. The areas are taken from the first sewn to the last. A piece of an overlap that this cuts off from what shows of its area is left out.

Under a satin column, and under the part that a running stitch or a column sews along the middle of, an area
reaches its whole overlap whichever way its rows run, as before. A column's stitches don't cover its
ends and its bends as a fill's rows cover an area, and a running stitch covers a thread's width of its part.
Where what an area sews across a line's part meets a shape beyond the line, the area reaches on under
that shape from there. Of the 2 areas beside a line of fur, the one sewn first then sews the line's part
and 1 mm under the other, and the other gives the part up: their overlap is whole, whatever the line's
width.

When parting an area's overlaps needs more work than the budget allows, the area sews its whole overlap,
and `SC-I0904` says so. `SC-W0913` tells of a place that still has 3 fills of thread or more.

**Where the thread lay.** Before the step, the running dog in 9 threads had 629 mm² at 3 fills or more.
Each stitch of its plan was put to what it sews, and the thread summed over each window of 1 mm, as the
measure sums it. The mean window of the thick area had 4.0 fills:

| Thread in the thick area | Fills, mean |
|---|---:|
| Overlaps of other areas | 1.47 |
| Satin columns | 0.84 |
| The area's own fill | 0.71 |
| Base layer | 0.43 |
| Outlines | 0.25 |
| Running lines | 0.18 |
| Lock stitches | 0.11 |
| Travel stitches | 0.03 |

Of the thick area, 61 % had the overlaps of 2 areas or more on it, and 20 % those of 3 or more. An outline lay
on an overlap in 59 %, and an overlap under a column or a line in 33 %. 2 columns joined in 6 %. Locks
made half a fill or more in 7 %, and travel stitches in 1 %. A third of the thick area lay between 3.0
and 3.5 fills, because a straight edge between 2 fills is at the limit by itself. The upper fill and the
overlap are a fill each, and the ends of their rows a quarter each. The base layer adds 0.3 to 0.45, and
the upper fill's outline a quarter.

**What was measured.** Each change was built as a switch and measured alone and with the others on the
standard pictures. These are the running dog's numbers in 9 threads. The bare share is what the top thread
leaves of the design ([covering the fabric](#covering-the-fabric)). The last column is that share once every
top stitch is pulled in by 0.4 mm where the thread turns, with the gaps that then open:

| Change | At 3 fills or more, mm² | At 6 or more | Bare, % | Stitches | Pulled in: bare, %, and gaps |
|---|---:|---:|---:|---:|---|
| As sewn before | 629 | 6.2 | 0.000 | 14,380 | 0.037, none |
| One overlap under a shape | 522 | 0 | 0.000 | 13,984 | 0.140, 1 |
| The overlap across the rows 0.4 mm | 528 | 2.0 | 0.000 | 13,574 | 0.056, none |
| Half the overlap under a narrow part of an area | 502 | 1.0 | 0.012 | 14,019 | 0.148, 1 |
| Half the overlap under columns and lines' parts | 589 | 4.8 | 0.007 | 14,241 | 0.098, 1 |
| At every other turn, the rows go on past the area | 538 | 2.2 | 0.013 | 14,246 | 0.115, 1 |
| A column reaches 0.4 mm past a rail | 611 | 5.2 | 0.000 | 14,380 | 0.043, none |
| No outline along an edge another area reaches under | 548 | 2.2 | 0.000 | 13,400 | 0.038, none |
| An overlap of 0.5 mm | 437 | 1.0 | 0.012 | 13,638 | 0.173, 1 |
| No overlap | 155 | 0 | 0.409 | 12,849 | 2.533, 47 |
| The rules as built, with the overlap across a line | 491 | 1.2 | 0.000 | 14,028 | 0.077, none |

The owner also asked on 2026-11-16 whether the overlap could sew every other row. A fill sews its rows out
and back, so a row can't stop short alone: it was built as rows that reach on past the area at every other
turn. With the other rules it measured the same thick area as the overlap by the rows, 393 mm² against 395, and left
twice the fabric bare. It was left out, and nothing in the fill changed.

Half the overlap under columns and under the parts of lines took 5 to 8 points more off. It left 6 slivers
on the dog on turf in 9 threads, where an area no longer reached across a line of fur, and on the running
dog in 6 threads it opened a gap of 0.6 mm² beside a column, which fails the check of the cover. A column
that reaches 0.4 mm past its rail takes 3 % off a dog and reverses `REQ-AUTO-029`. Leaving the outline out
along an edge that an area sewn before reaches under takes 13 to 17 points more off, the largest change
left. The owner chose the outlines ([An outline round a fill](#an-outline-round-a-fill)), so that is hers
to decide. The area sewn last keeping a shared place, in place of the first, opened 4 to 8 gaps once the
thread was pulled in: the first area's rows then end on its own edge with the shape. Without the overlap
across a line's part, the 2 areas beside a line of fur overlapped by 1 mm less the line's width, and the
running dog left 0.184 % bare once pulled in. With both areas keeping the line's part, the cover was as
before and the dogs had only 11 to 16 % less thick area.

**The base layer has one layer.** The overlaps were not all of it. The base layer sewed as Ink/Stitch's
knockdown fill does, in 3 layers 60° apart. Each layer sews along the piece's edge from the end of one row to the next, so 3 in 10 of the base layer's millimetres of thread lay along its own edge: 894 of 2,995 mm on
the running dog in 9 threads. There it measured 1.2 to 1.5 fills, and 2.3 at its worst, under the ends of
the fills' rows and under the outline. It measured 1 fill or more on 106 mm² of that dog's 1,841. Every
automatic fill has the rows of one angle, `auto_fill_angle`. So the base layer now sews one layer, its rows
across the fills' rows: the underlay a fill is usually given, once under the whole design. The setting
`auto_base_rows` at `lattice` sews the 3 layers as before, for a fabric with pile.

Measured and left out: the base layer cut away under the overlaps, which leaves 43 to 58 % more thick area,
since each layer of each piece then sews along its new edges, beside each edge between 2 fills. The base layer cut
away under satin columns leaves 2 to 5 % more and a gap with nothing under it. With `underpath` on, the needle moves from row to row inside the base layer and not along its edge, which leaves 1 to 2 % more. 2 layers leave 10 to 17 % less than 3, and one
layer 22 to 29 % less, whichever way its rows run. The check of the cover gives each of these the same result: it knows where the base layer's shape lies, and not where its thread does.

**A part whose one satin column would crowd its stitches is filled** (`REQ-AUTO-068`). What was left at 6
fills or more was no overlap and no fill. A fill's own thread reaches 2.5 fills nowhere on the dogs. A
satin column of a small ragged part does: a column lays 1.25 fills where it runs straight, and each dog had
34 to 40 mm² where one column alone laid 2.5 or more, up to 6.6. A column sews as many needle points on one
rail as on the other, so where one rail is short its stitches meet: a part 3 mm across of the dog on turf
sewed 19 stitches from a rail 5 mm long to one of 0.8 mm. A part that would sew as one column is now
filled when the column would sew more than 2 mm of one rail to 1 mm of the other. A part of several columns
isn't measured, since a column that ends under another is cut on the slant and fans there by the way it is
drawn: measured on every part, the bound filled every T and the letters of TS-13B. A part with a hole stays
a column too, as the white round TS-13D's star does: its fill would sew in pieces round the star. Measured
by the whole rails' lengths, the bound filled almost nothing: the dogs' columns crowd at one bend.

**What it changed**, as `--auto` sews each picture, before the step and after it:

| Picture | Thickest, fills | At 3 fills or more, mm² | At 6 or more, mm² | Bare, % | Stitches | Jump threads |
|---|---|---|---|---|---|---|
| Lettered patch | 5.8 to 5.3 | 142 to 107 | 0 | 0.000 | 4,736 to 4,089 | 7 |
| TS-13A, digitized half | 5.7 to 5.6 | 124 to 78 | 0 | 0.003 | 13,809 to 11,524 | 18 |
| TS-13B, digitized half | 6.6 to 5.6 | 120 to 49 | 0 | 0.059 to 0.062 | 8,205 to 6,973 | 43 |
| TS-13C, digitized half | 6.2 to 5.1 | 339 to 145 | 0 | 0.000 | 7,233 to 6,111 | 13 |
| TS-13D, digitized half | 7.5 to 6.5 | 229 to 109 | 1.0 to 0 | 0.000 | 10,293 to 8,878 | 12 |
| TS-13E, digitized half | 5.3 to 4.9 | 99 to 33 | 0 | 0.000 | 3,108 to 2,767 | 5 |
| Traced pear | 6.4 to 6.5 | 40 to 24 | 0 | 0.000 | 5,517 to 4,689 | 2 |
| Traced dog on turf, 6 threads | 8.4 to 5.5 | 436 to 178 | 11.8 to 0 | 0.025 to 0.017 | 11,382 to 10,514 | 69 to 71 |
| Traced dog on turf, 9 threads | 8.5 to 5.5 | 480 to 206 | 11.0 to 0 | 0.081 to 0.038 | 12,238 to 11,489 | 79 to 80 |
| Traced running dog, 6 threads | 7.9 to 6.1 | 511 to 184 | 6.5 to 0 | 0.025 to 0.000 | 12,427 to 11,550 | 60 to 58 |
| Traced running dog, 9 threads | 8.0 to 7.0 | 629 to 280 | 6.2 to 0 | 0.000 | 14,380 to 13,259 | 83 to 74 |

The overlaps alone left the dogs 363, 378, 412 and 491 mm² at 3 fills or more. With the base layer in 3
layers and the rest as it is now, they have 268, 320, 292 and 400.

The step's aims were no place at 6 fills or more and half the area at 3 or more on each dog, with the
cover as it was. No picture has a piece of 1 mm² or more at 6 fills or more, the smallest that the measure
counts, though single spots of 4 pictures measure 6.1 to 7.0. The dogs have 55 to 64 % less area at 3
or more. The cover is as it was by its check. No gap lies where the base layer isn't under it, and the bare share is lower or as it was. The dog on turf in 9 threads has one sliver of 0.18 mm², and the running dog in 6 lost its one. What is worse:

- The dog on turf in 9 threads changes thread 13 times where it changed 12, and the step's aim was no more
  than before. It was traced, by sewing each of that dog's 19 crowded parts as a column again. The dog
  changes thread 12 times with the parts of 2 grey shapes at the top of the picture as columns, which sew
  3.9 and 2.7 mm of one rail to a millimetre of the other. It also does with 2 parts of a tan and a pale
  grey shape lower down as columns, 4.9 and 5.9 mm to a millimetre. One part alone doesn't do it. The
  cause is rule 2 of the [sewing order](#sewing-order): a line sews after the fill it borders, and a line
  beside a column sews when its thread does. Each part that became a
  fill makes the lines of other threads that touch it wait for its thread. The search for the fewest
  changes is exact, so with these fills every order within rules 1 and 2 has 13 or more. The twelfth change comes
  back only by sewing 2 crowded columns again, or by letting a line sew before a fill it borders, which
  M13.22 measured: more stitches astray and more slivers. Neither was done.
- The dogs' satin columns are fewer: 10, 13, 6 and 11 where they were 25, 31, 21 and 28. The parts that
  sewed as short bent columns sew as flat fills, which changes the look of the fur close up.
- 16 stitches of the dogs show on another colour for 1 mm or more, where 14 did. Each of the new ones was
  traced: a row of the area sewn first lies on the part of a running line 0.5 to 0.8 mm wide, beside the
  line's thread, where the area beyond lay before. No cheap change takes them away. The line's thread is
  0.4 mm wide and its part wider, so a row of one of the 2 areas lies on the part beside the thread, or
  the fabric shows there. With both areas sewing the part, as before the step, the dogs kept all but 11
  to 16 % of their thick area, and with neither sewing across it the running dog left 0.184 % bare once
  pulled in.
- The dog on turf leaves 2 and 1 more jump threads. One is a jump inside an outline in 2 pieces. The others
  are moves that were under the 3 mm an element's end may lie from the next start, and are 3.0 to 4.0 mm
  now that an end moved by up to 1 mm.
- Once the thread is pulled in by 0.4 mm at every turn, 3 of the dogs leave 0.1 to 0.7 mm² more of their
  area with no thread at all on it, of 1,640 to 1,940 mm², and one 0.15 mm² less.
- A fill's base is thinner. Whether a fill lies flat on the fabric over one layer is for MC-8 to sew: nothing
  here measures it.

A straight edge between 2 fills that crosses their rows still measures about 2.9 fills by itself: the
upper fill 1.0, the ends of its rows 0.25, the overlap 1.0, the ends of its rows 0.25, the base layer 0.1
and the outline 0.25. The 178 to 280 mm² that the dogs keep at 3 fills or more are such edges with a column or a third shape beside them. 2 parts could still go. One is the outline along an edge between 2 areas, which the owner chose. The other is the overlap itself, which costs cover. The demo's badge was
not measured again.

### On black fabric

A preview is drawn on unbleached cotton or on white. `Settings::on` of the renderer names another fabric
colour, a docs shot names it with its `fabric` key, and the user page of automatic digitizing shows the
pear and the dog on black fabric beside cream (`REQ-RND-003`). The realistic style's shading is checked on
black, where a shadow drawn darker than the fabric can't show.

Cover isn't all that black fabric asks. A third photo of the owner's dog, in glasses, traced in 12 threads
on 2026-11-10 and no fixture yet, loses its black tail and chest and the top of its head on black fabric:
black thread on black fabric, with nothing to outline it. The measures here don't read that, and it isn't in a step
yet. The same trace fails the older measure, with a bare patch of 1.74 mm² in the glasses.

### Settings of the cover

The base layer's 4 settings, the 2 limits of the measure, the overlap and the pull compensation are rows
of the [Settings](#settings) table.

The base layer lies 0.4 mm inside the design's outline. The step first built it 0.6 mm inside, the pull
compensation and half a thread. 5 of the dogs' 19 slivers then reached into that band, where nothing lay
under them. The owner's decision of 2026-11-14 is 0.4 mm, on 3 grounds. Ink/Stitch's contour underlay of a
satin column lies 0.4 mm inside each rail by default (`contour_underlay_inset_mm`). A digitizing guide's
worked example puts its edge run 0.4 mm inside the edge
([Embroidery Legacy](https://embroiderylegacy.com/?p=29571), as the owner's research of that day read
it). And it is our pull compensation: the base layer lies as far inside the outline as a column sews
past it. Ink/Stitch's Knockdown Fill does nothing of its own at its edge, and its documentation doesn't give an
offset for a global underlay. The value is StitchCraft's own.

Measured at both offsets, the base layer's thread lies nowhere outside the design. The table gives how
much of it no top thread covers within 0.4 mm of the outline, on a shape of another colour than its own:

| Picture, digitized | At −0.6 mm | At −0.4 mm | Of that in the outermost 0.2 mm | Nearest the outline at −0.4 mm |
|---|---:|---:|---:|---:|
| Traced pear | 0 | 0.02 mm² | 0 | 0.21 mm |
| Traced dog on turf, 6 threads | 0.06 mm² | 0.55 mm² | 0.13 mm² | 0.05 mm |
| Traced dog on turf, 9 threads | 0.07 mm² | 0.70 mm² | 0.18 mm² | 0.05 mm |
| Traced running dog, 6 threads | 0.02 mm² | 0.15 mm² | 0.06 mm² | 0.10 mm |
| Traced running dog, 9 threads | 0.02 mm² | 0.23 mm² | 0.04 mm² | 0.18 mm |

These are places that the top thread leaves bare at either offset: the bare shares are the same. At
−0.6 mm fabric shows in them, and at −0.4 mm the base layer's thread. The dogs' stitches astray, moves
and jump threads are the same at both, and 3 of the 5 slivers still reach into the band. The measure
paints each stitch where the plan puts it, so it can't tell how far real thread pulls a shape's edge in,
and whether the base layer then shows beside it. MC-8 sews the dog on black fabric with the base layer
on and off to see that, and to confirm the defaults
([machine testing](../plan/machine-testing.md)). A dark base layer under a white fill may show through
it, which only a sew-out tells.

## How thick a design sews

The owner asked on 2026-11-15 whether so many overlaps would be too thick for the needle after several
colours. Her machine is a Brother PE800 with one needle. A design digitized automatically puts thread on
thread on purpose. Each area reaches 1 mm under the shapes sewn after it, and the base layer lies under
all of them. An outline, when the design sews one, lies on a fill's edge. The measure of the bare fabric counts covered pixels and
can't tell 1 layer from 6. So a finished plan is measured for its thickness too
(`stitchcraft_plan::thickness`, `REQ-AUTO-060` to `REQ-AUTO-062`).

### The 2 measures

A plan is measured where the machine sews it, each needle point on the machine's grid of 0.1 mm. The
engine's own positions have digits that no file shows. Measured on those, the warning of VectorCraft's
export differed from the one `stitch export` gives for the same file (the compatibility gate on
2026-11-16). The grid is laid from the plan's first needle point (M13.35). The same stitches measure
the same wherever they lie. On the machine's own grid, a test sheet drawn on a page measured otherwise
than its machine file, and the last pass ([The last pass](#the-last-pass)) changed the two otherwise. A coordinate is rounded to a millionth of a millimetre first. Shapes drawn on
round numbers put needle points half way between 2 points of the grid, and without that such a point
goes to one or the other by the last digits of its position.

Both are sums over a window of 1 mm², on square cells 0.5 mm wide. Each cell is 10 pixels of the grid
that the bare fabric is measured on, so the 2 pictures can be laid on each other. The measures have a
value at each spot, a corner of the cells, whose window is the 4 cells that meet there. Spots so lie
half a window apart. The cells are
this coarse because planning measures every design. On cells of 0.2 mm the measure took 80 % as long as
planning a test sheet, and VectorCraft stopped the export of TS-13A for the instructions it ran. On these
cells it takes 2 to 3 %.

**Thread, in fills.** Every stitch is a ribbon of thread 0.4 mm wide, whatever its role, since the needle
goes through underlay and travel stitches as it goes through top stitches. A window's thread is the
length of the ribbons in it, times 0.25 mm, over its area. A fill with rows 0.25 mm apart, which is
StitchCraft's fill by default, so measures 1, and the number reads as "this many fills deep". A satin
column measures 1.25, because it lays 2 stitches every 0.4 mm. The base layer measures 0.3.

The sources count objects on each other, and a fill is the object here. Counted in layers of thread, one
of our fills is 1.6 layers, because its rows lie closer than a thread is wide. The fill that the sources
call standard has rows 0.38 to 0.4 mm apart, so ours is the denser one. To read a number of this page in
layers of thread, multiply it by 1.6.

**Needle points per mm².** Each stitch's needle point counts once. A lock's are left out, since every
design ties its locks on purpose. This measure finds what the first one misses: short stitches and turns
bunched in a small patch, which cut the fabric and break thread on their own.

A square 15 mm a side that the engine fills 1, 2, 3 and 6 times measures as many fills 2 mm inside its
edge: 1.03 times the layers on average, since the needle also travels under its rows, and 0.89 to 0.97
times at the least.

### The limits

| Measure | Warning | Failing | Where the number is from |
|---|---:|---:|---|
| Thread, in fills | 3 | 6 | Brother's troubleshooting table names "three or more overlapping stitches" as a fault of the design. Embrilliance's density map turns red at 6 layers |
| Needle points per mm² | 15 | 30 | Our own. The rows of one fill end 5 to a square millimetre along its edge, so these are the edges of 3 and of 6 fills on each other |

A place counts when 1 mm² of it in one piece is at the limit or over it: 4 spots beside each other, each
standing for the quarter of a square millimetre round it. 3 rows that cross in a point are no fault.

No source publishes a measured limit for a home machine, and the sources disagree. One vendor wants all
layers together to be about 1 fill, a retailer finds 2 full designs on each other too much for a PE800,
Brother says 3 and Embrilliance 6. So the 4 values are starting values for a 75/11 needle and
40-weight polyester thread. They are settings of the shape, `auto_thick_warn_fills` and the 3 beside
it. A machine profile's values change only with a sew-out report, and these have none yet.
[MC-8](../plan/machine-testing.md) sews a ladder of squares to confirm them or change them.

### The warning

Planning measures a design that has an automatic shape, against the limits of its lowest one. When a
place is at a warning's limit or over it, planning says `SC-W0913` once, and the design is written all
the same. No design is refused for its thickness until a sew-out shows where one fails. The message gives the most of
each measure and where, the 3 shapes with the most thread at the thickest spot, and the areas at the 4
limits. A design drawn by hand is not measured: its author chose every layer.

`stitch preview --style thickness` draws the heat map of a machine file (`REQ-RND-004`). Thread under 2
fills is grey, and thread of 2 fills or more yellow. From 3 fills it is orange, and from 6 red.

The user page shows [the running dog's](../user/stitches/automatic.md#how-thick-it-sews).

A plan is measured once, and the measure keeps the stitches it went through. What lies at the
thickest spot is found from those: only a stitch whose box reaches within 1 mm of the spot is followed
again. A limit above the largest value has nothing over it, and the spots are not searched for it.
Counted in processor instructions on the sheets TS-13A and TS-13B, the measure takes
1.9 % of planning. Before these 3 savings it took 2.7 %.

### What today's designs measure

Measured on 2026-11-16 as `--auto` sewed each picture before M13.30, with the base layer and the outline
on. What M13.30 changed is measured in [Less thread where areas overlap](#less-thread-where-areas-overlap).
The last 2 pairs of columns give the thickest spot and the area at 3 fills or more with one of them off.

| Picture | Design, mm² | Thickest, fills | At 3 fills or more, mm² | At 6 or more, mm² | Most needle points | Base layer off | Outline off |
|---|---:|---:|---:|---:|---:|---:|---:|
| Lettered patch | 1,813 | 5.8 | 142 | 0 | 14 | 6.0, 156 | 5.8, 142 |
| Alpine Night, the demo's badge | 1,963 | 6.2 | 161 | 0 | 20 | 7.5, 194 | 5.2, 142 |
| TS-13A, digitized half | 5,756 | 5.7 | 124 | 0 | 22 | 5.8, 163 | 5.5, 115 |
| TS-13B, digitized half | 2,610 | 6.6 | 120 | 0 | 21 | 7.4, 96 | 5.5, 110 |
| TS-13C, digitized half | 2,201 | 6.2 | 339 | 0 | 19 | 7.0, 355 | 6.7, 343 |
| TS-13D, digitized half | 3,524 | 7.5 | 229 | 1.0 | 13 | 8.3, 347 | 7.5, 223 |
| TS-13E, digitized half | 845 | 5.3 | 99 | 0 | 11 | 5.2, 118 | 5.3, 97 |
| Traced pear | 2,252 | 6.4 | 40 | 0 | 17 | 6.4, 82 | 6.0, 28 |
| Traced dog on turf, 6 threads | 1,639 | 8.4 | 436 | 11.8 | 34 | 10.7, 560 | 8.2, 369 |
| Traced dog on turf, 9 threads | 1,639 | 8.5 | 480 | 11.0 | 31 | 9.1, 633 | 8.6, 403 |
| Traced running dog, 6 threads | 1,944 | 7.9 | 511 | 6.5 | 29 | 8.8, 682 | 7.9, 401 |
| Traced running dog, 9 threads | 1,944 | 8.0 | 629 | 6.2 | 28 | 9.2, 808 | 8.0, 516 |

The badge is measured on its PES file, and its area is that of a disc 50 mm across. The lettered patch
sews no outline, because its letters are satin columns.

On TS-13B the letters of one word measure differently, and that is what sews. In the word 15 mm tall
the base layer has a piece under the M, the A, the T and the H, whose strokes sew as 2 to 4 columns
each. It has none under the C, which sews as one column with an underlay of its own. A piece of the
base layer is the letter moved in by 0.4 mm, and it is kept only where a disc as wide as the base layer's
rows are apart, 2.6 mm, fits on it. The strokes are 2.7 mm wide. A disc fits only where 2 strokes
join, and the C is one stroke with no join. The smaller words
have no base layer, and a letter is thick there where its columns meet: the M has 4 columns and 3
joins. In the word 4 mm tall the M and the A still sew as columns, and the other letters as lines of
running stitch.

Every standard picture is over the warning's limit somewhere, and a quarter to a third of each traced
dog is. Each part of the stack has its own share:

- **The 1 mm that areas reach under their neighbours causes most of it.** Without any overlap the running
  dog in 9 threads has 155 mm² at 3 fills or more where it has 629 mm², and the other dogs 141 to
  144 mm². Among small patches the overlaps of several neighbours lie on each other.
- **The base layer makes a design thinner.** It replaces an underlay under each fill, which is more
  thread: with the base layer off the dogs have 28 to 34 % more area at 3 fills or more.
- **The outline adds 18 to 28 %** on the dogs. It lies on the edge, where 2 fills already do.

At a dog's thickest spot 5 layers or more lie on each other, each with 0.3 to 3.2 fills: small patches
whose overlaps cover each other whole. The dogs also have 46 to 104 mm² with 15 needle points or more to
a square millimetre, in patches 1 to 2 mm across whose rows are short and whose turns lie beside each
other.

The [runs in place of jumps](#runs-in-place-of-jumps) lie under later fills and add little: 1 to
12 mm² at 3 fills or more on a dog, 2 % at the most. Each dog's thickest spot stays within 0.1 of a fill.

### What would take thread away

This step measures and changes nothing that sews. Each row was measured on the running dog in 9 threads
by changing one setting, and the bare share is what the top thread leaves of the design
([covering the fabric](#covering-the-fabric)), 0.000 % today:

| Change | At 3 fills or more, mm² | Bare, % of the design | Stitches |
|---|---:|---:|---:|
| As sewn today | 629 | 0.000 | 14,380 |
| No outline (`auto_fill_outline` off) | 516 | 0.000 | 13,076 |
| No overlap (`auto_overlap_mm` 0) | 155 | 0.409 | 12,849 |
| Overlap of 0.5 mm | 437 | 0.012 | 13,638 |
| Smallest area 4 mm² (`auto_min_area_mm2`) | 567 | 0.000 | 13,671 |
| Smallest area 10 mm² | 323 | 0.138 | 11,321 |

Halving the overlap takes away 30 % and still covers. Merging patches under 10 mm² takes away 49 %, and
changes the picture. 2 more ways needed code. One cuts an area's overlap away where 2 fills already lie
under it, as other embroidery software removes hidden stitching. The other sews the overlap with rows
twice as far apart. Fills with rows 0.4 mm apart would lower every number of this page by 37 %. M13.30
built and measured both, with the bare-fabric measure checking each change
([Less thread where areas overlap](#less-thread-where-areas-overlap)).

### The thickest spot

M13.30 left thickest spots of 5.5 to 7 fills, and the owner read them as what will sew: "5.5 to 7 just
won't work". She is right to. The area at 3 fills or more says how much of a design is thick, and the
thickest spot says whether any of it is too thick to sew. A report gives both. M13.35 aims at no spot of
1 mm above 4 fills on any test design (`REQ-AUTO-073`), and its cases list each design still above that
with its thickest spot, a list that only shrinks.

Each design's 10 thickest spots were split by element and by kind of thread on 2026-11-16. On the sheets
and the pear one shape lays 2 to 4 fills by itself at the thickest spot, in 2 out of 3 such spots or more:

- **A satin column fans.** Its stitches keep their spacing on the outside of a corner or a bend, and on the
  inside they meet. TS-13A's blue strip sews 25 stitches from 5 mm of one rail to 1 mm of the other, 3.6 to
  4.6 fills by itself. Where 2 columns of one letter join, both sew whole, 2.3 and 1.9 fills in the M.
- **Lock stitches pile up.** A half stitch lock is 4 stitches of up to 1 mm, about 1 fill where it lies.
  The pear's stem ties in and off at one end, 1.9 fills of locks, and at TS-13B's thickest spot a fill, its
  outline and a line lock, 2.75 fills.
- **A narrow column walks its middle twice.** The centre walk goes there and back, 0.5 fill under the
  column's 1.25.

On the traced dogs every kind adds a little at the peaks. At their spots of 4 fills or more a column gives 1.0 to 1.6
fills, the overlap 0.85, a fill's rows 0.45, its turns and travel 0.6 to 0.8, the outline 0.3 to 0.4 and
the locks 0.2 to 0.45. In a third of those spots 5 elements or more meet, each with 0.3 fill or more.

2 rules for single shapes come first. An automatic column moves its crowded needle points in
(`REQ-AUTO-071`): a point closer than 0.35 mm to the last one left on its rail moves in by 25 % of its
stitch, the next by 50 %, as digitizing programs shorten stitches in a sharp turn. A rail's points lie
0.4 mm apart on a straight column, so no straight column loses any of its width. A larger distance takes
more thread away only because it thins every column's edges. And everything digitizing makes locks in
steps as long as the reference machine's shortest stitch, 0.3 mm (`REQ-AUTO-072`), as advised for small
lettering. Steps of 0.4 mm did nearly as well on the sheets, but where 3 areas meet at a corner the first
stitch is short, the half stitch there is short too, and 0.4 mm steps made the corner 4.15 fills where it
was 3.76. Measured with the case's plans:

| Design | Thickest, fills, M13.30 | Now | At 3 fills or more, mm², M13.30 | Now |
|---|---:|---:|---:|---:|
| Lettered patch | 5.31 | 5.04 | 107 | 75 |
| TS-13A, digitized half | 5.56 | 4.28 | 78 | 26 |
| TS-13B, digitized half | 5.65 | 4.29 | 49 | 20 |
| TS-13C, digitized half | 5.07 | 4.51 | 145 | 108 |
| TS-13D, digitized half | 6.46 | 5.17 | 109 | 69 |
| TS-13E, digitized half | 4.86 | 3.65 | 33 | 15 |
| Traced pear | 6.45 | 5.01 | 24 | 23 |
| Traced dog on turf, 6 threads | 5.54 | 5.24 | 178 | 161 |
| Traced dog on turf, 9 threads | 5.54 | 5.23 | 206 | 188 |
| Traced running dog, 6 threads | 6.05 | 5.21 | 184 | 170 |
| Traced running dog, 9 threads | 7.01 | 6.53 | 280 | 275 |

Stitches and jump threads stay as they were. The bare share of the dog on turf in 9 threads grew from
0.00038 % to 0.00041 %, and one jump thread of TS-13E's digitized half is no longer sewn down, neither traced. The plans here sew each design where it draws its shapes,
so a spot can differ from `stitch plan --auto`'s by up to 0.8 fill, as the 0.5 mm cells fall.

Alone, the locks took TS-13B from 5.65 to 4.58 fills and the pear to 5.35, and the columns took TS-13A to
4.73. No outline round fills halves the dogs' area at 3 fills or more and moves their thickest spots
little. It isn't built.

#### The last pass

The rules of single shapes can't bring the dogs to the cap: 5 to 8 elements meet at their thickest spots,
each with a little thread, and no rule of one shape knows the sum: the finished plan does. So a plan of a design digitized
automatically is measured once, and where a spot sews above 4 fills a last pass takes thread
(`REQ-AUTO-074`, `auto/thin.rs`). Taking a needle point out of a run takes no thread away, since its 2
stitches become one as long, and cutting a run leaves a jump thread. Thread leaves a spot only where the
stitching doubles back on itself, or where the end of a row can move. The pass looks at the stitches
within 2 mm of a spot above the cap and takes, in this order:

1. travel that goes out and comes back within 0.1 mm of where it was 2 needle points before: those 2
   points go, and the stitch before them runs on to the one after;
2. a lock's stitches beyond one back and forth, where it has 4 or more;
3. the base layer's row ends, where a row turns to the next within 1 mm and the 2 rows run nearly
   opposite ways: the end and the next row's start move back along their rows by the overlap
   (`auto_overlap_mm`), or by half of it;
4. the other shapes' top row ends, moved the same way.

A row's end moves only where it is hidden at both places: the top stitches of the elements sewn after it
lay 1 fill or more at its old and new points. The pass measures them from the plan's end, so that thread
taken from a later shape no longer hides an earlier one. The pass skips a change that would leave
a spot it lays thread in thicker than both the cap and what it was. It skips one that would make a stitch
shorter than the machine's shortest (a lock's 0.2 mm into or out of a lock point) or than the turn it
replaces, or longer than the machine's longest. It keeps every top stitch that shows, and it doesn't make
a jump thread. It keeps the measure up to date stitch by stitch (`Thickness::add` and `remove`) and hands
it to `SC-W0913`, so that the design is measured once. `SC-I1119` says what it took.

#### A narrow column walks once

What the last pass left at the thickest spots was mostly satin columns: a column's zigzag of 1.3 to 2.8
fills and its underlay of 0.6 to 0.8, over a fill's rows that run on under it. The pass can't take a
centre walk's way back without turning the column. So a column narrower than 2 mm, which has only a
centre walk under it, walks its middle once (`REQ-AUTO-075`, `center_walk_underlay_repeats` 1), as
digitizing guides give a narrow column one run down its middle. The walk then ends at the column's
far end, and the top stitches sew back to where it started. The order of a part's columns knows that: the
next column is the one with an end nearest the start of such a column. A shape that sets the repeats
keeps them. Measured with the cases' plans on 2026-11-17, the last pass in both columns:

| Design | Thickest, fills, last pass | Walked once | At 3 fills or more, mm² | Walked once | Jump threads | Walked once |
|---|---:|---:|---:|---:|---:|---:|
| Lettered patch | 4.73 | 4.33 | 78 | 58 | 7 | 8 |
| TS-13A, digitized half | 4.28 | 4.28 | 31 | 31 | 18 | 18 |
| TS-13B, digitized half | 4.02 | 3.99 | 31 | 24 | 43 | 42 |
| TS-13C, digitized half | 4.49 | 4.49 | 113 | 112 | 13 | 13 |
| TS-13D, digitized half | 4.17 | 4.17 | 79 | 79 | 12 | 12 |
| TS-13E, digitized half | 3.71 | 3.42 | 27 | 9 | 5 | 5 |
| Traced pear | 4.78 | 4.55 | 26 | 21 | 2 | 2 |
| Traced dog on turf, 6 threads | 4.91 | 4.53 | 172 | 164 | 71 | 73 |
| Traced dog on turf, 9 threads | 4.75 | 4.51 | 200 | 192 | 80 | 81 |
| Traced running dog, 6 threads | 5.15 | 5.15 | 190 | 184 | 58 | 58 |
| Traced running dog, 9 threads | 6.27 | 6.00 | 276 | 265 | 74 | 72 |

The designs sew 5 to 100 stitches fewer. A jump thread starts or ends elsewhere where a column now ends at
its start, so a dog has 2 more or 2 fewer. The lettered patch has one more, and one fewer that no later
stitch sews down. Where 2 columns of one shape join, the one that ends under the other reaches its pull
compensation, 0.4 mm, past the chord between them. Ending it at the chord took TS-13A to 4.15 and TS-13B
to 3.79 fills, and changed the dogs by 0.01 or less. The thread pulls such an end back by about as much,
which opens the join. The joins are left as they were.

A column that walks once starts at its start (`REQ-AUTO-086`, `start_at_nearest_point` off). Started at the
point of it nearest the needle, it ran along its middle under itself to its start before its walk, and
its top stitches came back to where it began. The run, the walk, the lock and its last stitches met at
one spot there. That spot moved with the shape sewn before: the lettered patch's M measured 4.32
fills at its V, and 4.48 when the A before it ended 0.4 mm further left (M13.36). Starting at its start,
the patch's thickest spot is 4.20 fills and the traced pear's 4.20, where they were 4.33 and 4.55. With the
rules of [holes and edges](#holes-and-edges-of-columns) too, the traced dogs' are 4.53, 4.49, 5.07 and 5.63 fills, where they were 4.53, 4.51, 5.07 and 5.89, and their area
at 3 fills or more is 130.5, 156.5, 142.0 and 207.75 mm², where it was 132.25, 160.0, 146.25 and 209.75. It
costs jumps. The dogs in 9 threads leave one jump thread more each. TS-13B's digitized half leaves 44 where
it left 42, as the 6 mm word's A and T jump to the far end of a column, and of TS-13C's 13 one is sewn down
where 5 were.

#### The outline under a later fill

An outline sews through a hidden stretch of its edge up to 25 mm ([an outline round a fill](#an-outline-round-a-fill)),
under the shape that hides it. Where a fill sewn later hides the stretch, the run lies along the edge where
that fill's rows end over the area's overlap, beside the fill's own outline. The dogs' thread piles up
along those edges between 2 patches. The owner chose on 2026-11-16 that an outline's thread may be left out only where
a fill sewn later covers it, so that what shows doesn't change. So a stretch that a later fill hides, in
part or whole, is sewn through only when it is 12 mm or shorter (`REQ-AUTO-076`, `FILL_BRIDGE` in
`auto/neighbours/outline.rs`). Each stretch left out costs a jump, and a short one lays little thread.
Measured with the probe's plans on 2026-11-17:

| Dog | Thickest, fills, walked once | Now | At 3 fills or more, mm² | Now | Jump threads | Now |
|---|---:|---:|---:|---:|---:|---:|
| On turf, 6 threads | 4.53 | 4.53 | 164 | 154 | 73 | 74 |
| On turf, 9 threads | 4.51 | 4.51 | 192 | 181 | 81 | 84 |
| Running, 6 threads | 5.15 | 5.07 | 184 | 174 | 58 | 61 |
| Running, 9 threads | 6.00 | 5.89 | 265 | 234 | 72 | 74 |

The sheets, the pear and the lettered patch don't change. Leaving out every stretch under a later fill
took the dogs' area at 3 fills or more down by 12 to 19 %, for 3 to 11 jump threads more each, and took
the dog on turf's thickest spot up to 5.09: the last pass could no longer move row ends that the stretch
had hidden. With no outline at all, which would change what shows, the dogs' thickest spots are still
4.50 to 5.62.

#### The thickest spots left

The cap of 4 fills isn't met. TS-13B and TS-13E's digitized halves are under it, at 3.99 and 3.42. The
other sheets, the pear and the lettered patch sew 4.17 to 4.55 fills at their thickest spots, where 2
satin columns of one shape join, where a column wider than 2 mm lies over a fill's rows, or, on TS-13C,
where 7 elements meet. The traced dogs sew
4.51 to 5.89. At their thickest spots 5 to 9 elements meet, each with 0.3 to 2 fills: columns that join,
the rows of the fills under them (the rims), the turns and travel of small fills, and the base layer. No
rule tried here takes enough of that without opening fabric or showing less of an outline. The lowest
honest cap now is about 4.5 fills on the sheets, the pear and the lettered patch, and 6 on the dogs.

With the ragged thin lines merged into their neighbours (M13.18, [Neighbours](#neighbours)), the traced dogs
sew 4.69, 4.80, 4.62 and 5.63 fills at their thickest spots, and 141.5, 153.25, 130.5 and 187.5 mm² at 3
fills or more. The dogs on turf sew thicker, at new spots where a satin column or a fill lies over 3 or 4
other fills.

#### The cap at the end of planning

Sewing a picture's thin lines as lines (M13.44) and sewing details last (M13.40) change the order the
blocks sew in. A changed order moves where fills start and end and which shape reaches under which,
so a spot far from the change can pile up thread. No rule about a line or a detail foresees that. Each
step tried planning twice and keeping the thinner plan, which doubled a traced photo's export. The owner
asked on 2026-11-18 for one general cap at the end of planning instead, which every design meets whatever
its shapes and their order (`REQ-AUTO-099` to `REQ-AUTO-103`). Its value is the setting
`auto_thick_cap_fills`, 4 fills by default, since MC-8's sew-out will set the real limits.

**Why the last pass stops.** The thread at every spot above 4 fills was split on 2026-11-18, in pieces of
0.2 mm, by kind. A piece of top thread counts as hidden where the top stitches of the shapes sewn after it
lay a fill or more:

| Design | Spots above 4 | Top that shows | Top under later shapes | Base layer | Locks | Travel |
|---|---:|---:|---:|---:|---:|---:|
| Badges | 3 | 49 % | 45 % | 3 % | 3 % | 0 % |
| TS-13A, digitized half | 5 | 39 % | 57 % | 4 % | 1 % | 0 % |
| TS-13C, digitized half | 13 | 36 % | 60 % | 3 % | 0 % | 0 % |
| TS-13D, digitized half | 2 | 49 % | 46 % | 2 % | 3 % | 0 % |
| Traced pear | 4 | 43 % | 49 % | 4 % | 3 % | 0 % |
| Traced dogs | 19 to 82 | 40 to 45 % | 47 to 54 % | 3 to 6 % | 1 to 3 % | 0 to 2 % |
| 4 other traced photos | 142 to 259 | 40 to 52 % | 43 to 54 % | 3 % | 1 to 2 % | 1 % |

No other underlay lies there. The last pass takes travel that doubles back, locks beyond one back and
forth, and row ends. Of the hidden top thread it takes only a row's end where the next row starts within
1 mm, moved back along the row. A satin column's stitches cross the column, each the next one's turn, so a
column that ends under another column of its shape, or under a later shape, keeps all of them. The middle
of a row it can't shorten. It never takes top thread that shows, and part of that covers nothing that other
stitches don't already cover: a column's points crowded in a bend or a join, a fill's short rows bunched at
a narrow tip.

**One rule.** A needle point taken out of a run joins its 2 stitches into one. That takes thread only where
the run turns at the point, and it doesn't leave a jump thread. So the cap looks at the finished plan alone, its
stitches with their roles and elements, and its measure. Where a spot sews above the cap, it takes needle
points out of the stitches that lay thread there, when other stitches already cover the places those stitches covered. A
stitch covers the places within half a thread, 0.2 mm, of it, as the bare-fabric measure paints it. The
kinds go in this order, each through the plan from its end:

| Kind | A needle point goes when | Why it comes here |
|---|---|---|
| Underlay and travel | top stitches sewn after it cover all of the joined stitch | Nobody sees it, and the thread above it already lies flat on the fabric |
| A shape's top under later shapes | the top stitches of shapes sewn after it cover the places its 2 stitches covered | Nobody sees it either, but it is the overlap that keeps fabric from showing between 2 shapes when the fabric pulls |
| A shape's own top | its own other top stitches and those of later shapes cover the places its 2 stitches covered | It shows, so it goes last, and only where it is doubled |

Through the plan from its end, a later shape loses its thread before an earlier one is judged under it, as in
the last pass. The joined stitch is no longer than the longest stitch of its element and kind, top or not,
so no stitch is longer than its shape already sews, and no shorter than the shorter of the 2 or the
machine's shortest. A lock's needle points, and the one after a lock, stay. A point stays when taking it would
leave a spot that the joined stitch lays thread in thicker than both the cap and what it was. Each needle
point the cap leaves is one of the plan's, in the same order. The jumps, the trims and the thread changes
are those of the plan. The cap reads roles, elements and places, whatever the picture shows. It takes
needle points only of the elements that digitizing made. A shape with settings of its own keeps the
stitches they give it, and its stitches cover and lay thread like the others. A test sheet's half as drawn sews so.
Where a spot stays above the cap, `SC-W0913` says "Above the cap." after what is stacked there. `SC-I1119`
says what the last pass took and then what each kind of the cap's gave.

The cap is `stitchcraft_engine::auto::cap`. It goes through the needle points whose stitch in or out comes
within 1 mm of a spot above the cap, and judges cover by the top stitches within 2.5 mm of such a spot,
found once. Places farther off count as not covered, and that keeps the points of a long stitch. Each kind goes
through the plan up to 4 times, and stops once a time takes nothing or no spot is above the cap. It keeps
the last pass's measure up to date with the pass's own helpers, so the design is still measured once. It
works on copies. When it runs out of work, the plan stays as the last pass left it, and `SC-I1119` says
so.

Where a satin column's points crowd in a bend, the third kind may take one. The stitch left then crosses
part of the column only: on TS-13C's face outline it lies 14° from square, where `REQ-AUTO-017` holds the
columns' rungs to 10°. Its places were covered twice over, so no colour changes, and MC-8 looks at it.
That case plans the columns with the cap at 50 fills, as digitizing lays them.

A joined stitch no longer than the longer of its 2 left the dogs at 4.02 to 4.18 fills: the next row's start
lay too far for the stitch into a turn. With the element's own longest stitch as the bound they come to 4.00.

**Measured** with a prototype on the plans of the cases, on 2026-11-18. The jump threads, the thread
changes, the slivers and the likeness stay as they were on every design. The cap as built measures the
same but for 3 designs, which differ a little: TS-13C's digitized half comes to 3.96
fills and 95.75 mm² at 3 fills or more, the badges to 19.50 mm² and TS-13D's half to 50.75 mm².

| Design | Thickest, fills | With the cap | At 3 fills or more, mm² | With the cap | Bare, % | With the cap | Needle points taken |
|---|---:|---:|---:|---:|---:|---:|---:|
| Badges | 4.40 | 3.99 | 23.75 | 19.00 | 0.0080 | 0.0080 | 3 |
| Lettered patch | 4.00 | 3.98 | 40.75 | 40.25 | 0.0029 | 0.0029 | 1 |
| TS-13A, digitized half | 4.28 | 3.99 | 26.00 | 23.75 | 0.0118 | 0.0133 | 7 |
| TS-13C, digitized half | 4.49 | 3.95 | 102.50 | 96.50 | 0.0000 | 0.0000 | 10 |
| TS-13D, digitized half | 4.23 | 3.97 | 51.25 | 50.50 | 0.0004 | 0.0004 | 3 |
| Traced pear | 4.20 | 3.86 | 13.75 | 13.00 | 0.0003 | 0.0003 | 7 |
| Traced dog on turf, 6 threads | 4.69 | 4.00 | 141.50 | 137.25 | 0.0102 | 0.0102 | 67 |
| Traced dog on turf, 9 threads | 4.80 | 4.00 | 153.25 | 150.75 | 0.0101 | 0.0101 | 57 |
| Traced running dog, 6 threads | 4.62 | 4.00 | 130.50 | 129.00 | 0.0004 | 0.0004 | 33 |
| Traced running dog, 9 threads | 5.63 | 4.00 | 187.50 | 176.50 | 0.0050 | 0.0050 | 187 |
| Traced magnolia | 5.54 | 4.00 | 462.75 | 442.00 | 0.0042 | 0.0054 | 439 |
| Traced coleus | 10.46 | 4.22 | 894.50 | 853.25 | 0.0067 | 0.0057 | 1059 |
| Traced goose | 5.60 | 4.00 | 499.50 | 482.75 | 0.0068 | 0.0068 | 473 |
| Traced building | 5.88 | 4.00 | 225.75 | 221.50 | 0.0002 | 0.0012 | 123 |

TS-13B and TS-13E's digitized halves and the other test pictures are under 4 fills already, and the cap
leaves them as they were. Of the 4 traced photos, which aren't in the repository, the coleus keeps one spot
at 4.22 fills, where 9 elements meet over a run of travel that no later top stitch covers end to end. The
cap takes 1 to 2 % of a photo's stitches, and fewer of the others'. The bare share rose on TS-13A, the magnolia
and the building by about 0.001 % of the design, where a joined stitch's ribbon misses a corner that the pixels
of the bare-fabric measure catch.

Without the third kind, the badges stay at 4.40 fills, TS-13A at 4.16 and the pear at 4.16. What holds
those spots is a shape's own top thread laid twice over the same places.

**With the orders the later steps make.** The cap has to work for whatever order a step chooses. So each
design was planned again with the thin lines kept at a likeness of 0.85 (M13.44, `auto_min_likeness`, its
branch at `16d729db`) and with the details sewn last and no guard (M13.40, at `ed6b6421`), each from its
branch before it merged:

| Design | Thin lines kept, fills | With the cap | Details last, fills | With the cap |
|---|---:|---:|---:|---:|
| Badges | 4.40 | 3.99 | 4.40 | 3.99 |
| Lettered patch | 4.20 | 4.00 | 4.20 | 4.00 |
| TS-13A, digitized half | 4.28 | 3.99 | 4.28 | 3.99 |
| TS-13B, digitized half | 3.99 | 3.99 | 3.99 | 3.99 |
| TS-13C, digitized half | 4.49 | 3.95 | 4.49 | 3.95 |
| TS-13D, digitized half | 4.17 | 3.99 | 4.17 | 3.99 |
| Traced pear | 4.20 | 3.86 | 4.20 | 3.86 |
| Traced dog on turf, 6 threads | 4.57 | 4.00 | 5.49 | 4.00 |
| Traced dog on turf, 9 threads | 4.80 | 4.00 | 4.69 | 4.00 |
| Traced running dog, 6 threads | 4.62 | 4.00 | 5.03 | 4.00 |
| Traced running dog, 9 threads | 5.63 | 4.00 | 5.02 | 3.99 |
| Traced magnolia | 5.68 | 4.00 | 5.26 | 4.00 |
| Traced coleus | 7.39 | 4.12 | 10.13 | 4.00 |
| Traced goose | 7.03 | 4.01 | 5.64 | 4.07 |
| Traced building | 5.75 | 4.00 | 5.65 | 4.03 |

The jump threads, the thread changes, the slivers and the likeness stayed as each branch had them. With the
details last, the dog on turf in 6 threads rose to 5.49 fills and the running dog in 6 to 5.03, and the cap
brought both to 4.00. The goose, the building and the coleus kept a spot at 4.01 to 4.12 fills in one order
or the other.

**Which cap every picture meets.** 4 fills, on every design of the repository in each of the 3 orders. Of the
4 traced photos outside it, the magnolia meets it in each order and the others come within 0.22 fills.
With the cap at 3.5 the badges, TS-13A and D and the pear come under it, but the lettered patch stays at 3.70,
TS-13B at 3.74, TS-13C at 3.68 and the dogs at 3.61 to 3.98 (`sweep` of the prototype, thickest spots
only). At 3 the thickest spots stay where they were at 3.5, while the area at 3 fills or more falls to
10 to 20 mm² on the dogs, for 976 to 1,461 needle points fewer. That trade, and what it does to the bare share,
was not measured, and MC-8 decides it.

**What it costs.** Counted in processor instructions, the cap took 1.8 % of planning TS-13A's digitized half,
0.8 % of TS-13C's, 0.4 % of the pear's and nothing of TS-13B's, which has no spot above the cap. It looks only
at the stitches within 2.5 mm of a spot above the cap, found once, as the last pass does, and it takes the
last pass's measure. The prototype that made a cover of every top stitch, and found the spots again each
round, took 11 % on TS-13A.

**Decisions for the owner.**

- The cap's value: 4 fills by default. Every test design and the traced dogs meet it, and nothing lower is
  met by all of them. Recommended until MC-8 sews the ladder.
- The third kind takes a shape's own top stitches, which show, where its other stitches cover the same
  places. Without it 3 designs stay above 4 fills. Recommended, since what shows doesn't change colour and
  the bare share moves by 0.001 % at most.
- The traced coleus, goose and building keep a spot at 4.01 to 4.22 fills in one order or more, where no
  needle point may go by these rules, and `SC-W0913` names it. Recommended as it is: a rule for that one spot would be tuned to one picture.
- The last pass runs before the cap, as now. Its row ends move rather than go, which the cap can't do. Whether the
  cap alone would do as well is a measurement for the build phase.

## Likeness

Each merge of automatic digitizing takes a jump thread or a thread change away: a speck (`REQ-AUTO-008`)
and a thin line (`REQ-AUTO-090`) sew as part of their neighbour. A merge can also take what makes a picture
read, such as an eye or a stripe. The rules go by geometry and can't tell an eye from noise. The
likeness measure checks afterwards what the geometry kept (`REQ-AUTO-093`,
`stitchcraft_testkit::fabric::likeness`).

### The measure

The trace is the design as drawn, before anything merges, painted as [the bare fabric](#covering-the-fabric)
paints it at 0.05 mm on a grid round the design alone. The stitches are painted on the same grid: each pixel
has the colour of the last top stitch whose thread, 0.4 mm wide, covers its centre. Underlay and travel
stitches lie under the top stitches, as locks do, and are left out.

1. A colour's parts are its pixels that touch at a side or a corner.
2. A part counts when it is as large as the smallest area sewn by default, 1 mm², and a disc a thread across
   fits in it. Smaller, it is a speck, which the rules merge or leave out, and no stitch of its own sews it
   narrower.
3. A part's core is its pixels that a disc a thread across, centred on them, fits in.
4. A pixel is shown when the stitches have its colour within half a thread of it.
5. A part is kept when half of its core or more is shown.
6. A colour's likeness is the share of its parts kept, and the picture's is the mean of its colours'.

A share of matching pixels is ruled by the large patches. An eye of 1.44 mm² on a field 20 mm wide is
0.36 % of the picture, and its loss moves that share by less than the stitches' edges move it. Each part
counts once, whatever its size, and so the eye's loss halves the likeness of that picture of 2 colours. Each
colour counts once too, so the few dark parts of a face weigh as much as a colour's many pieces of shading.
A change that keeps every part keeps the likeness, however it moves the stitches.

The core leaves out a strip half a thread wide along a part's edge, where its neighbours' rows end. A strand
1.4 mm wide that a running stitch sews along its middle shows its colour on 0.8 mm of its core's 1 mm, and is
kept. The parts of the traced dogs are shown on less than a fifth or more than three fifths of their cores, so
the line at a half moves no count.

A traced photo has hundreds of bits of texture smaller than 1 mm². Counted, they ruled the measure as the large
patches rule a share of pixels: 476 of a traced magnolia's 972 parts that a thread fits in are such bits.
Parts that digitizing merges as specks show when the smallest area is raised.

### The pictures measured

Measured on 2026-11-17, digitized as main sews them, and with `auto_fill_uneven` off, which sews thin lines
as lines as before M13.18 ([Neighbours](#neighbours)). The photos marked * are local and not in the
repository, since their licences are not its own.

| Picture | Kind | Parts | Kept | Likeness | With thin lines as lines |
|---|---|---:|---:|---:|---:|
| The pear | a photo of a fruit | 5 | 5 | 1.000 | 1.000 |
| The lettered patch | a drawing | 8 | 8 | 1.000 | 1.000 |
| TS-13A to TS-13E, digitized halves | drawings | 16, 31, 11, 14, 18 | all | 1.000 | 1.000 |
| The dog on turf, 6 threads | an animal photo | 97 | 75 | 0.814 | 0.993 |
| The dog on turf, 9 threads | an animal photo | 111 | 83 | 0.779 | 0.947 |
| The running dog, 6 threads | an animal photo | 77 | 66 | 0.898 | 0.970 |
| The running dog, 9 threads | an animal photo | 100 | 88 | 0.921 | 0.983 |
| A magnolia * | a flower photo | 496 | 233 | 0.480 | 0.981 |
| A coleus * | a leaf photo | 841 | 395 | 0.454 | 0.986 |
| A greylag goose * | a bird photo | 342 | 167 | 0.483 | 0.989 |
| A castle * | a building photo | 191 | 81 | 0.434 | 0.932 |

The dogs lose strands of fur and shading, and the dog on turf the pale strip above its nose. The magnolia
loses its thin twigs and the pale rims of its petals, and the shadows along its branches. Each is a thin line,
which merges into the patch beside it. With the smallest area raised to 3 mm², more parts merge as specks,
and the dogs keep 0.764, 0.631, 0.768 and 0.773.

### The limit

The pictures in the repository keep a likeness of 0.75 or more (`REQ-AUTO-094`), and each case records its
picture's number, so that any change that takes a part away shows. The limit is the lowest likeness of those
pictures, the dog on turf in 9 threads at 0.779, rounded down to a twentieth. The traced dogs and the pear set
it among photos, and the lettered patch and the test sheets' digitized halves among drawings.

Before M13.44 the local flowers, bird and building kept 0.43 to 0.48, under the limit. Thin lines are half
of their parts, and the rule of `REQ-AUTO-090` merged them all. With their thin lines sewn as lines they keep
0.93 to 0.99. Merging now keeps the picture's likeness ([Merging by likeness](#merging-by-likeness)). A merge
of thin pieces (M13.38) would take more of them, and waits for M13.44.

The before and after sheet shows the share of detail lost, 1 less the likeness, in percent, from the same
function ([before and after sheet](before-after-sheet.md)).

### Merging by likeness

Thin lines merge only while the picture keeps `auto_min_likeness` of its likeness ([Neighbours](#neighbours),
`REQ-AUTO-095`, M13.44). The setting is 0 by default, where every line merges as before the rule, and a user
raises it for a picture.

It is off by default because keeping lines can make a spot sew thicker far from any line. A line kept sews as
a running stitch, and the sewing order then puts the blocks of a colour in another order. A fill that sewed
first at a spot can then sew over others there. On one photo measured at 0.85 the thickest spot rose from 5.60
to 7.03 fills, at a place 3.6 mm or more from every line kept, where the block order had changed. Merging any
quarter of the lines kept there left 7.03 to 7.62 fills. Merging all of them put that spot back, and the
thickest spot moved to another place at 7.55. On another photo the thickest spot moved too, and the block
order changed there as well. Where a reorder piles thread can't be told from a line, so no rule about lines
prevents it, and planning twice to check costs a second plan. The answer is one cap on thickness at the end of
planning that every design meets, whatever its order (M13.35). The setting is turned on by default once that
cap is merged.

The measurements below are evidence of what the rule does, at a bound of 0.85 and as each picture sews with
the setting off. No picture is the rule's goal. The photos marked * are local. Thickest is the thickest spot in fills, and the area is at 3 fills or
more, in mm².

| Picture | Likeness | Shapes as running stitches | Jump threads | Thickest | Area |
|---|---|---|---|---|---|
| The dog on turf, 6 threads | 0.814 → 0.856 | 3 → 6 | 57 → 60 | 4.69 → 4.57 | 141.50 → 136.00 |
| The dog on turf, 9 threads | 0.779 → 0.829 | 4 → 8 | 58 → 62 | 4.80, the same | 153.25 → 153.50 |
| The running dogs, 6 and 9 threads | 0.898 and 0.921 | 1 and 3 | 49 and 64 | 4.62 and 5.63 | 130.50 and 187.50 |
| The pear, the lettered patch, TS-13A to TS-13E | 1.000 | no change | no change | no change | no change |
| A magnolia * | 0.480 → 0.849 | 32 → 199 | 155 → 313 | 5.54 → 5.68 | 462.75 → 545.75 |
| A coleus * | 0.454 → 0.841 | 30 → 304 | 237 → 473 | 10.46 → 7.39 | 894.50 → 1,004.75 |
| A greylag goose * | 0.483 → 0.820 | 21 → 98 | 95 → 170 | 5.60 → 7.03 | 499.50 → 517.75 |
| A castle * | 0.434 → 0.779 | 3 → 64 | 54 → 104 | 5.88 → 5.75 | 225.75 → 267.00 |

The running dogs and the drawings sew as on main, and the dogs on turf keep 3 and 4 lines. Their strands of
fur lie among many pieces of their colours, so each is a small share of its colour. The flowers' twigs, the
shadows along their branches and the pale rims of their petals are most of what their colours have there,
and they sew as lines: the photos keep 0.78 to 0.85, where they kept 0.43 to 0.48. That costs them 50 to 236
jump threads more. The goose's thickest spot rises by 1.43 fills and the magnolia's by 0.14, and the coleus's
falls by 3.07.

Merging counts only what it loses itself. The measure also counts the parts that specks and stitches astray
lose. It also counts as parts some lines that are too narrow for a disc a thread across as merging measures
them on their outlines, but not on the measure's grid. So a
picture keeps less than the bound: at a bound of 0.8 the goose kept 0.782 and the castle 0.728, under the
limit. With no line merged that is a part, the castle keeps 0.918. A bound of 0.85 kept every picture
measured at 0.75 or more, so 0.85 is the value to raise it to.

2 other ways were measured and not taken. Counted colour by colour, each colour keeping 0.75 of its parts,
merging kept more lines on the dogs, 6 and 8 on the dogs on turf, which left them 5 and 7 jump threads more,
and the running dog in 6 threads 0.39 fills thicker. Counted over the picture, a colour with many pieces gives
up its lines for a colour with few. The other way merged first the lines closest in colour (CIEDE2000) to the
area each would go into. At 0.8 the magnolia kept 0.802 and the coleus 0.793 so, the same as with the cheapest
first, with 30 and 71 jump threads more. It moved what is lost
from dark lines on dark patches to the pale rims of petals on pink ones.

## Sewing order

Automatic elements sew in an order StitchCraft chooses (`stitchcraft_engine::auto::neighbours::order`,
M13.3). The order is chosen after specks are merged, so that a speck merged into an area is no element of
its own, and before areas reach under their neighbours, which follows it. VectorCraft's Auto Grouping
draws each shape together with the shapes inside it. Sewn in paint order, a picture traced with it changes
thread at nearly every shape, and each change is a stop to rethread a home machine. The order comes from 6
rules.

1. **What overlaps keeps its paint order** (`REQ-AUTO-010`). Of 2 shapes whose paint overlaps, the lower
   sews first, or it would cover the upper one. Paint order is the design's, with each area digitized whole
   moved back before its details ([Neighbours](#neighbours)). An area paints the region it sews once the
   layers are flattened, and a satin column the area between its rails as they are sewn. Lines paint the
   areas their strokes paint as drawn, and a part's line or column the part. Shapes that only meet, as
   abutting paths do, may sew in either order. Rounding leaves a sliver no wider than twice the shapes'
   tolerance between edges drawn as one, and shapes overlap only where their overlap is wider. Where 2 lines
   0.1 mm wide cross, they overlap.
2. **A line after the fill it borders.** A line that touches a fill without overlapping it sews after the
   fill and covers its edge. Its running stitch is as wide as its thread. It can't reach under the fill, and a
   fill sewn after it lays the ends of its rows on it. Rule 1 comes first. A line that rule 1 already sews
   before the fill, through the shapes it overlaps, keeps its place.
3. **The fewest thread changes** (`REQ-AUTO-037`, M13.22). Within rules 1 and 2 the elements sew in
   batches of one thread. Once a thread is sewing, every element of it that is free to sew does, and those
   they free: an element is free when every element that must sew before it has. Sewing more of a thread
   now never costs a change later, so an order is a row of batches, and the row that changes thread least
   is searched for. The base layer sews before the first automatic element, so its thread is sewing when
   that element's run starts. Threads are the same when their colours are, as plan assembly
   counts thread changes. A PES file gives different colours different palette entries while they last
   (`REQ-THREAD-002`), so 2 colours are one thread to the machine only past the palette's 64 entries. An
   element sewn in several threads (M10.5), as a tartan fill is and a gradient fill will be, moves as one.
   Its colours stay together, in the order its generator sews them. It is a batch of its own, which costs a
   change unless the thread it starts in is sewing. The batch of the thread it ends in follows it
   (`REQ-AUTO-024`). Its threads are read before it is generated, from its shape and settings.
4. **A satin column after the fill it borders, where that costs no thread change.** A satin column that
   automatic digitizing made covers the fill's edge when it sews after the fill. Sewn first, it reaches
   its overlap under the fill ([Neighbours](#neighbours)), so the fabric is covered either way. Of the
   rows with the fewest changes, the search takes the one in which fewer columns sew before a fill they
   border. The column then waits for a later batch of its thread when the row has one after the fill.
   Satin columns with settings go by rule 2.
5. **Details last, where that costs no thread change** (`REQ-AUTO-096` to `REQ-AUTO-098`, M13.40),
   when the design turns `auto_details_last` on. An eye, a nose or a mark on a patch sews after the shapes round it, so that no fill's rows lie over it
   ([Details last](#details-last)). Of the rows with the fewest changes, the search takes the one in which
   fewer details sew before a shape round them, as it does for rule 4, and a detail waits for a later
   batch of its thread when the row has one. A line or a column round a detail goes by rules 2 and 4.
6. **The nearest next.** Of the free elements of a batch, the next is the one whose box lies nearest
   the box of the element sewn last. On a tie it is the one whose box's centre lies nearest, and then the
   one drawn first. Each element then starts and ends at its points nearest its neighbours
   (`REQ-GEN-003`).

**The search** (`stitchcraft_engine::auto::neighbours::order::fewest`). Finding the fewest thread changes
under rules of what sews before what is hard in general, and a design has few threads. The search keeps
rows of batches as far as they go, each with the elements it has sewn and the thread it leaves sewing. It
goes on from the row with the fewest changes so far and still needed, one for each thread that has
elements left. Its first whole row then has the fewest changes of any order. Of 2 rows that sew the same
elements and leave the same thread sewing, it keeps the one with fewer changes, then the one with fewer
columns before their fills, then the one found first. It tries the threads of the free elements in paint
order. A design then sews the same on every run. Each run of elements that nothing moves across is searched
on its own, and starts in the thread the run before it left. Where elements with settings of a run sew
before its first automatic element, the base layer sews between them and it, which the search doesn't
count. `order/fewest/tests.rs` compares the search with every order of 300 small runs drawn at random.

`tests/auto_order.rs` checks rules 1 to 4 together on random designs of squares and lines, wherever the
shapes fall.

Elements with settings keep their order among themselves, and automatic elements move round them within
the rules. The parts of a shape sewn as several elements, such as a line's satin columns, keep their order
too, so that a trim after the shape still comes after its last part. `--sort-colours` lets the elements
with settings move as well (`REQ-AUTO-011`), whether or not `--auto` is given. It is for a design drawn in
an order that suits drawing it rather than sewing it. The switch is the command line's. VectorCraft's
exporters order the automatic elements when their `auto` option is on, and sew the elements with settings
as drawn.
An element with a stop after it keeps its place: the elements drawn before it sew before it, and those
drawn after it after it. A user sets a stop for a reason, such as placing appliqué fabric, and it comes
where it was drawn. `SC-I0907` says when the order isn't paint order, with the thread changes of each, those
inside the elements sewn in several threads among them.
M10's routing tools don't start from this order. They route satin columns and running stitches drawn by
hand as Ink/Stitch's tools route them, and write the route into the document ([routing tools](routing.md#order)).
Each element that automatic digitizing makes sews its underlay just before its own top stitches. The
rules leave `underlays_first` off ([ADR-0018](adr/0018-underlays-first-is-a-setting-of-each-element.md)).

Shapes are near when their boxes come within twice the shapes' tolerance of each other. An automatic area's
box is round its region, and a narrow part's line's round the part. Any other shape's box is round its
path's points, grown for a stroke by half its width times √(L² + 1) for a miter limit L. That holds the
corners of a miter clipped at its limit, as a satin column's rails clip it. Other strokes grow by half their
width times √2, which holds a square cap's corners. Rule 4 measures with the same boxes. The order finds a
shape's paint only when the shape is near an element that moves.

**Work** (`REQ-AUTO-012`). Each element has the work budget to itself to find how it lies beside the
shapes drawn after it. It costs a unit for each of them, and the work of those near it. One that needs
more keeps its place in paint order as an element with a stop after it does: nothing moves across it.
An element after it that one before it frees still waits its turn after it, and each element sews once. A
shape whose paint can't be found within the budget keeps its place beside the shapes near it. Finding the
rows of batches has the budget once for the whole design. It costs a unit for each element looked at and
1,000 for each row kept. The default budget keeps 500,000 rows at most. From the run that needs more
on, the elements sew as they did before M13.22. Each thread sews while an element of it is free, then the
thread of the first free element in paint order, and every satin column sews after the fills it borders.
Placing the elements has the budget once more, and costs a unit for each free element looked at each time
an element is placed. An order that needs more sews the elements not placed by then in paint order, after
the others.
`SC-I0907` says each. `SC-I0904` says it of a satin column instead, whose area the areas sewn before it
then don't reach under either. A fill's generator needs the work of its region too, and says it of a fill
(`SC-E0004`).

**Measured.** The test pictures, sewn as drawn and digitized as [Neighbours](#neighbours) sews them.

| Test picture | Thread changes as drawn | Digitized | Jumps as drawn | Digitized |
|---|---|---|---|---|
| Stripes, 3 abutting | 2 | 2 | 3 | 3 |
| Badge, abutting | 2 | 2 | 3 | 3 |
| Badge, stacked | 2 | 2 | 3 | 3 |
| Badges in a row, abutting | 8 | 2 | 9 | 7 |
| Badges in a row, stacked | 8 | 2 | 9 | 7 |

The stripes and the badge on its own have one shape of each colour, which no order sews with fewer
changes, and digitizing keeps their order. The row of 3 badges, each 30 mm across, is drawn badge by badge,
as Auto Grouping draws it. It changes thread 8 times as drawn. Digitized, each thread sews its 3 shapes one
after another, and the row changes thread twice. Its red discs touch, and the needle sews on from one to
the next without a jump. The row jumps 7 times instead of 9. `tests/auto_pictures.rs` keeps the numbers
from falling back.

**Measured on the traced dogs** (M13.22). The fewest thread changes that any order has under each rule,
found by an exact search outside the engine, beside what the dogs sewed and sew:

| Traced dog | Sewn before M13.22 | Rule 1 alone | Rules 1 and 2 | Every column after its fill too | Sewn |
|---|---:|---:|---:|---:|---:|
| On turf, 6 threads | 12 | 5 | 8 | 8 | 8 |
| On turf, 9 threads | 22 | 8 | 12 | 13 | 12 |
| Running, 6 threads | 16 | 5 | 8 | 9 | 8 |
| Running, 9 threads | 28 | 8 | 12 | 14 | 12 |

Rule 1 costs the dogs nothing: with it alone each thread sews once, which is 5 changes in 6 threads and 8
in 9. Rule 2 costs 3 or 4 changes. A dark line of fur sews after the lighter fills it borders, and the
dark fills under those sew before them. The dark thread then sews twice. Every column after its fill would
cost up to 2 more, which rule 4 saves. Taking each thread while it lasted, as the order did before, cost 4
to 16 changes more than the rules need. With the running lines free to sew before the fills beside them,
the dogs change thread 5 and 8 times, but lay 29 stitches astray where M13.21 laid 21, and leave 4
slivers where it left 2: a fill sewn after a line ends its rows on the line. So rule 2 stays a rule.
A trace drawn colour by colour changes thread less in paint order than as sewn, 5, 9, 7 and 12 times on
the dogs, because its lines then sew before the fills beside them.

Fewer thread changes leave more jump threads. The machine cuts the thread at a change and not at a jump,
so a thread that sews once jumps where it changed before. The dogs jump 84, 91, 79 and 108 times, where
they jumped 82, 94, 81 and 112 times, and leave 77, 78, 70 and 95 jump threads to cut by hand, where they
left 71, 73, 64 and 84. Since M13.27 filled 5 of their blobs they jump 79, 91, 79 and 106 times, and leave
71, 78, 70 and 93 jump threads. With an outline after each fill (M13.28,
[an outline round a fill](#an-outline-round-a-fill)) they jump 84, 97, 82 and 109 times, and leave 76, 84,
73 and 96. With runs under the shapes sewn later (M13.17, [runs in place of jumps](#runs-in-place-of-jumps))
they leave 69, 79, 60 and 83. Since M13.30
([less thread where areas overlap](#less-thread-where-areas-overlap)) they leave 71, 80, 58 and 74, and
since narrow columns walk once (M13.35, [a narrow column walks once](#a-narrow-column-walks-once)) 73,
81, 58 and 72. Since an outline sews through no more than 12 mm under a later fill
([the outline under a later fill](#the-outline-under-a-later-fill)) they leave 74, 84, 61 and 74. The traced pear and the test pictures change thread once for each thread after
the first, as they did. The digitized halves of the TS-13 sheets change thread 2, 4, 4 and 6 times, where
they changed 5, 4, 5 and 8 times. TS-13B's black sews twice: its base layer and letters first, and the
lines of its windows after the fills they border. The search keeps 87 to 3,584 rows of batches on the
dogs, and 12,313 on a traced plant of 972 elements, of the 500,000 the default budget allows.

**What shows** (`REQ-AUTO-025`, `REQ-AUTO-026`). Digitized, a picture's stitches should cover the fabric
wherever it has a colour, and lay thread only where it has that thread's colour.
`stitchcraft_testkit::fabric` measures both on any design and its plan, a photo traced into shapes as well
as a test picture. It paints the picture on a grid of 0.05 mm as a viewer paints it, each place in the
colour of the last shape drawn over it, the background left out but where it shows inside the picture. Bare fabric is the part of the
picture's colours that the top stitches leave uncovered, each stitch's thread 0.4 mm wide, opened with a
disc as wide as the thread: a sliver narrower than a thread beside a running stitch closes as the thread
spreads. A patch of 1 mm² or more, the smallest area that digitizing sews, fails. A top stitch is astray
where it shows on another colour or on bare fabric for 1 mm or more, past the 0.4 mm of pull
compensation. The overlap lies under a shape sewn later, which covers it. An underlay or travel stitch is astray wherever it shows, and any stitch astray
fails. The moves sewn on from one element to the next (`REQ-ASM-002`) and the jump threads that the PE800
leaves are counted apart.

| Test picture, digitized | Largest bare patch | Longest stretch astray | Moves astray | Jump threads |
|---|---:|---:|---:|---:|
| Stripes, 3 abutting | 0 | 0.10 mm | 0 | 0 |
| Badge, abutting or stacked | 0 | 0.35 mm | 0 | 0 |
| Badges in a row, abutting or stacked | 0 | 0.40 mm | 0 | 4 |
| Emblem, abutting or stacked | 0 | 0.35 mm | 0 | 2 |
| Button, abutting or stacked | 0 | 0.40 mm | 6 | 0 |
| Traced pear | 0 | 0.44 mm | 0 | 2 |
| Traced dog on turf, 6 threads | 0 | 1.33 mm | 5 | 69 |
| Traced dog on turf, 9 threads | 0 | 3.00 mm | 9 | 79 |
| Traced running dog, 6 threads | 0.18 mm² | 1.95 mm | 2 | 60 |
| Traced running dog, 9 threads | 0 | 1.86 mm | 6 | 83 |

The button's 7 dots sew on from one to the next, each move 2.7 mm long across its yellow. The pear leaves 2
jump threads, where it left 4: the narrow ends of its olive and dark olive bands are uneven, and sew as
fills with their bands (M13.21). `tests/auto_fabric.rs` keeps the moves and the jump threads as measured.

The owner's dog, traced from her 2 photos in 6 and in 9 threads (M13.15, `docs/fixtures/README.md` › The
traced dogs), lays 16 stitches astray: 5 and 8 on the dog on turf in 6 and in 9 threads, and 1 and 2 on the
running dog (15 since M13.35's last pass moved a row of the dog on turf in 9 threads back under its
neighbour). 2 are straight moves, 2.25 and 2.1 mm long, each between 2 lines of one narrow part, which is
a stitch because it is shorter than the collapse length. 3 are stitches of outlines, 1.76 to 2.34 mm
long, each along the edge of a thin, ragged fill. The other 11 are rows of fills, each up to 4 mm
long, a fill's stitch. Before M13.30 the dogs laid 14, which the rest of this paragraph counts up to. M13.21, which fills where columns sewed, brought 20 of them. Of the 6 that were traced
then, 5 lay along a line of fur of another colour that sews on the fill or that the fill reaches under.
The line sews as wide as its thread, and the row shows beside it. The sixth lay under the edge of a column
sewn after its fill. M13.22's order took 8 of the 20 away and brought 4 others, beside neighbours that
now sew in another order. M13.27 fills 5 blobs of fur that sewed as columns
([which stitch a shape gets](#which-stitch-a-shape-gets)). A row of the largest shows beside a line of fur
in both running dogs, the second move crosses a blob that a column covered before, and one row of the
running dog in 9 threads no longer shows: 19 stitches where there were 17. M13.28 sews an outline after
each fill, and the fills end elsewhere. 8 of the rows no longer show and one other does, and the 2
stitches of outlines came: 14 stitches. Which neighbour hid each of the 8 rows was not traced. M13.30
left 11 rows where 10 lay, on other shapes, and a third stitch of an outline
([less thread where areas overlap](#less-thread-where-areas-overlap)). Before
M13.21 the dogs laid 7 stitches astray. 2 of them were satin stitches 7.89 and 4.72 mm long that fanned
across a shape biting into their column, and M13.21 removed those. `tests/auto_photos.rs` lists each stitch,
with where it lies and why, and its case fails on any other stitch astray and on any listed one that no
longer is. Of the [roadmap](../plan/roadmap.md)'s steps, M13.18 removes the rows, by merging the thin
lines into their neighbours, and the 2 moves with them. M13.17 looked for a hidden way for each of the 2
and didn't find one ([runs in place of jumps](#runs-in-place-of-jumps)). The dogs' moves astray are moves from one element to
the next, as the button's are. Their jump threads, 60 to 83, are those that no later shape covers: each
thread sews its shapes one after another, and the PE800 cuts no jump. Each dog takes
too long to digitize and measure in the tests' debug build, so CI's `perf` job runs these cases in the
release build ([conformance](conformance.md#cases-in-the-release-build)). The pieces of the dogs that
showed fabric before the rules above, the paw of the dog on turf and the running dog's ear, are cases of
`tests/auto_fabric.rs` too.

### Details last

(`REQ-AUTO-096` to `REQ-AUTO-098`, M13.40.) A digitizer sews an eye, a nose or a mark on a patch after the
fills round it, so that no fill's rows lie over it. StitchCraft can't tell an eye from a speck of noise,
and no rule may know what a picture shows. A detail is known by geometry and order alone. It is an
automatic area that meets a larger automatic area without overlapping it, and that the shapes round it
close in on. The strip one tolerance wide round its region lies under what the other shapes paint,
grown by twice the shapes' tolerance, for 3 quarters of its area or more (`order::Order::closed_in`). An eye or a mark has
neighbours all round it. A patch at the picture's edge is open along it, and a patch of fur is larger
than the shapes inside it.

**What the layers' rule missed.** [Neighbours](#neighbours) already digitizes an area wider than a satin
column whole under the details in its holes, and moves it back before them. That rule missed 4 cases,
each shown on main before M13.37 by a case of `tests/auto_details.rs`:

1. A detail where 2 or more patches meet lies in the hole of none of them, and the rule left it where it
   was drawn. The cases draw a mark 12 mm square where 2 patches meet.
2. An area no wider than a satin column keeps its holes. Where it sews as a fill, as a ring 3 mm wide
   round a square did, the square in its hole sewed first. Since M13.37 such a ring sews as satin
   columns, which sew after the square as an outline does, and the square reaches under them: no miss.
   A ring that asks to sew as a fill sews before the square with the setting off too, and the case
   stays to keep it so.
3. An area that can't move back past a shape with settings near it opens its hole again, and the detail
   sewed first (`REQ-AUTO-097`). A stop keeps every element on its side of it, so a detail drawn before
   a stop still sews before an area drawn after it.
4. A fill sewn after a detail eats its edge. On the marked patch, sewn on main, the black dot measures
   5.5 mm where it is drawn 6 mm, and the diamond 11.7 mm where it is drawn 12 mm: the later fill's rows
   and their pull compensation reach over them.

**Wishes to the search.** Each detail sews after the larger areas it meets where that costs no thread
change, as a satin column does after its fill (rule 5 above). Of the rows of batches with the fewest
changes, the search takes the one that sews fewer details early. A detail then waits for a later batch of
its thread. The owner's machine has one needle, and each thread change is a rethread by hand. As hard
rules, details last took the traced photos in 6 threads from 8 thread changes to 13 and 14. In 9 threads
the search ran out of its budget, and they changed thread 35 and 38 times.

**Off by default** (`REQ-AUTO-098`). The rule runs only when the design's lowest automatic shape turns
`auto_details_last` on. Off, the order skips the search for details, and a design sews as it did before
M13.40. The rule waits for one cap on thickness at the end of planning (M13.35). Reordering fills can
raise the thread at a spot where no detail lies. A fill that moves for a detail starts and ends
elsewhere, and its own travel runs elsewhere under its rows. It also reaches under other neighbours. On
one traced photo the thread over one spot rose from 4.69 fills to 5.49 by one wish:
the fill that moved crossed the spot 3 times more with its own travel, 9.50 mm of its thread in a box 1 mm
across where it had 3.76 mm, with the same stitches. On another the wishes changed which of the rows with
equally few changes the search took, and a fill sewn later retraced a row. Nothing in the shapes' geometry
told the wishes that do this from those that help. Leaving out the wishes of details whose rim the larger
area covers less than a share, for every share tried from a quarter to all of it, kept one photo at its
old thickest spot and raised another. A check that plans twice and keeps the better plan, or a cut-off by
the number of areas, would only hide that. An area count can't even tell a photo from a logo: a traced
photo can have fewer areas than a badge. Once the cap limits every spot, a reorder can't pile thread
anywhere, and the setting is turned on by default.

**Measured** (2026-11-18, planned at opt-level 2), off and then on:

| Picture | Thread changes | Jump threads | Details sewn early | Thickest spot, fills | Likeness |
|---|---|---|---|---|---|
| Dog on turf, 6 threads | 8, 8 | 57, 57 | 14, 15 of 43 | 4.69, 5.49 | 0.814, 0.814 |
| Dog on turf, 9 threads | 12, 12 | 58, 58 | 28, 16 of 51 | 4.80, 4.69 | 0.779, 0.779 |
| Running dog, 6 threads | 8, 8 | 49, 52 | 30, 20 of 35 | 4.62, 5.03 | 0.898, 0.898 |
| Running dog, 9 threads | 11, 11 | 64, 69 | 32, 20 of 50 | 5.63, 5.02 | 0.921, 0.921 |
| Marked patch, abutting | 3, 3 | 1, 1 | 3, 0 | 3.85, 3.41 | 1, 1 |
| Marked patch, stacked | 3, 3 | 1, 1 | 0, 0 | 3.39, 3.39 | 1, 1 |

The jump threads are those the machine leaves to cut (`stitchcraft_testkit::fabric`). The badges, the
emblem, the button, the pear and the lettered patch sew the same either way. On, the thickest spot fell
on 3 of these pictures and rose on 2, at spots away from any detail.

**Cost.** Counted in instructions with callgrind (`stitch export --auto`, a build without link-time
optimization, 2026-11-18). Off, every picture of `cargo xtask perf` writes a machine file byte for byte
the same as before M13.40, and its count moves by 0.24 % or less, within what a build's layout moves it.
On, the pear takes 8.3 % more, the dog on turf in 6 threads 13.6 % more and the running dog in 9 threads
9.8 % more.

**Open question.** Where a detail sews before a fill, the fill's rows and pull compensation reach about
0.25 mm over its edge, and a later step could end them at the detail's edge.

### Runs in place of jumps

Between 2 shapes of one thread the needle jumps, and a machine with one needle cuts no thread at a jump:
each is a thread to cut by hand. Where the stitches sewn later cover a way from one shape to the next, plan
assembly sews a run along it instead (`REQ-AUTO-053`, M13.17, `stitchcraft_engine::moves`). The needle
neither ties off nor jumps. The run is a row of travel stitches of the element it leads to, 2.5 mm long at
most, and the shapes sewn after it hide it. Assembly decides it, because only there is it known where the
needle is and whether a move is sewn on or jumped (`REQ-ASM-002`). Only moves between elements that
automatic digitizing made are looked at. Elements with settings keep their jumps, as Ink/Stitch sews them.

**What covers** is read from the stitches as generated, and not from the shapes: a fill with rows far
apart covers nothing, whatever its outline says. Every stitch of the top layers is painted 0.4 mm wide on
2 grids of 0.2 mm. One records, for each cell, the last fill or satin column whose thread covers it. The
other records the last stitches of any kind, whose colour is what shows there. Underlays and the base layer
are not painted. A cell is hidden from a move when a fill or satin column sewn after the move covers it
and every cell within 0.4 mm of it. The run then lies as far inside what covers it as an underlay lies
inside its fill, clear of the ends of the rows, where a fill covers least. A cell is also hidden where the
stitches that show are of the run's own colour and sewn after the move. The outline that follows a fill
is such: the run lies under it as the first pass of a double line.

**The ends.** An element ends on its own stitches and starts at its own edge, so a run can't be hidden
from its first millimetre to its last. Within 0.8 mm of each end, less than the 1 mm at which a stretch of
thread counts as showing (`REQ-AUTO-026`), it may cross cells of its own colour sewn before, and cells
covered later without the 0.4 mm to spare.

**The way** is the shortest over the cells a run may cross, found by a search that goes on from the way
that is cheapest so far with what is still ahead. It steps to the 8 cells round a cell and counts in whole
tenths of a cell. Equal ways are taken in a fixed order, which gives a design the same runs on every
machine. A run goes round what would show it, up to twice the move and 10 mm in length. The search stays within
10 mm of the box of the move's ends. The way is then pulled straight wherever the straight line
keeps to those cells, and each straight piece is sewn in equal stitches, with a needle point at every
corner, so that no stitch leaves the way.

**A move that is sewn on**, within the collapse length, stays a stitch unless it would show for 1 mm or
more on another colour or on bare fabric. Then it becomes a run where a way is hidden. Where none is, it
stays a stitch. The roadmap's row had it become a jump there. Measured, that left each traced dog 5 to 15
jump threads more, each no longer than the stitch it replaced and harder to cut than the stitch is to
live with, so the stitch stays and `tests/auto_photos.rs` goes on counting it.

**Work.** Painting and the searches share the work budget once for the whole design. Each cell of the
grids costs 100 units. The default budget then allows a design of 450 mm by 450 mm, and a larger design
gets the same memory at most. A stitch costs a unit for each cell of its box, and a search costs a unit
for each cell of its box and for each cell it looks at. When the work runs out, the moves from there on are sewn on or jumped as they
were, and `SC-I0914` names the element where it ran out.

**Measured before it was built** (2026-11-15, main `399f82b7`). The traced dogs left 76, 84, 73 and 96
jump threads, each a jump between 2 groups of one thread. Of those, 8, 9, 17 and 22 had a way hidden but
for its ends, and 14 to 23 more had a way that shows only on its own colour. For the others every way
crosses another colour that nothing later covers. No order of the shapes could leave fewer than 53, 58, 43
and 50, counted with every shape of a thread's run still to sew and free to start and end anywhere. The
lines that sew after the fills they border, and the last thread, have nothing sewn after them. The traced
pear's 2 jumps have no hidden way, and of the 6 of the badge `web/templates/alpine-night.svg` one has.

**Measured as built.**

| Picture | Jump threads before | Now | Thread in runs |
|---|---:|---:|---:|
| Traced pear | 2 | 2 | 0 |
| Dog on turf, 6 threads | 76 | 69 | 64 mm |
| Dog on turf, 9 threads | 84 | 79 | 28 mm |
| Running dog, 6 threads | 73 | 60 | 93 mm |
| Running dog, 9 threads | 96 | 83 | 109 mm |
| TS-13B, digitized half | 44 | 43 | 7 mm |

The dogs sew 18 to 52 stitches fewer than before, because the needle sews on into a run and out of it
without a lock. Their thread changes, their slivers and their stitches astray are as they were. TS-13A,
TS-13C and TS-13D sew as they did: their shapes of one thread lie apart on bare fabric.

**Alternatives measured.** A run on top of stitches of its own colour would replace 14 to 23 more jumps on each dog.
It would show as a line across the fill, and a travel stitch that shows is astray (`REQ-AUTO-026`). With
0.2 mm to spare in place of 0.4 mm the dogs left 0 to 5 jump threads fewer, with runs under the very ends
of a fill's rows. With a limit of 4 times the move and 20 mm, one dog of the 4 lost 2 more jumps, for
90 mm more thread. An order that sews next the shape with a hidden way to it could come nearer the 43 to
58 jumps that no order betters. The order is chosen before a stitch is generated, and what covers is read
from the stitches, so that is a step of its own.

## Settings

The rules' values are StitchCraft's own parameters, registered as any other
([parameter registry](params.md), [reference](../user/reference/params/auto.md)), since M13.7
(`REQ-AUTO-020`). Each rule goes by the settings of the shape it digitizes (`stitchcraft_engine::auto::AutoParams`):

The settings come in 3 tiers. The Auto effect's dialog shows the first 2, and the third is the rules' bounds,
which `stitch plan --set KEY=VALUE` sets on every shape digitized automatically, as a document can on one.
The [rules' reference](../user/reference/auto-rules.md) lists every setting with its default and its tier,
generated from the registry.

| Tier | Setting | Key | Rules |
|---|---|---|---|
| A shape | Stitch | `auto_stitch` | the stitch the shape sews as, where its parts can: left at `auto`, the rules choose |
| A design | Satin or fill | `auto_prefer` | leans the least slenderness, the most a width varies and the narrowest piece of a fill together, by half |
| A design | Fill uneven parts | `auto_fill_uneven` | whether a part of several lines whose width varies is filled |
| A design | Fill angle | `auto_fill_angle` | the angle of every fill made of the shape |
| A design | Outline round fills | `auto_fill_outline` | whether each fill sews [an outline](#an-outline-round-a-fill), read of the lowest shape |
| A design | Overlap | `auto_overlap_mm` | an area reaching under its neighbours and cut away under them, a satin column past a rail that a shape sewn after it runs along |
| A design | Pull compensation | `auto_pull_compensation_mm` | a satin column past a rail with no shape sewn after it beside it, and its end under another column |
| A design | Leave out the background | `auto_ignore_background` | the background, read of the lowest shape |
| A design | Base layer | `auto_base_layer` | whether the design sews one, read of the lowest shape whose settings read, as its other settings are |
| A design | Base layer's stitch length | `auto_base_stitch_length_mm` | its rows' stitches and their spacing |
| A design | Base layer keeps holes | `auto_base_keep_holes` | whether the outer shape keeps its holes |
| Advanced | Widest satin column | `auto_max_satin_width_mm` | a line's stitches, a part's columns and their longest stitch, the split of a part into its fill and the pieces that leave it. A shape may set it lower, never higher |
| Advanced | Narrowest triple run | `auto_min_triple_run_width_mm` | a running stitch tripled |
| Advanced | Least slenderness of a column | `auto_min_slenderness` | a compact part |
| Advanced | Most a column's width varies | `auto_max_unevenness` | an uneven part |
| Advanced | Farthest a column reaches from its line | `auto_max_reach` | a part with arms |
| Advanced | Narrowest piece of a fill | `auto_min_fill_width_mm` | what leaves the fill of an area |
| Advanced | Smallest area | `auto_min_area_mm2` | specks, the narrow parts too small to cut from a broad part, and what details may leave of a hole they fill |
| Advanced | Base layer's offset | `auto_base_offset_mm` | how far inside the design's outer shape it lies, and how wide the rim is |
| Advanced | Largest sliver | `auto_base_sliver_mm2` | the largest gap in the top thread that is left to the base layer: a limit of the [measure](#the-stricter-measure), by which nothing sews differently |
| Advanced | Most left to the base layer | `auto_base_bare_percent` | the most of the design that the top thread may leave bare over a base layer, the measure's other limit |

A design's settings are a shape's too: VectorCraft's Auto effect on a group or a layer sets them on every
path inside it. The Auto effect's dialog held 13 rows after M13.16 and was 903 px tall in a window of 900.
With the bounds and the 2 limits of the measure left out it holds 12. M13.28 gave the outline's switch a
row, and the base layer's offset became an advanced setting to make room. No picture so far has needed
that fine adjustment changed, which is matched to the pull compensation, where the base layer's switch,
its stitch length and its holes each change what one sees.

The defaults are the values on this page, and MC-8 confirms or changes them. A shape that sets none of the
settings takes the defaults, and an automatic shape with a setting that doesn't read sews nothing
(`SC-E0101`). Each area reaches its own overlap under the shapes sewn after it, and its specks are its own
parts smaller than its smallest area, wherever they go. The narrowest satin column is the design's
`min_satin_stroke_width_mm`, one value for the whole design, and the underlay of a column's width (above)
is no setting.

An automatic element reads the settings before it is sewn as any stitch type, and the ordinary elements it
becomes don't ([parameter registry](params.md#settings-of-automatic-digitizing)). They are in no stitch
family's dialog, and a shape that sets them is still automatic. VectorCraft's Auto effect (M13.7) sets them
from its dialog for the paths it is on and those inside the group it is on, and with `--auto` every other
shape takes the defaults.

## Previews in VectorCraft

The Auto effect is a live effect like the plug-ins of each stitch family
([plug-ins](vectorcraft-integration.md#plug-ins)). VectorCraft runs a group's effects on each path inside
it (`REQ-VC-006`), so the effect on a traced group previews every path's stitches, each sewn as its width
says. A live effect is given its path and its paints alone. Like `stitch export`, it reads the path as an
area when a fill paints it, and as a line along its topmost stroke. Its preview shows each shape's
stitches without the overlaps, the merged specks or the order, which `stitch export` adds. VectorCraft
draws what a live effect returns with every paint of its object. The stitches come back as gaps in the area that
the fill fills when a fill paints the object, and as lines that its stroke draws otherwise.

Digitizing is most of a preview's work, from about 1,450 instructions a unit to 2,260 in VectorCraft
0.18.0's host, and a live effect has 50 million ([live-effect previews](vectorcraft-integration.md#live-effect-previews-within-v1-budgets)).
A square's takes 12,900 units, an L's 21,000 and a ring's 31,800. The auto level digitizes within eight
tenths of the budget, then shows each element made as auto shows its family's: a fill's top rows, and a
satin column's and a running stitch's every stitch. When that costs too much it shows the outline, as a
larger square, an L, a ring and most traced letters do. Exact shows every stitch, and outline the path,
hatched across a fill. A preview that costs more than the level allows shows less, with `SC-I0810`, as the
other effects' previews do. The command line's preview always shows every stitch.

Tracing again replaces the group with a new one, without the effect. Apply the effect after the last
trace, or use `--auto`, which works without it.

## With VectorCraft's newer plug-in features

- **Exporters** (0.14, [P1](rfc-vectorcraft-abi-v2.md#p1-exporter-plug-ins)). File › Export › Export As › PES
  plans the document inside VectorCraft, and its `auto` option digitizes the objects without effects
  (M6.9). The flow is then trace and export, both in VectorCraft.
- **Effects in filter objects** (0.15, [P8](rfc-vectorcraft-abi-v2.md#p8-effects-and-stroke-style-in-filter-objects)).
  A filter could set on each path the effect of the family its width chose. The user then adjusts it in
  that effect's dialog.
- **Budget classes** ([P4](rfc-vectorcraft-abi-v2.md#p4-budget-classes-for-live-effects), still proposed for
  live effects). The Auto effect's previews of large shapes would get the heavy budget.

## Text in pictures

Text traced from a picture is shapes, and the rules for shapes sew its letters: strokes as satin columns,
small parts as running stitches. Small letters come out ragged, and neighbours that the tracing joined sew
as one blob. An embroidery font is drawn for the needle, and lettering sets text in one
([fonts](fonts.md#lettering)). M14.5 puts lettering where a picture's text is. A person marks the text and
says what it reads, since nothing here reads a picture. Spike M14.6 looks for a way to find and read the
text without a marker. A drawing's text that is still a text object sews without a marker: from M14.7 automatic
digitizing letters it ([text objects in automatic digitizing](fonts.md#text-objects-in-automatic-digitizing)).

The pass is `stitchcraft_svg::lettering::pictures`. It works on a design once a reader has made it and
before anything is digitized (`REQ-AUTO-063` to `REQ-AUTO-066`). What is left of the traced shapes is then
digitized as if the letters had never been traced, with its overlaps and its base layer.

### The marker

A marker is a shape named `stitch:text` and the text, such as `stitch:text SAMPLE PATCH`. The name is an
object's name in VectorCraft's Layers panel, and an object's label in an SVG file, Inkscape's
`inkscape:label`. Both readers already give a shape's name to its element, so one pass serves both. The
marker needs a fill or a stroke, since a reader is given only objects that paint. It is never sewn.

| The marker is | Its area | The text |
|---|---|---|
| a closed shape, such as a rectangle | what its outline encloses | lies level across it |
| an open line with a stroke | what lies within half the stroke's width of the line, between its ends | follows the line, the letters' tops to the left of the way the line is drawn |

A line is for text on an arc, as round a badge. It is drawn through the middle of the letters in the
direction the text reads, with a stroke wide enough to cover them. The line says which way the letters
stand, which the letters' shapes alone don't say. A straight line drawn at a slant sets slanted text the
same way. Finding the arc from the letters without a line was left out: it is a guess where a drawn line
is not, and M14.6 decides whether text is found without help at all.

A font other than the default, and lettering's trims and sorted colours, follow the text in double quotes:
`stitch:text "SAMPLE PATCH" font=geneva_simple trim=word`. The font is named by its folder, as a Lettering
effect names one. It is found where the reader's fonts are
([lettering in VectorCraft](vectorcraft-integration.md#lettering)). The default is the font built into the
plug-ins. A setting that doesn't read is named with `SC-W1113`, and the marker is still used. The scale and
the spacing are found from the traced letters, so a marker doesn't set them.

From the command line, `--text LEFT,TOP,WIDTH,HEIGHT:TEXT` marks a rectangle (`REQ-CLI-020`). The 4 numbers
are percent of the box round the design's shapes. A traced picture's background is traced too, so that box
is the picture, and a share of it can be read off the picture whatever size it sews at:

```sh
stitch plan patch.png -o patch.pes --text '20,35,60,30:MATCH'
```

The text after the colon is written as a marker's name writes it. An option marks level text only. For
text on an arc, `--traced` writes the traced document, a line is drawn on it in VectorCraft, and the
document is planned. A picture with a transparent margin measures from what was traced, not from the
picture's own edge.

### Which shapes are the letters

Every subpath that lies wholly inside the marker is taken out, of every element. A subpath is one closed
outline. Image Trace keeps all the areas of one colour in one compound path, so a letter is a subpath of
its colour's path, not an object, and the hole it leaves in the colour round it is a subpath of that
colour's path. Taking both out removes the letter and closes the hole. An island of the background's
colour inside a letter, its counter, goes the same way. What is left of the background is its own outline,
so it sews whole under the lettering. A picture drawn in layers has no holes to close, and its letters are
taken out as whole objects.

So the background is told from the letters by reaching outside the marker, and by nothing else. Colours
are not compared, since a traced edge has specks of other colours along it.

A subpath that crosses the marker's edge stays as it was traced. When more than half of its points lie
inside, it is probably a letter partly outside the marker, or a letter the tracing joined to something
outside, and `SC-W1114` counts those. Points are those of the outline flattened within 0.05 mm, which is a
tenth of the narrowest stroke an embroidery font sews.

### Size and place

What was taken out is measured along and across the line the text follows. A closed marker's line is
level, so along is across the page and across is up. The text is set once at the font's own size and
measured the same way. Then:

- **Height.** The scale, to a tenth of a percent, makes the box of what the text sews as tall as the box
  of what was taken out. The box holds parts that reach below the baseline, so a `p` in the text is
  measured with its tail.
- **Baseline.** The text's box is centred on the traced letters' box across the line. The baseline lies
  as far above the box's bottom as the text's lowest point lies below the baseline.
- **Width.** Every gap between 2 glyphs, a gap between words too, is widened or narrowed by one amount
  until the text is as wide as the traced letters. The amount is at most half the letters' height wider
  and a tenth of it narrower. Past half a height a word reads as separate letters. Closer than a tenth,
  the strokes of neighbours sew over each other. The text is then centred on the traced letters along
  the line.
- **Along an arc.** The baseline is the marker's line moved sideways to the baseline's distance, each
  corner of it pushed out along the line that halves the corner. Lettering along a path then places each
  glyph on it and turns it whole ([along a path](fonts.md#along-a-path)).

The scale stays within the heights the font sews at. Letters shorter or taller than that are set at the
nearest height the font sews, centred on the traced letters, and 2 warnings say so: lettering's own
`SC-W1106`, with the font's range, and `SC-W1115`, with both heights and both widths in millimetres.
`SC-W1115` is also said when the text ends up more than a tenth of the letters' height from their width.
The traced letters are not kept in such a case. Text at a height the font sews well reads better than a
blob, and the picture shows the difference.

A font that sews in one thread takes the thread that painted the most of what was taken out. A hole
counts against its thread, so the background, whose holes are as large as the letters, never wins. A font
of several colours keeps its own. The lettering's elements sew after the last element a subpath was taken
from, which puts them on the background. `SC-I1112` names each marker with the count of outlines taken
out, both heights, the text and the font.

### Markers that can't be used

Nothing is taken out before the lettering is set. A marker without text, one that encloses no area, one
with nothing wholly inside it, and one whose font isn't found, doesn't read or draws none of the text's
characters are each named by `SC-W1113` with the reason. The traced shapes then sew as they were, so a
design is never left with a hole and no text.

### What it sews

Measured as `stitch plan --auto` plans each picture, in Ink/Stitch Small Font, which sews from 3.6 to
15.2 mm. The thickness numbers are M13.29's
([how thick a design sews](#how-thick-a-design-sews)):

| Picture | Letters | Sewn as | Stitches | Jumps | Thickest spot, fills | At 3 fills or more, mm² |
|---|---|---|---|---|---|---|
| The lettered patch | 10 mm | traced | 4,669 | 10 | 5.5 | 150.5 |
| | | lettering | 4,615 | 6 | 5.1 | 128.5 |
| TS-13B's 4 words | 15, 10, 6 and 4 mm | traced | 2,941 | 27 | 5.1 | 32.0 |
| | | lettering | 3,317 | 12 | under 3 | 0 |
| The sample emblem, 100 mm, traced by VectorCraft 0.22 | 6.3 mm, on 2 arcs | traced | 25,934 | 36 | 7.5 | 681.5 |
| | | lettering | 25,138 | 24 | 6.4 | 614.2 |

The thickness doesn't get worse. Lettering lies on a whole background where traced letters were stacked
columns that overlap at their joints. The background has no overlap to reach under a letter any more.

Small Font's strokes are thinner than the bold block letters of TS-13B. At 15 and 10 mm the traced block
letters sew well as they are, and the lettering looks lighter than the picture. At 6 mm and below, and for
the emblem's letters, the lettering is the one that reads. From M14.8 a second, bolder font is built in,
and [the font is chosen by the letters' weight](#the-font-chosen-by-the-letters-weight).

### The font chosen by the letters' weight

The owner asked on 2026-11-16 why lettering that replaces big letters looks lighter than the picture.
The stitches were right. The one font built in was a light one. M14.8 measures how bold letters are and
builds in a bold font to choose by that measure (`REQ-AUTO-080`, `REQ-AUTO-081`).

**The measure** is the width of a stroke over the height of a letter, found the same way on a traced
outline and on a font's satin columns (`pictures::weight`). Each closed outline is taken at 48 places
evenly spaced along it. From each a ray goes straight into the shape, as far as the next edge. The side
that is in is the one where the ray meets the outlines an odd number of times. The stroke's width is the
median of those distances. The long sides of a stroke outweigh its ends and its joints, whose rays run
along the stroke and find its length. So an I and an O of one pen measure alike, which the area over the
outline's length doesn't give.

- **Traced letters** are the subpaths taken out of the thread that painted the most of them. The holes
  the letters leave in the colour round them belong to another thread and aren't measured. A letter drawn
  as a line is as wide as the line. The height is the one the lettering is fitted to.
- **A font** is measured on its capital letters A to Z. Each satin column's 2 rails are closed into an
  outline. The height is the median height of what each letter draws. Both are as drawn: a column's pull
  compensation and the thread's own width add to what sews, for traced letters as for a font's.

Measured so, at the fonts' own sizes, for the fonts of Ink/Stitch's library at `c7e3a05` that came into
question:

| Font | Licence | Stroke over height | Capital letters sew from, mm | To, mm |
|---|---|---|---|---|
| Barstitch regular | OFL | 0.096 | 12.2 | 36.5 |
| Ink/Stitch Small Font, built in | OFL | 0.125 | 3.8 | 16.2 |
| Ink/Stitch Medium Font | OFL | 0.126 | 12.1 | 24.2 |
| Geneva Simple Sans | CC BY-SA | 0.150 | 10.0 | 19.9 |
| Кирилиця, from Roboto | OFL | 0.175 | 7.9 | 28.8 |
| TT Directors | OFL | 0.186 | 9.3 | 25.9 |
| **Barstitch Bold, built in** | OFL | 0.188 | 12.5 | 32.5 |
| Glacial Tiny 60 AGS | OFL | 0.215 | 2.8 | 8.4 |
| Manga Impact | OFL | 0.244 | 8.6 | 43.2 |
| TT Masters | OFL | 0.271 | 9.0 | 27.1 |
| Ink/Stitch Masego | OFL | 0.275 | 13.9 | 20.9 |

And the traced letters:

| Picture | Letters | Stroke over height |
|---|---|---|
| The lettered patch, MATCH | 10 mm | 0.18 |
| TS-13B's 4 words | 15, 10, 6 and 4 mm | 0.18 |
| The sample emblem's 2 arcs, traced by VectorCraft 0.22 | 6.3 mm | 0.24 and 0.23 |

The block letters are drawn with strokes 18 percent of their height wide
(`testsheets::pictures::word`), and the measure finds that. Small Font is a third lighter than they
are. Barstitch Bold is within a twentieth of them.

**Why Barstitch Bold.** It is the nearest in weight to the block letters among the clean faces. It has
capital and small letters and 157 glyphs. Its licence file has a copyright line and no reserved name
([what was checked](fonts.md#fonts-built-into-the-plug-ins)). 2 fonts as bold sew smaller, and neither was
taken:

- TT Directors sews from 9.3 mm, which would hold the patch's 10 mm letters. It has capital letters only.
  Its licence file names a designer and no copyright holder, and says the font comes from a demo of a
  typeface that is sold. The OFL's notice needs a copyright line to carry.
- Кирилиця, drawn from Roboto, sews from 7.9 mm and has a copyright line. Its files are 3.4 MB, more than
  twice Barstitch Bold's, and an export that letters in it would read a glyph file of 2.5 MB.

TT Masters, from the same designer, sews from 9.0 mm, and its licence file is the same as TT Directors':
a demo of a typeface that is sold, and no copyright line.

Manga Impact and Masego are display faces, and bolder than the block letters by as much as Small Font is
lighter.

**The choice** (`pictures::chosen`). A marker that names a font is set in it. Without a font name, it is
given one of the fonts the reader's host offers, which are the fonts built in that the host has
([in VectorCraft](vectorcraft-integration.md#lettering)):

1. The fonts whose heights hold the letters' height come first. When none holds it, the fonts that sew
   nearest to it take their place.
2. Of those, the font nearest in weight to the letters is chosen, and the earlier in the list when 2 are
   as near. Letters without a width to measure take the earliest.

The height comes before the weight because a font sews only at the heights it was drawn for. Bold
letters set taller than the picture's would run into their neighbours and past the marker. A font's
heights are those of its capital letters: how tall it draws them, times its least and its greatest scale.

`SC-I1118` names the marker, the letters' height and weight, and the font with its weight, and says which
of 3 things happened. It gives weights to 2 places, as the fonts' are written down: 0.13 and 0.19.

| The letters | Set in | The note says |
|---|---|---|
| 15 mm, strokes 0.18 | Barstitch Bold | it is the nearest in weight of the fonts that sew at that height |
| 15 mm, strokes 0.14 | Small Font | the same |
| 10 mm, strokes 0.18 | Small Font | Barstitch Bold is the nearest in weight and sews from 12.5 mm, and leaving the text unmarked sews the traced letters |
| 40 mm, any | Barstitch Bold | no font here sews at that height, and this one sews nearest to it |

The fonts' numbers are written in `stitchcraft_vectorcraft::lettering::HELD`, since a plug-in chooses a
font before it reads one. A build that has the fonts measures them again (`REQ-FONT-022`). With fewer
than 2 fonts offered there is nothing to choose, and the marker is set in the default font without a
note.

**Bold letters from 6 to 12.5 mm** are the gap. Barstitch Bold doesn't sew that small, so Small Font
sets them at their own height. They look lighter than the picture, as before this step. The note
says from what height the bold font sews. Traced block letters sew well as shapes from about 10 mm, so
from 10 to 12.5 mm the person can leave the text unmarked. Below 10 mm the lettering is the one that
reads, light as it is.

**A text object** has no traced letters. Its Lettering effect sews in the font it names, or in the
default font. A bold `font-weight` doesn't choose the bold font: type set at 12 points is 3 mm tall, far
under what the bold font sews, and a document made before this step would sew otherwise than it did.

**Left for later.** An embolden setting that widens a font's satin columns would sew bold letters under
12.5 mm without a third font. It can close the counters of letters such as A and B, so it waits for a sew-out. The effect's dialog has
no list of fonts.

### Decisions for the owner

Each has a recommendation, which the step took.

- **A name marks the text.** The marker is a shape or a line with a name, as every other marker in a
  VectorCraft document is. A dialog of its own would need a filter, and a filter can't keep a shape's
  settings before VectorCraft 0.15. Recommended: the name.
- **A line through the letters for an arc.** The other choice is to find the arc from the letters, which
  can't tell which way they stand. Recommended: the drawn line.
- **Letters no font sews as tall** are set at the nearest height the font sews, with 2 warnings. The other
  choice leaves them traced. Recommended: lettering, since small traced letters are the ones that sew
  worst.
- **The limits of the spacing**, half a height wider and a tenth narrower, are starting values that no
  sew-out has confirmed. MC-10 sews a patch both ways.
- **`--text` in percent of the design's box.** Pixels of the picture are the other choice, and need the
  picture's size, which `stitch` doesn't read today. Recommended: percent.
- **A bolder font built in.** Decided on 2026-11-16: the owner asked for one, and M14.8 built in
  Barstitch Bold.
- **Bold letters from 6 to 12.5 mm**, which the bold font doesn't sew, are set in Small Font at their own
  height, with a note. The other choices are to leave them traced, which sews badly under 10 mm, and to
  set them in the bold font at 12.5 mm, taller than the picture has them. Recommended, and taken: the light
  font and the note.
- **A third font for that gap**, or an embolden setting, waits for the sew-out of both fonts
  ([fonts](fonts.md#decisions-for-the-owner)).
- **A text object's bold `font-weight` doesn't choose the bold font.** Recommended, and taken: the effect
  sews in the font it names or in the default, so documents sew as they did.

### What isn't built

- The Auto effect's preview in VectorCraft still draws the traced letters. A live effect is given one
  object and no name ([RFC](rfc-vectorcraft-abi-v2.md), P6). The exported file and its preview picture
  have the lettering.
- A marker has one line of text. 2 lines take 2 markers.
- A font is matched to the letters' weight, among 2 fonts. The letters' style, such as italic or script, doesn't choose a font yet.
- The emblem's numbers are from a document traced earlier, kept outside the repository. A case in the
  repository waits for a traced picture with lettered arcs among the fixtures.

## Pictures from the command line

`stitch plan picture.png` and `stitch export picture.png` trace the picture with VectorCraft's command line,
as spike M13.9 decided ([ADR-0015](adr/0015-pictures-traced-by-vectorcraft.md)) and M13.10 built
(`REQ-CLI-012`, `REQ-CLI-013`). A design whose extension is `.png`, `.jpg` or `.jpeg`, in any case, is a
picture. VectorCraft publishes `vectorcraft-cli` with each release, and `stitch` runs the one
`--vectorcraft-cli` names, or else the one on the `PATH`:

```sh
vectorcraft-cli run --commands - --export FOLDER/traced.vectorcraft
```

It gives it 4 steps on standard input, where a picture of any size fits. They are the compatibility gate's
`trace` case's:

1. `swatch.library.load` loads the threads `stitch palette` writes for the machine as a swatch library.
2. `file.place` places the picture's bytes, fitted inside a square `--size` millimetres across, or else
   inside the machine's hoop 0.5 mm in from each side, its aspect kept.
3. `imageTrace.make` traces it in colour with the library's colours as they are, at most `--colours` of
   them, by the Abutting method, so that the shapes share their edges.
4. `imageTrace.expand` keeps the paths, without the picture.

VectorCraft saves the document into a folder of the command's own beside the machine file, named as
`stitch`'s temporary files are, and the folder is removed once the document is read. It runs with
`VECTORCRAFT_NO_PREFS` set, so that it neither reads nor writes the user's VectorCraft preferences, and a
picture traces the same for everyone. The document is then the design. It is read with `--auto`, planned for
the machine `--profile` names or else the reference machine, and named after the picture. `--traced` writes
it beside the machine file, to correct the trace in VectorCraft. A bug-report bundle keeps it too, so that a
replay doesn't need VectorCraft.

**The picture's size.** VectorCraft places a picture at its own resolution, or 72 pixels to the inch when
its file says none, so a logo a few hundred pixels across would sew larger than any hoop. `stitch` fits the
picture into the machine's hoop unless `--size` gives the length of its longer side. The size is the
picture's, its background included, and the shapes left once the background is out sew smaller. Fitted to
the hoop, the picture keeps 0.5 mm from each side: a shape along its edge sews as a satin column, whose
stitches reach 0.4 mm past its rails, its pull compensation, from rails that may lie up to the tolerance
shapes meet within outside the shape. Fitted to the hoop itself, a picture whose shapes reach its sides
would sew past the hoop, which `SC-E0701` refuses.

**How many colours.** Image Trace keeps at most `--colours` of the library's colours, 6 unless it says
otherwise, as in its own panel. The count takes in the background's colour. A picture with smooth edges, or a
JPEG file, has colours between its shapes' colours, which may trace as threads of their own. A `--colours`
equal to the number of colours the picture has keeps them out. A badge in 3 colours on white traces in 4.

**When VectorCraft can't trace.** A command line that can't be started, missing from the `PATH` or from
the path named, is `SC-E0815`, which says how to install it, or to trace the picture in VectorCraft. One
that fails or doesn't save the document is `SC-E0816`, which ends with what it said. Then nothing is written but
the report.

StitchCraft traces nothing itself. A tracer of its own would repeat VectorCraft's, which a harness
measures. It would also trace a picture otherwise than VectorCraft does, and the command line would then
sew it unlike the tutorial's flow.

## Subject cut-out

A photo's background traces into shapes of its own, such as turf, a fence or tyres behind a dog. The
background rule leaves out one shape, the lowest, when it lies behind everything ([Neighbours](#neighbours)).
So `--subject` cuts a picture's subject out before VectorCraft traces it, as spike M13.13 decided
([ADR-0019](adr/0019-subject-cut-out-before-tracing.md)) and M13.14 built (`REQ-CLI-014` to `REQ-CLI-019`).
It is off by default, and a picture given without it is traced as [above](#pictures-from-the-command-line).

**The steps.** With `--subject`, `stitch` changes the picture before the 4 steps VectorCraft takes, in
`stitchcraft-cutout`:

1. It decodes the picture as its file stores the pixels, with the `image` crate, as VectorCraft decodes it:
   a PNG or JPEG file by what its bytes say, at most 16,384 px on a side and 50 megapixels in all.
2. It finds the subject's box. `--subject-box X0,Y0,X1,Y1` gives it in the picture's pixels. Otherwise
   U²-Net-p maps how likely each pixel is to belong to the main subject, at 320 × 320 px, the picture
   squeezed to that square by Lanczos's filter and normalized as rembg normalizes it. The map is stretched
   to run from 0 to 1, and its values of a half or more fall into pieces of pixels that touch, corners
   included. The box is the one round the piece with the most of the map in it, scaled back to the picture.
3. MobileSAM cuts the subject out of the box, with input and output prepared as SAM's own code prepares
   them. The encoder takes the picture once, the longer side scaled to 1,024 px, each side rounded to a whole
   pixel, normalized and padded with zeros to a square. Its decoder takes the points of `--subject-point`,
   labelled 1, and of `--not-subject`, labelled 0, then the box's 2 corners, labelled 2 and 3, each scaled
   as the picture's side along it is. It gives a map of 256 × 256 logits across the square, which is scaled
   to the square, cut to the picture's part of it and scaled to the picture, each time bilinearly between
   the pixels' middles. The logistic function of each value, times 255, is the pixel's alpha, or the
   picture's own where that is lower.
4. The picture with that alpha, cropped to the box round the pixels of an alpha of 128 or more, is written
   as a PNG file. `file.place` places it where it would place the picture, so `--size` and the hoop fit
   the subject.

Image Trace leaves out the pixels whose alpha is under 128, and gives a pixel on the cut edge the colour
beside it (VectorCraft's `crates/trace` at `3f8d7a0`). The trace then contains the subject alone, with no
background for the background rule to find. If MobileSAM keeps none of the pixels, or if U²-Net-p's map has
no piece above a half and no box is given, `SC-E1202` says so and suggests `--subject-box`. A picture the
cut-out can't read is `SC-E1203`. `stitch` then writes the report alone, as when VectorCraft can't trace.
Every loop over the picture charges the budget, which runs out with `SC-E0004`. A cut-out made says the box
it came from, how much of the picture it kept and its size (`SC-I1206`), and the report keeps it all.

**Correcting a cut-out.** A cut-out is wrong when the box contains more than the subject and MobileSAM keeps
it, or when MobileSAM keeps less than the subject. `--not-subject X,Y` marks a point of what isn't the
subject, and `--subject-point X,Y` a point of what is, each as often as needed and in the picture's pixels.
The decoder takes the points with the box, U²-Net-p's when the user gives none, since a point without a box
can lose much of the subject. A box without area or one that extends past the picture, a point outside it, and any
of these options or `--cutout` without `--subject` are usage errors, and so is `--subject` for a design that
isn't a picture. `--cutout FILE` writes the cut-out beside the machine file, with the other files asked for,
all or none, so that the user can see what was kept.

**In VectorCraft.** A filter can't read or replace a placed image, and a plug-in couldn't run the models
within its budget (ADR-0019, decision 2). `stitch cutout PICTURE -o FILE` writes the cut-out alone, with
the same marks, and says what it kept as `stitch plan` does. The user places it where they would place the
photo, and traces it with Image Trace as the tutorial does. A picture whose background another program made
transparent traces as its subject too, without `--subject`. The owner's traced dogs were made so (M13.15,
`docs/fixtures/README.md` › The traced dogs).

**The models** are 3 ONNX files, 49.0 MB together, which tract 0.23.10 runs in `stitchcraft-cutout`, a library
of the apps' layer that only `stitch` depends on:

| File | Model | Its weights' licence | Size | Made |
|---|---|---|---:|---|
| `u2netp.onnx` | [U²-Net-p](https://github.com/xuebinqin/U-2-Net), 1.1 million parameters | Apache-2.0 | 4.6 MB | rembg's ONNX conversion of its author's checkpoint |
| `mobilesam-encoder.onnx` | [MobileSAM](https://github.com/ChaoningZhang/MobileSAM)'s encoder, 5.7 million parameters | Apache-2.0 | 28.0 MB | exported from its authors' checkpoint |
| `mobilesam-decoder.onnx` | MobileSAM's decoder, 4.1 million parameters | Apache-2.0 | 16.5 MB | exported from its authors' checkpoint |

The repository pins each file's address, size and SHA-256 in `apps/stitchcraft-cutout/models/models.toml`,
and none of the files is committed. StitchCraft's release `cutout-models-1` publishes them, with their
licences, a notice and the record of how they were made. GitHub Actions builds that release from the
authors' own files, in the workflow `models.yml`. Its build job exports MobileSAM's 2 files with the script
beside the pins, `export.py`, from the checkpoint in MobileSAM's repository at a pinned commit, with
MobileSAM's own code and PyTorch's exporter, in the packages `requirements.txt` pins by hash. It exports
twice, and the 2 exports must agree. It fetches U²-Net-p's file from rembg's release, and stops unless each
file has its pinned size and SHA-256. The encoder's sum is the one exception, since its export has one of 2
sums by the machine: ADR-0019's decision 5 says what the build and the publish do then. The owner publishes the release by starting the workflow by hand on
`main` and typing the release's tag, and no other run publishes. A release published is never replaced, so a
new set of models takes a new tag. (Amended 2026-11-09, M13.14.) U²-Net-p's author publishes its checkpoint on
Google Drive and Baidu alone, so its file is rembg's conversion of it, as rembg's own release publishes it,
the file M13.13 measured: rembg is under the MIT licence, and the weights under the Apache-2.0 licence, as
the author wrote in issue 208. (Amended 2026-11-08, M13.14.)

`stitch` fetches the files with curl on first use into the folder `STITCHCRAFT_MODELS` names, or else
`stitchcraft/models` in the user's cache folder, which `cargo xtask corpus` uses for the fonts too. It reads
each whole and checks its size and SHA-256 before tract reads the same bytes. A file missing or different is
fetched again into a temporary file beside it and renamed into place only once it matches. When a file can't
be fetched or still differs, `SC-E1201` names each file's address, size and SHA-256 and the folder it belongs
in, and `stitch` writes the report alone. tract would seed a random operator from the computer's own
randomness, so a model that has one is refused before it runs (`SC-E1205`). A model tract can't run is
`SC-E1204`. NOTICE names the models and their licences.

**Built in, or left out.** The cut-out is `stitch`'s `cutout` feature, on by default, which brings tract and
the `image` crate in. A build without it has the same options, and `--subject` and `stitch cutout` stop there
with `SC-E1207`, which says the build leaves the cut-out out. The WASI build that makes the golden files
(`cargo xtask wasm --goldens`) is one. tract's own CI only checks that tract-onnx builds for `wasm32-wasip1`,
and with `getrandom`'s JavaScript backend chosen (`ci/cross.sh`). It runs tract-core's and tract-linalg's
tests there, and no model. The golden files never cut a subject out, and without the feature that build
doesn't depend on tract building for WASI or on `getrandom` there. In the release build for Linux x86_64, `stitch` is
33.3 MB with the cut-out and 6.8 MB without it, or 28.5 MB and 6.0 MB once stripped (2026-11-15). The owner
accepted the larger program.

**Replays.** The models run in `stitch` alone, and their results may differ in their last bits from one
processor to another. A bug-report bundle of a picture already contains the document VectorCraft traced.
With `--subject` it also records the models' SHA-256 sums, the box, where it came from, the points and the
crop, and leaves the picture out. Its replay says the picture was cut out and runs without the models and
without VectorCraft.

**Measured.** The cut-out was measured on the owner's 2 photos of her dog, against masks checked by eye
([ADR-0019](adr/0019-subject-cut-out-before-tracing.md)). With tract in `stitchcraft-cutout`, the cut-out
from U²-Net-p's box kept the dog at an intersection over union (IoU) of 0.985 on turf and 0.987 running. It
kept 0.5 % and 0.4 % of the dog's area wrongly and lost 1.0 % and 0.8 % of it. A point on the tyre behind the
running dog that isn't the subject gave 0.988. U²-Net-p alone kept that tyre, 19.8 % of the dog's area. M13.13
measured 0.985 and 0.988 with a third-party export of MobileSAM, whose encoder holds other weights than its
authors' checkpoint. On the 2.1 GHz Xeon of the [technical design](tdd.md), with one thread, onnxruntime
took 0.5 s for U²-Net-p, 1.6 s for MobileSAM's encoder and 0.1 s for its decoder. tract took 1.0 to 1.2 s,
2.5 to 2.9 s and 0.15 to 0.23 s. In Rust, with tract, the whole cut-out of a photo took 5.1 to 5.4 s and
343 MB at most.

**Cases** (M13.14). The 2 photos are committed as prepared, 1,200 px wide, with a mask of each checked by
hand, and recorded in `conformance/fixtures/README.md`. The owner took them of her own dog and gave them
under the MIT licence. The models' outputs for them are fixtures too, U²-Net-p's map and MobileSAM's map of
each prompt, saved from what tract gave. The cases of the box, the marks, the alpha, the crop and the file
run on those fixtures without the models, and so does `REQ-CLI-019`'s case of the bounds. Stand-ins for the
models, tiny ONNX files with their inputs and outputs, run through the same tract runner in the cases of
`stitch plan --subject` and `stitch cutout`. One picture drawn in code, a red disc
on grey stripes that touches a blue square, has exact masks. With the models, 2 points on the disc must keep
the disc alone, and a box round the square the square alone (`REQ-CLI-015`). `REQ-CLI-016`'s case runs
`stitch plan --subject` end to end where no model can be had, and expects `SC-E1201` and the report alone.
A job of its own, `models`, runs the cases that need the models, which every other job leaves out, and
[conformance](conformance.md#cases-with-the-models) says where it takes them from. It compares tract's maps with the fixtures, and
each cut-out with its mask, which it must match at an IoU of 0.97 or more, with at most 2 % of the subject's
area kept wrongly and 3 % lost.

## Testing

- Each rule gets its requirements and failing cases before its code ([ADR-0008](adr/0008-conformance-first.md)).
- The test pictures are drawn in code, as the test sheets are (`stitchcraft_engine::testsheets::pictures`),
  and no logo or picture of anyone else's is used. From M13.15 the owner's dog, cut out of her 2 photos
  and traced, is a test picture too ([sewing order](#sewing-order)), and from M13.14 the subject cut-out's
  cases use the photos ([above](#subject-cut-out)).
- Each test picture's automatic plan is compared with its plan in paint order: colour changes, jumps,
  trims, stitches, and how much of each shape and of each shared edge the stitches cover
  (`stitchcraft_testkit::metrics`). The cases measure the pictures where `stitchcraft_testkit::pictures`
  places them.
- Level B of the [compatibility gate](compatibility-gate.md) has a case, `trace`, in which VectorCraft's
  command line traces a picture drawn in the threads `stitch palette` writes, and StitchCraft sews the
  document it saved, on every track. In its case `picture`, `stitch plan` traces the picture itself.
- `stitch plan`'s own tests run it with a stand-in for VectorCraft's command line, the test program itself,
  which saves a document VectorCraft 0.18.0 traced (`apps/stitchcraft-cli/tests/picture.rs`).
- TS-13 sews the test pictures at MC-8 ([machine testing](../plan/machine-testing.md)), each twice. A
  sheet's top half is digitized automatically, and its bottom half is sewn as drawn, in paint order. The
  bottom half sews first and stops, so that nothing digitized moves into it, and each half's thread
  changes, jumps and time can be read. TS-13A sews the pictures of neighbours and sewing order, TS-13B
  block letters and lines traced as areas, TS-13C a face drawn with lines and shapes in parts, TS-13D
  the emblem and TS-13E the button, each traced abutting and stacked
  ([test sheets](../user/reference/test-sheets.md)). TS-13D sews the emblem 42 mm wide, at 0.7 times its
  size. There the white round each star is narrower than a satin column, and sews as a column round the
  star, while the navy is one fill under the disc and the bars. The PE800 cuts no jump, and the sew-out
  reports where a shape sews down the thread of a jump before it.
- `tests/auto_fabric.rs` measures what each test picture and the traced pear of the user pages leave bare
  and lay astray, digitized ([Sewing order](#sewing-order)). The pear is read from
  `docs/fixtures/traced-pear.vectorcraft`, a photo that VectorCraft traced, with the ragged edges a trace
  has.
- `tests/auto_sheets.rs` answers the TS-13 checks that the sheets' own stitches can, moved to where each sheet
  draws its shapes (`stitchcraft_testkit::pictures::sheet`). Where the bar runs on beside the square, each
  ends its rows at its own edge. The specks sew as the disc, and no shape sews over one drawn over it. A line
  traced as an area runs to within its width of its ends, and its branch from the line it meets. The smile's
  column ends where its round caps end, the columns' rungs lie within 10° of square to their strokes, and the
  face's columns cover the edges of the areas under them. The sew-out shows what only the machine can: whether
  fabric or a ridge shows once the thread pulls in, and how small a word still reads.
- The same cases count the jump threads each half leaves on the PE800, snipped at each thread change and
  stop, and how many a later stitch of their colour sews down (`stitchcraft_testkit::stitches`). Digitized,
  the letters sew as columns with a jump between each 2. A thread that sews once jumps between its shapes
  where it changed before M13.22, and the PE800 cuts at a thread change and not at a jump, so the digitized
  halves of TS-13A, TS-13C and TS-13D leave 3, 1 and 2 jump threads more. With an outline after each fill
  (M13.28) TS-13D's half left 2 more again, while it held the buttons that TS-13E has now (M13.31):

  | Sheet | As drawn: jump threads | sewn down | Digitized: jump threads | sewn down |
  |---|---:|---:|---:|---:|
  | TS-13A | 8 | 0 | 18 | 9 |
  | TS-13B | 6 | 0 | 43 | 19 |
  | TS-13C | 9 | 4 | 13 | 5 |
  | TS-13D | 1 | 0 | 11 | 3 |
  | TS-13E | 0 | 0 | 4 | 1 |

- Ink/Stitch sews the TS-13 sheets too, every night ([conformance](conformance.md#l3--inkstitch-differential-nightly),
  `REQ-AUTO-021`). Ink/Stitch can't choose a picture's stitches, but it can sew what StitchCraft chose:
  the elements StitchCraft sews a sheet with, written as an SVG file with their parameters. Its machine
  files then agree with StitchCraft's by every measure of the differential. Their thread footprints overlap
  StitchCraft's by 0.97 or more. StitchCraft sews 6 to 10 % fewer stitches, as it leaves out stitches
  shorter than the machine's shortest (`DEV-FIN-001`): fitted to the machine as StitchCraft fits its own
  plans, Ink/Stitch's files come within about 1 % of StitchCraft's stitches. Ink/Stitch's extra trims and
  jumps are jumps that go nowhere at thread changes, which a StitchCraft PES file has no record of
  (`DEV-FMT-001`), and without them the trims and jumps agree within 1.
- A wrong choice sews alike in both programs. So, every night too, the lines of each shape sewn wholly as
  running stitches are compared with the lines Ink/Stitch's Fill to Stroke draws for it (`REQ-AUTO-022`).
  The shapes are TS-13B's word 4 mm tall and its windows' lines, and the letters T, H and E from 3 to 10 mm
  tall in lines from 0.3 to 0.9 mm wide. Each branch Ink/Stitch finds has one of StitchCraft's, and away
  from their ends the lines lie within 0.15 of a width of each other.

Codes 0900 to 0999 are automatic digitizing's ([diagnostics](diagnostics.md)).
