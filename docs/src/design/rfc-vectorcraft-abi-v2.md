# RFC: VectorCraft plug-in ABI v2

| | |
|---|---|
| **Audience** | ArtCraft / VectorCraft maintainers |
| **Status** | Draft for discussion (to be opened as a GitHub discussion or issue before M9) |
| **Compatibility** | Additive: every v1 plug-in keeps working unchanged |
| **Author's interest** | StitchCraft (machine embroidery), but every proposal serves other plug-ins too |

## Summary

ABI v1 is a careful, safe design: sandboxed, import-free, deterministic, portable to the web. This RFC
proposes seven additive features that let richer plug-ins exist without weakening that sandbox. Each is
independent and can land on its own; together they let plug-ins export files, show informative
previews and present usable parameter dialogs.

## Motivation

Classes of plug-ins that v1 cannot serve well today, all of them common in illustration tools:

| Plug-in class | Blocked by |
|---|---|
| Output generators: embroidery (PES/DST), plotters (HPGL), laser cutters (G-code), cutting machines | no file-format plug-ins |
| Generators with many settings (halftones, hatching, embroidery, pattern fills) | ≤ 64 parameters, no labels, units, groups or conditions |
| Previews of things that are not the object's geometry (stitches, toolpaths, cut order) | live effects can only *replace* geometry, and must pay JSON serialization for every point |
| Heavy but legitimate computation (routing, packing, simulation) | fixed fuel/time budgets independent of output size |
| Plug-ins with document-wide settings (machine profile, plotter pens) | no document-level storage |

## Proposals

### P1 — Exporter plug-ins

A new manifest kind `"exporter"` with `"extensions": ["pes"]` and `"label": "Brother PES"`. On
**File › Export**, the host calls `vc_export(input, len, params, len) → (len << 32) | ptr` with the whole
document's exportable objects in paint order (the same object JSON as filters, plus groups, layers,
visibility and appearance effect records) and receives `{"bytes": base64, "warnings": [...]}` or
`{"error": "..."}`. The host writes the file; the plug-in still has no file-system access.

*Host change:* make `FORMATS` (`crates/engine/src/cmd/fileio/mod.rs`) extensible with plug-in entries
(write-only), list them in the export dialog and `export_extensions()`.

### P2 — Rich parameter schemas

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

### P3 — Display-only overlays for live effects

A live effect may return `"overlay": {"format": "f32-polylines", "data": base64}` in addition to (or
instead of) geometry: a compact binary list of polylines, each with an RGBA colour and a width in
points. The host draws overlays on the canvas above the object, never exports or hit-tests them, and
caches them with the effect result. This makes previews cheap (4 bytes per coordinate instead of JSON)
and keeps the object's real geometry untouched.

### P4 — Budget classes

Manifest `"budget": "light" | "heavy"`. Light keeps today's limits. Heavy plug-ins get a larger base
fuel (for example 1 G instructions) and wall-clock limit, run off the UI thread, and require the user to
allow them once at install. Fuel per output byte read back (not only per input byte) makes the budget
track the real cost of generators.

### P5 — Document-level plug-in data

`Document.plugin_data: Map<plugin id, JSON>` (≤ 1 MiB per plug-in), saved in `.vectorcraft`, passed to
the plug-in as `_context.documentData`, writable through a filter's output `"documentData"`. Today
`Document.unknown` holds similar foreign data for the perspective grid and brushes, so the storage pattern
already exists.

### P6 — Read-only context for live effects

Add the object's `fills`/`strokes` and, optionally, siblings in the same group whose names start with a
declared prefix (`"contextPrefixes": ["stitch:"]`) to the live-effect input. Embroidery uses this for
thread colours and start/end markers; other plug-ins for guides and anchors.

### P7 — Preserve foreign SVG attributes

On SVG import, keep attributes in unknown namespaces per node (`Node.foreign: Option<Box<Map>>`), and
write them back on SVG export. This makes VectorCraft a good citizen for files from Inkscape extensions
and other tools (Ink/Stitch, plotter workflows), independent of any plug-in.

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

We would contribute implementations and tests following VectorCraft's `AGENTS.md`, one proposal per PR,
and a small third-party contract test so future ABI changes are caught by `cargo xtask ci`.
