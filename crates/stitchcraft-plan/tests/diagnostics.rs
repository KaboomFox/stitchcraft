//! The diagnostics a machine profile reports about a plan, exactly as a user reads them (`diag_` cases:
//! every registered code has one, `docs/src/design/diagnostics.md`).

// Test code may unwrap (clippy.toml allows it inside #[test] functions; these helpers are test code too).
#![allow(clippy::unwrap_used)]

use stitchcraft_core::{Diagnostic, Mm, Point, Size};
use stitchcraft_plan::profiles::REFERENCE;
use stitchcraft_plan::{MachineProfile, PlanBuilder, Provenance, Rgb, Role, Thread};

/// What `profile` says about a design `width` × `height` mm.
fn fit(profile: &MachineProfile, width: f64, height: f64) -> Diagnostic {
    let mut b = PlanBuilder::new(Thread::named(Rgb::from_hex(0xED171F), "Red"));
    for (x, y) in [(0.0, 0.0), (width, height)] {
        b.stitch(Point::new(x, y).unwrap(), Provenance::plan(Role::Top));
    }
    let bounds = b.finish().bounds().unwrap();
    profile.check_fit(bounds).unwrap()
}

#[test]
fn diag_sc_e0701_a_design_larger_than_the_hoop_names_both_sizes() {
    let d = fit(REFERENCE, 131.0, 181.0);
    assert_eq!(
        d.to_string(),
        "error SC-E0701: The design is 131.0 × 181.0 mm, but the Brother PE800 with its 5 × 7 in hoop sews at most 130 × 180 mm."
    );
    assert_eq!(d.fix.map(|f| f.describe()).as_deref(), Some("Scale the design down, or split it into parts sewn in separate hoopings."));
}

#[test]
fn diag_sc_w0702_a_design_beyond_the_comfort_zone_gets_advice() {
    // No built-in profile has a comfort zone yet: the reference machine with one of 100 × 100 mm.
    let square = Size::new(Mm::from_tenths(1000), Mm::from_tenths(1000));
    let d = fit(&MachineProfile { comfort: Some(square), ..REFERENCE.clone() }, 110.0, 105.0);
    assert_eq!(
        d.to_string(),
        "warning SC-W0702: The design is 110.0 × 105.0 mm, larger than the 100 × 100 mm comfort zone of the Brother PE800 with its 5 × 7 in hoop."
    );
    assert_eq!(d.fix.map(|f| f.describe()).as_deref(), Some("Use a firm stabilizer and hoop the fabric drum-tight, or scale the design down."));
}
