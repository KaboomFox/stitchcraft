//! Unit tests of sewing a part's route; the conformance cases are in `tests/fill_routing.rs`.

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

const TRAVEL: Travel = Travel { length: 2.0, tolerance: 0.2, min_stitch: 1e-6, underpath: false, jump: 3.0 };

/// `route` through `segments` sewn in `part`, whose rings are `rings`, as `how` says: travelling along
/// the rings and rows, from no needle, with no pull compensation and no gap rows.
fn plain<'a>(
    route: &'a Route,
    part: &'a Polygon,
    rings: &'a Rings<'a>,
    (segments, rows): &'a RowSegments,
    how: &'a Stitching,
    meter: &mut Meter,
) -> Sewing<'a> {
    let network = Network::new(&route.nodes, rings, &row_ends(route), meter).unwrap();
    Sewing {
        route,
        part,
        rings,
        stitching: how,
        travel: TRAVEL,
        lines: None,
        network,
        start: None,
        segments,
        rows,
        pull: Pull::default(),
        gap_rows: 0,
    }
}

/// The runs `route` through `segments`, one to a row, sews in `part`, travelling along its rings and rows.
fn sew(
    part: &Polygon,
    route: &Route,
    rings: &Rings<'_>,
    segments: &[(Point, Point)],
    stitching: &Stitching,
    meter: &mut Meter,
    warnings: &mut Vec<Diagnostic>,
) -> Vec<Vec<Point>> {
    let segments = (segments.to_vec(), (0..segments.len()).collect());
    plain(route, part, rings, &segments, stitching, meter).sew(&mut SplitMix64::new(1), meter, warnings).unwrap()
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
    let runs = sew(&part, &route, &rings, &segments, &stitching, meter, &mut warnings);
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
    let runs = sew(&part, &route, &rings, &segments, &stitching(false), meter, &mut warnings);
    assert_eq!(runs.len(), 2, "the needle jumps once");
    assert_eq!((runs[0].first(), runs[1].first()), (Some(&p(0.0, 0.5)), Some(&p(6.0, 2.0))));
    assert_eq!(warnings.iter().map(|d| d.code).collect::<Vec<_>>(), [Code::InternalCheckFailed]);
}

/// Whether `q` lies within `within` of `part`'s rings or in it.
fn near_or_in(part: &Polygon, q: Point, within: f64) -> bool {
    part.rings().any(|ring| ring.windows(2).any(|side| distance_to_segment(q, side[0], side[1]) <= within)) || part.covers(q)
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
    let rows = segments(&part, &how, meter).unwrap();
    let route = route(&rings, &rows.0, None, None, meter).unwrap().unwrap();
    let lines = Underpath::new(&part, &rings, &route.nodes, &rows.0, 30.0, meter).unwrap();
    let travel = Travel { underpath: true, ..TRAVEL };
    let mut sewing = Sewing { travel, lines, ..plain(&route, &part, &rings, &rows, &how, meter) };
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
    // Sewn as a whole, the part is one run, which says nothing.
    let lines = Underpath::new(&part, &rings, &route.nodes, &rows.0, 30.0, meter).unwrap();
    let mut warnings = Vec::new();
    let mut sewing = Sewing { travel, lines, ..plain(&route, &part, &rings, &rows, &how, meter) };
    assert_eq!(sewing.sew(&mut SplitMix64::new(1), meter, &mut warnings).unwrap().len(), 1);
    assert_eq!(warnings, Vec::new());
}

#[test]
fn req_fill_tat_001_a_needle_inside_leads_in_from_the_lines_nearest_node_to_the_first_row() {
    // The needle inside a 10 mm square, 3 mm from its left side, and lines under rows 1 mm apart. The
    // route starts with travel from the left side to the first row, and the lead-in takes its place: from
    // the lines' node nearest the needle to the row's start. Without that travel the route starts with
    // the row, which it keeps, and the lead-in ends at the row's start too.
    let part = rectangle(10.0);
    let meter = &mut Budget::DEFAULT.meter();
    let rings = Rings::new(&part, meter).unwrap();
    let how = stitching(false);
    let rows = segments(&part, &how, meter).unwrap();
    let needle = p(3.0, 4.6);
    let mut route = route(&rings, &rows.0, Some(needle), None, meter).unwrap().unwrap();
    let Some(&Step::Row { from: start, .. }) = route.steps.get(1) else { panic!("{:?}", route.steps) };
    let at = route.nodes[start].point;
    let travel = Travel { underpath: true, ..TRAVEL };
    for replaces in [true, false] {
        if !replaces {
            route.steps.remove(0);
        }
        let lines = Underpath::new(&part, &rings, &route.nodes, &rows.0, 0.0, meter).unwrap();
        let mut sewing = Sewing { travel, lines, start: Some(needle), ..plain(&route, &part, &rings, &rows, &how, meter) };
        let (way, replaced) = sewing.lead_in(meter).unwrap().unwrap();
        // The needle is 3 mm or more from every side, so a node within 1 mm of it is well inside.
        assert!(way[0].distance(needle) < 1.0, "{:?}", way[0]);
        assert_eq!((way.last(), replaced), (Some(&at), replaces));
        let runs = sewing.sew(&mut SplitMix64::new(1), meter, &mut Vec::new()).unwrap();
        assert_eq!((runs.len(), runs[0].first()), (1, way.first()));
        let row = runs[0].iter().position(|&q| q == at).unwrap();
        assert!((runs[0][row + 1].y() - at.y()).abs() < 1e-9, "the row follows the lead-in");
    }
}

/// A 10 × 9 rectangle's rows at y = 1 to 8, each from x = 0 to 10, sewn by a route in 2 stretches: rows
/// 1 to `first` back and forth from the left, travel along the outline to the end of row 8 on the same
/// side, and rows 8 down to `first + 1`. The runs it sews with `gap_rows`, and with `pull`.
fn two_stretches(first: usize, gap_rows: usize, pull: Pull) -> Vec<Vec<Point>> {
    let part = Polygon { outline: vec![p(0.0, 0.0), p(0.0, 9.0), p(10.0, 9.0), p(10.0, 0.0), p(0.0, 0.0)], holes: Vec::new() };
    let meter = &mut Budget::DEFAULT.meter();
    let rings = Rings::new(&part, meter).unwrap();
    let segments: Vec<(Point, Point)> = (1..=8).map(|y| (p(0.0, f64::from(y)), p(10.0, f64::from(y)))).collect();
    let rows: Vec<usize> = (0..8).collect();
    // Row k's left end is node 2k, and its right end node 2k + 1.
    let nodes = segments.iter().flat_map(|&(a, b)| [a, b]).map(|q| Node { point: q, place: rings.locate(q, |_| true, meter).unwrap().unwrap() });
    let mut steps = Vec::new();
    let sew = |steps: &mut Vec<Step>, row: usize, rightwards: bool| {
        let (left, right) = (2 * row, 2 * row + 1);
        let (from, to) = if rightwards { (left, right) } else { (right, left) };
        if let Some(&Step::Row { to: before, .. }) = steps.last() {
            steps.push(Step::Travel { from: before, to: from, beside: row.abs_diff(before / 2) == 1 });
        }
        steps.push(Step::Row { segment: row, from, to });
    };
    for row in 0..first {
        sew(&mut steps, row, row % 2 == 0);
    }
    let side = first % 2;
    for (k, row) in (first..8).rev().enumerate() {
        sew(&mut steps, row, (k % 2 == 0) == (side == 0));
    }
    let route = Route { nodes: nodes.collect(), steps };
    let (how, rows) = (stitching(false), (segments, rows));
    let mut warnings = Vec::new();
    let mut sewing = Sewing { pull, gap_rows, ..plain(&route, &part, &rings, &rows, &how, meter) };
    let runs = sewing.sew(&mut SplitMix64::new(1), meter, &mut warnings).unwrap();
    assert_eq!(warnings, Vec::new());
    runs
}

/// Whether `run` passes through `points` in that order.
fn passes(run: &[Point], points: &[(f64, f64)]) -> bool {
    let mut rest = run.iter();
    points.iter().all(|&(x, y)| rest.any(|q| q.distance(p(x, y)) < 1e-9))
}

#[test]
fn req_fill_tat_007_leaving_a_stretch_of_more_than_3_rows_sews_the_gap_rows_past_its_last_and_comes_back() {
    // Rows 1 to 5, 4 of them after travel: leaving row 5's right end, 2 rows more, 6 back from the right
    // and 7 from the left, then straight back to row 5's end, and on round the outline to row 8.
    let run = two_stretches(5, 2, Pull::default()).concat();
    assert!(passes(&run, &[(10.0, 5.0), (10.0, 6.0), (0.0, 6.0), (0.0, 7.0), (10.0, 7.0), (10.0, 5.0), (10.0, 8.0)]), "{run:?}");
    // Without gap rows, row 6 is sewn once, after rows 8 and 7.
    let plain = two_stretches(5, 0, Pull::default()).concat();
    assert!(passes(&plain, &[(10.0, 5.0), (10.0, 8.0), (0.0, 8.0), (0.0, 7.0), (10.0, 7.0), (10.0, 6.0), (0.0, 6.0)]), "{plain:?}");
    // Between them, the gap rows' needle points as any row's, and the way back's one stitch.
    let rng = &mut SplitMix64::new(0);
    let mut row = |a: (f64, f64), b: (f64, f64)| row_points(p(a.0, a.1), p(b.0, b.1), &stitching(false), rng, &mut Budget::DEFAULT.meter()).unwrap();
    let gap: Vec<Point> = [row((10.0, 6.0), (0.0, 6.0)), row((0.0, 7.0), (10.0, 7.0)), vec![p(10.0, 5.0)]].concat();
    let from = run.iter().position(|q| *q == p(10.0, 5.0)).unwrap() + 1;
    assert_eq!(run[from..from + gap.len()], gap);
    assert_eq!(run.len(), plain.len() + gap.len());
}

#[test]
fn req_fill_tat_007_a_stretch_of_3_rows_after_travel_has_no_gap_rows() {
    // Rows 1 to 4: 3 of them after travel.
    assert_eq!(two_stretches(4, 2, Pull::default()), two_stretches(4, 0, Pull::default()));
}

#[test]
fn req_fill_tat_006_rows_and_gap_rows_reach_past_their_ends_and_travel_joins_the_ends() {
    // 0.5 mm past each row's left end and 0.25 mm plus a tenth of its 10 mm past its right end: rows
    // from x = -0.5 to 11.25, gap rows too, each handing over straight to the next.
    let pull = Pull { mm: [0.5, 0.25], share: [0.0, 0.1] };
    let run = two_stretches(5, 2, pull).concat();
    let ends = [(11.25, 5.0), (11.25, 6.0), (-0.5, 6.0), (-0.5, 7.0), (11.25, 7.0), (11.25, 5.0), (11.25, 8.0), (-0.5, 8.0)];
    assert!(passes(&run, &ends), "{run:?}");
    assert!(run.iter().all(|q| (-0.5 - 1e-9..=11.25 + 1e-9).contains(&q.x())), "{run:?}");
    // 12 mm past each row's left end, more than the row's own 10 mm: each row is still sewn from the end the
    // route enters it by, row 1 rightwards and row 2 back.
    let far = two_stretches(5, 0, Pull { mm: [12.0, 0.0], share: [0.0, 0.0] }).concat();
    assert!(passes(&far, &[(-12.0, 1.0), (10.0, 1.0), (10.0, 2.0), (-12.0, 2.0)]), "{far:?}");
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
    let (travel, how, rows) = (Travel { underpath: true, ..TRAVEL }, stitching(false), (vec![(points[1], points[0])], vec![0]));
    let sewing = Sewing { travel, lines: Some(lines), network, ..plain(&route, &part, &rings, &rows, &how, meter) };
    assert_eq!(sewing.way(0, 2, meter).unwrap(), Some(points.to_vec()));
}
