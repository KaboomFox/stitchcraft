//! Less thread where areas overlap (`REQ-AUTO-067`): how far an area reaches along its rows and across
//! them, under wide and narrow shapes and under a running line's part, and which of 2 areas sews under a
//! shape sewn after both. The cases of `tests/auto_thick.rs` measure what that takes off a design. And how
//! far an area stays under a part's satin column (`REQ-AUTO-157`, `REQ-AUTO-158`).

use stitchcraft_core::{Budget, Code, ElementId, Mm};

use super::*;
use crate::auto::neighbours::Sewn;
use crate::auto::neighbours::parts_of;
use crate::auto::neighbours::tests::{area, area_with, filled, least_work, reached, square};
use crate::design::{Cap, DesignSettings, Element, Join, Path, Segment, Shape, Subpath};

fn p(x: f64, y: f64) -> Point {
    Point::new(x, y).unwrap()
}

/// The region of the fill `id` of `elements`.
fn region(elements: &[Element], id: &str) -> Vec<Polygon> {
    let element = elements.iter().find(|element| element.id.as_str() == id).unwrap();
    let Shape::Fill { path, rule, .. } = &element.shape else { panic!("{:?}", element.shape) };
    parts_of(path, *rule, &mut Budget::DEFAULT.meter()).unwrap().unwrap()
}

/// The least work with which `run` succeeds, found by halving.
fn least(run: impl Fn(&mut Meter) -> bool) -> u64 {
    let enough = |work: u64| run(&mut Budget { max_work: work, ..Budget::DEFAULT }.meter());
    if enough(0) {
        return 0;
    }
    let (mut low, mut high) = (0, 1 << 24);
    assert!(enough(high));
    while high - low > 1 {
        let middle = (low + high) / 2;
        if enough(middle) { high = middle } else { low = middle }
    }
    high
}

/// Some 300 budgets under `least`, spread evenly, and every one under 30: a step that charges a unit fails
/// with none, and one that charges for a region's work fails somewhere in it.
fn sampled(least: u64) -> impl Iterator<Item = u64> {
    let step = usize::try_from(least / 300).unwrap().max(1);
    (0..least.min(30)).chain((30..least).step_by(step))
}

/// The largest x and the largest y of `parts`, to a thousandth of a millimetre.
fn far(parts: &[Polygon]) -> (f64, f64) {
    let most = |pick: fn(Point) -> f64| {
        (parts.iter().flat_map(|part| &part.outline).map(|q| pick(*q)).fold(f64::NEG_INFINITY, f64::max) * 1000.0).round() / 1000.0
    };
    (most(Point::x), most(Point::y))
}

/// How far the square from (0, 0) to (10, 10) with the settings `params` reaches under a square to its
/// right, across whose edge its rows run at 0°, and under a square below it, along whose edge they run.
fn reaches(params: &[(&str, &str)]) -> (f64, f64) {
    let first = |second: (f64, f64)| {
        let sewn = vec![area_with("first", (0.0, 0.0), (10.0, 10.0), params), area("second", second, (second.0 + 10.0, second.1 + 10.0))];
        far(&region(&reached(sewn, &Budget::DEFAULT).0, "first"))
    };
    (((first((10.0, 0.0)).0 - 10.0) * 1000.0).round() / 1000.0, ((first((0.0, 10.0)).1 - 10.0) * 1000.0).round() / 1000.0)
}

#[test]
fn req_auto_067_an_area_reaches_its_overlap_along_its_rows_and_its_pull_compensation_across_them() {
    // By default 1 mm where its rows end at the edge, and 0.4 mm where they run along it.
    assert_eq!(reaches(&[]), (1.0, 0.4));
    // The reach turns with the rows.
    assert_eq!(reaches(&[("auto_fill_angle", "90")]), (0.4, 1.0));
    // Each is its setting, and across the rows an area never reaches further than along them.
    assert_eq!(reaches(&[("auto_overlap_mm", "1.5"), ("auto_pull_compensation_mm", "0.6")]), (1.5, 0.6));
    assert_eq!(reaches(&[("auto_overlap_mm", "0.5"), ("auto_pull_compensation_mm", "0.5")]), (0.5, 0.5));
    assert_eq!(reaches(&[("auto_overlap_mm", "0.3"), ("auto_pull_compensation_mm", "2")]), (0.3, 0.3));
    // Without pull compensation it still reaches a hundredth of a millimetre across them.
    assert_eq!(reaches(&[("auto_pull_compensation_mm", "0")]), (1.0, 0.01));
    assert_eq!((LEAST, Reach::of(&AutoParams::defaults())), (0.01, Reach { along: 1.0, across: 0.4, column: 1.0, angle: 0.0 }));
}

#[test]
fn req_auto_067_rows_at_any_angle_reach_as_far_along_them() {
    // At 45° the reach is the same to the right and downwards: the ellipse's half width there, for half
    // axes of 1 and 0.4 mm.
    let (right, down) = reaches(&[("auto_fill_angle", "45")]);
    let half = ((1.0_f64 + 0.16) / 2.0).sqrt();
    assert!((right - half).abs() < 0.002 && (down - half).abs() < 0.002, "{right} {down} {half}");
    // A grown part keeps its holes and its winding: a ring 4 mm wide round a hole 2 mm wide grows on both
    // sides.
    let ring = Polygon { holes: vec![vec![p(4.0, 4.0), p(6.0, 4.0), p(6.0, 6.0), p(4.0, 6.0), p(4.0, 4.0)]], ..square((0.0, 0.0), (10.0, 10.0)) };
    let reach = Reach { along: 0.5, across: 0.2, column: 0.5, angle: 0.0 };
    let grown = reach.grown(std::slice::from_ref(&ring), &mut Budget::DEFAULT.meter()).unwrap();
    let [grown] = grown.as_slice() else { panic!("{grown:?}") };
    assert_eq!(far(std::slice::from_ref(grown)), (10.5, 10.2));
    assert_eq!(far(&[Polygon { outline: grown.holes[0].clone(), holes: Vec::new() }]), (5.5, 5.8));
    // Its 4 corners are quarters of the ellipse, and the hole's stay square.
    let corners = 4.0 * 0.5 * 0.2 * (1.0 - std::f64::consts::FRAC_PI_4);
    assert!((area_of(std::slice::from_ref(grown)) - (11.0 * 10.4 - corners - 1.0 * 1.6)).abs() < 0.01, "{}", area_of(std::slice::from_ref(grown)));
    // Nothing grows to nothing.
    assert_eq!(reach.grown(&[], &mut Budget::DEFAULT.meter()).map_err(|_| ()), Ok(Vec::new()));
}

/// How far the square from (0, 0) to (10, 10) reaches to the right under `second`, sewn after it.
fn under(second: Sewn) -> f64 {
    far(&region(&reached(vec![area("first", (0.0, 0.0), (10.0, 10.0)), second], &Budget::DEFAULT).0, "first")).0
}

#[test]
fn req_auto_067_an_area_reaches_half_as_far_under_a_narrow_shape() {
    assert_eq!((WIDE, UNDER_NARROW), (3.0, 0.5));
    let narrow = Reach { along: 1.0, across: 0.4, column: 0.7, angle: 30.0 }.narrow();
    assert_eq!(narrow, Reach { along: 0.5, across: 0.4, column: 0.7, angle: 30.0 });
    assert_eq!(Reach { along: 0.6, across: 0.4, column: 0.6, angle: 0.0 }.narrow().across, 0.3);
    // An area wider than 3 overlaps is reached under by the whole overlap, and a narrower one by half.
    let strip = |wide: f64| area("second", (10.0, 0.0), (10.0 + wide, 10.0));
    assert_eq!([under(strip(3.1)), under(strip(2.9)), under(strip(1.0))], [11.0, 10.5, 10.5]);
    // A broad area's narrow arm is narrow, and its body is not: under an arm 2 mm wide beside the square the
    // reach is half, and under the body below the arm it is whole.
    let arm = Polygon {
        outline: vec![p(10.0, 0.0), p(10.0, 10.0), p(20.0, 10.0), p(20.0, 5.0), p(12.0, 5.0), p(12.0, 0.0), p(10.0, 0.0)],
        holes: Vec::new(),
    };
    let shaped = Sewn { element: filled("second", &[arm]), ..area("second", (0.0, 0.0), (1.0, 1.0)) };
    let first = region(&reached(vec![area("first", (0.0, 0.0), (10.0, 10.0)), shaped], &Budget::DEFAULT).0, "first");
    let [first] = first.as_slice() else { panic!("{first:?}") };
    assert!(first.covers(p(10.4, 2.0)) && !first.covers(p(10.6, 2.0)), "the arm");
    assert!(first.covers(p(10.9, 8.0)) && !first.covers(p(11.1, 8.0)), "the body");
}

#[test]
fn req_auto_067_an_area_reaches_its_whole_overlap_under_a_satin_column_whichever_way_its_rows_run() {
    // A satin column 2 mm wide, its rails along the square's side or under it: the column's stitches don't
    // cover its ends and bends as a fill covers its area, so the square reaches 1 mm under it either way,
    // though the column is narrow and the square's rows run along the lower one.
    let rail = |from: (f64, f64), to: (f64, f64)| Subpath { start: p(from.0, from.1), segments: vec![Segment::Line(p(to.0, to.1))], closed: false };
    let column = |rails: Vec<Subpath>| {
        let element = Element {
            shape: Shape::stroke(Path { subpaths: rails }),
            params: [("satin_column", "true")].into_iter().collect(),
            ..filled("second", &[])
        };
        Sewn { element, area: None, automatic: false, reach: None, shape: 1, part: None }
    };
    assert_eq!(under(column(vec![rail((10.0, 0.0), (10.0, 10.0)), rail((12.0, 0.0), (12.0, 10.0))])), 11.0);
    let below = vec![area("first", (0.0, 0.0), (10.0, 10.0)), column(vec![rail((0.0, 10.0), (10.0, 10.0)), rail((0.0, 12.0), (10.0, 12.0))])];
    assert_eq!(far(&region(&reached(below, &Budget::DEFAULT).0, "first")).1, 11.0);
}

#[test]
fn req_auto_067_an_area_reaches_its_whole_overlap_under_the_part_of_a_running_line() {
    // A strip 0.8 mm wide below the square, sewn as a running stitch along its middle: the thread doesn't
    // cover the strip, so the square reaches across it, though its rows run along it, as far as the strip
    // goes. Under an area there it would reach 0.4 mm.
    let strip = square((0.0, 10.0), (10.0, 10.8));
    let middle = Subpath { start: p(0.4, 10.4), segments: vec![Segment::Line(p(9.6, 10.4))], closed: false };
    let shape = Shape::Stroke { path: Path { subpaths: vec![middle] }, width: Mm::new(0.8).unwrap(), join: Join::Round, cap: Cap::Round };
    let line = Sewn { element: Element { shape, ..filled("line", &[]) }, area: None, automatic: true, reach: None, shape: 1, part: Some(strip) };
    let first = region(&reached(vec![area("first", (0.0, 0.0), (10.0, 10.0)), line], &Budget::DEFAULT).0, "first");
    assert_eq!(far(&first).1, ((10.8 + TOUCHING) * 1000.0).round() / 1000.0);
}

#[test]
fn req_auto_067_across_a_running_lines_part_the_area_sewn_first_reaches_on_under_the_area_beyond() {
    // 2 squares with a strip 0.6 mm wide between them, which sews as a running stitch after both. The left
    // square, sewn first, reaches across the strip and its whole overlap under the right square from there,
    // and the right square gives the strip up: their overlap is 1 mm, whatever the strip's width.
    let strip = square((10.0, 0.0), (10.6, 10.0));
    let middle = Subpath { start: p(10.3, 0.3), segments: vec![Segment::Line(p(10.3, 9.7))], closed: false };
    let shape = Shape::Stroke { path: Path { subpaths: vec![middle] }, width: Mm::new(0.6).unwrap(), join: Join::Round, cap: Cap::Round };
    let line = Sewn { element: Element { shape, ..filled("line", &[]) }, area: None, automatic: true, reach: None, shape: 2, part: Some(strip) };
    let sewn = vec![area("left", (0.0, 0.0), (10.0, 10.0)), area("right", (10.6, 0.0), (20.6, 10.0)), line];
    let elements = reached(sewn, &Budget::DEFAULT).0;
    let (left, right) = (region(&elements, "left"), region(&elements, "right"));
    assert_eq!(far(&left).0, ((10.6 + TOUCHING + 1.0) * 1000.0).round() / 1000.0);
    assert!((area_of(&left) - (100.0 + 1.6 * 10.0)).abs() < 0.9, "{}", area_of(&left));
    assert!((area_of(&right) - 100.0).abs() < 1e-6, "{}", area_of(&right));
    // With no area beyond the line, the square reaches across the strip alone.
    let strip = square((10.0, 0.0), (10.6, 10.0));
    let middle = Subpath { start: p(10.3, 0.3), segments: vec![Segment::Line(p(10.3, 9.7))], closed: false };
    let shape = Shape::Stroke { path: Path { subpaths: vec![middle] }, width: Mm::new(0.6).unwrap(), join: Join::Round, cap: Cap::Round };
    let line = Sewn { element: Element { shape, ..filled("line", &[]) }, area: None, automatic: true, reach: None, shape: 2, part: Some(strip) };
    let alone = reached(vec![area("left", (0.0, 0.0), (10.0, 10.0)), line], &Budget::DEFAULT).0;
    assert_eq!(far(&region(&alone, "left")).0, ((10.6 + TOUCHING) * 1000.0).round() / 1000.0);
}

#[test]
fn req_auto_067_a_part_that_a_line_and_a_column_sew_along_is_reached_across_once() {
    // A strip 0.8 mm wide below the square and twice as long. A running line sews along its left half and a
    // satin column along its right half, as the 2 bars of an L do that is one part: the strip is the part
    // of both. The square reaches 1 mm along the strip from its own corner, as under a column, and not 1 mm
    // more from there, as it would under a shape beyond a line.
    let strip = || square((0.0, 10.0), (20.0, 10.8));
    let stroke = |from: f64, to: f64| {
        let middle = Subpath { start: p(from, 10.4), segments: vec![Segment::Line(p(to, 10.4))], closed: false };
        Shape::Stroke { path: Path { subpaths: vec![middle] }, width: Mm::new(0.8).unwrap(), join: Join::Round, cap: Cap::Round }
    };
    let run = Element { shape: stroke(0.4, 9.6), ..filled("line", &[]) };
    let line = Sewn { element: run, area: None, automatic: true, reach: None, shape: 1, part: Some(strip()) };
    let rail = |y: f64| Subpath { start: p(10.4, y), segments: vec![Segment::Line(p(19.6, y))], closed: false };
    let rails = Shape::stroke(Path { subpaths: vec![rail(10.0), rail(10.8)] });
    let satin = Element { shape: rails, params: [("satin_column", "true")].into_iter().collect(), ..filled("column", &[]) };
    let column = Sewn { element: satin, area: None, automatic: true, reach: None, shape: 1, part: Some(strip()) };
    let first = region(&reached(vec![area("first", (0.0, 0.0), (10.0, 10.0)), line, column], &Budget::DEFAULT).0, "first");
    assert_eq!(far(&first), (11.0, ((10.8 + TOUCHING) * 1000.0).round() / 1000.0));
}

/// 3 areas that meet at (10, 10): `a` above `b`, and `c` to the right of both, sewn in that order.
fn corner() -> Vec<Sewn> {
    vec![area("a", (0.0, 0.0), (10.0, 10.0)), area("b", (0.0, 10.0), (10.0, 20.0)), area("c", (10.0, 0.0), (20.0, 20.0))]
}

#[test]
fn req_auto_067_under_a_shape_sewn_after_2_areas_only_the_one_sewn_first_sews() {
    let (elements, said) = reached(corner(), &Budget::DEFAULT);
    assert_eq!(
        said.iter().map(|said| said.message.as_str()).collect::<Vec<_>>(),
        ["2 areas reach 1 mm under the shapes sewn after them that they touch."]
    );
    let (a, b) = (region(&elements, "a"), region(&elements, "b"));
    let ([a], [b]) = (a.as_slice(), b.as_slice()) else { panic!("{a:?} {b:?}") };
    // Beside the corner, under `c`, both would sew: `a` does, and `b` gives it up.
    assert!(a.covers(p(10.5, 10.2)) && !b.covers(p(10.5, 10.2)));
    // `a` keeps its whole reach, and `b` its reach elsewhere.
    assert!(a.covers(p(10.9, 5.0)) && a.covers(p(5.0, 10.3)) && b.covers(p(10.9, 15.0)));
    // What the 2 share is what `a` sews under `b` itself.
    let meter = &mut Budget::DEFAULT.meter();
    let shared = intersection(std::slice::from_ref(a), std::slice::from_ref(b), meter).unwrap().unwrap();
    let beyond = difference(&shared, &[square((0.0, 10.0), (10.0, 20.0))], meter).unwrap().unwrap();
    assert!(area_of(&shared) > 3.9 && no_area(area_of(&beyond)), "{} {}", area_of(&shared), area_of(&beyond));
}

/// A rim that shows the square from (0, 0) to (10, 10) and sews `under`.
fn rim(under: &[Polygon]) -> Rim {
    Rim { shown: vec![square((0.0, 0.0), (10.0, 10.0))], under: under.to_vec() }
}

#[test]
fn req_auto_067_an_area_gives_up_what_an_area_sewn_before_it_sews_under_a_later_shape() {
    let meter = || Budget::DEFAULT.meter();
    let whole = [square((10.0, 0.0), (11.0, 10.0))];
    let other = |under: &[Polygon]| Some(Rim { shown: vec![square((0.0, 10.0), (10.0, 20.0))], under: under.to_vec() });
    // Nothing is given up by an area that sews under no shape, beside no area sewn before it, or beside
    // one that sews elsewhere, even next to it.
    let mut alone = rim(&whole);
    assert_eq!(parted(&mut rim(&[]), &[other(&whole)], &mut meter()).map_err(|_| ()), Ok(None));
    assert_eq!(parted(&mut alone, &[None], &mut meter()).map_err(|_| ()), Ok(None));
    assert_eq!(parted(&mut alone, &[other(&[square((30.0, 0.0), (31.0, 10.0))])], &mut meter()).map_err(|_| ()), Ok(None));
    assert_eq!(parted(&mut alone, &[other(&[square((11.0, 0.0), (12.0, 10.0))])], &mut meter()).map_err(|_| ()), Ok(None));
    assert_eq!(alone, rim(&whole));
    // What the other sews is taken from it: the middle of its rim, and the 2 ends stay with it.
    let mut cut = rim(&whole);
    let parts = parted(&mut cut, &[None, other(&[square((10.0, 4.0), (12.0, 6.0))])], &mut meter()).map_err(|_| ()).unwrap().unwrap();
    assert!((area_of(&parts) - 108.0).abs() < 1e-9 && (area_of(&cut.under) - 8.0).abs() < 1e-9 && parts.len() == 1, "{parts:?}");
    assert_eq!(cut.shown, rim(&[]).shown);
    // A piece that this cuts off from what shows of the area is left out.
    let mut off = rim(&[square((10.0, 0.0), (12.0, 10.0))]);
    let parts = parted(&mut off, &[other(&whole)], &mut meter()).map_err(|_| ()).unwrap().unwrap();
    assert!((area_of(&parts) - 100.0).abs() < 1e-9 && no_area(area_of(&off.under)) && parts.len() == 1, "{parts:?}");
    // Each area sewn before it costs a unit, and those near their work.
    let far_off = other(&[square((30.0, 0.0), (31.0, 10.0))]);
    let cost = |before: &[Option<Rim>]| least(|meter| parted(&mut rim(&whole), before, meter).is_ok());
    assert_eq!((cost(&[]), cost(std::slice::from_ref(&far_off)), cost(&[far_off.clone(), None, far_off])), (0, 1, 2));
}

#[test]
fn req_auto_067_an_area_whose_overlap_needs_more_work_to_part_sews_it_whole_and_is_said_to() {
    // The corner's `b` gives up a part of its overlap with the work that takes, and with any less it keeps
    // its region, which is said.
    let parted = |work: u64| {
        let mut shapes = Shapes::new(corner(), &DesignSettings::default(), &Budget::DEFAULT);
        let (mut rims, mut split) = (vec![None; 3], vec![None; 3]);
        for index in (0..3).rev() {
            let reach = Reach::of(shapes.rules(index).unwrap());
            let found = beside(&mut shapes, index, reach, &mut split, &mut Budget::DEFAULT.meter()).map_err(|_| ()).unwrap().unwrap();
            shapes.set_region(index, found.parts);
            rims[index] = found.rim;
        }
        let said = firsts(&mut shapes, &mut rims, &Budget { max_work: work, ..Budget::DEFAULT });
        (region(&shapes.into_elements(), "b"), said)
    };
    let (whole, least) = (parted(0).0, least_work(|budget| parted(budget.max_work).1));
    let (given_up, said) = parted(least);
    assert!(said.is_empty() && area_of(&given_up) < area_of(&whole) - 0.1, "{said:?}");
    for work in sampled(least) {
        let (region, said) = parted(work);
        let [said] = said.as_slice() else { panic!("{work}: {said:?}") };
        assert_eq!((said.code, said.element.as_ref().map(ElementId::as_str), &region), (Code::AutoUnder, Some("b"), &whole), "{work}");
        assert_eq!(
            said.message,
            format!(
                "Finding where this area's overlap lies on that of an area sewn before it needed more than the work budget of {work} units, so \
                 both sew there."
            )
        );
    }
}

#[test]
fn req_auto_067_an_area_that_needs_more_work_for_its_reach_keeps_its_region_at_every_step() {
    // A square beside a broad area with a narrow arm and above the part of a running line, its rows turned:
    // every kind of reach. With any less work than it needs, the square is sewn as drawn, and that is said.
    let sewn = || {
        let arm = Polygon {
            outline: vec![p(10.0, 0.0), p(10.0, 10.0), p(20.0, 10.0), p(20.0, 5.0), p(12.0, 5.0), p(12.0, 0.0), p(10.0, 0.0)],
            holes: Vec::new(),
        };
        let strip = square((0.0, 10.0), (10.0, 10.8));
        let middle = Subpath { start: p(0.4, 10.4), segments: vec![Segment::Line(p(9.6, 10.4))], closed: false };
        let shape = Shape::Stroke { path: Path { subpaths: vec![middle] }, width: Mm::new(0.8).unwrap(), join: Join::Round, cap: Cap::Round };
        vec![
            area_with("first", (0.0, 0.0), (10.0, 10.0), &[("auto_fill_angle", "30")]),
            Sewn { element: filled("second", &[arm]), area: None, automatic: false, reach: None, shape: 1, part: None },
            Sewn { element: Element { shape, ..filled("line", &[]) }, area: None, automatic: true, reach: None, shape: 2, part: Some(strip) },
        ]
    };
    let least = least_work(|budget| reached(sewn(), budget).1);
    let drawn = filled("first", &[square((0.0, 0.0), (10.0, 10.0))]).shape;
    for work in sampled(least) {
        let (elements, said) = reached(sewn(), &Budget { max_work: work, ..Budget::DEFAULT });
        assert!(
            said.iter().any(|said| said.code == Code::AutoUnder && said.element.as_ref().map(ElementId::as_str) == Some("first")),
            "{work}: {said:?}"
        );
        assert_eq!(elements[0].shape, drawn, "{work}");
    }
}

#[test]
fn req_auto_067_a_shape_is_split_once_for_the_areas_sewn_before_it_that_share_a_width() {
    // 2 squares on the left of a broad area with a narrow arm, far from each other: the one taken second
    // finds the area split, and costs less than it does alone.
    let arm = || Polygon {
        outline: vec![p(10.0, 0.0), p(10.0, 40.0), p(20.0, 40.0), p(20.0, 5.0), p(12.0, 5.0), p(12.0, 0.0), p(10.0, 0.0)],
        holes: Vec::new(),
    };
    let second = || Sewn { element: filled("second", &[arm()]), area: None, automatic: false, reach: None, shape: 1, part: None };
    // `Sewn` isn't cloned: the same elements again.
    let pair_of = |sewn: &[Sewn]| -> Vec<Sewn> {
        sewn.iter()
            .map(|s| Sewn {
                element: s.element.clone(),
                area: s.area.clone(),
                automatic: s.automatic,
                reach: s.reach,
                shape: s.shape,
                part: s.part.clone(),
            })
            .collect()
    };
    let work = |sewn: Vec<Sewn>, index: usize, split: &mut Vec<Option<Split>>| {
        let run = |split: &mut Vec<Option<Split>>, meter: &mut Meter| {
            let mut shapes = Shapes::new(pair_of(&sewn), &DesignSettings::default(), &Budget::DEFAULT);
            let reach = Reach::of(shapes.rules(index).unwrap());
            beside(&mut shapes, index, reach, split, meter).ok().flatten().map(|found| far(&found.parts).0)
        };
        let cost = least(|meter| run(&mut split.clone(), meter).is_some());
        (cost, run(split, &mut Budget::DEFAULT.meter()).unwrap())
    };
    let pair =
        |params: &[(&str, &str)]| vec![area_with("upper", (0.0, 0.0), (10.0, 4.0), params), area("lower", (0.0, 30.0), (10.0, 34.0)), second()];
    let mut split = vec![None; 3];
    let lower = work(pair(&[]), 1, &mut split);
    assert!(split[2].as_ref().is_some_and(|split| split.width == 3.0
        && !split.wide.is_empty()
        && !split.narrow.is_empty()
        && split.open.is_empty()
        && split.lines.is_empty()));
    let (found, again) = (split.clone(), work(pair(&[]), 0, &mut split));
    let alone = work(pair(&[]), 0, &mut vec![None; 3]);
    assert!(again.0 < alone.0 && again.1 == alone.1 && split == found, "{again:?} {alone:?}");
    assert_eq!((lower.1, alone.1), (11.0, 10.5), "under the body, and under the arm");
    // An area with another overlap splits it by its own width: 2 mm wide, the arm is wide to an overlap of
    // 0.6 mm, which reaches whole under it.
    let other = work(pair(&[("auto_overlap_mm", "0.6")]), 0, &mut split);
    assert_eq!((other.1, split[2].as_ref().map(|split| split.width)), (10.6, Some(0.6 * WIDE)));
}

/// A part's satin column of the shape `shape`, its part the box from `from` to `to`, its rails along the
/// box's sides from top to bottom.
fn part_column(shape: usize, from: (f64, f64), to: (f64, f64)) -> Sewn {
    let rail = |x: f64| Subpath { start: p(x, from.1), segments: vec![Segment::Line(p(x, to.1))], closed: false };
    let rails = Shape::stroke(Path { subpaths: vec![rail(from.0), rail(to.0)] });
    let satin = Element { shape: rails, params: [("satin_column", "true")].into_iter().collect(), ..filled("column", &[]) };
    Sewn { element: satin, area: None, automatic: true, reach: None, shape, part: Some(square(from, to)) }
}

/// The region of the square from (0, 0) to (10, 10), with the settings `params`, beside `later`.
fn first_beside(params: &[(&str, &str)], later: Vec<Sewn>) -> Vec<Polygon> {
    let mut sewn = vec![area_with("first", (0.0, 0.0), (10.0, 10.0), params)];
    sewn.extend(later);
    region(&reached(sewn, &Budget::DEFAULT).0, "first")
}

/// Whether `parts` cover the point (`x`, `y`).
fn covered_at(parts: &[Polygon], x: f64, y: f64) -> bool {
    parts.iter().any(|part| part.covers(p(x, y)))
}

#[test]
fn req_auto_157_an_area_reaches_its_column_overlap_under_a_parts_satin_column_it_meets() {
    // A part's column 2 mm wide to the right of the square and one below it: the square's rows end at the
    // first and run along the second, and it reaches as far under both.
    let beside = |params: &[(&str, &str)]| far(&first_beside(params, vec![part_column(1, (10.0, 0.0), (12.0, 10.0))])).0;
    let below = |params: &[(&str, &str)]| {
        let rails = |y: f64| Subpath { start: p(0.0, y), segments: vec![Segment::Line(p(10.0, y))], closed: false };
        let satin =
            Element { shape: Shape::stroke(Path { subpaths: vec![rails(10.0), rails(12.0)] }), ..part_column(1, (0.0, 0.0), (1.0, 1.0)).element };
        let column = Sewn { element: satin, part: Some(square((0.0, 10.0), (10.0, 12.0))), ..part_column(1, (0.0, 0.0), (1.0, 1.0)) };
        far(&first_beside(params, vec![column])).1
    };
    let set = |kept: &'static str| vec![("auto_column_overlap_mm", kept)];
    // Empty, the default, it is the overlap: 1 mm, or what the area sets.
    assert_eq!([beside(&[]), below(&[])], [11.0, 11.0]);
    assert_eq!(beside(&[("auto_overlap_mm", "0.5")]), 10.5);
    // Set, it is the setting, either way the rows run.
    for (kept, reach) in [("0.6", 10.6), ("0.4", 10.4), ("0.2", 10.2), ("0", 10.0)] {
        assert_eq!([beside(&set(kept)), below(&set(kept))], [reach, reach], "{kept}");
    }
    // A value above the overlap reads as the overlap, and one equal to it is the overlap.
    assert_eq!(
        [beside(&set("1.5")), beside(&set("1")), beside(&[("auto_overlap_mm", "0.5"), ("auto_column_overlap_mm", "0.8")])],
        [11.0, 11.0, 10.5]
    );
    let reach = |params: &[(&str, &str)]| Reach::of(&AutoParams::from_set(&params.iter().copied().collect()).unwrap().params);
    assert_eq!(reach(&set("0.4")), Reach { along: 1.0, across: 0.4, column: 0.4, angle: 0.0 });
    assert_eq!(reach(&[("auto_overlap_mm", "0.3"), ("auto_column_overlap_mm", "0.3")]).column, 0.3);
    assert_eq!(reach(&[("auto_overlap_mm", "0.3"), ("auto_column_overlap_mm", "0.31")]).column, 0.3);
    // Below 0 it reads as 0, which is warned of (`SC-W0102`).
    let below_zero = AutoParams::from_set(&set("-0.1").into_iter().collect()).unwrap();
    assert_eq!((below_zero.params.auto_column_overlap_mm.map(Mm::get), below_zero.warnings.len()), (Some(0.0), 1));
}

#[test]
fn req_auto_157_an_area_is_cut_away_its_column_overlap_inside_a_parts_satin_column_that_covers_it() {
    // A part's column 6 mm wide of a shape drawn after the square covers its right side from x = 6, the
    // full height: by default the square is cut away 1 mm inside the column's edge, from x = 7.
    let column = || vec![part_column(1, (6.0, 0.0), (12.0, 10.0))];
    let cut_from = |params: &[(&str, &str)]| {
        let first = first_beside(params, column());
        // Halfway up, the square ends where it is cut away.
        let ends = (0..300).map(|k| 5.005 + f64::from(k) * 0.01).find(|x| !covered_at(&first, *x, 5.0)).unwrap();
        (ends * 10.0).round() / 10.0
    };
    assert_eq!(cut_from(&[]), 7.0);
    for (kept, from) in [("0.6", 6.6), ("0.4", 6.4), ("0.2", 6.2), ("0", 6.0)] {
        assert_eq!(cut_from(&[("auto_column_overlap_mm", kept)]), from, "{kept}");
    }
    // An area sewn after the square that meets the column is no edge of it: the cut is measured from the
    // edge of what covers the square. With an area over the square's top 3 mm, beside the column, the
    // square ends 0.4 mm inside the column's edge below that area, and 1 mm inside the area's edge.
    let mut both = column();
    both.push(area("over", (0.0, 7.0), (6.0, 10.0)));
    let first = first_beside(&[("auto_column_overlap_mm", "0.4")], both);
    assert!(covered_at(&first, 6.3, 5.0) && !covered_at(&first, 6.5, 5.0));
    assert!(covered_at(&first, 3.0, 7.9) && !covered_at(&first, 3.0, 8.1));
    assert!(!covered_at(&first, 6.3, 7.7), "under the area and the column");
}

#[test]
fn req_auto_157_the_setting_changes_no_other_overlap() {
    let kept = [("auto_column_overlap_mm", "0.4")];
    // Under an area sewn after it, the square reaches its overlap.
    assert_eq!(far(&first_beside(&kept, vec![area("second", (10.0, 0.0), (20.0, 10.0))])).0, 11.0);
    // Under a satin column of a stroke, a person's own or lettering's, it reaches its whole overlap.
    let rail = |x: f64| Subpath { start: p(x, 0.0), segments: vec![Segment::Line(p(x, 10.0))], closed: false };
    let stroke = Element {
        shape: Shape::stroke(Path { subpaths: vec![rail(10.0), rail(12.0)] }),
        params: [("satin_column", "true")].into_iter().collect(),
        ..filled("second", &[])
    };
    let column = Sewn { element: stroke, area: None, automatic: false, reach: None, shape: 1, part: None };
    assert_eq!(far(&first_beside(&kept, vec![column])).0, 11.0);
    // Under the part of a running line it reaches across the part, as without the setting.
    let strip = square((10.0, 0.0), (10.8, 10.0));
    let middle = Subpath { start: p(10.4, 0.4), segments: vec![Segment::Line(p(10.4, 9.6))], closed: false };
    let shape = Shape::Stroke { path: Path { subpaths: vec![middle] }, width: Mm::new(0.8).unwrap(), join: Join::Round, cap: Cap::Round };
    let line = || Sewn {
        element: Element { shape: shape.clone(), ..filled("line", &[]) },
        area: None,
        automatic: true,
        reach: None,
        shape: 1,
        part: Some(strip.clone()),
    };
    assert_eq!(first_beside(&kept, vec![line()]), first_beside(&[], vec![line()]));
    // Inside an area sewn after it that covers it, it is cut away its overlap inside the edge.
    let over = |params: &[(&str, &str)]| first_beside(params, vec![area("over", (6.0, 0.0), (12.0, 10.0))]);
    assert_eq!(over(&kept), over(&[]));
    // Empty or equal to the overlap, the setting leaves every region as it was.
    let columns = || vec![part_column(1, (6.0, 0.0), (12.0, 10.0)), part_column(2, (0.0, -2.0), (10.0, 0.0))];
    assert_eq!(first_beside(&[("auto_column_overlap_mm", "1")], columns()), first_beside(&[], columns()));
}

#[test]
fn req_auto_157_an_area_that_needs_more_work_under_a_parts_column_keeps_its_region_at_every_step() {
    // A square covered by a part's column on its right and meeting another below it, with less overlap
    // under them: with any less work than it needs, the square is sewn as drawn, and that is said.
    let sewn = || {
        vec![
            area_with("first", (0.0, 0.0), (10.0, 10.0), &[("auto_column_overlap_mm", "0.4")]),
            part_column(1, (6.0, 0.0), (12.0, 10.0)),
            part_column(2, (0.0, 10.0), (6.0, 12.0)),
        ]
    };
    let least = least_work(|budget| reached(sewn(), budget).1);
    let drawn = filled("first", &[square((0.0, 0.0), (10.0, 10.0))]).shape;
    for work in sampled(least) {
        let (elements, said) = reached(sewn(), &Budget { max_work: work, ..Budget::DEFAULT });
        assert!(
            said.iter().any(|said| said.code == Code::AutoUnder && said.element.as_ref().map(ElementId::as_str) == Some("first")),
            "{work}: {said:?}"
        );
        assert_eq!(elements[0].shape, drawn, "{work}");
    }
}

#[test]
fn req_auto_158_with_the_setting_at_the_overlap_an_area_takes_the_steps_it_took_before() {
    // A square covered by a part's column on its right and meeting another below it. Empty or at the
    // overlap, the setting leaves the area to the steps it took before it existed, so the area needs exactly
    // the work it needed then. A hair under the overlap, the columns are set apart and it needs more.
    let work = |kept: &'static str| {
        let sewn = || {
            let params: Vec<(&str, &str)> = if kept.is_empty() { Vec::new() } else { vec![("auto_column_overlap_mm", kept)] };
            vec![
                area_with("first", (0.0, 0.0), (10.0, 10.0), &params),
                part_column(1, (6.0, 0.0), (12.0, 10.0)),
                part_column(2, (0.0, 10.0), (6.0, 12.0)),
            ]
        };
        least_work(|budget| reached(sewn(), budget).1)
    };
    let before = work("");
    assert_eq!(before, BEFORE_WORK);
    assert_eq!(work("1"), before);
    assert!(work("0.999") > before, "{}", work("0.999"));
}

/// The work the square of the case above needs on the steps an area took before the setting, counted when
/// the case was written: those steps charge as they did, and setting the columns apart charges more.
const BEFORE_WORK: u64 = 24_477;

/// `REQ-AUTO-205`: the part of a neighbour that lies beyond the window round an area takes no part in the
/// area's overlaps. The square from (0, 0) to (10, 10) meets an area sewn after it, a square to its right
/// with a second part, a circle 80 mm away drawn with 50 points or with 2,000. Once the neighbour's own
/// growth and split are found, which a design does once for each shape, the square needs at most a unit more
/// for each point the circle gains: the look that leaves it out. On main f1689168 the circle's points went
/// into every union of the square's neighbours, and each cost the square many units.
#[test]
#[ignore = "planned: M13.66 builds the window round an area"]
fn req_auto_205_a_neighbour_s_part_beyond_the_window_costs_the_overlaps_no_geometry() {
    let work = |points: u32| {
        let sewn = || {
            let rules = AutoParams::from_set(&Default::default()).unwrap().params;
            let parts = [square((10.0, 0.0), (20.0, 10.0)), crate::auto::neighbours::tests::circle((100.0, 5.0), 5.0, points)];
            let beside_it = Sewn { element: filled("beside", &parts), area: Some(rules), automatic: true, reach: None, shape: 1, part: None };
            vec![area("square", (0.0, 0.0), (10.0, 10.0)), beside_it]
        };
        least(|meter| {
            let settings = DesignSettings::default();
            let mut shapes = Shapes::new(sewn(), &settings, &Budget::DEFAULT);
            let reach = Reach::of(shapes.rules(0).unwrap());
            let mut split = vec![None; shapes.len()];
            split_of(&mut shapes, 1, WIDE * reach.along, &mut split, &mut Budget::DEFAULT.meter()).unwrap();
            beside(&mut shapes, 0, reach, &mut split, meter).is_ok()
        })
    };
    let (few, many) = (work(50), work(2_000));
    assert!(many <= few + 1_950, "{few} units with 50 points, {many} with 2,000");
}
