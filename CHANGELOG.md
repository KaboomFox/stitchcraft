# Changelog

All notable changes are listed here, newest first. Golden-file changes must be listed with their reason.

## Unreleased

### Added
- MC-4 kit: TS-07, TS-08 and TS-09, the fill test sheets, drawn as designs and sewn by the engine.
  - **TS-07** fills 3 circles 60 mm across at pull compensations of 0, 0.2 and 0.4 mm, then sews a
    running stitch along each edge, for the gap or overlap between fill and outline.
  - **TS-08** fills squares 30 mm wide at row spacings of 0.25 to 0.6 mm and angles of 0°, 45° and 90°,
    for the spacing that covers the fabric without stiffening it.
  - **TS-09** fills a rectangle with 2 holes and a bar apart, without underlay, with it, and with
    gap-filling rows, for the travel, the holes and the jump to the part apart.
- M5.9: the coverage metric (`stitchcraft_testkit::metrics`) and the performance budget.
  - Coverage rasterizes a fill's region and the thread of every stitch its plan sews, 0.4 mm wide, at
    0.05 mm. A fill covers at least 98 % of its area at spacings from the machine's shortest stitch to
    0.4 mm (`REQ-FILL-TAT-004`), for rectangles, an L, circles, rings, stars and a 5 mm bar at 5 angles.
    Below the shortest stitch, rows that meet an edge square on leave out their first needle point there
    (`DEV-FIN-001`), and a 5 mm bar at 0.25 mm on the Brother is 97 % covered. Along an edge at 45°, the
    rows' ends lie 0.35 mm apart and keep their handovers.
  - `cargo xtask perf` plans the reference hoop's whole field (`REQ-FILL-TAT-009`) with the engine's
    `field` example in release. It counts the instructions one plan takes with valgrind's callgrind,
    against a record in `conformance/perf.toml` that only goes down, and reports the time against
    NFR-PERF-1's 50 ms: 282 million instructions and 29 ms at M5.9. It has its own CI job.
  - Planning the field takes a fifth fewer instructions than before: crossings of the travel lines snap
    with 3 lookups instead of 9, and the exact orientation test knows a cross product is 0 when both its
    products are. The stitches stay as they were.
- M5.8: a tatami fill's top rows are lengthened past their ends by `pull_compensation_mm` plus
  `pull_compensation_percent` of each row's length (`REQ-FILL-TAT-006`), as Ink/Stitch lengthens them.
  `gap_fill_rows` sews rows again where the route leaves a stretch of more than 3 rows
  (`REQ-FILL-TAT-007`), as Ink/Stitch does.
  - The first value applies at the end the rows start from, the second at the other. A fill reads a value
    below 0 as 0, as Ink/Stitch does, and `SC-W0312` says so. The part keeps its holes and its pieces,
    where Ink/Stitch rebuilds it round the lengthened rows (`DEV-FILL-009`). Underlay is not compensated.
  - Each gap-fill row is the grid's next row past the stretch's last, inside the part, where Ink/Stitch
    repeats the last row, which can run outside it (`DEV-FILL-010`).
  - The 2 pull compensation settings are declared once for satin columns and fills, with a reference page
    of their own. A satin reads them as before.
  - The golden files stay as they were: every fixture keeps both settings at 0.
- M5.7: a tatami fill's underlay covers each part shrunk by `fill_underlay_inset_mm` (`REQ-FILL-TAT-012`),
  as Ink/Stitch sews it. Its top rows cover each part grown by `expand_mm`, or shrunk where that is negative.
  - A part is grown or shrunk as GEOS buffers a polygon, with round corners of 16 segments to a quarter
    circle. Each ring is offset and the offsets are cut where they cross. The area they wind round is the
    part grown or shrunk. GEOS's rules for a ring that shrinking wears away, and for a small ring's offset
    turned inside out, are followed. The polygons agree with shapely's within a nanometre for 330 random
    polygons, some at the edges of those rules (`conformance/oracle/buffer.py`).
  - Shrinking can cut a part into pieces, sewn in order of least x, GEOS's order in all but a few cases.
    Each piece gets every layer of the underlay before the next, and each piece's top rows start
    where the last ended, where Ink/Stitch starts them all near the first's start (`DEV-FILL-008`). Where
    shrinking wears a part away entirely, the part itself is sewn, and `SC-W0309` (expand) or `SC-W0310`
    (inset) says so, as Ink/Stitch warns.
  - With `underpath`, a fill starts at the node of the lines under the rows nearest the needle, as
    Ink/Stitch's does. For a needle inside a part, as after an inset underlay, that node lies inside, and
    travel stitches run from it to the route's start under the rows.
- M5.6: a tatami fill sews its underlay before its top rows (`fill_underlay`, on by default,
  `REQ-FILL-TAT-011`), as Ink/Stitch sews it.
  - Each part gets a layer of rows for each angle of `fill_underlay_angle`, in the order given, or with
    none set one square to the top rows. The rows lie 3 times the row spacing apart unless
    `fill_underlay_row_spacing_mm` says otherwise, in stitches of the fill's longest stitch unless
    `fill_underlay_max_stitch_length_mm` does, with `fill_underlay_skip_last` and `underlay_underpath` in
    place of `skip_last` and `underpath`.
  - Each layer is routed as the top rows are. It starts at the point of the part's rings nearest the
    needle and ends there, and the next layer and then the top rows start there too. A layer too thin for a row is
    sewn round the part's outline, and `SC-I0308` says so. The needle goes on to it as Ink/Stitch's stitch
    plan does: straight on within the jump length, without the locks `force_lock_stitches` adds after
    other groups.
  - A list of angles is a kind of parameter (`OptionalAngleList`), each angle normalized as an angle is.
  - `fill_underlay_inset_mm` and `expand_mm` need a polygon buffered as GEOS buffers it, which comes in a
    step of its own, M5.7. Until then they are not registered (`SC-W0105`). Pull compensation and the
    coverage and performance step move to M5.8 and M5.9.
- M5.5: between rows a tatami fill's needle travels under the rows sewn after it (`underpath`, on by
  default, `REQ-FILL-TAT-005`), as Ink/Stitch's does.
  - Travel follows 3 gratings of lines across the part, at 45° to the rows either way and square to them,
    the cheapest way. A line costs less the farther it lies from the part's rings, a stretch of ring 3
    times its length, and a line closes once a row sewn crosses it. The way is smoothed and kept inside the
    part. With `underpath` off the needle runs along the outline, as in M5.4.
  - Where no line leads, as to a hole small enough to lie between the lines, the needle runs along the
    rings and rows, where Ink/Stitch goes straight. Ties between equally cheap ways, the rings the costs
    measure from and the points a way is smoothed through follow StitchCraft's own rules
    (`DEV-FILL-007`).
- M5.4: tatami fills are sewn (`REQ-FILL-TAT-001`, `REQ-FILL-TAT-008`).
  - Each part's row segments are sewn once each, back and forth, in the order Ink/Stitch routes them. One
    walk goes through the graph of the segments and the stretches of ring between their ends, with every
    other stretch doubled, and takes a row wherever one is left. The fill starts at the point of its rings
    nearest the needle and ends at the point nearest the next element. Between rows the needle runs along
    the outline, the shorter way round, in running stitches of the running stitch's length and tolerance.
  - Nodes of odd degree are paired by the shortest way between them, and no row is sewn twice, where
    Ink/Stitch can sew one again (`DEV-FILL-006`). The route can differ from Ink/Stitch's where the 2
    start a ring at different points (`DEV-FILL-005`).
  - A fill in parts sews the part nearest the needle first, and each part ends nearest the next. A part
    too thin for a row is sewn round its outline (`SC-W0305`), and a fill that bounds no area says so
    (`SC-W0303`). A fill leaves out needle points nearer the one before than the shortest stitch by
    finalize's rule, which is now one function that both apply.
  - A tatami fill offers its rings to the element before it, which ends nearest them. `fill_method` is
    registered, and the other methods are skipped with `SC-W0011`. The running stitch's length and
    tolerance apply to tatami fills, whose tolerance is 0.2 mm when unset, as Ink/Stitch sews it
    (`DEV-SAT-006`).
  - Travel under the rows (`underpath`), underlay and compensation come in the next M5 steps. The roadmap's
    step for parts is done here, and the later M5 steps move up by one.
- M5.3: a tatami fill's needle points are placed along its rows as Ink/Stitch places them
  (`REQ-FILL-TAT-003`). Fills are still skipped (`SC-W0011`) until M5.4 routes their rows.
  - The points lie on a grid the longest stitch apart, anchored at the design's origin, each row's a
    `staggers`-th of a stitch along from the row before's, so neighbouring rows never line them up. A
    segment takes its start, the grid's points between, and its end, unless `skip_last` is on or the last
    point lies within 0.1 mm of it. With random stitch lengths the points start a random share of a stitch
    in and vary by the jitter, drawn from StitchCraft's generator (`DEV-FILL-004`).
  - `staggers` and `skip_last` are registered for tatami fills, and `max_stitch_length_mm`,
    `enable_random_stitch_length`, `random_stitch_length_jitter_percent` and `random_seed` now apply to
    them too.
  - A parameter can have a default for each family of stitch types, as Ink/Stitch gives some keys
    another default on another kind of element. `max_stitch_length_mm` is 4 mm for fills and empty for
    manual stitch and satin columns. The engine reads an element's settings with its family's defaults.
    The reference pages and the JSON Schema list them, and the contract compares each Ink/Stitch row with
    its family's default.
  - A family with a default of its own needs a value, as Ink/Stitch's fills need a longest stitch. An
    empty value there is the family's default, and a length of 0 or less is raised to the least it
    accepts, with `SC-W0102`. A fill's longest stitch is 4 mm when left empty and 0.1 mm when 0. Manual
    stitch and satin columns read both as no longest stitch.
- M5.2: a tatami fill's rows are laid as Ink/Stitch lays them. Its row settings `angle`, `row_spacing_mm`
  and `end_row_spacing_mm` are read and checked. Fills are still skipped (`SC-W0011`).
  - Rows run at the angle, a whole number of row spacings from the design's origin, so fills side by side
    share their rows (`REQ-FILL-TAT-002`). With an end row spacing the spacing changes steadily towards it
    over the part's height and keeps it beyond. Ink/Stitch's rows can pile up without end there over
    parts a few rows tall (`DEV-FILL-003`).
  - Each row is cut where it meets the outline, as GEOS cuts it for Ink/Stitch (`REQ-FILL-TAT-010`). A row
    along an edge is a segment, and a row the outline touches from inside is split there. Shapely's pieces
    of 2,511 rows across 406 random polygons, 98 of them with holes, are recorded and checked
    (`conformance/fixtures/geometry/shapely-rows.txt`).
  - The roadmap's M5 now has 9 steps: needle points get one of their own (M5.3).
- M5.1: a fill's region, the area its path shows under its fill rule (`REQ-FILL-001`). Fills are not sewn
  yet (`SC-W0011`), but their region is built and what it leaves out is said.
  - Subpaths are flattened within a tenth of a CSS pixel and closed, and cut exactly wherever they cross,
    touch or run along each other. The faces between the cuts are filled by the fill rule, `nonzero` or
    `evenodd`, and the edges between filled and empty faces become outlines and holes.
  - Ink/Stitch fills every fill as if its rule were even-odd. A part the `nonzero` rule fills and the
    even-odd rule leaves empty is filled, as the drawing shows it, and `SC-I0306` says so
    (`DEV-FILL-001`). A subpath of fewer than 3 points bounds nothing (`DEV-FILL-002`).
  - A part of 3 square CSS pixels (0.21 mm²) or less is left out, as Ink/Stitch leaves it out, but with
    `SC-W0303` (`REQ-FILL-002`). A fill under 1.4 mm² gets `SC-W0304`, and a fill in parts `SC-W0307`.
  - The rest of M5 follows Ink/Stitch's tatami fill: its rows, stagger, routing, travel, underlay and
    compensation, on this region. The roadmap and the fills design say how.
- M4.9: satin columns drawn as one path are sewn between rails made from their centre line.
  - The rails are the line offset by half the stroke's width to each side, with the stroke's join, point
    for point as shapely offsets it for Ink/Stitch (`REQ-SAT-016`). Shapely's answers for 1,050 random
    lines are recorded and checked (`conformance/fixtures/geometry/shapely-offset.txt`).
  - Rungs go beside sharp corners and at nodes, 1 mm apart or more, and each is kept when it crosses
    each rail once (`REQ-SAT-017`). A line whose offsets split is cut in half, up to 20 times.
    `SC-W0213` counts the parts left out, and `SC-E0214` says when no rails can be made. A line closed
    with `Z` first starts where a rung crosses the stroke's edge twice.
  - The rails made stay the rails, where Ink/Stitch tells rails from rungs again and can take rungs for
    rails (`DEV-SAT-007`).
  - `SC-W0011` no longer names single-path satins: they are all sewn.
- M4.8: a stroke's width and join, and single-path satins too narrow to stitch across.
  - The SVG adapter reads each stroke's width and join as Ink/Stitch reads them (`REQ-SVG-004`). The
    width is the inherited `stroke-width`, scaled by the average of how far the transforms stretch the 2
    axes. The join is `stroke-linejoin`, with `stroke-miterlimit` for a miter. A width in `%`, `em` or
    `ex`, and a join in capitals, are read as a viewer reads them (`DEV-SVG-001`).
  - It reads Ink/Stitch's design settings from the file's metadata: `collapse_len_mm`,
    `min_stitch_len_mm` and `min_satin_stroke_width_mm` (`REQ-SVG-005`). Before, a file that changed
    them was sewn with the defaults, and nothing said so.
  - Satin columns drawn as one path and no wider than the design's `min_satin_stroke_width_mm`, 1 mm by
    default, are sewn as strokes, as in Ink/Stitch, and `SC-W0212` says so (`REQ-SAT-015`). Wider ones
    are still skipped with `SC-W0011` until M4.9.
- M4.7: satin columns start and end at their nearest points, as Ink/Stitch sews them by default
  (`REQ-SAT-013`, `REQ-SAT-014`, ADR 0014).
  - Elements are generated in sewing order, each with its neighbours (`REQ-GEN-003`). It is given where
    the elements before it left the needle, whatever their thread, and what the next element offers to
    end near.
  - **Start** (`start_at_nearest_point`, on by default). A column starts on the line between its rails
    at `running_stitch_position`, nearest the needle. When only its compensated outline is within the jump
    length, it starts there. The needle then follows the line under the column to its first stitch.
  - **End** (`end_at_nearest_point`, on by default). A column ends on its outline nearest where the next
    element starts. Every part of it is cut in 2 there. The column sews the first halves and follows the
    line to its end. Then it sews the second halves back towards the cut and stitches the end last.
  - Of equally near points, a column takes the one Ink/Stitch's geometry library, shapely, takes. Its
    answers for 1,600 random cases are recorded and checked
    (`conformance/fixtures/geometry/shapely-nearest.txt`).
  - A satin's `running_stitch_tolerance_mm` defaults to 0.2 mm, which Ink/Stitch sews when a file sets
    none. Its settings window shows 0.1 mm (`DEV-SAT-006`).
  - TS-05 and TS-06 sew each column as drawn, from its top, and their goldens are unchanged.
- MC-3 kit: TS-05 and TS-06, the satin test sheets, drawn as designs and sewn by the engine.
  - **TS-05** sews columns 1 to 10 mm wide at zigzag spacings of 0.3, 0.4 and 0.5 mm, for the spacing
    that covers the fabric, the widest column that sews well, and how much the thread pulls a column in.
  - **TS-06** sews the same 6 mm column with no underlay, a centre walk, a contour, a zigzag, and a
    contour with a zigzag, for the underlay defaults.
- M4.6: underlays as Ink/Stitch sews them (`REQ-SAT-004`, `REQ-SAT-010` to `REQ-SAT-012`). Before its top
  stitches, a satin column sews the underlays it turns on, all off by default. Straight stitches no
  longer than the running stitch's length (`running_stitch_length_mm`) join them.
  - The **centre walk** runs along the column at `center_walk_underlay_position`, there and back. An odd
    number of repeats ends at the column's end, and the rest of the column is sewn from there.
  - The **contour** runs along each rail, inset by `contour_underlay_inset_mm` and its percentage, and
    stops short of the column's ends by the inset.
  - The **zigzag** goes to the column's end and back, its insets half the contour's unless set: a new
    kind of parameter, a pair that may be left empty.
  - The centre walk keeps to its own tolerance (`DEV-SAT-004`). Ink/Stitch declares that setting but
    reads the satin's running stitch tolerance in its place.
  - Where the running stitch has several lengths, the stitches that join a satin's underlays take the
    first (`DEV-SAT-005`).
  - `SC-W0206` is a contour underlay too short for its insets, which keeps its length. `SC-W0402` names
    the parameter whose length it raised.
- M4.5: split stitches, as Ink/Stitch splits them (`REQ-SAT-003`). Where an element sets a longest stitch
  (`max_stitch_length_mm`), a longer satin stitch is split as `split_method` says.
  - **Default** splits it into the fewest equal parts no longer than the longest stitch, each split moved
    at random by up to `random_split_jitter_percent` of a part. With `random_split_phase`, the splits
    start at random instead and follow at the longest stitch.
  - **Simple** splits at whole multiples of the longest stitch, which line up in rows, and **staggered**
    moves them along by a `split_staggers`-th of it from one stitch to the next.
  - With even splits, a short stitch's inset is at most a third of the longest stitch.
  - `max_stitch_length_mm` moves from manual stitch's parameters to the settings every stitch type shares,
    for manual stitch and satin columns. Designs store it under the same key as before.
- M4.4: short stitches on curves as Ink/Stitch sews them, which has them on by default (`REQ-SAT-009`).
  On each rail of a satin column, a needle point closer than `short_stitch_distance_mm` (0.25 mm) to the
  last one left in place moves in along its stitch by `short_stitch_inset` (15 %) of the stitch. Points
  that crowd one after another take turns with several insets, such as `15 30`.
- M4.3: satin compensation and random variation, as in Ink/Stitch (`REQ-SAT-001`, `REQ-SAT-006` to
  `REQ-SAT-008`).
  - **Pull compensation** moves both ends of every stitch outward along it, by `pull_compensation_mm`
    plus `pull_compensation_percent` of the column's width there. Negative values move them inward, and
    ends moved past each other meet.
  - **Push compensation** (`push_compensation_mm`) shortens the column at its start and end, or
    lengthens it where negative. A rail too short for it keeps its length, and `SC-W0211` says so.
  - **Random variation.** `random_width_decrease_percent` and `random_width_increase_percent` vary each
    stitch's width, and `random_zigzag_spacing_percent` the step to it. The element's `random_seed`
    starts them, and their values differ from Ink/Stitch's (`DEV-SAT-002`).
  - A parameter for each side takes 1 value for both or 2: for the first rail's side then the second's,
    or for the start then the end.
  - `random_seed` is now one of the settings every stitch type shares, for the types that vary at
    random. It means what it did for running stitch.
- M4.2: satin columns are sewn. Their top stitches go rail to rail, placed as Ink/Stitch places them
  (`REQ-SAT-002`). Pull compensation, short stitches on curves, split stitches and underlays follow in
  M4.3 to M4.6, and until then satins differ from Ink/Stitch's where those apply. Ink/Stitch's short
  stitches are on by default.
  - **Rails** run the same way: `reverse_rails` (`automatic` by default) turns the second rail when it
    runs against the first, and `swap_satin_rails` makes the second rail the first.
  - **Sections.** The rungs cut both rails into sections. Without rungs, the rails' nodes pair up in
    order, as in Ink/Stitch. Rails with different numbers of nodes are named (`SC-W0210`).
  - **Pairs** of needle points go across the column, `zigzag_spacing_mm` apart (0.4 mm by default),
    measured across the column at its outside edge.
  - `satin_method` picks the satin stitch. E, S and zigzag stitches are skipped with `SC-W0011` until M7.
  - An SVG's satin columns are sewn once `stitch plan` reads Ink/Stitch's settings, in M8. Until then, a
    satin is sewn where a caller of the engine sets `satin_column`.
- M4.1: satin columns are recognized. A stroke whose `satin_column` setting is on is a satin column, and
  its path is read as its rails and rungs, as Ink/Stitch reads it (`REQ-SAT-005`). Satin columns are
  skipped with `SC-W0011` until M4.2 sews them, and what recognition finds is reported now.
  - **Rails** are told from rungs by which subpaths meet, by Ink/Stitch's rule, as the design page for
    satins describes. Where the rule names other than 2 rails, the 2 longest subpaths are taken, with
    `SC-W0202`, as in the `#` of 2 rails and exactly 2 rungs. Rails may meet, as at a pointed column's
    tips.
  - **Rungs** join the points where they cross the rails. A rung that misses a rail joins its nearest
    point (`SC-W0203`), and one that crosses a rail more than once is left out (`SC-W0207`, `DEV-SAT-001`).
  - A subpath that is one point is left out (`SC-W0205`), and a path with no subpath longer than a point
    is `SC-E0201`. A path of 1 subpath is a satin's centre line, sewn from M4.8 on.
  - The `satin_column` setting has its reference page.
- **User guide.** An install page, a tutorial that turns an SVG into a PES file, and how-to guides to
  check a machine file, convert it and fit a design to the hoop. A section for each stitch type shows
  running stitch and lock stitches, with pictures of what their parameters change.
  - **Pictures and output made by the code.** Command-line examples are declared in
    `docs/examples.toml`, and `cargo xtask docs` runs them with the `stitch` binary. A picture of a
    design is declared in `docs/shots.toml`, with a panel for each set of parameters, and the engine
    plans it. CI fails when either is stale.
  - The README and the book's front page say what StitchCraft does today.
- The MC-2 kit, with 3 test sheets drawn as designs and planned by the engine. Sewing them tests the
  engine's stitches, locks and plan assembly. The MC-1 sheets, drawn stitch by stitch, test the machine
  and the formats.
  - **TS-02B** is TS-02 with the trims, stop and locks elements ask for (`trim_after`, `stop_after`):
    does the machine trim where an element asks, and do the dashes hold?
  - **TS-03** sews running stitch at 1.5 to 4 mm, bean stitch three and five times, circles at three
    tolerances, and 20 stitches placed by hand at each of 0.3, 0.4, 0.5, 0.7 and 1.0 mm: the shortest that
    sews cleanly is the machine's shortest stitch, which the profile sets to 0.3 mm until then.
  - **TS-04** sews every lock shape but custom at three sizes, at both ends of lines trimmed after, to
    pull at: which hold, and which show?
  - A sheet fails to draw if the engine reports anything about it. Each sheet sews what its checks
    describe.
- M3.10: bug-report bundles.
  - **`stitch bug-report design.svg --says "…"`** writes one JSON file that reproduces what StitchCraft
    does with a design. The file contains the design file itself, the profile, the format and the
    version. It also records the plan as a SHA-256 of each entry, bit for bit, and the machine file as
    its size and SHA-256. Each diagnostic and a panic's message go in too. The command warns that the
    bundle contains the design.
  - **`stitch bug-report --replay BUNDLE`** plans the design again through the same code as
    `stitch plan` and compares each record. With the version that wrote it, the bundle reproduces
    (`REQ-CLI-001`). The replay lists what differs, then and now, and the exit status is 1. A file it
    cannot replay is `SC-E0012`.
  - **When StitchCraft finds a bug in itself,** in a failed plan check (`SC-E0009`) or a panic that
    `main` now catches, `stitch plan` writes a bundle in place of the machine file. It says where the
    bundle is and ends with the new exit status 4 (`REQ-CLI-002`). Other commands say how to report a
    panic.
  - **Docs.** The user guide's first how-to, [Report a bug](docs/src/user/how-to/report-a-bug.md).
- M3.9: finalize, and `stitch plan`.
  - **`stitch plan design.svg -o design.pes`** plans an SVG design for a machine
    (`--profile`, `brother-200x200` by default) and writes the machine file. `--preview` adds a picture
    of what it will sew, and `--report` a JSON report: counts, size, threads and every diagnostic with
    its element. The report is written even when nothing else is, to say why. Diagnostics on the
    terminal name their element.
  - **Finalize.** `stitchcraft_engine::plan` now fits the plan to the machine and checks it, and a plan
    it returns can be written as it is. Where one element runs straight on into the next and they nearly
    touch, a needle point less than its element's shortest stitch from the one before is left out
    (`SC-I0504`). That is the shortest stitch the element's generator used, and finalize never thins what
    the generator spaced. Stitches longer than the machine's longest are split into equal parts
    (`SC-I0703`), each part counting against the stitch budget. `plan` returns no plan for too many
    colour changes (`SC-E0601`), for a design larger than the hoop, reaching past its edge from its origin
    or with its stop position past the edge (`SC-E0701`), and for a broken plan invariant (`SC-E0009`).
    Ink/Stitch only drops stitches no longer than the element's shortest stitch, else 0.1 mm
    (`DEV-FIN-001`).
  - **Lock stitches** are stitches into or out of a lock point (`SewnStitch::is_lock`), so a tie-in's
    last stitch, into the stitching's first point, may be 0.2 mm like the rest of the lock
    (`REQ-PLAN-002`).
  - **Conformance.** `REQ-FIN-001..003` are active. The new `plan` data case kind takes an SVG through
    the engine to golden machine files, as `stitch plan` does.
- M3.8: plan assembly, and the engine's entry point, `stitchcraft_engine::plan(design, profile, budget)`.
  - **Generate.** Each element goes to its stitch type's generator. `stroke_method` picks running stitch,
    the default, or manual stitch. Other stroke methods, satins and fills are skipped with the new
    `SC-W0011` until their milestones. So is an element whose parameters cannot be read (`SC-E0101`) or
    whose work budget runs out (`SC-E0004`). The rest of the design still plans, and with nothing left,
    `SC-E0010` says so. Each element sews with the larger of the machine's shortest stitch and its own.
  - **Assemble.** Elements are sewn in document order, and a new thread colour starts a new colour block.
    Threads are compared by colour, as Ink/Stitch compares them, and two names for one colour are one
    block. The needle sews straight on to the next group in the same thread within the collapse length
    (3 mm, or the element's `min_jump_stitch_length_mm`). Otherwise the group ends with its tie-off, and
    the next starts with a jump and its tie-in. `trim_after` and `stop_after` add a trim or a stop after
    the element, with locks around them. Locks go only there, as `ties` says. `force_lock_stitches` adds
    the tie-off, and manual stitch is sewn without locks unless they are forced. Ink/Stitch joins groups
    the same way. An element that sews nothing has no place for its trim or stop. Both are left out, as
    Ink/Stitch leaves them out, and the new `SC-W0505` says so.
  - **Origin.** The plan is in hoop coordinates, with the design's origin, or the centre of its stitches,
    at the hoop's centre. A stop position adds a jump to it before each stop.
  - **Design settings** (`DesignSettings`) hold the collapse length, the shortest stitch, the origin and
    the stop position, with Ink/Stitch's defaults. The SVG adapter reads them from M8.
  - **Conformance.** `REQ-ASM-001`, `-002`, `-003`, `-005`, `REQ-LCK-001` and the new `REQ-GEN-002` are
    active. Start and end hints (`REQ-GEN-001`) come with the SVG adapter's command symbols, M8.
- M3.7: lock stitches (`stitchcraft_engine::locks`), the tie-in and tie-off at either end of a group's
  stitches. Plan assembly (M3.8) sews them where `ties` says.
  - **Shapes.** Each Ink/Stitch lock id is accepted, with StitchCraft's own shapes behind the ids
    (`DEV-LCK-001`). The half stitch goes forth and back twice over half the first stitch. `back_forth`
    does the same over one step of `lock_*_scale_mm`. The drawn shapes, 7 of them, are sized by
    `lock_*_scale_percent`. Each shape lies on the stitching it secures.
  - **Custom steps.** Numbers in `lock_custom_start` and `lock_custom_end` are steps along the stitching,
    read as Ink/Stitch reads them. Locks of steps follow the stitching round its corners, as Ink/Stitch's
    custom steps do, and `DEV-LCK-002` records the 2 places where they differ. A lock drawn as an SVG
    path is not sewn yet. The half stitch is sewn in its place, and the new `SC-W0503` says so. It also
    names an empty custom lock and pieces that are not numbers.
  - **Shortest lock stitch.** A lock stitch is at least 0.2 mm long. Shorter steps are lengthened, and a
    drawn lock is enlarged. Where a sharp turn would fold a lock of steps onto itself, it is sewn straight.
    The new `SC-W0502` reports each change.
  - **Settings windows** show `lock_*_scale_mm` only for the locks it sizes, and `lock_*_scale_percent`
    only for the locks it scales. A parameter can now be shown for several values of another
    (`when key in VALUES`).
  - **Conformance.** `REQ-LCK-002` and the new `REQ-LCK-004` are active. `REQ-LCK-001` (where locks go)
    moves to plan assembly, M3.8.
- **Prose lint.** `cargo xtask prose` runs Vale on the Markdown lines that a branch adds. It checks the
  project's own style and ai-tells, a published style for phrasing that machine-written text overuses. The
  rules are in the new [writing style](docs/src/contributing/writing-style.md) page, with a glossary. CI
  builds Vale and runs the check, and locally it runs when Vale is installed.
- **Docs audit.** `cargo xtask docs --check` reports a sentence of 12 or more words that is on two pages,
  and a path of the repository in inline code that does not exist.
- **Design pages follow the code.** Each docs page that describes code names its source files in an
  `implements` comment. Every source file of a crate or app is on such a page. `cargo xtask docs for PATH`
  lists the pages for a file, and a Claude Code hook lists them after each edit. A branch that changes a
  page's files changes the page too, or a commit message records that it still holds.
- **SVG input page.** A new design page describes what the SVG adapter reads and what it reports.
- M3.6: manual stitch (`stitchcraft_engine::generators::manual`).
  - **Needle points.** A needle point goes on every node of the path, in order. A curve gives only its
    end node.
  - **Longest stitch.** Stitches longer than `max_stitch_length_mm` are split into equal parts, never
    shorter than the shortest stitch. As in Ink/Stitch, 0 or less means no maximum: an optional length
    of 0 or less now counts as empty, where it used to be clamped up with `SC-W0102`.
  - **Bean stitch** applies; repeats don't.
  - **Shortest stitch.** No hand-placed stitch is shorter than the shortest stitch. A point too close to
    the one before is left out, the last point is kept, and the new `SC-W0403` says how many. Ink/Stitch
    drops such points silently.
  - **Conformance.** `REQ-RUN-006` and the new `REQ-RUN-008` are active.
- `SC-I0605`: writing DST says at how many places its machines will cut the thread where the plan does
  not trim. `stitchcraft_formats::encode` returns the file as `Encoded`, its bytes with notes on
  what the format makes the machine do that the plan does not say; `stitch convert` and
  `stitch testsheet` print them.
- M3.5: repeats, bean stitch and random length for the running stitch.
  - **Repeats.** `repeats` sews a run several times, every other pass backwards. Each pass starts where
    the last one ended, so a turnaround is never a stitch in place.
  - **Bean stitch.** `bean_stitch_repeats` sews each stitch 2b + 1 times. A list such as `"1 0"` is taken
    in turn along the stitches and runs on across repeats, each turnaround taking one step, as in
    Ink/Stitch. With a two-value list, each stretch is sewn the same way on every pass.
  - **Random length.** `enable_random_stitch_length` draws each stitch from its length ±
    `random_stitch_length_jitter_percent` and starts each span at a random phase, still ending exactly on
    the corners. The element's own generator draws the lengths, seeded with `random_seed`, so the same
    element and seed always give the same stitches.
  - **Where it lives.** Repeats and bean stitch are in `generators::passes`, shared with manual stitch
    from M3.6.
  - **Conformance.** `REQ-RUN-004`, `REQ-RUN-005` and the new `REQ-RUN-007` are active.
- M3.4: the running stitch (`stitchcraft_engine::generators::running`).
  - **Placement.** Curves are flattened to within a tenth of `running_stitch_tolerance_mm`. Corners
    (turns of more than 30° between segments) always get a needle point. Stitches are spread evenly between
    corners, so none is longer than its length and none is a short leftover. A pattern of lengths (`"3 1"`)
    repeats along the path. A stitch that strays from the curve by more than the tolerance is split.
  - **Measured straight.** Stitch lengths are the straight line between needle points. Where a path bends
    back within less than the shortest stitch (a cusp, a tight loop, a small closed shape), needle points
    are dropped rather than sewing a stitch that short. When the rules disagree, the shortest stitch wins,
    then corners, then the tolerance.
  - **Diagnostics.** `SC-W0401` reports a part of a path too small to stitch. `SC-W0402` reports a stitch
    length below twice the shortest stitch, which is raised to it.
  - **Shared code.** Strokes are flattened by the engine itself, not with `kurbo`, whose flattening uses
    the platform's maths library. Lengths are compared with one shared slack,
    `stitchcraft_core::units::LENGTH_SLACK`, which the plan checker uses too.
  - **Conformance.** `REQ-RUN-001` to `REQ-RUN-003` are active.
- M3.3: the engine's input model and the SVG reader. A `Design` holds elements in stitching order, each a
  stroke or a fill with exact lines and curves in millimetres, a thread and its parameters; `Design::new`
  checks that ids are unique and every point lies within 10 m. `stitchcraft_svg::read` turns an SVG file
  into one: paths and the basic shapes, groups and `<switch>`, transforms, the root's size and viewBox,
  fill and stroke colours (`currentColor`, `paint-order`, gradients by their first colour) and everything
  that hides an element. Arcs become cubic curves within a micrometre, and all of it computes the same on
  every platform. What it leaves out or simplifies is reported: `SC-W0802` for SVG features it does not
  stitch (text, images, clones, style sheets, clipping, masks, filters, markers, patterns, Ink/Stitch's
  settings, which arrive in M8), `SC-W0804` for geometry it cannot use; `SC-E0801` refuses what is not
  SVG. `REQ-SVG-001` and `REQ-SVG-002` are active, and `read_svg` joins the nightly fuzzing.
- M3.2: every registered diagnostic code has a case. A test named `diag_sc_<code>_<what>` produces the
  code from real input and checks the exact text a user reads; `cargo xtask conformance` fails for a code
  without one and lists every code with its cases in the report. All 13 codes have one.
- M3.1: the parameter registry. Parameters are declared once, beside the code that uses them, with
  `params!`; the typed struct, validation (`SC-E0101` for a value that cannot be used, `SC-W0102` for one
  clamped into range, `SC-W0105` for a key StitchCraft does not know, which is kept), the reference pages
  (`docs/src/user/reference/params.md`), a JSON Schema and the Ink/Stitch contract's StitchCraft column
  are generated from the declaration. The first declaration holds the 14 settings every stitch type shares
  (locks, trims, stops, shortest stitch and jump) with Ink/Stitch's keys and defaults; they change the
  stitches from M3.7. `cargo xtask docs --check` cross-checks the registry with the Ink/Stitch contract and
  the deviations ledger, whose first entry (`DEV-LCK-001`) records that the lock shapes are StitchCraft's
  own. `REQ-PRM-001` and `REQ-PRM-002` are active.
- M2.8: quality baselines. Each library crate's public API is a committed snapshot
  (`crates/*/public-api.txt`, `cargo xtask api`), so an API change shows in the pull request's diff.
  Rustdoc builds without warnings. Line coverage per crate may not drop below its floor
  (`conformance/coverage.toml`, a CI job). Mutation testing runs weekly against recorded counts of
  mutants no test notices (`conformance/mutation.toml`). In GitHub Actions every `cargo xtask` finding
  is an annotation.
- M2.5: coverage-guided fuzzing of the readers (`fuzz/`: `read_pes`, `read_dst` and
  `read_write_preview`, which also writes, reads back and previews whatever it read), an hour every
  night with a corpus that grows from night to night; the target bodies run on every pull request
  (`stitchcraft-testkit::fuzz`). `REQ-FMT-006` is active.
- M2.7: `stitch preview FILE -o FILE.png` (`--style realistic|simple`, `--scale`) draws any PES, PEC or
  DST file as it will sew; `stitch convert FILE -o FILE` rewrites one in another format; diagnostic
  `SC-W0604` when a file stores no thread colours. Documentation images are generated by
  `cargo xtask shots` and compared byte for byte on every pull request: the test-sheets reference and the
  first-sew-out tutorial now show each sheet. The `docs:refresh-shots` label regenerates them on a pull
  request (`docs-refresh.yml`).
- M2.6: preview renderer (`stitchcraft-render`) in a realistic and a simple style, drawn from the
  positions a machine file makes (`REQ-RND-001`) and byte-identical on every platform (`REQ-RND-002`);
  diagnostic `SC-E0005` for previews larger than 4,096 pixels on a side. Writers and previews share one
  rounding function, `Point::to_tenths` in `stitchcraft-core`.
- M2.4: an independent reader, pinned pyembroidery, reads every golden machine file the way
  StitchCraft's reader does (conformance case `pyembroidery-oracle`, run in CI).
- M2.3: round-trip property tests for every writer/reader pair, judged by machine-visible behaviour
  (`stitchcraft-testkit::equivalence`), with fixed seeds per PR and fresh seeds nightly.
- M2.1–M2.2: PES/PEC and DST readers (any PES version's PEC block; trims inferred from DST jump runs)
  and `stitch inspect` for any machine file, with an optional profile check; diagnostic `SC-E0603`.
- M1 (machine checkpoint MC-1 pending): budgets and the diagnostics registry (`stitch explain`); the
  stitch plan, its invariant checker and the `brother-200x200` profile with hoop and comfort-zone
  diagnostics; the Brother PEC palette with CIEDE2000 matching; PES v1 and DST writers; test sheets
  TS-01, TS-02 and TS-10A/B/C (`stitch testsheet`); `stitch profiles`; the conformance runner and its
  report (`cargo xtask conformance`), with requirements named by Rust tests (`req_<area>_<nnn>_…`);
  reference pages generated from the code (command line, profiles, formats, test sheets, diagnostic
  codes) and the first-sew-out tutorial with real output.

### Changed
- PES files flag every jump after their first record as a trim, as pystitch, Ink/Stitch's writer, flags
  them (`REQ-FMT-010`). A machine treats them as it treats Ink/Stitch's. StitchCraft flagged only the
  jumps after a trim in the plan. Whether a trim-flagged jump is cut is the machine's choice, and the
  reference PE800 cuts none (MC-1). A jump that goes nowhere is no longer written, since a record of it
  would be trim-flagged.
  - The round-trip checks compare a file with what its format tells the machine
    (`stitchcraft_testkit::equivalence::format_events`, `as_pes`), and a cut with nothing sewn since the
    last one counts as none (`REQ-FMT-002`, `REQ-FMT-003`).
  - TS-02, TS-02B and TS-09 had plain jumps inside a colour, and their files change with this. No other
    test sheet had any.
- Mutation testing runs each test binary with at most 4 GiB of address space. A mutant that keeps a loop
  from ending can make it take more memory than the CI runner has, which ended one part of M5.4's
  mutants without a result. At the cap the test aborts, which counts as noticed.
- The roadmap's M9, VectorCraft ABI v2 proposed upstream, comes last: after 1.0 and the work after it.
  1.0 has the ABI v1 plug-in from M6, and nothing before M9 waits on ArtCraft's answer to the RFC.
- The running stitch page's pictures show their differences plainly. Each stitch length gets the page's
  full width, the tolerance is shown on a half disc whose curve comes out round, angular or as a trapezoid,
  and random stitch length on 14 rows close together, whose needle points line up in columns or scatter.
  With `layout = "rows"` in `docs/shots.toml`, a shot's panels go one under another, each as wide as the
  page.
- A fill's subpaths and a satin column's are flattened within one tolerance, a tenth of a CSS pixel:
  `normalize::stroke::SHAPE_TOLERANCE`, which replaces `normalize::satin::TOLERANCE`.
- Mutation testing on a pull request runs in 4 parts, dealt round-robin as the weekly run deals them, and
  a fifth job adds them up. In one part, the 1,142 mutants in the lines M4.9 changes would have run past
  the job's limit of an hour and a half.
- The reference machine is a Brother PE800, as its owner says, not a Brother with an 8 × 8 in hoop
  ([ADR 0013](docs/src/design/adr/0013-brother-pe800-reference-machine.md)). Its profiles replace
  `brother-200x200`, one for each of its hoops: `brother-pe800-5x7` (130 × 180 mm, the reference and the
  default), `brother-pe800-4x4` (100 × 100 mm) and `brother-pe800-small` (20 × 60 mm). No profile has a
  comfort zone, so `SC-W0702` waits for a sew-out report that finds one.
  - **Test sheets** fit the reference hoop. TS-02 and TS-02B are 120 mm wide, their longest jump 30 mm.
    The TS-10 sheets fill each hoop's field: 100 × 100 mm (TS-10A), 130 × 180 mm (TS-10B) and 20 × 60 mm
    (TS-10C). Each sheet names the profile of the hoop it is sewn in.
  - **`stitch testsheet`** checks a sheet against its own hoop's profile unless `--profile` names another,
    and refuses a sheet larger than that hoop with `SC-E0701`. Before, its plan check would have called
    that a bug in StitchCraft (`SC-E0009`).
  - The hoop messages name the machine once: "The design is 210.0 × 0.0 mm, but the Brother PE800 with its
    5 × 7 in hoop sews at most 130 × 180 mm."
- M3.7: in the parameters' JSON Schema, `x-stitchcraft.visible_when` lists the values a parameter is shown
  for (`any_of`) in place of one (`equals`), and so does `stitchcraft_params::Condition`. The lock shapes
  list (`LOCKS`) moved from `common` to `locks`, next to the shapes it names.
- The Ink/Stitch contract is checked against Ink/Stitch's source. `conformance/inkstitch/check_params.py`
  compares every row of `inkstitch-params.toml` with the parameter declarations in an Ink/Stitch
  checkout, along with the method and lock identifiers. Ink/Stitch is parsed as text; nothing from it
  is imported or copied. At `d59c9ab`, all 145 parameters agree in name, type, unit and default. The one
  gap was four conditions: the lock scales apply only to some lock shapes (`*_scale_mm` to
  back-and-forth and custom locks, `*_scale_percent` to the drawn shapes and custom, neither to the half
  stitch), and the data now says so.
- Reading Ink/Stitch's source is allowed; copying it is not (ADR-0012, replacing decision 2 of ADR-0001).
  Behaviour still goes into the design docs in our own words and code is written from them, so the
  details its documentation leaves out (parameter edge cases, how lists repeat, the font files) can be
  checked against what Ink/Stitch does instead of guessed. This is VectorCraft's own line: its `AGENTS.md`
  forbids copying GPL code, not reading it. `cargo xtask cleanroom` still fails on GPL licence text and
  pasted Python source, and no longer on Ink/Stitch file paths, so a design doc can link what it read.
- Mutation testing on pull requests runs only the mutants in the lines a pull request changes, which
  takes minutes, and fails for any that no test notices unless it is a listed equivalent
  (`[[equivalent]]` in `conformance/mutation.toml`). The full run, with the per-crate comparison, is
  weekly and on demand; with the SVG reader it had grown to about 2,350 mutants, a quarter of an hour
  per part on every pull request that touched the baseline.
- M0.9: StitchCraft can move into VectorCraft's repository (ADR-0011). `cargo xtask compat join DIR`
  makes this folder, copied into a VectorCraft checkout, part of VectorCraft's workspace (or, with
  `--nested`, a workspace of its own that VectorCraft's crates can depend on). It edits VectorCraft's
  `Cargo.toml` in place and refuses when the two cannot merge. The tooling package is now
  `stitchcraft-xtask` (still `cargo xtask`); crates set `publish = false` themselves; every `cargo xtask`
  step works on StitchCraft's packages only, so it means the same inside VectorCraft's workspace. The
  daily `move` job in `compat.yml` rehearses both forms against VectorCraft's latest release and `main`.
- M0.10: the clean room is tighter. The two pages that reviewed Ink/Stitch are gone: one described
  Ink/Stitch's source code, which the clean room forbids even second-hand, and their lessons already live
  in StitchCraft's own requirements and decisions, which now give their own reasons. Ink/Stitch is named
  only where file compatibility needs it (its SVG attribute names, the compatibility contract, the
  deviations ledger) and in the notices saying StitchCraft is independent. `cargo xtask cleanroom` now
  scans every text file, documents included.

### Fixed
- A design exactly the hoop's size fits it. Rounding put rows of a fill's underlay at 90° a hair past the
  edge of the reference hoop's whole field, and the hoop check and the plan checker refused it, while the
  check that the design is centred allowed the hair, as every length check does (`LENGTH_SLACK`). Both now
  allow it too (`REQ-PLAN-001`, `REQ-PRF-002`).
- Planning no longer panics on a part that shrinking cuts into pieces whose least x lie in a chain of
  steps under a nanometre that spans more than one (`REQ-FILL-TAT-012`). The pieces were sorted by
  comparing 2 at a time, as level when their least x lie within a nanometre, which is no order on such
  a chain, and Rust's sort can panic on that. Now a run of pieces, each within a nanometre of the one
  before, is level and comes lowest first. Pieces that are not in such a chain come in the order they
  came before.
- With `skip_last`, a tatami fill's travel between 2 nodes that only an edge pairing them joins sews the
  row's end before it, as Ink/Stitch's does. Such an edge stands for a way of several edges
  (`DEV-FILL-006`), and Ink/Stitch never takes the 2 nodes for neighbours. The golden files stay as they
  were.
- A fill whose shapes meet at corners or share edges sews all of them, in 1 part where they share edges,
  however rounding left the points they share (review findings REG-3 and REG-7, `REQ-FILL-001`). Drawn
  with relative path data, as Inkscape writes it, such points come out a few units in the last place
  apart. The crossings near them were worked out in floating point and snapped to points nearby. That
  could leave 2 edges crossing with no point between them, and whole faces then came out filled or empty
  against the fill rule. In tests it happened to about 1 grid of squares in 8, 1 pinwheel or fan of
  triangles in 4 and 1 triangulated grid in 2. Other grids came out in several parts.
  - The rings are cut on a grid of 2⁻³⁵ mm (`normalize::region::snap`), much as Ink/Stitch puts a fill's
    shape on a grid of 10⁻¹⁰ CSS pixels. Points a rounding error apart become one, and an edge that passes
    through the cell of a corner or a crossing is bent through it, so that the edges meet only at their
    ends. Which way a ring turns is decided exactly.
  - A part grown or shrunk to an exact tangency, such as 2 squares joined by a neck and shrunk by half its
    width, keeps both squares (REG-4, `REQ-FILL-TAT-012`). One of them was lost at 33 of 180 angles.
  - An area that could not be built says only that it is a bug (`SC-E0009`), and no longer that the fill
    bounds no area (REG-8).
  - TS-07 and TS-09 change by a hair (Golden files, below).
- Satin columns are sewn as Ink/Stitch sews them in 3 more places, which a review against Ink/Stitch
  found (`REQ-SAT-002`, `REQ-SAT-005`).
  - **Swapped rails without rungs.** With rails of different numbers of nodes, one turned and the rails
    swapped, other nodes paired, and the stitches slanted to the wrong node. Each rail's nodes are turned
    as the rail drawn in that place now, so a swap only changes which rail sews first.
  - **Flattening.** A satin's curves were flattened within 0.01 mm, where Ink/Stitch flattens them within
    a tenth of a CSS pixel (0.026 mm) by the same halving. The rails have Ink/Stitch's points now: on most
    curves the points differ, by up to 0.02 mm, and a column drawn as one path gets its rungs at the same
    points as there.
  - **Rails of 2 nodes.** The rung added across a column of 2 curved rails without rungs cut each rail
    where the point near its start lies along it. Now the rung is a tenth longer than the line between its
    points, and each rail is cut where the rung comes nearest it, as in Ink/Stitch. The first stitches of
    such a column move by up to about 0.25 mm, less and less along it.
- The SVG reader reads files as Inkscape and Illustrator write them (`REQ-SVG-001`), which a review against
  Ink/Stitch found.
  - **Colours.** An ICC colour after the sRGB one (`#cd853f icc-color(…)`, from Inkscape's colour-managed
    picker) lost the outline it painted, and turned a fill black. Keywords such as `currentcolor` were read
    in one case only. A `style` declaration that did not read hid a valid presentation attribute, where CSS
    falls back to it. Each is read as a viewer reads it now, and a paint that still does not read is named
    (`SC-W0802`). A gradient of one colour, such as an Inkscape swatch, is that colour without a note.
  - **Lengths.** A shape's length with a space after it, such as `y1="80 "`, read as 0, and nothing said
    so. Spaces are allowed now, and a length that does not read is named (`SC-W0804`).
  - **Illustrator's entities.** "Preserve Illustrator Editing Capabilities" declares its namespaces as XML
    entities, and every such file was refused. Entities of plain text are read now. A file is still
    refused when an entity's value holds markup or other entities, or when its references would expand
    it past 64 MiB.
  - **Latin-1 files.** A file whose XML declaration says ISO-8859-1 was refused at its first byte outside
    ASCII. It is turned into UTF-8 now.
  - The note about style sheets says that the fills, outlines and hidden elements they set are lost too,
    not only their colours.
- **A shape's fill is sewn before its stroke,** as Ink/Stitch sews them, where the SVG reader followed
  `paint-order`. Inkscape writes `paint-order` when a user puts the stroke under the fill for looks, and a
  stroke sewn first is covered by the fill. `SC-W0802` notes a shape whose `paint-order` paints the stroke
  first. Ink/Stitch's `stroke_first` setting, which turns the order round, is read with the other settings
  in M8.
- **PEC files without the origin field.** pyembroidery 1.4.32 to 1.5.1 write PEC stitch data without the
  4 bytes that Brother's software writes before the first record. The PES/PEC reader skipped those bytes
  in every file. A file from those versions lost its first record or failed to read. The reader now
  takes the bytes as the field only if the design after them fits the header's box, and otherwise warns
  that Brother machines expect the field (`REQ-FMT-009`). Found by reviewing pystitch's fixes.
- **Palette credits.** `NOTICE` credits pystitch for palette entries 62 and 63, the appliqué functions.
- The SVG reader no longer stitches Ink/Stitch's own objects (`REQ-SVG-003`), which a review against
  Ink/Stitch found.
  - **Commands and connectors.** Command symbols (`<use>` of an `inkstitch_*` symbol) were reported as
    clones, and the connector from each symbol to its object was sewn as a line in its own colour. Lines
    drawn with Inkscape's connector tool were sewn too. None is stitched now. A connector that ties no
    command is noted.
  - **Commands applied.** `trim` and `stop` commands set the shape's `trim_after` and `stop_after`.
  - **Leaving out.** The `ignore_object` and `ignore_layer` commands, and the `inkstitch:ignore_object`
    setting, leave out what they name, and the new `SC-I0805` lists each (`REQ-ASM-004`, now active).
  - **Not applied yet.** `origin` and `stop_position` are noted as not applied yet. A trim or stop on
    something that is not a stitched shape is noted, as is an `ignore_layer` outside every layer.
  - **Helper paths.** Paths that carry Ink/Stitch's guide-line, anchor-line or pattern start marker are
    helpers for other shapes. They are no longer stitched; each is noted, since StitchCraft does not
    apply them yet.
  - **Markers.** A marker property set to `none` no longer hides another one that names a marker, and
    each of the three inherits on its own.
  - **Work.** Reading charges two units of work per XML node: one to find ids and commands, one to read.
- DST is read as DST machines sew it (`REQ-FMT-008`). Machines count jump records in a row and cut the
  thread before three or more — a machine setting; three is the common one, and pyembroidery's reading —
  where something was sewn since the thread was last cut or changed, so a jump longer than 24.2 mm is a
  trim too. StitchCraft recognised only its own spelling of a trim (three small jumps back to where they
  started), so a DST file with a long untrimmed jump read back without the cut its machines make, and
  pyembroidery read such a file differently. Found by the pyembroidery oracle on M3.9's plan golden;
  reproduced on `main` with the new canonical plan `long-jumps`. Round trips through DST are compared
  with what DST machines do (`stitchcraft_testkit::equivalence::dst_events`), and the fuzz body now
  checks DST read-backs as well as PES.
- The compatibility contract check took the lock shape `zigzag`, listed for `lock_*_scale_percent`, for
  the satin method of the same name, and so thought those parameters applied to zigzag satins only. A
  stitch type is now named only by an id of the row's own family; the common settings have none. It
  showed once the contract check (which records the lock shapes each size is shown for) and the running
  stitch's matching by stitch type were both on `main`, and failed its CI.
- The Ink/Stitch contract page matches a registered parameter with the rows for the stitch types it
  applies to, not every row with its key. Ink/Stitch gives some keys to several elements with different
  defaults: registering the running stitch's `running_stitch_tolerance_mm` (0.2 mm) had marked the
  satin's and the fill's (0.1 mm) registered too. A declaration for a stitch type Ink/Stitch does not give
  the key is now reported.
- Mutation testing deals the mutants to its four parts round-robin. Cut into consecutive slices, one part
  held every mutant of the formats crate, whose tests are the slowest, and took 23 minutes while the
  others took 4 to 7.
- Test-sheet drawing charges the stitch budget. Mutation testing found that a sign error in the drawing
  code would make lines grow without bound and use up memory before any check ran.
- Broken and ambiguous links in the formats crate's API documentation.
- The test-sheets reference said "1 stops"; counts are now worded the way `stitch` prints them.
- DST reading: a long jump followed by a long move back could be read as a trim (their split pieces
  cancelled exactly); trims are now runs of jumps of at most 1 mm, and the writer no longer writes
  zero-length jumps.
- **Gap-fill rows stay inside a fill with holes and notches** (`REQ-FILL-TAT-007`), which a review
  found. Each gap-fill row was the stretch of its grid row beside the stretch's last row, chosen afresh
  for each, so 2 in a row could lie on either side of a hole or a notch. The needle hopped between them,
  and back, in straight stitches: on a frame at 30° with 4 gap-fill rows, one ran 0.5 mm into the hole.
  Each gap-fill row now lies beside the one before it, so they are one strip, and the hops are kept inside
  the part as travel is.
- **Pull compensation sews a row once where it meets the outline without leaving the part**
  (`REQ-FILL-TAT-006`), which the same review found. A row that touches the outline from inside, or runs
  along it, is split there into segments that meet end to end, and each was lengthened into the next. The
  needle sewed past the joint and back, 3 times over twice the compensation. A segment is now lengthened
  only where the row leaves the part, as Ink/Stitch's rebuilt part has one segment there.
- **Stretches of rows end for gap-fill rows where Ink/Stitch's end** (`REQ-FILL-TAT-007`), which the same
  review found. A stretch ended where the needle went on to a row not beside the last one. Ink/Stitch
  counts the edges of the needle's way once the loops in it are taken out, and ends a stretch at more than
  one, so a way out and back before the handover to the row beside ends one too. The fills page said the
  two counted alike.
- **Small elements sewn over each other** no longer lose the design its plan (`REQ-FIN-001`,
  `REQ-FIN-003`), which a review found. Finalizing keeps the ends of each run of stitches and its lock
  points, and leaves out the points before them that are too near. When 2 of those still came too near
  each other, the stitch between them stayed, too short for the machine: 2 strokes 0.3 mm long laid over
  each other at the default settings left a tie-off 0.05 mm from a tie-in. The plan check refused it
  (`SC-E0009`), and the whole design was lost, with a bug report. Now the later one is left out too, the
  points before it stay, and a trim or stop after a run's last point left out happens where the needle
  stops. Only a run's first point always stays.
- **A satin column too small to sew** sews nothing, and the new `SC-W0215` says so (`REQ-SAT-018`),
  which the same review found. A column whose needle points all lie within its shortest stitch of its
  first, a speck a few tenths of a millimetre across, sewed stitches all shorter than the machine sews,
  and the design was lost the same way. Ink/Stitch sews such a column (`DEV-SAT-008`). The golden files
  stay as they were.
- **A tight bend inside a curve** is followed by a running stitch within its tolerance
  (`REQ-RUN-002`), which a review found. Where a curve turned back in a V narrower than the shortest
  stitch, the needle past the tip was dropped, and the stitch over the gap could miss the tip by more than
  a millimetre: by 1.24 mm for one cubic at the default settings, sewn 1.2 mm shorter than drawn. A
  stitch that strays is now split at its farthest point whenever both parts are at least the shortest
  stitch, and a part longer than the longest stitch is split along the path. A point nearer than the
  shortest stitch to a needle is still passed over, and strays from the stitches by less than that. The
  golden files stay as they were.
- An SVG file's entities stay within the 64 MiB limit on the file with every reference expanded, however
  they are declared (review finding SVG-1, `SC-E0801`). The limit counted each name at its last
  declaration, where XML and the parser keep the first. A long value declared first, with a short one of
  the same name after it, let a 1 MiB file expand to gigabytes and run the reader out of memory. A name
  declared twice now counts at its longer value. Declarations are looked for only in the document type's
  internal subset (`<!DOCTYPE svg [ … ]>`), so one in a comment or in character data elsewhere neither
  shortens a value nor refuses the file.

### Golden files
- Changed `conformance/golden/testsheets/TS-09.pes`. Its bottom copy has 2 gap-filling rows, and a
  stretch of its rows beside the square hole now ends as Ink/Stitch's gap fill ends one. 2 gap-fill rows
  are sewn there, 9 stitches more (7,788 to 7,797). MC-4 sews TS-09, so its kit is built from this
  version or a later one.
- Changed `conformance/golden/testsheets/TS-07.pes` and `TS-09.pes` (REG-3): a fill's points lie on the
  grid now, a hair from where they were. These sheets' rows lie at multiples of 0.25 mm, so many of their
  needle points lie halfway between 2 tenths of a millimetre, where a change in the last bits changes
  which way they round, and some round the other way now: 0.1 mm. Near the edges of TS-07's circles, where its rows
  meet the edge at a slant, the travel between rows takes a point more or less in places, 5 stitches more
  of 13,522. Their pictures in the docs change with them.
- Changed `conformance/golden/testsheets/TS-02.pes` and `TS-02B.pes` (9 jumps each), `TS-09.pes` (3),
  `conformance/golden/plans/strokes.pes` (5), and `conformance/golden/formats/every-command.pes` (1) and
  `long-jumps.pes` (3): those jumps are trim-flagged now, as pystitch flags them (`REQ-FMT-010`). Only
  their flag bits change, from `0x10` to `0x20`. The DST files stay as they were.
- Changed `conformance/golden/plans/strokes.pes` and `.dst` (M5.6). Each rectangle of the `strokes`
  fixture's fill sews its underlay first: upright rows 0.75 mm apart, square to the fill's flat rows, in
  stitches of 4 mm, from its point nearest the needle and back, where its top rows then start. The plan
  has 128 stitches more (427 to 555).
- Changed `conformance/golden/plans/strokes.pes` and `.dst` (M5.5). In each rectangle of the `strokes`
  fixture's fill the needle now goes from a top corner down to the far side under the rows instead of
  along the outline. The fill sews 2 stitches more.
- Changed `conformance/golden/plans/strokes.pes` and `.dst` (M5.4): the `strokes` fixture's fill is sewn
  now. The fixture draws it as 2 rectangles, to sew a fill in parts end to end.
- Changed `conformance/golden/testsheets/TS-02.pes`, `TS-02B.pes`, `TS-10A.pes`, `TS-10B.pes` and
  `TS-10C.pes`: the sheets redrawn for the PE800's hoops (ADR 0013).
- Added `conformance/golden/testsheets/TS-02B.pes`, `TS-03.pes` and `TS-04.pes`: the exact files MC-2
  sews. The pyembroidery oracle reads them.
- Added `conformance/golden/plans/strokes.pes` and `.dst`: the `strokes` fixture planned for the
  Brother, the first golden files from an SVG design rather than from a plan drawn in code.
- Added `conformance/golden/formats/long-jumps.pes` and `.dst`: untrimmed jumps of two and three DST
  records, from the start, between stitches and after a thread change. DST machines cut before the one
  between stitches only; the pyembroidery oracle reads both files.
- Added `conformance/golden/render/`: previews of a sampler design (simple and realistic) and of TS-01
  (realistic). They pin how previews look; a change to them is a change users will see.
- Added `conformance/golden/formats/` (canonical plans `every-command` and `one-stitch`, PES and DST)
  and `conformance/golden/testsheets/` (the exact MC-1 files: TS-01 PES and DST, TS-02, TS-10A/B/C PES).
- M0 bootstrap: design documents, roadmap and machine-testing protocol; layered workspace with
  `stitchcraft-core` foundations (millimetre units, deterministic math, SplitMix64); `cargo xtask` gates
  (`ci`, `layers`, `docs --check`, `conformance --check`, `cleanroom`, `unsafe-audit`, `filesize`,
  `wasm`); GitHub Actions for CI, docs, VectorCraft compatibility discovery and nightly checks.
