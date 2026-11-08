//! Underlays first (M10.15, ADR-0018): runs of elements that sew all their underlays before any of their top
//! stitches, through the engine's entry point, [`plan`] (`REQ-ASM-008`, `REQ-ASM-009`, `REQ-ASM-010`), and
//! `SC-I0506`.

// Test code may unwrap and index (clippy.toml allows it in tests).
#![allow(clippy::unwrap_used, clippy::indexing_slicing)]

use stitchcraft_core::{Budget, Code, Point};
use stitchcraft_engine::design::{Design, DesignSettings, Element};
use stitchcraft_engine::plan;
use stitchcraft_engine::route::{Route, Tool, route};
use stitchcraft_engine::testsheets;
use stitchcraft_plan::profiles::REFERENCE;
use stitchcraft_plan::{Role, StitchKind, StitchPlan, Thread};
use stitchcraft_testkit::designs::{BLUE, RED, along, clipped, fill, line, messages, planned, planned_with, polylines};

/// The settings of the crossing columns: satin columns with a contour and a zigzag underlay.
const COLUMN: [(&str, &str); 3] = [("satin_column", "true"), ("contour_underlay", "true"), ("zigzag_underlay", "true")];

/// The setting.
const FIRST: (&str, &str) = ("underlays_first", "true");

/// A satin column `id` 4 mm wide between straight rails, along the line from `from` to `to`, with a rung
/// across its middle, sewn in `thread` with the column's underlays and `more`.
fn column(id: &str, from: (f64, f64), to: (f64, f64), thread: &Thread, more: &[(&str, &str)]) -> Element {
    let (dx, dy) = (to.0 - from.0, to.1 - from.1);
    let length = (dx * dx + dy * dy).sqrt();
    let (nx, ny) = (-dy / length * 2.0, dx / length * 2.0);
    let side = |k: f64| [(from.0 + nx * k, from.1 + ny * k), (to.0 + nx * k, to.1 + ny * k)];
    let middle = ((from.0 + to.0) / 2.0, (from.1 + to.1) / 2.0);
    let rung = [(middle.0 + nx * 1.25, middle.1 + ny * 1.25), (middle.0 - nx * 1.25, middle.1 - ny * 1.25)];
    let (a, b) = (side(1.0), side(-1.0));
    let mut params: Vec<(&str, &str)> = COLUMN.to_vec();
    params.extend_from_slice(more);
    along(id, polylines(&[&a, &b, &rung]), thread, &params)
}

/// 2 columns that cross at (15, 10): `across`, 30 mm long along y = 10, then `down`, along x = 15, each with
/// its own settings.
fn crossing(across: &[(&str, &str)], down: &[(&str, &str)], threads: [&Thread; 2]) -> Vec<Element> {
    vec![column("across", (0.0, 10.0), (30.0, 10.0), threads[0], across), column("down", (15.0, -5.0), (15.0, 25.0), threads[1], down)]
}

/// Each stitch of `plan` that sews, but its locks, as its element's id and its role, in order, a stretch of
/// one element's stitches of one role once.
fn layers(plan: &StitchPlan) -> Vec<(String, Role)> {
    let mut out: Vec<(String, Role)> = Vec::new();
    for stitch in plan.blocks.iter().flat_map(|block| &block.stitches).filter(|s| s.kind == StitchKind::Normal && s.origin.role != Role::Lock) {
        let id = plan.element(stitch.origin.element.unwrap()).unwrap().as_str().to_string();
        if out.last() != Some(&(id.clone(), stitch.origin.role)) {
            out.push((id, stitch.origin.role));
        }
    }
    out
}

/// The needle points of `element`'s stitches of `role` in `plan`, in order.
fn points(plan: &StitchPlan, element: &str, role: Role) -> Vec<Point> {
    let index = plan.elements.iter().position(|id| id.as_str() == element).unwrap();
    let stitches = plan.blocks.iter().flat_map(|block| &block.stitches);
    stitches
        .filter(|s| s.kind == StitchKind::Normal && s.origin.role == role && s.origin.element.map(|e| e.index()) == Some(index))
        .map(|s| s.at)
        .collect()
}

fn owned(pairs: &[(&str, Role)]) -> Vec<(String, Role)> {
    pairs.iter().map(|(id, role)| (id.to_string(), *role)).collect()
}

#[test]
fn req_asm_009_2_columns_that_cross_sew_both_underlays_then_both_tops() {
    let first = planned(crossing(&[FIRST], &[FIRST], [&RED, &RED]));
    assert!(first.diagnostics.is_empty(), "{:?}", messages(&first));
    let plan = first.plan.unwrap();
    let sewn = [("across", Role::Underlay), ("down", Role::Underlay), ("across", Role::Top), ("down", Role::Top)];
    assert_eq!(layers(&plan), owned(&sewn));
    // Each element is one element of the plan, whose stitches are in 2 places.
    assert_eq!(plan.elements.iter().map(|id| id.as_str()).collect::<Vec<_>>(), ["across", "down"]);
    // As drawn, each column's underlay sews just before its top stitches, all of it tagged top.
    let drawn = planned(crossing(&[], &[], [&RED, &RED])).plan.unwrap();
    assert_eq!(layers(&drawn), owned(&[("across", Role::Top), ("down", Role::Top)]));
}

#[test]
fn req_asm_009_each_layer_ends_near_what_the_next_layer_offers() {
    let plan = planned(crossing(&[FIRST], &[FIRST], [&RED, &RED])).plan.unwrap();
    // The first column's underlay ends where the second column crosses it, which the second's underlay
    // starts at, and the second's underlay ends nearest the first column's top, which starts there. The
    // first column's top ends at the crossing again, where the second's top starts.
    let in_down = |q: &Point| (12.9..=17.1).contains(&q.x());
    let in_across = |q: &Point| (7.9..=12.1).contains(&q.y());
    let across_under = points(&plan, "across", Role::Underlay);
    let down_under = points(&plan, "down", Role::Underlay);
    let across_top = points(&plan, "across", Role::Top);
    assert!(in_down(across_under.last().unwrap()), "{:?}", across_under.last());
    assert!(in_down(down_under.first().unwrap()) && in_across(down_under.first().unwrap()), "{:?}", down_under.first());
    assert!(in_across(down_under.last().unwrap()), "{:?}", down_under.last());
    assert!(in_down(across_top.last().unwrap()), "{:?}", across_top.last());
    // Every underlay stitch lies inside its column, and every top stitch of the first column is sewn after
    // every underlay stitch.
    assert!(across_under.iter().all(in_across) && down_under.iter().all(in_down));
}

#[test]
fn req_asm_009_the_top_stitches_take_the_random_numbers_they_take_outside_a_run() {
    // Columns that start and end where they are drawn, their widths varied at random: the first column's top
    // stitches in the run are those it sews after its underlays as drawn.
    let ends = [("start_at_nearest_point", "false"), ("end_at_nearest_point", "false"), ("random_width_decrease_percent", "30")];
    let with = |more: &[(&'static str, &'static str)]| [&ends[..], more].concat();
    let first = planned(crossing(&with(&[FIRST]), &with(&[FIRST]), [&RED, &RED])).plan.unwrap();
    let drawn = planned(crossing(&with(&[]), &with(&[]), [&RED, &RED])).plan.unwrap();
    let (top, whole) = (points(&first, "across", Role::Top), points(&drawn, "across", Role::Top));
    assert!(top.len() > 50 && whole.len() > top.len(), "{} of {}", top.len(), whole.len());
    assert_eq!(top, whole[whole.len() - top.len()..]);
    // Another seed varies them otherwise.
    let seeded = planned(crossing(&with(&[FIRST, ("random_seed", "7")]), &with(&[FIRST]), [&RED, &RED])).plan.unwrap();
    assert_ne!(points(&seeded, "across", Role::Top), top);
}

#[test]
fn req_asm_009_the_run_s_last_element_trims_and_stops_after_its_top_stitches() {
    let trimmed = planned(crossing(&[FIRST], &[FIRST, ("trim_after", "true"), ("stop_after", "true")], [&RED, &RED])).plan.unwrap();
    let entries: Vec<_> = trimmed.blocks.iter().flat_map(|block| &block.stitches).collect();
    let last_top = entries.iter().rposition(|s| s.origin.role == Role::Top).unwrap();
    let commands: Vec<(usize, StitchKind)> = entries.iter().enumerate().filter(|(_, s)| s.kind.is_command()).map(|(i, s)| (i, s.kind)).collect();
    assert_eq!(commands.iter().map(|(_, kind)| *kind).collect::<Vec<_>>(), [StitchKind::Trim, StitchKind::Stop]);
    assert!(commands.iter().all(|(i, _)| *i > last_top), "{commands:?} after {last_top}");
    // Its underlay is followed by no trim: the needle goes on to the first column's top as between any 2
    // groups, with locks and a jump where that is far.
    assert_eq!(layers(&trimmed).len(), 4);
}

#[test]
fn req_asm_008_a_trim_or_a_stop_ends_a_run_after_its_element() {
    // A trim after the first column ends the run there: each column is alone, and sews as without the setting.
    for end in ["trim_after", "stop_after"] {
        let alone = planned(crossing(&[FIRST, (end, "true")], &[FIRST], [&RED, &RED]));
        let drawn = planned(crossing(&[(end, "true")], &[], [&RED, &RED]));
        assert_eq!(alone.plan, drawn.plan, "{end}");
        let said: Vec<Code> = alone.diagnostics.iter().map(|d| d.code).collect();
        assert_eq!(said, [Code::UnderlaysFirstAlone, Code::UnderlaysFirstAlone], "{end}");
    }
}

#[test]
fn req_asm_008_threads_part_runs_compared_by_colour() {
    // Red and blue: each alone, and sewn as without the setting.
    let parted = planned(crossing(&[FIRST], &[FIRST], [&RED, &BLUE]));
    assert_eq!(parted.plan, planned(crossing(&[], &[], [&RED, &BLUE])).plan);
    assert_eq!(parted.diagnostics.iter().map(|d| d.element.as_ref().unwrap().as_str()).collect::<Vec<_>>(), ["across", "down"]);
    // 2 names for one colour are one thread.
    let scarlet = Thread::named(RED.color, "Scarlet");
    let joined = planned(crossing(&[FIRST], &[FIRST], [&RED, &scarlet]));
    assert!(joined.diagnostics.is_empty(), "{:?}", messages(&joined));
    assert_eq!(layers(&joined.plan.unwrap())[..2], owned(&[("across", Role::Underlay), ("down", Role::Underlay)]));
}

#[test]
fn diag_sc_i0506_a_column_alone_sews_as_without_underlays_first() {
    // The second column sets it, the first doesn't: the second joins no run.
    let alone = planned(crossing(&[], &[FIRST], [&RED, &RED]));
    let said: Vec<String> = alone.diagnostics.iter().map(ToString::to_string).collect();
    assert_eq!(
        said,
        ["info SC-I0506: This element sets `underlays_first`, but neither element next to it in sewing order joins it: they don't set it, or \
          sew in another thread, or a trim or a stop parts them. So it sews its underlay just before its top stitches, as it does without the \
          setting."]
    );
    let fix = alone.diagnostics[0].fix.as_ref().map(|fix| fix.describe());
    assert_eq!(fix.as_deref(), Some("Set `underlays_first` on the elements that cross or overlap it, in its thread, or move the trim or the stop."));
    assert_eq!(alone.diagnostics[0].element.as_ref().map(|id| id.as_str()), Some("down"));
    assert_eq!(alone.plan, planned(crossing(&[], &[], [&RED, &RED])).plan);
}

#[test]
fn req_asm_009_a_fill_sews_its_underlay_before_the_satin_border_over_its_edge() {
    // A square filled with its underlay, and a satin column along its top edge, in one thread: the fill's
    // underlay, the column's, then the fill's top rows and the column's top stitches.
    let square = fill("square", &[&[(0.0, 0.0), (20.0, 0.0), (20.0, 20.0), (0.0, 20.0)]], &RED, &[FIRST]);
    let border = column("border", (-2.0, 0.0), (22.0, 0.0), &RED, &[FIRST]);
    let planned_first = planned(vec![square.clone(), border.clone()]);
    // Finalize merges a needle point of the border's start into the fill's last, which it lies beside.
    assert!(planned_first.diagnostics.iter().all(|d| d.code == Code::StitchesMerged), "{:?}", messages(&planned_first));
    let plan = planned_first.plan.unwrap();
    let sewn = [("square", Role::Underlay), ("border", Role::Underlay), ("square", Role::Top), ("border", Role::Top)];
    assert_eq!(layers(&plan), owned(&sewn));
    // The fill's underlay ends near the border, which its underlay starts at.
    let under = points(&plan, "square", Role::Underlay);
    assert!(under.last().unwrap().y() < 3.0, "{:?}", under.last());
    // Each fill method with an underlay sews it apart, and a cross stitch, which has none, sews whole.
    for method in ["contour_fill", "meander_fill", "circular_fill"] {
        let square = Element { params: [FIRST, ("fill_method", method)].into_iter().collect(), ..square.clone() };
        let plan = planned(vec![square, border.clone()]).plan.unwrap();
        assert_eq!(layers(&plan), owned(&sewn), "{method}");
    }
    let crosses = Element { params: [FIRST, ("fill_method", "cross_stitch")].into_iter().collect(), ..square };
    let plan = planned(vec![crosses, border]).plan.unwrap();
    assert_eq!(layers(&plan), owned(&[("border", Role::Underlay), ("square", Role::Top), ("border", Role::Top)]));
}

#[test]
fn req_asm_009_an_element_without_underlay_sews_in_the_second_pass() {
    // A running stitch between 2 columns, all in a run: the columns' underlays, then all 3 in order.
    let run = line("line", (0.0, 30.0), 30.0, &RED, &[FIRST]);
    let [across, down] = <[Element; 2]>::try_from(crossing(&[FIRST], &[FIRST], [&RED, &RED])).unwrap();
    let plan = planned(vec![across, run, down]).plan.unwrap();
    let sewn = [("across", Role::Underlay), ("down", Role::Underlay), ("across", Role::Top), ("line", Role::Top), ("down", Role::Top)];
    assert_eq!(layers(&plan), owned(&sewn));
}

#[test]
fn req_asm_009_a_route_sews_its_pieces_underlays_along_it_then_its_top_stitches() {
    // Routed, the crossing columns are cut into pieces, with runs under the satin between some of them. Each
    // run keeps its column's setting, so that the route is one run: the pieces' underlays in its order, then
    // the pieces and the runs (ADR-0018). The columns' rungs lie away from the crossing.
    let column = |id: &str, a: [(f64, f64); 2], b: [(f64, f64); 2], rung: [(f64, f64); 2]| {
        along(id, polylines(&[&a, &b, &rung]), &RED, &[COLUMN[0], COLUMN[1], COLUMN[2], FIRST])
    };
    let bar = column("bar", [(0.0, 8.0), (30.0, 8.0)], [(0.0, 12.0), (30.0, 12.0)], [(5.0, 7.0), (5.0, 13.0)]);
    let post = column("post", [(13.0, -5.0), (13.0, 25.0)], [(17.0, -5.0), (17.0, 25.0)], [(12.0, 0.0), (18.0, 0.0)]);
    let how = Route { tool: Tool::Satins, options: Tool::Satins.defaults(), start: None, end: None };
    let routed = route(&[bar, post], &how, &DesignSettings::default(), &Budget::DEFAULT);
    assert!(routed.diagnostics.is_empty(), "{:?}", routed.diagnostics);
    assert!(routed.pieces.iter().any(|piece| piece.under), "the route runs under the satin");
    assert!(routed.pieces.iter().all(|piece| piece.element.params.get(FIRST.0) == Some("true")));
    let outcome = planned(routed.pieces.iter().map(|piece| piece.element.clone()).collect());
    assert!(outcome.diagnostics.iter().all(|d| d.code == Code::StitchesMerged), "{:?}", messages(&outcome));
    let pieces = routed.pieces.iter().filter(|piece| !piece.under).map(|piece| (piece.element.id.as_str(), Role::Underlay));
    let sewn: Vec<(&str, Role)> = pieces.chain(routed.pieces.iter().map(|piece| (piece.element.id.as_str(), Role::Top))).collect();
    assert_eq!(layers(&outcome.plan.unwrap()), owned(&sewn));
}

#[test]
fn req_asm_009_an_element_of_a_run_that_sews_nothing_leaves_the_others_as_they_are() {
    let [across, _] = <[Element; 2]>::try_from(crossing(&[FIRST], &[], [&RED, &RED])).unwrap();
    let trimmed = Element { params: across.params.iter().chain([("trim_after", "true")]).collect(), ..across.clone() };
    let sewn_alone = owned(&[("across", Role::Underlay), ("across", Role::Top)]);
    // A column 0.1 mm long and 0.2 mm wide, too small to stitch, left out whole with SC-W0215 before a
    // column with a trim after it.
    let rails = polylines(&[&[(40.0, 0.0), (40.1, 0.0)], &[(40.0, 0.2), (40.1, 0.2)]]);
    let speck = along("speck", rails, &RED, &[COLUMN[0], ("center_walk_underlay", "true"), FIRST]);
    let outcome = planned(vec![speck, trimmed]);
    assert_eq!(outcome.diagnostics.iter().map(|d| d.code).collect::<Vec<_>>(), [Code::SatinTooSmall]);
    assert_eq!(layers(&outcome.plan.unwrap()), sewn_alone);
    // A column with no rails, 2 subpaths of a point each, is skipped with SC-E0201, and its trim with it.
    let none = along("none", polylines(&[&[(40.0, 0.0)], &[(41.0, 0.0)]]), &RED, &[COLUMN[0], COLUMN[1], FIRST, ("trim_after", "true")]);
    let skipped = planned(vec![across.clone(), none]);
    let said: Vec<Code> = skipped.diagnostics.iter().map(|d| d.code).collect();
    assert!(said.contains(&Code::SatinWithoutRails) && said.contains(&Code::TrimOrStopLeftOut), "{said:?}");
    assert_eq!(layers(&skipped.plan.unwrap()), sewn_alone);
    // A fill its clip leaves nothing of sews neither layer, and its trim is left out.
    let fill_params = [FIRST, ("trim_after", "true")];
    let gone = clipped(fill("gone", &[&[(0.0, 40.0), (5.0, 40.0), (5.0, 45.0)]], &RED, &fill_params), &[&[(50.0, 50.0), (60.0, 50.0), (60.0, 60.0)]]);
    let cleared = planned(vec![across, gone]);
    let said: Vec<Code> = cleared.diagnostics.iter().map(|d| d.code).collect();
    assert!(said.contains(&Code::TrimOrStopLeftOut), "{said:?}");
    assert_eq!(layers(&cleared.plan.unwrap()), sewn_alone);
}

#[test]
fn req_asm_009_layers_share_their_element_s_work_and_spend_the_design_s_stitches() {
    // A long column and a short one in a run. Just short of the work the long one needs, its top runs out:
    // it is skipped whole, its underlay too, and the short one sews.
    let elements = vec![column("long", (0.0, 0.0), (80.0, 0.0), &RED, &[FIRST]), column("short", (40.0, -5.0), (40.0, 5.0), &RED, &[FIRST])];
    let design = Design::new(elements, DesignSettings::default()).unwrap();
    let with = |work: u64| plan(&design, REFERENCE, &Budget { max_work: work, ..Budget::DEFAULT });
    let sews_long = |work: u64| with(work).plan.is_some_and(|plan| plan.elements.iter().any(|id| id.as_str() == "long"));
    let (mut low, mut high) = (1_u64, Budget::DEFAULT.max_work);
    while low < high {
        let middle = low + (high - low) / 2;
        if sews_long(middle) { high = middle } else { low = middle + 1 }
    }
    let short_of = with(low - 1);
    let said: Vec<String> = short_of.diagnostics.iter().map(ToString::to_string).collect();
    assert!(said.iter().any(|s| s.starts_with("error SC-E0004: Planning this element needed more than the work budget")), "{said:?}");
    assert_eq!(layers(&short_of.plan.unwrap()), owned(&[("short", Role::Underlay), ("short", Role::Top)]));
    // The design's stitches, which the layers spend in the order they sew: a single stitch ends planning in
    // the first layer, the long column's underlay, which SC-E0004 names.
    let one = plan(&design, REFERENCE, &Budget { max_stitches: 1, ..Budget::DEFAULT });
    let exhausted = one.diagnostics.iter().find(|d| d.code == Code::BudgetExhausted).unwrap();
    assert_eq!((one.plan, exhausted.element.as_ref().map(|id| id.as_str())), (None, Some("long")));
}

#[test]
fn req_asm_010_a_design_without_the_setting_or_with_every_element_alone_plans_as_before() {
    for id in ["TS-05", "TS-06", "TS-07", "TS-08"] {
        let sheet = testsheets::find(id).unwrap();
        let design = sheet.design().unwrap().unwrap();
        let original = plan(&design, sheet.profile, &Budget::DEFAULT);
        let with = |value: &dyn Fn(usize) -> Option<&'static str>| {
            let elements: Vec<Element> = design
                .elements()
                .iter()
                .enumerate()
                .map(|(k, element)| {
                    let mut element = element.clone();
                    if let Some(value) = value(k) {
                        element.params.set("underlays_first", value);
                    }
                    element
                })
                .collect();
            plan(&Design::new(elements, design.settings).unwrap(), sheet.profile, &Budget::DEFAULT)
        };
        // Off on every element: as before, word for word.
        assert_eq!(with(&|_| Some("false")), original, "{id}");
        // On every other element: each alone, sewn as before, and named.
        let alone = with(&|k| (k % 2 == 0).then_some("true"));
        assert_eq!(alone.plan, original.plan, "{id}");
        let notes = alone.diagnostics.iter().filter(|d| d.code == Code::UnderlaysFirstAlone).count();
        assert_eq!(notes, design.elements().len().div_ceil(2), "{id}");
        let others: Vec<_> = alone.diagnostics.into_iter().filter(|d| d.code != Code::UnderlaysFirstAlone).collect();
        assert_eq!(others, original.diagnostics, "{id}");
    }
}

#[test]
fn req_asm_010_a_run_s_elements_sew_as_their_settings_say_within_the_hoop() {
    // A run of the crossing columns with every setting at its default still fits the reference machine:
    // the plan invariants hold.
    let outcome = planned_with(crossing(&[FIRST], &[FIRST], [&RED, &RED]), DesignSettings::default());
    assert!(stitchcraft_plan::invariants::check(&outcome.plan.unwrap(), REFERENCE).is_empty());
}
