use stitchcraft_core::{Budget, Mm};
use stitchcraft_params::ParamSet;

use super::*;
use crate::design::{Join, Segment, Subpath};
use crate::generators::satin::rails;

fn p(x: f64, y: f64) -> Point {
    Point::new(x, y).unwrap()
}

/// The column drawn with `lines`, its rails and rungs, sewn with `settings`, ready to cut.
fn cuttable(lines: &[&[(f64, f64)]], settings: &[(&str, &str)]) -> Cuttable {
    let meter = &mut Budget::DEFAULT.meter();
    let line = |points: &[(f64, f64)]| {
        let (first, rest) = points.split_first().unwrap();
        Subpath { start: p(first.0, first.1), segments: rest.iter().map(|&(x, y)| Segment::Line(p(x, y))).collect(), closed: false }
    };
    let path = Path { subpaths: lines.iter().map(|points| line(points)).collect() };
    let satin = rails(&path, Mm::SVG_PX, Join::UNSET, &mut Vec::new(), meter).unwrap().unwrap();
    let params = SatinParams::from_set(&settings.iter().copied().collect::<ParamSet>()).unwrap().params;
    Cuttable::of(&satin, &params, meter).unwrap()
}

/// A bar from x = 0 to 10 between y = 0 and y = 3, with a rung across it at each of `rungs`.
fn bar(rungs: &[f64], settings: &[(&str, &str)]) -> Cuttable {
    let rungs: Vec<[(f64, f64); 2]> = rungs.iter().map(|&x| [(x, -1.0), (x, 4.0)]).collect();
    let mut lines: Vec<&[(f64, f64)]> = vec![&[(0.0, 0.0), (10.0, 0.0)], &[(0.0, 3.0), (10.0, 3.0)]];
    lines.extend(rungs.iter().map(<[(f64, f64); 2]>::as_slice));
    cuttable(&lines, settings)
}

/// The points of each subpath of `path`.
fn drawn(path: &Path) -> Vec<Vec<(f64, f64)>> {
    let round = |p: Point| ((p.x() * 1e6).round() / 1e6, (p.y() * 1e6).round() / 1e6);
    path.subpaths.iter().map(|s| std::iter::once(s.start).chain(s.segments.iter().map(|g| g.end())).map(round).collect()).collect()
}

fn piece(cuttable: &Cuttable, from: f64, to: f64, back: bool) -> Option<Piece> {
    cuttable.piece([from; 2], [to; 2], back, [0.0; 2], &mut Budget::DEFAULT.meter()).unwrap()
}

#[test]
fn the_centre_line_runs_through_the_middles_of_the_cuts() {
    let bar = bar(&[5.0], &[("zigzag_spacing_mm", "1")]);
    let line = bar.line();
    assert_eq!(line.len(), bar.cuts.len());
    assert!(line.len() >= 10 && line.iter().all(|point| (point.y() - 1.5).abs() < 1e-12), "{line:?}");
    assert_eq!((line[0], line[line.len() - 1]), (p(0.0, 1.5), p(10.0, 1.5)));
    assert_eq!(bar.lengths(), [10.0, 10.0]);
}

#[test]
fn a_cut_is_the_cut_of_the_point_at_its_fraction_of_the_cuts_points_rounded() {
    let bar = bar(&[5.0], &[("zigzag_spacing_mm", "1")]);
    let n = bar.cuts.len();
    let points = 2.0 * n as f64;
    // The point at 1.5 rounds to 2, the second cut's first; at 0.4 to 0, the first's; at 2.6 to 3, the
    // second's second.
    assert_eq!([0.0, 0.4, 1.5, 2.6].map(|at| bar.cut_index(at / points)), [0, 0, 1, 1]);
    // The last point and past it, the last cut; before the first, the first.
    assert_eq!([1.0, 1.5, -0.5, -0.0].map(|fraction| bar.cut_index(fraction)), [n - 1, n - 1, 0, 0]);
    // Each rail is cut where it comes nearest its point of the cut.
    let meter = &mut Budget::DEFAULT.meter();
    let [a, b] = bar.cut_at(1.5 / points, meter).unwrap().unwrap();
    assert!(a > 0.0 && a == b && (a - bar.cuts[1][0].x()).abs() < 1e-12, "{a} {b}");
    let none = Cuttable { cuts: Vec::new(), ..bar };
    assert_eq!((none.cut_index(0.5), none.cut_at(0.5, meter).unwrap()), (0, None));
}

#[test]
fn a_piece_is_the_rails_between_its_cuts_with_the_rungs_on_them() {
    let bar = bar(&[2.0, 5.0, 8.0], &[]);
    // From 1 to 9: the 3 rungs, and no rung added, as 3 tell the rails from the rungs.
    let whole = piece(&bar, 1.0, 9.0, false).unwrap();
    assert!(whole.reads_back);
    let lines = drawn(&whole.path);
    assert_eq!(lines[..2], [vec![(1.0, 0.0), (9.0, 0.0)], vec![(1.0, 3.0), (9.0, 3.0)]]);
    assert_eq!(lines[2..].iter().map(|rung| rung[0].0).collect::<Vec<_>>(), [2.0, 5.0, 8.0]);
    // Back, the rails run from 9 to 1.
    let back = drawn(&piece(&bar, 1.0, 9.0, true).unwrap().path);
    assert_eq!(back[..2], [vec![(9.0, 0.0), (1.0, 0.0)], vec![(9.0, 3.0), (1.0, 3.0)]]);
    // A rung across a piece's end is the end's: from 0 to 5, the rung at 2 is the piece's only.
    let half = drawn(&piece(&bar, 0.0, 5.0, false).unwrap().path);
    assert_eq!(half[2..].iter().map(|rung| rung[0].0).collect::<Vec<_>>(), [2.0], "{half:?}");
    // From 3 to 7, only the rung at 5, which is enough.
    let middle = drawn(&piece(&bar, 3.0, 7.0, false).unwrap().path);
    assert_eq!(middle[2..].iter().map(|rung| rung[0].0).collect::<Vec<_>>(), [5.0]);
}

#[test]
fn a_piece_starting_at_a_rung_is_read_back_as_drawn() {
    // From the rung at 5 to the end, with the rung at 8: 2 rungs would not tell the rails from the rungs,
    // and a rung added at the start would lie on the rung at 5.
    let bar = bar(&[5.0, 8.0], &[]);
    let from_rung = piece(&bar, 5.0, 10.0, false).unwrap();
    assert!(from_rung.reads_back, "{:?}", drawn(&from_rung.path));
    assert_eq!(drawn(&from_rung.path)[2..].iter().map(|rung| rung[0].0).collect::<Vec<_>>(), [8.0]);
    // A rung across the start on one rail only is kept: it pairs another point of the second rail.
    let slanted = cuttable(&[&[(0.0, 0.0), (10.0, 0.0)], &[(0.0, 3.0), (10.0, 3.0)], &[(4.9, -0.3), (6.1, 3.3)]], &[]);
    assert_eq!(slanted.rungs.len(), 1);
    let at = slanted.rungs[0].1;
    let first_rung = |from: [f64; 2], to: [f64; 2]| {
        let piece = slanted.piece(from, to, false, [0.0; 2], &mut Budget::DEFAULT.meter()).unwrap().unwrap();
        drawn(&piece.path)[2][0]
    };
    // Its points lie 5 and 6 mm along the rails: drawn reaching past them, from (4.99, -0.03).
    assert!(at[0] < at[1]);
    assert_eq!([first_rung([at[0]; 2], [10.0; 2]), first_rung([0.0; 2], [at[0], 10.0])], [(4.99, -0.03); 2]);
}

#[test]
fn a_piece_whose_rails_have_no_length_sews_nothing() {
    let bar = bar(&[5.0], &[]);
    assert_eq!(piece(&bar, 4.0, 4.0, false), None);
    assert_eq!(piece(&bar, 4.0, 3.0, false), None);
    // A piece shorter than the zigzag spacing is kept, and reads back as drawn (`DEV-ROUTE-004`).
    assert!(piece(&bar, 4.0, 4.3, false).unwrap().reads_back);
    // One rail with length is enough to draw.
    let one = bar.piece([4.0, 4.0], [4.0, 5.0], false, [0.0; 2], &mut Budget::DEFAULT.meter()).unwrap();
    assert!(one.is_some());
}

#[test]
fn a_piece_is_drawn_with_the_rails_named_and_swapped_as_the_column_draws_them() {
    // `reverse_rails` turns the first rail as it is sewn: the piece is drawn as the column is, for its own
    // setting to turn it again.
    let first_rail = |settings: &[(&str, &str)], second: [(f64, f64); 2]| {
        let column = cuttable(&[&[(0.0, 0.0), (10.0, 0.0)], &second, &[(5.0, -1.0), (5.0, 4.0)]], settings);
        drawn(&piece(&column, 0.0, 10.0, false).unwrap().path)[..2].to_vec()
    };
    let along = [(0.0, 3.0), (10.0, 3.0)];
    let drawn_as = vec![vec![(0.0, 0.0), (10.0, 0.0)], vec![(0.0, 3.0), (10.0, 3.0)]];
    assert_eq!(first_rail(&[("reverse_rails", "first")], along), drawn_as);
    assert_eq!(first_rail(&[("reverse_rails", "both")], along), drawn_as);
    assert_eq!(first_rail(&[("swap_satin_rails", "true")], along), drawn_as);
    // With `automatic`, rails drawn running opposite ways are drawn as they are sewn, running the same way.
    let opposite = [(10.0, 3.0), (0.0, 3.0)];
    assert_eq!(first_rail(&[], opposite), drawn_as);
    assert_eq!(first_rail(&[("reverse_rails", "none")], along), drawn_as);
}

#[test]
fn a_column_without_rungs_is_cut_with_the_rungs_its_nodes_add_and_a_rung_that_misses_is_left_out() {
    let nodes = cuttable(&[&[(0.0, 0.0), (5.0, 0.0), (10.0, 0.0)], &[(0.0, 3.0), (5.0, 3.0), (10.0, 3.0)]], &[]);
    assert!(nodes.rungs.iter().any(|(points, at)| points[0] == p(5.0, 0.0) && points[1] == p(5.0, 3.0) && *at == [5.0, 5.0]), "{:?}", nodes.rungs);
    let swapped = cuttable(&[&[(0.0, 0.0), (10.0, 0.0)], &[(0.0, 3.0), (10.0, 3.0)], &[(5.0, -1.0), (5.0, 4.0)]], &[("swap_satin_rails", "true")]);
    assert_eq!(swapped.rungs, [([p(5.0, 3.0), p(5.0, 0.0)], [5.0, 5.0])]);
    let missing = cuttable(&[&[(0.0, 0.0), (10.0, 0.0)], &[(0.0, 3.0), (10.0, 3.0)], &[(5.0, -1.0), (5.0, 2.0)], &[(7.0, -1.0), (7.0, 4.0)]], &[]);
    assert_eq!(missing.rungs, [([p(7.0, 0.0), p(7.0, 3.0)], [7.0, 7.0])]);
}
