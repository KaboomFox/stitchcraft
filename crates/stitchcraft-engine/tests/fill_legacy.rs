//! Ink/Stitch's legacy fill is sewn as a tatami fill without underlay (`REQ-FILL-005`, `DEV-FILL-014`).
//! Ink/Stitch keeps the method for its older files, and gives it to a fill whose `auto_fill` is off.
//! StitchCraft does not build it. Its rows are a tatami fill's, and it sews no underlay, so a tatami fill
//! without underlay sews such a fill nearly as Ink/Stitch does. Skipping it would leave a hole in the
//! design.

// Test code may unwrap (clippy.toml allows it in tests).
#![allow(clippy::unwrap_used)]

use stitchcraft_engine::PlanOutcome;
use stitchcraft_testkit::designs::{BLUE, RED, fill, messages, planned, shape_of};

/// A square 10 mm across, left of the fill under test, sewn before it.
const BEFORE: &[(f64, f64)] = &[(-30.0, 0.0), (-30.0, 10.0), (-20.0, 10.0), (-20.0, 0.0)];
/// A rectangle 20 × 15 mm.
const RECTANGLE: &[(f64, f64)] = &[(0.0, 0.0), (0.0, 15.0), (20.0, 15.0), (20.0, 0.0)];
/// Settings a tatami fill reads and a legacy fill shares with it, none of them their defaults.
const SHARED: &[(&str, &str)] =
    &[("angle", "30"), ("row_spacing_mm", "0.4"), ("staggers", "3"), ("skip_last", "true"), ("max_stitch_length_mm", "2.5")];

/// A tatami fill, then a fill sewn with `method`, `SHARED` and `own` settings.
fn after_a_fill(method: &str, own: &[(&str, &str)]) -> PlanOutcome {
    let mut params = vec![("fill_method", method)];
    params.extend_from_slice(SHARED);
    params.extend_from_slice(own);
    planned(vec![fill("before", &[BEFORE], &RED, &[]), fill("rectangle", &[RECTANGLE], &BLUE, &params)])
}

/// How many stitches `outcome`'s plan has.
fn stitches(outcome: &PlanOutcome) -> usize {
    outcome.plan.as_ref().unwrap().blocks.iter().map(|block| block.stitches.len()).sum()
}

/// That `one` and `other` planned alike; their shapes and `one`'s messages when they did not.
fn same(one: &PlanOutcome, other: &PlanOutcome) {
    assert!(one.plan == other.plan, "{}\n{}\n{:?}", shape_of(one), shape_of(other), messages(one));
}

#[test]
fn req_fill_005_a_legacy_fill_is_sewn_as_a_tatami_fill_without_underlay() {
    // Its underlay set on, which Ink/Stitch's legacy fill never sews. The same plan as a tatami fill's
    // without: its own settings read as a tatami fill's, and the fill before it ending near its area, as
    // before a tatami fill.
    let legacy = after_a_fill("legacy_fill", &[("fill_underlay", "true")]);
    same(&legacy, &after_a_fill("tatami_fill", &[("fill_underlay", "false")]));
    // A tatami fill with its underlay sews more.
    assert!(stitches(&after_a_fill("tatami_fill", &[])) > stitches(&legacy));
}

#[test]
fn diag_sc_w0314_a_legacy_fill_says_it_is_sewn_as_a_tatami_fill() {
    // `flip`, which Ink/Stitch's legacy fill reads, is kept and changes nothing.
    let flipped = after_a_fill("legacy_fill", &[("flip", "true")]);
    assert_eq!(
        messages(&flipped),
        [
            "warning SC-W0105: `flip` is not a StitchCraft parameter; it was kept but does not change the stitches.",
            "warning SC-W0314: This element's fill method is Ink/Stitch's legacy fill, which StitchCraft does not sew, so it is sewn as a \
             tatami fill without underlay.",
        ]
    );
    same(&flipped, &after_a_fill("legacy_fill", &[]));
    assert_eq!(messages(&after_a_fill("tatami_fill", &[])), Vec::<String>::new());
}
