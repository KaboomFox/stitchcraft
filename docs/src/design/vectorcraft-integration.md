# VectorCraft integration

<!-- implements: apps/stitchcraft-vc-plugin/src/**, crates/stitchcraft-vectorcraft/src/** -->

How StitchCraft lives inside [VectorCraft](https://github.com/storytold/vectorcraft): what VectorCraft
offers today, how we use it without changing VectorCraft (phase 1), what we propose upstream
(phase 2), and how an in-tree integration would look (phase 3).

**VectorCraft snapshot reviewed:** `main` at `89ea46c` (2026-10-08), version 0.6.0. File references
below are to that commit; the [compatibility gate](compatibility-gate.md) keeps us honest as it moves.

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
| Releases | Tags `v0.1.0` … `v0.7.0`; pushing the `release` branch drafts a GitHub Release; SemVer pre-releases (`-rc.1`) are flagged as pre-releases | `.github/workflows/release.yml` |

## 2. Extension points that exist today

### Plug-in ABI v1 (`crates/plugins`, `docs/plugins.md`)

- **Sandbox:** a core WebAssembly module run by `wasmi` 2.0, **no imports at all** (no WASI, clock,
  file system or randomness). Exports: `memory`, `vc_abi_version() → 1`, `vc_manifest()`,
  `vc_alloc(size)`, `vc_run(input, len, params, len)`. Input and output are JSON.
- **Kinds:** *object filters* (read selected paths and compound paths — geometry, fills, strokes,
  opacity — and return updated or new objects; one undo step) and *live effects* (rewrite one object's
  geometry every time it is drawn, cached by plug-in, parameters and input geometry).
- **Parameters:** manifest schema with `number`, `int`, `bool`, `choice`; at most **64 parameters**
  (`MAX_PARAMS`, `crates/plugins/src/manifest.rs:11`) and a 64 KiB manifest; no labels, units, groups or
  conditions — the dialog shows parameter names.
- **Limits** (`crates/plugins/src/runtime.rs`): module ≤ 32 MiB; memory ≤ 512 MiB; output ≤ 64 MiB JSON;
  **fuel 50 M instructions + 4,000 per input byte**; wall clock 60 s per filter run and **5 s per
  live-effect run** (`crates/plugins/src/effect.rs`); fresh instance per call.
- **Live-effect input is geometry only**: `type`, `path`, `fillRule`, `bounds` — no paint, no
  neighbours.
- **Persistence:** applying a live effect stores `{"id": "plugin.<id>", "params": {…}}` in the object's
  appearance (`Effect { id, params: serde_json::Value, visible }`, `crates/doc/src/appearance.rs`).
  Documents keep the record when the plug-in is missing and draw the object unchanged.
- **Not in v1:** tool plug-ins, panel plug-ins, host callbacks (progress, cancel), file-format plug-ins,
  live effects that change paint.

### Other facts that shape the design

- **No generic per-object metadata.** `Node` has `attrs: Option<Box<ObjectAttributes>>` with only
  centre display, image map, URL and note (`crates/doc/src/node.rs`). The effect record is therefore the
  right place for embroidery parameters.
- **Document-level foreign data** lives in `Document.unknown` (used for the perspective grid and
  brushes), preserved on round trips (`crates/doc/src/lib.rs`).
- **SVG import keeps only the attributes it understands.** The importer looks attributes up by name
  (presentation attributes, references, names such as `inkscape:label`) and has no place to keep unknown
  ones (`crates/svg/src/import.rs`), so `inkstitch:*` parameters do not survive opening an Ink/Stitch SVG
  in VectorCraft today. SVG export can embed the whole native document (`vectorcraft:document`) when
  editing data is preserved.
- **Export formats are a static table** (`FORMATS` in `crates/engine/src/cmd/fileio/mod.rs`): there is no
  registry a plug-in could add PES to.

## 3. Phases

| Phase | Needs from VectorCraft | Delivers | Milestone |
|---|---|---|---|
| 1. ABI v1 plug-ins | nothing | per-object embroidery settings, previews, tools; machine files via `stitch export file.vectorcraft` | M6 |
| 2. ABI v2 (upstream RFC) | generic plug-in features | export from File › Export, real stitch previews, rich parameter panels, progress | M9, after 1.0 |
| 3. In-tree crate (optional) | ArtCraft's interest, or a fork | native speed, simulator panel, satin tool | post-1.0 |

## 4. Phase 1 design (ABI v1, no VectorCraft change)

### Plug-ins

One `.wasm` per stitch family keeps each dialog relevant and under 64 parameters:

| Plug-in id | Kind | Stitch types | Notes |
|---|---|---|---|
| `<ns>.stroke` | live effect | running, bean, manual (P1); zigzag, ripple (later) | `stroke_method` choice selects the variant |
| `<ns>.satin` | live effect | satin (P1); E, S, zigzag (P2) | rails and rungs from the object's compound path |
| `<ns>.fill` | live effect | tatami (P1); contour, meander, circular (P2) | `fill_method` choice |
| `<ns>.tools` | object filter | make satin from two paths, add rung, bake preview to paths, generate test sheet | one undo step each |

`<ns>` is a reverse-DNS namespace defined in one constant (`PLUGIN_NAMESPACE`) in the VectorCraft adapter,
which reads the effect records too. The plug-in crate imports it. A live effect's id ends with its stitch
family's (`stitchcraft_params::Family::id`).
Manifests are generated from the [parameter registry](params.md#splitting-for-vectorcraft-abi-v1); a
test validates each against VectorCraft's manifest rules (id charset, ≤ 64 parameters, names, ranges) so
an invalid manifest never reaches a user.

**Modules.** The plug-in crate builds one module for each family, chosen by a Cargo feature named after
the family: `stroke`, `satin` or `fill`. Without one it builds M0.6's *hello* effect. The modules contain
the same code, which the native tests run whichever it is, and the feature only picks the manifest and
the effect that `vc_manifest` and `vc_run` serve. `cargo xtask compat contract` builds the four modules,
notes their sizes and loads them into VectorCraft's host.

**Manifests.** A family's manifest is made from the registry when VectorCraft asks for it, so a parameter
added to the registry shows in the dialog without another change. VectorCraft's parameters are numbers,
whole numbers, toggles and choices, and each always has a value. A record stores every declared
parameter, at its default when the user leaves it. So a parameter is declared when one such value means
what the parameter means:

- a length, an angle, a percentage or a number as a number in its range, and a count as a whole number;
- a list or a pair whose default is one value as that value, which stands for the whole list or both
  sides, as in Ink/Stitch;
- an optional length that 0 leaves unset as a number from 0.

Texts, seeds, and settings left empty to take another's value keep their defaults. In the registry today
they are the custom locks' steps, `random_seed`, a fill underlay's angles, a satin's zigzag underlay
insets and its shortest random split. A choice leaves out the options that a text spells out, which are
the custom locks. `satin_column` belongs to the effect, which sets it from its family. The dialog opens
with the group that declares the family's method. The groups only the family uses come next, then those
it shares with the other families. The settings every element shares come last, as in Ink/Stitch's
dialogs.

**What a live effect shows.** A family's effect sews its object with the engine's `generate`, on the
reference machine and with no neighbours, as one object of a design. It returns the stitches as geometry,
the exact level below. When it can't show stitches, it gives back why. VectorCraft then draws the object
as it is and shows the message as the plug-in's last error. That happens when the object cannot be sewn or
sews no stitch, with the message that says why, and for a satin column drawn as one path, which is as
wide as its stroke, with `SC-I0810`. A drawing that vanished instead could not be clicked to change the
setting that emptied it. ABI v1 has no way to pass VectorCraft the warnings about stitches the effect
shows, and the command line reports them (M6.5). How much of the stitches it shows, within what
VectorCraft lets it spend, is its preview level (below).

### The workflow

1. Draw the art in VectorCraft as usual.
2. Select an object → **Effect › Plug-ins › StitchCraft Fill** (or Stroke / Satin) → set parameters in
   the generated dialog. The parameters are now stored on the object (undoable, saved in the
   `.vectorcraft` file) and the object shows a stitch preview.
3. Save the document.
4. `stitch export design.vectorcraft --profile brother-pe800-5x7 -o design.pes` writes the machine file and
   a report; `--preview design.png` renders what will sew.

Stitching order is paint order (bottom first). Hidden objects are skipped, as VectorCraft draws nothing
for them, and locked objects are stitched. Thread colours come from the object's fill for a fill, and
from its stroke for a stroke or a satin column, as in Ink/Stitch, mapped to the profile's palette.

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
costs. The real stitches are always available from the CLI preview, and ABI v2's overlay channel removes
this limitation.

Paint caveat: a live effect cannot read the object's paint, so a fill preview shows ribbons painted with
the object's own fill; stroke previews use the object's stroke. Users set the fill/stroke to the thread
colour, which they want anyway for export.

### Commands in v1

- `trim_after` and `stop_after` are ordinary parameters.
- Start and end points: a small path named `stitch:start` or `stitch:end` grouped with the shape. The
  adapter reads the marker's centre as the hint and never stitches the marker. (ABI v2 makes these
  first-class.)
- Ignore: hide the object, or name it with the prefix `stitch:ignore`.

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

**Limits.** At most 64 MiB of JSON, unpacked, as for an SVG file. A gzip stream is unpacked only up to
the marker, and no further than the limit. Nesting deeper than the JSON reader allows (128 levels, about
40 levels of groups) refuses the file. VectorCraft itself keeps 32. The JSON is read into the few things
the adapter uses, and the rest is skipped as it is read: images, previews and text runs are never kept.
Every object, appearance item, effect and anchor charges the budget, and so does each element for its
copy of the object's path.

**Coordinates.** VectorCraft keeps every path in document coordinates. Moving or scaling a group moves
its paths' anchors, so layers, groups and compound paths have no transform. Coordinates are points, 1/72
inch, with y down, and the adapter multiplies them by 25.4/72 into millimetres. The document's origin
stays the origin. Coordinates are read exactly, as VectorCraft reads them, from their text in the JSON.
An anchor's handles are absolute positions. A missing handle, or one within a billionth of a point of its
anchor, leaves that end of the segment straight, and a segment without handles is a line.

**What is stitched.** The layers in order, bottom first, and in each its children in paint order:

- **Paths and compound paths** are the art. A compound path's children are one shape, with the compound's
  appearance and fill rule.
- **Groups and layers** pass their children on. Clipping is left for M8, as in the SVG adapter: a
  clipping group's or layer's art is stitched whole, with `SC-W0807`.
- **Hidden objects**, and everything in them, are left out without a word, as VectorCraft draws nothing
  for them. So are objects and paints of opacity 0, paints turned off and strokes of width 0.
- **Template layers, layers that don't print, and guides** are left out with `SC-I0809`. VectorCraft
  shows them, dimmed or as guides, but prints and exports none of them.
- **A Shaper group's sources**, its first child, are left out: the group shows the shapes it makes of
  them.
- **Text, images and symbols,** and the objects VectorCraft computes as it draws them (blends, envelopes,
  meshes, repeats, placed documents and VectorCraft's Intertwine objects), are left out with `SC-W0807`.
  The message names VectorCraft's command that makes paths of them, such as Type › Create Outlines for
  text. So is a kind of object this version doesn't read.

**Paints and threads.** An object without StitchCraft's effects is stitched as Ink/Stitch stitches an SVG
shape. Its visible fills are filled, in the appearance's order, and its visible strokes then have a
running stitch each. The thread's colour is a solid paint's colour, with CMYK and grey converted by
VectorCraft's formula without colour management ((1 − c)(1 − k) and 1 − k) and Lab through sRGB. A
gradient gives its first stop's colour, with `SC-W0807` unless every stop has that colour, and a pattern
gives black, with `SC-W0807`. A stroke's width, in points, and its join, with its miter limit (10 when
the file omits it, as VectorCraft draws it), shape a satin column drawn as one path. A stroke drawn with
a brush is stitched along its path, without the brush's art, with `SC-W0807`. A group's or a layer's own
fills and strokes, which paint every member, are left out with `SC-W0807`.

**Effects.** StitchCraft's live effects are kept in the object's appearance as `{"id":
"plugin.dev.stitchcraft.<family>", "params": {…}, "visible": true}`. The family is `stroke`, `satin` or
`fill` (`stitchcraft_params::Family`). An object with visible StitchCraft effects is stitched as they
say, in their order, instead of by its paints. A `fill` effect fills the object's shape, a `stroke`
effect stitches along it and a `satin` effect makes a satin column of it. With this rule, an object sews
what its effects say and nothing else. A fill with an outline takes a `fill` effect and a `stroke`
effect. The record's parameters are the element's. A number is written as a decimal, a toggle as `true`
or `false`, a choice as its id and a list as its values with spaces between. What the record leaves out
takes its default, as in the plug-in's dialog. The thread is the colour of the first fill VectorCraft
draws for a fill, and of the first stroke it draws for the others, falling back on the other kind. A
`stroke` or `satin` effect follows the first visible stroke's width and join, or a width of 0 when there
is none, and a satin column too narrow to stitch across is sewn as a stroke, with `SC-W0212`.
VectorCraft's own effects change what an object draws, and are left out with `SC-W0807` (Object › Expand
Appearance makes paths of what they draw).

**Ids.** An element is `vc:<object id>:<what>`: `fill` or `stroke` for a paint, with its rank among the
object's fills or strokes from the second on (`fill.2`), and the family for an effect. VectorCraft keeps
an object's id when it saves, so a report and a preview name the same object. A repeated id gets `~2`,
`~3`, … as in the SVG adapter.

**Reports.** `SC-E0806` refuses a file that isn't a VectorCraft document, or is larger than the adapter
reads. `SC-W0807` names what isn't stitched. `SC-W0808` names geometry that can't be used, such as a
number that doesn't read or a point more than 10 m from the origin, and a shape that draws nothing.
`SC-I0809` lists the layers and guides left out because VectorCraft doesn't print them. The compatibility
gate will have each VectorCraft version it tests write files for the adapter to read back (Level B, M6.6).

### Importing Ink/Stitch SVGs into VectorCraft

Because VectorCraft drops `inkstitch:*` attributes on SVG import, phase 1 offers
`stitch import-inkstitch design.svg -o design.vectorcraft`: our SVG adapter reads the Ink/Stitch file
and writes a VectorCraft document with the same art and the parameters as live-effect records (M8).

## 5. Phase 2: ABI v2 proposals (summary)

Detailed in [the RFC](rfc-vectorcraft-abi-v2.md). Every item is generic — useful to any plug-in, not
only embroidery:

1. **Exporter plug-ins** (a new kind) registered in a dynamic format table: File › Export › PES.
2. **Rich parameter schemas:** labels, help text, units, groups, `visibleWhen` conditions, `string` and
   `list` types, help URLs; a higher or per-group parameter limit.
3. **Display-only overlays** from live effects (coloured polylines in a compact binary buffer) that draw
   on the canvas without replacing the object's geometry — real stitch previews at low cost.
4. **Budget classes** declared in the manifest (`light`, `heavy`) with user consent, and output-
   proportional fuel.
5. **Document-level plug-in data** (`Document.plugin_data["<plug-in id>"]`) for the machine profile and
   palettes.
6. **Read-only context for live effects:** the object's paint and named sibling markers.
7. **SVG foreign-attribute preservation** per node, written back on export.

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
