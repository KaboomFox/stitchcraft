# Satin generators

<!-- implements: crates/stitchcraft-engine/src/normalize/satin.rs, crates/stitchcraft-engine/src/normalize/near.rs, crates/stitchcraft-engine/src/generators/satin/** -->

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
- **Single-path satin**: a centre line whose stroke gives the width ([below](#single-path-satin)). It makes quick borders.

## Recognizing rails and rungs

An element is a satin column when its `satin_column` setting is on, whatever its `stroke_method` says.
Its path's subpaths are flattened, and the reader finds where each pair of subpaths meets, crossing or
touching. A point the two share counts once, and a rung that ends exactly on a rail meets it.

StitchCraft tells rails from rungs as Ink/Stitch does, and a file sews the same in both.

1. A subpath that is one point is left out, with `SC-W0205`.
2. With 1 subpath left, the path is the column's centre line ([below](#single-path-satin)). With 2, they
   are the rails, and their nodes pair up in place of rungs (see Correspondence). With none, the element gets
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

## Single-path satin

A satin column drawn as one subpath is its centre line. Its stroke gives the column its width and its
corners: the stroke's width as the transforms scale it, and its join
([SVG input](../svg-input.md#stroke-width-and-join)).

A column no wider than the design's narrowest satin stroke is too narrow to stitch across
(`REQ-SAT-015`). That limit is `DesignSettings::min_satin_stroke_width`, Ink/Stitch's
`min_satin_stroke_width_mm`, and it is 1 mm unless the design sets it. Such a column is sewn as the
element's stroke settings sew a stroke, by default in running stitch. `SC-W0212` gives its width and the
limit. As a stroke, it offers the element before it its first point.

The rule counts the subpaths drawn, as Ink/Stitch counts them. A path of 2 subpaths, one of them a point,
is never sewn as a stroke for its width. Recognition leaves the point out and reads the other subpath as a
centre line.

A wider column becomes 2 rails along its centre line from M4.9 on. Until then it is skipped, and
`SC-W0011` says so.

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
nodes, and can warn where the nodes pair as drawn. Rails of 2 nodes each are cut once near their
starts, and sew as one section. The cut on each rail is where the point 0.2 CSS pixels along the straight
line from its first node to its last lies along it, as Ink/Stitch places the rung it adds there.

When the resulting stitch directions deviate from the local column normal by more than 45° somewhere,
the element gets `SC-W0208` ("add a rung here") with the location.

## Compensation

Thread under tension pulls the fabric in across a satin column, which then sews narrower than it is
drawn. The stitches also push the fabric out at the column's ends, and it sews longer. StitchCraft makes
up for both as Ink/Stitch does.

- **Push compensation** (`push_compensation_mm`) acts on the rails after any reversal, before they are
  cut into sections. It takes its length off each rail at the column's start and end, or adds it where
  negative, straight on from the rail's first or last segment. 2 values set the start, then the end.
  The points that say where to cut come from the rails as drawn, so a rung in a part taken off cuts the
  rail at its end and leaves its section out. A rail the shortening would leave shorter than half a CSS
  pixel keeps its length (`SC-W0211`), and a lengthening at its other end still applies.
- **Pull compensation** acts on each pair as it is placed. Both ends move outward along the line through
  the pair, each by `pull_compensation_mm` plus `pull_compensation_percent` of the pair's width, and 2
  values set the first rail's side, then the second's. Negative values move the ends inward. Ends that
  would cross meet instead, where their moves divide the width. A pair whose ends are closer than a
  ten-thousandth of a CSS pixel has no direction to move them in, and stays as it is.
- **Random variation** comes from the element's generator, seeded with `random_seed`
  ([determinism](../determinism.md)). Each pair's share of the width on each side is drawn between
  `pull_compensation_percent` less `random_width_decrease_percent` and plus
  `random_width_increase_percent`. Each step's spacing is drawn between the zigzag spacing less and plus
  `random_zigzag_spacing_percent` of it, and never below a hundredth of it. As in Ink/Stitch, a spacing
  is drawn at each section's start and after each pair, and the widths with each pair. Ink/Stitch draws
  from another generator, and its random values differ (`DEV-SAT-002`).

Pairs are placed and measured from each other before pull compensation, which leaves each pair's place
along the column as it was.

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
   - With `random_zigzag_spacing_percent`, each step's spacing is drawn at random (*Compensation*), and
     a pair moves to lie that spacing from the one before.
2. **Compensate width.** Each pair is widened by pull compensation as it is placed, with its random
   share (*Compensation*).
3. **Short stitches on curves,** as in Ink/Stitch, where they are on by default. On the inside of a
   tight curve, a rail's needle points crowd together and the thread piles up. On each rail, a point
   closer than `short_stitch_distance_mm` to the last point left in place there moves in along its
   stitch, by a `short_stitch_inset` percentage of the stitch's width. Points that crowd one after
   another take turns with the percentages and start over at the first, so `15 30` insets them by 15 %,
   30 %, 15 % and on. A point at least that far from the last one left in place stays, and the next
   points are measured from it. A distance of 0 insets none. The points are the compensated ones, and an
   inset moves its point toward the other end of the stitch as negative pull compensation does. Even
   splits (step 4) hold an inset to a third of the longest stitch.
4. **Split long stitches,** as Ink/Stitch splits them, where the element sets a longest stitch
   (`max_stitch_length_mm`, one of the settings every stitch type shares). A stitch's ends are its pair's
   compensated points, and it is sewn between their insets. `split_method` says where the splits go.
   - `default` splits a stitch into the fewest equal parts no longer than the longest stitch, and the
     stitch back after it into as many parts as the stitch across before. `random_split_jitter_percent`
     moves each split by up to that share of a part, either way, at random.
   - With `random_split_phase`, the default method starts the splits a random share of the longest
     stitch from the stitch's inset start instead, and follows at the longest stitch, longer or shorter
     by up to the jitter. Stitches no longer than `min_random_split_length_mm` (by default the longest
     stitch) are not split. Of 2 or more splits, one within the element's shortest stitch of an end is
     left out.
   - `simple` splits at whole multiples of the longest stitch from a stitch's start, and from a stitch
     back's end, where they lie between its insets. The splits line up in rows.
   - `staggered` does the same from a start that moves along by a `split_staggers`-th of the longest
     stitch from one stitch to the next, and comes back after that many stitches. Fractions are allowed.
   - With even splits (the default method without random phase), a short stitch's inset (step 3) is at
     most a third of the longest stitch.
5. **Alternate.** Each pair is sewn rail A first, then rail B, and the needle goes A, B, A, B and on.
   The stitch from A to B goes straight across, and the one from B to the next A slants. One zigzag cycle
   (A → B → A) spans `zigzag_spacing_mm`. The exit end is chosen by assembly (see *Start and end*).
   Random splits are drawn after the pairs are placed, in the order the stitches are sewn.

## Underlays

Underlays are sewn before the top stitches, as Ink/Stitch sews them. The centre walk comes first, then the
contour and the zigzag, each one the element turns on, and all of them go into one run with the top
stitches.

Each underlay places pairs of points across the column as the top stitches are placed (step 1 of *Top
stitches*), along the same sections, after any swap, reversal and push compensation. A pair moves in by
its underlay's insets as negative pull compensation moves it, and nothing varies at random, so turning an
underlay on leaves the top stitches as they were.

| Underlay | Parameters | What it does |
|---|---|---|
| Centre walk | `center_walk_underlay`, `_stitch_length_mm`, `_stitch_tolerance_mm`, `_repeats`, `_position` | Pairs every `_stitch_tolerance_mm`, each moved in until its ends meet `_position` percent of the way from the first rail to the second (50 is the middle). A running stitch of `_stitch_length_mm` follows the line through them within the tolerance, there and back `_repeats` times |
| Contour | `contour_underlay`, `_stitch_length_mm`, `_stitch_tolerance_mm`, `_inset_mm`, `_inset_percent` | Pairs every `_stitch_tolerance_mm`, inset by `_inset_mm` plus `_inset_percent` of the width on each side, and a running stitch along each side. Each side then stops short of the column's start by the first rail's inset in millimetres and of its end by the second's, as push compensation shortens a rail. A side too short for that keeps its length (`SC-W0206`). The first rail's side goes towards the column's end, and the second's back |
| Zigzag | `zigzag_underlay`, `_spacing_mm`, `_inset_mm`, `_inset_percent`, `_max_stitch_length_mm` | Pairs every half `_spacing_mm`, inset by its own insets, which are half the contour's when left empty. It zigzags to the column's end through one end of each pair, the rails taking turns. It comes back through the other ends. Each way, a rail's points lie the spacing apart. A stitch longer than `_max_stitch_length_mm` is split into equal parts |

A centre walk with an odd number of repeats ends at the column's end. The contour's first side then goes
back and its second side on, the zigzag starts from the end, and the top stitches run from the end to the
start. Straight stitches join each part to the next, all of one length and no longer than the first of
the running stitch's lengths (`running_stitch_length_mm`).

The walks are running stitches, placed as a stroke's are ([strokes](strokes.md#running-stitch)), and they
differ from Ink/Stitch's in the same ways (`DEV-RUN-001` to `DEV-RUN-003`). Where the insets cross in a
narrow column, a pair's ends meet as pull compensation's do, and the contour's sides then run along one
line. StitchCraft differs from Ink/Stitch in 2 more places. The centre walk keeps to its own tolerance,
where Ink/Stitch uses the satin's running stitch tolerance (`DEV-SAT-004`). A running stitch length of
several values gives the travel its first, where Ink/Stitch falls back to 2.5 mm (`DEV-SAT-005`).

## Start and end

A column starts near where the elements before it left the needle, and ends near where the next element
starts, as in Ink/Stitch, which does both by default. The engine gives each element 2 neighbours
([ADR 0014](../adr/0014-generators-see-their-neighbours.md)). The first is the last needle point of the
elements before it, whatever their thread. The second is what the next element offers to end near. Strokes
offer their first point. Satin columns that start at their own nearest point offer their rails, and
others the start of their first rail. Either way the rails are as they are sewn, after any swap and
reversal. Fills offer nothing until they are sewn (M5).

**The line.** The way to the start and the way to the end follow a line between the rails,
`running_stitch_position` percent of the way from the first rail to the second (50 is the middle). It is
built as the centre walk's is. Pairs are placed every `running_stitch_tolerance_mm` and moved in until
their ends meet the line. A running stitch of the first of the running stitch's lengths
(`running_stitch_length_mm`) goes through them within the same tolerance.

**Cuts.** A place on the column is found by its cut, one of the pairs placed at the zigzag spacing with
no compensation and nothing random. The cut is the pair with the point nearest the place. Where the
needle enters the line for a place is the line's point nearest that cut. A part of the column is cut in 2
at its own point nearest the cut. When the cut's 2 points coincide, where the rails meet, a part is not
cut: it is all first, or all second when its start is within 0.1 CSS pixels of the place.

**Starting at the nearest point** (`start_at_nearest_point`). With a needle point before it, the column
starts at the point of the line nearest that needle point. When that point is farther from the needle
than the jump length, and the compensated outline is nearer than the jump length, it starts at the
outline's point nearest the needle instead. The jump length is the element's `min_jump_stitch_length_mm`,
or the design's collapse length when the element sets none or 0. The compensated outline is the 2
polylines through the ends of the top stitches' pairs, after pull compensation and random width, before
short stitches. From its start, the needle enters the line where the start's cut says and follows it to
where the first stitch's cut says, and the column goes on from its first stitch as before.

**Ending at the nearest point** (`end_at_nearest_point`). With an element after it, the column ends at
the point of its compensated outline nearest the next stitch. The next stitch is the point of the
column's rails nearest the next element's first point, or nearest its rails when it offers them. The
column's rails are measured as they are sewn. The column ignores an end less than 5 CSS pixels (1.32 mm)
from the line's end, and ends as if it were the last element. Otherwise every part of the column is cut
in 2 at the end's cut. The centre walk is cut before its repeats, and the zigzag's ways before their long
stitches are split. The contour's sides and the top stitches are cut too. The column sews every part's
first half in order. Then the needle follows the line from the end's cut to the line's end, and the
column sews every part's second half in order and stitches the end last.

The halves follow each part's direction. The centre walk's second half is turned to run from the
column's end, and each half is sewn its repeats. The contour's first pass is its first side up to the cut
and its second side from the cut back to the start, and its second pass the rest of the second side, then
the rest of the first. The zigzag's first pass goes to the cut and back, and its second goes from the end
to the cut and back to the end. The top stitches' second half is turned to run from the column's end to
the cut.

**After an odd centre walk,** which brings the needle to the column's end, the column's start and end
trade places in the halves above. The first pass works from the column's end towards the cut, and the
second from its start. The centre walk's halves are its part from the cut to the end and its part from
the cut to the start, each sewn its repeats. The first pass begins at the cut. Between the passes the
needle goes to the line at the cut, where the second pass begins.

**Equally near points.** Shapes are often equally near at several points, as rails side by side are. The
column takes the point Ink/Stitch takes, found in the order its geometry library, shapely, finds them.
Shapes are measured from one to the other, polyline by polyline and side by side. Sides that cross or
touch meet exactly where they do, and distances are measured as that library measures them, so equal
distances compare as equal. A cut is measured from the part it cuts, and the line is entered measuring
from the cut. The engine's tests check its answers against shapely's for 1,600 random cases on coarse
grids, where ties are common (`conformance/fixtures/geometry/shapely-nearest.txt`, written by
`conformance/oracle/nearest.py`). Shapely computes a crossing point in extended precision, so where that
point also lies on another side, the 2 can differ in which side they find it on.

Straight stitches join each piece to the next as between underlays, no longer than the travel length. A
point of the line within the shortest stitch of the needle only routes it, and is left out. An end within
the shortest stitch of the column's last needle point moves that point to the end. The column's last
stitch, often a top stitch, then goes to the end in one stitch. Ink/Stitch sews the end as a stitch of
its own, which its stitch plan drops when it is no longer than its shortest stitch. An explicit start or
end command, read from M8, would replace the needle point or the next stitch.

## Variants (P2, M7)

| `satin_method` | Look | Design notes |
|---|---|---|
| `e_stitch` | A spine of running stitches along one rail with regular spikes to the other — the "E" or blanket stitch used for appliqué edges | Spine on the first rail (see `swap_satin_rails`); spike spacing from `zigzag_spacing_mm` |
| `s_stitch` | Curvy stitches that read like a textured fill inside the column | Specified at M7 from public documentation and sew-out comparison |
| `zigzag` | An open zigzag between the rails | Same sampler with wider spacing and no density expectations |

## Properties (conformance)

| Requirement | Property |
|---|---|
| `REQ-SAT-001` | Pull compensation moves both ends of every top stitch outward along it, by `pull_compensation_mm` plus `pull_compensation_percent` of its width (± 0.01 mm). Ends moved past each other meet |
| `REQ-SAT-002` | Consecutive pairs are the zigzag spacing apart, measured across the previous pair at its farther end, exactly so between straight parallel rails and within 5 % on curves. The last pair is at most a spacing on |
| `REQ-SAT-003` | With a longest stitch, a longer top stitch is split as `split_method` says. By default it splits into the fewest equal parts, and the stitch back into as many as the stitch across before. Simple and staggered splits fall at whole multiples of the longest stitch, from a start that `staggered` moves along |
| `REQ-SAT-004` | Underlays come before the top stitches and leave them as they were: the centre walk, then the contour and the zigzag. Straight stitches of one length join them, no longer than the running stitch's length. After an odd centre walk the rest runs from the column's end |
| `REQ-SAT-005` | Rails and rungs are told apart as Ink/Stitch tells them, in any drawing order. A path with no subpath longer than a point produces `SC-E0201` and no stitches, and a rung that misses a rail joins the rail's nearest point (`SC-W0203`) |
| `REQ-SAT-006` | A parameter with a value for each side changes only its own side: a rail's stitch ends, or the column's start or end |
| `REQ-SAT-007` | Random widths and spacing are reproducible from the element and its seed, and stay within their ranges |
| `REQ-SAT-008` | Push compensation shortens the rails at the column's start and end before they are cut, or lengthens them where negative. A rail it would leave shorter than half a CSS pixel keeps its length (`SC-W0211`) |
| `REQ-SAT-009` | On each rail, a top stitch end closer than `short_stitch_distance_mm` to the last end left in place moves in along its stitch by the next `short_stitch_inset` percentage. Ends far enough from it stay |
| `REQ-SAT-010` | The centre walk follows the line at its position between the rails within its tolerance, in stitches no longer than its length, there and back its repeats |
| `REQ-SAT-011` | The contour runs along each rail at its insets, towards the end on the first rail's side and back on the second's. It stops short of the column's start and end by the rails' insets in millimetres |
| `REQ-SAT-012` | The zigzag goes through one end of each pair, the rails taking turns, and back through the others. Its insets are half the contour's when left empty, and its long stitches split into equal parts |
| `REQ-SAT-013` | Starting at its nearest point, a column begins on the line between its rails nearest the needle, or on its outline when only that is within the jump length, and follows the line to its first stitch |
| `REQ-SAT-014` | Ending at its nearest point, a column ends on its outline nearest the next stitch. Every part is cut in 2 there. The column sews the first halves, then the second halves from the line's end, and stitches the end last |
| `REQ-SAT-015` | A column drawn as one subpath whose stroke is no wider than the design's narrowest satin stroke sews as a stroke would, with `SC-W0212`. It offers its first point, as a stroke does |

Machine checkpoint MC-3 sews a width ladder (1–10 mm) and an underlay comparison to tune defaults
([machine testing](../../plan/machine-testing.md)).

## Diagnostics

`SC-E0201`, `SC-W0202`, `SC-W0203`, `SC-W0205`, `SC-W0206` (a contour underlay too short for its insets
keeps its length), `SC-W0207`, `SC-W0208`, `SC-W0209` (a satin wider than 12 mm risks snagging: consider
split stitches or a fill), `SC-W0210`, `SC-W0211` and `SC-W0212`.
