//! Cases of the runs under what is sewn later (`REQ-AUTO-053`), on stitches laid by hand: blocks of rows
//! 0.25 mm apart, as a fill's rows cover its area, so that each case says exactly what covers what.

use stitchcraft_params::{ParamSet, StitchType};
use stitchcraft_plan::Thread;

use super::*;
use crate::common::CommonParams;
use crate::design::{FillRule, Hints, Path, Shape};

const RED: Rgb = Rgb::new(200, 30, 30);
const BLUE: Rgb = Rgb::new(30, 30, 200);

fn p(x: f64, y: f64) -> Point {
    Point::new(x, y).unwrap()
}

/// Rows 0.25 mm apart that cover the box from `(left, top)` to `(right, bottom)`, back and forth.
fn block(left: f64, top: f64, right: f64, bottom: f64) -> Vec<Point> {
    let (mut rows, mut y, mut forth) = (Vec::new(), top, true);
    while y <= bottom {
        rows.extend(if forth { [p(left, y), p(right, y)] } else { [p(right, y), p(left, y)] });
        (y, forth) = (y + 0.25, !forth);
    }
    rows
}

/// An element `id` in a thread of `colour`, and what a generator of `stitch_type` made of it: `groups`.
fn sewn(id: &str, colour: Rgb, stitch_type: StitchType, groups: Vec<Vec<Point>>) -> (Element, Generated) {
    let element = Element {
        id: ElementId::new(id).unwrap(),
        name: None,
        shape: Shape::Fill { path: Path { subpaths: Vec::new() }, rule: FillRule::NonZero, gradient: None },
        thread: Thread::new(colour),
        params: ParamSet::new(),
        automatic: false,
        hints: Hints::default(),
        clips: Vec::new(),
    };
    let common = CommonParams::from_set(&ParamSet::new()).unwrap().params;
    (element, Generated { common, stitch_type, groups, colours: Vec::new(), min_stitch: Mm::from_tenths(5), layer: Layer::Whole })
}

fn fill(id: &str, colour: Rgb, groups: Vec<Vec<Point>>) -> (Element, Generated) {
    sewn(id, colour, StitchType::TatamiFill, groups)
}

/// The ids of every element of `sewn`, as the elements that digitizing made.
fn all(sewn: &[(Element, Generated)]) -> BTreeSet<ElementId> {
    sewn.iter().map(|(element, _)| element.id.clone()).collect()
}

fn borrowed(sewn: &[(Element, Generated)]) -> Vec<(&Element, Generated)> {
    sewn.iter().map(|(element, generated)| (element, generated.clone())).collect()
}

/// The run `routing` gives for the move of `sewn` to its `place`th element, each of one group: from the last
/// point of the one before it to its first point.
fn run_to(routing: &mut Routing, sewn: &[(Element, Generated)], place: usize, sews_on: bool) -> Option<Vec<Point>> {
    let (before, next) = (&sewn[place - 1], &sewn[place]);
    let ends = [*before.1.groups[0].last().unwrap(), next.1.groups[0][0]];
    routing.run(place, [&before.0, &next.0], ends, next.0.thread.color, sews_on)
}

/// The run in place of the move from `sewn`'s element before `place` to the one at it, with the default
/// budget.
fn run(sewn: &[(Element, Generated)], place: usize, sews_on: bool) -> Option<Vec<Point>> {
    let made = all(sewn);
    let mut routing = Routing::of(&borrowed(sewn), &made, &Budget::DEFAULT).unwrap();
    let run = run_to(&mut routing, sewn, place, sews_on);
    assert_eq!(routing.said(&Budget::DEFAULT), None);
    run
}

/// 2 red squares 16 mm apart, the first ending at (4, 2) and the second starting at (20, 2), and a blue bar
/// from `left` on that reaches 1 mm under the second, as `bar` sews it.
fn squares_and_bar(left: f64, bar: StitchType) -> Vec<(Element, Generated)> {
    let mut first = block(0.0, 0.0, 4.0, 4.0);
    first.push(p(4.0, 2.0));
    let mut second = vec![p(20.0, 2.0)];
    second.extend(block(20.0, 0.0, 24.0, 4.0));
    vec![fill("a", RED, vec![first]), fill("b", RED, vec![second]), sewn("c", BLUE, bar, vec![block(left, 0.0, 21.0, 4.0)])]
}

/// The 6 needle points of 7 equal stitches straight from (4, 2) to (20, 2).
fn straight() -> Vec<Point> {
    (1..=6).map(|k| p(4.0, 2.0).lerp(p(20.0, 2.0), f64::from(k) / 7.0)).collect()
}

#[test]
fn req_auto_053_a_jump_runs_straight_under_a_shape_sewn_later_in_stitches_of_2_5_mm() {
    // 16 mm in 7 equal stitches, none longer than 2.5 mm: 6 needle points between the ends.
    assert_eq!(run(&squares_and_bar(3.0, StitchType::TatamiFill), 1, false), Some(straight()));
    assert_eq!((STITCH.get(), longest(16.0), longest(0.0)), (2.5, 42.0, 10.0));
    // A satin column covers as a fill does.
    assert_eq!(run(&squares_and_bar(3.0, StitchType::SatinColumn), 1, false), Some(straight()));
}

#[test]
fn req_auto_053_only_a_fill_or_satin_column_sewn_later_covers_a_run_of_another_colour() {
    // A running stitch is a thread wide and covers nothing.
    assert_eq!(run(&squares_and_bar(3.0, StitchType::RunningStitch), 1, false), None);
    // The bar sewn before the second square covers nothing of the move to it.
    let mut sewn = squares_and_bar(3.0, StitchType::TatamiFill);
    sewn.rotate_right(1);
    assert_eq!(run(&sewn, 2, false), None);
    // An underlay and the base layer cover nothing.
    for layer in [Layer::Underlay, Layer::Base] {
        let mut sewn = squares_and_bar(3.0, StitchType::TatamiFill);
        sewn[2].1.layer = layer;
        assert_eq!(run(&sewn, 1, false), None, "{layer:?}");
    }
    // The top layer of an element sewn as 2 layers covers.
    let mut sewn = squares_and_bar(3.0, StitchType::TatamiFill);
    sewn[2].1.layer = Layer::Top;
    assert_eq!(run(&sewn, 1, false), Some(straight()));
    // Rows 1 mm apart leave fabric between them: a sparse fill covers nothing.
    let mut sewn = squares_and_bar(3.0, StitchType::TatamiFill);
    sewn[2].1.groups = vec![(0..=4).flat_map(|row| [p(3.0, f64::from(row)), p(21.0, f64::from(row))]).collect()];
    assert_eq!(run(&sewn, 1, false), None);
}

#[test]
fn req_auto_053_a_run_lies_under_a_line_of_its_own_colour_sewn_later() {
    // A red line from the first square to the second, sewn after both: the run lies where the line will.
    let line_at = |colour: Rgb, y: f64| sewn("line", colour, StitchType::RunningStitch, vec![vec![p(3.0, y), p(21.0, y)]]);
    let line = |colour: Rgb| line_at(colour, 2.0);
    let mut sewn = squares_and_bar(3.0, StitchType::TatamiFill);
    sewn[2] = line(RED);
    assert_eq!(run(&sewn, 1, false), Some(straight()));
    // A line 0.4 mm to one side of both ends: the run steps onto it and off it, however short the steps, and
    // every needle point between lies within the line's thread.
    sewn[2] = line_at(RED, 2.4);
    let beside = run(&sewn, 1, false).unwrap();
    assert!(beside.len() >= 8 && beside.iter().all(|point| (point.y() - 2.4).abs() < 0.2), "{beside:?}");
    let ends = [p(4.0, 2.0).distance(beside[0]), beside[beside.len() - 1].distance(p(20.0, 2.0))];
    assert!(ends.iter().all(|&step| step < 0.5), "{ends:?}");
    // Under a line of another colour the run would show beside it, and on a line sewn before it would lie
    // on top.
    sewn[2] = line(BLUE);
    assert_eq!(run(&sewn, 1, false), None);
    sewn[2] = line(RED);
    sewn.rotate_right(1);
    assert_eq!(run(&sewn, 2, false), None);
}

#[test]
fn req_auto_053_only_moves_between_elements_that_digitizing_made_are_looked_at() {
    let sewn = squares_and_bar(3.0, StitchType::TatamiFill);
    let generated = borrowed(&sewn);
    // An element with settings on either side of the move keeps it as it is.
    for with_settings in ["a", "b"] {
        let made: BTreeSet<ElementId> = all(&sewn).into_iter().filter(|id| id.as_str() != with_settings).collect();
        let mut routing = Routing::of(&generated, &made, &Budget::DEFAULT).unwrap();
        assert_eq!(run_to(&mut routing, &sewn, 1, false), None, "{with_settings}");
    }
    // A design in which digitizing made nothing has no moves to look at.
    assert!(Routing::of(&generated, &BTreeSet::new(), &Budget::DEFAULT).is_none());
    // A move of no length is none.
    let made = all(&sewn);
    let mut routing = Routing::of(&generated, &made, &Budget::DEFAULT).unwrap();
    let end = p(4.0, 2.0);
    assert_eq!(routing.run(1, [&sewn[0].0, &sewn[1].0], [end, end], RED, false), None);
}

#[test]
fn req_auto_053_a_run_leaves_the_hidden_cells_only_within_0_8_mm_of_its_ends() {
    // The first square ends on its own stitches, short of where the bar hides the run: the first hidden cell
    // has its centre at 4.3 mm, 2 cells inside the bar's thread. The cell before it, at 4.1 mm, lies 0.65 mm
    // from an end at 3.45 mm, and 0.85 mm from one at 3.25 mm.
    let stub = |end: f64, left: f64| {
        let mut sewn = squares_and_bar(left, StitchType::TatamiFill);
        sewn[0].1.groups[0].push(p(end, 2.0));
        run(&sewn, 1, false)
    };
    assert!(stub(3.45, 4.0).is_some());
    assert_eq!(stub(3.25, 4.0), None);
    assert_eq!((STUB, ROOM), (0.8, 2));
    // Where the bar starts 1 mm past the square, the fabric between shows: no run, however near its end.
    assert_eq!(stub(4.0, 5.2), None);
}

/// 2 red squares 4 mm apart, a block of `between` sewn first between them, and a blue bar sewn last that
/// goes round the block: down from the first square to `depth`, across, and up to the second.
fn squares_and_way_round(depth: f64, between: Rgb) -> Vec<(Element, Generated)> {
    let mut first = block(0.0, 0.0, 4.0, 4.0);
    first.push(p(4.0, 2.0));
    let mut second = vec![p(8.0, 2.0)];
    second.extend(block(8.0, 0.0, 12.0, 4.0));
    let round = vec![block(3.0, 1.0, 5.0, depth + 1.0), block(3.0, depth - 1.0, 9.0, depth + 1.0), block(7.0, 1.0, 9.0, depth + 1.0)];
    vec![fill("under", between, vec![block(5.4, 0.0, 6.6, 4.0)]), fill("a", RED, vec![first]), fill("b", RED, vec![second]), fill("c", BLUE, round)]
}

const GREEN: Rgb = Rgb::new(30, 200, 30);

#[test]
fn req_auto_053_a_run_goes_round_what_would_show_it_and_no_farther_than_twice_the_move_and_10_mm() {
    let (from, to) = (p(4.0, 2.0), p(8.0, 2.0));
    let run_round = run(&squares_and_way_round(8.0, GREEN), 2, false).unwrap();
    let points: Vec<Point> = std::iter::once(from).chain(run_round.iter().copied()).chain([to]).collect();
    let stitches: Vec<f64> = points.windows(2).map(|pair| pair[0].distance(pair[1])).collect();
    let length: f64 = stitches.iter().sum();
    // Down 5 mm, across 4 mm and up 5 mm, cut across the corners as far as the bar hides it.
    assert!((12.0..16.0).contains(&length) && length <= longest(4.0), "{length}");
    assert!(stitches.iter().all(|&stitch| stitch > 0.0 && stitch <= STITCH.get() + 1e-9), "{stitches:?}");
    // No needle point lies on the block between the squares, or beside the bar.
    let within = |point: &Point| (3.0..=5.0).contains(&point.x()) || (7.0..=9.0).contains(&point.x()) || (7.0..=9.0).contains(&point.y());
    assert!(run_round.iter().all(within), "{run_round:?}");
    // The same design gives the same run.
    assert_eq!(run(&squares_and_way_round(8.0, GREEN), 2, false), Some(run_round.clone()));
    // Round a block 3 mm deeper the run would be longer than twice the move and 10 mm: the jump stays.
    assert_eq!(run(&squares_and_way_round(11.0, GREEN), 2, false), None);
    assert_eq!((longest(4.0), WINDOW), (18.0, 10.0));
    // The block of the run's own colour, sewn before, is no way either: the run would lie on top of it.
    assert_eq!(run(&squares_and_way_round(8.0, RED), 2, false), Some(run_round));
}

#[test]
fn req_auto_053_a_move_sewn_on_runs_under_only_where_its_stitch_would_show_for_1_mm() {
    // The move is within the collapse length. Over the green block its stitch shows, so it goes round.
    assert!(run(&squares_and_way_round(8.0, GREEN), 2, true).is_some());
    // Over a block of its own colour it shows as its shape does, and is sewn on as it was.
    assert_eq!(run(&squares_and_way_round(8.0, RED), 2, true), None);
    // 2 red squares with bare fabric between them at the height of the move, and a blue bar sewn later under
    // their lower halves: a way under, round the fabric. The first square's thread covers to 4.2 mm, and the
    // second square's from 0.2 mm before `left`.
    let squares = |left: f64| {
        let mut first = block(0.0, 0.0, 4.0, 4.0);
        first.push(p(4.0, 2.0));
        let mut second = vec![p(6.0, 2.0)];
        second.extend(block(left, 0.0, 10.0, 4.0));
        vec![fill("a", RED, vec![first]), fill("b", RED, vec![second]), fill("c", BLUE, vec![block(3.0, 2.4, 7.0, 6.0)])]
    };
    // 1 mm of fabric shows: a run, under the bar.
    let under = run(&squares(5.4), 1, true).unwrap();
    assert!(!under.is_empty() && under.iter().all(|point| point.y() > 2.2), "{under:?}");
    assert_eq!(SHOWS, 1.0);
    // 0.8 mm shows: the stitch stays, though the same way is there for a jump.
    assert_eq!(run(&squares(5.2), 1, true), None);
    assert!(run(&squares(5.2), 1, false).is_some());
}

#[test]
fn diag_sc_i0914_moves_that_need_more_work_than_the_budget_are_left_as_they_were_and_named() {
    let sewn = squares_and_bar(3.0, StitchType::TatamiFill);
    let (generated, made) = (borrowed(&sewn), all(&sewn));
    let with = |work: u64| {
        let budget = Budget { max_work: work, ..Budget::DEFAULT };
        let mut routing = Routing::of(&generated, &made, &budget).unwrap();
        let run = run_to(&mut routing, &sewn, 1, false);
        (run, routing.said(&budget), routing.meter.work_left())
    };
    // The work the design needs: its grids and stitches painted, the search's box, each cell looked at, and
    // the line pulled straight.
    let (run, said, left) = with(Budget::DEFAULT.max_work);
    let needed = Budget::DEFAULT.max_work - left;
    assert_eq!((run, said), (Some(straight()), None));
    let mut meter = Budget::DEFAULT.meter();
    let cover = Cover::painted(&generated, &mut meter).unwrap();
    let painting = Budget::DEFAULT.max_work - meter.work_left();
    let grids = u64::try_from(cover.later.len()).unwrap() * CELL_WORK;
    // The grids hold the stitches with 1 mm to spare on every side, in cells of 0.2 mm.
    assert_eq!((cover.columns, cover.rows, cover.later.len(), cover.top.len()), (130, 30, 3900, 3900));
    assert_eq!(CELL_WORK, 100);
    // The box searched is that of the ends' cells with 10 mm to spare: 181 cells by 101.
    let searching = 181 * 101;
    // With all it needs the move runs, and with a unit less at any step it stays a jump, which is said.
    assert_eq!(with(needed), (Some(straight()), None, 0));
    for (short, named) in [
        (0, "a"),
        (grids - 1, "a"),
        (grids, "a"),
        (painting - 1, "a"),
        (painting, "b"),
        (painting + searching - 1, "b"),
        (painting + searching, "b"),
        (needed - 1, "b"),
    ] {
        let (run, said, _) = with(short);
        let said = said.unwrap();
        assert_eq!((run, said.code, said.element.as_ref().map(ElementId::as_str)), (None, Code::AutoRunsLeft, Some(named)), "{short}");
        assert!(said.message.contains(&format!("work budget of {short} units")) && said.message.contains(&format!("`{named}`")), "{}", said.message);
    }
    // Once the work has run out, no later move is looked at.
    let budget = Budget { max_work: painting, ..Budget::DEFAULT };
    let mut routing = Routing::of(&generated, &made, &budget).unwrap();
    assert_eq!(run_to(&mut routing, &sewn, 1, false), None);
    routing.meter = Budget::DEFAULT.meter();
    assert_eq!(run_to(&mut routing, &sewn, 1, false), None);
}

#[test]
fn a_design_without_a_stitch_has_grids_of_no_cells() {
    let sewn = vec![fill("a", RED, vec![Vec::new()])];
    let cover = Cover::painted(&borrowed(&sewn), &mut Budget::DEFAULT.meter()).unwrap();
    assert_eq!((cover.columns, cover.rows, cover.later.len()), (0, 0, 0));
    assert_eq!(cover.at(&cover.later, (0, 0)), 0);
}
