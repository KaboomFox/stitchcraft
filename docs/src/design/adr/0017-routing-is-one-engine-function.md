# ADR-0017: Routing is one engine function, which VectorCraft's filters and lettering call

<!-- The status line, a label and its value, which the ADR index is checked against. -->
<!-- vale ai-tells.ColonUsage = NO -->
**Status:** Accepted · 2026-11-03, after spike M10.3
<!-- vale ai-tells.ColonUsage = YES -->

## Context

Ink/Stitch's Auto-Route Satin Columns and Auto-Route Running Stitch replace the elements a user selects
with new ones in a sewing order. Runs that later stitches cover join them, and jumps join only what lies
apart. Ink/Stitch's lettering routes the text of a font whose settings turn it on the same way. M10.4 and
M10.13 bring the 2 tools to StitchCraft, M14 sets text in Ink/Stitch's fonts, and M15.8 joins a ported
glyph's columns. Spike M10.3 chooses where routing runs. It could run in VectorCraft filters like M6.12's
Stroke to Satin, in a function that lettering calls, in a pass of `stitch plan`, or in all of them. The
spike also chooses whether routing starts from the order M13.3 gives a design, how a satin column is cut
in 2, and how a document marks where a route starts and ends.

What was checked, on 2026-11-03:

- **Ink/Stitch's tools,** read at `d59c9ab` ([ADR-0012](0012-read-dont-copy.md)), and their documentation.
  Auto-Route Satin Columns cuts each selected column into pieces at most 1 mm long along its centre line,
  and the strokes selected with the columns show where the needle may travel. Auto-Route Running Stitch
  cuts the selected strokes where they cross or pass near each other. Jumps join the pieces that lie
  apart, as short in all as they can be. The route runs from the leftmost end to the rightmost, or from
  the `autoroute_start` command to the `autoroute_end` command. A stretch it passes twice is sewn the
  first time as a running stitch, which the second time covers. The pieces keep their elements'
  settings, and they go in a new group after the last element, or with Preserve order into their
  elements' groups. The elements are deleted with their commands. Both tools can trim long jumps, and the
  satin tool can keep the elements.
- **Ink/Stitch's lettering** routes the satin columns and strokes of the whole text with Auto-Route Satin
  Columns, their order kept and without trims, when the font's `font.json` turns `auto_satin` on. A font
  that leaves the key out has it on. At `c7e3a05`, 2 of the 142 fonts in Ink/Stitch's library turn it on,
  Ink/Stitch Small Font and Medium Font, and 140 turn it off.
- **What a VectorCraft filter can do,** in VectorCraft's `docs/plugins.md` and `crates/plugins/src/objects.rs`
  at `c733ae97f`, after 0.18.0. A filter is given the selected paths in paint order, those in selected
  groups and layers among them, each with its name, geometry, paints and own effect records. It is given
  no group. In one undo step it changes, adds and deletes objects. A new object goes beside an object the
  filter lists, and the filter can't make a group or move the objects it was given. A heavy filter, from
  0.17, gets 1 G instructions and 300 s once the user allows it at install, and runs in the background
  without a preview. VectorCraft's documentation names routing a plotter's pen as work for one. A live
  effect is given its own object alone.
- **What StitchCraft has.** The satin generator finds a place on a column by its cut, one of the
  column's pairs at the zigzag spacing, where a column ends at its nearest point
  ([satin](../algorithms/satin.md#start-and-end)). `generators::satin::drawn` draws a column with its
  rails and rungs for Stroke to Satin and for automatic digitizing. Stroke to Satin's runs, which make a
  column's rails from its centre line, took 18 to 233 million instructions in VectorCraft's host. M13.3
  orders automatic elements by their overlaps with the shapes around them, their threads and their
  distances. A VectorCraft document marks where a shape starts and ends with objects named `stitch:start`
  and `stitch:end` (`REQ-VC-009`), and the SVG adapter sews none of Ink/Stitch's commands
  (`REQ-SVG-003`). `stitch` writes machine files, previews and reports, and M8.12's document writer, after
  1.0, is the first that writes a document for a user to edit
  ([ADR-0016](0016-inkstitch-files-imported-by-stitchcraft.md)). The differential already runs one of
  Ink/Stitch's tools, Fill to Stroke, as a black box.

## Decision

1. **One function routes.** `stitchcraft_engine::route` takes elements in their order, the options of
   Ink/Stitch's tool, and a start and an end. It routes them as Auto-Route Satin Columns (M10.4) or
   Auto-Route Running Stitch (M10.13) routes a selection. It gives back the elements that replace them,
   in sewing order, with their settings and trims. It is pure, deterministic and charged to a budget, as
   a generator is. VectorCraft's filters, lettering and M15.8 call it, as the points below say.
2. **In VectorCraft, routing is 2 heavy filters,** StitchCraft Auto-Route Satin Columns and StitchCraft
   Auto-Route Running Stitch, in Object › Plug-ins. A filter replaces the selected paths with the routed
   objects in one undo step, as Stroke to Satin replaces a line. The objects it writes have their
   StitchCraft effects. The user sees each piece's preview and edits the route as any other objects. The
   dialogs offer Ink/Stitch's options, declared once beside the function.
3. **Markers mark a route's ends.** In a VectorCraft document, objects named `stitch:route-start` and
   `stitch:route-end`, selected with the paths, mark the route's start and end at their middles, as
   Ink/Stitch's `autoroute_start` and `autoroute_end` commands mark them. The filters delete the markers
   they use, as Ink/Stitch's tools delete the commands of the elements they replace. `stitch export` sews
   neither.
4. **A satin column is cut at one of its pairs,** the kind of cut it ends at. Each rail is cut at its
   point of the pair. Each piece keeps the column's settings and the rungs that cross both its rails, and
   gets one rung more where it would otherwise be read back with other rails. A column drawn as one path
   is cut as the rails and rungs the engine makes of it, and its pieces are drawn with them.
5. **Lettering calls the function.** A font whose `auto_satin` is on has its text routed in its order, as
   Ink/Stitch's lettering routes it (M14.2). M15.8 routes a ported glyph's columns and lines with it, from
   the glyph's lower left to the next glyph.
6. **`stitch plan` routes no element the user drew,** and routing doesn't start from M13.3's order. A file
   sews in the order it is drawn, which a routing tool wrote if the file was routed. Routing while a
   design is planned is for the elements StitchCraft makes itself: lettering's, and a ported glyph's in
   automatic digitizing (M15.8).
7. **The command line gets `stitch route` after 1.0,** in a step of its own after M8.12, whose writer it
   needs. StitchCraft proposes no VectorCraft change for routing.

## Consequences

- A route is part of the document. Each piece previews with its effect, its dialog changes its settings,
  and `stitch export`, the exporters and `stitch plan` sew the pieces in their paint order as they sew any
  objects. An Ink/Stitch file routed by Ink/Stitch sews as before, and the `autoroute_start` and
  `autoroute_end` commands left in a file sew nothing.
- The filters read each path by its own StitchCraft effects, the only ones VectorCraft gives a filter. A
  path that sews by its group's effect (`REQ-VC-006`) is left as it is, as Ink/Stitch's tool leaves a
  fill. A selection with nothing to route is refused with a coded error, whose hint says to put the
  effect on the paths.
- The routed objects go where the topmost selected path was, as Ink/Stitch puts its new group after the
  last element. With order kept, each piece goes where its path was, where Ink/Stitch moves it to the end
  of its group, which a filter can't do. A group the filter empties stays in the document.
- VectorCraft installs the filters once the user allows them, and runs them in the background. A
  VectorCraft before 0.17 gives them a light run's budget, which a column drawn as one path can outrun, as
  it can Stroke to Satin's.
- The route is the same in the filter's WebAssembly, in `stitch lettering` and in the differential. The
  differential runs Ink/Stitch's tools and the function on TS-17C's elements and compares what each sews.
- `stitch lettering` routes text in a font whose `auto_satin` is on from the command line. A design drawn
  by hand is routed in VectorCraft, or in Inkscape with Ink/Stitch, until `stitch route` comes.
- `--sort-colours` moves routed pieces as it moves any element with settings, and can break their route.
  A routed design is in sewing order already.
- The cut could also serve a Cut Satin Column filter, as Ink/Stitch's tool of that name cuts a column at
  its `satin_cut_point` commands. No step plans one. Automatic digitizing could route a traced shape's
  columns too, and no step plans that either.
- Revisit the filters' reading and placing if VectorCraft gives a filter the groups round a path, or lets
  it make one. Revisit the command line if a drawn file must be routed without VectorCraft before 1.0:
  `stitch route` then brings a writer forward. Revisit the budget if M10.4's contract test finds routes
  that outrun a heavy filter's.

## Alternatives considered

- **A pass of `stitch plan`,** routing the elements a file marks while it is planned. The route would be
  nowhere in the document. VectorCraft's previews show each object alone and couldn't show it, and nobody
  could correct a cut or a run. A file would sew in an order it isn't drawn in, and otherwise than
  Ink/Stitch sews it. The pass would need a way to mark what it routes. It would also need rules for the
  trims and stops of the elements it cuts, and for their start positions. Rejected. Lettering and M15.8
  route while a design is planned because StitchCraft makes the elements they route.
- **Starting from M13.3's order.** Ink/Stitch's tools keep the order they are given or choose their own
  from the pieces. The differential checks StitchCraft's routes against theirs. M13.3 also orders a shape
  by the shapes it overlaps, which a filter isn't given. Rejected.
- **A live effect, such as an Auto-Route effect on a group.** VectorCraft runs a group's effects on each
  path inside it alone, so the effect can't see the pieces it would join. Rejected.
- **Light filters.** Stroke to Satin's runs, which make the rails of a column drawn as one path, took up to
  233 million instructions. A light run gets 50 million and 4,000 more for each byte of its input.
  Rejected.
- **`stitch:start` and `stitch:end` as the route's ends.** They already set where the shapes of their
  group start and end, and after routing they would set it for every piece. Ink/Stitch keeps its 2 kinds
  of command apart too. Rejected.
- **`stitch route` now, writing the SVG file.** It would edit a user's Ink/Stitch file, for which
  StitchCraft has no writer. Ink/Stitch's own tools route such a file. Rejected for now.
- **`stitch route` through VectorCraft's command line,** as `stitch plan` traces a picture
  ([ADR-0015](0015-pictures-traced-by-vectorcraft.md)). It would need VectorCraft, which routing from the
  command line is meant to do without. Rejected.

## References

- Ink/Stitch at `d59c9ab`: `lib/extensions/auto_satin.py`, `auto_run.py`, `cut_satin.py` and `redwork.py`,
  `lib/stitches/auto_satin.py`, `auto_run.py` and `utils/autoroute.py`,
  `lib/elements/satin_column/satin_column.py` and `rails.py`, `lib/lettering/font.py` and
  `lib/commands.py`. Its documentation of the [satin tools](https://inkstitch.org/docs/satin-tools/), the
  [stroke tools](https://inkstitch.org/docs/stroke-tools/) and the
  [commands](https://inkstitch.org/docs/commands/).
- Ink/Stitch's fonts at `c7e3a05`: each font's `font.json`.
- VectorCraft at `c733ae97f`: `docs/plugins.md` (Heavy filters, Input, Output) and
  `crates/plugins/src/objects.rs` (`apply_output`).
- [Routing tools](../routing.md), [cut in 2](../algorithms/satin.md#cut-in-2) and
  [the routing filters](../vectorcraft-integration.md#tools-the-routing-filters).
