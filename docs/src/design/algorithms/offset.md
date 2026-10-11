# Offset curves

<!-- implements: crates/stitchcraft-engine/src/normalize/offset/** -->

A satin column drawn as one path gets its rails by offsetting its centre line by half the stroke's width
to each side ([single-path satin](satin.md#single-path-satin)). Ink/Stitch makes them with shapely's
`LineString.offset_curve`, which the GEOS library computes, and the same rails sew the same column. So
StitchCraft builds the curves that function returns, point for point (`REQ-SAT-016`), with the join
Ink/Stitch passes and 16 segments to a quarter circle, shapely's default.

## The curve and its side

An offset curve runs beside a line at a fixed distance, on the line's left in y-up axes for a positive
distance, which is its right on screen, and on the other side for a negative one. It runs the same way
as the line. It is the part of the edge of the line's stroke, the area within the distance of the line,
that lies along that side. The answer is one curve, or several where the edge splits that side into
pieces, or none where the stroke covers the whole side.

A line of fewer than 2 distinct points, or of no length, has no offset, and a line of 2 points is that
segment moved sideways. At no distance the offset is the line as it is. Anything longer goes through 3
steps.

## 1. The raw curve

Each segment of the line is moved sideways by the distance, and consecutive moved segments are joined
at the node between them:

- **On the outside of a turn**, the join decides. A mitre extends the 2 moved segments to the point
  where they meet, unless that point lies farther from the node than the mitre limit times the
  distance. Then it is cut square across the turn's bisector at the limit, or bevelled when even the
  bevel lies beyond it. A round join is an arc about the node in equal steps of about a 64th of a
  full turn, and a bevel joins the moved ends straight, or takes them as one point when they are less
  than a thousandth of the distance apart.
- **On the inside of a turn**, the curve goes to the point where the moved segments cross, or, if they
  miss each other, in towards the node and back out along 2 short closing segments that the stroke
  covers.
- **A turn back** along the same line is a half circle with a round join, and a step across otherwise.

Before the segments are moved, the line is simplified on the side the curve lies on. A node where the
line turns towards that side, nearer than a hundredth of the distance to the node kept before it, is
dropped: there the moved segments would cross and leave a small loop. GEOS also measures the node against
the straight line past it, and against the lines to the points between, but a node that near the one
before always passes. The first and last segments always stay. Points of the curve less than a ten-thousandth of the distance from
the one before are left out.

## 2. The edge of the stroke

The raw curves of both sides, the left forwards and the right backwards, joined by round caps around
the line's ends, make one ring around the line. Where the line turns or comes back near itself, the
ring crosses itself. The stroke covers the points the ring winds round at least once, counted
clockwise.

The ring is cut at each point where it crosses itself, and at each end of a segment that lies on
another segment, within a billionth of the half width. Each piece is judged by the winding just beside
its middle, on each side. A piece with the covered area on one side only is part of the edge, turned to
keep that area on its right, and pieces that coincide are one edge. The winding is counted with an exact
orientation test ([determinism](../determinism.md)), so a point that lies on a segment is never on one
side on one computer and on the other side on another.

The edges join into rings. Where a ring touches itself, it takes the edge that turns farthest to the
right, and so stays as small as it can be, as the rings of GEOS's polygons are. The rings that run
clockwise are shells, and the others are holes. A ring of no area bounds nothing and is left out. The
offset is looked for on the shell of the largest area and on the holes inside it.

## 3. The sections

The pieces of the edge that lie on the raw curve of the offset's side, both ends within a ten-thousandth
of the distance of one of its segments, are the offset. A piece placed along several segments takes the
place of the last. Consecutive pieces along a ring join into sections, the first starting at the piece
placed first when it was placed, and the sections are ordered by where along the raw curve each starts.
One section is the curve, and more mean the offset splits.

Inside a tight bend the raw curve loops back on itself, inside the stroke: the edge skips the loop, and
so does the offset. Where the line comes back near itself, the stroke covers part of the raw curve,
which splits the offset there.

## Checking against shapely

`conformance/oracle/offset.py` asks shapely 2.2.0, with the GEOS 3.14.1 its wheels bundle, for the
offset curves of 1,050 random lines: gentle and wild walks, arcs, spirals, waves and zigzags, walks on a
grid of whole numbers, whose points often lie in line and whose segments run back along others, and
walks of tiny steps, which are simplified before they are offset. Each is offset at one of 6 distances
to either side, with one of the joins and 5 mitre limits. Shapely's answers are the fixture
`conformance/fixtures/geometry/shapely-offset.txt`, and the engine's unit tests check every curve point
within a billionth of a millimetre, a point repeated a hair apart counted once. Shapely runs as a black
box, through its public API.

5 grid walks differ, all lines that run back exactly along themselves or whose stroke's edge touches
itself exactly. There the pieces of the edge coincide, and whether GEOS takes 2 of them to meet turns on
the rounding of its own arithmetic. The engine merges points nearer each other than a billionth of the
half width, and judges pieces that coincide as one. Its curves differ there by a node on a straight
stretch, or by a short stretch beside the doubled-back part of the line. The test names those 5 lines,
so that a change to them is seen.

A closed line, one that ends where it starts, is buffered by GEOS as a ring, with no caps. The engine
offsets only open lines. A satin column's centre line with its ends together is cut in half first.

Building the edge costs work in proportion to the square of the ring's length: every piece is checked
against every other, and its winding against every segment of the ring. A line of 1,000 points takes up
to about 17 million units of work for each offset.
