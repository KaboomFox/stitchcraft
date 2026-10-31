//! Conformance metrics, defined once so that every case measures the same way
//! (`docs/src/design/conformance.md` › Metrics).
//!
//! **Coverage** is how much of a fill's area its thread covers, as an eye sees it from above. The area and
//! the thread are drawn on a grid of square pixels [`PIXEL`] wide. A pixel is the area's when its centre lies
//! inside the area's rings, by the even-odd rule, and covered when its centre lies within half of
//! [`THREAD`] of a stitch: of the straight thread between 2 needle holes, every stitch the plan sews
//! ([`StitchPlan::sewn_stitches`]), top stitches, underlay, travel and locks alike. Coverage is the share
//! of the area's pixels that are covered.
//!
//! The pixels are found a row at a time. A row of pixel centres crosses the area's rings at points that
//! pair up into the stretches inside it. A stitch's thread is a capsule, the stitch widened by half the
//! thread on either side and rounded at both ends, and it crosses the row in one stretch: where the row
//! passes through either end's disc or the band between them. So a stitch costs a step for each row it
//! crosses and each pixel it covers, never a distance from every pixel.
//!
//! **Edge coverage** is how much of an edge where 2 shapes meet the thread covers once it has pulled the
//! stitches in. Thread pulls a fill's rows in from the edges of their shape, so 2 shapes sewn up to where
//! they meet show fabric between them (`docs/src/design/image-to-embroidery.md` › Neighbours). The pull
//! moves the needle holes where the thread turns: each top stitch is shortened by the pull at an end where
//! the stitch sewn next to it, or before it, doesn't run straight on, within [`STRAIGHT_ON`]. So a row is
//! pulled in at its ends, and its needle holes between them stay. A stitch that the pull shortens to
//! nothing is pulled away. The edge is points a [`PIXEL`] apart along it, each covered when it lies within
//! half of [`THREAD`] of a stitch so shortened. Underlay and locks are left out, as they are meant to be
//! covered. Without pull it is the share of the edge's points that [`coverage`] would count covered by the
//! top stitches.

use std::collections::BTreeMap;

use stitchcraft_core::{Point, Rect};
use stitchcraft_engine::normalize::region::Region;
use stitchcraft_engine::normalize::stroke::distance_to_segment;
use stitchcraft_plan::{Role, StitchPlan};

/// The width of a thread, in millimetres.
pub const THREAD: f64 = 0.4;
/// The side of a pixel, in millimetres.
pub const PIXEL: f64 = 0.05;
/// Stitches that turn by less than this, in degrees, run straight on, as a row's stitches do: the pull
/// doesn't move the needle hole between them.
pub const STRAIGHT_ON: f64 = 10.0;

/// The share of `region` that the thread of `plan`'s stitches covers: 1 for a region without a pixel.
pub fn coverage(region: &Region, plan: &StitchPlan) -> f64 {
    let stitches: Vec<[Point; 2]> = plan.sewn_stitches().iter().map(|stitch| [stitch.from, stitch.to]).collect();
    covered_share(region, &stitches, THREAD / 2.0, PIXEL)
}

/// The share of `edges`, polylines where 2 shapes meet, that the thread of `plan`'s top stitches covers once
/// pulled in by `pull` millimetres where the thread turns: 1 for edges of no length.
pub fn edge_coverage(edges: &[Vec<Point>], plan: &StitchPlan, pull: f64) -> f64 {
    let stitches: Vec<[Point; 2]> =
        plan.sewn_stitches().iter().filter(|stitch| stitch.role == Role::Top).map(|stitch| [stitch.from, stitch.to]).collect();
    edge_share(edges, &stitches, pull, THREAD / 2.0, PIXEL)
}

/// The share of the points `step` apart along `edges` that lie within `reach` of one of `stitches`, in
/// sewing order, pulled in by `pull` where the thread turns: 1 for edges of no length.
fn edge_share(edges: &[Vec<Point>], stitches: &[[Point; 2]], pull: f64, reach: f64, step: f64) -> f64 {
    let pulled = pulled_in(stitches, pull);
    let near = Cells::of(&pulled, reach);
    let points = along(edges, step);
    if points.is_empty() {
        return 1.0;
    }
    let covered =
        points.iter().filter(|&&q| near.around(q).any(|i| pulled.get(i).is_some_and(|&[a, b]| distance_to_segment(q, a, b) <= reach))).count();
    covered as f64 / points.len() as f64
}

/// `stitches`, in sewing order, each shortened by `pull` at an end where the stitch before or after it
/// doesn't run straight on from it; those shortened to nothing left out.
fn pulled_in(stitches: &[[Point; 2]], pull: f64) -> Vec<[Point; 2]> {
    let straight_on = |first: &[Point; 2], next: &[Point; 2]| {
        let (u, v) = ((first[1].x() - first[0].x(), first[1].y() - first[0].y()), (next[1].x() - next[0].x(), next[1].y() - next[0].y()));
        let lengths = stitchcraft_core::math::hypot(u.0, u.1) * stitchcraft_core::math::hypot(v.0, v.1);
        first[1] == next[0] && lengths > 0.0 && (u.0 * v.0 + u.1 * v.1) / lengths >= stitchcraft_core::math::cos(STRAIGHT_ON.to_radians())
    };
    let mut pulled = Vec::with_capacity(stitches.len());
    for (i, stitch) in stitches.iter().enumerate() {
        let before = i.checked_sub(1).and_then(|j| stitches.get(j)).is_some_and(|before| straight_on(before, stitch));
        let after = stitches.get(i + 1).is_some_and(|after| straight_on(stitch, after));
        let [a, b] = *stitch;
        let (dx, dy) = (b.x() - a.x(), b.y() - a.y());
        let length = stitchcraft_core::math::hypot(dx, dy);
        let (from, to) = (if before { 0.0 } else { pull }, if after { 0.0 } else { pull });
        if length <= from + to {
            continue;
        }
        let at = |t: f64| Point::new(a.x() + dx / length * t, a.y() + dy / length * t).ok();
        if let (Some(start), Some(end)) = (at(from), at(length - to)) {
            pulled.push([start, end]);
        }
    }
    pulled
}

/// Points `step` apart along each of `polylines`, from its start, and its end.
fn along(polylines: &[Vec<Point>], step: f64) -> Vec<Point> {
    let mut points = Vec::new();
    for line in polylines {
        for pair in line.windows(2) {
            let [a, b] = [pair[0], pair[1]];
            let length = stitchcraft_core::math::hypot(b.x() - a.x(), b.y() - a.y());
            let steps = index((length / step).ceil());
            for k in 0..steps {
                let t = k as f64 * step / length;
                points.extend(Point::new(a.x() + t * (b.x() - a.x()), a.y() + t * (b.y() - a.y())).ok());
            }
        }
        points.extend(line.last().copied());
    }
    points
}

/// Stitches by the square millimetres they come within reach of, so that a point is measured against the
/// stitches near it only.
struct Cells {
    cells: BTreeMap<(i64, i64), Vec<usize>>,
}

impl Cells {
    /// `stitches` by the cells their thread, `reach` wide either side, touches.
    fn of(stitches: &[[Point; 2]], reach: f64) -> Cells {
        let mut cells: BTreeMap<(i64, i64), Vec<usize>> = BTreeMap::new();
        for (i, &[a, b]) in stitches.iter().enumerate() {
            let (x0, x1) = (cell(a.x().min(b.x()) - reach), cell(a.x().max(b.x()) + reach));
            let (y0, y1) = (cell(a.y().min(b.y()) - reach), cell(a.y().max(b.y()) + reach));
            for x in x0..=x1 {
                for y in y0..=y1 {
                    cells.entry((x, y)).or_default().push(i);
                }
            }
        }
        Cells { cells }
    }

    /// The stitches whose thread may reach `q`.
    fn around(&self, q: Point) -> impl Iterator<Item = usize> + '_ {
        self.cells.get(&(cell(q.x()), cell(q.y()))).into_iter().flatten().copied()
    }
}

/// The square millimetre `value` lies in, along one axis.
#[allow(clippy::cast_possible_truncation)] // a saturating cast; a design lies within 10 m
fn cell(value: f64) -> i64 {
    value.floor() as i64
}

/// The share of `region`'s pixels, `pixel` wide, whose centres lie within `reach` of one of `stitches`: 1
/// for a region without a pixel.
fn covered_share(region: &Region, stitches: &[[Point; 2]], reach: f64, pixel: f64) -> f64 {
    let Some(bounds) = Rect::around(region.parts.iter().flat_map(|part| part.outline.iter().copied())) else { return 1.0 };
    let grid = Grid::new(bounds, pixel);
    let mut covered = vec![false; grid.columns * grid.rows];
    for &[a, b] in stitches {
        for row in grid.rows_within(a.y().min(b.y()) - reach, a.y().max(b.y()) + reach) {
            let Some((low, high)) = crossing(a, b, reach, grid.y(row)) else { continue };
            for column in grid.columns_within(low, high) {
                covered[row * grid.columns + column] = true;
            }
        }
    }
    let rings: Vec<&[Point]> =
        region.parts.iter().flat_map(|part| std::iter::once(part.outline.as_slice()).chain(part.holes.iter().map(Vec::as_slice))).collect();
    let (mut inside, mut hit) = (0_usize, 0_usize);
    for row in 0..grid.rows {
        for (from, to) in spans(&rings, grid.y(row)) {
            // Half-open, so that 2 stretches meeting at a pixel's centre count it once.
            for column in grid.columns_within(from, to).filter(|&column| grid.x(column) < to) {
                inside += 1;
                hit += usize::from(covered[row * grid.columns + column]);
            }
        }
    }
    if inside == 0 { 1.0 } else { hit as f64 / inside as f64 }
}

/// The pixels over a region's bounding box: the first pixel's centre half a pixel in from its corner where
/// x and y are least.
struct Grid {
    first: Point,
    pixel: f64,
    columns: usize,
    rows: usize,
}

impl Grid {
    fn new(bounds: Rect, pixel: f64) -> Grid {
        let count = |extent: f64| index((extent / pixel).ceil());
        let first = Point::new(bounds.min().x() + pixel / 2.0, bounds.min().y() + pixel / 2.0).unwrap_or(bounds.min());
        Grid { first, pixel, columns: count(bounds.width()), rows: count(bounds.height()) }
    }

    /// The centre of column `column`, along x.
    fn x(&self, column: usize) -> f64 {
        self.first.x() + column as f64 * self.pixel
    }

    /// The centre of row `row`, along y.
    fn y(&self, row: usize) -> f64 {
        self.first.y() + row as f64 * self.pixel
    }

    /// The columns whose centres lie from `low` to `high`, both included.
    fn columns_within(&self, low: f64, high: f64) -> std::ops::Range<usize> {
        within(low - self.first.x(), high - self.first.x(), self.pixel, self.columns)
    }

    /// The rows whose centres lie from `low` to `high`, both included.
    fn rows_within(&self, low: f64, high: f64) -> std::ops::Range<usize> {
        within(low - self.first.y(), high - self.first.y(), self.pixel, self.rows)
    }
}

/// The indices of the centres, `pixel` apart from 0 and `count` of them, that lie from `low` to `high`,
/// both included.
fn within(low: f64, high: f64, pixel: f64, count: usize) -> std::ops::Range<usize> {
    let first = index((low / pixel).ceil()).min(count);
    let past = index((high / pixel).floor() + 1.0).min(count);
    first..past.max(first)
}

/// `value`, a whole number, as an index: 0 below 0 and for NaN, saturated far above.
#[allow(clippy::cast_possible_truncation)] // a saturating cast, clamped by callers
fn index(value: f64) -> usize {
    value.max(0.0) as usize
}

/// The stretches inside `rings` along the line at height `y`, by the even-odd rule: from each crossing of
/// a ring to the next, every other one. A side crosses the line where one end lies at or below it and the
/// other above, so a vertex on the line counts once.
fn spans(rings: &[&[Point]], y: f64) -> Vec<(f64, f64)> {
    let mut crossings: Vec<f64> = rings
        .iter()
        .flat_map(|ring| ring.windows(2))
        .filter_map(|side| {
            let &[a, b] = side else { return None };
            ((a.y() <= y) != (b.y() <= y)).then(|| a.x() + (y - a.y()) * (b.x() - a.x()) / (b.y() - a.y()))
        })
        .collect();
    crossings.sort_by(f64::total_cmp);
    crossings.as_chunks::<2>().0.iter().map(|&[a, b]| (a, b)).collect()
}

/// The stretch of the line at height `y` that lies within `reach` of the stitch from `a` to `b`, from its
/// least x to its greatest; `None` where the line passes farther away. The thread is the union of the
/// discs round both ends and the band between them, and the line crosses each in a stretch, or misses it;
/// the thread being convex, the stretches it crosses join into one.
fn crossing(a: Point, b: Point, reach: f64, y: f64) -> Option<(f64, f64)> {
    let pieces = [disc(a, reach, y), disc(b, reach, y), band(a, b, reach, y)];
    pieces.into_iter().flatten().reduce(|(low, high), (l, h)| (low.min(l), high.max(h)))
}

/// Where the line at height `y` crosses the disc of radius `reach` round `centre`.
fn disc(centre: Point, reach: f64, y: f64) -> Option<(f64, f64)> {
    let rise = y - centre.y();
    let left = reach * reach - rise * rise;
    (left >= 0.0).then(|| (centre.x() - left.sqrt(), centre.x() + left.sqrt()))
}

/// Where the line at height `y` crosses the band of points within `reach` of the stitch from `a` to `b`
/// whose foot on its line falls between its ends; `None` for a stitch of no length, which is its discs.
fn band(a: Point, b: Point, reach: f64, y: f64) -> Option<(f64, f64)> {
    let (dx, dy) = (b.x() - a.x(), b.y() - a.y());
    let squared = dx * dx + dy * dy;
    if squared == 0.0 {
        return None;
    }
    // At a point (x, y): how far along the stitch its foot lies, times the stitch's length squared, from 0
    // to that square; and how far it lies from the stitch's line, times its length, at most `reach` times
    // that length. Each is linear in x.
    let along = between(dx, (y - a.y()) * dy - a.x() * dx, 0.0, squared)?;
    let reach = reach * squared.sqrt();
    let aside = between(-dy, (y - a.y()) * dx + a.x() * dy, -reach, reach)?;
    let (low, high) = (along.0.max(aside.0), along.1.min(aside.1));
    (low <= high).then_some((low, high))
}

/// The x where `slope * x + offset` lies from `low` to `high`: every x where the slope is 0 and the offset
/// lies between them, none where it does not.
fn between(slope: f64, offset: f64, low: f64, high: f64) -> Option<(f64, f64)> {
    if slope == 0.0 {
        return (low <= offset && offset <= high).then_some((f64::NEG_INFINITY, f64::INFINITY));
    }
    let (one, other) = ((low - offset) / slope, (high - offset) / slope);
    Some((one.min(other), one.max(other)))
}

#[cfg(test)]
mod tests {
    use stitchcraft_core::rng::SplitMix64;
    use stitchcraft_engine::normalize::region::Polygon;
    use stitchcraft_engine::normalize::stroke::distance_to_segment;

    use super::*;
    use crate::designs::p;

    /// A part: `outline` and `holes`, each closed.
    fn part(outline: &[(f64, f64)], holes: &[&[(f64, f64)]]) -> Polygon {
        let ring = |points: &[(f64, f64)]| points.iter().chain(points.first()).map(|&(x, y)| p(x, y)).collect();
        Polygon { outline: ring(outline), holes: holes.iter().map(|hole| ring(hole)).collect() }
    }

    /// A 10 × 10 square from the origin.
    fn square() -> Polygon {
        part(&[(0.0, 0.0), (0.0, 10.0), (10.0, 10.0), (10.0, 0.0)], &[])
    }

    #[test]
    fn a_thread_along_the_middle_covers_the_rows_within_its_reach() {
        // Reaching 2.5 either side of y = 5: the rows of centres from 2.525 to 7.475, 100 of 200.
        let region = Region { parts: vec![square()] };
        let middle = [[p(-1.0, 5.0), p(11.0, 5.0)]];
        assert_eq!(covered_share(&region, &middle, 2.5, PIXEL), 0.5);
        // Ending at x = 5, its disc at the end reaches to 7.5 along its line.
        let half = covered_share(&region, &[[p(-1.0, 5.0), p(5.0, 5.0)]], 2.5, PIXEL);
        assert!((0.25..0.5).contains(&half), "{half}");
        // A stitch of no length is its disc: π 2.5² of the square's 100, to within the pixels' steps.
        let dot = covered_share(&region, &[[p(5.0, 5.0), p(5.0, 5.0)]], 2.5, PIXEL);
        assert!((dot - std::f64::consts::PI * 6.25 / 100.0).abs() < 0.002, "{dot}");
    }

    #[test]
    fn a_hole_is_no_part_of_the_region_and_neither_is_the_outside() {
        // A 4 × 4 hole in the middle: covering it alone covers nothing, and covering the rest covers all.
        let holed =
            Region { parts: vec![part(&[(0.0, 0.0), (0.0, 10.0), (10.0, 10.0), (10.0, 0.0)], &[&[(3.0, 3.0), (7.0, 3.0), (7.0, 7.0), (3.0, 7.0)]])] };
        let hole = [[p(3.5, 5.0), p(6.5, 5.0)]];
        assert_eq!(covered_share(&holed, &hole, 0.4, PIXEL), 0.0);
        let rows: Vec<[Point; 2]> = (0..=20).map(|k| [p(-1.0, 0.5 * f64::from(k)), p(11.0, 0.5 * f64::from(k))]).collect();
        assert_eq!(covered_share(&holed, &rows, 0.26, PIXEL), 1.0);
        // Nothing covers a stitch beside the region.
        assert_eq!(covered_share(&holed, &[[p(20.0, 0.0), p(20.0, 10.0)]], 5.0, PIXEL), 0.0);
        // 2 parts: covering one of 2 equal squares covers half.
        let two = Region { parts: vec![square(), part(&[(20.0, 0.0), (20.0, 10.0), (30.0, 10.0), (30.0, 0.0)], &[])] };
        assert_eq!(covered_share(&two, &rows, 0.26, PIXEL), 0.5);
    }

    #[test]
    fn a_region_without_a_pixel_is_covered() {
        assert_eq!(covered_share(&Region::default(), &[], 0.2, PIXEL), 1.0);
        assert_eq!(covered_share(&Region { parts: vec![part(&[(0.0, 0.0), (0.0, 0.01), (0.01, 0.0)], &[])] }, &[], 0.2, PIXEL), 1.0);
    }

    #[test]
    fn a_stitch_covers_on_a_row_the_points_within_reach_of_it() {
        // Random stitches, rows and points, against the distance itself: a point lies in the stretch exactly
        // where it lies within reach, but for points closer to the stretch's ends than rounding can tell.
        let mut rng = SplitMix64::new(7);
        let mut value = |low: f64, high: f64| rng.next_in(low, high);
        let mut checked = 0;
        for case in 0..2000 {
            let a = p(value(-3.0, 3.0), value(-3.0, 3.0));
            // Every 5th stitch is level, every 7th upright, and every 11th of no length.
            let b = match case {
                c if c % 11 == 0 => a,
                c if c % 7 == 0 => p(a.x(), value(-3.0, 3.0)),
                c if c % 5 == 0 => p(value(-3.0, 3.0), a.y()),
                _ => p(value(-3.0, 3.0), value(-3.0, 3.0)),
            };
            let (reach, y) = (value(0.05, 1.0), value(-4.5, 4.5));
            let stretch = crossing(a, b, reach, y);
            for _ in 0..10 {
                let x = value(-5.0, 5.0);
                if stretch.is_some_and(|(low, high)| (x - low).abs() < 1e-9 || (x - high).abs() < 1e-9) {
                    continue;
                }
                let near = distance_to_segment(p(x, y), a, b) <= reach;
                assert_eq!(stretch.is_some_and(|(low, high)| low <= x && x <= high), near, "{a:?} {b:?} reach {reach} at ({x}, {y})");
                checked += 1;
            }
        }
        assert!(checked > 19_000, "{checked}");
    }

    #[test]
    fn a_line_crosses_the_even_odd_stretches_once_at_a_vertex() {
        // A diamond, its left and right vertices at y = 5: the line through them is one stretch.
        let diamond = part(&[(0.0, 5.0), (5.0, 10.0), (10.0, 5.0), (5.0, 0.0)], &[]);
        assert_eq!(spans(&[diamond.outline.as_slice()], 5.0), [(0.0, 10.0)]);
        // Through its vertex of greatest y, nothing: no side reaches past it.
        assert_eq!(spans(&[diamond.outline.as_slice()], 10.0), []);
    }

    #[test]
    fn an_edge_is_covered_where_the_rows_reach_it_once_pulled_in() {
        // Rows 0.25 mm apart from either side end at x = 10, the edge between 2 shapes.
        let rows = |reach_past: f64| -> Vec<[Point; 2]> {
            (0..=40)
                .flat_map(|k| {
                    let y = 0.25 * f64::from(k);
                    [[p(0.0, y), p(10.0 + reach_past, y)], [p(20.0, y), p(10.0, y)]]
                })
                .collect()
        };
        let edge = [vec![p(10.0, 0.0), p(10.0, 10.0)]];
        // Without pull, the thread round the rows' ends covers the edge.
        assert_eq!(edge_share(&edge, &rows(0.0), 0.0, 0.2, PIXEL), 1.0);
        // Pulled in by 0.4 mm, both sides' rows leave the edge bare.
        assert_eq!(edge_share(&edge, &rows(0.0), 0.4, 0.2, PIXEL), 0.0);
        // Rows that reach 0.4 mm past it cover it again.
        assert_eq!(edge_share(&edge, &rows(0.4), 0.4, 0.2, PIXEL), 1.0);
        // A stitch alone no longer than twice the pull is pulled away; an edge of no length is covered.
        assert_eq!(edge_share(&edge, &[[p(9.7, 5.0), p(10.3, 5.0)]], 0.4, 0.2, PIXEL), 0.0);
        assert_eq!(edge_share(&[], &rows(0.0), 0.4, 0.2, PIXEL), 1.0);
        // The edge's points: one each 0.05 mm, and its end.
        assert_eq!(along(&edge, PIXEL).len(), 201);
    }

    #[test]
    fn the_pull_moves_the_needle_holes_where_the_thread_turns_only() {
        // A row across the edge in 3 stitches, then a stitch back: pulled in at the row's start and where it
        // turns back, and nowhere between.
        let row = [[p(0.0, 0.0), p(5.0, 0.0)], [p(5.0, 0.0), p(9.9, 0.0)], [p(9.9, 0.0), p(12.0, 0.0)], [p(12.0, 0.0), p(8.0, 0.25)]];
        let pulled = pulled_in(&row, 0.4);
        assert_eq!(&pulled[..3], [[p(0.4, 0.0), p(5.0, 0.0)], [p(5.0, 0.0), p(9.9, 0.0)], [p(9.9, 0.0), p(11.6, 0.0)]]);
        // The stitch back, 4 mm long, pulled in at both ends.
        let [from, to] = pulled[3];
        assert!((from.x() - 11.6).abs() < 0.01 && (to.x() - 8.4).abs() < 0.01, "{from:?} {to:?}");
        // So the short stitch across the edge at x = 10 stays, and covers it.
        assert_eq!(edge_share(&[vec![p(10.0, -0.1), p(10.0, 0.1)]], &row, 0.4, 0.2, PIXEL), 1.0);
        // A turn of less than 10° runs straight on; a turn of more doesn't.
        let bent = |degrees: f64| {
            let (sin, cos) = stitchcraft_core::math::sin_cos(degrees.to_radians());
            [[p(0.0, 0.0), p(1.0, 0.0)], [p(1.0, 0.0), p(1.0 + cos, sin)]]
        };
        assert_eq!(pulled_in(&bent(9.0), 0.4)[0], [p(0.4, 0.0), p(1.0, 0.0)]);
        assert_eq!(pulled_in(&bent(11.0), 0.4)[0], [p(0.4, 0.0), p(0.6, 0.0)]);
    }

    #[test]
    fn a_grid_counts_the_centres_from_and_to_its_bounds() {
        let grid = Grid::new(Rect::around([p(1.0, 2.0), p(2.0, 2.5)]).unwrap(), 0.1);
        assert_eq!((grid.columns, grid.rows), (10, 5));
        assert!((grid.x(0) - 1.05).abs() < 1e-12 && (grid.y(4) - 2.45).abs() < 1e-12);
        // The centres 1.25, 1.35 and 1.45, and none past the grid.
        assert_eq!(grid.columns_within(1.2, 1.5), 2..5);
        assert_eq!(grid.columns_within(1.9, 7.0), 9..10);
        assert_eq!(grid.rows_within(-3.0, 1.0), 0..0);
        assert_eq!(grid.rows_within(2.3, 2.2), 3..3);
    }
}
