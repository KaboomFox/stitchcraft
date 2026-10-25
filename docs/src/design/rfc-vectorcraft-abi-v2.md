# RFC: VectorCraft plug-in features

| | |
|---|---|
| **Audience** | ArtCraft / VectorCraft maintainers |
| **Status** | Mostly adopted ([status](#status)): 5 proposals in VectorCraft 0.14.0 to 0.16.0, 2 more on its `main`, and P3 open with the rest of P4 and P6 |
| **Compatibility** | Additive. Every plug-in written for ABI v1 keeps working, and `vc_abi_version` still returns 1 |
| **Author's interest** | StitchCraft (machine embroidery), but every proposal serves other plug-ins too |

This page began as the RFC for a plug-in ABI v2. VectorCraft took most of its proposals into ABI v1 instead.
Each came as keys that hosts from before it ignore, or as a kind of plug-in that they refuse to install, and a
plug-in that uses none of them installs anywhere. VectorCraft's
[plug-in documentation](https://github.com/storytold/vectorcraft/blob/main/docs/plugins.md) is the contract.
This page records what was adopted and when, and what StitchCraft still proposes. Its path keeps the old
name, which links point at. The [roadmap](../plan/roadmap.md) plans StitchCraft's use of the adopted
features in M6.8 to M6.12, and proposes the open ones in M9.

## Status

Checked against VectorCraft's `docs/plugins.md` and its history, on `main` at `8bf090b` (2026-10-24):

| Proposal | In VectorCraft | Since |
|---|---|---|
| [P1](#p1-exporter-plug-ins), exporter plug-ins | Adopted | 0.14.0 |
| [P2](#p2-rich-parameter-schemas), rich parameter schemas | Adopted | 0.14.0 |
| [P3](#p3-display-only-overlays-for-live-effects), overlays for live effects | Open | |
| [P4](#p4-budget-classes-for-filters), budget classes | Heavy filters adopted, heavy live effects open | `main`, after 0.16.0 |
| [P5](#p5-document-level-plug-in-data), document-level plug-in data | Adopted for filters and exporters | 0.16.0 |
| [P6](#p6-the-paints-of-a-live-effects-object), context for live effects | Fills and strokes adopted, neighbours open | 0.14.0, a stroke's style 0.15.0 |
| [P7](#p7-preserve-foreign-svg-attributes), foreign SVG attributes | Adopted | `main`, after 0.16.0 |
| [P8](#p8-effects-and-stroke-style-in-filter-objects), effects and stroke style in filters | Adopted | 0.15.0 |

The plug-in classes this RFC was for, and what VectorCraft serves of them now:

| Plug-in class | Now |
|---|---|
| Output generators for embroidery (PES/DST), plotters (HPGL), laser cutters (G-code) and cutting machines | Exporters write their own formats (P1) |
| Generators with many settings (halftones, hatching, embroidery, pattern fills) | Up to 256 parameters, labelled, in groups, with help and conditions (P2) |
| Previews of things that are not the object's geometry (stitches, toolpaths, cut order) | Still only by replacing geometry, at JSON's cost for every point (P3, open) |
| Heavy but legitimate computation (routing, packing, simulation) | Heavy filters (P4), and an export's own budget. A live effect still has a light run's budget |
| Plug-ins with document-wide settings (machine profile, plotter pens) | Data kept with the document (P5) |
| Tools that set up objects for a live effect (a satin column from a line, a test sheet) | Filters read and return effect records and a stroke's style (P8) |

## Adopted

How VectorCraft adopted each proposal, where it differs from what was proposed.

### P1: Exporter plug-ins

A plug-in of `"kind": "exporter"` lists 1 to 8 `extensions`. File › Export › Export As lists it after the
built-in formats as `plugin.<id>`, which `document.export` writes too. It is given the whole document as a
native `.vectorcraft` file's JSON, where the RFC proposed the filters' object JSON. The JSON lists every
layer, object, appearance and effect record, with the data plug-ins keep with the document. It returns
`{"dataBase64": …, "warnings": […]}`, and the host writes the file. An export gets 10 G instructions, plus
4,000 for each input byte, and 300 s, and runs on the export's worker with Background Export on. Its
`_context` names the file's extension and name, and a plug-in with parameters asks for them once the file
is picked.

### P2: Rich parameter schemas

As proposed: `label`, `unit`, `group`, `help`, `helpUrl` (an `https://` page) and `visibleWhen`, the types
`string` (with `maxLength`, at most 4,096) and `numberList` (with `minItems` and `maxItems`, at most 256).
A plug-in declares up to 256 parameters, at most 64 of them outside groups, where the RFC proposed a
manifest key to ask for more. `visibleWhen` names checkboxes and choices declared before the parameter. A
hidden parameter still goes to the plug-in. A plug-in's parameters scroll in its dialog when they don't fit
the window.

### P4: Budget classes for filters

A filter that declares `"budget": "heavy"` gets 1 G instructions, plus 4,000 for each input byte, and 300 s
(`HEAVY_FUEL`, `HEAVY_WALL_TIME_MS`). It installs once the user allows it, and runs in the background with
progress and Cancel. A host from before heavy filters ignores the key, and the filter gets a light run's
budget there. Only a filter may be heavy, as a live effect runs whenever the document is drawn. A heavy
filter's budget grows with its input alone: the host learns how much output there is only once the run has
ended.

### P5: Document-level plug-in data

A filter is given what it last kept with the document as `_context.documentData`, and keeps new data by
returning `documentData`, at most 1 MiB of JSON, in the same undo step as its objects. The native file
keeps it in `document.unknown.pluginData` under the plug-in's id. An exporter reads every plug-in's, in the
document it is given. A live effect isn't given it: its geometry depends on its object alone.

### P6: The paints of a live effect's object

A live effect is given its object's `fills` and `strokes`. Each stroke comes with its `width`, and from
0.15.0 with its `join`, `cap` and `miterLimit`, as a filter's does (P8). `_context.item` says where the
effect is: on the object, or in the own effects of one of its fills or strokes. The paints are read only,
and the effect still changes only the geometry. The objects beside it are not given
([below](#p6-the-objects-beside-a-live-effects-object)).

### P7: Preserve foreign SVG attributes

As proposed: attributes in other applications' XML namespaces stay on their objects from SVG import to SVG
export, and copies keep them. Nothing in VectorCraft reads them. An Ink/Stitch SVG opened in VectorCraft
keeps its `inkstitch:*` parameters when it is saved as SVG again.

### P8: Effects and stroke style in filter objects

As proposed: a filter is given each stroke's `join`, `cap` and `miterLimit`, as a live effect is, and each
object's own effect records. It may return `effects`, which replace the object's own. Each is admitted as
`effect.apply` admits it: a known effect, or an installed live-effect plug-in with parameters its manifest
takes. A tool can make a line into a satin column with its effect, or draw a test sheet whose objects sew.

## Open

### P3: Display-only overlays for live effects

A live effect may return `"overlay": {"format": "f32-polylines", "data": base64}`, with its geometry or
instead of it. The overlay is a compact binary list of polylines, each with an RGBA colour and a width in
points. The host draws overlays on the canvas above the object, never exports or hit-tests them, and caches
them with the effect result. This makes previews cheap, 8 bytes for each point instead of the 40 or so JSON
takes, and keeps the object's real geometry untouched. An embroidery preview could then show every stitch
of a large fill, where it shows a fill's rows or its outline today.

### P4: Budget classes for live effects

A live effect declares `"budget": "heavy"`, allowed once at install as a heavy filter is. It gets more
instructions and time and runs off the UI thread, with the object drawn as it is until the result comes.
Fuel for each byte read back, as well as for each byte given, would make the budget follow what a
generator writes. An embroidery effect's exact preview of a fill 7 mm across takes 27 million
instructions, and a larger fill's stops at the light budget's 50 million.

### P6: The objects beside a live effect's object

The live-effect input holds, optionally, the objects in the same group whose names start with a prefix the
manifest declares (`"contextPrefixes": ["stitch:"]`). Embroidery reads start and end markers from it, so a
preview starts and ends where `stitch export` does. Other plug-ins use the context for guides and anchors.

## Security and determinism

The proposals leave the sandbox as it is: plug-ins still cannot touch files, the network, clocks or
randomness. Exporters return bytes, which the host writes. Overlays are plain data, and heavy budgets are
opt-in and still bounded. Results are still deterministic functions of their inputs, which a host may
cache.

## Implementation sketch and tests

| Proposal | Main files | Tests |
|---|---|---|
| P3 | `crates/plugins/src/effect.rs`, canvas drawing in `crates/ui-egui/src/canvas.rs` | a fuzz test of overlay decoding, a cache key test |
| P4, live effects | `crates/plugins/src/{effect,runtime}.rs`, the install flow | fuel accounting tests, a drawing that waits for a heavy result |
| P6, objects beside | `crates/plugins/src/objects.rs` | ABI tests |

We would contribute implementations and tests following VectorCraft's `AGENTS.md`, one proposal per PR.
StitchCraft's compatibility gate already runs a third-party contract test against every VectorCraft
release, release branch and `main` ([compatibility gate](compatibility-gate.md)).
