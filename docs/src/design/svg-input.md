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

## Colour and visibility

- `fill`, `stroke`, `color` and `currentColor` give the thread colour. A gradient gives its first colour
  and a pattern its fallback colour, each with a warning.
- `display: none` hides an element and everything in it. A hidden or fully transparent paint is left
  out, as a viewer leaves it out. The insides of `<defs>`, `<symbol>`, `<marker>`, `<pattern>`,
  `<clipPath>` and `<mask>` are never drawn.

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

## Limits

A file is at most 64 MiB with a million XML nodes, and it does not declare XML entities. Reading charges the
budget for each XML node, path segment and 8 bytes of a transform or point list
([budgets](data-model.md#budgets)).
