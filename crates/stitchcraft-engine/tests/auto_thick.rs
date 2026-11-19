//! How thick a design sews (`REQ-AUTO-060` to `REQ-AUTO-062`, `docs/src/design/image-to-embroidery.md` ›
//! How thick a design sews): the measure on fills that the engine sews, and the warning of a design
//! digitized automatically (`SC-W0913`). The measure's own cases, on rows laid by hand, are in
//! `stitchcraft_plan::thickness`.

// Test code may unwrap and index (clippy.toml allows it in tests).
#![allow(clippy::unwrap_used, clippy::panic, clippy::indexing_slicing)]

use stitchcraft_core::{Budget, Code, Diagnostic, Point};
use stitchcraft_engine::PlanOutcome;
use stitchcraft_engine::design::{Design, DesignSettings, Element};
use stitchcraft_plan::Thread;
use stitchcraft_testkit::designs::{BLUE, RED, automatic, fill, in_place, planned_with};
use stitchcraft_testkit::fabric::{self, Limits, Measure, Thickness};

/// A rectangle from (`x`, `y`), `wide` by `tall`, as a fill in `thread` with the settings `params`.
fn rect(id: &str, (x, y): (f64, f64), (wide, tall): (f64, f64), thread: &Thread, params: &[(&str, &str)]) -> Element {
    fill(id, &[&[(x, y), (x + wide, y), (x + wide, y + tall), (x, y + tall)]], thread, params)
}

/// A square 15 mm a side sewn `layers` times, each a fill of its own with a fill's own rows and no
/// underlay, its angle turned 30° from the one before: the ladder of the machine checkpoint's sheet.
fn ladder(layers: usize) -> Design {
    let square = |layer: usize| {
        let angle = (layer * 30).to_string();
        rect(&format!("fill-{layer}"), (0.0, 0.0), (15.0, 15.0), &RED, &[("angle", &angle), ("fill_underlay", "false")])
    };
    Design::new((0..layers).map(square).collect(), DesignSettings::default()).unwrap()
}

/// What was said of how thick the design sews.
fn thick(outcome: &PlanOutcome) -> Vec<&Diagnostic> {
    outcome.diagnostics.iter().filter(|said| said.code == Code::AutoThick).collect()
}

#[test]
fn req_auto_060_a_square_filled_1_2_3_and_6_times_measures_as_many_fills() {
    for layers in [1, 2, 3, 6] {
        let plan = in_place(&ladder(layers)).plan.unwrap();
        let thickness = Thickness::of(&plan, &mut Budget::DEFAULT.meter()).unwrap().unwrap();
        // The spots 2 mm and more inside the square.
        let spots = (0..23).flat_map(|i| (0..23).map(move |j| Point::new(2.0 + f64::from(i) * 0.5, 2.0 + f64::from(j) * 0.5).unwrap()));
        let fills: Vec<f64> = spots.map(|at| thickness.at(Measure::Fills, at)).collect();
        let (least, mean) = (fills.iter().copied().fold(f64::INFINITY, f64::min), fills.iter().sum::<f64>() / fills.len() as f64);
        // Measured on 2026-11-15: the least is 0.89 to 0.97 of the layers and the mean 1.03, since a
        // fill fits its rows to its shape, and the needle also travels under the rows it has sewn.
        let layers = layers as f64;
        assert!(least >= 0.85 * layers && least <= layers, "{layers} layers: the least is {least}");
        assert!(mean >= 0.98 * layers && mean <= 1.06 * layers, "{layers} layers: the mean is {mean}");
    }
}

#[test]
fn diag_sc_w0913_a_design_digitized_automatically_is_told_where_it_sews_thick_and_is_written() {
    // 2 areas that meet: the first reaches 1 mm under the second, over the base layer, and each is a fill
    // with rows 0.25 mm apart, 1.6 fills of thread.
    let shapes = |params: &[(&str, &str)]| {
        let left = automatic(rect("left", (0.0, 0.0), (20.0, 15.0), &RED, params));
        let right = automatic(rect("right", (20.0, 0.0), (20.0, 15.0), &BLUE, &[]));
        Design::new(vec![left, right], DesignSettings::default()).unwrap()
    };
    // The plan is measured as planning leaves it, where a machine file has it: the measure goes by the
    // machine's grid, which a plan moved back to its shapes meets elsewhere.
    let planned = |design: &Design| planned_with(design.elements().to_vec(), design.settings);
    // Planning says what the measure finds. Measured on 2026-11-15: where the second area starts on the
    // overlap, its lock and its first rows lie on the first area's 1 mm and the base layer, 5.2 fills in
    // 1.5 mm², so even 2 areas warn there.
    let outcome = planned(&shapes(&[]));
    let report = fabric::thickness(outcome.plan.as_ref().unwrap(), &Limits::DEFAULT);
    assert!(outcome.plan.is_some() && thick(&outcome).len() == usize::from(report.warns()), "{:?}", thick(&outcome));
    assert!(report.fills.value > 2.0 && report.thick.area < 3.0 && report.dense.pieces == 0, "{report:?}");
    // Within its limits nothing is said of a design.
    let raised = planned(&shapes(&[("auto_thick_warn_fills", "9")]));
    assert!(raised.plan.is_some() && thick(&raised).is_empty(), "{:?}", thick(&raised));
    // The design goes by the limits of its lowest automatic shape: held to 2 fills, the overlap is thick. Above
    // that limit the cap takes more (`REQ-AUTO-112`), so the warning names what is left.
    let outcome = planned(&shapes(&[("auto_thick_warn_fills", "2")]));
    let said = thick(&outcome);
    assert!(outcome.plan.is_some() && said.len() == 1, "{:?}", outcome.diagnostics);
    let left = fabric::thickness(outcome.plan.as_ref().unwrap(), &Limits::DEFAULT).fills.value;
    assert!(left > 2.0 && left <= report.fills.value, "{left} {}", report.fills.value);
    assert!(said[0].message.starts_with(&format!("Thread is up to {left:.1} fills thick at ")), "{}", said[0].message);
    // The shapes stacked there are named as the Layers panel shows them, before their ids (`REQ-AUTO-127`).
    let left = outcome.names.name_of("left").unwrap();
    assert!(said[0].message.contains(&format!("({left:?}, left)")) && said[0].message.contains(", right)"), "{}", said[0].message);
    // And held to 2 needle points in a square millimetre, the ends of the rows are.
    assert_eq!(thick(&planned(&shapes(&[("auto_thick_warn_points", "2")]))).len(), 1);
}

#[test]
fn diag_sc_i1119_the_last_pass_says_what_it_took_and_what_is_left() {
    // The traced pear sews spots above 4 fills, where the pass takes thread (`REQ-AUTO-074`). The lettered
    // patch did until M13.37, when its letters came to sew as strokes (REQ-AUTO-087) and no spot of it was
    // left above 4.
    let outcome = in_place(&stitchcraft_testkit::pictures::pear());
    let plan = outcome.plan.as_ref().unwrap();
    let said: Vec<&Diagnostic> = outcome.diagnostics.iter().filter(|said| said.code == Code::AutoThinned).collect();
    assert_eq!(said.len(), 1, "{:?}", outcome.diagnostics);
    let report = fabric::thickness(plan, &Limits::DEFAULT);
    let message = &said[0].message;
    assert!(message.contains(" fills or more. The last pass took ") && message.contains(" in "), "{message}");
    assert!(message.contains(&format!("the thickest spot, {:.1} fills at ", report.fills.value)), "{message}");
    // The warning, where a place of 1 mm² is at its limit of 4 fills (`REQ-AUTO-113`), gives the plan as the pass
    // left it.
    let warned = thick(&outcome);
    let thickest = format!("Thread is up to {:.1} fills thick at ", report.fills.value);
    assert!(warned.len() <= 1 && warned.iter().all(|warned| warned.message.starts_with(&thickest)), "{warned:?}");
    // A design whose spots are all within the cap is not thinned, and nothing is said of a pass.
    let thin = in_place(&Design::new(vec![automatic(rect("one", (0.0, 0.0), (20.0, 15.0), &RED, &[]))], DesignSettings::default()).unwrap());
    assert!(thin.plan.is_some() && thin.diagnostics.iter().all(|said| said.code != Code::AutoThinned), "{:?}", thin.diagnostics);
}

#[test]
fn req_auto_062_a_design_without_an_automatic_shape_is_not_measured() {
    let outcome = in_place(&ladder(6));
    assert!(outcome.plan.is_some() && thick(&outcome).is_empty(), "{:?}", thick(&outcome));
    assert!(fabric::thickness(outcome.plan.as_ref().unwrap(), &Limits::DEFAULT).warns());
}

#[test]
fn req_auto_067_where_3_areas_meet_the_thread_as_sewn_is_one_overlap_deep() {
    // 3 areas that meet at (10, 10): 2 squares, one above the other, and a bar to the right of both. The bar
    // sews first and reaches 1 mm under both squares, across their rows. The upper square sews next, and
    // reaches 0.4 mm under the lower one, along its rows, but not where the bar already does.
    let gold = Thread::new(stitchcraft_plan::Rgb::new(200, 160, 0));
    let shapes = vec![
        automatic(rect("a", (0.0, 0.0), (10.0, 10.0), &RED, &[])),
        automatic(rect("b", (0.0, 10.0), (10.0, 10.0), &BLUE, &[])),
        automatic(rect("c", (10.0, 0.0), (10.0, 20.0), &gold, &[])),
    ];
    let plan = in_place(&Design::new(shapes, DesignSettings::default()).unwrap()).plan.unwrap();
    assert_eq!(plan.elements.iter().map(ToString::to_string).collect::<Vec<_>>(), ["stitchcraft:base", "c", "a", "b"]);
    let thickness = Thickness::of(&plan, &mut Budget::DEFAULT.meter()).unwrap().unwrap();
    let at = |x: f64, y: f64| thickness.at(Measure::Fills, Point::new(x, y).unwrap());
    let halves = |from: i32, to: i32| (from..=to).map(|k| f64::from(k) * 0.5);
    // Measured on 2026-11-16, as sewn. Along the bar's edge with the upper square, half a millimetre inside
    // the square: its fill, the bar's overlap under it and the base layer's one layer, 2.1 fills.
    let seam: Vec<f64> = halves(4, 16).map(|y| at(9.5, y)).collect();
    assert!(seam.iter().all(|fills| *fills > 1.9 && *fills < 2.3), "{seam:?}");
    // In the bar beside that edge, where no overlap lies: its fill, the ends of its rows and its outline.
    let beside: Vec<f64> = halves(6, 14).map(|y| at(10.5, y)).collect();
    assert!(beside.iter().all(|fills| *fills > 1.3 && *fills < 1.7), "{beside:?}");
    // At the corner, 3.8 fills at the most as sewn on 2026-11-16: the 2 squares, the bar's overlap under them,
    // the upper square's 0.4 mm under the lower one beside it, and their 2 outlines. A second whole overlap
    // there would make it 4.8. The cap of 3.25 fills (M13.46) takes it to 3.24.
    let corner = halves(18, 20).flat_map(|x| halves(18, 22).map(move |y| (x, y))).map(|(x, y)| at(x, y)).fold(0.0, f64::max);
    assert!(corner > 3.0 && corner < 4.1, "{corner}");
}

#[test]
fn req_auto_060_test_sheet_ts13f_s_squares_measure_the_fills_their_places_name() {
    // The ladder that a sew-out sets the limits with: its squares of 1 to 6 fills, each on one open layer.
    let plan = stitchcraft_engine::testsheets::find("TS-13F").unwrap().plan().unwrap();
    let thickness = Thickness::of(&plan, &mut Budget::DEFAULT.meter()).unwrap().unwrap();
    // The sheet is 112 by 57 mm round the plan's origin, and its squares are 15 mm a side, 19 mm apart.
    let mean = |left: f64, top: f64| {
        let spots = (0..23)
            .flat_map(|i| (0..23).map(move |j| Point::new(left - 56.0 + 2.0 + f64::from(i) * 0.5, top - 28.5 + 2.0 + f64::from(j) * 0.5).unwrap()));
        let fills: Vec<f64> = spots.map(|at| thickness.at(Measure::Fills, at)).collect();
        fills.iter().sum::<f64>() / fills.len() as f64
    };
    for fills in 1..=6 {
        let (left, layers) = (1.0 + f64::from(fills - 1) * 19.0, f64::from(fills));
        // Measured on 2026-11-16: the base layer is 0.10 of a fill, and the fills on it 1.03 each.
        for (row, top) in [("top", 0.0), ("middle", 22.0)] {
            let mean = mean(left, top);
            assert!(mean >= 0.98 * layers + 0.05 && mean <= 1.06 * layers + 0.15, "{row} row, {layers} fills: the mean is {mean}");
        }
    }
    // The outline adds its thread along the edge, so the thickest spot of the sheet is on the last outlined square.
    let report = fabric::thickness(&plan, &Limits::DEFAULT);
    println!("TS-13F: thickest {:.2} at {:?}, thick {:?}, failing {:?}", report.fills.value, report.fills.at, report.thick, report.failing);
    assert!(report.fills.value > 6.0 && report.fills.at.x() > 38.0 && report.fills.at.y() > -8.5, "{:?}", report.fills);
}
