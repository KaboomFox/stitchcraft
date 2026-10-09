//! The diagnostic the renderer reports, from real input, exactly as a user reads it (`diag_` cases:
//! every registered code has one, `docs/src/design/diagnostics.md`).

// Test code may unwrap (clippy.toml allows it inside #[test] functions).
#![allow(clippy::unwrap_used)]

use stitchcraft_core::{Budget, Point};
use stitchcraft_plan::{PlanBuilder, Provenance, Rgb, Role, Thread};
use stitchcraft_render::{Settings, Style, preview};

#[test]
fn diag_sc_e0005_a_preview_too_large_to_draw_says_which_scale_fits() {
    let mut b = PlanBuilder::new(Thread::named(Rgb::from_hex(0xED171F), "Red"));
    for x in [-500.0, 500.0] {
        b.stitch(Point::new(x, 0.0).unwrap(), Provenance::plan(Role::Top));
    }
    let settings = Settings::new(Style::Realistic, Settings::DEFAULT_SCALE).unwrap();
    let d = preview(&b.finish(), settings, &mut Budget::DEFAULT.meter()).unwrap_err().diagnostic();
    // 1,000 mm plus 2 mm on each side, at 8 pixels per millimetre.
    assert_eq!(d.to_string(), "error SC-E0005: The preview would be 8032 × 32 pixels; previews are at most 4096 pixels on a side.");
    assert_eq!(d.fix.map(|f| f.describe()).as_deref(), Some("Use a scale of at most 4 pixels per millimetre."));
}
