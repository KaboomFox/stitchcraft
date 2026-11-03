# ADR-0016: Ink/Stitch files come into VectorCraft through `stitch import-inkstitch`

<!-- The status line, a label and its value, which the ADR index is checked against. -->
<!-- vale ai-tells.ColonUsage = NO -->
**Status:** Accepted · 2026-11-02, after spike M8.11
<!-- vale ai-tells.ColonUsage = YES -->

## Context

An Ink/Stitch file should preview and sew in VectorCraft as its settings say (M8.12).
`stitch import-inkstitch` was planned while VectorCraft dropped `inkstitch:*` attributes on SVG import.
Since 0.17.0 VectorCraft keeps attributes in other applications' namespaces from SVG import to SVG
export ([P7](../rfc-vectorcraft-abi-v2.md#p7-preserve-foreign-svg-attributes)). Spike M8.11 compared 2
ways:

1. **StitchCraft writes the document.** `stitch import-inkstitch file.svg -o file.vectorcraft` reads the
   file with the SVG adapter and writes a document whose objects carry StitchCraft's effects.
2. **VectorCraft's SVG import keeps the attributes.** The VectorCraft adapter reads them from the saved
   document, and a filter makes them into StitchCraft's effects.

What was checked, on 2026-11-02, in VectorCraft's source at `v0.17.0`, `v0.18.0` and `main` at
`c733ae97f`, and in usvg 0.48.1, the SVG parser VectorCraft's import uses:

- **Where the attributes are.** In each object's and layer's attributes, `ObjectAttributes::foreign`.
  The native file writes them on the node as `"attrs": {"foreign": [{"ns", "prefix", "name",
  "value"}]}` (#3758, in 0.17.0). Import keeps them on paths, basic shapes, groups, links, `<use>`
  elements and images. It keeps none on the root `<svg>`, and nothing reads `<metadata>`. Inkscape's
  and Sodipodi's attributes are never kept as another application's.
- **Who is given them.** An exporter, which is given the native file's JSON, and `stitch export`, which
  reads the file. A filter is given each path's id, name, geometry, paints, effect records and opacity,
  and a live effect its geometry and paints, so neither is given them. No proposal of StitchCraft's
  RFC, adopted or open, gives them. The document data of G17 (P5) holds what a filter writes, 1 MiB
  for the whole document, and nothing of the SVG file.
- **What the import loses besides.** A connector's `inkscape:connection-start` and
  `inkscape:connection-end`, which tie a command to its object, are Inkscape's. The import drops them,
  and the connector stays as a stroked path. A command's `<use>` becomes an instance of a VectorCraft
  symbol. The design settings and the file's version are elements of `<metadata>`, which is dropped. A
  clone is expanded into a group with a copy of its source. usvg drops every id inside a `<use>`, so the
  copy has none of its source's settings. A helper path's marker becomes geometry beside the path.
- **What edits do to them.** Moving, turning and scaling keep them as they are, and so do copies.
  Pathfinder's results have none. They take their sources' appearance and its effect records, but not
  their attributes. Make Compound Path leaves them on the member paths, and Ungroup drops a group's.
  Nothing in VectorCraft shows or edits them, and a filter can't remove them.
- **What StitchCraft has.** The SVG adapter reads every setting and updates older files. With M8.4 to
  M8.10 it applies commands, clip paths, design settings and clones. The VectorCraft adapter cuts a clip
  group's art from M8.7. It already writes objects with effect records for filters, and Test Sheet draws
  designs as objects whose records read back as their settings (`REQ-VC-015`). The Design Settings filter
  keeps a document's settings in its `pluginData` (`REQ-VC-013`). StitchCraft depends on VectorCraft only
  through the plug-in ABI and the documented `.vectorcraft` format (`AGENTS.md`), and nothing before M9
  waits on ArtCraft ([roadmap](../../plan/roadmap.md)).

## Decision

1. Ink/Stitch files come into VectorCraft through
   `stitch import-inkstitch file.svg -o file.vectorcraft`, as M8.12 plans it. The SVG adapter reads
   the file as `stitch plan` reads it, an older version updated. A document writer writes each element
   as an object with its family's StitchCraft effect, Ink/Stitch's commands as the names a VectorCraft document uses, clip paths as
   clip groups and clones as copies. It keeps the design settings as the Design Settings filter keeps
   them.
2. The document it writes doesn't keep `inkstitch:*` attributes. Its effect records are its only
   settings.
3. The VectorCraft adapter doesn't read a document's foreign attributes as settings. From M8.12 it names
   an object that has an attribute of Ink/Stitch's namespace with a coded warning, whose hint is to import
   the SVG file with `stitch import-inkstitch`. The object sews as its effects and paints say.
4. StitchCraft proposes no VectorCraft change for this.

## Consequences

- An imported file previews at once, as each object has its family's live effect. `stitch export` and
  File › Export write what `stitch plan` writes of the SVG file, which M8.12's Level B case checks on
  every track.
- The settings are effect records. The Appearance panel shows them, the plug-in's dialog edits them, and
  Pathfinder, Graphic Styles and copies keep them. The preview follows each edit.
- The document is a snapshot of what StitchCraft reads. A setting the registry doesn't declare yet,
  such as one of an M10 stitch type, is named with `SC-W0105` and not written. Importing the file again
  after M10 brings it in. Images and text, which the SVG adapter doesn't sew, are not written. The SVG
  file stays the whole drawing.
- The document has no way back to an Ink/Stitch file: VectorCraft's SVG export doesn't write Ink/Stitch
  settings for effect records. A writer of Ink/Stitch files from a document, as the test kit writes them
  for the differential, would be a roadmap step of its own.
- A file opened with VectorCraft's own SVG import keeps its settings on its objects for SVG export, as
  P7 intends. `stitch export` names those objects instead of sewing them by settings it could read
  only in part.
- M8.12 comes after M8.7 and M8.10, so that the clip groups it writes sew cut and its clones are what
  the SVG adapter reads of them. On 2026-11-02 it moved after 1.0, as a nice to have: `stitch plan`
  already sews an Ink/Stitch file, and the document matters only for a design edited further in
  VectorCraft. It gives Ink/Stitch's target point a name in VectorCraft documents,
  which have none for it yet, or names the command it leaves out. The writer serves
  `stitch import design.pes` after 1.0 too.
- Revisit this if VectorCraft gives plug-ins the foreign attributes and its SVG import keeps the
  connectors' ties, the metadata and the clones' links. VectorCraft could then open an Ink/Stitch file
  with the same result. Revisit it too if VectorCraft adds importer plug-ins, as an importer would run
  this writer.

## Alternatives considered

- **VectorCraft's SVG import keeps the attributes, and a filter makes them into effects.** It previews
  nothing with ABI v1, as no filter or live effect is given the attributes, and giving them is an
  upstream change M8 would wait on. Even then the document has lost the commands' ties, the design
  settings, the file's version and the settings of the clones' sources. A file that uses any of them
  sews otherwise than `stitch plan` sews it, and a connector may sew as a running stitch. Once a filter
  has made the effects, each setting is in the document twice, as an attribute nobody sees and as an
  effect record. The first edit leaves the attribute stale, and SVG export writes the stale value.
  Rejected.
- **The VectorCraft adapter reads the attributes, with no filter.** `stitch export` would sew each
  object's settings with no VectorCraft change. Nothing would preview, though, and the losses above stay.
  It would be a third place for settings, beside effect records and the SVG file. Rejected. Its one gain,
  keeping settings StitchCraft can't read yet, the SVG file gives too.
- **Write the effects and keep the attributes as well.** VectorCraft's SVG export would write the
  settings back for Ink/Stitch, but the 2 copies part at the first edit. Rejected.

## References

- VectorCraft #3758, "SVG: keep other applications' attributes from import to export" (`8906abf30`),
  and #3720, "Plug-ins: filters keep data with the document (G17)" (`150538450`).
- VectorCraft at `v0.18.0`: `docs/plugins.md` (Input, Output, Not in v1),
  `crates/doc/src/node/attributes.rs` and `node/wire.rs`, `crates/svg/src/import.rs`,
  `import/prepass.rs`, `import/importer.rs` and `export/output.rs`, `crates/plugins/src/objects.rs`
  and `effect.rs`, `crates/engine/src/cmd/fileio/encode.rs`, `cmd/pathops.rs` and
  `cmd/object/compound.rs`.
- usvg 0.48.1: `src/parser/svgtree/parse.rs`, `src/parser/use_node.rs` and `src/parser/converter.rs`.
- [RFC P7](../rfc-vectorcraft-abi-v2.md#p7-preserve-foreign-svg-attributes),
  [VectorCraft integration](../vectorcraft-integration.md#importing-inkstitch-svgs-into-vectorcraft)
  and [SVG input](../svg-input.md#inkstitch-objects).
