//! Stroke to Satin (`REQ-VC-014`): a satin column drawn as one path, made into one drawn with rails and
//! rungs, sews as it did; or is refused, and says why.

// Test code may unwrap, panic and index (clippy.toml allows it in tests).
#![allow(clippy::unwrap_used, clippy::panic, clippy::indexing_slicing)]

use stitchcraft_core::{Budget, Mm, Point};
use stitchcraft_engine::PlanOutcome;
use stitchcraft_engine::design::{Join, Path, Segment, Subpath};
use stitchcraft_engine::generators::satin::drawn::{Column, Unmade, from_line};
use stitchcraft_testkit::designs::{RED, along, p, planned, polylines, widened};

/// The setting that makes a stroke a satin column.
const SATIN: [(&str, &str); 1] = [("satin_column", "true")];

/// What Stroke to Satin makes of the one subpath of `line`, a column `width` mm wide.
fn converted(line: &Path, width: f64) -> Result<Column, Unmade> {
    let [subpath] = line.subpaths.as_slice() else { panic!("a line is one subpath") };
    from_line(subpath, Mm::new(width).unwrap(), Join::UNSET, &mut Budget::DEFAULT.meter()).unwrap()
}

/// The needle points of `outcome`, with what each stitch is.
fn sewn(outcome: &PlanOutcome) -> Vec<(String, Point)> {
    outcome.plan.as_ref().unwrap().stitches().map(|s| (format!("{:?}", s.kind), s.at)).collect()
}

/// Checks that the column Stroke to Satin makes of `line`, `width` mm wide, sews as `line` does with the
/// satin effect: the same stitches, each within a millionth of a millimetre.
fn sews_alike(name: &str, line: Path, width: f64) {
    let column = converted(&line, width).unwrap_or_else(|why| panic!("{name}: {why}"));
    assert_eq!(column.path().subpaths.len(), 2 + column.rungs.len(), "{name}");
    let as_line = planned(vec![widened(along("c", line, &RED, &SATIN), width)]);
    let as_drawn = planned(vec![along("c", column.path(), &RED, &SATIN)]);
    // Both say the same, such as stitches that lean far along the column at a sharp corner.
    let said = |outcome: &PlanOutcome| outcome.diagnostics.iter().map(|d| d.code).collect::<Vec<_>>();
    assert_eq!(said(&as_drawn), said(&as_line), "{name}");
    assert!(as_line.plan.is_some() && as_drawn.plan.is_some(), "{name}: {:?}", as_line.diagnostics);
    let (a, b) = (sewn(&as_line), sewn(&as_drawn));
    let alike = a.len() == b.len() && a.iter().zip(&b).all(|((k, p), (l, q))| k == l && p.distance(*q) < 1e-6);
    assert!(alike, "{name}: {} stitches as a line, {} drawn", a.len(), b.len());
}

/// A path of one subpath of cubic curves from `start`, each `[control, control, end]`, closed or not.
fn curves(start: (f64, f64), curves: &[[(f64, f64); 3]], closed: bool) -> Path {
    let segments = curves.iter().map(|[c1, c2, end]| Segment::Cubic(p(c1.0, c1.1), p(c2.0, c2.1), p(end.0, end.1))).collect();
    Path { subpaths: vec![Subpath { start: p(start.0, start.1), segments, closed }] }
}

/// A circle of `radius` round (`x`, `y`), 4 cubic curves from its right-hand point, closed.
fn circle((x, y): (f64, f64), radius: f64) -> Path {
    let k = 4.0 / 3.0 * (std::f64::consts::SQRT_2 - 1.0) * radius;
    let r = radius;
    curves(
        (x + r, y),
        &[
            [(x + r, y + k), (x + k, y + r), (x, y + r)],
            [(x - k, y + r), (x - r, y + k), (x - r, y)],
            [(x - r, y - k), (x - k, y - r), (x, y - r)],
            [(x + k, y - r), (x + r, y - k), (x + r, y)],
        ],
        true,
    )
}

#[test]
fn req_vc_014_a_line_made_into_rails_and_rungs_sews_as_it_did() {
    sews_alike("a straight line", polylines(&[&[(0.0, 0.0), (30.0, 0.0)]]), 3.0);
    sews_alike("a V", polylines(&[&[(0.0, 0.0), (15.0, 20.0), (30.0, 0.0)]]), 3.0);
    let zigzag: Vec<(f64, f64)> = (0..12).map(|k| (f64::from(k) * 5.0, if k % 2 == 0 { 0.0 } else { 4.0 })).collect();
    sews_alike("a zigzag of 10 corners", polylines(&[&zigzag]), 3.0);
    let s = curves((0.0, 0.0), &[[(5.0, -8.0), (10.0, -8.0), (15.0, 0.0)], [(20.0, 8.0), (25.0, 8.0), (30.0, 0.0)]], false);
    sews_alike("an S curve 30 mm long", s.clone(), 3.0);
    sews_alike("the S curve, 6 mm wide", s, 6.0);
    sews_alike("a circle 10 mm across", circle((0.0, 0.0), 5.0), 3.0);
    sews_alike("a circle 30 mm across", circle((0.0, 0.0), 15.0), 3.0);
    sews_alike("a circle 50 mm across", circle((0.0, 0.0), 25.0), 3.0);
}

#[test]
fn req_vc_014_a_rung_drawn_crosses_both_rails_a_little_past_the_points_it_pairs() {
    let column = converted(&polylines(&[&[(0.0, 0.0), (10.0, 0.0)]]), 4.0).unwrap();
    // A straight line 10 mm long gets a rung in its middle, its points 4 mm apart, drawn a hundredth of that
    // longer at each end.
    assert_eq!(column.rungs.len(), 1);
    let [a, b] = column.rungs[0];
    // The line's points are rounded to a ten-thousandth of a CSS pixel before the rails are made.
    assert!(a.distance(p(5.0, -2.04)) < 1e-4 && b.distance(p(5.0, 2.04)) < 1e-4, "{:?}", column.rungs);
    assert!(column.rails.iter().all(|rail| rail.len() == 2), "{:?}", column.rails);
}

#[test]
fn req_vc_014_a_column_of_2_rungs_or_none_is_drawn_with_one_more_at_its_start() {
    // A V gets 2 rungs, at the corner. A dash 3.3 mm long and 5 mm wide gets 2 too, longer than its rails,
    // which the rule for telling rails from rungs would take for the rails (DEV-SAT-007).
    let v = polylines(&[&[(0.0, 0.0), (15.0, 20.0), (30.0, 0.0)]]);
    let dash = polylines(&[&[(0.0, 0.0), (1.1, 0.0), (2.2, 0.0), (3.3, 0.0)]]);
    for (name, line, width) in [("a V", v, 3.0), ("a dash", dash, 5.0)] {
        let column = converted(&line, width).unwrap();
        assert_eq!(column.rungs.len(), 3, "{name}");
        // The first pairs the rails' first points, reaching past them.
        let [a, b] = column.rungs[0];
        let [first, second] = column.rails.each_ref().map(|rail| rail[0]);
        assert!(a.distance(first) < 0.011 * width && b.distance(second) < 0.011 * width, "{name}: {:?}", column.rungs);
        sews_alike(name, line, width);
    }
}

#[test]
fn req_vc_014_a_line_that_would_not_sew_alike_is_refused() {
    // Too short, and too tight for its width: no rails.
    assert_eq!(converted(&polylines(&[&[(0.0, 0.0), (0.2, 0.0)]]), 3.0), Err(Unmade::NoRails));
    assert_eq!(converted(&polylines(&[&[(0.0, 0.0), (10.0, 0.0), (10.0, 1.0), (0.0, 1.0)]]), 2.0), Err(Unmade::NoRails));
    // A U that a line crosses: the part in the U is left out.
    let bent = polylines(&[&[(0.0, 0.0), (10.0, 0.0), (10.0, 1.0), (0.0, 1.0), (13.2, -16.6)]]);
    assert_eq!(converted(&bent, 2.0), Err(Unmade::PartsLeftOut(1)));
    // An X: its halves are made on their own and joined, and its rails cross each other, so they meet more
    // than the one rung, and would be read back by length.
    assert_eq!(converted(&polylines(&[&[(0.0, 0.0), (20.0, 20.0), (20.0, 0.0), (0.0, 20.0)]]), 2.0), Err(Unmade::ReadOtherwise));
    // Each says what is wrong, after the line's name.
    assert_eq!(
        Unmade::PartsLeftOut(2).to_string().split(", which").next(),
        Some("has 2 parts that cross themselves or turn more tightly than the column is wide")
    );
    assert!(Unmade::NoRails.to_string().starts_with("is too short or turns too tightly"));
    assert!(Unmade::PartsLeftOut(1).to_string().starts_with("has a part that crosses itself"));
    assert!(Unmade::ReadOtherwise.to_string().contains("read back with other rails or rungs"));
}
