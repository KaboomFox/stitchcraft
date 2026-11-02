//! A meander's network over one part (`REQ-FILL-MEA-002`, `docs/src/design/algorithms/fills.md` › Meander
//! fill › The network): the tile's lines that lie in the part, those with a free end taken away, and the
//! pieces left joined.
//!
//! **Points by their place.** A point of the network is named by its cell and its corner, never by where
//! it lies, so 2 lines that meet at a corner meet exactly, whatever the arithmetic that placed them.
//! Ink/Stitch names its points by where they lie, rounded to whole CSS pixels (`DEV-FILL-020`).
//!
//! **Inside.** A line lies in the part when no point of it is outside and some point is inside, as shapely's
//! `contains` decides it: a line may touch the outline or a hole, but not cross it. It is cut where a
//! corner of the part's rings lies on it, and each piece's middle must lie in the part. A line that
//! crosses a side of the rings is out.

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use stitchcraft_core::exact::{self, Side};
use stitchcraft_core::math::sin_cos;
use stitchcraft_core::{Exhausted, Meter, Point};

use super::tiles::{Tile, offset, plus};
use crate::generators::tatami::rings::Rings;
use crate::normalize::region::Polygon;

/// How long a tile's side is at a scale of 100 %, in millimetres.
pub(crate) const SIDE_MM: f64 = 2.0;

/// How a tile is laid over a part: scaled across and down, then turned, in radians clockwise as drawn.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Placement {
    /// The scale across and down: 1 lays the tile's sides 2 mm long.
    pub scale: (f64, f64),
    /// The turn, in radians clockwise as drawn.
    pub angle: f64,
}

/// A part's network: its points, in the order of their places in the tiling, and each one's neighbours.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct Network {
    pub(crate) points: Vec<Point>,
    pub(crate) next: Vec<BTreeSet<usize>>,
}

impl Network {
    /// How many points it has.
    pub(crate) fn len(&self) -> usize {
        self.points.len()
    }

    /// Where point `i` lies.
    pub(crate) fn point(&self, i: usize) -> Point {
        self.points.get(i).copied().unwrap_or(Point::ORIGIN)
    }

    /// The neighbours of point `i`, in order.
    pub(crate) fn neighbours(&self, i: usize) -> impl Iterator<Item = usize> + '_ {
        self.next.get(i).into_iter().flatten().copied()
    }
}

/// A point of the tiling: its cell along each step, and its corner.
type Place = (i32, i32, usize);

/// The network of `tile` laid over `part`, whose rings `rings` measures, as `placement` lays it; `None`
/// when no line of it is left. One unit of `meter` for each cell, each line tested and each pair of
/// points measured between pieces.
pub(crate) fn network(part: &Polygon, rings: &Rings<'_>, tile: &Tile, placement: Placement, meter: &mut Meter) -> Result<Option<Network>, Exhausted> {
    let laid = Laid::new(part, tile, placement);
    let ((i0, j0), (i1, j1)) = laid.cells(part);
    let mut edges: BTreeSet<(Place, Place)> = BTreeSet::new();
    for i in i0..=i1 {
        for j in j0..=j1 {
            meter.charge(1)?;
            for line in &tile.lines {
                meter.charge(1)?;
                let (from, to) = ((i, j, line.from), (i + line.cell.0, j + line.cell.1, line.to));
                // As found: the tile lists each line once, and each point's neighbours are a set.
                if contains(rings, laid.at(from), laid.at(to), meter)? {
                    edges.insert((from, to));
                }
            }
        }
    }
    let places: BTreeMap<Place, usize> = {
        let all: BTreeSet<Place> = edges.iter().flat_map(|&(a, b)| [a, b]).collect();
        all.into_iter().enumerate().map(|(i, place)| (place, i)).collect()
    };
    let mut next = vec![BTreeSet::new(); places.len()];
    for (a, b) in &edges {
        let (Some(&a), Some(&b)) = (places.get(a), places.get(b)) else { continue };
        next.get_mut(a).map(|n| n.insert(b));
        next.get_mut(b).map(|n| n.insert(a));
    }
    let points: Vec<Point> = places.keys().map(|&place| laid.at(place)).collect();
    let kept = without_free_ends(Network { points, next }, meter)?;
    if kept.len() == 0 {
        return Ok(None);
    }
    joined(kept, meter).map(Some)
}

/// A tile laid over a part: its steps and corners scaled and turned, the corner 0 of cell 0 at the middle
/// of the part's bounds.
struct Laid<'t> {
    steps: [Point; 2],
    corners: Vec<Point>,
    middle: Point,
    tile: &'t Tile,
}

impl<'t> Laid<'t> {
    fn new(part: &Polygon, tile: &'t Tile, placement: Placement) -> Self {
        let (sin, cos) = sin_cos(placement.angle);
        let (sx, sy) = placement.scale;
        // Scaled across and down, then turned clockwise as drawn, y down, as Ink/Stitch turns its tiles.
        let place = |p: Point| {
            let (x, y) = (p.x() * SIDE_MM * sx, p.y() * SIDE_MM * sy);
            Point::new(x * cos - y * sin, y * cos + x * sin).unwrap_or(Point::ORIGIN)
        };
        let [s, t] = tile.steps;
        let (mut low, mut high) = ((f64::INFINITY, f64::INFINITY), (f64::NEG_INFINITY, f64::NEG_INFINITY));
        for q in &part.outline {
            low = (low.0.min(q.x()), low.1.min(q.y()));
            high = (high.0.max(q.x()), high.1.max(q.y()));
        }
        let middle = Point::new((low.0 + high.0) / 2.0, (low.1 + high.1) / 2.0).unwrap_or(Point::ORIGIN);
        Laid { steps: [place(s), place(t)], corners: tile.corners.iter().map(|&c| place(c)).collect(), middle, tile }
    }

    /// Where the point at `place` lies.
    fn at(&self, (i, j, corner): Place) -> Point {
        let c = self.corners.get(corner).copied().unwrap_or(Point::ORIGIN);
        plus(plus(self.middle, offset(&self.steps, i, j)), c)
    }

    /// The first and last cells, along each step, whose lines may reach `part`: those over its bounds, and
    /// the tile's reach beyond them.
    fn cells(&self, part: &Polygon) -> ((i32, i32), (i32, i32)) {
        let [s, t] = self.steps;
        let det = s.x() * t.y() - s.y() * t.x();
        let (mut low, mut high) = ((f64::INFINITY, f64::INFINITY), (f64::NEG_INFINITY, f64::NEG_INFINITY));
        for q in &part.outline {
            let d = (q.x() - self.middle.x(), q.y() - self.middle.y());
            let (u, v) = ((d.0 * t.y() - d.1 * t.x()) / det, (s.x() * d.1 - s.y() * d.0) / det);
            low = (low.0.min(u), low.1.min(v));
            high = (high.0.max(u), high.1.max(v));
        }
        // A line reaches a cell from as far as its own reach, and its corner lies anywhere in its cell.
        let reach = 1 + self.tile.lines.iter().map(|l| l.cell.0.abs().max(l.cell.1.abs())).max().unwrap_or(0);
        let whole = |v: f64| if v.is_finite() { v.clamp(-1e6, 1e6) } else { 0.0 };
        #[allow(clippy::cast_possible_truncation)]
        let cell = |v: f64, up: bool| (if up { whole(v).ceil() } else { whole(v).floor() }) as i32;
        ((cell(low.0, false) - reach, cell(low.1, false) - reach), (cell(high.0, true) + reach, cell(high.1, true) + reach))
    }
}

/// Whether the line from `p` to `q` lies in the part `rings` measures: no point of it outside, and some
/// point inside, as shapely's `contains` decides. Costs as [`Rings::meeting`] and [`Rings::covers`] do.
pub(crate) fn contains(rings: &Rings<'_>, p: Point, q: Point, meter: &mut Meter) -> Result<bool, Exhausted> {
    if p == q || !rings.covers(p, meter)? || !rings.covers(q, meter)? {
        return Ok(false);
    }
    let (mut cuts, mut crossed) = (vec![0.0, 1.0], false);
    let along = |x: Point| {
        let (d, w) = ((q.x() - p.x(), q.y() - p.y()), (x.x() - p.x(), x.y() - p.y()));
        (d.0 * w.0 + d.1 * w.1) / (d.0 * d.0 + d.1 * d.1)
    };
    rings.meeting(p, q, meter, |side| {
        let (a, b) = (side.a, side.b);
        let (sa, sb) = (exact::side(p, q, a), exact::side(p, q, b));
        let (sp, sq) = (exact::side(a, b, p), exact::side(a, b, q));
        if sa != Side::On && sb != Side::On && sa != sb && sp != Side::On && sq != Side::On && sp != sq {
            crossed = true;
        }
        for (corner, on) in [(a, sa), (b, sb)] {
            let t = along(corner);
            if on == Side::On && t > 0.0 && t < 1.0 {
                cuts.push(t);
            }
        }
    })?;
    if crossed {
        return Ok(false);
    }
    cuts.sort_by(f64::total_cmp);
    cuts.dedup();
    let mut inside = false;
    for pair in cuts.windows(2) {
        let &[s, t] = pair else { continue };
        let middle = p.lerp(q, (s + t) / 2.0);
        if !rings.covers(middle, meter)? {
            return Ok(false);
        }
        inside |= rings.inside(middle, meter)?;
    }
    Ok(inside)
}

/// `network` with each point that meets fewer than 2 lines taken away, again and again, until none is
/// left, and its points numbered again in order. One unit of `meter` for each point and line looked at.
fn without_free_ends(network: Network, meter: &mut Meter) -> Result<Network, Exhausted> {
    let Network { points, mut next } = network;
    let mut gone = vec![false; points.len()];
    let mut free: VecDeque<usize> = (0..points.len()).filter(|&i| next.get(i).is_none_or(|n| n.len() < 2)).collect();
    while let Some(i) = free.pop_front() {
        meter.charge(1)?;
        if gone.get(i).copied().unwrap_or(true) {
            continue;
        }
        if let Some(g) = gone.get_mut(i) {
            *g = true;
        }
        let neighbours: Vec<usize> = next.get_mut(i).map(std::mem::take).into_iter().flatten().collect();
        for n in neighbours {
            meter.charge(1)?;
            // Every neighbour is a point of the network, so `next` has its lines.
            let freed = next.get_mut(n).is_some_and(|theirs| {
                theirs.remove(&i);
                theirs.len() < 2
            });
            if freed && !gone.get(n).copied().unwrap_or(true) {
                free.push_back(n);
            }
        }
    }
    let renumbered: Vec<Option<usize>> = {
        let mut count = 0;
        gone.iter()
            .map(|&g| {
                (!g).then(|| {
                    count += 1;
                    count - 1
                })
            })
            .collect()
    };
    let kept = |i: usize| renumbered.get(i).copied().flatten();
    let mut network = Network::default();
    for (i, &point) in points.iter().enumerate() {
        if kept(i).is_some() {
            network.points.push(point);
            network.next.push(next.get(i).into_iter().flatten().filter_map(|&n| kept(n)).collect());
        }
    }
    Ok(network)
}

/// `network` with its pieces joined: between each 2 pieces their nearest points, the first pair found of
/// those as near, and of those joins the shortest that join every piece, as few as do. One unit of
/// `meter` for each pair of points measured.
fn joined(mut network: Network, meter: &mut Meter) -> Result<Network, Exhausted> {
    let pieces = pieces(&network, meter)?;
    if pieces.len() < 2 {
        return Ok(network);
    }
    // The nearest points of each 2 pieces, a join each.
    let mut joins: Vec<(f64, usize, usize, usize, usize)> = Vec::new();
    for (a, first) in pieces.iter().enumerate() {
        for (b, second) in pieces.iter().enumerate().skip(a + 1) {
            let mut best: Option<(f64, usize, usize)> = None;
            for &p in first {
                for &q in second {
                    meter.charge(1)?;
                    let d = network.point(p).distance(network.point(q));
                    if best.is_none_or(|(near, _, _)| d < near) {
                        best = Some((d, p, q));
                    }
                }
            }
            if let Some((d, p, q)) = best {
                joins.push((d, a, b, p, q));
            }
        }
    }
    joins.sort_by(|x, y| x.0.total_cmp(&y.0).then((x.1, x.2).cmp(&(y.1, y.2))));
    // The shortest joins that bring every piece together, Kruskal's way.
    let mut group: Vec<usize> = (0..pieces.len()).collect();
    let root = |group: &[usize], mut g: usize| {
        while let Some(&up) = group.get(g).filter(|&&up| up != g) {
            g = up;
        }
        g
    };
    for (_, a, b, p, q) in joins {
        let (ra, rb) = (root(&group, a), root(&group, b));
        if ra != rb {
            if let Some(g) = group.get_mut(rb) {
                *g = ra;
            }
            network.next.get_mut(p).map(|n| n.insert(q));
            network.next.get_mut(q).map(|n| n.insert(p));
        }
    }
    Ok(network)
}

/// The pieces of `network`, each its points in order, in the order of their first points. One unit of
/// `meter` for each point reached.
fn pieces(network: &Network, meter: &mut Meter) -> Result<Vec<Vec<usize>>, Exhausted> {
    let mut seen = vec![false; network.len()];
    let mut pieces = Vec::new();
    for first in 0..network.len() {
        if seen.get(first).copied().unwrap_or(true) {
            continue;
        }
        let mut piece = Vec::new();
        let mut queue = VecDeque::from([first]);
        if let Some(s) = seen.get_mut(first) {
            *s = true;
        }
        while let Some(p) = queue.pop_front() {
            meter.charge(1)?;
            piece.push(p);
            for n in network.neighbours(p) {
                if let Some(s) = seen.get_mut(n).filter(|s| !**s) {
                    *s = true;
                    queue.push_back(n);
                }
            }
        }
        piece.sort_unstable();
        pieces.push(piece);
    }
    Ok(pieces)
}

#[cfg(test)]
mod tests {
    use stitchcraft_core::Budget;

    use super::*;
    use crate::generators::meander::tiles::tile;
    use crate::generators::tatami::rings::distance_to_side;
    use crate::normalize::fixture::part;

    fn p(x: f64, y: f64) -> Point {
        Point::new(x, y).unwrap()
    }

    const FLAT: Placement = Placement { scale: (1.0, 1.0), angle: 0.0 };

    fn laid(outline: &[(f64, f64)], holes: &[&[(f64, f64)]], id: &str, placement: Placement) -> Option<Network> {
        let meter = &mut Budget::DEFAULT.meter();
        let part = part(outline, holes);
        let rings = Rings::new(&part, meter).unwrap();
        network(&part, &rings, &tile(id).unwrap(), placement, meter).unwrap()
    }

    #[test]
    fn req_fill_mea_002_a_square_s_network_is_its_grid_inside_it_with_no_free_end() {
        // A square 9 mm a side round (0, 0), squares 2 mm a side with a point at the middle: the points from
        // -4 to 4 mm, 5 by 5, every line inside, each point meeting 2 lines or more.
        let network = laid(&[(-4.5, -4.5), (4.5, -4.5), (4.5, 4.5), (-4.5, 4.5)], &[], "squares", FLAT).unwrap();
        assert_eq!(network.len(), 25);
        assert!(network.points.iter().all(|q| q.x().abs() <= 4.0 + 1e-9 && q.y().abs() <= 4.0 + 1e-9));
        assert!(network.next.iter().all(|n| n.len() >= 2));
        assert_eq!(network.next.iter().map(BTreeSet::len).sum::<usize>(), 2 * 40);
        // Exactly 8 mm across, the outermost lines lie along the outline, not inside, and go; the lines in to
        // them touch it at one end and stay, until their free ends take them away: 3 by 3 points are left.
        let touching = laid(&[(-4.0, -4.0), (4.0, -4.0), (4.0, 4.0), (-4.0, 4.0)], &[], "squares", FLAT).unwrap();
        assert_eq!(touching.len(), 9);
    }

    #[test]
    fn a_line_that_crosses_a_hole_is_left_out_and_free_ends_go() {
        // A hole 1 mm across round the middle point takes its 4 lines away, and the middle point with them.
        let hole = [(-0.5, -0.5), (0.5, -0.5), (0.5, 0.5), (-0.5, 0.5)];
        let network = laid(&[(-4.5, -4.5), (4.5, -4.5), (4.5, 4.5), (-4.5, 4.5)], &[&hole], "squares", FLAT).unwrap();
        assert_eq!(network.len(), 24);
        assert!(network.points.iter().all(|q| q.distance(p(0.0, 0.0)) > 1.0));
        // A strip 1 mm wide holds no square, and has no network.
        assert_eq!(laid(&[(0.0, 0.0), (20.0, 0.0), (20.0, 1.0), (0.0, 1.0)], &[], "squares", FLAT), None);
    }

    #[test]
    fn pieces_apart_are_joined_by_their_nearest_points() {
        // 2 rectangles joined by a neck no line of the grid runs along: 2 pieces, joined by one line across
        // the neck, between the first 2 points of all those as near.
        let outline = [
            (-9.0, -3.0),
            (-1.0, -3.0),
            (-1.0, 0.5),
            (1.0, 0.5),
            (1.0, -3.0),
            (9.0, -3.0),
            (9.0, 3.0),
            (1.0, 3.0),
            (1.0, 1.5),
            (-1.0, 1.5),
            (-1.0, 3.0),
            (-9.0, 3.0),
        ];
        let network = laid(&outline, &[], "squares", FLAT).unwrap();
        let across: Vec<(Point, Point)> = (0..network.len())
            .flat_map(|a| network.neighbours(a).map(move |b| (a, b)))
            .filter(|&(a, b)| a < b && network.point(a).x() < -1.0 && network.point(b).x() > 1.0)
            .map(|(a, b)| (network.point(a), network.point(b)))
            .collect();
        assert_eq!(across, [(p(-2.0, -2.0), p(2.0, -2.0))]);
    }

    #[test]
    fn the_tile_is_scaled_and_turned_about_the_middle() {
        let square = [(-10.0, -10.0), (10.0, -10.0), (10.0, 10.0), (-10.0, 10.0)];
        // Twice as large across: the points 4 mm apart across and 2 mm down.
        let wide = laid(&square, &[], "squares", Placement { scale: (2.0, 1.0), angle: 0.0 }).unwrap();
        assert!(wide.points.contains(&p(4.0, 2.0)) && !wide.points.iter().any(|q| (q.x() - 2.0).abs() < 1e-9));
        // Turned a quarter turn clockwise as drawn: the point 2 mm right of the middle lies 2 mm below it.
        let turned = laid(&square, &[], "honeycomb", Placement { scale: (1.0, 1.0), angle: std::f64::consts::FRAC_PI_2 }).unwrap();
        assert!(turned.points.iter().any(|q| q.distance(p(0.0, 2.0)) < 1e-9), "{:?}", &turned.points[..4]);
    }

    #[test]
    fn req_fill_mea_002_every_corner_of_the_tiling_well_inside_the_part_is_a_point_of_its_network() {
        // Over a square and a right triangle 4 mm a side away from the origin, under sides of 0.2 mm, level and
        // turned: a corner of the tiling more than 2 sides inside the part meets only lines inside it, so it is
        // a point of the network, however the part lies over the tile's cells.
        let square = [(0.0, 0.0), (4.0, 0.0), (4.0, 4.0), (0.0, 4.0)];
        let triangle = [(0.0, 0.0), (4.0, 0.0), (0.0, 4.0)];
        let tenth = |angle: f64| Placement { scale: (0.1, 0.1), angle };
        let laid_out = [("squares", tenth(0.0)), ("honeycomb", tenth(0.5)), ("honeycomb", tenth(1.0)), ("triangles", tenth(-1.0))];
        let meter = &mut Budget::DEFAULT.meter();
        for outline in [&square[..], &triangle[..]] {
            let part = part(outline, &[]);
            let rings = Rings::new(&part, meter).unwrap();
            let deep = |q: Point, meter: &mut Meter| {
                let near = |v: f64| (0.4..3.6).contains(&v);
                near(q.x())
                    && near(q.y())
                    && part.outline.windows(2).all(|side| distance_to_side(q, side[0], side[1]) > 0.4)
                    && rings.inside(q, meter).unwrap()
            };
            for (id, placement) in laid_out {
                let tile = tile(id).unwrap();
                let network = network(&part, &rings, &tile, placement, meter).unwrap().unwrap();
                let laid = Laid::new(&part, &tile, placement);
                let mut found = 0;
                for i in -60..=60 {
                    for j in -60..=60 {
                        for corner in 0..tile.corners.len() {
                            let q = laid.at((i, j, corner));
                            if deep(q, meter) {
                                found += 1;
                                assert!(network.points.contains(&q), "{id}, {placement:?}: {q:?} is no point of the network");
                            }
                        }
                    }
                }
                assert!(found > 50, "{id}, {placement:?}: {found} corners well inside");
            }
        }
    }

    #[test]
    fn every_line_of_the_tiling_in_the_part_starts_in_a_cell_the_network_looks_at() {
        // Hexagons with sides of 10 mm over a rectangle 300 mm by 30 mm and one 30 mm by 300 mm, level and
        // turned: each line of the tiling that lies in the part starts in a cell between the first and the last
        // that `network` looks at, however far the part reaches along either step.
        let meter = &mut Budget::DEFAULT.meter();
        let tile = tile("honeycomb").unwrap();
        for outline in [[(0.0, 0.0), (300.0, 0.0), (300.0, 30.0), (0.0, 30.0)], [(0.0, 0.0), (30.0, 0.0), (30.0, 300.0), (0.0, 300.0)]] {
            let part = part(&outline, &[]);
            let rings = Rings::new(&part, meter).unwrap();
            for angle in [0.0, 0.3] {
                let laid = Laid::new(&part, &tile, Placement { scale: (5.0, 5.0), angle });
                let ((i0, j0), (i1, j1)) = laid.cells(&part);
                let mut found = 0;
                for (i, j) in (-40..=40).flat_map(|i| (-40..=40).map(move |j| (i, j))) {
                    for line in &tile.lines {
                        let (from, to) = ((i, j, line.from), (i + line.cell.0, j + line.cell.1, line.to));
                        if contains(&rings, laid.at(from), laid.at(to), meter).unwrap() {
                            found += 1;
                            assert!(
                                i0 <= i && i <= i1 && j0 <= j && j <= j1,
                                "{outline:?}, {angle}: a line from ({i}, {j}), past ({i0}, {j0}) to ({i1}, {j1})"
                            );
                        }
                    }
                }
                assert!(found > 20, "{outline:?}, {angle}: {found} lines");
            }
        }
    }

    #[test]
    fn a_line_touching_the_outline_from_inside_is_in_and_one_leaving_it_is_out() {
        let meter = &mut Budget::DEFAULT.meter();
        // A notch at the top: its corner at (0, 1) points into the square.
        let part = part(&[(-2.0, -2.0), (2.0, -2.0), (2.0, 2.0), (1.0, 2.0), (0.0, 1.0), (-1.0, 2.0), (-2.0, 2.0)], &[]);
        let rings = Rings::new(&part, meter).unwrap();
        assert!(contains(&rings, p(-1.0, 1.0), p(1.0, 1.0), meter).unwrap(), "touching the notch's corner");
        assert!(!contains(&rings, p(0.0, 1.5), p(0.0, 0.0), meter).unwrap(), "from inside the notch, outside");
        assert!(!contains(&rings, p(-2.0, -2.0), p(2.0, -2.0), meter).unwrap(), "along the outline only");
        assert!(contains(&rings, p(-2.0, -2.0), p(2.0, 2.0), meter).unwrap(), "corner to corner, across");
        // Along the top from one side of the notch to the other, the middle lies in the notch.
        assert!(!contains(&rings, p(-1.5, 2.0), p(1.5, 2.0), meter).unwrap(), "across the notch");
        assert!(!contains(&rings, p(1.0, 0.0), p(1.0, 0.0), meter).unwrap(), "a line of no length");
    }

    #[test]
    fn a_line_from_a_ring_inwards_is_in_and_one_across_a_hole_is_out() {
        let meter = &mut Budget::DEFAULT.meter();
        let hole = [(-0.5, -0.5), (0.5, -0.5), (0.5, 0.5), (-0.5, 0.5)];
        let holed = part(&[(-2.0, -2.0), (2.0, -2.0), (2.0, 2.0), (-2.0, 2.0)], &[&hole]);
        let rings = Rings::new(&holed, meter).unwrap();
        assert!(contains(&rings, p(-2.0, 1.0), p(0.0, 1.0), meter).unwrap(), "from the outline");
        assert!(contains(&rings, p(0.0, 1.0), p(2.0, 1.0), meter).unwrap(), "to the outline");
        assert!(!contains(&rings, p(-1.0, 0.0), p(1.0, 0.0), meter).unwrap(), "across the hole");
        // Short of the long side of a triangle, which runs across the line's way on beyond its end: in.
        let triangle = part(&[(0.0, 0.0), (3.0, 0.0), (0.0, 3.0)], &[]);
        assert!(contains(&Rings::new(&triangle, meter).unwrap(), p(1.0, 1.0), p(1.4, 1.4), meter).unwrap(), "short of a side");
        // From a side of a triangular hole, whose corner opposite lies on the line behind its start: in, the
        // line not reaching back through the hole to that corner.
        let triangle = [(-1.0, 0.0), (0.0, 1.0), (0.0, -1.0)];
        let holed = part(&[(-3.0, -3.0), (3.0, -3.0), (3.0, 3.0), (-3.0, 3.0)], &[&triangle]);
        let rings = Rings::new(&holed, meter).unwrap();
        assert!(contains(&rings, p(0.0, 0.0), p(2.0, 0.0), meter).unwrap(), "from the hole");
    }

    #[test]
    fn a_line_is_cut_where_a_corner_of_a_ring_lies_on_it() {
        // A notch from the right whose corner touches the line up the middle at its middle: in, cut there into
        // 2 halves inside, however long the line.
        let meter = &mut Budget::DEFAULT.meter();
        let notched = part(&[(-3.0, -3.0), (3.0, -3.0), (3.0, -1.0), (0.0, 0.0), (3.0, 1.0), (3.0, 3.0), (-3.0, 3.0)], &[]);
        let rings = Rings::new(&notched, meter).unwrap();
        assert!(contains(&rings, p(0.0, -2.0), p(0.0, 2.0), meter).unwrap());
        assert!(contains(&rings, p(0.0, -0.5), p(0.0, 0.5), meter).unwrap());
        // A step: inside for a quarter of a millimetre, then along a side to its far corner, cut where the
        // line meets the side, so that a piece of it lies inside.
        let step = part(&[(-3.0, -3.0), (-1.0, -3.0), (-1.0, -1.0), (3.0, -1.0), (3.0, 3.0), (-3.0, 3.0)], &[]);
        let rings = Rings::new(&step, meter).unwrap();
        assert!(contains(&rings, p(-1.25, -1.0), p(3.0, -1.0), meter).unwrap());
    }

    #[test]
    fn a_line_is_found_in_or_out_or_the_work_runs_out() {
        let part = part(&[(-2.0, -2.0), (2.0, -2.0), (2.0, 2.0), (1.0, 2.0), (0.0, 1.0), (-1.0, 2.0), (-2.0, 2.0)], &[]);
        let rings = Rings::new(&part, &mut Budget::DEFAULT.meter()).unwrap();
        let (from, to) = (p(-1.0, 1.0), p(1.0, 1.0));
        let meter = &mut Budget::DEFAULT.meter();
        assert!(contains(&rings, from, to, meter).unwrap());
        // Any less work runs out somewhere on the way: at an end, among the sides, or at a piece's middle.
        let needed = Budget::DEFAULT.max_work - meter.work_left();
        for work in 0..needed {
            let short = &mut Budget { max_stitches: 0, max_work: work }.meter();
            assert_eq!(contains(&rings, from, to, short), Err(Exhausted::Work), "{work} of {needed} units");
        }
    }

    #[test]
    fn free_ends_go_one_after_another_and_the_points_left_are_numbered_again() {
        // A triangle, points 1, 3 and 4, with a tail of 2 lines off point 4: its tip, point 0, goes, and then
        // point 2, whose only line is left.
        let points = vec![p(-2.0, 1.0), p(0.0, 0.0), p(-1.0, 1.0), p(1.0, 0.0), p(0.0, 1.0)];
        let lines: [&[usize]; 5] = [&[2], &[3, 4], &[0, 4], &[1, 4], &[1, 2, 3]];
        let next = lines.iter().map(|n| n.iter().copied().collect()).collect();
        let kept = without_free_ends(Network { points, next }, &mut Budget::DEFAULT.meter()).unwrap();
        let triangle = [[1, 2], [0, 2], [0, 1]].iter().map(|n| n.iter().copied().collect()).collect();
        assert_eq!(kept, Network { points: vec![p(0.0, 0.0), p(1.0, 0.0), p(0.0, 1.0)], next: triangle });
    }

    #[test]
    fn a_line_on_its_own_goes_whole_though_its_second_end_is_found_free_twice() {
        // Both ends of a lone line are free from the start. Taking the first frees the second again, so the
        // second is waiting twice, and the second time it is gone already.
        let next = vec![[1].into_iter().collect(), [0].into_iter().collect()];
        let kept = without_free_ends(Network { points: vec![p(0.0, 0.0), p(1.0, 0.0)], next }, &mut Budget::DEFAULT.meter()).unwrap();
        assert_eq!(kept, Network::default());
    }

    #[test]
    fn three_pieces_in_a_row_are_joined_by_the_2_shortest_joins() {
        // 3 squares 6 mm high in a row, joined by necks no line of the grid runs along: the middle one is
        // joined to each of the others, and the 2 ends, farther apart, are not joined to each other.
        let outline = [
            (-15.0, -3.0),
            (-5.0, -3.0),
            (-5.0, 0.5),
            (-3.0, 0.5),
            (-3.0, -3.0),
            (3.0, -3.0),
            (3.0, 0.5),
            (5.0, 0.5),
            (5.0, -3.0),
            (15.0, -3.0),
            (15.0, 3.0),
            (5.0, 3.0),
            (5.0, 1.5),
            (3.0, 1.5),
            (3.0, 3.0),
            (-3.0, 3.0),
            (-3.0, 1.5),
            (-5.0, 1.5),
            (-5.0, 3.0),
            (-15.0, 3.0),
        ];
        let network = laid(&outline, &[], "squares", FLAT).unwrap();
        let apart = |a: Point, b: Point| (a.x() + 4.0).signum() != (b.x() + 4.0).signum() || (a.x() - 4.0).signum() != (b.x() - 4.0).signum();
        let joins: Vec<(Point, Point)> = (0..network.len())
            .flat_map(|a| network.neighbours(a).map(move |b| (a, b)))
            .filter(|&(a, b)| a < b && apart(network.point(a), network.point(b)))
            .map(|(a, b)| (network.point(a), network.point(b)))
            .collect();
        assert_eq!(joins, [(p(-6.0, -2.0), p(-2.0, -2.0)), (p(2.0, -2.0), p(6.0, -2.0))]);
    }
}
