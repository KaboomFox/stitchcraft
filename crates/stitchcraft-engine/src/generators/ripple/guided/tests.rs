//! The guided ripples' unit cases (`REQ-RIP-012` to `REQ-RIP-014`): where a guide places the copies of a
//! ripple's outline, worked out by hand on guides along the axes.

use stitchcraft_core::Budget;
use stitchcraft_params::ParamSet;

use super::*;
use crate::design::{Join, Segment};

fn p(x: f64, y: f64) -> Point {
    Point::new(x, y).unwrap()
}

/// The ripple's settings as `with` says.
fn own(with: &[(&str, &str)]) -> RippleParams {
    RippleParams::from_set(&with.iter().copied().collect()).unwrap().params
}

/// `points` to a thousandth of a millimetre.
fn rounded(points: &[Point]) -> Vec<(f64, f64)> {
    let round = |v: f64| (v * 1000.0).round() / 1000.0 + 0.0;
    points.iter().map(|q| (round(q.x()), round(q.y()))).collect()
}

/// Polylines through each of `parts`, closed where `closed` says.
fn path(parts: &[&[(f64, f64)]], closed: bool) -> Path {
    let subpath = |points: &&[(f64, f64)]| Subpath {
        start: p(points[0].0, points[0].1),
        segments: points[1..].iter().map(|&(x, y)| Segment::Line(p(x, y))).collect(),
        closed,
    };
    Path { subpaths: parts.iter().map(subpath).collect() }
}

/// A guide line through `points`, its nodes its points.
fn line_guide(points: &[(f64, f64)]) -> RippleGuide {
    RippleGuide::Line { path: path(&[points], false), nodes: true }
}

/// A satin guide drawn with `parts`, its rails and rungs, read with the satin column's defaults and `with`.
fn satin_guide(parts: &[&[(f64, f64)]], with: &[(&str, &str)]) -> RippleGuide {
    let set: ParamSet = with.iter().copied().collect();
    let params = SatinParams::from_set_as(&set, StitchType::SatinColumn).unwrap().params;
    RippleGuide::Satin { path: path(parts, false), width: Mm::ZERO, join: Join::UNSET, params: Box::new(params) }
}

/// A column 20 mm long and 4 mm wide along x, from x = 0, its rung across its middle.
const COLUMN: [&[(f64, f64)]; 3] = [&[(0.0, 0.0), (20.0, 0.0)], &[(0.0, 4.0), (20.0, 4.0)], &[(10.0, -0.5), (10.0, 4.5)]];

/// The copies `guide` places of `outline` as `with` says, for a closed line where `closed`, and what it says.
fn placed_by(
    outline: &[(f64, f64)],
    guide: &RippleGuide,
    with: &[(&str, &str)],
    closed: bool,
    anchor: Option<[Point; 2]>,
) -> (Option<Copies>, Vec<String>) {
    let own = own(with);
    let adds = closed || own.join_style == "1";
    let placing = Placing { own: &own, adds, closed, anchor };
    let outline: Vec<Point> = outline.iter().map(|&(x, y)| p(x, y)).collect();
    let mut warnings = Vec::new();
    let copies = copies(&outline, guide, &placing, &mut warnings, &mut Budget::DEFAULT.meter()).unwrap();
    (copies, warnings.iter().map(ToString::to_string).collect())
}

/// The copies `guide` places of `outline` as `with` says, each to a thousandth of a millimetre, when it says
/// nothing.
fn copied(outline: &[(f64, f64)], guide: &RippleGuide, with: &[(&str, &str)], closed: bool, anchor: Option<[Point; 2]>) -> Vec<Vec<(f64, f64)>> {
    let (copies, said) = placed_by(outline, guide, with, closed, anchor);
    assert_eq!(said, Vec::<String>::new());
    copies.unwrap().lines.iter().map(|line| rounded(line)).collect()
}

/// A tent 2 mm wide and 1 mm tall, open, from (0, 0) to (2, 0).
const TENT: [(f64, f64); 3] = [(0.0, 0.0), (1.0, 1.0), (2.0, 0.0)];

#[test]
fn req_rip_012_copies_move_along_a_guide_line_and_scale_from_scale_start_to_scale_end() {
    // A straight guide 10 mm long and 3 copies: at its start, its middle and its end, each scaled about its
    // place, from 100 % to 50 %.
    let guide = line_guide(&[(0.0, 0.0), (10.0, 0.0)]);
    let copies = copied(&[(0.0, 0.0), (2.0, 1.0)], &guide, &[("line_count", "3"), ("scale_end", "50")], false, None);
    assert_eq!(copies, [vec![(0.0, 0.0), (2.0, 1.0)], vec![(5.0, 0.0), (6.5, 0.75)], vec![(10.0, 0.0), (11.0, 0.5)]]);
    // From 50 % to 150 %, the copies grow.
    let copies = copied(&[(0.0, 0.0), (2.0, 1.0)], &guide, &[("line_count", "3"), ("scale_start", "50"), ("scale_end", "150")], false, None);
    assert_eq!(copies, [vec![(0.0, 0.0), (1.0, 0.5)], vec![(5.0, 0.0), (7.0, 1.0)], vec![(10.0, 0.0), (13.0, 1.5)]]);
    // 1 copy lies at the guide's start, at scale_start.
    let one = copied(&[(0.0, 0.0), (2.0, 1.0)], &guide, &[("line_count", "1"), ("scale_start", "50")], false, None);
    assert_eq!(one, [vec![(0.0, 0.0), (1.0, 0.5)]]);
}

#[test]
fn req_rip_012_copies_turn_as_the_guide_turns_from_its_start() {
    // An L: right 10 mm, then down 10 mm. The third copy's guide has turned by 90° from the start's, so it
    // turns by 90° about its place, (10, 10).
    let guide = line_guide(&[(0.0, 0.0), (10.0, 0.0), (10.0, 10.0)]);
    let outline = [(0.0, 0.0), (2.0, 1.0)];
    let turned = copied(&outline, &guide, &[("line_count", "3"), ("scale_end", "100")], false, None);
    assert_eq!(turned, [vec![(0.0, 0.0), (2.0, 1.0)], vec![(10.0, 0.0), (12.0, 1.0)], vec![(10.0, 10.0), (9.0, 12.0)]]);
    // Without rotate_ripples it only moves.
    let moved = copied(&outline, &guide, &[("line_count", "3"), ("scale_end", "100"), ("rotate_ripples", "false")], false, None);
    assert_eq!(moved[2], [(10.0, 10.0), (12.0, 11.0)]);
    // Scaled to 50 % at the end, along both axes, across alone, down alone or neither, after the turn.
    let axis =
        |scale_axis: &str| copied(&outline, &guide, &[("line_count", "3"), ("scale_end", "50"), ("scale_axis", scale_axis)], false, None)[2].clone();
    assert_eq!(
        ["0", "1", "2", "3"].map(axis),
        [[(10.0, 10.0), (9.5, 11.0)], [(10.0, 10.0), (9.5, 12.0)], [(10.0, 10.0), (9.0, 11.0)], [(10.0, 10.0), (9.0, 12.0)]]
    );
}

#[test]
fn req_rip_012_the_guide_s_start_direction_is_taken_over_its_first_tenth_of_a_css_pixel() {
    // A first segment of 0.02 mm, shorter than 0.1 CSS pixels (0.0265 mm), so the direction runs to the second
    // segment, 0.0065 mm down it. The second copy, at the guide's end, turns from there.
    let guide = line_guide(&[(0.0, 0.0), (0.02, 0.0), (0.02, 10.0)]);
    let copies = copied(&[(0.0, 0.0), (2.0, 0.0)], &guide, &[("line_count", "2"), ("scale_end", "100")], false, None);
    let reach = START_DIRECTION - 0.02;
    let start = direction(p(0.0, 0.0), p(0.02, reach));
    let [cos, sin] = between(start, direction(p(0.0, 0.0), p(0.02, 10.0)));
    assert_eq!(copies[1], rounded(&[p(0.02, 10.0), p(0.02 + 2.0 * cos, 10.0 + 2.0 * sin)]));
    assert_eq!(copies[1], [(0.02, 10.0), (0.638, 11.902)]);
}

#[test]
fn req_rip_012_copies_lie_at_their_places_along_the_guide_or_at_its_points() {
    let guide = line_guide(&[(0.0, 0.0), (10.0, 0.0)]);
    let starts = |with: &[(&str, &str)]| -> Vec<(f64, f64)> {
        let mut with = with.to_vec();
        with.push(("scale_end", "100"));
        copied(&[(0.0, 0.0), (1.0, 0.0)], &guide, &with, false, None).iter().map(|copy| copy[0]).collect()
    };
    // An exponent of 2 packs them towards the start, and flipped towards the end.
    assert_eq!(starts(&[("line_count", "3"), ("exponent", "2")]), [(0.0, 0.0), (2.5, 0.0), (10.0, 0.0)]);
    assert_eq!(starts(&[("line_count", "3"), ("exponent", "2"), ("flip_exponent", "true")]), [(0.0, 0.0), (7.5, 0.0), (10.0, 0.0)]);
    // min_line_dist_mm: the guide's length over it, rounded up, with line_count's parity: 10 / 3 gives 4.
    assert_eq!(starts(&[("min_line_dist_mm", "3")]).len(), 4);
    assert_eq!(starts(&[("min_line_dist_mm", "3"), ("line_count", "9")]).len(), 3);
    // At the rungs: a copy at each point of the guide, whatever line_count says.
    let points = line_guide(&[(0.0, 0.0), (4.0, 0.0), (4.0, 3.0)]);
    let at_points =
        copied(&[(0.0, 0.0), (1.0, 0.0)], &points, &[("satin_guide_pattern_position", "render_at_rungs"), ("scale_end", "100")], false, None);
    assert_eq!(at_points.iter().map(|copy| copy[0]).collect::<Vec<_>>(), [(0.0, 0.0), (4.0, 0.0), (4.0, 3.0)]);
    // The last turns down with the guide's last segment.
    assert_eq!(at_points[2], [(4.0, 3.0), (4.0, 4.0)]);
}

#[test]
fn req_rip_012_a_closed_line_or_the_point_join_adds_a_copy() {
    let guide = line_guide(&[(0.0, 0.0), (10.0, 0.0)]);
    let count = |closed: bool, with: &[(&str, &str)]| placed_by(&[(0.0, 0.0), (1.0, 0.0)], &guide, with, closed, None).0.unwrap().lines.len();
    assert_eq!(
        [count(false, &[("line_count", "3")]), count(true, &[("line_count", "3")]), count(false, &[("line_count", "3"), ("join_style", "1")])],
        [3, 4, 4]
    );
}

#[test]
fn req_rip_012_a_guide_line_flattened_or_through_its_nodes() {
    // A curve's flattened points, or its nodes alone where the guide is a manual stitch.
    let curve = Path {
        subpaths: vec![Subpath { start: p(0.0, 0.0), segments: vec![Segment::Cubic(p(0.0, 5.0), p(10.0, 5.0), p(10.0, 0.0))], closed: false }],
    };
    let flat = guide_line(&curve, false, &mut Budget::DEFAULT.meter()).unwrap().unwrap();
    assert!(flat.len() > 10 && flat.first() == Some(&p(0.0, 0.0)) && flat.last() == Some(&p(10.0, 0.0)), "{flat:?}");
    assert_eq!(guide_line(&curve, true, &mut Budget::DEFAULT.meter()).unwrap(), Some(vec![p(0.0, 0.0), p(10.0, 0.0)]));
    // The first subpath of more than one node, and none of no length.
    let parts = path(&[&[(5.0, 5.0)], &[(0.0, 0.0), (3.0, 4.0)]], false);
    assert_eq!(guide_line(&parts, true, &mut Budget::DEFAULT.meter()).unwrap(), Some(vec![p(0.0, 0.0), p(3.0, 4.0)]));
    assert_eq!(guide_line(&path(&[&[(1.0, 1.0), (1.0, 1.0)]], false), true, &mut Budget::DEFAULT.meter()).unwrap(), None);
    assert_eq!(guide_line(&Path { subpaths: Vec::new() }, false, &mut Budget::DEFAULT.meter()).unwrap(), None);
    // A closed subpath's nodes end where it starts.
    let square = path(&[&[(0.0, 0.0), (2.0, 0.0), (2.0, 2.0)]], true);
    assert_eq!(node_points(&square.subpaths[0]), [p(0.0, 0.0), p(2.0, 0.0), p(2.0, 2.0), p(0.0, 0.0)]);
    let shut = path(&[&[(0.0, 0.0), (2.0, 0.0), (0.0, 0.0)]], true);
    assert_eq!(node_points(&shut.subpaths[0]), [p(0.0, 0.0), p(2.0, 0.0), p(0.0, 0.0)]);
}

#[test]
fn diag_sc_w0409_a_guide_that_can_t_place_copies_is_left_out_and_said() {
    // A guide line of no length.
    let (copies, said) = placed_by(&TENT, &line_guide(&[(1.0, 1.0), (1.0, 1.0)]), &[], false, None);
    assert!(copies.is_none());
    assert_eq!(said, ["warning SC-W0409: This ripple stitch's guide can't be used: its guide line has no length. It is sewn without it."]);
    // A satin guide whose 2 subpaths are points, which leave it no rail.
    let (copies, said) = placed_by(&TENT, &satin_guide(&[&[(5.0, 5.0)], &[(6.0, 6.0)]], &[]), &[], false, None);
    assert!(copies.is_none());
    assert_eq!(
        said.last().map(String::as_str),
        Some(
            "warning SC-W0409: This ripple stitch's guide can't be used: its satin guide has no rails to place copies between. It is sewn without it."
        )
    );
}

#[test]
fn req_rip_013_a_satin_guide_places_each_copy_s_ends_on_a_pair_across_it() {
    // 3 copies at x = 0, 10 and 20: the tent's ends land on each pair, turned 90° and scaled by 2.
    let column = satin_guide(&COLUMN, &[]);
    let copies = copied(&TENT, &column, &[("line_count", "3")], false, None);
    let at = |x: f64| vec![(x, 0.0), (x - 2.0, 2.0), (x, 4.0)];
    assert_eq!(copies, [at(0.0), at(10.0), at(20.0)]);
    // The rails swapped: from the second rail to the first.
    let swapped = satin_guide(&COLUMN, &[("swap_satin_rails", "true")]);
    let back = |x: f64| vec![(x, 4.0), (x + 2.0, 2.0), (x, 0.0)];
    assert_eq!(copied(&TENT, &swapped, &[("line_count", "3")], false, None), [back(0.0), back(10.0), back(20.0)]);
    // min_line_dist_mm counts along the guide's middle, 20 mm long: 7 copies 3.333 mm apart.
    let spread = copied(&TENT, &column, &[("min_line_dist_mm", "3"), ("line_count", "1")], false, None);
    assert_eq!(spread.iter().map(|copy| copy[0].0).collect::<Vec<_>>(), [0.0, 3.333, 6.667, 10.0, 13.333, 16.667, 20.0]);
    // At the rungs: at the end of each section, the rung's and the guide's end.
    let rungs = copied(&TENT, &column, &[("satin_guide_pattern_position", "render_at_rungs")], false, None);
    assert_eq!(rungs, [at(10.0), at(20.0)]);
}

#[test]
fn req_rip_013_a_closed_line_without_an_anchor_line_is_centred_between_the_rails() {
    // A square 2 mm a side, its centroid (1, 1): each copy is centred on its pair's middle, (x, 2). 4 copies,
    // 1 more for the closed line, 6.667 mm apart.
    let square = [(0.0, 0.0), (2.0, 0.0), (2.0, 2.0), (0.0, 2.0), (0.0, 0.0)];
    let column = satin_guide(&COLUMN, &[]);
    let copies = copied(&square, &column, &[("line_count", "3")], true, None);
    let at = |x: f64| [(x - 1.0, 1.0), (x + 1.0, 1.0), (x + 1.0, 3.0), (x - 1.0, 3.0), (x - 1.0, 1.0)].to_vec();
    assert_eq!(copies, [at(0.0), at(6.667), at(13.333), at(20.0)]);
    // Between rails that spread from 4 mm to 8 mm apart, at the rungs at x = 5, 10 and 15, 5, 6 and 7 mm wide:
    // each copy is centred on its pair's middle and scaled by its pair's width over the first's. A closed
    // line's copies lie at every section's end but the guide's.
    let rungs: [&[(f64, f64)]; 3] = [&[(5.0, -0.5), (5.0, 5.5)], &[(10.0, -0.5), (10.0, 6.5)], &[(15.0, -0.5), (15.0, 7.5)]];
    let wider = satin_guide(&[&[(0.0, 0.0), (20.0, 0.0)], &[(0.0, 4.0), (20.0, 8.0)], rungs[0], rungs[1], rungs[2]], &[]);
    let at_rungs = [("satin_guide_pattern_position", "render_at_rungs")];
    let first = vec![(4.0, 1.5), (6.0, 1.5), (6.0, 3.5), (4.0, 3.5), (4.0, 1.5)];
    let second = vec![(8.8, 1.8), (11.2, 1.8), (11.2, 4.2), (8.8, 4.2), (8.8, 1.8)];
    let third = vec![(13.6, 2.1), (16.4, 2.1), (16.4, 4.9), (13.6, 4.9), (13.6, 2.1)];
    assert_eq!(copied(&square, &wider, &at_rungs, true, None), [first.clone(), second, third]);
    // Not scaled where scale_axis says neither, and across alone where it says so.
    let second = vec![(9.0, 2.0), (11.0, 2.0), (11.0, 4.0), (9.0, 4.0), (9.0, 2.0)];
    let third = vec![(14.0, 2.5), (16.0, 2.5), (16.0, 4.5), (14.0, 4.5), (14.0, 2.5)];
    assert_eq!(copied(&square, &wider, &[at_rungs[0], ("scale_axis", "3")], true, None), [first.clone(), second, third]);
    let second = vec![(8.8, 2.0), (11.2, 2.0), (11.2, 4.0), (8.8, 4.0), (8.8, 2.0)];
    let third = vec![(13.6, 2.5), (16.4, 2.5), (16.4, 4.5), (13.6, 4.5), (13.6, 2.5)];
    assert_eq!(copied(&square, &wider, &[at_rungs[0], ("scale_axis", "1")], true, None), [first, second, third]);
}

#[test]
fn req_rip_013_a_closed_line_turns_with_the_pairs_unless_rotate_ripples_is_off() {
    // Rails that turn a right angle, with 2 rungs before the turn and one after it. At the rungs, a closed
    // line's copies lie on the pairs (5, 0)–(5, 4), (8, 0)–(8, 4) and (10, −5)–(14, −5), the last turned
    // −90° from the first, centred on (5, 2), (8, 2) and (12, −5).
    let rungs: [&[(f64, f64)]; 3] = [&[(5.0, -0.5), (5.0, 4.5)], &[(8.0, -0.5), (8.0, 4.5)], &[(9.5, -5.0), (14.5, -5.0)]];
    let bent =
        satin_guide(&[&[(0.0, 0.0), (10.0, 0.0), (10.0, -10.0)], &[(0.0, 4.0), (14.0, 4.0), (14.0, -10.0)], rungs[0], rungs[1], rungs[2]], &[]);
    let square = [(0.0, 0.0), (2.0, 0.0), (2.0, 2.0), (0.0, 2.0), (0.0, 0.0)];
    let at_rungs = ("satin_guide_pattern_position", "render_at_rungs");
    let first = vec![(4.0, 1.0), (6.0, 1.0), (6.0, 3.0), (4.0, 3.0), (4.0, 1.0)];
    let second = vec![(7.0, 1.0), (9.0, 1.0), (9.0, 3.0), (7.0, 3.0), (7.0, 1.0)];
    let turned = vec![(11.0, -4.0), (11.0, -6.0), (13.0, -6.0), (13.0, -4.0), (11.0, -4.0)];
    assert_eq!(copied(&square, &bent, &[at_rungs], true, None), [first.clone(), second.clone(), turned]);
    let still = vec![(11.0, -6.0), (13.0, -6.0), (13.0, -4.0), (11.0, -4.0), (11.0, -6.0)];
    assert_eq!(copied(&square, &bent, &[at_rungs, ("rotate_ripples", "false")], true, None), [first, second, still]);
}

#[test]
fn req_rip_013_a_copy_on_the_first_pair_again_at_the_end_is_left_out() {
    // A ring between 2 squares, its rails closed: the last pair is the first again.
    let ring = satin_guide(
        &[
            &[(0.0, 0.0), (10.0, 0.0), (10.0, 10.0), (0.0, 10.0), (0.0, 0.0)],
            &[(2.0, 2.0), (8.0, 2.0), (8.0, 8.0), (2.0, 8.0), (2.0, 2.0)],
            &[(5.0, -0.5), (5.0, 2.5)],
        ],
        &[],
    );
    let (copies, said) = placed_by(&TENT, &ring, &[("line_count", "5")], false, None);
    assert_eq!(said, Vec::<String>::new());
    // 5 copies asked for 7.92 mm apart along its middle, 31.67 mm long. Measured on the outside of its turns,
    // 3 fit before the guide's end, which is its start again.
    let starts: Vec<(f64, f64)> = copies.unwrap().lines.iter().map(|copy| rounded(copy)[0]).collect();
    // The third at x = 6.283 mm, written as thousandths.
    assert_eq!(starts, [(0.0, 0.0), (10.0, 0.997), (6283.0 / 1000.0, 10.0)]);
    // A guide of one section places one pair at its rungs, which is its first and last, and none is left.
    let one = vec![[vec![p(0.0, 0.0), p(5.0, 0.0)], vec![p(0.0, 4.0), p(5.0, 4.0)]]];
    let params = SatinParams::from_set(&ParamSet::new()).unwrap().params;
    let own = own(&[("satin_guide_pattern_position", "render_at_rungs")]);
    let placing = Placing { own: &own, adds: false, closed: false, anchor: None };
    let mut warnings = Vec::new();
    let outline = [p(0.0, 0.0), p(1.0, 0.0)];
    assert_eq!(between_rails(&outline, &one, &params, &placing, &mut warnings, &mut Budget::DEFAULT.meter()).unwrap(), None);
    let said: Vec<String> = warnings.iter().map(ToString::to_string).collect();
    assert_eq!(
        said,
        ["warning SC-W0409: This ripple stitch's guide can't be used: its satin guide is too short to place a copy on. It is sewn without it."]
    );
}

#[test]
fn req_rip_013_a_guide_whose_middle_or_first_pair_has_no_length_places_nothing() {
    // Rails that meet: every pair has no width, and the middle runs along them.
    let params = SatinParams::from_set(&ParamSet::new()).unwrap().params;
    let own = own(&[]);
    let say = |sections: Vec<Section>, closed: bool| {
        let placing = Placing { own: &own, adds: closed, closed, anchor: None };
        let mut warnings = Vec::new();
        let outline = [p(0.0, 0.0), p(1.0, 0.0), p(1.0, 1.0), p(0.0, 0.0)];
        let copies = between_rails(&outline, &sections, &params, &placing, &mut warnings, &mut Budget::DEFAULT.meter()).unwrap();
        (copies, warnings.iter().map(ToString::to_string).collect::<Vec<_>>())
    };
    let flat = vec![[vec![p(0.0, 0.0), p(5.0, 0.0)], vec![p(0.0, 0.0), p(5.0, 0.0)]]];
    let (copies, said) = say(flat, true);
    assert_eq!((copies, said.len()), (None, 1));
    assert!(said[0].contains("its satin guide's first pair of points has no width"), "{said:?}");
    // A guide of one point has a middle of no length.
    let point = vec![[vec![p(1.0, 1.0), p(1.0, 1.0)], vec![p(1.0, 2.0), p(1.0, 2.0)]]];
    for closed in [false, true] {
        let (copies, said) = say(point.clone(), closed);
        assert_eq!(copies, None);
        assert!(said.len() == 1 && said[0].contains("its satin guide's middle has no length"), "{said:?}");
    }
}

#[test]
fn req_rip_014_an_anchor_line_s_ends_land_on_each_pair() {
    // The square's anchor runs across its middle, from (0, 1) to (2, 1), and lands on each pair: the square
    // turns 90° and doubles. A closed line with an anchor is placed so, not centred.
    let square = [(0.0, 0.0), (2.0, 0.0), (2.0, 2.0), (0.0, 2.0), (0.0, 0.0)];
    let column = satin_guide(&COLUMN, &[]);
    let anchor = Some([p(0.0, 1.0), p(2.0, 1.0)]);
    let copies = copied(&square, &column, &[("line_count", "1"), ("satin_guide_pattern_position", "render_at_rungs")], true, anchor);
    let at = |x: f64| vec![(x + 2.0, 0.0), (x + 2.0, 4.0), (x - 2.0, 4.0), (x - 2.0, 0.0), (x + 2.0, 0.0)];
    assert_eq!(copies, [at(10.0), at(20.0)]);
    // An open line's anchor takes the place of its own ends too.
    let tilted =
        copied(&TENT, &column, &[("line_count", "1"), ("satin_guide_pattern_position", "render_at_rungs")], false, Some([p(1.0, 0.0), p(1.0, 1.0)]));
    assert_eq!(tilted[0], [(6.0, 0.0), (10.0, 4.0), (14.0, 0.0)]);
}

#[test]
fn req_rip_014_an_anchor_line_whose_ends_are_one_point_is_left_out() {
    let square = [(0.0, 0.0), (2.0, 0.0), (2.0, 2.0), (0.0, 2.0), (0.0, 0.0)];
    let column = satin_guide(&COLUMN, &[]);
    let (copies, said) = placed_by(&square, &column, &[("line_count", "3")], true, Some([p(1.0, 1.0), p(1.0, 1.0)]));
    assert_eq!(said, ["warning SC-W0409: This ripple stitch's guide can't be used: its anchor line's ends are one point. It is sewn without it."]);
    // Without it, the closed line is centred.
    assert_eq!(
        copies.unwrap().lines.iter().map(|line| rounded(line)).collect::<Vec<_>>(),
        copied(&square, &column, &[("line_count", "3")], true, None)
    );
}

#[test]
fn req_rip_013_adaptive_places_a_copy_where_its_box_clears_the_last_one_s_moved_back() {
    // A zigzag across the anchor, 0.5 mm either side, doubled across the column: each copy's box runs 1 mm
    // either side of its pair, and the last box moved back by 1 mm, doubled, lies 1 to 3 mm ahead. The next
    // copy is placed on the first pair, 0.1 CSS pixels apart, more than 4 mm on.
    let zigzag = [(0.0, 0.0), (1.0, 0.5), (1.5, -0.5), (2.0, 0.0)];
    let column = satin_guide(&COLUMN, &[]);
    let starts = |with: &[(&str, &str)]| -> Vec<f64> {
        let mut with = with.to_vec();
        with.push(("satin_guide_pattern_position", "adaptive"));
        copied(&zigzag, &column, &with, false, None).iter().map(|copy| copy[0].0).collect()
    };
    assert_eq!(starts(&[("min_line_dist_mm", "1")]), [0.0, 4.022, 8.043, 12.065, 16.087]);
    // Without min_line_dist_mm, 0.1 CSS pixels apart: more than 2.053 mm on.
    assert_eq!(starts(&[]), [0.0, 2.064, 4.128, 6.191, 8.255, 10.319, 12.383, 14.446, 16.51, 18.574]);
    // Each copy is the zigzag turned 90° and doubled, its ends on the pair's.
    let first = copied(&zigzag, &column, &[("satin_guide_pattern_position", "adaptive")], false, None);
    assert_eq!(first[0], [(0.0, 0.0), (-1.0, 2.0), (1.0, 3.0), (0.0, 4.0)]);
    // Wherever the zigzag lies, its ends land on the pairs: moved by (3, 5), or turned a quarter round, it
    // places the same copies.
    let moved: Vec<(f64, f64)> = zigzag.iter().map(|&(x, y)| (x + 3.0, y + 5.0)).collect();
    assert_eq!(copied(&moved, &column, &[("satin_guide_pattern_position", "adaptive")], false, None), first);
    let turned: Vec<(f64, f64)> = zigzag.iter().map(|&(x, y)| (-y, x)).collect();
    assert_eq!(copied(&turned, &column, &[("satin_guide_pattern_position", "adaptive")], false, None), first);
    // As in Ink/Stitch, the box is moved back across the guide the same way whichever way the copies run. With
    // the rails swapped they run the other way, and the box moved back lies behind each copy: the zigzag, 1 mm
    // tall and doubled across the column, is copied (1 − 0.5) × 2 mm apart, where the other way (1 + 0.5) × 2.
    let swapped = satin_guide(&COLUMN, &[("swap_satin_rails", "true")]);
    let with = [("min_line_dist_mm", "0.5"), ("satin_guide_pattern_position", "adaptive")];
    let swapped_starts: Vec<f64> = copied(&zigzag, &swapped, &with, false, None).iter().map(|copy| copy[0].0).collect();
    assert_eq!(swapped_starts[..4], [0.0, 1.005, 2.011, 3.016]);
    assert_eq!(swapped_starts.len(), 20);
    assert_eq!(starts(&[("min_line_dist_mm", "0.5")]), [0.0, 3.016, 6.033, 9.049, 12.065, 15.081, 18.098]);
}

#[test]
fn req_rip_013_an_adaptive_guide_places_1000_copies_at_most_and_says_how_many_places_it_found() {
    // A line 1 mm long across rails 1 mm apart and 100 mm long: each copy is a segment across them, and with
    // min_line_dist_mm 0.01 each clears the last, so every pair, 0.1 CSS pixels apart, holds one.
    let sections = vec![[vec![p(0.0, 0.0), p(100.0, 0.0)], vec![p(0.0, 1.0), p(100.0, 1.0)]]];
    let meter = &mut Budget::DEFAULT.meter();
    let tried = satin::ripple_pairs(&sections, ADAPTIVE_STEP, 0.0, meter).unwrap().len();
    assert!(tried > MOST, "{tried}");
    let own = own(&[("min_line_dist_mm", "0.01")]);
    let copies = adaptive(&[p(0.0, 0.0), p(1.0, 0.0)], &sections, (p(0.0, 0.0), 1.0, [1.0, 0.0]), &own, meter).unwrap();
    assert_eq!((copies.lines.len(), copies.cut), (MOST, Some(Cut::Places(tried))));
    assert_eq!(rounded(&copies.lines[1]), [(0.026, 0.0), (0.026, 1.0)]);
}

#[test]
fn a_guide_reads_as_a_satin_column_where_it_is_one_or_has_several_subpaths() {
    let own = own(&[("reverse_rails", "second"), ("swap_satin_rails", "true")]);
    let read = |subpaths: &[&[(f64, f64)]], with: &[(&str, &str)]| {
        let guide = Guide { path: path(subpaths, false), width: Mm::new(2.0).unwrap(), join: Join::Round, params: with.iter().copied().collect() };
        let mut said = Vec::new();
        (super::guide(&guide, &own, &mut said), said.iter().map(ToString::to_string).collect::<Vec<_>>())
    };
    let one: &[&[(f64, f64)]] = &[&[(0.0, 0.0), (10.0, 0.0)]];
    assert_eq!(read(one, &[]), (Some(RippleGuide::Line { path: path(one, false), nodes: false }), Vec::new()));
    assert!(matches!(read(one, &[("stroke_method", "manual_stitch")]).0, Some(RippleGuide::Line { nodes: true, .. })));
    // A satin column, or 2 subpaths, its rails read with the ripple's reverse_rails and swap_satin_rails.
    for (subpaths, with) in [(one, &[("satin_column", "true")][..]), (&COLUMN[..2], &[][..])] {
        let Some(RippleGuide::Satin { width, join, params, .. }) = read(subpaths, with).0 else { panic!("{subpaths:?}") };
        assert_eq!((width, join, params.reverse_rails, params.swap_satin_rails), (Mm::new(2.0).unwrap(), Join::Round, "second", true));
    }
    // Its own settings that don't read leave it out, said by SC-W0409 alone: their error would refuse the
    // ripple, which sews without the guide. What reading them warns of is kept.
    let (none, said) = read(&COLUMN[..2], &[("zigzag_spacing_mm", "wide")]);
    assert_eq!(none, None);
    assert_eq!(said, ["warning SC-W0409: This ripple stitch's guide can't be used: its satin guide's settings don't read. It is sewn without it."]);
    let (_, said) = read(&COLUMN[..2], &[("zigzag_spacing_mm", "wide"), ("swap_satin_rails", "maybe")]);
    assert_eq!(said.len(), 1, "{said:?}");
    let (none, said) = read(&COLUMN[..2], &[("zigzag_spacing_mm", "200"), ("contour_underlay", "maybe")]);
    assert_eq!(none, None);
    assert!(said.len() == 2 && said[0].starts_with("warning SC-W0102") && said[1].starts_with("warning SC-W0409"), "{said:?}");
    let (some, said) = read(&COLUMN[..2], &[("zigzag_spacing_mm", "200")]);
    assert!(some.is_some() && said.len() == 1 && said[0].starts_with("warning SC-W0102"), "{said:?}");
}

#[test]
fn a_place_moves_turns_and_scales_in_that_order() {
    // Moved by (0.5, -1), turned by the angle whose cosine is 0.6 about (1, 2), then scaled by 2 across and 3
    // down about it.
    let place = Place { move_by: [0.5, -1.0], turn: [0.6, 0.8], scale: [2.0, 3.0], about: p(1.0, 2.0) };
    let [x, y] = place.apply([3.0, 4.0]);
    assert!((x - 2.4).abs() < 1e-12 && (y - 9.8).abs() < 1e-12, "{x} {y}");
    assert_eq!(place.corners(&[[3.0, 4.0]; 4]).map(|[x, y]| ((x * 1e9).round(), (y * 1e9).round())), [(2.4e9, 9.8e9); 4]);
    let meter = &mut Budget::DEFAULT.meter();
    assert_eq!(rounded(&placed(&[p(3.0, 4.0), p(1.0, 2.0)], place, meter).unwrap()), [(2.4, 9.8), (3.2, 1.4)]);
    // Each point costs a unit of work.
    let mut tight = Budget { max_stitches: 100, max_work: 1 }.meter();
    assert_eq!(placed(&[p(0.0, 0.0), p(1.0, 1.0)], place, &mut tight), Err(Exhausted::Work));
}

#[test]
fn directions_turns_and_scales_are_plain_arithmetic() {
    assert_eq!(direction(p(1.0, 1.0), p(4.0, 5.0)), [0.6, 0.8]);
    // A direction of no length points along x, as atan2(0, 0) is 0.
    assert_eq!(direction(p(1.0, 1.0), p(1.0, 1.0)), [1.0, 0.0]);
    assert_eq!(between([1.0, 0.0], [0.0, 1.0]), [0.0, 1.0]);
    assert_eq!(between([0.6, 0.8], [0.6, 0.8]), [1.0, 0.0]);
    assert_eq!(between([0.0, 1.0], [1.0, 0.0]), [0.0, -1.0]);
    assert_eq!(["0", "1", "2", "3", "9"].map(|axis| axes(axis, 0.5)), [[0.5, 0.5], [0.5, 1.0], [1.0, 0.5], [1.0, 1.0], [0.5, 0.5]]);
    assert_eq!([ratio(3.0, 2.0), ratio(3.0, 0.0)], [1.5, 0.0]);
    // numpy.linspace's spacing, its last exactly the end.
    assert_eq!(spaced(1.0, 0.5, 3), [1.0, 0.75, 0.5]);
    assert_eq!(spaced(0.3, 0.7, 2), [0.3, 0.7]);
    assert_eq!(spaced(2.0, 5.0, 1), [2.0]);
    assert_eq!(spaced(0.0, 1.0, 0), Vec::<f64>::new());
    assert_eq!(spaced(0.1, 0.2, 4).last(), Some(&0.2));
    // 1 + 3 × (−0.7 / 3) is 0.30000000000000004, and the last is the end itself.
    assert_eq!(spaced(1.0, 0.3, 4).last(), Some(&0.3));
    assert_eq!(bounds(&[p(1.0, 5.0), p(-2.0, 3.0), p(4.0, 4.0)]), ([-2.0, 3.0], [4.0, 5.0]));
    assert_eq!(bounds(&[]), ([0.0, 0.0], [0.0, 0.0]));
    assert_eq!(capped(MOST), (MOST, None));
    assert_eq!(capped(MOST + 1), (MOST, Some(Cut::Places(MOST + 1))));
    assert_eq!(counted(Count { lines: 7, cut: Some(9.5) }), (7, Some(Cut::Distance(9.5))));
    let mut meter = Budget { max_stitches: 6, max_work: u64::MAX }.meter();
    assert_eq!((room(&[p(0.0, 0.0); 3], 2, &mut meter), room(&[p(0.0, 0.0); 3], 3, &mut meter)), (Ok(()), Err(Exhausted::Stitches)));
}

#[test]
fn a_line_s_centroid_is_its_segments_middles_weighed_by_length() {
    // A right angle of arms 2 and 4: middles (1, 0) and (2, 2), weighed 2 and 4.
    assert_eq!(centroid(&[p(0.0, 0.0), p(2.0, 0.0), p(2.0, 4.0)]), Some(p(10.0 / 6.0, 8.0 / 6.0)));
    // A line of no length: the mean of its points. None: no point.
    assert_eq!(centroid(&[p(1.0, 1.0), p(1.0, 1.0), p(1.0, 1.0)]), Some(p(1.0, 1.0)));
    assert_eq!(centroid(&[p(3.0, 1.0)]), Some(p(3.0, 1.0)));
    assert_eq!(centroid(&[]), None);
}

#[test]
fn boxes_that_share_a_point_are_not_disjoint() {
    let square = |x: f64, y: f64, side: f64| [[x, y], [x, y + side], [x + side, y + side], [x + side, y]];
    assert!(disjoint(&square(0.0, 0.0, 1.0), &square(2.0, 0.0, 1.0)));
    assert!(disjoint(&square(0.0, 0.0, 1.0), &square(0.0, 2.0, 1.0)));
    assert!(!disjoint(&square(0.0, 0.0, 1.0), &square(1.0, 0.0, 1.0)), "they touch along x = 1");
    assert!(!disjoint(&square(0.0, 0.0, 2.0), &square(1.0, 1.0, 2.0)));
    // A diamond that only its own side's line holds apart from a square.
    let diamond = |shift: f64| [[2.5 + shift, 1.6 + shift], [3.4 + shift, 2.5 + shift], [2.5 + shift, 3.4 + shift], [1.6 + shift, 2.5 + shift]];
    assert!(disjoint(&square(0.0, 0.0, 2.0), &diamond(0.0)));
    assert!(!disjoint(&square(0.0, 0.0, 2.0), &diamond(-0.1)));
    // Boxes flat as lines along one line, apart or touching, and boxes that are points.
    let segment = |from: f64, to: f64| [[from, 0.0], [from, 0.0], [to, 0.0], [to, 0.0]];
    assert!(disjoint(&segment(0.0, 1.0), &segment(2.0, 3.0)));
    assert!(!disjoint(&segment(0.0, 1.0), &segment(1.0, 2.0)));
    assert!(disjoint(&[[0.0, 0.0]; 4], &[[1.0, 0.0]; 4]));
    assert!(!disjoint(&[[1.0, 0.0]; 4], &[[1.0, 0.0]; 4]));
}

#[test]
fn a_turned_box_is_held_apart_by_the_line_square_to_its_side() {
    // A box 4 by 1 turned 30°, and a square 0.2 a side 0.2 beyond its long side: only the line square to that
    // side, at 120°, has them on each side of it. Along x, along y, and at -30° and 60°, their shadows meet.
    let (cos, sin) = (3.0_f64.sqrt() / 2.0, 0.5);
    let at = |u: f64, v: f64| [u * cos - v * sin, u * sin + v * cos];
    let turned = [at(0.0, 0.0), at(4.0, 0.0), at(4.0, 1.0), at(0.0, 1.0)];
    let square = |[x, y]: [f64; 2]| [[x, y], [x + 0.2, y], [x + 0.2, y + 0.2], [x, y + 0.2]];
    assert!(disjoint(&turned, &square(at(2.0, 1.3))));
    // Moved 0.4 back across that side, it overlaps the box.
    assert!(!disjoint(&turned, &square(at(2.0, 0.9))));
    // A trapezoid's slanted side holds apart a square beyond it, where no line along or square to its other
    // sides does.
    let trapezoid = [[0.0, 0.0], [0.0, 4.0], [4.0, 2.0], [4.0, 0.0]];
    let square = |x: f64, y: f64| [[x, y], [x, y + 0.5], [x + 0.5, y + 0.5], [x + 0.5, y]];
    assert!(disjoint(&trapezoid, &square(1.25, 3.5)));
    assert!(!disjoint(&trapezoid, &square(1.25, 3.0)));
}
