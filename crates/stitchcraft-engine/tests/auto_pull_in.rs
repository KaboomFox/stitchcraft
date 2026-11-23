//! The cap below the limit (`REQ-AUTO-194` to `REQ-AUTO-196`, M13.64,
//! `docs/src/design/image-to-embroidery.md` › The cap below the limit): a spot the cap can't lower no longer
//! stops it lowering the others, and with `auto_short_stitches` `knots` it pulls in crowded points of a
//! column's tight turns, and points whose new end a later shape covers, down to the cap, each by its first share
//! only, so that no edge that shows gets a second row of stitch ends.

// Test code may unwrap and index (clippy.toml allows it in tests).
#![allow(clippy::unwrap_used, clippy::panic, clippy::indexing_slicing)]

use stitchcraft_core::Code;
use stitchcraft_core::math::sin_cos;
use stitchcraft_engine::design::{Design, DesignSettings, Element};
use stitchcraft_engine::testsheets::SHEETS;
use stitchcraft_testkit::designs::{RED, along, automatic, in_place, planned, polylines, rules, widened};
use stitchcraft_testkit::fabric;
use stitchcraft_testkit::pictures::{self, Trace};

/// The rule setting of M13.58 that M13.64 gives a third value, and its value for M13.58's columns.
const SHORT_STITCHES: &str = "auto_short_stitches";

/// A line 5 mm wide along x for 17 mm, then 15 mm turned by `turn` degrees, then 17 mm along x again, with the
/// settings `params`: M13.60's corner, which a turn of 60° or less leaves one column.
fn bent_line(turn: f64, params: &[(&str, &str)]) -> Element {
    let (s, c) = sin_cos(turn.to_radians());
    let corner = (22.0 + 15.0 * c, 20.0 + 15.0 * s);
    let points = [(5.0, 20.0), (22.0, 20.0), corner, (corner.0 + 17.0, corner.1)];
    automatic(widened(along("line", polylines(&[&points]), &RED, params), 5.0))
}

/// The thickest spot of `outcome`'s plan, in fills, and how many points SC-I1119 says the cap pulled in.
fn thickest_and_pulled(outcome: &stitchcraft_engine::PlanOutcome) -> (f64, usize) {
    let said = outcome.diagnostics.iter().filter(|d| d.code == Code::AutoThinned).map(|d| d.message.clone());
    let count = said
        .filter_map(|message| message.split("pulled in ").nth(1).map(str::to_string))
        .filter_map(|rest| rest.split_whitespace().next().map(str::to_string))
        .filter_map(|number| number.replace(',', "").parse::<usize>().ok())
        .sum();
    let plan = outcome.plan.as_ref().unwrap();
    (fabric::thickness(plan, &fabric::Limits::DEFAULT).fills.value, count)
}

/// The area of `design` planned in place at `limit` fills or more, in mm².
fn area_at(design: &Design, limit: f64) -> f64 {
    let plan = in_place(design).plan.unwrap();
    fabric::thickness(&plan, &fabric::Limits { warn_fills: limit, ..fabric::Limits::DEFAULT }).thick.area
}

#[test]
#[ignore = "planned: M13.64 builds the cap below the limit"]
fn req_auto_194_a_spot_the_cap_cannot_lower_does_not_stop_it_lowering_the_others() {
    // TS-13A's half set by hand sews 5.31 fills, which the cap may not touch. On main at `acdda98f` the cap
    // stops there: the whole sheet has 39.50 mm² at 4 fills or more, the hand-set half alone 39.25, and the
    // shapes that digitizing made keep spots above 4 that the cap could lower. Going on to the next spot, it
    // takes the whole sheet to 35.25, under what the hand-set half has alone.
    let sheet = SHEETS.iter().find(|sheet| sheet.id == "TS-13A").unwrap();
    let whole = sheet.design().unwrap().unwrap();
    let hand: Vec<Element> = whole.elements().iter().filter(|element| !element.automatic).cloned().collect();
    let hand = Design::new(hand, whole.settings).unwrap();
    let limit = rules().auto_thick_warn_fills;
    let (both, alone) = (area_at(&whole, limit), area_at(&hand, limit));
    assert!(both < alone, "the whole sheet has {both:.2} mm² at {limit} fills or more, its hand-set half {alone:.2}");
}

#[test]
#[ignore = "planned: M13.64 builds the cap below the limit"]
fn req_auto_195_a_column_sewn_round_a_sharp_turn_sews_no_knot() {
    // A 5 mm line turning by 60° stays one column (REQ-AUTO-178) and fans into a knot on the inside of the
    // turn: 3.83 fills on main, where the cap pulls in only above 4. Pulled in at the knot down to the cap, the
    // prototype sewed 3.09.
    let (thickest, pulled) = thickest_and_pulled(&planned(vec![bent_line(60.0, &[])]));
    assert!(thickest <= 3.5 && pulled > 4, "{thickest:.3} fills, {pulled} points pulled in");
}

#[test]
#[ignore = "planned: M13.64 builds the cap below the limit"]
fn req_auto_195_the_engine_s_set_sews_thinner_with_few_points_pulled_in() {
    // REQ-AUTO-073's designs: 100.2 mm² at 3.25 fills or more and 95 points pulled in on main at `acdda98f`.
    // Pulled in at tight turns and under later shapes down to the cap, the area falls by a third or more, no
    // design sews a spot above 4, and the points pulled in stay under 230 (REQ-AUTO-172's tenth of 2,310). The
    // prototype: 56.8 mm² and 204 points.
    let mut designs: Vec<(String, Design)> = pictures::all()
        .into_iter()
        .chain([Trace::Abutting, Trace::Stacked].into_iter().flat_map(|trace| [pictures::emblem(trace), pictures::button(trace)]))
        .map(|picture| (picture.name, Design::new(picture.elements, DesignSettings::default()).unwrap()))
        .collect();
    designs.push(("pear".to_string(), pictures::pear()));
    designs.push(("lettered-patch".to_string(), pictures::lettered_patch()));
    for sheet in SHEETS.iter().filter(|sheet| sheet.digitized()) {
        let design = sheet.design().unwrap().unwrap();
        let shapes: Vec<Element> = design.elements().iter().filter(|element| element.automatic).cloned().collect();
        designs.push((sheet.id.to_string(), Design::new(shapes, design.settings).unwrap()));
    }
    let (cap, limit) = (rules().auto_thick_cap_fills, rules().auto_thick_warn_fills);
    let (mut area, mut total) = (0.0, 0);
    for (name, design) in &designs {
        let outcome = in_place(design);
        let (thickest, pulled) = thickest_and_pulled(&outcome);
        assert!(thickest <= limit + 1e-9, "{name}: {thickest:.3} fills");
        let plan = outcome.plan.unwrap();
        area += fabric::thickness(&plan, &fabric::Limits { warn_fills: cap, ..fabric::Limits::DEFAULT }).thick.area;
        total += pulled;
    }
    assert!(area <= 100.25 * 2.0 / 3.0, "{area:.2} mm² at {cap} fills or more");
    assert!(total > 95 && total < 230, "{total} points pulled in");
}

#[test]
#[ignore = "planned: M13.64 builds the cap below the limit"]
fn req_auto_196_above_limit_keeps_the_columns_of_m13_58() {
    // With `above_limit` the cap pulls points in only above the limit, as on main at `acdda98f`: the 60° corner
    // keeps its 3.83 fills, and the cap pulls in the 4 points it pulls there today. By default it pulls in more.
    let (today, pulled) = thickest_and_pulled(&planned(vec![bent_line(60.0, &[(SHORT_STITCHES, "above_limit")])]));
    assert!((today - 3.826).abs() < 0.005 && pulled == 4, "{today:.3} fills, {pulled} points pulled in");
    let (thickest, more) = thickest_and_pulled(&planned(vec![bent_line(60.0, &[])]));
    assert!(thickest < today - 0.2 && more > pulled, "{thickest:.3} fills, {more} points pulled in");
}
