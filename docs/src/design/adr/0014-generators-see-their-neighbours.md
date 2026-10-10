# ADR-0014: Generators see their neighbours: the needle before, the next element after

<!-- The status line, a label and its value, which the ADR index is checked against. -->
<!-- vale ai-tells.ColonUsage = NO -->
**Status:** Accepted · 2026-10-10
<!-- vale ai-tells.ColonUsage = YES -->

## Context

The engine pipeline's design generated each element on its own. The previous element's geometry was to
suggest where the next one starts, and no generator was to wait for another's stitches, so elements could
be generated in parallel and cached ([engine pipeline](../engine-pipeline.md)).

Ink/Stitch, whose files StitchCraft sews alike, does otherwise. A satin column starts at the point of its
line nearest where the elements before it left the needle: their last stitch, which only generating them
gives. It ends at the point of its outline nearest where the next element starts, which the next
element's shape and settings say. Both are on by default (`start_at_nearest_point`,
`end_at_nearest_point`), and fills do the same from M5. A start taken from the previous element's
geometry differs from Ink/Stitch's whenever that element ends somewhere its geometry does not say. A satin
column that ends at its nearest point is one such element. Fills and running stitches sewn there and back
are others.

## Decision

1. Elements are generated in sewing order. Each is given the last needle point of the elements before it
   that sew any, whatever their thread.
2. Each is also given what the next element in the design offers to end near: its first point, or its
   shape when it starts at its own nearest point. The offer depends on the next element's shape and
   settings only, never on its stitches, so no element waits for a later one.
3. Generators are pure functions of their shape, parameters, these 2 neighbours, seed and budget.
4. Where an element's stitch type is known, it offers what Ink/Stitch's would, even before StitchCraft
   sews that type. Fills offer nothing until they are sewn (M5), and the column before a fill ends as if
   it were the last element.

## Consequences

- Generation is sequential. Each element still has its own budget, and a cache keyed on an element's
  inputs includes its 2 neighbours.
- Moving, adding or removing an element can change the stitches of the elements beside it, as in
  Ink/Stitch.
- Start and end commands, read from M8, take the place of the neighbours they override.

## Alternatives considered

- **Start from the previous element's geometry:** keeps generation parallel, but sews differently from
  Ink/Stitch wherever an element does not end where its shape suggests. Rejected: files must sew alike.
- **Generate twice,** first in parallel without neighbours and then again with them. Every element whose
  start depends on the one before still waits for it. The second pass is sequential anyway. Rejected.
