# stitchcraft-vectorcraft

Layer **L3**. Adapts VectorCraft data into the engine's `Design`. `stitchcraft_vectorcraft::read(bytes,
budget)` reads a `.vectorcraft` document and returns the design and warnings about what was left out or
simplified, as `stitchcraft-svg` does for SVG. For the plug-in, it reads the object VectorCraft's plug-in
ABI v1 passes a live effect as the element the effect's family sews, with its preview level
(`read_effect`), and writes needle points back as the geometry VectorCraft draws (`effect_output`), or why
there are none (`effect_error`).

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
Design: `docs/src/design/vectorcraft-integration.md#reading-vectorcraft-files-stitchcraft-vectorcraft`.
The VectorCraft versions it reads are proven by `docs/src/design/compatibility-gate.md`.

## Invariants

- Depends on VectorCraft only through its documented file format and plug-in JSON, never its crates.
- Contains no stitch logic, only translation.
- Never fails on content it can skip. Kinds of objects, paints and colours it doesn't read are named
  and left out (`SC-W0807`), and so is geometry it can't use (`SC-W0808`). Only a file that is not a
  VectorCraft document, or is too large, is refused (`SC-E0806`). The reader is fuzzed nightly
  (`fuzz/fuzz_targets/read_vectorcraft.rs`).
- What it leaves out is said, with `SC-W0807` and `SC-W0808`, or with `SC-I0809` for the layers and
  guides VectorCraft doesn't print. Only what VectorCraft draws nothing for is left out without a word.
- Positions are points times 25.4/72, and the same on every platform. So are colours, whose Lab
  conversion uses `stitchcraft_core::math`, never the platform's maths.
- Work is bounded: at most 64 MiB of JSON, unpacked, nesting as deep as the JSON reader allows, and the
  budget charged for every object, appearance item, effect, anchor and element.

## Dependencies

`stitchcraft-core`, `stitchcraft-params`, `stitchcraft-plan`, `stitchcraft-engine`; `serde` and
`serde_json` (the document's JSON) and `flate2` with its Rust backend (compressed files), which builds
for `wasm32-unknown-unknown`. `cargo xtask layers` keeps `flate2` here and in the test kit.
