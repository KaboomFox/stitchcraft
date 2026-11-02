//! A circular fill (`REQ-FILL-CIR-001` to `REQ-FILL-CIR-006`): a double spiral round the middle of each
//! part, cut by the part, its pieces routed as a tatami fill's rows, after the tatami fill's underlay. The
//! circles, the cutting and the sewing of the pieces are tested in the generator's modules
//! (`generators::circular`, `generators::tatami::sew::lines`); these cases sew whole designs.

// Test code may unwrap, index and panic (clippy.toml allows the first 2 in tests).
#![allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::panic)]

use stitchcraft_core::{Code, Point};
use stitchcraft_engine::PlanOutcome;
use stitchcraft_plan::{Role, StitchKind};
use stitchcraft_testkit::designs::{RED, fill, messages, p, planned};

/// The square `size` mm a side from (`x`, 0).
fn square(x: f64, size: f64) -> Vec<(f64, f64)> {
    vec![(x, 0.0), (x + size, 0.0), (x + size, size), (x, size)]
}

/// A disc of `radius` round (`x`, `y`): a ring of 128 points.
fn disc((x, y): (f64, f64), radius: f64) -> Vec<(f64, f64)> {
    (0..128)
        .map(|k| {
            let (sin, cos) = stitchcraft_core::math::sin_cos(std::f64::consts::TAU * f64::from(k) / 128.0);
            (x + radius * cos, y + radius * sin)
        })
        .collect()
}

/// A circular fill of `rings` with `own` settings, with no underlay unless `own` says otherwise, planned
/// alone.
fn circular(rings: &[&[(f64, f64)]], own: &[(&str, &str)]) -> PlanOutcome {
    let mut params = vec![("fill_method", "circular_fill"), ("fill_underlay", "false")];
    params.extend_from_slice(own);
    planned(vec![fill("circular", rings, &RED, &params)])
}

/// The needle points `outcome` sews, its locks left out.
fn needles(outcome: &PlanOutcome) -> Vec<Point> {
    let plan = outcome.plan.as_ref().unwrap_or_else(|| panic!("{:?}", messages(outcome)));
    plan.stitches().filter(|s| s.kind == StitchKind::Normal && s.origin.role != Role::Lock).map(|s| s.at).collect()
}

/// The jumps `outcome` sews.
fn jumps(outcome: &PlanOutcome) -> usize {
    outcome.plan.as_ref().unwrap().stitches().filter(|s| s.kind == StitchKind::Jump).count()
}

/// The codes `outcome` said.
fn codes(outcome: &PlanOutcome) -> Vec<Code> {
    outcome.diagnostics.iter().map(|d| d.code).collect()
}

#[test]
fn req_fill_cir_001_the_spiral_turns_round_the_part_s_centroid() {
    // An L 20 mm a side with arms 10 mm wide: its centroid is (8⅓, 8⅓), off the middle of its bounds, and
    // the spiral turns in to it, its needle points within 2 row spacings of it there.
    let l = [(0.0, 0.0), (20.0, 0.0), (20.0, 10.0), (10.0, 10.0), (10.0, 20.0), (0.0, 20.0)];
    let sewn = circular(&[&l], &[]);
    assert_eq!(codes(&sewn), Vec::new());
    let centroid = p(25.0 / 3.0, 25.0 / 3.0);
    let nearest = needles(&sewn).iter().map(|q| q.distance(centroid)).fold(f64::INFINITY, f64::min);
    assert!(nearest <= 0.5, "{nearest}");
    // Circles 1 mm apart fading to 2 mm sew fewer stitches than circles 1 mm apart, and more than circles
    // 2 mm apart.
    let count = |own: &[(&str, &str)]| needles(&circular(&[&square(0.0, 20.0)], own)).len();
    let (even, fading, wide) =
        (count(&[("row_spacing_mm", "1")]), count(&[("row_spacing_mm", "1"), ("end_row_spacing_mm", "2")]), count(&[("row_spacing_mm", "2")]));
    assert!(even > fading && fading > wide, "{even} {fading} {wide}");
}

#[test]
fn req_fill_cir_002_every_needle_point_lies_in_the_part() {
    // A square round a hole set off its middle: the spiral's pieces and the travel between them keep to the
    // part, out of the hole.
    let hole = [(12.0, 6.0), (12.0, 10.0), (16.0, 10.0), (16.0, 6.0)];
    let sewn = circular(&[&square(0.0, 20.0), &hole], &[]);
    assert_eq!((codes(&sewn), jumps(&sewn)), (Vec::new(), 1));
    let points = needles(&sewn);
    assert!(points.len() > 500);
    let near = 1e-6;
    let in_square = |q: &Point| (-near..=20.0 + near).contains(&q.x()) && (-near..=20.0 + near).contains(&q.y());
    let in_hole = |q: &Point| q.x() > 12.0 + near && q.x() < 16.0 - near && q.y() > 6.0 + near && q.y() < 10.0 - near;
    assert!(points.iter().all(|q| in_square(q) && !in_hole(q)));
}

#[test]
fn req_fill_cir_003_a_round_part_cut_once_is_one_run_from_its_outline() {
    // A disc 10 mm round, circles 5 mm apart: the spiral enters it once and leaves once. With no needle, it
    // starts where the spiral enters, on the outline, and sews one run.
    let sewn = circular(&[&disc((20.0, 20.0), 10.0)], &[("row_spacing_mm", "5")]);
    assert_eq!((codes(&sewn), jumps(&sewn)), (Vec::new(), 1));
    let points = needles(&sewn);
    assert!((points[0].distance(p(20.0, 20.0)) - 10.0).abs() < 0.05, "{:?}", points[0]);
    // The same seed sews the same random lengths, and another seed others.
    let random = |seed: &str| {
        needles(&circular(&[&disc((20.0, 20.0), 10.0)], &[("row_spacing_mm", "5"), ("enable_random_stitch_length", "true"), ("random_seed", seed)]))
    };
    assert_eq!(random("1"), random("1"));
    assert_ne!(random("1"), random("2"));
}

#[test]
fn req_fill_cir_004_the_pieces_are_routed_in_one_run_and_skip_last_leaves_their_ends_out() {
    // A star-like square: past its inscribed circle the spiral is many pieces, routed in one run.
    let square = square(0.0, 20.0);
    let sewn = circular(&[&square], &[]);
    assert_eq!((codes(&sewn), jumps(&sewn)), (Vec::new(), 1));
    let all = needles(&sewn).len();
    // With skip_last, each piece's last needle point is left out where the needle goes on to a piece beside
    // it: fewer points.
    let skipped = needles(&circular(&[&square], &[("skip_last", "true")])).len();
    assert!(skipped < all, "{skipped} {all}");
    // With underpath off, the needle travels along the outline instead: as many pieces, other travel.
    let along = circular(&[&square], &[("underpath", "false")]);
    assert_eq!((codes(&along), jumps(&along)), (Vec::new(), 1));
    assert_ne!(needles(&along), needles(&sewn));
}

#[test]
fn req_fill_cir_006_the_underlay_comes_first_and_each_part_ends_nearest_the_next() {
    // With the default underlay, rows up and down the square, then the spiral, in one run: after the
    // underlay's last row, no 2 stitches in a row run up or down.
    let sewn = circular(&[&square(0.0, 20.0)], &[("fill_underlay", "true")]);
    assert_eq!((jumps(&sewn), codes(&sewn)), (1, Vec::new()));
    let points = needles(&sewn);
    let upright: Vec<bool> = points.windows(2).map(|w| (w[1].x() - w[0].x()).abs() < 1e-9).collect();
    let last_row = upright.windows(2).rposition(|u| u[0] && u[1]).unwrap();
    assert!(last_row > 10 && last_row < points.len() / 4, "rows to {last_row} of {}", points.len());
    // 2 squares 10 mm apart: the first, nearest the needle, ends near the second, on its side towards it.
    let two = circular(&[&square(0.0, 10.0), &square(20.0, 10.0)], &[]);
    let points = needles(&two);
    let first_end = points.iter().take_while(|q| q.x() < 15.0).last().unwrap();
    assert!(first_end.x() > 10.0 - 1.0, "the first part ends at {first_end:?}");
    // Bean stitch sews the spiral 3 times over, and 2 repeats twice.
    let disc = disc((20.0, 20.0), 10.0);
    let once = needles(&circular(&[&disc], &[("row_spacing_mm", "5")])).len();
    let beaned = needles(&circular(&[&disc], &[("row_spacing_mm", "5"), ("bean_stitch_repeats", "1")])).len();
    let twice = needles(&circular(&[&disc], &[("row_spacing_mm", "5"), ("repeats", "2")])).len();
    assert_eq!((beaned, twice), (3 * once - 2, 2 * once - 1));
}

#[test]
fn diag_sc_w0318_a_part_within_its_row_spacing_of_its_middle_is_one_circle() {
    // A disc 1 mm round, circles 2 mm apart: one circle 2 mm round its middle, which reaches past it.
    let sewn = circular(&[&disc((10.0, 10.0), 1.0)], &[("row_spacing_mm", "2")]);
    assert_eq!(
        messages(&sewn),
        ["warning SC-W0318: This circular fill lies within its row spacing of its middle, so it is sewn as one circle of that radius."]
    );
    assert!(needles(&sewn).iter().all(|q| (q.distance(p(10.0, 10.0)) - 2.0).abs() < 0.2));
}
