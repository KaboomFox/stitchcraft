//! A tatami fill sewn (`REQ-FILL-TAT-001`, `REQ-FILL-TAT-008`): every row segment once, back and forth in
//! the order Ink/Stitch routes them, from the point nearest the needle to the point nearest the next
//! element; parts one at a time, nearest the needle first, each ending nearest the next. Between rows the
//! needle travels along the outline, or with `underpath` inside the part, under the rows not sewn yet
//! (`REQ-FILL-TAT-005`). Before its top rows each part sews its underlay, a layer for each angle, each
//! back to where it started (`REQ-FILL-TAT-011`). A part too thin for a row is sewn round its outline
//! (`SC-W0305`), and a layer of underlay too thin for one is too (`SC-I0308`).

// Test code may unwrap and index (clippy.toml allows it in tests).
#![allow(clippy::unwrap_used, clippy::panic, clippy::indexing_slicing)]

use stitchcraft_core::math::sin_cos;
use stitchcraft_core::rng::SplitMix64;
use stitchcraft_core::{Budget, Code, ElementId, Point};
use stitchcraft_engine::design::{Element, FillRule, Path, Segment, Shape, Subpath};
use stitchcraft_engine::generators::Approach;
use stitchcraft_engine::generators::tatami::rows::{Grid, rows};
use stitchcraft_engine::generators::tatami::sew::{Layer, Travel, tatami_fill};
use stitchcraft_engine::generators::tatami::stitches::{Stitching, row_points};
use stitchcraft_engine::normalize::region::{Polygon, Region, build};
use stitchcraft_params::ParamSet;
use stitchcraft_plan::{Role, StitchKind};
use stitchcraft_testkit::designs::{RED, line, messages, p, planned};

/// A path of closed subpaths, one through each list of points.
fn rings(parts: &[&[(f64, f64)]]) -> Path {
    let subpaths = parts
        .iter()
        .filter_map(|points| {
            let (&(x, y), rest) = points.split_first()?;
            Some(Subpath { start: p(x, y), segments: rest.iter().map(|&(x, y)| Segment::Line(p(x, y))).collect(), closed: true })
        })
        .collect();
    Path { subpaths }
}

fn region(parts: &[&[(f64, f64)]]) -> Region {
    build(&rings(parts), FillRule::EvenOdd, &mut Budget::DEFAULT.meter()).unwrap().region
}

/// Rows at `angle`, 1 mm apart, in stitches of 3 mm staggered over 4 rows.
fn stitching(angle: f64) -> Stitching {
    Stitching { grid: Grid { angle, spacing: 1.0, end_spacing: None }, length: 3.0, staggers: 4.0, skip_last: false, jitter: None }
}

/// Travel along the outline in stitches of 2 mm, a shortest stitch too short to thin anything, and the
/// design's jump length.
const TRAVEL: Travel = Travel { length: 2.0, tolerance: 0.2, min_stitch: 1e-6, underpath: false, jump: 3.0 };
/// The same, under the rows.
const UNDER: Travel = Travel { underpath: true, ..TRAVEL };

fn sewn(region: &Region, how: &Stitching, needle: Option<Point>, next: Option<&Approach>) -> Vec<Vec<Point>> {
    sewn_with(TRAVEL, region, how, needle, next)
}

fn sewn_with(travel: Travel, region: &Region, how: &Stitching, needle: Option<Point>, next: Option<&Approach>) -> Vec<Vec<Point>> {
    layered(region, &Layer { stitching: *how, travel }, &[], needle, next)
}

/// The runs of a fill over `region`, its `top` rows after its `underlay`, which says nothing about it.
fn layered(region: &Region, top: &Layer, underlay: &[Layer], needle: Option<Point>, next: Option<&Approach>) -> Vec<Vec<Point>> {
    let stitched = tatami_fill(region, top, underlay, needle, next, &mut SplitMix64::new(1), &mut Budget::DEFAULT.meter()).unwrap();
    assert_eq!(stitched.warnings, Vec::new());
    stitched.runs
}

/// Layers of underlay at `angles`, rows 3 mm apart in stitches of 3 mm, travelling under the rows.
fn underlay(angles: &[f64]) -> Vec<Layer> {
    let layer = |angle| Layer { stitching: Stitching { grid: Grid { spacing: 3.0, ..stitching(angle).grid }, ..stitching(angle) }, travel: UNDER };
    angles.iter().copied().map(layer).collect()
}

/// How far `q` is from the segment from `a` to `b`.
fn to_segment(q: Point, a: Point, b: Point) -> f64 {
    let (dx, dy) = (b.x() - a.x(), b.y() - a.y());
    let t = (((q.x() - a.x()) * dx + (q.y() - a.y()) * dy) / (dx * dx + dy * dy)).clamp(0.0, 1.0);
    q.distance(a.lerp(b, t))
}

/// How far `q` is from `part`'s rings.
fn to_rings(part: &Polygon, q: Point) -> f64 {
    std::iter::once(&part.outline)
        .chain(&part.holes)
        .flat_map(|ring| ring.windows(2).map(|w| to_segment(q, w[0], w[1])))
        .fold(f64::INFINITY, f64::min)
}

/// Whether `q` lies in `part`, or within a nanometre of its rings.
fn on_or_in(part: &Polygon, q: Point) -> bool {
    near_or_in(part, q, 1e-9)
}

/// Whether `q` lies in `part`, or within `within` of its rings.
fn near_or_in(part: &Polygon, q: Point, within: f64) -> bool {
    let rings: Vec<&Vec<Point>> = std::iter::once(&part.outline).chain(&part.holes).collect();
    let near = to_rings(part, q) <= within;
    let crossings = |ring: &Vec<Point>| {
        ring.windows(2)
            .filter(|w| {
                (w[0].y() <= q.y()) != (w[1].y() <= q.y()) && w[0].x() + (q.y() - w[0].y()) / (w[1].y() - w[0].y()) * (w[1].x() - w[0].x()) > q.x()
            })
            .count()
    };
    near || rings.iter().map(|ring| crossings(ring)).sum::<usize>() % 2 == 1
}

/// A frame: a 12 × 9 square with a 4 × 3 hole, and an L, whose rows run through its inner corner.
const FRAME: [&[(f64, f64)]; 2] = [&[(0.0, 0.0), (12.0, 0.0), (12.0, 9.0), (0.0, 9.0)], &[(4.0, 3.0), (8.0, 3.0), (8.0, 6.0), (4.0, 6.0)]];
const ELL: [&[(f64, f64)]; 1] = [&[(0.0, 0.0), (10.0, 0.0), (10.0, 4.0), (4.0, 4.0), (4.0, 8.0), (0.0, 8.0)]];

#[test]
fn req_fill_tat_001_every_row_is_sewn_once_in_one_run() {
    let shapes = [(&FRAME[..], 0.0), (&FRAME[..], 30.0), (&ELL[..], 0.0), (&ELL[..], 90.0), (&ELL[..], 45.0)];
    for ((shape, angle), travel) in shapes.into_iter().flat_map(|shape| [(shape, TRAVEL), (shape, UNDER)]) {
        let region = region(shape);
        let how = stitching(angle);
        let runs = sewn_with(travel, &region, &how, None, None);
        assert_eq!(runs.len(), 1, "one part, one run");
        let run = &runs[0];
        // Each row segment's needle points between its ends turn up in the run once, one after another, one
        // way or the other. Travel along an edge a row runs along can land on one of the same points, but not
        // on 2 in a row: its stitches are shorter than the rows'.
        let part = &region.parts[0];
        let mut rows_checked = 0;
        for row in rows(part, &how.grid, &mut Budget::DEFAULT.meter()).unwrap() {
            for segment in row.segments {
                let points = row_points(segment.start, segment.end, &how, &mut SplitMix64::new(1), &mut Budget::DEFAULT.meter()).unwrap();
                let inside = &points[1..points.len() - 1];
                if inside.is_empty() {
                    continue;
                }
                // Sewn from its other end, a point is worked out from there and may differ in its last bits.
                let same = |a: &[Point], b: &mut dyn Iterator<Item = &Point>| a.iter().zip(b).all(|(x, y)| x.distance(*y) < 1e-9);
                let times = run.windows(inside.len()).filter(|w| same(w, &mut inside.iter()) || same(w, &mut inside.iter().rev())).count();
                if inside.len() > 1 {
                    assert_eq!(times, 1, "angle {angle}: {segment:?}");
                } else {
                    assert!(times >= 1, "angle {angle}: {segment:?}");
                }
                rows_checked += 1;
            }
        }
        assert!(rows_checked > 5, "{rows_checked}");
        // Every needle point lies in the part or on its outline: rows inside, travel along the rings or
        // inside the part, where the way under the rows is kept.
        assert!(run.iter().all(|q| on_or_in(part, *q)), "angle {angle}: a point outside");
    }
}

/// A ring 30 mm across with a hole 16 mm across, drawn as polygons of 72 and 48 sides.
fn annulus() -> Vec<Vec<(f64, f64)>> {
    let circle = |r: f64, n: u32| {
        (0..n).map(|i| sin_cos(f64::from(i) / f64::from(n) * std::f64::consts::TAU)).map(|(sin, cos)| (15.0 + r * cos, 15.0 + r * sin)).collect()
    };
    vec![circle(15.0, 72), circle(8.0, 48)]
}

#[test]
fn req_fill_tat_005_travel_runs_inside_the_part_under_the_rows_not_sewn_yet() {
    let ring = annulus();
    let ring: Vec<&[(f64, f64)]> = ring.iter().map(Vec::as_slice).collect();
    for (shape, angle) in [(&FRAME[..], 0.0), (&FRAME[..], 30.0), (&ELL[..], 0.0), (&ELL[..], 45.0), (&ring[..], 0.0), (&ring[..], 60.0)] {
        let region = region(shape);
        let part = &region.parts[0];
        let how = Stitching { grid: Grid { spacing: 0.4, ..stitching(angle).grid }, ..stitching(angle) };
        let segments: Vec<(Point, Point)> =
            rows(part, &how.grid, &mut Budget::DEFAULT.meter()).unwrap().into_iter().flat_map(|row| row.segments).map(|s| (s.start, s.end)).collect();
        let on_a_row = |q: Point| segments.iter().any(|&(a, b)| to_segment(q, a, b) < 1e-9);
        // How many travel needle points lie off the rows and at least 0.5 mm from the rings.
        let inside = |travel: Travel| {
            let runs = sewn_with(travel, &region, &how, None, None);
            let mut inside = 0;
            for run in &runs {
                // Row stitches run between 2 needle points of a row; the rest is travel.
                let mut sewn: Vec<(Point, Point)> = Vec::new();
                for w in run.windows(2) {
                    let (a, b) = (w[0], w[1]);
                    if segments.iter().any(|&(s, e)| to_segment(a, s, e) < 1e-9 && to_segment(b, s, e) < 1e-9) {
                        sewn.push((a, b));
                        continue;
                    }
                    // Travel crosses no row already sewn, keeps inside the part (within 0.05 mm of it), and
                    // with `underpath` leaves the rows and rings.
                    let crosses = |&(c, d): &(Point, Point)| {
                        let side = |o: Point, s: Point, t: Point| (s.x() - o.x()) * (t.y() - o.y()) - (s.y() - o.y()) * (t.x() - o.x());
                        side(a, b, c) * side(a, b, d) < -1e-12 && side(c, d, a) * side(c, d, b) < -1e-12
                    };
                    assert!(!sewn.iter().any(crosses), "angle {angle}: travel {a:?} to {b:?} over a row already sewn");
                    assert!(near_or_in(part, b, 0.05), "angle {angle}: {b:?} outside");
                    if !on_a_row(b) && to_rings(part, b) >= 0.5 {
                        inside += 1;
                    }
                }
            }
            inside
        };
        assert_eq!(inside(TRAVEL), 0, "angle {angle}: along the outline and the rows");
        assert!(inside(UNDER) > 0, "angle {angle}: under the rows");
    }
}

#[test]
fn req_fill_tat_001_the_fill_starts_nearest_the_needle_and_ends_nearest_the_next_element() {
    let region = region(&ELL);
    // The needle left of the L's foot, the next element starting right of its top: the fill starts on the
    // left side, level with the needle, and ends on the top side, above the next element's start.
    // Points along a side are worked out, so they are compared to a nanometre.
    let near = |a: Point, b: Point| a.distance(b) < 1e-9;
    let runs = sewn(&region, &stitching(0.0), Some(p(-3.0, 6.0)), Some(&Approach::Point(p(2.0, -5.0))));
    let run = &runs[0];
    assert!(near(run[0], p(0.0, 6.0)) && near(*run.last().unwrap(), p(2.0, 0.0)), "{:?}", (run[0], run.last()));
    // A next element inside the fill: the fill ends on the outline nearest its start.
    let runs = sewn(&region, &stitching(0.0), None, Some(&Approach::Point(p(8.0, 3.5))));
    assert!(near(*runs[0].last().unwrap(), p(8.0, 4.0)), "{:?}", runs[0].last());
    // A next element that offers its shape: the first of its lines that starts in the fill ends it nearest
    // that start, here on the left side level with it, and otherwise the fill ends at its point nearest the
    // shape, here the corner of the L's foot.
    let starts_in = Approach::Shape(vec![vec![p(30.0, 30.0), p(40.0, 30.0)], vec![p(1.0, 3.0), p(1.0, 20.0)]]);
    let runs = sewn(&region, &stitching(0.0), None, Some(&starts_in));
    assert!(near(*runs[0].last().unwrap(), p(0.0, 3.0)), "{:?}", runs[0].last());
    let beside = Approach::Shape(vec![vec![p(20.0, 6.0), p(30.0, 6.0)]]);
    let runs = sewn(&region, &stitching(0.0), None, Some(&beside));
    assert!(near(*runs[0].last().unwrap(), p(10.0, 4.0)), "{:?}", runs[0].last());
    // With neither, it starts at its first row's start and comes back there.
    let runs = sewn(&region, &stitching(0.0), None, None);
    let first_row = rows(&region.parts[0], &stitching(0.0).grid, &mut Budget::DEFAULT.meter()).unwrap()[0].segments[0];
    assert_eq!((runs[0][0], *runs[0].last().unwrap()), (first_row.start, first_row.start));
}

#[test]
fn req_fill_tat_008_parts_are_sewn_nearest_the_needle_first_each_ending_nearest_the_next() {
    let left: &[(f64, f64)] = &[(0.0, 0.0), (6.0, 0.0), (6.0, 4.0), (0.0, 4.0)];
    let right: &[(f64, f64)] = &[(10.0, 1.0), (16.0, 1.0), (16.0, 5.0), (10.0, 5.0)];
    let apart = region(&[left, right]);
    assert_eq!(apart.parts.len(), 2);
    // The needle right of both: the right part first, ending on its left side nearest the left part, which
    // it then starts nearest.
    let runs = sewn(&apart, &stitching(0.0), Some(p(20.0, 3.0)), None);
    assert_eq!(runs.len(), 2);
    assert!(runs[0].iter().all(|q| q.x() >= 10.0) && runs[1].iter().all(|q| q.x() <= 6.0));
    assert_eq!(runs[0].last().unwrap().x(), 10.0);
    assert_eq!(runs[1][0].x(), 6.0);
    // With no needle, the part reaching farthest left first.
    let runs = sewn(&apart, &stitching(0.0), None, None);
    assert!(runs[0].iter().all(|q| q.x() <= 6.0));
    assert_eq!(runs[0].last().unwrap().x(), 6.0, "it ends nearest the right part");
    // A part the needle is in is 0 away, ahead of a part as near its outline: here a triangle drawn first,
    // touching the square, with a hole, at the point of the square's outline nearest the needle.
    let triangle: &[(f64, f64)] = &[(4.0, 2.0), (8.0, 0.0), (8.0, 4.0)];
    let square: &[(f64, f64)] = &[(0.0, 0.0), (4.0, 0.0), (4.0, 4.0), (0.0, 4.0)];
    let hole: &[(f64, f64)] = &[(0.5, 0.5), (0.5, 1.5), (1.5, 1.5), (1.5, 0.5)];
    let touching = region(&[triangle, square, hole]);
    assert_eq!(touching.parts.len(), 2);
    let runs = sewn(&touching, &stitching(0.0), Some(p(3.0, 2.0)), None);
    assert!(runs[0].iter().all(|q| q.x() <= 4.0), "the square first: {:?}", runs[0]);
}

/// A fill element `id` of `parts`.
fn fill(id: &str, parts: &[&[(f64, f64)]]) -> Element {
    Element {
        id: ElementId::new(id).unwrap(),
        name: None,
        shape: Shape::Fill { path: rings(parts), rule: FillRule::NonZero },
        thread: RED,
        params: ParamSet::default(),
    }
}

#[test]
fn diag_sc_w0305_a_fill_too_thin_for_a_row_is_sewn_round_its_outline() {
    // 0.15 mm tall, between the rows at 0 and 0.25: no row meets it.
    let sliver: &[(f64, f64)] = &[(0.0, 0.05), (20.0, 0.05), (20.0, 0.2), (0.0, 0.2)];
    let outcome = planned(vec![fill("sliver", &[sliver]), line("ok", (0.0, 40.0), 10.0, &RED, &[])]);
    assert_eq!(
        messages(&outcome),
        ["warning SC-W0305: This fill is too thin for a row of stitches at its row spacing, so it is sewn as a running stitch round its outline."]
    );
    let region = region(&[sliver]);
    let top = Layer { stitching: Stitching { grid: Grid { angle: 0.0, spacing: 0.25, end_spacing: None }, ..stitching(0.0) }, travel: TRAVEL };
    let stitched = tatami_fill(&region, &top, &[], None, None, &mut SplitMix64::new(1), &mut Budget::DEFAULT.meter()).unwrap();
    let run = &stitched.runs[0];
    assert_eq!((run[0], *run.last().unwrap()), (p(0.0, 0.05), p(0.0, 0.05)), "round its outline from its start");
    // Its underlay at 90° has rows across it, and comes back to its first row's start, on its left side.
    // The top rows then go round the outline from its start, 0.15 mm away: within the jump length the
    // needle goes straight on there, and beyond it they are a run of their own, which assembly reaches with
    // a jump, as Ink/Stitch's plan joins them.
    let layered = |jump| {
        let top = Layer { travel: Travel { jump, ..top.travel }, ..top };
        let under = underlay(&[90.0]).into_iter().map(|layer| Layer { travel: Travel { jump, ..layer.travel }, ..layer }).collect::<Vec<_>>();
        tatami_fill(&region, &top, &under, None, None, &mut SplitMix64::new(1), &mut Budget::DEFAULT.meter()).unwrap().runs
    };
    let (on, apart) = (layered(0.2), layered(0.1));
    assert_eq!((on.len(), apart.len()), (1, 2));
    assert_eq!((apart[0][0], *apart[0].last().unwrap()), (p(0.0, 0.2), p(0.0, 0.2)));
    assert_eq!(&apart[1], run);
    assert_eq!(on[0], [&apart[0][..], &apart[1][..]].concat());
    // One part of 2 too thin, and 2 of 3.
    let square: &[(f64, f64)] = &[(0.0, 10.0), (5.0, 10.0), (5.0, 15.0), (0.0, 15.0)];
    let outcome = planned(vec![fill("two", &[sliver, square])]);
    assert!(messages(&outcome).contains(
        &"warning SC-W0305: 1 of this fill's parts is too thin for a row of stitches at its row spacing, so it is sewn as a running stitch round \
          its outline."
            .to_string()
    ));
    let lower: &[(f64, f64)] = &[(0.0, 30.05), (20.0, 30.05), (20.0, 30.2), (0.0, 30.2)];
    let outcome = planned(vec![fill("three", &[sliver, lower, square])]);
    assert!(
        messages(&outcome).contains(
            &"warning SC-W0305: 2 of this fill's parts are too thin for a row of stitches at its row spacing, so each is sewn as a running stitch \
          round its outline."
                .to_string()
        )
    );
}

#[test]
fn req_fill_tat_001_routing_is_charged_to_the_budget() {
    let region = region(&FRAME);
    let top = Layer { stitching: stitching(0.0), travel: TRAVEL };
    let mut meter = Budget::DEFAULT.meter();
    tatami_fill(&region, &top, &[], None, None, &mut SplitMix64::new(1), &mut meter).unwrap();
    let work = Budget::DEFAULT.max_work - meter.work_left();
    assert!(work > 100, "{work}");
    let tight = &mut Budget { max_stitches: 1_000_000, max_work: work - 1 }.meter();
    assert!(tatami_fill(&region, &top, &[], None, None, &mut SplitMix64::new(1), tight).is_err());
    // The underlay is charged too: the same budget does not reach the top rows after it.
    let tight = &mut Budget { max_stitches: 1_000_000, max_work: work }.meter();
    assert!(tatami_fill(&region, &top, &underlay(&[90.0]), None, None, &mut SplitMix64::new(1), tight).is_err());
}

#[test]
fn req_fill_tat_011_each_layer_comes_back_to_where_it_started_and_the_top_rows_follow() {
    // The frame from outside its right side, the L from beside its inner corner, and the frame with no
    // needle, where the first layer starts at its first row's start.
    let next = Approach::Point(p(-2.0, 0.0));
    for (shape, needle) in [(&FRAME[..], Some(p(13.0, 4.0))), (&ELL[..], Some(p(5.0, 5.5))), (&FRAME[..], None)] {
        let region = region(shape);
        let (top, underlay) = (Layer { stitching: stitching(30.0), travel: UNDER }, underlay(&[120.0, 0.0]));
        let alone = |layer: &Layer, needle: Option<Point>, next: Option<&Approach>| {
            let runs = layered(&region, layer, &[], needle, next);
            assert_eq!(runs.len(), 1);
            runs.into_iter().next().unwrap()
        };
        // Each layer, sewn with nothing after it, comes back to where it started, and the next starts there,
        // as the top rows do: each a ring's point worked out again, so to a nanometre.
        let first = alone(&underlay[0], needle, None);
        let start = first[0];
        let second = alone(&underlay[1], Some(start), None);
        let rows = alone(&top, Some(start), Some(&next));
        for q in [first.last(), second.first(), second.last(), rows.first()] {
            assert!(q.unwrap().distance(start) < 1e-9, "{needle:?}: {q:?} is not {start:?}");
        }
        // Sewn together, the part is one run: the layers in turn, then the top rows, each going on from
        // where the last ended.
        let together = layered(&region, &top, &underlay, needle, Some(&next));
        assert_eq!(together, [[&first[..], &second[1..], &rows[1..]].concat()], "{needle:?}");
    }
    // Without a needle the first layer starts at its first row's start.
    let region = region(&FRAME);
    let how = underlay(&[120.0])[0].stitching;
    let first_row = rows(&region.parts[0], &how.grid, &mut Budget::DEFAULT.meter()).unwrap()[0].segments[0];
    let runs = layered(&region, &Layer { stitching: stitching(30.0), travel: UNDER }, &underlay(&[120.0]), None, None);
    assert_eq!(runs[0][0], first_row.start);
}

#[test]
fn req_fill_tat_011_a_fill_sews_its_underlay_first_unless_it_is_turned_off() {
    // A 10 mm square at 0°: its underlay's rows stand upright, 0.75 mm apart, in stitches of 4 mm, where
    // its top rows lie flat and travel runs in stitches of 2.5 mm. The first long upright stitch comes
    // before the first long flat one, and with the underlay off there is none.
    let square: &[(f64, f64)] = &[(0.0, 0.0), (10.0, 0.0), (10.0, 10.0), (0.0, 10.0)];
    let first_long = |params: &[(&str, &str)]| {
        let element = Element { params: params.iter().copied().collect(), ..fill("square", &[square]) };
        let outcome = planned(vec![element]);
        let plan = outcome.plan.unwrap();
        let stitches: Vec<Point> = plan.stitches().filter(|s| s.kind == StitchKind::Normal && s.origin.role == Role::Top).map(|s| s.at).collect();
        let long = |upright: bool| {
            stitches.windows(2).position(|w| {
                let (dx, dy) = ((w[1].x() - w[0].x()).abs(), (w[1].y() - w[0].y()).abs());
                if upright { dx < 1e-9 && dy > 3.0 } else { dy < 1e-9 && dx > 3.0 }
            })
        };
        (long(true), long(false))
    };
    let (upright, flat) = first_long(&[]);
    assert!(upright.unwrap() < flat.unwrap(), "{upright:?} {flat:?}");
    assert_eq!(first_long(&[("fill_underlay", "false")]).0, None);
    // At the angles it is given, in turn: flat first, then upright.
    let (upright, flat) = first_long(&[("fill_underlay_angle", "0 90")]);
    assert!(flat.unwrap() < upright.unwrap(), "{upright:?} {flat:?}");
}

#[test]
fn diag_sc_i0308_a_layer_of_underlay_too_thin_for_a_row_is_sewn_round_the_outline() {
    // Upright bars 0.5 mm wide, between the upright underlay rows at 0, 0.75, … 3 and 3.75 mm, which flat
    // top rows 0.25 mm apart cross.
    let bar: &[(f64, f64)] = &[(0.1, 0.0), (0.6, 0.0), (0.6, 20.0), (0.1, 20.0)];
    let other: &[(f64, f64)] = &[(3.1, 0.0), (3.6, 0.0), (3.6, 20.0), (3.1, 20.0)];
    let outcome = planned(vec![fill("bar", &[bar])]);
    assert_eq!(
        messages(&outcome),
        ["info SC-I0308: 1 layer of this fill's underlay finds no row at the underlay's row spacing, so it is sewn as a running stitch round \
          its part's outline."]
    );
    // 2 bars, each with an upright layer and a flat one, whose rows cross them: 2 of 4 layers find none.
    let both = Element { params: [("fill_underlay_angle", "90 0")].into_iter().collect(), ..fill("bars", &[bar, other]) };
    let shown = messages(&planned(vec![both]));
    assert!(
        shown.contains(
            &"info SC-I0308: 2 layers of this fill's underlay find no row at the underlay's row spacing, so each is sewn as a running stitch \
              round its part's outline."
                .to_string()
        ),
        "{shown:?}"
    );
    // Round the outline from its start, back to it, and the top rows from there.
    let region = region(&[bar]);
    let top = Layer { stitching: Stitching { grid: Grid { angle: 0.0, spacing: 0.25, end_spacing: None }, ..stitching(0.0) }, travel: TRAVEL };
    let thin = Layer { stitching: Stitching { grid: Grid { angle: 90.0, spacing: 0.75, end_spacing: None }, ..stitching(90.0) }, travel: TRAVEL };
    let stitched = tatami_fill(&region, &top, &[thin], None, None, &mut SplitMix64::new(1), &mut Budget::DEFAULT.meter()).unwrap();
    assert_eq!(stitched.warnings.iter().map(|d| d.code).collect::<Vec<_>>(), [Code::FillUnderlayTooThin]);
    assert_eq!(stitched.runs.len(), 1);
    let outline = &region.parts[0].outline;
    let back = stitched.runs[0].iter().skip(1).position(|q| q == &outline[0]).unwrap() + 1;
    assert!(outline.iter().all(|corner| stitched.runs[0][..=back].contains(corner)), "{:?}", &stitched.runs[0][..=back]);
}
