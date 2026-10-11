"""Record shapely's buffers of polygons, for a fill's inset underlay and its expanded top rows
(REQ-FILL-TAT-012).

Ink/Stitch sews a fill's underlay on each part shrunk by `fill_underlay_inset_mm`, and its top rows on the
part grown by `expand_mm`, both with shapely's `buffer`, which is GEOS's: round corners, 16 segments to a
quarter circle. This script records shapely's answers for random polygons: stars that turn both ways,
blocks of grid squares and half squares, whose points often lie in line and which have holes, flattened
rings with dense points, dumbbells whose neck a shrinking cuts, and squares with holes near each other and
the edge, which a growing merges or fills. Each is buffered at several distances, shrinking and growing.
Then come small regular polygons shrunk just past nothing, which GEOS's rules for curves turned inside out
decide. Last come the edges of its rules for rings that a shrinking wears away: triangles shrunk by about
the radius of their inscribed circle, and regular polygons of 9 to 13 corners shrunk by exactly half their
box's least width, which GEOS still offsets.
The tests of `crates/stitchcraft-engine/src/normalize/offset/` check that the engine builds the same
polygons. shapely runs as a black box: only its public API is used and only its answers are kept.

Regenerate the fixture (shapely 2.2.0's wheels bundle GEOS 3.14.1):

    python3 -m venv target/geometry-venv
    target/geometry-venv/bin/pip install shapely==2.2.0
    target/geometry-venv/bin/python -I conformance/oracle/buffer.py \\
        > conformance/fixtures/geometry/shapely-buffer.txt

Each case is one line of numbers, whitespace apart: the polygon, the distance, and the answer. A polygon is
a count of rings, its outline and then its holes, each a count of points and their coordinates, closed. The
answer is a count of polygons, in shapely's order, each written as the input is. Outlines run clockwise and
holes counter-clockwise in y-up axes, in the input as in the engine's regions, and in the answer as shapely
gives them.
"""

import math
import random

import shapely
import shapely.affinity
import shapely.geometry.polygon
import shapely.ops

SEED = 20261011
DISTANCES = [0.05, 0.1, 0.25, 0.5, 1.0, 2.0]


def oriented(polygon):
    """`polygon` with its outline clockwise and its holes counter-clockwise (y up)."""
    return shapely.geometry.polygon.orient(polygon, sign=-1.0)


def parts(shape):
    """The polygons `shape` falls into, each of some area."""
    return [g for g in getattr(shape, "geoms", [shape]) if g.geom_type == "Polygon" and g.area > 1e-6]


def star(rng):
    """A polygon round a centre, each point at its own distance from it: it turns both ways."""
    n = rng.randint(5, 16)
    angles = sorted(rng.uniform(0, 2 * math.pi) for _ in range(n))
    points = []
    for a in angles:
        r = rng.uniform(2, 10)
        points.append((round(r * math.cos(a), 4), round(r * math.sin(a), 4)))
    return [shapely.Polygon(points)]


def blocks(rng):
    """The polygons a union of random grid squares and half squares falls into, scaled up: points in line,
    corners touching, and holes."""
    cells = []
    for _ in range(rng.randint(4, 14)):
        x, y = rng.randrange(0, 6), rng.randrange(0, 6)
        corners = [(x, y), (x + 1, y), (x + 1, y + 1), (x, y + 1)]
        kind = rng.randrange(5)
        cells.append(shapely.Polygon(corners if kind == 4 else [corners[(kind + k) % 4] for k in range(3)]))
    scale = rng.choice([1.0, 2.0, 3.0])
    return [shapely.affinity.scale(p, scale, scale, origin=(0, 0)) for p in parts(shapely.ops.unary_union(cells))]


def ring(rng):
    """A flattened ellipse with dense points, often with a hole: the simplifying before offsetting matters."""
    n = rng.randint(24, 64)
    a, b = rng.uniform(2, 12), rng.uniform(2, 12)
    outline = [(round(a * math.cos(2 * math.pi * i / n), 6), round(b * math.sin(2 * math.pi * i / n), 6)) for i in range(n)]
    holes = []
    if rng.random() < 0.6:
        k = rng.uniform(0.2, 0.8)
        m = rng.randint(12, 32)
        holes.append([(round(k * a * math.cos(2 * math.pi * i / m), 6), round(k * b * math.sin(2 * math.pi * i / m), 6)) for i in range(m)])
    return [shapely.Polygon(outline, holes)]


def dumbbell(rng):
    """2 boxes joined by a neck, turned at random: a shrinking cuts the neck."""
    w, h, gap, neck = rng.uniform(2, 6), rng.uniform(2, 6), rng.uniform(1, 6), rng.uniform(0.2, 2.0)
    shape = shapely.ops.unary_union([
        shapely.box(0, 0, w, h),
        shapely.box(w + gap, 0, 2 * w + gap, h),
        shapely.box(w / 2, h / 2 - neck / 2, w * 1.5 + gap, h / 2 + neck / 2),
    ])
    shape = shapely.affinity.rotate(shape, rng.uniform(0, 180), origin=(0, 0))
    return [shapely.Polygon([(round(x, 6), round(y, 6)) for x, y in p.exterior.coords]) for p in parts(shape)]


def holed(rng):
    """A square with holes near each other and near its edge: a growing fills them, a shrinking merges them
    or opens them onto the outside."""
    size = rng.uniform(6, 14)
    holes = []
    for _ in range(rng.randint(1, 3)):
        x, y = rng.uniform(0.3, size - 2.3), rng.uniform(0.3, size - 2.3)
        hole = shapely.box(x, y, x + rng.uniform(0.3, 2.0), y + rng.uniform(0.3, 2.0))
        if all(hole.distance(other) > 0.05 for other in holes):
            holes.append(hole)
    polygon = shapely.Polygon(shapely.box(0, 0, size, size).exterior.coords, [h.exterior.coords for h in holes])
    return [shapely.Polygon([(round(x, 6), round(y, 6)) for x, y in polygon.exterior.coords],
                            [[(round(x, 6), round(y, 6)) for x, y in h.coords] for h in polygon.interiors])]


def small(rng):
    """A small regular polygon of 4 to 9 corners, turned at random, and a distance just past its inscribed
    circle's radius: shrunk, its curve turns inside out into a small copy of it, which GEOS keeps or leaves
    out by its rules."""
    n, r, turn = rng.randint(4, 9), rng.uniform(1, 3), rng.uniform(0, 2 * math.pi)
    big = r / math.cos(math.pi / n)
    points = [(round(big * math.cos(turn + 2 * math.pi * i / n), 6), round(big * math.sin(turn + 2 * math.pi * i / n), 6)) for i in range(n)]
    return [shapely.Polygon(points)], -r * rng.choice([1.001, 1.005, 1.01, 1.02, 1.05])


def triangle(rng):
    """A random triangle and a distance about the radius of its inscribed circle: GEOS wears a triangle away
    when the distance is greater."""
    while True:
        polygon = shapely.Polygon([(round(rng.uniform(-8, 8), 4), round(rng.uniform(-8, 8), 4)) for _ in range(3)])
        if polygon.area > 4:
            break
    radius = 2 * polygon.area / polygon.length
    return polygon, -radius * rng.choice([0.99, 0.999, 1.001, 1.005, 1.01, 1.05])


def level(rng):
    """A regular polygon of 9, 11 or 13 corners, turned at random, and a distance of exactly half its box's
    least width: GEOS wears a ring away when its box is less than twice the distance across, so it offsets
    this one, past nothing, and keeps the small copy of it that the curve turns into."""
    n, r, turn = rng.choice([9, 11, 13]), rng.uniform(1, 3), rng.uniform(0, 2 * math.pi)
    polygon = shapely.Polygon([(round(r * math.cos(turn + 2 * math.pi * i / n), 6), round(r * math.sin(turn + 2 * math.pi * i / n), 6)) for i in range(n)])
    minx, miny, maxx, maxy = polygon.bounds
    return polygon, -min(maxx - minx, maxy - miny) / 2


def numbers(polygon):
    words = [str(1 + len(polygon.interiors))]
    for r in [polygon.exterior, *polygon.interiors]:
        coords = list(r.coords)
        words += [str(len(coords))] + [repr(float(c)) for point in coords for c in point]
    return words


def case(rng, polygon, distance=None):
    if distance is None:
        distance = rng.choice(DISTANCES) * rng.choice([1, -1])
    found = polygon.buffer(distance)
    answer = [] if found.is_empty else list(getattr(found, "geoms", [found]))
    words = numbers(polygon) + [repr(distance), str(len(answer))]
    for p in answer:
        words += numbers(p)
    return " ".join(words)


def main():
    rng = random.Random(SEED)
    print("# Buffers by shapely 2.2.0 (GEOS 3.14.1); conformance/oracle/buffer.py, seed 20261011. Do not edit; regenerate.")
    for make, count in [(star, 60), (blocks, 80), (ring, 20), (dumbbell, 50), (holed, 50)]:
        written = 0
        while written < count:
            for polygon in make(rng):
                polygon = oriented(polygon)
                if polygon.is_valid and written < count:
                    print(case(rng, polygon))
                    written += 1
    for _ in range(40):
        polygons, distance = small(rng)
        print(case(rng, oriented(polygons[0]), distance))
    for make, count in [(triangle, 20), (level, 10)]:
        for _ in range(count):
            polygon, distance = make(rng)
            print(case(rng, oriented(polygon), distance))


if __name__ == "__main__":
    main()
