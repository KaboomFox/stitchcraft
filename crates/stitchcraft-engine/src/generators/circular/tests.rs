//! Unit tests of the circular fill; the conformance cases are in `tests/fill_circular.rs`.

use stitchcraft_core::Budget;

use super::*;
use crate::generators::tatami::fixture::p;
use crate::normalize::stroke::distance_to_segment;

const TRAVEL: Travel = Travel { length: 2.5, tolerance: 0.2, min_stitch: 0.3, underpath: true, jump: 3.0 };

/// A circular fill of circles `spacing` apart, in running stitches of 2.5 mm, with no underlay.
fn circular(spacing: f64) -> Circular {
    Circular {
        target: None,
        spacing,
        end_spacing: None,
        stitching: Stitching { length: 2.5, tolerance: 0.2, jitter: None, beans: vec![0], repeats: 1 },
        skip_last: false,
        angle: 0.0,
        underlay: Vec::new(),
        inset: 0.0,
        expand: 0.0,
        force_locks: false,
        travel: TRAVEL,
    }
}

/// A disc of `radius` round (`x`, `y`), a ring of 128 points.
fn disc((x, y): (f64, f64), radius: f64) -> Polygon {
    let mut outline: Vec<Point> = (0..128)
        .map(|k| {
            let (sin, cos) = sin_cos(std::f64::consts::TAU * f64::from(k) / 128.0);
            p(x + radius * cos, y + radius * sin)
        })
        .collect();
    outline.push(outline[0]);
    Polygon { outline, holes: Vec::new() }
}

fn sew(part: &Polygon, circular: &Circular, needle: Option<Point>) -> Stitched {
    let region = Region { parts: vec![part.clone()] };
    circular_fill(&region, circular, needle, None, &mut SplitMix64::new(7), &mut Budget::DEFAULT.meter()).unwrap()
}

fn codes(stitched: &Stitched) -> Vec<Code> {
    stitched.warnings.iter().map(|w| w.code).collect()
}

#[test]
fn req_fill_cir_001_a_circle_has_64_sides_from_its_right_hand_point_turning_up_as_drawn() {
    let ring = circle(p(10.0, 5.0), 2.0);
    assert_eq!((ring.len(), ring[0], ring[64]), (65, p(12.0, 5.0), p(12.0, 5.0)));
    // A 64th of a turn on, up the page: y falls.
    let (sin, cos) = sin_cos(std::f64::consts::TAU / 64.0);
    assert!(ring[1].distance(p(10.0 + 2.0 * cos, 5.0 - 2.0 * sin)) < 1e-12, "{:?}", ring[1]);
    // A quarter of the way round, its top as drawn.
    assert!(ring[16].distance(p(10.0, 3.0)) < 1e-12, "{:?}", ring[16]);
    assert!(ring.iter().all(|q| (q.distance(p(10.0, 5.0)) - 2.0).abs() < 1e-12));
}

#[test]
fn req_fill_cir_001_circles_lie_a_spacing_apart_out_to_2_spacings_past_the_farthest_point() {
    let meter = &mut Budget::DEFAULT.meter();
    let radius = |ring: &[Point]| ring[0].x();
    // The farthest point 2 mm away, circles 0.5 apart: 0.5 to 2.5, as 2.5 is under 2 + 2 × 0.5, and the
    // circle at the middle, 0.1 CSS pixel round, last.
    let rings = circles(Point::ORIGIN, 2.0, 0.5, None, meter).unwrap();
    let radii: Vec<f64> = rings.iter().map(|ring| radius(ring)).collect();
    assert_eq!(radii[..5], [2.5, 2.0, 1.5, 1.0, 0.5]);
    assert_eq!(radii.len(), 6);
    assert!((radii[5] - MIDDLE).abs() < 1e-15);
    // Fading from 0.5 to 1 at the farthest point, 2 mm away: each step is 0.5 plus the share of 0.5 the
    // radius is of 2, then 1 past the farthest point, up to 2 + 2 × 1.
    let fading = circles(Point::ORIGIN, 2.0, 0.5, Some(1.0), meter).unwrap();
    let radii: Vec<f64> = fading.iter().rev().skip(1).map(|ring| radius(ring)).collect();
    let mut expected = vec![0.5];
    while let Some(&last) = expected.last() {
        let next: f64 = last + if last < 2.0 { 0.5 + 0.5 * last / 2.0 } else { 1.0 };
        if next >= 4.0 {
            break;
        }
        expected.push(next);
    }
    assert_eq!(radii, expected);
    // Shrinking to a tenth of the spacing never stops short of the end: past the farthest point the step
    // is the end spacing.
    let shrinking = circles(Point::ORIGIN, 1.5, 1.0, Some(0.1), meter).unwrap();
    assert!(shrinking.len() < 20, "{}", shrinking.len());
}

#[test]
fn req_fill_cir_001_the_spiral_turns_round_its_middle_0_1_css_pixel_from_it() {
    // A disc 10 mm round, circles 1 mm apart: the spiral turns in to the circle 0.1 CSS pixel round its
    // middle, 0.026 mm, and back out, so its needle point nearest the middle lies on that circle.
    let run = sew(&disc((0.0, 0.0), 10.0), &circular(1.0), None).runs.concat();
    let nearest = run.iter().map(|q| q.distance(Point::ORIGIN)).fold(f64::INFINITY, f64::min);
    assert!((nearest - 0.1 * MM_PER_SVG_PX).abs() < 1e-9, "{nearest}");
}

#[test]
fn req_fill_cir_005_a_part_within_a_row_spacing_of_its_middle_is_sewn_as_one_circle_that_size() {
    // A disc 1 mm round, circles 2 mm apart: one circle 2 mm round its middle, from its right-hand point,
    // and SC-W0318.
    let part = disc((5.0, 5.0), 1.0);
    let stitched = sew(&part, &circular(2.0), None);
    assert_eq!(codes(&stitched), [Code::FillCircularTooSmall]);
    let run = stitched.runs.concat();
    assert!(run[0].distance(p(7.0, 5.0)) < 1e-9, "{:?}", run[0]);
    assert!(run.iter().all(|q| (q.distance(p(5.0, 5.0)) - 2.0).abs() < 0.2), "{run:?}");
}

#[test]
fn req_fill_cir_005_a_part_the_spiral_misses_is_sewn_round_its_outline() {
    // A part lying between 2 turns of its spiral has no piece of it: a running stitch round its outline,
    // from the outline's start, without bean stitch or repeats, as Ink/Stitch's, and SC-W0305.
    let part = disc((0.0, 0.0), 3.0);
    let meter = &mut Budget::DEFAULT.meter();
    let rings = Rings::new(&part, meter).unwrap();
    let doubled = Circular { stitching: Stitching { beans: vec![1], repeats: 2, ..circular(1.0).stitching }, ..circular(1.0) };
    let mut tally = Tally { pieces: 1, ..Tally::default() };
    let runs = tally.sew(Spiral::Pieces(Vec::new()), (&part, &rings), &doubled, (None, None), &SplitMix64::new(1), meter, &mut Vec::new()).unwrap();
    let outline = along_line(&part.outline, 2.5, 0.2, 0.3, meter).unwrap();
    assert_eq!(runs, [outline]);
    assert_eq!(tally.said().map(|said| said.code).collect::<Vec<_>>(), [Code::FillTooThin]);
}

#[test]
fn what_the_tally_says_counts_the_parts() {
    let said = |small, missed, pieces| -> Vec<String> {
        Tally { pieces, small, missed, spans: 0 }.said().map(|said| format!("{} {}", said.code.id(), said.message)).collect()
    };
    assert_eq!(said(0, 0, 3), Vec::<String>::new());
    assert_eq!(
        said(1, 1, 2),
        [
            "SC-W0318 1 of this circular fill's parts lies within the row spacing of its middle, so it is sewn as one circle of that radius.",
            "SC-W0305 This circular fill's spiral misses 1 of its parts, so it is sewn as a running stitch round its outline."
        ]
    );
    assert_eq!(
        said(2, 3, 5),
        [
            "SC-W0318 2 of this circular fill's parts lie within the row spacing of their middle, so each is sewn as one circle of that radius.",
            "SC-W0305 This circular fill's spiral misses 3 of its parts, so each is sewn as a running stitch round its outline."
        ]
    );
    assert_eq!(said(0, 1, 1), ["SC-W0305 This circular fill's spiral misses it, so it is sewn as a running stitch round its outline."]);
}

#[test]
fn req_fill_cir_005_a_part_exactly_a_row_spacing_round_is_a_spiral() {
    // A square 2 mm a side round (1, 1): its corners lie √2 mm from its middle. Circles √2 mm apart make a
    // spiral, as Ink/Stitch's do: only a spacing past the farthest point makes one circle.
    let part = Polygon { outline: vec![p(0.0, 0.0), p(0.0, 2.0), p(2.0, 2.0), p(2.0, 0.0), p(0.0, 0.0)], holes: Vec::new() };
    let meter = &mut Budget::DEFAULT.meter();
    let rings = Rings::new(&part, meter).unwrap();
    let spiral = |spacing: f64, meter: &mut Meter| Tally::default().spiral(&part, &rings, &circular(spacing), meter).unwrap();
    assert!(matches!(spiral(std::f64::consts::SQRT_2, meter), Spiral::Pieces(_)));
    assert!(matches!(spiral(1.5, meter), Spiral::Small(_)));
}

#[test]
fn req_fill_cir_003_one_piece_is_joined_to_the_needle_along_the_outline() {
    // A disc 10 mm round, circles 5 mm apart: its spiral enters once and leaves once, one piece. From a
    // needle beside its top as drawn, the fill first runs along the outline to where the spiral enters, at
    // its right-hand side, the shorter way round.
    let part = disc((0.0, 0.0), 10.0);
    let stitched = sew(&part, &circular(5.0), Some(p(0.0, -11.0)));
    assert_eq!(codes(&stitched), Vec::<Code>::new());
    let run = stitched.runs.concat();
    assert!(run[0].distance(p(0.0, -10.0)) < 0.05, "starts on the outline nearest the needle: {:?}", run[0]);
    // The first stitches keep to the outline, until the spiral leaves it.
    assert!(run[1].distance(Point::ORIGIN) > 9.9, "{:?}", run[1]);
    assert!(run.iter().all(|q| q.distance(Point::ORIGIN) <= 10.0 + 1e-9), "inside the disc");
    // Without a needle it starts where the spiral enters.
    let alone = sew(&part, &circular(5.0), None).runs.concat();
    assert!((alone[0].distance(Point::ORIGIN) - 10.0).abs() < 0.05 && alone[0].x() > 0.0, "{:?}", alone[0]);
}

#[test]
fn req_fill_cir_003_one_piece_ends_along_the_outline_nearest_the_next_part() {
    // 2 discs 10 mm round, 30 mm apart, circles 5 mm apart: each spiral is one piece. The first disc's run
    // goes on from where its spiral leaves along the outline, the shorter way round, to its point nearest the
    // second disc.
    let region = Region { parts: vec![disc((0.0, 0.0), 10.0), disc((30.0, 0.0), 10.0)] };
    let stitched = circular_fill(&region, &circular(5.0), None, None, &mut SplitMix64::new(7), &mut Budget::DEFAULT.meter()).unwrap();
    assert_eq!(codes(&stitched), Vec::<Code>::new());
    let first = &stitched.runs[0];
    let last = first[first.len() - 1];
    assert!(last.distance(p(10.0, 0.0)) < 0.05, "{last:?}");
    assert!(first[first.len() - 2].distance(Point::ORIGIN) > 9.9, "along the outline: {:?}", first[first.len() - 2]);
}

#[test]
fn the_preview_shows_the_pieces_of_the_spiral_or_the_circle_a_small_part_is_sewn_as() {
    // A disc 10 mm round, circles 1 mm apart: segments inside the disc, along the spiral. A disc 1 mm round,
    // circles 2 mm apart: one circle 2 mm round its middle, 64 sides.
    let meter = &mut Budget::DEFAULT.meter();
    let spiral = top_path(&Region { parts: vec![disc((0.0, 0.0), 10.0)] }, &circular(1.0), meter).unwrap();
    assert!(
        spiral.len() > 64 && spiral.iter().all(|s| s.start.distance(Point::ORIGIN) <= 10.0 + 1e-9 && s.end.distance(Point::ORIGIN) <= 10.0 + 1e-9)
    );
    let small = top_path(&Region { parts: vec![disc((0.0, 0.0), 1.0)] }, &circular(2.0), meter).unwrap();
    assert_eq!(small.len(), 64);
    assert!(small.iter().all(|s| (s.start.distance(Point::ORIGIN) - 2.0).abs() < 1e-9), "{small:?}");
}

#[test]
fn req_fill_cir_004_several_pieces_are_each_sewn_once_inside_the_part() {
    // A square 20 mm a side round its middle, circles 0.5 mm apart: past its inscribed circle the spiral
    // leaves and enters it again and again, so it is many pieces, routed in one run.
    let part = Polygon { outline: vec![p(0.0, 0.0), p(0.0, 20.0), p(20.0, 20.0), p(20.0, 0.0), p(0.0, 0.0)], holes: Vec::new() };
    let stitched = sew(&part, &circular(0.5), Some(p(-1.0, -1.0)));
    assert_eq!((stitched.runs.len(), codes(&stitched)), (1, Vec::<Code>::new()));
    let run = &stitched.runs[0];
    let inside = |q: &Point| (-1e-6..=20.0 + 1e-6).contains(&q.x()) && (-1e-6..=20.0 + 1e-6).contains(&q.y());
    assert!(run.iter().all(inside), "every needle point in the square");
    // It covers the square: every point of a 1 mm grid inside it lies within 0.6 mm of a stitch.
    for i in 1..20 {
        for j in 1..20 {
            let q = p(f64::from(i), f64::from(j));
            assert!(run.windows(2).any(|s| distance_to_segment(q, s[0], s[1]) < 0.6), "{q:?} left bare");
        }
    }
    // The spiral is its pieces, and the preview shows each of them.
    let segments = top_path(&Region { parts: vec![part] }, &circular(0.5), &mut Budget::DEFAULT.meter()).unwrap();
    assert!(segments.len() > 100);
}

#[test]
fn req_fill_cir_006_repeats_double_the_stitching_and_bean_stitch_triples_the_spiral() {
    let part = disc((0.0, 0.0), 10.0);
    let once = sew(&part, &circular(5.0), None).runs.concat();
    let twice = sew(&part, &Circular { stitching: Stitching { repeats: 2, ..circular(5.0).stitching }, ..circular(5.0) }, None).runs.concat();
    // There and back: the second pass is the first reversed, its turn not sewn twice.
    assert_eq!(twice.len(), 2 * once.len() - 1);
    assert_eq!(twice[once.len()..], once.iter().rev().skip(1).copied().collect::<Vec<_>>()[..]);
    let beaned = sew(&part, &Circular { stitching: Stitching { beans: vec![1], ..circular(5.0).stitching }, ..circular(5.0) }, None).runs.concat();
    assert_eq!(beaned.len(), 3 * once.len() - 2);
}

#[test]
fn each_span_draws_its_own_random_lengths() {
    // 2 stretches alike, 30 mm along x and 1 mm apart, at random lengths: each is a span of its own, its
    // draws keyed by its number, so their needle points fall at other places along them.
    let random = Circular { stitching: Stitching { jitter: Some(0.5), ..circular(1.0).stitching }, ..circular(1.0) };
    let line = Spanned::joined(&[vec![p(0.0, 0.0), p(30.0, 0.0)], vec![p(0.0, 1.0), p(30.0, 1.0)]]);
    let run = Tally::default().stitched(&line, &random, &SplitMix64::new(7), &mut Budget::DEFAULT.meter()).unwrap();
    let along = |y: f64| run.iter().filter(|q| (q.y() - y).abs() < 1e-9).map(|q| q.x()).collect::<Vec<_>>();
    assert!(along(0.0).len() > 5, "{run:?}");
    assert_ne!(along(0.0), along(1.0));
}

#[test]
fn the_spiral_is_charged_to_the_budget() {
    let part = disc((0.0, 0.0), 10.0);
    let region = Region { parts: vec![part] };
    let mut used = Budget::DEFAULT.meter();
    circular_fill(&region, &circular(0.5), None, None, &mut SplitMix64::new(7), &mut used).unwrap();
    let work = Budget::DEFAULT.max_work - used.work_left();
    for short in [work / 10, work / 2] {
        let meter = &mut Budget { max_stitches: u32::MAX, max_work: short }.meter();
        assert!(circular_fill(&region, &circular(0.5), None, None, &mut SplitMix64::new(7), meter).is_err());
    }
}
