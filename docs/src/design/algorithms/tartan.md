# Tartan fill

<!-- implements: crates/stitchcraft-engine/src/generators/tartan/** -->

A plaid woven of warp and weft, sewn as rows of running stitches across the fill (P3, M10.6). A woven
tartan's threads cross over and under each other, and each colour shows in the stripes it is woven into,
both ways. Rows that slant across the stripes of both directions, taken by the warp and the weft in turns,
give that look. Where stripes of 2 colours cross, the warp's rows alternate with the weft's, mixing the
colours as woven threads do. Each colour's rows sew in that colour's thread.

StitchCraft sews Ink/Stitch's tartan fill as Ink/Stitch sews it, read at `d59c9ab`: `lib/tartan/utils.py`
and `lib/tartan/palette.py` for the sett, `lib/stitches/tartan_fill.py` for the stitches, and the fill
element's tartan settings in `lib/elements/fill_stitch.py`. This page describes them in our own words
(ADR-0012), and the code is written from it. Ink/Stitch was also run as a black box on setts and shapes
made for the purpose. M10.6 sews straight rows. M10.7 adds the herringbone, and an editor that writes setts
from Ink/Stitch's palette code or from a threadcount of the Scottish Register of Tartans.

Colorful, Emilio 20 tartan, Kum Tsoan relief, Kum Tsoan tartan and Perspective tricolore, 5 fonts of
Ink/Stitch's library at `c7e3a05`, sew tartan fills. All but the last set herringbones 1.6 to 5 mm wide on
some of their shapes.

## Parameters

| Key | Meaning | Default |
|---|---|---|
| `tartan` | the sett, an object of JSON as Ink/Stitch's tartan editor saves it | empty, for Ink/Stitch's default sett |
| `tartan_angle` | the rows' angle to the stripes, in degrees | -45 |
| `rows_per_thread` | how many rows go to the warp before as many go to the weft | 2 |
| `herringbone_width_mm` | the width of a herringbone's bands, 0 for straight rows | 0 |
| `stop_at_ending_point` | whether every colour starts where the piece starts and ends where it ends | off |

A tartan reads the tatami fill's `row_spacing_mm`, `max_stitch_length_mm`, `staggers`, `skip_last` and
`expand_mm`, its whole underlay, the fill's running stitch and its tolerance for the needle's travel, and
`bean_stitch_repeats` for its runs. Ink/Stitch's window doesn't show `skip_last` for a tartan, but its
tartan reads it, and the compatibility contract says so (`read_by`).

## The sett

The `tartan` setting is an object of JSON. Its `palette` is the code of the stripes. `symmetry` mirrors the
sett at its ends, or else it repeats. `scale` scales every width, in percent, `rotate` turns the whole sett,
in degrees, `offset_x` and `offset_y` move it, in millimetres, and a stripe no wider than
`min_stripe_width` sews as a run. A key that is missing takes the value Ink/Stitch's editor starts from: no
turn or offset, mirrored, at 100 %, and runs for stripes of 1 mm or less. The editor's other keys, such as
`equal_warp_weft` and `output`, don't change what is sewn and are passed over. Ink/Stitch stops with an error on a
sett that isn't JSON or lacks a key it reads. StitchCraft takes the editor's value for a missing key, and
skips the element for the rest, with `SC-E0323`, which says why.

**The palette code.** Each stripe is a colour in brackets, then its width in millimetres:
`(#0a55a3)/2.4 (#f0f0f0)0.4 (#ed171f)/2.4`. The colour is `#` with 3 or 6 hexadecimal digits, or a colour
name of SVG. A `?` before the width leaves the stripe unsewn, and a `*` sews it in the colour of the last
stripe before it sewn as a fill. A `|` parts the warp's stripes from the weft's, and without one the weft is
the warp. Ink/Stitch takes the stripes wherever they lie in the text and passes over what lies between
them, such as slashes and the dots of a repeated sett. Its editor writes a repeated sett's code as a list,
`...['(#00673e)5.0', '(#000000)1.5']...`, which reads as its stripes. A colour that can't be read leaves
its stripe unsewn, as Ink/Stitch leaves it, with `SC-W0328`. A code with more than one `|`, or a width that
isn't a number, can't be read.

**Colour letters.** A code without brackets is a threadcount: each stripe is a run of colour letters, then a
`?`, a `*` or neither, then its count of threads, as in `B/24 W4 R2`. Letters without a count are passed
over, and the fill reads each thread as 0.5 mm wide, as Ink/Stitch's tartan does. The letters, in capitals
or not, stand for the shades the Scottish Register of Tartans publishes in "Tartan Register colours"
(<https://www.tartanregister.gov.uk/docs/Colour_shades.pdf>), `B` for blue and `DG` for dark green. The
list gives several shades for most of its 25 letters. StitchCraft's table (`generators::tartan::shades`)
holds, for each letter, the shade of the list that Ink/Stitch sews, so that a sett written in letters sews
the same threads in both programs. The list is
Crown copyright. The register's copyright page (<https://www.tartanregister.gov.uk/copyright>) lets its
text be reused with an acknowledgement, which `NOTICE` gives. Letters the register uses for no colour leave
their stripe unsewn, with `SC-W0328`.

**The register's threadcount.** The register's search gives a sett as a threadcount with a palette of its
own. A line saying `Threadcount:` comes before the stripes, letters and counts run together as
`B24W4B24R2K24G24W2`, and a line saying `Pallet:` before each letter's colour, as `B=0000FFBLUE;`. A code
that says `Threadcount` reads so: the counts from the line after the last `Threadcount:` line, and each
letter's colour from the line after the last `Pallet:` line, whose letters, `=`, a `#` or none and 6
hexadecimal digits name a colour. There, letters without a count make the code unreadable. With a `|`
anywhere in a code of letters, every stripe is the warp's and the weft has none, as in Ink/Stitch.

An empty setting sews Ink/Stitch's default sett, `(#101010)/5.0 (#FFFFFF)/?5.0`: stripes 5 mm wide of a
thread almost black, with 5 mm left unsewn between them. `SC-W0325` says so. VectorCraft's fill dialog
shows the setting as text, and StitchCraft Tartan, a filter, writes it from a code ([the
editor](#the-editor)).

**A direction's width.** Ink/Stitch measures each direction's stripes, once scaled: the stripes as wide as
`min_stripe_width` or wider add up, but for those sewn in the colour before them. A direction without such
a stripe measures its narrower stripes that are sewn instead. The wider direction's width widens the box
the stripes are laid over (below). A direction that measures 0, or has no stripe sewn in its own colour,
sews nothing, and a sett whose 2 directions sew nothing sews its underlay alone, with `SC-W0326`.

## The full sett

The stripes, scaled, become a sequence of bands, each of a colour or of none, and runs, each a line in a
colour between 2 bands.

- **Runs.** A stripe sewn in its own colour no wider than `min_stripe_width` sews as a run along its
  middle. Ink/Stitch compares the 2 widths in CSS pixels, and StitchCraft compares them the same way, so a
  stripe at the limit falls the same side. Half its width goes to the band before it, and half to those
  after it. Ink/Stitch adds that half to each band that follows, not to the next alone. It stops at a band
  left unsewn or sewn in the colour before it, which takes the half, or where the next run gives its own
  half instead. A run at the end leaves its half as a band of its own. StitchCraft lays the bands so too.
  Ink/Stitch, run as a black box on setts with runs among their stripes and at their ends, mirrored or
  repeated, lays the half more than once.
- **The colour round a run.** The band before a run is in the colour of the last band sewn as a fill. A run
  before any such band lies in the colour of the first stripe wider than the narrowest for a mirrored sett,
  which it goes back to, and of the last for a repeated one, whose pass before ends with it.
- **Mirrored.** With `symmetry` on, the sequence goes back from its last step but one to its second. The 2
  stripes it turns at are then sewn once, as is a run it turns at. Then the whole repeats.

## Stripes and rows

Each part of the fill is grown by `expand_mm` and cut into the pieces that leaves, as a tatami fill's parts
are. Each piece gets its stripes and rows of its own.

**The box.** The stripes cover a square round the middle of the piece's bounds. It reaches as far from the
middle as the radius of the smallest circle round the piece, plus a margin. The margin is the wider
direction's width or the width of 2 rows per thread of rows, whichever is more. Its left and top sides move out by the sizes of `offset_x`
and `offset_y`. The circle's radius is found from the piece's convex hull, by the randomized incremental
method (Welzl's), its points shuffled by a fixed seed so that it is the same every time.

**The stripes.** The warp's full sett is laid from the box's left side as upright bands, and the weft's from
its top as level bands, one after another until a band starts past the far side. Bands of one colour that meet
are one band. The whole is turned by `rotate` degrees about the middle of the bounds, counter-clockwise in
y-up axes, as Ink/Stitch's geometry library turns it. The canvas shows y down, and there a positive turn
shows clockwise. StitchCraft works in the sett's own frame. It turns the piece back by the sett's angle,
which makes each band a slab between 2 lines square to an axis, and then turns the lines it makes forward
again.

**The rows** run across the box at `tartan_angle` to the stripes, from its turned left side to its turned
right side. The left side's length is cut into equal steps no longer than the row spacing, and a row starts
at each step. Along a row, needle points lie the longest stitch apart, from a `staggers`-th of a stitch
further on than the row before's, and both its ends are needle points too. A row with room for fewer than
2 needle points between its ends has its ends alone. The rows go to the warp and the weft in turns of
`rows_per_thread`. Ink/Stitch sorts each direction's rows by their starts, x first, which takes them from
one end of the left side or the other, as the sett's angle and the rows' angle turn it. Every other row of
a direction runs back, and its rows zigzag.

## A colour's lines

**In each band**, each row of the band's direction that crosses it for more than half a CSS pixel gives its
stretch inside the band, with the needle points that lie there. Each stretch is joined to the next by a
straight line along the band's edge, in equal stitches no longer than the running stitch, unless either end
lies within 0.005 pixel of the piece's rings. So a band's rows make one zigzag, which breaks at the
outline. Only the stretches that touch the piece's bounds count, with one more either side, whose joins may
cross into it. The zigzag is cut where it leaves the piece. Where a line ends inside the piece,
more than 0.05 pixel from its rings, it runs on from that end along the band's edge to the nearest ring, in
the running stitch's length.

**The runs** lie along their stripes' middles across the box, cut by the piece. A piece of 0.5 mm or less is
left out. Each piece is cut into equal stitches no longer than the running stitch, along the warp, and the
longest stitch, along the weft, as Ink/Stitch cuts them.

**Each colour's lines** are its bands' in the warp, then in the weft. The colours come in the order of
their first band or run in the warp's sett, and then come the weft's colours that the warp has none of.

## Herringbone

With `herringbone_width_mm` above 0, the rows lie in bands that wide (M10.7). A herringbone twill's ribs
turn at the edge of each band, and the cloth shows columns of chevrons: each band's rows slant the other way
from the band before's, and the weft's rows run on from band to band. Ink/Stitch lays them so in
`lib/stitches/tartan_fill.py`, read at `d59c9ab`, and Colorful, Emilio 20 tartan and Kum Tsoan tartan set
herringbones 1.6 to 5 mm wide.

**The bands.** In the sett's frame, before it is turned, the bands stand upright side by side from the
box's left side, 0.005 CSS pixel apart so that no 2 bands' rows meet. Each band is the herringbone's width,
and bands are laid while one starts before the box's right side. They turn with the sett, and `tartan_angle`
doesn't turn their rows, which cross the stripes at 45°.

**A band's rows** join its 2 sides, each as long as the box is high plus the band's width and cut into
equal steps no longer than the row spacing. A row goes from each step of the left side to the same step of
the right side. In the first band, the third and so on, the left side starts a band's width above the box,
and the rows run down to the right, as the canvas shows them. In the others it starts at the box's top, and
the rows run up to the right. Every band so has the same rows, moved across and slanted its own way, and
StitchCraft works them out once. Along each row the needle points are staggered from the band's first row,
as straight rows' are. In the first band, the third and so on, the first `rows_per_thread` rows of every
2 × `rows_per_thread` go to the weft and the others to the warp, and the other way round in the other bands.
Each direction's rows of a band are taken in the order straight rows are, every other one sewn back.

**The warp** is laid band by band: the second band, the fourth and so on, then the first, the third and so
on. In each band, each warp stripe's rows are joined along the stripe's edges and cut by the piece, as
straight rows are, but a line that ends inside the piece doesn't run on to its rings. A warp colour comes
in the order of its first line.

**The weft** is joined across the bands. Each band's weft rows are first joined end to start into one
zigzag, in stitches no longer than the running stitch. Each weft stripe that reaches the piece cuts every
band's zigzag, and a band's pieces in the stripe longer than half a CSS pixel are joined along the stripe's
edges, unless an end lies on the piece's rings. While an edge of the stripe crosses the piece, the bands'
lines in the stripe are then joined into one, from the band at one end of the stripe to the band at the
other. Ink/Stitch takes the stripe's edges from its outline as its geometry library orders that outline,
from the corner of least x. With the sett turned less than a quarter turn either way, or a quarter turn
exactly, the stripe's lower edge, as the sett's frame shows it with y down, runs to the right, and the
bands are taken from the right. Otherwise its upper edge runs to the left, and they are taken from the
left. The end of the first band's last line farther along that edge goes on to the next band. Each line
after it starts at its end on the edge nearer where the line before ended, the first edge on a tie, and is
joined to it along that edge. Ink/Stitch moves the weft's lines right and down by 0.00123 pixel before it
cuts them by the piece, so that a line along the outline falls inside or outside it as there, and StitchCraft
moves them as much. Every weft colour whose stripes reach the piece comes after the warp's colours, lines or
none.

**Where a line meets itself.** Ink/Stitch's geometry library breaks a weft line where it finds the line
meets itself as it cuts the line by the piece. Near the outline a weft line can run twice along the same
stretch of a stripe's edge, and whether the library finds a meeting there hangs on the last bits of its
arithmetic. StitchCraft sews such a line whole (`DEV-FILL-030`).

**What it costs.** The budget is charged for every band's rows as if each were made, their needle points,
each stripe each band is tested against, and each point of a zigzag, before memory is taken for them.

## Sewing

A tartan sews its parts in a tatami fill's order, nearest the needle first ([fills](fills.md#5-parts)). Each
part sews the tatami fill's underlay first, in the element's own thread. Then each of its pieces sews its
colours.

**The colours sewn as fills** come in the order above. Their last colour that has runs too moves to the end
of them, so the thread doesn't change between its rows and its runs. Each colour's lines are routed as a
tatami fill's rows are routed ([routing](fills.md#3-routing-every-segment-once)) and sewn once. Between
them the needle runs along the piece's outline, never under the rows. A colour goes from where the colour
before it ended, or the needle, towards the end that the next part or element offers. The travel before
its first line and after its last is left out, so it starts and ends with its rows. Where the needle or the
end is missing, the colour starts or ends at the outline's second point, as in Ink/Stitch. Its geometry library rebuilds the outline,
which then starts one point on from where the drawing starts it, so that point is the outline's third here.

**With `stop_at_ending_point` on**, every colour keeps its travel. Each starts where the needle was when the
piece's colours began, after its underlay, and travels on to the end.

**The runs** come after every colour sewn as a fill, the colour they share with the fills first. Each
colour's runs are routed the same way, from where the colour before ended towards the end. Their travel is kept, and
`bean_stitch_repeats` sews each of their stitches back and forth but the travel's.

**Several parts.** Each part sews its underlay and then its colours. Then, as Ink/Stitch sorts a tartan's
groups, its underlays and colours sewn as fills are ordered by where their colour first comes among them,
then its runs the same way, so that each thread sews in one stretch. An underlay in a colour a fill sews too
sorts with that colour.

**Threads and colour blocks.** Each colour's lines, and each colour's runs, are a colour block of their own
thread, which assembly trims after when the element's `trim_after` is on. The underlay sews in the element's
thread and ends no colour block ([several threads in one
element](../engine-pipeline.md#several-threads-in-one-element)). A colour of the element's own sews in the
element's thread, named or not, and the others in threads of their colours alone. A named and an unnamed
thread of one colour would be 2 threads. A PES file writes them as 2 entries of its palette, and the second
is another colour. The sewing order reads a tartan's threads from its settings alone, before it is sewn: its
own thread for the underlay, then the colours above.

**Stitches.** A needle point nearer the one before than the element's shortest stitch is left out, as
finalize would leave it out. Where the rows cross a stripe sewn as a fill in a stretch shorter than the
shortest stitch, a warp stripe's width over the cosine of the rows' angle or a weft stripe's over its sine,
`SC-W0327` says needle points of it are left out. A herringbone's rows cross the stripes at 45°, and a row
crosses no more of a stripe than its band's width: a herringbone narrower than the narrowest stripe is said
instead. The budget is charged for the sett's steps, the rows and their needle points, the
bands each row is tested against and the stitches, before memory is taken for them. A sett too fine for the
budget ends the element with `SC-E0004`.

A preview too costly to sew shows each colour's lines and runs, without the stitches.

## How it compares with Ink/Stitch

The differential sews TS-17B and 4 files of its own in both programs, and they agree by every measure
(`REQ-FILL-TRT-011`). Their colour blocks are the same, in the same threads, with the same trims and jumps,
and each starts and ends at the same points. StitchCraft sews 2 to 6 % fewer stitches. It leaves out needle
points nearer each other than the machine's shortest stitch, 0.3 mm, where Ink/Stitch leaves out those
within 0.1 mm (`DEV-FIN-001`): most of them are the stitches where a row enters or leaves a band. At
`tartan_angle` 0, the joins between rows along a band's edge are a row spacing long, 0.25 mm, and a probe
square sewed 13.5 % fewer stitches for it. Between their runs, the 2 programs' needles can travel along
different stretches of the outline, where they pair the graph's odd nodes differently (`DEV-FILL-006`).
Ink/Stitch's geometry library divides a run's length by the stitch length in pixels, and where the quotient
is a whole number its rounding can give one stitch more. A run 40 mm long in stitches of 2.5 mm takes 17
stitches there, against 16 here.

The glyph differential sews the tartan fonts' 313 glyphs too. Their files are older than Ink/Stitch's
version 4, and there a fill without a longest stitch of its own sews stitches 3 mm long, as Ink/Stitch
updates it ([files of older versions](../svg-input.md#files-of-older-inkstitch-versions)). 277 of the glyphs
agree, or differ only as the ledger explains. The 35 that set a herringbone are sewn otherwise until M10.7,
and `conformance/inkstitch/glyphs.toml` lists them as known. It lists Colorful's ’ as known too: its
S-stitch satin column starts elsewhere than Ink/Stitch's.

## Test sheet

TS-17B sews 5 tartans 40 mm square, each trimmed after its colours ([machine
testing](../../plan/machine-testing.md)). One sett is mirrored at its ends, and one is repeated, written as
Ink/Stitch's editor writes it. One has a warp and a weft apart, and one has stripes 0.5 and 0.8 mm wide sewn
as runs. The last is turned 30° and scaled by 150 %. Each underlay sews in a colour of its sett other than its first
stripe's, so the underlay is a colour block of its own: in the first stripe's colour, the jump from the
underlay to the rows would leave its thread under them on the reference machine. Every stripe is one of
Brother's threads, so both programs' files name each thread as itself. VectorCraft's Test Sheet filter
doesn't draw TS-17B, as its fill dialog has no field for a sett.

## Properties (conformance)

| Requirement | Property |
|---|---|
| `REQ-FILL-TRT-001` | The sett is the `tartan` setting, its palette code read for each direction's stripes, with Ink/Stitch's default for an empty one (`SC-W0325`), unsewn stripes for unread colours (`SC-W0328`), and `SC-E0323` for a sett that can't be read |
| `REQ-FILL-TRT-002` | The full sett is scaled and mirrored or repeated. Its narrow stripes are runs, whose halves widen the bands after them |
| `REQ-FILL-TRT-003` | The stripes cover a square round the piece, as wide as its smallest circle plus the margin, moved by the offsets and turned by the sett's angle, bands of a colour that meet made one |
| `REQ-FILL-TRT-004` | The rows cross the square at `tartan_angle`, a row spacing apart at most, staggered, in turns of `rows_per_thread` |
| `REQ-FILL-TRT-005` | Each band's rows are joined along its edges into a zigzag, cut by the piece, its ends inside run on to the rings |
| `REQ-FILL-TRT-006` | The underlay first, in the element's thread, then each colour's lines in its own thread, routed and without the travel at its ends |
| `REQ-FILL-TRT-007` | The runs after the fills, cut into equal stitches, their travel kept, with bean stitch |
| `REQ-FILL-TRT-008` | `stop_at_ending_point` keeps every colour's travel, from the piece's start to its end |
| `REQ-FILL-TRT-009` | Several parts sort their underlays and colours, then their runs, by colour |
| `REQ-FILL-TRT-010` | A herringbone is sewn as straight rows, with `SC-W0324`, until M10.7 |
| `REQ-FILL-TRT-011` | TS-17B's tartans and the differential's own sew in Ink/Stitch as in StitchCraft by every measure |
