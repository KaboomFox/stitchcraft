//! The smoothing pass's own cases: its work budget, the shapes beside the areas, and the geometry of a line.

#![allow(clippy::cast_possible_truncation, clippy::cast_sign_loss, clippy::cast_precision_loss)]

use stitchcraft_core::ElementId;
use stitchcraft_params::ParamSet;
use stitchcraft_plan::{Rgb, Thread};

use super::*;
use crate::design::{DesignSettings, FillRule};

/// A point.
fn p(x: f64, y: f64) -> Point {
    Point::new(x, y).unwrap()
}

/// A fill named `id` through `ring`, automatic or with settings, with `params`.
fn area(id: &str, ring: &[(f64, f64)], automatic: bool, params: &[(&str, &str)]) -> Element {
    let start = p(ring[0].0, ring[0].1);
    let segments = ring.iter().skip(1).map(|&(x, y)| Segment::Line(p(x, y))).collect();
    let path = Path { subpaths: vec![Subpath { start, segments, closed: true }] };
    Element {
        id: ElementId::new(id).unwrap(),
        name: None,
        shape: Shape::Fill { path, rule: FillRule::NonZero, gradient: None },
        thread: Thread::new(Rgb::new(0, 0, 0)),
        params: params.iter().copied().collect::<ParamSet>(),
        automatic,
        lettered: None,
        hints: Default::default(),
        clips: Vec::new(),
    }
}

/// A saw-tooth from `(x0, y)` to `(x1, y)`, a tooth 0.06 mm high every 0.5 mm.
fn saw(x0: f64, x1: f64, y: f64) -> Vec<(f64, f64)> {
    let steps = ((x1 - x0).abs() / 0.25).round() as usize;
    let dx = (x1 - x0) / steps as f64;
    (0..=steps).map(|i| (x0 + dx * i as f64, if i % 2 == 1 { y + 0.06 } else { y })).collect()
}

/// An area 4 by 2 mm under a saw-tooth top, and the area above it whose lower edge is the same saw-tooth.
fn pair(lower: &[(&str, &str)], upper: &[(&str, &str)], upper_automatic: bool) -> Vec<Element> {
    let mut low = vec![(0.0, 0.0), (4.0, 0.0)];
    low.extend(saw(4.0, 0.0, 2.0));
    let mut high = saw(0.0, 4.0, 2.0);
    high.extend([(4.0, 4.0), (0.0, 4.0)]);
    vec![area("a", &low, true, lower), area("b", &high, upper_automatic, upper)]
}

/// What the pass makes of `elements` with `budget`: the design, and what it says.
fn smoothed(elements: Vec<Element>, budget: &Budget) -> (Option<Design>, Vec<Diagnostic>) {
    let design = Design::new(elements, DesignSettings::default()).unwrap();
    let rules: Vec<Option<AutoParams>> = design.elements().iter().map(|element| super::super::rules(&element.params)).collect();
    let mut said = Vec::new();
    (outlines(&design, &rules, budget, &mut said), said)
}

/// The points of the outline of the element `index` of `design`.
fn points(design: &Design, index: usize) -> usize {
    let Shape::Fill { path, .. } = &design.elements()[index].shape else { panic!("a fill") };
    path.subpaths.iter().map(|subpath| subpath.segments.len() + 1).sum()
}

#[test]
fn req_auto_131_an_area_whose_work_runs_out_keeps_its_outline_and_is_named() {
    let smooth = [("auto_smooth_mm", "0.4")];
    // With every amount of work up to what the pass needs, the areas whose work ran out keep their
    // outlines, and the notice names them.
    let mut needed = None;
    for work in 0..200_000 {
        let budget = Budget { max_work: work, ..Budget::DEFAULT };
        let (design, said) = smoothed(pair(&smooth, &smooth, true), &budget);
        let [notice] = said.as_slice() else { panic!("{said:?}") };
        assert_eq!(notice.code, Code::AutoSmoothed);
        if !notice.message.contains("needed more work") {
            needed = Some(work);
            assert_eq!((points(design.as_ref().unwrap(), 0), points(design.as_ref().unwrap(), 1)), (6, 6), "{notice}");
            break;
        }
        // An area that ran out keeps its outline, and so the edge it shares stays as drawn for both: their
        // other sides are straight, and nothing changes.
        assert_eq!(design, None, "{notice}");
    }
    let needed = needed.unwrap();
    assert!(needed > 1000, "{needed}");
    // Beside a shape with settings, the work of finding the edges they share runs out too, at every amount.
    for work in 0..needed {
        let (design, said) = smoothed(pair(&smooth, &[], false), &Budget { max_work: work, ..Budget::DEFAULT });
        assert_eq!((design, said.len()), (None, 1), "{work}");
    }
    let (_, said) = smoothed(pair(&smooth, &smooth, true), &Budget { max_work: 0, ..Budget::DEFAULT });
    assert_eq!(said[0].message, "The areas `a` and `b` needed more work to smooth than the budget allows, and keep their outlines.");
}

#[test]
fn req_auto_133_an_edge_shared_with_a_shape_with_settings_stays_as_drawn() {
    // The upper area has settings: the lower area's top edge stays as drawn, its 17 points and all, and only
    // the lower area is smoothed, which the notice counts.
    let (design, said) = smoothed(pair(&[("auto_smooth_mm", "0.4")], &[], false), &Budget::DEFAULT);
    assert!(design.is_none(), "{design:?}");
    assert_eq!(
        said[0].message,
        "The outline of 1 automatic area was smoothed within 0.4 mm: it is drawn with 19 points, where 19 drew it within 0.02 mm."
    );
    // Set off on the upper area, it is the same.
    let (design, _) = smoothed(pair(&[("auto_smooth_mm", "0.4")], &[("auto_smooth_mm", "0")], true), &Budget::DEFAULT);
    assert!(design.is_none(), "{design:?}");
}

#[test]
fn req_auto_132_an_edge_2_areas_share_is_smoothed_at_the_smaller_setting() {
    // At 0.4 and 0.05 the shared edge moves at most 0.05 mm, and keeps more points than at 0.4 for both.
    let (both, _) = smoothed(pair(&[("auto_smooth_mm", "0.4")], &[("auto_smooth_mm", "0.4")], true), &Budget::DEFAULT);
    let (mixed, said) = smoothed(pair(&[("auto_smooth_mm", "0.4")], &[("auto_smooth_mm", "0.05")], true), &Budget::DEFAULT);
    assert!(said[0].message.contains("within 0.05 to 0.4 mm"), "{}", said[0].message);
    assert!(points(&mixed.unwrap(), 1) > points(&both.unwrap(), 1));
}

#[test]
fn req_auto_131_no_area_with_a_setting_leaves_the_design_as_it_is() {
    let (design, said) = smoothed(pair(&[], &[], true), &Budget::DEFAULT);
    assert_eq!((design, said), (None, Vec::new()));
}

#[test]
fn a_step_turns_round_and_sorts_the_same_either_way() {
    let cubic = Step { from: p(0.0, 0.0), segment: Segment::Cubic(p(1.0, 0.0), p(2.0, 1.0), p(3.0, 3.0)) };
    assert_eq!(cubic.reversed(), Step { from: p(3.0, 3.0), segment: Segment::Cubic(p(2.0, 1.0), p(1.0, 0.0), p(0.0, 0.0)) });
    assert_eq!(cubic.reversed().reversed(), cubic);
    let quad = Step { from: p(0.0, 0.0), segment: Segment::Quad(p(1.0, 2.0), p(3.0, 0.0)) };
    assert_eq!(quad.reversed(), Step { from: p(3.0, 0.0), segment: Segment::Quad(p(1.0, 2.0), p(0.0, 0.0)) });
    for step in [cubic, quad, Step { from: p(5.0, 1.0), segment: Segment::Line(p(-1.0, 2.0)) }] {
        let ((key, this_way), (back, other_way)) = (step.key(), step.reversed().key());
        assert_eq!(key, back);
        assert_ne!(this_way, other_way);
    }
    assert_eq!(at(p(-0.0, 0.0)), at(p(0.0, -0.0)));
}

#[test]
fn a_curve_flattens_to_the_same_points_either_way_round() {
    let cubic = Step { from: p(0.0, 0.0), segment: Segment::Cubic(p(1.0, 0.0), p(2.0, 1.0), p(3.0, 3.0)) };
    let meter = &mut Budget::DEFAULT.meter();
    let forward = flattened(&[cubic], false, meter).unwrap();
    let mut backward = flattened(&[cubic.reversed()], false, meter).unwrap();
    backward.reverse();
    assert!(forward.len() > 4);
    assert_eq!(forward, backward);
    assert_eq!(flattened(&[cubic], false, &mut Budget { max_work: 0, ..Budget::DEFAULT }.meter()), Err(Exhausted::Work));
}

#[test]
fn a_whole_ring_sorts_from_its_least_point_either_way_round() {
    let ring = |points: &[(f64, f64)]| {
        let steps: Vec<Step> = points
            .iter()
            .zip(points.iter().cycle().skip(1))
            .map(|(&(x0, y0), &(x1, y1))| Step { from: p(x0, y0), segment: Segment::Line(p(x1, y1)) })
            .collect();
        let keys = steps.iter().map(|step| step.key().0).collect();
        Ring { element: 0, subpath: 0, steps, keys }
    };
    let square = [(1.0, 1.0), (2.0, 1.0), (2.0, 2.0), (1.0, 2.0)];
    let one = ring(&square);
    let (sorted, reversed) = canonical(&one, &[0, 1, 2, 3], true);
    let mut turned = square;
    turned.rotate_left(2);
    turned.reverse();
    let other = ring(&turned);
    let (again, other_reversed) = canonical(&other, &[0, 1, 2, 3], true);
    assert_eq!(sorted, again);
    assert_ne!(reversed, other_reversed);
    // Open, the stretch runs from its end that sorts first.
    let (open, reversed) = canonical(&one, &[2, 3], false);
    assert!(reversed);
    assert_eq!(open[0], Step { from: p(1.0, 1.0), segment: Segment::Line(p(1.0, 2.0)) });
}

#[test]
fn the_corners_of_a_line_are_its_sharpest_turns_over_the_reach() {
    let meter = &mut Budget::DEFAULT.meter();
    let square = [p(0.0, 0.0), p(4.0, 0.0), p(4.0, 4.0), p(0.0, 4.0)];
    assert_eq!(line::corners(&square, true, 0.8, meter).unwrap(), vec![0, 1, 2, 3]);
    // Open, its ends are not corners.
    assert_eq!(line::corners(&square, false, 0.8, meter).unwrap(), vec![1, 2]);
    // 2 corners closer than the reach turn as one, and the sharper stays.
    let notch = [p(0.0, 0.0), p(4.0, 0.0), p(4.0, 0.3), p(0.0, 0.5)];
    assert_eq!(line::corners(&notch, true, 0.8, meter).unwrap(), vec![1, 3]);
    // A turn of 59° is no corner, a turn of 61° is.
    let bend = |degrees: f64| {
        let (sin, cos) = stitchcraft_core::math::sin_cos(stitchcraft_core::math::to_radians(degrees));
        [p(-4.0, 0.0), p(0.0, 0.0), p(4.0 * cos, 4.0 * sin)]
    };
    assert!(line::corners(&bend(59.0), false, 0.8, meter).unwrap().is_empty());
    assert_eq!(line::corners(&bend(61.0), false, 0.8, meter).unwrap(), vec![1]);
    assert_eq!(line::corners(&square, true, 0.8, &mut Budget { max_work: 3, ..Budget::DEFAULT }.meter()), Err(Exhausted::Work));
}

#[test]
fn a_line_resamples_at_even_steps_and_blurs_with_its_ends_held() {
    let meter = &mut Budget::DEFAULT.meter();
    let open = [p(0.0, 0.0), p(1.0, 0.0), p(1.0, 1.0)];
    let samples = line::resampled(&open, false, 4, meter).unwrap().unwrap();
    assert_eq!(samples, vec![p(0.0, 0.0), p(0.5, 0.0), p(1.0, 0.0), p(1.0, 0.5), p(1.0, 1.0)]);
    let round = line::resampled(&[p(0.0, 0.0), p(1.0, 0.0), p(1.0, 1.0), p(0.0, 1.0)], true, 4, meter).unwrap().unwrap();
    assert_eq!(round, vec![p(0.0, 0.0), p(1.0, 0.0), p(1.0, 1.0), p(0.0, 1.0)]);
    // A straight line stays straight, its ends where they are.
    let straight: Vec<Point> = (0..=10).map(|i| p(f64::from(i), 2.0 * f64::from(i))).collect();
    let blurred = line::blurred(&straight, false, 3.0, meter).unwrap().unwrap();
    for (a, b) in blurred.iter().zip(&straight) {
        assert!(a.distance(*b) < 1e-9, "{a:?} {b:?}");
    }
    assert_eq!((blurred[0], blurred[10]), (straight[0], straight[10]));
    // A tiny deviation leaves the samples as they are.
    assert_eq!(line::blurred(&open, false, 0.2, meter).unwrap().unwrap(), open.to_vec());
    // Round a square, the Gaussian pulls each corner in by the same amount.
    let blurred = line::blurred(&round, true, 1.0, meter).unwrap().unwrap();
    let pulled: Vec<f64> = blurred.iter().map(|q| q.distance(p(0.5, 0.5))).collect();
    assert!(pulled.iter().all(|d| (d - pulled[0]).abs() < 1e-12 && *d < 0.7), "{pulled:?}");
    // One sample, reflected through itself at both ends, stays.
    assert_eq!(line::blurred(&[p(1.0, 1.0)], false, 2.0, meter).unwrap().unwrap(), vec![p(1.0, 1.0)]);
    assert_eq!(line::blurred(&straight, false, 3.0, &mut Budget { max_work: 10, ..Budget::DEFAULT }.meter()), Err(Exhausted::Work));
    assert_eq!(line::resampled(&open, false, 4, &mut Budget { max_work: 4, ..Budget::DEFAULT }.meter()), Err(Exhausted::Work));
}

#[test]
fn how_far_a_smoothed_line_lies_from_the_line_drawn() {
    let meter = &mut Budget::DEFAULT.meter();
    let drawn = [p(0.0, 0.0), p(1.0, 0.25), p(2.0, 0.0)];
    let flat = [p(0.0, 0.0), p(1.0, 0.0), p(2.0, 0.0)];
    // The tip of the drawn jag lies 0.25 mm from the flat line, and the flat line 0.25 * 2 / sqrt(4.25) at most
    // from the jag: the farther way is 0.25 mm.
    assert!((line::apart(&drawn, &flat, false, 1.0, meter).unwrap() - 0.25).abs() < 1e-12);
    assert_eq!(line::apart(&flat, &flat, false, 1.0, meter).unwrap(), 0.0);
    // Round a closed line, the window wraps past the first point.
    let ring = [p(0.0, 0.0), p(1.0, 0.0), p(1.0, 1.0), p(0.0, 1.0)];
    let moved = [p(0.0, 0.1), p(1.0, 0.1), p(1.0, 1.0), p(0.0, 1.0)];
    assert!((line::apart(&ring, &moved, true, 0.3, meter).unwrap() - 0.1).abs() < 1e-12);
    assert_eq!(line::apart(&ring, &moved, true, 0.3, &mut Budget { max_work: 2, ..Budget::DEFAULT }.meter()), Err(Exhausted::Work));
}

#[test]
fn the_fewest_points_draw_a_line_within_the_distance() {
    let meter = &mut Budget::DEFAULT.meter();
    let line = [p(0.0, 0.0), p(1.0, 0.01), p(2.0, 0.0), p(3.0, 0.5), p(4.0, 0.0)];
    assert_eq!(line::fewest(&line, 0.02, meter).unwrap(), vec![p(0.0, 0.0), p(2.0, 0.0), p(3.0, 0.5), p(4.0, 0.0)]);
    assert_eq!(line::fewest(&line, 0.5, meter).unwrap(), vec![p(0.0, 0.0), p(4.0, 0.0)]);
    assert_eq!(line::fewest(&line[..2], 0.02, meter).unwrap(), line[..2].to_vec());
    let ring = [p(0.0, 0.0), p(1.0, 0.0), p(2.0, 0.001), p(2.0, 2.0), p(1.0, 2.0), p(0.0, 2.0)];
    assert_eq!(line::fewest_round(&ring, 0.02, meter).unwrap(), vec![p(0.0, 0.0), p(2.0, 0.001), p(2.0, 2.0), p(0.0, 2.0)]);
    assert_eq!(line::fewest_round(&ring[..3], 0.02, meter).unwrap(), ring[..3].to_vec());
    assert_eq!(line::fewest(&line, 0.02, &mut Budget { max_work: 3, ..Budget::DEFAULT }.meter()), Err(Exhausted::Work));
    assert_eq!(line::fewest_round(&ring, 0.02, &mut Budget { max_work: 5, ..Budget::DEFAULT }.meter()), Err(Exhausted::Work));
}

#[test]
fn the_lines_that_cross() {
    let meter = &mut Budget::DEFAULT.meter();
    let segment = |a: (f64, f64), b: (f64, f64)| (p(a.0, a.1), p(b.0, b.1));
    let lines = vec![
        (vec![segment((0.0, 0.0), (3.0, 3.0))], true),
        (vec![segment((0.0, 3.0), (3.0, 0.0))], false),
        (vec![segment((5.0, 0.0), (5.0, 2.0)), segment((5.0, 2.0), (6.0, 2.0))], false),
        (vec![segment((4.5, 1.0), (5.5, 1.0))], false),
    ];
    // The first 2 cross, and the first moved. The last 2 cross, but neither moved. Touching at an end is no
    // crossing.
    assert_eq!(line::crossing(&lines, meter).unwrap(), BTreeSet::from([0, 1]));
    let touching = vec![(vec![segment((0.0, 0.0), (2.0, 0.0))], true), (vec![segment((2.0, 0.0), (2.0, 2.0))], true)];
    assert!(line::crossing(&touching, meter).unwrap().is_empty());
    assert_eq!(line::crossing(&lines, &mut Budget { max_work: 20, ..Budget::DEFAULT }.meter()), Err(Exhausted::Work));
    assert_eq!(line::crossing(&lines, &mut Budget { max_work: 5, ..Budget::DEFAULT }.meter()), Err(Exhausted::Work));
    assert!((line::area(&[p(0.0, 0.0), p(2.0, 0.0), p(2.0, 3.0)]) - 3.0).abs() < 1e-12);
    assert!((line::area(&[p(0.0, 0.0), p(2.0, 3.0), p(2.0, 0.0)]) + 3.0).abs() < 1e-12);
}

/// A fill named `id` through each of `rings`, automatic, with `params`.
fn areas(id: &str, rings: &[Vec<(f64, f64)>], params: &[(&str, &str)]) -> Element {
    let mut element = area(id, &rings[0], true, params);
    let Shape::Fill { path, .. } = &mut element.shape else { panic!("a fill") };
    for ring in &rings[1..] {
        let (start, segments) = (p(ring[0].0, ring[0].1), ring.iter().skip(1).map(|&(x, y)| Segment::Line(p(x, y))).collect());
        path.subpaths.push(Subpath { start, segments, closed: true });
    }
    element
}

/// A ring of `n` points round a circle of `radius` about `(x, y)`.
fn circle(x: f64, y: f64, radius: f64, n: usize) -> Vec<(f64, f64)> {
    (0..n)
        .map(|i| {
            let (sin, cos) = stitchcraft_core::math::sin_cos(std::f64::consts::TAU * i as f64 / n as f64);
            (x + radius * cos, y + radius * sin)
        })
        .collect()
}

/// The points of every subpath of the element `index` of `design`, the start first.
fn outline(design: &Design, index: usize) -> Vec<Vec<Point>> {
    let Shape::Fill { path, .. } = &design.elements()[index].shape else { panic!("a fill") };
    path.subpaths.iter().map(|subpath| once(subpath.start).chain(subpath.segments.iter().map(|s| s.end())).collect()).collect()
}

#[test]
fn req_auto_131_a_ring_without_corners_is_smoothed_round() {
    // A circle 5 mm round drawn with 72 points has no corner: it is smoothed as one ring, every point within
    // 0.2 mm of the circle, and drawn with fewer points.
    let (design, said) = smoothed(vec![area("a", &circle(10.0, 10.0, 5.0, 72), true, &[("auto_smooth_mm", "0.2")])], &Budget::DEFAULT);
    let ring = &outline(design.as_ref().unwrap(), 0)[0];
    assert!(ring.len() < 72, "{}", ring.len());
    for q in ring {
        let off = (q.distance(p(10.0, 10.0)) - 5.0).abs();
        assert!(off <= 0.2, "{q:?} {off}");
    }
    assert_eq!(said.len(), 1);
    // The least work that smooths it, found by halving.
    let at = |work: u64| {
        smoothed(vec![area("a", &circle(10.0, 10.0, 5.0, 72), true, &[("auto_smooth_mm", "0.2")])], &Budget { max_work: work, ..Budget::DEFAULT })
    };
    let (mut short, mut enough) = (0, 1_000_000);
    while enough - short > 1 {
        let middle = u64::midpoint(short, enough);
        if at(middle).0.is_some() { enough = middle } else { short = middle }
    }
    assert_eq!(at(enough).0, design);
    // With any less, the ring keeps its outline and the notice names it: the last 2,000 amounts run out in
    // each of the pass's last stages.
    for work in enough - 2_000..enough {
        let (smoothed, said) = at(work);
        assert_eq!(smoothed, None, "{work}");
        assert!(said[0].message.contains("needed more work"), "{work}: {}", said[0].message);
    }
}

#[test]
fn req_auto_134_a_ring_whose_area_any_smoothing_changes_stays_as_drawn() {
    // A dot 0.1 mm across, smoothed within 0.3 mm, loses more than a tenth of its area at every deviation the
    // pass tries: it stays as drawn, and the notice still counts it.
    let (design, said) = smoothed(vec![area("a", &circle(1.0, 1.0, 0.05, 16), true, &[("auto_smooth_mm", "0.3")])], &Budget::DEFAULT);
    assert_eq!(design, None);
    assert_eq!(
        said[0].message,
        "The outline of 1 automatic area was smoothed within 0.3 mm: it is drawn with 4 points, where 4 drew it within 0.02 mm."
    );
}

#[test]
fn req_auto_134_a_smoothed_edge_that_would_cross_a_hole_is_smoothed_less() {
    // An area 9.6 by 10 mm whose top waves 0.15 mm up and down every 1.6 mm, with a hole just under a crest.
    let wave: Vec<(f64, f64)> = (0..=192)
        .map(|i| {
            let x = 0.05 * f64::from(i);
            (x, 10.0 + 0.15 * stitchcraft_core::math::sin_cos(std::f64::consts::TAU * x / 1.6).0)
        })
        .collect();
    let mut outer = vec![(0.0, 0.0), (9.6, 0.0)];
    outer.extend(wave.iter().rev());
    let hole = vec![(0.35, 10.0), (0.35, 10.12), (0.45, 10.12), (0.45, 10.0)];
    let crest = |design: &Design| -> f64 {
        let near = outline(design, 0)[0].iter().filter(|q| (0.35..=0.45).contains(&q.x()) && q.y() > 9.0).map(|q| q.y()).collect::<Vec<_>>();
        near.into_iter().fold(f64::INFINITY, f64::min)
    };
    let smooth = [("auto_smooth_mm", "0.4")];
    // Without the hole, the crest comes down through where the hole would be.
    let (alone, _) = smoothed(vec![areas("a", &[outer.clone()], &smooth)], &Budget::DEFAULT);
    assert!(crest(alone.as_ref().unwrap()) < 10.1, "{}", crest(alone.as_ref().unwrap()));
    // With it, the wave is smoothed less, and the crest stays above the hole.
    let (holed, _) = smoothed(vec![areas("a", &[outer, hole.clone()], &smooth)], &Budget::DEFAULT);
    let holed = holed.unwrap();
    assert!(crest(&holed) > 10.12, "{}", crest(&holed));
    assert_eq!(outline(&holed, 0)[1], hole.iter().map(|&(x, y)| p(x, y)).collect::<Vec<_>>());
    assert_eq!(fingerprint(Some(&holed)), "41 191.626350735 380.053194341 | 4 1.600000000 40.240000000");
}

#[test]
fn a_piece_that_moves_too_far_is_smoothed_less() {
    // Teeth 1 mm tall every 0.5 mm blur farther than 0.1 mm at a deviation of 1 mm: the deviation shrinks
    // until the piece lies within 0.1 mm of the teeth.
    let teeth: Vec<Point> = (0..=8).map(|i| p(0.25 * f64::from(i), if i % 2 == 1 { 1.0 } else { 0.0 })).collect();
    let meter = &mut Budget::DEFAULT.meter();
    let steps = samples(line::length(&teeth, false), 0.1).unwrap();
    let resampled = line::resampled(&teeth, false, steps, meter).unwrap().unwrap();
    let step = line::length(&teeth, false) / steps as f64;
    let first = line::blurred(&resampled, false, 1.0 / step, meter).unwrap().unwrap();
    assert!(line::apart(&teeth, &first, false, 0.4, meter).unwrap() > 0.1);
    let kept = piece(&teeth, false, 0.1, 1.0, meter).unwrap();
    assert_ne!(kept, teeth);
    assert!(line::apart(&teeth, &kept, false, 0.4, meter).unwrap() <= 0.1);
}

#[test]
fn the_sharpest_of_a_run_of_turning_points_is_the_corner() {
    // Points every 0.1 mm along an L: 3 of them turn by 60° or more over 0.4 mm, and the sharpest, the bend
    // itself at (5, 0), is the corner.
    let mut l: Vec<Point> = (0..=50).map(|i| p(0.1 * f64::from(i), 0.0)).collect();
    l.extend((1..=50).map(|i| p(5.0, 0.1 * f64::from(i))));
    assert_eq!(line::corners(&l, false, 0.4, &mut Budget::DEFAULT.meter()).unwrap(), vec![50]);
}

#[test]
fn how_far_apart_round_a_closed_line_with_a_window_past_its_end_or_round_it() {
    let meter = &mut Budget::DEFAULT.meter();
    let ring: Vec<Point> = circle(0.0, 0.0, 1.0, 60).into_iter().map(|(x, y)| p(x, y)).collect();
    let shrunk: Vec<Point> = circle(0.0, 0.0, 0.9, 60).into_iter().map(|(x, y)| p(x, y)).collect();
    // A window past the line's end wraps to its start; one longer than the line takes all of it.
    for window in [0.5, 10.0] {
        let apart = line::apart(&ring, &shrunk, true, window, meter).unwrap();
        assert!((apart - 0.1).abs() < 0.01, "{window}: {apart}");
    }
}

/// Each subpath of each fill of `design` as its point count and the sums of its points' coordinates, to 9
/// places: exact enough to tell any change in the pass's arithmetic.
fn fingerprint(design: Option<&Design>) -> String {
    let Some(design) = design else { return "as drawn".to_string() };
    let mut out = Vec::new();
    for index in 0..design.elements().len() {
        for ring in outline(design, index) {
            let (x, y) = ring.iter().fold((0.0, 0.0), |(x, y), q| (x + q.x(), y + q.y()));
            out.push(format!("{} {x:.9} {y:.9}", ring.len()));
        }
    }
    out.join(" | ")
}

/// The least work with which `elements` come out smoothed (the pass's result is `Some`), found by halving.
fn least_work(elements: &[Element]) -> u64 {
    let (mut short, mut enough) = (0, 4_000_000);
    while enough - short > 1 {
        let middle = u64::midpoint(short, enough);
        let budget = Budget { max_work: middle, ..Budget::DEFAULT };
        if smoothed(elements.to_vec(), &budget).0.is_some() { enough = middle } else { short = middle }
    }
    enough
}

/// The work `run` charges a meter.
fn charged(run: impl FnOnce(&mut Meter)) -> u64 {
    let mut meter = Budget { max_work: 1_000_000_000, ..Budget::DEFAULT }.meter();
    run(&mut meter);
    1_000_000_000 - meter.work_left()
}

#[test]
fn the_pass_does_exactly_its_sums_and_its_work() {
    let smooth = [("auto_smooth_mm", "0.2")];
    let round = vec![area("a", &circle(10.0, 10.0, 5.0, 72), true, &smooth)];
    let (design, said) = smoothed(round.clone(), &Budget::DEFAULT);
    assert_eq!(fingerprint(design.as_ref()), "65 645.007068377 650.000000000");
    assert_eq!(
        said[0].message,
        "The outline of 1 automatic area was smoothed within 0.2 mm: it is drawn with 64 points, where 40 drew it within 0.02 mm."
    );
    assert_eq!(least_work(&round), 40436);
    let both = pair(&[("auto_smooth_mm", "0.4")], &[("auto_smooth_mm", "0.4")], true);
    assert_eq!(fingerprint(smoothed(both.clone(), &Budget::DEFAULT).0.as_ref()), "6 13.461538462 8.029848824 | 6 9.461538462 16.029848824");
    assert_eq!(least_work(&both), 6977);
    // A ring 0.6 mm across, smoothed within 0.3 mm, keeps its area within a tenth only once its deviation
    // has shrunk.
    let small = vec![area("a", &circle(1.0, 1.0, 0.3, 24), true, &[("auto_smooth_mm", "0.3")])];
    assert_eq!(fingerprint(smoothed(small, &Budget::DEFAULT).0.as_ref()), "15 15.225364249 15.000000000");
    // 2 rings that touch at one point are each cut there, and smoothed with that point held.
    let left = circle(0.0, 0.0, 2.0, 72);
    let right: Vec<(f64, f64)> = left.iter().map(|&(x, y)| (4.0 - x, y)).collect();
    let touching = vec![area("a", &left, true, &smooth), area("b", &right, true, &smooth)];
    assert_eq!(fingerprint(smoothed(touching, &Budget::DEFAULT).0.as_ref()), "33 1.800900313 0.058719751 | 33 130.199099687 -0.058719751");
}

#[test]
fn a_repeated_point_changes_nothing() {
    let smooth = [("auto_smooth_mm", "0.2")];
    let ring = circle(10.0, 10.0, 5.0, 72);
    let mut repeated = ring.clone();
    repeated.insert(10, ring[10]);
    let (once, _) = smoothed(vec![area("a", &ring, true, &smooth)], &Budget::DEFAULT);
    let (twice, _) = smoothed(vec![area("a", &repeated, true, &smooth)], &Budget::DEFAULT);
    assert_eq!(fingerprint(twice.as_ref()), fingerprint(once.as_ref()));
}

#[test]
fn req_auto_131_an_area_that_runs_out_leaves_the_others_smoothed() {
    // A small ring and a large one, apart: with the least work that smooths the small one, the large one runs
    // out, keeps its outline and is named.
    let smooth = [("auto_smooth_mm", "0.2")];
    let rings = vec![area("a", &circle(2.0, 2.0, 1.0, 24), true, &smooth), area("b", &circle(20.0, 20.0, 8.0, 96), true, &smooth)];
    let work = least_work(&rings);
    let (design, said) = smoothed(rings.clone(), &Budget { max_work: work, ..Budget::DEFAULT });
    assert_eq!(outline(design.as_ref().unwrap(), 1), outline(&Design::new(rings, DesignSettings::default()).unwrap(), 1));
    assert_ne!(outline(design.as_ref().unwrap(), 0)[0].len(), 24);
    assert!(
        said[0].message.ends_with("The area `b` needed more work to smooth than the budget allows, and keeps its outline."),
        "{}",
        said[0].message
    );
}

#[test]
fn a_piece_too_short_to_sample_stays_and_an_open_one_is_smoothed() {
    let meter = &mut Budget::DEFAULT.meter();
    // 2 steps of a closed piece are too few to go round: it stays as drawn.
    let tiny = [p(0.0, 0.0), p(0.01, 0.0), p(0.0, 0.01)];
    assert_eq!(piece(&tiny, true, 0.1, 0.1, meter).unwrap(), tiny.to_vec());
    // An open piece of 2 steps is smoothed.
    let bent = [p(0.0, 0.0), p(0.01, 0.005), p(0.02, 0.0)];
    assert_ne!(piece(&bent, false, 0.1, 0.1, meter).unwrap(), bent.to_vec());
}

#[test]
fn the_segments_near_a_stretch_of_a_line() {
    let arcs = [0.0, 1.0, 2.0, 3.0, 4.0];
    for (closed, from, to, near) in [
        (true, -0.5, 0.5, vec![0, 3]),
        (true, 3.5, 4.5, vec![3, 0]),
        (true, 1.0, 5.5, vec![0, 1, 2, 3]),
        (true, 1.5, 2.5, vec![1, 2]),
        (true, 0.5, 2.5, vec![0, 1, 2]),
        (true, 0.0, 0.5, vec![0]),
        (true, 3.5, 4.0, vec![3]),
        (false, 2.5, 7.0, vec![2, 3]),
        (false, 1.0, 1.5, vec![0, 1]),
        (false, -0.5, 0.5, vec![0]),
        (false, 3.5, 4.5, vec![3]),
    ] {
        assert_eq!(line::near(&arcs, closed, from, to), near, "{closed} {from} {to}");
    }
}

#[test]
fn segments_cross_only_through_each_other() {
    let s = |a: (f64, f64), b: (f64, f64)| (p(a.0, a.1), p(b.0, b.1));
    // An end on the other segment is no crossing, whichever segment it ends.
    assert!(!line::cross(s((0.0, 0.0), (2.0, 0.0)), s((1.0, -1.0), (1.0, 0.0))));
    assert!(!line::cross(s((1.0, -1.0), (1.0, 0.0)), s((0.0, 0.0), (2.0, 0.0))));
    assert!(line::cross(s((10.0, 10.0), (12.0, 10.0)), s((11.0, 9.0), (11.0, 11.0))));
    // 2 short segments that cross inside one cell of the grid.
    let small = vec![(vec![s((0.1, 0.1), (0.3, 0.3))], true), (vec![s((0.1, 0.3), (0.3, 0.1))], true)];
    assert_eq!(line::crossing(&small, &mut Budget::DEFAULT.meter()).unwrap(), BTreeSet::from([0, 1]));
    let lines = vec![
        (vec![s((0.0, 0.0), (3.0, 3.0))], true),
        (vec![s((0.0, 3.0), (3.0, 0.0))], false),
        (vec![s((5.0, 0.0), (5.0, 2.0)), s((5.0, 2.0), (6.0, 2.0))], false),
        (vec![s((4.5, 1.0), (5.5, 1.0))], false),
    ];
    assert_eq!(charged(|meter| assert!(line::crossing(&lines, meter).is_ok())), 75);
}

#[test]
fn the_line_helpers_at_their_bounds() {
    let meter = &mut Budget::DEFAULT.meter();
    // A closed line of 2 points has no segment back: it is as long as its one segment.
    assert_eq!(line::length(&[p(0.0, 0.0), p(3.0, 4.0)], true), 5.0);
    // A last segment of no length resamples to its end.
    assert_eq!(
        line::resampled(&[p(0.0, 0.0), p(2.0, 0.0), p(2.0, 0.0)], false, 2, meter).unwrap().unwrap(),
        vec![p(0.0, 0.0), p(1.0, 0.0), p(2.0, 0.0)]
    );
    // A closed triangle smaller than the reach turns as one run: one corner.
    assert_eq!(line::corners(&[p(0.0, 0.0), p(0.2, 0.0), p(0.0, 0.1)], true, 0.8, meter).unwrap().len(), 1);
    // A deviation of exactly a quarter sample blurs.
    let curved: Vec<Point> = (0..=10).map(|i| p(f64::from(i), f64::from(i * i) / 10.0)).collect();
    assert_ne!(line::blurred(&curved, false, 0.25, meter).unwrap().unwrap(), curved);
    // 3 deviations reach 9 samples each side, and 3 samples blurred over 10 reach 13.
    assert_eq!(fingerprint_points(&line::blurred(&curved, false, 3.0, meter).unwrap().unwrap()), "11 55.000000000 44.510002027");
    assert_eq!(fingerprint_points(&line::blurred(&curved[..3], false, 10.0, meter).unwrap().unwrap()), "3 3.000000000 0.597925545");
    assert_eq!(charged(|meter| assert!(line::blurred(&curved, false, 3.0, meter).is_ok())), 11 * 19);
    // 3 points along a line thin to its ends, and so does a ring of 4 with one along a side.
    assert_eq!(line::fewest(&[p(0.0, 0.0), p(1.0, 0.0), p(2.0, 0.0)], 0.02, meter).unwrap(), vec![p(0.0, 0.0), p(2.0, 0.0)]);
    let ring = [p(0.0, 0.0), p(1.0, 0.0), p(2.0, 0.0), p(1.0, 1.0)];
    assert_eq!(line::fewest_round(&ring, 0.02, meter).unwrap(), vec![p(0.0, 0.0), p(2.0, 0.0), p(1.0, 1.0)]);
    assert_eq!(charged(|meter| assert!(line::fewest(&curved, 0.02, meter).is_ok())), 44);
}

/// The point count and coordinate sums of `points`, to 9 places.
fn fingerprint_points(points: &[Point]) -> String {
    let (x, y) = points.iter().fold((0.0, 0.0), |(x, y), q| (x + q.x(), y + q.y()));
    format!("{} {x:.9} {y:.9}", points.len())
}
