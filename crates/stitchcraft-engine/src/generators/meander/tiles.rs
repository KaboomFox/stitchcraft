//! StitchCraft's meander tiles (`REQ-FILL-MEA-001`): the regular and semiregular tilings of the plane, whose
//! sides are the lines a meander wanders along (`docs/src/design/algorithms/fills.md` › Meander fill ›
//! Tiles).
//!
//! **Why tilings.** A meander needs a network with no free end, which it can wander through without
//! passing a point twice. The sides of a tiling by polygons are such a network, and the 11 tilings in
//! which every corner looks alike are plain mathematics, made here in code. Ink/Stitch's tiles are
//! drawings it ships, which StitchCraft does not copy (`DEV-FILL-018`).
//!
//! **Which tilings.** 7 of the 11. The 2 with dodecagons are left out: a detour cannot go round a
//! dodecagon, every way round one being 11 lines, past the 7 a detour may have, and over a square 100 mm a
//! side the meander left out 91 % of the network of dodecagons and triangles, and 32 % of the network of
//! dodecagons, hexagons and squares. The 2 snub tilings are not made yet.
//!
//! **How a tile is made.** Each is given by its cell, the 2 steps that repeat it, and the corners of one
//! cell, all in sides of 1. Its lines are found, not listed: every 2 corners a side apart, in the cell or
//! one beside it, are joined. A tiling's corners are never a side apart but along a side, which the tests
//! check by counting the lines at each corner.

use stitchcraft_core::Point;
use stitchcraft_core::math::sin_cos;
use stitchcraft_params::ChoiceOption;

/// How far 2 corners may lie from a side apart and still be joined, in sides.
const SIDE: f64 = 1e-9;
/// How many cells away a line's far corner is looked for. No tiling here has a side that reaches farther,
/// as the tests' counts of the lines at each corner check.
const REACH: i32 = 2;

/// A line of a tile: from corner `from` of a cell to corner `to` of the cell `cell` steps along from it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Line {
    pub(crate) from: usize,
    pub(crate) to: usize,
    pub(crate) cell: (i32, i32),
}

/// A tile: the 2 steps that repeat its cell, the corners of one cell and the lines between them, all in
/// sides of 1.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Tile {
    pub(crate) steps: [Point; 2],
    pub(crate) corners: Vec<Point>,
    pub(crate) lines: Vec<Line>,
}

/// The tile a fill sews that names none of StitchCraft's.
pub(crate) const DEFAULT: &str = "honeycomb";

/// The tiles `meander_pattern` accepts: StitchCraft's, its default first, then Ink/Stitch's, kept for the
/// designs that name them and sewn as the default (`SC-W0316`). Ink/Stitch's ids are its tiles' folder
/// names at the commit the contract was read at, in their order, which `conformance/inkstitch/
/// check_params.py` compares.
pub(crate) const PATTERNS: &[ChoiceOption] = &[
    ChoiceOption::new("honeycomb", "Honeycomb"),
    ChoiceOption::new("squares", "Squares"),
    ChoiceOption::new("triangles", "Triangles"),
    ChoiceOption::new("octagons-squares", "Octagons and squares"),
    ChoiceOption::new("hexagons-triangles", "Hexagons and triangles"),
    ChoiceOption::new("hexagons-squares-triangles", "Hexagons, squares and triangles"),
    ChoiceOption::new("squares-triangles", "Squares and triangles"),
    ChoiceOption::kept("N3-11a", "Ink/Stitch's N3-11a"),
    ChoiceOption::kept("N3-12", "Ink/Stitch's N3-12"),
    ChoiceOption::kept("N3-16a", "Ink/Stitch's N3-16a"),
    ChoiceOption::kept("N3-17", "Ink/Stitch's N3-17"),
    ChoiceOption::kept("N3-18", "Ink/Stitch's N3-18"),
    ChoiceOption::kept("N3-18-modified", "Ink/Stitch's N3-18-modified"),
    ChoiceOption::kept("N3-20", "Ink/Stitch's N3-20"),
    ChoiceOption::kept("N3-23b", "Ink/Stitch's N3-23b"),
    ChoiceOption::kept("N3-25c", "Ink/Stitch's N3-25c"),
    ChoiceOption::kept("N3-26b", "Ink/Stitch's N3-26b"),
    ChoiceOption::kept("N3-27", "Ink/Stitch's N3-27"),
    ChoiceOption::kept("N3-30a", "Ink/Stitch's N3-30a"),
    ChoiceOption::kept("N3-51b", "Ink/Stitch's N3-51b"),
    ChoiceOption::kept("N3-57f-modified", "Ink/Stitch's N3-57f-modified"),
    ChoiceOption::kept("N3-58b", "Ink/Stitch's N3-58b"),
    ChoiceOption::kept("N3-6", "Ink/Stitch's N3-6"),
    ChoiceOption::kept("N3-7", "Ink/Stitch's N3-7"),
    ChoiceOption::kept("N3-8a", "Ink/Stitch's N3-8a"),
    ChoiceOption::kept("N3-8a-modified", "Ink/Stitch's N3-8a-modified"),
    ChoiceOption::kept("N3-8b", "Ink/Stitch's N3-8b"),
    ChoiceOption::kept("N4-13b", "Ink/Stitch's N4-13b"),
    ChoiceOption::kept("N4-13c", "Ink/Stitch's N4-13c"),
    ChoiceOption::kept("N4-13d", "Ink/Stitch's N4-13d"),
    ChoiceOption::kept("N4-13e", "Ink/Stitch's N4-13e"),
    ChoiceOption::kept("N4-13f", "Ink/Stitch's N4-13f"),
    ChoiceOption::kept("N4-16a", "Ink/Stitch's N4-16a"),
    ChoiceOption::kept("N4-19", "Ink/Stitch's N4-19"),
    ChoiceOption::kept("N4-20", "Ink/Stitch's N4-20"),
    ChoiceOption::kept("N4-21c", "Ink/Stitch's N4-21c"),
    ChoiceOption::kept("N4-22", "Ink/Stitch's N4-22"),
    ChoiceOption::kept("N4-23a", "Ink/Stitch's N4-23a"),
    ChoiceOption::kept("N4-23c", "Ink/Stitch's N4-23c"),
    ChoiceOption::kept("N4-27", "Ink/Stitch's N4-27"),
    ChoiceOption::kept("N4-29e", "Ink/Stitch's N4-29e"),
    ChoiceOption::kept("N4-29f", "Ink/Stitch's N4-29f"),
    ChoiceOption::kept("N4-31", "Ink/Stitch's N4-31"),
    ChoiceOption::kept("N4-38", "Ink/Stitch's N4-38"),
    ChoiceOption::kept("N4-42e", "Ink/Stitch's N4-42e"),
    ChoiceOption::kept("N4-44", "Ink/Stitch's N4-44"),
    ChoiceOption::kept("N4-52", "Ink/Stitch's N4-52"),
    ChoiceOption::kept("N4-54d", "Ink/Stitch's N4-54d"),
    ChoiceOption::kept("N4-5a", "Ink/Stitch's N4-5a"),
    ChoiceOption::kept("N4-5a-2", "Ink/Stitch's N4-5a-2"),
    ChoiceOption::kept("N4-82", "Ink/Stitch's N4-82"),
    ChoiceOption::kept("N4-85d", "Ink/Stitch's N4-85d"),
    ChoiceOption::kept("N5-1e1", "Ink/Stitch's N5-1e1"),
    ChoiceOption::kept("N5-1q2", "Ink/Stitch's N5-1q2"),
    ChoiceOption::kept("N5-1t", "Ink/Stitch's N5-1t"),
    ChoiceOption::kept("N6-1", "Ink/Stitch's N6-1"),
    ChoiceOption::kept("N6-2", "Ink/Stitch's N6-2"),
    ChoiceOption::kept("N6-5b", "Ink/Stitch's N6-5b"),
    ChoiceOption::kept("N6-6a", "Ink/Stitch's N6-6a"),
    ChoiceOption::kept("N6-6c", "Ink/Stitch's N6-6c"),
    ChoiceOption::kept("NC5-11a", "Ink/Stitch's NC5-11a"),
    ChoiceOption::kept("NC5-20a", "Ink/Stitch's NC5-20a"),
    ChoiceOption::kept("P3-1", "Ink/Stitch's P3-1"),
    ChoiceOption::kept("P3-12", "Ink/Stitch's P3-12"),
    ChoiceOption::kept("P4-1", "Ink/Stitch's P4-1"),
    ChoiceOption::kept("P4-10", "Ink/Stitch's P4-10"),
    ChoiceOption::kept("P4-15", "Ink/Stitch's P4-15"),
    ChoiceOption::kept("P4-19", "Ink/Stitch's P4-19"),
    ChoiceOption::kept("P4-23", "Ink/Stitch's P4-23"),
    ChoiceOption::kept("P4-24", "Ink/Stitch's P4-24"),
    ChoiceOption::kept("P4-25", "Ink/Stitch's P4-25"),
    ChoiceOption::kept("P4-43", "Ink/Stitch's P4-43"),
    ChoiceOption::kept("P4-43-modified", "Ink/Stitch's P4-43-modified"),
    ChoiceOption::kept("P4-47", "Ink/Stitch's P4-47"),
    ChoiceOption::kept("P5-10_11", "Ink/Stitch's P5-10_11"),
    ChoiceOption::kept("P5-19", "Ink/Stitch's P5-19"),
    ChoiceOption::kept("P5-23_24", "Ink/Stitch's P5-23_24"),
    ChoiceOption::kept("P5-4", "Ink/Stitch's P5-4"),
    ChoiceOption::kept("P6-6_11", "Ink/Stitch's P6-6_11"),
    ChoiceOption::kept("diamond-square", "Ink/Stitch's diamond-square"),
    ChoiceOption::kept("hexagon", "Ink/Stitch's hexagon"),
    ChoiceOption::kept("weird-one", "Ink/Stitch's weird-one"),
];

/// The tile `id` names, or `None` when it names none of StitchCraft's.
pub(crate) fn tile(id: &str) -> Option<Tile> {
    let r3 = 3.0_f64.sqrt();
    let r2 = 2.0_f64.sqrt();
    let p = |x: f64, y: f64| Point::new(x, y).unwrap_or(Point::ORIGIN);
    let (steps, corners): ([Point; 2], Vec<Point>) = match id {
        "honeycomb" => ([p(1.5, r3 / 2.0), p(1.5, -r3 / 2.0)], vec![p(0.0, 0.0), p(1.0, 0.0)]),
        "squares" => ([p(1.0, 0.0), p(0.0, 1.0)], vec![p(0.0, 0.0)]),
        "triangles" => ([p(1.0, 0.0), p(0.5, r3 / 2.0)], vec![p(0.0, 0.0)]),
        "octagons-squares" => {
            // Octagons on a square grid share their level and upright sides, and squares fill between.
            let l = 1.0 + r2;
            let (sin, _) = sin_cos(std::f64::consts::PI / 8.0);
            ([p(l, 0.0), p(0.0, l)], round(8, std::f64::consts::PI / 8.0, 0.5 / sin, p))
        }
        "hexagons-triangles" => ([p(2.0, 0.0), p(1.0, r3)], vec![p(0.0, 0.0), p(1.0, 0.0), p(0.5, r3 / 2.0)]),
        "hexagons-squares-triangles" => {
            // Hexagons on a triangular grid, a square on each side and a triangle between 3.
            let l = 1.0 + r3;
            ([p(l, 0.0), p(l / 2.0, l * r3 / 2.0)], round(6, std::f64::consts::PI / 6.0, 1.0, p))
        }
        "squares-triangles" => ([p(1.0, 0.0), p(0.5, 1.0 + r3 / 2.0)], vec![p(0.0, 0.0), p(0.0, 1.0)]),
        _ => return None,
    };
    let corners = in_cell(&steps, corners);
    let lines = lines(&steps, &corners);
    Some(Tile { steps, corners, lines })
}

/// The `n` corners of a regular polygon round the origin, `radius` from it, the first at `turn` from the
/// level, each the next turned an `n`th of a full turn.
fn round(n: u32, turn: f64, radius: f64, p: impl Fn(f64, f64) -> Point) -> Vec<Point> {
    (0..n)
        .map(|k| {
            let (sin, cos) = sin_cos(turn + std::f64::consts::TAU * f64::from(k) / f64::from(n));
            p(radius * cos, radius * sin)
        })
        .collect()
}

/// The corners `points`, each moved by whole steps into the cell, and each the cell's corners hold more
/// than once kept once.
fn in_cell(steps: &[Point; 2], points: Vec<Point>) -> Vec<Point> {
    let mut kept: Vec<Point> = Vec::new();
    for point in points {
        let (u, v) = across(steps, point);
        let moved = at(steps, u - u.floor(), v - v.floor());
        if !kept.iter().any(|&k| same(steps, k, moved)) {
            kept.push(moved);
        }
    }
    kept
}

/// Every line of a tile with `steps` and `corners`: 2 corners a side apart, in a cell or in one near it,
/// each line once.
fn lines(steps: &[Point; 2], corners: &[Point]) -> Vec<Line> {
    let mut lines = Vec::new();
    for (from, &a) in corners.iter().enumerate() {
        for (to, &b) in corners.iter().enumerate() {
            for i in -REACH..=REACH {
                for j in -REACH..=REACH {
                    // Each line is found from both its ends: kept from the lower corner, or from the first
                    // cell of a corner joined to itself.
                    let once = from < to || (from == to && (i, j) > (0, 0));
                    let there = plus(b, offset(steps, i, j));
                    if once && (a.distance(there) - 1.0).abs() < SIDE {
                        lines.push(Line { from, to, cell: (i, j) });
                    }
                }
            }
        }
    }
    lines
}

/// Where `point` lies in steps of `steps`.
fn across(steps: &[Point; 2], point: Point) -> (f64, f64) {
    let [s, t] = steps;
    let det = s.x() * t.y() - s.y() * t.x();
    ((point.x() * t.y() - point.y() * t.x()) / det, (s.x() * point.y() - s.y() * point.x()) / det)
}

/// The point `u` and `v` steps along.
fn at(steps: &[Point; 2], u: f64, v: f64) -> Point {
    let [s, t] = steps;
    Point::new(u * s.x() + v * t.x(), u * s.y() + v * t.y()).unwrap_or(Point::ORIGIN)
}

/// The step `i` cells along the first step and `j` along the second.
pub(crate) fn offset(steps: &[Point; 2], i: i32, j: i32) -> Point {
    at(steps, f64::from(i), f64::from(j))
}

/// `a` moved by `b`, or `a` itself should the sum not be finite.
pub(crate) fn plus(a: Point, b: Point) -> Point {
    Point::new(a.x() + b.x(), a.y() + b.y()).unwrap_or(a)
}

/// Whether `a` and `b` are one corner, whole steps apart.
fn same(steps: &[Point; 2], a: Point, b: Point) -> bool {
    (-1..=1).any(|i| (-1..=1).any(|j| a.distance(plus(b, offset(steps, i, j))) < SIDE))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// How many lines meet at each corner of `tile`.
    fn meeting(tile: &Tile) -> Vec<usize> {
        (0..tile.corners.len()).map(|c| tile.lines.iter().map(|l| usize::from(l.from == c) + usize::from(l.to == c)).sum()).collect()
    }

    #[test]
    fn req_fill_mea_001_each_tile_s_corners_meet_as_many_lines_as_its_tiling_has_polygons_round_them() {
        let expected = [
            ("honeycomb", 2, 3),
            ("squares", 1, 4),
            ("triangles", 1, 6),
            ("octagons-squares", 4, 3),
            ("hexagons-triangles", 3, 4),
            ("hexagons-squares-triangles", 6, 4),
            ("squares-triangles", 2, 5),
        ];
        let ours: Vec<&str> = PATTERNS.iter().filter(|t| t.offered).map(|t| t.id).collect();
        assert_eq!(expected.map(|(id, ..)| id).to_vec(), ours);
        assert_eq!((ours[0], PATTERNS.len()), (DEFAULT, 7 + 75));
        for (id, corners, lines) in expected {
            let tile = tile(id).unwrap();
            assert_eq!(tile.corners.len(), corners, "{id}");
            assert_eq!(meeting(&tile), vec![lines; corners], "{id}");
            // Every line is a side long, and every corner lies in the cell.
            for line in &tile.lines {
                let (a, b) = (tile.corners[line.from], plus(tile.corners[line.to], offset(&tile.steps, line.cell.0, line.cell.1)));
                assert!((a.distance(b) - 1.0).abs() < 1e-9, "{id}: {line:?}");
            }
        }
        assert_eq!(tile("N3-11a"), None);
    }

    #[test]
    fn a_corner_on_the_cell_s_edge_is_kept_once() {
        let steps = [Point::new(1.0, 0.0).unwrap(), Point::new(0.0, 1.0).unwrap()];
        let points = vec![Point::new(0.0, 0.0).unwrap(), Point::new(1.0, 1.0).unwrap(), Point::new(0.5, 2.0).unwrap()];
        assert_eq!(in_cell(&steps, points), [Point::new(0.0, 0.0).unwrap(), Point::new(0.5, 0.0).unwrap()]);
    }
}
