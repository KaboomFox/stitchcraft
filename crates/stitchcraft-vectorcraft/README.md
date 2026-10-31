# stitchcraft-vectorcraft

Layer **L3**. Adapts VectorCraft data into the engine's `Design`. `stitchcraft_vectorcraft::read(bytes,
budget)` reads a `.vectorcraft` document and returns the design and warnings about what was left out or
simplified, as `stitchcraft-svg` does for SVG. For the plug-in, it reads the object VectorCraft's plug-in
ABI v1 passes a live effect as the element the effect's family sews, with its preview level
(`read_effect`), and writes needle points back as the geometry VectorCraft draws (`effect_output`), or why
there are none (`effect_error`). It reads what VectorCraft passes a filter, and writes the objects a filter
gives back (`filter`).

**Status:** M6.1 reads formats 1 to 4, plain or compressed, with or without the data format 4 keeps after
its JSON (`src/container.rs`). It reads layers, groups, paths and compound paths in paint order, and
visibility. Fills and strokes give threads in VectorCraft's colours, CMYK, grey and Lab included
(`src/color.rs`). Strokes keep their width and join. StitchCraft's live effects say how an object sews
(`REQ-VC-004`, `src/document.rs`). The JSON is read into the few fields the adapter uses (`src/wire.rs`).
M6.2 adds what a live effect reads and writes (`src/effect.rs`). It reads paths (`src/geometry.rs`) and
parameter values (`src/values.rs`) with a document's code, and an object sews alike from a file and in
its preview. The plug-in's `preview_level` is kept apart from the element's settings, in a file as in a
live effect (`PREVIEW_LEVEL`). From M6.3 the geometry is written by hand, to a thousandth of a point
(`src/writer.rs`): stitches as lines or as a fill's ribbons, and any segments as ribbons. From M6.4 a
group's or a layer's StitchCraft effects sew each path inside it, as VectorCraft draws them, in the thread
of the paint it draws over the others, which is how a graphic style applied to a group sews (`REQ-VC-006`).
From M6.5 Ink/Stitch's commands can be written in an object's or a layer's name. `stitch:ignore` leaves
it out, and `stitch:start` and `stitch:end` mark where the shape grouped with them starts or ends
(`REQ-VC-009`). From M13.1 strokes keep their cap too. With `--auto`, `read_with` marks an object as an
automatic element when neither it nor a group or layer around it has a StitchCraft effect
(`REQ-AUTO-001`). Of such a line sewn across its width, the reader names the alignment and the width
profile VectorCraft draws, which its stitches don't follow (`REQ-AUTO-005`). From M13.7 StitchCraft's Auto
effect (`AUTO`) makes an object automatic without the switch, when it is on the object or on a group or a
layer around it, its parameters the object's settings of the rules. `read_auto_effect` reads the object
its live effect is given as those automatic elements, an area for its fill and a line along its topmost
stroke (`REQ-VC-016`). `effect_drawn` writes stitches as
ribbons for a fill or lines for a stroke, whichever paints the object.
From M6.11 a document's design settings and machine, which StitchCraft's Design Settings filter keeps with
it from VectorCraft 0.16, plan the design: `src/settings.rs` writes that data (`kept_data`) and reads it
back, the settings into the design and the machine into `VectorCraft::machine`, which `machine` reads
alone (`REQ-VC-013`).
From M6.12 `src/filter.rs` reads the selected objects VectorCraft gives a filter, each path as a document's
and the stroke a satin column would follow, and writes the objects StitchCraft's tools give back: paths in
points, paints, strokes and effect records (`REQ-VC-014`, `REQ-VC-015`). What it hands back unchanged, a
paint, a record or an opacity, it writes as the text VectorCraft gave, so VectorCraft keeps it as it was.
`EXTENSIONS` lists the documents' file name extensions, a template's among them, by which `stitch plan`
tells a document from an SVG file. `read_part` is the part of a file the reader reads, which a bug-report
bundle keeps: a plain file's JSON, without the images and other data after it. `plugin_id`, `effect_id`
and `effect_name` are the plug-ins' ids and their live effects', one definition for the plug-in, this
reader and the compatibility gate. `param_text` writes a parameter's value as an element holds it, which
the gate compares settings with.
Design: `docs/src/design/vectorcraft-integration.md#reading-vectorcraft-files-stitchcraft-vectorcraft`.
The VectorCraft versions it reads are proven by `docs/src/design/compatibility-gate.md`.

## Invariants

- Depends on VectorCraft only through its documented file format and plug-in JSON, never its crates.
- Contains no stitch logic, only translation.
- Never fails on content it can skip. Kinds of objects, paints and colours it doesn't read are named
  and left out (`SC-W0807`), and so is geometry it can't use (`SC-W0808`). Only a file that is not a
  VectorCraft document, or is too large, is refused (`SC-E0806`). The reader is fuzzed nightly
  (`fuzz/fuzz_targets/read_vectorcraft.rs`).
- What it leaves out is said, with `SC-W0807` and `SC-W0808`, with `SC-I0809` for the layers and guides
  VectorCraft doesn't print, with `SC-I0811` for what a name leaves out, or with `SC-W0812` for a design
  setting or a machine kept with the document that it can't read. Only what VectorCraft draws
  nothing for is left out without a word, and a name on it is not read.
- Positions are points times 25.4/72, and the same on every platform. So are colours, whose Lab
  conversion uses `stitchcraft_core::math`, never the platform's maths.
- Work is bounded: at most 64 MiB of JSON, unpacked, nesting as deep as the JSON reader allows, and the
  budget charged for every object, appearance item, effect, anchor and element.

## Dependencies

`stitchcraft-core`, `stitchcraft-params`, `stitchcraft-plan`, `stitchcraft-engine`; `serde` and
`serde_json` (the document's JSON) and `flate2` with its Rust backend (compressed files), which builds
for `wasm32-unknown-unknown`. `cargo xtask layers` keeps `flate2` here and in the test kit.
