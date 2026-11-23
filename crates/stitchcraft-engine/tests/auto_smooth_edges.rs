//! Smooth satin edges (`REQ-AUTO-170` to `REQ-AUTO-173`, `REQ-SAT-030`, M13.58,
//! `docs/src/design/image-to-embroidery.md` › Smooth satin edges): an automatic satin column's stitches run
//! full length, a bend keeps its spacing on the inside, and the cap pulls a crowded point in only where a spot
//! would otherwise sew above the warning's limit. `auto_short_stitches` `crowded` gives today's columns.

// Test code may unwrap and index (clippy.toml allows it in tests).
#![allow(clippy::unwrap_used, clippy::panic, clippy::indexing_slicing)]

use stitchcraft_core::math::{self, sin_cos};
use stitchcraft_core::{Budget, Code, Point};
use stitchcraft_engine::auto::digitize;
use stitchcraft_engine::design::{Design, DesignSettings, Element};
use stitchcraft_engine::generators::satin::{SHORT_STITCH_DISTANCE, SHORT_STITCH_INSET};
use stitchcraft_engine::testsheets::SHEETS;
use stitchcraft_plan::{Role, StitchPlan};
use stitchcraft_testkit::designs::{RED, along, automatic, fill, in_place, planned, polylines, rules, widened};
use stitchcraft_testkit::fabric;
use stitchcraft_testkit::pictures::{self, Trace};

/// The rule setting of M13.58 and its value for today's columns.
const SHORT_STITCHES: &str = "auto_short_stitches";
/// The satin column's setting of M13.58 (`REQ-SAT-030`).
const BEND_OPEN: &str = "bend_open_percent";

/// The centre of the half rings.
const CENTRE: (f64, f64) = (20.0, 20.0);

/// The points of an arc of radius `r` about [`CENTRE`] from angle `from` to `to`, in 72 straight pieces.
fn arc(r: f64, from: f64, to: f64) -> Vec<(f64, f64)> {
    (0..=72)
        .map(|i| {
            let (sin, cos) = sin_cos(from + (to - from) * f64::from(i) / 72.0);
            (CENTRE.0 + r * cos, CENTRE.1 + r * sin)
        })
        .collect()
}

/// A half ring drawn as an area from radius `inside` to `outside`, automatic, with the settings `params`.
fn half_ring(inside: f64, outside: f64, params: &[(&str, &str)]) -> Element {
    let mut outline = arc(outside, 0.0, std::f64::consts::PI);
    outline.extend(arc(inside, std::f64::consts::PI, 0.0));
    automatic(fill("half", &[&outline], &RED, params))
}

/// How far `q` lies from [`CENTRE`].
fn radius(q: Point) -> f64 {
    math::hypot(q.x() - CENTRE.0, q.y() - CENTRE.1)
}

/// The top stitches of `plan` that run across a ring about [`CENTRE`], their ends' radii differing by more
/// than 0.8 of their length and by more than `least` millimetres, away from the ring's 2 ends, each as its
/// inner and its outer needle point. A stitch along a rail, of the contour underlay, isn't one.
fn across(plan: &StitchPlan, least: f64) -> Vec<[Point; 2]> {
    plan.sewn_stitches()
        .iter()
        .filter(|s| s.role == Role::Top && s.from_role == Role::Top)
        .filter(|s| {
            let rise = (radius(s.from) - radius(s.to)).abs();
            rise > least && rise > 0.8 * s.length()
        })
        .filter(|s| [s.from, s.to].iter().all(|q| q.y() > CENTRE.1 + 3.0))
        .map(|s| if radius(s.from) < radius(s.to) { [s.from, s.to] } else { [s.to, s.from] })
        .collect()
}

/// The distances between the distinct points `points` taken in turn round [`CENTRE`].
fn gaps(points: impl Iterator<Item = Point>) -> Vec<f64> {
    let mut points: Vec<Point> = points.collect();
    let angle = |q: &Point| math::atan2(q.y() - CENTRE.1, q.x() - CENTRE.0);
    points.sort_by(|a, b| angle(a).total_cmp(&angle(b)));
    points.dedup_by(|a, b| a.distance(*b) < 0.01);
    points.windows(2).map(|pair| pair[0].distance(pair[1])).collect()
}

#[test]
fn req_auto_170_a_bend_keeps_its_inside_spacing_and_every_stitch_runs_full_length() {
    // A half ring 3 mm wide whose inside is 8 mm from its centre: its outside would need 0.55 mm to keep the
    // inside at 0.4, so the outside opens to 0.5 mm and the inside lies 0.5 x 8 / 11 apart on the rails, 0.5 x 7.6 / 11.4 at the stitches' ends. No end is
    // moved in: each stitch reaches past both rails by the column's pull compensation, within 0.1 mm. Today
    // every other inside point stops 0.12 mm inside the inside rail, held there by REQ-AUTO-085.
    let plan = planned(vec![half_ring(8.0, 11.0, &[])]).plan.unwrap();
    let stitches = across(&plan, 0.5);
    let reach = rules().auto_pull_compensation_mm.get();
    assert!(stitches.len() > 40, "{}", stitches.len());
    let short: Vec<&[Point; 2]> =
        stitches.iter().filter(|[inner, outer]| radius(*inner) > 8.0 - reach + 0.1 || radius(*outer) < 11.0 + reach - 0.1).collect();
    assert!(short.is_empty(), "{short:?}");
    let outside = gaps(stitches.iter().map(|[_, outer]| *outer));
    let inside = gaps(stitches.iter().map(|[inner, _]| *inner));
    // The stitches' ends lie on the radii widened by the pull compensation: 7.6 and 11.4 mm.
    let ratio = (8.0 - reach) / (11.0 + reach);
    assert!(outside.iter().all(|gap| *gap <= 0.55), "{outside:?}");
    opened(&inside, 0.5 * ratio, 0.4 * ratio);
}

/// Checks that the gaps `inside` lie `wanted` apart on the whole, within the 5 % that a column's pairs may
/// land off their spacing (`pairs`), and each farther apart than `kept`, where they would lie with the
/// spacing kept on the outside.
fn opened(inside: &[f64], wanted: f64, kept: f64) {
    let mean = inside.iter().sum::<f64>() / inside.len() as f64;
    assert!((mean / wanted - 1.0).abs() < 0.05, "{mean} against {wanted}: {inside:?}");
    assert!(inside.iter().all(|gap| *gap > kept * 1.05), "{inside:?}");
}

#[test]
fn req_auto_170_digitizing_sets_no_short_stitches_and_opens_a_bend() {
    let column = |params: &[(&str, &str)]| {
        let outline = [(0.0, 0.0), (30.0, 0.0), (30.0, 3.0), (0.0, 3.0)];
        let made = digitize(&automatic(fill("band", &[&outline], &RED, params)), &DesignSettings::default(), &mut Budget::DEFAULT.meter()).unwrap();
        made.elements[0].clone()
    };
    let setting = |element: &Element, key: &str| element.params.iter().find(|(k, _)| *k == key).map(|(_, value)| value.to_string());
    let default = column(&[]);
    // It moves no point in as it is made: its insets are what the cap pulls a point in by (`REQ-AUTO-171`),
    // the first held to its edge, since a straight band's rails pair evenly (`REQ-AUTO-085`).
    assert_eq!(
        [setting(&default, SHORT_STITCH_DISTANCE), setting(&default, SHORT_STITCH_INSET)],
        [Some("0".to_string()), Some("13.2 50".to_string())]
    );
    assert_eq!(setting(&default, BEND_OPEN).as_deref(), Some("25"));
    // A shape that sets any of the 3 keeps what it sets.
    let own = column(&[(SHORT_STITCH_DISTANCE, "0.1"), (BEND_OPEN, "0")]);
    assert_eq!([setting(&own, SHORT_STITCH_DISTANCE).as_deref(), setting(&own, BEND_OPEN).as_deref()], [Some("0.1"), Some("0")]);
}

/// Each TS-13 sheet's digitized half, by name.
fn sheet(id: &str) -> Design {
    let sheet = SHEETS.iter().find(|sheet| sheet.id == id).unwrap();
    let design = sheet.design().unwrap().unwrap();
    let shapes: Vec<Element> = design.elements().iter().filter(|element| element.automatic).cloned().collect();
    Design::new(shapes, design.settings).unwrap()
}

/// How many top points of satin columns SC-I1119 says the cap pulled in: the number before "points of
/// satin columns" after "pulled in".
fn pulled_in(design: &Design) -> (f64, usize) {
    let outcome = in_place(design);
    let said: Vec<String> = outcome.diagnostics.iter().filter(|d| d.code == Code::AutoThinned).map(|d| d.message.clone()).collect();
    let plan = outcome.plan.unwrap();
    let (thickest, _) = fabric::thickest(&plan, &fabric::columns(design));
    let count = said
        .iter()
        .filter_map(|message| message.split("pulled in ").nth(1))
        .filter_map(|rest| rest.split_whitespace().next())
        .filter_map(|number| number.replace(',', "").parse::<usize>().ok())
        .sum();
    (thickest, count)
}

#[test]
fn req_auto_171_the_cap_pulls_crowded_points_in_where_a_spot_is_above_the_limit() {
    // TS-13A's strip fans into a corner, 6.05 fills with no point moved in, 5.38 with its bend opened: the
    // cap pulls points in there. TS-13C's outline curves gently under 2 hair lines, 4.11 fills with no point
    // moved in: its bend opened or its points pulled in, it sews no spot above the limit.
    let limit = rules().auto_thick_warn_fills;
    let (thickest, count) = pulled_in(&sheet("TS-13A"));
    assert!(thickest <= limit + 1e-9 && count > 0, "TS-13A: {thickest:.2} fills, {count} points pulled in");
    let (thickest, _) = pulled_in(&sheet("TS-13C"));
    assert!(thickest <= limit + 1e-9, "TS-13C: {thickest:.2} fills");
}

#[test]
fn req_auto_172_the_cap_pulls_in_a_tenth_of_today_s_short_stitches_and_says_so() {
    let mut designs: Vec<(String, Design)> = pictures::all()
        .into_iter()
        .chain([Trace::Abutting, Trace::Stacked].into_iter().flat_map(|trace| [pictures::emblem(trace), pictures::button(trace)]))
        .map(|picture| (picture.name, Design::new(picture.elements, DesignSettings::default()).unwrap()))
        .collect();
    designs.push(("pear".to_string(), pictures::pear()));
    designs.push(("lettered-patch".to_string(), pictures::lettered_patch()));
    for id in ["TS-13A", "TS-13B", "TS-13C", "TS-13D", "TS-13E"] {
        designs.push((id.to_string(), sheet(id)));
    }
    let limit = rules().auto_thick_warn_fills;
    let mut total = 0;
    for (name, design) in &designs {
        let (thickest, count) = pulled_in(design);
        assert!(thickest <= limit + 1e-9, "{name}: {thickest:.2} fills");
        total += count;
    }
    // 2,310 points move in at 0.35 mm (2026-11-21), and the cap pulled 155 in on the prototype. With the
    // pull-in below the limit at tight turns and under later shapes (M13.64, REQ-AUTO-195) it pulls in more.
    assert!(total > 0 && total < 231, "{total}");
}

#[test]
fn req_auto_173_crowded_gives_today_s_columns() {
    let outline = [(0.0, 0.0), (30.0, 0.0), (30.0, 3.0), (0.0, 3.0)];
    let made = digitize(
        &automatic(fill("band", &[&outline], &RED, &[(SHORT_STITCHES, "crowded")])),
        &DesignSettings::default(),
        &mut Budget::DEFAULT.meter(),
    )
    .unwrap();
    let column = &made.elements[0];
    let setting = |key: &str| column.params.iter().find(|(k, _)| *k == key).map(|(_, value)| value.to_string());
    // A straight band's rails pair evenly: its first inset is held to its edge (`REQ-AUTO-085`).
    assert_eq!(
        [setting(SHORT_STITCH_DISTANCE), setting(SHORT_STITCH_INSET), setting(BEND_OPEN)],
        [Some("0.35".to_string()), Some("13.2 50".to_string()), None]
    );
    // TS-13C's digitized half plans as on main 1952f48c: 3.92 fills at its thickest, and nothing pulled in.
    let mut ts13c = sheet("TS-13C").elements().to_vec();
    for element in &mut ts13c {
        element.params.set(SHORT_STITCHES, "crowded");
    }
    let (thickest, count) = pulled_in(&Design::new(ts13c, sheet("TS-13C").settings).unwrap());
    assert_eq!(((thickest * 100.0).round() / 100.0, count), (3.92, 0));
}

#[test]
fn req_sat_030_a_bend_opens_its_outside_to_keep_its_inside_spacing() {
    // A satin stroke 3 mm wide along a half circle of 9.5 mm: inside radius 8, outside 11.
    let line = arc(9.5, 0.0, std::f64::consts::PI);
    let column = |params: &[(&str, &str)]| {
        let mut all = vec![("satin_column", "true")];
        all.extend_from_slice(params);
        let plan = planned(vec![widened(along("arc", polylines(&[&line]), &RED, &all), 3.0)]).plan.unwrap();
        let stitches = across(&plan, 0.5);
        (gaps(stitches.iter().map(|[inner, _]| *inner)), gaps(stitches.iter().map(|[_, outer]| *outer)))
    };
    // At 0, as Ink/Stitch: the outside 0.4 mm apart, the inside 0.4 x 8 / 11.
    let (inside, outside) = column(&[]);
    assert!(outside.iter().all(|gap| (gap - 0.4).abs() < 0.04), "{outside:?}");
    assert!(inside.iter().all(|gap| *gap < 0.33), "{inside:?}");
    // At 25 the outside opens to 0.5 mm, and the inside lies 0.5 x 8 / 11 apart on the whole, closer than 0.4 here.
    let (inside, outside) = column(&[(BEND_OPEN, "25")]);
    assert!(outside.iter().all(|gap| *gap <= 0.53), "{outside:?}");
    opened(&inside, 0.5 * 8.0 / 11.0, 0.4 * 8.0 / 11.0);
}
