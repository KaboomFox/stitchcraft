"""Record how shapely cuts rows with polygons, for the rows of a tatami fill (REQ-FILL-TAT-010).

Ink/Stitch lays a fill's rows as lines across its shape and keeps the pieces of each line that its geometry
library, shapely, finds in the shape. Those pieces decide where rows start and end: a row along the
outline is a piece, a row the outline touches from inside is cut there, and a row that only touches the
shape at a point has none. This script draws random polygons from grid squares and half squares, where
rows often run along edges and through corners, and blocks of the grid with such cells taken out of their
inside, which leaves holes. It cuts horizontal rows with them and prints shapely's pieces; the tests of
`crates/stitchcraft-engine/src/generators/tatami/rows.rs` check that the engine finds the same. shapely
runs as a black box: only its public API is used and only its answers are kept.

Regenerate the fixture (shapely 2.2.0's wheels bundle GEOS 3.14.1):

    python3 -m venv target/geometry-venv
    target/geometry-venv/bin/pip install shapely==2.2.0
    target/geometry-venv/bin/python -I conformance/oracle/rows.py \\
        > conformance/fixtures/geometry/shapely-rows.txt

Each case is one line, whitespace apart:

    SPACING  RINGS  ROWS

RINGS is a count of closed rings, the outline first and then the holes, each a count of points and their
coordinates. ROWS is a count of rows, each its y, a count of pieces and each piece's 2 x coordinates,
smaller first, the pieces in order of x. Rows lie at whole multiples of SPACING, from the largest at or
below the polygon's least y, while below its greatest y; rows without a piece are not listed.
"""

import math
import random

import shapely
import shapely.ops

SEED = 20261011
# (grid size, cells drawn, cases).
GRIDS = [(5, 6, 150), (7, 12, 150)]
# (block size, cells taken out of its inside, cases), drawn after GRIDS so that their cases stay as they were.
HOLED = [(6, 4, 100)]
SPACINGS = [0.5, 1.0]


def cell(rng, grid, low=0):
    """A random grid square from `low` to `grid` on each axis, or one of its 4 halves cut along a diagonal."""
    x, y = rng.randrange(low, grid), rng.randrange(low, grid)
    corners = [(x, y), (x + 1, y), (x + 1, y + 1), (x, y + 1)]
    kind = rng.randrange(5)
    if kind == 4:
        return shapely.Polygon(corners)
    return shapely.Polygon([corners[(kind + k) % 4] for k in range(3)])


def parts(shape):
    """The polygons `shape` falls into."""
    return [g for g in getattr(shape, "geoms", [shape]) if g.geom_type == "Polygon" and g.area > 0]


def polygons(rng, grid, cells):
    """The polygons the union of random cells falls into."""
    return parts(shapely.ops.unary_union([cell(rng, grid) for _ in range(cells)]))


def holed(rng, block, cells):
    """The polygons a block of the grid falls into with random cells taken out of its inside, away from its
    edge: the cells left out are holes, or split the block where they meet."""
    taken = shapely.ops.unary_union([cell(rng, block - 1, low=1) for _ in range(cells)])
    return parts(shapely.box(0, 0, block, block).difference(taken))


def number(value):
    return repr(float(value))


def ring_words(ring):
    coords = list(ring.coords)
    return [str(len(coords))] + [number(c) for point in coords for c in point]


def pieces(polygon, y):
    """The pieces of the row at y in the polygon, as (x1, x2) pairs, smaller first, in order of x."""
    minx, _, maxx, _ = polygon.bounds
    found = polygon.intersection(shapely.LineString([(minx - 1, y), (maxx + 1, y)]))
    lines = [g for g in getattr(found, "geoms", [found]) if g.geom_type == "LineString" and not g.is_empty]
    out = []
    for line in lines:
        xs = [c[0] for c in line.coords]
        out.append((min(xs[0], xs[-1]), max(xs[0], xs[-1])))
    return sorted(out)


def case(polygon, spacing):
    _, miny, _, maxy = polygon.bounds
    rows = []
    y = math.floor(miny / spacing) * spacing
    while y < maxy:
        found = pieces(polygon, y)
        if found:
            rows.append((y, found))
        y += spacing
    words = [number(spacing), str(1 + len(polygon.interiors))]
    for ring in [polygon.exterior, *polygon.interiors]:
        words += ring_words(ring)
    words.append(str(len(rows)))
    for y, found in rows:
        words += [number(y), str(len(found))] + [number(v) for piece in found for v in piece]
    return " ".join(words)


def main():
    rng = random.Random(SEED)
    print(f"# Rows cut by shapely {shapely.__version__} (GEOS {shapely.geos_version_string}); "
          "conformance/oracle/rows.py, seed {SEED}.".replace("{SEED}", str(SEED)))
    for draw, sizes in [(polygons, GRIDS), (holed, HOLED)]:
        for size, cells, count in sizes:
            made = 0
            while made < count:
                for polygon in draw(rng, size, cells):
                    for spacing in SPACINGS:
                        print(case(polygon, spacing))
                    made += 1


if __name__ == "__main__":
    main()
