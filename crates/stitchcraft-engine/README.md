# stitchcraft-engine

Layer **L2**. Turns a `Design` (host-independent elements with shapes, parameters and threads) into a
checked `StitchPlan`: digitize automatic elements → normalize → validate → generate per element →
assemble → finalize → check.

**Status:** the input model, `design::Design`, since M3.3 (`docs/src/design/data-model.md`); stroke
normalization (`normalize::stroke`) and the running stitch (`generators::running`) since M3.4, with its
repeats, bean stitch (`generators::passes`) and random length since M3.5; manual stitch
(`generators::manual`) since M3.6. Lock stitches (`locks`) of each shape, at either end of a group, are
here since M3.7. Since M3.8 the entry point, `plan`, sends each element to its generator (`generate`) and
joins the groups into one plan (`assemble`). Since M3.9 it fits the plan to the machine and checks it
(`finalize`), and a plan that `plan` returns can be written as it is. Since M4.1 a satin column's path
is read as its rails and rungs (`normalize::satin`), as Ink/Stitch reads it. Since M4.2 its top stitches
are placed as Ink/Stitch places them (`generators::satin`), since M4.3 compensated and varied at random
as Ink/Stitch does, since M4.4 inset where they crowd on curves, and since M4.5 split where they are
long. Since M4.6 its underlays come first, as Ink/Stitch sews them, and since M4.7 it starts and ends
at its nearest points. Since M4.8 a column drawn as one path and too narrow to stitch across is sewn as a
stroke, and since M4.9 a wider one is sewn between rails made from its centre line
(`normalize::centre_line`), offset as shapely offsets them (`normalize::offset`). Since M5.1 a fill's
region is built by its fill rule (`normalize::region`), and what it leaves out is said, and since M5.2
its tatami rows are laid as Ink/Stitch lays them (`generators::tatami`). Since M5.4 a tatami fill is
sewn part by part, its rows in the order Ink/Stitch routes them (`generators::tatami::route`), since
M5.5 its needle runs between rows under the rows sewn after it (`generators::tatami::underpath`),
since M5.6 each part sews its underlay first, a layer for each angle (`generators::tatami::sew`),
since M5.7 the underlay covers the part shrunk by its inset and the top rows the part grown by `expand_mm`,
as shapely buffers it (`normalize::offset`), and since M5.8 the top rows are lengthened by the pull
compensation a satin's stitches share (`generators::pull`), with gap-filling rows where the route leaves a
stretch of rows (`generators::tatami`). Since M7.2 a zigzag stroke is sewn as Ink/Stitch sews it
(`generators::zigzag`), and since M7.3 an E-stitch, a satin column's spikes from a spine along its first
rail. Every satin method then names stitches that lean far along the column, and top stitches longer than
12 mm (`generators::satin`). Since M7.4 an S-stitch sews each pair once across, every other one back the
other way, and since M7.5 a zigzag satin sews a zigzag between the rails, its pairs at half the spacing.
Since M13.1 a shape without settings, read with `--auto`, is digitized automatically before anything else
(`auto`). A line no wider than the design's narrowest satin stroke runs, a line up to 7 mm wide becomes
satin columns along its centre line, and a wider line becomes the tatami fill of the area it paints
(`auto::area`). Digitizing doesn't add an error of its own. Since M13.2 the automatic areas are then sewn
as neighbours (`auto::neighbours`). The picture's background is left out and specks merge into the areas
beside them. Each area reaches 0.4 mm under the shapes sewn after it from where it touches them, or is cut
away under the areas that cover it. Regions are combined, grown and shrunk with `normalize::region::ops`.
Since M13.3 the elements sew in an order of their own (`auto::neighbours::order`), chosen between the
specks and the overlaps. Shapes that overlap keep their paint order, and a line sews after the fill it
borders. Each thread then sews as long as it can, the nearest element next. Elements with settings keep
their order unless `--sort-colours` is given (`Design::sort_colours`), and an element with a stop after
it keeps its place. Since M13.4 a part of an automatic area narrower than the narrowest satin stroke
everywhere runs along its centre line (`auto::narrow`), which a constrained Delaunay triangulation of the
part (`normalize::triangulation`) gives through the middles of its chords (`normalize::strips`). A running
stitch that automatic digitizing makes is tripled from 0.5 mm wide. Since M13.5 a part no wider than a satin
column sews as satin columns along its centre lines, between its own outlines (`auto::columns`,
`normalize::strips::columns`), drawn with rails and rungs (`generators::satin::drawn::from_rails`), and
every automatic satin column has the underlay of its width and a pull compensation of 0.4 mm. Since M13.6
a part wider than a satin column in places is split (`auto::split`). An opening with a disc as wide as a
satin column leaves its broad parts, which are filled. Its narrow parts, cut square where they meet them,
sew over the fill as lines and columns. The test pictures that the TS-13 sheets sew, digitized beside
the same shapes in paint order, are drawn here too (`testsheets::pictures`), so that the cases measure
what the machine sews.
Since M6.12 a column drawn as one path can be drawn with its rails and rungs instead, for a host's Stroke
to Satin tool (`generators::satin::drawn`). They sew as the column does. A design can be moved
(`Design::moved`), as a host draws a test sheet (`testsheets`, `TestSheet::design`).
Design:
`docs/src/design/engine-pipeline.md` and `docs/src/design/algorithms/`.

## Invariants

- Knows no host, file format or renderer; never does I/O.
- Generators are pure functions of (shape, typed params, hints, seed, budget); output is deterministic.
- Never panics; every loop over data charges the budget; every fallback emits a coded diagnostic.
- Elements are generated independently from geometry-based hints, so results can be cached and planned in
  parallel without changing the output.

## Dependencies

`stitchcraft-core`, `stitchcraft-params`, `stitchcraft-plan`; `thiserror`. Tests also use `proptest` and
`stitchcraft-testkit`. Curves are flattened here, not with `kurbo`, so every platform gets the same
points (`docs/src/design/engine-pipeline.md` › Normalize), and a fill's region is built here, not with
`i_overlay`, so it keeps the drawing's points and charges the budget (ADR 0005).

The `field` example uses `stitchcraft-testkit` too. It plans the reference hoop's whole field for
`cargo xtask perf` (NFR-PERF-1).
