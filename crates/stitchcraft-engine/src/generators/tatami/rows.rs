//! The rows of a tatami fill: parallel lines across each part of the fill's region, cut into segments
//! where they meet its outline (`REQ-FILL-TAT-002`, `REQ-FILL-TAT-010`).
//!
//! Rows lie where Ink/Stitch lays them. They run at the fill's angle, counter-clockwise from horizontal
//! on screen. Measured across them from the design's origin, the first row of a part lies the largest
//! whole number of spacings from the origin that does not pass the part, so fills side by side at the same
//! angle and spacing share their rows. With an end row spacing, each step to the next row grows (or
//! shrinks) with its distance from the first row, from the row spacing to the end spacing over the
//! part's height. Ink/Stitch carries the change on past that height, where a shrinking spacing can stack
//! rows without end; here the step stays at the end spacing (`DEV-FILL-003`).
//!
//! A row's segments are the stretches of it in the part, its outline included, split wherever the row
//! meets the outline: where it crosses it, where the outline touches it from inside, and at both ends of
//! a stretch it runs along. That is how GEOS cuts a line with a polygon for Ink/Stitch. A row that only
//! touches the part at a point has no segment.
//!
//! Which stretches are in the part is decided by counting, along the row, the edges that cross it, each
//! counted over its height from its lower end up to but not including its upper end. An edge that only
//! reaches the row from one side then counts once or twice, and the stretches beside the point stay as
//! they were.

use stitchcraft_core::{Exhausted, Meter, Point, math};

use crate::normalize::region::{Polygon, SAME};

/// Where rows lie: their angle in degrees, counter-clockwise from horizontal on screen, and how far apart
/// they are, in millimetres, with the spacing of the last ones when it differs.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Grid {
    /// The angle, in degrees.
    pub angle: f64,
    /// The spacing of the first rows.
    pub spacing: f64,
    /// The spacing the rows change to, across the part, if any.
    pub end_spacing: Option<f64>,
}

/// A stretch of a row in a part, from its start to its end along the row's direction.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Segment {
    /// Where it starts: a point of the part's outline.
    pub start: Point,
    /// Where it ends: a point of the outline farther along the row.
    pub end: Point,
}

/// A row across a part.
#[derive(Clone, Debug, PartialEq)]
pub struct Row {
    /// How far across the rows it lies, from the origin, in millimetres.
    pub across: f64,
    /// Its segments, in order along it.
    pub segments: Vec<Segment>,
}

/// The rows across `part` on `grid` that meet it in a segment, in order across. One unit of `meter` for
/// each row, and one for each edge of the part's rings each row is cut against.
pub fn rows(part: &Polygon, grid: &Grid, meter: &mut Meter) -> Result<Vec<Row>, Exhausted> {
    let (along, across) = axes(grid.angle);
    let rings: Vec<Ring> = std::iter::once(&part.outline).chain(&part.holes).map(|ring| Ring::new(ring, along, across)).collect();
    let edges = rings.iter().map(|ring| ring.points.len().saturating_sub(1)).sum::<usize>();
    let heights = part.outline.iter().map(|p| dot(*p, across));
    let (low, high) = heights.fold((f64::INFINITY, f64::NEG_INFINITY), |(low, high), v| (low.min(v), high.max(v)));
    let height = high - low;
    let mut found = Vec::new();
    if height <= 0.0 {
        return Ok(found);
    }
    let first = low - low.rem_euclid(grid.spacing);
    let mut v = first;
    while v < high {
        meter.charge(u64::try_from(edges).unwrap_or(u64::MAX).saturating_add(1))?;
        let segments = cut(&rings, v);
        if !segments.is_empty() {
            found.push(Row { across: v, segments });
        }
        v += grid.end_spacing.map_or(grid.spacing, |end| grid.spacing + (end - grid.spacing) * ((v - first) / height).min(1.0));
    }
    Ok(found)
}

/// The direction along rows at `angle` degrees and the direction across them, square to it, in the
/// drawing's axes (y down). Whole right angles are exact, so that rows at 0° or 90° run exactly along the
/// axes.
pub(crate) fn axes(angle: f64) -> ((f64, f64), (f64, f64)) {
    const RIGHT_ANGLES: [(f64, (f64, f64)); 4] = [(0.0, (0.0, 1.0)), (90.0, (1.0, 0.0)), (180.0, (0.0, -1.0)), (270.0, (-1.0, 0.0))];
    let turned = angle.rem_euclid(360.0);
    let (sin, cos) = RIGHT_ANGLES.iter().find(|(a, _)| *a == turned).map_or_else(|| math::sin_cos(math::to_radians(angle)), |&(_, sc)| sc);
    ((cos, -sin), (sin, cos))
}

/// How far `p` lies in `direction`.
fn dot(p: Point, direction: (f64, f64)) -> f64 {
    p.x() * direction.0 + p.y() * direction.1
}

/// A ring of a part, with how far along and across the rows each of its points lies.
struct Ring<'a> {
    points: &'a [Point],
    projected: Vec<(f64, f64)>,
}

impl<'a> Ring<'a> {
    fn new(points: &'a [Point], along: (f64, f64), across: (f64, f64)) -> Self {
        Ring { points, projected: points.iter().map(|&p| (dot(p, along), dot(p, across))).collect() }
    }
}

/// Where a row meets a part's outline: how far along the row, the point, and whether the row passes
/// between inside and outside there by the half-open count.
type Meet = (f64, Point, bool);

/// The segments of the row `v` across the rows, split where it meets the rings: the stretches inside,
/// and those along the rings.
fn cut(rings: &[Ring<'_>], v: f64) -> Vec<Segment> {
    let mut meets: Vec<Meet> = Vec::new();
    let mut runs_along: Vec<(f64, f64)> = Vec::new();
    for ring in rings {
        for (pair, projected) in ring.points.windows(2).zip(ring.projected.windows(2)) {
            let ((a, b), ((au, av), (bu, bv))) = ((pair[0], pair[1]), (projected[0], projected[1]));
            // Counted from its lower end up to, but not including, its upper end.
            let counted = av.min(bv) <= v && v < av.max(bv);
            if av == v && bv == v {
                runs_along.push((au.min(bu), au.max(bu)));
                meets.extend([(au, a, false), (bu, b, false)]);
            } else if av == v {
                meets.push((au, a, counted));
            } else if bv == v {
                meets.push((bu, b, counted));
            } else if counted {
                let at = a.lerp(b, (v - av) / (bv - av));
                meets.push((au + (bu - au) * ((v - av) / (bv - av)), at, true));
            }
        }
    }
    meets.sort_by(|m, n| m.0.total_cmp(&n.0));
    let mut inside = false;
    let mut segments = Vec::new();
    for pair in meets.windows(2) {
        let ((u, start, flips), (next, end, _)) = (pair[0], pair[1]);
        inside ^= flips;
        let on_ring = runs_along.iter().any(|&(low, high)| low <= u && next <= high);
        if (inside || on_ring) && start.distance(end) > SAME {
            segments.push(Segment { start, end });
        }
    }
    segments
}

#[cfg(test)]
mod tests {
    use stitchcraft_core::Budget;

    use super::*;
    use crate::normalize::fixture::cases;

    fn p(x: f64, y: f64) -> Point {
        Point::new(x, y).unwrap()
    }

    /// A part with the closed outline and holes given as corners.
    fn part(outline: &[(f64, f64)], holes: &[&[(f64, f64)]]) -> Polygon {
        let ring = |corners: &[(f64, f64)]| {
            let mut points: Vec<Point> = corners.iter().map(|&(x, y)| p(x, y)).collect();
            points.push(points[0]);
            points
        };
        Polygon { outline: ring(outline), holes: holes.iter().map(|hole| ring(hole)).collect() }
    }

    fn across(found: &[Row]) -> Vec<f64> {
        found.iter().map(|row| row.across).collect()
    }

    fn found(part: &Polygon, grid: Grid) -> Vec<Row> {
        rows(part, &grid, &mut Budget::DEFAULT.meter()).unwrap()
    }

    fn level(spacing: f64) -> Grid {
        Grid { angle: 0.0, spacing, end_spacing: None }
    }

    /// Each row's segments, each from (x, y) to (x, y).
    type Ends = Vec<Vec<((f64, f64), (f64, f64))>>;

    /// The segments of each row as (start, end) pairs of (x, y).
    fn ends(found: &[Row]) -> Ends {
        found.iter().map(|row| row.segments.iter().map(|s| ((s.start.x(), s.start.y()), (s.end.x(), s.end.y()))).collect()).collect()
    }

    #[test]
    fn rows_lie_a_whole_number_of_spacings_from_the_origin() {
        // A square from y = 0.3 to 1.3: rows at 0.5 and 1.0, none at 0 (before it) or 1.5 (past it).
        let square = part(&[(0.0, 0.3), (2.0, 0.3), (2.0, 1.3), (0.0, 1.3)], &[]);
        assert_eq!(across(&found(&square, level(0.5))), [0.5, 1.0]);
        // Below the origin too: the first row is the whole number of spacings at or before the part.
        let below = part(&[(0.0, -1.3), (2.0, -1.3), (2.0, -0.3), (0.0, -0.3)], &[]);
        assert_eq!(across(&found(&below, level(0.5))), [-1.0, -0.5]);
    }

    #[test]
    fn a_row_along_the_outline_is_a_segment_and_one_touching_it_at_a_point_is_none() {
        // The square's top edge lies on the first row; its bottom edge on no row, as rows stop short of it.
        let square = part(&[(0.0, 0.0), (2.0, 0.0), (2.0, 1.0), (0.0, 1.0)], &[]);
        let rows = found(&square, level(0.5));
        assert_eq!(across(&rows), [0.0, 0.5]);
        assert_eq!(ends(&rows)[0], [((0.0, 0.0), (2.0, 0.0))]);
        // A triangle's apex touches the row at y = 0 from below: no segment there.
        let triangle = part(&[(1.0, 0.0), (2.0, 1.0), (0.0, 1.0)], &[]);
        assert_eq!(across(&found(&triangle, level(0.5))), [0.5]);
        // 2 towers on a base: the row along their tops is 2 segments, and the outside between them none.
        let towers = part(&[(0.0, 0.0), (2.0, 0.0), (2.0, 2.0), (4.0, 2.0), (4.0, 0.0), (6.0, 0.0), (6.0, 4.0), (0.0, 4.0)], &[]);
        assert_eq!(ends(&found(&towers, level(2.0)))[0], [((0.0, 0.0), (2.0, 0.0)), ((4.0, 0.0), (6.0, 0.0))]);
        // A stretch along the outline no longer than the precision, a millionth of a micrometre, is none.
        let sliver = part(&[(0.0, 0.0), (SAME, 0.0), (0.0, 1.0)], &[]);
        assert_eq!(Point::ORIGIN.distance(p(SAME, 0.0)), SAME);
        assert!(found(&sliver, level(1.0)).is_empty());
    }

    #[test]
    fn rows_are_split_where_the_outline_touches_them_from_inside() {
        // A notch from y = 2 reaching down to the row at y = 1, its tip at x = 2: 2 segments meeting there.
        let vee = part(&[(0.0, 0.0), (4.0, 0.0), (4.0, 2.0), (2.5, 2.0), (2.0, 1.0), (1.5, 2.0), (0.0, 2.0)], &[]);
        let rows = found(&vee, level(1.0));
        assert_eq!(ends(&rows)[1], [((0.0, 1.0), (2.0, 1.0)), ((2.0, 1.0), (4.0, 1.0))]);
        // A notch with a floor on the row: the floor is a segment of its own, between the 2 inside.
        let notch = part(&[(0.0, 0.0), (6.0, 0.0), (6.0, 2.0), (4.0, 2.0), (4.0, 1.0), (2.0, 1.0), (2.0, 2.0), (0.0, 2.0)], &[]);
        let rows = found(&notch, level(1.0));
        assert_eq!(ends(&rows)[1], [((0.0, 1.0), (2.0, 1.0)), ((2.0, 1.0), (4.0, 1.0)), ((4.0, 1.0), (6.0, 1.0))]);
    }

    #[test]
    fn rows_cross_slanted_sides_where_they_cross_them() {
        // A parallelogram leaning right: the row at y = 5 crosses its left side at x = 5 and its right at 7.
        let leaning = part(&[(0.0, 0.0), (2.0, 0.0), (12.0, 10.0), (10.0, 10.0)], &[]);
        assert_eq!(ends(&found(&leaning, level(5.0))), [vec![((0.0, 0.0), (2.0, 0.0))], vec![((5.0, 5.0), (7.0, 5.0))]]);
    }

    #[test]
    fn rows_are_cut_by_holes_and_pass_through_corners_where_the_outline_crosses() {
        // A diamond: the row through its side corners is one segment.
        let diamond = part(&[(2.0, 0.0), (4.0, 2.0), (2.0, 4.0), (0.0, 2.0)], &[]);
        assert_eq!(ends(&found(&diamond, level(2.0))), [vec![((0.0, 2.0), (4.0, 2.0))]], "the first row touches its top corner only");
        // A hole cuts the row through it in 2; a row along the hole's top edge in 3, the edge among them.
        let holed = part(&[(0.0, 0.0), (6.0, 0.0), (6.0, 4.0), (0.0, 4.0)], &[&[(2.0, 1.0), (2.0, 3.0), (4.0, 3.0), (4.0, 1.0)]]);
        let rows = found(&holed, level(1.0));
        assert_eq!(ends(&rows)[1], [((0.0, 1.0), (2.0, 1.0)), ((2.0, 1.0), (4.0, 1.0)), ((4.0, 1.0), (6.0, 1.0))]);
        assert_eq!(ends(&rows)[2], [((0.0, 2.0), (2.0, 2.0)), ((4.0, 2.0), (6.0, 2.0))]);
    }

    #[test]
    fn rows_turn_with_the_angle() {
        let square = part(&[(0.0, 0.0), (2.0, 0.0), (2.0, 2.0), (0.0, 2.0)], &[]);
        // At 90° rows are upright, lying across from x = 0 to the right, and run up the screen.
        let upright = found(&square, Grid { angle: 90.0, spacing: 1.0, end_spacing: None });
        assert_eq!(across(&upright), [0.0, 1.0]);
        assert_eq!(ends(&upright), [vec![((0.0, 2.0), (0.0, 0.0))], vec![((1.0, 2.0), (1.0, 0.0))]]);
        // At 180° they lie as at 0°, across from the other side, and run right to left.
        let turned = found(&square, Grid { angle: 180.0, spacing: 1.0, end_spacing: None });
        assert_eq!(across(&turned), [-2.0, -1.0]);
        assert_eq!(ends(&turned), [vec![((2.0, 2.0), (0.0, 2.0))], vec![((2.0, 1.0), (0.0, 1.0))]]);
        // At 270°, or -90°, as at 90° across from the other side, and run down the screen.
        for angle in [270.0, -90.0] {
            let down = found(&square, Grid { angle, spacing: 1.0, end_spacing: None });
            assert_eq!(across(&down), [-2.0, -1.0]);
            assert_eq!(ends(&down), [vec![((2.0, 0.0), (2.0, 2.0))], vec![((1.0, 0.0), (1.0, 2.0))]]);
        }
        // At 45° the first row touches the square's corner only, and the next 2 cross it.
        let slanted = found(&square, Grid { angle: 45.0, spacing: 1.0, end_spacing: None });
        assert_eq!(across(&slanted), [1.0, 2.0]);
        let (start, end) = (slanted[0].segments[0].start, slanted[0].segments[0].end);
        assert!((start.x() + start.y() - 2f64.sqrt()).abs() < 1e-12 && (start.distance(end) - 2.0).abs() < 1e-12, "{start:?} {end:?}");
        // At 120° each row runs up the screen and to the left, at 120° from the x axis turning that way.
        let steep = found(&square, Grid { angle: 120.0, spacing: 0.5, end_spacing: None });
        assert!(steep.len() > 3);
        for segment in steep.iter().flat_map(|row| &row.segments) {
            let (dx, dy) = (segment.end.x() - segment.start.x(), segment.end.y() - segment.start.y());
            let length = (dx * dx + dy * dy).sqrt();
            assert!((dx / length + 0.5).abs() < 1e-12 && (dy / length + 3f64.sqrt() / 2.0).abs() < 1e-12, "{segment:?}");
        }
    }

    #[test]
    fn graded_rows_change_steadily_to_the_end_spacing_and_stay_there() {
        // From 1 to 3 over a height of 10: each step 1 + 2 × the distance from the first row / 10.
        let tall = part(&[(0.0, 0.0), (1.0, 0.0), (1.0, 10.0), (0.0, 10.0)], &[]);
        let rows = found(&tall, Grid { angle: 0.0, spacing: 1.0, end_spacing: Some(3.0) });
        let expected = [0.0, 1.0, 2.2, 3.64, 5.368, 7.4416, 9.92992];
        assert_eq!(rows.len(), expected.len());
        assert!(rows.iter().zip(expected).all(|(row, v)| (row.across - v).abs() < 1e-12), "{:?}", across(&rows));
        // Shrinking from 1 to 0.1 over a height of 1, the first row 0.5 before the part: past the height
        // the step stays 0.1, where the gradient carried on would stack rows towards 1.11 without end.
        let short = part(&[(0.0, 0.5), (1.0, 0.5), (1.0, 1.5), (0.0, 1.5)], &[]);
        let rows = found(&short, Grid { angle: 0.0, spacing: 1.0, end_spacing: Some(0.1) });
        let expected = [1.0, 1.1, 1.2, 1.3, 1.4];
        assert_eq!(rows.len(), expected.len());
        assert!(rows.iter().zip(expected).all(|(row, v)| (row.across - v).abs() < 1e-12), "{:?}", across(&rows));
        // The distance is from the first row, wherever that lies: the tall part 10 further down has its rows
        // 10 further down.
        let lower = part(&[(0.0, 10.0), (1.0, 10.0), (1.0, 20.0), (0.0, 20.0)], &[]);
        let rows = found(&lower, Grid { angle: 0.0, spacing: 1.0, end_spacing: Some(3.0) });
        let expected = [10.0, 11.0, 12.2, 13.64, 15.368, 17.4416, 19.92992];
        assert_eq!(rows.len(), expected.len());
        assert!(rows.iter().zip(expected).all(|(row, v)| (row.across - v).abs() < 1e-12), "{:?}", across(&rows));
    }

    /// Shapely's pieces of rows cut with random polygons of grid squares and half squares, and blocks with
    /// such cells taken out of their inside, which leaves holes (`conformance/fixtures/geometry/shapely-rows.txt`,
    /// written by `conformance/oracle/rows.py`).
    const SHAPELY: &str = include_str!("../../../../../conformance/fixtures/geometry/shapely-rows.txt");

    /// Each row's place across and its pieces, as (smaller x, larger x) in order of x.
    type Pieces = Vec<(f64, Vec<(f64, f64)>)>;

    #[test]
    fn rows_are_cut_as_shapely_cuts_them() {
        let mut checked = 0;
        for (line, mut words) in cases(SHAPELY) {
            let spacing = words.number();
            let rings = words.polylines();
            let expected: Pieces = (0..words.count())
                .map(|_| {
                    let y = words.number();
                    (y, (0..words.count()).map(|_| (words.number(), words.number())).collect())
                })
                .collect();
            let part = Polygon { outline: rings[0].clone(), holes: rings[1..].to_vec() };
            assert_cut_as(&part, spacing, &expected, line);
            // The same case leaned and squashed, x to 2x + y and y to y / 2, so that its edges slant and climb
            // by halves: a row's pieces map with it, x to 2x + its y. The numbers stay exact in binary.
            let map = |q: &Point| p(2.0 * q.x() + q.y(), q.y() / 2.0);
            let leaned = Polygon {
                outline: part.outline.iter().map(map).collect(),
                holes: part.holes.iter().map(|hole| hole.iter().map(map).collect()).collect(),
            };
            let moved: Pieces =
                expected.iter().map(|(y, pieces)| (y / 2.0, pieces.iter().map(|&(a, b)| (2.0 * a + y, 2.0 * b + y)).collect())).collect();
            assert_cut_as(&leaned, spacing / 2.0, &moved, line);
            checked += 1;
        }
        assert_eq!(checked, 812);
    }

    /// Asserts that the rows of `part`, `spacing` apart, have the pieces `expected` (the fixture's `line`).
    fn assert_cut_as(part: &Polygon, spacing: f64, expected: &Pieces, line: usize) {
        let got: Pieces = found(part, level(spacing))
            .iter()
            .map(|row| (row.across, row.segments.iter().map(|s| (s.start.x().min(s.end.x()), s.start.x().max(s.end.x()))).collect()))
            .collect();
        let same = got.len() == expected.len()
            && got.iter().zip(expected).all(|((gy, gp), (ey, ep))| {
                gy == ey && gp.len() == ep.len() && gp.iter().zip(ep).all(|(g, e)| (g.0 - e.0).abs() < 1e-9 && (g.1 - e.1).abs() < 1e-9)
            });
        assert!(same, "line {line}, spacing {spacing}: shapely {expected:?}, rows {got:?}");
    }

    #[test]
    fn a_part_of_no_height_has_no_rows() {
        let flat = part(&[(0.0, 1.0), (2.0, 1.0)], &[]);
        assert!(found(&flat, level(1.0)).is_empty());
    }

    #[test]
    fn the_budget_bounds_the_work() {
        let square = part(&[(0.0, 0.0), (2.0, 0.0), (2.0, 1.0), (0.0, 1.0)], &[]);
        // 2 rows, each cut against the square's 4 edges.
        let mut meter = Budget::DEFAULT.meter();
        rows(&square, &level(0.5), &mut meter).unwrap();
        assert_eq!(Budget::DEFAULT.max_work - meter.work_left(), 2 * 5);
        assert!(rows(&square, &level(0.5), &mut Budget { max_stitches: 1, max_work: 9 }.meter()).is_err());
    }
}
