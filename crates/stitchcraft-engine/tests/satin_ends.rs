//! Where a satin column starts and ends (`REQ-SAT-013`, `REQ-SAT-014`), and the neighbours every element
//! is generated with (`REQ-GEN-003`): near where the elements before it left the needle, and near where
//! the next element starts, as Ink/Stitch sews them by default.
//!
//! Most cases sew a straight column 10 mm long and 6 mm wide along x, between rails at y = 0 and y = 6,
//! with no underlay. Its line runs at y = 3 in stitches of 2.5 mm, and its cuts are its pairs, every
//! 0.4 mm along x. Each case compares the column sewn between neighbours with the column sewn alone.

// Test code may unwrap, panic and index (clippy.toml allows it in tests).
#![allow(clippy::unwrap_used, clippy::panic)]

use stitchcraft_core::units::MM_PER_SVG_PX;
use stitchcraft_core::{Budget, Point};
use stitchcraft_engine::PlanOutcome;
use stitchcraft_engine::design::{DesignSettings, Element, Shape};
use stitchcraft_engine::generate::approach;
use stitchcraft_engine::generators::{Approach, Neighbours};
use stitchcraft_plan::{Role, Stitch, StitchKind, Thread};
use stitchcraft_testkit::designs::{BLUE, RED, along, messages, p, planned, polylines, widened};
use stitchcraft_testkit::satins::{ladder, sewn_between, sewn_satin};

/// The column 10 mm long and 6 mm wide, sewn with `params` between `neighbours`, and its warnings.
fn column(params: &[(&str, &str)], neighbours: &Neighbours) -> (Vec<Point>, Vec<String>) {
    sewn_between(&ladder(10.0, 6.0, &[]), params, neighbours)
}

/// The column sewn alone, with `params`.
fn alone(params: &[(&str, &str)]) -> Vec<Point> {
    sewn_satin(&ladder(10.0, 6.0, &[]), params).0
}

/// With the needle left at `needle`.
fn after(needle: (f64, f64)) -> Neighbours {
    Neighbours { needle: Some(p(needle.0, needle.1)), next: None }
}

/// Before an element that starts at `start`.
fn before(start: (f64, f64)) -> Neighbours {
    Neighbours { needle: None, next: Some(Approach::Point(p(start.0, start.1))) }
}

/// Whether `got` are the points `want`, to within rounding.
fn same(got: &[Point], want: &[Point]) -> bool {
    got.len() == want.len() && got.iter().zip(want).all(|(got, want)| got.distance(*want) < 1e-9)
}

/// The points that split the line from `a` to `b` into `parts` equal parts, as the needle travels it.
fn travel(a: Point, b: Point, parts: u32) -> Vec<Point> {
    (1..parts).map(|k| a.lerp(b, f64::from(k) / f64::from(parts))).collect()
}

#[test]
fn req_sat_013_a_column_starts_on_its_line_nearest_the_needle() {
    let top = alone(&[]);
    // The needle 7 mm above the line, 4 mm above the second rail: farther than the jump length (3 mm)
    // from both. The column starts at (5.1, 3) and follows the line from the cut through it, at x = 5.2,
    // to the first stitch's, at x = 0. The line's points at x = 5.2 and 5 are within 0.3 mm of the start
    // and left out, and the needle goes on to the first stitch, (0, 0), in stitches no longer than 2.5 mm.
    let (sewn, warnings) = column(&[], &after((5.1, 10.0)));
    assert!(warnings.is_empty(), "{warnings:?}");
    let way = [p(5.1, 3.0), p(3.8, 3.0), p(2.5, 3.0), p(0.0, 3.0), p(0.0, 1.5)];
    assert!(same(&sewn[..way.len()], &way), "{:?}", &sewn[..way.len() + 2]);
    assert!(same(&sewn[way.len()..], &top), "then the column as it is sewn alone");
    // Off, or with no needle before it, the column starts where it is drawn.
    assert!(same(&column(&[("start_at_nearest_point", "false")], &after((5.1, 10.0))).0, &top));
    assert!(same(&column(&[], &Neighbours::default()).0, &top));
    // The way leads to the first stitch wherever it is: turned, the column starts sewing at (10, 0).
    let turned = [("reverse_rails", "both")];
    let (sewn, _) = column(&turned, &after((5.1, 10.0)));
    let way = [p(5.1, 3.0), p(7.5, 3.0), p(10.0, 3.0), p(10.0, 1.5)];
    assert!(same(&sewn[..way.len()], &way), "{:?}", &sewn[..way.len() + 2]);
    assert!(same(&sewn[way.len()..], &alone(&turned)));
}

#[test]
fn req_sat_013_a_column_starts_on_its_outline_when_only_that_is_near() {
    let top = alone(&[]);
    // The needle 2 mm above the second rail and 5 mm above the line: the column starts on its edge.
    let (sewn, _) = column(&[], &after((5.1, 8.0)));
    assert!(sewn[0].distance(p(5.1, 6.0)) < 1e-9, "{:?}", &sewn[..4]);
    assert!(sewn.ends_with(&top));
    // The edge is the compensated one: 1 mm of pull compensation moves it to y = 7.
    let (sewn, _) = column(&[("pull_compensation_mm", "1")], &after((5.1, 8.0)));
    assert!(sewn[0].distance(p(5.1, 7.0)) < 1e-9, "{:?}", &sewn[..4]);
    // The element's jump length decides: 5 mm keeps the start on the line, and 0 is the design's 3 mm.
    let (sewn, _) = column(&[("min_jump_stitch_length_mm", "5")], &after((5.1, 8.0)));
    assert!(sewn[0].distance(p(5.1, 3.0)) < 1e-9, "{:?}", &sewn[..4]);
    let (sewn, _) = column(&[("min_jump_stitch_length_mm", "0")], &after((5.1, 8.0)));
    assert!(sewn[0].distance(p(5.1, 6.0)) < 1e-9, "{:?}", &sewn[..4]);
    // An outline exactly the jump length away is not within it.
    let (sewn, _) = column(&[("min_jump_stitch_length_mm", "2")], &after((5.1, 8.0)));
    assert!(sewn[0].distance(p(5.1, 3.0)) < 1e-9, "{:?}", &sewn[..4]);
    // The needle on the line, or nearer the line than the jump length: the line.
    let (sewn, _) = column(&[], &after((5.1, 5.5)));
    assert!(sewn[0].distance(p(5.1, 3.0)) < 1e-9, "{:?}", &sewn[..4]);
}

#[test]
fn req_sat_013_the_line_runs_at_its_position_between_the_rails() {
    // A quarter of the way from the first rail to the second: y = 1.5.
    let (sewn, warnings) = column(&[("running_stitch_position", "25")], &after((5.1, 10.0)));
    assert!(warnings.is_empty(), "{warnings:?}");
    assert!(sewn[0].distance(p(5.1, 1.5)) < 1e-9, "{:?}", &sewn[..4]);
    assert!(sewn.iter().take_while(|point| point.x() > 0.0).all(|point| (point.y() - 1.5).abs() < 1e-9), "{sewn:?}");
}

#[test]
fn req_sat_014_a_column_ends_on_its_outline_nearest_the_next_stitch() {
    let top = alone(&[]);
    // The next element starts above the middle of the second rail: the next stitch is (5.1, 6), on the
    // rails, and the end is there on the outline. Its cut is the pair at x = 5.2, which the top stitches
    // reach where they slant to (5.2, 0). The column sews to there, follows the line from the cut to its
    // end, sews the rest from the column's end back to the cut, and stitches the end.
    let (sewn, warnings) = column(&[], &before((5.1, 9.0)));
    assert!(warnings.is_empty(), "{warnings:?}");
    let cut = top.iter().position(|point| point.distance(p(5.2, 0.0)) < 1e-9).unwrap();
    let (last, end) = (*top.last().unwrap(), p(5.1, 6.0));
    let way = [p(5.2, 3.0), p(7.5, 3.0), p(10.0, 3.0)];
    let mut want = top[..=cut].to_vec();
    want.extend(travel(top[cut], way[0], 2));
    want.extend(way);
    want.extend(travel(way[2], last, 2));
    want.extend(top[cut..].iter().rev());
    want.extend(travel(top[cut], end, 3));
    want.push(end);
    assert!(same(&sewn, &want), "{sewn:?}");
    // Off, or with nothing after it, the column ends where it is drawn.
    assert!(same(&column(&[("end_at_nearest_point", "false")], &before((5.1, 9.0))).0, &top));
    assert!(same(&column(&[], &Neighbours::default()).0, &top));
}

#[test]
fn req_sat_014_an_end_near_the_line_s_end_changes_nothing() {
    // The end is on the outline, the rails' lines, so only in a column at most 2.6 mm wide can it come within
    // 5 CSS pixels (1.32 mm) of the line's end. In one 2 mm wide, before an element past its end and below
    // it, the end would be (10, 0), 1 mm from the line's end at (10, 1): the column is sewn as it is alone.
    let narrow = ladder(10.0, 2.0, &[]);
    assert!(same(&sewn_between(&narrow, &[], &before((12.0, -1.0))).0, &sewn_satin(&narrow, &[]).0));
    // In one 3 mm wide the end, (10, 0), is 1.5 mm from the line's end, at (10, 1.5). The top stitches are
    // cut at their last pair, where they reach (10, 0): the needle goes to the line's end and back across the
    // pair, and the column's last stitch, from (10, 3), ends at the end.
    let wide = ladder(10.0, 3.0, &[]);
    let top = sewn_satin(&wide, &[]).0;
    let (sewn, warnings) = sewn_between(&wide, &[], &before((12.0, -1.0)));
    assert!(warnings.is_empty(), "{warnings:?}");
    let mut want = top[..top.len() - 1].to_vec();
    want.extend([p(10.0, 1.5), p(10.0, 3.0), p(10.0, 0.0)]);
    assert!(same(&sewn, &want), "{:?}", &sewn[sewn.len() - 6..]);
    // Exactly 5 CSS pixels from the line's end is not nearer than that: a column twice as wide ends at its
    // edge.
    let five_px = 5.0 * MM_PER_SVG_PX;
    let (sewn, _) = sewn_between(&ladder(10.0, 2.0 * five_px, &[]), &[], &before((12.0, -1.0)));
    assert!(sewn.last().unwrap().distance(p(10.0, 0.0)) < 1e-9, "{:?}", &sewn[sewn.len() - 3..]);
}

#[test]
fn req_sat_014_the_next_element_s_rails_say_where_the_next_stitch_is() {
    // A column beside this one, 4 mm past its second rail, starting at its nearest point: the next
    // stitch is the point of this column's rails nearest its rails, the first of equally near ones.
    let rails = vec![vec![p(0.0, 10.0), p(10.0, 10.0)], vec![p(0.0, 16.0), p(10.0, 16.0)]];
    let (sewn, _) = column(&[], &Neighbours { needle: None, next: Some(Approach::Shape(rails.clone())) });
    assert!(sewn.last().unwrap().distance(p(0.0, 6.0)) < 1e-9, "{:?}", &sewn[sewn.len() - 3..]);
    // Cut at its start, the column is sewn from its first stitch along the line to its end, and back.
    let top = alone(&[]);
    assert!(sewn[0].distance(top[0]) < 1e-9);
    let back: Vec<Point> = top.iter().rev().copied().collect();
    let turn = sewn.iter().position(|point| point.distance(back[0]) < 1e-9).unwrap();
    assert!(same(&sewn[turn..turn + back.len()], &back), "{sewn:?}");
    // The rails are measured as they are sewn. Turned, this column's rails start at x = 10, and the first of
    // equally near points is there.
    let next = Neighbours { needle: None, next: Some(Approach::Shape(rails)) };
    let (sewn, _) = column(&[("reverse_rails", "both")], &next);
    assert!(sewn.last().unwrap().distance(p(10.0, 6.0)) < 1e-9, "{:?}", &sewn[sewn.len() - 3..]);
    // Swapped, the rail at y = 6 comes first: of the 2 rails as near the next element's first point, the
    // next stitch is on that one.
    let (sewn, _) = column(&[], &before((5.1, 3.0)));
    assert!(sewn.last().unwrap().distance(p(5.1, 0.0)) < 1e-9, "{:?}", &sewn[sewn.len() - 3..]);
    let (sewn, _) = column(&[("swap_satin_rails", "true")], &before((5.1, 3.0)));
    assert!(sewn.last().unwrap().distance(p(5.1, 6.0)) < 1e-9, "{:?}", &sewn[sewn.len() - 3..]);
}

#[test]
fn req_sat_014_an_end_as_near_2_cuts_is_cut_at_the_first() {
    // Pairs every 0.5 mm, and the end at x = 5.25, as near the pair at 5 as the pair at 5.5: the column is
    // cut at 5, and the needle follows the line from there.
    let (sewn, _) = column(&[("zigzag_spacing_mm", "0.5")], &before((5.25, -3.0)));
    assert!(sewn.last().unwrap().distance(p(5.25, 0.0)) < 1e-9, "{:?}", &sewn[sewn.len() - 3..]);
    let has = |q: Point| sewn.iter().any(|point| point.distance(q) < 1e-9);
    assert!(has(p(5.0, 3.0)) && !has(p(5.5, 3.0)), "{sewn:?}");
}

/// Where each of `landmarks` is first sewn in `sewn`.
fn first_at(sewn: &[Point], landmarks: &[(f64, f64)]) -> Vec<usize> {
    landmarks.iter().map(|&(x, y)| sewn.iter().position(|point| point.distance(p(x, y)) < 1e-9).unwrap()).collect()
}

#[test]
fn req_sat_014_every_underlay_is_cut_in_2_at_the_end() {
    // All 3 underlays, the end above the middle of the second rail, (5.1, 6), and the cut at x = 5.2.
    let all = [("center_walk_underlay", "true"), ("contour_underlay", "true"), ("zigzag_underlay", "true")];
    let (sewn, warnings) = column(&all, &before((5.1, 9.0)));
    assert!(warnings.is_empty(), "{warnings:?}");
    // The first pass works from the start to the cut: the centre walk there and back, the contour up its
    // first side (y = 0.4) and back down its second (y = 5.6), the zigzag there and back, the top stitches.
    // The needle follows the line to its end, and the second pass works from there back to the cut: the
    // centre walk, the contour's second side and then its first, the zigzag, the top stitches.
    let landmarks =
        [(0.0, 3.0), (0.4, 0.4), (0.4, 5.6), (0.0, 0.2), (0.0, 0.0), (7.5, 3.0), (10.0, 3.0), (9.6, 5.6), (9.6, 0.4), (10.0, 0.2), (10.0, 6.0)];
    let at = first_at(&sewn, &landmarks);
    assert!(at.windows(2).all(|w| w[0] < w[1]), "{at:?}");
    assert!(sewn[..at[5]].iter().all(|point| point.x() < 5.2 + 1e-9), "the first pass stays before the cut");
    // The second pass stays past the cut, up to the top stitches' last, (5.2, 0), and the end is stitched last.
    let (second_pass, to_end) = sewn[at[6]..].split_at(sewn.len() - at[6] - 3);
    assert!(second_pass.iter().all(|point| point.x() > 5.2 - 1e-9), "{second_pass:?}");
    let mut want = travel(p(5.2, 0.0), p(5.1, 6.0), 3);
    want.push(p(5.1, 6.0));
    assert!(same(to_end, &want), "{to_end:?}");
    // With only a contour, the second pass starts on the contour at x = 9.6, and the needle still follows
    // the line to its end first.
    let (sewn, _) = column(&[("contour_underlay", "true")], &before((5.1, 9.0)));
    assert!(sewn.iter().any(|point| point.distance(p(10.0, 3.0)) < 1e-9), "{sewn:?}");
    // An odd centre walk turns the passes round: the first works from the cut to the column's end and back,
    // and the second from the cut to the start and back. It starts at the cut, on the line.
    let odd = [all.as_slice(), &[("center_walk_underlay_repeats", "3")]].concat();
    let (sewn, _) = column(&odd, &before((5.1, 9.0)));
    assert!(sewn[0].distance(p(5.2, 3.0)) < 1e-9, "{:?}", &sewn[..3]);
    let landmarks =
        [(10.0, 3.0), (9.6, 0.4), (9.6, 5.6), (10.0, 0.2), (10.0, 6.0), (5.0, 3.0), (0.0, 3.0), (0.4, 5.6), (0.4, 0.4), (0.0, 0.2), (0.0, 0.0)];
    let at = first_at(&sewn, &landmarks);
    assert!(at.windows(2).all(|w| w[0] < w[1]), "{at:?}");
    assert!(sewn[..at[5]].iter().all(|point| point.x() > 5.2 - 1e-9), "the first pass stays past the cut");
    // Between the passes the needle goes from the top stitches at the cut to the line there, and no farther.
    let between = [p(5.2, 6.0), p(5.2, 4.5), p(5.2, 3.0), p(5.0, 3.0)];
    assert!(sewn.windows(between.len()).any(|window| same(window, &between)), "{sewn:?}");
    // The top stitches' last, (5.2, 6), is within the shortest stitch of the end: the end takes its place.
    assert!(same(&sewn[sewn.len() - 2..], &[p(5.2, 0.0), p(5.1, 6.0)]), "{:?}", &sewn[sewn.len() - 4..]);
}

/// A straight satin column `id` between rails at `y` and `y + 6`, 10 mm long, sewn with `thread` and
/// `params`.
fn satin(id: &str, y: f64, thread: &Thread, params: &[(&str, &str)]) -> Element {
    let path = polylines(&[&[(0.0, y), (10.0, y)], &[(0.0, y + 6.0), (10.0, y + 6.0)]]);
    along(id, path, thread, &[params, &[("satin_column", "true")]].concat())
}

/// `element` made a fill of the square 10 mm wide at its path's start, which StitchCraft does not sew yet.
fn as_fill(element: Element) -> Element {
    let start = element.shape.path().points().next().unwrap();
    let (x, y) = (start.x(), start.y());
    let square = polylines(&[&[(x, y), (x + 10.0, y), (x + 10.0, y + 10.0), (x, y + 10.0), (x, y)]]);
    Element { shape: Shape::Fill { path: square, rule: Default::default() }, ..element }
}

/// The needle points the element `index` sews in `outcome`'s plan, locks left out.
fn sewn_by(outcome: &PlanOutcome, index: usize) -> Vec<Point> {
    let plan = outcome.plan.as_ref().unwrap();
    let mine = |s: &&Stitch| s.kind == StitchKind::Normal && s.origin.role != Role::Lock && s.origin.element.map(|e| e.index()) == Some(index);
    plan.stitches().filter(mine).map(|s| s.at).collect()
}

#[test]
fn req_gen_003_elements_see_the_needle_before_and_the_next_element() {
    // 2 columns, the second 4 mm past the first. The first ends nearest the second's rails, at (0, 6), and
    // the second starts on its line nearest that, at (0, 13).
    let outcome = planned(vec![satin("a", 0.0, &RED, &[]), satin("b", 10.0, &RED, &[])]);
    assert!(messages(&outcome).is_empty(), "{:?}", messages(&outcome));
    let (a, b) = (sewn_by(&outcome, 0), sewn_by(&outcome, 1));
    assert!(a.last().unwrap().distance(p(0.0, 6.0)) < 1e-9, "{a:?}");
    assert!(b[0].distance(p(0.0, 13.0)) < 1e-9, "{b:?}");
    // Whatever their thread.
    let outcome = planned(vec![satin("a", 0.0, &RED, &[]), satin("b", 10.0, &BLUE, &[])]);
    assert!(sewn_by(&outcome, 1)[0].distance(p(0.0, 13.0)) < 1e-9);
    // An element that sews nothing leaves the needle where it was, and offers nothing to end near: here a
    // fill of a method not sewn yet.
    let fill = as_fill(along("fill", polylines(&[&[(0.0, 30.0), (1.0, 30.0)]]), &RED, &[("fill_method", "contour_fill")]));
    let outcome = planned(vec![satin("a", 0.0, &RED, &[]), fill, satin("b", 10.0, &RED, &[])]);
    let (a, b) = (sewn_by(&outcome, 0), sewn_by(&outcome, 1));
    assert_eq!(a, sewn_satin(&ladder(10.0, 6.0, &[]), &[]).0, "it ends as it does alone");
    assert!(b[0].distance(p(10.0, 13.0)) < 1e-9, "{b:?}");
}

#[test]
fn req_gen_003_what_an_element_offers_comes_from_its_shape_and_settings() {
    let stroke = along("s", polylines(&[&[(2.0, 3.0), (8.0, 3.0)]]), &RED, &[]);
    assert_eq!(approach(&stroke, &DesignSettings::default(), &Budget::DEFAULT), Some(Approach::Point(p(2.0, 3.0))));
    let rails = vec![vec![p(0.0, 0.0), p(10.0, 0.0)], vec![p(0.0, 6.0), p(10.0, 6.0)]];
    assert_eq!(approach(&satin("c", 0.0, &RED, &[]), &DesignSettings::default(), &Budget::DEFAULT), Some(Approach::Shape(rails)));
    // Its rails as they are sewn: swapped, or turned.
    let swapped = vec![vec![p(0.0, 6.0), p(10.0, 6.0)], vec![p(0.0, 0.0), p(10.0, 0.0)]];
    assert_eq!(
        approach(&satin("c", 0.0, &RED, &[("swap_satin_rails", "true")]), &DesignSettings::default(), &Budget::DEFAULT),
        Some(Approach::Shape(swapped))
    );
    let turned = vec![vec![p(10.0, 0.0), p(0.0, 0.0)], vec![p(0.0, 6.0), p(10.0, 6.0)]];
    assert_eq!(
        approach(&satin("c", 0.0, &RED, &[("reverse_rails", "first")]), &DesignSettings::default(), &Budget::DEFAULT),
        Some(Approach::Shape(turned))
    );
    // Not starting at its nearest point, a column offers its first rail's start, after the swap and the
    // reversal.
    let first = |params: &[(&str, &str)]| {
        approach(&satin("c", 0.0, &RED, &[&[("start_at_nearest_point", "false")], params].concat()), &DesignSettings::default(), &Budget::DEFAULT)
    };
    assert_eq!(first(&[]), Some(Approach::Point(p(0.0, 0.0))));
    assert_eq!(first(&[("swap_satin_rails", "true")]), Some(Approach::Point(p(0.0, 6.0))));
    assert_eq!(first(&[("reverse_rails", "first")]), Some(Approach::Point(p(10.0, 0.0))));
    // A tatami fill offers its area's rings, the drawing's first point first; a fill of a method not sewn
    // yet offers nothing.
    let ring = vec![p(2.0, 3.0), p(2.0, 13.0), p(12.0, 13.0), p(12.0, 3.0), p(2.0, 3.0)];
    assert_eq!(approach(&as_fill(stroke.clone()), &DesignSettings::default(), &Budget::DEFAULT), Some(Approach::Shape(vec![ring])));
    let contour = Element { params: [("fill_method", "contour_fill")].into_iter().collect(), ..as_fill(stroke) };
    assert_eq!(approach(&contour, &DesignSettings::default(), &Budget::DEFAULT), None);
    // A path that is no satin, unreadable settings and a spent budget offer nothing.
    // A column of one point, as narrow as a stroke that sets no width, is a stroke and offers that point
    // (`REQ-SAT-015`). 3 mm wide it is a column with no rails.
    let no_rails = along("n", polylines(&[&[(0.0, 0.0)]]), &RED, &[("satin_column", "true")]);
    assert_eq!(approach(&no_rails, &DesignSettings::default(), &Budget::DEFAULT), Some(Approach::Point(p(0.0, 0.0))));
    assert_eq!(approach(&widened(no_rails, 3.0), &DesignSettings::default(), &Budget::DEFAULT), None);
    let unreadable = along("u", polylines(&[&[(0.0, 0.0), (1.0, 0.0)]]), &RED, &[("satin_column", "maybe")]);
    assert_eq!(approach(&unreadable, &DesignSettings::default(), &Budget::DEFAULT), None);
    assert_eq!(approach(&satin("c", 0.0, &RED, &[]), &DesignSettings::default(), &Budget { max_stitches: 1, max_work: 1 }), None);
}
