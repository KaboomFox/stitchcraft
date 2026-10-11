//! A tatami fill sewn: its parts one at a time, each part's rows in the order `route` gives,
//! with running stitches along the needle's way between them (`REQ-FILL-TAT-001`, `REQ-FILL-TAT-008`).
//!
//! **Parts.** Ink/Stitch sews a fill's parts nearest the needle first, a part the needle is in being 0
//! away, and with no needle, from the part farthest left. Each part ends at its point nearest the next
//! part, and the last at the point of the fill nearest the next element: its first stitch, or for an
//! element that starts nearest the needle, its shape. A part is a group of its own, which assembly joins to
//! the next as it joins any 2 groups, with a jump and the locks and trims the settings ask for, or
//! straight on when they are close (`SC-W0307` says the fill is in parts).
//!
//! **Rows.** A part's segments are taken row by row, across the rows, and in each row by how far their
//! start lies from the corner of the part's bounding box where x and y are least, as Ink/Stitch takes them.
//! Each is sewn with its needle points ([`super::stitches`]) from the end the route enters it by.
//!
//! **Travel.** Between rows the needle runs under the rows not sewn yet (`underpath`, [`super::underpath`]),
//! in running stitches of the running stitch length, within its tolerance (`running::along_line`). With
//! `underpath` off, in a part the lines miss, and between 2 nodes no line joins, as to a hole too small for
//! any line to reach, it runs along the part's rings and rows instead ([`super::travel`]), as Ink/Stitch's
//! runs along the outline. The travel's first stitch lands on the row's end, which the row has sewn
//! already, so it is left out, as in Ink/Stitch. With `skip_last`, the row's end is not sewn, and the
//! travel keeps its first stitch, unless the next row starts beside it. Rows and rings join every node of a
//! part, so a way along them always leads on; were none found, the needle would jump, and `SC-E0009` would
//! say so as a bug.
//!
//! **No rows.** A part too thin for any row is sewn as a running stitch round its outline, from the
//! outline's start, as Ink/Stitch sews it (`SC-W0305`).
//!
//! **The shortest stitch.** Rows the row spacing apart start and end nearer each other than the shortest
//! stitch a machine sews well, and a row's first grid point can lie just past its start. A needle point
//! nearer the one before it than the shortest stitch is left out, so the stitch runs on to the next, as
//! Ink/Stitch's stitch plan leaves out points nearer than its own shortest stitch. A run's last point stays,
//! and the points before it that it is too near go instead. It is finalize's rule, the same function
//! (`crate::thin`), applied here so that the generator hands finalize stitches it has nothing to thin.

use stitchcraft_core::rng::SplitMix64;
use stitchcraft_core::units::at_least;
use stitchcraft_core::{Code, Diagnostic, Exhausted, Meter, Point};

use super::clamp::clamp;
use super::graph::NodeId;
use super::rings::Rings;
use super::route::{Route, Step, route};
use super::rows::rows;
use super::stitches::{Stitching, row_points};
use super::travel::{Network, between};
use super::underpath::{Underpath, smooth};
use crate::generators::running::along_line;
use crate::generators::{Approach, Stitched};
use crate::normalize::near::{nearest_pair, nearest_to_lines, nearest_to_point};
use crate::normalize::region::{Polygon, Region};
use crate::thin::thin;

/// How the needle travels between rows: in stitches this long, within this tolerance of the way, none
/// shorter than the shortest stitch, in millimetres, and under the rows or along the rings.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Travel {
    /// The running stitch's length.
    pub length: f64,
    /// How far a stitch may stray from the way.
    pub tolerance: f64,
    /// The shortest stitch.
    pub min_stitch: f64,
    /// Whether travel runs under the rows not sewn yet (`underpath`).
    pub underpath: bool,
}

/// The stitches of a tatami fill over `region`, stitched as `stitching` says, travelling as `travel` says,
/// from the needle where the elements before left it to near the `next` element. Random lengths draw from
/// `rng`. Each part is a run. Work is charged to `meter` for every ring point, row, node, edge, step and
/// stitch.
pub fn tatami_fill(
    region: &Region,
    stitching: &Stitching,
    travel: Travel,
    needle: Option<Point>,
    next: Option<&Approach>,
    rng: &mut SplitMix64,
    meter: &mut Meter,
) -> Result<Stitched, Exhausted> {
    let parts = in_order(region, needle, meter)?;
    let end = ending(region, next, meter)?;
    let mut needle = needle;
    let (mut runs, mut warnings) = (Vec::new(), Vec::new());
    let mut outlined = 0;
    for (i, part) in parts.iter().enumerate() {
        let here = match parts.get(i + 1) {
            Some(after) => nearest_pair(&rings_of(part), &rings_of(after), meter)?.map(|(on, _)| on),
            None => end,
        };
        let rings = Rings::new(part, meter)?;
        let segments = segments(part, stitching, meter)?;
        let sewn = match route(&rings, &segments, needle, here, meter)? {
            Some(route) => {
                let lines = if travel.underpath { Underpath::new(part, &rings, &route.nodes, &segments, stitching.grid.angle, meter)? } else { None };
                let network = Network::new(&route.nodes, &rings, &row_ends(&route), meter)?;
                Sewing { route: &route, part, rings: &rings, stitching, travel, lines, network }.sew(rng, meter, &mut warnings)?
            }
            None => {
                outlined += 1;
                vec![along_line(&part.outline, travel.length, travel.tolerance, travel.min_stitch, meter)?]
            }
        };
        let sewn: Vec<Vec<Point>> = sewn
            .into_iter()
            .map(|run| thin(run, |p| *p, |a, b| !at_least(a.distance(*b), travel.min_stitch), |_| false, |_| ()))
            .filter(|run| !run.is_empty())
            .collect();
        needle = sewn.iter().rev().find_map(|run| run.last().copied()).or(needle);
        runs.extend(sewn);
    }
    if outlined > 0 {
        let message = match (outlined, parts.len()) {
            (1, 1) => {
                "This fill is too thin for a row of stitches at its row spacing, so it is sewn as a running stitch round its outline.".to_string()
            }
            (1, _) => "1 of this fill's parts is too thin for a row of stitches at its row spacing, so it is sewn as a running stitch round its \
                       outline."
                .to_string(),
            (n, _) => format!(
                "{n} of this fill's parts are too thin for a row of stitches at its row spacing, so each is sewn as a running stitch round its \
                 outline."
            ),
        };
        warnings.push(Diagnostic::new(Code::FillTooThin, message));
    }
    Ok(Stitched { runs, warnings })
}

/// A part's rings as polylines: the outline, then the holes.
fn rings_of(part: &Polygon) -> Vec<&[Point]> {
    part.rings().collect()
}

/// `region`'s parts in the order they are sewn: nearest `needle` first, or with no needle, by how far left
/// they reach; the drawing's order on a tie.
fn in_order<'a>(region: &'a Region, needle: Option<Point>, meter: &mut Meter) -> Result<Vec<&'a Polygon>, Exhausted> {
    let mut keyed = Vec::with_capacity(region.parts.len());
    for part in &region.parts {
        let key = match needle {
            Some(p) if covers(part, p, meter)? => 0.0,
            Some(p) => nearest_to_point(&rings_of(part), p, meter)?.map_or(f64::INFINITY, |(_, d)| d),
            None => part.outline.iter().map(|q| q.x()).fold(f64::INFINITY, f64::min),
        };
        keyed.push((key, part));
    }
    keyed.sort_by(|a, b| a.0.total_cmp(&b.0));
    Ok(keyed.into_iter().map(|(_, part)| part).collect())
}

/// Where the fill ends: its point nearest what the `next` element offers. A point in the fill is itself.
/// Of a shape, the first line whose start lies in the fill ends it there, and otherwise the fill's point
/// nearest the shape, as Ink/Stitch's geometry library finds it.
fn ending(region: &Region, next: Option<&Approach>, meter: &mut Meter) -> Result<Option<Point>, Exhausted> {
    let rings: Vec<&[Point]> = region.parts.iter().flat_map(rings_of).collect();
    match next {
        None => Ok(None),
        Some(Approach::Point(p)) => {
            for part in &region.parts {
                if covers(part, *p, meter)? {
                    return Ok(Some(*p));
                }
            }
            Ok(nearest_to_point(&rings, *p, meter)?.map(|(on, _)| on))
        }
        Some(Approach::Shape(lines)) => {
            for part in &region.parts {
                for start in lines.iter().filter_map(|line| line.first()) {
                    if covers(part, *start, meter)? {
                        return Ok(Some(*start));
                    }
                }
            }
            nearest_to_lines(&rings, &lines.iter().map(Vec::as_slice).collect::<Vec<_>>(), meter)
        }
    }
}

/// Whether `p` lies in `part` or on its rings. One unit of `meter` for each of their points.
fn covers(part: &Polygon, p: Point, meter: &mut Meter) -> Result<bool, Exhausted> {
    meter.charge(rings_of(part).iter().map(|ring| u64::try_from(ring.len()).unwrap_or(u64::MAX)).fold(0, u64::saturating_add))?;
    Ok(part.covers(p))
}

/// The part's row segments, row by row across the rows and in each row by how far their start lies from
/// the corner of the part's bounding box where x and y are least, each from its start to its end.
fn segments(part: &Polygon, stitching: &Stitching, meter: &mut Meter) -> Result<Vec<(Point, Point)>, Exhausted> {
    let corner = part.outline.iter().fold((f64::INFINITY, f64::INFINITY), |(x, y), q| (x.min(q.x()), y.min(q.y())));
    let corner = Point::new(corner.0, corner.1).unwrap_or(Point::ORIGIN);
    let mut segments = Vec::new();
    for row in rows(part, &stitching.grid, meter)? {
        let mut pieces = row.segments;
        pieces.sort_by(|a, b| a.start.distance(corner).total_cmp(&b.start.distance(corner)));
        segments.extend(pieces.into_iter().map(|piece| (piece.start, piece.end)));
    }
    Ok(segments)
}

/// The ends of each row segment the route sews, in the order sewn.
fn row_ends(route: &Route) -> Vec<[NodeId; 2]> {
    route.steps.iter().filter_map(|step| if let Step::Row { from, to, .. } = *step { Some([from, to]) } else { None }).collect()
}

/// A part as it is sewn.
struct Sewing<'a> {
    route: &'a Route,
    part: &'a Polygon,
    rings: &'a Rings<'a>,
    stitching: &'a Stitching,
    travel: Travel,
    /// The lines under the rows, with `underpath` where they cross the part.
    lines: Option<Underpath>,
    /// The ways along the part's rings and rows.
    network: Network,
}

impl Sewing<'_> {
    /// The runs the route sews: one, unless the needle has to jump, which is a bug that a diagnostic added
    /// to `warnings` says, once for the part.
    fn sew(&mut self, rng: &mut SplitMix64, meter: &mut Meter, warnings: &mut Vec<Diagnostic>) -> Result<Vec<Vec<Point>>, Exhausted> {
        let route = self.route;
        let point = |node: usize| route.nodes.get(node).map(|n| n.point);
        let mut runs = Vec::new();
        let mut run: Vec<Point> = Vec::new();
        let mut jumped = false;
        if let Some(Step::Travel { from, .. }) = route.steps.first() {
            run.extend(point(*from));
        }
        for step in &route.steps {
            match *step {
                Step::Row { segment, from, to } => {
                    let (Some(a), Some(b)) = (point(from), point(to)) else { continue };
                    extend(&mut run, row_points(a, b, self.stitching, rng, meter)?);
                    if let Some(lines) = &mut self.lines {
                        lines.sew(segment);
                    }
                }
                Step::Travel { from, to, beside } => match self.way(from, to, meter)? {
                    Some(way) => {
                        let stitches = along_line(&way, self.travel.length, self.travel.tolerance, self.travel.min_stitch, meter)?;
                        let keep_first = self.stitching.skip_last && !beside;
                        extend(&mut run, stitches.into_iter().skip(usize::from(!keep_first)));
                    }
                    None => {
                        jumped = true;
                        runs.push(std::mem::take(&mut run));
                        run.extend(point(to));
                    }
                },
            }
        }
        runs.push(run);
        if jumped {
            warnings.push(Diagnostic::new(Code::InternalCheckFailed, "The needle found no way between 2 rows of this fill, so it jumps there."));
        }
        Ok(runs)
    }

    /// The needle's way from node `from` to node `to`: under the rows the cheapest way along the lines,
    /// smoothed and kept inside the part, and where no line joins them [`between`]'s way along the rings and
    /// rows, `None` where none leads either.
    fn way(&self, from: NodeId, to: NodeId, meter: &mut Meter) -> Result<Option<Vec<Point>>, Exhausted> {
        if let Some(lines) = &self.lines
            && let Some(way) = lines.way(from, to, meter)?
        {
            return Ok(Some(clamp(&smooth(&way, self.travel.min_stitch, meter)?, self.part, self.rings, meter)?));
        }
        between(&self.route.nodes, self.rings, &self.network, from, to, meter)
    }
}

/// Adds `points` to `run`, each unless it is where the run already is.
fn extend(run: &mut Vec<Point>, points: impl IntoIterator<Item = Point>) {
    for p in points {
        if run.last() != Some(&p) {
            run.push(p);
        }
    }
}

#[cfg(test)]
mod tests {
    use stitchcraft_core::Budget;

    use super::*;
    use crate::generators::tatami::fixture::{frame, p, rectangle};
    use crate::generators::tatami::route::Node;
    use crate::generators::tatami::rows::Grid;
    use crate::normalize::stroke::distance_to_segment;

    /// Rows 1 mm apart in stitches of 3 mm, and travel in stitches of 2 mm, nothing thinned.
    fn stitching(skip_last: bool) -> Stitching {
        Stitching { grid: Grid { angle: 0.0, spacing: 1.0, end_spacing: None }, length: 3.0, staggers: 4.0, skip_last, jitter: None }
    }

    const TRAVEL: Travel = Travel { length: 2.0, tolerance: 0.2, min_stitch: 1e-6, underpath: false };

    /// The runs `route` sews in `part`, travelling along its rings and rows.
    fn sew(
        part: &Polygon,
        route: &Route,
        rings: &Rings<'_>,
        stitching: &Stitching,
        meter: &mut Meter,
        warnings: &mut Vec<Diagnostic>,
    ) -> Vec<Vec<Point>> {
        let network = Network::new(&route.nodes, rings, &row_ends(route), meter).unwrap();
        Sewing { route, part, rings, stitching, travel: TRAVEL, lines: None, network }.sew(&mut SplitMix64::new(1), meter, warnings).unwrap()
    }

    #[test]
    fn with_skip_last_a_row_s_end_is_sewn_before_travel_unless_the_next_row_starts_beside_it() {
        // 3 rows across a 10 × 4 rectangle. Routed, the bottom row hands over to the middle row, which
        // starts beside its end, and the middle row to the far end of the top row, round the top corners.
        let part = rectangle(4.0);
        let segments = [1.0, 2.0, 3.0].map(|y| (p(0.0, y), p(10.0, y)));
        let meter = &mut Budget::DEFAULT.meter();
        let rings = Rings::new(&part, meter).unwrap();
        let route = route(&rings, &segments, None, None, meter).unwrap().unwrap();
        let stitching = stitching(true);
        let mut warnings = Vec::new();
        let runs = sew(&part, &route, &rings, &stitching, meter, &mut warnings);
        assert_eq!((runs.len(), warnings), (1, Vec::new()));
        let run = &runs[0];
        let mut handovers = Vec::new();
        for pair in route.steps.windows(2) {
            let [Step::Row { from, to, .. }, Step::Travel { beside, .. }] = *pair else { continue };
            let (start, end) = (route.nodes[from].point, route.nodes[to].point);
            // The row's last needle point, short of its end, and what comes after it.
            let last = *row_points(start, end, &stitching, &mut SplitMix64::new(1), meter).unwrap().last().unwrap();
            let after = run.iter().position(|&q| q == last).map(|i| run[i + 1]).unwrap();
            assert_eq!(after == end, !beside, "{:?}", route.steps);
            handovers.push(beside);
        }
        assert_eq!(handovers, [true, false]);
    }

    #[test]
    fn the_network_runs_along_the_rows_the_route_sews() {
        let steps = vec![
            Step::Travel { from: 4, to: 0, beside: false },
            Step::Row { segment: 1, from: 0, to: 1 },
            Step::Travel { from: 1, to: 2, beside: true },
            Step::Row { segment: 0, from: 3, to: 2 },
        ];
        assert_eq!(row_ends(&Route { nodes: Vec::new(), steps }), [[0, 1], [3, 2]]);
    }

    #[test]
    fn a_needle_that_finds_no_way_jumps_and_says_so_as_a_bug() {
        // A row between 2 points of the frame's outline and one between 2 points of its hole, which nothing
        // joins: the travel from the first to the second finds no way.
        let part = frame();
        let segments = [(p(0.0, 0.5), p(10.0, 0.5)), (p(4.0, 2.0), p(6.0, 2.0))];
        let meter = &mut Budget::DEFAULT.meter();
        let rings = Rings::new(&part, meter).unwrap();
        let route = route(&rings, &segments, None, None, meter).unwrap().unwrap();
        let mut warnings = Vec::new();
        let runs = sew(&part, &route, &rings, &stitching(false), meter, &mut warnings);
        assert_eq!(runs.len(), 2, "the needle jumps once");
        assert_eq!((runs[0].first(), runs[1].first()), (Some(&p(0.0, 0.5)), Some(&p(6.0, 2.0))));
        assert_eq!(warnings.iter().map(|d| d.code).collect::<Vec<_>>(), [Code::InternalCheckFailed]);
    }

    /// Whether `q` lies in `part` or within `within` of its rings.
    fn near_or_in(part: &Polygon, q: Point, within: f64) -> bool {
        part.covers(q) || part.rings().any(|ring| ring.windows(2).any(|side| distance_to_segment(q, side[0], side[1]) <= within))
    }

    #[test]
    fn req_fill_tat_005_every_way_under_the_rows_lies_inside_the_part() {
        // Rows 0.25 apart across the frame at 30°, sewn in the route's order: every way the needle travels
        // lies in the part or within 0.05 mm of its rings, though smoothing cuts the lines' corners round
        // the hole.
        let part = frame();
        let meter = &mut Budget::DEFAULT.meter();
        let rings = Rings::new(&part, meter).unwrap();
        let how = Stitching { grid: Grid { angle: 30.0, spacing: 0.25, end_spacing: None }, ..stitching(false) };
        let segments = segments(&part, &how, meter).unwrap();
        let route = route(&rings, &segments, None, None, meter).unwrap().unwrap();
        let lines = Underpath::new(&part, &rings, &route.nodes, &segments, 30.0, meter).unwrap();
        let network = Network::new(&route.nodes, &rings, &row_ends(&route), meter).unwrap();
        let travel = Travel { underpath: true, ..TRAVEL };
        let mut sewing = Sewing { route: &route, part: &part, rings: &rings, stitching: &how, travel, lines, network };
        let mut ways = 0;
        for step in &route.steps {
            match *step {
                Step::Row { segment, .. } => {
                    if let Some(lines) = &mut sewing.lines {
                        lines.sew(segment);
                    }
                }
                Step::Travel { from, to, .. } => {
                    let way = sewing.way(from, to, meter).unwrap().unwrap();
                    assert_eq!((way.first(), way.last()), (Some(&route.nodes[from].point), Some(&route.nodes[to].point)));
                    for pair in way.windows(2) {
                        for q in [pair[0], pair[0].lerp(pair[1], 0.5)] {
                            assert!(near_or_in(&part, q, 0.05), "{q:?} in {way:?}");
                        }
                    }
                    ways += 1;
                }
            }
        }
        assert!(ways > 3, "{ways}");
    }

    #[test]
    fn where_no_line_leads_the_needle_runs_along_the_rings_and_rows() {
        // A hole 0.2 mm across in the middle of a triangle the lines leave between them: the part is small,
        // so they lie 1 mm and 0.71 mm apart, and none reaches the hole. From the hole's left side the needle
        // runs along the row through it to the outline, and up the outline's left side.
        let hole = vec![p(4.44, 2.02), p(4.64, 2.02), p(4.64, 2.22), p(4.44, 2.22), p(4.44, 2.02)];
        let part = Polygon { holes: vec![hole], ..rectangle(4.0) };
        let meter = &mut Budget::DEFAULT.meter();
        let rings = Rings::new(&part, meter).unwrap();
        let points = [p(4.44, 2.12), p(0.0, 2.12), p(0.0, 1.0)];
        let nodes = points.map(|q| Node { point: q, place: rings.locate(q, |_| true, meter).unwrap().unwrap() }).to_vec();
        let route = Route { nodes, steps: Vec::new() };
        let lines = Underpath::new(&part, &rings, &route.nodes, &[(points[1], points[0])], 0.0, meter).unwrap().unwrap();
        assert_eq!(lines.way(0, 2, meter).unwrap(), None, "no line reaches the hole");
        let network = Network::new(&route.nodes, &rings, &[[1, 0]], meter).unwrap();
        let (travel, how) = (Travel { underpath: true, ..TRAVEL }, stitching(false));
        let sewing = Sewing { route: &route, part: &part, rings: &rings, stitching: &how, travel, lines: Some(lines), network };
        assert_eq!(sewing.way(0, 2, meter).unwrap(), Some(points.to_vec()));
    }
}
