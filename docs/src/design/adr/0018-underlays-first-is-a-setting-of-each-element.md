# ADR-0018: Underlays first is a setting of each element, shared by runs of elements in one thread

<!-- The status line, a label and its value, which the ADR index is checked against. -->
<!-- vale ai-tells.ColonUsage = NO -->
**Status:** Accepted · 2026-11-03, after spike M10.14
<!-- vale ai-tells.ColonUsage = YES -->

## Context

Each element sews its underlay and its top stitches before the next element starts. Where 2 satin columns
cross, the second column's underlay is sewn over the first column's top stitches, and the crossing stacks
top stitches, underlay and top stitches again. A satin border over a fill's edge stacks the same way.
Sewing the underlays of such elements first and their top stitches after lays every underlay on the fabric
under every top stitch. The aim is a flatter crossing, which MC-12 is to check on the reference machine.
The needle then goes back from the last underlay to the first top stitch, and elements of other threads
would need more colour changes. Spike M10.14 chooses how a design sets this order, as a setting of a group
of elements or of each element. It also chooses how VectorCraft documents and SVG files keep the setting,
whether automatic digitizing uses it, and what Ink/Stitch sews from a file that has it. M10.15 builds
what it chooses.

What was checked, on 2026-11-03:

- **Ink/Stitch's order,** read at `d59c9ab` ([ADR-0012](0012-read-dont-copy.md)). Ink/Stitch embroiders
  the elements in document order, each given the stitches before it and the next element. A satin column
  makes its underlays and its top stitches as separate groups and joins them into one. The underlays come
  first, or, when the column ends at its point nearest the next element, each half's underlays come before
  that half's top stitches. A fill sews its shapes one at a time, each shape's underlay before its top
  stitches. Each element's underlay sews just before that element's top stitches.
- **Ink/Stitch's sew stack.** It is an element made from the JSON of a shape's `sew_stack` attribute,
  which lists stitch layers. The layers sew one after another. Each starts where the layer before it
  ended, and the last ends near the next element. Only a running stitch layer exists. Ink/Stitch adds the
  stack to a shape's elements only when its debug settings turn it on, and `sew_stack_only` then leaves
  the shape's other elements out. The [compatibility contract](../inkstitch-compat-contract.md) lists both
  attributes as not planned.
- **Ink/Stitch and an attribute it doesn't list.** Ink/Stitch reads each setting by its name, from its own
  list of attributes. Its Params dialog writes the values of the settings it shows, and its updater
  removes legacy names alone. So an attribute of its namespace that isn't on the list doesn't change what
  Ink/Stitch sews, and stays in the file through both. Duplicate Params copies every attribute of the
  namespace from the first selected shape to the others, and Remove Embroidery Settings deletes them all.
- **Wilcom EmbroideryStudio,** in its documentation. Branching joins objects into one branched object,
  which takes the colour of the first object in its sequence. Where the objects touch or overlap, the
  underlay of the whole branched object is sewn first, followed by the cover stitches.
- **What StitchCraft has.** Generators give assembly each element's needle points without roles. A satin
  column's underlays and top stitches are one run of needle points
  ([satin](../algorithms/satin.md#underlays)), and so are a fill part's underlay and top rows
  ([fills](../algorithms/fills.md#6-underlay)). Both underlays are placed without random variation.
  Assembly tags every stitch of a group top, keeps the elements in their order and starts a colour block
  at each thread change. Elements are generated in sewing order,
  each given the needle before it and what the next element offers
  ([ADR-0014](0014-generators-see-their-neighbours.md)). The settings every stitch type shares are one
  declaration of the registry. The registry's only settings of StitchCraft's own so far are automatic
  digitizing's, which no stitch type reads. In an SVG file each `inkstitch:*` attribute of a shape is a
  parameter of its elements (`REQ-SVG-009`), and a group's settings are named and not applied. In a
  VectorCraft document each parameter is a row of each stitch family's dialog, made from the registry, and
  a group's effects apply alike to every path in the group (`REQ-VC-006`). A live effect sews its object
  alone. M13.3 orders the automatic elements, and with `--sort-colours` every element, so that each thread
  sews as long as it can. The routing tools write a route in its sewing order, and each piece keeps the
  settings of the element it comes from ([ADR-0017](0017-routing-is-one-engine-function.md)). On a home
  machine such as the reference machine, each colour change is a stop to rethread
  ([image to embroidery](../image-to-embroidery.md#sewing-order)). TS-17C is to sew 2 satin columns that
  cross, their underlays first, and MC-12's report is to say whether they lie flatter than as drawn
  ([machine testing](../../plan/machine-testing.md)).

## Decision

1. **A setting of each element.** `underlays_first` is a toggle among the settings every stitch type
   shares, off by default and StitchCraft's own. The registry declares it once, and its reference entry,
   its JSON Schema and its row in each stitch family's dialog in VectorCraft come from that declaration.
2. **Runs.** Consecutive elements that set it and sew in one thread form a run, in the sewing order stage
   0 gives. An element's trim or stop ends its run after it. An element in several threads (M10.5) is in
   no run. A run of one element sews as it would without the setting, and a note names an element whose
   setting joins no run.
3. **The underlays, then the top stitches.** A run sews its elements' underlays in their order, then their
   top stitches in the same order. Each element of a run is generated as 2 layers, its underlay and its
   top, in the order the run sews them. Each layer gets the needle before it and what the next layer
   offers, which ADR-0014 gives an element. Assembly joins the layers like any other groups. It tags the
   needle points of the underlay layers underlay.
4. **Files.** In a VectorCraft document the setting is a parameter of an object's StitchCraft effect, and
   a group's effect gives it to every path in the group. SVG files keep it as the attribute
   `inkstitch:underlays_first` of each shape, which the SVG adapter reads as every setting of a shape. A
   group's attribute is named and not applied, as every setting of a group is.
5. **Ink/Stitch sews such a file element by element.** It doesn't read the attribute, and sews each
   element's underlay and top stitches before the next element's, as StitchCraft sews them with the
   setting off. The differential has no case for the setting.
6. **Automatic digitizing and lettering don't set it.** The elements they make take their settings from
   the rules and from the fonts.
7. StitchCraft proposes no VectorCraft change for this.

## Consequences

- A design that doesn't set it plans as before, and every golden file stays as it was.
- In VectorCraft the user turns it on in the dialog of each object's effect, or once in a group's effect
  where every path of the group sews alike. In an SVG file it is added with Inkscape's XML editor, and
  Ink/Stitch's tools treat it as [SVG input](../svg-input.md#settings-of-a-shape) says.
- Since a run's elements share a thread, the design's colour changes stay as they were. A fill and a
  border in other threads sew one after the other, each with its underlay, whatever they set.
- Each run adds the needle's way back from its last underlay to its first top stitch, a jump with its
  locks where it is longer than the jump length. Elements that lie apart add jumps between their underlays
  too. The setting is for elements that cross or overlap.
- 2 sets of elements next to each other in one thread form one run. A trim after the first set keeps them
  apart.
- An element's start position places the start of its first layer, and its end position the end of its
  top. The 2 layers share the element's work budget, as its underlay and top share it outside a run.
  Random variation gives the top stitches what it gives them without the setting.
- `--sort-colours` moves the elements that set it as it moves any element with settings, and the runs form
  in the order it chooses. A run of routed pieces sews its underlays along the route, and the needle then
  goes back to the route's first piece for the top stitches.
- A live effect sews its object alone, so VectorCraft's previews can't show the order. `stitch export` and
  the exporters sew it.
- M10.15's cases check that a run's underlays all come before its top stitches, that the top stitches
  keep the elements' order, and that a design without the setting plans as before. TS-17C sews 2 satin
  columns of one thread that cross, as drawn beside the same 2 with their underlays first, for MC-12 to
  compare. The test kit stops at a setting of StitchCraft's own when it writes a file for Ink/Stitch. The
  differential's file of TS-17C leaves that pair out.
- The attribute is in Ink/Stitch's namespace. Should a later Ink/Stitch name a setting `underlays_first`,
  the contract updated to it fails `cargo xtask docs --check`, as a setting of StitchCraft's own may not
  take an Ink/Stitch key. A migration then renames StitchCraft's
  ([parameter registry](../params.md#versioning-and-migration)).
- Revisit runs of one thread if a sew-out shows that a border lies flatter with its underlay sewn before a
  fill of another thread, which costs 2 colour changes more. Revisit automatic digitizing if MC-12 shows
  that crossing columns lie flatter with their underlays first: a step of M13 could then set it on the
  columns of one shape that cross, measured on the test pictures. Revisit the namespace when the project
  has a domain of its own, or if Ink/Stitch asks other programs to keep out of its namespace. Revisit a
  group's own setting in VectorCraft if VectorCraft gives a plug-in data of its own on a group, or shows a
  filter the groups round a path.

## Alternatives considered

- **A setting of the group.** The engine has no groups. Adapters give it the elements in sewing order, and
  stage 0 can move a group's elements apart. Of a group's settings, Ink/Stitch reads `ignore_object`
  alone, and StitchCraft names the others. In VectorCraft every path of a group sews as the group's
  effects say. One group's effect can't serve a fill and its satin border. A live effect that only marks
  the group could store a setting of the group itself, and VectorCraft would run it on each path inside
  at every redraw. A name like the markers' could store one too, though no dialog shows it and SVG files
  have no such names. Rejected. A group's effect already gives the setting to every path of a group that
  sews alike.
- **A design setting.** Every element would join a run with its neighbours of the same thread, though the
  order serves only elements that cross or overlap. Ink/Stitch's metadata holds its 4 settings for the
  whole design, and StitchCraft names any other element there. Rejected.
- **Runs across threads,** each underlay in its element's thread. Sewn this way, a run of 2 threads at
  least doubles its colour changes, each a stop on a home machine. Sewing the underlays in one thread
  would change what the user drew where an underlay shows. Rejected. Wilcom's branched objects take one
  colour too.
- **Reordering the groups as generated.** Each element would still start and end near its neighbours in
  element order, so the needle would jump between every 2 layers. The generators join underlay and top
  into one run of needle points, and would have to give them apart anyway. Rejected.
- **The top stitches in reverse order,** to start them where the underlays end. An element drawn over
  another must sew its top stitches after the other's, as M13.3's first rule says. Rejected.
- **Numbered or named runs,** so that 2 runs next to each other stay apart. The plug-in's dialogs leave
  texts out, and a name would keep its default in VectorCraft. A number would have to be set alike on
  every element of a run. A trim keeps 2 runs apart already, and runs that join only lengthen the
  needle's way between the 2 passes. Rejected.
- **A namespace of StitchCraft's own,** such as a `stitchcraft:` attribute. The SVG adapter reads settings
  from Ink/Stitch's namespace, and Ink/Stitch ignores the attribute in either. A namespace of StitchCraft's
  own needs a URI, which waits on the project's name and domain ([TDD](../tdd.md#19-open-questions),
  question 2). Rejected for now.
- **Automatic digitizing sets it** on the columns of a shape that cross or meet. No sew-out has shown the
  gain yet, MC-8 is to confirm the rules as they are, and the way back from each run's last underlay would
  add jumps that M13.3's order saves. Rejected for now.
- **Ink/Stitch's sew stack.** It orders the stitch layers of one element, and Ink/Stitch builds it only
  with its debug settings on. Not planned, as the contract says.
- **A filter that sets it on the selected objects.** VectorCraft gives a filter each selected path's own
  effects, which it can change in one undo step. The dialogs and a group's effect cover the designs that
  need the setting now. Not planned until designs need it on many objects of several families.

## References

- Ink/Stitch at `d59c9ab`: `lib/extensions/base.py` (`elements_to_stitch_groups`),
  `lib/elements/satin_column/satin_column.py` (`to_stitch_groups`, `_sort_stitch_groups`),
  `lib/elements/fill_stitch.py` (`to_stitch_groups`), `lib/sew_stack/__init__.py` and
  `lib/sew_stack/stitch_layers/`, `lib/elements/utils/nodes.py`, `lib/debug/debug.py`,
  `lib/elements/element.py` (`get_param`), `lib/svg/tags.py`, `lib/extensions/params.py`,
  `duplicate_params.py` and `remove_embroidery_settings.py`, and `lib/update.py`.
- [Automatic branching](https://docs.wilcom.com/embroiderystudio/28/en/OnlineHelp/Quality/connectors/connectors-20.htm)
  and [Apply automatic underlay to branched objects](https://docs.wilcom.com/embroiderystudio/28/en/OnlineHelp/Quality/connectors/connectors-23.htm),
  in Wilcom EmbroideryStudio's documentation, version 28.
- [Underlays first](../engine-pipeline.md#underlays-first),
  [settings of a shape](../svg-input.md#settings-of-a-shape) and
  [the workflow in VectorCraft](../vectorcraft-integration.md#the-workflow).
