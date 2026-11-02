# stitchcraft-svg

Layer **L3**. Reads SVG documents into the engine's `Design`: `stitchcraft_svg::read(bytes, budget)`
returns the design and warnings about what was left out or simplified. Ink/Stitch's remaining commands
and clones follow in M8 (M8.8 to M8.10).

**Status:** v1 (M3.3) reads paths and the basic shapes, groups and `<switch>`, transforms, the root's
size and viewBox (so positions come out in millimetres), fill and stroke colours with `currentColor`, and
everything that hides an element. A shape's fill is sewn before its stroke, as Ink/Stitch sews them.
Since M4.8 it reads each stroke's width and join (`REQ-SVG-004`), which give a satin column drawn as one
path its width and corners, and Ink/Stitch's design settings in the file's metadata (`src/settings.rs`,
`REQ-SVG-005`). Since M13.1 it reads each stroke's cap. With `--auto`, `read_with` marks a shape without
`inkstitch:` attributes as an automatic element (`REQ-AUTO-001`). Its corners are joined as SVG draws
them, and a transform that stretches such a line more one way than the other is named (`REQ-AUTO-005`).
Ink/Stitch's own objects are not stitched (`src/inkstitch.rs`, `REQ-SVG-003`):
- command symbols and the connectors that tie them to objects;
- lines drawn with Inkscape's connector tool;
- guide, anchor-line and pattern helper paths.

Its trim and stop commands set `trim_after` and `stop_after`, under today's names or those from before
its file version 3. Its ignore commands and `ignore_object` setting leave objects and layers out and list
them (`SC-I0805`, `REQ-ASM-004`). Since M8.4 its start, end and target commands give the elements of the
shape they are tied to their hints, at the symbol's place (`REQ-GEN-001`, `REQ-FILL-CIR-008`); the first
connector in the file counts, and a command whose place doesn't read is named and left out
(`SC-W0802`).

Since M8.5 clip paths cut the shapes they clip (`REQ-SVG-011`, `src/document/clip.rs`). The clip path of a
shape, and that of each group round it, is a clip of the shape's elements, which the engine cuts them to
(`REQ-GEN-005`). A clip path is its children's areas together, each by its `clip-rule`, in the user space
of what it clips or in shares of a shape's box. A group's children count, as Inkscape draws them, and a
clip path's own `clip-path` cuts it too. `SC-W0802` names what isn't applied.

Since M8.1 each `inkstitch:*` setting of a shape is a parameter of the elements it makes, by its own name
(`REQ-SVG-009`): an Ink/Stitch file sews with its stitch types, spacings, underlays and locks. The
registry names a setting it doesn't declare (`SC-W0105`), and `SC-W0802` names a group's settings, which
change nothing in Ink/Stitch either. `stroke_first` sews a shape's stroke before its fill (`REQ-SVG-010`).

A file an older Ink/Stitch saved is read as Ink/Stitch updates it (`src/version.rs`, `REQ-SVG-007`).
Before file version 4, for one, a fill's stitches were 3 mm long, and a shape of such a file gets that
length as a parameter. Since M8.3 the reader makes the updates of the shapes' settings too. In a file
before version 3, a zigzag stroke's pull compensation is halved for each side, a satin column starts
and ends where it is drawn, and a start or end command works at its connector's end on its object
(M8.4). In a file before version 4, a fill with `auto_fill` off is a legacy fill. `SC-I0818` counts the shapes
and commands each update changed. A file with Ink/Stitch's content and no version, which Ink/Stitch asks its user about,
and one from a newer Ink/Stitch are read as they are, with `SC-W0802`.

Style sheets set properties as in a viewer (`src/sheet.rs`, `REQ-SVG-006`). Their rules with element,
class and id selectors, the ones Illustrator and CorelDRAW write, are read, in the order of CSS's cascade.
One `SC-W0802` names the other selectors and the at-rules.

Design: `docs/src/design/data-model.md` (the `Design`), `docs/src/design/architecture.md#hosts-ports-and-adapters`.

## Invariants

- Contains no stitch logic; only translation.
- Never fails on content: any path data, degenerate arcs included, is read the way SVG viewers draw it
  (`REQ-SVG-002`); the reader is fuzzed nightly (`fuzz/fuzz_targets/read_svg.rs`). Only a file that is not
  SVG at all, or whose size cannot be read, is refused (`SC-E0801`).
- Nothing is dropped silently: `SC-W0802` for SVG features it does not stitch, `SC-W0804` for geometry
  it cannot use, `SC-I0805` for what the file itself asks to leave out, unless a viewer would not show it
  either. Ink/Stitch's command symbols and their connectors are its controls, not drawing, so they are
  left out without a word; a command that is not applied is noted.
- Positions are exact to within a micrometre (`REQ-SVG-001`) and the same on every platform: transforms
  and arcs use `stitchcraft_core::math`, never the platform's trigonometry.
- Work is bounded. A file is at most 64 MiB with its entities expanded (`src/text.rs`) and a million XML
  nodes, and its style sheets have at most a million selectors, declarations and at-rules. Reading
  charges the budget two units per XML node, one in the pass that indexes the file and one in the pass
  that reads it, and the search for Ink/Stitch's file version a unit per node it looks at and per step up
  from an element of its name. Style sheets cost a unit per 8 bytes, and each element a unit per class,
  per rule tested against it and per declaration it takes.
- Parameters will round-trip unchanged (`REQ-PRM-003`, M8).

## Dependencies

`stitchcraft-core`, `stitchcraft-params`, `stitchcraft-plan`, `stitchcraft-engine`; `roxmltree` (the XML
tree) and `svgtypes` (SVG's micro-syntaxes), the versions VectorCraft's lockfile carries, and only here
(`cargo xtask layers`).
