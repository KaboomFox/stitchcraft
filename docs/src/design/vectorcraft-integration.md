# VectorCraft integration

<!-- implements: apps/stitchcraft-vc-plugin/src/**, crates/stitchcraft-vectorcraft/src/** -->

How StitchCraft lives inside [VectorCraft](https://github.com/storytold/vectorcraft): what VectorCraft
offers today, how we use it without changing VectorCraft (phase 1), what we propose upstream
(phase 2), and how an in-tree integration would look (phase 3).

**VectorCraft snapshot reviewed:** `main` at `89ea46c` (2026-10-08), version 0.6.0, and its plug-in API
again at `8bf090b` (2026-10-24), after 0.16.0, at `v0.17.0` (2026-10-25) and at `v0.18.0` (2026-10-28).
File references below are to the first unless a version is named. The
[compatibility gate](compatibility-gate.md) tests the plug-ins against each VectorCraft release and branch.

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
| Releases | Tags `v0.1.0` … `v0.18.0`. Pushing the `release` branch drafts a GitHub Release, and SemVer pre-releases (`-rc.1`) are flagged as pre-releases | `.github/workflows/release.yml` |

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
  10 G instructions and 300 s (0.14). From 0.17 a filter that declares `"budget": "heavy"` gets 1 G and
  300 s, once the user allows it at install, and runs in the background. From 0.23 only a person allows
  it: with OK in the app's dialog, or by starting the command line with `--allow-plugin-grants`. From 0.18 a run pays nothing for
  compiling the functions it calls, which wasmi does at each one's first call. Before, a first run paid,
  and could stop with most of its budget left when the 20 M-instruction slice it was in ran out while a
  function compiled. From M13.2 on, the gate's Level B found 0.17.0's command line stopping the PES
  exporter so on the test sheet TS-04, an export that takes 34 M instructions in 0.18.0's host.
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
  `inkstitch:*` parameters. From 0.17 VectorCraft keeps attributes in other applications' namespaces from
  SVG import to SVG export, and reads none of them. SVG export can embed the whole native
  document (`vectorcraft:document`) when editing data is preserved.
- **Export formats are a static table** (`FORMATS` in `crates/engine/src/cmd/fileio/mod.rs`). From 0.14 an
  exporter plug-in adds a format to it, `plugin.<id>`, which is how StitchCraft can add PES.

## 3. Phases

| Phase | Needs from VectorCraft | Delivers | Milestone |
|---|---|---|---|
| 1. ABI v1 plug-ins | nothing | per-object embroidery settings, previews, tools; machine files via `stitch export file.vectorcraft` | M6 |
| 2. Plug-in features (upstream RFC) | generic plug-in features, most of them in VectorCraft 0.14 to 0.16 | previews as wide as a stroke, export from File › Export, labelled dialogs, design settings and a machine per document, and filters that make satin columns and draw test sheets (M6.8 to M6.12). Overlay previews of every stitch, and heavy live effects, after 1.0 (M9) | M6, M9 |
| 3. In-tree crate (optional) | ArtCraft's interest, or a fork | native speed, simulator panel, satin tool | post-1.0 |

## 4. Phase 1 design (ABI v1, no VectorCraft change)

### Plug-ins

One `.wasm` per stitch family keeps each dialog relevant. The stroke's and the satin's declare at most 64
parameters, which every VectorCraft release takes, and the fill's declares more from M10.10
([the fill dialog's 65 settings](#the-fill-dialogs-65-settings)):

| Plug-in id | Kind | Stitch types | Notes |
|---|---|---|---|
| `<ns>.stroke` | live effect | running, bean, manual (P1), zigzag (M7.2), ripple (M10.8) | `stroke_method` choice selects the variant |
| `<ns>.satin` | live effect | satin (P1), E (M7.3), S (M7.4), zigzag (M7.5) | rails and rungs from the object's compound path |
| `<ns>.fill` | live effect | tatami (P1); contour, meander, circular (P2) | `fill_method` choice |
| `<ns>.auto` | live effect | the stitches automatic digitizing chooses by its rules (M13.7, M13.21) | on a group, previews each path inside ([image to embroidery](image-to-embroidery.md)) |
| `<ns>.pes`, `<ns>.dst` | exporter (VectorCraft 0.14) | every family | File › Export › Export As writes the same machine file as `stitch export` (M6.9, [below](#exporters-file--export-writes-pes-and-dst)) |
| `<ns>.settings` | filter (VectorCraft 0.16) | none | keeps the design settings and the machine with the document (M6.11, [below](#design-settings-and-machine-kept-with-the-document)) |
| `<ns>.stroke-to-satin` | heavy filter (VectorCraft 0.17) | satin | makes each selected line into a satin column drawn with its rails and rungs (M6.12, [below](#tools-stroke-to-satin-and-test-sheet)) |
| `<ns>.testsheet` | filter (VectorCraft 0.16) | every family | draws a test sheet into the document, each object with its effect (M6.12, [below](#tools-stroke-to-satin-and-test-sheet)) |
| `<ns>.route-satins` | heavy filter (VectorCraft 0.17) | satin, stroke | routes the selected satin columns, as Ink/Stitch's Auto-Route Satin Columns does (M10.4, [below](#tools-the-routing-filters)) |
| `<ns>.route-runs` | heavy filter (VectorCraft 0.17) | stroke | routes the selected strokes, as Ink/Stitch's Auto-Route Running Stitch does (M10.13, [below](#tools-the-routing-filters)) |
| `<ns>.tartan` | filter (VectorCraft 0.15) | fill | sets the sett of the selected paths' tartan fills from a code of its stripes (M10.7, [below](#a-tartans-sett)) |
| `<ns>.lettering` | live effect (VectorCraft 0.14) | a font's own | keeps the embroidery font a text object sews in, and returns its geometry unchanged (M14.4, [below](#lettering)) |

The tools an object filter was to give are VectorCraft's own commands ([tools](#tools-vectorcrafts-own-commands))
and M6.12's 2 filters ([Stroke to Satin and Test Sheet](#tools-stroke-to-satin-and-test-sheet)). M10.4
and M10.13 add 2 routing filters ([routing filters](#tools-the-routing-filters)), and M10.7 one that sets a
tartan's sett ([a tartan's sett](#a-tartans-sett)).

`<ns>` is a reverse-DNS namespace defined in one constant (`PLUGIN_NAMESPACE`) in the VectorCraft adapter,
which reads the effect records too. The adapter makes the ids from it, a plug-in's with `plugin_id` and its
live effect's with `effect_id`, and `effect_name` reads one back. The plug-in crate and the compatibility
gate use the same functions. A live effect's id ends with its stitch family's
(`stitchcraft_params::Family::id`).
Manifests are generated from the [parameter registry](params.md#splitting-for-vectorcraft-abi-v1); a
test validates each against VectorCraft's manifest rules (id charset, the most parameters, names, ranges,
and from 0.14 the keys that say how the dialog shows a parameter), so a manifest VectorCraft would refuse fails
the tests before a release.

**Modules.** The plug-in crate builds one module for each family, chosen by a Cargo feature named after the
family: `stroke`, `satin` or `fill`, one for each machine format, by its extension: `pes` or `dst`, and one
for each filter: `settings`, `stroke-to-satin`, `testsheet`, `route-satins`, `route-runs` and `tartan`.
The feature `lettering` builds the Lettering effect.
Without one it builds M0.6's *hello* effect. The modules contain the same code, which the native tests run
whichever it is, and the feature only picks the manifest and the run that `vc_manifest` and `vc_run` serve.
`cargo xtask compat contract` builds every module, notes their sizes and loads them into VectorCraft's host.

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
- an optional length that 0 leaves unset as a number from 0;
- a tartan's sett as a text, from M10.7 ([below](#a-tartans-sett)).

The other texts, seeds, and settings left empty to take another's value keep their defaults. In the registry
today they are the custom locks' steps, `random_seed`, a fill underlay's angles, and a satin's zigzag underlay
insets and its shortest random split. A choice leaves out the options that a text spells out, which are the
custom locks, and `fill_method` leaves out Ink/Stitch's legacy fill, which StitchCraft sews as a tatami fill
(`REQ-FILL-005`). `satin_column` belongs to the effect, which sets it from its family. `turn_stitches`, the switch that
automatic digitizing sets on its own fills, is left out of the fill's dialog. The Auto effect's dialog
leaves out the settings of the registry's Advanced group, which are
the bounds of the rules and the 2 limits of the cover's measure (M13.21), the base layer's offset, and
the 4 limits of how thick a design sews (M13.29).
The switch that sews an outline round each fill has a row since M13.28, in the offset's place. The dialog
opens with
the group that declares the family's method. The groups only the family uses, for its default method, come
next, then those it shares with the other families, then those of its other methods, such as the zigzag
stroke's. The settings every element shares come last, as in Ink/Stitch's dialogs.

#### The fill dialog's 65 settings

M10.9's stitch positions made the fill dialog 64 parameters, the most that VectorCraft 0.12 and 0.13 take
in a manifest. M10.10's strategies need `guided_fill_strategy` in the dialog: VectorCraft keeps only the
settings a manifest declares, so a strategy the dialog doesn't hold can't be set in a document at all. It
is the 65th.

VectorCraft's rule from 0.14 is 256 parameters, at most 64 of them outside groups, since a longer dialog is
sorted into sections (`MAX_PARAMS` and `MAX_UNGROUPED` in its `crates/plugins/src/manifest.rs`, and
"Parameters" in its `docs/plugins.md`). Every setting of a StitchCraft dialog is in its registry group, a
section of the dialog, and the fill effect installs only from 0.14 for its sett's text. So the manifest
check now follows VectorCraft's own rule (`REQ-VC-023`):

- at most 256 parameters, and at most 64 outside groups;
- more than 64 only in a manifest that VectorCraft 0.12 and 0.13 refuse anyway, an exporter's or one with a
  text, so that no effect those releases install grows past what they take.

`visibleWhen` hides rows without lowering the count, so it was no way out. The alternatives were weighed
and turned down:

- **A dialog of its own for the guided fill**, as another effect. A fill could then not change its method
  in place, and a document would hold 2 kinds of fill effect for one registry.
- **The strategies left to files**, set in an SVG file and sewn by `stitch export`, with a coded note in
  VectorCraft. A design drawn in VectorCraft could then never offset or buffer its guide.
- **Another setting left out to make room.** Each of the 64 is one a fill method needs, and the next
  setting would ask the same question again.

The compatibility gate installs the fill effect in every supported release from 0.16, so a release that
took fewer would fail it ([compatibility gate](compatibility-gate.md)).

#### A tartan's sett

A tartan's sett is one setting, `tartan`, an object of JSON whose code lists the stripes
([tartan fill](algorithms/tartan.md#the-sett)). M10.6's fill dialog offered the tartan fill with its
numbers alone, `tartan_angle`, `rows_per_thread`, `herringbone_width_mm` and `stop_at_ending_point`, and a
tartan fill made in VectorCraft sewed Ink/Stitch's default sett. M10.7 decided how VectorCraft sets a sett,
and chose both of the ways open to it (`REQ-VC-021`):

- **The fill dialog shows the sett as text**, its row labelled Sett and shown for the tartan fill alone. A
  live effect is given only the settings its manifest declares, and VectorCraft drops a record's other keys
  before it runs the effect. So without this row no preview could sew a sett of its own, whatever wrote it.
  The row also shows the sett an Ink/Stitch file brought, and takes one pasted in. With it the fill dialog
  had 63 parameters, and it is the only text a dialog declares (`manifest::TEXTS`). A text is also why the
  fill effect installs from VectorCraft 0.14, where a dialog may declare more than 64 settings
  ([below](#the-fill-dialogs-65-settings)).
- **StitchCraft Tartan, a light filter**, writes the row from what a person has, a palette code or a
  threadcount in the register's colour letters, with the width of a thread, the sett's turn, its scale, how
  far it is moved and the width up to which a stripe sews as a run
  (`apps/stitchcraft-vc-plugin/src/tartan.rs`). It is Ink/Stitch's editor reading a code ([the
  editor](algorithms/tartan.md#the-editor)). A selected path whose own first StitchCraft effect is the fill
  effect gets the tartan fill and the sett, its other settings kept as VectorCraft gave them. A path with no
  StitchCraft effect gets the fill effect with those 2 settings, and VectorCraft fills in the rest. A filter
  can't warn, so a code it can't read, letters the register uses for no colour, a code without a stripe, a
  setting longer than the row's 4,096 characters, and a selection without a path to take the sett leave the
  drawing as it was, with `SC-E0821`. Records a filter gives are kept from 0.15, and before it the filter
  declines with `SC-E0813`.

The rejected ways each failed a part of the job. The filter alone would write a key the fill's manifest
doesn't declare. The preview never gets such a key, and the canvas would show Ink/Stitch's default sett while
File › Export sewed the filter's. With the row alone, a person would write an object of JSON by hand on one
line. A threadcount, the form the register gives a sett in, wouldn't read there. A filter that drew the
stripes as objects of their own would lose the weave, which comes from routing every colour's rows over the
whole shape.

**What it costs.** A text is a kind of parameter VectorCraft 0.14 brought, and 0.12 and 0.13 refuse a
manifest with one whole. So from M10.7 the fill effect installs from VectorCraft 0.14, as the exporters
do. Those 2 releases don't export a machine file either, so a fill made there was only ever a preview.

The Test Sheet filter drew TS-17B from M10.7, each tartan's sett in its fill effect's record as the
row's text, and from M10.12 each of its gradients as a gradient paint
([below](#a-gradients-geometry)). From M10.9 the sheet's guided leaf copies a guide line. Drawn into a
document, the guide would be a `stitch:guide` marker grouped with the leaf, and a filter doesn't make groups.
Only `stitch testsheet` draws TS-17B now. The filter still draws a linear gradient fill with its gradient
paint, though no sheet it draws has one.

**How the dialog shows them** (`REQ-VC-012`, M6.10). From 0.14 a parameter can say how the dialog shows
it, and the plug-in says all of it from the registry (`apps/stitchcraft-vc-plugin/src/dialog.rs`):

- Its row has the registry's label, and the unit of its kind of value: mm, % or °.
- The registry's groups are the dialog's sections, as they are the headings of the reference pages.
  VectorCraft draws a section where its group first appears, with every row of the group in it. Each
  group's rows come together in the manifest, and a release before 0.14 shows them in the same order.
- Its help is the registry's help as plain text, cut at a sentence within VectorCraft's 1,000 characters.
  A choice's help ends with what each option means, by its label in the registry: VectorCraft's
  drop-down list shows the ids that designs store, such as `tatami_fill`. Its help page is its entry in
  the parameter reference on the docs site, the book of `main` (`dialog::SITE_URL`). The docs check finds
  the page and the heading each manifest links to.
- Each row shows only while the method is one the parameter applies to. Where the registry shows a
  setting only with some values of another, as its `when` says, the dialog does too: the length variation
  of a random stitch length shows while that is on, and a satin's staggers for staggered splits. Rows that
  nothing in the dialog would show are left out. VectorCraft gives the plug-in the value of a hidden row as
  well, so what the dialog shows never changes what sews.
- The plug-in's own parameters, the preview level and the exporters' 2 options, have a label, help and a
  page too.

A release before 0.14 ignores these keys, since its `ParamSpec` reads a parameter's type, range and
default and lets other keys pass (`crates/plugins/src/manifest.rs` at 0.12.0). Its dialog shows the keys
and every parameter, as it did. The compatibility gate's pictures of the dialogs are taken in the pinned
release's window, and the window probe checks that each shows the rows the manifest shows at the defaults,
by label, each with the control of its kind.

**What a live effect shows.** A family's effect sews its object with the engine's `generate`, on the
reference machine and with no neighbours, as one object of a design. It returns the stitches as geometry,
the exact level below. When it can't show stitches, it gives back why. VectorCraft then draws the object
as it is and shows the message as the plug-in's last error. That happens when the object cannot be sewn or
sews no stitch, with the message that says why. A drawing that vanished instead could not be clicked to
change the setting that emptied it. ABI v1 has no way to pass VectorCraft the warnings about stitches the
effect shows, and `stitch export` reports them. How much of the stitches it shows, within what VectorCraft
lets it spend, is its preview level (below).

**The stroke an effect follows** (`REQ-VC-010`, M6.8). A satin column drawn as one path and a zigzag stroke
are as wide as their stroke. From 0.14 VectorCraft gives a live effect its object's fills and strokes, and a
stroke or a satin effect on the object follows the topmost stroke that has a paint and a width. It reads
the stroke's width, and from 0.15 its join, cap and miter limit, with the reader's own code
(`stroke_style` in `crates/stitchcraft-vectorcraft`). `stitch export` follows the topmost stroke VectorCraft
draws, and the 2 can differ in 2 ways. VectorCraft gives a live effect every stroke of its object without
its opacity or whether it shows, and gives it none of the paints of the groups around it. Given no stroke,
before 0.14, for an object without one, or where only a group's stroke paints it, the effect leaves a satin
column drawn as one path and a zigzag stroke as they are, with `SC-I0810`. An effect on one of the object's
fills or strokes leaves the object as it is too, with `SC-I0810`: `stitch export` doesn't use such an effect
(`SC-W0807`).

### The workflow

1. Draw the art in VectorCraft as usual.
2. Select an object → **Effect › Plug-ins › StitchCraft Fill** (or Stroke / Satin) → set parameters in
   the generated dialog. The parameters are now stored on the object (undoable, saved in the
   `.vectorcraft` file) and the object shows a stitch preview.
3. Save the document.
4. `stitch export design.vectorcraft -o design.pes --preview design.png` writes the machine file, for the
   machine `--profile` names, or else the one the document keeps
   ([below](#design-settings-and-machine-kept-with-the-document)), or else `brother-pe800-5x7`, and a
   picture of what will sew. It says
   what the reader left out or changed, with the engine's warnings. `stitch export` is another name for
   `stitch plan`, which reads a VectorCraft document (`.vectorcraft`, `.drawcraft`, or a `.vctemplate`
   template) as it reads an SVG file (`REQ-VC-008`), and `--report` adds the JSON report. A bug-report
   bundle includes the part of the document the reader reads, which is its JSON without the images after
   it. Its replay reads that part the same way. From VectorCraft 0.14, File › Export › Export As writes
   the same file with StitchCraft's exporters, without saving first ([below](#exporters-file--export-writes-pes-and-dst)).

Stitching order is paint order (bottom first). Hidden objects are skipped, as VectorCraft draws nothing
for them, and locked objects are stitched. Thread colours come from the object's fill for a fill, and
from its stroke for a stroke or a satin column, as in Ink/Stitch, mapped to the profile's palette.

From M10.15, objects next to each other in paint order and in one thread, whose StitchCraft effects set
`underlays_first`, sew their underlays before their top stitches
([underlays first](engine-pipeline.md#underlays-first)). The toggle is a row of every stitch family's
dialog. Set on a group's effect, it applies to every path in the group (`REQ-VC-006`). A live effect sews
its object alone, so a preview doesn't show the order, and `stitch export --preview` draws what sews.

### Exporters: File › Export writes PES and DST

VectorCraft 0.14 added exporter plug-ins (the RFC's [P1](rfc-vectorcraft-abi-v2.md#p1-exporter-plug-ins)).
File › Export › Export As lists each after the built-in formats by its name, and `document.export` writes
one for a path with the extension it lists. VectorCraft gives the exporter the document as a native
`.vectorcraft` file's JSON, as `document.save` writes it but without the data of images of 8 KiB or more,
with the parameters of the dialog it shows when the file is picked, and the file's extension and name. The
exporter returns the file in base64, with warnings for the user, or declines with a message, and then
nothing is written (VectorCraft's `docs/plugins.md`, Exporters). The VectorCraft adapter's encoder
(`stitchcraft_vectorcraft::base64`) writes it, as it writes the picture `stitch plan` gives VectorCraft's
command line to trace ([image to embroidery](image-to-embroidery.md#pictures-from-the-command-line)).

M6.9 makes StitchCraft's formats exporters (`REQ-VC-011`, `apps/stitchcraft-vc-plugin/src/export.rs`).
There are two, *PES embroidery (StitchCraft)* and *DST embroidery (StitchCraft)*, each a module of its
own, and a VectorCraft before 0.14 refuses to install them. An exporter writes the file `stitch export`
writes of the document:

- It reads the document with the same reader and plans it with the engine for the machine its `profile`
  option names, written in its own format, whose colour changes the plan keeps to (`REQ-PLAN-006`). The
  design takes the file's name. The option's default, `document`,
  is the machine the document keeps, or `brother-pe800-5x7` when it keeps none, as `stitch export` plans
  it without `--profile` ([below](#design-settings-and-machine-kept-with-the-document)).
  Reading the file back, as `stitch export` does, stops it at a writer's bug (`SC-E0009`). The file has
  every thread of the plan, those of an element sewn in several among them (`REQ-THREAD-003`). The budget is
  `stitch export`'s own too. Its `auto` option digitizes the objects without StitchCraft effects, as
  `--auto` does, and sews them in the order StitchCraft chooses. The exporters have no switch like
  `--sort-colours`, and the objects with effects keep their order among themselves
  ([image to embroidery](image-to-embroidery.md#sewing-order)). Level B of the [compatibility gate](compatibility-gate.md) exports a document with
  VectorCraft's command line and checks that the files are `stitch export`'s, byte for byte.
- What `stitch export` says, the export says as its warnings, one line each, naming the object and with
  the hint after it. VectorCraft shows 100 of at most 500 characters each, so of more the last says how
  many more `stitch export` lists. It cuts a longer line in the middle of a word. The export ends such a
  line at its last whole sentence that fits. `SC-W0913`, the longest warning, is worded to fit with its
  hint: on a traced picture it has fewer than 500 characters.
- When anything said is an error, the exporter declines with the first, after how many there are, and
  VectorCraft doesn't write the file, as `stitch export` doesn't (`REQ-CLI-003`). VectorCraft shows the
  first 300 characters of a decline, after `plug-in failed:`.

An export may run 10 billion instructions, and 4,000 more for each byte of the document, and 300 seconds.
The engine's budget is `stitch export`'s, so a design that needs more is stopped by VectorCraft with its
own error, `plug-in exceeded its instruction budget`, and `stitch export` writes it. The contract test
`req_vc_011_a_fill_of_the_whole_hoop_exports_within_vectorcraft_s_limits` measures the largest designs
the reference machine sews (VectorCraft 0.16.0 to 0.18.0 alike, 2026-10-29):

| Design, each fill with its defaults | Instructions | Of those allowed |
|---|---|---|
| A fill of 128 × 178 mm, the 5 × 7 in hoop's whole field | 1.6 billion | 16 % |
| 63 fills of 14 mm squares across the field | 4.3 billion | 43 % |

A fill's work grows less than its area, and every fill takes some whatever its size: the squares cover half
the field and take more than twice as much. About 140 fills of that size are as many as an export can
plan, which few drawn designs for a home machine's hoop reach. A traced photo does, and
[an export that finishes](#an-export-that-finishes) says what exports it. The test
sheets drawn as designs export within 17 % of what an export may run, once the Test Sheet filter has drawn
them into a document ([below](#tools-stroke-to-satin-and-test-sheet)). A live effect is given no warnings
channel, and VectorCraft asks an exporter for no picture: `stitch export --preview` still draws what will sew.

### An export that finishes

M6.17 (`REQ-VC-025`). M6.15's browser check found that VectorCraft stops the export of a traced photo with
the Auto effect, in PES and in DST, with `plug-in exceeded its instruction budget`. The numbers below were
measured on 2026-11-15 with the host of VectorCraft 0.24.1 itself, its `vectorcraft-plugins` crate run
natively, and the PES exporter CI built from main. Each document is a fixture of `docs/fixtures/` with the
Auto effect on its traced group.

| Design | Document, bytes | Instructions allowed | Instructions needed | Needed for each allowed |
|---|---:|---:|---:|---:|
| The pear | 71,212 | 10.28 billion | about 8.7 billion | 0.8 |
| Dog on turf, 6 threads | 235,641 | 10.94 billion | about 51 billion | 4.7 |
| Dog running, 6 threads | 237,117 | 10.95 billion | about 56 billion | 5.1 |
| Dog on turf, 9 threads | 262,913 | 11.05 billion | about 59 billion | 5.3 |
| Dog running, 9 threads | 265,074 | 11.06 billion | 64 to 69 billion | 6.2 |

The host stopped each dog after 6 to 7 seconds, and the editor in Chromium after about 27. The desktop
application uses the same crate with the same limits, so it refuses the export the browser refuses. Level B
of the compatibility gate has the case: the scenario `traced-photo` traces the dog's cut-out, gives it the
Auto effect and expects VectorCraft's command line to stop the PES export. When a VectorCraft allows
enough, the scenario fails, and these pages change.

**The exporter can't be made to fit.** `stitch export` plans the same document in a second, 9.8 billion
instructions of the processor's own. A profile of that run puts about 65 % of them in arranging the
regions of the traced shapes, which overlap: cutting them where they cross takes 28 % and finding the
crossings 11 %. The exporter reads the document once and plans it once, and nothing is done again for
each shape. With the arranging free of charge the dog would still need 1.7 times what is allowed. Since M13.30
the dog on turf takes 26.6 billion natively, and [where automatic digitizing spends its
work](#where-automatic-digitizing-spends-its-work) gives each stage's share.

**The exporter can't say so first.** A module of ABI v1 has no imports, so it can't ask the host how many
instructions are left. It can't count them itself either. The engine's `Budget` counts each element's
work and doesn't add the elements up, and one unit of it cost between 330 and 5,300 instructions in
these runs, by the kind of work. A guess would refuse designs that fit. So no coded diagnostic comes
before the stop, VectorCraft's own error is what the user reads, and the user pages say what to do then.
The [RFC](rfc-vectorcraft-abi-v2.md#p10-an-export-budget-that-fits-a-traced-photo) asks VectorCraft for
both: more room, and a way to know it.

**So the work is done where nothing counts instructions.**

- On the desktop, `stitch export FILE.vectorcraft` writes the file from the saved document. It took 0.9
  to 1.3 seconds for each of the 4 dogs.
- In the browser demo, the Start here panel has **Export PES**, **Export DST** and **Show the real
  stitches**. Each runs the exporter plug-in's own module, which the browser compiles and runs without a
  count.

The panel's code is `web/exports.mjs`. It takes the document in front from the editor through the page's
documented call: `document.export` with the format `vectorcraft` answers the file the editor saves, in
base64 (VectorCraft's `docs/control-protocol.md`, the `app.export` row and "In the browser"). It starts a
Web Worker, `web/engine-worker.mjs`, and the editor goes on answering the visitor meanwhile. Every tenth of
a second the panel says how long the engine has worked so far. The worker's `web/engine.mjs` instantiates `pes.wasm` or
`dst.wasm` with no imports and calls `vc_run` with what VectorCraft would pass. Each parameter has its
manifest's default. The `_context` has the mode and the file's extension and name, and the name is the
document's title without its extension. Then the panel offers the file as a download with its size and
the time, and lists what the export said. The JavaScript is glue: every stitch is the module's.

The module was chosen over `stitch` built for WASI for these reasons:

- The modules are on the site already. The demo grows by the 3 scripts alone, 8,639 bytes.
- Level B already proves that an exporter writes `stitch export`'s file, byte for byte.
- `stitch` for WASI would need a shim for the system calls it makes, and the shim would be written here
  or brought in with its licence. Its module would be one more to publish.
- A module with no imports could be tried in Node on a machine without a wasm toolchain.

The picture needs one addition to the exporter. With `picture` as the `mode` of its `_context`, it
answers the picture `stitch export --preview` draws of the file read back, a PNG, in the file's place
(`PICTURE` in `apps/stitchcraft-vc-plugin/src/export.rs`). VectorCraft's own mode is `export`, so File ›
Export always gets the file. A design whose file isn't written has no picture.

Run in Node 22 before the panel was built, the 2 modules wrote the PES and the DST of the 4 dogs and the
pear byte for byte as `stitch export` writes them, in 2.4 to 4.0 seconds for a dog with the module's
compiling, and 1 second for the pear. The [before and after sheet](before-after-sheet.md#the-browsers-checks)
makes the same comparison in Chromium on every pull request, the picture included.

### Where automatic digitizing spends its work

M13.32 (`REQ-AUTO-083`). `cargo xtask perf` counts the instructions `stitch` takes to write each traced dog
and the pear with the Auto effect, and each TS-13 sheet, and splits each count into the stages of
automatic digitizing ([guardrails](guardrails.md)). The numbers below were measured on 2026-11-17 on main
`8a59600d`, natively, by callgrind, in the `profiling` build at opt-level 2 with Rust 1.97. Rust 1.97 gets
opt-level 3 wrong, so the build here differs from CI's, which uses Rust 1.99 at opt-level 3. The records in
`conformance/perf.toml` are CI's counts. A WebAssembly module runs 3.6 to 3.7 times the native count on
the TS-13 sheets (M13.31), so a dog would need about 100 to 125 billion in VectorCraft, an estimate.
VectorCraft allows it about 11 billion.

Millions of instructions for each stage, and the time the command takes:

| Stage | Dog on turf, 6 threads | Dog running, 9 threads | Pear | TS-13A | TS-13F |
|---|---:|---:|---:|---:|---:|
| Reading the shapes | 14 | 16 | 8 | 0 | 0 |
| Flattening the layers | 385 | 566 | 38 | 68 | 0 |
| The split into parts by the rules | 1,686 | 1,885 | 571 | 111 | 0 |
| The specks | 0 | 0 | 0 | 6 | 0 |
| The base layer | 99 | 76 | 17 | 66 | 0 |
| The search for the sewing order | 1,122 | 1,321 | 108 | 25 | 0 |
| The outline | 142 | 200 | 141 | 8 | 0 |
| The neighbours' overlaps | 21,692 | 27,753 | 1,338 | 245 | 0 |
| The rest of the neighbours' work | 28 | 35 | 8 | 6 | 0 |
| The stitches of each fill | 1,086 | 1,326 | 329 | 1,178 | 588 |
| The stitches of each column | 4 | 7 | 2 | 5 | 8 |
| The stitches of each line | 12 | 15 | 2 | 2 | 0 |
| The travel to and from each shape | 207 | 247 | 33 | 43 | 19 |
| The runs in place of jumps | 21 | 63 | 8 | 67 | 0 |
| The thickness measure | 13 | 17 | 8 | 41 | 0 |
| The machine file | 9 | 11 | 4 | 18 | 15 |
| The rest of the run | 86 | 142 | 24 | 91 | 31 |
| **The whole run** | **26,607** | **33,679** | **2,640** | **1,982** | **662** |
| Seconds on the command line | 2.2 | 2.9 | 0.2 | 0.2 | 0.1 |

The other pictures fall between these: the dog on turf in 9 threads takes 30,791 million and the running
dog in 6 threads 28,557. TS-13B to TS-13E take 1,730, 1,886, 1,539 and 1,116. TS-13F's ladder of squares
has no shape digitized automatically. A sheet's shapes are drawn in code, and that work is in the rest of
its run. The last pass of M13.35 gets a row of its own when it lands. Each stage's functions are listed in
`xtask/src/pictures.rs`, and the CI job's summary gives the table for all 11 pictures.

**The overlaps cost most, and M13.30 made them 4 times dearer.** On the dog on turf, finding where each
area reaches under its neighbours takes 82 % of the run. It does about 1,800 operations on regions:
growing a shape (298 calls, 5.9 billion), cutting one from another (565 calls, 3.8 billion), the parts 2
shapes share (306 calls, 3.6 billion) and joining shapes (2.7 billion or more). Growing a reach takes 1.5
billion more. Each operation arranges the edges of its regions where they cross, and that arranging takes
17.9 billion of the run's 26.6. Sorting the arrangement's points takes 8.5 billion of it. Built the same
way, the commit before M13.30 was merged (`265fa8af`) took 9.7 billion for the whole export and 4.9 billion
for the overlaps, where [an export that finishes](#an-export-that-finishes) measured 9.8 billion. M13.30
is the only change to the overlaps or to the regions between the 2 commits, and its overlaps, which follow
the rows and lie on no other overlap, added 16.8 billion.

**How the counts grow.** On the same dog in 6 and in 9 threads, the overlaps grow with the number of
shapes: from 104 to 120 shapes the dog on turf's grew 1.16 times, and from 99 to 121 the running dog's
grew 1.19 times. A dog's overlaps take 210 to 235 million instructions a shape. The pear's take 167
million, though its 8 shapes have 74 corners each and a dog's 18 on average. So the work goes with how many
neighbours a shape has more than with its corners. The fills grow with their stitches, at 70,000 to
100,000 instructions a stitch in the pictures and 30,000 to 50,000 on the sheets, whose squares are
simpler. The machine file and the thickness measure take about 1,000 instructions a stitch each.

**What M13.33 can bring down.** The 3 largest costs, in order:

1. The overlaps' region operations, 82 % of a dog. The dog on turf's 104 shapes are grown 298 times,
   and each operation costs 6 to 20 million instructions. Growing a shape once for all the overlaps
   that need it, and operating only on the shapes whose boxes meet, could take out much of the growing
   and of the cutting after it. The target is the 4.9 billion they took before M13.30, with the same
   stitches.
2. The arranging under every region operation, 67 % of a dog, half of it a sort. A sort of points that
   are nearly in order already, or points sorted once for the operations that share a region, would cost
   less. This brings down the split and the sewing order too, which use the same operations.
3. The split into parts by the rules and the search for the sewing order, 6 % and 4 % of a dog, and the
   fills, 4 %. In the release build, finding a part's tails (`split::tails`) took 1.0 billion of the
   split's 1.6, and the parts 2 shapes share took 0.7 billion of the sewing order's 1.1.

A live preview and the drawing of a test sheet have smaller budgets, 50 million for a light filter. A
live preview spends what its budget of units allows and shows less when that runs out, and the
contract tests check each level (`REQ-VC-003`). The Test Sheet filter digitizes nothing. TS-13F's drawing
took 86 % of its budget, and the compatibility gate fails a sheet's drawing above 90 % (`REQ-VC-015`).

### Design settings and machine kept with the document

Ink/Stitch keeps 3 lengths for the whole design with an SVG file, `collapse_len_mm`, `min_stitch_len_mm`
and `min_satin_stroke_width_mm` ([SVG input](svg-input.md#design-settings)), and a VectorCraft document has
no place of its own for them. From M8.9 the filter keeps its fourth such setting too, `rotate_on_export`,
which turns the design as it is written. VectorCraft 0.16 keeps up to 1 MiB of JSON with a document for each filter,
the RFC's [P5](rfc-vectorcraft-abi-v2.md#p5-document-level-plug-in-data). The filter returns it as
`documentData`, and the native file saves it in `document.unknown.pluginData` under the plug-in's id. An
exporter reads every plug-in's in the document it is given, and a live effect isn't given it.

M6.11 makes a filter keep them (`REQ-VC-013`): *StitchCraft Design Settings*, in Object › Plug-ins
(`<ns>.settings`, `apps/stitchcraft-vc-plugin/src/settings.rs`). Its dialog has the machine, a choice of
the built-in profiles, then each design setting as a length, and the turn on export, a choice of none,
left and right, as Ink/Stitch's preferences offer it. The settings' names, labels and help come from
`stitchcraft_engine::design::SETTINGS` and `Turn`, which the SVG adapter reads Ink/Stitch's settings with.
The machine is StitchCraft's own, as Ink/Stitch's hoop is the page. On OK the filter keeps all 5, the
settings under Ink/Stitch's names, the turn in degrees, and the machine's id as `profile`:

```json
{"profile": "brother-pe800-5x7", "collapse_len_mm": 3, "min_stitch_len_mm": 0, "min_satin_stroke_width_mm": 1, "rotate_on_export": 90}
```

The filter returns each object it is given by its id alone, as VectorCraft deletes an input object that a
filter leaves out, and so its run leaves every object as it was.

The VectorCraft adapter reads the data (`stitchcraft_vectorcraft::settings`) as the SVG adapter reads
Ink/Stitch's. The settings kept become the design's settings, and the machine kept is
`VectorCraft::machine`, which `stitchcraft_vectorcraft::machine` reads before the rest of the document.
`SC-W0812` names what doesn't read, which keeps its default:

- a setting that isn't a length of 0 or more, or that this StitchCraft doesn't know;
- a machine it doesn't have;
- data that isn't an object, whose settings all keep their defaults.

In a damaged file whose `pluginData` isn't an object, VectorCraft finds nothing kept, and so does the
adapter. `stitch export` and `stitch bug-report` plan for the machine `--profile` names, or else the one kept, or
else the reference machine (`stitchcraft_cli::commands::plan::machine_for`). The exporters' `profile`
option starts at `document`, which follows the same order, and names a built-in profile only when the user
picks one.

VectorCraft opens a filter's dialog at its manifest's defaults, not at what the filter kept. The filter
keeps every setting each time it runs, and the user sets all of them again. A live effect isn't given what
the document keeps, and the previews are planned with the default settings for the reference machine. A
longer shortest stitch kept with the document shows in the machine file and in `stitch export --preview`,
and not in the preview.

The contract test `req_vc_013_the_settings_filter_keeps_the_machine_each_exporter_plans_for` has
VectorCraft's host run the filter as `plugin.run` runs it, then exports the document with what it kept. Level B's `settings`
scenario does the same with VectorCraft's command line: a shortest stitch of 1.5 mm and a quarter turn
right change the PES file, the exports are `stitch export`'s, and with the small hoop kept the export of a
1 in square fails with `SC-E0701`. The filter keeps the turn as its number of degrees, where its dialog
offers it as a choice.

### Tools: VectorCraft's own commands

The plan had one object filter, `<ns>.tools`, to make a satin column from 2 paths, add a rung, bake a
preview to paths, make a line into a satin column and draw a test sheet. VectorCraft's own commands do the
first three, and M6.4 checked
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

The last two needed what ABI v1 gave a filter later, and M6.12 made them filters of their own.

### Tools: Stroke to Satin and Test Sheet

M6.12's 2 filters draw objects with StitchCraft's effects, which a filter's output may hold from
VectorCraft 0.15 (the RFC's P8). The VectorCraft adapter reads what a filter is given and writes what it
gives back (`stitchcraft_vectorcraft::filter`). A paint, an effect record and an opacity given back are
written as the JSON text VectorCraft gave, since VectorCraft keeps a paint's swatch, and a record as it
was, only when they come back unchanged, which it tells by their values. A path is written in points,
each coordinate the shortest decimal that reads back to it. VectorCraft admits a new effect record as
`effect.apply` would apply it: a StitchCraft effect's only while its plug-in is installed, its parameters
filled in to their defaults.

**Stroke to Satin** (`REQ-VC-014`, `apps/stitchcraft-vc-plugin/src/stroke_to_satin.rs`), Ink/Stitch's
Stroke to Satin, once Convert Line to Satin, makes each subpath of each selected path that has a stroke
into a satin column drawn with its rails and rungs. The line is the column's centre line, as wide as the
stroke a satin effect on it would follow, with that stroke's join. The engine makes the rails and rungs it
makes of such a column when it sews one (`REQ-SAT-016`, `REQ-SAT-017`), and draws them as straight lines,
each rung longer than the gap between the points it pairs by a hundredth of it at either end, so that it
crosses the rails ([satin generators](algorithms/satin.md#drawn-with-its-rails-and-rungs)). Read back, the column
pairs the same points, so it sews as the line did. A column of 2 rungs or none gets a rung at an end
too, since Ink/Stitch's rule would read the 2 longest lines as its rails (`DEV-SAT-007`). The rung goes
across the column's start, or across its end where the satin effect's push compensation lengthens the
start. Push compensation lengthens the rails before they are cut, so a rung across a lengthened end would
make the part added there a section of its own.

Each column is a compound path without fills. Its stroke is the line's paint one CSS pixel wide, which
shows the rails and draws the satin effect's stitches thin. The first column takes the object's place,
and the others go above it with its opacity. The satin effect takes the place of the object's first
StitchCraft effect, with the line's satin settings or the defaults. The object's other StitchCraft effects
are left out, and its other effects kept. An object the satin effect already reads as rails is given back
as it is. The filter refuses with `SC-E0814`, and the document stays as it was, when the selection is
empty, a path has no stroke that paints or can't be used, or a line can't be made into a column that sews
as it does. Such a line's rails can't be made whole (`SC-E0214`, `SC-W0213`), or they cross each other,
or the satin effect's push compensation lengthens both of the line's ends when its column needs a rung at
one.
Before 0.15, which drops the records a filter gives, it declines with `SC-E0813`.

M6.4 measured making the rails inside VectorCraft's host at up to 162 million instructions, where a light
run gets about 52 million. The rails are the line offset as GEOS offsets it, which builds the boundary of
the whole stroke for each rail. So Stroke to Satin is a heavy filter (P4): VectorCraft 0.17 gives each run 1 G
instructions and 300 s, once the user allows it at install, and runs it in the background. The contract
test `req_vc_014_stroke_to_satin_makes_columns_that_export_as_the_lines_did` converts M6.4's lines in
VectorCraft 0.17.0's and 0.18.0's hosts alike, 3 mm wide, as `plugin.run` runs a filter, and StitchCraft's
PES exporter writes each column's file as it wrote the line's (2026-10-29):

| Line, 3 mm wide | Instructions | Of those allowed |
|---|---|---|
| A zigzag of 10 corners | 18 million | 2 % |
| An S curve 30 mm long | 50 million | 5 % |
| A circle 10 mm across | 73 million | 7 % |
| A circle 30 mm across | 91 million | 9 % |
| An S curve 60 mm long | 91 million | 9 % |
| A circle 50 mm across | 233 million | 23 % |

That is at most 360 instructions for each unit of the engine's work, which the plug-in prices at 450, and
the filter stops with `SC-E0004` before VectorCraft would. A VectorCraft before 0.17 runs it as a light
filter, which almost every curved line outruns, and stops it with its own error.

**Test Sheet** (`REQ-VC-015`, `apps/stitchcraft-vc-plugin/src/testsheet.rs`) draws one of the sheets
drawn as designs, TS-02B to TS-09, TS-11A, TS-11B, the TS-13 sheets and TS-17A, into the document: the
sheet's design moved by whole millimetres so that its middle is nearest the middle of the artboard
`_context` names (`Design::moved`). TS-17A moves by whole multiples of 6 mm, which keep its crosses' cells on
the canvas's grids of 2 and 3 mm (`TestSheet::step`). The turned square's grid starts at the square's
corner, and moves with it. Each element is an object named by its id, with its path and its thread as
the paint its family sews with, a linear gradient fill's gradient as its fill ([below](#a-gradients-geometry)).
Its settings are its family's effect record, each the
value its dialog row takes, which reads back as the setting it was. The objects selected are given back as
they are, and the sheet goes above them. The document sews as the sheet's design moved. A fill's rows lie where the design's
origin puts them, so moved they may fall elsewhere across a shape, and the sheet looks the same. The
threads lose their names in the Brother palette, since VectorCraft unlinks the swatch of a paint a filter
gives, and the PES file is the same. VectorCraft 0.15 keeps the records, but with an empty selection it
looks to a filter as 0.14 does, which would drop them. Only the `documentData` every filter gets from
0.16 tells them apart, so before 0.16 the filter declines with `SC-E0813`. It declines with `SC-E0817`
where parts of the sheet would lie more than 10 m from the document's origin, farther than StitchCraft
stitches (`Design::new`). The sheets drawn stitch by stitch are plans, which no object holds. TS-11C's
meander fills wander as their elements' ids draw, and the objects drawn would have ids of their own, so
the filter doesn't draw it either (`TestSheet::drawable`). The TS-13 sheets are designs whose shapes
StitchCraft digitizes automatically (`TestSheet::digitized`): each such shape has the Auto effect's
record (M13.7, `REQ-VC-016`), and StitchCraft Auto must be installed for VectorCraft to keep it.
A record is made once for each set of settings, and a solid paint once for each thread: objects that share
them share the one made. TS-13F's 179 objects have 15 records and 2 paints, and making them for each
object took it past a light filter's run.

The contract test `req_vc_015_each_test_sheet_draws_into_a_document_that_exports_within_vectorcraft_s_limits`
draws each sheet into a new document in VectorCraft 0.17.0's and 0.18.0's hosts alike, and exports it with
the PES exporter (2026-10-29). Drawing takes 14 to 25 million instructions, at most half of a light filter's run.
Exporting takes 23 million to 1.6 billion, at most 17 % of an export's, the most for the fills of TS-07,
TS-08 and TS-09. TS-17B joined the sheets drawn in M10.7, after that measure, and left them in M10.9. The
test prints each sheet's costs.

The TS-13 sheets, digitized automatically, cost far more. On 2026-11-16 TS-13A, TS-13B and TS-13C each
took 7.9 to 8.0 billion instructions to export, and TS-13D with its 4 pictures 10.872 of the 10.905 billion
allowed. Drawing TS-13D took 49.5 of a light filter's 50.1 million. M13.31 moved its buttons to TS-13E, and
the test now fails a sheet whose export leaves too little room
([compatibility gate](compatibility-gate.md#level-a--abi-contract-fast-minutes)). The filter's help has a
line for each sheet under "The test sheet to draw:", in the 1,000 characters VectorCraft shows of a help.

### Tools: the routing filters

From M10.4 and M10.13, 2 filters in Object › Plug-ins route the selection with the engine's routing
function, as Ink/Stitch's tools route theirs ([routing tools](routing.md),
[ADR-0017](adr/0017-routing-is-one-engine-function.md)). StitchCraft Auto-Route Satin Columns
(`<ns>.route-satins`) routes satin columns and the strokes among them, and StitchCraft Auto-Route Running
Stitch (`<ns>.route-runs`) routes strokes.

**What they read.** VectorCraft gives a filter the selected paths in paint order, with the paths in the
selected groups and layers, and Preserve order keeps that order. Each path routes as a satin column when
the first of its own StitchCraft effects that is on is the satin effect, and as a stroke when it is the
stroke effect. Auto-Route Running Stitch takes the strokes alone, and leaves a satin column as it is, as
Ink/Stitch's tool leaves one. VectorCraft gives a
filter each path's own effect records, and none of its groups'. So a path that sews by a group's effect
(`REQ-VC-006`) stays as it is, as Ink/Stitch's tool leaves a fill. So does a path without a StitchCraft
effect of its own. A selection with nothing to route is refused with a coded error, whose hint says to
put the effect on the paths themselves. Objects named `stitch:route-start` and `stitch:route-end` among
the paths give the route's ends ([start and end](routing.md#start-and-end)).

**What they write.** A column's piece is a compound path of its rails and rungs with the column's satin
effect, drawn as Stroke to Satin draws a column. A stroke's stretch, and a run under the others, is a path
with the stroke effect, which for a run under a column keeps the column's `underlays_first`, and for a run
under the others its stroke's ([order](routing.md#order)). Each is painted with its element's stroke and
named as Ink/Stitch names it, and Trim jump stitches turns on `trim_after` in the effect of the piece before
a long jump. In one undo step, the pieces replace the paths they come from and the markers used. A filter
can't make a group, where
Ink/Stitch puts its route in a new group after the last element. So the pieces go in their sewing order
where the topmost path routed was, in its group or layer. With Preserve order, Ink/Stitch moves each piece
to the end of its element's group, and the filter puts the pieces of each path, the runs along it among
them, where the path was, in sewing order. With Keep original paths, an option of Auto-Route Satin Columns
alone, the paths and markers stay as they are, and the route goes above the topmost path. A group the run leaves empty stays in the document. Each
piece keeps the other effects of the path it comes from, as Stroke to Satin keeps a line's. Its paint is
that path's stroke 1 CSS pixel wide, or black for a path without a stroke.

**Budget.** Both are heavy filters (VectorCraft 0.17), as Stroke to Satin is. Each run gets 1 G
instructions and 300 s, once the user allows the filter at install, and runs in the background, with no
preview in its dialog. Stroke to Satin's run, which makes a column's rails from its centre line, took up to
233 million instructions for one line ([above](#tools-stroke-to-satin-and-test-sheet)). A route makes the
rails of every column drawn as one path that it routes. The engine's work is priced in instructions, as Stroke to Satin
prices it: 450 a unit, a tenth of the run's spend for reading the paths and 8 tenths for routing them.
The filter stops with `SC-E0004` before VectorCraft would stop it. Auto-Route Running Stitch doesn't make
rails, and most of its work for many lines is comparing them 2 by 2 for the cuts, at the same price. In
VectorCraft's host, a contract test routes TS-17C's letters, drawn as lines with the satin effect, and
another its network of lines with the stroke effect, and each prints what the route takes. With
VectorCraft's command line, a Level B case of each filter routes 2 or 3 lines and checks that the route
exports as `stitch export` sews it.

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
| One anchor, written by hand | 18 to 20 bytes along a stroke or a satin column, 26 to 27 in a fill's gaps |
| A unit of the engine's work for a stroke | about 880 instructions |
| A unit of the engine's work for a satin column | about 4,730 instructions |
| A unit of the engine's work making a satin column's rails from its centre line (M6.8, VectorCraft 0.16.0) | about 330 instructions |
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
| Exact | The stitch polyline (strokes, satins), or the fill's shape with a gap along each stitch (fills) | For a stroke or a satin column, within nine tenths of the budget |
| Rows | The fill's shape with a gap along each row segment of its top rows, or along each side of a contour fill's path round its rings (from M7.7), a meander fill's path (from M7.8) or a circular fill's pieces of spiral (from M7.9): where its stitches lie and which way they run | For a fill, within eight tenths |
| Outline | The object's path as lines for a stroke or a satin column, and a fill's shape with a gap along a hatch line every 3 mm in its direction | When the others cost too much |

**A fill reads as solid** (M6.16, `REQ-VC-024`). VectorCraft paints what a live effect returns with the
object's own fill, and thread covers a fill whole. Until M6.16 a fill came back as thin ribbons, one 0.2 mm
wide along each stitch, row or hatch line and along its outline, and the fill painted only those. The
rows' ribbons covered eight tenths of the shape. The outline's covered less than a tenth, so that a large
fill, which auto shows by its outline, looked emptier with the effect than without. Now a fill comes back
as its own shape, flattened, and each stitch, row or hatch line is a gap in it
(`stitchcraft_vectorcraft::effect_solid`). A gap is a ribbon wound against the shape around it, so that
both fill rules leave it empty. The winding number of the shape at the middle of the line tells which way
that is. It also tells whether the fill rule fills there. A line outside the shape is left without a gap,
since the fill would paint a ribbon there. Telling tests every side of the outline for each line, and
that work is charged to the budget, 64 sides to a unit. A gap along a stitch or a row is 0.01 mm wide,
and one along a hatch line 0.1 mm, since hatch lines lie far apart. The plug-in's case
`req_vc_024_a_fill_s_preview_covers_its_shape_at_every_level_auto_picks` measures the share of each
shape's area that its preview covers, on the browser demo's badge and on squares from 3.5 mm to 903 mm:

| Preview | Covered before M6.16 | Covered from M6.16 |
|---|---|---|
| A fill's rows, on squares from 3.5 mm to 564 mm a side | 80 % | 96 % |
| A fill's outline with its hatch | 6 to 16 % | 96 to 97 % |
| A fill's outline without its hatch, as auto shows the largest | under 1 % | 100 % |
| A small fill's every stitch | 80 % or less | 94 to 95 % |
| The Auto effect on the badge's ridges, which shows their outlines | 10 % | 96 to 97 % |
| The Auto effect on the badge's sky, border and moon, which shows their outlines without a hatch | 1 to 5 % | 100 % |
| The Auto effect on the badge's lake, star and snowcap, which shows a satin column's stitches or a fill's rows | 79 to 88 % | 93 to 95 % |

A satin column's stitches end outside the edge of a small shape, as its thread does. Their gaps show there
as hairlines of the fill, which add less than 3 % of the star's area. A stroke's and a satin column's own
effects return lines, which the object's stroke draws, and they are unchanged.

The shape costs about what the ribbons cost. Its outline takes one anchor a point where it took a ribbon
a side, and telling which way the gaps are wound is little work beside laying the rows. Measured in
VectorCraft 0.24.1's host with the contract crate's runs (2026-11-15), before and after:

| Preview | Instructions before | Instructions from M6.16 |
|---|---|---|
| A fill's rows, a square 3.5 mm a side | 2.77 million | 2.82 million |
| A fill's rows, a square 226 mm a side | 12.7 million | 13.5 million |
| A fill's outline, a square 3.5 mm a side | 3.00 million | 2.99 million |
| A fill's outline, a square 903 mm a side | 6.46 million | 6.67 million |
| The Auto effect at auto, a filled square 56 mm a side | 12.8 million | 13.1 million |
| The Auto effect at auto, a filled ring 30 mm across | 30.6 million | 30.3 million |
| The Auto effect's outline, a filled square 903 mm a side | 4.82 million | 5.04 million |

The `preview_level` parameter, `auto` by default, lets the user pick a level (VectorCraft reserves the
name `preview`). A level the user picks gets the whole budget, and when that is not enough, `SC-I0810`
names a cheaper one. The units of a stroke's work and of a satin column's cost the same on every shape
measured, so a budget of units bounds their exact previews. A fill's units vary too much for that. A 7 mm
square's exact preview already takes more than half of what VectorCraft allows, and a budget of units that
ran out early, in the dearer work, took 71 million instructions. So auto shows a fill's rows, and its exact
preview is the user's to pick, with all that VectorCraft allows, which stops one that needs more with its
own error. A satin column drawn as one path makes its rails from its centre line first, in many units of
about 330 instructions each, which the satin column's price counts as dear as its stitches. One bend takes
33,100 units, so auto shows the outline of a centre line that bends. Its exact preview is the user's to
pick too: 12.6 million instructions for one bend, 25 million for a quarter circle of 17 points and 46
million for a sine curve of 32 points, of the 51 to 55 million VectorCraft allows them. The
contract test `req_vc_003_every_preview_level_finishes_within_vectorcraft_s_limits` runs each level on
shapes of growing size on every track the [compatibility gate](compatibility-gate.md) tests, and fails
when VectorCraft stops a run that must finish. It prints what each run takes, which the budget's numbers
are measured from. `hello_reads_and_writes_json_within_the_recorded_budget` still measures what JSON
costs. The real stitches are always available from the CLI preview, and the RFC's overlay channel (P3, in
review upstream) would remove this limitation.

**The Auto effect's previews** (M13.7, `REQ-VC-016`). The Auto effect reads its object as automatic
elements, an area when a fill paints it and a line along its topmost stroke. It digitizes them, then shows
each element they become as auto shows its family's: a fill's top rows, and a satin column's and a running
stitch's every stitch. VectorCraft draws what it returns with all of the object's paints. The stitches
come back as gaps in the object's area for the fill when a fill paints the object, and as lines for the stroke otherwise.
Digitizing is most of the work. Measured in VectorCraft 0.18.0's host with the contract crate's runs
(2026-10-31), a unit of it takes about 1,450 instructions for a filled square or an L. Finding the centre
lines of a line traced as an area takes up to 2,260, as for TS-13B's thinnest window line. The plug-in
prices a unit at 2,700:

| Shape, filled | Units of digitizing | Auto shows | Instructions, auto | Instructions, outline |
|---|---|---|---|---|
| A square 14 mm a side | 12,900 | its rows | 20 million | 1.1 million |
| A square 56 mm a side | 12,800 | its outline | 22 million | 1.2 million |
| An L 40 mm tall, its arm 5 mm wide | 21,000 | its outline | 22 million | 1.3 million |
| A ring 30 mm across, 4 mm wide | 31,800 | its outline | 33 million | 7.2 million |
| A line traced as an area 0.3 mm wide, with a branch | 23,100 | its outline | 31 million | 1.3 million |

Auto digitizes within eight tenths of the budget and keeps the rest for the outline it falls back on. A
small simple shape shows its stitches. A larger one, an L, a ring and most traced letters show their
outlines: digitizing them leaves too little for their stitches, or needs more than there is. Exact digitizes and sews within
all that VectorCraft allows, which a filled shape's fill outruns. The command line's preview shows every
stitch, and the RFC's budget classes ([P4](rfc-vectorcraft-abi-v2.md#p4-budget-classes-for-live-effects))
would give these previews room. `req_vc_003_every_preview_level_finishes_within_vectorcraft_s_limits` runs
the Auto effect's levels on these shapes and on lines too, and fails if VectorCraft stops one that must
finish.

Paint caveat: a live effect changes only geometry. A fill preview's shape is painted with the object's
own fill, and a stroke preview with its stroke. From 0.14 a stroke or a satin effect reads its stroke's
style, and still cannot change the paints. Users set the fill/stroke to the thread colour, which they want
anyway for export. An element sewn in several threads (M10.5), as tartan and linear gradient fills are,
previews every group of its stitches with the object's own paints too. A tartan's preview shows one
colour, and a linear gradient fill's the object's gradient, which paints its stitches where they lie. ABI v1 gives the geometry a live effect returns no paint of its own. The exporters and
`stitch export` write each thread, and the command line's preview draws each (`REQ-THREAD-003`).

### Commands in v1

Ink/Stitch's commands are symbols linked to an object, and ABI v1 has nothing to link. `trim_after` and
`stop_after` are ordinary parameters. StitchCraft reads the others from names, an object's or a layer's,
which VectorCraft's Layers panel sets (`REQ-VC-009`). A name is read when it begins with `stitch:`,
after any white space, in any mix of upper and lower case. Its word ends at the first white space, so a
note can follow it:

| Name | Ink/Stitch's command | What StitchCraft does |
|---|---|---|
| `stitch:ignore` | `ignore_object`, `ignore_layer` | Leaves the object or the layer out with everything in it, and lists it with `SC-I0811` |
| `stitch:start`, `stitch:end` | `starting_point`, `ending_point` | Gives the shapes of the group or layer it is in, and of the groups inside it, their start or end position: the centre of the box round its anchors. Their fills and satin columns start or end there (`REQ-GEN-001`). The marker is left out of the stitches. The first marker of each kind that shows counts, a group's own before those of the groups round it. `SC-W0807` names another, one with no anchor that reads, and the markers of a shape digitized automatically, which places its own. A group or a layer is never a marker: `SC-W0807` names it, and what it holds is read as under any other name |
| `stitch:target` | `target_point` | From M10.8, gives the shapes of the group or layer it is in, and of the groups inside it, the point they aim at: the centre of the box round its anchors. Their ripple stitches shrink or spiral to it (`REQ-RIP-002`), and their circular fills turn round it (`REQ-FILL-CIR-008`). It is read as the start and end markers are, and `SC-W0807` names the same ones |
| `stitch:guide`, `stitch:anchor` | Ink/Stitch's guide-line and anchor-line markers, which mark paths rather than tie commands | From M10.11, gives the shapes of the group or layer it is in, and of the groups inside it, the line their ripple stitches' copies follow, or the line across those copies whose ends lie on a satin guide's pairs (`REQ-RIP-012` to `REQ-RIP-014`). The marked path is the guide, and one of several subpaths, or a compound path, is a satin guide, its rails read with a satin column's defaults but for the ripple's `reverse_rails` and `swap_satin_rails`. From M10.9 a guide also gives their guided fills the line their rows follow (`REQ-VC-023`), its first subpath, or with the buffer strategy all of them. In Ink/Stitch too, one guide line steers both. A ripple reads the start and the end of an anchor's first subpath. From M10.10 a guided fill reads the anchor's whole path, which starts its rows that close where it crosses them (`REQ-FILL-GDD-012`). The marker is left out of the stitches. It is read as the start and end markers are, and `SC-W0807` names the same ones |
| `stitch:origin`, `stitch:stop-position` | `origin`, `stop_position` | Gives the design its origin, which goes to the hoop's centre, or the stop position the frame moves to before each stop (`REQ-ASM-005`): the centre of the box round its anchors, wherever the marker is (M8.8). The marker is left out of the stitches. 2 origins, or 2 stop positions in a design with a stop, are an error (`SC-E0819`), as Ink/Stitch doesn't write a file with 2 such commands. `SC-W0807` names a marker with no anchor that reads, which places nothing, and a group or a layer of either name, which is never a marker |
| `stitch:route-start`, `stitch:route-end` | `autoroute_start`, `autoroute_end` | From M10.4, marks where the routing filters start and end a route, at the centre of the box round its anchors, when it is selected with the paths they route. The filters delete it with the paths ([routing filters](#tools-the-routing-filters)). `stitch export` leaves it out of the stitches. Before M10.4 the reader reads the name as the row below says |
| `stitch:text` and the text | | From M14.5, marks text in a picture: lettering of the text takes the place of the traced shapes inside the path, as tall as they were ([text in pictures](image-to-embroidery.md#text-in-pictures), `REQ-VC-028`). The path is read with its paints, so it needs a fill or a stroke, and is left out of the stitches. A group or a layer so named is not a marker, and `SC-W0807` says so |
| Any other word after `stitch:` | | Ignores the name, and `SC-W0807` names the object, in case the word is a misspelt marker |

Hiding an object leaves it out too, without a word, and a name on a hidden object says nothing. A live
effect is given its object's geometry, with its fills and strokes from VectorCraft 0.14 (VectorCraft's
`docs/plugins.md`, Input), never its name. So
VectorCraft's preview still shows the stitches of an object these names leave out, a marker included,
when it has a StitchCraft effect or is in a group that has one. `stitch export` leaves them out. A guided
fill's preview doesn't see the guide beside its object. It sews the tatami fill `SC-W0331` names, and
`stitch export` sews the rows along the guide. The RFC's sixth proposal, still open in that part, would let a live
effect read the markers beside its object, so its preview could start and end at them.

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
uses, and the rest is skipped as it is read: images and previews are never kept, and of a text only its
characters and their fill. Every object,
appearance item, effect and anchor charges the budget, and so does each element for its copies of the
object's path, name and settings. An object's name goes into each element made of it, and a group's
StitchCraft settings into each element of the paths in it. Those copies come to at most 64 MiB, as much
as the JSON read. Each element's copy of its object's path takes 56 bytes a segment, and a long path under
many paints would hold far more than the file: 4 MB drawing 2,500 fills on a path of 200,000 anchors would
hold 28 GB before the budget ran out. So the copies of paths come to at most 32 bytes for each byte of
JSON read, which an object's fill and stroke on its most compact path keep well within. A file whose
copies come to more is refused with `SC-E0806`. A clip group's clip is held once, shared by the elements it
cuts, and costs each of them a unit for each of its segments.

**Coordinates.** VectorCraft keeps every path in document coordinates. Moving or scaling a group moves
its paths' anchors, so layers, groups and compound paths have no transform. Coordinates are points, 1/72
inch, with y down, and the adapter multiplies them by 25.4/72 into millimetres. The document's origin
stays the origin. It is the canvas's origin, through which a cross stitch fill's grid passes
(`canvas_grid_origin`, [cross stitch](algorithms/cross-stitch.md#the-grid)), decided in M10.1. A new
document's first artboard has its top left corner there, and moving an artboard leaves the grid where it is.
Coordinates are read exactly, as VectorCraft reads them, from their text in the JSON.
An anchor's handles are absolute positions. A missing handle, or one within a billionth of a point of its
anchor, leaves that end of the segment straight, and a segment without handles is a line.

**What is stitched.** The layers in order, bottom first, and in each its children in paint order:

- **Paths and compound paths** are the art. A compound path's children are one shape, with the compound's
  appearance and fill rule.
- **Groups and layers** pass their children on, and their StitchCraft effects with them (below). A clip
  group's or a clipping layer's art is cut to its clipping object, as VectorCraft clips it (below).
- **Hidden objects**, and everything in them, are left out without a word, as VectorCraft draws nothing
  for them. So are objects and paints of opacity 0, paints turned off and strokes of width 0.
- **Objects and layers named `stitch:ignore`**, and everything in them, are left out with `SC-I0811`.
  Start and end markers are never stitched ([commands](#commands-in-v1)).
- **Template layers, layers that don't print, and guides** are left out with `SC-I0809`. VectorCraft
  shows them, dimmed or as guides, but prints and exports none of them.
- **A Shaper group's sources** are left out. The group shows the shapes it makes of them, and keeps them
  as its first child, transparent. The Layers panel's Make Clipping Mask (`layer.clippingMask.toggle`)
  puts a Shaper group's clipping object before them.
- **Text** with StitchCraft's Lettering effect sews in an embroidery font ([lettering](#lettering)). Without
  the effect, text under automatic digitizing sews in the default embroidery font, or a bold text in the
  bold font where it sews at the text's size ([below](#text-without-the-lettering-effect),
  [fonts](fonts.md#text-objects-in-automatic-digitizing)). Other text is left out with `SC-W0807`, whose hint names
  the effect and Type › Create Outlines.
- **Images and symbols,** and the objects VectorCraft computes as it draws them (blends, envelopes,
  meshes, repeats, placed documents and VectorCraft's Intertwine objects), are left out with `SC-W0807`.
  The message names VectorCraft's command that makes paths of them. So is a kind of object this version
  doesn't read.

**Paints and threads.** An object without StitchCraft's effects is stitched as Ink/Stitch stitches an SVG
shape. Its visible fills are filled, in the appearance's order, and its visible strokes then have a running
stitch each. Since M13.1, read with `--auto` (`stitchcraft_vectorcraft::read_with`), the elements of an
object are automatic instead when neither it nor a group or layer around it has a StitchCraft effect
([image to embroidery](image-to-embroidery.md), `REQ-AUTO-001`). StitchCraft then chooses their stitches by
its rules. Since M13.7 StitchCraft's Auto effect makes them automatic without the switch (`REQ-VC-016`).
It is read where a stitch effect is read, on the object or on a group or a layer around it, the nearest
first, and its parameters are the object's settings of the rules. A stitch effect says how an object sews,
so an Auto effect beside one on the same object or group isn't used, and `SC-W0807` says so. Read with `--sort-colours`, the objects with effects sew in the order StitchCraft chooses too
(`REQ-AUTO-011`). A stroke's cap is read with its width and join, and an automatic line's stitches follow its
round or square caps. Of a line sewn across its width, the reader names the alignment inside or outside a
closed path and the width profile, which its stitches don't follow (`SC-W0807`, `REQ-AUTO-005`). The
thread's colour is a solid paint's colour, with CMYK and grey converted by
VectorCraft's formula without colour management ((1 − c)(1 − k) and 1 − k) and Lab through sRGB. A
gradient gives the colour of its first stop VectorCraft shows, of an opacity other than 0, with `SC-W0807`
unless every stop is shown in that colour. With `fill_method` set to `linear_gradient_fill`, the fill takes
that colour without a word and sews the gradient ([below](#a-gradients-geometry)). Gradients whose stops are
all of opacity 0 paint nothing, as those without stops do, and are left out without a word. A pattern
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

**Clip groups** (`REQ-VC-017`, M8.7). A group or a layer whose kind has `"clip": true` clips what it holds
to its first object, the clipping object. Object › Clipping Mask › Make wraps the objects selected in such a
group, the topmost first, which must be a path, a compound path or text. It takes that object's paints and
effects away, and marks a path `"clipping": true` (`clip_make` in `crates/engine`). The Layers panel's
Make Clipping Mask clips a layer or a group the same way. VectorCraft clips to the clipping object's area
whatever its paints, hidden or not (`Node::clip_shapes` in `crates/doc`, read at 0.19.0, and alike from
0.13):

- A path's area is the one its fill rule fills, and a compound path's is its paths' together, by its own
  rule, holes and all.
- A nested clip group's is its own clipping object's.
- Another group's or a layer's is that of its members that show, together, but a Shaper group's sources.
- Text's is its glyphs' outlines, an image's or a placed document's its frame, and a live object's what it
  draws. A guide or a symbol doesn't add to it.

One shape keeps its rule, several are united, and a clipping object of none hides the group's contents.
The adapter makes the clipping object's area into a clip of each element made inside the group, however
deep, as the SVG adapter does with a clip path (`Element::clips`). Clip groups inside one another give one
clip each, and the engine cuts an element to all of them (`REQ-GEN-005`). It cuts a fill's area and a
stroke's lines, each piece sewn on its own. Satin columns are sewn whole, as in Ink/Stitch (`SC-W0217`),
and an element wholly outside its clips isn't sewn (`SC-W0013`). An automatic shape is cut before it is
digitized. The clipping object isn't cut to its own clip, as VectorCraft paints its fills inside its own
area and its strokes over the clip's edge. The clips round its group do cut it.

The adapter doesn't read glyphs, frames or what a live object draws. Text, an image, a placed document, a
live object or a kind it doesn't know adds nothing to the clip, and `SC-W0807` names it, with the
VectorCraft command that makes paths of it, such as Type › Create Outlines. A path in the clipping object
whose geometry can't be used doesn't add to the clip either, and `SC-W0808` names it. A clip left with no
shape cuts away all of what it clips, where VectorCraft shows what lies in the glyphs or the frame.

VectorCraft runs a group's geometry effects on every path inside it, the clipping objects of the clip groups
it holds among them (`for_each_leaf` in `crates/effects`), and clips to what they return. Where StitchCraft's
effects apply to a clipping object, the preview is cut to the stitches they draw of it, a fill's rows or a
stroke's line. What sews is cut to the clipping object as it is drawn, and `SC-W0807` says so. Its hint is
to apply the effects to the objects the group clips.

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

#### A gradient's geometry

A linear gradient fill made in VectorCraft sews the linear gradient of the object's topmost fill that
paints, in `stitch export` and in StitchCraft Fill's live effect alike (`REQ-VC-022`, M10.12). VectorCraft's
gradient paint keeps its colours apart from where it lies: `{"type": "gradient", "gradient": {"kind",
"stops"}, "geom"?, "angle"}`. Its `geom` gives the start and end in the document's coordinates, in points,
which the reader turns into millimetres. VectorCraft fits a gradient without one to its object each time it
draws it. The fitted gradient runs through the middle of the object's bounds round its curves, at `angle`
counter-clockwise as the canvas shows it. It is as long as the bounds are across that way. The reader fits
it alike, charging the budget for each segment it measures (`GradientGeom::fit` in VectorCraft's `crates/color`, read for 0.17.0 to
0.21.0, and `paint.setGradientGeom` in its `docs/mcp.md`).

Each stop's offset is kept from 0 to 1 and never before the stop before, and its opacity from 0 to 1, 1 where
the file leaves it out. Its colour converts as a solid paint's does, and one that doesn't read is black, with
`SC-W0807`. A stop's midpoint moves where VectorCraft draws the 2 colours half mixed, which rows of whole
colours can't follow, and `SC-W0807` names a midpoint other than half way. A gradient more than 10 m from the
origin is named too, and the fill sews its first colour. A radial or freeform gradient, and one of one stop,
is sewn in its first colour, as other fill methods sew a gradient. A fill without a linear gradient fades its
own colour out (`SC-W0329`).

**Decision: the Test Sheet filter draws gradients.** A filter's output may hold gradient paints in the native format's
form, `geom` among them, which VectorCraft clamps and keeps (its `docs/plugins.md`, Output). So Test Sheet
draws a linear gradient fill with its gradient as a gradient paint, its `geom` in points, and the document
sews as the sheet's design (`apps/stitchcraft-vc-plugin/src/testsheet/tests.rs`). Rejected: a solid paint of
the first colour, which would sew the fade of `SC-W0329`. A gradient without its `geom` was rejected too:
VectorCraft would fit it to the object's bounds at its angle, not where the sheet puts it. The filter draws
TS-17B, which holds gradients from M10.12, and its draw-every-sheet case sews them as the sheet does.

### Lettering

M14.4 sews a text object in an embroidery font (`REQ-VC-026`, `REQ-VC-027`). [Fonts](fonts.md#lettering)
describes lettering itself, and [fonts built into the plug-ins](fonts.md#fonts-built-into-the-plug-ins)
where a plug-in finds a font.

**A text object as a plug-in gets it,** read in VectorCraft v0.24.1 (`docs/plugins.md`,
`crates/doc/src/text.rs` and `crates/plugins/src/effect.rs` of its source):

| Plug-in | What it is given of text |
|---|---|
| Exporter | The whole document. A text object has its runs, with their characters and character styles, and its kind, its transform and its effect records |
| Filter that lists `inputs: ["type"]`, from API level 2 | The selected text, read only: its runs, its paragraphs' styles and its transform |
| Live effect | The object's outlines as one path, with its paints. The text is not among them |

No plug-in is given a font's data or a glyph's outlines. An exporter can read the text and can't outline
it, so M14.4 sets it in a font of StitchCraft's own. A live effect can't even read the text, so
it can't draw the embroidery font before export
([RFC](rfc-vectorcraft-abi-v2.md), P6).

**The setting is an effect on the text.** StitchCraft Lettering (`<ns>.lettering`) is a live
effect that a person applies to a text object. Its record keeps the font's name and lettering's settings,
and its run returns the geometry it was given, as *hello* does. 2 other ways were weighed:

- A filter that replaces the text by lettering's shapes. The text could no longer be retyped, and the
  shapes are satin columns that mean nothing as art.
- A name kept with the document, by the Design Settings filter. One font would serve every text, and a
  text's settings would lie far from it.

With the effect the text is still a text object. Retyping it changes what sews, the Appearance panel shows the effect
and opens its dialog, and the exporter finds the record on the object it belongs to.

**The dialog** is generated from lettering's own settings (`stitchcraft_svg::lettering::SETTINGS`), each
with its default and help, by Ink/Stitch's names. It has 9 rows, which fit VectorCraft's dialog without
sections:

| Row | Setting | Default |
|---|---|---|
| Font | `font`, a text | empty, which is `small_font` |
| Size | `scale`, in percent of the font's size | 100 |
| Align | `text_align` | `left` |
| Letter, word and line spacing | `letter_spacing`, `word_spacing`, `line_height`, in millimetres | 0 |
| Sew lines back and forth | `back_and_forth` | on |
| Sort colours | `color_sort` | `off` |
| Trim | `trim` | `off`, for a machine that cuts no thread |

The 2 settings that place a text along a path are left out, since a text object sews at its own place.

**The font is a text field,** where a list of the fonts built in was the other choice. A font is named by its folder, as
Ink/Stitch finds it, or by the `name` of its `font.json`, in any case. A list would make the effect's
manifest differ between a build with fonts and one without, and VectorCraft checks a record's value
against the manifest installed. A document would then lose its font in a plug-in built with other fonts.
A name also reads the same from the command line. The dialog's help names the default font, and an
export that doesn't find a font names those it has.

#### Text without the Lettering effect

From M14.7 a text object under automatic digitizing sews without the effect too (`REQ-VC-029`), as
[a text object in automatic digitizing](fonts.md#text-objects-in-automatic-digitizing) does. A text is
under automatic digitizing in the cases a path is:

- The text has the Auto effect.
- The text has no StitchCraft effect of its own, and the nearest group or layer around it that has
  StitchCraft effects has the Auto effect and no stitch effect.
- The document is read with `--auto`, and neither the text nor a group or a layer around it has a
  StitchCraft effect.

A stitch effect says how a path sews and means nothing for a text. A text with one, its own or a group's
around it, is left out as before.

The adapter reads horizontal point type. Its lines are the lines of its characters. The first starts at
the origin of `xf`, and each next line lies a leading further down. The leading is the character style's
`leading`, or 1.2 times its `size` when it sets none, which is VectorCraft's automatic leading. The size,
the leading, the fill and the typeface's name are those of the first run that has characters, so a text of
several sizes sews at its first. The paragraph's `justify` anchors each line at its start, its middle or
its end, and a justified paragraph is anchored at its start. `xf` turns the lines as it turns the
baseline, and scales the size and the leading as it scales across the baseline. A size that doesn't read
is 12 points, and `SC-W0807` says so. The document states no length for a line, so each is fitted to an
estimated one. Area type, type on a path and vertical type are left out with
`SC-W1111`, as they are with the Lettering effect.

Only a plug-in that is given the text can do this:

| Where | A text automatic digitizing reaches |
|---|---|
| File › Export to PES or DST, and the browser demo | Sews in the font built into the exporters |
| `stitch export` and `stitch plan` | Sews in the default font of the folder `STITCHCRAFT_FONTS` names |
| The Auto effect's own run, which draws the stitches on the canvas | Is given the text's outline as one path, and digitizes that as a shape. So the canvas doesn't show what the export sews |

**What the adapter reads.** A text object's kind holds its runs, its transform `xf` and whether it is
point type. The adapter joins the runs' characters, in order, and lettering splits the lines at line
feeds. An inline graphic's character is left out and said (`SC-W0807`).

- **Place.** Point type's origin is the start of its first baseline, and that is the origin of a text
  lettering sets. The text is turned as `xf` turns the baseline. The size is the effect's `scale`, since
  an embroidery font sews only within its own heights, so the scale of `xf` and the screen font's size
  aren't used. A mirrored or slanted text sews turned only (`SC-W0807`).
- **Thread.** A font that sews in one thread sews in the colour the text's first characters are filled
  with, and a font of several colours keeps its own.
- **Clips.** The clips around the object cut the letters, as they cut a path's elements. As a clip
  group's clipping object a text is left out, as before.
- **What isn't set.** Area type wraps its lines by the screen font's widths, and type on a path follows
  its path by them. An embroidery font has other widths, so neither can sew where VectorCraft draws it.
  Vertical type needs glyphs drawn to sew downwards. Each is left out with `SC-W1111`. M14.3's lettering
  along a path could take type on a path in a later step.
- **What is said.** The warnings of lettering and of the font's reader are passed on, a font's once
  however many texts name it. Their notes for information aren't, since they are said of every glyph.
  `stitch lettering` prints them. A message that names the fonts found names 8 and counts the others.
- **Other effects** on the text, and the screen font's attributes, change what VectorCraft draws and
  nothing that sews.

**Before export** the text looks as VectorCraft draws it, in its screen font. Nothing on the canvas
shows the embroidery font, its size or its width. The user page says so, and gives the font's height in
millimetres for a scale. When the text can't sew, the export says `SC-W1111`: the font isn't found, with
the fonts that are, or the text is of a kind that isn't set. VectorCraft shows a declined export's
message without its warnings, and a document of one text whose font is missing has nothing to stitch. So
a declined export says its first `SC-W1111` after its error.

**The layer edge.** The adapter sets the text itself, so that the plug-in and `stitch` share one reader.
It depends on `stitchcraft-svg`, where lettering and the font reader are, beside it in layer 3. The edge
is listed in `xtask/src/layers.rs`. The other choice was a second copy of the placing in each app.

**Cost.** The Level A case `req_vc_027_…` measures a lettered document's export in VectorCraft's host
against the 10 billion instructions it allows, and fails above 85 percent. Measured on 16 November 2026 in
VectorCraft 0.24.1, in Ink/Stitch Small Font:

| Text | Instructions | Of those allowed |
|---|---|---|
| A word of 5 letters | 2.01 billion | 20 % |
| 4 lines of 2 words | 2.45 billion | 24 % |

Reading the font takes most of it, about 2 billion instructions: both its glyph files are read whole,
all 187 glyphs, whichever the text uses. The font is read once for a document, however many texts name it. Reading only
the glyphs a text names would cut that, and is left for a step that needs it.

Each exporter's module grew from 3,525 to 5,304 KiB. The font's files are 810 KiB of that, and the SVG
reader and lettering, which an exporter didn't contain before, are the other 969 KiB. The modules of the
effects and filters grew by 2 KiB each.

### Importing Ink/Stitch SVGs into VectorCraft

Because VectorCraft up to 0.16 drops `inkstitch:*` attributes on SVG import, phase 1 offers
`stitch import-inkstitch design.svg -o design.vectorcraft`: our SVG adapter reads the Ink/Stitch file
and writes a VectorCraft document with the same art and the parameters as live-effect records (M8.12,
after 1.0).
From 0.17 VectorCraft keeps the attributes from SVG import to SVG export, but its live effects read none
of them. [ADR-0016](adr/0016-inkstitch-files-imported-by-stitchcraft.md) keeps `stitch import-inkstitch`
as the way in, after spike M8.11: VectorCraft gives a plug-in no attribute of another application, and
its import loses what Ink/Stitch's settings depend on.

## 5. Phase 2: plug-in features proposed upstream (summary)

Detailed in [the RFC](rfc-vectorcraft-abi-v2.md), which began as a proposal for an ABI v2. VectorCraft
took most of it into ABI v1 instead, each as an addition older hosts ignore or refuse to install. Every
item is generic, useful to any plug-in and not only embroidery:

| Proposal | In VectorCraft | StitchCraft's use |
|---|---|---|
| 1. Exporter plug-ins, for File › Export › PES | 0.14.0 | M6.9 |
| 2. Rich parameter schemas: labels, units, groups, help, conditions, texts and lists, 256 parameters | 0.14.0 | M6.10 |
| 3. Display-only overlays from live effects, real stitch previews at low cost | in review | M9 |
| 4. Budget classes, with the user's consent | heavy filters in 0.17.0, heavy live effects proposed | M6.12 (Stroke to Satin), M10.4 and M10.13 (the routing filters), M9 |
| 5. Document-level plug-in data, for the machine profile and palettes | 0.16.0, for filters and exporters | M6.11 |
| 6. Read-only context for live effects | the object's paints in 0.14.0, the markers beside it proposed | M6.8, M9 |
| 7. SVG foreign-attribute preservation | 0.17.0 | Ink/Stitch files keep their settings through VectorCraft |
| 8. Effects and stroke style in filter objects | 0.15.0 | M6.12 (Stroke to Satin, Test Sheet) |

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

## Browser distribution

M6.13 publishes VectorCraft's web editor with StitchCraft's plug-ins on the project's Pages site. The work
came from crowecawcaw's pull request #260. [Try StitchCraft in your browser](../browser-demo.md) says what
a visitor sees, `web/README.md` how to build it, and the [docs pipeline](docs-pipeline.md#publishing) how
it is published.

### The editor and its release

`web/build.py` builds the editor with Trunk from VectorCraft's source at a release, and doesn't change
it. It writes 2 folders. The app is the editor with the plug-ins, and the demo folder is the plug-ins, the
loader and the starter patch without the editor.

`compat/vectorcraft.toml` records the release beside the gate's pins, as a tag and the commit the tag
names, and the build stops when the tag names another commit. The release is the stable track's, from
0.24 on: the loader calls `uiInspect` and `docInspect`, which VectorCraft's page has from that release. A
test fails when the demo's release is older than the stable track's.

The loader touches VectorCraft only through the calls its page offers as `window.vectorcraft`. VectorCraft
documents them in its `docs/control-protocol.md`, under "In the browser". The loader installs each light
module with `plugin.install` and opens the starter drawing with `document.open`. It doesn't install the heavy
filters, which VectorCraft installs only once a person allows it, and the page offers them as files.

Trunk runs without the network. The workflow installs Trunk and the 2 tools Trunk would otherwise download,
wasm-bindgen and wasm-opt, each checked against a checksum kept in the repository
(`.github/actions/tools/action.yml`).

The editor is built once for each state of what it is built from. `web/build.py` keeps the built editor
in `target/web/editor` under a key. The key is a SHA-256 of 4 things: the release as
`compat/vectorcraft.toml` records it, `rust-toolchain.toml`, the versions of Trunk, wasm-bindgen and
wasm-opt in `.github/actions/tools/action.yml`, and `web/build.py` itself. A build that finds an editor under its key copies it and builds only the plug-ins, which are
always this revision's. `web.yml` keeps that folder in the Actions cache under the same key, with the
runner's system before it, and restores no other key. So a pull request that changes the release, the
compiler, a tool's version or the build script builds the editor, and every other round takes it from the
cache.

A kept editor is not checked against a checksum, because 2 builds of it don't give the same bytes. What
protects it is where a cache may come from. GitHub gives a run the caches written on its own branch and on
`main`, and a run for a pull request writes to that pull request's own scope, a fork's too. So the editor that
`main` publishes was built by a run on `main`, from code that was merged, and no pull request can put
another under its key.

### Licences

VectorCraft's code is licensed MIT OR Apache-2.0, which allows publishing a build of it with its licence
texts and notice. The app has them as `VectorCraft-*.txt`, beside StitchCraft's own.

The ArtCraft name and logos are not under that licence. VectorCraft's `docs/brand/LICENSE-brand.txt` says
that a published work derived from VectorCraft carries none of the marks and doesn't present itself as an
ArtCraft product, and that it may say in plain text what it is based on. What was checked, by reading
VectorCraft at the recorded commit on 2026-11-15:

- No Rust source, manifest or page of VectorCraft names a file of `docs/brand/`, but a test of its `xtask`.
  The editor's build doesn't embed a logo.
- The editor's own text names ArtCraft: the Help menu and the home screen link to its website and its
  community, and the About dialog says "Part of ArtCraft". The build leaves that text in, because it doesn't
  change the source. The text says who makes VectorCraft. The owner read the brand licence and decided on
  2026-11-14 that the text stays: the editor is VectorCraft's own, unchanged, and StitchCraft is a plug-in
  for it.
- What the build adds says "StitchCraft in VectorCraft" in plain text: the page's title, the panel and the
  book. The panel says that the build is unofficial and made by the StitchCraft project.

A preview doesn't contain the editor. Of VectorCraft it publishes only pictures of the editor's window.

### Sizes on the site

The editor is published once, at `/app/`, and a preview contains what its pull request can change. The
Pages branch is one commit. The [docs pipeline](docs-pipeline.md#one-publisher-and-a-branch-of-one-commit)
says how both are kept.

The first build measured them on 2026-11-15, for VectorCraft v0.24.1. The `web` workflow's build job prints
the table of every build, in the step that lists the site's files as stored and as sent.

| Folder | What it contains | Bytes | Bytes sent, gzip -6 |
|---|---|---:|---:|
| `/app/` | The editor and the plug-ins | 116,987,438 | 40,752,318 |
| `/app/`, the editor's module alone | `vectorcraft-web-*_bg.wasm` | 97,474,876 | 34,249,458 |
| A preview's `demo/` | The plug-ins, the loader, the starter patch | 18,869,636 | 6,291,317 |

The preview of pull request 266, its book with its `demo/` and the 3 pictures, took 40.6 MB of the branch.
The editor's module is 5 times the size of a preview's `demo/`, and no preview contains it. M6.17's panel
exports added 3 scripts of 8,639 bytes together, and no module.

### The starter drawing

From M6.14 a session without a document opens Alpine Night from `web/templates/alpine-night.svg`, in a new
document: the plain drawing, without a StitchCraft effect. M6.13 opened the finished design. The owner
tried it on 2026-11-14 and had expected to turn the drawing into stitches herself.

The loader opens the SVG and not a second template without effects. VectorCraft made the finished example
by opening this SVG and applying the effect, and one file of the art can't drift from another. The browser
check fails when VectorCraft answers the SVG with a warning.

The panel's first button turns the document in front into stitches. It selects everything with `select.all`
and reads each shape's effects from `effect.list`. Then `effect.apply` gives the Auto effect to the shapes
that don't have a StitchCraft effect, and a shape with one keeps it. VectorCraft's list of commands
documents each call.

A lesser button opens the finished example in a new document, from
`web/templates/alpine-night.vctemplate`, a template in VectorCraft's own format. VectorCraft wrote it from
the SVG, with the Auto effect on each shape, and `web/generate-template.mjs` writes it again.

The browser check takes its pictures along that path. The first 2 show the drawing as it opens and after
the button, and the last shows the Auto effect's dialog. It opens the design settings' dialog too, without a picture. M6.13
showed that dialog, which is not on this path, and every preview keeps each picture on the Pages branch. `docs/fixtures/README.md` records
the art's author and licence.
