//! The meander fill's own tests: each tile's path through a part sewn as one run inside it, passing no
//! point twice, from the needle to the end; what the fill says about parts too small for its tile; and how
//! the path is sewn: zigzag, clip, bean stitch and repeats.

use std::collections::BTreeSet;

use stitchcraft_core::Budget;
use stitchcraft_core::math::hypot;

use super::*;
use crate::normalize::fixture::part;

const TRAVEL: Travel = Travel { length: 2.5, tolerance: 0.2, min_stitch: 0.3, underpath: true, jump: 3.0 };

/// A meander fill of `tile` at a scale of 100 %, not turned, with no underlay, sewn as a running stitch.
fn meander(tile: &'static str) -> Meander {
    Meander {
        tile,
        placement: Placement { scale: (1.0, 1.0), angle: 0.0 },
        smoothness: 0.0,
        zigzag: None,
        clip: false,
        beans: Vec::new(),
        repeats: 1,
        underlay: Vec::new(),
        inset: 0.0,
        expand: 0.0,
        force_locks: false,
        travel: TRAVEL,
    }
}

fn sewn_with(region: &Region, meander: &Meander, needle: Option<Point>, seed: u64) -> Stitched {
    meander_fill(region, meander, needle, None, &mut SplitMix64::new(seed), &mut Budget::DEFAULT.meter()).unwrap()
}

fn sewn(region: &Region, meander: &Meander) -> Stitched {
    sewn_with(region, meander, Some(Point::ORIGIN), 7)
}

fn square(low: f64, high: f64) -> Polygon {
    part(&[(low, low), (high, low), (high, high), (low, high)], &[])
}

fn region(parts: Vec<Polygon>) -> Region {
    Region { parts }
}

fn codes(stitched: &Stitched) -> Vec<Code> {
    stitched.warnings.iter().map(|w| w.code).collect()
}

/// The ids of StitchCraft's own tiles, those a meander fill offers.
fn ours() -> impl Iterator<Item = &'static str> {
    tiles::PATTERNS.iter().filter(|t| t.offered).map(|t| t.id)
}

/// Whether every point of `run` lies in the square from `low` to `high` across and down.
fn inside(run: &[Point], low: f64, high: f64) -> bool {
    let within = |v: f64| (low - 1e-9..=high + 1e-9).contains(&v);
    run.iter().all(|q| within(q.x()) && within(q.y()))
}

/// The path of `meander` over the one part of `region`, as its points.
fn path_of(region: &Region, meander: &Meander) -> Vec<Point> {
    let segments = top_path(region, meander, &SplitMix64::new(7), &mut Budget::DEFAULT.meter()).unwrap();
    segments.first().map(|s| s.start).into_iter().chain(segments.iter().map(|s| s.end)).collect()
}

#[test]
fn each_tile_sews_a_square_as_one_run_inside_it() {
    for tile in ours() {
        let stitched = sewn(&region(vec![square(0.0, 20.0)]), &meander(tile));
        assert_eq!(stitched.runs.len(), 1, "{tile}");
        let run = &stitched.runs[0];
        assert!(run.len() > 50, "{tile}: {} stitches", run.len());
        assert!(inside(run, 0.0, 20.0), "{tile}");
        assert!(run.windows(2).all(|w| w[0].distance(w[1]) <= TRAVEL.length + 1e-9), "{tile}: a stitch longer than the longest");
        assert_eq!(codes(&stitched), Vec::new(), "{tile}");
    }
}

#[test]
fn the_path_passes_no_point_twice_from_the_needle_to_half_way_round() {
    let part = square(0.0, 20.0);
    let rings = Rings::new(&part, &mut Budget::DEFAULT.meter()).unwrap();
    let start = part.outline[0];
    for tile in ours() {
        let path = path_of(&region(vec![part.clone()]), &meander(tile));
        let places: BTreeSet<(u64, u64)> = path.iter().map(|p| (p.x().to_bits(), p.y().to_bits())).collect();
        assert_eq!(places.len(), path.len(), "{tile}: a point passed twice");
        // Every point is one of the network's, the first nearest the outline's first point, where a fill
        // with no needle starts, and the last nearest the point half way round the outline from it.
        let laid = tiles::tile(tile).unwrap();
        let network = network::network(&part, &rings, &laid, meander(tile).placement, &mut Budget::DEFAULT.meter()).unwrap().unwrap();
        let nearest = |to: Point| (0..network.len()).map(|i| network.point(i)).min_by(|a, b| a.distance(to).total_cmp(&b.distance(to))).unwrap();
        assert!(path.iter().all(|p| network.points.contains(p)), "{tile}");
        assert_eq!(path.first().copied(), Some(nearest(start)), "{tile}");
        assert_eq!(path.last().copied(), Some(nearest(across(&part.outline, start))), "{tile}");
        // The detours take it through most of the network.
        assert!(path.len() * 3 > network.len() * 2, "{tile}: {} of {} points", path.len(), network.len());
    }
}

#[test]
fn the_point_half_way_round_a_square_from_a_corner_is_the_opposite_corner() {
    let outline = square(0.0, 20.0).outline;
    assert_eq!(across(&outline, Point::ORIGIN), Point::new(20.0, 20.0).unwrap());
    // From a point off the outline, it measures from the outline's point nearest it.
    assert_eq!(across(&outline, Point::new(-1.0, 5.0).unwrap()), Point::new(20.0, 15.0).unwrap());
}

#[test]
fn the_same_seed_sews_the_same_path_and_another_seed_another() {
    let square = region(vec![square(0.0, 20.0)]);
    let honeycomb = meander(tiles::DEFAULT);
    let seven = sewn_with(&square, &honeycomb, None, 7);
    assert_eq!(sewn_with(&square, &honeycomb, None, 7), seven);
    assert_ne!(sewn_with(&square, &honeycomb, None, 8).runs, seven.runs);
}

#[test]
fn a_tile_stitchcraft_does_not_have_is_sewn_as_the_default() {
    let square = region(vec![square(0.0, 20.0)]);
    assert_eq!(sewn(&square, &meander("N3-11a")), sewn(&square, &meander(tiles::DEFAULT)));
}

#[test]
fn diag_sc_w0317_a_part_too_small_for_its_tile_is_sewn_round_its_outline() {
    let tiny = region(vec![square(0.0, 1.0)]);
    let stitched = sewn(&tiny, &meander(tiles::DEFAULT));
    assert_eq!(codes(&stitched), [Code::FillMeanderTooSmall]);
    assert_eq!(
        stitched.warnings[0].message,
        "This meander fill is too small for any line of its tile, so it is sewn as a running stitch round its outline."
    );
    assert_eq!(stitched.runs.len(), 1);
    let on_outline = |p: &Point| [p.x(), p.y()].iter().any(|v| v.abs() < 1e-9 || (v - 1.0).abs() < 1e-9);
    assert!(stitched.runs[0].iter().all(on_outline), "{:?}", stitched.runs[0]);
    // Sides of 0.2 mm fit: the 3 hexagons round the part's middle, which a point of the tile lies at.
    let mut small = meander(tiles::DEFAULT);
    small.placement.scale = (0.1, 0.1);
    assert_eq!(codes(&sewn(&tiny, &small)), Vec::new());
}

#[test]
fn the_tally_says_how_many_parts_are_sewn_round_their_outline() {
    let said = |pieces, outlined| Tally { pieces, outlined }.said().map(|d| (d.code, d.message));
    assert_eq!(said(3, 0), None);
    assert_eq!(said(2, 1).map(|(code, _)| code), Some(Code::FillMeanderTooSmall));
    assert_eq!(
        said(2, 1).map(|(_, message)| message).as_deref(),
        Some("1 of this meander fill's parts is too small for any line of its tile, so it is sewn as a running stitch round its outline.")
    );
    assert_eq!(
        said(3, 2).map(|(_, message)| message).as_deref(),
        Some("2 of this meander fill's parts are too small for any line of its tile, so each is sewn as a running stitch round its outline.")
    );
}

#[test]
fn each_part_ends_nearest_the_next() {
    // 2 squares 10 mm apart, farther than a jump: the first, nearest the needle, ends near the second.
    let two = region(vec![square(0.0, 10.0), part(&[(20.0, 0.0), (30.0, 0.0), (30.0, 10.0), (20.0, 10.0)], &[])]);
    let stitched = sewn(&two, &meander(tiles::DEFAULT));
    assert_eq!(stitched.runs.len(), 2);
    let end = stitched.runs[0].last().copied().unwrap();
    assert!(end.x() > 10.0 - 2.0 * network::SIDE_MM, "the first part ends at {end:?}");
    assert!(stitched.runs[1].iter().all(|p| p.x() >= 20.0 - 1e-9));
}

#[test]
fn a_zigzag_moves_each_point_across_the_line_to_one_side_and_then_the_other() {
    let at = |x: f64, y: f64| Point::new(x, y).unwrap();
    let line = [at(0.0, 0.0), at(0.5, 0.0), at(1.0, 0.0), at(1.5, 0.0)];
    // A quarter turn from the way along +x, (x, y) to (−y, x), is +y: down as drawn.
    assert_eq!(zigzag(&line, 2.0), [at(0.0, 1.0), at(0.5, -1.0), at(1.0, 1.0), at(1.5, 0.0)]);
    // A point where the line has no length stays where it is.
    assert_eq!(zigzag(&[at(0.0, 0.0), at(0.0, 0.0), at(1.0, 0.0)], 2.0), [at(0.0, 0.0), at(0.0, -1.0), at(1.0, 0.0)]);
    // Along (0.6, 0.8), 5 mm a step, 1 mm wide: (−0.8, 0.6) half a millimetre one way, then the other.
    let slanting = zigzag(&[at(1.0, 2.0), at(4.0, 6.0), at(7.0, 10.0), at(10.0, 14.0)], 1.0);
    let expected = [at(0.6, 2.3), at(4.4, 5.7), at(6.6, 10.3), at(10.0, 14.0)];
    assert!(slanting.iter().zip(&expected).all(|(a, b)| a.distance(*b) < 1e-12) && slanting.len() == 4, "{slanting:?}");
}

#[test]
fn a_zigzag_strays_from_the_part_unless_clipped() {
    let square = region(vec![square(0.0, 20.0)]);
    let mut wide = meander(tiles::DEFAULT);
    wide.zigzag = Some((1.0, 3.0));
    let loose = sewn(&square, &wide);
    assert!(!inside(&loose.runs.concat(), 0.0, 20.0));
    // Without clip, the zigzag is the stitches: 2 to every millimetre of the path.
    let path = path_of(&square, &wide);
    let long: f64 = path.windows(2).map(|w| w[0].distance(w[1])).sum();
    assert!((loose.runs.concat().len() as f64 - 2.0 * long).abs() < 0.1 * long, "{} stitches over {long} mm", loose.runs.concat().len());
    wide.clip = true;
    let clipped = sewn(&square, &wide);
    assert!(inside(&clipped.runs.concat(), 0.0, 20.0));
    assert!(clipped.runs.concat().windows(2).all(|w| w[0].distance(w[1]) <= TRAVEL.length + 1e-9));
}

#[test]
fn smoothing_rounds_the_turns_of_the_path() {
    let square = region(vec![square(0.0, 20.0)]);
    let sharp = sewn(&square, &meander("squares"));
    let mut round = meander("squares");
    round.smoothness = 1.0;
    let smoothed = sewn(&square, &round);
    assert_ne!(smoothed.runs, sharp.runs);
    // How many times the run turns more than 60 degrees from one stitch to the next.
    let turns = |run: &[Point]| {
        run.windows(3)
            .filter(|w| {
                let (a, b) = ((w[1].x() - w[0].x(), w[1].y() - w[0].y()), (w[2].x() - w[1].x(), w[2].y() - w[1].y()));
                a.0 * b.0 + a.1 * b.1 < 0.5 * hypot(a.0, a.1) * hypot(b.0, b.1)
            })
            .count()
    };
    // A tile of squares turns square corners, at 2 stitches of every 3 here; smoothed, the path rounds them,
    // and turns so sharply at fewer than half as many of its stitches (a sixth here, where it turns back).
    let (sharp, smoothed) = (&sharp.runs[0], &smoothed.runs[0]);
    assert!(
        turns(smoothed) * sharp.len() * 2 < turns(sharp) * smoothed.len(),
        "{} of {}, smoothed {} of {}",
        turns(sharp),
        sharp.len(),
        turns(smoothed),
        smoothed.len()
    );
}

#[test]
fn repeats_double_the_path_and_bean_stitch_triples_each_stitch() {
    let square = region(vec![square(0.0, 10.0)]);
    let once = sewn(&square, &meander(tiles::DEFAULT)).runs[0].len();
    let mut twice = meander(tiles::DEFAULT);
    twice.repeats = 3;
    // 4 passes, the needle staying where each turns round.
    assert_eq!(sewn(&square, &twice).runs[0].len(), 4 * once - 3);
    let mut bold = meander(tiles::DEFAULT);
    bold.beans = vec![1];
    assert_eq!(sewn(&square, &bold).runs[0].len(), 3 * once - 2);
}

#[test]
fn the_settings_read_as_the_tile_its_placement_and_the_zigzag() {
    let read = |pairs: &[(&str, &str)]| MeanderParams::from_set(&pairs.iter().copied().collect()).unwrap().params;
    assert_eq!(read(&[]).tile(), ("honeycomb", false));
    assert_eq!(read(&[("meander_pattern", "squares")]).tile(), ("squares", false));
    // One of Ink/Stitch's tiles is kept, and sews the default.
    assert_eq!(read(&[("meander_pattern", "N3-11a")]).tile(), ("honeycomb", true));
    assert_eq!(read(&[]).placement(), Placement { scale: (1.0, 1.0), angle: 0.0 });
    let laid = read(&[("meander_scale_percent", "50 200"), ("meander_angle", "90")]).placement();
    assert_eq!(laid.scale, (0.5, 2.0));
    assert!((laid.angle - std::f64::consts::FRAC_PI_2).abs() < 1e-12);
    assert_eq!(read(&[]).zigzag(), None);
    assert_eq!(read(&[("zigzag_spacing_mm", "1.5"), ("zigzag_width_mm", "2")]).zigzag(), Some((1.5, 2.0)));
}
