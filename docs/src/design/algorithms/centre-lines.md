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
  strips meet. Each gives a junction and a short branch to an end. A branch from a junction to an end that
  is no longer than twice the junction triangle's longest edge is left out, the shortest first, until no
  junction has one. A junction left with 2 branches is a bend of the line.
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
digitizing averages it along the line, and a part from 0.5 mm wide is a triple run. The edges are put in the
cells of a grid as fine as the pieces the part was cut into. A point's nearest edge is looked for in the
cells round it, ring by ring, until no edge farther out could be nearer. A narrow part's edges lie a cell or
so from its middle. Each point then costs a few units of work.

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
