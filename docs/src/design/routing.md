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
    pub tool: Tool,              // Satins, as Auto-Route Satin Columns routes; Runs comes with M10.13
    pub options: Options,        // the tool's options, as its dialog sets them
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
    pub under: bool,             // a run that a column's satin, sewn after it, covers
}
```

It reads each element with the design's settings, as the generators read it. A satin column's rails and
rungs are read as the satin generators read them, made first for a column drawn as one path, and a stroke
is read by its first subpath. An element it doesn't take stays as it is, out of `replaced`, and `SC-I1002`
says why. It writes ordinary elements, which every
host sews as it sews any. Each piece says which element it comes from, so that a host can place and name
it. The function charges its budget for every loop over pieces, and keeps them in ordered collections.
Ties go to the earlier element and to the earlier piece along it, so a route is the same on every platform
and in every host. A route that needs more work than the budget allows gives back no piece, with
`SC-E0004`, and a host then keeps the elements as they were.

The options are those of Ink/Stitch's dialogs, with its labels and defaults:

| Option | Tool | What it does |
|---|---|---|
| Preserve order | both | Each element is joined to the next in the order they come in, which the route keeps |
| Trim jump stitches | both | The thread is trimmed after a piece that a long jump follows |
| Keep original paths | satin columns | The host keeps the original elements beside the route |
| Add nodes at intersections | running stitches, on by default | Strokes are split where they cross or pass near each other |

Each option is declared once beside the function, in `OPTIONS`, with Ink/Stitch's name, label and default
and a help of its own. The filters' dialogs are made from those declarations, as the stitch types' dialogs
are made from the registry, and so are the differential's command lines.

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
   jump between their ends, which the route can take only forwards.
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
9. **Names.** AutoSatin 1, AutoSatin Running Stitch 2, and on, counting every piece. With Preserve order,
   a piece keeps its element's name when the element has one.

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

Ink/Stitch's Auto-Route Running Stitch routes a selection of strokes the same way, with these
differences:

1. **What it routes.** Strokes of every stroke method. Each subpath is a line, cut where it crosses
   itself.
2. **Splits.** With Add nodes at intersections, each line is also cut where it crosses or touches
   another. 2 lines that pass within 50 CSS pixels (13.2 mm) of each other without touching are cut where
   they come nearest. A stretch shorter than 0.1 CSS pixels is left out. Without the option, each subpath
   is one line, cut only where it crosses itself.
3. **Pieces.** Each stretch between 2 points of a line is a piece. Pieces meet at the points they share.
   The jumps, the ends and the way are found as for satin columns.
4. **Elements.** Pieces in a row make one element, until a jump comes, the run goes from on top to under
   or back, or a piece on top comes from another stroke. A piece passed twice is under the first time and
   on top the second time.
5. **Settings.** An element on top keeps its stroke's settings, but an even number of repeats is lowered
   by 1, so that it ends at its far end. Under the others, an element is a running stitch with the
   stroke's stitch length and tolerance, without repeats or bean stitch, and a manual stitch where the
   stroke was one.
6. **Trims.** With Trim jump stitches, a jump longer than 0.75 mm trims the thread after the element
   before it, and the elements under the others right after a trim are left out.
7. **Names.** AutoRun 1, AutoRun Underpath 2, and on, counting every element.

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
among the paths it is given, selected with them or lying in a selected group. The filter refuses a
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
| `REQ-VC-018` | The Auto-Route Satin Columns filter replaces the selection with the route in one undo step, within a heavy filter's budget | M10.4 |
| `REQ-VC-019` | It reads the route markers selected with the paths, and refuses 2 of a kind | M10.4 |

M10.13 adds the requirements of Auto-Route Running Stitch. Strokes are split where they cross, and where
2 pass within 13.2 mm of each other unless the option is off. A run under the others sews without repeats
or bean stitch, and a stretch on top keeps its settings, an even number of repeats lowered by 1. A jump
longer than 0.75 mm trims.
