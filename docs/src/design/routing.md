# Routing tools

<!-- implements: crates/stitchcraft-engine/src/route/** -->

Routing joins satin columns, or running stitches, into one route with as few jumps as it can. It cuts
columns and puts runs under the stitches that come later, in an order of its own. Ink/Stitch has 2 tools
for it, Auto-Route Satin Columns ([satin tools](https://inkstitch.org/docs/satin-tools/)) and Auto-Route
Running Stitch ([stroke tools](https://inkstitch.org/docs/stroke-tools/)), and its lettering routes the
text of a font whose `auto_satin` is on. M10.4 and M10.13 build StitchCraft's routing, which sews as
Ink/Stitch's does, in the form [ADR-0017](adr/0017-routing-is-one-engine-function.md) chose. Ink/Stitch's
behaviour on this page was read in its source at `d59c9ab` ([ADR-0012](adr/0012-read-dont-copy.md)).

## Where routing runs

One function of the engine routes, and every place that routes calls it:

| Where | What it routes | From and to | Step |
|---|---|---|---|
| StitchCraft Auto-Route Satin Columns, a VectorCraft filter | The selected paths whose own effects make them satin columns or strokes | The markers selected with them, or the leftmost and the rightmost ends | M10.4 |
| StitchCraft Auto-Route Running Stitch, a VectorCraft filter | The selected paths whose own effects make them strokes | The same | M10.13 |
| Lettering, `stitch lettering` and from M14.4 VectorCraft's text objects | The text's satin columns and strokes, in a font whose `auto_satin` is on, their order kept and without trims | The first element's leftmost end and the last element's rightmost | M14.2 |
| Automatic digitizing of a ported glyph | The glyph's columns and lines | The glyph's lower left, and the point nearest the next glyph | M15.8 |
| The differential, and the test sheet TS-17C | The sheet's letters and lines | As each case places them | M10.4, M10.13 |

The filters write the route into the document ([the routing filters](vectorcraft-integration.md#tools-the-routing-filters)).
`stitch plan` routes no element it reads: it sews a design in the order the design gives, and a design that
a tool routed gives the route's order. Lettering, and automatic digitizing of a ported glyph, route the
elements they make while the design is planned. The command line routes a drawn design with `stitch route` after 1.0, once
M8.12's document writer can write the routed document ([roadmap](../plan/roadmap.md#after-10)).

## The function

`stitchcraft_engine::route`, from M10.4:

```rust,ignore
pub fn route(elements: &[Element], how: &Route, settings: &DesignSettings, budget: &Budget) -> Routed;

pub struct Route {
    pub tool: Tool,              // Satins, as Auto-Route Satin Columns routes, or Runs, as Auto-Route Running Stitch does
    pub options: Options,        // the tool's options, as its dialog sets them: tool.defaults() as it opens
    pub start: Option<Point>,
    pub end: Option<Point>,
}

pub struct Routed {
    pub pieces: Vec<Piece>,      // in sewing order, none when the route fails
    pub replaced: Vec<usize>,    // the elements routed, by their places in `elements`
    pub diagnostics: Vec<Diagnostic>,
}

pub struct Piece {
    pub element: Element,        // a column's piece, a stroke's stretch, or a run under the others
    pub from: usize,             // the element it was made from, by its place in `elements`
    pub under: bool,             // a run that the stitches sewn after it cover
}
```

It reads each element with the design's settings, as the generators read it. A satin column's rails and
rungs are read as the satin generators read them, made first for a column drawn as one path. Among satin
columns a stroke is read by its first subpath, and routing running stitches, by each of its subpaths. The
function leaves an element it can't route as it is, out of `replaced`, and `SC-I1002` says why. It writes ordinary elements, which every
host sews as it sews any. Each piece says which element it comes from, so that a host can place and name
it. The function charges its budget for every loop over pieces, and keeps them in ordered collections.
Ties go to the earlier element and to the earlier piece along it, so a route is the same on every platform
and in every host. A route that needs more work than the budget allows gives back no piece, with
`SC-E0004`, and a host then keeps the elements as they were.

The options are those of Ink/Stitch's dialogs, with its labels and defaults:

| Option | Tool | Default | What it does |
|---|---|---|---|
| Preserve order | both | off | Each element is joined to the next in the order they come in, which the route keeps |
| Trim jump stitches | both | on for satin columns, off for running stitches | The thread is trimmed after a piece that a long jump follows |
| Keep original paths | satin columns | off | The host keeps the original elements beside the route |
| Add nodes at intersections | running stitches | on | Strokes are cut where they cross, touch or pass near each other |

Each tool declares its options once beside the function, in the list `Tool::options` gives, with
Ink/Stitch's names, labels and defaults and a help of their own. The filters' dialogs are made from those
declarations, as the stitch types' dialogs are made from the registry, and so are the differential's command
lines.

## Auto-satin (M10.4)

Ink/Stitch's Auto-Route Satin Columns routes a selection in these steps:

1. **What it routes.** Satin columns, and the strokes among them, which show where the needle may
   travel. A fill, and a column or a stroke of no length, stays as it is. A stroke counts by its first
   subpath. StitchCraft also leaves as it is a shape digitized automatically, an element whose settings
   don't read, a column whose rails can't be made, and a stroke whose first subpath ends where it starts,
   which Ink/Stitch deletes unsewn (`DEV-ROUTE-003`). `SC-I1002` names each.
2. **Pieces.** A column's centre line runs through the middles of its cuts, the pairs at the zigzag
   spacing with no compensation and nothing random ([satin](algorithms/satin.md#start-and-end)). The line
   is cut into equal lengths, as few as keep each at most 1 mm long, and each is a piece. A stroke is one
   piece. The pieces' ends are where the route can go, and 2 ends at the same point are one place.
3. **Jumps.** Pieces joined end to end form groups. Without Preserve order, the jumps join the groups at
   their nearest ends, and of all such sets of jumps the one whose lengths add up to the least is taken,
   a minimum spanning tree of the groups. With it, each element is joined to the next by the shortest
   jump between their ends, which the route can take only forwards. A jump is added only where no link
   already leads from its one end to its other. Ink/Stitch's graph has one link between 2 places, so
   there its jump is that link.
4. **Ends.** The route starts at the end nearest its start, or at the leftmost end, and stops at the end
   nearest its end, or at the rightmost end. With Preserve order, the route starts at one of the first
   element's ends and stops at one of the last element's.
5. **The way.** The way with the fewest pieces and jumps from the start to the end comes first. The
   places along it are then visited from the start. At each, every piece still to sew that can be reached
   from there is sewn depth first, out along each branch and back. A column's piece passed twice is sewn as
   a running stitch along its centre line the first time, and as satin over that run the second time.
   Strokes are sewn each time the route passes them.
6. **Elements.** Pieces of one column in a row, the same way round, are one column, cut where the first
   starts and the last ends ([cut in 2](algorithms/satin.md#cut-in-2)) and turned to run the way the
   route runs. A piece ends at the cut at the same fraction of the column's pairs as its end's fraction of
   the centre line. Runs in a row along one element are one running stitch. A column's piece keeps the
   column's settings, but an odd number of centre walk repeats is lowered by 1, to no fewer than 2, so that
   the piece sews from its start ([underlays](algorithms/satin.md#underlays)). A run under a column takes the
   column's running stitch length, or else its centre walk's or its contour's stitch length, its
   shortest jump and its bean stitch repeats, where the column sets them. A stroke's stretch takes the
   stroke's.
7. **Trims.** With Trim jump stitches, a jump that runs more than 1 mm outside the 2 elements it joins
   trims the thread after the piece before it. A column counts as its rails, lines that hide only what
   runs along them, and a stroke as the convex hull of its subpaths, so a jump exactly 1 mm long between 2
   columns doesn't trim. The runs right after a trim are left out, and the route
   ends with a trim too.
8. **One column with a start and no end.** A run along its centre line from the start back to the
   column's start goes before it, and the column stays as it is.
9. **Names.** AutoSatin 1, AutoSatin Running Stitch 2, and on, counting every piece. A run that a trim
   leaves out is counted too, and no piece takes its number. Ink/Stitch names the pieces before it trims.
   With Preserve order, a piece keeps its element's name when the element has one.

StitchCraft's route is Ink/Stitch's in each of these steps, and draws the pieces otherwise in a few
places, each in the ledger:

- A piece gets a rung across its start or its end only where its rungs alone wouldn't tell its rails
  from them, where Ink/Stitch gives each a rung 0.3 CSS pixels in (`DEV-ROUTE-001`).
- A piece's rails are drawn in the column's order, so that its settings swap and turn them as they do
  the column's. With `swap_satin_rails`, Ink/Stitch's pieces sew their satin the other way round
  (`DEV-ROUTE-002`).
- A piece whose rail is shorter than the zigzag spacing is kept, where Ink/Stitch leaves the stretch
  unsewn (`DEV-ROUTE-004`), and 2 pieces that join the same 2 places are both sewn, where Ink/Stitch's
  graph keeps the one added later (`DEV-ROUTE-005`).

How the searches break ties, and which published algorithms they are, is in the module docs of
`stitchcraft_engine::route`.

## Auto-run (M10.13)

Ink/Stitch's Auto-Route Running Stitch routes a selection of strokes in these steps:

1. **What it routes.** Strokes, whatever their stroke method. Each subpath is a line, flattened within 0.1
   CSS pixels, or a manual stitch's nodes. A satin column and a fill stay as they are. StitchCraft also
   leaves as it is a shape digitized automatically, an element whose settings don't read, and a stroke of
   no length, which Ink/Stitch deletes. `SC-I1002` names each.
2. **Cuts.** With Add nodes at intersections, a line is first cut where it crosses or touches itself, into
   lines in order along it. Then each line is compared with each line after it, a stroke's own subpaths and
   their parts among them. 2 lines are cut where they cross, touch or run along each other. 2 that pass
   within 50 CSS pixels (13.2 mm) of each other without touching are each cut at its point nearest the
   other, found as the geometry library finds the first of equally near points, and lines exactly 50 pixels
   apart are cut too. Without the option, no line is cut, and lines meet only at the points they share.
3. **Pieces.** Each stretch of a line between 2 cuts is cut at its own points into straight pieces, the
   graph's links from each point to the next. The places, the jumps, the ends and the way are found as for
   satin columns (steps 3 to 5 above). With Preserve order, a stroke whose pieces don't join up, such as one
   with subpaths apart, also has its parts joined each to each, by a jump the route can take either way.
4. **Under and on top.** A piece the route passes twice is under the others the first time and on top the
   last time: the route runs out along a branch under, and back along it on top.
5. **Elements.** Pieces in a row make one element, which ends at a jump and where the route turns from on
   top to under or back. It also ends before a piece on top from another stroke, or a piece under from a
   stroke whose run under the others sews otherwise.
6. **Settings.** An element on top is a copy of its stroke with the stroke's settings, but an even number
   of repeats is lowered by 1, so that it ends at its far end. One under the others is a running stitch
   with its stroke's running stitch length and tolerance, or a manual stitch where the stroke is one,
   without repeats or bean stitch. Each has its stroke's thread and stroke, and none of its clips,
   positions or target point.
7. **Trims.** With Trim jump stitches, off as the dialog opens, a jump longer than 0.75 mm trims the thread
   after the element before it, and the elements under the others right after a trim are left out. The
   route doesn't trim at its end, but where it ends with a long jump back, that jump trims.
8. **Names.** AutoRun 1, AutoRun Underpath 2, and on, counting every element made, those a trim leaves out
   not among them.

Comparing every 2 lines costs work that grows as the square of their count, so of more than 1,000 lines,
none is cut by another, and `SC-W1004` says so. The lines are then routed as with the option off.

StitchCraft routes as Ink/Stitch does in each of these steps, and differs in a few places, each in the
ledger:

- 2 lines that cross are cut at one point, and a cut within a millionth of a millimetre of a point already
  cut or drawn is that point. Ink/Stitch cuts each line at the crossing's projection onto it, and joins 2
  lines only where its cuts agree to 16 digits, so slanted lines it cuts can miss each other
  (`DEV-ROUTE-007`).
- 2 lines that meet in several stretches, or in a stretch and at a point, are cut wherever they meet, and a
  stretch between 2 cuts is routed however short it is. Ink/Stitch cuts such lines nowhere, and leaves out
  a stretch of 0.1 CSS pixels or less (`DEV-ROUTE-008`).
- Each element has the settings and the thread of the stroke it runs along. Ink/Stitch gives a stretch on
  top that a run under the others follows the settings of the stroke that run goes along, and a run under
  several strokes the last one's (`DEV-ROUTE-006`).
- Either of 2 jumps in a row trims when it is long, where Ink/Stitch looks only at the first
  (`DEV-ROUTE-009`).
- With Preserve order, a stroke's parts are joined at the ends of their pieces, so all of them are sewn,
  where Ink/Stitch can leave a subpath apart unsewn (`DEV-ROUTE-010`). 2 pieces that join the same 2 places
  are both sewn, as for satin columns (`DEV-ROUTE-005`).

## Start and end

Ink/Stitch's `autoroute_start` and `autoroute_end` commands are tied to one of the elements selected,
and each marks the point where its symbol is. Their connectors are hidden, and the symbols can be moved
anywhere. The documentation of [commands](https://inkstitch.org/docs/commands/) asks for one of each in a
route. Auto-Route Satin Columns stops with a message when the selection has 2 of a kind, and Auto-Route
Running Stitch takes the first it finds. Before its file version 3, Ink/Stitch named them `satin_start`,
`satin_end`, `run_start` and `run_end` (`DEV-SVG-014`). The tools delete them with the elements they
replace, and StitchCraft's SVG adapter sews none of them (`REQ-SVG-003`). Ink/Stitch's Redwork, a tool
that sews every line exactly twice and that no step plans, starts at the `autoroute_start` command too.

In a VectorCraft document, from M10.4, an object named `stitch:route-start` or `stitch:route-end` marks
the route's start or end at the centre of the box round its anchors, as `stitch:start` marks a shape's
start ([commands in v1](vectorcraft-integration.md#commands-in-v1)). The markers a filter reads are those
among the paths it is given, selected with them or lying in a selected group. Each filter refuses a
selection with 2 markers of a kind, with a coded error, since a filter can't warn. It deletes the
markers it used, unless the original paths are kept. A marker without an anchor that reads marks
nothing, and stays. `stitch export` leaves route markers
out of the stitches, as it leaves out the other markers.

Lettering and automatic digitizing give the function their points themselves.

## Order

Routing doesn't start from the order M13.3 gives a design ([ADR-0017](adr/0017-routing-is-one-engine-function.md)).
Without Preserve order, a route's order comes from where its pieces lie, whatever the order they come in.
With it, the order is the one they come in: the paint order of the selection in VectorCraft, and the
text's order in lettering. M13.3 orders the automatic elements, and with `--sort-colours` every element,
by the shapes each overlaps in the whole design ([sewing order](image-to-embroidery.md#sewing-order)). A
routed design is drawn in its sewing order, which `stitch plan` keeps. `--sort-colours` moves routed
pieces as it moves the other elements with settings, and isn't meant for a routed design.

A column's piece keeps `underlays_first` with the rest of its column's settings, and a run has no
underlay. Pieces in one thread that set it sew their underlays along the route, and the needle then goes
back to the first of them for the top stitches ([underlays first](engine-pipeline.md#underlays-first)).

## Requirements

The routing's requirements are an area of their own, `REQ-ROUTE-…`, and the filters' are among
`REQ-VC-…`:

| Requirement | What it says | Step |
|---|---|---|
| `REQ-ROUTE-001` | A column is cut into equal pieces at most 1 mm long along its centre line, each sewing the pairs the column sews along it | M10.4 |
| `REQ-ROUTE-002` | A stretch passed twice is sewn first as a run along the centre line, under the satin that sews over it last | M10.4 |
| `REQ-ROUTE-003` | Without Preserve order, the jumps are a minimum spanning tree of the groups. With it, each element is joined to the next | M10.4 |
| `REQ-ROUTE-004` | A route starts and stops at the ends nearest its start and end, or else at the leftmost and the rightmost | M10.4 |
| `REQ-ROUTE-005` | With Trim jump stitches, a jump more than 1 mm outside the elements it joins trims | M10.4 |
| `REQ-ROUTE-006` | A route is the same each time, and one that runs out of work leaves the elements as they were, with `SC-E0004` | M10.4 |
| `REQ-ROUTE-007` | What a route takes and leaves, and how its pieces are named | M10.4 |
| `REQ-ROUTE-008` | TS-17C's letters, routed by Ink/Stitch's tool and by the function, sew alike by every measure of the differential | M10.4 |
| `REQ-ROUTE-009` | Auto-run routing takes the strokes, each subpath a line, and leaves the rest as they are | M10.13 |
| `REQ-ROUTE-010` | Lines are cut where they cross, touch or run along each other, and at their nearest points within 50 CSS pixels, with Add nodes at intersections | M10.13 |
| `REQ-ROUTE-011` | A piece passed twice runs under the others the first time, and each element has its stroke's settings, a run under the others its lengths alone | M10.13 |
| `REQ-ROUTE-012` | The pieces are routed as satin columns' are, a jump longer than 0.75 mm trims with Trim jump stitches, and the elements are named AutoRun and AutoRun Underpath | M10.13 |
| `REQ-ROUTE-013` | TS-17C's network of lines, routed by Ink/Stitch's tool and by the function, sews alike by every measure of the differential | M10.13 |
| `REQ-VC-018` | The Auto-Route Satin Columns filter replaces the selection with the route in one undo step, within a heavy filter's budget | M10.4 |
| `REQ-VC-019` | Both filters read the route markers selected with the paths, and refuse 2 of a kind | M10.4, M10.13 |
| `REQ-VC-020` | The Auto-Route Running Stitch filter replaces the selected strokes with the route in one undo step, within a heavy filter's budget | M10.13 |
