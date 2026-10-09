//! The diagnostics a machine profile reports about a plan, exactly as a user reads them (`diag_` cases:
//! every registered code has one, `docs/src/design/diagnostics.md`).

// Test code may unwrap (clippy.toml allows it inside #[test] functions; these helpers are test code too).
#![allow(clippy::unwrap_used)]

use stitchcraft_core::{Diagnostic, Point};
use stitchcraft_plan::profiles::find;
use stitchcraft_plan::{PlanBuilder, Provenance, Rgb, Role, Thread};

/// What the Brother 200 × 200 mm profile says about a design `width` × `height` mm.
fn fit(width: f64, height: f64) -> Diagnostic {
    let mut b = PlanBuilder::new(Thread::named(Rgb::from_hex(0xED171F), "Red"));
    for (x, y) in [(0.0, 0.0), (width, height)] {
        b.stitch(Point::new(x, y).unwrap(), Provenance::plan(Role::Top));
    }
    let bounds = b.finish().bounds().unwrap();
    find("brother-200x200").unwrap().check_fit(bounds).unwrap()
}

#[test]
fn diag_sc_e0701_a_design_larger_than_the_hoop_names_both_sizes() {
    let d = fit(201.0, 120.0);
    assert_eq!(d.to_string(), "error SC-E0701: The design is 201.0 × 120.0 mm; the hoop of Brother, 200 × 200 mm hoop is 200 × 200 mm.");
    assert_eq!(d.fix.map(|f| f.describe()).as_deref(), Some("Scale the design down, or split it into parts sewn in separate hoopings."));
}

#[test]
fn diag_sc_w0702_a_design_beyond_the_comfort_zone_gets_advice() {
    let d = fit(190.0, 150.0);
    assert_eq!(
        d.to_string(),
        "warning SC-W0702: The design is 190.0 × 150.0 mm, larger than the 150 × 150 mm comfort zone of Brother, 200 × 200 mm hoop."
    );
    assert_eq!(d.fix.map(|f| f.describe()).as_deref(), Some("Use a firm stabilizer and hoop the fabric drum-tight, or scale the design down."));
}
