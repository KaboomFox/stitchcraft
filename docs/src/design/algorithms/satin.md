# Satin generators

<!-- implements: crates/stitchcraft-engine/src/normalize/satin.rs, crates/stitchcraft-engine/src/generators/satin/** -->

A satin column is a band of closely spaced stitches that swing from one edge to the other. It is the
signature look of lettering and borders, and the stitch type where pull compensation and underlay
matter most. Phase P1 (M4); the E, S and zigzag variants are P2 (M7).

## Shape

```text
 rail A  ●────────●──────────●─────────●
          \  |     \    |     \   |
 rungs     \ |      \   |      \  |       (optional: say which points correspond)
 rail B  ●──\┴───────\──┴───────\─┴───●
```

- **Rails** are the two edges. They run in the same general direction.
- **Rungs** are short segments crossing both rails; they pin which point on rail A corresponds to which
  point on rail B, controlling the stitch angle through curves.
- **Single-path satin**: a centre line with a width (the stroke width, or a parameter), for quick borders.

## Recognizing rails and rungs

An element is a satin column when its `satin_column` setting is on, whatever its `stroke_method` says.
Its path's subpaths are flattened, and the reader finds where each pair of subpaths meets, crossing or
touching. A point the two share counts once, and a rung that ends exactly on a rail meets it.

StitchCraft tells rails from rungs as Ink/Stitch does, and a file sews the same in both.

1. A subpath that is one point is left out, with `SC-W0205`.
2. With 1 subpath left, the path is the column's centre line, sewn from M4.8 on. With 2, they are the
   rails, and their nodes pair up in place of rungs (see Correspondence). With none, the element gets
   `SC-E0201` and no stitches.
3. With 3 subpaths, the rails are the 2 that meet exactly 1 other subpath. With 4 or more, the rails are
   the 2 that meet more than 2 others. This step takes only subpaths longer than a tenth of a CSS pixel.
4. When step 3 does not find exactly 2 rails, the 2 longest subpaths are taken as the rails, and
   `SC-W0202` names them. Of equally long subpaths, the first drawn comes first. Rails apart from each
   other, with exactly 2 rungs between them, are always taken by length, because each of the 4
   subpaths meets 2 others, as the strokes of a `#` do. The longer pair is usually the rails, and with
   a third rung step 3 finds them.
5. Rails may meet each other, as the two sides of a pointed column do at its tips. That meeting counts
   in step 3.
6. Every other subpath is a rung. A rung joins the point where it crosses the first rail to the point
   where it crosses the second. Where it misses a rail, the point of that rail nearest the rung is used,
   with `SC-W0203`. A rung that crosses a rail more than once does not say which crossing it means. It
   is left out, with `SC-W0207`, and the satin is sewn without it.

Recognition runs before any stitch is computed. The generator only ever gets rails, and odd geometry is
a diagnostic that names the subpath, never a crash. Subpaths are numbered from 1 in the order the path
draws them, and the rails keep that order.

StitchCraft differs from Ink/Stitch in 2 places, `DEV-SAT-001` in the deviations ledger
(`conformance/deviations.toml`). Ink/Stitch still uses a rung that crosses one rail twice and misses
the other, or that runs along a rail for a while, at one of its crossings. StitchCraft leaves it out.
Ink/Stitch also counts a line of zero length among the subpaths in step 3, where StitchCraft leaves it
out as a point.

## Orientation

- `swap_satin_rails` makes the second rail the first, before any reversal. The first rail sews first in
  each pair, and the column starts on it. Asymmetric values (one value per side) name it first, and the
  E-stitch spine runs on it.
- `reverse_rails = automatic` reverses rail B, the second, when that brings the rails' points closer
  together, as in Ink/Stitch. Points at every tenth of each rail's length, from its start to 90 %, are
  paired twice, with rail B forwards and with it backwards. The pairing whose distances add up to less
  wins. `none`, `first`, `second` and `both` force it.

## Correspondence

Every rung cuts each rail at the distance along it of the rung's point on it, after any reversal. Each
rail is cut at its own distances, in order, and the n-th part of one rail goes with the n-th part of the
other: a section. A part of no length leaves its section out, as where two rungs meet a rail at one
point or one meets it at an end. Within a section, the point at a fraction of rail A's part goes with the
point at the same fraction of rail B's.

A column of 2 subpaths has no rungs, and its rails' nodes cut the rails instead, as in Ink/Stitch. The
2nd node of one rail goes with the 2nd node of the other, and on in order, after any reversal, without
each rail's 2 ends. Rails with different numbers of nodes pair as many as the one with fewer has
(`SC-W0210`). Ink/Stitch's own warning for this counts the rails' points after flattening, not their
nodes, and can warn where the nodes pair as drawn. Rails of 2 nodes each are cut once, 0.2 CSS pixels
from their starts, and sew as one section.

When the resulting stitch directions deviate from the local column normal by more than 45° somewhere,
the element gets `SC-W0208` ("add a rung here") with the location.

## Top stitches (method `satin_column`)

1. **Place pairs along the sections,** as Ink/Stitch places them. A pair of needle points goes across
   the column, one on each rail at the same fraction of its section. Each pair is meant to lie
   `zigzag_spacing_mm` from the one before, measured across the column. The measure is taken at a right
   angle to the previous pair, at whichever of its ends is farther (the outside of a curve). Where the
   previous pair has no length, it is the distance from that pair's point.
   - Within a section, a pair is placed the spacing's share of the section's longer part past the one
     before. A pair that lands more than 5 % off the spacing moves by its step scaled by how far off it
     is. It moves at most twice, and the first move stops at the section's end.
   - The first pair of a section after the first goes as far past the section's start as the previous
     pair is short of the spacing.
   - The column starts with a pair at its start and ends with one at its end, unless the last pair is
     within 0.1 mm of it.
   - `random_zigzag_spacing_percent` jitters each step (seeded, M4.3).
2. **Compensate width.** For each pair, move both ends outward along the A–B line by
   `pull_compensation_mm + pull_compensation_percent × width` per side; one value applies to both sides,
   two values (`"0.2 0.4"`) apply per rail. `random_width_increase_percent` and
   `random_width_decrease_percent` add per-side jitter. Push compensation (`push_compensation_mm`)
   pulls the column's two ends inward along the column, because the fabric pushes satin stitches
   outward at the ends.
3. **Short stitches on curves.** Where consecutive points on the inner rail are closer than
   `short_stitch_distance_mm`, every other stitch ends `short_stitch_inset` percent of the width short of
   that rail, so the inner edge does not pile up.
4. **Split long stitches.** A stitch longer than `max_stitch_length_mm` (if set) is split into equal
   pieces. `split_method` decides where the split points fall from one stitch to the next:
   `simple` (same fractions every time), `staggered` (offsets cycle every `split_staggers` stitches, like
   tatami rows) or `default` (random phase with `random_split_jitter_percent`, `random_split_phase`,
   and pieces no shorter than `min_random_split_length_mm`). Split points never line up into a visible
   seam on more than `split_staggers` consecutive stitches.
5. **Alternate.** Each pair is sewn rail A first, then rail B, and the needle goes A, B, A, B and on.
   The stitch from A to B goes straight across, and the one from B to the next A slants. One zigzag cycle
   (A → B → A) spans `zigzag_spacing_mm`. The exit end is chosen by assembly (see *Start and end*).

## Underlays

Sewn before the top stitches, in this order, each optional:

| Underlay | Parameters | What it does |
|---|---|---|
| Centre walk | `center_walk_underlay`, `_stitch_length_mm`, `_stitch_tolerance_mm`, `_repeats`, `_position` | Running stitch along the line at `position` percent between the rails (50 = centre). Repeats alternate direction; an even count ends where it started |
| Contour | `contour_underlay`, `_stitch_length_mm`, `_stitch_tolerance_mm`, `_inset_mm`, `_inset_percent` | Running stitch along each rail, inset toward the centre; stabilizes the edges |
| Zigzag | `zigzag_underlay`, `_spacing_mm`, `_inset_mm`, `_inset_percent`, `_max_stitch_length_mm` | A sparse zigzag inside the inset band; lifts the top stitches |

The underlays are routed so the column ends where the top stitching should start; if the inset band
collapses (the column is too narrow) the underlay is skipped with `SC-W0206`.

## Start and end

With `start_at_nearest_point`, the column begins at whichever end is nearest the previous element's
exit; with `end_at_nearest_point`, the top stitching finishes at the end nearest the next element's
entry, which may mean the underlay runs one way and the top the other. Explicit start/end commands
override both (`REQ-GEN-001`). Travel between underlay passes uses `running_stitch_length_mm`,
`running_stitch_tolerance_mm` and `running_stitch_position`.

## Variants (P2, M7)

| `satin_method` | Look | Design notes |
|---|---|---|
| `e_stitch` | A spine of running stitches along one rail with regular spikes to the other — the "E" or blanket stitch used for appliqué edges | Spine on the first rail (see `swap_satin_rails`); spike spacing from `zigzag_spacing_mm` |
| `s_stitch` | Curvy stitches that read like a textured fill inside the column | Specified at M7 from public documentation and sew-out comparison |
| `zigzag` | An open zigzag between the rails | Same sampler with wider spacing and no density expectations |

## Properties (conformance)

| Requirement | Property |
|---|---|
| `REQ-SAT-001` | Every top stitch end lies on its rail, offset outward by exactly the compensation (± 0.01 mm) |
| `REQ-SAT-002` | Consecutive pairs are the zigzag spacing apart, measured across the previous pair at its farther end, exactly so between straight parallel rails and within 5 % on curves. The last pair is at most a spacing on |
| `REQ-SAT-003` | No top stitch exceeds `max_stitch_length_mm` after splitting; split seams follow the chosen method |
| `REQ-SAT-004` | Underlays lie inside the inset band and precede the top stitches |
| `REQ-SAT-005` | Rails and rungs are told apart as Ink/Stitch tells them, in any drawing order. A path with no subpath longer than a point produces `SC-E0201` and no stitches, and a rung that misses a rail joins the rail's nearest point (`SC-W0203`) |
| `REQ-SAT-006` | Asymmetric parameters affect only their side |
| `REQ-SAT-007` | Random parameters are reproducible from the seed |

Machine checkpoint MC-3 sews a width ladder (1–10 mm) and an underlay comparison to tune defaults
([machine testing](../../plan/machine-testing.md)).

## Diagnostics

`SC-E0201`, `SC-W0202`, `SC-W0203`, `SC-W0205`, `SC-W0206`, `SC-W0207`, `SC-W0208`, and `SC-W0209` (a satin
wider than 12 mm risks snagging: consider split stitches or a fill).
