# RFC: VectorCraft plug-in ABI v2

| | |
|---|---|
| **Audience** | ArtCraft / VectorCraft maintainers |
| **Status** | Proposed upstream in October 2026, one pull request per proposal. Seven are merged, the overlays are under review, and P6's sibling markers are not built ([outcome](#outcome)) |
| **Compatibility** | Additive: every v1 plug-in keeps working unchanged |
| **Author's interest** | StitchCraft (machine embroidery), but every proposal serves other plug-ins too |

## Summary

ABI v1 is a careful, safe design: sandboxed, import-free, deterministic, portable to the web. This RFC
proposes eight additive features that let richer plug-ins exist without weakening that sandbox. Each is
independent and can land on its own; together they let plug-ins export files, show informative
previews and present usable parameter dialogs.

## Outcome

VectorCraft's maintainers took the proposals one pull request at a time, as additive parts of ABI v1.
A plug-in still returns 1 from `vc_abi_version`, and a host from before a key ignores it. A host from
before exporters refuses that kind at install. VectorCraft's `docs/plugins.md` is the contract now, and
the proposals below are as they were written.

| Proposal | Pull request | In VectorCraft | What landed, where it differs from the proposal |
|---|---|---|---|
| P1 exporters | [#3171](https://github.com/storytold/vectorcraft/pull/3171) | 0.14.0 | The kind `"exporter"` with `extensions`, run by `vc_run`, without a `vc_export`. It is given the whole document as the native file's JSON, and `_context` is `{"mode": "export", "extension", "name"}`. It returns `{"dataBase64", "warnings"?}`. An export gets 10 G instructions plus 4,000 per input byte, and 300 s, and Background Export's Cancel stops it |
| P2 parameter schemas | [#3172](https://github.com/storytold/vectorcraft/pull/3172) | 0.14.0 | `label`, `unit`, `group`, `help`, `helpUrl` and `visibleWhen`, and the types `string` and `numberList`. A plug-in declares up to 256 parameters, at most 64 outside groups, without a `maxParams` key |
| P3 overlays | [#3807](https://github.com/storytold/vectorcraft/pull/3807) | under review | As proposed |
| P4 budget classes | [#3721](https://github.com/storytold/vectorcraft/pull/3721) | `main`, after 0.16.0 | A filter may ask for the heavy budget with `"budget": "heavy"`, and a live effect or an exporter may not. A heavy run gets 1 G instructions plus 4,000 per input byte, and 300 s. Installing asks the user (`allowHeavy`), and the app runs the filter in the background with Cancel. The output adds no fuel, as the host learns how large it is when the run has ended |
| P5 document data | [#3720](https://github.com/storytold/vectorcraft/pull/3720) | 0.16.0 | The data is part of the document's foreign data, `unknown.pluginData` by plug-in id, without a `plugin_data` section. A filter reads its own in `_context.documentData` and writes `documentData` (at most 1 MiB, `null` removes it) in the same undo step as its objects. A live effect isn't given it. An exporter reads every plug-in's in the document it is given |
| P6 context for live effects | [#3173](https://github.com/storytold/vectorcraft/pull/3173) | 0.14.0 | The object's `fills` and `strokes`, read only, and `_context.item`, the fill or stroke the effect sits on. A change of paint runs the effect again. The sibling markers (`contextPrefixes`) are not built |
| P7 foreign SVG attributes | [#3758](https://github.com/storytold/vectorcraft/pull/3758) | `main`, after 0.16.0 | `ObjectAttributes::foreign`, beside the object's `data-*` attributes, and `attrs.foreign` in the native file. They are kept on the elements whose `data-*` attributes are kept, which leaves type out, at most 64 an object and 64 KiB a value, with SVG's, XLink's, XML's, Inkscape's, Sodipodi's and VectorCraft's own namespaces aside |
| P8 effects and stroke style | [#3174](https://github.com/storytold/vectorcraft/pull/3174) | 0.15.0 | As proposed. A path a filter hands back unchanged is still a live shape |

The third-party contract test proposed at the end of this page has not been written.

## Motivation

Classes of plug-ins that v1 cannot serve well today, all of them common in illustration tools:

| Plug-in class | Blocked by |
|---|---|
| Output generators: embroidery (PES/DST), plotters (HPGL), laser cutters (G-code), cutting machines | no file-format plug-ins |
| Generators with many settings (halftones, hatching, embroidery, pattern fills) | ≤ 64 parameters, no labels, units, groups or conditions |
| Previews of things that are not the object's geometry (stitches, toolpaths, cut order) | live effects can only *replace* geometry, and must pay JSON serialization for every point |
| Heavy but legitimate computation (routing, packing, simulation) | fixed fuel/time budgets independent of output size |
| Plug-ins with document-wide settings (machine profile, plotter pens) | no document-level storage |
| Tools that set up objects for a live effect (a satin column from a line, a test sheet) | a filter returns geometry and paint without effect records, and is not given a stroke's join, cap or miter limit |

## Proposals

### P1: Exporter plug-ins

A new manifest kind `"exporter"` with `"extensions": ["pes"]` and `"label": "Brother PES"`. On
**File › Export**, the host calls `vc_export(input, len, params, len) → (len << 32) | ptr` with the whole
document's exportable objects in paint order (the same object JSON as filters, plus groups, layers,
visibility and appearance effect records) and receives `{"bytes": base64, "warnings": [...]}` or
`{"error": "..."}`. The host writes the file; the plug-in still has no file-system access.

*Host change:* make `FORMATS` (`crates/engine/src/cmd/fileio/mod.rs`) extensible with plug-in entries
(write-only), list them in the export dialog and `export_extensions()`.

### P2: Rich parameter schemas

Optional keys on each parameter, ignored by v1 hosts:

```json
"row_spacing_mm": {
  "type": "number", "min": 0.1, "max": 10, "default": 0.25,
  "label": "Row spacing", "unit": "mm", "group": "Fill",
  "help": "Distance between rows of stitches. Smaller is denser.",
  "helpUrl": "https://…/params/row_spacing_mm",
  "visibleWhen": {"fill_method": ["tatami_fill", "contour_fill"]}
}
```

plus types `"string"` (with `maxLength`) and `"numberList"` (with `minItems`/`maxItems`), and a manifest
`"maxParams"` up to 256 when `"groups"` are used. *Host change:* the generated dialog renders labels,
units, groups as collapsible sections and evaluates `visibleWhen`.

### P3: Display-only overlays for live effects

A live effect may return `"overlay": {"format": "f32-polylines", "data": base64}` in addition to (or
instead of) geometry: a compact binary list of polylines, each with an RGBA colour and a width in
points. The host draws overlays on the canvas above the object, never exports or hit-tests them, and
caches them with the effect result. This makes previews cheap (4 bytes per coordinate instead of JSON)
and keeps the object's real geometry untouched.

### P4: Budget classes

Manifest `"budget": "light" | "heavy"`. Light keeps today's limits. Heavy plug-ins get a larger base
fuel (for example 1 G instructions) and wall-clock limit, run off the UI thread, and require the user to
allow them once at install. Fuel per output byte read back (not only per input byte) makes the budget
track the real cost of generators.

In VectorCraft's own host, a filter making a line 3 mm wide into the 2 rails of a satin column takes
from 51 million instructions for an S curve 30 mm long to 162 million for a circle 50 mm across. That is
what Ink/Stitch's Convert Line to Satin does. A run gets about 52 million today.

### P5: Document-level plug-in data

`Document.plugin_data: Map<plugin id, JSON>` (≤ 1 MiB per plug-in), saved in `.vectorcraft`, passed to
the plug-in as `_context.documentData`, writable through a filter's output `"documentData"`. Today
`Document.unknown` holds similar foreign data for the perspective grid and brushes, so the storage pattern
already exists.

### P6: Read-only context for live effects

Add the object's `fills`/`strokes` and, optionally, siblings in the same group whose names start with a
declared prefix (`"contextPrefixes": ["stitch:"]`) to the live-effect input. Embroidery reads thread
colours, start and end markers and a stroke's width from it. A satin column drawn as one path and a zigzag
stroke are as wide as their stroke, and without its width a v1 effect leaves both as they are
(`SC-I0810`). Other plug-ins use the context for guides and anchors.

### P7: Preserve foreign SVG attributes

On SVG import, keep attributes in unknown namespaces per node (`Node.foreign: Option<Box<Map>>`), and
write them back on SVG export. This makes VectorCraft a good citizen for files from Inkscape extensions
and other tools (Ink/Stitch, plotter workflows), independent of any plug-in.

### P8: Effects and stroke style in filter objects

A filter's input objects include each stroke's `join`, `cap` and `miterLimit` beside its `paint` and
`width`, and a filter may return `"effects": [{id, params, visible}]` for an object, which replaces the
object's appearance effects as `fills` and `strokes` replace its paints. The host checks each record as
`effect.apply` does: a known effect id, and parameters its manifest accepts. A tool could then make a line
into a satin column with its effect, or draw a test sheet whose objects sew as one. A column made from a
line needs the stroke's join at its corners.

## Security and determinism

None of the proposals adds imports: plug-ins still cannot touch files, the network, clocks or
randomness. Exporters return bytes to the host; overlays are plain data; heavy budgets are opt-in and
still bounded. Results remain deterministic functions of their inputs, so caching stays valid.

## Implementation sketch and tests

| Proposal | Main files | Tests |
|---|---|---|
| P1 | `crates/plugins/src/{manifest,runtime}.rs`, `crates/engine/src/cmd/fileio/mod.rs`, export dialog | ABI tests with a WAT exporter fixture; export command sweep |
| P2 | `crates/plugins/src/manifest.rs`, plug-in dialog in `crates/ui-egui` | manifest property tests; headless frame test of the dialog |
| P3 | `crates/plugins/src/effect.rs`, canvas drawing in `crates/ui-egui/src/canvas.rs` | overlay decoding fuzz test; cache key test |
| P4 | `crates/plugins/src/runtime.rs`, install flow | fuel accounting tests |
| P5 | `crates/doc/src/lib.rs`, `crates/format` | format round-trip property test |
| P6 | `crates/plugins/src/objects.rs` | ABI tests |
| P7 | `crates/doc/src/node.rs`, `crates/svg/src/{import,export}.rs` | SVG round-trip tests |
| P8 | `crates/plugins/src/objects.rs` | ABI tests; an effect record a manifest refuses is refused |

We would contribute implementations and tests following VectorCraft's `AGENTS.md`, one proposal per PR,
and a small third-party contract test so future ABI changes are caught by `cargo xtask ci`.
