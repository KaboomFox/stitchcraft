# Changelog

All notable changes are listed here, newest first. Golden-file changes must be listed with their reason.

## Unreleased

### Added
- M13.5: columns from shapes. With `--auto`, a part of a shape that isn't narrow, and is nowhere wider than
  the widest satin column, 7 mm, sews as satin columns between its own outlines (`REQ-AUTO-016`).
  - Its centre lines are found as a narrow part's are, but a branch is a bump only when it ends at a tip and
    at its junction too, as the second sharp corner of a wide square end does: each arm of a t's crossbar is
    a column of its own, where the stem's column would cover the crossbar with stitches from one end of it
    to the other. A line is cut where it turns by more than 60°, and a line that comes back to a junction it
    ran through is cut into the line up to the junction and a ring.
  - Each line sews one column over the triangles round it, its rails the part's outline on either side, a
    ring's its outer and inner outline. A column that ends where another runs on ends under it: it sews
    first and is 0.4 mm longer at that end. Otherwise it ends at its farthest point along its line's way,
    flat along the end's edge within 20° of square, or at a point.
  - Its rungs lie square to the mean of its rails' ways (`REQ-AUTO-017`). Drawn with its rails and rungs,
    as Stroke to Satin draws a column (`generators::satin::drawn::from_rails`), it must read back the same:
    a part one of whose columns doesn't stays in the fill, with `SC-I0902`.
  - A column that ends under another sews first, then the longest, then each the one nearest where the last
    ended. A column no wider than the design's narrowest satin stroke runs along its line as a triple run.
    The columns paint their part to its neighbours, and `SC-I0901` counts an area as satin columns when none
    of it is filled and one of its parts sews a satin column.
  - An automatic satin column, a stroke's as well as a shape's, has the underlay of its width
    (`REQ-AUTO-018`). Below 2 mm wide its underlay is a centre walk, and up to 4 mm a contour. A wider column
    has a contour and a zigzag. Its pull compensation is 0.4 mm, and it ends at its rails' ends.

- M13.4: centre lines. With `--auto`, a part of a shape that is narrower than the design's narrowest satin
  stroke everywhere sews as a running stitch along its centre line (`REQ-AUTO-013`).
  - A part is that narrow when nothing of it is left once it is shrunk by half the width with the polygon
    buffer. A part smaller than 1 mm² stays a speck. The rest of the shape stays a tatami fill, and each
    narrow part's line sews after it, named `#2`, `#3` and so on.
  - The centre line comes from a constrained Delaunay triangulation of the part (`REQ-AUTO-014`), built in the
    engine with exact predicates (`stitchcraft_core::exact::incircle`). Points 4 or more on one circle, as a
    square's corners are, are cut into triangles the same way whatever order they come in. It joins the
    middles of the chords, stops within a round end and runs on to a sharp point. Bumps of the outline and the
    corners past sharp turns are left out, at a junction the 2 straightest branches are one line, and no
    piece of the line is sewn twice.
  - A narrow part's line paints the part to its neighbours. It sews after a fill it borders, and the areas
    sewn before it reach under it. One that the areas sewn after it cover whole sews nothing.
  - A running stitch that automatic digitizing makes, of a stroke or a narrow part, is a triple run from
    0.5 mm wide (`REQ-AUTO-015`). A stroke 0.5 to 1 mm wide ran once before.

- M13.3: the sewing order. With `--auto`, the automatic elements sew in an order of their own, chosen after
  specks are merged and before areas reach under their neighbours (`REQ-AUTO-010`, `SC-I0907`).
  - Shapes whose paint overlaps by more than twice the shapes' tolerance keep their paint order, the lower
    first. Shapes that only meet, as abutting paths do, may sew in either order.
  - A satin column or a line that touches a fill without overlapping it sews after the fill, over its edge,
    unless shapes that overlap hold it before.
  - Within those rules each thread sews as long as an element of it is free to sew, and the next element is
    the one whose box lies nearest the last one's.
  - Elements with settings keep their order among themselves, the parts of a shape sewn as several
    elements keep theirs, and nothing moves across an element with a stop after it.
  - `--sort-colours` on `stitch plan` and `stitch export` orders the elements with settings too, with or
    without `--auto` (`REQ-AUTO-011`). A bug-report bundle records it, and its replay orders the same.
  - Each element has the work budget to itself to find how it lies beside the shapes drawn after it, and
    choosing the order has it once more. What needs more keeps paint order, and that is said
    (`REQ-AUTO-012`).
  - A row of 3 badges is drawn badge by badge, as VectorCraft's Auto Grouping draws it
    (`stitchcraft_testkit::pictures::badges`). Digitized, it changes thread twice instead of 8 times, and
    jumps 7 times instead of 9.

- M13.2: automatic areas sewn as neighbours. With `--auto`, the shapes of a traced picture are sewn where
  they meet, in 3 steps (`REQ-AUTO-007` to `REQ-AUTO-009`).
  - The lowest shape is left out as the picture's background (`SC-W0906`) when it is an automatic area with
    other shapes drawn on it, and its outlines, holes aside, cover the box around the design to within
    0.5 mm of each side.
  - An automatic area, or a part of one, smaller than 1 mm² is a speck. It is merged into the automatic
    area it shares the longest edge with, its own included, or left out when it shares none (`SC-I0905`).
    A part of a filled line that small is a speck too.
  - Each automatic area reaches 0.4 mm under each area, satin column or filled line sewn after it, from
    where it touches that shape. Where areas sewn after it cover it, it is cut away 0.4 mm inside their
    edges, and a part they cover whole sews nothing. A stacked trace then sews what an abutting one does
    (`SC-I0904`). Shapes with settings sew as they say, and a column or a line sewn after an area goes over
    it.
  - Each area has the work budget to itself and costs the work of the shapes near it. An area whose
    overlaps need more work keeps its region, and a speck that needs more is left out. When the background
    needs more, it is sewn. Each is said.
  - Test pictures drawn in code, stripes and a badge traced abutting and stacked, measure the edges where
    shapes meet with the thread pulled in 0.4 mm: the abutting badge's go from 89.1 % covered as drawn to
    98.7 % digitized (`stitchcraft_testkit::pictures`, `metrics::edge_coverage`).
- M6.12: StitchCraft's tools in VectorCraft, as 2 filters under **Object › Plug-ins**.
  - **StitchCraft Stroke to Satin** makes each selected line into a satin column drawn with its 2 rails and
    rungs, with the satin effect, as Ink/Stitch's Stroke to Satin does (`REQ-VC-014`). The rails and rungs
    are those the engine makes of a column drawn as one path when it sews it. The column sews as the line
    did, as wide as its stroke. Its rails and rungs can then be edited. The column keeps the line's stroke
    paint, one CSS pixel wide, its satin settings, or the defaults, and its other effects. Each line of a
    path makes a column of its own, and a column already drawn with rails is left as it is. When a line
    can't be made into such a column, or a path has no stroke, the run is refused with `SC-E0814`, and the
    document stays as it was. It is a heavy filter of VectorCraft 0.17, which gives it 1 billion instructions once the user
    allows it when installing it: M6.4's lines take 18 to 233 million in VectorCraft's host. The engine
    draws the column (`generators::satin::drawn`), each rung a little past both rails, and one more rung at
    the start of a column of 2 rungs or none, which Ink/Stitch's rule would read otherwise.
  - **StitchCraft Test Sheet** draws a test sheet drawn as a design, TS-02B to TS-09, into the document
    from VectorCraft 0.16, each object with its StitchCraft effect and settings (`REQ-VC-015`). The sheet is
    moved by whole millimetres to the middle of the artboard (`Design::moved`). It sews as its design moved,
    and exports within 17 % of an export's budget. `TestSheet::design` gives a sheet's design, and
    `TestSheet::designed` says whether it has one.
  - `stitchcraft_vectorcraft::filter` reads what VectorCraft gives a filter and writes what a filter gives
    back. What it hands back unchanged it writes as the text VectorCraft gave. A paint then keeps its
    swatch, and a record stays as it was.
  - The plug-in builds 9 modules, `stroke-to-satin` and `testsheet` the new ones. The tests allow a heavy
    `budget` in a filter's manifest alone, as VectorCraft 0.17 does.
  - Both filters run in VectorCraft's host in Level A. Level B's new scenarios `stroke-to-satin` and
    `testsheets` check that the columns sew as the lines did and that each sheet sews as its design moved.
    Each heavy module a scenario installs is allowed with `allowHeavy`, and the new `sheets` check names
    the sheet drawn in a document.
- M6.11: a VectorCraft document keeps its design settings and machine (`REQ-VC-013`). From VectorCraft
  0.16, **Object › Plug-ins › StitchCraft Design Settings** keeps the machine and Ink/Stitch's 3 design
  settings, the shortest jump, the shortest stitch and the narrowest satin stroke, with the document, as
  data VectorCraft keeps for a filter. A 7th plug-in module builds it (`--features settings`).
  - `stitch export` and the exporters plan the document with the settings kept, and for the machine kept,
    unless `--profile` or the export's dialog names another. The exporters' Machine option is `document`
    by default, which is that machine, or `brother-pe800-5x7` when the document keeps none.
  - What doesn't read keeps its default, with `SC-W0812`. A VectorCraft before 0.16 would drop the data,
    and there the filter declines with `SC-E0813`.
  - VectorCraft opens the filter's dialog at the defaults each time. It doesn't give a live effect what
    the document keeps, and the previews keep the defaults and the reference machine.
  - The engine lists the 3 design settings with their names, labels and help in one table,
    `stitchcraft_engine::design::SETTINGS`, which the SVG adapter, the VectorCraft adapter and the
    filter's dialog all read.
  - Level B's new `settings` scenario keeps a shortest stitch of 1.5 mm and the 4 × 4 in hoop with
    VectorCraft's command line, which changes the PES file, and checks the exports against `stitch
    export`'s. With the small hoop kept, the export of its 1 in square fails with `SC-E0701`. An export in
    a scenario that must fail names its error's code with `fails`, and ends its run.
- M6.10: VectorCraft's dialogs show the parameters as the parameter registry describes them
  (`REQ-VC-012`). From VectorCraft 0.14 each row has the registry's label and its unit, and the registry's
  groups are the dialog's sections, as they are the headings of the reference pages. Resting the pointer
  on a label shows its help, and the button after the field opens the parameter's entry in the reference
  on the docs site. A choice's help says what each option means, since the drop-down list shows the ids
  that designs store. A row shows only for the methods it applies to, and only with the values of another
  setting the registry shows it with, such as a satin's staggers for staggered splits. VectorCraft
  gives the plug-in the values of hidden rows as well, and nothing sews otherwise.
  - The preview level and the exporters' options have their own labels, help and pages. The exporters'
    `auto` is *Digitize automatically*, and `profile` is *Machine*.
  - Releases before 0.14 ignore these keys, and show the dialogs as they did.
  - `cargo xtask docs --check` finds the page and the heading of each help page a manifest links to. The
    window probe checks each dialog's rows at the defaults by label, and the pictures of the dialogs are
    new.
- M6.9: PES and DST exporters for VectorCraft (`REQ-VC-011`). From VectorCraft 0.14, File › Export ›
  Export As lists *PES embroidery (StitchCraft)* and *DST embroidery (StitchCraft)*, 2 more plug-in modules
  (`--features pes` and `--features dst`). An exporter writes the file `stitch export` writes of the
  document, with the same reader, engine, format writer and budget. Its `profile` option is the machine,
  `brother-pe800-5x7` by default, its `auto` option is `--auto`, and the design takes the file's name.
  - What `stitch export` says comes back as the export's warnings, one line each, naming the object, with
    the hint after it. VectorCraft shows 100, and of more the last counts the rest. When anything said is
    an error, the exporter doesn't write the file, and VectorCraft shows the first error.
  - The compatibility gate builds and installs the exporters. Level B's new `export` scenario exports a
    document with VectorCraft's command line, and checks the files and the warnings against `stitch
    export`'s. Level A measures exports in VectorCraft's host: a fill of the 5 × 7 in hoop's whole field
    takes 1.6 billion of the 10 billion instructions VectorCraft allows, and 63 fills of 14 mm squares 4.3
    billion.
- M6.8: live effects read their object's stroke (`REQ-VC-010`). From VectorCraft 0.14 a live effect is
  given its object's fills and strokes. A stroke or a satin effect on the object follows the topmost stroke
  that has a paint and a width: its width, and from 0.15 its join, cap and miter limit, read with the code
  that reads a document's. `stitch export` follows the topmost stroke VectorCraft draws. So a satin column
  drawn as one path and a zigzag stroke show their stitches as wide as their stroke.
  - Given no stroke, before 0.14, for an object without one, or where only a group's stroke paints it, the
    effect leaves such an object as it is, with `SC-I0810`. Its message says that VectorCraft gives no
    stroke, and how to give one.
  - An effect on one of an object's fills or strokes leaves the object as it is, with `SC-I0810`, where it
    showed the stitches of an object that `stitch export` doesn't sew: the reader doesn't use such an
    effect (`SC-W0807`).
  - A satin column drawn as one path makes its rails first, in many units of the engine's work, which the
    preview prices as dear as its stitches. So the auto level shows the outline of a centre line that bends,
    and the exact level its stitches: one bend takes 12.6 million instructions in VectorCraft's host.
  - The contract tests show both as wide as their stroke in VectorCraft's host, and run their previews on
    centre lines straight and curved at each level.
- M13.1: automatic elements. `stitch plan --auto` and `stitch export --auto` choose the stitches of each
  shape that has no settings, by its width (`REQ-AUTO-001` to `REQ-AUTO-006`): a VectorCraft object with no
  StitchCraft effect on it or on a group around it, and an SVG element with no `inkstitch:` attribute. A
  shape with settings is not digitized automatically, and without the switch nothing changes.
  - An area is a tatami fill. A line no wider than the design's narrowest satin stroke
    (`min_satin_stroke_width_mm`, 1 mm unless the design sets it) is a running stitch. A line up to 7 mm
    wide is a satin column as wide as the line, its path the column's centre line: one column for each of
    its subpaths, named `#2`, `#3` and so on after the first. A wider line is the tatami fill of the area
    it paints, its corners joined and its ends capped as the line's join and cap say. Widths are compared
    to the micrometre, so 7 mm and 1 mm written in CSS pixels sew as 7 mm and 1 mm.
  - A part of a line that a satin column cannot follow, a small traced ring, a hairpin or a line shorter
    than a CSS pixel, is filled instead, and so is a dot a round or square cap draws (`SC-I0902`). A dot
    with a butt cap draws nothing and is left out (`SC-W0401`). A line whose filled area would reach more
    than 10 m from the origin is left out (`SC-W0903`). So `--auto` doesn't add an error of its own.
  - A trim or a stop after a line sewn as several columns follows the last of them.
  - A line traced in VectorCraft sews as wide and as long as VectorCraft draws it: the readers now read a
    stroke's cap (SVG's `stroke-linecap`, VectorCraft's cap), and an automatic line's round or square caps
    reach half its width past its ends. An automatic SVG line's corners are joined as SVG draws them, a
    join not set being SVG's miter limited by `stroke-miterlimit`. Of a VectorCraft line sewn across its
    width, its alignment and width profile, which the stitches don't follow, are named (`SC-W0807`), and of
    an SVG line, transforms that stretch it more one way than the other (`SC-W0802`).
  - `SC-I0901` counts the shapes digitized automatically, by the stitches they sew as.
  - `stitch bug-report --auto` records the switch in the bundle, and a replay digitizes the same. Bundles
    without it are written and read as before.
- M7.5: the zigzag satin is sewn, as Ink/Stitch sews it (`REQ-SAT-026` to `REQ-SAT-028`). Set
  `satin_column` on and `satin_method` to `zigzag`. The pairs a satin column places are placed at half the
  zigzag spacing, and one point of each is sewn, the rails taking turns from the first: every stitch
  slants half the spacing along the column.
  - The pairs of the zigzag are inset where their points crowd, each point along its own stitch. Its long
    stitches split by the column's split method. The simple and staggered methods number the stitch back
    to pair i 2i and the stitch across it 2i + 1. The default method splits a stitch back into its own
    fewest parts, where a satin column's takes the count of the stitch across before it.
  - The contour underlay stops where the zigzag stops on each rail, as in Ink/Stitch. After an odd centre
    walk it stops at the same places, where Ink/Stitch looks for them in the turned run and cuts its sides
    elsewhere (`DEV-SAT-011`).
  - The outline the column starts and ends on is Ink/Stitch's, through pairs at the full spacing.
  - A zigzag satin 0.1 mm long or less has no stitch across, and is left out with its underlays, with
    `SC-W0215` (`REQ-SAT-018`). Ink/Stitch sews its underlays alone (`DEV-SAT-008`).
  - A user page with pictures, *Zigzag satin*, and the zigzag satin listed for MC-6. Every satin method
    is sewn now, and `SC-W0011` skips none of them.
- M7.4: the S-stitch is sewn, as Ink/Stitch sews its "S" stitch (`REQ-SAT-024`, `REQ-SAT-025`). Set
  `satin_column` on and `satin_method` to `s_stitch`. Each pair a satin column places is sewn once
  across, the first and every other one from the second rail, with a step along a rail to the next.
  - A stitch across splits as one from the first rail to the second would, and the simple and staggered
    methods number the stitches from 0, so staggered splits move along at every stitch. The steps are
    never split, as in Ink/Stitch, which works out splits for them and sews none.
  - `zigzag_spacing_mm`'s reference entry says what a spacing is for each satin method.
  - A user page with pictures, *S-stitch*, and the S-stitch listed for MC-6. `SC-W0011` no longer skips
    an S-stitch. The zigzag satin is the one satin method it still skips.
- M7.3: the E-stitch is sewn, as Ink/Stitch sews its "E" stitch (`REQ-SAT-020`, `REQ-SAT-021`). Set
  `satin_column` on and `satin_method` to `e_stitch`. A spine runs along the first rail, with a spike
  across the column at each pair, an edge for appliqué.
  - The spikes are the pairs a satin column places, compensates and insets. Each splits as a stitch
    across does, and comes back down through its own splits. The spine splits, with no randomness, where
    the zigzag spacing is longer than the longest stitch.
  - Every satin method now names stitches that lean more than 45° from square to the column, once for
    each stretch of them, where a rung would set them straight (`SC-W0208`, `REQ-SAT-022`). It also names
    its longest top stitch when that is longer than 12 mm (`SC-W0209`, `REQ-SAT-023`). Ink/Stitch gives
    neither warning.
  - A satin column places the pairs that cut it every time now, for the leaning check, and its budget
    counts their work.
  - A user page with pictures, *E-stitch*, and the E-stitch listed for MC-6.
- M7.2: zigzag strokes are sewn, as Ink/Stitch sews its zig-zag stitch (`REQ-RUN-009` to `REQ-RUN-012`).
  Set `stroke_method` to `zigzag_stitch`. The needle crosses the path from side to side, as wide as the
  stroke. Its points are spread evenly along the path in a whole number of zigs and zags.
  - Its settings are `zigzag_spacing_mm`, a stroke's own declaration of the key, which reads a list and
    sews the first value, `stroke_pull_compensation_mm` and `zigzag_angle`. Repeats and bean stitch apply,
    and a closed stroke ends on its first needle point.
  - A new kind of setting, `BoundedAngle`: an angle kept within its range rather than turned by whole
    turns, as Ink/Stitch reads the zigzag's angle. 270° reads as 89° with `SC-W0102`.
  - Ink/Stitch spreads the points along a running stitch at half the spacing, and StitchCraft along the
    path, which differs by hundredths of a millimetre (`DEV-RUN-007`).
  - A user page with pictures, *Zigzag stroke*.
  - The stroke plug-in's dialog lists the 3 settings after those a stroke shares with the other families,
    so the running stitch's stay first. Its live effect leaves a zigzag stroke as it is drawn, with
    `SC-I0810`, as VectorCraft does not tell a plug-in a stroke's width. `stitch export` sews it as wide as
    its stroke.
  - The stroke method's help no longer says the zigzag is skipped. `SC-W0011`'s case uses a ripple stitch.
- M7.1: a family of stitch types can declare a key again, with a kind and range of its own (`REQ-PRM-005`).
  Ink/Stitch's `zigzag_spacing_mm` is a list of lengths for a zigzag stroke and one length for a satin
  column, and a meander fill reads it as a length that is 0 by default. The registry's audit allows one
  declaration of a key for each family, and an element reads the key as its family's declaration says.
  - `find_for` finds the declaration a family reads, and `declarations` finds them all.
  - The JSON Schema gives a key with several declarations any of their schemas, each with the stitch types
    it applies to. The index has a row for each declaration, and the compatibility contract compares each
    declaration with Ink/Stitch's rows for its stitch types. The reference pages show a deviation that
    names the key with each of its declarations.
  - The audit reads a condition from its own declaration, and requires a deprecated parameter's
    replacement for each stitch type it applies to.
- M6.7: the docs show VectorCraft itself, made by the stable release `compat/vectorcraft.toml` pins.
  `cargo xtask shots --vectorcraft` makes the pictures, and the `shots-ui` job checks them on every pull
  request, within 2 pixels in a thousand.
  - A `vectorcraft-render` picture is the export of a document `vectorcraft-cli` draws from the shot's
    steps, the plug-ins' previews included. A `vectorcraft-ui` picture is VectorCraft's window, driven
    through its accessibility tree to the Effect › Plug-ins menu or to a family's dialog. The dialog must
    show a row for each parameter of the module's manifest, with the control of the parameter's type.
  - A tutorial, *A patch in VectorCraft*. A disc of fill, a star of running stitch and a satin border of 2
    rails are drawn and previewed in VectorCraft, then exported and sewn.
  - VectorCraft doesn't scroll a plug-in's dialog. Fill's, 1,385 pixels tall, and satin's, 1,803, run off
    the window at the size it opens, 1440 × 900. The tutorial says how to use them meanwhile, and their
    pictures are taken in a window 2,000 pixels tall.
  - The `docs:refresh-shots` label makes VectorCraft's pictures first, and commits those that could be
    made when another fails.
- M6.6: the VectorCraft compatibility gate is complete. Every day both levels run on VectorCraft's four
  tracks, the latest release and pre-release and the `release` and `main` branches (`cargo xtask compat
  track`).
  - Level B: VectorCraft's own command line draws, applies the plug-ins' effects and saves, in the
    scenarios of `compat/scenarios/`, and StitchCraft checks what it saved. A saved file must sew as its
    effects were sent. VectorCraft must list each module, whose last error must be empty after each
    drawing of the document. The scenarios include a graphic style, a compound satin column and a
    document saved without the plug-ins. A release's `vectorcraft-cli` comes from its Linux archive,
    checked by its SHA-256, and a branch's is built.
  - Each track has one issue, labelled by how much it blocks, and closed when the track passes again.
  - A new release that passes is adopted by a pull request the run opens, which pins it and adds it to
    `compat/supported.toml`. The user guide's supported-versions page is generated from that record.
- M6.5: `stitch plan` reads VectorCraft documents, `.vectorcraft`, `.drawcraft` and `.vctemplate`, as it
  reads SVG files, and `stitch export` is another name for it (`REQ-VC-008`). It writes the machine file,
  the picture and the report, and says what the reader left out. A bug-report bundle includes the part of
  the document the reader reads, its JSON without the images after it, and a replay reads it again.
  - Ink/Stitch's commands are written in an object's or a layer's name (`REQ-VC-009`). `stitch:ignore`
    leaves it out with everything in it, and the new `SC-I0811` lists it. `stitch:start` and `stitch:end`
    mark where the fill or the satin column grouped with them starts or ends, and are never stitched.
    Until the generators start and end at such hints (M8), `SC-W0807` names each marker. It also names a
    group or a layer named as a marker, and any other name that begins with `stitch:`, and reads them as
    other names.
  - The plan case `vectorcraft-patch` takes a VectorCraft document to golden machine files, and the docs'
    pictures of designs can draw one.
  - A how-to guide, *Embroider a VectorCraft design*. The install page says how to build the plug-ins and
    keep them installed in VectorCraft.
- M6.4: VectorCraft's own commands are StitchCraft's tools there, checked in its source. Make Compound
  Path joins a satin column's 2 paths, whichever way it turns them (`REQ-VC-007`), and adds rungs. Expand
  Appearance bakes a preview, and Graphic Styles keep embroidery settings as presets (`REQ-VC-006`). A
  how-to guide shows each. The tools filter the plan had is left for later. Making a line into a satin
  column took 51 to 162 million instructions in VectorCraft's host against about 52 million, and a filter
  cannot give the objects it draws effects. The ABI v2 RFC proposes both (P4, and the new P8).
  - `SC-I0810`'s hint for a satin column drawn as one path names Object › Compound Path › Make.
- M6.3: the VectorCraft plug-ins' previews have levels, and each spends at most what VectorCraft lets a
  live effect spend (`REQ-VC-003`). The dialog's `preview_level` is auto, exact, rows for a fill, or the
  outline. Auto shows a stroke's and a satin column's stitches, and a fill's top rows, one ribbon for
  each row segment. The outline shows when they cost too much. A level the user picks gets the whole
  budget, and `SC-I0810` names a cheaper one when that is not enough.
  - The plug-in prices the engine's work in instructions, which the contract tests measured in VectorCraft's
    host. The engine's work limits a preview more than the JSON does. A 7 mm square fill's exact preview
    takes 27 million of the 50 million instructions VectorCraft allows, and its rows 1.7 million.
  - `stitchcraft_engine::generate::rows` lays a fill's top rows without sewing them, as `generate` lays
    them.
  - The geometry is written by hand, to a thousandth of a point: 18 to 27 bytes an anchor instead of 40,
    and 16 instructions a byte instead of hundreds.
  - The reader leaves `preview_level` out of an element's settings.
- M6.2: VectorCraft plug-ins for the stitch families. Each is its own WebAssembly module, which a feature
  of `stitchcraft-vc-plugin` picks. A family's live effect sews the object it is applied to with the
  engine, and VectorCraft draws the stitches instead of the object's art. A stroke's stitches and a satin
  column's come back as paths, and a fill's as one thin ribbon for each stitch.
  - The parameter registry makes each dialog. A dialog has every parameter of its family that one value
    of VectorCraft's kinds can hold, with the method's group first and the settings every element shares
    last, as in Ink/Stitch (`REQ-VC-002`). A record of a dialog's defaults sews as an element without
    settings.
  - When an effect can't show stitches, VectorCraft draws the object as it is and shows why as the
    plug-in's last error. For an object that sews nothing, that is the message that says why. A satin
    column drawn as one path gets `SC-I0810`, since VectorCraft doesn't pass a live effect its width.
  - `cargo xtask compat contract` builds the four modules and notes their sizes. The contract tests
    install each in VectorCraft's host and run its effect. The command's `--wasm` option is gone.
- M6.1: `stitchcraft-vectorcraft` reads `.vectorcraft` files into designs, as the SVG adapter reads SVG.
  It reads formats 1 to 4, plain or compressed, and stops at the data format 4 keeps after its JSON.
  Layers, groups, paths and compound paths are stitched in paint order, each fill and stroke in its
  colour, CMYK, grey and Lab converted as VectorCraft draws them.
  - StitchCraft's fill, stroke and satin effects, kept in an object's appearance, say how it sews, with
    their parameters (`REQ-VC-004`). An object without them sews its fills, then its strokes.
  - What isn't stitched is named, with how to make paths of it in VectorCraft (`REQ-VC-005`). That
    includes text, images, symbols, blends and other live objects, clipping, masks and VectorCraft's own
    effects (`SC-W0807`). Geometry that can't be used is `SC-W0808`, and template layers, layers that
    don't print and guides `SC-I0809`. A file that isn't a VectorCraft document is refused (`SC-E0806`),
    and a newer format is read as far as it goes (`SC-W0803`).
  - At most 64 MiB of JSON, unpacked a piece at a time, and the budget charged for every object, anchor
    and element. The `read_vectorcraft` fuzz target runs nightly from the new VectorCraft fixtures,
    VectorCraft's own `ribbons` example among them.
  - `stitchcraft_core::Labels` gives repeated ids their suffixes, in both adapters.
- M0.8: VectorCraft's window runs in CI without a screen, and xa11y drives it. `cargo xtask compat window`
  runs the window probe (`compat/vc-window`) against the pinned stable release's app. It fetches the app
  from the release's Linux archive and checks it against the SHA-256 now in `compat/vectorcraft.toml`.
  The tests run in a desktop of their own, with Xvfb, where Mesa draws in software, and a D-Bus session
  whose accessibility bus says a screen reader is on.
  - Above the status bar, the screenshots match to the pixel: twice in one window, across two starts and
    across runs. The status bar's frame timings change with every frame, and the docs' `vectorcraft-ui`
    pictures will leave the bar out (M6.7).
  - xa11y chooses View › Outline through AT-SPI's press, and View › Preview with clicks where the tree
    places them. The control channel sees both changes.
  - The compat workflow's `window` job runs the probe daily, and on pull requests that touch the plug-in,
    `compat/`, the tooling or a manifest.
- M0.7: the geometry is the engine's own, and ADR-0005 says so. The spike checked it against the ADR's
  criteria. 10,000 random polygons with holes grow and shrink into valid parts, in a new property test
  (`any_polygon_with_holes_grows_and_shrinks_into_valid_parts`). The golden files come out the same as
  WebAssembly, and 300 insets of a large region take 218 to 448 ms, where GEOS takes 104 to 149 ms and
  the ADR asked for 200. M7 makes the offsetter fast enough before contour fill relies on it.
  - `cargo xtask wasm --goldens`, the `wasm-goldens` gate, builds `stitch` for `wasm32-wasip1` and runs it
    under wasmtime on every test-sheet and plan case. All 16 golden files come out byte for byte. The
    gates job installs the target and wasmtime 49.0.3, pinned by hash.
  - `stitch` built for WASI writes its files. It named its temporary files after its process id, which a
    WASI program lacks, and stopped at the first file. There the names count from 0.
- M0.6: what a plug-in's JSON costs in VectorCraft's host is measured. A contract test runs the *hello*
  plug-in in VectorCraft's own host and finds the instructions it spends for each byte of JSON it reads
  (512) and writes (822). An anchor of a path is 40 bytes of JSON, and the host runs about 1.9 billion
  instructions a second on GitHub's Linux runners. A live effect's 50 million instructions run out long
  before its 5 seconds, and buy about 60 KB of JSON written as *hello* writes it: M6.3 sizes its preview
  levels from that (`vectorcraft-integration.md`). The test runs on every track of the compatibility
  gate and fails if either cost rises by more than a quarter. `cargo xtask compat contract` runs the
  contract tests optimized, and shows what the tests that pass print.
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
- An automatic satin column digitized from a stroke (M13.1) has the underlay of its width, a pull
  compensation of 0.4 mm and ends at its rails' ends (`REQ-AUTO-018`). It had a satin column's defaults
  before, without underlay or pull compensation. It ended at the points nearest the shapes before and after
  it, and a straight run across it joined it to them.
- `SC-I0902` is said of a part of an area that can't be sewn as satin columns as well as of a line. Its title
  is `Satin column can't be made; filled`.
- Mutation testing on a pull request runs in 8 parts instead of 4. The lines M13.5 changes hold about 590
  mutants, and 2 of 4 parts ran past the jobs' limit of an hour and a half.
- A speck that shares as long an edge with 2 areas goes into the one drawn first. It went into the one sewn
  first, which was the same until the sewing order (`REQ-AUTO-008`).
- `stitch plan`, `stitch export` and `stitch bug-report` take `--auto` and `--sort-colours` from one
  shared set of switches, and their help reads alike.
- `SC-E0813` is now *VectorCraft too old for the filter*. Each StitchCraft filter declines with it in a
  VectorCraft older than it needs, Design Settings and Test Sheet before 0.16, and Stroke to Satin before
  0.15. Its message names the version to update to.
- The compatibility gate pins VectorCraft 0.18.0, adopted from 0.17.0. Every contract case and scenario
  passes against it unchanged, and the exports, the test sheets and Stroke to Satin cost what they cost in
  0.17.0. The plug-in ABI is still v1. From 0.18.0 a run pays nothing for compiling the functions it calls.
  In 0.17.0 a first run paid, and could stop with most of its budget left. From M13.2 on, the gate found
  0.17.0's command line stopping the PES exporter so on the test sheet TS-04, which 0.18.0 exports in 34
  million of its 10 billion instructions. VectorCraft's pictures of the plug-ins menu and the effects'
  dialogs are drawn by 0.18.0, which draws a text field's value near the top of its box. The RFC's status
  and the integration page name 0.18.0.
- The compatibility gate pins VectorCraft 0.17.0, adopted from 0.16.0. Every contract case and scenario
  passes against it unchanged, and an export of a fill of the whole hoop costs what it cost in 0.16.0.
  0.17.0 adds heavy filters, which M6.12's Convert Line to Satin needs: a filter that declares
  `"budget": "heavy"` gets 1 billion instructions and 300 seconds, once the user agrees to install it. It
  also keeps other applications' SVG attributes from import to export, so an Ink/Stitch file opened in
  VectorCraft keeps its `inkstitch:*` settings. The RFC's table, the integration page and the roadmap name
  the release that adopted each.
- The workspace denies `cognitive_complexity` above 30, `too_many_arguments` and `wildcard_imports`, as
  VectorCraft's does on its `main` after 0.18.0. Joined to VectorCraft's workspace, StitchCraft's crates
  take its lints but still read StitchCraft's `clippy.toml`, which now sets VectorCraft's threshold of 30
  (ADR-0011). The one function over it, the test of what VectorCraft rejects in a plug-in's manifest, is 4
  tests now.
- The names of the parameter reference pages, `ParamGroup::slug` and `ParamGroup::title`, moved from the
  docs generator to `stitchcraft-params`, where the plug-in's dialogs name the same pages. And
  `stitchcraft_core::text::plain` gives the plain text of Markdown help, for VectorCraft's dialogs.
- `read_back`, which reads a machine file just written with its plan's threads, moved from the command line
  to `stitchcraft-formats`, and a diagnostic's first line to `Diagnostic::headline` in `stitchcraft-core`.
  The exporters check and word what they write as `stitch export` does.
- The RFC for VectorCraft's plug-ins says what VectorCraft adopted, and the roadmap plans StitchCraft's use
  of it. The RFC proposed an ABI v2. VectorCraft took 5 of its proposals into ABI v1 in 0.14.0 to 0.16.0,
  and 2 more are on its `main`: exporters, rich parameters, data kept with the document, effects and stroke
  style in filters, a live effect's paints, heavy filters and foreign SVG attributes kept from import to
  export. `vc_abi_version` still returns 1. A table on the RFC's page gives each proposal's version and
  pull request. Overlay previews are in review upstream. The page proposes the rest, heavy live effects and
  the objects beside a live effect's object.
  - M6.9 to M6.12, new in the roadmap, use them before M13.7: StitchCraft's machine formats as VectorCraft
    exporters, dialogs labelled and grouped by the parameter registry, the design settings and machine
    kept with the document, and a tools filter that makes a line into a satin column. M9 proposes the rest, still last.
  - The technical design, ADR-0002, the VectorCraft integration page, *Image to embroidery*, the
    parameters page, the new-parameter playbook and `REQ-VC-008`'s rationale say the same.
- The compatibility gate pins VectorCraft 0.16.0, adopted from 0.13.1. From 0.14, VectorCraft gives a
  live effect its object's fills and strokes beside the path, read only, and its host's `effect::apply`
  takes them as a sixth argument. The contract tests didn't compile against it, and every track failed
  from 0.14.0 on. They pass the object's paints now. A new case checks that each family's effect sews an
  object with a fill and a stroke as it sews one without, with the effect on the object or on one of its
  paints.
  - The plug-ins don't read the paints yet. `SC-I0810` now says that the effect doesn't read the stroke of
    a satin column drawn as one path or of a zigzag stroke, where it said that VectorCraft doesn't give it.
  - M6.8, new in the roadmap, makes the live effects read their object's stroke before M13.7.
- The roadmap has 2 more milestones, from a review of how people use Ink/Stitch, which is a new plan page
  (*How people use Ink/Stitch*).
  - M13, image to embroidery, comes next, after M7.5 and before M7.6. A picture traced in VectorCraft is
    to sew well with `stitch export --auto`. StitchCraft chooses the stitches of each shape by its width,
    and the shapes overlap where they meet. Specks and the background are left out, and the sewing is
    ordered for few colour changes. The new design page *Image to embroidery* says how, and TS-13 and MC-8
    test it on the machine.
  - M14, lettering in embroidery fonts, moves from after 1.0 to after M8. Its last steps set the text in a
    picture again as lettering, and TS-14 and MC-9 test it.
  - `--sort-colours` gives any design the order of M13.3, and opening machine files in VectorCraft is
    planned after 1.0.
  - The technical design's non-goals for 1.0 keep photo stitch alone, and diagnostic codes 0900 to 0999
    are automatic digitizing's.
- A fill set to Ink/Stitch's legacy fill is sewn as a tatami fill without underlay, and `SC-W0314` says so
  (`REQ-FILL-005`, `DEV-FILL-014`). It was skipped as a stitch type not sewn yet (`SC-W0011`). Legacy fill
  is off the roadmap, since the method exists for Ink/Stitch's older files. Its own settings, `flip` and
  `reverse`, are not planned, and the VectorCraft plug-in's fill dialog no longer offers it.
- The compatibility gate pins VectorCraft 0.13.1, adopted from 0.7.0 by way of 0.12.0. Pull requests are
  checked against it, and it makes the docs' pictures of VectorCraft.
- A plan case names its design with `design`, which was `svg`, as the design can be a VectorCraft
  document.
- A replay reads a bug-report bundle of up to 192 MiB, which can hold a design of 64 MiB in hexadecimal
  (`REQ-CLI-005`). It read 64 MiB, as from any path, and refused a bundle that `stitch` itself had
  written.
- A VectorCraft group's or layer's StitchCraft effects sew each path inside it that VectorCraft paints,
  as it draws them (`REQ-VC-006`). They were named with `SC-W0807` and left out.
  - Thread and stroke come from the topmost paint over the object, which is a group's own unless the
    group's Contents row is moved up. A path without paint of its own was sewn in black.
  - A clip group's clipping object, a path or a compound path, takes the group's effects and its own
    paints alone.
  - A path that StitchCraft effects reach from more than one level sews as the nearest say, and
    `SC-W0807` names it: VectorCraft runs each on the stitches of the one before.
- An effect takes its thread from the topmost fill or stroke VectorCraft draws, an opaque one first, and
  a stroke or satin effect follows that stroke. It took the first, and a stroke of paint none, which
  VectorCraft writes for an object without one, gave a satin column drawn as one path its width.
- cargo-deny accepts RUSTSEC-2026-0370 to 0372, dated 2026-10-16, which say zvariant, zbus_names
  and zvariant_derive are unmaintained since zbus 6 took them in. Only the window probe uses them, through
  zbus 5 in the xa11y it pins, and they made every pull request's gates fail. `deny.toml` gives each its
  reason.
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
- The install page named VectorCraft's preferences pane for the plug-ins folder as it was up to 0.14,
  Performance & Storage. From 0.15 it is Plug-ins & Scratch Disks, as VectorCraft's
  `crates/engine/src/cmd/prefscmds.rs` names it at each tag.
- Where a satin column's parts meet nearer than the shortest stitch, they leave finishing no needle point
  to leave out (`REQ-SAT-004`). A zigzag underlay came back 0.2 mm in from the rail where an S-stitch
  starts, and where the top stitches start after an odd centre walk or when the column ends near the next
  element. The end's cut could fall where the zigzag underlay's 2 ways cross, and the way along the line
  could end beside where the centre walk is cut. Finishing left a point out there with `SC-I0504`, often
  the end of a top stitch. Now an underlay's last point, or the column's start, moves to the next part's
  first, and the way's points that near it are left out. Beside a top stitch's end the next part's points
  give way instead, as where a column 0.5 mm wide cuts its centre walk. The top stitches keep their ends.
  At the default pull compensation an S-stitch over a zigzag underlay got the note every time, and every
  method often did with an odd centre walk or an end near the next element.
- A satin column's end cuts each part at its own needle point when that lies within the shortest stitch of
  the cut (`REQ-SAT-014`). A part was cut where it crossed the cut, and finishing left out a needle point
  that near another, with `SC-I0504`, a note about a point the design never drew. A zigzag satin, whose
  cuts lie on its own points, got the note at every angle by default. The other methods got it in some
  places, where a centre walk's or a contour's needle points lay that near a cut, and with some pull
  compensations. The distance is measured as finishing measures it, so a cut exactly the shortest stitch
  from a point stays where it is.
- The command line's tests leave nothing in the system's temporary folder. Each run of them left 14
  folders there, about 65 MB, and mutation testing runs them hundreds of times: 5.7 GB had built up
  where this was found. Each test writes its files in a folder of its own, `stitchcraft_testkit::temp`'s
  `TempDir`, removed when the test ends.
- A tatami fill's route charges the work budget for finding the rings that rows end on (`REQ-FILL-TAT-013`).
  It looked at every node for each ring, without charging, at the fill's start and end in each layer: in
  a part of many holes and rows, millions of looks the budget never saw. It looks at each node once now,
  and charges for each.
- A zigzag stroke too narrow for the machine's shortest stitch says so (`SC-W0405`). Its stitches across
  were shorter than the shortest stitch, and finishing left out every other needle point: a zigzag 0.2 mm
  wide at the default spacing sewed as a line along one side, with only `SC-I0504`, which says nothing
  visible changes. The warning gives how many stitches are that short, and the width the zigzag needs.
- `SC-W0102` says what a list out of range is read as, where it said it is used. A zigzag sews only its
  first spacing, so for `zigzag_spacing_mm` of `0.4 500` it said 0.4 and 100 mm were used.
- The design pages describe what Ink/Stitch sews for the steps to come, as the Ink/Stitch sweep of M5
  to M7 read it.
  - The contour fill's spirals run from ring to ring and leave out the branches they don't reach, where
    the page promised spirals split at the shape's necks.
  - The meander fill's path takes random detours through a tile's graph, where the page walked an Euler
    circuit through it, and the circular fill sews a double spiral whatever its settings.
  - The satin page's rows for the S-stitch and the zigzag satin say how their stitches go.
  - A zigzag stroke's later spacings change Ink/Stitch's zigzag on curves, through the running stitch it
    follows.
  - Where the gratings miss a part, StitchCraft's travel keeps to the rings, and Ink/Stitch smooths it
    (`DEV-FILL-013`). Ink/Stitch also warns of an inset that wears a part away without an underlay.
  - A zigzag stroke with a second subpath that draws nothing still closes, and Ink/Stitch leaves its
    zigzag open (`DEV-RUN-008`).
- Gap-fill rows follow a stretch of a tatami fill's rows where Ink/Stitch's do (`REQ-FILL-TAT-007`).
  - Gap-fill rows skipped a stretch whose last row was sewn on through a joint, as a row along the bottom
    of a notch is. The way the rows went was taken from that row's 2 segments, which point no way, and is
    taken from the row sewn before them now.
  - A hop round a small hole, from one half of a row to the other, counted as leaving the stretch, and
    gap-fill rows were sewn between the halves. The edge that pairs the hole's 2 nodes stood for a way of
    several edges. Where a row or a stretch of ring joins the 2 nodes, it stands for that one edge now,
    which Ink/Stitch's graph doubles there.
- Which rows a tatami fill's part has no longer depends on rounding at its sides (`REQ-FILL-TAT-002`,
  `REQ-FILL-TAT-010`, `DEV-FILL-012`). Rows were laid by adding the spacing row by row, and a spacing
  such as 0.1 mm, which rounds in binary, added up its rounding: 10 mm held 101 rows 0.1 mm apart, the
  last a hair inside the far side, and 51 rows 0.2 mm apart. Each row now lies a whole number of spacings
  past the first, multiplied out. A side drawn on a row lies a hair off it too, once the region puts its
  corners on its grid, and the row crossed the part beside the side or missed it: the top of a notch drawn
  on a row 0.1 mm apart was cut in 1 or 2 segments, not 3. A point of the outline within 1e-9 mm of a row
  now lies on it, and a row within 1e-9 mm of a part's far side is left out.
- A satin column ending near the next element is cut at the same place wherever it lies (`REQ-SAT-014`,
  `DEV-SAT-010`). Widened by pull compensation, the stitch across at the end's cut lies along the cut
  but for rounding, and where it was cut came down to the rounding: at either rail, or part of the way
  across. On a column turned by 5.37° with 0.5 mm of pull compensation, the cut fell two thirds of the
  way across. A stitch along the cut is now cut as sides along one line meet: widened, at the cut's
  first rail point, and narrowed, at its own start.
- A VectorCraft document no longer sews differently from what VectorCraft draws without a word
  (`REQ-VC-005`). `SC-W0807` now names what VectorCraft draws of a stroke and the stitches don't follow:
  - A stroke's arrowheads aren't stitched, and a dashed stroke is stitched along all of its path.
  - A stroke painted below a fill is sewn after the fill, over it.
  - A satin column or a zigzag made of a stroke is centred on its path and as wide as the stroke all
    along. A 3 mm stroke aligned inside a closed path, say, sewed its column 1.5 mm out from what
    VectorCraft draws. The stroke's alignment, width profile and caps are named, and so is a zigzag of an
    object with no stroke, which is 0 mm wide.
  - 2 StitchCraft effects on one object or group. Each is sewn on the shape as drawn, but VectorCraft
    draws the second on the stitches of the first. The integration design said to make a fill with an
    outline that way: it now takes 2 objects, as the how-to on columns and presets says too.
- A small file can no longer make StitchCraft fill memory (`REQ-GEN-004`). Generators charged only the
  work budget, and the design's stitch limit was checked when the elements were joined, after their
  needle points were made. 10 mm of zigzag at the smallest spacing, sewn 100 times with bean stitch 10,
  made 4.2 million points, and a metre of it would make 420 million, gigabytes, before the limit refused
  them. A generator now spends a stitch for each needle point it makes and settles at the ones it gives.
  The points it lays out before sewing them must fit in the stitches left. Each element is given what the
  elements before it left, and planning stops at the first that needs more, with `SC-E0004` naming it.
  `SC-E0004`'s explanation and hint say what to change.
- The compatibility gate starts a program again while Linux says it is busy, for up to a second. Its
  tests write stand-ins for VectorCraft's command line and for GitHub's, and run them. A process another
  test forked while a stand-in was written held it open, and Linux wouldn't run it: CI's coverage job on
  `main` failed with "Text file busy".
- The window probe starts a dialog's row only at a label, a static text. VectorCraft 0.13.1 names a
  checkbox after its row's label, and the probe took the checkbox for a label as well: it read the next
  row's label as the checkbox's control, and made none of the dialogs' pictures.
- The compatibility gate's adoption test brings its own pins. It copied the repository's, which pin the
  release it adopts once the adopting pull request does, and it failed in that pull request.
- A satin column of 2 rings sews straight across, wherever each ring starts and however it is closed
  (`REQ-SAT-019`). The second ring now starts at its node nearest the first's first node, and its nodes
  too where they pair up (`DEV-SAT-009`). VectorCraft's Make Compound Path, from 0.10.0, turns the front
  of 2 rings round, which starts it at what was its last node, a quarter of the way round a circle. The
  border of the VectorCraft tutorial's patch, drawn by VectorCraft 0.12.0, sewed stitches across the
  column, 292 of them longer than 12 mm. Ink/Stitch sews such rings from their own starts, with a
  warning. A ring's start also counts once at each end of its nodes. VectorCraft closes a ring with a
  segment back to its start and `Z`, which lists the start twice. Turned round, such a ring paired its
  nodes one place apart. Beside a ring closed by `Z` alone, it got `SC-W0210`.
- The compatibility gate files an issue under a label the repository doesn't have yet. When no label
  matches its search, `gh label list --json` writes an empty output and exits with 0, and the gate read
  that as broken JSON. GitHub Actions may not open pull requests here, so the run that adopted VectorCraft
  0.12.0 had to file an issue naming its waiting branch, and failed to.
- Different threads get different Brother colours in PES files (`REQ-THREAD-002`). Each thread took its
  nearest colour on its own, so 2 threads near one Brother colour were written as one. The machine showed
  the same colour for both, and the file read back as one thread with a stop.
  - A design's threads now get their colours together, the nearest pairs first. Past the palette's 61
    thread colours, threads share, and `SC-W0606` says how many.
  - A thread read from a PES file keeps its colour, Brother's applique steps 62 to 64 included, which
    `stitch convert` turned into ordinary threads. `stitch plan`, `convert` and `inspect` name each thread
    by the colour the file has.
  - A PES file made from a DST file gives each of its black placeholder threads a colour of its own.
  - `DEV-THREAD-001` records where the colours differ from those Ink/Stitch's files name.
- A PES design whose top-left corner lies more than 204.7 mm from the start is refused (`REQ-FMT-011`,
  `SC-E0602`). Its origin field wrapped round, and the design read back elsewhere, as `stitch convert` of
  a DST file from a large frame showed.
- A PES file of a design that starts by sewing keeps its pictures' top row blank (`REQ-FMT-012`). The move
  from the start was drawn across the frame, and the byte after the stitch data's end mark was not 0.
  pyembroidery's and pystitch's readers, which end the data at `FF 00`, read on into the pictures.
- A command that cannot write one of its files writes none, and still prints what was said about the
  design (`REQ-CLI-006`). When `stitch plan` could not write its `--preview` or `--report`, it had
  written the machine file already. Only the failed write was printed, without the warnings, or a refused
  design's errors. A bug-report bundle that cannot be written says why once, where it said it twice.
- `stitch` reads no more than 64 MiB from a pipe or a device either (`REQ-CLI-005`). It took the size the
  file system gives, which is 0 for them, so `stitch inspect /dev/zero` read until memory ran out.
- `stitch bug-report --replay` prints what it takes from a bundle as text only (`REQ-CLI-004`). Control
  characters, line breaks and invisible ones among them, are written as escapes, so a bundle can no
  longer set the terminal's title, clear its screen or print a line of its own.
- `stitch bug-report --replay` reproduces a bundle whose design's file name is not UTF-8, or which was
  written on another platform (`REQ-CLI-001`). The replay took the design name from the file name, and
  the machine file's label came out otherwise. Bundles now record the name the run used.
- Style sheets set an SVG's colours and what it hides (`REQ-SVG-006`). Illustrator and CorelDRAW write a
  drawing's colours as classes in a `<style>` element, and a file read without it sewed black fills where
  the drawing shows outlines, and sewed what the sheet hides.
  - The reader applies a rule whose selector names an element or `*`, then classes and ids, or a list of
    such selectors, in the order of CSS's cascade. One `SC-W0802` names the other selectors and the
    at-rules, whose settings are not used.
  - An `!important` declaration in a `style` attribute now overrides a later one there, as in CSS.
  - A file's style sheets may have up to a million selectors and declarations. Each element is charged to
    the budget for the declarations it takes, those of its `style` attribute included.
- A circle or an ellipse starts at its top and turns left on screen, as Ink/Stitch reads it
  (`REQ-SVG-001`). It started at its right and turned the other way, which moved the tie-in and the
  direction of its running stitch. The golden `strokes` plan changes for its ring, and so does the
  tutorial's badge, whose move from the star to the ring is 7 mm where it was 27 mm.
- Ink/Stitch's `starting_point` and `ending_point` commands are noted as not applied yet (`SC-W0802`,
  `REQ-SVG-003`), where they were dropped without a word.
- An empty root `width`, `height` or `viewBox`, or a root `preserveAspectRatio` that does not read, is
  ignored as a viewer ignores it, and `SC-W0804` names it. The whole file was refused. A viewBox with no
  area says so, where the message said it was not 4 numbers.
- A fill's parts that tie at their least x are sewn in order of their least y there
  (`REQ-FILL-TAT-008`), as Ink/Stitch sews them. The drawing's order decided.
- `SC-I0313` names a fill whose rule leaves a part beside its edge empty, which subpaths running along
  each other bound, such as a notch drawn as a hole along the outline. Ink/Stitch's repair fills such a
  part, and `SC-I0306` no longer says Ink/Stitch leaves it empty. `DEV-FILL-001` describes the repair as
  it is, not as the even-odd rule.
- The roadmap plans clip paths for M8, as Ink/Stitch applies them.
- A tatami fill's rows are walked in the order Ink/Stitch walks them (`REQ-FILL-TAT-001`). Ink/Stitch
  walks a copy of its graph, and its graph library's copy lists a node's neighbours in another order, so
  where a node had several edges left the walk took another turn. The golden `strokes` plan changes, 557
  stitches where it sewed 555. So do 2 MC-4 sheets: TS-08's square at 45° and 0.5 mm, along its edges,
  and TS-09's ways under the rows and the direction of some rows.
- Before a tatami fill, a satin column ends where Ink/Stitch ends it (`REQ-SAT-014`). A fill offers its
  area, and the column ends at the start of its first rail that lies in it, or else of its second. It
  ended where its rails came nearest the fill's edge.
- Before a satin column, a fill ends at the first of the column's rail starts that lies in it, taken in
  the rails' order as Ink/Stitch takes them (`REQ-GEN-003`). It tested each part against both starts
  before the next part.
- Before another fill, a fill ends at the first start of the next fill's outlines that lies in it, or
  else at the first of its own that lies in the next, as Ink/Stitch's geometry library finds them. A
  hole's start counted, and the fill's own outlines were not tested.
- `DEV-FILL-011` records that with `underpath` off the needle travels round a ring the shorter way, where
  Ink/Stitch takes the way with fewer nodes of its graph. The docs said Ink/Stitch takes the shorter way.
- The check for changed golden files compares a pull request with its base branch as it is now. It took
  the base GitHub records when a pull request is opened, so a pull request opened before others merged
  counted their golden files, and failed without the `golden-change` label it did not need.
- The weekly mutation run fails when it measured less than it should (`cargo xtask mutants`). A part whose
  unmutated tree failed its tests looked like a part where every mutant was caught: cargo-mutants still
  writes its lists, empty, and the shard step took any exit. The step now fails on exits other than 0, 2
  and 3, as the pull request's does, and the comparison fails a part whose `outcomes.json` records a
  failed baseline, and a crate `conformance/mutation.toml` records that had no mutant tried.
- `cargo xtask cleanroom` reads every text file, licence files without an extension included. It
  matches licence text in any case and the short GPL notice of a Python module. It finds any import of
  Ink/Stitch's Inkscape and graph libraries, an import from shapely, and a module's relative imports. Of
  Ink/Stitch's 89 `lib` modules it now finds 86, where it found 39.
- `cargo xtask perf` counts the instructions of the `field` example's own `plan`, which is never inlined,
  and fails a count below half the record. Callgrind counts 0 without a word when no function has the name
  it is given, and the gate passed 0.
- An element's settings are read as Ink/Stitch reads them (`REQ-PRM-002`, `REQ-PRM-004`).
  - Empty text, or spaces alone, is not set, and the setting takes its default. Ink/Stitch's settings
    window stores a field the user clears as empty text, and an element with one was refused
    (`SC-E0101`).
  - An element reads only the settings its stitch type uses, and those its other settings show. Before,
    a running stitch was refused for a satin's or a zigzag's spacing, or for a length variation while
    random stitch length was off, none of which it uses.
  - Spaces alone separate the values of a list or a pair. A comma did too, so `3,5` sewed stitches of
    3 mm and 5 mm, where Ink/Stitch sews its default. Text with a comma is now `SC-E0101`, with a hint to
    use spaces and points (`DEV-PRM-001`). An angle of a list may still end with one comma.
  - A `min_random_split_length_mm` of 0 splits every stitch, as in Ink/Stitch. It read as empty, the
    longest stitch.
- A `random_seed` replaces the element's id as its seed, as in Ink/Stitch (`REQ-RUN-007`). It was mixed
  with the id, so elements given one seed varied apart, and a seed of 0 was no seed. `DEV-RUN-005`
  records that the random lengths still differ from Ink/Stitch's.
- A fill whose `fill` setting is off sews nothing, as in Ink/Stitch (`REQ-FILL-003`). StitchCraft did not
  know the setting, and sewed the fill.
- With `force_lock_stitches`, each piece of top rows that `expand_mm` cuts a part into is tied off and
  jumped to, as Ink/Stitch's groups are (`REQ-FILL-TAT-012`). The pieces were joined.
- `DEV-SAT-005` now records that a tatami fill's travel takes the first of a list of
  `running_stitch_length_mm`, as a satin column's does. Ink/Stitch's fill reads the key as one number.
- Where a running stitch's even stitches stray from a curve, the span is spread again in more of them,
  as long as that takes no more stitches than splitting the stray ones (`REQ-RUN-001`, `DEV-RUN-002`).
  A circle 7 mm across is sewn in 10 stitches of 2.16 mm, where it was sewn in 18 of 1.1 to 1.4 mm.
- Manual stitch's bean stitch list takes a step for every point Ink/Stitch lists, and sews nothing for
  one where the needle already is (`REQ-RUN-006`). Ink/Stitch lists the end of each stitch it splits
  twice, as it lists a node drawn twice, and a node left out for the shortest stitch keeps its step. The
  list fell on other stitches than Ink/Stitch's.
- An SVG arc is cut into curves as inkex cuts it, in equal pieces, one more than its whole quarter turns
  (`REQ-SVG-001`, `DEV-SVG-002`). It was cut into pieces of at most 30°, and manual stitch put a needle
  point at each. A circle or an ellipse is drawn as inkex draws it: from its top 3 quarters round to its
  right in one arc, and a quarter back in another. It was 4 arcs of a quarter each.
- A satin column with no section to sew says so, `SC-E0216`, as Ink/Stitch refuses it. Its rungs or its
  push compensation can take each section's length off one of its rails, and it sewed nothing without a
  word.
- A satin column drawn as one line whose rungs are all left out is one section, as in Ink/Stitch. Its
  rails' points were paired in order.
- A satin column drawn as one line is closed when its own subpath is. A point closed with `Z` after an
  open line made the line closed, and its rails looped back.
- A rung that misses a rail joins the rail where shapely's nearest points put it, of points as near. It
  took another.
- Running out of work while joining the elements and fitting them to the machine says so for the design
  (`SC-E0004`). It said that an element had been skipped, and named none.
- A design too large never reads as one that fits. `SC-E0701` and `SC-W0702` give a design's size to the
  tenth of a millimetre above, and so do `SC-E0701`'s messages for an origin or a stop position that
  reaches past the hoop's edge. A design 130.04 mm wide read as 130.0 mm, in a hoop 130 mm wide.
- The plan checker checks `REQ-PLAN-007`: every stitch and jump names an element the plan lists.
  `REQ-PLAN-004` is retired, since whether a trim is tied off depends on lock settings a plan does not
  hold, and assembly's cases check it (`REQ-ASM-003`, `REQ-LCK-001`).
- `SplitMix64::next_in` stays below its upper end. Where the floats lie far apart, the low end plus a
  share of the span rounded up to it.
- The parameter registry hashes a seed's text with `stitchcraft_core::rng::fnv1a64`, not with a copy of it.
- Docs: the core crate's README says that `exact::side` is exact far from overflow and underflow, as its
  module docs say. `Budget` says that each element has the work budget to itself.
  `SC-E0701`'s explanation covers a design placed past the hoop's edge. The data model says that entries
  62 and 63 of the PEC palette come from pystitch.
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
- Building a fill's region charges the work budget for all of its work (review finding REG-5,
  `REQ-FILL-004`). Placing each ring that bounds no face found every face's bounds again for it, and
  joining each hole apart from every outline found every outline's points, bounds and area again, and
  charged nothing. In a test build, a fill of 30 × 30 discs with round holes spent 40 of its 61 seconds
  there, on 6 of the 214 million units it was charged. Each ring's bounds are found once now, and every
  point walked is charged: those steps take 2 seconds, on 11 million units.
- A negative `staggers` staggers a tatami fill's rows the other way, as in Ink/Stitch, and 0 counts as 1
  (review finding ROW-3, `REQ-FILL-TAT-003`). A negative value was raised to 0.01 with
  `SC-W0102`, which lines every row's needle points up into the furrows `staggers` is there to avoid. A
  value past 100 either way is still held at 100, with `SC-W0102`.
- A tatami fill's row that crosses a side along x or along y ends on the side exactly, as GEOS cuts it
  (ROW-4, `REQ-FILL-TAT-002`). It could end a unit in the last place off, so rows ending on one side
  compared as unequal. TS-08 and TS-09 change by a hair (Golden files, below).
- Travel that leaves a fill's part and comes back in within 0.02 CSS pixels, measured along x and y
  together, goes straight on, as Ink/Stitch's diamonds of 0.01 px round the 2 points then meet (ROW-5,
  `REQ-FILL-TAT-005`). StitchCraft measured a straight line, and called the diamonds circles. The golden
  files stay as they were.
- An SVG root with only a `width` or only a `height` takes the other from its viewBox's shape, as SVG 2
  sizes an image (review finding SVG-7, `REQ-SVG-001`, `DEV-SVG-009`). The missing side was the
  viewBox's, in pixels, which shrank or moved the drawing: a root 100 mm wide with a viewBox of 100 by
  100 was drawn 26.46 mm wide.
- A `<polyline>` or `<polygon>` whose list of points has an error is stitched up to the error with
  `SC-W0804`, as path data is (review finding SVG-8, `DEV-SVG-010`). It was cut there without a word, and
  an error before its second point was reported as an empty size or empty data.
- Docs: the deviations ledger records 8 more ways the SVG reader follows a viewer where Ink/Stitch does
  not, `DEV-SVG-003` to `DEV-SVG-010`, and the SVG design page cites each (review finding SVG-7). The
  style module and the design page no longer say that an invalid declaration of any property gives way:
  `display`, `visibility`, the opacities, `fill-rule` and `paint-order` are read as inkex reads them,
  from the declaration the cascade puts first (SVG-9). The Ink/Stitch contract plans `stroke_first` for
  M8, as the code and the changelog do (SVG-10).
- A running stitch with random length draws each span's lengths from its own stream of the element's
  generator, keyed by the span's subpath and its place there (review finding RUN-3, `REQ-RUN-007`,
  `SplitMix64::stream`). Every span drew from one stream of the element's, so a change to one span drew
  new lengths for every span after it. Ink/Stitch keys each stretch the same way, and starts its keys again
  in each subpath, so subpaths drawn alike sew alike there; StitchCraft's keys hold the subpath too, so
  lines drawn alike side by side do not line their needle points up (`DEV-RUN-005`).
- Manual stitch says when it sews stitches longer than `max_stitch_length_mm`, which it does where
  splitting them would sew stitches shorter than the shortest stitch (review finding RUN-6, `SC-W0404`,
  `REQ-RUN-006`). It did so without a word. Ink/Stitch splits such a stitch however short its parts come
  out (`DEV-RUN-006`).
- Docs: `DEV-RUN-004` gives the 3 ways Ink/Stitch sews a subpath of one point or a part too short
  (RUN-8), and `DEV-RUN-005` and the strokes page give how its random length draws and places its
  stitches (RUN-7).
  `DEV-RUN-006` records manual stitch's shortest stitch, and the docs of `passes` no longer say manual
  stitch comes in M3.6 (RUN-9).
- A design without an origin is centred on the box around its finished plan's stitches, as Ink/Stitch
  centres it (review finding PLN-5, `REQ-ASM-005`). Assembly centred the plan before finalize left out the
  points too near their neighbours, and a point left out at the box's edge left the design off centre by up
  to half the shortest stitch. Finalize centres the plan now, once it has fitted it. The golden files stay
  as they were.
- `SC-I0504` gives the shortest stitch that applied to each point left out, and says it was too near a
  needle point next to it (review finding PLN-7, `REQ-FIN-001`). The points a lock point leaves out before
  it were too near that lock point, by a lock stitch's 0.2 mm, and the message said they were less than
  the element's shortest stitch from the point before them.
- A warning about a lock's settings is given once for its element, with no place, where it was given at
  every part the lock was sewn at (review finding PLN-8, `SC-W0502`, `SC-W0503`). A lock sewn straight where
  the stitching turns is still named at each place.
- A custom lock's step of 0 is named with the pieces left out (review finding PLN-9, `SC-W0503`). It was
  dropped without a word. `DEV-LCK-002` lists all 5 ways custom steps differ from Ink/Stitch's.
- Docs: `approach` says that a stroke of a method not sewn yet still offers its first point, as
  ADR 0014 decides and Ink/Stitch's strokes all do, and which settings an offer depends on (review finding
  PLN-10). The engine pipeline page says how stitches are tagged by role, which stitches finalize splits,
  what Ink/Stitch's shortest stitch is when an element sets none, and how PES files flag jumps. The docs of
  `min_jump_stitch_length_mm` say a move that long is sewn on too, and 3 stale comments are fixed
  (PLN-11).
- The straight stitches that join a satin column's underlays to each other and to its top stitches keep
  to the shortest stitch, as its line to the start and the end does (review finding SAT-6, `REQ-SAT-004`,
  `DEV-RUN-003`). With `running_stitch_length_mm` below twice the shortest stitch, the line was sewn in
  stitches twice that long and the joins in stitches of the length set, which finalize then left out. Both
  are sewn in the raised length now, and one `SC-W0402` says so.
- Docs: a rung's place in a satin column drawn as one path is measured from the last place tried (kept or
  not), as the code and Ink/Stitch measure it (review finding SAS-4). `DEV-SVG-011` records that a path read
  from SVG has the subpaths a viewer draws. inkex keeps a move-to with nothing drawn after it as a point,
  and runs a closed subpath on, so the satin page no longer says both count them alike (SAS-6). Ink/Stitch
  takes the rungs of a short, wide column for its rails with exactly 2 rungs, not 2 or more
  (`DEV-SAT-007`, SAS-7). `SC-W0208` and `SC-W0209`, which no step registered, move to M7 with the other
  satin methods (SAT-4).
- A toggle reads Ink/Stitch's words for on, `yes`, `y`, `true`, `t` and `1`, their opposites and `off`, in
  any case (review finding PAR-7, `REQ-PRM-002`, `DEV-PRM-001`). `on` read as true, where Ink/Stitch reads
  it as off, and `y` and `t` refused the element. `on` is `SC-E0101` now, and the SVG reader's yes shares
  the words.
- A parameter's text is stripped of the white space round it before it is read, whatever its kind, as
  Ink/Stitch strips it (review findings PAR-9 and PLN-4, `REQ-PRM-002`, `REQ-LCK-004`). A custom lock's numbers with a tab,
  a line end or a no-break space round them were read as a drawn lock, and the half stitch was sewn.
- An optional length below 0 is named with `SC-W0102` when it is read as not set (review finding PLN-3,
  `DEV-PRM-003`). It was left unset without a word, and Ink/Stitch reads it as a length: a jump length
  below 0 ties off and jumps between every 2 groups there.
- A unit follows its number with no space, for every kind (review finding PAR-10). One length on its own
  read `0.3 mm`, which a list could not. No value longer than 4,096 bytes reads, a list stops at its 17th value,
  and a refused value's message quotes its first 64 characters (PAR-14). A 16 MiB list took 128 MiB to
  refuse, and its message quoted all of it.
- Docs: the deviations ledger records that ranges are StitchCraft's (`DEV-PRM-002`, PAR-11), and the size
  limits on values (`DEV-PRM-004`, PAR-10). An entry lists the parameters whose reading or effect it
  changes, and each parameter's reference entry shows those entries, where it said "same key, meaning and
  default" whatever the ledger said (PAR-13). The reference index lists how reading any value differs.
  `end_row_spacing_mm`'s help says each part of a fill fades over its own height, not the fill's (ROW-6). The
  JSON Schema gives a condition's values as JSON values, and `force_lock_stitches` says all it does. The
  design page says which generated outputs exist and which steps plan the others, and that a key's kind and
  range per family come with M7 (PAR-12).
- A command refuses outputs that name the same file as each other or as a file it reads, with exit status
  2 (review finding CLI-7, `REQ-CLI-007`). `stitch plan mine.svg -o mine.svg --format pes` replaced the
  design, and `--report` naming the machine file left the report under its name.
- Files are written under temporary names of their own, created new, and synced before they are renamed
  into place (review finding CLI-13, `REQ-CLI-006`). The temporary name was fixed, and a link planted under
  it in a shared folder steered the write to another file.
- `stitch plan`, `testsheet` and `convert` print the size and counts of the file they wrote, read back as
  `stitch inspect` reads it, and `stitch plan`'s report and picture are of it too (review findings CLI-12
  and CLI-17, `REQ-CLI-008`). They printed the plan's counts, and `convert` the input's. A DST machine's
  cut before a long jump showed as a jump thread, and the trims a PES file flags on its jumps were not
  counted.
- A closed standard output keeps a command's exit status, and any other error writing its output is said
  and ends with 3 (review finding CLI-10, `REQ-CLI-009`). A refused design piped to a reader that was gone
  ended with 0, and `stitch profiles > /dev/full` with 1 and no word.
- Help and usage errors are coloured only on a terminal, and not with `NO_COLOR` set or a `dumb` terminal
  (review finding CLI-8, `REQ-CLI-010`).
- `stitch bug-report --replay` refuses `--profile`, which it took and ignored, since a replay plans for the
  bundle's profile (review finding CLI-9).
- `stitch inspect` gives lengths to a hundredth of a millimetre and counts stitches in place (review
  finding CLI-11). Its count of short stitches says how many are 0.2 mm or longer, which a lock stitch
  may explain. It said "from 0.3 mm" beside "4 shorter than 0.3 mm", and counted no
  stitch in place at all.
- A bug-report bundle names the commit StitchCraft was built from, and `stitch --version` shows it, and a
  replay reads a bundle's format before the rest (review finding CLI-14, `REQ-CLI-001`). Every build said
  0.0.1, and a replay called a bundle of another format not a bundle. `stitch plan` makes everything it
  prints and writes before it writes a file, and a panic's bundle says whether it came while planning,
  drawing the picture or writing, which a replay reruns as far as it can (review finding CLI-16,
  `REQ-CLI-002`).
- A build names its commit only when git tracks its sources (`REQ-CLI-001`). Built from a copy inside
  another repository, as in the rehearsal of the move into VectorCraft, it named that repository's commit,
  which holds none of the sources, and the test of `REQ-CLI-001` failed there.
- The test sheets from MC-2 on sew nothing over a jump thread the reference machine leaves. It cuts no
  jump (MC-1), and TS-03, TS-04, TS-05 and TS-06 started each row or column along the thread of the jump
  to it, while TS-07's outlines crossed theirs. Stitches sewn over the thread would pin the tail a pull
  test pulls, beside the lock. Rows and columns now alternate between both directions, TS-03's circles are
  sewn right to left, TS-06's centre-walk column first, and TS-07's outlines from where their jumps arrive.
  A test checks every sheet from MC-2 on for it.
- TS-04's half stitch has 3 sizes, on first stitches of 1, 1.5 and 2.5 mm: its 2.5 and 4 mm lines both had
  the 1 mm lock. The checks `stitch testsheet` prints say which end of a TS-04 line is its start, to count
  TS-03's stitches inside the locks at their ends, and that a clean 0.3 mm line means 0.3 mm or less. The
  question whether TS-02B's jumps were cut is gone: MC-1 answered it.
- The running stitch's records say how close its stitches keep to a curve (`REQ-RUN-001`, `DEV-RUN-002`,
  `strokes.md`). The path is flattened within a tenth of `running_stitch_tolerance_mm`, and the stitches
  keep within the other nine tenths of its points, so they keep within the tolerance of the drawn path.
  The records said the whole tolerance, as Ink/Stitch measures it at its points, and so predicted 13 and 9
  stitches for TS-03's 0.1 and 0.2 mm circles, where StitchCraft sews 14 and 10 (review finding RUN2-1).
- Docs now match the code on the machine testing page's pictures, the architecture page's release profile
  and the report's contents, and in the CLI's README on `stitch convert` and the bug report guide on
  `design.svg_hex` (review finding CLI-18).
- `cargo xtask` refuses an argument a command does not take, with exit status 2 and before anything runs
  (review finding XT-14). `cargo xtask docs --checks` rewrote the pages and passed. The commands and their
  options are declared once, with `clap`, which writes the help from them.
- `cargo xtask docs --check` checks an `#anchor` against the ids mdBook writes, with smart punctuation,
  and fails for an included file that does not exist (review finding XT-11). A heading with `--` or
  emphasis in it had another id than the book's, and a heading in an indented line or a comment had one
  where the book has none. mdBook builds a page whose included file is missing. A file in a generated
  directory is an error once its generator is gone, and an image must be where its shot writes it.
- An `implements` comment in a code block is an example, and declares nothing (review finding XT-12).
- A shot of a test sheet with panels is refused (review finding XT-13). A sheet was drawn once, and the
  images its panels named were never compared.
- A code a page names must be registered once its milestone is done, and a `DEV-…` id must be in the
  deviations ledger (review finding XT-10). Any code with a row in the design's table passed, and
  deviation ids were not checked.
- Locally, `cargo xtask conformance` without pyembroidery passes, and CI still requires it (review finding
  XT-15). A requirement whose one case was skipped failed the run, which the docs said it would not.
- A page counts as generated when a generator writes it, whatever its first line says (review finding
  XT-23). The VectorCraft compatibility page said a command that does not exist yet wrote it, and the docs
  audit and the prose lint passed over it. It says now that step M6.6 will write it.
- `cargo xtask conformance --check` requires a row in the fixtures index for every fixture (review finding
  XT-17). The fixtures of shapely's buffers and rows had none.
- The page on the docs pipeline lists the steps the docs workflow runs, and the conformance page its case
  kinds and what keeps a requirement's id (review findings XT-16, XT-19). The contract page leaves its
  count of parameters to its last line, which counts them.
- CI makes each test sheet's and plan's machine files on Linux, macOS and Windows and compares them with
  their golden files (review findings XT-1 and CORE-8). The conformance suite's data cases run in `cargo
  test` now, where only the Linux job ran them. The docs promised a job that compared hashes across
  platforms, which CI never had. They describe what CI does instead, and the suite no longer writes the
  `hashes.json` that job was to read. WebAssembly is built, not run, as the docs now say.
- The oracle test of nearest points counts its 1,600 cases, as the other oracle tests do, so a fixture
  cut short fails it (review finding XT-5). The rows fixture's header says not to edit it, as the others'
  headers do.
- The nightly job on the newest Rust leaves out the API snapshots and the prose lint, which failed it every
  night (review finding XT-6). The snapshots are the pinned toolchain's output and the lint is Vale's, so
  neither says anything about a newer compiler. `cargo xtask ci --skip` takes the gates to leave out.
- `cargo xtask prose` finds the added lines whatever diff prefixes Git is configured with (review finding
  XT-7). With `diff.mnemonicPrefix` or `diff.noprefix` set, it found none and passed.
- `cargo xtask unsafe-audit` no longer takes a comment for code (review finding XT-8). A crate root's
  `#![forbid(unsafe_code)]` counts in its code only, and a character literal such as `'\"'` no longer hides
  the code after it. Each `unsafe` block needs a `// SAFETY:` comment of its own, and the compatibility
  probe's crate root must forbid unsafe code too. The checks leave out folders named `book` or `corpus`
  only where the book and the fuzz corpus are made.
- The satin cases' helpers sew a column with the element's shortest stitch where it is longer than the
  machine's, as the engine does (review finding XT-18). They took the machine's. A test compares a column
  the helpers sew with the one the engine plans.
- Clippy refuses every transcendental method of `f64` and `f32`, `powi` among them, and `RandomState`
  (review findings XT-20 and CORE-9). In library crates it refuses calls for files, threads, the
  environment and processes, which `wasm32-unknown-unknown` lacks (review finding XT-21). `deny.toml` lets
  `rand` and `getrandom` in only through proptest.
- CI installs each tool at a pinned version, from one list in `.github/actions/tools` (review finding
  XT-22). The compatibility job fails when it cannot discover VectorCraft's versions, or when the stable
  or main track is empty, where before its tracks passed untested. cargo-deny checks the fuzz targets'
  dependencies too. Changed golden files need a line added to the changelog, where any change to it
  passed, a deletion included.
- The Ink/Stitch contract records the value Ink/Stitch sews for a file that sets none where its code reads
  a missing attribute as another value than its settings window shows (review finding PAR-14). A satin's
  and a fill's `running_stitch_tolerance_mm` sew 0.2 mm, shown as 0.1 mm, and a ripple's `staggers` sews
  1, shown as 0. `check_params.py` compares every getter in Ink/Stitch's code with the contract, and the
  contract page compares the registry's defaults with the values sewn.
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
- **Fills with many or detailed holes** are sewn instead of refused (`REQ-FILL-TAT-013`), which a review
  found. Travel under the rows placed points on the rings, weighed its lines by their distance from the
  rings and kept its way inside the part by looking at every side of every ring, for each point, line and
  stitch. A 70 mm square with 14 × 14 round holes needed 813 million units of work, more than the
  budget, and was refused with `SC-E0004`, the whole design with it. The ring sides are now filed in a
  grid, and each question looks at the sides near it alone, with the same answers: the square needs 21
  million units. The golden files stay as they were, and the hoop's whole field plans in about 1.5 % fewer
  instructions.
- To keep the needle's way between a fill's rows inside the part, StitchCraft looked at the rings 3 times
  for each stitch and charged the work budget for one of them, which the same review found. The other 2
  are now charged too, a unit for each cell and side they look at.
- **`stitch plan` refuses a design with an element left out with an error** (`REQ-CLI-003`), which the
  same review found. The engine leaves out an element it cannot sew, such as one whose planning runs past
  its work budget (`SC-E0004`), and plans the rest. The command wrote the machine file without the
  element and ended with exit status 0, though its help, its README and the diagnostics reference say
  that nothing is written after an error. Now only the report is written, and the exit status is 1, as
  Ink/Stitch writes its file only when no element fails. The picture is drawn before anything is
  written, so a picture that cannot be drawn stops the machine file too.
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
- Changed `conformance/golden/render/sampler-simple.png`, `sampler-realistic.png` and `TS-01-realistic.png`
  (`REQ-RND-002`), and the 55 pictures in the docs that the renderer draws. tiny-skia 0.12 encodes PNG files
  with version 0.18 of the png crate, which writes them 4 to 6 times smaller. Every pixel is as it was: each
  file decodes to the pixels of the one it replaces. The golden previews are compared byte for byte, so
  they are written again with the new encoder.
- Changed `conformance/golden/testsheets/TS-08.pes` (`REQ-FILL-TAT-002`, `REQ-FILL-TAT-010`). In the
  square at 0° and 0.4 mm, the underlay's rows, 1.2 mm apart and added up, put one 1e-14 mm inside the
  square's far side, along it. That row is left out now, and the underlay takes another way through its
  rows. The first top row, 1e-14 mm inside the near side, runs along it now. The square sews 11 stitches
  more: TS-08 has 16,248, where it had 16,237. In the square at 0° and 0.3 mm a row moves by 3e-13 mm,
  and a tie-off stitch that lies halfway between 2 tenths of a millimetre rounds the other way. MC-4 sews
  TS-08, so its kit is built from this version or a later one. Its picture in the docs changes with it.
- Added `conformance/golden/plans/patch.pes` and `.dst` (M6.5), which the `vectorcraft-patch` case plans
  from `conformance/fixtures/vectorcraft/patch.vectorcraft`. The patch sews a fill, a satin column, a
  running stitch and a group's fill effect in 2,466 stitches and 4 threads. Its placement line is left
  out by its name, and its start marker isn't stitched.
- Changed `conformance/golden/testsheets/TS-03.pes`, `TS-04.pes`, `TS-05.pes`, `TS-06.pes` and
  `TS-07.pes`. Each is sewn in a new order, so that no jump thread lies under stitching sewn after it.
  TS-04's half stitch is on first stitches of 1, 1.5 and 2.5 mm (623 stitches, where there were 600), and
  the others keep their stitches, in a new order and direction. MC-2, MC-3 and MC-4 sew these sheets, so
  their kits are built from this version or a later one.
- Changed `conformance/golden/testsheets/TS-08.pes` and `TS-09.pes` (ROW-4): rows end exactly on the
  sides of the squares and of the rectangle now, a unit in the last place from where they ended. Many
  needle points of these sheets lie halfway between 2 tenths of a millimetre, so some round the other way:
  673 stitches of TS-08 and 1,350 of TS-09 move by 0.1 mm. Their stitch counts stay as they were, and
  their pictures in the docs change with them.
- Changed `conformance/golden/testsheets/TS-09.pes`. Its bottom copy has 2 gap-filling rows, and a
  stretch of its rows beside the square hole now ends as Ink/Stitch's gap fill ends one. 2 gap-fill rows
  are sewn there, 9 stitches more (7,788 to 7,797). MC-4 sews TS-09, so its kit is built from this
  version or a later one.
- Changed `conformance/golden/plans/strokes.pes` and `.dst` (step 13 of the review's fixes). The
  fixture's ring is drawn in inkex's 2 arcs, and the design, centred by its stitches, is 4 micrometres
  narrower. Its fill's travel is spread evenly where it strayed, and the plan has a stitch fewer (557 to
  556).
- Changed `conformance/golden/testsheets/TS-03.pes`: its circles at a tolerance of 0.1 mm and 0.2 mm are
  sewn in even stitches, 23 and 19 where they were 25 each (588 stitches, where there were 596).
- Changed `conformance/golden/testsheets/TS-08.pes` and `TS-09.pes`: the travel between rows is spread
  evenly where it strayed. TS-08's square at 0° and 0.4 mm has a stitch fewer, its square at 90° and
  0.6 mm moves stitches, and TS-09's travel moves in all 3 copies.
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
