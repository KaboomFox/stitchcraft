//! A small detail on top stays on top (`REQ-AUTO-182`, M13.61): a small disc with a dot drawn on it, as an eye
//! with its pupil, sews whole in rows under the dot, and the dot sews after it and shows.
//!
//! Design: `docs/src/design/image-to-embroidery.md` › A small detail on top. Each case draws its shapes in
//! code, after the review set's fish: a broad body, a white disc 4.4 mm across on it and a black dot 1.8 mm
//! across on the disc, stacked as a picture traced with its colours stacked is. As the layers are flattened
//! (`REQ-AUTO-023`) the disc shows as a ring round the dot. Before M13.61 that ring sewed as one satin column
//! round the dot: its stitches all ran in to the dot's edge, a wheel, and the dot sewed before it, so the
//! wheel's ends covered it.

// Test code may unwrap and index (clippy.toml allows it in tests).
#![allow(clippy::unwrap_used, clippy::panic, clippy::indexing_slicing)]

use stitchcraft_core::Budget;
use stitchcraft_core::math::{atan2, sin_cos};
use stitchcraft_engine::auto::digitized;
use stitchcraft_engine::design::{Design, DesignSettings, Element};
use stitchcraft_plan::{Rgb, Role, Thread};
use stitchcraft_testkit::designs::{automatic, fill, planned, rules_said};

type Pt = (f64, f64);

const GOLD: Thread = Thread::new(Rgb::new(232, 169, 0));
const WHITE: Thread = Thread::new(Rgb::new(244, 241, 232));
const BLACK: Thread = Thread::new(Rgb::new(26, 26, 26));

/// The ellipse about `centre` with half axes `rx` and `ry`, as `n` points, anticlockwise.
fn ellipse(centre: Pt, rx: f64, ry: f64, n: usize) -> Vec<Pt> {
    (0..n)
        .map(|k| {
            let (s, c) = sin_cos(std::f64::consts::TAU * k as f64 / n as f64);
            (centre.0 + rx * c, centre.1 + ry * s)
        })
        .collect()
}

/// An automatic area `id` of `rings` sewn with `thread`.
fn area(id: &str, rings: &[&[Pt]], thread: &Thread) -> Element {
    automatic(fill(id, rings, thread, &[]))
}

/// The eye's centre, its radius, the pupil's centre and the pupil's radius, as the review set's fish draws them.
const EYE: (Pt, f64) = ((16.0, 27.5), 2.2);
const PUPIL: (Pt, f64) = ((15.6, 27.5), 0.9);

/// A gold body 34 mm by 18 mm, a white eye on it and a black pupil on the eye, each drawn whole and stacked.
fn fish() -> Vec<Element> {
    vec![
        area("body", &[&ellipse((26.0, 30.0), 17.0, 9.0, 96)], &GOLD),
        area("eye", &[&ellipse(EYE.0, EYE.1, EYE.1, 64)], &WHITE),
        area("pupil", &[&ellipse(PUPIL.0, PUPIL.1, PUPIL.1, 48)], &BLACK),
    ]
}

/// What `SC-I0912` says of the shape `id` of `elements`, planned.
fn rule_of(elements: Vec<Element>, id: &str) -> String {
    let said = rules_said(&planned(elements));
    said.into_iter().find(|(element, _)| element.starts_with(id)).map(|(_, message)| message).unwrap_or_default()
}

/// The ids of the elements that `elements` sew as, in the order they sew.
fn order(elements: Vec<Element>) -> Vec<String> {
    let design = Design::new(elements, DesignSettings::default()).unwrap();
    let (out, _) = digitized(&design, &Budget::DEFAULT);
    out.iter().map(|element| element.id.as_str().to_string()).collect()
}

/// How far `at` lies from the segment from `a` to `b`.
fn distance(at: Pt, a: Pt, b: Pt) -> f64 {
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    let along = (dx * dx + dy * dy).max(1e-12);
    let t = (((at.0 - a.0) * dx + (at.1 - a.1) * dy) / along).clamp(0.0, 1.0);
    ((a.0 + t * dx - at.0).powi(2) + (a.1 + t * dy - at.1).powi(2)).sqrt()
}

/// The share of the spots of a 0.1 mm grid within `inside` mm of `centre` where the thread on top, the last
/// sewn stitch that passes within half a thread's width, 0.2 mm, is `thread`, in the plan of `elements`.
fn seen(elements: Vec<Element>, centre: Pt, inside: f64, thread: &Thread) -> f64 {
    let plan = planned(elements).plan.unwrap();
    let sewn: Vec<(Pt, Pt, Rgb)> =
        plan.sewn_stitches().iter().map(|s| ((s.from.x(), s.from.y()), (s.to.x(), s.to.y()), plan.blocks[s.block].thread.color)).collect();
    let (mut spots, mut shown) = (0_u32, 0_u32);
    let steps = (inside / 0.1).floor() as i32;
    for i in -steps..=steps {
        for j in -steps..=steps {
            let at = (centre.0 + f64::from(i) * 0.1, centre.1 + f64::from(j) * 0.1);
            if (at.0 - centre.0).hypot(at.1 - centre.1) > inside {
                continue;
            }
            spots += 1;
            let top = sewn.iter().rev().find(|(a, b, _)| distance(at, *a, *b) <= 0.2);
            shown += u32::from(top.is_some_and(|(_, _, colour)| *colour == thread.color));
        }
    }
    f64::from(shown) / f64::from(spots.max(1))
}

/// The share of the top stitches of `thread` longer than 0.5 mm, in the plan of `elements`, that run within
/// 20 degrees of their commonest way: rows run one way, and a wheel's stitches every way.
fn rows(elements: Vec<Element>, thread: &Thread) -> f64 {
    let plan = planned(elements).plan.unwrap();
    let ways: Vec<f64> = plan
        .sewn_stitches()
        .iter()
        .filter(|s| s.role == Role::Top && plan.blocks[s.block].thread.color == thread.color)
        .filter(|s| (s.to.x() - s.from.x()).hypot(s.to.y() - s.from.y()) > 0.5)
        .map(|s| atan2(s.to.y() - s.from.y(), s.to.x() - s.from.x()).to_degrees().rem_euclid(180.0))
        .collect();
    let near = |way: f64, of: f64| ((way - of + 90.0).rem_euclid(180.0) - 90.0).abs() <= 20.0;
    let most = (0..90).map(|k| ways.iter().filter(|&&way| near(way, f64::from(k) * 2.0)).count()).max().unwrap_or(0);
    most as f64 / ways.len().max(1) as f64
}

#[test]
fn req_auto_182_a_disc_with_a_dot_on_it_sews_whole_under_the_dot() {
    // Shown, the eye is a ring round the pupil 0.9 to 1.7 mm wide, whose column would sew 2.4 mm of its
    // outer rail to 1 mm of its inner one. Held to the crowding bound, it is no column: its hole closes, and
    // the eye, a disc 4.4 mm across, is compact and filled whole.
    let rule = rule_of(fish(), "eye");
    assert!(rule.contains("compact"), "the eye is filled whole: {rule}");
}

#[test]
fn req_auto_182_the_dot_sews_after_the_disc_and_shows() {
    let ids = order(fish());
    let at = |id: &str| ids.iter().position(|sewn| sewn.starts_with(id)).unwrap();
    assert!(at("pupil") > at("eye"), "the pupil sews after the eye: {ids:?}");
    // Measured on main 1952f48c, the white thread lay over 60 % of the pupil.
    let share = seen(fish(), PUPIL.0, PUPIL.1 - 0.2, &BLACK);
    assert!(share >= 0.9, "black shows on {share:.2} of the pupil");
}

#[test]
fn req_auto_182_the_discs_stitches_run_in_rows_not_a_wheel() {
    // Measured on main 1952f48c: 0.26 of the eye's stitches ran one way, a wheel; filled, its rows do.
    let share = rows(fish(), &WHITE);
    assert!(share >= 0.6, "{share:.2} of the eye's stitches run one way");
}

#[test]
fn req_auto_182_a_ring_that_doesnt_crowd_keeps_its_column() {
    // A white ring 3 mm wide round a black disc 10 mm across, on the body: its column sews 1.6 mm of its outer
    // rail to 1 mm of its inner one, within the bound. It stays an outline round the disc, as before.
    let elements = vec![
        area("body", &[&ellipse((26.0, 30.0), 17.0, 12.0, 96)], &GOLD),
        area("ring", &[&ellipse((26.0, 30.0), 8.0, 8.0, 96)], &WHITE),
        area("disc", &[&ellipse((26.0, 30.0), 5.0, 5.0, 96)], &BLACK),
    ];
    let rule = rule_of(elements, "ring");
    assert!(rule.contains("column"), "the ring keeps its column: {rule}");
}

#[test]
fn req_auto_182_a_border_round_a_field_on_top_keeps_its_column() {
    // A red plate with a cream field 3 mm inside it stacked on top, as a patch is drawn: shown, the plate is a
    // border 3 mm wide round the field. A square border crowds a stretch at each corner, more than 2 mm of its
    // outer edge to 1 mm of its inner one, but the whole way round it is 1.11 times as long outside as inside,
    // and a round one 1.15 times: no wheel. Both stay a satin column round the field.
    let red = Thread::new(Rgb::new(200, 0, 0));
    // The square's corners are cut, 4.5 mm on the plate and less on the field, so that the border is 3 mm
    // wide all round, as the patches drawn in the repository are.
    let square = |id: &str, inset: f64, thread: &Thread| {
        let (a, b, c) = (inset, 70.0 - inset, 40.0 - inset);
        let cut = 4.5 - inset * (std::f64::consts::FRAC_PI_8).tan();
        area(id, &[&[(a + cut, a), (b - cut, a), (b, a + cut), (b, c - cut), (b - cut, c), (a + cut, c), (a, c - cut), (a, a + cut)]], thread)
    };
    let shapes = [
        ("square", vec![square("border", 0.0, &red), square("field", 3.0, &WHITE)]),
        (
            "round",
            vec![area("border", &[&ellipse((35.0, 35.0), 23.0, 23.0, 96)], &red), area("field", &[&ellipse((35.0, 35.0), 20.0, 20.0, 96)], &WHITE)],
        ),
    ];
    for (name, elements) in shapes {
        let rule = rule_of(elements, "border");
        assert!(rule.contains("column"), "{name}: the border keeps its column: {rule}");
    }
}

#[test]
fn req_auto_182_a_disc_asked_to_sew_as_satin_keeps_its_column() {
    // The eye set to sew as satin columns (`auto_stitch`): it is sewn as asked, a column round the pupil,
    // crowded or not.
    let mut elements = fish();
    elements[1] = automatic(fill("eye", &[&ellipse(EYE.0, EYE.1, EYE.1, 64)], &WHITE, &[("auto_stitch", "satin")]));
    let rule = rule_of(elements, "eye");
    // Shown, it is still the ring round the pupil, 1.29 mm wide on average, sewn along its one centre line.
    assert!(rule.contains("rule asked") && rule.contains("1 centre line"), "the eye sews as asked: {rule}");
}

#[test]
fn req_auto_182_a_ring_round_nothing_keeps_its_column() {
    // The eye's white as a ring of its own on bare fabric: no area fills its hole, so nothing sews on top, and
    // a fill in its place would sew in pieces round the hole. It keeps its column, crowded or not, as before.
    let hole: Vec<Pt> = ellipse(PUPIL.0, PUPIL.1, PUPIL.1, 48).into_iter().rev().collect();
    let elements = vec![area("eye", &[&ellipse(EYE.0, EYE.1, EYE.1, 64), &hole], &WHITE)];
    let rule = rule_of(elements, "eye");
    assert!(rule.contains("column"), "the ring keeps its column: {rule}");
}
