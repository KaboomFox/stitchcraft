# Image to embroidery

<!-- implements: crates/stitchcraft-engine/src/auto/**, crates/stitchcraft-engine/src/moves.rs, crates/stitchcraft-engine/src/moves/**, crates/stitchcraft-plan/src/thickness/**, apps/stitchcraft-cli/src/commands/palette.rs, apps/stitchcraft-cli/src/trace.rs, apps/stitchcraft-cutout/src/**, apps/stitchcraft-cli/src/subject.rs, apps/stitchcraft-cli/src/subject/**, apps/stitchcraft-cli/src/commands/cutout.rs, apps/stitchcraft-cli/src/commands/expand.rs, crates/stitchcraft-svg/src/lettering/pictures.rs, crates/stitchcraft-svg/src/lettering/pictures/**, apps/stitchcraft-cli/src/text.rs, apps/stitchcraft-cli/src/keep.rs, crates/stitchcraft-engine/src/design/mark.rs, crates/stitchcraft-engine/src/design/mark/**, crates/stitchcraft-engine/src/names.rs, crates/stitchcraft-engine/src/names/** -->

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

### Choosing a style

The style stays the person's choice (`REQ-AUTO-150`, M13.42). An `auto` style would choose `logo` or `photo`
from the picture's own measures, and no measure tried chooses well on a varied set. This section keeps
the evidence, measured on 2026-11-20 on `main` at `6c565e4c`.

**What the choice changes.** The 2 styles differ in 2 settings. `auto_prefer` of `satin` leans 3 bounds
towards columns. `auto_fill_uneven` off keeps the columns of an uneven part and sews each ragged thin line
as a line where `photo` merges it into its neighbour. So the wrong style costs:

| Picture | In its own style | In the other |
|---|---|---|
| A traced photo | its thin lines merge, and it sews thin | `logo`: 2 to 11 times the area at 3 fills, up to 7.1 fills, the cap out of budget (`SC-W0913`) |
| Flat art, lettering or a drawing | its lines and columns sew as drawn | `photo`: up to 20 % of its detail lost, as lines merge into fills (`SC-I0915`) |

Some designs sew alike in both: a badge of broad flat shapes, a drawing of even lines or a sky of flat
bands doesn't change any part's rule. The 4 traced dogs keep 1 to 5 % less of their detail in `photo`, so neither
style is better on every count.

**The set.** 25 pictures of known style and 3 undecided ones. Drawn in code for this step: flat art in an
even pen, flat art with hard bands of shade, photo-like pictures with soft edges and grain, 2 drawings in an
uneven pen, lettering on a field, and 2 flat pictures photographed, with light that falls off and grain. A
person picks `logo` for the flat art, the drawings, the lettering and the photographed art, and `photo`
for the soft pictures. A person can't place 3: a badge with a soft gradient, and 2 pictures that are half photo and half
flat art. The repository adds the 4 traced dogs, the pear, the traced badge and the lettered patch, and 4
traced photos of flowers, a bird and a building were measured locally. Each was traced by VectorCraft
0.24.1 in 6 colours, 100 mm across, from 1,000 and 500 pixels.

**The measures.** None separates the 2 groups:

- **Flat colour in the picture**, the share of pixels whose neighbours match within 3 levels: 0.80 to 0.96
  for drawn flat art, as a JPEG too, and 0.29 at most for the photo-like pictures. A photographed badge
  reads 0.001, and a dog cut out of its photo on a white ground reads 0.64, so both get the wrong style.
- **Flat colour in the subject**, the ground's colour left out: the dog reads 0.03, but a drawing in an
  uneven pen reads 0.43 from 1,000 pixels and 0.10 from 500, below a furry picture's 0.25.
- **Thin lines drawn evenly**, the roadmap's example: a trace draws every thin line ragged, a pen line
  too. Every picture had 0 to 0.006 of its thin-line area even, and the even shield's 4 lines were the only exception.
- **Small parts**, the share of parts under 2 mm²: 0 to 0.88 for flat art and 0.04 to 0.97 for photos.
  The running dog reads 0.07 at 100 mm, 0.46 at 60 mm and 0 at 150 mm.
- **Uneven parts**, the share of area in parts of several centre lines whose width varies by more than
  0.3: 0.38 for a hand-drawn flower, 0.77 from 500 pixels, and 0 to 0.26 for 5 of the photos.

Even thin lines tell nothing apart. A bound on flat colour, of the picture or of its subject, misstyles 4
of the 19 pictures with a source to measure, and a bound on small or uneven parts 6 or 7 of the 25. Each
bound moves with the picture's pixels and size. Which style sews a picture better can flip too: a lettered disc sewed
572 mm² at 3 fills in `logo` and 87 in `photo` from 1,000 pixels, and 73 and 351 from 500. A picture of
both kinds, such as a label on a photo, wants `logo` on the label and `photo` on the rest, which one style
can't give. Shape settings, marks (`--keep`) and Expand serve such a picture.

**What is said instead.** One notice, `SC-I0923` (`REQ-AUTO-151`, `auto::style`), tells the person that the
choice matters for this design and which way the other style goes. The other style is the preset whose
`auto_fill_uneven` differs from the shape's, with its values set over the shape's settings as `--style`
sets them. The notice counts 2 things:

- **Thin lines.** In the photo style, the lines that merged into their neighbours. In the logo style, the
  ragged thin lines that merging would take, found as merging finds them, which stay as they are.
- **Parts.** Each part is decided again from the measures that decided it, under the other style's
  bounds. A part filled here that the other style sews as satin columns counts with its rule, and a part
  sewn as columns that the other style fills counts as such.

The notice names the other style and its settings, and says nothing else: no opinion of which style is
right, and nothing of what the picture shows. A design whose counts are all 0 gets none. It changes no
stitch. The count of the parts plans nothing again, so it leaves out how a fill splits off its narrow
pieces, which `auto_prefer` leans too, and the lines that a merge keeps for the picture's likeness.

**What it costs.** COST

**What it says on the set.** SURVEY

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
| Thread, in fills | 4 | 6 | Brother's troubleshooting table names "three or more overlapping stitches" as a fault of the design, and the warning started at 3. From M13.46 it starts at 4 ([The cap and the warning](#the-cap-and-the-warning)). Embrilliance's density map turns red at 6 layers |
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
fills is grey, and thread of 2 fills or more yellow. From 4 fills it is orange, and from 6 red.

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
`SC-I1119` says what the last pass took and then what each kind of the cap's gave, and what is left above
the cap ([The cap and the warning](#the-cap-and-the-warning)).

The cap is `stitchcraft_engine::auto::cap`. It goes through the needle points whose stitch in or out comes
within 1 mm of a spot above the cap, and judges cover by the top stitches within 2.5 mm of such a spot,
found once. Places farther off count as not covered, and that keeps the points of a long stitch. Each kind goes
through the plan up to 4 times, and stops once a time takes nothing or no spot is above the cap. It keeps
the last pass's measure up to date with the pass's own helpers, so the design is still measured once. It
works on copies. When it runs out of work, the plan stays as the last pass left it, and `SC-I1119` says
so.

Until M13.46 the cap took a satin column's top stitches too. Where a column's points crowded in a bend,
the third kind could take one. The stitch left then crossed part of the column only. On TS-13C's face
outline it lay 14° from square, where `REQ-AUTO-017` holds the columns' rungs to 10°. Since M13.46 the cap
takes none of a column's top stitches ([The cap and satin columns](#the-cap-and-satin-columns)).

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

#### The cap and satin columns

The owner asked on 2026-11-18 whether her machine can sew designs this thick. The trade's rule of thumb is
that one layer of satin over a fill sews well, and that 2 dense layers on each other risk broken needles
and thread. In this page's measure a fill under a satin column is 2 to 3 fills, so a cap of 4 is at the
edge. With the cap at 3 the area at 3 fills or more nearly went, but a ring of satin got a ragged inner edge
and a satin letter showed specks of fabric where its strokes join (M13.46).

**Why.** The cap's test of cover checks whether the places along a needle point's 2 stitches lie within half a
thread of other top thread. A column's stitches lie 0.4 mm apart or closer, nearer still at an inner edge or
in a bend. The stitches beside any of its points pass that test. The test reads places along stitches. It misses
the column's edge, which its needle points make, and the angle of its stitches, since a place on a stitch
says nothing of either. Traced on TS-13C's and the lettered patch's
digitized shapes with the cap at 3, the cap took 95 points of columns:

| Kind | Points of columns | What the joined stitch does |
|---|---:|---|
| A shape's top under later shapes | 49 | It runs 45° across the column, half of them more, where a later shape covers part of it. Where that shape's edge pulls in, it shows |
| A shape's own doubled top | 46 | A point on one edge goes, and its 2 stitches across become one stitch of 0.4 mm along the other edge: a notch in the edge |

The same cap took 158 points of fills' top stitches and 7 of underlay, which change nothing that shows. At
4 fills the cap changed the columns of 11 of the 24 designs measured, by 1 to 10 points, in the same 2 ways.
The last pass moved no column's stitch.

**The rule.** Below the warning's limit the cap takes no needle point of a satin column's top stitches
(`REQ-AUTO-112`), and above it only as a last resort ([Above the warning's limit](#above-the-warnings-limit)).
The column keeps the edge and the angle that digitizing gave it. Its underlay and travel are the cap's to take, and so
are the stitches of the fills under it and beside it. A column is an element that digitizing made whose
stitch type is satin, which planning knows from the stitches each element generates. The rule reads that
and the stitches' roles, and no picture, shape or limit of its own.

**A column's edge.** Lowered to 3.5, the cap left TS-13C's face columns covering 0.982 of the edges under
them, once pulled in, where `REQ-AUTO-007` holds them to 0.99. The last pass and the cap took the overlap that the face's
and the eyes' rows lay under the columns, which keeps a column's edge covered when the fabric pulls the
column in. The last pass moved those rows' ends back from under the columns, and the cap took 44 points of
them by its second kind, where it took 5 at 4. Either alone left the edges at 0.982 or 0.983. Both read a
column's stitches where they are drawn. So since M13.46, below the warning's limit (`REQ-AUTO-114`):

- the last pass counts no satin column's thread as hiding a row's end;
- the cap counts a column's top stitch as covering a place only as the fabric pulls it, shortened by the
  overlap at each end where the thread turns by 10° or more, as `REQ-AUTO-007` measures;
- where taking a point makes a stitch beside it turn where it ran straight on, the part that the pull then
  moves in must stay covered.

Judging every covering stitch as pulled, the fills' rows too, kept the overlap under every shape, but left the
cap nearly nothing to take: at 4 the dogs sewed up to 5.63 fills. Only a column pulls in so hard that its
edge needs the rows under it.

**Where a joined stitch lies.** With the cap at 3.25, a stitch the cap joined across a bend of the ear's
rows lay beside the shape for 1.7 mm, on another colour, where `REQ-AUTO-026` allows 1.5. And on TS-13E a
stitch it joined measured 0.262 mm on the plan's own points, off the machine's grid, at least 0.3 mm on the
grid, where `REQ-PLAN-002` asks every stitch to be as long as the machine's shortest. So a joined top stitch lays its
thread only where its shape's own top thread or a later shape's lies, and its length is checked on the
plan's points as well (`REQ-AUTO-100`).

#### The cap's default

With these rules every design was planned with the cap at 4, 3.75, 3.5, 3.25 and 3, and each was held to
every requirement, with `auto_details_last` off (M13.48). 3.25 is the lowest at which the cases of every requirement pass.
At 3 the cap joins the base layer's row ends, hidden under the top, across the rows that `REQ-AUTO-028`
holds straight. A bare share that rises far under its limit doesn't count against a cap, and the spots that
satin columns hold are reported, not counted. The numbers are under
[Above the warning's limit](#above-the-warnings-limit), with the cap's last resort.

#### Above the warning's limit

Left whole everywhere, the columns held spots at up to 5.25 fills on a traced dog with the cap at 3.25, and
the badges at 4.40. Main's cap, which took their points, kept every repository design and dog at 4.00 or less.
The owner's question was whether her machine can sew these designs, so this step can't raise the thickest
spot. Above the warning's limit, 4 fills (`auto_thick_warn_fills`), a spot is too thick to sew well, and that
outweighs a column's edge (`REQ-AUTO-112`):

- The last pass counts a column's thread as hiding a row's end where the turn lays thread in a spot above
  the limit.
- The cap's 3 kinds go first through the needle points near such a spot, judging a column's stitches as
  drawn, without the checks of a column's pulled edge and of where a joined stitch lies. Then they go down
  to the cap with every rule.
- What is still above the limit then loses top points of the columns whose stitches lay thread there, one at
  a time. The candidates are the points that lower the thickest spot by half as much as the best one, or
  more. Of those the cap prefers one whose joined stitch runs across the column, turning less than 45° from
  the stitch into the point, and then the one that turns least. It stops once no spot is above the
  limit.
- No change takes a spot at the limit or under it above it (`REQ-AUTO-074`, `REQ-AUTO-100`). The check read
  only the thickest spot along the stitches a change adds, so a stitch across a spot of 7 fills could raise
  a spot of 3.75 beside it to 4.25. Each spot held to what it was blocked changes that add a hundredth of a
  fill anywhere above the cap, which left the dogs at up to 5.1 fills, so the check is at the limit.

The least change alone took more than it had to. A point whose stitches barely lower the spot changes a
column for nothing: on the dog on turf in 6 threads, where 2 columns of one shape meet over a fill at 4.36
fills, it took 2 points that lowered the spot by less than a hundredth before the 3 that brought it under 4.

**Measured** on 2026-11-19, main against this step, each at its defaults. The plan with the cap at 50 fills
has every top needle point of a column, and the points it has that this plan lacks are the points taken. A notch is a new
stitch of a column that turns more than 45° from the one it replaced, and a leaning stitch one that turns 10°
to 45°:

| Design | Thickest, main | M13.46 | At 3 fills or more, mm², main | M13.46 | Column points taken, main | M13.46 | Notches, main | M13.46 | Leaning, main | M13.46 |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| Badges | 3.99 | 4.00 | 19.50 | 18.25 | 3 | 2 | 3 | 2 | 0 | 0 |
| Lettered patch | 3.98 | 3.98 | 40.25 | 38.75 | 1 | 1 | 1 | 1 | 0 | 0 |
| TS-13A, digitized half | 3.99 | 3.99 | 23.75 | 19.50 | 6 | 2 | 4 | 2 | 2 | 0 |
| TS-13B, digitized half | 3.85 | 3.85 | 4.25 | 3.25 | 0 | 0 | 0 | 0 | 0 | 0 |
| TS-13C, digitized half | 3.96 | 3.92 | 95.75 | 83.75 | 9 | 1 | 2 | 1 | 1 | 0 |
| TS-13D, digitized half | 3.97 | 3.97 | 50.75 | 41.50 | 0 | 0 | 0 | 0 | 0 | 0 |
| TS-13E, digitized half | 3.39 | 3.24 | 2.50 | 2.50 | 0 | 0 | 0 | 0 | 0 | 0 |
| Traced pear | 3.86 | 3.99 | 13.00 | 10.50 | 1 | 1 | 1 | 1 | 0 | 0 |
| Traced dog on turf, 6 threads | 4.00 | 3.87 | 131.25 | 54.25 | 2 | 3 | 2 | 3 | 0 | 0 |
| Traced dog on turf, 9 threads | 4.00 | 3.97 | 149.00 | 63.75 | 0 | 0 | 0 | 0 | 0 | 0 |
| Traced running dog, 6 threads | 4.00 | 3.92 | 129.00 | 53.50 | 3 | 1 | 1 | 0 | 0 | 1 |
| Traced running dog, 9 threads | 4.00 | 3.98 | 176.50 | 94.00 | 10 | 7 | 7 | 7 | 1 | 0 |
| Traced magnolia | 4.00 | 4.22 | 521.25 | 217.50 | 1 | 1 | 1 | 1 | 0 | 0 |
| Traced coleus | 4.12 | 4.16 | 959.25 | 306.25 | 0 | 0 | 0 | 0 | 0 | 0 |
| Traced goose | 4.00 | 4.37 | 489.25 | 156.75 | 3 | 0 | 2 | 0 | 1 | 0 |
| Traced building | 4.00 | 3.82 | 255.75 | 91.75 | 0 | 0 | 0 | 0 | 0 | 0 |

On the repository's designs and the dogs no spot is above 4.00. The columns lose 18 points where main's cap
took 35, with 17 notches where it made 21 and 1 leaning stitch where it made 4. The area at 3 fills or more
falls by 58 to 64 % on the dogs. Against main, the dog on turf in 6 threads loses 3 column points where
main's cap took 2, one more notch, and the running dog in 6 threads has 1 leaning stitch where main's has
none. The badges' thickest spot is at the limit, 4.00, where main's is 3.99, and the pear's 3.99, where
main's is 3.86. The cases of `REQ-AUTO-102` hold the bare share within its limit and the slivers, jumps,
thread changes and likeness to those of the cap at 50 fills.

Of the 4 traced photos outside the repository, the magnolia keeps a spot at 4.22 fills, the coleus at 4.16
and the goose at 4.37, where main's cap held them to 4.00, 4.12 and 4.00. Each is a spot of fills alone, 4
to 9 of them, whose top thread shows. Before the cap the goose's spot was at 4 fills or less, and it rose
during the cap. That isn't traced further here.

**What holds a spot above the cap.** At 3.25 the thickest spot of each design above it, split by element, and
the step whose work its cause is. This step doesn't add a rule for them:

| Design | Thickest | Satin columns' top | Other top | Under | What lies there | Owner |
|---|---:|---:|---:|---:|---|---|
| Badges, TS-13A's badges | 4.00, 3.99 | 3.21 to 3.43 | 0.41 to 0.65 | 0.12 to 0.16 | 2 columns of one ring meet over a fill | M13.47, joins |
| Lettered patch, TS-13B's word | 3.98, 3.85 | 2.95 to 2.99 | 0 to 1.02 | 0 to 0.86 | 2 columns of the M meet over the field | M13.47, joins |
| Traced dog on turf, 6 threads | 3.87 | 2.08 | 1.51 | 0.27 | a column over a fill where 2 columns of one shape meet | M13.47, joins |
| Buttons | 3.52 to 3.66 | 1.89 to 2.09 | 1.26 to 1.62 | 0 to 0.31 | the rim's column over the disc | none yet: a column over a fill |
| TS-13D, digitized half | 3.97 | 2.24 | 1.48 | 0.25 | one column of a white ring over the star and the navy | none yet: a bend |
| Traced pear | 3.99 | 2.40 | 1.38 | 0.22 | the stem's 2 columns, of 2 shapes, over a fill | M13.49, columns of 2 shapes over a fill |
| TS-13C, digitized half | 3.92 | 2.31 | 1.36 | 0.25 | the eyes' lines over the face and the eyes | none yet: a column over a fill's turns |
| Traced dog on turf, 9 threads | 3.97 | 1.85 | 1.54 | 0.58 | a column over a fill and the base layer | none yet: a column over a fill |
| Traced running dog, 6 threads | 3.92 | 0 | 2.86 | 1.06 | 4 fills and the base layer, whose tops show | none yet: fills alone |
| Traced running dog, 9 threads | 3.98 | 0 | 3.80 | 0.18 | 4 fills, whose tops show | none yet: fills alone |

On the 2 running dogs the thickest spot is now one of fills alone, 3.92 and 3.98 fills, above the cap of
3.25. The cap took what its rules let it there. Their top thread shows, and the cap takes a shape's own top
only where it is doubled.

#### The cap and the warning

The cap is what planning aims at. The limits of `REQ-AUTO-061` are what a person acts on. A spot that satin
columns hold stays above the cap of 3.25 on 10 of the 20 repository designs. The warning used to name the cap there, and
it started at 3 fills, which nearly every design has somewhere, so it was said of nearly every design. Since M13.46 (`REQ-AUTO-113`):

- `SC-I1119` says what the last pass and the cap took and what is left above the cap, whenever a spot was
  above it, also when nothing could be taken.
- `SC-W0913` warns at its own limits and never names the cap. Its limit of thread is 4 fills, where it was
  3. One layer of satin over a fill measures 2 to 3 fills and sews well, and 2 dense layers on a fill measure
  4 or more. The failing limit stays at 6.

With the cap's last resort no digitized design of the repository and no dog sews above 4 fills. TS-13A, D and
E still warn, at places of their halves as drawn, which the cap doesn't touch.

#### The cap and lettering

The owner compared, on 2026-11-19, a patch's word sewn from its traced letters with the same word marked
([text in pictures](#text-in-pictures)) and sewn in a built-in font. She chose the marked text for words on
patches: its joins are clean and it leaves fewer jump threads. The cap is for her machine, and it didn't
reach lettering. A lettering element has settings of its own, the font's, and the cap takes needle points
only of the elements digitizing made. M13.50 measured how thick lettering sews and what may go
(`REQ-AUTO-119` to `REQ-AUTO-124`). The rules hold for any text, any built-in font, any size it sews at and
any background.

**How thick lettering sews.** Every letter and digit of the 2 built-in fonts was planned at 3 heights each,
alone and on a field, an automatic fill 3 mm round it, as a patch's field lies under its word. So were 4
made-up words, the lettered patch with its word marked, and TS-13B's 4 words marked. Measured on
2026-11-19 with M13.46's cap, which doesn't reach lettering:

| Letters, 62 a set | Thickest, fills | Median of the letters' thickest | At 3 fills or more, mm² | At 4 or more |
|---|---:|---:|---:|---:|
| Small Font, 3.6, 7.6 and 15.2 mm, alone | 3.72 to 4.28 | 2.95 to 3.21 | 2 to 16 | 0 |
| Small Font, on a field | 4.87 to 5.35 | 3.96 to 4.15 | 175 to 1,361 | 1.0 to 14.8 |
| Barstitch Bold, 12.5, 22.5 and 32.5 mm, alone | 4.99 to 5.36 | 3.74 to 4.00 | 172 to 202 | 3.5 to 6.2 |
| Barstitch Bold, on a field | 6.31 to 6.67 | 4.60 to 5.00 | 1,296 to 3,820 | 189 to 257 |

The marked patch sews 4.53 fills at its thickest spot and 75.25 mm² at 3 fills or more, TS-13B's words 4.16
and 11.0. At a letter's thickest spot lie, on average:

| Thread at the thickest spot | Small Font | Barstitch Bold |
|---|---:|---:|
| The top stitches of the columns that meet there: 2 at 55 to 85 % of the spots, 1 or 3 at the others | 1.8 to 2.1 | 2.3 to 2.6 |
| The columns' own underlay | 0.48 to 0.75 | 0.33 to 0.40 |
| The font's running stitches, which run between a letter's columns and fill its narrow parts | 0.21 to 0.29 | 0.45 to 0.99 |
| Locks, where the thread jumps to the next letter or part | 0.1 to 0.5 | 0.1 to 0.5 |
| The field's top stitches and the base layer, on a field | 1.15 to 1.24 | 1.16 to 1.19 |

A hand-digitized font overlaps its columns where they join on purpose. Thread pulls a column narrower, and
columns that only touch part and show fabric between them. The overlap that pull compensation gives is what
keeps them joined, and on narrow strokes it is enough without more
([Wilcom's help on digitizing custom fonts](https://docs.wilcom.com/embroiderystudio/27/en/OnlineHelp/Management/custom_fonts/custom_fonts-23.htm);
a retailer's article puts it at 0.2 mm on stable fabric and 1 mm or more on knits,
[Hooping Station](https://hoopingstation.com/blogs/articles/ink-stitch-push-pull-compensation-for-satin-fonts-the-overlap-trick-that-stops-gaps-plus-underlay-settings-that-actually-matter)).
Barstitch Bold sets 0.3 mm of pull compensation and Small Font 0.2. So the 2 columns at a join are the
font's own drawing, and the overlap within its pull compensation must stay.

**What may go, by geometry.** The cap takes a needle point out of a run and joins its 2 stitches. That
takes thread where the run turns, and none from the middle of a straight run.

| Thread | What became of it | Rule |
|---|---|---|
| The field and the base layer under the letters | A fill's rows run straight under a letter. The cap takes none of them. Cut away under the letters, as an area is under a shape digitized from an area, the field turned its rows at every letter's edge, and those turns piled up. Cut 0.5 mm inside the letters' edges, Small Font at 15.2 mm went from 4.87 fills to 6.23 at its thickest, with 14 times the area at 4 fills or more, and cut 1 mm inside, to 5.70 with twice the area. A field sews whole under lettering, as an area does under a column digitized from a stroke | none |
| A column's own underlay | Tagged top today, since a font's column sews whole. The cap read it as the column's surface. Tagged underlay, the cap's first kind takes its turns where top stitches sewn after it cover them | `REQ-AUTO-120`, `REQ-AUTO-122` |
| The font's running stitches under a later column | The cap's second kind, as for any shape's top under later shapes, once the column covers as its own pull compensation pulls it | `REQ-AUTO-119`, `REQ-AUTO-121` |
| Locks | They stand where the thread jumps, and the owner cuts those threads by hand. The last pass keeps one back and forth of each, as for any design | `REQ-AUTO-119` |
| A column's top under another column | Not the cap's: the overlap is the font's. Taken by the second kind where wholly under a later column, it took little more than the rules above, and the column's end under the other is what pull compensation needs | none |

A column's underlay stays where no other column lies over it. It holds the column's top up and keeps its
edges sharp, and taken wherever its own top hid it, it went at every column's end. Where 2 columns overlap
the other's top holds them. Taking the underlay out of each column more than its pull compensation inside
another, in the plan rather than by the cap, didn't lower a letter's thickest spot by more than 0.1 fills: a
centre walk or a contour lies along its own column's middle, and seldom inside the other.

**Whose lettering.** Lettering that automatic digitizing places is digitizing's: the text a marker names in
a picture (M14.5) and a text object that automatic digitizing letters (M14.7). It is in the cap's reach
(`REQ-AUTO-119`), its columns are columns (`REQ-AUTO-112`), and a design of lettering alone is measured with
the default limits. A lettering object a person places sews by its own settings, as every element with
settings does. The cap leaves it as it is, and `SC-W0913` tells of a spot above the warning's limit
(`REQ-AUTO-124`). Planning knows the lettering it may change by a mark the lettering passes give the elements
they place, `Element::lettered`: `Lettered::Digitized` from the passes of marked text and text objects, and
`Lettered::Set` where a person sets text in a font. The elements' ids aren't that mark, since a person may
name a shape so, and no file holds it. A design with lettering of either kind and no automatic shape is
measured by the rules' default limits. With nothing in the cap's reach, its plan is only measured.

**Underlay tagged.** A lettering column sewn whole tags as underlay every needle point the satin generator
joins before its first top stitch (`REQ-AUTO-120`), the underlays and the needle's way to them included. Other columns sewn whole keep every stitch tagged top. Tagged on every column, the
underlay changed TS-13D by 9 stitches, since the cap's rules for digitized columns were measured with their
underlay tagged top. That is a step of its own.

**How far a column covers.** A lettering column's stitch covers a place only as its own pull compensation pulls it in, at
each end where the thread turns: `pull_compensation_mm` and `pull_compensation_percent` of the stitch's length,
the larger of the 2 sides' each, and 0.2 mm at the least (`REQ-AUTO-121`). It covers so even where the cap
judges other columns' cover as drawn, above the warning's limit.

**The last resort and lettering.** Above the warning's limit, M13.46's cap takes a satin column's top
stitch as a last resort. On lettering it does not, and the last pass moves no end of a lettering column's
stitches back along it (`REQ-AUTO-123`): where 3 columns cross at 7.8 fills, it narrowed a column by 2 stitches. Measured on the 744 letters and the
words with the rules above and with the last resort, against the rules alone:

| | Rules above | With the last resort |
|---|---:|---:|
| Thickest spot, letters alone, Barstitch Bold | 4.13 to 4.68 | 3.96 to 3.99 |
| Thickest spot, letters on a field, Barstitch Bold | 5.47 to 5.74 | 4.64 to 5.01 |
| Thickest spot, letters on a field, Small Font | 4.87 to 5.35 | 4.44 to 5.35 |
| The marked patch | 4.53 | 4.38 |
| Columns' top thread taken | none | 5,364 mm |
| Notches, a joined stitch along a column's edge | 0 | 629 |
| Stitches turned 10° to 45° | 0 | 137 |
| Joins with more fabric bare, of 1,444 (M13.49's measure of the stitched letter) | 16 | 245 |
| Joins where the line between 2 columns' stitches shows longer | 11 | 149 |

Moving today's letters 0.05 mm, the noise of these measures, bares 12 joins more and makes the line between 2 columns longer at 10. The
rules' 16 barer joins are 0.0025 mm² more at the median and 0.06 mm² at the most, on Barstitch Bold's 6 and
e at 32.5 mm on a field. In the stitched previews the last resort ragged the inner edge of an S and an 8 and changed the angle of
an X's crossing, a Z's corner and an M's top. The owner chose lettering for how it looks. And on a field it
leaves the thickest spots above the limit all the same, where the field and 2 columns' tops make 3.0 to 3.8
fills by themselves.

**Measured** on the build, on 2026-11-20, against `main` at M13.46's merge, over the same sets:

| Set | Thickest today | Built | At 3 fills or more today, mm² | Built | At 4 or more today | Built |
|---|---:|---:|---:|---:|---:|---:|
| Small Font alone | 3.72 to 4.28 | 3.71 to 3.85 | 21 | 16 | 0 | 0 |
| Small Font on a field | 4.87 to 5.35 | 4.87 to 5.35 | 2,048 | 2,038 | 20.0 | 19.0 |
| Barstitch Bold alone | 4.99 to 5.36 | 4.13 to 4.72 | 568 | 266 | 14.5 | 4.8 |
| Barstitch Bold on a field | 6.31 to 6.67 | 5.79 to 5.96 | 7,467 | 7,015 | 677.8 | 325.0 |
| Made-up words, Barstitch Bold, on a field | 5.95 to 6.60 | 5.15 to 6.10 | 1,071 | 1,016 | 138.0 | 53.2 |
| The marked patch | 4.53 | 4.53 | 75.25 | 75.00 | 0 | 0 |
| TS-13B's words marked | 4.16 | 3.68 | 11.00 | 2.50 | 0 | 0 |

No column's top stitch moved on any letter, word or picture. The cap took 3,936 mm of the font's running
stitches under later columns, and the last pass shortened the locks at jumps. It took
3 needle points of a column's underlay. The prototype took 790 mm of underlay, since it counted any later
element's top over it, the font's running stitches too, where the build counts another column's only
(`REQ-AUTO-122`). So Barstitch Bold on a field sews 0.2 to 0.3 fills more at its thickest than on the
prototype. The jumps, the jump threads, 210 in all, and the trims are those of today on every design, and
the plans have 0.8 % fewer stitches. Planning every design together took 5.0 s, against 4.5 s today, on the
shared machine at opt-level 1. A set of 62 letters alone had no cap to run before, and now takes 7 to 95 ms
more.

The look of the 1,444 joins, by M13.49's measures of the stitched letters, against `main` moved 0.05 mm:

| Joins worse, of 1,444 | Moved 0.05 mm | Built |
|---|---:|---:|
| More fabric bare | 12 | 16 |
| A longer line between 2 columns' stitches | 10 | 14 |
| A raggeder edge, of about 1,000 | 10 | 22 |

The built letters bare no join by more than 0.03 mm², and 0.0003 mm² at the median. All the worse joins are
Barstitch Bold's, 6 of the barer ones on its e and 4 on its 6. At 32.5 mm on a field the cap took 10 needle
points of the e's running stitches under its columns, and 8 of the 6's. M13.47's lean changed at 328 joins, 242 of them less: it
reads the running stitches, which lie hidden under the columns. The previews of 17 letters in each font at 2
heights on a field, and of the marked patch, look as today's.

The rules leave Small Font as it was. Its thickest spots are 2 columns' tops, the field and the base
layer, and a centre walk along each column, and none of those may go. The marked patch keeps its spot of 4.53
fills, where 2 columns meet over the field between the M and the A.

**Decisions for the owner.**

- Lettering that digitizing places is in the cap's reach, and a person's own lettering is warned of and
  never changed. Recommended, since the rule doesn't read the font, the text or the size.
- The last resort leaves lettering's columns whole (`REQ-AUTO-123`). Recommended, since it changed the
  letters' look on the previews and still left the spots on a field above the limit. The other choice is
  M13.46's rule for every column, which took the marked patch to 4.38.
- The field sews whole under lettering. Recommended: cut away, it turned its rows at the letters' edges
  and sewed thicker.
- Small Font on a field stays at 4.9 to 5.4 fills at its thickest joins, and the marked patch at 4.53. What
  MC-8 shows decides whether a word on a patch needs more. A lighter field under the letters would be a
  setting of the field's, in a step of its own. M13.50 doesn't add one.

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
`REQ-AUTO-095`, M13.44). The setting is 0.85 by default since M13.45. At 0 every line merges, as before the
rule.

It was merged at 0, off, because keeping lines can make a spot sew thicker far from any line. A line kept sews as
a running stitch, and the sewing order then puts the blocks of a colour in another order. A fill that sewed
first at a spot can then sew over others there. On one photo measured at 0.85 the thickest spot rose from 5.60
to 7.03 fills, at a place 3.6 mm or more from every line kept, where the block order had changed. Merging any
quarter of the lines kept there left 7.03 to 7.62 fills. Merging all of them put that spot back, and the
thickest spot moved to another place at 7.55. On another photo the thickest spot moved too, and the block
order changed there as well. Where a reorder piles thread can't be told from a line, so no rule about lines
prevents it, and planning twice to check costs a second plan. The answer is one cap on thickness at the end of
planning that every design meets, whatever its order (M13.35). With the cap merged, the owner turned the
setting on by default (M13.45). [Details last](#details-last) gives the numbers of both settings on.

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

**Off by default** (`REQ-AUTO-098`, M13.48). The rule runs only when the design's lowest automatic
shape has `auto_details_last` on. Off, the order skips the search for details, and a design sews as it did
before M13.40. The rule landed off, and waited for one cap on thickness at the end of planning (M13.35). Reordering fills can
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
anywhere, and so M13.45 turned the setting on by default. M13.48 turned it off again, for its jump
threads ([off by default again](#details-last), below).

**A detail inside a detail** (`REQ-AUTO-108`, M13.45). A detail can have details of its own, as a pupil on
an iris or a window in a door does. With both settings on, a traced dog's grey area that the darker fur
closes in on moved to sew after the fur, while the tan patch on it, which would cost a thread change to
move, stayed where it was. The grey's outline and rows then lay over the tan's edge. So a detail moves only
together with what sews on top of it. A wish moves its second element later. Its nest is that wish, each
wish that something sews after that element, a detail of it or a satin column bordering it, and the
nests of those. The search grants a nest whole or not at all. It also keeps the row itself from doing the
same. Of the rows with the fewest changes it takes the one that misses the fewest inner wishes, those whose
first element another wish moves, and only then the one that misses the fewest wishes. So wherever a row
with as few changes keeps a pupil on top, the search doesn't take one that sews the iris after the fur and
leaves its pupil under it. A nest goes by the shapes' sizes and where they meet. It doesn't need a second
plan or a measure of what shows. The cases in `tests/auto_details.rs` and `order/fewest/tests.rs` draw a pupil on
an iris where 3 patches meet.

Making every smaller area that a moving detail meets one of its followers, as the nest's own wishes are,
was measured and not taken. With more inner pairs to weigh in the choice of the row, the search took a row
that left the dog's tan patch under the grey's edge again. The small areas of the dogs that show less, by
each area's own pixels that show its thread (`fabric::shown`), were as many as before.

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

**Both on by default** (M13.45, `REQ-AUTO-103`). With the cap merged, the owner turned on details last and
merging by likeness, at 0.85 ([merging by likeness](#merging-by-likeness)), on 2026-11-18. Each design of the
repository sews as before, stitch for stitch, but the traced dogs and the marked patch drawn abutting, which
now sews as the stacked one does. Measured on 2026-11-19 at opt-level 2,
main before M13.45 and then this step, with the area at 3 fills or more in mm², the bare share in
thousandths of a percent and the likeness in thousandths. What shows of each area is its own pixels that
show its thread (`fabric::shown`): the last column counts the areas under 20 mm² that show 0.1 mm² or more
less, and more, with the sum of each. The photos marked * are local:

| Picture | Thickest, fills | Area | Bare | Likeness | Jump threads | Thread changes | Stitches | Small areas showing less, more |
|---|---|---|---|---|---|---|---|---|
| Dog on turf, 6 threads | 4.00, 4.00 | 137.25, 126.00 | 10, 21 | 814, 856 | 57, 61 | 8, 8 | 9,985, 9,999 | 2 of 80 (0.6 mm²), 12 (9.4 mm²) |
| Dog on turf, 9 threads | 4.00, 4.00 | 150.75, 130.00 | 10, 10 | 779, 829 | 58, 66 | 12, 12 | 10,675, 10,697 | 7 of 98 (6.0 mm²), 24 (18.9 mm²) |
| Running dog, 6 threads | 4.00, 4.00 | 129.00, 106.75 | 0, 1 | 898, 898 | 49, 56 | 8, 8 | 11,077, 10,853 | 9 of 65 (8.7 mm²), 14 (15.8 mm²) |
| Running dog, 9 threads | 4.00, 4.00 | 176.50, 173.75 | 5, 2 | 921, 921 | 64, 71 | 11, 11 | 12,369, 12,546 | 7 of 86 (5.2 mm²), 16 (16.9 mm²) |
| Marked patch, abutting | 3.95, 3.98 | 14.25, 14.25 | 0, 0 | 1000, 1000 | 1, 1 | 3, 3 | 4,077, 4,054 | none |
| A magnolia * | 4.00, 4.00 | 441.75, 470.50 | 5, 6 | 480, 848 | 155, 335 | 9, 10 | 34,258, 36,938 | 107 of 1,982 (65.1 mm²), 266 (359.7 mm²) |
| A coleus * | 4.22, 4.01 | 851.50, 899.00 | 6, 53 | 454, 839 | 237, 511 | 12, 13 | 56,886, 60,741 | 231 of 3,932 (139.5 mm²), 461 (623.7 mm²) |
| A greylag goose * | 4.00, 4.00 | 482.50, 473.25 | 7, 16 | 483, 822 | 95, 171 | 8, 10 | 36,668, 37,981 | 35 of 1,784 (18.0 mm²), 140 (211.7 mm²) |
| A castle * | 4.00, 4.00 | 221.75, 228.50 | 1, 10 | 434, 779 | 54, 114 | 8, 9 | 26,201, 27,892 | 30 of 1,412 (26.5 mm²), 86 (125.6 mm²) |

The machine file holds as many stitches as the plan. The dogs keep their thickest spots within the cap, and
all 4 sew less area at 3 fills or more. The lines kept and the details moved cost the dogs 4 to 8 jump
threads and the photos 60 to 274. More small areas show more than show less: the details now sew on top.
What got worse:

- Each local photo changes thread once or twice more. A line kept sews after the fills it borders, which
  can bring its thread back. Which lines did it was not traced.
- The coleus keeps a spot at 4.01 fills, where it kept 4.22, and `SC-W0913` names it. It also leaves 2
  slivers, and its bare share rises from 0.006 % to 0.053 %, within the cover's limit of 0.5 %.
- 2 to 14 % of the small areas show less. On the running dog in 6 threads a patch of 16 mm² shows 2.2 mm²
  less: an area of 108 mm² beside it now sews after it. The search took another row of threads with as few
  changes, to grant wishes elsewhere, and that area's thread now sews later. A nest binds only what a wish
  moves, not a whole thread's batch that a change of row moves. Traced by the order of the 2 areas alone.

Planning takes longer where lines are kept, since each line is measured as a part: by the clock on a shared
machine, the local photos but the magnolia took 1.8 to 2.2 times as long. Counted in instructions by CI,
the dogs' exports took 10.6 to 13.3 % more and the pear's 9.4 % more before nests, and the test sheets as
much as before (`conformance/perf.toml`).

These leave every limit as it was, since a limit comes from the whole set.

**Off by default again** (M13.48, `REQ-AUTO-098`). Shown the cost of each setting alone on the traced dogs,
the owner kept merging by likeness on and turned details last off by default on 2026-11-19. Measured at
opt-level 2 with M13.45's nests, each dog with merging by likeness on: the jump threads with details last
off, then on. The last column counts the areas under 20 mm² that details last makes show 0.1 mm² or more
more, and less, with the sum of each, as the table above does:

| Picture | Jump threads, off, on | Likeness | Small areas showing more, less |
|---|---|---|---|
| Dog on turf, 6 threads | 60, 61 | 856, 856 | 2 of 80 (2.7 mm²), none |
| Dog on turf, 9 threads | 62, 66 | 829, 829 | 20 of 98 (12.1 mm²), 7 (5.8 mm²) |
| Running dog, 6 threads | 49, 56 | 898, 898 | 14 of 65 (15.8 mm²), 9 (8.7 mm²) |
| Running dog, 9 threads | 64, 71 | 921, 921 | 16 of 86 (16.9 mm²), 7 (5.1 mm²) |

Merging by likeness alone costs the dogs 3, 4, 0 and 0 jump threads, and raises the likeness of the 2 dogs
where it costs. Details last adds 1, 4, 7 and 7 more and leaves the likeness as it is. The owner's machine
trims no jump thread within a colour, so each is cut by hand. Most of the cost is the row of threads: to
grant a wish, the search takes another row with as few thread changes, and a thread's whole batch then
sews later. That is the cause M13.45 traced on the running dog in 6 threads. The setting stays, for a
design where an eye or a mark must keep its edge clear. It comes back on by default once the search's
tie-break no longer moves a whole batch of a thread later for a wish, and the dogs show a clear gain for
it.

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

**Left for later.** Bold letters under 12.5 mm, which M14.9 measured and plans to widen
([below](#bold-letters-under-the-bold-fonts-heights)). The effect's dialog has
no list of fonts.

### Bold letters under the bold font's heights

Barstitch Bold sews capital letters from 12.5 mm, the least scale its `font.json` gives, 0.5, times
the 25.0 mm it draws them at. Lettering keeps a scale within the font's `min_scale` and `max_scale` with
`SC-W1106`. Lettering on patches is often 6 to 12 mm tall and bold, and
there a marker or a bold text object gets Small Font's strokes whatever the drawing's weight. M14.9
measured the gap on the 62 letters and digits of both fonts, and 4 ways to close it, on 2026-11-20. The
probes are kept outside the repository, beside the hand-off of the step.

**The measures**, of each letter set alone and on a field 3 mm round it, with M13.50's cap:

- the stroke as sewn, the median length of the satin columns' top stitches with their pull compensation,
  given in millimetres and in heights of the capital letters;
- the narrowest column: the least of a letter's columns' median widths;
- counters: the holes in the thread's cover, 0.5 fills or more on a grid of 0.1 mm, against those the
  font sews at a large height. A counter is open to read when a circle 1 mm wide fits it, the narrowest
  satin column of digitizing;
- letters that run together: the parts of the cover of a sheet of 9 lines against the letters on them;
- the thickest spot, and the area at 4 fills or more on the field.

**Today.** Small Font sews every letter at every height from 5 to 14 mm. No stitch is under 0.3 mm, and no
letter's columns run into the next letter's. Its pull compensation, 0.2 mm on each side, is the same at every
height, so it sews heavier for its height the smaller it is set. Of the 22 counters of its letters:

| Capitals, mm | Stroke sewn, mm | Of the height | Narrowest column, mm | Counters open to read | Thickest, alone and on a field | On a field at 4 fills or more, mm² |
|---:|---:|---:|---:|---:|---|---:|
| 5 | 1.01 | 0.20 | 0.87 | 3, the A's closed | 3.65 and 4.90 | 2.0 |
| 6 | 1.12 | 0.19 | 0.96 | 8 | 4.21 and 5.09 | 3.2 |
| 7 | 1.25 | 0.18 | 1.05 | 16 | 3.95 and 5.03 | 2.8 |
| 8 | 1.38 | 0.17 | 1.15 | 16 | 3.72 and 5.16 | 4.0 |
| 9 | 1.51 | 0.17 | 1.24 | 19 | 3.93 and 5.14 | 4.0 |
| 10 | 1.63 | 0.16 | 1.33 | 21 | 3.93 and 5.22 | 6.8 |
| 11 | 1.74 | 0.16 | 1.43 | 22 | 3.76 and 5.05 | 8.5 |
| 12 | 1.85 | 0.15 | 1.52 | 22 | 4.35 and 4.99 | 13.5 |
| 14 | 2.08 | 0.15 | 1.71 | 22 | 3.80 and 4.94 | 14.2 |

Barstitch Bold at 13 mm sews strokes 3.06 mm wide, 0.24 of the height, with 19 of its 21 counters open to
read, 5.78 fills at its thickest on a field and 64 mm² at 4 fills or more. 18 of the 64 letters of the
sheet touch the next one there, 8 at 16 mm. The font's spacing is drawn tight, and its 0.3 mm of pull
compensation on each side closes it at its least height.

So the gap is narrower than the heights say. At 6 mm Small Font sews strokes 0.19 of the height wide, as
heavy for their height as the traced block letters are drawn (0.18). The difference grows with the height,
to 0.15 against Barstitch Bold's 0.24 at 12 mm.

**The candidates.** Each was planned at every height from 5 to 14 mm. At 6, 8, 10 and 12 mm, stroke
in heights, counters open to read (of 21 for Barstitch Bold, 22 for Small Font), counters closed, the area at
4 fills or more on a field, and the letters of 64 that touch the next:

| Candidate | 6 mm | 8 mm | 10 mm | 12 mm |
|---|---|---|---|---|
| Today, Small Font | 0.19, 8, none, 3, none | 0.17, 16, none, 4, none | 0.16, 21, none, 7, none | 0.15, 22, none, 14, none |
| A, Barstitch Bold set below its least scale | 0.29, 0, 7 closed, 50, 54 | 0.26, 4, a and e, 41, 49 | 0.25, 15, none, 56, 40 | 0.24, 18, none, 57, 24 |
| A2, as A with its pull compensation scaled with the height | 0.24, 0, A a e, 17, 52 | 0.24, 3, none, 29, 40 | 0.24, 17, none, 40, 35 | 0.24, 18, none, 58, 24 |
| B, Small Font with columns widened 33 % | 0.26, 3, A e 4, 35, 2 | 0.25, 14, the 4, 44, none | 0.24, 18, none, 62, none | 0.23, 20, none, 84, none |
| B2, Small Font with columns widened 20 % | 0.23, 3, A and 4, 17, none | 0.22, 16, none, 22, none | 0.21, 19, none, 39, none | 0.20, 21, none, 48, none |

A column widened by a percent gains that part of its width on each side, the effect of Ink/Stitch's
`pull_compensation_percent` added to the glyph's own. Every candidate sews its columns 1 mm wide or more
from 6 mm up and its stitches 0.3 mm long or more, and every letter plans. The thickest spot on a field stays between 5.0 and 5.9
fills in all of them. Barstitch Bold sews 5.78 at 13 mm today.

Where each fails:

- **A and A2** run the letters together. Barstitch Bold's letters touch at 13 mm already, and below it
  more of them do: 35 of 64 at 10 mm with A2, 40 at 8 mm. The counters of a and e close under 9 mm in A, and
  in A2 every counter is still a hole down to 7 mm but under 1 mm wide, too small to read once sewn. A
  wider letter spacing would part the letters and make the word longer than the traced one it replaces.
- **B** closes the 4's counter at 8 and 9 mm, and leaves 14 counters of 22 open to read at 8 mm against
  16 today. It doubles the area at 4 fills or more against B2.
- **B2** keeps today's counters from 8 mm up, 16 of 22 at 8 mm and 21 at 12 mm, and keeps the letters
  apart. Under 8 mm it closes the A and the 4 and leaves 3 of 22 open to read at 6 mm, against 8 today. It
  is a little lighter than Barstitch Bold, 0.20 to 0.22 of the height against 0.24.

**A third font.** Ink/Stitch's library, read at `c7e3a05`, has no bold font for 6 to 12 mm. Its small fonts
end at 8.4 to 9.0 mm by their own ranges, and its medium ones start at 9 to 12. Glacial Tiny 60 AGS (OFL)
is the boldest small one, strokes sewn 0.22 of the height at its greatest size, 8.4 mm by its range, and sews 5.4 fills at
its thickest alone and 6.8 on a field, with 559 mm² at 4 fills or more on a field at that size. Excalibur
Small says it is in the public domain, and its narrowest columns sew 0.53 mm wide. None of them was measured for weight
by M14.8's measure, and none is a better start than the 2 fonts built in.

**What the trade does.** Digitizers put the narrowest satin column at about 1 mm, the needle's width,
and 1.5 mm in a font made for embroidery
([ASI](https://members.asicentral.com/news/how-to/may-2018/keep-your-small-letters-crisp/),
[ZSK](https://www.zsk.de/en/services/how-to-overview/small-embroideries.php),
[Embrilliance](https://embrilliance.com/Help/Platform%20Win%201169/digitizingtipsforfontcreation.htm)).
Columns of small letters are widened, the counters opened and the underlay left out or kept to a centre
walk
([Impressions](https://impressionsmagazine.com/how-to-digitize-small-lettering/)). A quarter inch, 6 mm, is
the usual least height of text in 40-weight thread, and finer thread or a run-stitch font goes below it, a
continuous run at about 3 mm
([Ignition Drawing](https://www.ignitiondrawing.com/blog/Small-Lettering-Guide-Part-2)). A font's least
size, such as 6 mm for a serif text font, is given with the font
([Hatch](https://blog.hatchembroidery.com/choosing-the-right-size-embroidery-lettering-for-your-project/)).
No source gives a least size for counters. The 1 mm used here is the narrowest column's.

**The design** (`REQ-FONT-024`, `REQ-AUTO-140`, `REQ-AUTO-141`). The setting is built, and off by default.
The 2 automatic choices wait for the owner's sew-out to confirm their limits:

- **`widen_percent`**, a lettering setting of StitchCraft's own beside Ink/Stitch's, from 0 to 50 and 0 by
  default. With 20, a satin column 1 mm wide sews 0.2 mm wider on each side: lettering adds the value
  to the `pull_compensation_percent` of every satin column it places. Running stitches and fills don't
  change, and a value outside 0 to 50 is set at the nearest with `SC-W0102`, as the parameters' registry
  holds a value in its range. A glyph's own value that doesn't read stays, for the planner to refuse. At 0
  a text sews as it did before. `stitch lettering --widen-percent`, the Lettering effect's dialog and its
  record take it by that name.
- **A marker** with letters bolder than their font and too small for a bolder one has its text widened.
  The percent is 50 × (the letters' weight over the font's, less 1), rounded and at most 20. A widening under 5 % is left out. Letters under 8 mm are set as today,
  since a widened Small Font closes its counters there. `SC-I1118` gives the percent, or says why
  the letters aren't widened.
- **A bold text object** that the bold font can't sew at its height sews in the default font widened as
  far toward the bold font's weight, with the same limits and the same note.

B2 opens as many counters as today's Small Font from 8 mm up, measured on all 62 letters and digits, and
that sets the 2 limits, 20 % and 8 mm. A sew-out confirms them or moves them, as it does a profile's values.

**The build's numbers.** Measured on 2026-11-20 with the setting itself, on Small Font's 62 letters and
digits. Each cell gives the stroke in heights, the narrowest column in mm, counters open to read of 22, the
thickest spot alone and on a field in fills, and the area at 4 fills or more on a field in mm²:

| widen_percent | 8 mm | 10 mm | 12 mm |
|---|---|---|---|
| 0, today | 0.17, 1.15, 16, 3.72 and 5.16, 4 | 0.16, 1.33, 21, 3.93 and 5.22, 7 | 0.15, 1.52, 22, 4.35 and 4.99, 14 |
| 10 | 0.20, 1.30, 16, 4.04 and 5.14, 10 | 0.19, 1.52, 19, 4.50 and 5.24, 19 | 0.18, 1.74, 21, 4.25 and 5.34, 27 |
| 20 | 0.22, 1.45, 16, 4.25 and 5.24, 22 | 0.21, 1.71, 19, 4.13 and 5.27, 39 | 0.20, 1.97, 21, 4.31 and 5.30, 48 |

They match prototype B2. Small Font's glyphs don't set a pull compensation in percent, so the prototype's
value equals the setting's. All 22 counters of the set are holes in the thread, though the A, the B, the a, the e and the 4 keep
theirs under 1 mm, and at 8 mm and 20 % the R joins them. The 64 letters stand apart, and the narrowest gap
between 2 letters falls from 1.5 to 1.0 mm at 8 mm. Every stitch is 0.3 mm or longer.

**A finding for the sew-out.** Barstitch Bold's letters touch at its least heights today, as measured
above. This step leaves its spacing as it is, and MC-8 shows whether the touching shows in thread.

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
- **Bold letters from 8 to 12.5 mm** (M14.9). The recommendation is Small Font with its columns widened
  ([bold letters under the bold font's heights](#bold-letters-under-the-bold-fonts-heights)), by at most 20 %
  and from 8 mm. Its counters open as today's do, and its letters stand apart. Barstitch
  Bold set below its least height is bolder and runs its letters together. A third font from Ink/Stitch's
  library is the other choice, and none of them sews at 9 to 12 mm by its own range. MC-8 sews the
  candidates side by side.
- **A text object's bold `font-weight` doesn't choose the bold font.** Recommended, and taken: the effect
  sews in the font it names or in the default, so documents sew as they did.

### The build's measure

**Measured** on 2026-11-20, the branch built at opt-level 2, with a measuring program kept out of the
repository. Each design is planned automatic, expanded, and planned again with the same
origin: at the engine, read back from the SVG file `stitch expand` writes, and, for the 5 designs traced
into a VectorCraft document, read back from the document it writes.

| Design | Parts | Engine | SVG file | `.vectorcraft` document |
|---|---:|---|---|---|
| Dog on turf, 6 threads | 157 | bit for bit | bit for bit | 4 needle points fewer of 9,599 |
| Dog on turf, 9 threads | 175 | bit for bit | bit for bit | 2 needle points more of 10,175 |
| Dog running, 6 threads | 150 | bit for bit | bit for bit | within 1.4e-13 mm |
| Dog running, 9 threads | 192 | bit for bit | bit for bit | within 1.5e-13 mm |
| Pear | 10 | bit for bit | bit for bit | within 5.7e-14 mm |
| Lettered patch | 16 | bit for bit | bit for bit | |
| Badges | 16 | bit for bit | bit for bit | |
| Emblem | 10 | bit for bit | bit for bit | |
| TS-13A to TS-13D, digitized halves | 22 to 78 | bit for bit | bit for bit | |
| TS-13E, digitized half | 34 | bit for bit | within 6.4e-14 mm | |

A VectorCraft document keeps points in points. A part's millimetres read back to within a unit in their
last place. Most products of a float and a point's size can't be written to read back exactly. The 2 dogs
on turf sew the same few needle points otherwise at the engine when every point of their expanded design
moves by a unit in its last place, so a fill's row there lies on a rounding's edge. Every other plan is the
automatic one to 1e-13 mm. The traced badge of the command line's case writes the same PES bytes through both files.

**Editing a part.** The largest fill of each expanded design was turned 45°, sewn as a contour fill, or
deleted, and the other parts whose stitches changed were counted. On the badges, the emblem and TS-13A none
to 3 change: the outline that sews with the fill and the part sewn next, which starts nearest where it ends.
On the traced dogs 13 to 97 of 150 to 192 parts change. The cap at the end of planning accounts for most of
them: it thins the plan where it sews thick, and an edited part changes where that is. With the cap out of
reach, the turn doesn't change another part on the 9-thread dog on turf, and changes 18 on the 9-thread
running dog: the parts the needle sews under between 2 parts, and the next ones. A person who wants a part left alone by the cap sets its
`digitized_as` to `no`.

Stitched previews of the running dog, the lettered patch and TS-13C, automatic beside expanded beside
expanded with its largest fill turned, show the first 2 alike. The golden files stay as they were, since every
design sews as before.

### What isn't built

The Expand Embroidery filter in VectorCraft waits for the RFC's P11 and P10. Until then the person runs
`stitch expand` and opens the document. The manifests declare `digitized_as` and `turn_stitches`
already (`REQ-VC-035`).

## Marks that keep parts

Automatic digitizing decides by geometry and measures alone which small shapes are specks to merge (step 4 of
[Neighbours](#neighbours)) and which thin lines merge into their neighbours (step 3, and
[merging by likeness](#merging-by-likeness)). [Details last](#details-last) decides by geometry too which parts
sew last. Geometry can't know which small part a person cares about. The same speck is noise in one picture and
the point of another, such as a bird's eye or the dot over a letter, and in other pictures a flower's centre or
a window. M13.39 lets the person say so with a mark, which works on any subject and names none.

### The rule

A mark is an area of the design. Automatic digitizing sews the parts that lie wholly inside it by their width,
where it would merge them, and changes nothing else (`REQ-AUTO-104` to `REQ-AUTO-107`).

- **Which parts.** The parts of the automatic areas as the layers leave them once flattened (step 2), each one
  connected piece of one area with its holes. The speck and line steps look at these pieces. A part is inside
  a mark when every point of its outline is, flattened within 0.05 mm as a text marker's subpaths are.
- **Specks.** Automatic digitizing doesn't merge a marked speck into the area beside it, or leave it out. It
  sews by its width, as a larger part does. Narrower than the narrowest satin stroke everywhere, it is a
  running stitch along its middle, and wider, satin columns or a fill. A speck of 0.8 mm², 1.6 mm by 0.5 mm,
  sews a running stitch 1.6 mm long.
- **Thin lines.** A marked thin line sews as a line in its own thread, whatever the likeness bound
  (`auto_min_likeness`).
- **Colour.** Merged, a part sews in its neighbour's thread, and marked, in its own. The threads of a picture
  are the trace's, and a mark doesn't change them.
- **Partly inside.** Automatic digitizing decides a part that crosses the mark's edge as if there were no mark,
  since a part cut at the mark's edge would sew an edge the picture doesn't have. A person who wants it drawn
  draws the mark larger.
- **Left alone.** The rules of the background, the flattened layers, the widths, the order, the overlaps, the
  base layer and the cap on thickness go as without a mark. The order they find can differ, since every part
  sewn is a shape more ([what marks sew](#what-marks-sew)). A marked part reaches under its neighbours, and
  they reach under it, as for any part. A speck or a line outside the mark may still merge into it. Details
  last stays its own setting: a marked detail sews last when `auto_details_last` is on, as it would unmarked.
  The mark doesn't name a stitch. For another stitch, a person gives the part its own effect, as for any
  shape.

One mark covers what the row of M13.39 asked: a marked part's lines and specks don't merge, and it sews by the
rule of its width. A second mark, to merge what the rules sew, isn't proposed, since the pictures measured
didn't need one.

### Drawing a mark

A mark is a shape named `stitch:keep`, or `stitch:keep` and a note of the person's own, such as `stitch:keep
left eye`. The name is read as every marker's: in any case, after any white space. Its area is read as a
[text marker's](#the-marker) is, by the same code. A closed shape's area is what its outline encloses. For an
open line with a stroke, it is the band within half the stroke's width of the line, between its ends. A line suits a whisker or a twig. The mark needs a
fill or a stroke, since a reader is given only objects that paint, and a rectangle or an ellipse drawn with a
stroke and no fill shows the picture under it. It is never sewn.

- **In VectorCraft**, the person draws the mark with VectorCraft's own tools over the traced picture and names
  it in the Layers panel. VectorCraft 0.23.0 gives a plug-in no canvas tool. A group or a layer so named is not
  a mark, and `SC-W0807` says so (`REQ-VC-031`).
- **In an SVG file**, the mark is an element whose `inkscape:label` is the name.
- **From the command line**, `--keep LEFT,TOP,WIDTH,HEIGHT` marks a rectangle in percent of the box round the
  design's shapes, as `--text` does, read by the same code (`REQ-CLI-021`). The marks are named `keep-1`,
  `keep-2` and so on, and a bug-report bundle keeps them with the rules' settings.

```sh
stitch plan bird.png -o bird.pes --keep '62,18,6,5'
```

**Read when the design is planned.** The roadmap's row planned an Apply Marks command. Instead a reader takes
the marks out of the design's elements and gives them to the design, as it reads text markers, so `stitch`,
File › Export and the bug report each read them on every plan. A command would copy what the names already say,
and its result would go stale when the picture is traced again. The Auto effect's preview shows each shape
alone and doesn't merge, so it looks the same with a mark or without one
([previews](#previews-in-vectorcraft)).

### Where it runs

The readers take each mark out of the design's elements and give it to the design, with its id
(`stitchcraft_engine::design::mark::Mark`, `mark::taken_out`). An object painted with a fill and a stroke
is read as 2 elements of one path, and is one mark. The reading of a marker's area lies in the engine
(`design::mark::Area`), and the text markers of `stitchcraft_svg::lettering::pictures` use it from there.
Planning reads each mark's area once (`auto::marks`), with the budget's work to itself. Automatic
digitizing tests a part against the marks where a step would take the part away: in the width rules, which
leave a speck in its area's fill (`auto::narrow`), and in the steps of thin lines (`auto::neighbours::lines`)
and of specks (`auto::neighbours::specks`). The step of thin lines tests every part of every automatic
area, so that each mark knows what it holds. Testing a part costs a unit for each mark, and where the box
round the part lies in the box round the mark, a unit for each of the part's points and each segment of
the mark, charged to the budget that step has for the part. A part that needs more is decided as if there
were no mark, and `SC-I0916` says so. Without marks, nothing is tested and nothing is charged.

`SC-I0916` names each mark with the parts that lie wholly inside it, and how many of them sew that would have
merged, as specks and as thin lines. `SC-W0917` names a mark that encloses no area, a mark with no part wholly
inside it and a mark in a design without an automatic shape, which then sews as without it.

### What marks sew

Measured on 2026-11-19 with the build, on `main` with M13.35's cap and before M13.45, so with both
`auto_min_likeness` and `auto_details_last` off (`stitch plan --auto`'s numbers, as M13.41's
measure and M13.29's count them, and the stitches of the PES file read back). No picture is a goal: one
procedure marked each, never an eye. Each picture was planned without marks. Its 3 largest parts that the
plan loses ([likeness](#likeness)) were then each marked by the box round them, grown by 0.5 mm, as a person
draws a rectangle round an eye. A picture that loses none was marked round its 3 smallest parts, and TS-13A
round 3 of the specks it merges.

| Picture | Marks | Likeness | Parts kept | Thickest spot, fills | At 3 fills or more, mm² | Jump threads | Thread changes | Stitches | Export's count | Specks merged | `SC-I0915` notes |
|---|---|---|---|---|---|---|---|---|---|---|---|
| Dog on turf, 6 threads | none | 0.814 | 75 of 97 | 4.00 | 137.25 | 57 | 8 | 9,985 | 9,985 | 0 | 22 |
|  | 3 | 0.832 | 78 of 97 | 4.00 | 130.50 | 59 | 8 | 10,045 | 10,045 | 0 | 19 |
| Dog on turf, 9 threads | none | 0.779 | 83 of 111 | 4.00 | 150.75 | 58 | 12 | 10,675 | 10,675 | 0 | 29 |
|  | 3 | 0.814 | 87 of 111 | 4.00 | 154.75 | 62 | 12 | 10,856 | 10,856 | 0 | 25 |
| Running dog, 6 threads | none | 0.898 | 66 of 77 | 4.00 | 129.00 | 49 | 8 | 11,077 | 11,077 | 0 | 14 |
|  | 3 | 0.942 | 69 of 77 | 4.00 | 134.50 | 52 | 8 | 11,170 | 11,170 | 0 | 11 |
| Running dog, 9 threads | none | 0.921 | 88 of 100 | 4.00 | 176.50 | 64 | 11 | 12,369 | 12,369 | 0 | 15 |
|  | 3 | 0.938 | 91 of 100 | 4.00 | 190.00 | 67 | 12 | 12,473 | 12,473 | 0 | 12 |
| Pear | none | 1.000 | 5 of 5 | 3.86 | 13.00 | 2 | 2 | 4,660 | 4,660 | 0 | 0 |
|  | 3 | 1.000 | 5 of 5 | 3.86 | 13.00 | 2 | 2 | 4,660 | 4,660 | 0 | 0 |
| Lettered patch | none | 1.000 | 8 of 8 | 3.98 | 40.25 | 8 | 2 | 4,026 | 4,026 | 0 | 0 |
|  | 3 | 1.000 | 8 of 8 | 3.98 | 40.25 | 8 | 2 | 4,026 | 4,026 | 0 | 0 |
| TS-13A | none | 1.000 | 16 of 16 | 3.99 | 23.75 | 18 | 2 | 11,513 | 11,513 | 7 | 0 |
|  | 3 | 1.000 | 16 of 16 | 3.99 | 23.75 | 23 | 3 | 11,563 | 11,563 | 0 | 0 |
| TS-13B | none | 1.000 | 31 of 31 | 3.85 | 4.25 | 46 | 4 | 6,895 | 6,895 | 0 | 0 |
|  | 3 | 1.000 | 31 of 31 | 3.85 | 4.25 | 46 | 4 | 6,895 | 6,895 | 0 | 0 |
| TS-13C | none | 1.000 | 11 of 11 | 3.96 | 95.75 | 13 | 4 | 6,064 | 6,064 | 0 | 0 |
|  | 3 | 1.000 | 11 of 11 | 3.96 | 95.75 | 13 | 4 | 6,064 | 6,064 | 0 | 0 |
| TS-13D | none | 1.000 | 14 of 14 | 3.97 | 50.75 | 12 | 3 | 7,994 | 7,994 | 0 | 0 |
|  | 3 | 1.000 | 14 of 14 | 3.97 | 50.75 | 12 | 3 | 7,994 | 7,994 | 0 | 0 |
| TS-13E | none | 1.000 | 18 of 18 | 3.39 | 2.50 | 5 | 2 | 2,716 | 2,716 | 0 | 0 |
|  | 3 | 1.000 | 18 of 18 | 3.39 | 2.50 | 5 | 2 | 2,716 | 2,716 | 0 | 0 |
| Badge, abutting | none | 1.000 | 3 of 3 | 3.01 | 0.00 | 0 | 2 | 3,431 | 3,431 | 0 | 0 |
|  | 3 | 1.000 | 3 of 3 | 3.01 | 0.00 | 0 | 2 | 3,431 | 3,431 | 0 | 0 |
| Badge, stacked | none | 1.000 | 3 of 3 | 3.01 | 0.00 | 0 | 2 | 3,431 | 3,431 | 0 | 0 |
|  | 3 | 1.000 | 3 of 3 | 3.01 | 0.00 | 0 | 2 | 3,431 | 3,431 | 0 | 0 |
| Emblem, abutting | none | 1.000 | 6 of 6 | 3.57 | 1.25 | 2 | 3 | 4,997 | 4,997 | 0 | 0 |
|  | 3 | 1.000 | 6 of 6 | 3.57 | 1.25 | 2 | 3 | 4,997 | 4,997 | 0 | 0 |
| Emblem, stacked | none | 1.000 | 6 of 6 | 3.39 | 1.00 | 2 | 3 | 4,998 | 4,998 | 0 | 0 |
|  | 3 | 1.000 | 6 of 6 | 3.39 | 1.00 | 2 | 3 | 4,998 | 4,998 | 0 | 0 |
| Button | none | 1.000 | 9 of 9 | 3.96 | 2.75 | 0 | 2 | 1,847 | 1,847 | 0 | 0 |
|  | 3 | 1.000 | 9 of 9 | 3.96 | 2.75 | 0 | 2 | 1,847 | 1,847 | 0 | 0 |
| Marked patch | none | 1.000 | 5 of 5 | 3.95 | 14.25 | 1 | 3 | 4,077 | 4,077 | 0 | 0 |
|  | 3 | 1.000 | 5 of 5 | 3.95 | 14.25 | 1 | 3 | 4,077 | 4,077 | 0 | 0 |
| Magnolia | none | 0.480 | 233 of 496 | 4.00 | 441.75 | 155 | 9 | 34,258 | 34,258 | 1,522 | 277 |
|  | 3 | 0.504 | 246 of 496 | 4.00 | 490.25 | 184 | 9 | 35,092 | 35,092 | 1,441 | 264 |
| Coleus | none | 0.454 | 395 of 841 | 4.22 | 851.50 | 237 | 12 | 56,886 | 56,886 | 3,250 | 487 |
|  | 3 | 0.461 | 401 of 841 | 4.22 | 877.25 | 282 | 12 | 57,527 | 57,527 | 3,175 | 480 |
| Goose | none | 0.483 | 167 of 342 | 4.00 | 482.50 | 95 | 8 | 36,668 | 36,668 | 1,486 | 184 |
|  | 3 | 0.493 | 171 of 342 | 5.09 | 492.75 | 114 | 8 | 37,160 | 37,160 | 1,453 | 180 |
| Castle | none | 0.434 | 81 of 191 | 4.00 | 221.75 | 54 | 8 | 26,201 | 26,201 | 1,334 | 116 |
|  | 3 | 0.449 | 85 of 191 | 4.00 | 230.25 | 69 | 8 | 26,715 | 26,715 | 1,302 | 112 |

- **The marked parts sew.** Of the 48 parts the likeness measure finds inside the marks of the dogs and the
  photos, 47 sew. The other, a part of 5 mm² in the goose, sews as a line that shows less than half its area. Each photo's
  likeness rises, by 0.007 for the coleus to 0.044 for the running dog in 6 threads.
- **The pictures whose marks merge nothing sew as without them,** stitch for stitch: the pear, the lettered
  patch, TS-13B to TS-13E, the badges and emblems traced either way, the button and the marked patch. No part
  lies wholly inside a mark of the lettered patch or inside any of TS-13C's 3, since the part the likeness
  measure sees is a piece of a larger area, and `SC-W0917` says so. TS-13A sews all 7 of its marked specks
  as short running stitches, with 5 more jump threads, 1 more thread change and 50 more stitches. 96 of its stitches far from the marks
  start or end elsewhere.
- **Outside the marks, a traced photo sews in another order.** Each marked part is an element of its own,
  and the order of the whole design ([sewing order](#sewing-order)) is found again. Of the stitches more than
  2 mm from every mark, 51 to 1,394 of a dog and 1,629 to 7,468 of a photo start or end elsewhere. So the
  area at 3 fills or more moves both ways, from 6.75 mm² less on the dog on turf in 6 threads to 48.5 mm²
  more on the magnolia.
- **The cap holds but for one spot.** Every picture sews at most 4 fills where it did, but the goose: a
  spot about 19 mm from its marks sews 5.09 fills, where one shape of grass lays 2.6 fills by itself, and
  `SC-W0913` names it "above the cap". The coleus sews 4.22 fills with marks and without them.
- **Marked specks cost jump threads.** The boxes round the parts of a traced photo take in specks of the
  trace's noise too: 32 to 84 of them in a photo's 3 marks, and 4 to 13 thin lines. They cost the photos 15
  to 45 jump threads. The dogs' marks took in no speck, and cost 2 to 4.
- **What it costs to plan.** Without marks nothing is tested, and every picture sews as on `main`. With
  the pear's 3 marks, `stitch export --keep` took 1,074,680 instructions more of 3.18 billion (0.03 %),
  testing every part of every area against the marks, and wrote the same file. With the marks of the
  running dog in 6 threads it took 3.4 % more, 29.38 billion, for the parts that now sew and the new order
  (callgrind, a build at opt-level 1).

### Decisions

The owner's choices of 2026-11-18, each the one recommended. The owner confirmed the 4th, that marks keep
specks too, on 2026-11-18.

- **One mark, `stitch:keep`.** The row asked that a marked part's lines and specks don't merge, and that it
  sews by its own rule. That is one rule, so one mark does both. Other names were `stitch:detail`, which
  suggests the order of [details last](#details-last) that a mark doesn't change, and `stitch:favour`, which
  doesn't say what it does. Chosen: `stitch:keep`.
- **Read when planned, with no Apply Marks command,** as text markers are. Chosen: no command.
- **Wholly inside.** Automatic digitizing decides a part crossing the mark's edge as if it were unmarked. One other
  choice sews a part when most of it is inside, which is a guess. Another cuts a part at the edge, which sews an
  edge the picture doesn't have. Chosen: wholly inside, as text markers take their letters.
- **Marks sew specks too.** The row asked that a marked part's specks don't merge, and a dot under 1 mm², such
  as the light in an eye, is a speck. A mark also takes in the specks of a trace's noise, which cost the photos
  7 to 39 jump threads and no likeness. The other choice sews marked thin lines and merges specks.
  Chosen: sew specks, since a mark is the person's word. `SC-I0916` counts the specks, so a person can
  draw a tighter mark.
- **A marked speck sews by its width,** usually a short running stitch. The other choice is a fill, which sews
  a knot under 1 mm². Chosen: by its width.
- **The rest of a photo may sew in another order.** Each marked part is a shape more to order. Holding the
  rest of the order fixed would need a second order for marked designs. Chosen: accept it, and let
  M13.35's cap bound the thickness, as for details last and likeness.
- **Details last stays its own setting.** A mark could also sew its parts last. That moves fills, which can
  thicken a spot elsewhere, as M13.40 measured. Chosen: leave the order alone until the cap is merged and
  `auto_details_last` is turned on.

### What isn't built

- The Auto effect's preview in VectorCraft shows each shape alone, so it can't show what a mark sews. The
  exported file and its preview picture do.
- A mark that merges what the rules sew, or that names a stitch.

## A guide on one shape

Automatic digitizing chooses the rows of each fill by geometry: every fill has the rows of `auto_fill_angle`.
No rule can know the form that the rows of fur or a ribbon follow. The person draws it. Before M13.24 a guided fill needed the shape taken out of automatic digitizing by hand,
which lost its overlaps with its neighbours, the base layer under it and its place in the sewing order. A
guide keeps all of them, and works on any subject without naming one.

### The rule

A guide is a line the person draws over the picture. Each filled part of an automatic area that the line
crosses sews as a guided fill whose rows follow it. The other parts and the steps after them sew as
without the guide (`REQ-AUTO-109` to `REQ-AUTO-111`).

- **Which parts.** The parts the rules by width leave filled ([stitch types by width](#stitch-types-by-width)):
  the broad parts of an area, its compact and uneven parts, and the broad pieces of a part split by width. A
  part follows a guide when a stretch of the guide's line lies inside it where the area shows. An area
  digitized whole under the details that fill its holes ([neighbours](#neighbours), `REQ-AUTO-023`) shows only round them, so a
  guide drawn over a detail steers the detail and leaves the area under it alone. Specks, under the smallest area
  `auto_min_area_mm2`, stay in the area's fill and merge as before.
- **The parts it crosses.** Unlike a mark, which keeps the parts lying wholly inside it, the guide steers the
  parts it crosses. The guided fill lengthens the guide straight on at both ends until its rows cover
  the whole part ([guided fill](algorithms/guided.md)). A guide that crosses only a corner of a large part steers all of
  it. The person sees that in the exported file's preview and draws the guide again.
- **Parts sewn along their middles keep their stitches.** A satin column already lays its stitches across
  its width, which follows the part's form, and a running stitch follows its centre line. Such a part
  sews the same stitches when a guide crosses it, and `SC-I0918` counts it.
- **2 guides on one part.** The part follows the guide drawn last, the one a person sees on top. Blending
  2 guides would guess what the person meant.
- **One element for each area and guide.** The parts of one area that follow one guide are one element,
  since a guided fill sews several parts nearest the needle first. These elements come after the area's
  other elements, its fill and its parts sewn along their middles, in the order the guides are drawn, so
  that the other elements keep their names. The first element of an area keeps its name and the others are
  numbered, so a guided fill is the area's own element when nothing else of the area sews. The area keeps
  its thread.
- **Left alone.** The background, the flattened layers, the specks and the thin lines go as without a guide,
  and so do the rules by width, the overlaps, the base layer and the cap on thickness. The guided fill takes
  its own defaults and the area's other settings. The base
  layer keeps its rows across `auto_fill_angle`, so under a guided fill it lies at an angle that changes
  along the rows. Each element a guide makes is a shape more to order, so the order can change
  ([what guides sew](#what-guides-sew)).

### The guide's line

The guide's line is its first subpath, as a guided fill reads its guide with the copy strategy, its
default. Its stroke's width is ignored. A closed guide is opened where it
closes, as a guided fill opens it. Other subpaths are ignored.

### Drawing a guide

A guide is a shape named `stitch:guide`, or `stitch:guide` and a note of the person's own, such as
`stitch:guide left petal`. The name is read as a mark's, by the same code (`design::mark::note`), and the
readers take guides out of the elements beside the marks (`mark::taken_out`). It is never sewn.

- **In VectorCraft**, the person draws the guide with the Pen or Pencil tool over the traced picture and
  names it in the Layers panel. VectorCraft 0.23.0 gives a plug-in no canvas tool. `stitch:guide` is the
  name VectorCraft documents already give a guide grouped with the shapes it steers ([the markers
  table](vectorcraft-integration.md)). A path so named keeps that meaning for shapes with settings, and is
  also a guide of the design for every automatic area it crosses, wherever it lies in the layers. A traced
  picture lies in one layer, and a person draws all its guides in that layer. A guide its group doesn't use is no
  longer said to be unused (`SC-W0807`) when the design has an automatic shape, and an automatic shape
  grouped with a guide alone is no longer told that its markers are unused. `SC-W0919` names a guide that
  steers nothing (`REQ-VC-032`).
- **In an SVG file**, the guide is a path whose `inkscape:label` is the name. Ink/Stitch's guide line, a
  path with its marker grouped with a shape, keeps its Ink/Stitch meaning and guides only that group.
- **From the command line**, `--guide X,Y,X,Y…` draws a guide through 2 points or more, in percent of the
  box round the design's shapes as `--keep` reads its numbers, joined by straight lines. A few points
  serve, since the guided fill smooths its rows. The guides are named `guide-1`, `guide-2` and so on, and a
  bug-report bundle keeps them with the rules' settings (`REQ-CLI-022`).

```sh
stitch plan leaf.png -o leaf.pes --guide '20,80,45,50,60,20'
```

**What the notices say.** `SC-I0918` names each guide with the areas and the number of parts that follow
it, and the parts it crosses that keep their stitches. `SC-W0919` names a guide that crosses no filled
part of an automatic area, whose line has no length, or that is in a design with no automatic shape. The
design then sews as without it. The Auto effect's preview in VectorCraft draws straight rows until a live
effect is given its document's other objects (the [RFC](rfc-vectorcraft-abi-v2.md)'s P6). The exported
file and its preview picture follow the guide.

### Where it runs

Planning reads each guide's line once, beside the marks (`auto::marks`). In the rules by width
(`auto::sewn`), each filled part that is no speck is tested against the guides' lines, as a running
stitch is cut to its clips (`normalize::region::cut::inside`). Each pair of a segment of a line and a side
of the part costs a unit of work of the area's element. A design without guides tests
nothing. The guided fill then sews as it does for a shape set by hand.

### What guides sew

Measured on 2026-11-19 with the build on `main` at `7486520b`, built at opt-level 1, by the numbers of
M13.41's measure and M13.29's count, and the stitches of the PES file read back. No picture is a goal: one
procedure drew every guide, never a form, and phase 1's prototype measured the same guides. Each picture
was planned without guides. Its 3 largest filled parts were then each given a guide along their longest
axis, the line between the 2 points of the part's outline farthest apart, bent so that its middle lies a
fifth of its length to the left, through 25 points. A guide drawn so crosses other parts too, as a person's
would. The last column counts, among the elements of the areas that the guides miss, those that sew the
same stitches as without guides.

| Picture | Guides | Parts that follow | Likeness | Thickest spot, fills | At 3 fills or more, mm² | Bare, % | Slivers | Jump threads | Thread changes | Stitches | Export's count | Other areas' elements the same |
|---|---|---|---|---|---|---|---|---|---|---|---|---|
| Dog on turf, 6 threads | none |  | 0.856 | 4.00 | 131.25 | 0.021 | 0 | 60 | 8 | 10,068 | 10,068 |  |
|  | 3 | 16 in 16 areas | 0.856 | 4.00 | 144.00 | 0.030 | 0 | 55 | 9 | 10,374 | 10,374 | 35 of 117 |
| Dog on turf, 9 threads | none |  | 0.829 | 4.00 | 149.00 | 0.010 | 0 | 62 | 12 | 10,742 | 10,742 |  |
|  | 3 | 13 in 13 areas | 0.829 | 4.00 | 164.25 | 0.015 | 0 | 59 | 12 | 11,167 | 11,167 | 68 of 143 |
| Running dog, 6 threads | none |  | 0.898 | 4.00 | 129.00 | 0.000 | 0 | 49 | 8 | 11,077 | 11,077 |  |
|  | 3 | 15 in 15 areas | 0.898 | 4.00 | 139.50 | 0.005 | 0 | 46 | 11 | 11,422 | 11,422 | 62 of 117 |
| Running dog, 9 threads | none |  | 0.921 | 4.00 | 176.50 | 0.005 | 0 | 64 | 11 | 12,369 | 12,369 |  |
|  | 3 | 8 in 8 areas | 0.921 | 4.00 | 213.75 | 0.014 | 0 | 63 | 13 | 12,659 | 12,659 | 56 of 174 |
| Pear | none |  | 1.000 | 3.86 | 13.00 | 0.000 | 0 | 2 | 2 | 4,660 | 4,660 |  |
|  | 3 | 5 in 4 areas | 1.000 | 4.00 | 16.75 | 0.007 | 0 | 3 | 2 | 5,218 | 5,218 | 2 of 3 |
| Lettered patch | none |  | 1.000 | 3.98 | 40.25 | 0.003 | 0 | 8 | 2 | 4,026 | 4,026 |  |
|  | 3 | 1 in 1 area | 1.000 | 3.98 | 49.75 | 0.002 | 0 | 8 | 2 | 4,328 | 4,328 | 13 of 15 |
| TS-13A | none |  | 1.000 | 3.99 | 23.75 | 0.013 | 1 | 18 | 2 | 11,513 | 11,513 |  |
|  | 3 | 5 in 5 areas | 1.000 | 3.99 | 24.00 | 0.014 | 1 | 18 | 2 | 12,021 | 12,021 | 30 of 30 |
| TS-13B | none |  | 1.000 | 3.85 | 4.25 | 0.066 | 0 | 46 | 4 | 6,895 | 6,895 |  |
|  | 3 | 3 in 3 areas | 1.000 | 3.85 | 3.25 | 0.066 | 0 | 46 | 4 | 7,035 | 7,035 | 72 of 72 |
| TS-13C | none |  | 1.000 | 3.96 | 95.75 | 0.000 | 0 | 13 | 4 | 6,064 | 6,064 |  |
|  | 3 | 5 in 4 areas | 1.000 | 3.99 | 107.25 | 0.005 | 0 | 15 | 4 | 6,368 | 6,368 | 14 of 18 |
| TS-13D | none |  | 1.000 | 3.97 | 50.75 | 0.000 | 0 | 12 | 3 | 7,994 | 7,994 |  |
|  | 3 | 3 in 3 areas | 1.000 | 3.99 | 101.00 | 0.000 | 0 | 12 | 3 | 9,008 | 9,008 | 17 of 19 |
| TS-13E | none |  | 1.000 | 3.39 | 2.50 | 0.000 | 0 | 5 | 2 | 2,716 | 2,716 |  |
|  | 3 | 7 in 7 areas | 1.000 | 3.41 | 2.75 | 0.018 | 0 | 5 | 2 | 3,023 | 3,023 | 22 of 22 |
| Badge, abutting | none |  | 1.000 | 3.01 | 0.00 | 0.000 | 0 | 0 | 2 | 3,431 | 3,431 |  |
|  | 3 | 3 in 3 areas | 1.000 | 3.22 | 0.00 | 0.010 | 0 | 0 | 2 | 3,957 | 3,957 | 1 of 1 |
| Badge, stacked | none |  | 1.000 | 3.01 | 0.00 | 0.000 | 0 | 0 | 2 | 3,431 | 3,431 |  |
|  | 3 | 3 in 3 areas | 1.000 | 3.22 | 0.00 | 0.010 | 0 | 0 | 2 | 3,957 | 3,957 | 1 of 1 |
| Emblem, abutting | none |  | 1.000 | 3.57 | 1.25 | 0.000 | 0 | 2 | 3 | 4,997 | 4,997 |  |
|  | 3 | 3 in 3 areas | 1.000 | 3.62 | 3.00 | 0.000 | 0 | 2 | 3 | 6,188 | 6,188 | 4 of 4 |
| Emblem, stacked | none |  | 1.000 | 3.39 | 1.00 | 0.000 | 0 | 2 | 3 | 4,998 | 4,998 |  |
|  | 3 | 3 in 3 areas | 1.000 | 3.71 | 1.00 | 0.000 | 0 | 2 | 3 | 6,153 | 6,153 | 4 of 4 |
| Button | none |  | 1.000 | 3.96 | 2.75 | 0.000 | 0 | 0 | 2 | 1,847 | 1,847 |  |
|  | 3 | 4 in 4 areas | 1.000 | 4.00 | 2.25 | 0.009 | 0 | 0 | 2 | 2,014 | 2,014 | 10 of 10 |
| Marked patch | none |  | 1.000 | 3.95 | 14.25 | 0.000 | 0 | 1 | 3 | 4,077 | 4,077 |  |
|  | 3 | 3 in 3 areas | 1.000 | 3.97 | 15.25 | 0.005 | 0 | 1 | 3 | 4,503 | 4,503 | 2 of 6 |
| Magnolia | none |  | 0.849 | 4.00 (above the cap) | 521.25 | 0.006 | 0 | 313 | 10 | 38,197 | 38,197 |  |
|  | 3 | 26 in 25 areas | 0.848 | 4.00 | 603.00 | 0.005 | 0 | 302 | 10 | 39,946 | 39,946 | 289 of 560 |
| Coleus | none |  | 0.841 | 4.12 (above the cap) | 959.25 | 0.041 | 1 | 473 | 13 | 62,219 | 62,219 |  |
|  | 3 | 15 in 15 areas | 0.842 | 4.01 (above the cap) | 977.25 | 0.041 | 1 | 459 | 13 | 62,249 | 62,249 | 424 of 1020 |
| Goose | none |  | 0.820 | 4.00 | 489.25 | 0.016 | 0 | 170 | 10 | 38,352 | 38,352 |  |
|  | 3 | 10 in 9 areas | 0.820 | 4.00 | 549.25 | 0.014 | 0 | 169 | 10 | 39,509 | 39,509 | 270 of 384 |
| Castle | none |  | 0.779 | 4.00 | 255.75 | 0.010 | 0 | 104 | 9 | 28,526 | 28,526 |  |
|  | 3 | 7 in 6 areas | 0.779 | 4.00 | 280.00 | 0.009 | 0 | 102 | 9 | 29,395 | 29,395 | 122 of 210 |

- **The parts follow their guides.** Each guide steered up to 16 filled parts, the parts it was drawn along
  and those it crossed on the way. The rows bend with the guide in the preview pictures, and the export
  sews what the plan says, stitch for stitch.
- **The repository's drawn designs sew the rest as before.** TS-13A, TS-13B, TS-13E, the badges, the
  emblems and the button sew the same stitches in every element of the areas the guides miss. Elsewhere
  the order changes. Each element a guide makes is a shape more to order, so the rest of a traced photo or
  dog starts and ends elsewhere. Thread changes rise by up to 3, on the running dog in 6 threads.
- **The cap holds.** No guided picture sews above 4 fills where `main` doesn't, and the coleus, which sews
  4.12 fills on `main`, sews 4.01. The prototype sewed the goose at 4.02 and the castle at 4.28, where
  parts of one area that rounding left a hair apart each reached under the other once a guide made them 2
  elements. Such fills stay one area for the overlaps (`REQ-AUTO-110`). The area at 3 fills or more grows
  on most pictures, by up to 82 mm² of the magnolia's 521 and 50 mm² on TS-13D, since a guided fill sews
  more stitches than the tatami fill it replaces: its copies lie closer where the guide bends.
- **The fabric is covered as before.** The prototype left up to 0.23 % of a design bare, at the edges where the
  guided rows ended square on: automatic digitizing keeps a fill's turns as stitches (`turn_stitches`), and
  the guided fill didn't read it, so every second row started a needle point inside the edge. It reads it
  now ([guided fill](algorithms/guided.md#sewing-the-lines)). What is left is at most 0.018 % more of a
  design, on TS-13E, and 0.009 % on a dog, where a few rows meet the outline at a slant and end short of
  it, far under the limit of 0.15 %.
- **A guide over a detail leaves the area under it alone.** The badge's preview showed a guide drawn over
  its disc steering the field round it too, since the field is digitized whole under the disc. The guide's
  line is now tested where the area shows.
- **The likeness doesn't move,** by at most 0.001, since a guide doesn't change a part's colour or place.
- **What it costs to plan.** The guided fills cost more or less than tatami fills by their stitches, and the
  test against the guides is a small share of it. With the prototype, planned and measured with its 3
  guides, the pear took 1.2 % more instructions than without them, and the running dog in 6 threads 4.1 %
  less (callgrind, opt-level 1).

### Decisions for the owner

Each with the recommendation, which the step took on 2026-11-19. The roadmap's row no longer
says the area is grouped with the guide.

- **Which parts follow: those the guide crosses.** The roadmap's row planned an area grouped with the guide.
  A traced picture is hundreds of shapes in one layer, and a person can't easily group one of them, and in
  VectorCraft a guide in that layer is already grouped with all of them. The other choices are the parts
  wholly inside the guide's stroke, read as a mark's band, which leaves out the large parts whose rows show
  most, and the parts of the shapes grouped with it. Recommended: the parts it crosses, drawn over the
  picture as a mark is.
- **One name, `stitch:guide`.** VectorCraft documents already name a guide so. The other choice is a name
  of its own, such as `stitch:rows`, which would keep the 2 readings apart but gives a person 2 names for
  one idea. Recommended: one name, with both readings.
- **The guide drawn last steers** a part that 2 cross. The other choices, the guide nearest the part's
  middle or the one with the longest stretch inside it, are guesses a person can't see. Recommended: the
  last drawn.
- **Columns and running stitches keep their stitches.** The other choice sews a crossed part no wider than
  a satin column as a guided fill too. A column's stitches already follow its form. Recommended: keep them.
- **No setting, the guided fill's defaults.** The guide is enough, as the row says. A person who wants
  other rows gives the shape its own effect, as for any shape. Recommended: no setting.
- **The rest of a photo may sew in another order,** as for marks. Each element a guide makes is a shape
  more to order. Recommended: accept it, and let M13.35's cap bound the thickness.

### What isn't built

- A guide that steers satin columns. Their stitches already lie across the part's width.
- A guide with an area, which would steer only the parts inside it, or a guide's stroke read as a band.
  The prototype's guides did their work without them.
- Turning the base layer to follow a guided fill.

## Names of shapes

A traced photo has hundreds of shapes, and every manual control starts with the same question: which shape
is this? A mark ([marks that keep parts](#marks-that-keep-parts)), a guide (M13.24) and a shape's own
effect all need the person to find the shape in VectorCraft. Today a notice names a shape by its element
id, `vc:63:fill`, which is the number VectorCraft keeps in the file. The Layers panel never shows that
number. Roadmap step M13.25 gives each shape of a traced design a name that the Layers panel, the notices
and the report all show (`REQ-AUTO-125` to `REQ-AUTO-127`, `REQ-VC-033`, `REQ-CLI-023`), and says when a
path's own Auto effect replaces its group's (`REQ-VC-034`, `SC-W0920`). The engine's `names` module
builds the names.

### What VectorCraft shows

Read in VectorCraft's source at `3f8d7a0`, its `.vectorcraft` format and the Layers panel's code. The app's
window wasn't run: VectorCraft isn't built here.

- **A row shows the object's name.** A node of a `.vectorcraft` document has an optional `name`. The Layers
  panel shows `Node::display_name()` (`crates/doc`): the name when there is one, else the kind in angle
  brackets, `<Compound Path>`, `<Path>` or `<Group>`. In a traced document no node has a name. It is a
  layer with one group in it, and in the group a compound path or a path for each shape. So a traced photo shows hundreds of rows
  that read `<Compound Path>`.
- **Search All finds by name.** The panel's search keeps the rows whose shown name, in lower case, contains
  what the person typed (`list_rows` in `crates/ui-egui/src/panels/layers.rs`). A name written into the file
  is what a person can search for. A query matches inside longer names, so `red fill 1` also finds
  `red fill 12`.
- **A long name is cut at the row's end.** The panel paints the name clipped to the row, which loses its
  right end on a narrow panel. VectorCraft keeps at most 255 characters of a name.
- **A name stays a name.** StitchCraft's reader gives an element its node's name already (`Element::name`),
  and a name that begins with `stitch:` is a marker. A name StitchCraft writes must never begin with it.

Seen on 2026-11-20 in VectorCraft's own window, by the pinned release on CI (the shot
`vectorcraft-layers-names`): at the panel's default width, a row of a shape in the traced group shows about
20 characters. `Dark Olive column (1)`, of 21, loses its closing bracket, and `Russet Brown column (1)`, of
23, shows only its first 18, with no `…`. The accessibility tree doesn't give the rows' text. The probe says so
and keeps the picture. The longest names on the repository's traced documents, the pear and the 4
dogs, have 23 characters (`Russet Brown column (1)`, `Dark Brown left out (1)`). Not checked: whether a
person can read the names at the panel's smallest row size.

### The parts of a name

A name tells a person the thread, how the shape sews and which one of them it is: `Red fill (3)`, `Deep Gold
column (1)`, `Black line (12)`.

- **The thread** is the thread's name. A traced shape's colour is one of the machine's threads exactly,
  since the trace uses the library's colours as they are, so its name is the palette's name of that colour.
  A colour that no thread of the palette has is written as its hex code, `#7d6f00`.
- **How it sews** is the way the shape's main part sews: the part that keeps the id of its lowest element,
  which is an area's fill, or its first narrow part when nothing of it is filled, or a stroke's first
  column. A shape none of whose parts keeps that id goes by its first part in the sewing order. A fill of
  any method is `fill`, a satin column is `column`, and a running stitch or another stroke is `line`. A
  shape that sews nothing is `left out`, as the background the design leaves out is (`SC-W0906`), and so
  is one merged whole into its neighbours.
- **The number** counts the shapes of that thread and that way of sewing in the Layers panel's order, from
  its top row, 1 first. The top row is the object drawn last. The number is written in brackets, so that a
  search for `fill (1)` doesn't also find `fill (12)`.
- **A shape** is what the elements of one object make. Their ids differ only in their last part
  (`vc:6:fill` and `vc:6:stroke` are of `vc:6`), and the parts digitizing makes of them add `#` or `~`
  and more to an element's id (`vc:6:fill#2`). A part has its shape's name.

The longest name on the pictures measured was 18 characters with a hex code in place of the thread's name,
and the longest thread name of the machine's palette has 15 (`Wisteria Violet`). A name then fits in 27.

**Where it sews.** A name doesn't say where the shape is in the sewing order. The order is chosen again at
every plan, and one small edit moves many shapes in it, as the measures below show. So the report lists
the shapes in their sewing order at each plan, each with its place, which the name doesn't give.

### How stable names are

Measured on 2026-11-19 with a probe test (not committed), on the repository's traced badge, pear and 2
dogs and on 2 of the 4 local photos, a magnolia and a coleus. Each design was read with
`--auto`, digitized, and for the first 4 also planned on the reference machine. Then each of 9 shapes,
spread through it, was deleted, or painted in another of the design's colours, and the names were made again.
A count is of the other shapes whose names changed.

Planned twice unchanged, every design gave the same names: digitizing is deterministic.

| Number counted | Badge (3) | Pear (5) | Dog on turf (78) | Dog running (92) |
| --- | --- | --- | --- | --- |
| In the whole sewing order | 0 to 2 | 0 to 3 | 30 to 60 | 18 to 70 |
| By thread in the sewing order | 0 to 2 | 0 to 2 | 7 to 54 | 1 to 32 |
| By thread and kind in the sewing order | 0 | 0 | 3 to 25 | 1 to 29 |
| By thread and kind in the Layers panel's order | 0 | 0 | 5 on average | 4 on average |

The first 3 rows are for the deletion of 4 shapes. Deleting one shape reorders the sewing of shapes far from
it, and a name with its place in the sewing order changes for up to 3 in 4 of the others. A number in the
Layers panel's order changes only for the shapes of the same thread and kind below the one deleted.

On the local photos, numbered by thread and kind in the Layers panel's order, a deletion renamed 23 of the
magnolia's 424 other shapes on average. With the number counted by thread alone, without the kind, it renamed
45. The magnolia and the coleus, of 704 shapes, gave the same names when digitized twice.

The names as built were measured on 2026-11-19 with a probe test (not committed) that planned each design
on the reference machine, at opt-level 2, and then planned it again with each of 4 shapes deleted, spread
through it. Here every shape counts, those merged whole into their neighbours too, which are `left out`.

| Design | Shapes | Longest name | Others renamed by a deletion | Plan |
| --- | --- | --- | --- | --- |
| Badge | 4 | 18 | 0 | under 0.1 s |
| Pear | 6 | 23 | 0 | 0.2 s |
| Dog on turf, 6 threads | 97 | 22 | 0 to 10 | 2 s |
| Dog running, 9 threads | 107 | 22 | 0 to 2 | 2 to 3 s |
| Magnolia | 2,014 | 26 | 84 to 430 | 43 to 69 s |
| Coleus | 3,985 | 25 | 176 to 631 | 84 to 104 s |
| Goose | 1,813 | 27 | 23 to 362 | 104 to 136 s |
| Building | 1,444 | 26 | 10 to 289 | 100 to 111 s |

Every design gave the same names when planned twice, and no thread was named by its hex code: the traced
colours are the machine's threads. On a photo a deletion renames up to a fifth of the other shapes. A
deleted shape changes which specks and thin lines merge into their neighbours, so shapes move between `fill`
and `left out`, and the numbers of their thread below them move with them. Naming is one pass over the
elements and the steps, and isn't timed apart from planning. The trace's own time wasn't measured, since
VectorCraft isn't built here.

So the names are written once, when the picture is traced, and are then the shapes' own names, as a name a
person gives one is. StitchCraft never renames a shape that has a name. A shape the person draws later has
none, and it is named, for that plan only, by the same rule with the next free number.

### Places that show the names

One function of the engine makes a name (`names::name`), and one of `stitchcraft-core` gives the words that
name a shape in any message (`diag::called`): its name in quotes, then its element id for a bug report,
`"Red fill (3)", vc:63:fill`. Planning gives each diagnostic about a shape with a name that name
(`Diagnostic::name`), and its headline shows it. Every place below uses them.

- **The traced document.** `stitch plan picture.png --traced FILE` writes each shape's name into its node,
  and changes nothing else in the document (`REQ-VC-033`). The design that is planned is that document, with its names,
  so planning `FILE` again sews the same stitches and says the same notices.
- **The notices.** A notice about a shape names it, and so does a warning of the reader. A line of `stitch
  plan` has the name before the id, `info SC-I0912 ("Red fill (3)", vc:63:fill): …` (`REQ-AUTO-127`). The
  shapes at `SC-W0913`'s thickest spot are named the same way, each in brackets: `("Red fill (3)",
  vc:63:fill) #ed171f 2.1`. `SC-I0916` and `SC-I0918` are about a mark and a guide, which they name by
  its id as before, and they count the parts they keep or steer.
- **The report.** Each diagnostic with an element has its `name` too, and a list `shapes` gives each shape's
  name, id, thread, kind and place in the sewing order (`REQ-CLI-023`).
- **The bug-report bundle** of a picture keeps the traced document with its names. A replay then names the
  shapes as the run did.

### A path's own Auto effect inside an automatic group

A group's Auto effect makes everything inside it automatic with its settings. A path inside with an Auto
effect of its own sews by its own settings alone (`REQ-VC-016`), since the nearest effects say how a path
sews. VectorCraft writes an effect with every parameter, its defaults included (`merged_params` in its
`crates/effects`, read at `3f8d7a0`), so the path's own effect stores every setting, and the reader can't
tell a setting the person chose from one left at its default. The group's settings can't be merged under
the path's.

The roadmap row said this happens without a word. A case showed that `SC-W0807` said it, in the
words for StitchCraft effects that reach a path from 2 levels: "it is sewn as the nearest say". A person
learns from them neither which of the group's settings the path loses nor which path it is, since an unnamed
path is only "the object" with its id. The warning `SC-W0920` is said in place of `SC-W0807` where an
Auto effect lies inside an automatic group, with the path's name and both values of every group setting
that the path's own effect sets otherwise, leaving out those the group keeps at their defaults
(`REQ-VC-034`). The stitches stay as they are today.

### Decisions taken

The owner took the 6 recommendations of phase 1 on 2026-11-19:

- Names are written once, at the trace, and kept as the shapes' own names. Names made again at every plan
  would follow the sewing order and change for many shapes after each edit.
- The number counts in the Layers panel's order, and the report gives the place in the sewing order beside
  the name.
- The number stands in brackets, so that a search finds one shape.
- The shapes left out are named too, as `left out`, since notices name the background.
- The layers keep their order. Automatic digitizing cuts an area away under the areas over it in the
  layers, and the layers' order is part of the design.
- A new code, `SC-W0920`, says that an Auto effect inside an automatic group replaces the group's settings,
  so that `stitch explain` can say what to do.

### What isn't built

- `SC-I0916` and `SC-I0918` don't name the shapes whose parts a mark keeps or a guide steers. The marks
  and the guides count the parts in digitizing, before the shapes have names.
- The VectorCraft plug-in's previews don't show names. A document opened in VectorCraft has the names
  `--traced` wrote, and a path the person draws later has a name for each plan only.
- VectorCraft's own Layers panel isn't changed.

## Expand

Expand (M13.26) is the other way out of automatic digitizing. Marks, guides and names steer a design while
it stays automatic. When no control gives what a person wants, Expand makes the design ordinary elements
with ordinary effects, which the person edits one by one, as the automatic digitizing of other embroidery
software leaves a design. The expanded design sews as the automatic one sewed, and it no longer changes
when the picture, its marks or the Auto effect's settings change. Phase 1 recorded this decision on
2026-11-20, with a prototype measured on 13 designs of the set of pictures, and the owner took each of its
recommendations. The build followed the same day: `stitch expand` and `auto::expand`.

### The decision

Planning already turns every automatic element into ordinary elements before anything is generated
([automatic elements](#automatic-elements)). Expand writes those elements out, each with the settings that
say what it sews, and every later plan reads them as shapes set by hand. A second digitizer, or a
translation of the rules into other settings, isn't needed: the elements are the ones the automatic plan
sews.

Planning does more after digitizing, by which elements digitizing made, and no setting of an element says
so today. Between 2 elements that digitizing made, the needle runs under later shapes in place of a jump
(`REQ-AUTO-053`). Every stitch of the base layer is underlay (`REQ-AUTO-028`). The cap at the end of
planning thins the elements digitizing made and the lettering it placed, with the limits of the lowest
automatic shape (`REQ-AUTO-074`).

Expand keeps them with one new setting common to every stitch effect, `digitized_as`: `no`, the default,
for a shape set by hand, `part` for an element that sews as digitizing made it, `base` for a piece of
the base layer, and `lettering` for lettering that digitizing placed, which keeps its underlay and its
pull in the cap's reach (`REQ-AUTO-120`, `REQ-AUTO-121`). Planning reads it from any element, so a person can also set it by hand, or turn it off
on a part. The cap's limits become design settings, kept with the document as its machine is
([design settings](vectorcraft-integration.md#design-settings-and-machine-kept-with-the-document)). With
them the expanded design plans to the automatic plan exactly (`REQ-AUTO-142`, `REQ-AUTO-143`).

### The parts and their settings

Each part is an object, in the order the parts sew, and each object's effect is its family's
([parameter registry](params.md)). Ink/Stitch's names are used where its parameters are the
interoperability contract.

| What digitizing makes | The ordinary element | Its settings |
|---|---|---|
| An area's fill, with its angle, underlay and overlap | the area as digitizing sews it, which reaches under the shapes sewn after it and holds the specks merged into it | the fill effect, tatami, with `angle`, `row_spacing_mm`, `max_stitch_length_mm`, `staggers`, `underpath`, `fill_underlay` and `turn_stitches` |
| A satin column | a line along the column's centre, as wide as the column (`REQ-SAT-016`) | the satin effect with `satin_column`, the underlay of its width, `pull_compensation_mm`, `push_compensation_mm`, the short stitches and the nearest start and end |
| A line, a narrow part and an outline | a line along the path the stitches follow | the stroke effect, with `bean_stitch_repeats` for a triple run and `running_stitch_tolerance_mm` |
| A guided fill (`REQ-AUTO-109`) | the fill, and its guide line grouped with it | the fill effect, guided |
| Lettering it placed | the letters' columns and lines, as they sew | as a satin column and a line |
| A part left out, the background or a speck | nothing | none |
| Lettering it placed for a marker | its columns | `digitized_as` = `lettering` |
| A piece of the base layer | a fill under the details | the fill effect, with `digitized_as` = `base` |
| The run from one part to the next | nothing: planning finds it | `digitized_as` = `part` on both |
| The cap's thinning | nothing: planning thins the plan | `digitized_as` = `part`, and the design's limits |
| The sewing order | the objects' order | none |
| The lock stitches of a small part | | `lock_start`, `lock_end` and their scales |

The prototype counted the settings the parts carry on the 13 designs below. Every one is in the
parameters registry. Ink/Stitch has all but `turn_stitches`. The fill effect's dialog leaves
`turn_stitches` out, and VectorCraft admits only the parameters a manifest declares. The dialog needs
the row. 2 elements also carried `auto_fill_outline`, a setting of the Auto effect. Expand drops every
`auto_` key.

Each part takes its shape's name ([names of shapes](#names-of-shapes)), numbered as the notices number
the parts. What Expand can't keep is said by a coded warning, and nothing is dropped silently:

- Lettering becomes columns, and its text can't be edited as text. Lettering set in a font no longer
  marks its underlay as lettering does (`SC-W0922`).
- A shape that needs more work than the budget allows is left out, as planning leaves it out
  (`SC-E0004`), and `SC-W0922` names it.
- A part cut to a clip is written whole into a VectorCraft document, which `SC-W0922` names. No part of
  the set has one: digitizing cuts its shapes to their clips first.
- An SVG file sewn by Ink/Stitch loses `digitized_as` and the design's limits, so Ink/Stitch sews it with
  jumps and without the cap.

A guided fill is written with its guide, in a group with it, as are a part's start, end and target markers.

### Expand on the command line and in VectorCraft

**The command line.** `stitch expand picture.vectorcraft -o expanded.vectorcraft` writes the documented
`.vectorcraft` format, and `-o expanded.svg` an SVG file with Ink/Stitch's attributes
(`stitchcraft_svg::written`). It reads the design as `stitch plan --auto` does. Each object of a layer that
holds an automatic shape stays in the document, hidden and locked, below a new group of the expanded
parts, "Expanded embroidery", as Image Trace keeps the source image beside its paths
(`stitchcraft_vectorcraft::expanded`). The document stays the one VectorCraft saved but for those objects'
`visible` and `locked`, the new group, the document's `next_id` and the Design Settings filter's data, which
keeps the cap's limits. A picture given to `stitch expand` is traced first, as `stitch plan` traces it.

**VectorCraft, today.** In one undo step, a filter may replace the selected paths with new objects that
have StitchCraft's effect records, as Stroke to Satin does
([tools](vectorcraft-integration.md#tools-stroke-to-satin-and-test-sheet)). An Expand filter can't work
today:

- A filter reads a path's own effects alone ([routing filters](vectorcraft-integration.md#tools-the-routing-filters)).
  The Auto effect is usually on the traced group, so the filter can't read the rules' settings.
- Digitizing a design takes an export's work. TS-13A to TS-13C took 7.9 to 8.0 billion instructions to
  export, where a heavy filter gets 1 billion, and a traced photo needs 51 to 69 billion
  ([P10](rfc-vectorcraft-abi-v2.md#p10-an-export-budget-that-fits-a-traced-photo)).

So in VectorCraft a person runs `stitch expand` and opens the document it writes. The smallest change to
ask of VectorCraft, through the [plug-in features RFC](rfc-vectorcraft-abi-v2.md), is
[P11](rfc-vectorcraft-abi-v2.md#p11-the-effects-of-a-paths-groups-in-a-filter). With it a filter reads the
effects of the groups round each path. A heavy filter needs P10's budget as well. A filter
can't make a group either, which a guided fill's guide needs. That is a later ask, and until then an
Expand filter leaves the guide out and says so.

### How faithful it is

**Measured** on 2026-11-20 with the prototype, `main` at `ebbdd62a`, built at opt-level 2. Each design is
planned automatic, then expanded with ordinary effects alone, without `digitized_as` and the cap's
limits, and planned again with the same origin:

| Design | Stitches | Jumps | Jump threads | Thickest, fills | At 3 fills or more, mm² | Needle points as with the cap off |
|---|---:|---:|---:|---:|---:|---|
| Dog on turf, 6 threads | 9,530 → 10,219 | 69 → 78 | 60 → 69 | 3.87 → 5.09 | 54.25 → 138.00 | no |
| Dog on turf, 9 threads | 10,100 → 10,887 | 75 → 82 | 62 → 69 | 3.97 → 5.13 | 63.75 → 151.50 | no |
| Dog running, 6 threads | 10,630 → 11,169 | 58 → 69 | 49 → 60 | 3.92 → 4.96 | 53.50 → 128.75 | no |
| Dog running, 9 threads | 11,779 → 12,674 | 76 → 92 | 64 → 80 | 3.98 → 5.85 | 94.00 → 190.50 | no |
| Pear | 4,628 → 4,674 | 5 → 6 | 2 → 3 | 3.99 → 4.35 | 10.50 → 13.50 | no |
| Lettered patch | 4,000 → 4,033 | 11 → 12 | 8 → 9 | 3.98 → 4.00 | 38.75 → 40.50 | no |
| Badges | 4,619 → 4,655 | 7 | 4 | 4.00 → 4.56 | 18.25 → 24.75 | yes |
| Emblem | 4,992 → 4,997 | 6 | 2 | 3.25 → 3.57 | 0.00 → 1.25 | yes |
| TS-13A, digitized half | 11,484 → 11,522 | 21 | 18 | 3.99 → 4.29 | 19.50 → 26.50 | yes |
| TS-13B, digitized half | 6,868 → 6,913 | 51 → 54 | 46 → 49 | 3.85 | 3.25 → 4.25 | no |
| TS-13C, digitized half | 5,949 → 6,081 | 18 → 20 | 13 → 15 | 3.92 → 4.54 | 83.75 → 104.25 | no |
| TS-13D, digitized half | 7,928 → 8,012 | 16 | 12 | 3.97 → 4.73 | 41.50 → 53.25 | yes |
| TS-13E, digitized half | 2,712 → 2,716 | 8 | 5 | 3.24 → 3.39 | 2.50 | yes |

On every design the thread changes, the order of the elements, the stitches astray and the likeness are
the same. Bare fabric is the same or less. With the cap off in the automatic plan, 5 designs have every
needle point the same, in the same order, and differ only in the base layer's stitches, which are
underlay in the automatic plan and top stitches once expanded (187 to 1,085 a design). The other 8 jump
where the automatic plan runs under later shapes, 1 to 16 jumps more, with 2 to 128 needle points
different round them. So the cap and the runs account for every difference measured, and the base layer
for every difference of the stitches' parts.

**The tolerance.** With `digitized_as` and the design's limits, the expanded design's plan is the automatic
plan. It has the same threads. Each stitch lies at the same point, with the same kind and part, in the
same order. So the machine file is the same byte for byte. The planning functions are deterministic and get the same
elements and the same 3 sets. The requirement holds the 2 plans to the bit. A document written out reads its points
back to within a unit in the last place of a point (`stitchcraft_vectorcraft::filter`). A fill's rows
could then fall on the other side of a rounding. The case of a written document checks every
needle point to within 0.001 mm, with the same count, kinds, threads and order. A person's edit breaks it.
So does a format that can't store a part's settings, and a guide that a filter can't group with its fill.

Stitched previews of a traced dog, the lettered patch and TS-13C, automatic beside expanded with
ordinary effects alone, show the extra jump threads, such as one along TS-13C's balloon string.

### Other embroidery software

- Wilcom EmbroideryStudio's Auto-Digitize makes objects of chosen kinds, Tatami Fill, Turning Satin,
  Centerline Run and Outline Run, with the current properties, and a person then edits their properties,
  stitch angles, entry and exit points and sequence
  ([Auto-digitize individual shapes](https://docs.wilcom.com/embroiderystudio/28/en/OnlineHelp/Automatic/automatic/automatic-11.htm)).
  Its Smart Design sets the sequence by closest join, and the person reorders colours and turns underlay
  on or off ([Auto-digitize entire designs](https://docs.wilcom.com/embroiderystudio/27/en/OnlineHelp/Automatic/automatic/automatic-10.htm)).
  The pages describe one run from the artwork to the objects, and describe no link that updates them.
- Hatch Embroidery's Auto-Digitize leaves objects that a person reshapes, gives another fill type in Object
  Properties and orders in the Sequence tab ([a user guide](https://makerspaces.northeastern.edu/create/library/embroidery-2-diy-training-using-hatch-auto-digitize/)).
- Brother PE-Design's Auto Punch sets, before it converts, which areas sew and which become lines, and is
  run again with Retry after a change
  ([PE-Design 8 manual, p. 128](https://www.manualsdir.com/manuals/377010/brother-pe-design-8.html?page=128)).
- Ink/Stitch has no automatic digitizing of a picture. Its Jump to Stroke tool makes the jumps between
  elements running stitches placed under the following stitches
  ([stroke tools](https://inkstitch.org/docs/stroke-tools/)), which is how `digitized_as` = `part` sews.

A person coming from them finds the same objects with properties, in an order that is the sewing order,
which they edit freely. What differs: the expanded fills already reach under their neighbours and have a
base layer under them, and planning keeps the runs under later shapes and the cap while the parts keep
`digitized_as`.

### Decisions for the owner

Each with the recommendation of phase 1:

1. **One setting, `digitized_as`, for what ordinary effects can't say.** Recommended. The other way is
   to freeze each run as a stroke, as Ink/Stitch's Jump to Stroke does. The cap would then be lost
   with a warning. On the dogs that raises the area at 3 fills or more 2 to 2.6 times, and a run frozen in place
   no longer lies under the shape a person moves.
2. **The traced group stays, hidden and locked, under the expanded parts.** Recommended, so the person can
   compare and go back. The other way deletes it.
3. **Lettering placed for a marker becomes its columns.** Recommended, since they are what sewed. The other
   way writes a text object with the lettering effect. A person can then edit its text. The lettering's
   own rules sew it, and their stitches can differ from the automatic plan's.
4. **The command line first, VectorCraft through P11.** Recommended. A `.vectorcraft` exporter would
   write a second document, and a filter can't run until P11 and P10 are in VectorCraft.
5. **The cap's limits as design settings.** Recommended. The other way keeps them on each part, where
   the parts could disagree.
6. **The name.** VectorCraft's Object › Expand already makes an Image Trace object plain paths.
   Recommended: `stitch expand` on the command line, and Expand Embroidery in VectorCraft's menus, so
   that the 2 commands aren't mistaken for each other.

### What isn't built

Everything: `stitch expand`, `digitized_as`, the design settings of the cap, the writer of the expanded
document and the SVG file, and the warning, whose number the build phase takes. Its requirements are
`REQ-AUTO-142` to `REQ-AUTO-144`, `REQ-CLI-025` and `REQ-VC-035`, each with a case kept as planned.

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
   them, by the Abutting method, so that the shapes share their edges. With `--fewer-points` it fits their
   outlines with fewer points ([the tracer's options](#the-tracers-options)).
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

## Outlines thread can't show

Roadmap step M13.51 measured the outlines of traced pictures and gives `stitch plan` an option,
`--fewer-points`, off by default, that asks Image Trace for fitting options of its own (`REQ-CLI-024`). Step M13.52 builds the smoothing pass that this
section describes, as a setting off by default (`REQ-AUTO-131` to `REQ-AUTO-134`, planned), and adds the
notice `SC-I0921`.

The owner, on 2026-11-19: "often time there are too many points and you need to simplify the number of
points in the shape". She simplifies traced shapes by hand in a drawing program before she sews them. The
trade treats it as a fault: clusters of tiny points sew dense spots and ragged edges. Wilcom's
[Smooth Curves](https://docs.wilcom.com/embroiderystudio/e4/en/MainHelp/Automatic/vectors/Smoothing_curves.htm)
and Bernina's [Smooth curves](https://softwarehelp.bernina.com/en/MainHelp/PART_Modifying/edit_objects/Smooth_curves.htm)
each take one precision value. A higher value leaves fewer points, which follow the outline less closely.

### The points and wobble of traced outlines

Every traced picture of the repository was measured, with a traced lettered patch and 4 traced photos kept
outside it. The test sheets' traced halves are drawn exactly in code and have no wobble. A thread is
0.4 mm wide.

| Picture | Shapes | Anchors | Anchors per mm | Most on one subpath | Points needed within 0.1 mm | Jags per 10 mm | Teeth per 10 mm |
|---|---|---|---|---|---|---|---|
| The pear | 6 | 592 | 0.51 | 267 | 13 % | 0.57 | 2.28 |
| Dog on turf, 6 threads | 97 | 1,902 | 0.81 | 146 | 12 % | 0.75 | 1.74 |
| Dog on turf, 9 threads | 115 | 2,096 | 0.84 | 161 | 11 % | 0.67 | 1.82 |
| Dog running, 6 threads | 83 | 1,827 | 0.75 | 115 | 11 % | 0.46 | 1.45 |
| Dog running, 9 threads | 107 | 2,090 | 0.74 | 91 | 11 % | 0.45 | 1.46 |
| Lettered patch, traced | 37 | 3,230 | 0.75 | 176 | 7 % | 1.55 | 0.53 |
| Photo of a flower | 2,014 | 51,613 | 3.07 | 1,905 | 9 % | 8.08 | 2.81 |
| Photo of a leafy plant | 3,985 | 118,672 | 3.74 | 1,132 | 11 % | 14.84 | 3.07 |
| Photo of a bird | 1,813 | 68,099 | 3.49 | 3,629 | 10 % | 11.14 | 3.15 |
| Photo of a building | 1,444 | 63,361 | 4.14 | 3,932 | 12 % | 16.58 | 2.18 |

- **The shapes share their edges exactly.** Image Trace's Abutting method gives 2 neighbours the same
  anchors along the edge they share. Of the pear's 294 distinct anchors, 282 belong to 2 shapes and 8 to 3,
  where 3 colours meet. The edges coincide to less than a micrometre. 69 % of the pear's outline length
  and 89 to 99 % of the others' lies along another shape.
- **Few points draw what thread shows.** The column of points needed counts the points of a polyline
  that stays within 0.1 mm of the outline flattened ([Douglas and Peucker, 1973](https://doi.org/10.3138/FM57-6770-U75U-7727)),
  as a share of the flattened outline's points.
- **A jag** is a turn of 20° or more that turns back within 0.4 mm along the outline, measured on the
  outline resampled every 0.05 mm, the direction taken over 0.1 mm. Thread is wider than a jag.
  **A tooth** turns back within 0.4 to 1.6 mm, and thread shows it.
- **The pear's ragged colour edges are teeth.** Its photo's pixel is 0.28 mm. Where one band of colour
  meets the next in a gradient, the trace follows a staircase of 3 to 5 pixels, and its teeth stand 0.5 to
  1.5 mm tall.

**What the wobble costs.** The planner's rules read an outline's points. The traced dogs, given as the
same outlines flattened to polylines within 0.005 mm, sew 16 to 22 of their 69 to 92 shapes by another
rule, most of them `refused` for columns where the curves gave columns. Today the 4 dogs sew 12,659
stitches under 0.5 mm, 29 % of their stitches, and lay 12 stitches astray. A move of the whole picture by
0.05 mm in x and y, the shift test of M13.47, changes the rule of no shape on any of these pictures.

**Traced letters flip for another reason.** M13.47 found traced letters sewn as fills where the same
letters drawn exactly sew as columns. Its 558 letters, 3 typefaces at 6, 12 and 25 mm, each traced from a
render of 40 pixels per mm, were measured. 32 letters sew by another rule than their exact outlines. The
H of the sans-serif bold at 12 mm is refused for columns because its stems' inner edges lean 0.025 mm over
4.5 mm. That edge is a straight line, and smoothing leaves a straight line where it is. Smoothed at 0.1 and 0.2 mm, 55 and 46 traced letters
sew by another rule than their exact outlines smoothed the same way, and smoothing alone changes the rule
of about 50 exact letters. A block H, T and L drawn with one jog of 0.025, 0.05 or 0.1 mm at an inner
corner sew as columns, as they do without the jog. A leaning stem belongs to the columns' design, and
smoothing doesn't fix it.

### The tracer's options

`stitch plan picture.png` asks Image Trace for its mode, palette, colours and method, and leaves its
fitting options at their defaults, which are Paths 50, Corners 75 and Noise 25, with Snap Curves To Lines
off. The tracer fits an edge once for both shapes that share it. Its options smooth an edge without
opening a gap. The pear and the 4 dogs were traced again from their sources by VectorCraft 0.24.1's command line, once for each value,
and planned by `main` at `00b45915`. The counts are the 5 pictures' together:

| Options | Anchors | Jags per 10 mm | Teeth per 10 mm | Colour error | Stitches under 0.5 mm | Astray | Jump threads | Detail lost | Shapes |
|---|---|---|---|---|---|---|---|---|---|
| Defaults (50, 75, 25, off) | 8,507 | 0.58 | 1.75 | 30.1 | 13,430 | 12 | 237 | 9.9 % | 408 |
| Paths 0 | 7,070 | 0.58 | 1.76 | 30.1 | 13,106 | 17 | 221 | 9.9 % | 408 |
| Paths 25 | 7,505 | 0.58 | 1.75 | 29.9 | 13,380 | 17 | 222 | 9.9 % | 408 |
| Paths 100 | 16,469 | 1.08 | 1.70 | 29.5 | 13,825 | 2 | 233 | 10.4 % | 408 |
| Corners 0 | 8,037 | 0.39 | 1.41 | 30.2 | 13,594 | 11 | 215 | 10.0 % | 408 |
| Corners 25 | 8,011 | 0.38 | 1.43 | 30.2 | 13,319 | 7 | 217 | 10.1 % | 408 |
| Corners 100 | 9,678 | 1.09 | 2.06 | 29.9 | 13,722 | 7 | 237 | 9.6 % | 408 |
| Noise 10 | 10,832 | 0.74 | 1.88 | 28.5 | 14,002 | 44 | 285 | 12.6 % | 702 |
| Noise 50 | 7,166 | 0.52 | 1.68 | 31.4 | 12,817 | 11 | 196 | 2.2 % | 276 |
| Noise 200 | 4,841 | 0.40 | 1.50 | 36.7 | 10,081 | 2 | 73 | 0.0 % | 115 |
| Snap Curves To Lines | 8,466 | 0.58 | 1.72 | 30.2 | 13,188 | 10 | 232 | 9.9 % | 408 |
| Paths 25, Corners 25 | 6,307 | 0.29 | 1.24 | 30.0 | 13,112 | 8 | 220 | 10.2 % | 408 |
| Paths 0, Corners 0 | 5,336 | 0.23 | 1.06 | 30.2 | 13,190 | 13 | 218 | 9.8 % | 408 |

The colour error is the mean distance in RGB between the trace and its source picture, over the picture's
pixels. Detail lost is the likeness measure's, against the trace itself. A trace that drops shapes
doesn't lose detail by that measure, and its colour error shows the loss.

- **Paths** sets how closely a curve follows the pixels: within 0.08 pixels at 100, 0.33 at 50 and 0.78
  at 0. At 0 it saves 17 % of the anchors and changes nothing else.
- **Corners** sets the turn that makes a corner: 90° at 0, 49° at 75 and 35° at 100. Each step of a
  staircase turns about 90°. The default keeps a staircase as corners. At 0 or 25 the jags fall by a
  third.
- **Noise** is the size in pixels below which an area is dropped. Raised, it drops the dogs' small shapes and
  their detail with them: the colour error rises by 1.4 at 50 and 6.6 at 200. Lowered to 10, it adds 294
  shapes and 32 stitches astray.
- **Snap Curves To Lines** moves each measure by less than 2 neighbouring values of Paths or Corners do.

**Built (M13.51).** `stitch plan --fewer-points` sends Paths 0 and Corners 0 for a picture it traces, in
every style. Noise stays 25. Without the option, the default, `stitch plan` doesn't send a fitting option,
as before M13.51. Corners 0 rounds each turn under 90° of a drawn logo, such as an octagon's, so a person
leaves the option out for a logo.

Each picture was traced from its source by VectorCraft 0.24.1's command line with the defaults and with
the 2 options, and planned by this step's code. The pictures are the pear, the 4 dogs, the badge of the
compatibility gate drawn flat and a stitched preview of the lettered patch, traced as a photo of it.
Each cell gives the trace with the defaults, then with the options. Area at 3 fills is the area under 3
fills of thread or more:

| Measure | Pear | Dog on turf, 6 | Dog on turf, 9 | Dog running, 6 | Dog running, 9 | Badge | Lettered patch |
|---|---|---|---|---|---|---|---|
| shapes traced | 6 to 6 | 97 to 97 | 115 to 115 | 83 to 83 | 107 to 107 | 12 to 12 | 749 to 749 |
| anchors | 592 to 346 | 1902 to 1195 | 2096 to 1361 | 1827 to 1126 | 2090 to 1308 | 754 to 326 | 13642 to 9800 |
| jags/10mm | 0.57 to 0.13 | 0.75 to 0.24 | 0.67 to 0.35 | 0.46 to 0.2 | 0.45 to 0.22 | 0.21 to 0.1 | 0.4 to 0.19 |
| teeth/10mm | 2.28 to 1.66 | 1.74 to 1.04 | 1.82 to 1.11 | 1.45 to 0.72 | 1.46 to 0.75 | 0.52 to 0.07 | 0.43 to 0.36 |
| colour error | 23.5 to 23.6 | 29.9 to 30.5 | 29.1 to 29.4 | 36.0 to 35.5 | 31.8 to 31.9 | 13.7 to 14.1 | 74.1 to 74.6 |
| stitches | 4628 to 4551 | 9530 to 9459 | 10100 to 9859 | 10630 to 10240 | 11779 to 11849 | 3144 to 3726 | 16683 to 16964 |
| Stitches under 0.5 mm | 757 to 735 | 2582 to 2609 | 2797 to 2727 | 2969 to 2828 | 3263 to 3357 | 355 to 568 | 2196 to 2203 |
| jump threads | 2 to 2 | 60 to 53 | 62 to 60 | 49 to 48 | 64 to 55 | 11 to 8 | 354 to 321 |
| thread changes | 2 to 2 | 8 to 7 | 12 to 11 | 8 to 8 | 11 to 11 | 2 to 3 | 8 to 8 |
| thickest fills | 3.99 to 3.81 | 3.87 to 4.0 | 3.97 to 3.96 | 3.92 to 4.04 | 3.98 to 3.99 | 4.0 to 3.98 | 3.92 to 4.0 |
| Area at 3 fills (mm²) | 10.5 to 9.0 | 54.2 to 74.5 | 63.8 to 80.5 | 53.5 to 70.5 | 94.0 to 114.2 | 32.8 to 21.8 | 31.8 to 101.2 |
| bare % | 0.0 to 0.0 | 0.04 to 0.02 | 0.04 to 0.01 | 0.01 to 0.0 | 0.02 to 0.01 | 1.06 to 0.29 | 1.23 to 1.96 |
| astray | 0 to 0 | 4 to 5 | 3 to 3 | 1 to 3 | 4 to 2 | 13 to 21 | 90 to 118 |
| detail lost % | 0.0 to 0.0 | 14.4 to 13.9 | 17.1 to 16.1 | 10.2 to 10.8 | 7.9 to 8.0 | 25.0 to 6.2 | 16.3 to 16.3 |
| Parts sewn | 9 to 10 | 156 to 148 | 174 to 177 | 149 to 138 | 191 to 178 | 22 to 22 | 428 to 472 |
| Time to plan (s) | 0.5 to 0.5 | 4.2 to 2.6 | 5.7 to 4.7 | 5.4 to 4.2 | 6.0 to 6.5 | 0.3 to 0.3 | 14.0 to 11.7 |
| shapes whose rule changes | 0 of 5 | 40 of 78 | 50 of 90 | 29 of 69 | 35 of 92 | 5 of 9 | 69 of 550 |

- **Better.** The anchors fall by 28 to 57 %, the jags by 48 to 77 % and the teeth by 16 to 87 %. The dogs
  sew 1.5 % fewer stitches and leave 8 % fewer jump threads, and the colour error and the detail lost
  stay within 1 of what they were. Every picture traces into the same shapes, so no shape is lost. The
  badge's ring keeps more of its likeness, since it is traced as a ring of even width.
- **Worse.** The dogs have 28 % more area at 3 fills, 340 mm² where they had 266, and one of them sews
  4.04 fills at its thickest spot, past the 4 fills where the warning starts. The lettered patch has 3
  times the area at 3 fills, and 118 stitches astray where it had 90. The badge, flat art traced without
  the `logo` style, sews 18 % more stitches and 60 % more short ones.
- **Rules.** 29 to 50 of each dog's 69 to 92 shapes sew by another rule, as the shapes' outlines change.
  The pear's shapes sew as they did.

**Off by default.** The previews of the dogs with and without the options look alike to the eye, with
other jumps, and the options change how a
traced picture sews far more than its outlines change. A shape's stitch rule reacts to how its outline
is drawn, so a third to a half of each dog's shapes sew by another rule, more of the design sews thick,
and one dog passes the warning's limit. Until that rule is steady against how an outline is drawn, which
step M13.53 takes up, the fewer points aren't a gain a person gets for free. They wait for M13.53 to be
measured again, and turned on by default if they then show a clear gain.

The committed traces of the pear and the dogs are not traced again: they are the inputs of their own
cases, and this step changes only what `stitch plan` asks the tracer.

### Smoothing in StitchCraft

The tracer's options reach only pictures `stitch plan` traces itself. A document traced in VectorCraft by
hand, or by another tracer, reaches StitchCraft as it is. The pass below smooths an automatic area's
outline at a tolerance, before anything is digitized. M13.52 builds it. The numbers below are a prototype's,
measured in M13.51 on traces made with the tracer's defaults.

**The rule.** The setting `auto_smooth_mm` is how far a point of an outline may move. 0 turns the pass off.

1. **Edges.** The outlines of the automatic areas whose setting is on are cut into edges at their
   junctions: a point where 3 or more edges meet. A segment is the same in 2 outlines when its ends and
   control points are, either way round, so an edge 2 areas share is one edge. An edge shared with an
   area whose setting is 0, or with a shape with settings, stays as drawn. An edge 2 areas share is
   smoothed at the smaller of their settings.
2. **Corners.** Its corners cut an edge again. A corner is a point where the outline's direction
   over 4 times the tolerance before it and after it differs by 60° or more, the sharpest point of each
   run of such points. Junctions and corners stay where they are. A staircase of pixels smaller than the
   tolerance turns both ways within that span, and so has no corner.
3. **Smoothing.** Each piece between fixed points is resampled every fifth of the tolerance and smoothed
   by a Gaussian whose deviation is the tolerance, measured along the piece. The piece is reflected through
   each fixed end, so that the ends stay. The smoothed piece may lie no farther than the tolerance from the
   piece as drawn, nor the piece as drawn from it (their Hausdorff distance). Where it would, or where a
   ring would gain or lose more than 10 % of its area, the deviation shrinks by 0.7, up to 8 times, and
   then the piece stays as drawn.
4. **Crossings.** An edge whose smoothed line would cross another line is smoothed again with half the
   deviation. That repeats until the line crosses none, and at last the edge stays as drawn.
5. **Fewer points.** The smoothed edge is drawn with the fewest of its points that stay within 0.02 mm
   of it, and every area along it takes that line.

Every area that shares an edge has the same line along it, so no gap or overlap opens between them. The
pass charges the element's budget for each point it resamples and each pair of segments it tests for a
crossing. An area whose pass needs more keeps its outline, and `SC-I0921` names it.

**The literature.** Douglas and Peucker keep the points that lie farthest from a chord, so they keep the
tips of jags, and cut points without smoothing. Visvalingam and Whyatt
([1993](https://eprints.lancs.ac.uk/id/eprint/135032)) remove the point whose triangle with its
neighbours has the least area, one point after another, and leave outlines less jagged. Saalfeld
([1999](https://www.bohrium.com/paper-details/topologically-consistent-line-simplification-with-the-douglas-peucker-algorithm/811959494144688129-144))
showed that removing points can make a line cross another, and gave tests that keep it from doing so.
PostGIS's [ST_CoverageSimplify](https://postgis.net/docs/ST_CoverageSimplify.html) simplifies the edges a
set of polygons shares, each once, with a variant of Visvalingam and Whyatt, so that the polygons still
cover their area without gaps: the shared edges of the rule are the same idea. A Gaussian along a curve
is the smoothing of curvature scale space
([Mokhtarian and Mackworth, 1992](https://www.cs.ubc.ca/~mack/Publications/b2hd-IEEE-PAMI92.html)). It
pulls a convex curve inward by about the deviation squared over twice its radius, under 0.01 mm for a
radius of 2 mm at 0.2 mm. The rule uses the Gaussian because it averages a jag away, where a method that
keeps points keeps its tip.

**VectorCraft's Simplify.** `object.path.simplify` refits each path, or each run of selected segments
between anchors that stay
([its MCP guide](https://github.com/storytold/vectorcraft/blob/main/docs/mcp.md#simplify)). It refits a
path on its own, and its guide doesn't say that a run refits the same way in both directions, so the 2
shapes of a shared edge can take 2 different lines. `stitch plan doc.vectorcraft` also runs without
VectorCraft. The pass is StitchCraft's. A Simplify that refits the edges a selection's shapes share once
is a candidate for the plug-in features RFC.

### The prototype's numbers

The prototype is a script of the step's notes, written from the rule above without the 10 % area bound.
It smoothed each picture at 4 tolerances, and `main` at `00b45915` planned the results. Points are those
needed within 0.02 mm, each shared edge counted once. Short stitches and stitches are the change from the
pictures as traced. Recoloured is the share of the picture's area whose colour the smoothing changed,
drawn at 10 pixels per mm.

| Tolerance | Points, 4 dogs | Points, 4 photos | Jags, dogs | Jags, photos | Teeth, dogs | Short stitches, dogs | Stitches, dogs | Short stitches, pear | Short stitches, patch | Recoloured, dogs | Recoloured, photos | Dogs' shapes whose area moved over 10 % | Photos' shapes lost |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| 0.1 mm | 11,481 to 11,647 | 224,528 to 177,423 | 0.58 to 0.71 | 12.7 to 8.0 | 1.62 to 1.70 | -1 % | -0.3 % | +3 % | -13 % | 1.5 to 1.8 % | 2.8 to 6.0 % | 0 | 0 |
| 0.2 mm | 11,481 to 11,727 | 224,528 to 144,448 | 0.58 to 0.58 | 12.7 to 4.1 | 1.62 to 1.37 | -3 % | -1.7 % | -6 % | -12 % | 1.8 to 2.2 % | 4.0 to 8.7 % | 0 | 0 |
| 0.4 mm | 11,481 to 10,659 | 224,528 to 113,794 | 0.58 to 0.23 | 12.7 to 1.8 | 1.62 to 0.69 | -6 % | -2.9 % | -15 % | -13 % | 3.0 to 3.9 % | 5.9 to 12.7 % | 14 | 41 |
| 0.8 mm | 11,481 to 8,838 | 224,528 to 85,922 | 0.58 to 0.06 | 12.7 to 1.0 | 1.62 to 0.24 | -16 % | -8.7 % | -21 % | -15 % | 6.1 to 7.6 % | 8.3 to 17.8 % | 136 | 1287 |

- **At 0.1 and 0.2 mm** nothing moves farther than half a thread. The photos, traced at 3 to 4 anchors per
  mm, need 21 % fewer points at 0.1 mm and 36 % fewer at 0.2 mm, and at 0.2 mm their jags fall from 12.7 to 4.1 in every 10 mm.
  Planned at 0.2 mm, the 4 photos sew 7 % fewer stitches and 18 to 24 % fewer stitches under 0.5 mm, with as many jumps, and plan in 18 to 59 % less time. Their fabric measures, stitches astray and likeness, were not run: each takes hours on the shared machine. The dogs and the pear, already fitted as curves at under 1 anchor per mm, need as many
  points as before, and sew 3 and 6 % fewer short stitches at 0.2 mm. The traced lettered patch sews 12 % fewer and 4
  more of its parts as satin columns.
- **At 0.4 mm** the jags and teeth of the dogs halve, and the outlines move as far as a thread is wide.
- **At 0.8 mm** the pear's teeth are gone in the step's pictures, but 7 % of the dogs' area
  changes colour and 136 of their shapes grow or shrink by more than 10 %. The photos lose 1,287 small
  shapes, which the area bound keeps.
- **Rules.** The dogs sew 17 to 29 of their shapes by another rule at 0.1 and 0.2 mm, about as many as the
  same outlines flattened, with more columns (48 to 59 at 0.2 mm) and as many refused. The shift test
  doesn't change a rule at 0.2 mm either. At 0.4 mm 104 parts are refused where 85 are today. Not every changed rule is better: on the flower photo at 0.2 mm, a petal's part that was filled sews
  as a column whose stitches fan along an edge still ragged at a thread's scale.
- **Points on the dogs.** The tracer has already fitted their outlines as curves. The pass draws a smoothed
  curve with straight lines within 0.02 mm, which takes as many points. The tracer's Paths 0 and Corners 0
  are what cuts their points.

### The setting and the notice

- **`auto_smooth_mm`**, a length from 0 to 1 mm among the Advanced settings, labelled "Smooth
  outlines". The Auto effect's dialog holds 12 rows, so it has none. 0 is off and the default, for a document and for a picture `stitch plan` traces alike. A
  person sets it with `--set auto_smooth_mm=0.2`. 0.2 mm is half a thread: on every picture of the set it
  cut short stitches, with nothing moved that thread shows.
- **`SC-I0921`**, info, once for the design: "The outlines of 107 automatic areas were smoothed within
  0.2 mm: they are drawn with 3126 points, where 3067 drew them within 0.02 mm." It names the areas
  whose pass needed more than the budget, which keep their outlines.
- **Shapes a person drew.** VectorCraft's Expand leaves plain paths, so a document doesn't say which of
  its shapes were traced. StitchCraft never smooths a document's shapes unless the setting asks, and never a shape with
  settings. An edge such a shape shares stays as drawn on both sides.

### The owner's decisions

On 2026-11-20 the owner decided:

1. **The tracer's options.** Paths 0 and Corners 0 as an option of `stitch plan`, off by default, as for
   the smoothing pass. M13.51 built it.
2. **The smoothing pass.** Off by default everywhere, pictures `stitch plan` traces included, and built all
   the same as a setting (M13.52).
3. **The pear's teeth.** Thread shows them, so no default removes them. A person who wants a clean edge
   between 2 bands of colour sets `auto_smooth_mm` to 0.8 mm once M13.52 builds it.
4. **Traced letters that flip.** Not these steps': a later step looks at the columns' rule for a stem that
   leans by a pixel.

### What M13.52 built

The pass is the engine's `auto::smooth` module, run on the automatic areas after their clips and before the
background and the layers. Its cases are in `crates/stitchcraft-engine/tests/auto_smooth.rs` and in the
module's own tests. 2 things differ from the rule above:

- **Corners next to each other.** The 4 corners of a rectangle are 4 points next to each other in its outline.
  A run of turning points goes on only while its points lie within 4 times the setting of each other along
  the line: then a rectangle keeps its 4 corners, and a thin strip's end is one turn.
- **An edge that keeps its line.** An edge whose smoothed line has the same points as drawn, a straight side
  for one, keeps the segments it was drawn with.

### The build's numbers

The step's probe planned every picture of the set off, at 0.2 mm and at 0.4 mm, with the branch at
`076162e9` built at opt-level 2. The fabric measures of the 4 local photos were not run, since each takes
hours. Points are those of `SC-I0921`, which counts an edge 2 areas share once for each area. The other
columns are the change from the setting off.

| Pictures | Points at 0.2 mm | Short stitches at 0.2 mm | Short stitches at 0.4 mm | Stitches at 0.2 mm | Planning time at 0.2 mm | Rules changed at 0.2 mm |
|---|---|---|---|---|---|---|
| 4 traced dogs | 19047 to 18271 | -3.0 % (-0.2 to -6.8 %) | -4.7 % | -0.9 % | -9 % | 21 to 27 of 69 to 92 shapes |
| The pear | 1295 to 1408 | -5.9 % | -12.8 % | -0.7 % | as before | none |
| The lettered patch | 17972 to 19959 | -12.8 % | -10.8 % | -3.3 % | -32 % | 156 of 550 |
| The flat badge | 1503 to 1426 | -15.2 % | -12.7 % | -8.1 % | -50 % | none |
| 4 traced photos | 387799 to 244567 | -21.3 % (-19.2 to -22.9 %) | -25.7 % | -7.1 % | -45 % | 72 to 277 shapes each |

What gets worse at 0.2 mm:

- **Stitches astray** rise on every dog, from 1 to 4 to between 4 and 9. At 0.4 mm they reach 10 to
  25. A shape whose part changes rule sews its stitches a little off the drawn outline.
- **The area at 3 fills or more** grows on 3 of the 4 dogs, by 6 to 33 %, and the thickest spot by up to
  0.15 fills, still under the cap's warning.
- **The lettered patch** loses 133 of its thin traced lines. Each was a sliver just over 1 mm², and smoothed
  within a tenth of its area it falls under the speck bound, 0.96 to 1.0 mm², and merges into its neighbour. So the patch
  sews 260 shapes where it sewed 389, with 73 fewer jump threads, but its bare fabric grows from 1.2 to
  1.9 % and its stitches astray from 90 to 128.
- **More columns on the photos.** The flower photo sews 27 parts as satin columns where it sewed 13, and
  their stitches fan along edges that are still ragged at a thread's scale.

The likeness of the dogs moves by less than 0.01 either way, and the pear and the badge sew as before but
for shorter stitches. The area bound keeps every ring within a tenth of its area, and every ring stays. The
probe's pictures, off, at 0.2 mm and at 0.4 mm, are in the step's notes.

Against the prototype, the build cuts short stitches as much. The dogs sew 3 % fewer and the pear 6 %, as
the prototype's did, the patch 13 % where it cut 12 %, and the photos 19 to 23 % where it cut 18 to 24 %. Its points
differ for 2 reasons. The notice counts a shared edge for each of its areas, where the prototype counted
it once. A straight side is drawn with the segments it had, where the prototype drew every edge anew. So the dogs need
4 % fewer points at 0.2 mm, where the prototype's needed 2 % more. The pear and the patch need 9 and 11 %
more, since a smoothed curve takes more points within 0.02 mm than the curves the tracer fitted.

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
