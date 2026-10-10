"""Record where shapely finds nearest points, for the engine's nearest-point helpers (REQ-SAT-014).

A satin column starts and ends at nearest points between shapes, and shapes are often equally near at
several points: rails side by side, a point midway between 2 rails, a stitch along a cut. Ink/Stitch takes
the one its geometry library, shapely, finds first. This script asks shapely about random shapes on coarse
grids, where ties are common, and prints its answers; the tests of
`crates/stitchcraft-engine/src/normalize/near.rs` check that the engine takes the same points. shapely runs
as a black box, as decode.py runs pyembroidery: only its public API is used and only its answers are kept.

Regenerate the fixture (shapely 2.2.0's wheels bundle GEOS 3.14.1):

    python3 -m venv target/geometry-venv
    target/geometry-venv/bin/pip install shapely==2.2.0
    target/geometry-venv/bin/python -I conformance/oracle/nearest.py \
        > conformance/fixtures/geometry/shapely-nearest.txt

Each case is a kind and then numbers, whitespace apart. SHAPES is a count of polylines, each a count of
points and then their coordinates; LINE is a count of points and their coordinates.

    pt  SHAPES x y          ex ey   the point of SHAPES nearest the point (x, y)
    ll  SHAPES OTHER        ex ey   the point of SHAPES nearest the shapes OTHER, measured from OTHER
    ap  LINE cx cy dx dy    along   how far along LINE its point nearest the segment lies, measured from LINE
    as  LINE cx cy dx dy    along   the same, measured from the segment
"""

import random

import shapely
import shapely.ops

SEED = 20261010
# (grid size, most points in a polyline, cases): a fine grid with longer polylines, and a coarse one with
# shorter ones, where more of them touch and overlap.
GRIDS = [(6, 4, 200), (3, 3, 200)]


def polyline(rng, grid, most):
    """A polyline of 2 to `most` points on the grid, not all one point."""
    points = [(rng.randint(0, grid), rng.randint(0, grid)) for _ in range(rng.randint(2, most))]
    if all(point == points[0] for point in points):
        points[-1] = (points[0][0] + 1, points[0][1])
    return points


def shapes(rng, grid, most):
    return [polyline(rng, grid, most) for _ in range(rng.randint(1, 3))]


def numbers(*values):
    return " ".join(repr(value) if isinstance(value, float) else str(value) for value in values)


def line(points):
    return numbers(len(points), *(c for point in points for c in point))


def many(lines):
    return " ".join([str(len(lines))] + [line(points) for points in lines])


def cases(rng, grid, most):
    ours = shapes(rng, grid, most)
    p = (rng.randint(-1, grid + 1), rng.randint(-1, grid + 1))
    on = shapely.ops.nearest_points(shapely.Point(p), shapely.MultiLineString(ours))[1]
    yield f"pt {many(ours)} {numbers(*p)} {numbers(on.x, on.y)}"
    theirs = shapes(rng, grid, most)
    on = shapely.ops.nearest_points(shapely.MultiLineString(theirs), shapely.MultiLineString(ours))[1]
    yield f"ll {many(ours)} {many(theirs)} {numbers(on.x, on.y)}"
    part = polyline(rng, grid, most)
    c = (rng.randint(0, grid), rng.randint(0, grid))
    d = (rng.randint(0, grid), rng.randint(0, grid))
    if c == d:
        d = (c[0], c[1] + 1)
    walk, cut = shapely.LineString(part), shapely.LineString([c, d])
    along = walk.project(shapely.ops.nearest_points(walk, cut)[0])
    yield f"ap {line(part)} {numbers(*c, *d)} {numbers(along)}"
    along = walk.project(shapely.ops.nearest_points(cut, walk)[1])
    yield f"as {line(part)} {numbers(*c, *d)} {numbers(along)}"


def main():
    rng = random.Random(SEED)
    print(f"# Written by conformance/oracle/nearest.py with shapely {shapely.__version__} "
          f"(GEOS {shapely.geos_version_string}). Do not edit; regenerate.")
    for grid, most, count in GRIDS:
        for _ in range(count):
            for case in cases(rng, grid, most):
                print(case)


if __name__ == "__main__":
    main()
