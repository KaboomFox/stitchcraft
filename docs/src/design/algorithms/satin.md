# Satin generators

<!-- implements: crates/stitchcraft-engine/src/normalize/satin.rs, crates/stitchcraft-engine/src/normalize/near.rs, crates/stitchcraft-engine/src/normalize/centre_line.rs, crates/stitchcraft-engine/src/generators/satin/**, crates/stitchcraft-engine/src/generators/pull.rs -->

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
Its path's subpaths are flattened as Ink/Stitch flattens them: each curve is halved until its control
points lie within a tenth of a CSS pixel of the line between its ends. The same subpaths are then the
same polylines in both, which decide where the needle points go, where subpaths meet, and where a column
drawn as one path gets its rungs. A path read from SVG has the subpaths a viewer draws, and inkex, which
Ink/Stitch reads files with, does not always count them alike (`DEV-SVG-011`). The reader finds where
each pair of subpaths meets, crossing or touching. A point the two share counts once, and a rung that
ends exactly on a rail meets it.

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
   with `SC-W0203`, measured against the rail as push compensation leaves it ([Compensation](#compensation)).
   Of points as near as each other, it is the one shapely's nearest points of the rung and the rail give
   (see *Equally near points*). A rung that crosses a rail more than once does not say
   which crossing it means. It is left out, with `SC-W0207`, and the satin is sewn without it.

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

The rule counts the path's subpaths as Ink/Stitch counts them, before any is left out as a point. A path
of 2 subpaths, one of them a point, is never sewn as a stroke for its width. Recognition leaves the point
out and reads the other subpath as a centre line. In SVG, a move-to with nothing drawn after it is no
subpath, where inkex keeps it as a point, so Ink/Stitch can sew a path as a satin column where
StitchCraft sews it as a stroke (`DEV-SVG-011`).

A wider column is sewn between 2 rails made from its centre line. Ink/Stitch makes the rails and places
the rungs this way. It measures in CSS pixels, and its limits below are converted to millimetres.

### Rails

The rails are the centre line offset by half the stroke's width to each side, with the stroke's join at
their corners (`REQ-SAT-016`). In y-up axes the first lies to the right of the line as it runs and the
second to its left. The offsets are shapely's `offset_curve`, which Ink/Stitch calls, point for point
([offset curves](offset.md)). Inside a bend the offsets meet, and outside it they turn as the join
says.

### Rungs

Rungs cross the column where its line runs straight beside a sharp corner, and at its nodes
(`REQ-SAT-017`):

1. Each corner of the line scores the square of its turn in degrees, in the hundredth of the line's
   length where it lies. Each score is spread over 4 hundredths to either side, weighted 1, 2, 4, 8, 16,
   8, 4, 2, 1. A rung goes where the spread scores stop falling or start rising: before and after each
   sharp corner, where the stitches can fan round it.
2. A rung goes at each point of the flattened line as well, never at its very start or end. A straight
   line of 2 points gets one rung in its middle.
3. Going along the line, a rung less than 1 mm from the last place tried is left out, whether step 4
   kept that place's rung or not.
4. Each rung is perpendicular to the line, 1.2 times the column's width long, and is kept only when it
   crosses each rail exactly once. It pairs the 2 points where it crosses them.

A column whose rungs are all left out is one section, from its rails' starts to their ends (see
Correspondence), as in Ink/Stitch. Ink/Stitch adds rungs across the nodes from the path as drawn, and a
path of one line gives none.

### Lines that cross themselves

Where the line crosses itself or comes back near itself, its offsets split into several curves. A line
whose offsets split, or whose ends lie within a CSS pixel of each other, is cut in half by length, and
each half is made on its own, the halves of a half again, at most 20 times deep. The halves' rails are
joined end to end, and a rung pairs the first points of the second half's rails, where the halves meet.

A part whose offsets still split after 20 cuts is left out, and so is a part whose offset vanishes to
one side, where the line turns more tightly than the column is wide. `SC-W0213` counts the stretches of the line left
out, and the rest of the column is sewn. A stretch shorter than a CSS pixel is left out at once, since
every half of it would be too. A column with no part left has no rails, and gets `SC-E0214` and no
stitches.

### Closed lines

A line whose subpath is closed with `Z` is a closed line, as Ink/Stitch reads a path whose last command
is `Z`. Subpaths left out as points have no say, and an open line followed by a point closed with `Z` is
open. After a closed line, a move-to, which draws nothing, leaves it closed, where Ink/Stitch reads it as
open (`DEV-SAT-001`).

Before its rails are made, a closed line is rolled to start in the middle of its first segment whose
rung, a thousandth of a CSS pixel longer than the column is wide, crosses the edge of the stroke exactly
twice: away from where the line crosses itself or folds tight. An end of the rung that lies on the edge
counts as a crossing, as shapely counts it. A line that ends where it starts, closed or drawn back to its
start, then starts and ends in the middle of its first segment. Its points are rounded to a
ten-thousandth of a CSS pixel as Ink/Stitch rounds them. Points that then repeat are left out.

### Which lines are the rails

The rails made from the line are the column's rails, and every rung pairs the points where it crosses
them. Ink/Stitch hands the lines it makes back to its satin column as subpaths, which tells rails from
rungs again by the rules of [Recognizing rails and rungs](#recognizing-rails-and-rungs). When a column
is short beside its width, its rungs are longer than its rails. With exactly 2 rungs, or with rungs
that cross each other, where the lines meet does not tell the rails apart, and Ink/Stitch takes the 2
longest lines, rungs, as the rails. With 3 rungs or more that do not cross, each rail meets more than 2
lines and each rung only the 2 rails, so the rails are found by where they meet. A line that crosses
itself can make a rung meet more lines than a rail does. StitchCraft keeps the rails it made in both
cases, `DEV-SAT-007` in the deviations ledger. Where 2 parts join, Ink/Stitch adds a rung across the gap
and uses the points where it crosses the rails. Those are the first points of the second part's rails
wherever the parts meet on a straight stretch, and StitchCraft pairs those first points. Ink/Stitch also
keeps a first part whose offset vanishes to one side, with its other rail, where StitchCraft leaves it
out with the rest.

Making the rails costs work in proportion to the square of the line's number of points. A line of 1,000
points takes up to about 35 million units of the default budget of 500 million. A plan makes a column's
rails twice: once to sew it, and once for the element before it to end near it.

### Drawn with its rails and rungs

Ink/Stitch's Stroke to Satin hands the user a column made from a centre line as a path of its rails and
rungs, and StitchCraft's does in VectorCraft (`REQ-VC-014`, [VectorCraft
integration](../vectorcraft-integration.md#tools-stroke-to-satin-and-test-sheet)). `generators::satin::drawn`
draws the rails as they are made. Each rung is a straight line that starts and ends past the points it
pairs, by a hundredth of the distance between them, so that it crosses each rail once. Read back by [the
rules above](#recognizing-rails-and-rungs), the path gives the same rails, and its rungs pair the points
the line's rungs pair, to within a millionth of a millimetre. It sews as the line does.

A column of 2 rungs or none would be read back otherwise. With 2 rungs, where the lines meet doesn't tell
the rails apart, and the 2 longest lines are taken, which on a column shorter than it is wide are its
rungs. With none, a path of 2 lines is 2 rails whose nodes pair up. So such a column is drawn with one
more rung, which pairs the rails' first points: the section it ends has no length, and the column sews as
before. Rails that meet each other, as a pointed column's do at its tip, meet one more line, and only 2
rungs or more tell them from the rungs: such a column with fewer is drawn with a rung pairing the rails'
other ends. Push compensation lengthens the rails before they are cut. Across an end it lengthens, the
rung would make the part added there a section of its own, whose points pair otherwise. So where it
lengthens the column's start, the rung pairs the rails' last points, and an end it lengthens takes the
rung only when no other can. Stroke to Satin refuses a line whose column would need one there
(`SC-E0814`). Automatic digitizing draws the columns it finds in a part as wide as a satin column the same
way, from their rails and rungs (`drawn::from_rails`, `REQ-AUTO-017`), and reads each back. One that
reaches under other columns at both ends then sews the overlap at the end that takes the rung as a section
of its own. A line whose
rails can't be made (`SC-E0214`), or leave parts of it out (`SC-W0213`), isn't made into a column, since the
column would sew less than the line. Rails that cross each other aren't drawn
either, since their column reads back with other rails. A line no wider than the design's narrowest satin stroke sews as a
stroke ([above](#single-path-satin)). Drawn with its rails, it is a satin column as narrow as it is drawn,
which is what the tool is asked for.

## Orientation

- `swap_satin_rails` makes the second rail the first, before any reversal. The first rail sews first in
  each pair, and the column starts on it, but for an S-stitch, whose first pair goes from the second rail.
  Asymmetric values (one value per side) name it first, and the E-stitch spine runs on it.
- `reverse_rails = automatic` reverses rail B, the second, when that brings the rails' points closer
  together, as in Ink/Stitch. Points at every tenth of each rail's length, from its start to 90 %, are
  paired twice, with rail B forwards and with it backwards. The pairing whose distances add up to less
  wins. `none`, `first`, `second` and `both` force it.
- Of 2 rails that are rings, each ending where it starts (closed with `Z`, or drawn back to its start),
  rail B first starts at its node nearest rail A's first node, before any reversal (`DEV-SAT-009`).
  Ink/Stitch takes each ring's own start, and warns that a closed rail has no definite start.
  VectorCraft's Make Compound Path, from 0.10.0, turns the front of 2 rings round, which starts it at
  what was its last node. Rings sewn from their own starts then get stitches across the column. A node,
  not the nearest point, keeps rings that start together as drawn, with a rung across their starts where
  one is drawn. A column made from a centre line keeps its rails as made, which start together.

## Correspondence

Every rung cuts each rail at the distance along it of the rung's point on it, after any reversal. Each
rail is cut at its own distances, in order, and the n-th part of one rail goes with the n-th part of the
other: a section. A part of no length leaves its section out, as where two rungs meet a rail at one
point or one meets it at an end. A column left with no section sews nothing, and `SC-E0216` says why.
Ink/Stitch refuses it too, unless rounding its rails in CSS pixels leaves a section a few millionths of
a pixel long, which it sews as a fan. Within a section, the point at a fraction of rail A's part goes with the
point at the same fraction of rail B's.

A column of 2 subpaths has no rungs, and gets rungs across its rails' nodes instead, as Ink/Stitch adds
them. The 2nd node of one rail goes with the 2nd node of the other, and on in order, without each rail's
2 ends. Rails with different numbers of nodes pair as many as the one with fewer has (`SC-W0210`).
Ink/Stitch's own warning for this counts the rails' points after flattening, not their nodes, and can
warn where the nodes pair as drawn. Rails of 2 nodes each get one rung, across the points 0.2 CSS pixels
along the straight line from each rail's first node to its last, and sew as one section. Of 2 rings, the
second's nodes start at the node its line starts at, and each ring's start counts once at each end of
its nodes. VectorCraft closes a ring with a segment back to its start and `Z`, which lists its start
twice at the end of its nodes.

Each rail's nodes are turned as `reverse_rails` turns the rail drawn in that place, first or second,
before any swap. Swapping the rails then changes only which one sews first, and the same nodes pair
either way. Where the rails have different numbers of nodes, the longer rail's last nodes, in the order
the turning leaves them, go without a partner.

Each rung added is a tenth longer than the line between its points, half at each end, and cuts each
rail where it comes nearest it, measured as at the start and end (see *Equally near points*). That is
the node itself, or on a curved rail of 2 nodes, where the rung crosses the curve, or the curve's point
nearest the rung when the rung does not reach it. A rung added that meets the rails, as push
compensation leaves them, at more than 2 points doesn't say which points pair, and is left out, as
Ink/Stitch leaves it out. Such a rung crosses a node whose rail turns back across it.

Where the pairing leaves stitches leaning far along the column, `SC-W0208` names the place to add a rung
([Leaning and long stitches](#leaning-and-long-stitches)).

## Compensation

Thread under tension pulls the fabric in across a satin column, which then sews narrower than it is
drawn. The stitches also push the fabric out at the column's ends, and it sews longer. StitchCraft makes
up for both as Ink/Stitch does.

- **Push compensation** (`push_compensation_mm`) acts on the rails after any reversal, before they are
  cut into sections. It takes its length off each rail at the column's start and end, or adds it where
  negative, straight on from the rail's first or last segment. 2 values set the start, then the end.
  The points where a drawn rung crosses the rails come from the rails as drawn, so a rung in a part taken
  off cuts the rail at its end and leaves its section out. A rung that misses a rail, and a rung added
  across the nodes, are measured against the compensated rails, as Ink/Stitch measures every rung. Near
  an end that push compensation lengthens, a rung that misses the rail can come nearest the part it adds.
  It then cuts the rail on that part. Ink/Stitch at d59c9ab, run as a black box, sews such a column as it
  sews one drawn with longer rails and a rung across them there. A rail the shortening would leave shorter than half a CSS pixel keeps its length
  (`SC-W0211`), and a lengthening at its other end still applies.
- **Pull compensation** acts on each pair as it is placed. Both ends move outward along the line through
  the pair, each by `pull_compensation_mm` plus `pull_compensation_percent` of the pair's width, and 2
  values set the first rail's side, then the second's. Negative values move the ends inward. Ends that
  would cross meet instead, where their moves divide the width. A pair whose ends are closer than a
  ten-thousandth of a CSS pixel has no direction to move them in, and stays as it is. A tatami fill
  lengthens its rows with the same move, as Ink/Stitch does, and the 2 settings are declared once for
  both (`stitchcraft_engine::generators::pull`, [fills](fills.md#7-pull-compensation-and-gap-fill-rows)).
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
     is. It moves at most twice, and the first move stops at the section's end. That brings the pairs of
     a smooth curve within 5 % of the spacing. Round a sharp corner, where the next pair turns far from
     the one before, a pair can end further off: one 17.5 % off on rails that bend by 63° and 72° and
     back (`req_sat_002_on_a_curve_the_outer_edge_is_sewn_at_the_spacing`).
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

**A column too small to sew.** A column whose needle points, underlays and travel included, all lie
within its shortest stitch of its first, a speck a few tenths of a millimetre across, has no stitch the
machine sews well. It sews nothing, and `SC-W0215` says so, as a stroke that small is left out
(`SC-W0401`). Ink/Stitch sews it (`DEV-SAT-008`).

## E-stitch (method `e_stitch`)

An E-stitch edges appliqué with a spine of stitches along the first rail and a spike across the column at
each pair, like a row of E's laid end to end: the blanket stitch. Ink/Stitch's documentation suggests a
wider zigzag spacing for it than for a satin. StitchCraft sews it as Ink/Stitch sews its "E" stitch, read
at `d59c9ab`.

1. **Pairs.** The pairs are placed, compensated and inset as the top stitches' are (steps 1 to 3 of
   [Top stitches](#top-stitches-method-satin_column)), with their random variation drawn alike. Each pair
   is a spike, and the spikes lie `zigzag_spacing_mm` apart.
2. **Spikes.** At each pair the needle goes from the first rail's point across to the second rail's, and
   back down the same needle holes. The stitch across is split as a top stitch across is (step 4), as the
   stitch across that a satin column would sew at that pair. Its way back goes through the same splits in
   reverse, where a satin column's stitch back to the next pair is split anew.
3. **The spine.** From one spike to the next, the needle goes along the first rail in one stitch, from the
   spike's inset point to the next spike's. Where the zigzag spacing is longer than the longest stitch,
   that stitch is split, with no randomness. Its splits are those of a stitch from the spike's inset point
   to the next pair's compensated point on the first rail: by the default method the fewest equal parts no
   longer than the longest stitch, and by the simple and staggered methods whole multiples of the longest
   stitch from its start. As in Ink/Stitch, the zigzag spacing as set decides whether the spine is split,
   whatever length its stitches come out.
4. **Order.** The spikes go from the column's start to its end, or from its end after an odd centre walk,
   and `swap_satin_rails` puts the spine on the other rail. The underlays, the start and the end are a
   satin column's: the top stitches' run is cut and turned as a satin column's is
   ([Start and end](#start-and-end)).

## S-stitch (method `s_stitch`)

An S-stitch sews one stitch across the column at each pair, every other one back the other way, and goes
on from each to the next along a rail, so that the needle draws S's laid end to end. Split by a longest
stitch, its stitches cover the column in rows that follow it, as a fill's rows do. StitchCraft sews it as
Ink/Stitch sews its "S" stitch, read at `6555892`.

1. **Pairs.** The pairs are placed, compensated and inset as the top stitches' are (steps 1 to 3 of
   [Top stitches](#top-stitches-method-satin_column)), with their random variation drawn alike.
2. **Stitches across.** Each pair is sewn once, from one inset point to the other. The first pair, and
   every other one after it, goes from the second rail to the first, and the others from the first to the
   second. Each stitch ends on the rail the next one starts from. A stitch splits as a stitch across from
   the first rail to the second would (step 4). It is sewn the way its pair goes, with its
   splits in reverse where it starts on the second rail. The default method splits it into the fewest
   equal parts, with no count taken from the stitch before. The simple and staggered methods number the
   stitches 0, 1, 2 and on, where a satin column's stitches across take 1, 3, 5. The staggered method
   moves an S-stitch's splits along at every stitch.
3. **Steps.** From one pair to the next, the needle goes along the rail its stitch ended on in one stitch,
   from that inset point to the next pair's. The step is never split, whatever its length. Ink/Stitch
   works out splits for it where the zigzag spacing is longer than the longest stitch, and sews none of
   them (`lib/elements/satin_column/stitches.py`, `_do_s_stitch`).
4. **Order.** The pairs go from the column's start to its end, and an odd centre walk turns the whole run
   round, to go from its end. `swap_satin_rails` swaps the rail the column starts on. Its underlays, start
   and end are those of a satin column ([Start and end](#start-and-end)).

## Zigzag satin (method `zigzag`)

A zigzag satin sews a zigzag between the rails. The needle goes from a point on the first rail to a point
on the second half the zigzag spacing along, then back to the first rail half the spacing on again. Each
rail's points lie the zigzag spacing apart, and the needle crosses the column twice for each spacing along
it, as a satin column's needle does. Every stitch slants, where a satin column's stitches across go square
to the column. StitchCraft sews it as Ink/Stitch sews its zigzag satin, read at `6555892`
(`lib/elements/satin_column/stitches.py`, `_do_zigzag`).

1. **Pairs.** The pairs are placed and compensated as the top stitches' are (steps 1 and 2 of
   [Top stitches](#top-stitches-method-satin_column)), at half the zigzag spacing. Their random variation
   is drawn alike.
2. **Alternate points.** One point of each pair is sewn. The first pair gives its first rail's point, the
   next pair its second rail's point, and the pairs after them alternate the same way. Each 2 of those
   points in a row are a pair of the zigzag, from the first rail to the second. A point left over at the
   column's end is left out. A column with no second pair, 0.1 mm long or less, has no stitch across. It
   is left out with its underlays, as a column too small to sew is (`SC-W0215`, `DEV-SAT-008`).
3. **Short stitches** inset those pairs where their points crowd (step 3). Each point moves along its own
   slanted stitch.
4. **Stitches.** The needle goes across each pair, from its first inset point to its second, then back
   to the next pair's first, as in a satin column. Long stitches split by the column's split method
   (step 4). By the simple and staggered methods, the stitch back to the zigzag's pair numbered i splits
   as a satin column's stitch numbered 2i would, measured from its end on the first rail. The stitch
   across that pair splits as one numbered 2i + 1. By the default method each stitch splits into its own
   fewest equal parts. A satin column's stitch back takes its count from the stitch across before it, and
   a zigzag satin's does not, as in Ink/Stitch.
5. **Order, start and end** follow a satin column's. An odd centre walk turns the run round. The end cuts
   the top stitches where they come nearest the end's cut ([Start and end](#start-and-end)), which on a
   zigzag falls on one of its points or part way along a stitch. The outline that the start and the end
   are found on is Ink/Stitch's. It runs through pairs placed and compensated at the full zigzag
   spacing, as a satin column's top stitches are, and not through the zigzag's own. An outline through
   those would end where they do, as much as 0.1 mm from where Ink/Stitch's ends, and turn its corners
   elsewhere. The outline's random variation is drawn from a copy of the generator as the zigzag's pairs
   found it, so that the zigzag's own draws stay as they were. With none, the outline is exactly
   Ink/Stitch's.
6. **Underlays** are a satin column's, but the contour stops where the zigzag does, as in Ink/Stitch
   (`lib/elements/satin_column/underlay.py`, `_do_contour_underlay`). The zigzag's points, as inset, mark
   where it stops on each rail: its last point on the first rail, and its first and last on the second.
   Each side is cut at its points nearest those on its rail, and then stops short of the column's ends by
   the insets ([Underlays](#underlays)). A cut that would leave less than half a CSS pixel of a side is
   not made, as for the insets. Such a cut comes from a zigzag of one stitch. After an odd centre walk
   the sides stop at the same places. Ink/Stitch looks for the points in the run as sewn, turned round,
   and so cuts its first side nowhere and its second to a stub (`DEV-SAT-011`).

## Leaning and long stitches

Every satin method gives 2 warnings about how its stitches lie. Ink/Stitch gives neither.

- **Leaning stitches** (`SC-W0208`). Without rungs, or with too few, the stitches follow the rails'
  lengths alone, and where one rail runs on ahead of the other they lean along the column. The column
  looks for that in the pairs placed at the zigzag spacing with no compensation and nothing random, the
  pairs that cut it (*Cuts* in [Start and end](#start-and-end)). At a pair, the column runs halfway
  between the ways its 2 rails run there, each the line from the point of the pair before on that rail to
  the point of the pair after, taken with length 1. The first and last pair use their own point in place
  of the missing neighbour's, and a rail whose 2 points coincide gives way to the other. Where one rail
  steps out, a stitch square to the other rail so leans less than 45°. A pair leans too far when its
  stitch goes further along the column than across it, more than 45° from square to a tenth of a degree,
  so that rounding never tips a 45° stitch over. A pair shorter than the element's shortest stitch does
  not lean, as where the rails meet, and neither does one whose rails both stand still or run opposite
  ways. Each stretch of pairs in a row that lean too far gets one `SC-W0208`, at the middle of its middle
  pair, with the number of its stitches and the most any of them leans, to a tenth of a degree. A rung
  there, square to the column, sets them straight.
- **Long stitches** (`SC-W0209`). A stitch longer than 12 mm floats loose over the fabric, where it snags
  and sags. The longest of the column's top stitches, sewn with their splits and insets, gets one
  `SC-W0209` when it is longer than 12 mm. The warning is at the stitch's middle and gives its length.
  The stitches are measured as the column sews them: where its end cuts a stitch in 2, each half is a
  stitch. The underlays and the needle's travel are left out. Setting `max_stitch_length_mm` splits long
  stitches, and a shape that wide may sew better as a fill. Finalize splits a stitch longer than the
  machine's longest stitch anyway (`SC-I0703`).

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
start. Straight stitches join each part to the next, all of one length and no longer than the column's
travel length. That is the first of the running stitch's lengths (`running_stitch_length_mm`), raised to
twice the shortest stitch where it is shorter, as a running stitch's lengths are (`DEV-RUN-003`), and
`SC-W0402` says so once for the column.

Where a part starts within the shortest stitch of the needle, the needle's last point gives way to the
part's first. An underlay's last point, or the column's start, moves to it, and the stitch into that
point ends there. Points that only route the needle are left out ([Start and end](#start-and-end)). A top
stitch's point gives way to no part. Where the way along the line leaves the needle at one, the next
part's points within the shortest stitch of it are left out instead, and the needle goes on to the first
one farther. Otherwise finalize leaves out the part's first point with `SC-I0504`, even where it is a top
stitch's end. Parts meet that near in many columns. A zigzag underlay comes back 0.2 mm in from the rail
where an S-stitch starts, and after an odd centre walk it often ends that near where the top stitches
start. The end's cut can fall where the zigzag underlay's 2 ways cross, and the way along the line can
end beside where a centre walk is cut. In a column 0.5 mm wide, the centre walk is cut beside the top
stitches' end. Ink/Stitch joins its parts as they are, and its plan keeps a stitch longer than its own
shortest stitch, 0.1 mm by default (`DEV-RUN-003`).

Under a zigzag satin the contour's sides first stop where the zigzag does
([Zigzag satin](#zigzag-satin-method-zigzag), step 6).

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
reversal. Tatami fills offer their area, their parts.

**The line.** The way to the start and the way to the end follow a line between the rails,
`running_stitch_position` percent of the way from the first rail to the second (50 is the middle). It is
built as the centre walk's is. Pairs are placed every `running_stitch_tolerance_mm` and moved in until
their ends meet the line. A running stitch of the column's travel length
([Underlays](#underlays)) goes through them within the same tolerance.

**Cuts.** A place on the column is found by its cut, one of the pairs placed at the zigzag spacing with
no compensation and nothing random. The cut is the pair with the point nearest the place. Where the
needle enters the line for a place is the line's point nearest that cut. A part of the column is cut in 2
at its own point nearest the cut. A side of it that lies along the cut, both its ends within 1e-5 CSS
pixels of the cut's line, meets the cut as sides along one line do. So a stitch across that pull
compensation widens past the rails is cut at the cut's first rail point, and one it narrows at its own
start (`DEV-SAT-010`). Where a needle point of the part lies within the element's shortest stitch of that
point, as finalize measures it, the part is cut at the needle point instead. Each half then ends in a
stitch the machine sews, where finalize would leave a needle point out with a note (`SC-I0504`). Cuts
fall beside needle points most on a zigzag satin, whose cuts lie on its own points or within their pull
compensation, and at a stitch across that lies along the cut, where pull compensation moves the stitch's
end just past the cut's first rail point. Ink/Stitch cuts where the part comes nearest, and its plan drops
a stitch no longer than its shortest stitch. When the cut's 2 points coincide, where the rails meet, a
part is not cut: it is all first, or all second when its start is within 0.1 CSS pixels of the place.

**Starting at the nearest point** (`start_at_nearest_point`). With a needle point before it, the column
starts at the point of the line nearest that needle point. When that point is farther from the needle
than the jump length, and the compensated outline is nearer than the jump length, it starts at the
outline's point nearest the needle instead. The jump length is the element's `min_jump_stitch_length_mm`,
or the design's collapse length when the element sets none or 0. The compensated outline is the 2
polylines through the ends of the top stitches' pairs, after pull compensation and random width, before
short stitches. A zigzag satin's runs through pairs at the full spacing (*Zigzag satin*, step 5). From its
start, the needle enters the line where the start's cut says and follows it to
where the first stitch's cut says, and the column goes on from its first stitch as before.

**Ending at the nearest point** (`end_at_nearest_point`). With an element after it, the column ends at
the point of its compensated outline nearest the next stitch. The next stitch is the point of the
column's rails nearest the next element's first point, or nearest its rails when it offers them. Before
a fill whose area holds the start of the column's first rail, or failing that of its second, the next
stitch is that start, and otherwise the point of the rails nearest the fill's rings. The column's rails
are measured as they are sewn. The column ignores an end less than 5 CSS pixels (1.32 mm)
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

**A point inside an area.** An area is 0 away from every point in it or on its rings, so before it
measures between lines, Ink/Stitch's geometry library looks for such a point. It tests the next
element's points first, then the element's own, each against every part of the other's area before the
next point, and the first that lies in a part is the next stitch. Polylines are tested by their starts,
in order, and an area by its parts' outline starts, never a hole's. So before a fill, the column's next
stitch is the start of its first rail that lies in the fill, or else of its second. These rules were
checked against shapely 2.2.0 as a black box. A fill finds its own next stitch by them too
([fills](fills.md#5-parts)).

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
point of the line within the shortest stitch of the needle, or of the next piece's first point, only
routes it, and is left out. An end within
the shortest stitch of the column's last needle point moves that point to the end. The column's last
stitch, often a top stitch, then goes to the end in one stitch. Ink/Stitch sews the end as a stitch of
its own, which its stitch plan drops when it is no longer than its shortest stitch. An explicit start or
end command, read from M8, would replace the needle point or the next stitch.

## Variants (P2, M7)

| `satin_method` | Look | Design notes |
|---|---|---|
| `e_stitch` | A spine of running stitches along one rail with regular spikes to the other: the "E" or blanket stitch used for appliqué edges | Sewn from M7.3 ([E-stitch](#e-stitch-method-e_stitch)) |
| `s_stitch` | Stitches across the column, every other one back the other way, joined by steps along the rails | Sewn from M7.4 ([S-stitch](#s-stitch-method-s_stitch)) |
| `zigzag` | An open zigzag between the rails | Sewn from M7.5 ([Zigzag satin](#zigzag-satin-method-zigzag)) |

## Properties (conformance)

| Requirement | Property |
|---|---|
| `REQ-SAT-001` | Pull compensation moves both ends of every top stitch outward along it, by `pull_compensation_mm` plus `pull_compensation_percent` of its width (± 0.01 mm). Ends moved past each other meet |
| `REQ-SAT-002` | Consecutive pairs are the zigzag spacing apart, measured across the previous pair at its farther end, exactly so between straight parallel rails. A pair more than 5 % off moves at most twice, which brings a smooth curve's pairs within 5 %. Round a sharp corner a pair can end further off. The last pair is at most a spacing on |
| `REQ-SAT-003` | With a longest stitch, a longer top stitch is split as `split_method` says. By default it splits into the fewest equal parts, and the stitch back into as many as the stitch across before. Simple and staggered splits fall at whole multiples of the longest stitch, from a start that `staggered` moves along |
| `REQ-SAT-004` | Underlays come before the top stitches and leave them as they were: the centre walk, then the contour and the zigzag. Straight stitches of one length join them, no longer than the running stitch's length raised to twice the shortest stitch. A part that starts within the shortest stitch of the needle takes the place of an underlay's last point, and the needle's way gives way to it. Beside a top stitch's point the part's points give way instead. After an odd centre walk the rest runs from the column's end |
| `REQ-SAT-005` | Rails and rungs are told apart as Ink/Stitch tells them, in any drawing order. A path with no subpath longer than a point produces `SC-E0201` and no stitches, and a rung that misses a rail joins the rail's nearest point (`SC-W0203`), the one shapely gives of points as near, once push compensation has moved the rail's ends. A column with no section to sew produces `SC-E0216` and no stitches |
| `REQ-SAT-006` | A parameter with a value for each side changes only its own side: a rail's stitch ends, or the column's start or end |
| `REQ-SAT-007` | Random widths and spacing are reproducible from the element and its seed, and stay within their ranges |
| `REQ-SAT-008` | Push compensation shortens the rails at the column's start and end before they are cut, or lengthens them where negative. A rung that misses a rail cuts it where the rail so compensated comes nearest the rung. A rail it would leave shorter than half a CSS pixel keeps its length (`SC-W0211`) |
| `REQ-SAT-009` | On each rail, a top stitch end closer than `short_stitch_distance_mm` to the last end left in place moves in along its stitch by the next `short_stitch_inset` percentage. Ends far enough from it stay |
| `REQ-SAT-010` | The centre walk follows the line at its position between the rails within its tolerance, in stitches no longer than its length, there and back its repeats |
| `REQ-SAT-011` | The contour runs along each rail at its insets, towards the end on the first rail's side and back on the second's. It stops short of the column's start and end by the rails' insets in millimetres |
| `REQ-SAT-012` | The zigzag goes through one end of each pair, the rails taking turns, and back through the others. Its insets are half the contour's when left empty, and its long stitches split into equal parts |
| `REQ-SAT-013` | Starting at its nearest point, a column begins on the line between its rails nearest the needle, or on its outline when only that is within the jump length, and follows the line to its first stitch |
| `REQ-SAT-014` | Ending at its nearest point, a column ends on its outline nearest the next stitch. Every part is cut in 2 there, at its own needle point when one lies within the shortest stitch of the cut. The column sews the first halves, then the second halves from the line's end, and stitches the end last |
| `REQ-SAT-015` | A column drawn as one subpath whose stroke is no wider than the design's narrowest satin stroke sews as a stroke would, with `SC-W0212`. It offers its first point, as a stroke does |
| `REQ-SAT-018` | A column whose needle points all lie within its shortest stitch of its first sews nothing, with `SC-W0215`, and the rest of the design is planned. A zigzag satin with no stitch across sews nothing either, not even its underlays |
| `REQ-SAT-019` | Of 2 rings, the second starts at its node nearest the first's first node, its nodes too. The stitches go straight across whatever node each ring was drawn from and however it is closed, and rings drawn alike keep their starts (`DEV-SAT-009`) |
| `REQ-SAT-020` | An E-stitch sews each of a satin column's pairs as a spike: from the first rail's inset point across to the second's and back through the same splits. The stitch across splits as a top stitch across does, by default into the fewest equal parts |
| `REQ-SAT-021` | Between spikes an E-stitch goes along the first rail in one stitch. Where the zigzag spacing is longer than the longest stitch, that stitch splits with no randomness: into the fewest equal parts by default, at whole multiples of the longest stitch by the simple and staggered methods |
| `REQ-SAT-022` | Each stretch of pairs in a row whose stitches lean more than 45° from square to the column, to a tenth of a degree, gets one `SC-W0208` at the middle of its middle pair, whatever the satin method. The column runs halfway between its 2 rails. Stitches square to a rail, and stitches at 45°, give none |
| `REQ-SAT-023` | A satin column whose longest top stitch, as sewn, is longer than 12 mm gets one `SC-W0209` at that stitch's middle, whatever the satin method. A longest stitch that splits them, or an end that cuts one in 2, leaves none |
| `REQ-SAT-024` | An S-stitch sews each of a satin column's pairs once across, the first and every other one from the second rail, the rest from the first. A stitch splits as one from the first rail to the second would. The default method makes the fewest equal parts, and the simple and staggered methods number the stitch as its pair is, from 0 |
| `REQ-SAT-025` | An S-stitch steps along a rail from each pair's inset point to the next pair's in one stitch, which it never splits |
| `REQ-SAT-026` | A zigzag satin places a satin column's pairs at half the zigzag spacing and sews one point of each, the rails alternating from the first rail. Each 2 points in a row are a pair across, and a point left over is left out. The pairs are inset along their own stitches and sewn across and back as a satin column's. Its outline runs through pairs at the full spacing |
| `REQ-SAT-027` | A zigzag satin's stitches split by the column's split method. The simple and staggered methods number the stitch back to pair i 2i and the stitch across it 2i + 1. By default each splits into its own fewest equal parts, the stitch back taking no count from the stitch across before it |
| `REQ-SAT-028` | Under a zigzag satin, the contour's first side stops at the zigzag's last point on the first rail, and its second side at its first and last points on the second rail, before the insets. An odd centre walk moves none of these places |

Machine checkpoint MC-3 sews a width ladder (1–10 mm) and an underlay comparison to tune defaults
([machine testing](../../plan/machine-testing.md)).

## Diagnostics

`SC-E0201`, `SC-W0202`, `SC-W0203`, `SC-W0205`, `SC-W0206` (a contour underlay too short for its insets
keeps its length), `SC-W0207`, `SC-W0208` (stitches leaning more than 45° from square to the column),
`SC-W0209` (a top stitch longer than 12 mm), `SC-W0210`, `SC-W0211`, `SC-W0212`, `SC-W0213`, `SC-E0214`,
`SC-W0215` (a column too small to sew) and `SC-E0216` (a column without a section to sew). The running
stitch's `SC-W0402` names a travel length raised to twice the shortest stitch.
