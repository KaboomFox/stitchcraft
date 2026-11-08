//! Guided ripples, grids and clips (milestone M10.11, `REQ-RIP-012` to `REQ-RIP-018`), through the
//! ripple stitch's generator in the engine's public API: what each sews, needle point by needle point. Where a
//! guide places its copies is in `src/generators/ripple/guided/tests.rs`, and the ripple in a plan in
//! `tests/ripple.rs`.

// Test code may unwrap, index and panic (clippy.toml allows the first 2 in tests).
#![allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::panic)]

use stitchcraft_core::rng::SplitMix64;
use stitchcraft_core::{Budget, Mm, Point};
use stitchcraft_engine::design::{Clip, FillRule, Guide, Join, Path};
use stitchcraft_engine::generators::passes::RepeatParams;
use stitchcraft_engine::generators::ripple::{Ripple, RippleGuide, RippleParams, guide, ripple_stitch, unapplied};
use stitchcraft_engine::generators::running::RunningParams;
use stitchcraft_engine::generators::satin::SatinParams;
use stitchcraft_params::{ParamSet, StitchType};
use stitchcraft_testkit::designs::{p, polylines};

/// A ripple set as `with` says, for a machine whose shortest stitch is 0.3 mm and whose jumps start at 3 mm,
/// guided by `guide`, its anchor line `anchor`, and cut to `clips`, its rails read as the engine reads them.
fn ripple(with: &[(&str, &str)], guide: Option<RippleGuide>, anchor: Option<[Point; 2]>, clips: Vec<Clip>) -> Ripple {
    let set: ParamSet = with.iter().copied().collect();
    let own = RippleParams::from_set(&set).unwrap().params;
    let satin = SatinParams::from_set_as(&set, StitchType::RippleStitch).unwrap().params;
    Ripple {
        satin: SatinParams { reverse_rails: own.reverse_rails, swap_satin_rails: own.swap_satin_rails, ..satin },
        ripple: own,
        running: RunningParams::from_set(&set).unwrap().params,
        passes: RepeatParams::from_set(&set).unwrap().params,
        min_stitch: Mm::new(0.3).unwrap(),
        jump: Mm::new(3.0).unwrap(),
        target: None,
        guide,
        anchor,
        unapplied: unapplied(&set),
        clips,
    }
}

/// The runs `path` sews as `ripple` says, each needle point to a thousandth of a millimetre, and what it says.
fn sewn(path: &Path, ripple: &Ripple) -> (Vec<Vec<(f64, f64)>>, Vec<String>) {
    let stitched = ripple_stitch(path, ripple, &SplitMix64::new(7), &mut Budget::DEFAULT.meter()).unwrap();
    (stitched.runs.iter().map(|run| rounded(run)).collect(), stitched.warnings.iter().map(ToString::to_string).collect())
}

/// The runs `path` sews as `ripple` says, when it says nothing.
fn runs(path: &Path, ripple: &Ripple) -> Vec<Vec<(f64, f64)>> {
    let (runs, said) = sewn(path, ripple);
    assert_eq!(said, Vec::<String>::new());
    runs
}

/// `points` to a thousandth of a millimetre.
fn rounded(points: &[Point]) -> Vec<(f64, f64)> {
    let round = |v: f64| (v * 1000.0).round() / 1000.0 + 0.0;
    points.iter().map(|q| (round(q.x()), round(q.y()))).collect()
}

/// A guide line through `points`.
fn guide_line(points: &[(f64, f64)]) -> Option<RippleGuide> {
    Some(RippleGuide::Line { path: polylines(&[points]), nodes: false })
}

/// The guide the engine reads from a guide drawn as `parts`, set as `with` says, for a ripple set as `own`.
fn read_guide(parts: &[&[(f64, f64)]], with: &[(&str, &str)], own: &[(&str, &str)]) -> Option<RippleGuide> {
    let drawn = Guide { path: polylines(parts), width: Mm::ZERO, join: Join::UNSET, params: with.iter().copied().collect() };
    let mut said = Vec::new();
    let read = guide(&drawn, &RippleParams::from_set(&own.iter().copied().collect()).unwrap().params, &mut said);
    assert_eq!(said, Vec::new());
    read
}

/// The clip of the rectangle from `(x0, y0)` to `(x1, y1)`.
fn rectangle(x0: f64, y0: f64, x1: f64, y1: f64) -> Clip {
    Clip::of(polylines(&[&[(x0, y0), (x1, y0), (x1, y1), (x0, y1), (x0, y0)]]), FillRule::NonZero)
}

/// A line 4 mm long along x, centred on the origin.
fn short() -> Path {
    polylines(&[&[(-2.0, 0.0), (2.0, 0.0)]])
}

/// A column 20 mm long along x and 4 mm wide, from x = −10, with a rung across its middle.
fn column() -> Path {
    polylines(&[&[(-10.0, 0.0), (10.0, 0.0)], &[(-10.0, 4.0), (10.0, 4.0)], &[(0.0, -0.5), (0.0, 4.5)]])
}

/// The points `(x, y)` for each `y` of `ys`.
fn down(x: f64, ys: &[f64]) -> Vec<(f64, f64)> {
    ys.iter().map(|&y| (x, y)).collect()
}

/// The points `(x, y)` for each `x` of `xs`.
fn across(y: f64, xs: &[f64]) -> Vec<(f64, f64)> {
    xs.iter().map(|&x| (x, y)).collect()
}

#[test]
fn req_rip_012_copies_follow_a_guide_line_back_and_forth() {
    // The line's 3 needle points, copied 5 mm apart down the guide, each joined to the next by a stitch.
    let guided = ripple(&[("line_count", "3"), ("scale_end", "100")], guide_line(&[(0.0, 0.0), (0.0, 10.0)]), None, Vec::new());
    let run = [across(0.0, &[-2.0, 0.0, 2.0]), vec![(2.0, 2.5)], across(5.0, &[2.0, 0.0, -2.0]), vec![(-2.0, 7.5)], across(10.0, &[-2.0, 0.0, 2.0])];
    assert_eq!(runs(&short(), &guided), [run.concat()]);
}

#[test]
fn req_rip_012_a_guide_that_can_t_place_copies_is_left_out_and_the_ripple_sewn_without_it() {
    let unguided = sewn(&short(), &ripple(&[], None, None, Vec::new()));
    let (sewn_runs, said) = sewn(&short(), &ripple(&[], guide_line(&[(1.0, 1.0), (1.0, 1.0)]), None, Vec::new()));
    assert_eq!(sewn_runs, unguided.0);
    assert_eq!(said, ["warning SC-W0409: This ripple stitch's guide can't be used: its guide line has no length. It is sewn without it."]);
}

#[test]
fn req_rip_012_more_places_than_1000_along_a_guide_sew_1000_copies() {
    let points: Vec<(f64, f64)> = (0..1001).map(|k| (0.0, f64::from(k) * 0.1)).collect();
    let at_points = ripple(&[("satin_guide_pattern_position", "render_at_rungs"), ("scale_end", "100")], guide_line(&points), None, Vec::new());
    let (sewn_runs, said) = sewn(&short(), &at_points);
    assert_eq!(said, ["warning SC-W0407: This ripple stitch's guide has 1001 places for copies, more than 1000, so it sews 1000."]);
    // The 1000th copy, at the guide's point 999, sewn from its end.
    assert_eq!(sewn_runs[0][sewn_runs[0].len() - 3..], [(2.0, 99.9), (0.0, 99.9), (-2.0, 99.9)]);
}

#[test]
fn req_rip_013_a_satin_guide_lays_the_copies_across_its_rails() {
    // Rails 4 mm apart along x: the line, 2 mm long, is turned across them and doubled, at x = 0, 10 and 20.
    let rails: [&[(f64, f64)]; 2] = [&[(0.0, 0.0), (20.0, 0.0)], &[(0.0, 4.0), (20.0, 4.0)]];
    let satin = read_guide(&rails, &[], &[]);
    assert!(matches!(satin, Some(RippleGuide::Satin { .. })), "{satin:?}");
    let line = polylines(&[&[(0.0, 0.0), (2.0, 0.0)]]);
    let run = [
        down(0.0, &[0.0, 2.0, 4.0]),
        across(4.0, &[2.5, 5.0, 7.5]),
        down(10.0, &[4.0, 2.0, 0.0]),
        across(0.0, &[12.5, 15.0, 17.5]),
        down(20.0, &[0.0, 2.0, 4.0]),
    ];
    assert_eq!(runs(&line, &ripple(&[("line_count", "3")], satin, None, Vec::new())), [run.concat()]);
    // The ripple's swap_satin_rails reads the guide from its second rail: each copy runs from y = 4 to y = 0.
    let swapped = read_guide(&rails, &[], &[("swap_satin_rails", "true")]);
    let first = runs(&line, &ripple(&[("line_count", "3"), ("swap_satin_rails", "true")], swapped, None, Vec::new()));
    assert_eq!(first[0][..3], down(0.0, &[4.0, 2.0, 0.0]));
}

#[test]
fn req_rip_014_an_anchor_line_sets_where_the_copies_lie_across_a_satin_guide() {
    // The anchor crosses the line at its middle, from (1, −1) to (1, 1): each copy lies along the rails, 4 mm
    // long, half way across.
    let satin = read_guide(&[&[(0.0, 0.0), (20.0, 0.0)], &[(0.0, 4.0), (20.0, 4.0)]], &[], &[]);
    let line = polylines(&[&[(0.0, 0.0), (2.0, 0.0)]]);
    let anchored = ripple(&[("line_count", "3")], satin, Some([p(1.0, -1.0), p(1.0, 1.0)]), Vec::new());
    let run = [
        across(2.0, &[-2.0, 0.0, 2.0]),
        across(2.0, &[4.5, 7.0, 9.5]),
        across(2.0, &[12.0, 10.0, 8.0]),
        across(2.0, &[10.5, 13.0, 15.5]),
        across(2.0, &[18.0, 20.0, 22.0]),
    ];
    assert_eq!(runs(&line, &anchored), [run.concat()]);
}

#[test]
fn req_rip_015_a_grid_crosses_the_copies_through_their_needle_points() {
    // The line's needle points 2 mm apart, copied at y = 0, 5 and 10. The grid starts at the copies' corner
    // nearest the ripple's end, (2, 10), and runs down and up the lines through x = 2, 0 and −2.
    let with = [("line_count", "3"), ("scale_end", "100"), ("grid_size_mm", "2")];
    let gridded = ripple(&with, guide_line(&[(0.0, 0.0), (0.0, 10.0)]), None, Vec::new());
    let ripple_run =
        [across(0.0, &[-2.0, 0.0, 2.0]), vec![(2.0, 2.5)], across(5.0, &[2.0, 0.0, -2.0]), vec![(-2.0, 7.5)], across(10.0, &[-2.0, 0.0, 2.0])];
    let steps = [0.0, 2.5, 5.0, 7.5, 10.0];
    let back: Vec<f64> = steps.iter().rev().copied().collect();
    let grid_run = [down(2.0, &back), down(0.0, &steps), down(-2.0, &back)];
    let sewn_runs = runs(&short(), &gridded);
    assert_eq!(sewn_runs, [ripple_run.concat(), grid_run.concat()]);
    // With grid_first, the grid first, and everything backwards.
    let first =
        runs(&short(), &ripple(&[with.as_slice(), &[("grid_first", "true")]].concat(), guide_line(&[(0.0, 0.0), (0.0, 10.0)]), None, Vec::new()));
    let backwards: Vec<Vec<(f64, f64)>> = sewn_runs.iter().rev().map(|run| run.iter().rev().copied().collect()).collect();
    assert_eq!(first, backwards);
}

#[test]
fn req_rip_015_a_column_s_grid_runs_through_its_pairs_a_grid_size_apart() {
    // 3 lines along the column, at y = 0, 2 and 4, the last ending at (10, 4). The grid's lines cross them at
    // its pairs, 5 mm apart, from x = 10 back to x = −10.
    let gridded = runs(&column(), &ripple(&[("line_count", "3"), ("grid_size_mm", "5")], None, None, Vec::new()));
    assert_eq!(gridded.len(), 2);
    let grid = &gridded[1];
    assert_eq!((grid.first(), grid.last()), (Some(&(10.0, 4.0)), Some(&(-10.0, 0.0))));
    let xs: Vec<f64> = grid.iter().filter(|(_, y)| *y == 2.0).map(|(x, _)| *x).collect();
    assert_eq!(xs, [10.0, 5.0, 0.0, -5.0, -10.0]);
    // Without flip_copies the lines bend into each other along the column, and so do the lines the grid
    // crosses. Every other grid line runs the other way, and they are all sewn as they run, joined flat: from
    // (10, 4), the ripple's end, down, then up the next line, as a zigzag.
    let one_way = runs(&column(), &ripple(&[("line_count", "3"), ("grid_size_mm", "5"), ("flip_copies", "false")], None, None, Vec::new()));
    let zigzag = [
        down(10.0, &[4.0, 2.0]),
        vec![(8.333, 1.833), (6.667, 1.667)],
        down(5.0, &[1.5, 3.5]),
        vec![(3.333, 3.333), (1.667, 3.167)],
        down(0.0, &[3.0, 1.0]),
        vec![(-1.667, 0.833), (-3.333, 0.667)],
        down(-5.0, &[0.5, 2.5]),
        vec![(-6.667, 2.333), (-8.333, 2.167)],
        down(-10.0, &[2.0, 0.0]),
    ];
    assert_eq!((one_way.len(), one_way[0].last()), (2, Some(&(10.0, 4.0))));
    assert_eq!(one_way[1], zigzag.concat());
}

#[test]
fn req_rip_015_a_grid_sews_its_lines_from_its_first_whatever_the_ripple_left_out() {
    // 5 copies, 2.5 mm apart, joined at points: 4 lines that bend from one copy into the next, the first left
    // out. The grid's lines run through the 3 left, from the corner nearest the ripple's end, (−2, 10). With
    // the point join and flip_copies, the second is sewn from its end, as the ripple's skips don't count.
    let with = [("line_count", "4"), ("scale_end", "100"), ("grid_size_mm", "2"), ("join_style", "1"), ("skip_start", "1")];
    let pointed = runs(&short(), &ripple(&with, guide_line(&[(0.0, 0.0), (0.0, 10.0)]), None, Vec::new()));
    let lines = [(2.0, 2.5), (0.0, 3.75), (-2.0, 5.0), (0.0, 6.25), (2.0, 7.5), (0.0, 8.75), (-2.0, 10.0)];
    let grid = [down(-2.0, &[10.0, 7.5, 5.0]), down(0.0, &[3.75, 6.25, 8.75]), down(2.0, &[7.5, 5.0, 2.5])];
    assert_eq!(pointed, [lines.to_vec(), grid.concat()]);
}

#[test]
fn req_rip_016_manual_placement_sews_the_line_s_nodes_through_each_copy() {
    // The line's nodes, 4 mm apart, copied at y = 0 and y = 10: the first from its end, then 4 stitches up.
    let line = polylines(&[&[(-4.0, 0.0), (0.0, 0.0), (4.0, 0.0)]]);
    let by_hand = ripple(
        &[("line_count", "2"), ("scale_end", "100"), ("manual_pattern_placement", "true")],
        guide_line(&[(0.0, 0.0), (0.0, 10.0)]),
        None,
        Vec::new(),
    );
    let run = [across(0.0, &[4.0, 0.0, -4.0]), down(-4.0, &[2.5, 5.0, 7.5]), across(10.0, &[-4.0, 0.0, 4.0])];
    assert_eq!(runs(&line, &by_hand), [run.concat()]);
}

#[test]
fn req_rip_017_a_ripple_cut_to_a_clip_sews_its_pieces_inside_joined_along_the_clip() {
    // The column's 3 lines cut at x = ±5: from (−5, 0) to (5, 4), along the clip's sides between them.
    let clipped = runs(&column(), &ripple(&[("line_count", "3")], None, None, vec![rectangle(-5.0, -5.0, 5.0, 5.0)]));
    assert_eq!(clipped.len(), 1);
    let run = &clipped[0];
    assert_eq!((run.first(), run.last()), (Some(&(-5.0, 0.0)), Some(&(5.0, 4.0))));
    assert!(run.iter().all(|&(x, y)| (-5.0..=5.0).contains(&x) && (0.0..=4.0).contains(&y)), "{run:?}");
    // Repeated, each run is sewn there and back again, from where it ended.
    let twice = runs(&column(), &ripple(&[("line_count", "3"), ("repeats", "2")], None, None, vec![rectangle(-5.0, -5.0, 5.0, 5.0)]));
    let back: Vec<(f64, f64)> = run.iter().rev().skip(1).copied().collect();
    assert_eq!(twice, [[run.clone(), back].concat()]);
}

#[test]
fn req_rip_017_each_part_of_the_clips_is_a_run_the_part_of_the_last_piece_last() {
    // 2 lines, the second ending at x = −10: the left part, where the ripple ends, is sewn last.
    let squares = polylines(&[
        &[(-8.0, -5.0), (-2.0, -5.0), (-2.0, 5.0), (-8.0, 5.0), (-8.0, -5.0)],
        &[(2.0, -5.0), (8.0, -5.0), (8.0, 5.0), (2.0, 5.0), (2.0, -5.0)],
    ]);
    let parts = vec![Clip::of(squares, FillRule::NonZero)];
    let both = runs(&column(), &ripple(&[("line_count", "2")], None, None, parts));
    assert_eq!(both.len(), 2);
    assert!(both[0].iter().all(|&(x, _)| x >= 2.0) && both[1].iter().all(|&(x, _)| x <= -2.0), "{both:?}");
    assert_eq!(both[1].last(), Some(&(-8.0, 4.0)));
}

#[test]
fn req_rip_017_a_grid_cut_to_clips_starts_where_the_ripple_ends() {
    // The grid's route starts from the ripple's end, (6, 4), and travels down the clip's side to where the
    // grid comes in, then sews its lines inside, at x = 5, 0 and −5.
    let clip = vec![rectangle(-6.0, -5.0, 6.0, 5.0)];
    let gridded = runs(&column(), &ripple(&[("line_count", "3"), ("grid_size_mm", "5")], None, None, clip));
    assert_eq!(gridded.len(), 2);
    assert_eq!((gridded[0].last(), gridded[1].first()), (Some(&(6.0, 4.0)), Some(&(6.0, 4.0))));
    let xs: Vec<f64> = gridded[1].iter().filter(|(_, y)| *y == 2.0).map(|(x, _)| *x).collect();
    assert_eq!(xs, [6.0, 5.0, 0.0, -5.0]);
}

#[test]
fn diag_sc_w0013_a_ripple_its_clips_leave_nothing_of_sews_nothing() {
    let (sewn_runs, said) = sewn(&column(), &ripple(&[], None, None, vec![rectangle(50.0, 50.0, 60.0, 60.0)]));
    assert_eq!(sewn_runs, Vec::<Vec<(f64, f64)>>::new());
    assert_eq!(said, ["warning SC-W0013: Its clip paths leave nothing of this ripple stitch, so it sews nothing."]);
}

#[test]
fn req_rip_018_a_column_s_rails_are_read_with_the_ripple_s_reverse_rails_and_swap_satin_rails() {
    let first = |with: &[(&str, &str)]| runs(&column(), &ripple(&[with, &[("line_count", "3")]].concat(), None, None, Vec::new()))[0][0];
    assert_eq!(first(&[]), (-10.0, 0.0));
    assert_eq!(first(&[("swap_satin_rails", "true")]), (-10.0, 4.0));
    assert_eq!(first(&[("reverse_rails", "both")]), (10.0, 0.0));
}
