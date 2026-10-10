# Fill generators

<!-- implements: crates/stitchcraft-engine/src/normalize/region/** -->

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

Rows run across the region at `angle`, `row_spacing_mm` apart, on lines a whole number of spacings from
the design's origin. Fills side by side at the same angle and spacing then share their rows. With
`end_row_spacing_mm` the spacing changes steadily from the first row to the last. Where a row crosses
the region more than once, each stretch inside is a *segment*. A row that only touches the region at a
point has none.

### 2. Needle points along a row

A row's needle points lie on a grid along it, `max_stitch_length_mm` apart and anchored at the origin.
Each next row's grid is shifted by `1/staggers` of a stitch, and fills side by side tile. A segment
starts with a needle point at its start, then takes the grid's points. Its end gets one too, unless
`skip_last` is on or the last grid point is within 0.1 mm of it. With `enable_random_stitch_length` the
points are spaced by the seeded random lengths instead.

### 3. Routing: every segment once

The segments' ends lie on the region's outline. Joined by the stretches of outline between neighbouring
ends, every other stretch twice and more where needed, they make a graph in which every point meets an
even number of edges. One path can then take every edge (Hierholzer). The fill starts at the point
nearest the needle and goes first to where it will end. That is the point of its outline nearest the
next element's first stitch, or its start when nothing follows. From there it sews its segments in one
loop that comes back there, taking a segment wherever one is left. The rows go back and forth like a mown
lawn, and the rows sewn later hide the way the needle came. Between segments the needle runs along the
outline in running stitches (`running_stitch_length_mm`, `running_stitch_tolerance_mm`).

### 4. Travel under the rows

With `underpath`, on by default, travel may cross the region where rows sewn later will cover it. It
follows lines square to the rows and lines at 45° to them either way. A line costs less the farther it
lies from the outline. Once a row is sewn, the lines crossing it are closed: no travel runs over a row
already sewn. Travel stays inside the region (`REQ-FILL-TAT-005`). Where no way inside joins 2 points,
the parts are joined as below, and `SC-W0501` says so.

### 5. Parts

A region in parts sews them one at a time, in order of their distance from the needle. Each part ends
at its point nearest the next part, and the last nearest the next element. Jumps join the parts, with
ties and trims where the element's settings say, never stitches across empty fabric (`SC-W0307`,
`REQ-FILL-TAT-008`).

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

If no row meets the region (it is thinner than the row spacing), it is sewn as a running stitch round
its outline, as Ink/Stitch sews it, and `SC-W0305` says so.

### Properties (conformance)

| Requirement | Property |
|---|---|
| `REQ-FILL-TAT-001` | Every row segment is covered exactly once by top stitches |
| `REQ-FILL-TAT-002` | Measured row spacing equals the parameter (± 2 %), or follows the start→end gradient |
| `REQ-FILL-TAT-003` | Needle points follow the stagger grid; no furrow of aligned points longer than `staggers` rows |
| `REQ-FILL-TAT-004` | Coverage: rasterized at 0.05 mm with 0.4 mm thread width, ≥ 98 % of the region is covered at spacing ≤ 0.4 mm |
| `REQ-FILL-TAT-005` | Travel stitches lie inside the region (± 0.05 mm) |
| `REQ-FILL-TAT-006` | Pull compensation preserves the number of holes and components |
| `REQ-FILL-TAT-007` | All rows, including gap-fill rows, lie inside the region (± tolerance + compensation) |
| `REQ-FILL-TAT-008` | A region in parts sews each part on its own, joined by jumps and `SC-W0307`, never long straight stitches |
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
