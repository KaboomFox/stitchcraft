use stitchcraft_core::{Budget, Code, ElementId, Mm, Point};

use super::*;
use crate::design::{Cap, Join, Segment, Shape};

fn p(x: f64, y: f64) -> Point {
    Point::new(x, y).unwrap()
}

/// A guide `id` along `points`, a stroke 0.2 mm wide, closed when `closed`.
fn guide(id: &str, points: &[(f64, f64)], closed: bool) -> Mark {
    let (first, rest) = points.split_first().unwrap();
    let line = Subpath { start: p(first.0, first.1), segments: rest.iter().map(|&(x, y)| Segment::Line(p(x, y))).collect(), closed };
    let shape = Shape::Stroke { path: Path { subpaths: vec![line] }, width: Mm::new(0.2).unwrap(), join: Join::Round, cap: Cap::Butt };
    Mark { id: ElementId::new(id).unwrap(), shape }
}

/// A square part from (`x`, `y`), `side` across, its ring closed.
fn part(x: f64, y: f64, side: f64) -> Polygon {
    Polygon { outline: vec![p(x, y), p(x + side, y), p(x + side, y + side), p(x, y + side), p(x, y)], holes: Vec::new() }
}

fn budget(work: u64) -> Budget {
    Budget { max_work: work, ..Budget::DEFAULT }
}

#[test]
fn req_auto_111_without_guides_nothing_is_tested_or_charged() {
    let guides = Guides::of(&[], &Budget::DEFAULT);
    assert!(guides.is_empty());
    assert_eq!(guides.crossing(&part(0.0, 0.0, 1.0), None, &mut budget(0).meter()), Ok(None));
    assert_eq!(guides.path(0), None);
    assert!(guides.said(true).is_empty());
}

#[test]
fn req_auto_109_a_part_follows_a_guide_whose_line_has_a_stretch_inside_it() {
    let guides = Guides::of(&[guide("across", &[(-1.0, 5.0), (11.0, 5.0)], false)], &Budget::DEFAULT);
    assert!(!guides.is_empty());
    assert_eq!(guides.crossing(&part(0.0, 0.0, 10.0), None, &mut Budget::DEFAULT.meter()), Ok(Some(0)));
    // A part the line misses, and one it only touches at a corner, don't follow it.
    assert_eq!(guides.crossing(&part(0.0, 6.0, 4.0), None, &mut Budget::DEFAULT.meter()), Ok(None));
    assert_eq!(guides.crossing(&part(11.0, 5.0, 2.0), None, &mut Budget::DEFAULT.meter()), Ok(None));
    // A unit for each of the line's segment and the square's 4 sides, and one for each of the 3 stretches
    // between its cuts: 7.
    assert_eq!(guides.crossing(&part(0.0, 0.0, 10.0), None, &mut budget(7).meter()), Ok(Some(0)));
    assert!(guides.crossing(&part(0.0, 0.0, 10.0), None, &mut budget(6).meter()).is_err());
}

#[test]
fn req_auto_110_a_part_two_guides_cross_follows_the_last() {
    let guides = Guides::of(
        &[
            guide("first", &[(-1.0, 5.0), (11.0, 5.0)], false),
            guide("second", &[(5.0, -1.0), (5.0, 11.0)], false),
            guide("far", &[(20.0, 0.0), (30.0, 0.0)], false),
        ],
        &Budget::DEFAULT,
    );
    assert_eq!(guides.crossing(&part(0.0, 0.0, 10.0), None, &mut Budget::DEFAULT.meter()), Ok(Some(1)));
    assert_eq!(guides.path(1).map(|path| path.subpaths[0].start), Some(p(5.0, -1.0)));
}

#[test]
fn req_auto_109_a_closed_guide_is_opened_where_it_closes() {
    // A closed guide round 3 sides of a square: opened, its line doesn't run back along the fourth, so a
    // part under that side alone isn't crossed.
    let guides = Guides::of(&[guide("ring", &[(0.0, 0.0), (10.0, 0.0), (10.0, 10.0), (0.0, 10.0)], true)], &Budget::DEFAULT);
    assert_eq!(guides.crossing(&part(-1.0, 3.0, 2.0), None, &mut Budget::DEFAULT.meter()), Ok(None));
    assert_eq!(guides.crossing(&part(9.0, 3.0, 2.0), None, &mut Budget::DEFAULT.meter()), Ok(Some(0)));
}

#[test]
fn diag_sc_i0918_a_guide_is_named_with_what_follows_it() {
    let guides = Guides::of(&[guide("g", &[(0.0, 0.0), (1.0, 0.0)], false), guide("h", &[(0.0, 1.0), (1.0, 1.0)], false)], &Budget::DEFAULT);
    guides.steered(0, 3);
    guides.steered(0, 1);
    guides.kept(0);
    guides.steered(1, 1);
    guides.kept(1);
    guides.kept(1);
    // Nothing is counted for a guide that isn't there.
    guides.steered(2, 5);
    guides.kept(2);
    let said = guides.said(true);
    assert_eq!(said.iter().map(|d| d.code).collect::<Vec<_>>(), [Code::AutoGuided, Code::AutoGuided]);
    assert_eq!(
        said[0].message,
        "4 filled parts of 2 automatic areas sew as guided fills whose rows follow the guide `g`. It crosses 1 part sewn along their middles, which keep their stitches."
    );
    assert!(
        said[1].message.starts_with("1 filled part of 1 automatic area sew") && said[1].message.contains("crosses 2 parts"),
        "{}",
        said[1].message
    );
    assert_eq!(said[1].element.as_ref().map(ElementId::as_str), Some("h"));
}

#[test]
fn diag_sc_w0919_a_guide_that_steers_nothing_is_named() {
    // A guide of one point has no length, one whose reading runs out of work isn't read, and one that
    // crosses nothing steers nothing. In a design with no automatic shape, every guide is unused.
    let point = guide("point", &[(1.0, 1.0), (1.0, 1.0)], false);
    let empty = Mark { id: ElementId::new("empty").unwrap(), shape: Shape::stroke(Path::default()) };
    let line = guide("line", &[(0.0, 0.0), (1.0, 0.0)], false);
    let guides = Guides::of(&[point.clone(), empty, line.clone()], &Budget::DEFAULT);
    let said = guides.said(true);
    assert_eq!(said.iter().map(|d| d.code).collect::<Vec<_>>(), [Code::AutoGuideUnused; 3]);
    assert!(said[0].message.ends_with("its line has no length.") && said[2].message.ends_with("it crosses no filled part of an automatic area."));
    let short = Guides::of(std::slice::from_ref(&line), &budget(0));
    assert!(short.said(true)[0].message.contains("needed more than the work budget of 0 units"));
    let without = Guides::of(std::slice::from_ref(&line), &Budget::DEFAULT);
    without.steered(0, 1);
    assert!(without.said(false)[0].message.ends_with("the design has no shape digitized automatically."));
    assert_eq!(unused_all(&[point, line]).len(), 2);
}

#[test]
fn req_auto_109_a_part_whole_under_its_details_follows_a_guide_only_where_it_shows() {
    // A square 10 mm across, digitized whole under a detail that fills its middle, from 3 to 7 mm: a guide
    // across the detail alone doesn't steer it, and one that crosses what shows of it does.
    let shown = [Polygon {
        outline: vec![p(0.0, 0.0), p(10.0, 0.0), p(10.0, 10.0), p(0.0, 10.0), p(0.0, 0.0)],
        holes: vec![vec![p(3.0, 3.0), p(3.0, 7.0), p(7.0, 7.0), p(7.0, 3.0), p(3.0, 3.0)]],
    }];
    let over_the_detail = Guides::of(&[guide("detail", &[(4.0, 5.0), (6.0, 5.0)], false)], &Budget::DEFAULT);
    let across = Guides::of(&[guide("across", &[(1.0, 5.0), (6.0, 5.0)], false)], &Budget::DEFAULT);
    let square = part(0.0, 0.0, 10.0);
    assert_eq!(over_the_detail.crossing(&square, None, &mut Budget::DEFAULT.meter()), Ok(Some(0)));
    assert_eq!(over_the_detail.crossing(&square, Some(&shown), &mut Budget::DEFAULT.meter()), Ok(None));
    assert_eq!(across.crossing(&square, Some(&shown), &mut Budget::DEFAULT.meter()), Ok(Some(0)));
    // The line's segment against the square's 4 sides, its 1 stretch, then against the 8 sides of what
    // shows and its 2 stretches: 15 units, and with 14 the work runs out.
    assert_eq!(across.crossing(&square, Some(&shown), &mut budget(15).meter()), Ok(Some(0)));
    assert!(across.crossing(&square, Some(&shown), &mut budget(14).meter()).is_err());
}
