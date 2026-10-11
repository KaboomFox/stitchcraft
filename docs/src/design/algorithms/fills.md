# Fill generators

<!-- implements: crates/stitchcraft-engine/src/normalize/region/**, crates/stitchcraft-engine/src/generators/tatami/** -->

A fill covers the area its path bounds, its [region](#region), with rows of stitches. Tatami is P1 (M5);
contour, meander and circular are P2 (M7); guided, linear gradient, tartan and cross stitch are P3 (M10).

A fill sews the drawing's shape with Ink/Stitch's stitches. Ink/Stitch's rows, their spacing and stagger,
the order it sews them in, its travel, underlay and compensation are what make its fills look good
embroidered. StitchCraft keeps them, with their parameters' names and defaults, and a file sews alike in
both. Where the area Ink/Stitch fills departs from the drawing, StitchCraft follows the drawing, and a
diagnostic says where the two differ. A part Ink/Stitch leaves out without a word is left out with a
warning.

## Region

A fill's region is the area an SVG renderer paints for its path, in parts with holes
(`stitchcraft_engine::normalize::region`, `REQ-FILL-001`):

1. **Rings.** Each subpath is flattened within a tenth of a CSS pixel, as Ink/Stitch flattens a fill's
   outline, and closed. A subpath of fewer than 3 distinct points bounds nothing and is left out
   (`DEV-FILL-002`).
2. **Cuts.** Wherever 2 segments cross, touch or run along each other, both are cut there. Which side of
   a line a point lies on is decided exactly. A point on a segment is then found on it, and segments that
   only touch are never taken to cross. Where 2 segments cross inside both, the crossing is worked out in
   floating point, and a crossing within 10⁻⁹ mm of a point already found is that point. A stretch drawn
   more than once becomes one edge that counts how many times it was drawn each way.
3. **Faces.** The edges bound faces. Outside everything the winding number is 0, and crossing an edge
   from its right to its left adds its count. Every face's winding number follows from its neighbours'
   in whole numbers.
4. **Fill rule.** A face is filled when its winding number is not 0 (`nonzero`, SVG's default) or is odd
   (`evenodd`). Ink/Stitch fills every fill as if its rule were even-odd. Where `nonzero` fills a face
   that even-odd leaves empty, StitchCraft fills it and `SC-I0306` says so (`DEV-FILL-001`).
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

The work is charged to the element's budget, one unit for each point, segment, cut, half edge, pair of
segments whose spans along x overlap, and ring compared with a face. Property tests draw random rings on
a grid and count how many times they wind round points off the edges. Such a point lies in the region
exactly when the fill rule fills it, unless it lies in a part left out. Every ring found is simple and
turns the right way.

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
it `row_spacing_mm` apart until they pass the part's far side (`REQ-FILL-TAT-002`). Fills side by side
at the same angle and spacing then share their rows. Angles of whole right angles are exact, so rows at
0° or 90° run exactly along the drawing's axes.

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
fraction of a stagger draws diagonals that show less. Fills side by side at the same angle share the
grid, as they share their rows.

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
   nearest the next element. Each of the 2 points gets a node, which splits the stretch of that ring whose
   chord passes nearest it. Then the node nearest the needle is where the fill starts, and the one
   nearest the next element where it ends. Without a needle the fill starts at its first segment's start,
   and without a next element it ends where it starts. Only rings that rows reach are looked at.
   Ink/Stitch joins an unreached ring with straight lines, which it sews as rows (`DEV-FILL-006`).
4. **Even degrees.** A row through a corner of the outline, or one along it, shares a node with the row
   beside it, and some nodes then meet an odd number of edges. Ink/Stitch adds paths between such nodes
   that may run along rows, and sews those rows a second time. StitchCraft pairs the odd nodes so that the
   needle's way between pairs is shortest in all and joins each pair with an edge. Up to 16 nodes every
   pairing is weighed, and past that each node pairs with the nearest left. No row is sewn twice
   (`DEV-FILL-006`).
5. **Walk.** The walk starts at the end node. At each node it takes the node's row if one is left,
   and otherwise the node's first edge left. A node lists its edges neighbour by neighbour, each neighbour
   where its first edge to the node came and its edges in the order they came. A neighbour whose last edge
   is taken drops out of the list. When the walk reaches a node with no edge left it steps back, and the
   edges, in the order it steps back over them, are the route. The route comes back to the end node, and
   before it the needle runs from the fill's start to that node. The running stitches between rows then
   mostly come just before the rows beside them, which cover them, and the rows go back and forth like a
   mown lawn.
6. **Between rows.** Steps between rows in a row are one stretch, from where the last row ended to where
   the next starts. Between 2 points of one ring the needle runs along the ring, the shorter way round, as
   in Ink/Stitch. Between 2 rings it takes the shortest way along rings and rows (`DEV-FILL-006`). It sews
   running stitches of the first `running_stitch_length_mm`, at least twice the shortest stitch, within
   `running_stitch_tolerance_mm`. The first of them would sew the row's end again, so it is left out. With
   `skip_last` the row's end is not sewn, and the first running stitch stays unless the next row starts
   beside it.
7. **The shortest stitch.** Rows the row spacing apart start and end nearer each other than the shortest
   stitch a machine sews well, and a row's first grid point can lie just past its start. A needle point
   nearer the one before it than the shortest stitch is left out, as Ink/Stitch leaves out points nearer
   than its own shortest stitch. A part's last point stays, and the points before it that are too near it
   go instead, finalize's rule applied in the generator.

The route follows Ink/Stitch's from the same graph. StitchCraft's rings start where the drawing starts
them, and Ink/Stitch's where GEOS's overlay starts them, so which stretches are doubled, and with them the
route, can differ (`DEV-FILL-005`). Until travel under the rows arrives (M5.5), the needle runs along the
outline between rows, as Ink/Stitch's does with `underpath` off.

### 4. Travel under the rows

With `underpath`, on by default, travel may cross the region where rows sewn later will cover it. It
follows lines square to the rows and lines at 45° to them either way. A line costs less the farther it
lies from the outline. Once a row is sewn, the lines crossing it are closed: no travel runs over a row
already sewn. Travel stays inside the region (`REQ-FILL-TAT-005`). Where no way inside joins 2 points,
the parts are joined as below, and `SC-W0501` says so.

### 5. Parts

A region in parts sews them one at a time, as Ink/Stitch sews them (`REQ-FILL-TAT-008`, since M5.4).
The part nearest the needle goes first, a part the needle is in being 0 away. With no needle, the part
with the least x goes first. Each part ends at its point nearest the next part. The last ends at the point
of the fill nearest the next element's first stitch, or nearest its shape when it starts nearest the
needle. Each part is a group of its own, which assembly joins to the next as it joins any 2 groups:
straight on within the collapse length, and otherwise with a jump and the ties and trims the element's
settings give. `SC-W0307` says the fill is in parts.

### 6. Underlay

Before its top rows, each part sews its underlay, on by default. The underlay is sewn like the top
rows, on the part shrunk by `fill_underlay_inset_mm`. It makes one pass for each angle in
`fill_underlay_angle` (default: `angle + 90°`), with `fill_underlay_row_spacing_mm` (default: three
times the top spacing) and `fill_underlay_max_stitch_length_mm` (default: the top's).
`fill_underlay_skip_last` and `underlay_underpath` take the place of `skip_last` and `underpath`. Each pass
starts where the one before ended. The top rows cover the part grown by `expand_mm`.

### 7. Pull compensation and gap-fill rows

The thread pulls a row in at its ends. Each segment is lengthened at both ends by `pull_compensation_mm`
plus `pull_compensation_percent` of its length (one value, or one for each end). Ink/Stitch then rebuilds
the region round the lengthened rows, which can close a hole or a gap narrower than the compensation.
StitchCraft keeps the region's holes, gaps and parts, which the designer may have left on purpose
(`REQ-FILL-TAT-006`).

Where the needle leaves a stretch of more than 3 rows for another, the fabric can open a gap along its
last row. There, `gap_fill_rows` (rounded up to an even number) sews that many more rows, back and forth.
Ink/Stitch repeats the last row, a row further on each time, and a repeat can run outside the region.
StitchCraft takes the extra rows from the row grid and keeps them inside the region
(`REQ-FILL-TAT-007`).

### 8. No rows

A part thinner than the row spacing has no row. It is sewn as a running stitch round its outline, from
the outline's start, as Ink/Stitch sews it, and `SC-W0305` says so (since M5.4). Where the fill rule
leaves every face empty, the fill sews nothing, and `SC-W0303` says so.

### Properties (conformance)

| Requirement | Property |
|---|---|
| `REQ-FILL-TAT-001` | Every row segment is covered exactly once by top stitches |
| `REQ-FILL-TAT-002` | Measured row spacing equals the parameter (± 2 %), or follows the start→end gradient |
| `REQ-FILL-TAT-003` | Needle points follow the stagger grid: a row's points lie a `staggers`-th of the longest stitch along from the row before's, and no 2 neighbouring rows line them up. A segment ends at its end unless `skip_last` is on or its last point lies within 0.1 mm of it |
| `REQ-FILL-TAT-004` | Coverage: rasterized at 0.05 mm with 0.4 mm thread width, ≥ 98 % of the region is covered at spacing ≤ 0.4 mm |
| `REQ-FILL-TAT-005` | Travel stitches lie inside the region (± 0.05 mm) |
| `REQ-FILL-TAT-006` | Pull compensation preserves the number of holes and components |
| `REQ-FILL-TAT-007` | All rows, including gap-fill rows, lie inside the region (± tolerance + compensation) |
| `REQ-FILL-TAT-008` | A region in parts sews each part on its own, nearest the needle first, each ending nearest the next part, the last nearest the next element |
| `REQ-FILL-TAT-009` | A 130 × 180 mm rectangle at 0.4 mm, the reference hoop's whole field, plans within the NFR-PERF-1 budget |

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
