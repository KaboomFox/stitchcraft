//! A meander fill (`REQ-FILL-MEA-001` to `REQ-FILL-MEA-005`): a line that wanders through the sides of a
//! tiling inside each part without passing a point twice, after the tatami fill's underlay; and a value
//! only the node's other stitch type reads, which an element leaves to it (`REQ-PRM-006`). The tiles, the
//! network, the path and how it is stitched are tested in the generator's modules
//! (`generators::meander`); these cases sew whole designs.

// Test code may unwrap, index and cast (clippy.toml allows the first 2 in tests).
#![allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::panic, clippy::cast_possible_truncation, clippy::cast_precision_loss)]

use std::collections::BTreeSet;

use stitchcraft_core::{Code, Point};
use stitchcraft_engine::PlanOutcome;
use stitchcraft_plan::{Role, StitchKind};
use stitchcraft_testkit::designs::{RED, along, fill, messages, p, planned, polylines};

/// The ids of StitchCraft's meander tiles.
const TILES: [&str; 7] =
    ["honeycomb", "squares", "triangles", "octagons-squares", "hexagons-triangles", "hexagons-squares-triangles", "squares-triangles"];

/// The square `size` mm a side from (`x`, 0).
fn square(x: f64, size: f64) -> Vec<(f64, f64)> {
    vec![(x, 0.0), (x + size, 0.0), (x + size, size), (x, size)]
}

/// A meander fill of `rings` with `own` settings, with no underlay unless `own` says otherwise, planned
/// alone.
fn meander(rings: &[&[(f64, f64)]], own: &[(&str, &str)]) -> PlanOutcome {
    let mut params = vec![("fill_method", "meander_fill"), ("fill_underlay", "false")];
    params.extend_from_slice(own);
    planned(vec![fill("meander", rings, &RED, &params)])
}

/// The needle points `outcome` sews, its locks left out.
fn needles(outcome: &PlanOutcome) -> Vec<Point> {
    let plan = outcome.plan.as_ref().unwrap_or_else(|| panic!("{:?}", messages(outcome)));
    plan.stitches().filter(|s| s.kind == StitchKind::Normal && s.origin.role != Role::Lock).map(|s| s.at).collect()
}

/// The codes `outcome` said.
fn codes(outcome: &PlanOutcome) -> Vec<Code> {
    outcome.diagnostics.iter().map(|d| d.code).collect()
}

/// The lengths of the stitches between `points`.
fn lengths(points: &[Point]) -> Vec<f64> {
    points.windows(2).map(|w| w[0].distance(w[1])).collect()
}

#[test]
fn req_fill_mea_001_each_tile_sews_lines_a_side_long_scaled_and_turned() {
    // At 100 %, every line is 2 mm long, and the running stitch of 2.5 mm sews no stitch longer. Where the
    // path runs straight on along several lines, it spreads its stitches evenly along them.
    for tile in TILES {
        let sewn = meander(&[&square(0.0, 20.0)], &[("meander_pattern", tile)]);
        assert_eq!(codes(&sewn), Vec::new(), "{tile}");
        let points = needles(&sewn);
        assert!(points.len() > 50, "{tile}: {} needle points", points.len());
        assert!(lengths(&points).iter().all(|&l| l > 1.0 && l <= 2.5 + 1e-6), "{tile}");
    }
    // The honeycomb turns at every point, so each stitch is a line, 2 mm long, and its points lie where the
    // tiling puts them with a point at the middle of the part, (10, 10): across, 0 or 2 mm on from a whole
    // number of 3 mm, and down, a whole number of √3 mm.
    let honeycomb = needles(&meander(&[&square(0.0, 20.0)], &[]));
    assert!(lengths(&honeycomb).iter().all(|l| (l - 2.0).abs() < 1e-6));
    let whole = |v: f64| (v - v.round()).abs() < 1e-6;
    let placed = |q: &Point| whole((q.y() - 10.0) / 3.0_f64.sqrt()) && (whole((q.x() - 10.0) / 3.0) || whole((q.x() - 12.0) / 3.0));
    assert!(honeycomb.iter().all(placed));
    // Each line of the honeycomb as a stitch, (dx, dy) without their signs, each to 1 µm.
    let sides = |points: &[Point]| -> BTreeSet<(i64, i64)> {
        let micrometres = |v: f64| (v.abs() * 1000.0).round() as i64;
        points.windows(2).map(|w| (micrometres(w[1].x() - w[0].x()), micrometres(w[1].y() - w[0].y()))).collect()
    };
    assert_eq!(sides(&honeycomb), BTreeSet::from([(2000, 0), (1000, 1732)]));
    // Scaled to 200 % across and 100 % down, in stitches long enough for each line.
    let stretched = needles(&meander(&[&square(0.0, 20.0)], &[("meander_scale_percent", "200 100"), ("running_stitch_length_mm", "5")]));
    assert_eq!(sides(&stretched), BTreeSet::from([(4000, 0), (2000, 1732)]));
    // Turned a quarter turn clockwise as drawn: the lines across run down.
    let turned = needles(&meander(&[&square(0.0, 20.0)], &[("meander_angle", "90")]));
    assert_eq!(sides(&turned), BTreeSet::from([(0, 2000), (1732, 1000)]));
}

#[test]
fn req_fill_mea_002_the_lines_keep_out_of_holes_and_a_part_too_small_is_sewn_round_its_outline() {
    // A hole 6 mm across in the middle of a square, the other way round: no needle point inside it, and no
    // stitch crosses it.
    let hole = [(7.0, 7.0), (7.0, 13.0), (13.0, 13.0), (13.0, 7.0)];
    let points = needles(&meander(&[&square(0.0, 20.0), &hole], &[]));
    let inside = |q: &Point| q.x() > 7.0 + 1e-9 && q.x() < 13.0 - 1e-9 && q.y() > 7.0 + 1e-9 && q.y() < 13.0 - 1e-9;
    assert!(!points.iter().any(inside));
    assert!(points.windows(2).all(|w| !inside(&w[0].lerp(w[1], 0.5))));
    // A square 1.5 mm a side, larger than a fill too small to sew, holds no line of the honeycomb: sewn
    // round its outline, with SC-W0317.
    let tiny = meander(&[&square(30.0, 1.5)], &[]);
    assert_eq!(codes(&tiny), [Code::FillMeanderTooSmall]);
    let on_outline = |q: &Point| [q.x() - 30.0, q.y()].iter().any(|v| v.abs() < 1e-9 || (v - 1.5).abs() < 1e-9);
    assert!(needles(&tiny).iter().all(on_outline));
}

#[test]
fn diag_sc_w0317_a_part_too_small_for_the_tile_is_sewn_round_its_outline() {
    // One part of 2 is too small: said once, for that part, after the parts are said.
    let two = meander(&[&square(0.0, 20.0), &square(30.0, 1.0)], &[]);
    assert_eq!(codes(&two), [Code::FillPartsApart, Code::FillMeanderTooSmall]);
    assert_eq!(
        messages(&two)[1],
        "warning SC-W0317: 1 of this meander fill's parts is too small for any line of its tile, so it is sewn as a running stitch round its \
         outline."
    );
}

#[test]
fn req_fill_mea_003_the_path_passes_no_point_twice_and_its_seed_chooses_it() {
    let square = square(0.0, 20.0);
    let points = needles(&meander(&[&square], &[]));
    let places: BTreeSet<(u64, u64)> = points.iter().map(|q| (q.x().to_bits(), q.y().to_bits())).collect();
    assert_eq!(places.len(), points.len(), "a point passed twice");
    // The same seed sews the same path, and another seed another.
    let seeded = |seed: &str| needles(&meander(&[&square], &[("random_seed", seed)]));
    assert_eq!(seeded("1"), seeded("1"));
    assert_ne!(seeded("1"), seeded("2"));
    // It starts at the point of the network nearest the needle, at the origin.
    assert!(points[0].distance(p(0.0, 0.0)) < 3.0, "{:?}", points[0]);
}

#[test]
fn req_fill_mea_004_the_path_zigzags_is_clipped_and_sews_bean_stitch_and_repeats() {
    let square = square(0.0, 20.0);
    let plain = needles(&meander(&[&square], &[]));
    let long: f64 = lengths(&plain).iter().sum();
    // A zigzag of points half its spacing apart along the path, 3 mm wide: 2 to every millimetre of the
    // path, and some outside the square.
    let zigzag = needles(&meander(&[&square], &[("zigzag_spacing_mm", "1"), ("zigzag_width_mm", "3")]));
    assert!((zigzag.len() as f64 - 2.0 * long).abs() < 0.1 * long, "{} needle points over {long} mm", zigzag.len());
    let within = |q: &Point| (-1e-6..=20.0 + 1e-6).contains(&q.x()) && (-1e-6..=20.0 + 1e-6).contains(&q.y());
    assert!(!zigzag.iter().all(within));
    // Clipped, it stays inside the square, in running stitches.
    let clipped = needles(&meander(&[&square], &[("zigzag_spacing_mm", "1"), ("zigzag_width_mm", "3"), ("clip", "true")]));
    assert!(clipped.iter().all(within));
    assert!(lengths(&clipped).iter().all(|&l| l <= 2.5 + 1e-6));
    // Each repeat sews everything so far again the other way, and bean stitch sews each stitch 3 times.
    let count = |own: &[(&str, &str)]| needles(&meander(&[&square], own)).len();
    assert_eq!(count(&[("repeats", "2")]), 2 * plain.len() - 1);
    assert_eq!(count(&[("bean_stitch_repeats", "1")]), 3 * plain.len() - 2);
    // Smoothed, the path turns less sharply.
    assert_ne!(needles(&meander(&[&square], &[("smoothness_mm", "1")])), plain);
}

#[test]
fn req_fill_mea_005_the_underlay_comes_first_and_each_part_ends_nearest_the_next() {
    // With the default underlay, rows square to the fill's angle of 0 run up and down, then the path, in
    // one run: the only jump is the one to the fill's start.
    let jumps = |outcome: &PlanOutcome| outcome.plan.as_ref().unwrap().stitches().filter(|s| s.kind == StitchKind::Jump).count();
    let sewn = meander(&[&square(0.0, 20.0)], &[("fill_underlay", "true")]);
    assert_eq!((jumps(&sewn), codes(&sewn)), (1, Vec::new()));
    let points = needles(&sewn);
    let upright: Vec<bool> = lengths(&points).iter().zip(points.windows(2)).map(|(_, w)| (w[1].x() - w[0].x()).abs() < 1e-9).collect();
    let last_row = upright.windows(2).rposition(|u| u[0] && u[1]).unwrap();
    // The honeycomb has no upright line: after the rows, the path's lines, each 2 mm long, but for the
    // step from the underlay's end to the path.
    let after = lengths(&points[last_row + 3..]);
    let lines = after.iter().filter(|l| (*l - 2.0).abs() < 1e-6).count();
    assert!(last_row > 10 && lines * 100 >= after.len() * 95, "rows to {last_row}, then {lines} of {} lines", after.len());
    // 2 squares 10 mm apart: the first, nearest the needle, ends near the second, on its side towards it.
    let two = meander(&[&square(0.0, 10.0), &square(20.0, 10.0)], &[]);
    let points = needles(&two);
    let first_end = points.iter().take_while(|q| q.x() < 15.0).last().unwrap();
    assert!(first_end.x() > 10.0 - 4.0, "the first part ends at {first_end:?}");
}

#[test]
fn diag_sc_w0316_one_of_ink_stitch_s_tiles_sews_the_honeycomb() {
    let square = square(0.0, 20.0);
    let theirs = meander(&[&square], &[("meander_pattern", "N3-11a")]);
    assert_eq!(
        messages(&theirs),
        ["warning SC-W0316: This meander fill's pattern, `N3-11a`, is one of Ink/Stitch's tiles, which StitchCraft doesn't have, so it wanders \
             through the honeycomb tile instead."]
    );
    assert_eq!(needles(&theirs), needles(&meander(&[&square], &[])));
    // A pattern that is no tile of either is refused.
    assert_eq!(codes(&meander(&[&square], &[("meander_pattern", "no-such-tile")]))[0], Code::ParamInvalid);
}

#[test]
fn req_prm_006_a_meander_fill_leaves_a_zigzag_stroke_s_2_spacings_to_it() {
    // A zigzag stroke's 2 spacings, which a meander fill on the same node cannot read as its one: it sews
    // without a zigzag, as Ink/Stitch's does, and says so.
    let square = square(0.0, 20.0);
    let two = meander(&[&square], &[("zigzag_spacing_mm", "0.4 0.8")]);
    assert_eq!(
        messages(&two),
        ["info SC-I0106: `zigzag_spacing_mm` is \"0.4 0.8\", which only a zigzag stitch reads, so this meander fill takes its default, 0, as \
             Ink/Stitch's does."]
    );
    assert_eq!(needles(&two), needles(&meander(&[&square], &[])));
    // One spacing is the fill's own, and it zigzags.
    assert_ne!(needles(&meander(&[&square], &[("zigzag_spacing_mm", "1")])), needles(&meander(&[&square], &[])));
    // A value no declaration of the key reads is the element's own, and refused.
    assert_eq!(codes(&meander(&[&square], &[("zigzag_spacing_mm", "wide")]))[0], Code::ParamInvalid);
}

#[test]
fn diag_sc_i0106_a_satin_column_leaves_a_zigzag_stroke_s_2_spacings_to_it() {
    // A satin column given 2 spacings sews with its own default, 0.4 mm, as Ink/Stitch's does.
    let satin = |own: &[(&str, &str)]| {
        let path = polylines(&[&[(0.0, 30.0), (10.0, 30.0)], &[(0.0, 36.0), (10.0, 36.0)]]);
        planned(vec![along("satin", path, &RED, &[&[("satin_column", "true")], own].concat())])
    };
    let left = satin(&[("zigzag_spacing_mm", "0.4 0.8")]);
    assert_eq!(
        messages(&left),
        ["info SC-I0106: `zigzag_spacing_mm` is \"0.4 0.8\", which only a zigzag stitch reads, so this satin column takes its default, 0.4, \
             as Ink/Stitch's does."]
    );
    assert_eq!(needles(&left), needles(&satin(&[])));
}
