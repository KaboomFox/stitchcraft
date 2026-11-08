//! `REQ-GEN-001`: an element's start and end positions, which its design gives it as Ink/Stitch's commands
//! or VectorCraft's markers, take the place of the needle before it and of what the element after it
//! offers, for the stitch types that use them. `REQ-FILL-CIR-008`: a circular fill turns round its target
//! point. These cases sew whole designs, the positions placed as the readers place them
//! (`crates/stitchcraft-svg/tests/inkstitch.rs`, `crates/stitchcraft-vectorcraft/tests/markers.rs`). The
//! designs lie within the reference hoop round their origin.

// Test code may unwrap, index and panic (clippy.toml allows the first 2 in tests).
#![allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::panic)]

use stitchcraft_core::{Budget, Point};
use stitchcraft_engine::PlanOutcome;
use stitchcraft_engine::design::{DesignSettings, Element, Hints};
use stitchcraft_engine::generate::approach;
use stitchcraft_engine::generators::Approach;
use stitchcraft_plan::{Role, Stitch, StitchKind};
use stitchcraft_testkit::designs::{RED, along, fill, messages, p, planned, polylines};

/// How near a corner of a square 20 mm a side its fill starts or ends, to start or end at that corner: the
/// other corners lie 16 mm away or more.
const NEAR: f64 = 4.0;

/// `element` with the start and end positions `start` and `end`.
fn placed(element: Element, start: Option<Point>, end: Option<Point>) -> Element {
    Element { hints: Hints { start, end, ..Hints::default() }, ..element }
}

/// The needle points the element `index` sews in `outcome`'s plan, locks left out.
fn sewn_by(outcome: &PlanOutcome, index: usize) -> Vec<Point> {
    let plan = outcome.plan.as_ref().unwrap_or_else(|| panic!("{:?}", messages(outcome)));
    let mine = |s: &&Stitch| s.kind == StitchKind::Normal && s.origin.role != Role::Lock && s.origin.element.map(|e| e.index()) == Some(index);
    plan.stitches().filter(mine).map(|s| s.at).collect()
}

/// A square fill 20 mm a side round (0, 10), filled by `method`, with no underlay.
fn square(method: &str) -> Element {
    let ring = [(-10.0, 0.0), (10.0, 0.0), (10.0, 20.0), (-10.0, 20.0)];
    fill("fill", &[&ring], &RED, &[("fill_method", method), ("fill_underlay", "false")])
}

/// A short line before `middle`, ending at (-50, 0), and one after it, starting at (50, 40): the needle and
/// the next element the positions take the place of.
fn around(middle: Element) -> Vec<Element> {
    let before = along("before", polylines(&[&[(-50.0, -10.0), (-50.0, 0.0)]]), &RED, &[]);
    let after = along("after", polylines(&[&[(50.0, 40.0), (50.0, 50.0)]]), &RED, &[]);
    vec![before, middle, after]
}

/// A satin column 40 mm long between rails at y = 0 and y = 6, with the settings `own`.
fn column(own: &[(&str, &str)]) -> Element {
    let rails = polylines(&[&[(-20.0, 0.0), (20.0, 0.0)], &[(-20.0, 6.0), (20.0, 6.0)]]);
    along("column", rails, &RED, &[own, &[("satin_column", "true")]].concat())
}

#[test]
fn req_gen_001_a_fill_starts_and_ends_nearest_its_positions() {
    // Without positions, the fill starts nearest the needle, at its corner (-10, 0), and ends nearest the
    // next element, at its corner (10, 20). With them, it starts nearest the start position, here its
    // corner (10, 0), and ends nearest the end position, its corner (-10, 20), whatever its neighbours. A
    // legacy fill is sewn as a tatami fill (DEV-FILL-014), positions and all. A meander fill's line starts
    // and ends on the tiling, its points a tile or so apart, and a cross stitch fill at the corners of its
    // crosses on the 3 mm grid, x = ±9 and y = 0 or 21, the cells beyond them covered too little.
    let (start, end) = (p(15.0, -5.0), p(-15.0, 25.0));
    for method in ["tatami_fill", "legacy_fill", "meander_fill", "circular_fill", "cross_stitch"] {
        let plain = sewn_by(&planned(around(square(method))), 1);
        assert!(plain[0].distance(p(-10.0, 0.0)) < NEAR, "{method}: {:?}", plain[0]);
        assert!(plain.last().unwrap().distance(p(10.0, 20.0)) < NEAR, "{method}: {:?}", plain.last());
        let outcome = planned(around(placed(square(method), Some(start), Some(end))));
        assert!(messages(&outcome).iter().all(|m| method == "legacy_fill" && m.contains("SC-W0314")), "{method}: {:?}", messages(&outcome));
        let sewn = sewn_by(&outcome, 1);
        assert!(sewn[0].distance(p(10.0, 0.0)) < NEAR, "{method}: {:?}", sewn[0]);
        assert!(sewn.last().unwrap().distance(p(-10.0, 20.0)) < NEAR, "{method}: {:?}", sewn.last());
    }
}

#[test]
fn req_gen_001_a_contour_fill_uses_its_start_position_alone() {
    // The contour fill starts on its outline nearest the start position, and ends where its rings end, as
    // without an end position.
    let (start, end) = (p(15.0, -5.0), p(-15.0, 25.0));
    let alone = sewn_by(&planned(around(placed(square("contour_fill"), Some(start), None))), 1);
    let both = sewn_by(&planned(around(placed(square("contour_fill"), Some(start), Some(end)))), 1);
    assert!(alone[0].distance(p(10.0, 0.0)) < 1.5, "{:?}", alone[0]);
    assert_eq!(both, alone);
}

#[test]
fn req_gen_001_a_fill_in_parts_starts_its_first_part_alone_nearest_its_start_position() {
    // A square, and below it a triangle whose tip points up at it. The start position lies nearer the
    // square, which goes first: it starts at its corner (10, 0), nearest the position, and ends nearest the
    // triangle, at (0, 20). The triangle then starts nearest that, at its tip, where Ink/Stitch's starts at
    // its point nearest the start position, (5, 35) (DEV-FILL-023).
    let square = [(-10.0, 0.0), (10.0, 0.0), (10.0, 20.0), (-10.0, 20.0)];
    let triangle = [(0.0, 30.0), (20.0, 50.0), (-20.0, 50.0)];
    let parts = fill("fill", &[&square, &triangle], &RED, &[("fill_underlay", "false")]);
    let outcome = planned(vec![placed(parts, Some(p(50.0, -10.0)), None)]);
    assert!(messages(&outcome).iter().all(|m| m.contains("SC-W0307")), "{:?}", messages(&outcome));
    let sewn = sewn_by(&outcome, 0);
    assert!(sewn[0].distance(p(10.0, 0.0)) < 1.5, "{:?}", sewn[0]);
    let second = sewn.iter().find(|q| q.y() > 25.0).unwrap();
    assert!(second.distance(p(0.0, 30.0)) < 1.5, "{second:?}");
}

#[test]
fn req_gen_001_a_satin_column_starts_at_its_start_position_and_ends_at_its_end_position() {
    // With a start position, the column's first needle point is the start position itself, from which the
    // needle follows the line to the first stitch; with an end position, its last is the end position,
    // whatever its nearest-point settings say.
    let (start, end) = (p(0.0, 3.0), p(10.0, 3.0));
    for own in [&[][..], &[("start_at_nearest_point", "false"), ("end_at_nearest_point", "false")]] {
        let outcome = planned(around(placed(column(own), Some(start), Some(end))));
        assert!(messages(&outcome).is_empty(), "{own:?}: {:?}", messages(&outcome));
        let sewn = sewn_by(&outcome, 1);
        assert_eq!((sewn[0], *sewn.last().unwrap()), (start, end), "{own:?}");
    }
    // An end position within 5 CSS pixels (1.32 mm) of the line's end changes nothing: the column ends as
    // it does without one, its nearest point to the next element turned off.
    let own = [("end_at_nearest_point", "false")];
    let plain = sewn_by(&planned(around(column(&own))), 1);
    let near_end = sewn_by(&planned(around(placed(column(&own), None, Some(p(19.0, 3.0))))), 1);
    assert_eq!(near_end, plain);
}

#[test]
fn req_gen_001_a_stroke_uses_neither() {
    let line = || along("line", polylines(&[&[(-20.0, 0.0), (20.0, 0.0)]]), &RED, &[]);
    let plain = sewn_by(&planned(around(line())), 1);
    assert_eq!(sewn_by(&planned(around(placed(line(), Some(p(0.0, 5.0)), Some(p(10.0, 5.0))))), 1), plain);
}

#[test]
fn req_gen_001_a_fill_or_a_satin_column_with_a_start_position_offers_it() {
    // A fill of any method, sewn by StitchCraft or not, and a satin column (DEV-SAT-012) offer the element
    // before it its start position; a stroke offers its first point, as without one.
    let settings = DesignSettings::default();
    let start = Some(p(15.0, -5.0));
    let offer = |element: Element| approach(&placed(element, start, None), &settings, &Budget::DEFAULT);
    for method in ["tatami_fill", "contour_fill", "legacy_fill", "tartan_fill", "guided_fill"] {
        assert_eq!(offer(square(method)), Some(Approach::Point(p(15.0, -5.0))), "{method}");
    }
    assert_eq!(offer(column(&[])), Some(Approach::Point(p(15.0, -5.0))));
    let line = along("line", polylines(&[&[(-20.0, 0.0), (20.0, 0.0)]]), &RED, &[]);
    assert_eq!(offer(line), Some(Approach::Point(p(-20.0, 0.0))));
    // So a fill before it ends nearest the start position: here at its corner (-40, 20), where without it
    // the 2 squares are equally near all along a side.
    let before = fill("before", &[&[(-60.0, 0.0), (-40.0, 0.0), (-40.0, 20.0), (-60.0, 20.0)]], &RED, &[("fill_underlay", "false")]);
    let outcome = planned(vec![before, placed(square("tatami_fill"), Some(p(-38.0, 22.0)), None)]);
    let last = *sewn_by(&outcome, 0).last().unwrap();
    assert!(last.distance(p(-40.0, 20.0)) < 1.5, "{last:?}");
}

#[test]
fn req_fill_cir_008_a_circular_fill_turns_round_its_target_point() {
    // Its circles lie round the target point in place of the square's centroid, (0, 10), the same point
    // serving a part it lies outside of. Along circles round a point, the needle points keep their distance
    // from it, give or take a row spacing a turn; measured from another point, it changes by up to a stitch
    // length from one to the next.
    let target = |at: Point| {
        let element = Element { hints: Hints { target: Some(at), ..Hints::default() }, ..square("circular_fill") };
        let outcome = planned(vec![element]);
        assert!(messages(&outcome).is_empty(), "{:?}", messages(&outcome));
        sewn_by(&outcome, 0)
    };
    let swing = |sewn: &[Point], centre: Point| {
        let mut steps: Vec<f64> = sewn.windows(2).map(|w| (w[0].distance(centre) - w[1].distance(centre)).abs()).collect();
        steps.sort_by(f64::total_cmp);
        steps[steps.len() / 2]
    };
    let (centroid, off_centre, outside) = (p(0.0, 10.0), p(-6.0, 15.0), p(-20.0, 10.0));
    let plain = sewn_by(&planned(vec![square("circular_fill")]), 0);
    for (sewn, round, not) in [(plain, centroid, off_centre), (target(off_centre), off_centre, centroid), (target(outside), outside, centroid)] {
        assert!(swing(&sewn, round) < 0.1 && swing(&sewn, not) > 0.5, "{round:?}: {} {}", swing(&sewn, round), swing(&sewn, not));
    }
    // Outside the square the circles still cover it, round the target.
    assert!(target(outside).iter().all(|q| (-10.5..=10.5).contains(&q.x())));
}
