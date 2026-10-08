# VectorCraft integration

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
| Releases | Tags `v0.1.0` … `v0.6.0`; pushing the `release` branch drafts a GitHub Release; SemVer pre-releases (`-rc.1`) are flagged as pre-releases | `.github/workflows/release.yml` |

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
| 2. ABI v2 (upstream RFC) | generic plug-in features | export from File › Export, real stitch previews, rich parameter panels, progress | M9 |
| 3. In-tree crate (optional) | ArtCraft's interest, or a fork | native speed, simulator panel, satin tool | post-1.0 |

## 4. Phase 1 design (ABI v1, no VectorCraft change)

### Plug-ins

One `.wasm` per stitch family keeps each dialog relevant and under 64 parameters:

| Plug-in id | Kind | Stitch types | Notes |
|---|---|---|---|
| `<ns>.running` | live effect | running, bean, manual (P1); zigzag, ripple (later) | `stroke_method` choice selects the variant |
| `<ns>.satin` | live effect | satin (P1); E, S, zigzag (P2) | rails and rungs from the object's compound path |
| `<ns>.fill` | live effect | tatami (P1); contour, meander, circular (P2) | `fill_method` choice |
| `<ns>.tools` | object filter | make satin from two paths, add rung, bake preview to paths, generate test sheet | one undo step each |

`<ns>` is a reverse-DNS namespace defined in one constant (`PLUGIN_NAMESPACE`) in the plug-in crate.
Manifests are generated from the [parameter registry](params.md#splitting-for-vectorcraft-abi-v1); a
test validates each against VectorCraft's manifest rules (id charset, ≤ 64 parameters, names, ranges) so
an invalid manifest never reaches a user.

### The workflow

1. Draw the art in VectorCraft as usual.
2. Select an object → **Effect › Plug-ins › StitchCraft Fill** (or Running / Satin) → set parameters in
   the generated dialog. The parameters are now stored on the object (undoable, saved in the
   `.vectorcraft` file) and the object shows a stitch preview.
3. Save the document.
4. `stitch export design.vectorcraft --profile brother-200x200 -o design.pes` writes the machine file and
   a report; `--preview design.png` renders what will sew.

Stitching order is paint order (bottom first). Hidden objects are skipped (and listed in the report);
locked objects are stitched. Thread colours come from the object's fill (fills, satins) or stroke
(running stitch), mapped to the profile's palette.

### Live-effect previews within v1 budgets

A live effect must finish in 5 s and about 50 M instructions, and the `wasmi` interpreter runs
roughly 10–100× slower than native code. Generating a large fill is cheap; **serializing tens of
thousands of stitches as JSON geometry is not.** The plug-in therefore estimates the output size before
generating and picks a preview level:

| Level | Geometry returned | Used when |
|---|---|---|
| Exact | The stitch polyline (strokes, satins) or one thin closed ribbon per stitch (fills) | Estimated output under the budget |
| Rows | One ribbon per fill row segment (direction and density visible) | Large fills |
| Outline | The original geometry plus a direction hatch | Estimate still too large |

A `preview` choice parameter lets the user force a level. Exact budgets are measured in spike M0.6 and
fixed in M6.3 with a conformance case per level. If a run still fails, VectorCraft draws the object
unchanged and reports the error through `plugin.info`'s `lastError`; our message says which level to
choose. The real stitches are always available from the CLI preview, and ABI v2's overlay channel
removes this limitation.

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

The native format is documented JSON. The adapter reads only what it needs — layers, groups, paths,
compound paths, visibility, paints, appearance effects with our plug-in ids — and treats the file as
untrusted (size caps, depth caps, no panics). The format version is checked; an unknown newer version is
read best-effort with `SC-W0803`. Which VectorCraft versions we read correctly is proven by the
compatibility gate, which saves files with each VectorCraft version and reads them back.

Also verified in M6: how transforms are represented on nodes (applied geometry vs. a separate transform),
so the adapter places stitches exactly where the art is drawn (a scenario in the gate compares the
VectorCraft render with our preview).

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
