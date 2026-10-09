# Lock stitches

<!-- implements: crates/stitchcraft-engine/src/locks/** -->

A lock, or tie, is a few small stitches where an element's stitching starts (the tie-in) or ends (the
tie-off). The lock stops the thread from pulling out when it is trimmed or the needle jumps on.
[Plan assembly](../engine-pipeline.md#lock-stitches-ties) puts locks at the ends of groups, as `ties`,
`force_lock_stitches`, jumps and trims say. This page specifies the lock itself:
`stitchcraft_engine::locks`, phase P1 (M3.7).

**Parameters:** `lock_start` and `lock_end` (the shape), `lock_custom_start`, `lock_custom_end`,
`lock_start_scale_mm`, `lock_end_scale_mm`, `lock_start_scale_percent`, `lock_end_scale_percent`
([reference](../../user/reference/params/common.md#lock-stitches)).

## Placement

The anchor is the group's first needle point for a tie-in and its last for a tie-off. A group with no
needle point other than the anchor has no stitch to lock, and assembly sews it without one.

- **Steps** are distances along the stitching from the anchor, positive into it. A lock of steps follows
  the stitching round its corners. It lies on the stitches that come after it, and they cover it. A
  negative distance goes straight back along the first stitch, before the anchor. Past the end of
  stitching shorter than the lock, the lock goes straight on from the last stitch.
- **A drawn lock** lies in a frame at the anchor. Its x axis runs along the stitch from the anchor to the
  next needle point at another place, pointing into the stitching. Its y axis is a quarter turn from x.

A sharp turn can bring two needle points of a lock of steps closer together than the steps between them.
When following the stitching would sew a stitch shorter than 0.2 mm, the lock is sewn straight along the
first stitch, in the frame, and `SC-W0502` says so.

## Tie-in and tie-off

A lock is sewn in the same order at both ends. At a tie-off, into the stitching is back along it.

- A **tie-in** leads into the anchor. The needle sews the lock from its first point, and the group's first
  needle point follows.
- A **tie-off** leaves from the anchor: the group's last needle point, then the lock.

The built-in locks come back to where they started. A tie-in starts where the stitching starts, and a
tie-off ends where the stitching ended, which is where the thread is trimmed.

## Shapes

The ids are Ink/Stitch's, and a file that names a bowtie gets one. The shapes behind them are
StitchCraft's own designs (deviation `DEV-LCK-001`). One table in
`crates/stitchcraft-engine/src/locks/shapes.rs` defines them. The lists of locks that each size parameter
applies to come from that table.

| Lock | Kind | Size | What the needle sews |
|---|---|---|---|
| `half_stitch` (default) | steps | the first stitch | forth and back over half the first (or last) stitch, twice: steps half as long as that stitch, from 0.2 mm to 1 mm |
| `back_forth` | steps | `lock_*_scale_mm` | forth and back over one step, twice: the half stitch's rhythm, at a size the user sets |
| `custom` | steps | `lock_*_scale_mm` | the element's own steps ([custom locks](#custom-locks)) |
| `arrow` | drawn | `lock_*_scale_percent` | a shaft along the stitching to the tip of an arrowhead 1.4 mm ahead, round the head and back down the shaft |
| `bowtie` | drawn | `lock_*_scale_percent` | two triangles tip to tip across the stitching, 1.2 mm long, their long stitches crossing in the middle |
| `cross` | drawn | `lock_*_scale_percent` | an X 0.8 mm across, 0.3 to 1.1 mm ahead |
| `star` | drawn | `lock_*_scale_percent` | a five-pointed star 1.2 mm across, one of its points at the anchor |
| `simple` | drawn | `lock_*_scale_percent` | a diamond of four 0.58 mm stitches round the stitching |
| `triangle` | drawn | `lock_*_scale_percent` | a triangle opening ahead, its tip at the anchor, 1.2 mm long |
| `zigzag` | drawn | `lock_*_scale_percent` | zigzag across the stitching for 1.2 mm, then straight back along it |

A settings window shows `lock_*_scale_mm` only for the locks made of steps it sizes (`back_forth`,
`custom`) and `lock_*_scale_percent` only for the drawn ones and `custom`, as Ink/Stitch's does. The half
stitch is sized from the stitch it lies on and takes its size from neither parameter.

The drawn shapes follow the same rules. Each one secures the thread in the same way and hides under the
stitching it secures:

- a loop from the anchor round and back to it
- at most 1.4 mm along the stitching and 0.6 mm across it at 100 %, mostly ahead of the anchor
- stitches 0.46 mm to 1.4 mm long at 100 %, well clear of the 0.2 mm a lock stitch needs and long enough to
  grip

## Steps

Steps are distances along the stitching, in sewing order, positive into it. A tie-in's needle positions are
the anchor less the steps still to come, and its last step ends at the anchor. A tie-off's positions are
the anchor plus the steps sewn so far. Take steps of `2 -1` in sizes of 0.5 mm. A tie-in starts 0.5 mm
behind the start of the stitching, goes 1 mm forth and goes 0.5 mm back onto it. A tie-off goes 1 mm back
over the last stitches and 0.5 mm forth again. Steps that add up to 0, as the built-in locks' steps do,
start a tie-in and end a tie-off at the anchor.

## Custom locks

`lock_custom_start` and `lock_custom_end` hold numbers separated by spaces, or an SVG path that draws the
lock. The numbers are steps, in sizes of `lock_*_scale_mm`. The kind of text sets how a file sews, and
StitchCraft reads it as Ink/Stitch does (`REQ-LCK-004`). A text made only of digits, spaces, dots, commas
and minus signs is numbers, and a newline may end it. Any other text is a path. The steps are the pieces
between spaces that read as numbers, and `1,5` is not one of them.

StitchCraft reports what it cannot sew (`SC-W0503`), where Ink/Stitch passes over it in silence:

- A piece that is not a number, or a step longer than 10 m (the data model's reach), is left out.
- With no step left, as in an empty text, the half stitch is sewn instead.
- A lock drawn as an SVG path is not sewn yet, and the half stitch is sewn instead. The engine never reads
  SVG, and the SVG adapter will hand it the path (roadmap M8).

A step of 0 moves nowhere and would sew in place. It is not a step.

## The shortest lock stitch

A lock stitch is at least 0.2 mm long (`LOCK_MIN_STITCH`), and the plan checker holds lock stitches to
that length (`REQ-PLAN-002`). The needle never goes back into the hole it just left. Joins count: a
tie-in's last stitch into the anchor and a tie-off's first stitch out of it.

- A step shorter than 0.2 mm is lengthened to it, keeping its direction.
- A drawn lock whose shortest stitch would be shorter is enlarged as a whole until that stitch is 0.2 mm
  long, keeping its shape.
- A lock of steps that a sharp turn would fold onto itself is sewn straight ([placement](#placement)).

`SC-W0502` reports each of these, with the size. The half stitch never needs them.

## Compared with Ink/Stitch

Read at `d59c9ab` ([ADR-0012](../adr/0012-read-dont-copy.md)). The ids, how a custom lock's text is read,
and which size parameter applies to which lock are the same. The shapes differ (`DEV-LCK-001`):

- Ink/Stitch's half stitch goes towards the first needle point at least 0.5 mm away, in a straight line to
  it, out and back by halves of that distance capped at 1.5 mm. StitchCraft's goes forth and back over
  half the first stitch, which then covers it.
- Ink/Stitch's back-and-forth lock goes two steps out and two back. StitchCraft's goes forth and back over
  one step, twice, like its half stitch.
- The drawn shapes are StitchCraft's own designs.
- Ink/Stitch draws custom SVG paths. StitchCraft sews the half stitch with `SC-W0503` until M8.

Both follow the stitching with custom steps. They differ in 2 places (`DEV-LCK-002`):

- Ink/Stitch puts the needle points past the end of stitching shorter than the lock on its last needle
  point. StitchCraft goes straight on from the last stitch, and no two needle points coincide.
- Ink/Stitch follows a turn however sharp. StitchCraft sews straight a lock that a turn would fold onto
  itself.

## Properties

The conformance suite checks, through the engine's public API (`crates/stitchcraft-engine/tests/locks.rs`):

1. **Each id** sews a lock that joins the stitching where it starts or ends (`REQ-LCK-002`).
2. **The shortest lock stitch** is 0.2 mm, joins included, for any group, lock and size. The same input
   sews the same lock (`REQ-LCK-002`, property test).
3. **Sizes:** the drawn locks scale with `lock_*_scale_percent` alone, and the locks made of steps with
   `lock_*_scale_mm` alone. The half stitch scales with neither (`REQ-LCK-002`).
4. **Custom steps** sew as Ink/Stitch reads them, round the stitching's corners (`REQ-LCK-004`).

Plan assembly (M3.8) checks where locks go (`ties`, `REQ-LCK-001`). Machine checkpoint MC-2 checks with test
sheet TS-04 whether the default locks hold when the tail is pulled (`REQ-LCK-003`).
