# VectorCraft integration

<!-- implements: apps/stitchcraft-vc-plugin/src/**, crates/stitchcraft-vectorcraft/src/** -->

How StitchCraft lives inside [VectorCraft](https://github.com/storytold/vectorcraft): what VectorCraft
offers today, how we use it without changing VectorCraft (phase 1), what we propose upstream
(phase 2), and how an in-tree integration would look (phase 3).

**VectorCraft snapshot reviewed:** `main` at `89ea46c` (2026-10-08), version 0.6.0, and its plug-in API
again at `8bf090b` (2026-10-24), after 0.16.0. File references below are to the first unless a version is
named. The [compatibility gate](compatibility-gate.md) tests the plug-ins against each VectorCraft
release and branch.

## 1. VectorCraft in brief

| Fact | Detail | Where |
|---|---|---|
| What | Clean-room reimplementation of Illustrator in pure Rust; desktop (macOS, Windows, Linux, FreeBSD) and web (wasm) | `README.md` |
| Licence | MIT OR Apache-2.0; no GPL code allowed in the tree | `Cargo.toml`, `AGENTS.md` |
| Toolchain | Rust 2024, `rust-version = 1.95` | `Cargo.toml` |
| Layers | `geom`, `color` L0 → `doc` L1 → `pathops`, `brush`, `trace`, `text`, `effects`, `plugins` L2 → `render`, `svg`, `pdf`, `format`, `cad`, `eps`, `metafile` L3 → `tools` L4 → `engine` L5 → `ui-egui`, `mcp` L6 → apps | `xtask/src/layers.rs` |
| Commands | Declarative `cmd!` registry: id, label, menu path, shortcut, params doc, `enabled`, `run`; reachable from UI, JSON control channel and MCP | `crates/engine/src/cmd/*` |
| Headless CLI | `vectorcraft-cli run [--in FILE] [--cmd ID [--params JSON]]... [--export FILE]...`, `convert`, `info`, `commands`, `mcp` | `apps/vectorcraft-cli/src/main.rs` |
| Control channel | JSON lines; `engine.execute`, `ui.menu.invoke`, `ui.dialog.set`, `ui.click`, `ui.screenshot {path?}` (needs a presented frame), `ui.render {path?, scale?}` (headless artboard render) | `crates/ui-egui/src/control.rs` |
| Releases | Tags `v0.1.0` … `v0.16.0`. Pushing the `release` branch drafts a GitHub Release, and SemVer pre-releases (`-rc.1`) are flagged as pre-releases | `.github/workflows/release.yml` |

## 2. Extension points that exist today

### Plug-in ABI v1 (`crates/plugins`, `docs/plugins.md`)

- **Sandbox:** a core WebAssembly module run by `wasmi` 2.0, **no imports at all** (no WASI, clock,
  file system or randomness). Exports: `memory`, `vc_abi_version() → 1`, `vc_manifest()`,
  `vc_alloc(size)`, `vc_run(input, len, params, len)`. Input and output are JSON.
- **Kinds:** *object filters* (read selected paths and compound paths — geometry, fills, strokes,
  opacity — and return updated or new objects; one undo step) and *live effects* (rewrite one object's
  geometry every time it is drawn, cached by plug-in, parameters and input geometry). From 0.14,
  *exporters* write the whole document in a file format of their own, listed by File › Export › Export As.
- **Parameters:** manifest schema with `number`, `int`, `bool`, `choice`; at most **64 parameters**
  (`MAX_PARAMS`, `crates/plugins/src/manifest.rs:11`) and a 64 KiB manifest; no labels, units, groups or
  conditions until 0.14, and the dialog shows the parameters' names. From 0.14: `string` and `numberList`
  too, up to 256 parameters with at most 64 outside groups, and each parameter's `label`, `unit`, `group`,
  `help`, `helpUrl` and `visibleWhen`.
- **Limits** (`crates/plugins/src/runtime.rs`): module ≤ 32 MiB; memory ≤ 512 MiB; output ≤ 64 MiB JSON;
  **fuel 50 M instructions + 4,000 per input byte**; wall clock 60 s per filter run and **5 s per
  live-effect run** (`crates/plugins/src/effect.rs`), with a fresh instance for each call. An export gets
  10 G instructions and 300 s (0.14). On `main` after 0.16, a filter that declares `"budget": "heavy"` gets
  1 G and 300 s, once the user allows it at install.
- **Live-effect input** is the object's geometry: `type`, `path`, `fillRule`, `bounds`. From 0.14 it
  holds the object's `fills` and `strokes` too, read only, each stroke with its width, and from 0.15 its
  join, cap and miter limit. `_context.item` says whether the effect is on the object or on one of its
  paints. The objects beside it are still not given.
- **Filters** are given each stroke's join, cap and miter limit and each object's effect records, and may
  return effect records (0.15). From 0.16 a filter keeps up to 1 MiB of data with the document
  (`documentData`).
- **Persistence:** applying a live effect stores `{"id": "plugin.<id>", "params": {…}}` in the object's
  appearance (`Effect { id, params: serde_json::Value, visible }`, `crates/doc/src/appearance.rs`).
  Documents keep the record when the plug-in is missing and draw the object unchanged.
- **Not in v1:** tool plug-ins, panel plug-ins, host callbacks (progress, cancel), importers, live
  effects that change paint. File-format plug-ins came with the exporters (0.14).

### Other facts that shape the design

- **No generic per-object metadata.** `Node` has `attrs: Option<Box<ObjectAttributes>>` with only
  centre display, image map, URL and note (`crates/doc/src/node.rs`). The effect record is therefore the
  right place for embroidery parameters.
- **Document-level foreign data** lives in `Document.unknown` (used for the perspective grid and
  brushes), preserved on round trips (`crates/doc/src/lib.rs`). From 0.16 the data filters keep with a
  document is there too, under `pluginData`.
- **SVG import keeps only the attributes it understands.** The importer looks attributes up by name
  (presentation attributes, references, names such as `inkscape:label`) and has no place to keep unknown
  ones (`crates/svg/src/import.rs`), so an Ink/Stitch SVG opened in VectorCraft up to 0.16 loses its
  `inkstitch:*` parameters. VectorCraft's `main` keeps attributes in other applications' namespaces after
  0.16, from SVG import to SVG export, and reads none of them. SVG export can embed the whole native
  document (`vectorcraft:document`) when editing data is preserved.
- **Export formats are a static table** (`FORMATS` in `crates/engine/src/cmd/fileio/mod.rs`). From 0.14 an
  exporter plug-in adds a format to it, `plugin.<id>`, which is how StitchCraft can add PES.

## 3. Phases

| Phase | Needs from VectorCraft | Delivers | Milestone |
|---|---|---|---|
| 1. ABI v1 plug-ins | nothing | per-object embroidery settings, previews, tools; machine files via `stitch export file.vectorcraft` | M6 |
| 2. Plug-in features (upstream RFC) | generic plug-in features, most of them in VectorCraft 0.14 to 0.16 | previews as wide as a stroke, export from File › Export, labelled dialogs, design settings and a machine per document, and the tools filter (M6.8 to M6.12). Overlay previews of every stitch, and heavy live effects, after 1.0 (M9) | M6, M9 |
| 3. In-tree crate (optional) | ArtCraft's interest, or a fork | native speed, simulator panel, satin tool | post-1.0 |

## 4. Phase 1 design (ABI v1, no VectorCraft change)

### Plug-ins

One `.wasm` per stitch family keeps each dialog relevant and under 64 parameters, which every VectorCraft
release takes:

| Plug-in id | Kind | Stitch types | Notes |
|---|---|---|---|
| `<ns>.stroke` | live effect | running, bean, manual (P1), zigzag (M7.2), ripple (later) | `stroke_method` choice selects the variant |
| `<ns>.satin` | live effect | satin (P1), E (M7.3), S (M7.4), zigzag (M7.5) | rails and rungs from the object's compound path |
| `<ns>.fill` | live effect | tatami (P1); contour, meander, circular (P2) | `fill_method` choice |
| `<ns>.auto` | live effect | the stitches automatic digitizing chooses by width (M13.7) | on a group, previews each path inside ([image to embroidery](image-to-embroidery.md)) |

The tools an object filter was to give are VectorCraft's own commands, and the rest come with M6.12's tools
filter ([tools](#tools-vectorcrafts-own-commands)).

`<ns>` is a reverse-DNS namespace defined in one constant (`PLUGIN_NAMESPACE`) in the VectorCraft adapter,
which reads the effect records too. The adapter makes the ids from it, a plug-in's with `plugin_id` and its
live effect's with `effect_id`, and `effect_name` reads one back. The plug-in crate and the compatibility
gate use the same functions. A live effect's id ends with its stitch family's
(`stitchcraft_params::Family::id`).
Manifests are generated from the [parameter registry](params.md#splitting-for-vectorcraft-abi-v1); a
test validates each against VectorCraft's manifest rules (id charset, ≤ 64 parameters, names, ranges) so
an invalid manifest never reaches a user.

**Modules.** The plug-in crate builds one module for each family, chosen by a Cargo feature named after
the family: `stroke`, `satin` or `fill`. Without one it builds M0.6's *hello* effect. The modules contain
the same code, which the native tests run whichever it is, and the feature only picks the manifest and
the effect that `vc_manifest` and `vc_run` serve. `cargo xtask compat contract` builds the four modules,
notes their sizes and loads them into VectorCraft's host.

**Manifests.** A family's manifest is made from the registry when VectorCraft asks for it, so a parameter
added to the registry shows in the dialog without another change. A key that several families declare,
each its own way, is in each family's manifest as that family declares it
([parameter registry](params.md#naming-rules), rule 7). VectorCraft's parameters are numbers,
whole numbers, toggles and choices, and each always has a value. VectorCraft 0.14 adds texts and lists of
numbers, which older releases refuse. A record stores every declared
parameter, at its default when the user leaves it. So a parameter is declared when one such value means
what the parameter means:

- a length, an angle, a percentage or a number as a number in its range, and a count as a whole number;
- a list or a pair whose default is one value as that value, which stands for the whole list or both
  sides, as in Ink/Stitch;
- an optional length that 0 leaves unset as a number from 0.

Texts, seeds, and settings left empty to take another's value keep their defaults. In the registry today
they are the custom locks' steps, `random_seed`, a fill underlay's angles, a satin's zigzag underlay
insets and its shortest random split. A choice leaves out the options that a text spells out, which are
the custom locks, and `fill_method` leaves out Ink/Stitch's legacy fill, which StitchCraft sews as a
tatami fill (`REQ-FILL-005`). `satin_column` belongs to the effect, which sets it from its family. The dialog opens
with the group that declares the family's method. The groups only the family uses, for its default
method, come next, then those it shares with the other families, then those of its other methods, such as
the zigzag stroke's. The settings every element shares come last, as in Ink/Stitch's dialogs.

**What a live effect shows.** A family's effect sews its object with the engine's `generate`, on the
reference machine and with no neighbours, as one object of a design. It returns the stitches as geometry,
the exact level below. When it can't show stitches, it gives back why. VectorCraft then draws the object
as it is and shows the message as the plug-in's last error. That happens when the object cannot be sewn or
sews no stitch, with the message that says why, and for a satin column drawn as one path, which is as
wide as its stroke, with `SC-I0810`. A zigzag stroke is as wide as its stroke too, and the effect leaves it
as it is, with `SC-I0810`. VectorCraft gives a live effect its object's fills and strokes from 0.14, and
M6.8 will show both as wide as their stroke. A drawing that vanished instead could not be clicked to change
the setting that emptied it. ABI v1 has no way to pass VectorCraft the warnings about stitches the effect
shows, and `stitch export` reports them. How much of the stitches it shows, within what VectorCraft lets
it spend, is its preview level (below).

### The workflow

1. Draw the art in VectorCraft as usual.
2. Select an object → **Effect › Plug-ins › StitchCraft Fill** (or Stroke / Satin) → set parameters in
   the generated dialog. The parameters are now stored on the object (undoable, saved in the
   `.vectorcraft` file) and the object shows a stitch preview.
3. Save the document.
4. `stitch export design.vectorcraft -o design.pes --preview design.png` writes the machine file, for the
   machine `--profile` names (`brother-pe800-5x7` by default), and a picture of what will sew. It says
   what the reader left out or changed, with the engine's warnings. `stitch export` is another name for
   `stitch plan`, which reads a VectorCraft document (`.vectorcraft`, `.drawcraft`, or a `.vctemplate`
   template) as it reads an SVG file (`REQ-VC-008`), and `--report` adds the JSON report. A bug-report
   bundle includes the part of the document the reader reads, which is its JSON without the images after
   it. Its replay reads that part the same way.

Stitching order is paint order (bottom first). Hidden objects are skipped, as VectorCraft draws nothing
for them, and locked objects are stitched. Thread colours come from the object's fill for a fill, and
from its stroke for a stroke or a satin column, as in Ink/Stitch, mapped to the profile's palette.

### Tools: VectorCraft's own commands

The plan had one object filter, `<ns>.tools`, to make a satin column from 2 paths, add a rung, bake a
preview to paths and draw a test sheet. VectorCraft's own commands do the first three, and M6.4 checked
them in its source, 0.7.0 to 0.12.0. Make Compound Path changed at 0.10.0. Before, the compound path takes
the appearance of the frontmost path it is made of and turns none round. From 0.10.0, it takes the
backmost path's, and reverses each path in front that winds the same way as the backmost one. A
straight path counts as winding clockwise, so of 2 straight rails the front one is always reversed. So
the StitchCraft effect is applied after the paths are joined, which works in every version:

| Task | In VectorCraft | Why it sews |
|---|---|---|
| Satin column of 2 paths | Object › Compound Path › Make (`object.compoundPath.make`), then the satin effect | The column turns a second rail that runs the other way back (`reverse_rails`, automatic), as Ink/Stitch does (`REQ-VC-007`). The reversal also starts a ring at what was its last node, and of 2 rings the second starts at its node nearest the first's start (`REQ-SAT-019`) |
| Rung | Pen tool line across the rails, then Make Compound Path with the column | Which way the rung runs, and where among the paths it lies, doesn't change the points it pairs. Before 0.10.0 the compound takes the rung's appearance, and the effect is applied again |
| Baked preview | Object › Expand Appearance (`effect.expandAppearance`) | It runs the live effect and makes objects of what each fill and stroke paints, strokes outlined. Those are the preview's shapes, which sew differently from the effect. Bake a copy to show, and sew the original |
| Presets | Graphic Styles (`graphicStyle.new`, `graphicStyle.apply`) | Applying a style copies its effects into each object, or into the group it is applied to, whose effects apply to each path inside (`REQ-VC-006`) |

**Convert Line to Satin** (Ink/Stitch's `stroke_to_satin`) is M6.12's. It makes a column drawn as one path
into 2 rails and rungs, as the engine makes them when it sews one. A filter is given the stroke's width,
and from VectorCraft 0.15 its join, and may return the column with its satin effect. M6.4 measured the
conversion inside VectorCraft's host, as a live effect with the same limits as a filter:

| Line, 3 mm wide | Instructions |
|---|---|
| A zigzag of 10 corners | 17 million |
| An S curve 30 mm long | 51 million |
| A circle 10 mm across | 68 million |
| A circle 30 mm across | 72 million |
| An S curve 60 mm long | 77 million |
| A circle 50 mm across | 162 million |

A light run gets about 52 million, and almost every curved line would run out. The rails are the line
offset as GEOS offsets it, and each offset builds the boundary of the whole stroke, once for each rail. A
heavy filter gets 1 G, which VectorCraft's `main` adds after 0.16, and M6.12 waits for a release with it.

**A test sheet** drawn into the document is M6.12's too. Up to 0.14 a filter's output held geometry, paints,
opacity and names, and never an effect record, and the objects it drew would not sew as a test sheet. From
0.15 a filter may return effect records. Until M6.12, `stitch testsheet` makes the sheets.

### Live-effect previews within v1 budgets

A live effect gets 50 million instructions, plus 4,000 for each byte of its input, and 5 seconds. Spike
M0.6 measured what JSON costs in VectorCraft's host (`wasmi` 2.0, in VectorCraft 0.12.0 and on `main` at
`8bc6532`, 2026-10-17). The *hello* plug-in reads its input with `serde_json` and writes the geometry
back:

| Measured | Value |
|---|---|
| Reading JSON | 512 instructions a byte |
| Writing JSON from a `serde_json` value | 822 instructions a byte |
| One anchor of a path, as JSON | 40 bytes |
| Speed on GitHub's Linux runners | about 1.9 billion instructions a second |

The instruction budget runs out long before the clock: 50 million instructions take about 26 ms. Reading
a path uses an eighth of the 4,000 instructions each of its bytes brings. M6.3 measured the plug-ins
themselves in the host, on the shapes the contract tests run, with the same numbers in VectorCraft 0.12.0
and on `main`:

| Measured | Value |
|---|---|
| Writing the plug-ins' geometry, by hand, to a thousandth of a point | about 16 instructions a byte |
| One anchor, written by hand | 18 to 20 bytes along a stroke or a satin column, 26 to 27 in a fill's ribbons |
| A unit of the engine's work for a stroke | about 880 instructions |
| A unit of the engine's work for a satin column | about 4,730 instructions |
| A unit of the engine's work for a fill | 1,900 to 5,300 instructions, by the part of the work |
| The exact preview of a 7 mm square fill with its defaults | 27 million instructions, 85 % of the engine's work spent on the travel under its rows |
| The rows preview of square fills from 3.5 mm to 226 mm | 1.6 to 11 million instructions |
| The outline preview of square fills from 3.5 mm to 903 mm | 1.7 to 5 million instructions |

So the engine's work, and not the JSON, is what limits a preview. The plug-in turns the instructions it
has left after reading its input into a budget of the engine's work units, and prices what it writes by
the anchor. It shows less when the engine runs out, or when what it would write costs more than its
share, instead of being stopped (`apps/stitchcraft-vc-plugin/src/preview.rs`):

| Level | Geometry returned | Auto shows it |
|---|---|---|
| Exact | The stitch polyline (strokes, satins) or one thin closed ribbon per stitch (fills) | For a stroke or a satin column, within nine tenths of the budget |
| Rows | One ribbon per row segment of a fill's top rows: where its stitches lie and which way they run | For a fill, within eight tenths |
| Outline | The object's path as lines for a stroke or a satin column, and as ribbons for a fill, which are hatched every 3 mm in its direction | When the others cost too much |

The `preview_level` parameter, `auto` by default, lets the user pick a level (VectorCraft reserves the
name `preview`). A level the user picks gets the whole budget, and when that is not enough, `SC-I0810`
names a cheaper one. The units of a stroke's work and of a satin column's cost the same on every shape
measured, so a budget of units bounds their exact previews. A fill's units vary too much for that. A 7 mm
square's exact preview already takes more than half of what VectorCraft allows, and a budget of units that
ran out early, in the dearer work, took 71 million instructions. So auto shows a fill's rows, and its exact
preview is the user's to pick, with all that VectorCraft allows, which stops one that needs more with its
own error. The
contract test `req_vc_003_every_preview_level_finishes_within_vectorcraft_s_limits` runs each level on
shapes of growing size on every track the [compatibility gate](compatibility-gate.md) tests, and fails
when VectorCraft stops a run that must finish. It prints what each run takes, which the budget's numbers
are measured from. `hello_reads_and_writes_json_within_the_recorded_budget` still measures what JSON
costs. The real stitches are always available from the CLI preview, and the RFC's overlay channel (P3, in
review upstream) would remove this limitation.

Paint caveat: a live effect changes only geometry. A fill preview's ribbons are painted with the object's
own fill, and a stroke preview with its stroke. From 0.14 the effect reads the paints but still cannot
change them. Users set the fill/stroke to the thread colour, which they want anyway for export.

### Commands in v1

Ink/Stitch's commands are symbols linked to an object, and ABI v1 has nothing to link. `trim_after` and
`stop_after` are ordinary parameters. StitchCraft reads the others from names, an object's or a layer's,
which VectorCraft's Layers panel sets (`REQ-VC-009`). A name is read when it begins with `stitch:`,
after any white space, in any mix of upper and lower case. Its word ends at the first white space, so a
note can follow it:

| Name | Ink/Stitch's command | What StitchCraft does |
|---|---|---|
| `stitch:ignore` | `ignore_object`, `ignore_layer` | Leaves the object or the layer out with everything in it, and lists it with `SC-I0811` |
| `stitch:start`, `stitch:end` | `starting_point`, `ending_point` | Marks where the fill or the satin column grouped with it starts or ends, and never stitches the marker. Until the generators start and end at hints (`REQ-GEN-001`, M8), they choose where, and `SC-W0807` names the marker. A group or a layer is never a marker: `SC-W0807` names it, and what it holds is read as under any other name |
| Any other word after `stitch:` | | Ignores the name, and `SC-W0807` names the object, in case the word is a misspelt marker |

Hiding an object leaves it out too, without a word, and a name on a hidden object says nothing. A live
effect is given its object's geometry, with its fills and strokes from VectorCraft 0.14 (VectorCraft's
`docs/plugins.md`, Input), never its name. So
VectorCraft's preview still shows the stitches of an object these names leave out, a marker included,
when it has a StitchCraft effect or is in a group that has one. `stitch export` leaves them out. The RFC's
sixth proposal, still open in that part, would let a live effect read the markers beside its object, so
its preview could start and end at them.

### Reading `.vectorcraft` files (`stitchcraft-vectorcraft`)

`stitchcraft_vectorcraft::read` turns a `.vectorcraft` file into a design, and reports what it left out
or simplified, as the SVG adapter does for SVG files. It reads the format as VectorCraft documents it
(`crates/format` and the types `crates/doc` saves) and links none of VectorCraft's crates. VectorCraft
versions 0.7.0 to 0.12.0 were read for this design.

**The container.** A file is JSON, compressed with gzip when its first two bytes are `1F 8B`. VectorCraft
compresses from format 3 on, but the adapter reads any version either way. In format 4, images and other
large data can follow the JSON, behind the marker `\n\0VCBLOBS\0`. JSON text never holds a NUL byte. The
JSON ends at the first one, and nothing after the marker is read. The top level must say `"format":
"vectorcraft"`, or `"drawcraft"`, VectorCraft's earlier name, and hold a `document`. Formats 1 to 4 read
alike: points as `{"x", "y"}` or `[x, y]` in any of them. A newer format is read as far as it goes, with
`SC-W0803`, where VectorCraft's own readers refuse it.

**Limits.** At most 64 MiB of JSON, unpacked, as for an SVG file, and the command line reads at most 64
MiB of any file (`REQ-CLI-005`), images included. A gzip stream is unpacked only up to the marker, and no
further than the limit. Nesting deeper than the JSON reader allows (128 levels, about 40 levels of
groups) refuses the file. VectorCraft itself keeps 32. The JSON is read into the few things the adapter
uses, and the rest is skipped as it is read: images, previews and text runs are never kept. Every object,
appearance item, effect and anchor charges the budget, and so does each element for its copy of the
object's path.

**Coordinates.** VectorCraft keeps every path in document coordinates. Moving or scaling a group moves
its paths' anchors, so layers, groups and compound paths have no transform. Coordinates are points, 1/72
inch, with y down, and the adapter multiplies them by 25.4/72 into millimetres. The document's origin
stays the origin. Coordinates are read exactly, as VectorCraft reads them, from their text in the JSON.
An anchor's handles are absolute positions. A missing handle, or one within a billionth of a point of its
anchor, leaves that end of the segment straight, and a segment without handles is a line.

**What is stitched.** The layers in order, bottom first, and in each its children in paint order:

- **Paths and compound paths** are the art. A compound path's children are one shape, with the compound's
  appearance and fill rule.
- **Groups and layers** pass their children on, and their StitchCraft effects with them (below). Clipping
  is left for M8, as in the SVG adapter: a clipping group's or layer's art is stitched whole, with
  `SC-W0807`.
- **Hidden objects**, and everything in them, are left out without a word, as VectorCraft draws nothing
  for them. So are objects and paints of opacity 0, paints turned off and strokes of width 0.
- **Objects and layers named `stitch:ignore`**, and everything in them, are left out with `SC-I0811`.
  Start and end markers are never stitched ([commands](#commands-in-v1)).
- **Template layers, layers that don't print, and guides** are left out with `SC-I0809`. VectorCraft
  shows them, dimmed or as guides, but prints and exports none of them.
- **A Shaper group's sources** are left out. The group shows the shapes it makes of them, and keeps them
  as its first child, transparent. The Layers panel's Make Clipping Mask (`layer.clippingMask.toggle`)
  puts a Shaper group's clipping object before them.
- **Text, images and symbols,** and the objects VectorCraft computes as it draws them (blends, envelopes,
  meshes, repeats, placed documents and VectorCraft's Intertwine objects), are left out with `SC-W0807`.
  The message names VectorCraft's command that makes paths of them, such as Type › Create Outlines for
  text. So is a kind of object this version doesn't read.

**Paints and threads.** An object without StitchCraft's effects is stitched as Ink/Stitch stitches an SVG
shape. Its visible fills are filled, in the appearance's order, and its visible strokes then have a running
stitch each. Since M13.1, read with `--auto` (`stitchcraft_vectorcraft::read_with`), the elements of an
object are automatic instead when neither it nor a group or layer around it has a StitchCraft effect
([image to embroidery](image-to-embroidery.md), `REQ-AUTO-001`). StitchCraft then chooses their stitches by
their width. A stroke's cap is read with its width and join, and an automatic line's stitches follow its
round or square caps. Of a line sewn across its width, the reader names the alignment inside or outside a
closed path and the width profile, which its stitches don't follow (`SC-W0807`, `REQ-AUTO-005`). The
thread's colour is a solid paint's colour, with CMYK and grey converted by
VectorCraft's formula without colour management ((1 − c)(1 − k) and 1 − k) and Lab through sRGB. A
gradient gives its first stop's colour, with `SC-W0807` unless every stop has that colour, and a pattern
gives black, with `SC-W0807`. A stroke's width, in points, and its join, with its miter limit (10 when
the file omits it, as VectorCraft draws it), shape a satin column drawn as one path. A stroke drawn with
a brush is stitched along its path, without the brush's art, with `SC-W0807`.

What VectorCraft draws of a stroke that its stitches don't follow is named with `SC-W0807`, and the
message says how to stitch it, usually with Object › Path › Outline Stroke:

- A stroke's arrowheads are not stitched, and a dashed stroke is stitched along all of its path. A dash
  pattern that VectorCraft draws solid, with a length below 0 or that is not a number, or only 0s, is
  no dash.
- A stroke painted below a fill is sewn after the fill, over it.
- A satin column or a zigzag made of a stroke is as wide as the stroke all along and centred on the
  path. VectorCraft draws a closed path's stroke inside or outside it when it is aligned so, and varies
  its width along a width profile. It also caps an open path's ends with round or square caps. A running
  stitch along the path is the same whichever way the stroke is drawn.

A group's or a layer's own
fills and strokes, which paint every member, are not stitched as shapes of their own. They give
StitchCraft effects their thread and stroke (below), with `SC-W0807`.

**Effects.** StitchCraft's live effects are kept in the object's appearance as `{"id":
"plugin.dev.stitchcraft.<family>", "params": {…}, "visible": true}`. The family is `stroke`, `satin` or
`fill` (`stitchcraft_params::Family`). An object with visible StitchCraft effects is stitched as they
say, in their order, instead of by its paints. A `fill` effect fills the object's shape, a `stroke`
effect stitches along it and a `satin` effect makes a satin column of it. With this rule, an object sews
what its effects say and nothing else. Each effect is sewn on the shape as it is drawn. VectorCraft
draws an effect after the first on the stitches of the one before, though, so the preview of an object
with 2 is not what is sewn, and `SC-W0807` says so, once for an object and once for a group. A fill with
an outline takes 2 objects: the shape with a `fill` effect, and a copy pasted in front of it (Edit › Paste
in Front) with a `stroke` effect. The record's parameters are the element's. A number is written as a decimal, a toggle as `true`
or `false`, a choice as its id and a list as its values with spaces between. What the record leaves out
takes its default, as in the plug-in's dialog. The thread is the colour of the topmost fill VectorCraft
draws for a fill, and of the topmost stroke for the others, falling back on the other kind. An opaque
paint comes before one of opacity under 1, which lets what is under it show. A `stroke` or `satin` effect
follows the width and join of that stroke, or a width of 0 when none is drawn. VectorCraft writes a
stroke of paint none for an object without one, which it doesn't draw. A satin column too narrow to
stitch across is sewn as a stroke, with `SC-W0212`. `SC-W0807` names a zigzag 0 mm wide. VectorCraft's own effects change what an object
draws, and are left out with `SC-W0807` (Object › Expand Appearance makes paths of what they draw).

**A group's effects** (`REQ-VC-006`). VectorCraft runs a group's or a layer's geometry effects on each
path and compound path inside it, however deep, with the group's bounds as their frame (`reshape_leaves`
in `crates/effects`, 0.7.0 to 0.12.0), and paints what they return with the path's own paints. The
group's own paints paint the shapes inside it as its effects and those of the groups around reshaped them
(`evaluate_container`). They go over its contents. The Contents row of its Appearance panel
(`contents_index`) is last in the list by default, and the paints listed under it go under the contents.
StitchCraft's live effects are geometry effects, and applying a graphic style to a group puts its effects
and paints on the group. So each path inside sews as the group's StitchCraft effects say, and as the
preview shows it. A name can leave one out ([commands](#commands-in-v1)). Its thread and stroke come from
the paints over it, the topmost first, as for an object's own paints. The groups' paints over their
contents come first, the outermost first, then the object's own, then the groups' paints under their
contents, the innermost first. The same order applies to an object's own effects, which VectorCraft
paints with its own paints, under the groups' paints over the object's shape. A path that neither its own
paints nor a group's paint is invisible, and is left out without a word.

A clip group's first object is its clipping object. VectorCraft paints a path, a compound path or text
there with its own paints alone (`clip_paint` in `crates/doc`): its fills under the clipped art and its
strokes over it. A group's effects run on such a path too. After Make Clipping Mask it has no paint, and
sews nothing. VectorCraft draws nothing of a group or an image there, which an imported file can have,
and neither is sewn. Where StitchCraft effects meet from more than one level, a path's own and a group's,
or 2 groups', VectorCraft draws one effect run on the stitches another drew, which a machine cannot sew.
The path sews as the nearest effects say, and `SC-W0807` names it.

**Ids.** An element is `vc:<object id>:<what>`: `fill` or `stroke` for a paint, with its rank among the
object's fills or strokes from the second on (`fill.2`), and the family for an effect. VectorCraft keeps
an object's id when it saves, so a report and a preview name the same object. A repeated id gets `~2`,
`~3`, … as in the SVG adapter.

**Reports.** `SC-E0806` refuses a file that isn't a VectorCraft document, or is larger than the adapter
reads. `SC-W0807` names what isn't stitched. `SC-W0808` names geometry that can't be used, such as a
number that doesn't read or a point more than 10 m from the origin, and a shape that draws nothing.
`SC-I0809` lists the layers and guides left out because VectorCraft doesn't print them, and `SC-I0811` the
objects and layers left out by their names. The compatibility gate will have each VectorCraft version it
tests write files for the adapter to read back (Level B, M6.6).

### Importing Ink/Stitch SVGs into VectorCraft

Because VectorCraft up to 0.16 drops `inkstitch:*` attributes on SVG import, phase 1 offers
`stitch import-inkstitch design.svg -o design.vectorcraft`: our SVG adapter reads the Ink/Stitch file
and writes a VectorCraft document with the same art and the parameters as live-effect records (M8).
VectorCraft's `main` keeps the attributes from SVG import to SVG export after 0.16, but its live effects
read none of them.

## 5. Phase 2: plug-in features proposed upstream (summary)

Detailed in [the RFC](rfc-vectorcraft-abi-v2.md), which began as a proposal for an ABI v2. VectorCraft
took most of it into ABI v1 instead, each as an addition older hosts ignore or refuse to install. Every
item is generic, useful to any plug-in and not only embroidery:

| Proposal | In VectorCraft | StitchCraft's use |
|---|---|---|
| 1. Exporter plug-ins, for File › Export › PES | 0.14.0 | M6.9 |
| 2. Rich parameter schemas: labels, units, groups, help, conditions, texts and lists, 256 parameters | 0.14.0 | M6.10 |
| 3. Display-only overlays from live effects, real stitch previews at low cost | in review | M9 |
| 4. Budget classes, with the user's consent | heavy filters on `main` after 0.16.0, heavy live effects proposed | M6.12, M9 |
| 5. Document-level plug-in data, for the machine profile and palettes | 0.16.0, for filters and exporters | M6.11 |
| 6. Read-only context for live effects | the object's paints in 0.14.0, the markers beside it proposed | M6.8, M9 |
| 7. SVG foreign-attribute preservation | `main` after 0.16.0 | Ink/Stitch files keep their settings through VectorCraft |
| 8. Effects and stroke style in filter objects | 0.15.0 | M6.12 |

## 6. Phase 3: an in-tree crate (optional)

If ArtCraft wants embroidery natively — or in a fork — a `vectorcraft-embroidery` crate at VectorCraft's
L2 (depending on `geom`, `doc` and our MIT/Apache engine crates) would provide commands in
`crates/engine/src/cmd/embroidery.rs` (apply settings, export PES/DST through the `FORMATS` table, test
sheets), panels in `crates/ui-egui` (Embroidery parameters, Stitch Simulator) and a satin rung tool in
`crates/tools`. The engine and adapters do not change; only a new adapter is written. This is why the
engine must stay host-agnostic.

## 7. Patterns we adopt from VectorCraft

VectorCraft's conventions are mostly excellent and we follow them; where they drift we improve them.
See [ADR-0009](adr/0009-adopt-vectorcraft-conventions.md) for the audit.
