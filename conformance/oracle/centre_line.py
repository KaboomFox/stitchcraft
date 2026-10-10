"""Record shapely's answers to the measures that make a satin column's rails from its centre line
(REQ-SAT-016).

Ink/Stitch makes a satin column drawn as one path into rails and rungs with shapely: it cuts the line in
half by length, finds points along it, measures how far along it a point lies, tests whether a rung
crosses a rail at one point, and starts a closed line where a rung crosses the edge of its stroke twice.
This script asks shapely those questions about random lines and prints its answers; the tests of
`crates/stitchcraft-engine/src/normalize/centre_line.rs` check that the engine answers the same. shapely
runs as a black box: only its public API is used and only its answers are kept.

Regenerate the fixture (shapely 2.2.0's wheels bundle GEOS 3.14.1):

    python3 -m venv target/geometry-venv
    target/geometry-venv/bin/pip install shapely==2.2.0
    target/geometry-venv/bin/python -I conformance/oracle/centre_line.py \
        > conformance/fixtures/geometry/shapely-centre-line.txt

Each case is a kind and then numbers, whitespace apart. LINE is a count of points and their coordinates.

    half  LINE           FIRST SECOND     the line cut in half by length (shapely.ops.substring), each a LINE
    at    LINE f         x y              the point a fraction f of the way along the line (interpolate)
    place LINE x y       along            how far along the line its point nearest (x, y) lies (project)
    once  LINE ax ay bx by   n            how many points the segment shares with the line: 1 when it is a
                                          single point, 0 when none, 2 for more or for an overlap
    start LINE width     i                the first segment of the closed LINE whose middle rung, the width
                                          and a thousandth long, crosses the boundary of the line's buffer
                                          by half the width at exactly 2 points; -1 when none does
"""

import math
import random

import shapely
import shapely.affinity
import shapely.ops

SEED = 20261012


def numbers(points):
    return [str(len(points))] + [repr(float(c)) for point in points for c in point]


def walk(rng, most, turn=2.0):
    points = [(0.0, 0.0)]
    angle = rng.uniform(-math.pi, math.pi)
    for _ in range(rng.randint(2, most) - 1):
        angle += rng.uniform(-turn, turn)
        step = rng.uniform(0.2, 5)
        x, y = points[-1]
        points.append((round(x + step * math.cos(angle), 3), round(y + step * math.sin(angle), 3)))
    return points


def ring(rng):
    """A closed line: a star-like polygon, sometimes tight, sometimes crossing itself."""
    n = rng.randint(3, 12)
    centre_r = rng.uniform(1, 10)
    points = []
    for i in range(n):
        angle = 2 * math.pi * i / n + rng.uniform(-0.3, 0.3)
        radius = centre_r * rng.uniform(0.2, 1.5)
        points.append((round(radius * math.cos(angle), 3), round(radius * math.sin(angle), 3)))
    return points + [points[0]]


def kind(intersection):
    if intersection.is_empty:
        return 0
    if intersection.geom_type == "Point":
        return 1
    return 2


def main():
    rng = random.Random(SEED)
    print("# Written by conformance/oracle/centre_line.py with shapely 2.2.0 (GEOS 3.14.1). Do not edit; regenerate.")
    for _ in range(200):
        line = shapely.LineString(walk(rng, 10))
        first = shapely.ops.substring(line, 0, 0.5, normalized=True)
        second = shapely.ops.substring(line, 0.5, 1, normalized=True)
        print(" ".join(["half"] + numbers(list(line.coords)) + numbers(list(first.coords)) + numbers(list(second.coords))))
    for _ in range(200):
        line = shapely.LineString(walk(rng, 8))
        f = rng.choice([rng.random(), 0.0, 1.0, 0.5, 1.0005])
        p = line.interpolate(f, normalized=True)
        print(" ".join(["at"] + numbers(list(line.coords)) + [repr(f), repr(p.x), repr(p.y)]))
    for _ in range(200):
        coords = walk(rng, 8, 3.0)
        line = shapely.LineString(coords)
        p = rng.choice(coords) if rng.random() < 0.5 else (rng.uniform(-5, 5), rng.uniform(-5, 5))
        print(" ".join(["place"] + numbers(coords) + [repr(float(p[0])), repr(float(p[1])), repr(line.project(shapely.Point(p)))]))
    for _ in range(300):
        coords = walk(rng, 6, 3.0)
        a = rng.choice(coords + [(rng.uniform(-5, 5), rng.uniform(-5, 5))])
        b = (round(a[0] + rng.uniform(-6, 6), 3), round(a[1] + rng.uniform(-6, 6), 3))
        found = kind(shapely.LineString([a, b]).intersection(shapely.LineString(coords)))
        print(" ".join(["once"] + numbers(coords) + [repr(float(c)) for c in (*a, *b)] + [str(found)]))
    for _ in range(300):
        coords = ring(rng)
        width = rng.choice([0.5, 1.0, 2.0, 3.0, 5.0])
        closed = shapely.LinearRing(coords)
        edge = closed.buffer(width / 2).boundary
        first = -1
        for i, (p, q) in enumerate(zip(coords[:-1], coords[1:])):
            segment = shapely.LineString([p, q])
            if segment.length == 0:
                continue
            rung = shapely.affinity.rotate(segment, 90)
            rung = shapely.affinity.scale(rung, (width + 0.001) / segment.length, (width + 0.001) / segment.length)
            meets = rung.intersection(edge)
            if meets.geom_type == "MultiPoint" and len(meets.geoms) == 2:
                first = i
                break
        print(" ".join(["start"] + numbers(coords) + [repr(width), str(first)]))


if __name__ == "__main__":
    main()
