# Fill generators

Fills cover a `Region` (valid polygons with holes, from the normalizer). Tatami is P1 (M5); contour,
meander and circular are P2 (M7); guided, linear gradient, tartan and cross stitch are P3 (M10).

## Tatami fill

Parallel rows of running stitches across the region, with the needle points of neighbouring rows offset
so they never line up into visible furrows. Most fills in most designs are tatami.

**Parameters:** `angle`, `row_spacing_mm`, `end_row_spacing_mm`, `max_stitch_length_mm`, `staggers`,
`skip_last`, `underpath`, `running_stitch_length_mm`, `running_stitch_tolerance_mm`, `gap_fill_rows`,
`pull_compensation_mm`, `pull_compensation_percent`, `expand_mm`, `random_seed`,
`enable_random_stitch_length`, `random_stitch_length_jitter_percent`; underlay: `fill_underlay`,
`fill_underlay_angle`, `fill_underlay_row_spacing_mm`, `fill_underlay_max_stitch_length_mm`,
`fill_underlay_inset_mm`, `fill_underlay_skip_last`, `underlay_underpath`.

### 1. Rows

1. Rotate the region by `−angle` so rows are horizontal (the rotation and its inverse are exact affine
   maps; all later geometry happens in this frame).
2. Apply `expand_mm` as an offset of the region (a deliberate shape change the user asked for).
3. Cast scan lines at `y = y₀ + k·spacing`. With `end_row_spacing_mm` the spacing varies linearly from
   the first to the last row (a density gradient). `y₀` is anchored to a global grid (multiples of the
   spacing from the design origin) so adjacent regions with the same settings share rows.
4. Intersect each scan line with every edge using the half-open rule (an edge spans `[y_min, y_max)`),
   so a vertex exactly on a scan line is counted once. Sort the crossings; consecutive pairs are the
   row *segments* inside the region.
5. **Pull compensation extends each segment at both ends** by `pull_compensation_mm +
   pull_compensation_percent × segment length` (one value or two values for start/end side), clipped so
   it never reaches into a neighbouring segment of the same row. Holes, gaps and separate components
   keep their topology — the shape is never buffered as a whole, which would close gaps the designer
   left on purpose (`REQ-FILL-TAT-006`).

### 2. Needle points along a row

Needle points fall on a global grid along the row direction: positions where
`(x + offset_k) mod max_stitch_length = 0`, with `offset_k = (k mod staggers) / staggers ×
max_stitch_length` for row k. The segment's two ends are always penetrations; a grid point closer than
the minimum stitch length to an end is dropped. Optional random length jitter perturbs grid points by
the seeded jitter without changing the ends. `skip_last` omits the final penetration of each row (a
softer edge where rows turn).

### 3. Routing: covering every segment with little travel

Treat the row segments as edges that must be sewn exactly once, and the region's boundary and interior
as places where the needle may travel. This is a rural postman problem; we use a standard coverage
decomposition instead of a general solver:

1. **Boustrophedon cells.** Sweep the rows in order and group segments into *cells*: maximal stacks of
   consecutive rows with exactly one segment each that overlap their neighbours (Choset & Pignon's
   boustrophedon cellular decomposition). A cell is sewn back and forth with no travel at all.
2. **Cell graph.** Cells that touch (share a split or merge event) are adjacent. Order cells with a
   depth-first traversal of this graph starting from the cell nearest the entry hint, preferring the
   neighbour whose start is nearest the current exit; ties break by cell index (determinism).
3. **Travel between cells** follows the shortest path *inside* the region on a visibility graph of the
   region's vertices (A* with Euclidean heuristic). With `underpath` the travel may cross the interior
   (it is covered by later rows); without it, travel follows the boundary. Travel is sewn as running
   stitch with `running_stitch_length_mm`.
4. **Disconnected parts.** If no inside path exists (the region has separate components), the parts are
   planned as separate sub-groups joined by tie-off → jump/trim → tie-in, with `SC-W0307` — never a
   straight stitch across empty fabric (`SC-W0501`), and never a silent change to what is sewn.
5. **Exit.** The last cell is chosen, when possible, to end near the exit hint.

### 4. Gap-fill rows

`gap_fill_rows` adds extra rows along the joins between cells (where fabric distortion opens gaps).
They come from the same scan-line machinery and are clipped to the region (`REQ-FILL-TAT-007`), never
copies of a previous row shifted along the stitch angle.

### 5. Underlay

The same algorithm on the region inset by `fill_underlay_inset_mm`, at `fill_underlay_angle` (default:
`angle + 90°`), with `fill_underlay_row_spacing_mm` (default: three times the top spacing) and its own
maximum stitch length, sewn first and routed to end near the start of the top layer.

### 6. Tiny regions

If no scan line meets the region (it is thinner than the row spacing), the generator outlines it with a
running stitch and emits `SC-W0305` — the user decides whether that is acceptable.

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
| `REQ-FILL-TAT-008` | Disconnected regions yield trims and `SC-W0307`, never long straight stitches |
| `REQ-FILL-TAT-009` | A 150 × 150 mm square at 0.4 mm plans within the NFR-PERF-1 budget |

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

- H. Choset and P. Pignon, "Coverage Path Planning: The Boustrophedon Cellular Decomposition," *Field and
  Service Robotics*, Springer, 1998.
- H. Zhao et al., "Connected Fermat Spirals for Layered Fabrication," *ACM Transactions on Graphics*
  35(4), SIGGRAPH 2016.
- H. A. Eiselt, M. Gendreau, G. Laporte, "Arc Routing Problems, Part II: The Rural Postman Problem,"
  *Operations Research* 43(3), 1995.
- C. Hierholzer, "Über die Möglichkeit, einen Linienzug ohne Wiederholung und ohne Unterbrechung zu
  umfahren," *Mathematische Annalen* 6, 1873.
- D. Hilbert, "Über die stetige Abbildung einer Linie auf ein Flächenstück," *Mathematische Annalen* 38, 1891.
