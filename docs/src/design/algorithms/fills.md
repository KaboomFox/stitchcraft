# Fill generators

<!-- implements: crates/stitchcraft-engine/src/normalize/region/**, crates/stitchcraft-engine/src/generators/tatami/**, crates/stitchcraft-engine/src/generators/pull.rs -->

A fill covers the area its path bounds, its [region](#region), with rows of stitches. Tatami is P1 (M5);
contour, meander and circular are P2 (M7); guided, linear gradient, tartan and cross stitch are P3 (M10).

A fill sews the drawing's shape with Ink/Stitch's stitches. Ink/Stitch's rows, their spacing and stagger,
the order it sews them in, its travel, underlay and compensation are what make its fills look good
embroidered. StitchCraft keeps them, with their parameters' names and defaults, and a file sews alike in
both. Where the area Ink/Stitch fills departs from the drawing, StitchCraft follows the drawing, and a
diagnostic says where the two differ. A part Ink/Stitch leaves out without a word is left out with a
warning.

A fill whose `fill` setting is off sews nothing, and nothing is said of it, as in Ink/Stitch
(`REQ-FILL-003`). The path's outline, if it has one, is sewn as its stroke settings say, and the element
before it still ends near its area, as Ink/Stitch's does.

## Region

A fill's region is the area an SVG renderer paints for its path, in parts with holes
(`stitchcraft_engine::normalize::region`, `REQ-FILL-001`):

1. **Rings.** Each subpath is flattened within a tenth of a CSS pixel, as Ink/Stitch flattens a fill's
   outline, and closed. A subpath of fewer than 3 distinct points bounds nothing and is left out
   (`DEV-FILL-002`).
2. **Cuts.** Wherever 2 segments cross, touch or run along each other, both are cut there, on a grid of
   2⁻³⁵ mm, about 2.9 × 10⁻¹¹ mm. The rings' points go on the grid, much as Ink/Stitch puts a fill's
   shape on a grid of 10⁻¹⁰ CSS pixels, about 2.6 × 10⁻¹¹ mm. So the corners that shapes share, which
   relative path data and transforms leave a few units in the last place apart, become one point, and a
   gap or an overlap that narrow closes. A segment that passes through the cell of a corner, or of a
   point where 2 segments cross, is bent through that point. This is snap rounding, and the edges then
   meet only at their ends however rounding left the drawing. Before it, crossings worked out in floating
   point and snapped to points nearby could leave 2 edges crossing with no point between them, and whole
   faces filled or empty against the rule. On the grid, which side of a line a point lies on and which
   way a ring turns are decided exactly. A stretch drawn more than once becomes one edge that counts how
   many times it was drawn each way.
3. **Faces.** The edges bound faces. Outside everything the winding number is 0, and crossing an edge
   from its right to its left adds its count. Every face's winding number follows from its neighbours'
   in whole numbers.
4. **Fill rule.** A face is filled when its winding number is not 0 (`nonzero`, SVG's default) or is odd
   (`evenodd`). Ink/Stitch's area differs in 2 ways, and StitchCraft follows the drawing in both
   (`DEV-FILL-001`). Ink/Stitch takes every subpath but the largest as a hole. Where `nonzero` fills a
   face inside that even-odd leaves empty, StitchCraft fills it, and `SC-I0306` says Ink/Stitch leaves it
   empty. And Ink/Stitch repairs a shape that is not valid by merging the subpaths' lines, a stretch drawn
   twice into one, and filling the faces they bound by how deeply they nest. A face beside the outside
   nests in nothing, and is always filled. Where the fill rule leaves such a face empty, a notch drawn as a
   hole along the outline or a subpath drawn twice, `SC-I0313` says Ink/Stitch fills it.
5. **Rings of the parts.** The edges between filled and empty faces are joined into rings with the
   filled side on their right. Outlines turn clockwise and holes counter-clockwise in y-up axes, the other
   way round on screen, where y points down. Where rings meet at a point, they are joined the way GEOS
   joins them, the geometry library behind Ink/Stitch's shapes. Parts that touch at a point stay apart. A
   hole that touches its outline stays a hole. Each ring starts at its point that comes first in the
   drawing, and parts and holes keep the drawing's order too.
6. **Parts too small to sew.** A part of 3 square CSS pixels or less (0.21 mm²) is too small for a row
   of stitches and is left out. Ink/Stitch leaves it out without a word. StitchCraft says so with
   `SC-W0303` (`REQ-FILL-002`). A fill under 20 square CSS pixels (1.4 mm²) gets `SC-W0304`, as
   Ink/Stitch warns of it. A fill in parts gets `SC-W0307`, since each part is sewn on its own.

The work is charged to the element's budget, one unit for each point, edge bent and grid point tested
against it, pair of edges whose spans along x overlap, half edge, ring compared with a face or an outline,
and point walked to find a ring's bounds or its place among the faces and outlines (`REQ-FILL-004`). Each
ring's bounds are found once. A ring that bounds no face is located only in the faces whose bounds cover
its own, and a hole apart from every outline only in the outlines whose bounds cover its own.
Property tests draw random rings on a grid, and random rings with float coordinates beside near-copies of
themselves, moved or turned by a hair or drawn back, and count how many times they wind round points off
the edges. Such a point lies in the region exactly when the fill rule fills it, unless it lies in a part
left out. Every ring found is simple and turns the right way, and the edges the rings are cut into meet
only at their ends. Drawings of shapes that share corners and edges, written with relative path data as
Inkscape writes them, fill their shapes' area, in 1 part where the shapes share edges.

## Tatami fill

Parallel rows of running stitches across the region, with the needle points of neighbouring rows offset
so they never line up into visible furrows. Most fills in most designs are tatami. Each step below sews
as Ink/Stitch sews, on StitchCraft's region, unless it says where it differs. The roadmap's M5 steps
build them in this order.

**Parameters:** `angle`, `row_spacing_mm`, `end_row_spacing_mm`, `max_stitch_length_mm`, `staggers`,
`skip_last`, `underpath`, `running_stitch_length_mm`, `running_stitch_tolerance_mm`, `gap_fill_rows`,
`pull_compensation_mm`, `pull_compensation_percent`, `expand_mm`, `random_seed`,
`enable_random_stitch_length`, `random_stitch_length_jitter_percent`; underlay: `fill_underlay`,
`fill_underlay_angle`, `fill_underlay_row_spacing_mm`, `fill_underlay_max_stitch_length_mm`,
`fill_underlay_inset_mm`, `fill_underlay_skip_last`, `underlay_underpath`.

### 1. Rows

Rows are laid across each part of the region on its own (`stitchcraft_engine::generators::tatami::rows`,
since M5.2). They run at `angle`, counter-clockwise from horizontal on screen: at 0° left to right, at
90° up the screen. Measured across them from the design's origin, the first row of a part lies the
largest whole number of `row_spacing_mm` from the origin that does not pass the part, and the rows follow
it `row_spacing_mm` apart up to the part's far side, with none on it (`REQ-FILL-TAT-002`). Fills side by
side at the same angle and spacing then share their rows. Angles of whole right angles are exact, so rows
at 0° or 90° run exactly along the drawing's axes. The nth row lies n spacings past the first, multiplied
out, since a spacing such as 0.1 mm rounds in binary, and added up row by row its rounding grew into a
row on the far side.

With `end_row_spacing_mm`, each step to the next row is the row spacing plus the difference to the end
spacing times the distance from the first row over the part's height. The spacing then reaches the end
spacing a part's height from the first row. Ink/Stitch carries the change on beyond that. Over a part
only a few rows tall, a shrinking spacing then turns so small that the rows pile up short of the far side
without end. StitchCraft keeps the end spacing from there on (`DEV-FILL-003`).

A row's *segments* are the stretches of it in the part, its outline included, split at every point where
the row meets the outline (`REQ-FILL-TAT-010`). Those are the points where it crosses the outline or the
outline touches it from inside, and both ends of a stretch where it runs along the outline. A row along
an edge is then a segment of its own, and a row that only touches the part at a point has none. This is
how GEOS, the library behind Ink/Stitch's shapes, cuts a line with a polygon. Its answers for 2,511 rows
across 406 random polygons of grid squares and half squares, 98 of them with holes, are recorded and
checked (`conformance/fixtures/geometry/shapely-rows.txt`).
Which stretches lie inside is decided by counting the edges that cross the row, each over its height
from its lower end up to but not including its upper end.

A side drawn on a row lies a hair off it by the time it is cut: the region puts its corners on a grid
2.9e-11 mm wide, and the row's place rounds. So a point of the outline within 1e-9 mm of a row lies on
it, and a row within 1e-9 mm of a part's far side lies on that side, and is left out. A part's first and
last rows, and the segments of a row drawn along a side, then don't depend on the rounding. Ink/Stitch's
do (`DEV-FILL-012`).

The top rows alone, laid over each part grown by `expand_mm` as the fill grows it, are what a preview
shows of a fill too costly to sew within the preview's budget (`generate::rows`, since M6.3). Laying
them costs a small part of sewing the fill, since no needle point is placed and no route is searched. A
28 mm square takes 632 units of the engine's work for its rows, and 54,034 to sew.

### 2. Needle points along a row

A row's needle points lie on a grid along it, as Ink/Stitch places them
(`stitchcraft_engine::generators::tatami::stitches`, since M5.3). The grid's points are the longest
stitch apart, `max_stitch_length_mm`. A fill always has one: 4 mm when it is not set or left empty, and
0.1 mm, with `SC-W0102`, when it is 0 or less, as in Ink/Stitch. Measured along the rows from
the design's origin, they lie at whole numbers of stitches plus the row's offset (`REQ-FILL-TAT-003`). A
row's number is how far across the rows it lies from the origin, over `row_spacing_mm`, rounded to the
nearest whole number, halves to the even one. Its offset is the fractional part of that number over
`staggers`, times the longest stitch. Each row's points then lie a `staggers`-th of a stitch along from the row
before's and come back after `staggers` rows, so neighbouring rows never line them up into furrows. A
fraction of a stagger draws diagonals that show less. A negative `staggers` staggers the rows the other
way, and 0 counts as 1, which lines every row's points up, as in Ink/Stitch. Fills side by side at the
same angle share the grid, as they share their rows.

A segment is sewn from one end to the other, as routing decides. It takes its start, then every
grid point past the start and before the end, and then its end, unless `skip_last` is on or the last
point lies within 0.1 mm of it. A start on the grid is sewn once. Sewn the other way, a segment takes the
same grid points.

With `enable_random_stitch_length`, random lengths take the grid's place. The point nearest the start
lies a random share of the longest stitch past it, and each next one a stitch on, longer or shorter by up to
`random_stitch_length_jitter_percent` of it, while they lie before the end. The draws come from the
element's generator, seeded with `random_seed`, in the order the segments are sewn. Ink/Stitch seeds a
generator for each segment, and its random values differ (`DEV-FILL-004`).

### 3. Routing: every segment once

Each part's segments are sewn once each, in the order Ink/Stitch routes them
(`stitchcraft_engine::generators::tatami::route`, since M5.4, `REQ-FILL-TAT-001`).

1. **Order.** The segments are taken row by row, across the rows, and within a row by how far their start
   lies from the corner of the part's bounding box where x and y are least. Each runs along its row.
2. **Graph.** Each segment is an edge between its 2 ends, and the ends are nodes. A node belongs to the
   ring nearest it, or to the outline when a hole is as near. Its place is its distance along that ring
   from the ring's start. Along each ring an edge joins each node to the next, and every other one of those
   stretches, from the first, gets a second edge. A node then meets its row and 3 stretches of ring, an
   even number of edges. Ink/Stitch's graph knows an edge between 2 nodes by its kind, so a ring of 2
   nodes doubles only one of its 2 stretches. This graph does the same.
3. **Start and end.** The fill starts at the point of its rings nearest the needle and ends at the point
   nearest its next stitch, which the next element's shape gives (section 5). Each of the 2 points gets a
   node, which splits the stretch of that ring whose chord passes nearest it. Then the node nearest the
   needle is where the fill starts, and the one nearest the next stitch where it ends. Without a needle
   the fill starts at its first segment's start, and without a next element it ends where it starts. Only
   rings that rows reach are looked at. Ink/Stitch joins an unreached ring with straight lines, which it
   sews as rows (`DEV-FILL-006`).
4. **Even degrees.** A row through a corner of the outline, or one along it, shares a node with the row
   beside it, and some nodes then meet an odd number of edges. Ink/Stitch adds paths between such nodes
   that may run along rows, and sews those rows a second time. StitchCraft pairs the odd nodes so that the
   needle's way between pairs is shortest in all and joins each pair with an edge. Up to 16 nodes every
   pairing is weighed, and past that each node pairs with the nearest left. No row is sewn twice
   (`DEV-FILL-006`). A pairing edge stands for the way between its 2 nodes that Ink/Stitch's paths double:
   one edge where a row or a stretch of ring joins the 2, and several otherwise. Travel along it is never a
   handover between neighbouring rows for `skip_last`, and it ends a stretch for gap-fill rows only where it
   stands for several.
5. **Walk.** Ink/Stitch walks a copy of the graph, which its graph library makes by adding the edges
   again, node by node in the order the nodes came, each node's edges in its own order. So in the copy a
   node lists first its neighbours that came before it, in the order they came, and then the others,
   itself included, in its own order. The walk starts at the end node. At each node it takes the node's
   row if one is left, and otherwise the node's first edge left. A node lists its edges neighbour by
   neighbour, each neighbour's in the order they came. A neighbour whose last edge is taken drops out of
   the list. When the walk reaches a node with no edge left it steps back, and the edges, in the order it
   steps back over them, are the route. The route comes back to the end node, and before it the needle
   runs from the fill's start to that node. The running stitches between rows then mostly come just
   before the rows beside them, which cover them, and the rows go back and forth like a mown lawn.
6. **Between rows.** Steps between rows in a row are one stretch, from where the last row ended to where
   the next starts. The needle travels it under the rows (section 4), or with `underpath` off along the
   ring between 2 points of one ring, the shorter way round, and between 2 rings the shortest way along
   rings and rows (`DEV-FILL-006`). Ink/Stitch's needle takes the way round with fewer nodes of its
   graph, a node every CSS pixel along the outline's sides, which is about the shorter way but not always
   (`DEV-FILL-011`). It sews running stitches of the first `running_stitch_length_mm`, at least twice the
   shortest stitch, within `running_stitch_tolerance_mm`. Ink/Stitch reads a fill's value as one number,
   and sews a value of several in its default, 2.5 mm (`DEV-SAT-005`). The first running stitch would sew
   the row's end again, so it is left out. With `skip_last` the row's end is not sewn, and the first
   running stitch stays unless the next row starts beside it.
7. **The shortest stitch.** Rows the row spacing apart start and end nearer each other than the shortest
   stitch a machine sews well, and a row's first grid point can lie just past its start. A needle point
   nearer the one before it than the shortest stitch is left out, as Ink/Stitch leaves out points nearer
   than its own shortest stitch. A part's last point stays, and the points before it that are too near it
   go instead, unless that would bring it within the shortest stitch of the part's first: then it goes
   itself. This is finalize's rule, applied in the generator. Where the rows lie nearer each other than the
   machine's shortest stitch and meet an edge square on, each row hands over to the next in a stitch too
   short to sew, so the next row's first needle point is left out (`DEV-FIN-001`). The thread then runs
   from the row's end to the next row's second needle point, and a sliver at each row's start shows. On
   the Brother, whose shortest stitch is 0.3 mm, a bar 5 mm wide at Ink/Stitch's default of 0.25 mm is
   97 % covered, and at 0.3 mm or more it is covered to its edges (`REQ-FILL-TAT-004`). Ink/Stitch's
   shortest stitch, 0.1 mm by default, keeps the handover. Along an edge at a slant to the rows, their
   ends lie farther apart: the spacing over the sine of the angle between edge and rows, 0.35 mm at 45°
   for rows 0.25 mm apart, which the Brother sews.

The route follows Ink/Stitch's from the same graph. StitchCraft's rings start where the drawing starts
them, and Ink/Stitch's where GEOS's overlay starts them, so which stretches are doubled, and with them the
route, can differ (`DEV-FILL-005`).

### 4. Travel under the rows

With `underpath`, on by default, the needle runs between rows inside the part, under the rows sewn
after it, which cover it, as Ink/Stitch's does (`stitchcraft_engine::generators::tatami::underpath`,
since M5.5, `REQ-FILL-TAT-005`).

1. **Lines.** 3 gratings of lines cross the part, laid as rows are (section 1). 2 of them lie at 45° to
   the rows, one either way, their lines 2 mm apart, and the third square to the rows, its lines √2 mm
   apart. A part smaller than 10,000 square CSS pixels (700 mm²) has less room, and its lines lie half as
   far apart. The square lines pass through the crossings of the others, so the 3 make one network. Its
   nodes are the crossings, the lines' ends on the rings and the route's nodes, and along each ring an
   edge joins each node to the next. A crossing within 0.005 px of a node already found is that node.
   Where either 45° grating misses the part, the needle runs as with `underpath` off.
2. **Costs.** A piece of line costs its length in CSS pixels over its distance from the part's rings,
   simplified within 0.5 mm, plus a tenth of a pixel. A stretch of ring costs 3 times the straight line
   between its ends. Travel then keeps to the middle of the part, away from its edges.
3. **Sewn rows close lines.** Once a row is sewn, the pieces of line that cross it close, and no later
   travel follows them over it.
4. **The way.** The needle takes the cheapest way. The way is cut into points 10 px apart within 4 px of
   it, the corners of the points after its start are cut 5 times, a quarter of the way along each side
   from each end, and its own ends go back at either end: it leaves its start straight and curves from
   there. Then it is kept inside the part. Where it leaves the part or runs along a ring, the ring takes
   its place, the shorter way round, from where it leaves to where it comes back: the hole it leaves into,
   or else the outline. It is sewn in running stitches as in section 3.
5. **A hole between the lines.** Where no line joins the 2 points, as for a hole small enough to lie
   between the lines, the needle runs along the rings and rows, as with `underpath` off. Ink/Stitch goes
   straight there, but it travels between 2 rings only from the fill's start to its end, before any row
   is sewn. StitchCraft also travels between rings to pair nodes (`DEV-FILL-006`), after rows are sewn,
   which a straight way would cross.
6. **A needle inside the part.** The fill starts at the node of the network nearest the needle (since
   M5.7). For a needle on the rings or outside the part, that is the route's start. For one inside, as
   after an inset underlay, it is a node inside, and the needle travels the cheapest way from there to the
   route's start, or on to the end of the route's first stretch of travel, as Ink/Stitch's does.

Equally cheap ways, the simplified rings and the points a way is cut into follow StitchCraft's own rules,
so the way can differ from Ink/Stitch's in its details (`DEV-FILL-007`).

Travel asks the rings where a point lies on them, how far a line lies from them, and where a stitch meets
them or lies inside. Each question looks only at the ring sides near its point, line or stitch, filed in a
grid of square cells about as many as the sides, and gets the answer a look at every side would give
(`REQ-FILL-TAT-013`).

### 5. Parts

A region in parts sews them one at a time, as Ink/Stitch sews them (`REQ-FILL-TAT-008`, since M5.4).
The part nearest the needle goes first, a part the needle is in being 0 away. With no needle, the part
with the least x goes first. Ink/Stitch sorts the parts with a stable sort. GEOS's repair gave the parts
of 400 of 400 random fills tied at their least x in order of their point of least x, and of least y
among the points level with it (shapely 2.2.0, GEOS 3.14.1). Parts that tie go in that order here too.
Each part ends at its point nearest the next part. The last ends at the point of the fill nearest its
next stitch. That is the next element's first stitch when it lies in the fill. Of a satin column's
rails, the start of the first that lies in the fill is the next stitch, and of a fill's parts, the first
outline's start that does. Failing those, for a fill after a fill, it is the first start of this fill's
outlines that lies in the next, and otherwise the fill's point nearest the next element. Ink/Stitch's
geometry library finds it so ([satin](satin.md#start-and-end)).
Each part is a group of its own, which assembly joins to the next as it joins any 2 groups:
straight on within the collapse length, and otherwise with a jump and the ties and trims the element's
settings give. `SC-W0307` says the fill is in parts.

### 6. Underlay

Before its top rows, each part sews its underlay, on by default (`REQ-FILL-TAT-011`, since M5.6). The
underlay is a layer of rows for each angle in `fill_underlay_angle`, in the order given, or with none set
one at `angle + 90°`, square to the top rows. A layer's rows lie evenly `fill_underlay_row_spacing_mm`
apart (default: 3 times the row spacing), with no end spacing. Their needle points lie on the stagger
grid with the fill's `staggers`, `fill_underlay_max_stitch_length_mm` apart (default: the fill's longest
stitch), never at random lengths. `fill_underlay_skip_last` and `underlay_underpath` take the place of
`skip_last` and `underpath`.

Each layer is routed as the top rows are (sections 3 and 4). It starts at the point of the part's rings
nearest the needle and ends there, as Ink/Stitch routes a fill with nothing to end near. The next layer,
and then the top rows, start where it ended, so a part is still one run. A layer with no row is sewn round
the part's outline from its start, as the top rows are (section 8), and `SC-I0308` says so. The needle
goes on to that start as Ink/Stitch's stitch plan joins a layer to the next one: straight on within the
element's jump length, and farther with a jump. The generator makes that join itself, because Ink/Stitch
never forces locks after a layer of underlay, where assembly would after any group of an element with
`force_lock_stitches`.

The underlay is sewn on the part shrunk by `fill_underlay_inset_mm`, and the top rows on the part grown by
`expand_mm`, or shrunk where it is negative (`REQ-FILL-TAT-012`, since M5.7). A part is grown or shrunk
as GEOS buffers a polygon, which Ink/Stitch calls, with round corners of 16 segments to a quarter circle
(`stitchcraft_engine::normalize::offset`). Each ring is offset, the offsets are cut where they cross, and
the faces they wind round clockwise at least once are the area. Before offsetting, GEOS leaves out a ring
that shrinking wears away. After, it leaves out a small ring's offset that has turned inside out. The
engine follows both rules. The polygons agree with shapely's within a nanometre for 330 random polygons,
some at the edges of both rules (`conformance/oracle/buffer.py`).

Shrinking can cut a part into pieces. Each piece is sewn on its own, in order of its point of least x, and
of least y among pieces level with each other: a run of pieces, each with its least x within a nanometre
of the one before. That is GEOS's order in all but a few cases, where GEOS orders
pieces with a sliver among them, or pieces level at their least x, as it walks its graph (`DEV-FILL-008`).
Every layer of the underlay is sewn on one piece before the next, each starting nearest the needle. Each
piece's top rows start nearest the needle too, after the piece before, and end nearest the next part or
element. Ink/Stitch starts every piece's top rows nearest where the needle was before the first
(`DEV-FILL-008`). Ink/Stitch sews each piece's top rows as a group of its own. With
`force_lock_stitches`, so does StitchCraft: assembly ties each off and jumps to the next, however near. Where shrinking wears a part away entirely, the part itself is sewn, as in Ink/Stitch,
and `SC-W0310` (inset) or `SC-W0309` (expand) says so, as Ink/Stitch warns.

### 7. Pull compensation and gap-fill rows

The thread pulls a row in at its ends. Each top row's segment is lengthened at both ends by
`pull_compensation_mm` plus `pull_compensation_percent` of its length (`REQ-FILL-TAT-006`, since M5.8).
The first value applies at the end the rows start from at the fill's angle. A second value, if given,
applies at the other end. Where 2 segments of a row meet end to end, because the row only touches the
outline there from inside or runs along it (section 1), the row goes on in the part. Both segments end at
that point as they are, so the row is sewn once through it, as through Ink/Stitch's rebuilt part, where the
row is one segment. Ink/Stitch moves a satin's stitches with the same function, and the two settings are declared once
for both (`stitchcraft_engine::generators::pull`). A fill reads a value below 0 as 0, as Ink/Stitch does,
and `SC-W0312` says so. The route is planned on the part as drawn. The needle's ways keep their course,
but each starts where the row before it ends and ends where the next row starts, both lengthened.
Underlay is not compensated, as in Ink/Stitch.

Ink/Stitch rebuilds the part round the lengthened rows, each widened by half the row spacing with round
ends, and smooths the outline. Its rows then reach up to half a row spacing further, and it fills holes
smaller than a row spacing squared and holes the widened rows close over. Where the widened rows form
separate pieces, it keeps only the largest. StitchCraft keeps the part's holes, gaps and pieces, which the designer drew
(`DEV-FILL-009`).

Where the route leaves a stretch of more than 3 rows for another, the fabric can open a gap along its
last row before the rows past it are sewn. There, `gap_fill_rows`, rounded up to an even number, sews
that many more rows past the last one, back and forth, and comes back to where it ended
(`REQ-FILL-TAT-007`, since M5.8). A stretch's rows are counted as Ink/Stitch counts them: the rows
reached by travel. The route's first row is one of them, because Ink/Stitch's route always starts with
travel, of no length where it starts on that row. Ink/Stitch's gap fill ends a stretch where the walk
goes from one row to the next by more than one edge once the loops between them are taken out, and so does
this one. An edge that comes back to a node where an earlier one of those edges ended takes out the edges
since, and itself, but a loop back to the row it left stays. So travel out and back before a handover to the
row beside ends a stretch, and so does a pairing edge that stands for a way of several. The extra rows go
on the way the stretch went, away from the last row sewn before the last one in another row. A last row
sewn on through a joint is 2 segments of one row, and the way comes from the row before them, as
Ink/Stitch takes it from the row the needle left for that row. Each extra row is the next row of the
grid, the stretch of its segment beside the extra row before it, or the last row for the first,
lengthened as the rows are. The extra rows are one strip, stop at the part's edge, and stop where the part
ends. The hop from each to the next, and the way back, are kept inside the part as travel is. Ink/Stitch
repeats the last row a spacing further each time, and a repeat can run outside the part (`DEV-FILL-010`).
Underlay has no gap-fill rows.

### 8. No rows

A part thinner than the row spacing has no row. It is sewn as a running stitch round its outline, from
the outline's start, as Ink/Stitch sews it, and `SC-W0305` says so (since M5.4). A layer of underlay with
no row is sewn so too, and `SC-I0308` says so (since M5.6). Where the fill rule leaves every face empty,
the fill sews nothing, and `SC-W0303` says so.

### Properties (conformance)

| Requirement | Property |
|---|---|
| `REQ-FILL-TAT-001` | Every row segment is covered exactly once by top stitches |
| `REQ-FILL-TAT-002` | Measured row spacing equals the parameter (± 2 %), or follows the start→end gradient |
| `REQ-FILL-TAT-003` | Needle points follow the stagger grid: a row's points lie a `staggers`-th of the longest stitch along from the row before's, and no 2 neighbouring rows line them up. A segment ends at its end unless `skip_last` is on or its last point lies within 0.1 mm of it |
| `REQ-FILL-TAT-004` | Coverage (`stitchcraft_testkit::metrics`): rasterized at 0.05 mm with 0.4 mm thread width, ≥ 98 % of the region is covered at spacings from the machine's shortest stitch to 0.4 mm, since M5.9 |
| `REQ-FILL-TAT-005` | With `underpath`, travel takes the cheapest way along the lines under the rows not sewn yet, and its way lies inside the region (± 0.05 mm) |
| `REQ-FILL-TAT-006` | Each top row extends past its segment's ends by the compensation; the part keeps its holes and pieces |
| `REQ-FILL-TAT-007` | Gap-fill rows follow a stretch of more than 3 rows, which ends where Ink/Stitch's gap fill ends one; all rows, gap-fill rows included, and the stitches between them lie inside the part within its compensation |
| `REQ-FILL-TAT-008` | A region in parts sews each part on its own, nearest the needle first, each ending nearest the next part, the last nearest the next element |
| `REQ-FILL-TAT-009` | A 130 × 180 mm rectangle at 0.4 mm, the reference hoop's whole field, plans within the NFR-PERF-1 budget, guarded by `cargo xtask perf` since M5.9 |
| `REQ-FILL-TAT-011` | Before its top rows a part sews a layer of underlay for every angle, which starts and ends at its point nearest the needle, where the top rows then start |
| `REQ-FILL-TAT-012` | The underlay covers the part shrunk by its inset and the top rows the part grown by `expand_mm`, as shapely buffers it, within a nanometre |
| `REQ-FILL-TAT-013` | Questions about the rings look only at their sides near each point, line and stitch, and answer as a look at every side would: a 70 mm square with 14 × 14 round holes and a 120 × 60 mm background with 60 detailed holes plan within a tenth of the default work budget |

Machine checkpoint MC-4 sews a density ladder, an angle set and a fill-with-outline registration test
to tune pull compensation ([machine testing](../../plan/machine-testing.md)).

## Contour fill

Rows that follow the region's outline inward, like growth rings (P2). Strategies (`contour_strategy`):

- **Inner to outer:** inset the boundary repeatedly by `row_spacing_mm` (polygon offsetting with
  `join_style` round/mitred/bevelled) until empty, forming a tree of rings (a ring can split into several
  as the shape narrows). Sew each subtree from the innermost ring outward, connecting rings at their
  nearest points.
- **Single spiral / double spiral:** connect the rings of each branch into one continuous spiral (single:
  outside to centre; double: in and back out), following the Connected Fermat Spirals construction (Zhao
  et al., SIGGRAPH 2016). Before spiralling, **necks** (where the inset ring tree branches) split the region
  into parts that each spiral cleanly; a part that still cannot be connected falls back to inner-to-outer
  with `SC-W0311`.
- `clockwise` and `avoid_self_crossing` control direction and how ring connections are placed;
  `smoothness_mm` simplifies rings before stitching.

Properties: rings are spaced by the parameter (± 5 %); consecutive rings never cross; the spiral is one
continuous path per part.

## Meander fill

A space-filling pattern (P2): a tile is repeated over the region, scaled (`meander_scale_percent`) and
rotated (`meander_angle`), clipped to the region (`clip`), and turned into one continuous path by
connecting the clipped pieces and walking the result with Hierholzer's algorithm after making the graph
Eulerian. Tiles are StitchCraft's own, **generated in code** (Hilbert, Peano, Truchet-style arcs,
waves, pebbles…), never copied from other projects; `meander_pattern` accepts our tile ids and maps
Ink/Stitch tile names to the closest of ours as a recorded deviation. The path is sewn as running stitch
(optionally zigzag with `zigzag_spacing_mm`/`zigzag_width_mm`, bean stitch, repeats).

## Circular fill

Concentric circles (or one spiral) around the target point (a command; default: the region's centroid),
spaced by `row_spacing_mm` (optionally graded by `end_row_spacing_mm`), clipped to the region and routed
like tatami rows (P2).

## Guided fill (P3)

Rows that follow a guide line instead of a straight angle: the guide is copied (`Copy`) or offset
(`Parallel Offset`, `Buffer`) across the region at `row_spacing_mm`, each copy clipped to the region and
sewn with the tatami needle grid (`staggers`, `stitch_position_method`). Specified fully at M10.

## Linear gradient fill (P3)

A tatami variant whose rows alternate between the colours of a linear gradient with a probability that
follows the gradient position, producing a blend from interleaved threads; one colour block per stop.
Specified at M10.

## Tartan fill (P3)

A woven plaid from a stripe specification (sett): warp and weft stripes become fills at `tartan_angle`
with `rows_per_thread` and optional herringbone. Specified at M10.

## Cross stitch (P3)

The region is covered by grid cells (`pattern_size_mm`, `cross_offset_mm`, `cross_rotation`,
`canvas_grid_origin`); a cell becomes a cross when the region covers at least `fill_coverage` percent of
it; crosses use `cross_stitch_method` (simple, half, upright, double, Smyrna, flipped variants) and are
routed row by row. Specified at M10.

## References

- H. Zhao et al., "Connected Fermat Spirals for Layered Fabrication," *ACM Transactions on Graphics*
  35(4), SIGGRAPH 2016.
- C. Hierholzer, "Über die Möglichkeit, einen Linienzug ohne Wiederholung und ohne Unterbrechung zu
  umfahren," *Mathematische Annalen* 6, 1873.
- D. Hilbert, "Über die stetige Abbildung einer Linie auf ein Flächenstück," *Mathematische Annalen* 38, 1891.
