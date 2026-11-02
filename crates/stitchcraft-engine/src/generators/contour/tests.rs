//! The contour fill's own tests: a part's path through its rings sewn as running stitches, bean stitch
//! and repeats, and what the fill says about rings it leaves out and parts too narrow for a spiral.

use stitchcraft_core::Budget;

use super::*;
use crate::normalize::fixture::part;

const TRAVEL: Travel = Travel { length: 2.5, tolerance: 0.2, min_stitch: 0.3, underpath: true, jump: 3.0 };

/// A contour fill with no underlay, rings 1 mm apart in stitches of 3 mm, as `strategy` lays them.
fn contour(strategy: Strategy) -> Contour {
    Contour {
        strategy,
        spacing: 1.0,
        corners: Corners::ROUND,
        clockwise: true,
        avoid_self_crossing: false,
        smoothness: 0.0,
        stitching: Stitching { length: 3.0, tolerance: 0.2, jitter: None, beans: Vec::new(), repeats: 1 },
        underlay: Vec::new(),
        inset: 0.0,
        expand: 0.0,
        force_locks: false,
        travel: TRAVEL,
    }
}

fn sewn(region: &Region, contour: &Contour, needle: Option<Point>) -> Stitched {
    contour_fill(region, contour, needle, &mut SplitMix64::new(7), &mut Budget::DEFAULT.meter()).unwrap()
}

fn square(size: f64) -> Region {
    Region { parts: vec![part(&[(0.0, 0.0), (size, 0.0), (size, size), (0.0, size)], &[])] }
}

fn codes(stitched: &Stitched) -> Vec<Code> {
    stitched.warnings.iter().map(|w| w.code).collect()
}

#[test]
fn each_strategy_sews_a_square_as_one_run_inside_it() {
    for strategy in [Strategy::InnerToOuter, Strategy::SingleSpiral, Strategy::DoubleSpiral] {
        let stitched = sewn(&square(20.0), &contour(strategy), Some(Point::ORIGIN));
        assert_eq!(stitched.runs.len(), 1, "{strategy:?}");
        let run = &stitched.runs[0];
        assert!(run.len() > 100, "{strategy:?}: {} stitches", run.len());
        assert!(run.iter().all(|q| (-1e-9..=20.0 + 1e-9).contains(&q.x()) && (-1e-9..=20.0 + 1e-9).contains(&q.y())), "{strategy:?}");
        assert!(run.windows(2).all(|w| w[0].distance(w[1]) <= 3.0 + 1e-9), "{strategy:?}: a stitch longer than the longest");
        assert_eq!(codes(&stitched), Vec::new(), "{strategy:?}");
    }
}

#[test]
fn diag_sc_i0315_a_part_too_narrow_for_a_spiral_is_sewn_inner_to_outer() {
    // A strip 1.5 mm wide has one ring inside its outline at most: no turn for either spiral.
    let strip = Region { parts: vec![part(&[(0.0, 0.0), (20.0, 0.0), (20.0, 1.5), (0.0, 1.5)], &[])] };
    for strategy in [Strategy::SingleSpiral, Strategy::DoubleSpiral] {
        let stitched = sewn(&strip, &contour(strategy), None);
        assert_eq!(codes(&stitched), [Code::FillContourSpiralTooShort], "{strategy:?}");
        // The fill has one part, so the message speaks of the fill.
        assert_eq!(stitched.warnings[0].message, "This contour fill is too narrow for its spiral, so it is sewn inner to outer.");
        let inner = sewn(&strip, &contour(Strategy::InnerToOuter), None);
        assert_eq!(stitched.runs, inner.runs, "{strategy:?}");
    }
    // A spiral of one turn of 2 points, a stitch, is sewn as a spiral: a strip 0.3 mm wide with one ring
    // inside its outline, 0.1 mm in, and 3.6 mm round, 2.4 points to a longest stitch of 30 mm.
    let narrow = part(&[(0.0, 0.0), (1.5, 0.0), (1.5, 0.3), (0.0, 0.3)], &[]);
    let mut spiral = contour(Strategy::SingleSpiral);
    spiral.spacing = 0.1;
    spiral.stitching.length = 30.0;
    let mut tally = Tally::default();
    let path = tally.path(&narrow, &spiral, Point::ORIGIN, &mut Budget::DEFAULT.meter()).unwrap();
    assert_eq!((path.concat().len(), tally.short), (2, 0), "{path:?}");
}

#[test]
fn diag_sc_e0009_a_level_whose_edges_do_not_join_is_said() {
    // A square 70 m from the origin, past the region grid's reach: its rings cannot be found, and the fill
    // says so, sewing its outline alone.
    let far = Region { parts: vec![part(&[(70_000.0, 0.0), (70_010.0, 0.0), (70_010.0, 10.0), (70_000.0, 10.0)], &[])] };
    let stitched = sewn(&far, &contour(Strategy::InnerToOuter), None);
    assert_eq!(codes(&stitched), [Code::InternalCheckFailed]);
    assert!(stitched.runs[0].iter().all(|q| q.x() >= 70_000.0 - 1e-9));
}

#[test]
fn diag_sc_w0311_rings_inner_to_outer_cannot_reach_are_said() {
    // A tree whose child lies 3 row spacings inside its parent: inner to outer cannot step to it, and leaves
    // it out with the ring under it.
    let ring = |a: f64, b: f64| [(a, a), (b, a), (b, b), (a, b), (a, a)].map(|(x, y)| Point::new(x, y).unwrap()).to_vec();
    let rings = tree::Tree {
        nodes: vec![
            tree::Node { ring: ring(0.0, 10.0), children: vec![1] },
            tree::Node { ring: ring(3.0, 7.0), children: vec![2] },
            tree::Node { ring: ring(4.0, 6.0), children: Vec::new() },
        ],
    };
    let mut tally = Tally::default();
    let piece = part(&[(0.0, 0.0), (10.0, 0.0), (10.0, 10.0), (0.0, 10.0)], &[]);
    tally.inner(&rings, &piece, &contour(Strategy::InnerToOuter), Point::ORIGIN, &mut Budget::DEFAULT.meter()).unwrap();
    let said: Vec<(Code, String)> = tally.said().map(|d| (d.code, d.message)).collect();
    assert_eq!(said, [(Code::FillContourRingsLeftOut, "This contour fill leaves out 2 of its rings, which its path does not reach.".to_string())]);
}

#[test]
fn each_span_draws_its_own_random_lengths() {
    // 2 stretches alike, 30 mm along x and 1 mm apart, at random lengths: each is a span of its own, its
    // draws keyed by its number, so their needle points fall at other places along them.
    let mut random = contour(Strategy::InnerToOuter);
    random.stitching.jitter = Some(0.5);
    let p = |x: f64, y: f64| Point::new(x, y).unwrap();
    let stretches = [vec![p(0.0, 0.0), p(30.0, 0.0)], vec![p(0.0, 1.0), p(30.0, 1.0)]];
    let run = Tally::default().stitched(&stretches, &random, &SplitMix64::new(7), &mut Budget::DEFAULT.meter()).unwrap();
    let along = |y: f64| run.iter().filter(|q| (q.y() - y).abs() < 1e-9).map(|q| q.x()).collect::<Vec<_>>();
    assert!(along(0.0).len() > 5, "{run:?}");
    assert_ne!(along(0.0), along(1.0));
}

#[test]
fn repeats_double_the_path_and_bean_stitch_triples_each_stitch() {
    let once = sewn(&square(10.0), &contour(Strategy::InnerToOuter), None).runs[0].len();
    let mut twice = contour(Strategy::InnerToOuter);
    twice.stitching.repeats = 3;
    // 4 passes, the needle staying where each turns round.
    let run = &sewn(&square(10.0), &twice, None).runs[0];
    assert_eq!(run.len(), 4 * once - 3);
    let mut bold = contour(Strategy::InnerToOuter);
    bold.stitching.beans = vec![1];
    assert_eq!(sewn(&square(10.0), &bold, None).runs[0].len(), 3 * once - 2);
}

#[test]
fn diag_sc_w0311_rings_a_spiral_leaves_out_are_said() {
    // 2 squares joined by a neck: shrunk past the neck, the outline ring has 2 children, and a spiral follows
    // one of them.
    let dumbbell = Region {
        parts: vec![part(
            &[
                (0.0, 0.0),
                (10.0, 0.0),
                (10.0, 4.0),
                (14.0, 4.0),
                (14.0, 0.0),
                (24.0, 0.0),
                (24.0, 10.0),
                (14.0, 10.0),
                (14.0, 6.0),
                (10.0, 6.0),
                (10.0, 10.0),
                (0.0, 10.0),
            ],
            &[],
        )],
    };
    let mut wide = contour(Strategy::SingleSpiral);
    wide.spacing = 1.5;
    let stitched = sewn(&dumbbell, &wide, None);
    assert_eq!(codes(&stitched), [Code::FillContourRingsLeftOut]);
    assert!(stitched.warnings[0].message.contains("of its rings"), "{}", stitched.warnings[0].message);
    // Inner to outer reaches both sides.
    wide.strategy = Strategy::InnerToOuter;
    assert_eq!(codes(&sewn(&dumbbell, &wide, None)), Vec::new());
}

#[test]
fn the_tally_says_each_count_once() {
    let said = |tally: Tally| tally.said().map(|d| (d.code, d.message)).collect::<Vec<_>>();
    assert_eq!(said(Tally::default()), Vec::new());
    let one = said(Tally { pieces: 1, left_out: 1, short: 1, failed: true, ..Tally::default() });
    assert_eq!(
        one.iter().map(|(code, _)| *code).collect::<Vec<_>>(),
        [Code::FillContourRingsLeftOut, Code::FillContourSpiralTooShort, Code::InternalCheckFailed]
    );
    assert_eq!(one[0].1, "This contour fill leaves out 1 of its rings, which its path does not reach.");
    assert_eq!(one[1].1, "This contour fill is too narrow for its spiral, so it is sewn inner to outer.");
    let more = said(Tally { pieces: 3, left_out: 4, short: 2, ..Tally::default() });
    assert_eq!(more[0].1, "This contour fill leaves out 4 of its rings, which its path does not reach.");
    assert_eq!(more[1].1, "2 of this contour fill's parts are too narrow for its spiral, so each is sewn inner to outer.");
    let one_of_two = said(Tally { pieces: 2, short: 1, ..Tally::default() });
    assert_eq!(one_of_two[0].1, "1 of this contour fill's parts is too narrow for its spiral, so it is sewn inner to outer.");
}

#[test]
fn the_settings_read_as_the_strategy_and_corners() {
    let read = |pairs: &[(&str, &str)]| ContourParams::from_set(&pairs.iter().copied().collect()).unwrap().params;
    assert_eq!(read(&[]).strategy(), Strategy::InnerToOuter);
    assert_eq!(read(&[("contour_strategy", "1")]).strategy(), Strategy::SingleSpiral);
    assert_eq!(read(&[("contour_strategy", "2")]).strategy(), Strategy::DoubleSpiral);
    assert_eq!(read(&[]).corners(), Corners { join: Join::Round, quadrant_segments: 8 });
    assert_eq!(read(&[("join_style", "1")]).corners().join, Join::Miter { limit: 10.0 });
    assert_eq!(read(&[("join_style", "2")]).corners().join, Join::Bevel);
}
