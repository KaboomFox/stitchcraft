"""Record shapely's offset curves, for the engine's offsets (REQ-SAT-016).

A satin column drawn as one path becomes 2 rails, its centre line offset by half its width to each side.
Ink/Stitch offsets it with shapely's `LineString.offset_curve`, whose answers this script records for
random lines: straight lines that turn sharply, arcs, spirals, waves and zigzags, walks on a coarse grid,
whose points often lie in line and whose segments turn straight back, and walks of tiny steps, whose
nodes lie within a hundredth of the distance of each other, at several distances, with each join and
miter limit. The tests of `crates/stitchcraft-engine/src/normalize/offset/` check that
the engine builds the same curves. shapely runs as a black box, as decode.py runs pyembroidery: only its
public API is used and only its answers are kept.

Regenerate the fixture (shapely 2.2.0's wheels bundle GEOS 3.14.1):

    python3 -m venv target/geometry-venv
    target/geometry-venv/bin/pip install shapely==2.2.0
    target/geometry-venv/bin/python -I conformance/oracle/offset.py \
        > conformance/fixtures/geometry/shapely-offset.txt

Each case is one line of numbers and words, whitespace apart: the line (a count of points and their
coordinates), the distance, the join (mitre, round or bevel) and the miter limit, then the answer: a count
of curves (0 when the offset is empty, more than 1 when it falls apart), each a count of points and their
coordinates.
"""

import math
import random

import shapely

SEED = 20261011
JOINS = ["mitre", "round", "bevel"]
LIMITS = [1.0, 2.0, 4.0, 5.0, 10.0]


def walk(rng, turn, shortest, longest, most):
    """A polyline that turns by up to `turn` radians at each of up to `most` points."""
    points = [(0.0, 0.0)]
    angle = 0.0
    for _ in range(rng.randint(2, most) - 1):
        angle += rng.uniform(-turn, turn)
        step = rng.uniform(shortest, longest)
        x, y = points[-1]
        points.append((round(x + step * math.cos(angle), 3), round(y + step * math.sin(angle), 3)))
    return points


def curve(rng):
    """A flattened arc, spiral, wave or zigzag."""
    kind = rng.choice(["arc", "spiral", "wave", "zigzag"])
    if kind == "arc":
        radius, n, span = rng.uniform(1, 20), rng.randint(5, 20), rng.uniform(0.5, 6.0)
        points = [(radius * math.cos(span * i / (n - 1)), radius * math.sin(span * i / (n - 1))) for i in range(n)]
    elif kind == "spiral":
        n, a, b, span = rng.randint(10, 20), rng.uniform(0.5, 3), rng.uniform(0.1, 1.0), rng.uniform(3, 15)
        points = [((a + b * t) * math.cos(t), (a + b * t) * math.sin(t)) for t in (span * i / (n - 1) for i in range(n))]
    elif kind == "wave":
        n, height, k, width = rng.randint(10, 20), rng.uniform(0.2, 5), rng.uniform(0.2, 3), rng.uniform(5, 30)
        points = [(width * i / (n - 1), height * math.sin(k * width * i / (n - 1))) for i in range(n)]
    else:
        points = [(i * rng.uniform(0.3, 3), rng.uniform(-2, 2)) for i in range(rng.randint(4, 12))]
    return [(round(x, 6), round(y, 6)) for x, y in points]


def grid(rng):
    """A walk on a grid of whole numbers: points in line, turns straight back, segments along others. It
    never ends where it starts: GEOS buffers a closed line as a ring, without caps, and the engine offsets
    only open lines."""
    while True:
        points = [(0.0, 0.0)]
        for _ in range(rng.randint(2, 8) - 1):
            while True:
                x, y = points[-1]
                step = (x + rng.randint(-2, 2), y + rng.randint(-2, 2))
                if step != points[-1]:
                    break
            points.append((float(step[0]), float(step[1])))
        if points[-1] != points[0]:
            return points


def dense(rng):
    """A walk of tiny steps between longer ones, so that the line is simplified before it is offset."""
    points = [(0.0, 0.0)]
    angle = 0.0
    for _ in range(rng.randint(4, 10) - 1):
        angle += rng.uniform(-1.0, 1.0)
        step = rng.choice([rng.uniform(0.0002, 0.01), rng.uniform(1, 5)])
        x, y = points[-1]
        points.append((round(x + step * math.cos(angle), 6), round(y + step * math.sin(angle), 6)))
    return points


def numbers(points):
    return [str(len(points))] + [repr(float(c)) for point in points for c in point]


def case(rng, points):
    distance = rng.choice([0.1, 0.3, 0.5, 1.0, 2.0, 4.0]) * rng.choice([1, -1])
    join = rng.choice(JOINS)
    limit = rng.choice(LIMITS)
    found = shapely.LineString(points).offset_curve(distance, join_style=join, mitre_limit=limit)
    if found.is_empty:
        curves = []
    elif found.geom_type == "LineString":
        curves = [list(found.coords)]
    else:
        curves = [list(part.coords) for part in found.geoms]
    words = numbers(points) + [repr(distance), join, repr(limit), str(len(curves))]
    for curve_points in curves:
        words += numbers(curve_points)
    return " ".join(words)


def main():
    rng = random.Random(SEED)
    print("# Written by conformance/oracle/offset.py with shapely 2.2.0 (GEOS 3.14.1). Do not edit; regenerate.")
    for _ in range(250):
        print(case(rng, walk(rng, 1.2, 2, 10, 6)))
    for _ in range(250):
        print(case(rng, walk(rng, 3.0, 0.2, 6, 8)))
    for _ in range(250):
        print(case(rng, curve(rng)))
    for _ in range(150):
        print(case(rng, grid(rng)))
    for _ in range(150):
        print(case(rng, dense(rng)))


if __name__ == "__main__":
    main()
