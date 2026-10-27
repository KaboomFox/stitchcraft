//! The narrow parts of an automatic area: each sewn as a running stitch along its centre line, tripled from
//! 0.5 mm wide (`REQ-AUTO-013` to `REQ-AUTO-015`).
//!
//! Design: `docs/src/design/image-to-embroidery.md` › Stitch types by width, and
//! `docs/src/design/algorithms/centre-lines.md` › Width along the line. A line traced from a picture is an
//! area, and filled, an area that narrow sews rows no longer than it is wide wherever they cross it. So a
//! part of an area that is narrower than the design's narrowest satin stroke everywhere sews as a running
//! stitch along its middle, as a stroke that narrow does:
//!
//! - **Narrow.** A part is narrower than a width everywhere when, shrunk by half that width, it leaves no
//!   area: no circle that wide fits inside it. It is shrunk with the polygon buffer that sews a fill's
//!   inset underlay (`ops::buffered`), by half of the width taken half a micrometre wider, so that a part as
//!   wide as the width, give or take a rounding error, is narrow, as a stroke is when its width is compared
//!   to the micrometre. A part whose area is more than its perimeter times that half, plus π times its
//!   square for each hole past the first, holds a wider circle somewhere: no part that narrow covers more.
//!   It isn't shrunk, which would only confirm it at the cost of its whole outline.
//! - **Not a speck.** A part smaller than [`SMALLEST_AREA`] is a speck, which stays in the area to be merged
//!   into a neighbour (`REQ-AUTO-008`).
//! - **A line.** Its centre lines ([`strips`]) come from its triangulation, its edges cut into pieces no
//!   longer than it is wide on average, twice its area over its perimeter, or [`FINEST`] if that is longer.
//!   A part whose centre lines have no length, a dot, stays in the area.
//! - **Its width** is that of the largest circle that fits inside it centred on its centre lines, twice the
//!   distance from their points to its outline and holes, averaged along them. It says whether the part is
//!   a triple run ([`super::tripled`]). The part's edges are put in the cells of a grid as fine as the
//!   pieces it is cut into ([`Edges`]), and each point's nearest edge is looked for in the cells round it,
//!   ring by ring, until no edge farther out could be nearer: a narrow part's edges lie a cell or so from
//!   its middle, so measuring each point costs a few units of work, not one for each edge of the part.

use std::collections::BTreeMap;

use stitchcraft_core::{Code, Diagnostic, ElementId, Exhausted, Meter, Mm, Point};

use super::{SMALLEST_AREA, no_area};
use crate::normalize::region::Polygon;
use crate::normalize::region::ops::{self, area_of};
use crate::normalize::strips::{self, Line};
use crate::normalize::stroke::{distance_to_segment, segments};

/// The shortest piece a narrow part's edges are cut into for its triangulation, in millimetres: a part
/// thinner than this on average is cut no finer, which bounds its points by its perimeter.
const FINEST: f64 = 0.1;

/// A narrow part of an area.
#[derive(Clone, Debug, PartialEq)]
pub(super) struct Narrow {
    /// The part.
    pub(super) part: Polygon,
    /// Its centre lines, in the order they are sewn.
    pub(super) lines: Vec<Line>,
    /// Its width along its centre lines, on average.
    pub(super) width: Mm,
}

/// An area's parts, sorted by their width.
#[derive(Clone, Debug, Default, PartialEq)]
pub(super) struct Parts {
    /// The parts that stay the area's fill: its broad parts, its specks and its dots.
    pub(super) filled: Vec<Polygon>,
    /// Its narrow parts, in the order of its parts.
    pub(super) narrow: Vec<Narrow>,
    /// `SC-E0009` for each part whose width or centre line could not be found, a bug; the part stays in the
    /// fill.
    pub(super) said: Vec<Diagnostic>,
}

/// `parts`, the automatic area `id`'s, sorted by their width: those narrower than `narrowest` everywhere,
/// with their centre lines, unless they are specks or dots, and the rest. Finding a part's width and centre
/// line is part of the work of digitizing the area, and costs `meter`'s.
pub(super) fn sorted(id: &ElementId, parts: Vec<Polygon>, narrowest: Mm, meter: &mut Meter) -> Result<Parts, Exhausted> {
    let mut sorted = Parts::default();
    for part in parts {
        meter.charge(1)?;
        match lines(&part, narrowest, meter)? {
            Found::Lines(lines) => match width(&part, &lines, meter)? {
                Some(width) => sorted.narrow.push(Narrow { part, lines, width }),
                None => sorted.filled.push(part),
            },
            Found::Filled => sorted.filled.push(part),
            Found::Broken(failed) => {
                sorted.said.push(broken(id, failed));
                sorted.filled.push(part);
            }
        }
    }
    Ok(sorted)
}

/// What becomes of a part.
enum Found {
    /// It is narrow, and sews along these centre lines, unless they have no length.
    Lines(Vec<Line>),
    /// It stays in the fill: it is broad somewhere, or a speck.
    Filled,
    /// What could not be found of it, and why, a bug.
    Broken(&'static str),
}

/// The centre lines of `part` when it is narrower than `narrowest` everywhere and no speck, with `meter`'s
/// work.
fn lines(part: &Polygon, narrowest: Mm, meter: &mut Meter) -> Result<Found, Exhausted> {
    let area = part.area();
    if area < SMALLEST_AREA {
        return Ok(Found::Filled);
    }
    let half = (narrowest.get() + 0.0005) / 2.0;
    let holes = part.holes.len().saturating_sub(1) as f64;
    if area > perimeter(part) * half + std::f64::consts::PI * holes * half * half {
        return Ok(Found::Filled);
    }
    let Ok(shrunk) = ops::buffered(std::slice::from_ref(part), -half, meter)? else { return Ok(Found::Broken(UNJOINED)) };
    if !no_area(area_of(&shrunk)) {
        return Ok(Found::Filled);
    }
    match strips::centre_lines(part, piece(part), meter)? {
        Ok(lines) => Ok(Found::Lines(lines)),
        Err(_) => Ok(Found::Broken(LOST)),
    }
}

/// How long the pieces of `part`'s edges are, for its triangulation and for the grid its width is measured
/// with: as long as the part is wide on average, twice its area over its perimeter, or [`FINEST`] if that is
/// longer.
fn piece(part: &Polygon) -> f64 {
    (2.0 * part.area() / perimeter(part)).max(FINEST)
}

/// The length of `part`'s outline and holes, in millimetres.
fn perimeter(part: &Polygon) -> f64 {
    part.rings().flat_map(segments).map(|(a, b)| a.distance(b)).sum()
}

/// The width of `part` along `lines`, its centre lines: the diameter of the largest circle that fits inside it
/// centred on each of their points, twice the point's distance from the nearest edge of its outline and
/// holes, averaged along the lines. `None` for lines of no length, a dot's. A unit of `meter`'s work for each
/// point and edge.
fn width(part: &Polygon, lines: &[Line], meter: &mut Meter) -> Result<Option<Mm>, Exhausted> {
    let edges = Edges::of(part, piece(part), meter)?;
    let (mut sum, mut length) = (0.0, 0.0);
    for line in lines {
        let mut across = Vec::with_capacity(line.points.len());
        for &p in &line.points {
            across.push(2.0 * edges.nearest(p, meter)?);
        }
        let closing = line.closed.then(|| (line.points.len().saturating_sub(1), 0));
        for (i, j) in (1..line.points.len()).map(|j| (j - 1, j)).chain(closing) {
            let (Some(a), Some(b), Some(wa), Some(wb)) = (line.points.get(i), line.points.get(j), across.get(i), across.get(j)) else { continue };
            let piece = a.distance(*b);
            sum += piece * (wa + wb) / 2.0;
            length += piece;
        }
    }
    Ok(if length > 0.0 { Mm::new(sum / length).ok() } else { None })
}

/// The edges of a part in the cells of a square grid, each cut into pieces no longer than a cell, so that
/// the edge nearest a point is found among the cells round it.
struct Edges {
    /// A cell's side, in millimetres.
    cell: f64,
    /// The pieces of edges that reach into each cell, by the cell's column and row.
    cells: BTreeMap<(i64, i64), Vec<(Point, Point)>>,
    /// The least and greatest column and row of a cell with a piece in it.
    span: [i64; 4],
}

impl Edges {
    /// The edges of `part`'s outline and holes in cells `cell` millimetres wide, with `meter`'s work: a unit
    /// for each piece and each cell it reaches.
    fn of(part: &Polygon, cell: f64, meter: &mut Meter) -> Result<Edges, Exhausted> {
        let mut edges = Edges { cell, cells: BTreeMap::new(), span: [i64::MAX, i64::MAX, i64::MIN, i64::MIN] };
        for (a, b) in part.rings().flat_map(segments) {
            let pieces = (a.distance(b) / cell).ceil().max(1.0);
            let mut k = 0.0;
            while k < pieces {
                meter.charge(1)?;
                let (from, to) = (a.lerp(b, k / pieces), a.lerp(b, (k + 1.0) / pieces));
                let (x0, x1) = (edges.column(from.x().min(to.x())), edges.column(from.x().max(to.x())));
                let (y0, y1) = (edges.column(from.y().min(to.y())), edges.column(from.y().max(to.y())));
                for x in x0..=x1 {
                    for y in y0..=y1 {
                        meter.charge(1)?;
                        edges.cells.entry((x, y)).or_default().push((from, to));
                    }
                }
                let [low_x, low_y, high_x, high_y] = &mut edges.span;
                (*low_x, *low_y, *high_x, *high_y) = ((*low_x).min(x0), (*low_y).min(y0), (*high_x).max(x1), (*high_y).max(y1));
                k += 1.0;
            }
        }
        Ok(edges)
    }

    /// The column, or row, of the cells that `v` millimetres lies in. A part lies within metres of the
    /// origin, and a cell is a tenth of a millimetre or more, so the count fits.
    #[allow(clippy::cast_possible_truncation)]
    fn column(&self, v: f64) -> i64 {
        (v / self.cell).floor() as i64
    }

    /// The distance from `p` to the nearest edge, with `meter`'s work: a unit for each cell and each piece
    /// looked at. The cells are looked at in rings round `p`'s, and once the nearest piece found lies no
    /// farther than the cells beyond the ring could be, they aren't.
    fn nearest(&self, p: Point, meter: &mut Meter) -> Result<f64, Exhausted> {
        if self.cells.is_empty() {
            return Ok(f64::INFINITY);
        }
        let (x, y) = (self.column(p.x()), self.column(p.y()));
        let [low_x, low_y, high_x, high_y] = self.span;
        let apart = |a: i64, b: i64| a.saturating_sub(b).saturating_abs();
        let last = apart(x, low_x).max(apart(high_x, x)).max(apart(y, low_y)).max(apart(high_y, y));
        let mut best = f64::INFINITY;
        let mut ring = 0;
        while ring <= last {
            for (i, j) in ring_cells(x, y, ring) {
                meter.charge(1)?;
                for &(a, b) in self.cells.get(&(i, j)).into_iter().flatten() {
                    meter.charge(1)?;
                    best = best.min(distance_to_segment(p, a, b));
                }
            }
            // Every cell beyond this ring lies at least `ring` cells from p.
            if best <= ring as f64 * self.cell {
                break;
            }
            ring += 1;
        }
        Ok(best)
    }
}

/// The cells `ring` cells round the cell in column `x` and row `y`: that cell itself for ring 0.
fn ring_cells(x: i64, y: i64, ring: i64) -> Vec<(i64, i64)> {
    if ring == 0 {
        return vec![(x, y)];
    }
    let rows = (x - ring..=x + ring).flat_map(|i| [(i, y - ring), (i, y + ring)]);
    let columns = (y - ring + 1..y + ring).flat_map(|j| [(x - ring, j), (x + ring, j)]);
    rows.chain(columns).collect()
}

/// What is said of a part whose shrinking, to tell whether it is narrow, couldn't be joined.
const UNJOINED: &str = "Finding how wide a part of this area is, the edges of a region could not be joined into rings";

/// What is said of a narrow part whose triangulation lost its shape.
const LOST: &str = "Finding the centre line of a narrow part of this area, its triangulation lost its shape";

/// `SC-E0009` for a part of the automatic area `id` that `failed` says what failed of, a bug: the part stays
/// in the area's fill.
fn broken(id: &ElementId, failed: &str) -> Diagnostic {
    let message = format!("{failed}, so the part is filled. This is a bug in StitchCraft.");
    Diagnostic::new(Code::InternalCheckFailed, message).with_element(id.clone())
}

#[cfg(test)]
mod tests {
    use stitchcraft_core::Budget;
    use stitchcraft_core::math::sin_cos;
    use stitchcraft_core::rng::SplitMix64;

    use super::*;
    use crate::normalize::region::geom;

    #[test]
    fn the_grid_finds_the_nearest_edge_as_measuring_every_edge_does() {
        // Star-shaped outlines with a star-shaped hole, and points in and around them: the distance found in
        // the cells round a point is the least over every edge, for cells of every size.
        let mut rng = SplitMix64::new(20_261_029);
        for case in 0..40 {
            let star = |rng: &mut SplitMix64, low: f64, high: f64, count: u64| -> Vec<Point> {
                let ring: Vec<Point> = (0..count)
                    .map(|i| {
                        let (sin, cos) = sin_cos(std::f64::consts::TAU * i as f64 / count as f64);
                        let r = rng.next_in(low, high);
                        Point::new(10.0 + r * cos, -5.0 + r * sin).unwrap()
                    })
                    .collect();
                geom::closed(&ring)
            };
            let count = 5 + rng.next_u64() % 30;
            let part = Polygon { outline: star(&mut rng, 6.0, 12.0, count), holes: vec![star(&mut rng, 1.0, 4.0, count)] };
            let cell = [0.1, 0.7, 3.0, 50.0][case % 4];
            let mut meter = Budget::DEFAULT.meter();
            let edges = Edges::of(&part, cell, &mut meter).unwrap();
            for _ in 0..50 {
                let p = Point::new(rng.next_in(-5.0, 25.0), rng.next_in(-20.0, 10.0)).unwrap();
                let every = part.rings().flat_map(segments).map(|(a, b)| distance_to_segment(p, a, b)).fold(f64::INFINITY, f64::min);
                let found = edges.nearest(p, &mut meter).unwrap();
                assert!((found - every).abs() <= 1e-12 * every.max(1.0), "{found} {every} at {p:?}, cell {cell}");
            }
        }
    }

    /// `n` points round a circle of radius `r` about (`x`, `y`), counter-clockwise in y-up axes from `turn`
    /// of a step past the x axis.
    fn circle(x: f64, y: f64, r: f64, n: usize, turn: f64) -> Vec<Point> {
        (0..n)
            .map(|i| {
                let (sin, cos) = sin_cos(std::f64::consts::TAU * (i as f64 + turn) / n as f64);
                Point::new(x + r * cos, y + r * sin).unwrap()
            })
            .collect()
    }

    /// A part with the outline `points`, wound clockwise in y-up axes as a region winds it.
    fn outline(points: &[(f64, f64)]) -> Polygon {
        let mut ring: Vec<Point> = points.iter().map(|&(x, y)| Point::new(x, y).unwrap()).collect();
        if geom::signed_area(&geom::closed(&ring)) > 0.0 {
            ring.reverse();
        }
        Polygon { outline: geom::closed(&ring), holes: Vec::new() }
    }

    #[test]
    fn each_kind_of_part_is_sorted_by_its_width() {
        // An area's parts, with a narrowest satin stroke of 1.5 mm:
        // - a square 10 mm across, broad by its area alone;
        // - a speck;
        // - a strip 0.6 mm wide with an end 2 mm square, which its area doesn't show broad and shrinking does;
        // - a strip 0.6 mm wide, narrow;
        // - a regular octagon 1.5 mm across its corners, a dot, whose caps meet;
        // - and a ring whose hole pokes out through an edge of its outline, 0.012 mm, which no region has. Its
        //   triangulation loses its shape, as a bug would make it.
        // Each but the strip stays in the fill, and SC-E0009 names the area of the last.
        let square = outline(&[(0.0, 10.0), (10.0, 10.0), (10.0, 20.0), (0.0, 20.0)]);
        let speck = outline(&[(30.0, 0.0), (30.5, 0.0), (30.5, 0.5), (30.0, 0.5)]);
        let ended = outline(&[(0.0, 0.0), (18.0, 0.0), (18.0, -0.7), (20.0, -0.7), (20.0, 1.3), (18.0, 1.3), (18.0, 0.6), (0.0, 0.6)]);
        let strip = outline(&[(0.0, 5.0), (20.0, 5.0), (20.0, 5.6), (0.0, 5.6)]);
        let mut corners = circle(40.0, 0.0, 0.75, 8, 0.5);
        corners.reverse();
        let dot = Polygon { outline: geom::closed(&corners), holes: Vec::new() };
        let mut corners = circle(50.0, 0.0, 1.0, 20, 0.5);
        corners.reverse();
        let ring = Polygon { outline: geom::closed(&corners), holes: vec![geom::closed(&circle(50.4, 0.0, 0.6, 40, 0.0))] };
        let area = ElementId::new("area").unwrap();
        let parts = vec![square.clone(), speck.clone(), ended.clone(), strip.clone(), dot.clone(), ring.clone()];
        let sorted = sorted(&area, parts, Mm::new(1.5).unwrap(), &mut Budget::DEFAULT.meter()).unwrap();
        assert_eq!(sorted.filled, [square, speck, ended, dot, ring]);
        let [Narrow { part, lines, width }] = sorted.narrow.as_slice() else { panic!("{:?}", sorted.narrow) };
        assert_eq!((part, lines.len()), (&strip, 1));
        assert!((width.get() - 0.6).abs() < 1e-9, "{width:?}");
        let [said] = sorted.said.as_slice() else { panic!("{:?}", sorted.said) };
        assert_eq!((said.code, said.element.as_ref()), (Code::InternalCheckFailed, Some(&area)));
        assert_eq!(said.message, format!("{LOST}, so the part is filled. This is a bug in StitchCraft."));
    }

    #[test]
    fn a_part_without_edges_is_nowhere_near() {
        let part = Polygon { outline: Vec::new(), holes: Vec::new() };
        let edges = Edges::of(&part, 1.0, &mut Budget::DEFAULT.meter()).unwrap();
        assert_eq!(edges.nearest(Point::ORIGIN, &mut Budget::DEFAULT.meter()), Ok(f64::INFINITY));
    }
}
