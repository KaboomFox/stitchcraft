# SVG input

<!-- implements: crates/stitchcraft-svg/src/** -->

The SVG adapter, `stitchcraft-svg`, turns an SVG file into a [`Design`](data-model.md#engine-input-design-stitchcraft-engine)
and a list of warnings. It reads what an SVG viewer draws, in millimetres, and reports what it leaves
out. The engine never sees SVG ([architecture](architecture.md#hosts-ports-and-adapters)).

## What becomes an element

- **Shapes.** Each `<path>`, `<rect>`, `<circle>`, `<ellipse>`, `<line>`, `<polyline>` and `<polygon>`
  that paints becomes one element per paint. A fill becomes an area and a stroke becomes an outline.
  A `<line>` has no fill.
- **Order.** Elements follow document order, which is SVG's paint order and the sewing order. A
  shape's two paints follow its `paint-order`.
- **Groups.** `<g>`, `<a>` and the child a `<switch>` chooses pass their transform and style to their
  children.
- **Ids.** An element's id is `svg:<label>:fill` or `svg:<label>:stroke`. The label is the SVG
  element's `id`, or `<tag>@<n>` for the n-th element of that kind. A repeated id gets `~2`, `~3` and
  so on. The adapter's own diagnostics name the SVG element as `svg:<label>`.
- **Names.** An `inkscape:label` becomes the element's name.

## Geometry and units

- The root's `width`, `height`, `viewBox` and `preserveAspectRatio` map user units to millimetres. A
  user unit is a CSS pixel, 1/96 inch, unless they say otherwise.
- Transforms compose through every ancestor. The matrices use `stitchcraft_core::math`, so the result
  is the same on every platform ([determinism](determinism.md)).
- Path data is read as SVG viewers draw it, arcs included. An arc becomes cubic curves of at most 30°.
  An error in the data ends the path at the error.
- Positions are exact to within a micrometre (`REQ-SVG-001`). Any path data reads without failing the
  adapter, and fuzzing checks it (`REQ-SVG-002`).
- A length may have spaces around it. One that does not read is 0, as in a viewer, and `SC-W0804` names
  it.

## Colour and visibility

- `fill`, `stroke`, `color` and `currentColor` give the thread colour. A gradient gives its first colour
  and a pattern its fallback colour, each with a warning. A gradient of one colour, such as an Inkscape
  swatch, gives that colour without one.
- Colours read as editors write them. Keywords such as `currentcolor` may be in any case. An ICC colour
  after the sRGB one, which Inkscape's colour-managed picker writes, is left for the sRGB colour.
- A declaration that does not read gives way to the next one, as in CSS: the last valid one in `style`,
  else a valid presentation attribute. When none of an element's paints reads, the inherited paint
  stays, and `SC-W0802` names the value.
- `display: none` hides an element and everything in it. A hidden or fully transparent paint is left
  out, as a viewer leaves it out. The insides of `<defs>`, `<symbol>`, `<marker>`, `<pattern>`,
  `<clipPath>` and `<mask>` are never drawn.

## Ink/Stitch objects

An Ink/Stitch SVG has 3 kinds of object that Ink/Stitch reads and does not sew. The adapter sews none of
them (`REQ-SVG-003`).

- **Commands.** The `trim` and `stop` commands turn on the object's `trim_after` and `stop_after`. The
  `ignore_object` and `ignore_layer` commands leave out an object or a layer, as the
  `inkstitch:ignore_object` setting does, and `SC-I0805` lists each object left out. The
  [command table](inkstitch-compat-contract.md#commands) gives what the adapter does with each of the
  12 commands.
- **Connectors** tie a command to its object.
- **Helper paths** have one of Ink/Stitch's start markers. They steer the stitches of other objects.

`SC-W0802` reports a connector that ties no command, a helper path, and a command that the adapter does
not apply yet. The module docs of `crates/stitchcraft-svg/src/inkstitch.rs` describe how the adapter
finds these objects, with the Ink/Stitch files they were read from.

## Reports

- `SC-W0802` reports a feature that is not stitched. Text, images, clones, nested `<svg>` elements and
  style sheets are such features. So are clipping, masks, filters, markers and the Ink/Stitch settings
  that are not read yet.
- `SC-W0804` reports geometry that cannot be used:
  - path data with an error, stitched up to the error
  - a transform that does not parse, left out with its contents
  - an element that draws nothing
  - a point more than 10 m from the origin
- `SC-E0801` refuses a file that is not SVG, or that is larger than the adapter reads.

## Text and limits

A file is UTF-8 text, or ISO-8859-1 when its XML declaration says so, which the adapter turns into UTF-8.
A file in any other encoding is refused when it has a byte outside ASCII.

A file is at most 64 MiB with a million XML nodes. Illustrator declares its namespaces as XML entities,
and an entity whose value is plain text is read. A file is refused when an entity's value holds markup or
other entities, or when its references would expand the file past 64 MiB. Without both rules a small
file could need a large amount of memory. Reading charges the budget for each XML node, path segment and
8 bytes of a transform or point list ([budgets](data-model.md#budgets)).
