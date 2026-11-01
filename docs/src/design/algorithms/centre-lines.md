# Centre lines

<!-- implements: crates/stitchcraft-engine/src/normalize/triangulation/**, crates/stitchcraft-engine/src/normalize/strips/** -->

A narrow part of a shape sews along its middle. Automatic digitizing sews a part narrower than the
narrowest satin column as a running stitch along it (M13.4), and from M13.5 a wider one as satin columns
between its own outlines. Which parts are narrow, and how they sew, is on the
[image to embroidery](../image-to-embroidery.md#stitch-types-by-width) page. This page says how a part's
centre line is found (`REQ-AUTO-014`).

The part is cut into triangles by a constrained Delaunay triangulation: every edge of its outline and holes
is an edge of a triangle, and no triangle's circle holds a corner that the triangle can see. The edges that
2 triangles share cross the part, and their middles follow its middle (`stitchcraft_engine::normalize`'s
`triangulation` and `strips`).

## Points along the edges

A shared edge crosses the part only where its triangles are about as long as the part is wide, so long
edges, such as the sides of a straight line, are cut first. Each edge is cut into the fewest pieces, all as
long as each other, that are no longer than the part is wide on average: twice its area over its
perimeter, and never less than 0.1 mm, which bounds the points by the perimeter.

Every point goes on the grid that a fill's region is built on, 2^-35 mm ([fills](fills.md)). The region's
own points lie on it already. On the grid every difference of 2 coordinates is an exact float. The exact
predicates below then take one product for each term of their sums.

## The triangulation

1. **Points.** A box well round the part, its corners on whole millimetres, is cut into 2 triangles, and
   the points go in one at a time, ring by ring. A point inside a triangle splits it into 3,
   and a point on an edge splits the 2 triangles beside the edge into 4. A point already in the mesh, such
   as a corner where a hole touches the outline, is that point. The edges round a new point are then
   flipped until each passes the Delaunay test: the corner across the edge lies outside the circle through
   the triangle's corners (Lawson 1977). The triangle that a point lies in is found by walking towards it
   from the triangle where the last point was found, a step or 2 in ring order.
2. **Edges of the part.** An edge of the part that no triangle has yet is made by flipping the edges it
   crosses, until none does (Sloan 1993). An edge whose 2 triangles make a quadrilateral that isn't convex
   waits its turn again. A point on the edge between its ends cuts it into 2 edges. The part's edges are never
   flipped after that.
3. **Inside.** The triangles round the box's corners lie outside the part, and crossing an edge of the part
   goes in or out. The triangles inside it are then found by walking out from the box.
4. **Delaunay again.** The edges between triangles inside the part are flipped until each passes the test.
   Where 4 or more points lie on one circle with the other points outside it, as a square's corners do, more
   than one cut of them into triangles passes the test, and a tie-break chooses one. Each point's lift, its
   squared distance from the origin, counts as raised by a vanishing amount of its own, the least point's by x
   and then y the most (a symbolic perturbation, after Edelsbrunner and Mücke 1990). No 4 points then lie on
   one circle, and there is one constrained Delaunay triangulation, which flipping reaches from any
   triangulation of a polygon. The triangles are then the same in whatever order the points went in.

Each question of where a point lies, on which side of a line or inside which circle, is decided exactly
(`stitchcraft_core::exact`, after Shewchuk 1997). Flipping then stops and every walk ends, and a part gives
the same triangles on every platform. Each step costs a unit of work. Were 2 edges of the part to cross,
the mesh would lose its shape, which is a bug. Automatic digitizing then fills the part and says so with
`SC-E0009`.

## Strips, junctions and ends

Where 2 triangles inside the part share an edge, the edge is a chord. A triangle is part of a strip when it
has 2 chords, a junction, where 3 strips meet, when it has 3, and an end when it has 1.

- **Ends.** The circle on a chord is the one whose diameter the chord is, its edge included. A round end
  lies within the circle on its chord, and so does a square one: such an end is a cap, which the line
  doesn't enter. Each cap grows across the chords while the circle on each contains every corner of the
  cap. It grows through each strip triangle, and through a junction once caps reach 2 of its sides, across
  its third chord, the 2 caps growing on as one. The line ends at the middle of the last chord a cap grew
  across, or at the middle of a junction whose other sides are caps. Where an end's chord doesn't contain
  the end's own corner, as at a sharp point, the end is a tip, and the line runs on to the corner.
  Igarashi, Matsuoka and Tanaka's *Teddy* (1999) trims the ends of a shape's spine the same way.
- **Round ends.** The points round a round end lie on a circle, or nearly, and any triangles between them
  pass the Delaunay test. Many of those triangles are junctions, and the caps of the end's corners come to
  2 sides of each. Were a cap to stop at a junction, the line would bend through the junctions to the
  edge. The caps grow through them instead, to where the end meets the rest of the part, and the line
  ends there, on the part's middle.
- **The line.** The line joins the middles of the chords outside the caps. It crosses each strip triangle
  from chord to chord, and runs to each of a junction's chords outside the caps from the junction's middle,
  the average of its corners.
- **Bumps.** A traced outline has small bumps, and a sharp turn has a corner beyond where the middles of its
  strips meet. Each gives a junction and a short branch to an end. A branch from a junction to a tip that
  is no longer than twice the junction triangle's longest edge is left out, and so is a branch to a round
  or a square end that reaches no further than that edge, the shortest first, until no junction has one. A
  junction left with 2 branches is a bend of the line. A sharp turn's corner is a tip, and reaches further
  from where the middles meet than a bump of the outline does. A branch to a round or a square end that
  reaches further than the junction is wide is an arm. Each arm of the bar of a T 4 mm tall reaches 1.6 mm,
  about twice the bar's width. Taken for a bump, one arm was left out and the other became a bend, so the T
  sewed as a Γ. Ink/Stitch's Fill to Stroke, run on the same letters with its threshold at the stroke's
  width, keeps both arms. A branch reaches along its line and on through its end's cap, to the cap's
  farthest corner: the cap is part of the arm. Measured only to where its line ends, the arm of a T 4.8 mm
  tall in lines 0.8 mm wide reached 1.01 mm, under the 1.06 mm of its junction triangle's longest edge,
  and the T lost it. Through its cap it reaches 1.55 mm. A bump's cap is as tall as the bump, and a bump
  as tall as its part is wide reaches as far as its junction is wide: on a strip 0.6 mm wide, a round bump
  up to 0.55 mm tall is still a bump.
- **Junctions.** At each junction left, the 2 branches that run on most nearly straight are one line, and
  the third ends there. A branch's direction runs from the junction's middle to the first of its points
  that lies at least as far away as the junction triangle's longest edge, or to its end if none does. The 2
  whose directions are most nearly opposite run on.
- **Rings.** A part that is a ring of strips, such as a letter O, has a closed line.
- **Each piece once.** A line that would run on along a piece of the centre line that another line has
  sewn ends there instead, so no piece is sewn twice. At the waist of a figure 8, each junction may pair
  the waist with a different loop. The line round one loop then runs on through the waist and closes, and
  the line round the other loop ends where it comes back to the waist.
- **Order.** The lines sew one after another, the longest first. The next one is the line with an end
  nearest where the last one ended, sewn from that end. A closed line starts at its point nearest.

Dots have no line, since the caps of their ends meet.

## Width along the line

A part's width at a point of its centre line is the diameter of the largest circle centred there that fits
inside the part: twice the distance from the point to the nearest edge of its outline and holes. Automatic
digitizing averages it along the line, and a part from 0.5 mm wide is a triple run. A line that ends at a
tip runs on into the corner, where the circle shrinks to a point, for its stitches to cover the corner. The
piece that leads into the corner is left out of the average, unless the part's lines have no other. With
it, a leg of a letter A 1.08 mm wide measured under 1 mm and sewed as a line, beside the other leg's satin
column, whose line took in the junction at the top. The edges are put in the cells of a grid as fine as
the pieces the part was cut into. A point's nearest edge is looked for in the cells round it, ring by
ring, until no edge farther out could be nearer. A narrow part's edges lie a cell or so from its middle.
Each point then costs a few units of work.

## Columns

A part no wider than a satin column sews a satin column along each of its lines, over the triangles round
the line and between the part's own outlines (`REQ-AUTO-016` and `REQ-AUTO-017`, in `strips::columns`). Its
lines are found as above, but for these steps:

- **Bumps.** A column beside a branch would cover it with stitches out along the branch: a stem that took
  the arms of a t's crossbar for bumps would sew the crossbar with stitches from one end of it to the
  other. So a branch is a bump only when it ends at a tip and at its junction too. It ends at its junction
  when the junction's other 2 branches run on most nearly straight, as beside the second sharp corner of a
  wide square end. A branch that ends in a cap is an arm with an end of its own. A branch to a tip that
  another branch runs on into is that line's end: the leg of an A runs on below its crossbar into its
  foot's sharp corner.
- **Corners.** A line that turns by more than 60° at a junction's middle, each way measured as a branch's
  is, is cut there: a column that turned so would sew a fan of stitches into the inside of the corner. Of
  2 such corners within twice the larger junction triangle's longest edge of each other, as where a V ends
  flat, the line is cut only at the sharper, and a ring is cut only at 2 or more: cut once, it would still
  go round its hole.
- **Lassos.** A line that comes back to a junction it ran through, as the line of a b runs down its stem,
  round its bowl and back, is cut there into the line up to the junction and a ring.
- **Triangles.** A triangle goes to the line with the most pieces in it, the first such line on a tie, so
  a junction that a line runs on through goes to that line. The caps and bumps that no line crosses go to
  the line of the triangle beside them, nearest first. The edges of a line's triangles that no other of
  them shares are its column's outline, one ring round an open line's and 2 round a closed one's. A line
  with any other outline doesn't make a column.
- **Ends.** A line that ends at a junction another line owns, or at a chord with another line's triangle
  beyond it, ends at that chord. Its column ends under the other column, which sews after it. Otherwise
  the column's own end lies beyond the last chord the line crosses, with any bend the line runs through on
  the way there, as far as the line's middle. Such a bend is a junction whose third side is a cap or a
  bump, as beside the blunt corner of a slanted end. The end is the column's farthest point along the line's way at that chord, measured from the line's
  point before it. The last piece of a line runs to a tip or to a junction's middle, off the column's way.
  A way measured on it would end a slanted or round end at a corner. An edge of the outline there that lies
  within 20° of square to that way, and is no longer than twice the chord the end begins at, is the end.
  The column ends flat along all of it, as far as it runs straight. A cap can stop at a chord from a point
  between an edge's pieces. A column that ended at that point would leave the other piece on a rail, with
  its stitches leaning towards it. A longer edge runs along the column, as its side does where its line
  turns into a tip. An edge along a cut, where a narrow part was cut from a broad one
  ([image to embroidery](../image-to-embroidery.md)), is the end however long it is: across a tapering
  point, the cut is wider than twice the chord the end begins at. Otherwise the column ends at that point,
  its rails meeting. A column can be only its
  end, as a stub beyond a junction is. The chord its end begins at is then on its outline, and its end goes
  round the other way.
- **Rails.** The rails are the outline on either side between the ends, both from the line's start. A
  ring's are its outer and inner outline, the same way round, started at a rung whose point on the inner
  one is the inner one's point nearest its point on the outer, so that a satin column starts them together
  (`DEV-SAT-009`).
- **Rungs.** A triangle with one edge on a rail and its third corner on the other gives a rung from that
  corner, at a right angle to the column's way there. The rung ends where it meets the edge, or at the
  edge's nearer end if it misses it. The column's way is the mean of its 2 rails' ways, the corner's rail's
  at the corner and the other's along the edge. It runs along the middle of a band or round a ring, and
  between the sides where a column tapers or ends at a corner. A corner keeps its rung nearest that right
  angle. Where rungs crowd into the inside of a tight curve, their points closer on a rail than twice how
  far a rung is drawn past each rail, the one nearer a right angle is kept. A rung at a rail's end is left
  out: the rails' ends pair up.
- **Drawn.** The column is drawn with its rails and a line across them for each rung, as the Stroke to
  Satin filter draws one (`REQ-VC-014`), and read back as any satin column is, it must give the same rails
  and pair the same points. A part one of whose columns doesn't stays in the area's fill, with `SC-I0902`.

## Sources

- C. L. Lawson, *Software for C¹ surface interpolation*, in *Mathematical Software III* (1977), for
  flipping edges until each passes the Delaunay test.
- S. W. Sloan, *A fast algorithm for generating constrained Delaunay triangulations*, Computers &
  Structures 47 (1993), for making a missing edge by flipping the edges it crosses.
- J. R. Shewchuk, *Adaptive Precision Floating-Point Arithmetic and Fast Robust Geometric Predicates*
  (1997), for the exact predicates.
- H. Edelsbrunner and E. P. Mücke, *Simulation of Simplicity*, ACM Transactions on Graphics 9 (1990), for
  breaking ties between points on one circle by perturbing them symbolically.
- T. Igarashi, S. Matsuoka and H. Tanaka's *Teddy*, a sketching interface for 3D freeform design
  (SIGGRAPH 1999), for the circles that trim a spine's ends.
