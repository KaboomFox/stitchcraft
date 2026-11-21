# RFC: VectorCraft plug-in features

| | |
|---|---|
| **Audience** | ArtCraft / VectorCraft maintainers |
| **Status** | Mostly adopted ([status](#status)): 7 proposals in VectorCraft 0.14.0 to 0.17.0. P3 is in review, and the rest of P4 and P6 is open. P9 to P11 are proposed here and not yet raised upstream |
| **Compatibility** | Additive. Every plug-in written for ABI v1 keeps working, and `vc_abi_version` still returns 1 |
| **Author's interest** | StitchCraft (machine embroidery), but every proposal serves other plug-ins too |

This page began as the RFC for a plug-in ABI v2. VectorCraft took most of its proposals into ABI v1 instead,
from pull requests by StitchCraft's author.
Each came as keys that hosts from before it ignore, or as a kind of plug-in that they refuse to install, and a
plug-in that uses none of them installs anywhere. VectorCraft's
[plug-in documentation](https://github.com/storytold/vectorcraft/blob/main/docs/plugins.md) is the contract.
This page records what was adopted and when, and what StitchCraft still proposes. Its path keeps the old
name, which links point at. The [roadmap](../plan/roadmap.md) plans StitchCraft's use of the adopted
features in M6.8 to M6.12, and proposes the open parts of P4 and P6 in M9. Raising P9 upstream is the
owner's choice ([fonts](fonts.md#decisions-for-the-owner)).

## Status

Checked against VectorCraft's `docs/plugins.md` and its history, in the 0.18.0 release (2026-10-28). The
gate pins 0.24.1 now, and the table isn't checked against it yet, P3's review among the rest:

| Proposal | In VectorCraft | Since | Pull request |
|---|---|---|---|
| [P1](#p1-exporter-plug-ins), exporter plug-ins | Adopted | 0.14.0 | [#3171](https://github.com/storytold/vectorcraft/pull/3171) |
| [P2](#p2-rich-parameter-schemas), rich parameter schemas | Adopted | 0.14.0 | [#3172](https://github.com/storytold/vectorcraft/pull/3172) |
| [P3](#p3-display-only-overlays-for-live-effects), overlays for live effects | In review | | [#3807](https://github.com/storytold/vectorcraft/pull/3807) |
| [P4](#p4-budget-classes-for-filters), budget classes | Heavy filters adopted, heavy live effects open | 0.17.0 | [#3721](https://github.com/storytold/vectorcraft/pull/3721) |
| [P5](#p5-document-level-plug-in-data), document-level plug-in data | Adopted for filters and exporters | 0.16.0 | [#3720](https://github.com/storytold/vectorcraft/pull/3720) |
| [P6](#p6-the-paints-of-a-live-effects-object), context for live effects | Fills and strokes adopted, neighbours open | 0.14.0, a stroke's style 0.15.0 | [#3173](https://github.com/storytold/vectorcraft/pull/3173) |
| [P7](#p7-preserve-foreign-svg-attributes), foreign SVG attributes | Adopted | 0.17.0 | [#3758](https://github.com/storytold/vectorcraft/pull/3758) |
| [P8](#p8-effects-and-stroke-style-in-filter-objects), effects and stroke style in filters | Adopted | 0.15.0 | [#3174](https://github.com/storytold/vectorcraft/pull/3174) |
| [P9](#p9-type-as-glyph-outlines), type as glyph outlines | Proposed by StitchCraft, not yet raised upstream | | |
| [P10](#p10-an-export-budget-that-fits-a-traced-photo), an export budget that fits a traced photo | Proposed by StitchCraft, not yet raised upstream | | |
| [P11](#p11-the-effects-of-a-paths-groups-in-a-filter), the effects of a path's groups in a filter | Proposed by StitchCraft, not yet raised upstream | | |
| P12, placed images given to a filter ([ADR-0019](adr/0019-subject-cut-out-before-tracing.md)) | Not raised: a plug-in still couldn't run the cut-out's models | | |

The plug-in classes this RFC was for, and what VectorCraft serves of them now:

| Plug-in class | Now |
|---|---|
| Output generators for embroidery (PES/DST), plotters (HPGL), laser cutters (G-code) and cutting machines | Exporters write their own formats (P1) |
| Generators with many settings (halftones, hatching, embroidery, pattern fills) | Up to 256 parameters, labelled, in groups, with help and conditions (P2) |
| Previews of things that are not the object's geometry (stitches, toolpaths, cut order) | Still only by replacing geometry, at JSON's cost for every point (P3, in review) |
| Heavy but legitimate computation (routing, packing, simulation) | Heavy filters (P4), and an export's own budget, which a traced photo needs 5 to 6 times over (P10). A live effect still has a light run's budget |
| Plug-ins with document-wide settings (machine profile, plotter pens) | Data kept with the document (P5) |
| Tools that set up objects for a live effect (a satin column from a line, a test sheet) | Filters read and return effect records and a stroke's style (P8) |
| Plug-ins that work on text (lettering, engraving, cutting type) | Filters and live effects receive paths alone, and an exporter only a type object's text and its font's name (P9) |

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
of a large fill, where it shows a fill's rows or its outline today. [#3807](https://github.com/storytold/vectorcraft/pull/3807) adds overlays, and is in
review.

### P4: Budget classes for live effects

A live effect declares `"budget": "heavy"`, allowed once at install as a heavy filter is. It gets more
instructions and time and runs off the UI thread, with the object drawn as it is until the result comes.
Fuel for each byte read back, as well as for each byte given, would make the budget follow what a
generator writes. An embroidery effect's exact preview of a fill 7 mm across takes 27 million
instructions, and a larger fill's stops at the light budget's 50 million. StitchCraft's M6.19 waits for
this or for P3: from M6.18 a fill too large for its rows shows its shape whole, without the hatch that
a person read as thread, and the rows come back when a heavy live effect or an overlay can afford them.

### P6: The objects beside a live effect's object

The live-effect input holds, optionally, the objects in the same group whose names start with a prefix the
manifest declares (`"contextPrefixes": ["stitch:"]`). Embroidery reads start and end markers from it, so a
preview starts and ends where `stitch export` does. Other plug-ins use the context for guides and anchors.

### P9: Type as glyph outlines

Proposed by StitchCraft, and not yet raised upstream. A filter is also given each selected type object, and a
live effect the type object it is applied to, read only. Each comes as its glyph outlines in the path form, subpaths of
anchors, with its text, its font and its fills and strokes. The outlines are the ones VectorCraft shows on the
canvas and makes with Type › Create Outlines. An exporter's document has each type object's outlines beside its
text record. A plug-in's sandbox can't read font files, and a plug-in can't shape or outline type itself.
Today a user runs Create Outlines first, and the text is then no longer editable. StitchCraft would sew
lettering typed in VectorCraft from the outlines, in M14 and M15 ([fonts](fonts.md)).

Nothing in VectorCraft's plans covers it, at `3f8d7a0` (2026-10-25). Its `docs/plugins.md` gives a filter
the selected paths and compound paths alone, and a live effect one object's geometry. An exporter is given the
document's JSON, where a type object is its text and a font's name. The plug-ins still missing in G17 of its
`docs/gaps.md` are tool and panel plug-ins, importers and a plug-in directory. Its roadmaps leave type out.

### P10: An export budget that fits a traced photo

An export may run 10 billion instructions and 4,000 more for each byte of the document. A photo traced
into 6 or 9 colours 100 mm tall, each shape digitized by width, needs 51 to 69 billion in an embroidery
exporter, for documents of 236 to 265 KB that are allowed 11 billion
([the measurements](vectorcraft-integration.md#an-export-that-finishes)). VectorCraft stops the export
after 6 seconds on the desktop and after 27 in the browser, and the user reads `plug-in exceeded its
instruction budget` with no file. The same module, run by a browser without a count, writes the file in 2
to 4 seconds. The proposal has 4 parts:

- An exporter declares `"budget": "heavy"`, allowed once at install as a heavy filter is. A heavy export
  may run 100 billion instructions and the same amount for each byte, which is 1.5 times the largest
  design measured. The 300 seconds stay as they are and bound the time.
- A heavy export runs off the UI thread, with its progress shown and a way to cancel it. VectorCraft's
  interpreter ran 51 billion instructions in 20 seconds natively. In the browser, where the interpreter
  is itself WebAssembly, it ran 11 billion in 27 seconds, so 51 billion would take about 2 minutes there.
- An export's `_context` names the instructions it may run, as `fuel`. A plug-in that knows its own cost
  can then decline with a message of its own before the work, where it is stopped part way today.
- The error for a stopped export says which limit stopped it and names the plug-in, so that the plug-in's
  documentation can say what to do next.

A browser host could go further and run an exporter's module with the browser's own compiler in a
worker, since an exporter has no imports and changes nothing in the document. The time limit alone would
then bound it. StitchCraft's browser demo does this beside the editor for its own exporters.

### P11: The effects of a path's groups in a filter

Proposed by StitchCraft for Expand (M13.26), and not yet raised upstream. Each object a filter is given
also lists `groupEffects`: the effect records of the groups and layers it lies in, the outermost first,
each as `effects` lists a record, read only. A filter that replaces an object then knows the settings the
object took from its groups, as an exporter reads them in the whole document. StitchCraft's Auto effect is
usually on the traced group, and today a filter can't read its settings. Expand also digitizes the whole
design, which takes an export's work, so it needs P10's budget for a heavy filter too
([Expand](image-to-embroidery.md#expand)). Until both are in VectorCraft, a person expands a design with
`stitch expand` and opens the document it writes.

## Security and determinism

The proposals leave the sandbox as it is: plug-ins still cannot touch files, the network, clocks or
randomness. Exporters return bytes, which the host writes. Overlays are plain data, and heavy budgets are
opt-in and still bounded. The outlines of type (P9) are data the host gives, and the budget charges for
their points as for any input. Results are still deterministic functions of their inputs, which a host may
cache.

## Implementation sketch and tests

| Proposal | Main files | Tests |
|---|---|---|
| P3 | `crates/plugins/src/effect.rs`, canvas drawing in `crates/ui-egui/src/canvas.rs` | a fuzz test of overlay decoding, a cache key test |
| P4, live effects | `crates/plugins/src/{effect,runtime}.rs`, the install flow | fuel accounting tests, a drawing that waits for a heavy result |
| P6, objects beside | `crates/plugins/src/objects.rs` | ABI tests |
| P9 | `crates/plugins/src/objects.rs`, with the text outliner of `crates/effects/src/live.rs` | ABI tests of type sent to filters, live effects and exporters |

We would contribute implementations and tests following VectorCraft's `AGENTS.md`, one proposal per PR.
StitchCraft's compatibility gate already runs a third-party contract test against every VectorCraft
release, release branch and `main` ([compatibility gate](compatibility-gate.md)).
