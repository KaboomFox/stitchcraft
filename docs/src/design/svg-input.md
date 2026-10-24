# SVG input

<!-- implements: crates/stitchcraft-svg/src/** -->

The SVG adapter, `stitchcraft-svg`, turns an SVG file into a [`Design`](data-model.md#engine-input-design-stitchcraft-engine)
and a list of warnings. It reads what an SVG viewer draws, in millimetres, and reports what it leaves
out. The engine never sees SVG ([architecture](architecture.md#hosts-ports-and-adapters)).

## What becomes an element

- **Shapes.** Each `<path>`, `<rect>`, `<circle>`, `<ellipse>`, `<line>`, `<polyline>` and `<polygon>`
  that paints becomes one element per paint. A fill becomes an area and a stroke becomes an outline.
  A `<line>` has no fill.
- **Where a shape starts.** A running stitch starts where its path starts and runs the way it runs, and
  each basic shape starts and runs as Ink/Stitch reads it. A circle or an ellipse starts at its top and
  turns left on screen. SVG 2 starts it at its right and turns the other way, which only dashes or
  markers would show.
- **Order.** Elements follow document order, which is SVG's paint order and the sewing order. A
  shape's fill is sewn before its stroke, as Ink/Stitch sews them, and the outline covers the fill's edge.
  When `paint-order` paints the stroke first, `SC-W0802` says the fill is sewn first.
- **Groups.** `<g>`, `<a>` and `<switch>` pass their transform and style to their children. A
  `<switch>` draws only its first child whose conditions pass, as a viewer does. Ink/Stitch sews every
  child of a switch, and drops the transforms of the switch and of the elements above it
  (`DEV-SVG-004`).
- **Ids.** An element's id is `svg:<label>:fill` or `svg:<label>:stroke`. The label is the SVG
  element's `id`, or `<tag>@<n>` for the n-th element of that kind. A repeated id gets `~2`, `~3` and
  so on (`stitchcraft_core::Labels`, as in the VectorCraft adapter). The adapter's own diagnostics name
  the SVG element as `svg:<label>`.
- **Names.** An `inkscape:label` becomes the element's name.
- **Automatic elements.** Read with `--auto` (`Reading::auto`), a shape with no attribute in Ink/Stitch's
  namespace is an automatic element, whose stitches StitchCraft chooses by its width
  ([image to embroidery](image-to-embroidery.md#automatic-elements), `REQ-AUTO-001`). The trim and stop
  of Ink/Stitch's commands attached to it still apply. A shape with any `inkstitch:` attribute is not
  automatic: it sews as its settings say once they are read (M8), and with the defaults until then.

## Geometry and units

- The root's `width`, `height`, `viewBox` and `preserveAspectRatio` map user units to millimetres. A
  user unit is a CSS pixel, 1/96 inch, unless they say otherwise. A root with only a width or only a
  height takes the other from its viewBox's shape, as SVG 2 sizes an image, and a percentage counts as
  missing. Ink/Stitch takes both sides from the viewBox, in pixels, when either is missing
  (`DEV-SVG-009`).
- A viewer ignores a root `width`, `height` or `viewBox` that is empty, and a `preserveAspectRatio` that
  does not read. The adapter ignores them too, and `SC-W0804` names each. A `width`, `height` or
  `viewBox` that does not read could put the design at any size, and the file is refused (`SC-E0801`).
  So is a size of 0 or less, and a viewBox with no area, in which a viewer draws nothing.
- Transforms compose through every ancestor. The matrices use `stitchcraft_core::math`, so the result
  is the same on every platform ([determinism](determinism.md)).
- Path data is read as SVG viewers draw it, arcs included. An error in the data, or in the list of
  points of a `<polyline>` or `<polygon>`, ends the path at the error, and `SC-W0804` names it. inkex
  skips what is not a number in a list of points, and Ink/Stitch sews the rest (`DEV-SVG-010`).
- A path has the subpaths a viewer draws. A move-to with nothing drawn after it is no subpath, and a
  command that draws after a closepath starts a new subpath where the closed one started. inkex keeps
  such a move-to as a subpath of one point and runs the closed subpath on, so Ink/Stitch can count other
  subpaths, which tell a satin column's rails from its rungs (`DEV-SVG-011`).
- An arc becomes cubic curves as inkex, which Ink/Stitch reads files with, cuts it: in equal pieces, one
  more than the whole quarter turns it makes. Manual stitch puts a needle point on each piece's end, as
  Ink/Stitch's does. inkex counts the quarter turns in floating point, and its rounding can cut an arc of
  a whole number of quarter turns into one piece fewer (`DEV-SVG-002`).
- A `<circle>` or an `<ellipse>` starts at its top and goes round anticlockwise on screen, as inkex
  draws it, so a stroke round it starts and runs as Ink/Stitch's does.
- A `<rect>` with one radius rounds its corners with it, and one with a radius of 0 has square corners.
  An `<ellipse>` with one radius is a circle of that radius. That is how SVG 2 draws them. inkex rounds
  a rect whose `rx` or `ry` is 0 with the other, and takes an ellipse's missing radius as 0
  (`DEV-SVG-006`).
- Positions are exact to within a micrometre (`REQ-SVG-001`). Any path data reads without failing the
  adapter, and fuzzing checks it (`REQ-SVG-002`).
- A length may have spaces around it. One that does not read is 0, as in a viewer, and `SC-W0804` names
  it. A percentage is of the viewport's width, height or diagonal over √2, an `em` is 16 user units and
  an `ex` 8, as with a viewer's default font. inkex reads them as 0 in a shape's attributes
  (`DEV-SVG-005`).

## Colour and visibility

- `fill`, `stroke`, `color` and `currentColor` give the thread colour. A gradient gives its first colour
  and a pattern its fallback colour, each with a warning. A gradient of one colour, such as an Inkscape
  swatch, gives that colour without one. A gradient without stops paints nothing, as in a viewer, and
  so does a reference to an element the file does not have, unless a colour is written after it.
  Ink/Stitch sews such a fill in black (`DEV-SVG-007`).
- Colours read as editors write them. Keywords such as `currentcolor` may be in any case. An ICC colour
  after the sRGB one, which Inkscape's colour-managed picker writes, is left for the sRGB colour.
- Properties come from presentation attributes (`fill="red"`), the rules of style sheets and the `style`
  attribute, in the order of CSS's cascade, as in a viewer (`REQ-SVG-006`). A rule overrides a
  presentation attribute, and the `style` attribute overrides a rule. Of 2 rules, the more specific one
  applies (more ids, then more classes, then an element name), and of 2 as specific, the later one. An
  `!important` declaration overrides all of these. A paint, colour, stroke width, join or miter limit
  that does not read gives way to the next declaration, as in CSS. When none of an element's paints
  reads, the inherited paint stays, and `SC-W0802` names the value. inkex takes the declaration the
  cascade puts first whether it reads or not, and Ink/Stitch sews a fill whose colour does not read in
  black and leaves such a stroke out (`DEV-SVG-008`). `display`, `visibility`, `opacity`,
  `fill-opacity`, `stroke-opacity`, `fill-rule` and `paint-order` are read as inkex reads them, from
  the declaration the cascade puts first. A value there that does not read is ignored, and the
  declarations after it are not used, where a viewer would use the next one.
- Style sheets are the `<style>` elements of the file, wherever they are, whose type is CSS. Illustrator
  and CorelDRAW write a drawing's colours as classes there. The adapter reads a rule whose selector is an
  element name or `*` followed by classes and ids, such as `rect`, `.cls-1`, `#logo` or `path.st0.st1`.
  It reads a list of such selectors, such as `.a, .b`, as well. One `SC-W0802` names the other selectors,
  such as `g > rect` or `a:hover`, and the at-rules, such as `@media` and `@import`. What they set is not
  used. Ink/Stitch reads styles through inkex, which applies more kinds of selector.
- `display: none` hides an element and everything in it. A hidden or fully transparent paint is left
  out, as a viewer leaves it out. `visibility` hides a paint, and so does an `opacity` of 0 on its
  element or above it. A `fill-opacity` or `stroke-opacity` of 0 makes its paint clear, as does a colour
  that is `transparent` or has an alpha of 0. Ink/Stitch leaves out only what `display` hides and a fill
  whose `fill-opacity` is written `0` (`DEV-SVG-003`). The insides of `<defs>`, `<symbol>`, `<marker>`,
  `<pattern>`, `<clipPath>` and `<mask>` are never drawn.

## Stroke width and join

A stroke's width and join give a satin column drawn as one path its width and its corners
([single-path satin](algorithms/satin.md#single-path-satin)). Other strokes are sewn along their paths,
however wide they are drawn. Both are read as Ink/Stitch reads them (`REQ-SVG-004`), except an automatic
element's join, read as SVG draws it. Its cap matters only to an automatic element, whose stitches cover
the line as it is drawn (`REQ-AUTO-003`).

- **Width.** The width is `stroke-width`, inherited, 1 user unit when nothing sets it. It is a length in
  the shape's own user units, as in a viewer, and a percentage is of the viewport's diagonal over √2.
  The transforms down to the shape scale it by the average of how far they stretch the x and y axes.
  That is exact when they scale both axes alike. Under an uneven scale or a skew, a viewer draws the
  stroke wider one way than the other, and the average is the middle way. An automatic line sewn across
  its width, as a satin column or a fill, is then as wide as that average all along, and `SC-W0802`
  names it (`REQ-AUTO-005`). The transforms scale a stroke with `vector-effect: non-scaling-stroke` too, as
  Ink/Stitch scales it, though a viewer does not.
- **Join.** `round` and `bevel` are read as they are named. A `miter` join is limited by
  `stroke-miterlimit`, 4 when nothing sets it, as SVG says. Any other join, or none, is a miter limited
  at 5, as in Ink/Stitch. SVG 2's `miter-clip` and `arcs` count as other joins. An automatic element's
  corners follow the drawing instead (`REQ-AUTO-004`): a join that is not set is SVG's initial miter,
  limited by `stroke-miterlimit`, and so are `miter-clip` and `arcs`, which few viewers draw.
- **Cap.** `stroke-linecap`, inherited, `butt` when nothing sets it: `butt`, `round` or `square`, in any
  case. Ink/Stitch sews no cap, and reads none.
- **Values that do not read.** A width, join or limit that does not read is ignored, as a viewer
  ignores it, and `SC-W0804` names it. A cap that does not read is ignored too, without a word: it changes
  nothing for a shape with settings. A negative width and a limit below 1 do not read. A stroke too
  wide to measure, such as `1e308in`, is left out with `SC-W0804`. Its fill is still sewn.

Ink/Stitch reads a width in `%`, `em` or `ex` as 0, and a join written in capitals as no join at all. The
adapter reads them as a viewer draws them (`DEV-SVG-001`).

## Ink/Stitch objects

An Ink/Stitch SVG has 3 kinds of object that Ink/Stitch reads and does not sew. The adapter sews none of
them (`REQ-SVG-003`).

- **Commands.** The `trim` and `stop` commands turn on the object's `trim_after` and `stop_after`. The
  `ignore_object` and `ignore_layer` commands leave out an object or a layer, as the
  `inkstitch:ignore_object` setting does when it says yes in one of Ink/Stitch's words, which a toggle
  reads too, and `SC-I0805` lists each object left out. The
  `starting_point` and `ending_point` commands, which place where Ink/Stitch starts and ends a fill or a
  satin column, are not applied yet (`REQ-GEN-001`), and `SC-W0802` names each one on an object a viewer
  shows. The [command table](inkstitch-compat-contract.md#commands) gives what the adapter does with
  each of the 12 commands.
- **Connectors** tie a command to its object.
- **Helper paths** have one of Ink/Stitch's start markers. They steer the stitches of other objects.

`SC-W0802` reports a connector that ties no command, a helper path, and a command that the adapter does
not apply yet. The module docs of `crates/stitchcraft-svg/src/inkstitch.rs` describe how the adapter
finds these objects, with the Ink/Stitch files they were read from.

## Design settings

Ink/Stitch keeps 3 settings for the whole design in the file's first `<metadata>` element:
`collapse_len_mm`, `min_stitch_len_mm` and `min_satin_stroke_width_mm`. Each is an element of
Ink/Stitch's namespace whose text is the value in millimetres, and the first of each name counts. They
become the design's collapse length, shortest stitch and narrowest satin stroke (`REQ-SVG-005`).
`SC-W0802` names a value that is not a number of 0 or more, and the design keeps its default. The
defaults are in the [data model](data-model.md#engine-input-design-stitchcraft-engine).

## Reports

- `SC-W0802` reports a feature that is not stitched. Text, images, clones, nested `<svg>` elements and
  the style sheet rules that are not read are such features. So are clipping, masks, filters, markers
  and the Ink/Stitch settings that are not read yet. Clipping is planned for M8.
- `SC-W0804` reports geometry that cannot be used:
  - path data or a list of points with an error, stitched up to the error
  - a transform that does not parse, left out with its contents
  - an element that draws nothing
  - a point more than 10 m from the origin
  - a stroke width, join or miter limit that does not read, and a stroke too wide to measure
  - an empty root `width`, `height` or `viewBox`, and a root `preserveAspectRatio` that does not read
- `SC-E0801` refuses a file that is not SVG, or that is larger than the adapter reads. It refuses a file
  whose size cannot be used too: a root `width`, `height` or `viewBox` that does not read, a size of 0
  or less, or a viewBox with no area.

## Text and limits

A file is UTF-8 text, or ISO-8859-1 when its XML declaration says so, which the adapter turns into UTF-8.
A file in any other encoding is refused when it has a byte outside ASCII.

A file is at most 64 MiB with a million XML nodes, and its style sheets have at most a million selectors,
declarations and at-rules. Illustrator declares its namespaces as XML entities, and an entity whose value
is plain text is read. A file is refused when an entity's value holds markup or other entities, or when
its references would expand the file past 64 MiB. Without these rules a small file could need a large
amount of memory. Only the document type declaration's internal subset, `<!DOCTYPE svg [ … ]>`, declares
entities: a declaration in a comment or in character data declares nothing. A name declared twice counts
at its longer value. XML keeps the first declaration, and the limit must hold whichever the parser keeps.
Reading charges the budget for each XML node, path segment and 8 bytes of a transform, point list or
style sheet. Each element is charged for its classes, the style sheet rules tested against it and the
declarations it takes ([budgets](data-model.md#budgets)).
