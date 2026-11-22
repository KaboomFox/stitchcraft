//! Parts of automatic areas as wide as a satin column (`REQ-AUTO-016` to `REQ-AUTO-018`): a part that isn't
//! narrow, and is nowhere wider than 7 mm, sews as satin columns along its centre lines, between its own
//! outlines, each drawn with its rails and rungs and with the settings of its width; a column that ends under
//! another sews first, longer at that end; and the areas sewn before the columns reach under them.

// Test code may unwrap and index (clippy.toml allows it in tests).
#![allow(clippy::unwrap_used, clippy::panic, clippy::indexing_slicing)]

use stitchcraft_core::math::{self, sin_cos};
use stitchcraft_core::{Budget, Code, Mm, Point};
use stitchcraft_engine::auto::{
    CONTOUR_FROM, CROWDED_INSETS, Digitized, EDGE_REACH, OPEN_HOLE, SHORT_STITCHES, Stitches, ZIGZAG_ABOVE, digitize, digitized,
};
use stitchcraft_engine::design::{Design, DesignSettings, Element, Shape};
use stitchcraft_engine::generators::passes::BEAN_STITCH_REPEATS;
use stitchcraft_engine::generators::pull::PULL_COMPENSATION;
use stitchcraft_engine::generators::satin::{
    CENTER_WALK_UNDERLAY, CENTER_WALK_UNDERLAY_REPEATS, CONTOUR_UNDERLAY, END_AT_NEAREST_POINT, PUSH_COMPENSATION, SATIN_COLUMN,
    SHORT_STITCH_DISTANCE, SHORT_STITCH_INSET, START_AT_NEAREST_POINT, ZIGZAG_UNDERLAY,
};
use stitchcraft_engine::normalize::region;
use stitchcraft_engine::normalize::satin::{Pairing, Shape as Read, recognize};
use stitchcraft_engine::testsheets::find;
use stitchcraft_plan::{Role, StitchKind};
use stitchcraft_testkit::designs::{BLUE, RED, along, automatic, fill, messages, planned, polylines, rules, unbased, widened};

/// What the automatic `element` sews as, in a design with the default settings.
fn made(element: &Element) -> Digitized {
    digitize(&automatic(element.clone()), &DesignSettings::default(), &mut Budget::DEFAULT.meter()).unwrap()
}

/// A red fill `id` of the outline `outline`.
fn shape(id: &str, outline: &[(f64, f64)]) -> Element {
    fill(id, &[outline], &RED, &[])
}

/// A rectangle's corners, from `(x0, y0)` to `(x1, y1)`.
fn rectangle((x0, y0): (f64, f64), (x1, y1): (f64, f64)) -> Vec<(f64, f64)> {
    vec![(x0, y0), (x1, y0), (x1, y1), (x0, y1)]
}

/// The setting `key` of `element`.
fn setting<'e>(element: &'e Element, key: &str) -> Option<&'e str> {
    element.params.iter().find(|(k, _)| *k == key).map(|(_, value)| value)
}

/// The underlays `element` turns on, of the centre walk, the contour and the zigzag.
fn underlays(element: &Element) -> Vec<&'static str> {
    [CENTER_WALK_UNDERLAY, CONTOUR_UNDERLAY, ZIGZAG_UNDERLAY].into_iter().filter(|key| setting(element, key) == Some("true")).collect()
}

/// The rails and rungs the satin column `element` is read back with.
fn read(element: &Element) -> ([Vec<Point>; 2], Vec<[Point; 2]>) {
    let Shape::Stroke { path, .. } = &element.shape else { panic!("`{}` is not a stroke: {:?}", element.id, element.shape) };
    let found = recognize(path, &mut Budget::DEFAULT.meter()).unwrap();
    assert!(found.warnings.is_empty(), "{:?}", found.warnings);
    let Ok(Read::Rails(satin)) = found.shape else { panic!("{:?}", found.shape) };
    let Pairing::Rungs(rungs) = satin.pairing else { panic!("{:?}", satin.pairing) };
    (satin.rails, rungs.into_iter().map(|rung| rung.points).collect())
}

/// The area the fill `element` sews, in square millimetres.
fn area(element: &Element) -> f64 {
    let Shape::Fill { path, rule, .. } = &element.shape else { panic!("`{}` is not a fill: {:?}", element.id, element.shape) };
    region::build(path, *rule, &mut Budget::DEFAULT.meter()).unwrap().region.area()
}

#[test]
fn req_auto_016_a_round_dot_stays_in_the_fill_wherever_it_lies() {
    // A dot drawn as a circle of 32 corners, 3.4 mm and 2.7 mm across, at each place a button's dots lie
    // (`testsheets::pictures::button`). Its triangles leave its middle as lines that depend on where it lies,
    // through ties between corners on one circle, but never longer than the dot is wide: it is a dot, filled,
    // wherever it lies.
    for (radius, (x, y)) in [(1.7, (15.0, 15.0)), (1.36, (103.0, 38.4))] {
        for k in 0..7_u32 {
            let (sin, cos) = stitchcraft_core::math::sin_cos(-std::f64::consts::FRAC_PI_2 + std::f64::consts::TAU * f64::from(k) / 7.0);
            let (cx, cy) = (x + 5.6 / 1.7 * radius * cos, y + 5.6 / 1.7 * radius * sin);
            let ring: Vec<(f64, f64)> = (0..32_u32)
                .map(|j| {
                    let (s, c) = stitchcraft_core::math::sin_cos(std::f64::consts::TAU * f64::from(j) / 32.0);
                    (cx + radius * c, cy + radius * s)
                })
                .collect();
            let out = made(&shape("dot", &ring));
            assert_eq!(out.stitches, Some(Stitches::Fill), "a dot {radius} mm round ({cx}, {cy}): {:?}", out.elements);
        }
    }
}

#[test]
fn req_auto_031_a_rectangle_is_a_column_from_3_times_its_width() {
    // A square 3 mm wide is compact, and filled, and so is a rectangle as wide and 8.9 mm long, whose area is
    // less than 3 times its width squared. One 9 mm long is exactly that, and one 9.5 mm long more: each is a
    // satin column, wherever it lies.
    for (x, y) in [(0.0, 0.0), (10.3, 7.1), (-4.6, 21.9)] {
        for (long, stitches) in [(3.0, Stitches::Fill), (8.9, Stitches::Fill), (9.0, Stitches::Satin), (9.5, Stitches::Satin)] {
            let rectangle = made(&shape("rectangle", &rectangle((x, y), (x + long, y + 3.0))));
            assert_eq!(rectangle.stitches, Some(stitches), "{long} mm long at ({x}, {y}): {:?}", rectangle.elements);
        }
    }
}

#[test]
fn req_auto_016_a_band_sews_as_one_satin_column_between_its_sides() {
    // A band 3 mm wide and 20 mm long: one satin column, which keeps the band's name, read back with the
    // band's long sides as its rails and rungs straight across it, and settings that say so.
    let band = shape("band", &rectangle((0.0, 0.0), (20.0, 3.0)));
    let out = made(&band);
    assert_eq!(out.stitches, Some(Stitches::Satin));
    let [column] = out.elements.as_slice() else { panic!("{:?}", out.elements) };
    assert_eq!(column.id.as_str(), "band");
    assert!(!column.automatic);
    let Shape::Stroke { width, .. } = column.shape else { panic!() };
    assert!((width.get() - 3.0).abs() < 1e-6, "{width:?}");
    let (rails, rungs) = read(column);
    for (rail, y) in rails.iter().zip(if rails[0][0].y() == 0.0 { [0.0, 3.0] } else { [3.0, 0.0] }) {
        assert!(rail.iter().all(|q| q.y() == y), "{rail:?}");
        assert_eq!((rail[0].x() - rail.last().unwrap().x()).abs(), 20.0, "{rail:?}");
    }
    assert!(!rungs.is_empty() && rungs.iter().all(|[a, b]| (a.x() - b.x()).abs() < 1e-6), "{rungs:?}");
    assert_eq!(setting(column, SATIN_COLUMN), Some("true"));
    // The column paints the band to its neighbours.
    assert!((out.parts[0].as_ref().unwrap().area() - 60.0).abs() < 1e-6);
    // Planned, it sews and says so.
    let outcome = planned(vec![automatic(band)]);
    assert!(outcome.plan.is_some());
    let said = messages(&outcome);
    assert!(said.contains(&"info SC-I0901: 1 shape is digitized automatically: it sews as a satin column.".to_string()), "{said:?}");
}

#[test]
fn req_auto_016_the_bound_is_the_widest_satin_column_to_the_micrometre() {
    // 7 mm wide, the widest satin column: shrunk by 3.5 mm taken half a micrometre wider, nothing of it is
    // left, so it sews satin columns. 10 micrometres wider is filled.
    let band = |width: f64| shape("band", &rectangle((0.0, 0.0), (30.0, width)));
    assert_eq!(made(&band(7.0)).stitches, Some(Stitches::Satin));
    assert_eq!(made(&band(7.01)).stitches, Some(Stitches::Fill));
}

#[test]
fn req_auto_016_a_column_that_ends_under_another_sews_first_and_reaches_under_it() {
    // A T: a bar 3 mm wide, and a stem 3 mm wide below it. The stem's column ends under the bar's: it sews
    // first, as `t`, and the bar's after it. The T is drawn in 2 strokes (REQ-AUTO-088), and the stem stops
    // against the bar without a push compensation: wider than the pull compensation over tan 20° (1.1 mm),
    // it ends on the bar's side, and the bar's stitches, which run along the stem there, reach 0.4 mm over
    // the seam with their own pull compensation. Until M13.37 the stem was 0.4 mm longer there by its push.
    let t = |stem: f64| {
        let (left, right) = (10.0 - stem / 2.0, 10.0 + stem / 2.0);
        shape("t", &[(0.0, 0.0), (20.0, 0.0), (20.0, 3.0), (right, 3.0), (right, 15.0), (left, 15.0), (left, 3.0), (0.0, 3.0)])
    };
    let out = made(&t(3.0));
    let ids: Vec<&str> = out.elements.iter().map(|e| e.id.as_str()).collect();
    assert_eq!(ids, ["t", "t#2"]);
    let (stem, bar) = (&out.elements[0], &out.elements[1]);
    let (stem_rails, _) = read(stem);
    assert!(stem_rails.iter().flatten().all(|q| q.y() >= 3.0 - 1e-9), "{stem_rails:?}");
    assert_eq!(setting(stem, PUSH_COMPENSATION), None);
    assert_eq!(setting(bar, PUSH_COMPENSATION), None);
    // A narrow stem, 1.5 mm, has no push either (REQ-AUTO-029).
    let narrow = made(&t(1.5));
    assert_eq!(setting(&narrow.elements[0], PUSH_COMPENSATION), None);
    // The stem's end under the bar is the bar's side, from the stem's one side to its other.
    let ends: Vec<Point> = stem_rails.iter().flat_map(|rail| [rail[0], *rail.last().unwrap()]).filter(|q| q.y() < 3.0 + 1e-9).collect();
    assert_eq!(ends.len(), 2, "{stem_rails:?}");
    assert!(ends.iter().any(|q| q.x() == 8.5) && ends.iter().any(|q| q.x() == 11.5), "{ends:?}");
}

#[test]
fn req_auto_016_a_v_sews_as_2_columns_one_ending_under_the_other() {
    // A V whose arms, 3 mm wide, meet in a flat end: its line turns by more than 60° there, and the arms sew
    // as 2 satin columns, the one that ends under the other first, 0.4 mm longer at that end.
    let v = shape("v", &[(0.0, 15.0), (3.0, 15.0), (6.0, 3.0), (9.0, 15.0), (12.0, 15.0), (7.5, 0.0), (4.5, 0.0)]);
    let out = made(&v);
    assert_eq!(out.stitches, Some(Stitches::Satin));
    let [first, second] = out.elements.as_slice() else { panic!("{:?}", out.elements) };
    let under = format!("{}", -rules().auto_pull_compensation_mm.get());
    let push = setting(first, PUSH_COMPENSATION).unwrap();
    assert!(push == format!("{under} 0") || push == format!("0 {under}"), "{push}");
    assert_eq!((setting(first, SATIN_COLUMN), setting(second, SATIN_COLUMN)), (Some("true"), Some("true")));
    assert_eq!(setting(second, PUSH_COMPENSATION), None);
}

#[test]
fn req_auto_016_a_column_too_narrow_for_satin_runs_along_its_line() {
    // A T whose bar is 4 mm wide and whose stem is 1.5 mm wide, in a design whose narrowest satin stroke is
    // 2 mm: the stem's column runs along its line as a triple run, as a stroke that narrow would, and the
    // bar's sews as a satin column. Its 2 strokes are each even and unlike each other, so the T keeps its
    // columns only with `auto_fill_uneven` off: left on, the rules fill the bar and run the stem.
    let outline = [(0.0, 0.0), (20.0, 0.0), (20.0, 4.0), (10.75, 4.0), (10.75, 16.0), (9.25, 16.0), (9.25, 4.0), (0.0, 4.0)];
    let t = fill("t", &[&outline], &RED, &[("auto_fill_uneven", "false")]);
    let settings = DesignSettings { min_satin_stroke_width: Mm::new(2.0).unwrap(), ..DesignSettings::default() };
    let out = digitize(&automatic(t), &settings, &mut Budget::DEFAULT.meter()).unwrap();
    assert_eq!(out.stitches, Some(Stitches::Satin));
    let [stem, bar] = out.elements.as_slice() else { panic!("{:?}", out.elements) };
    let Shape::Stroke { width, .. } = stem.shape else { panic!("{:?}", stem.shape) };
    assert!((width.get() - 1.5).abs() < 0.1, "{width:?}");
    assert_eq!((setting(stem, SATIN_COLUMN), setting(stem, BEAN_STITCH_REPEATS)), (None, Some("1")));
    assert_eq!(setting(bar, SATIN_COLUMN), Some("true"));
}

#[test]
fn req_auto_016_a_letters_legs_sew_alike() {
    // TS-13B's letter A 6 mm tall, its legs 1.08 mm wide: wider than the narrowest satin stroke, 1 mm, so both
    // sew as satin columns, as its crossbar does. The line along one leg ran on into the corner at its foot,
    // where it measures nothing, and its width, averaged along it, came out under 1 mm: that leg sewed as a
    // line beside the other's column, whose line took in the junction at the top. Which leg did turned with
    // the drawing (`tests/transformed.rs`). Measured without its piece into the corner, each leg is a column.
    let design = find("TS-13B").unwrap().design().unwrap().unwrap();
    let a = design.elements().iter().find(|element| element.id.as_str() == "ts13b:auto:word-6:a").unwrap();
    let out = digitize(a, &design.settings, &mut Budget::DEFAULT.meter()).unwrap();
    assert_eq!(out.elements.len(), 3, "{:?}", out.elements);
    for column in &out.elements {
        assert_eq!(setting(column, SATIN_COLUMN), Some("true"), "{}", column.id);
    }
}

#[test]
fn req_auto_016_the_areas_sewn_before_a_column_reach_under_it() {
    // 2 red squares, and a blue band 3 mm wide between them, as a trace whose colours abut draws them. The
    // band sews as a satin column, after both squares, which it borders, and each square reaches the
    // overlap, 1 mm, under it.
    let red = |id: &str, from: (f64, f64), to: (f64, f64)| automatic(shape(id, &rectangle(from, to)));
    let blue = automatic(fill("band", &[&rectangle((10.0, 0.0), (13.0, 10.0))], &BLUE, &[]));
    let design = stitchcraft_engine::design::Design::new(
        unbased(vec![red("left", (0.0, 0.0), (10.0, 10.0)), blue, red("right", (13.0, 0.0), (23.0, 10.0))]),
        DesignSettings::default(),
    )
    .unwrap();
    let (out, _) = stitchcraft_engine::auto::digitized(&design, &Budget::DEFAULT);
    let ids: Vec<&str> = out.iter().map(|e| e.id.as_str()).collect();
    assert_eq!(ids, ["left", "right", "band"]);
    assert_eq!(setting(&out[2], SATIN_COLUMN), Some("true"));
    assert!((area(&out[0]) - 110.0).abs() < 0.12 && (area(&out[1]) - 110.0).abs() < 0.12, "{} {}", area(&out[0]), area(&out[1]));
}

#[test]
fn req_auto_017_a_curved_columns_stitches_lie_square_to_it() {
    // Half a ring 3 mm wide, its middle 10.5 mm from its centre: each stitch across the whole of it runs
    // along the radius through its middle, to within 10°.
    let centre = (20.0, 20.0);
    let ring = |r: f64, from: f64, to: f64| -> Vec<(f64, f64)> {
        (0..=36)
            .map(|i| {
                let (sin, cos) = sin_cos(from + (to - from) * f64::from(i) / 36.0);
                (centre.0 + r * cos, centre.1 + r * sin)
            })
            .collect()
    };
    let mut outline = ring(12.0, 0.0, std::f64::consts::PI);
    outline.extend(ring(9.0, std::f64::consts::PI, 0.0));
    let half = shape("half", &outline);
    assert_eq!(made(&half).stitches, Some(Stitches::Satin));
    let plan = planned(vec![automatic(half)]).plan.unwrap();
    let tops: Vec<Point> = plan.stitches().filter(|s| s.kind == StitchKind::Normal && s.origin.role == Role::Top).map(|s| s.at).collect();
    let radius = |q: Point| math::hypot(q.x() - centre.0, q.y() - centre.1);
    let mut across = 0;
    for pair in tops.windows(2) {
        let (a, b) = (pair[0], pair[1]);
        // A stitch across the whole column, from past one rail to past the other: not one of the contour
        // underlay's, along a rail, nor one shortened on the inside of the curve.
        if (radius(a) - radius(b)).abs() < 3.0 {
            continue;
        }
        across += 1;
        let middle = a.lerp(b, 0.5);
        let (rx, ry) = (middle.x() - centre.0, middle.y() - centre.1);
        let (sx, sy) = (b.x() - a.x(), b.y() - a.y());
        let cosine = (rx * sx + ry * sy).abs() / (math::hypot(rx, ry) * math::hypot(sx, sy));
        assert!(cosine >= math::cos(math::to_radians(10.0)), "{a:?} {b:?}: {}°", math::to_degrees(math::acos(cosine.min(1.0))));
    }
    assert!(across > 50, "{across}");
}

/// The top stitches of the automatic shape `element`, planned alone, each from one needle point to the next.
fn top_stitches(element: &Element) -> Vec<[Point; 2]> {
    let plan = planned(vec![automatic(element.clone())]).plan.unwrap();
    let tops: Vec<Point> = plan.stitches().filter(|s| s.kind == StitchKind::Normal && s.origin.role == Role::Top).map(|s| s.at).collect();
    tops.windows(2).map(|pair| [pair[0], pair[1]]).collect()
}

#[test]
fn req_auto_170_a_column_keeps_the_short_stitches_its_shape_sets() {
    let outline = rectangle((0.0, 0.0), (30.0, 3.0));
    let column = |params: &[(&str, &str)]| made(&fill("band", &[&outline], &RED, params)).elements[0].clone();
    let set =
        |column: &Element| [setting(column, SHORT_STITCH_DISTANCE).map(str::to_string), setting(column, SHORT_STITCH_INSET).map(str::to_string)];
    // A shape that sets either keeps it, and the other is Ink/Stitch's, with either value of
    // `auto_short_stitches` (`REQ-AUTO-173`).
    for value in ["above_limit", "crowded"] {
        assert_eq!(set(&column(&[(SHORT_STITCHES, value), (SHORT_STITCH_DISTANCE, "0.1")])), [Some("0.1".to_string()), None]);
        assert_eq!(set(&column(&[(SHORT_STITCHES, value), (SHORT_STITCH_INSET, "10")])), [None, Some("10".to_string())]);
    }
}

#[test]
fn req_auto_173_crowded_spans_a_straight_column_with_every_stitch_and_moves_a_bend_s_inside_points_in() {
    // With `auto_short_stitches` `crowded`, as before M13.58 (REQ-AUTO-071, retired). Straight, 3 mm wide: a
    // rail's needle points lie 0.4 mm apart, farther than the 0.35 mm that crowds them, so every top stitch
    // crosses the whole column and its pull compensation.
    let shape = |id: &str, outline: &[(f64, f64)]| fill(id, &[outline], &RED, &[(SHORT_STITCHES, "crowded")]);
    let band = shape("band", &rectangle((0.0, 0.0), (30.0, 3.0)));
    let stitches = top_stitches(&band);
    // Its contour underlay, which runs along its rails, has the top's role in a plan: a stitch across is one
    // that runs more across than along, and its ends are left out.
    let across = |[a, b]: &&[Point; 2]| (a.y() - b.y()).abs() > (a.x() - b.x()).abs() && a.x().min(b.x()) > 1.0 && a.x().max(b.x()) < 29.0;
    let short: Vec<&[Point; 2]> = stitches.iter().filter(across).filter(|[a, b]| (a.y() - b.y()).abs() < 3.0).collect();
    assert!(stitches.iter().filter(across).count() > 100);
    assert!(short.is_empty(), "{short:?}");
    // A half ring 3 mm wide whose inside is 4 mm from its centre: the inside rail is half as long as the
    // outside, its points 0.2 mm apart, and every second one is moved in, a quarter of the column and then
    // half of it in turn, so the inside has half the needle points of the outside.
    let centre = (20.0, 20.0);
    let ring = |r: f64, from: f64, to: f64| -> Vec<(f64, f64)> {
        (0..=36)
            .map(|i| {
                let (sin, cos) = sin_cos(from + (to - from) * f64::from(i) / 36.0);
                (centre.0 + r * cos, centre.1 + r * sin)
            })
            .collect()
    };
    let mut outline = ring(7.0, 0.0, std::f64::consts::PI);
    outline.extend(ring(4.0, std::f64::consts::PI, 0.0));
    let half = shape("half", &outline);
    assert_eq!(made(&half).stitches, Some(Stitches::Satin));
    let radius = |q: Point| math::hypot(q.x() - centre.0, q.y() - centre.1);
    let points: Vec<f64> = top_stitches(&half).iter().map(|[a, _]| radius(*a)).collect();
    let inside = points.iter().filter(|r| **r < 4.2).count();
    let outside = points.iter().filter(|r| **r > 6.8).count();
    let between = points.iter().filter(|r| (4.2..6.8).contains(*r)).count();
    assert!(outside > 30 && inside * 2 <= outside + 2 && between * 2 + 4 >= outside, "inside {inside}, between {between}, outside {outside}");
}

#[test]
fn req_auto_075_a_narrow_column_walks_its_middle_once_and_ends_where_it_started() {
    // Below 2 mm, compared to the micrometre, a stroke's column and a part's walk once, unless the shape says
    // how often; wider columns have no centre walk.
    let stroke = |width: f64, params: &[(&str, &str)]| widened(along("line", polylines(&[&[(0.0, 0.0), (30.0, 0.0)]]), &RED, params), width);
    let band = |width: f64, params: &[(&str, &str)]| fill("band", &[&rectangle((0.0, 0.0), (30.0, width))], &RED, params);
    let from = CONTOUR_FROM.get();
    for (width, params, expected) in
        [(1.5, &[][..], Some("1")), (from - 0.001, &[], Some("1")), (from, &[], None), (1.5, &[(CENTER_WALK_UNDERLAY_REPEATS, "3")], Some("3"))]
    {
        for made in [made(&stroke(width, params)), made(&band(width, params))] {
            assert_eq!(setting(&made.elements[0], CENTER_WALK_UNDERLAY_REPEATS), expected, "{width} mm {params:?}");
        }
    }
    // Planned alone, a band 1.5 mm wide and 30 mm long sews its last stitch near where it sewed its first.
    let plan = planned(vec![automatic(band(1.5, &[]))]).plan.unwrap();
    let sewn: Vec<Point> = plan.stitches().filter(|s| s.kind == StitchKind::Normal).map(|s| s.at).collect();
    let (first, last) = (sewn[0], *sewn.last().unwrap());
    assert!(first.distance(last) < 2.0, "{first:?} {last:?}");
}

/// An O traced as an area centred on (`x`, `y`): a ring `side` millimetres across outside, round a round
/// hole `hole` millimetres across, each with 48 sides.
fn ring(id: &str, (x, y): (f64, f64), side: f64, hole: f64) -> Element {
    let circle = |across: f64, turn: f64| -> Vec<(f64, f64)> {
        (0..48)
            .map(|i| {
                let (sin, cos) = sin_cos(turn * std::f64::consts::TAU * f64::from(i) / 48.0);
                (x + across / 2.0 * cos, y + across / 2.0 * sin)
            })
            .collect()
    };
    fill(id, &[&circle(side, 1.0), &circle(hole, -1.0)], &RED, &[])
}

#[test]
fn req_auto_084_a_column_along_a_hole_wide_enough_to_read_has_no_pull_compensation_on_that_side() {
    let reach = rules().auto_pull_compensation_mm.get();
    let pulled = |made: &Digitized| made.elements.iter().map(|column| setting(column, PULL_COMPENSATION).unwrap().to_string()).collect::<Vec<_>>();
    // A ring 6 mm across round a hole a little wider than a disc of 0.8 mm, and wider: its column's inner
    // rail has none, its outer rail the rules'.
    let open = OPEN_HOLE.get();
    for hole in [open + 0.05, 2.0] {
        let made = made(&ring("o", (3.0, 3.0), 6.0, hole));
        assert_eq!(made.stitches, Some(Stitches::Satin), "{hole} mm");
        let sides = pulled(&made);
        assert!(sides.iter().all(|pull| pull == &format!("0 {reach}") || pull == &format!("{reach} 0")), "{hole} mm: {sides:?}");
    }
    // A hole narrower closes under its own threads' width whatever sews round it: the column has the rules'.
    let made = made(&ring("o", (3.0, 3.0), 6.0, open - 0.05));
    assert_eq!(pulled(&made), [reach.to_string()]);
    // A shape drawn later whose box meets the hole's box may fill it or lie over it: the ring reaches under
    // it as before. A line 1 mm wide round the ring's top and right side, its box from 2 mm round the hole's,
    // meets it when it runs down to 1.5 mm, half its width short of the hole's box, and not 0.001 mm higher;
    // and a shape drawn before the ring leaves the hole open.
    let reach_of = |elements: Vec<Element>| {
        let design = Design::new(elements.into_iter().map(automatic).collect(), DesignSettings::default()).unwrap();
        let (made, _) = digitized(&design, &Budget::DEFAULT);
        let columns = made.iter().filter(|element| element.id.as_str().starts_with('o') && setting(element, SATIN_COLUMN) == Some("true"));
        columns.map(|column| setting(column, PULL_COMPENSATION).unwrap().to_string()).collect::<Vec<_>>()
    };
    let round = |to: f64| widened(along("line", polylines(&[&[(-2.0, -2.0), (8.0, -2.0), (8.0, to)]]), &BLUE, &[]), 1.0);
    let o = || ring("o", (3.0, 3.0), 6.0, 2.0);
    assert_eq!(reach_of(vec![o(), round(1.5)]), [reach.to_string()]);
    for elements in [vec![o(), round(1.499)], vec![round(1.5), o()]] {
        let open = reach_of(elements);
        assert!(open.len() == 1 && open[0] != reach.to_string(), "{open:?}");
    }
}

#[test]
fn req_auto_029_a_column_along_a_hole_kept_open_sews_under_a_shape_beside_it_and_keeps_no_pull_compensation_on_the_hole() {
    let reach = rules().auto_pull_compensation_mm.get();
    // A frame 40 mm by 5 round a slot 1 mm high, and a band drawn after it along its top: no shape drawn later
    // meets the slot, which the frame's columns keep open, and the column along the frame's top sews 1 mm
    // under the band. Each column has a length for each rail, the overlap or its own compensation.
    let frame = automatic(fill("frame", &[&rectangle((0.0, 0.0), (40.0, 5.0)), &rectangle((2.0, 3.0), (38.0, 2.0))], &RED, &[]));
    let band = automatic(shape("band", &rectangle((0.0, 5.0), (40.0, 10.0))));
    let design = Design::new(vec![frame.clone(), band.clone()], DesignSettings::default()).unwrap();
    let (made, _) = digitized(&design, &Budget::DEFAULT);
    let pulls: Vec<&str> =
        made.iter().filter(|element| element.id.as_str().starts_with("frame")).filter_map(|column| setting(column, PULL_COMPENSATION)).collect();
    let overlap = rules().auto_overlap_mm.get();
    let (along_hole, under_band) = (format!("{reach} 0"), format!("{overlap} 0"));
    assert_eq!(pulls.len(), 4, "{pulls:?}");
    assert_eq!(pulls.iter().filter(|pull| **pull == along_hole).count(), 3, "{pulls:?}");
    assert_eq!(pulls.iter().filter(|pull| **pull == under_band).count(), 1, "{pulls:?}");
    // The design plans with no setting that doesn't read.
    let outcome = planned(vec![frame, band]);
    assert!(outcome.diagnostics.iter().all(|said| said.code != Code::ParamInvalid), "{:?}", messages(&outcome));
    assert!(outcome.plan.is_some());
}

#[test]
fn req_auto_085_a_column_whose_rails_pair_evenly_moves_its_points_in_no_farther_than_its_edge_and_a_reach() {
    let reach = rules().auto_pull_compensation_mm.get();
    let insets = |element: &Element| setting(element, SHORT_STITCH_INSET).map(str::to_string);
    // A straight band 1.8 mm wide: its first inset is held to the compensation and the reach, in tenths of a
    // percent of its stitch, and the second stays.
    let band = made(&shape("band", &rectangle((0.0, 0.0), (30.0, 1.8))));
    let held = (1000.0 * (reach + EDGE_REACH) / (1.8 + 2.0 * reach)).round() / 10.0;
    assert_eq!(held, 19.2);
    assert_eq!(insets(&band.elements[0]), Some(format!("{held} 50")));
    // A band 1.1 mm wide would be held above a quarter: it keeps the quarter.
    let narrow = made(&shape("narrow", &rectangle((0.0, 0.0), (30.0, 1.1))));
    assert_eq!(insets(&narrow.elements[0]), Some(CROWDED_INSETS.to_string()));
    // A column along a hole kept open is held to the hole's edge, where it has no compensation: by the reach
    // alone.
    let o = made(&ring("o", (3.0, 3.0), 6.0, 2.0));
    let Shape::Stroke { width, .. } = &o.elements[0].shape else { panic!("{o:?}") };
    let width = width.get();
    let edge = (1000.0 * EDGE_REACH / (width + reach)).round() / 10.0;
    assert_eq!(insets(&o.elements[0]), Some(format!("{edge} 50")));
    // A shape that sets how its points move in keeps it.
    let set = made(&fill("set", &[&rectangle((0.0, 0.0), (30.0, 1.8))], &RED, &[(SHORT_STITCH_INSET, "10")]));
    assert_eq!(insets(&set.elements[0]), Some("10".to_string()));
}

#[test]
fn req_auto_086_a_column_that_walks_its_middle_once_starts_at_its_start() {
    let band = |width: f64, params: &[(&str, &str)]| made(&fill("band", &[&rectangle((0.0, 0.0), (30.0, width))], &RED, params));
    let from = CONTOUR_FROM.get();
    for (width, params, expected) in [
        (1.5, &[][..], Some("false")),
        (from - 0.001, &[], Some("false")),
        (from, &[], None),
        (1.5, &[(START_AT_NEAREST_POINT, "true")], Some("true")),
    ] {
        assert_eq!(setting(&band(width, params).elements[0], START_AT_NEAREST_POINT), expected, "{width} mm {params:?}");
    }
}

#[test]
fn req_auto_018_a_column_has_the_underlay_and_settings_of_its_width() {
    // Compared to the micrometre: a centre walk below 2 mm, a contour from 2 mm to 4 mm, and a contour and a
    // zigzag wider, for a stroke's column and for a part's.
    let stroke = |width: f64| widened(along("line", polylines(&[&[(0.0, 0.0), (30.0, 0.0)]]), &RED, &[]), width);
    let band = |width: f64| shape("band", &rectangle((0.0, 0.0), (30.0, width)));
    let walk: &[&str] = &[CENTER_WALK_UNDERLAY];
    let contour: &[&str] = &[CONTOUR_UNDERLAY];
    let both: &[&str] = &[CONTOUR_UNDERLAY, ZIGZAG_UNDERLAY];
    let (from, above) = (CONTOUR_FROM.get(), ZIGZAG_ABOVE.get());
    for (width, expected) in
        [(1.5, walk), (from - 0.001, walk), (from, contour), (3.0, contour), (above, contour), (above + 0.001, both), (5.0, both)]
    {
        for made in [made(&stroke(width)), made(&band(width))] {
            assert_eq!(made.stitches, Some(Stitches::Satin), "{width} mm");
            let column = &made.elements[0];
            assert_eq!(underlays(column), expected, "{width} mm: {}", column.id);
            // The rules' pull compensation, and no end near the next shape.
            assert_eq!(setting(column, PULL_COMPENSATION), Some(rules().auto_pull_compensation_mm.get().to_string().as_str()));
            assert_eq!(setting(column, END_AT_NEAREST_POINT), Some("false"));
        }
    }
}
