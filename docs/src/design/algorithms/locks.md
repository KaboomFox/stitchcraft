# Lock stitches

A lock, or tie, is a few small stitches where an element's stitching starts (the tie-in) or ends (the
tie-off), so the thread holds when it is trimmed or jumps on. [Plan
assembly](../engine-pipeline.md#lock-stitches-ties) decides which ends of which groups get one (`ties`,
`force_lock_stitches`, jumps and trims); this page specifies the lock itself: `stitchcraft_engine::locks`,
phase P1 (M3.7).

**Parameters:** `lock_start` and `lock_end` (the shape), `lock_custom_start`, `lock_custom_end`,
`lock_start_scale_mm`, `lock_end_scale_mm`, `lock_start_scale_percent`, `lock_end_scale_percent`
([reference](../../user/reference/params/common.md#lock-stitches)).

## The frame

The anchor is the group's first needle point for a tie-in and its last for a tie-off. The lock is drawn in
a frame at the anchor: x along the stitch from the anchor to the next needle point that is not the anchor
itself, pointing into the stitching, and y a quarter turn from it. The lock is straight in that frame, so
one that reaches no further than that stitch lies on it, and the stitching covers it. A group with no such
second point has no stitch to lock and gets no lock.

## The two ends

A lock is sewn in the same order at both ends; only the frame turns round with the stitching.

- A **tie-in** leads into the anchor: the needle arrives at the lock's first point, sews the lock, and the
  group's first needle point follows.
- A **tie-off** leaves from the anchor: the group's last needle point, then the lock.

Every built-in lock comes back to where it started, so a tie-in starts where the stitching starts and a
tie-off ends where the stitching ended, which is where the thread is trimmed.

## Shapes

The ids are Ink/Stitch's, so a file that asks for a bowtie gets one. The shapes behind them are
StitchCraft's own designs (deviation `DEV-LCK-001`): they are defined once, in
`crates/stitchcraft-engine/src/locks/shapes.rs`, and the lists of locks each size parameter applies to are
computed from that table.

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
stitch is sized from the stitch it lies on and takes neither.

Every drawn shape keeps the same rules, so each one holds the same way and hides under the stitching it
secures:

- a loop from the anchor round and back to it;
- at most 1.4 mm along the stitching and 0.6 mm across it at 100 %, mostly ahead of the anchor;
- stitches 0.46 mm to 1.4 mm long at 100 %, well clear of the 0.2 mm a lock stitch needs and long enough to
  grip.

## Steps

Steps are distances along x, in sewing order, positive into the stitching. A tie-in's needle positions are
the anchor less the steps still to come, so its last step lands on the anchor; a tie-off's are the anchor
plus the steps sewn so far. With steps of `2 -1` in sizes of 0.5 mm, a tie-in starts 0.5 mm behind the
start of the stitching, goes 1 mm forth and 0.5 mm back onto it; a tie-off goes 1 mm back over the last
stitches and 0.5 mm forth again. Steps that add up to 0, as every built-in lock's do, start a tie-in and
end a tie-off at the anchor.

## Custom locks

`lock_custom_start` and `lock_custom_end` hold either numbers separated by spaces — steps, in sizes of
`lock_*_scale_mm` — or an SVG path that draws the lock. Which of the two a text is decides how a file
sews, so StitchCraft reads it as Ink/Stitch does (`REQ-LCK-004`): a text made only of digits, spaces, dots,
commas and minus signs is numbers (a newline may end it); anything else is a path. The steps are the
pieces between spaces that read as numbers, so `1,5` is not one.

StitchCraft says what it cannot sew, where Ink/Stitch passes over it in silence (`SC-W0503`):

- a piece that is not a number, or a step longer than 10 m (the data model's reach), is left out;
- with no step left — an empty text, say — the half stitch is sewn instead;
- a lock drawn as an SVG path is not sewn yet, and the half stitch is sewn instead: the engine never reads
  SVG, so the SVG adapter will hand it the path (roadmap M8).

A step of 0 moves nowhere and is no step: it would sew in place.

## The shortest lock stitch

No lock stitch is shorter than 0.2 mm (`LOCK_MIN_STITCH`, which the plan checker holds lock stitches to:
`REQ-PLAN-002`), so the needle never goes back into the hole it just left. Joins count: a tie-in's last
stitch into the anchor and a tie-off's first stitch out of it. A step shorter than that is lengthened to
it, keeping its direction; a drawn lock whose shortest stitch would be shorter is enlarged as a whole until
it is that long, keeping its shape. Both say by how much (`SC-W0502`). The half stitch never needs it.

## Compared with Ink/Stitch

Read at `d59c9ab` ([ADR-0012](../adr/0012-read-dont-copy.md)). The ids, how a custom lock's text is read,
and which size parameter applies to which lock are the same; the shapes differ (`DEV-LCK-001`):

- Ink/Stitch's half stitch goes towards the first needle point at least 0.5 mm away, in a straight line to
  it, out and back by halves of that distance capped at 1.5 mm. StitchCraft's goes forth and back over
  half the first stitch, which then covers it.
- Ink/Stitch's back-and-forth lock goes two steps out and two back; StitchCraft's goes forth and back over
  one step, twice, like its half stitch.
- The drawn shapes are StitchCraft's own designs.
- Ink/Stitch draws custom SVG paths; StitchCraft sews the half stitch with `SC-W0503` until M8.

## Properties

The conformance suite checks, through the engine's public API (`crates/stitchcraft-engine/tests/locks.rs`):

1. **Every id** sews a lock that joins the stitching where it starts or ends (`REQ-LCK-002`).
2. **No lock stitch shorter than 0.2 mm,** joins included, for any group, lock and size; the same input
   always sews the same lock (`REQ-LCK-002`, property test).
3. **Sizes:** drawn locks scale with `lock_*_scale_percent` alone, locks made of steps with
   `lock_*_scale_mm` alone, and the half stitch with neither (`REQ-LCK-002`).
4. **Custom steps** sew as Ink/Stitch reads them (`REQ-LCK-004`).

Where locks go (`ties`, `REQ-LCK-001`) is checked with plan assembly (M3.8), and whether the default locks
hold when the tail is pulled (`REQ-LCK-003`) at machine checkpoint MC-2, with test sheet TS-04.
