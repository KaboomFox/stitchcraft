//! A tatami fill covers its area (`REQ-FILL-TAT-004`): rows from the machine's shortest stitch to a
//! thread's width apart leave none of it showing but a strip along its edges, which the coverage metric
//! (`stitchcraft_testkit::metrics`) measures as an eye sees it. And the reference hoop's whole field plans
//! (`REQ-FILL-TAT-009`), which `cargo xtask perf` times.

// Test code may unwrap and index (clippy.toml allows it in tests).
#![allow(clippy::unwrap_used, clippy::panic, clippy::indexing_slicing)]

use stitchcraft_core::math::sin_cos;
use stitchcraft_core::{Budget, Mm};
use stitchcraft_engine::design::{Design, DesignSettings, Shape};
use stitchcraft_engine::normalize::region::build;
use stitchcraft_engine::plan;
use stitchcraft_plan::profiles::REFERENCE;
use stitchcraft_plan::{MachineProfile, StitchPlan};
use stitchcraft_testkit::designs::{RED, fill, messages, p};
use stitchcraft_testkit::metrics::coverage;

/// A 40 × 30 rectangle.
const RECTANGLE: &[(f64, f64)] = &[(-20.0, -15.0), (-20.0, 15.0), (20.0, 15.0), (20.0, -15.0)];
/// An L, 30 mm across both ways and 10 mm thick.
const ELL: &[(f64, f64)] = &[(-15.0, -15.0), (-15.0, 15.0), (15.0, 15.0), (15.0, 5.0), (-5.0, 5.0), (-5.0, -15.0)];
/// A bar 5 mm wide and 40 mm tall, whose rows at 0° are 5 mm long.
const BAR: &[(f64, f64)] = &[(-2.5, -20.0), (-2.5, 20.0), (2.5, 20.0), (2.5, -20.0)];

/// A fill's rings: its outline, then its holes, each the other way round.
type Rings<'a> = Vec<&'a [(f64, f64)]>;

/// `count` points round a circle of `radius` about the origin, the other way round where `backwards`.
fn circle(radius: f64, count: u32, backwards: bool) -> Vec<(f64, f64)> {
    let turn = if backwards { -1.0 } else { 1.0 };
    (0..count)
        .map(|k| sin_cos(turn * std::f64::consts::TAU * f64::from(k) / f64::from(count)))
        .map(|(sin, cos)| (radius * cos, radius * sin))
        .collect()
}

/// A star of 5 points, 18 mm from its centre, and 8 mm between them.
fn star() -> Vec<(f64, f64)> {
    circle(18.0, 10, false)
        .into_iter()
        .zip(circle(8.0, 10, false))
        .enumerate()
        .map(|(k, (out, inside))| if k % 2 == 0 { out } else { inside })
        .collect()
}

/// Fill `parts`, sewn on `profile`'s machine with Ink/Stitch's defaults and `params`, and its coverage of
/// its own region.
fn sewn_on(profile: &MachineProfile, parts: &[&[(f64, f64)]], params: &[(&str, &str)]) -> (StitchPlan, f64) {
    let element = fill("fill", parts, &RED, params);
    let Shape::Fill { path, rule } = &element.shape else { panic!("a fill has a fill's shape") };
    let region = build(path, *rule, &mut Budget::DEFAULT.meter()).unwrap().region;
    let design = Design::new(vec![element], DesignSettings { origin: Some(p(0.0, 0.0)), ..DesignSettings::default() }).unwrap();
    let outcome = plan(&design, profile, &Budget::DEFAULT);
    assert_eq!(messages(&outcome), Vec::<String>::new());
    let plan = outcome.plan.unwrap();
    let share = coverage(&region, &plan);
    (plan, share)
}

/// The same on the reference machine, the Brother PE800.
fn sewn(parts: &[&[(f64, f64)]], params: &[(&str, &str)]) -> (StitchPlan, f64) {
    sewn_on(REFERENCE, parts, params)
}

#[test]
fn req_fill_tat_004_rows_from_the_shortest_stitch_to_0_4_mm_apart_cover_98_percent_of_the_area() {
    // Top rows alone, without underlay, on the Brother, whose shortest stitch is 0.3 mm: a rectangle, an L,
    // a circle, a ring, a star and a narrow bar, at 5 angles.
    let (round, outer, hole, star) = (circle(15.0, 72, false), circle(15.0, 72, false), circle(6.0, 36, true), star());
    let shapes: [(&str, Rings); 6] = [
        ("rectangle", vec![RECTANGLE]),
        ("L", vec![ELL]),
        ("circle", vec![&round]),
        ("ring", vec![&outer, &hole]),
        ("star", vec![&star]),
        ("bar", vec![BAR]),
    ];
    for (name, parts) in &shapes {
        for spacing in ["0.3", "0.4"] {
            for angle in ["0", "30", "45", "90", "135"] {
                let (_, share) = sewn(parts, &[("row_spacing_mm", spacing), ("angle", angle), ("fill_underlay", "false")]);
                assert!(share >= 0.98, "{name} at {spacing} mm and {angle}°: {share}");
            }
        }
    }
}

#[test]
fn req_fill_tat_004_rows_closer_than_the_shortest_stitch_leave_their_starts_showing() {
    // 0.25 mm apart, Ink/Stitch's default, each row hands over to the next along the bar's sides, which
    // meet the rows square on, in a stitch of 0.25 mm. The Brother's shortest stitch is 0.3 mm, so the next
    // row's first needle point is left out (DEV-FIN-001), the thread runs from the row's end to the next
    // row's second needle point, and a sliver at each row's start shows. The narrow bar has many starts for
    // its area.
    let params = [("row_spacing_mm", "0.25"), ("fill_underlay", "false")];
    let (_, brother) = sewn(&[BAR], &params);
    assert!((0.95..0.98).contains(&brother), "{brother}");
    // On a machine that sews 0.1 mm, as Ink/Stitch's default shortest stitch is, the bar is covered.
    let fine = MachineProfile { min_stitch: Mm::from_tenths(1), ..REFERENCE.clone() };
    let (_, share) = sewn_on(&fine, &[BAR], &params);
    assert!(share >= 0.999, "{share}");
    // A wider part has fewer starts for its area: the rectangle on the Brother.
    let (_, rectangle) = sewn(&[RECTANGLE], &params);
    assert!((0.99..0.995).contains(&rectangle), "{rectangle}");
    // Along an edge at a slant to the rows, their ends lie farther apart than the rows: at 45°, 0.35 mm,
    // which the Brother sews. The rectangle's handovers stay, and it is covered.
    let (_, slanted) = sewn(&[RECTANGLE], &[params.as_slice(), &[("angle", "45")]].concat());
    assert!(slanted >= 0.999, "{slanted}");
}

#[test]
fn req_fill_tat_004_rows_farther_apart_than_the_thread_leave_the_area_showing() {
    // 1 mm apart, a 0.4 mm thread covers about 0.4 of the rectangle; the travel between rows adds a little.
    let (_, share) = sewn(&[RECTANGLE], &[("row_spacing_mm", "1"), ("fill_underlay", "false")]);
    assert!((0.4..0.5).contains(&share), "{share}");
}

#[test]
fn req_fill_tat_009_the_reference_hoop_s_whole_field_plans() {
    // 130 × 180 mm, the Brother PE800's whole field, at 0.4 mm with the default underlay: rows of either
    // layer at the field's edge, rounded a hair past it, still lie within it.
    let field: &[(f64, f64)] = &[(-65.0, -90.0), (-65.0, 90.0), (65.0, 90.0), (65.0, -90.0)];
    let (plan, share) = sewn(&[field], &[("row_spacing_mm", "0.4")]);
    assert!(plan.stats().stitches > 20_000, "{}", plan.stats().stitches);
    assert!(share >= 0.99, "{share}");
}
