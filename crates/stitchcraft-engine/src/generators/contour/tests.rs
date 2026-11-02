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
        let inner = sewn(&strip, &contour(Strategy::InnerToOuter), None);
        assert_eq!(stitched.runs, inner.runs, "{strategy:?}");
    }
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
