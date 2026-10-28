# Centre lines

<!-- implements: crates/stitchcraft-engine/src/normalize/triangulation/** -->

A narrow part of a shape sews along its middle. Automatic digitizing sews a part narrower than the
narrowest satin column as a running stitch along it (M13.4), and from M13.5 a wider one as satin columns
between its own outlines. Which parts are narrow, and how they sew, is on the
[image to embroidery](../image-to-embroidery.md#stitch-types-by-width) page. This page says how a part's
centre line is found. M13.4 comes in 2 parts: the first cuts the part into triangles, and the second finds
the line through them and sews it.

The part is cut into triangles by a constrained Delaunay triangulation: every edge of its outline and holes
is an edge of a triangle, and no triangle's circle holds a corner that the triangle can see. The edges that
2 triangles share cross the part, and their middles follow its middle
(`stitchcraft_engine::normalize::triangulation`).

## Points along the edges

A shared edge crosses the part only where its triangles are about as long as the part is wide, so long
edges, such as the sides of a straight line, are cut first. Each edge is cut into the fewest pieces, all as
long as each other, that are no longer than a length the caller gives. The centre lines give the part's
width on average, twice its area over its perimeter, and never less than 0.1 mm, which bounds the points by
the perimeter.

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
the mesh would lose its shape, which is a bug that the triangulation reports to its caller.

## Sources

- C. L. Lawson, *Software for C¹ surface interpolation*, in *Mathematical Software III* (1977), for
  flipping edges until each passes the Delaunay test.
- S. W. Sloan, *A fast algorithm for generating constrained Delaunay triangulations*, Computers &
  Structures 47 (1993), for making a missing edge by flipping the edges it crosses.
- J. R. Shewchuk, *Adaptive Precision Floating-Point Arithmetic and Fast Robust Geometric Predicates*
  (1997), for the exact predicates.
- H. Edelsbrunner and E. P. Mücke, *Simulation of Simplicity*, ACM Transactions on Graphics 9 (1990), for
  breaking ties between points on one circle by perturbing them symbolically.
