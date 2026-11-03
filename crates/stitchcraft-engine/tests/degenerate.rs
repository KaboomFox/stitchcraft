//! The degenerate corpus (`stitchcraft_testkit::degenerate`) sewn every way the engine sews a shape: as a
//! fill by each fill method under each fill rule, as a stroke by each stroke method, narrow and wide, and as
//! a satin column by each satin method (`docs/src/design/algorithms/README.md`, property 5). None may panic
//! or report a bug (`SC-E0009`). Each sews the same when planned again, and a plan of a shape near the
//! origin keeps the machine's invariants. A method not built yet is skipped with its warning, and is run
//! here as soon as it is built.

// Test code may unwrap and panic (clippy.toml allows it in tests).
#![allow(clippy::unwrap_used, clippy::panic)]

use proptest::prelude::*;
use stitchcraft_core::{Budget, Code, ElementId, Mm, Severity};
use stitchcraft_engine::design::{Cap, Design, DesignSettings, Element, FillRule, Join, Path, Segment, Shape, Subpath};
use stitchcraft_engine::generate::{FILL_METHODS, STROKE_METHODS};
use stitchcraft_engine::generators::satin::SATIN_METHODS;
use stitchcraft_engine::plan;
use stitchcraft_params::ParamSet;
use stitchcraft_plan::invariants::check;
use stitchcraft_plan::profiles::REFERENCE;
use stitchcraft_testkit::degenerate::{FAR, corpus, long, random};
use stitchcraft_testkit::designs::{RED, p};

/// Each element the engine sews `path` as, with what it is for a failure to name: a fill by each method under
/// each rule, a stroke by each method 0.5 and 3 mm wide, and a satin column by each method.
fn elements(path: &Path) -> Vec<(String, Element)> {
    let element = |shape: Shape, params: &[(&str, &str)]| Element {
        id: ElementId::new("degenerate").unwrap(),
        name: None,
        shape,
        thread: RED,
        params: params.iter().copied().collect::<ParamSet>(),
        automatic: false,
        hints: Default::default(),
        clips: Vec::new(),
    };
    let stroke = |width: f64| Shape::Stroke { path: path.clone(), width: Mm::new(width).unwrap(), join: Join::UNSET, cap: Cap::Butt };
    let mut all = Vec::new();
    for method in FILL_METHODS {
        for rule in [FillRule::NonZero, FillRule::EvenOdd] {
            let shape = Shape::Fill { path: path.clone(), rule };
            all.push((format!("a fill by {} ({rule:?})", method.id), element(shape, &[("fill_method", method.id)])));
        }
    }
    for method in STROKE_METHODS {
        for width in [0.5, 3.0] {
            all.push((format!("a stroke {width} mm wide by {}", method.id), element(stroke(width), &[("stroke_method", method.id)])));
        }
    }
    for method in SATIN_METHODS {
        let params = [("satin_column", "true"), ("satin_method", method.id)];
        all.push((format!("a satin column by {}", method.id), element(stroke(3.0), &params)));
    }
    all
}

/// What is wrong with sewing `element`, `what` it is, within `budget`: nothing, or why the property fails.
/// `near` says whether its shape lies near the origin, where its plan must fit the reference machine, and
/// `again` whether to plan it a second time, which must sew the same.
fn faults(what: &str, element: &Element, budget: &Budget, near: bool, again: bool) -> Vec<String> {
    let design = match Design::new(vec![element.clone()], DesignSettings::default()) {
        Ok(design) => design,
        // A design the host can't build is refused before it is sewn: nothing to test.
        Err(_) => return Vec::new(),
    };
    let outcome = plan(&design, REFERENCE, budget);
    let mut faults: Vec<String> =
        outcome.diagnostics.iter().filter(|d| d.code == Code::InternalCheckFailed).map(|d| format!("{what}: a bug: {d}")).collect();
    if outcome.plan.is_none() && !outcome.diagnostics.iter().any(|d| d.code.severity() == Severity::Error) {
        faults.push(format!("{what}: no plan, and no error says why: {:?}", outcome.diagnostics));
    }
    if let Some(plan) = &outcome.plan
        && near
    {
        let broken = check(plan, REFERENCE);
        if !broken.is_empty() {
            faults.push(format!("{what}: the plan breaks the machine's invariants: {broken:?}"));
        }
    }
    if again && plan(&design, REFERENCE, budget) != outcome {
        faults.push(format!("{what}: sews otherwise when planned again"));
    }
    faults
}

#[test]
fn every_stitch_type_sews_the_degenerate_corpus_without_a_bug() {
    let mut found = Vec::new();
    for shape in corpus() {
        // The far square lies FAR mm out, past the hoop: its plan isn't the machine's to check.
        let near = shape.path.subpaths.iter().all(|subpath| subpath.start.x() < FAR);
        for (what, element) in elements(&shape.path) {
            found.extend(faults(&format!("{}, {what}", shape.name), &element, &Budget::DEFAULT, near, true));
        }
    }
    assert!(found.is_empty(), "{} faults:\n{}", found.len(), found.join("\n"));
}

#[test]
fn a_path_of_a_million_points_sews_within_its_budget_or_says_why_not() {
    // A thousandth of the default budget's work, which every way of sewing the path runs out of (SC-E0004):
    // each gives a plan or says why not, and none reports a bug.
    let budget = Budget { max_work: Budget::DEFAULT.max_work / 1000, ..Budget::DEFAULT };
    let path = long();
    let mut found = Vec::new();
    let mut ran_out = 0;
    for (what, element) in elements(&path) {
        found.extend(faults(&what, &element, &budget, true, false));
        let design = Design::new(vec![element], DesignSettings::default()).unwrap();
        ran_out += usize::from(plan(&design, REFERENCE, &budget).diagnostics.iter().any(|d| d.code == Code::BudgetExhausted));
    }
    assert!(found.is_empty(), "{found:#?}");
    assert!(ran_out > 0, "the budget is large enough to sew every way, so this case tests nothing");
}

#[test]
fn a_stitch_finalize_keeps_is_still_long_enough_once_the_plan_is_moved_to_the_hoop_s_centre() {
    // Found by the random paths. A satin column whose rails start 1e-9 mm apart ends with a stitch across of
    // 1e-9 mm, whose last point, where the tie-off is anchored, finalize leaves out. The tie-off's first
    // stitch then starts from the point before it, 1e-9 mm short of the lock's 0.2 mm: at the edge of what
    // counts as 0.2 mm. Moving the plan to the hoop's centre rounded it 1e-16 mm shorter, past the edge, and
    // the check refused the plan as a bug (SC-E0009).
    let path = Path {
        subpaths: vec![
            Subpath { start: p(1e-9, 2.0), segments: vec![Segment::Line(p(0.0, 0.0))], closed: true },
            Subpath { start: p(0.0, 2.0), segments: vec![Segment::Line(p(4.000000001, 4.0)), Segment::Line(p(8.0, 0.0))], closed: true },
        ],
    };
    let (_, satin) = elements(&path).into_iter().find(|(what, _)| what == "a satin column by satin_column").unwrap();
    let outcome = plan(&Design::new(vec![satin], DesignSettings::default()).unwrap(), REFERENCE, &Budget::DEFAULT);
    let codes: Vec<Code> = outcome.diagnostics.iter().map(|d| d.code).collect();
    assert!(outcome.plan.is_some() && !codes.contains(&Code::InternalCheckFailed), "{:?}", outcome.diagnostics);
}

proptest! {
    #![proptest_config(stitchcraft_testkit::strategies::config(64))]

    #[test]
    fn every_stitch_type_sews_a_random_path_that_tends_to_degenerate_without_a_bug(path in random()) {
        let found: Vec<String> = elements(&path).iter().flat_map(|(what, element)| faults(what, element, &Budget::DEFAULT, true, false)).collect();
        prop_assert!(found.is_empty(), "{path:?}:\n{}", found.join("\n"));
    }
}
