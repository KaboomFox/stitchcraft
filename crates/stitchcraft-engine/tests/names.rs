//! Names of shapes as planning gives them (`REQ-AUTO-125`; `docs/src/design/image-to-embroidery.md` › Names
//! of shapes), where the engine's own cases, on elements made by hand, can't reach: planning that runs out of
//! work while it names.

// Test code may unwrap.
#![allow(clippy::unwrap_used)]

use stitchcraft_core::{Budget, Code};
use stitchcraft_engine::design::{Design, DesignSettings};
use stitchcraft_engine::plan;
use stitchcraft_plan::profiles::REFERENCE;
use stitchcraft_testkit::designs::{BLUE, RED, fill};

#[test]
fn req_auto_125_naming_that_needs_more_work_than_the_budget_allows_names_nothing_and_says_so() {
    let square = |x: f64| [(x, 0.0), (x + 10.0, 0.0), (x + 10.0, 10.0), (x, 10.0)];
    let elements = vec![fill("red", &[&square(0.0)], &RED, &[]), fill("blue", &[&square(20.0)], &BLUE, &[])];
    let design = Design::new(elements, DesignSettings::default()).unwrap();
    // With the work for each, both shapes are named, and placed.
    let named = plan(&design, REFERENCE, &Budget::DEFAULT);
    let places: Vec<Option<usize>> = named.names.sewing_order().iter().map(|shape| shape.place).collect();
    assert_eq!(places, [Some(1), Some(2)]);
    // A unit of work names nothing: a unit for each shape is needed, and the design is told.
    let starved = plan(&design, REFERENCE, &Budget { max_work: 1, ..Budget::DEFAULT });
    assert!(starved.names.sewing_order().is_empty());
    assert!(starved.diagnostics.iter().any(|said| said.code == Code::BudgetExhausted && said.element.is_none()), "{:?}", starved.diagnostics);
}
