//! What automatic digitizing leaves of a picture (`REQ-AUTO-025`, `REQ-AUTO-026`): no fabric bare where the
//! picture has a colour, and no thread that shows where it should not. Each test picture and the traced pear
//! that the docs show are planned with their stitches where they draw their shapes, and measured as
//! `stitchcraft_testkit::fabric` measures any design.

// Test code may unwrap and index (clippy.toml allows it in tests).
#![allow(clippy::unwrap_used, clippy::panic, clippy::indexing_slicing)]

use stitchcraft_engine::design::{Design, DesignSettings};
use stitchcraft_plan::StitchPlan;
use stitchcraft_testkit::designs::{in_place, rules};
use stitchcraft_testkit::fabric::{self, ASTRAY};
use stitchcraft_testkit::pictures::{self, Trace};

/// Each test picture and the traced pear by name, digitized automatically and planned in place.
fn planned() -> Vec<(String, Design, StitchPlan)> {
    let mut designs: Vec<(String, Design)> = pictures::all()
        .into_iter()
        .chain([Trace::Abutting, Trace::Stacked].into_iter().flat_map(|trace| [pictures::emblem(trace), pictures::button(trace)]))
        .map(|picture| (picture.name, Design::new(picture.elements, DesignSettings::default()).unwrap()))
        .collect();
    designs.push(("pear".to_string(), pictures::pear()));
    designs
        .into_iter()
        .map(|(name, design)| {
            let outcome = in_place(&design);
            let plan = outcome.plan.unwrap_or_else(|| panic!("{name}: {:?}", outcome.diagnostics));
            (name, design, plan)
        })
        .collect()
}

#[test]
fn req_auto_025_digitized_pictures_leave_no_patch_of_fabric_bare() {
    let smallest = rules().auto_min_area_mm2;
    let bare: Vec<String> = planned()
        .iter()
        .filter_map(|(name, design, plan)| {
            let unsewn = fabric::unsewn(design, plan);
            (unsewn.largest >= smallest).then(|| format!("{name}: {:.3} mm² at {:?}, {} patches", unsewn.largest, unsewn.at, unsewn.patches))
        })
        .collect();
    assert!(bare.is_empty(), "{bare:#?}");
}

#[test]
fn req_auto_026_digitized_pictures_lay_no_thread_that_shows_where_it_should_not() {
    let shown: Vec<String> = planned()
        .iter()
        .filter_map(|(name, design, plan)| {
            let astray = fabric::astray(design, plan);
            (astray.longest >= ASTRAY).then(|| format!("{name}: {:?}, the longest {:.3} mm", astray.stitches, astray.longest))
        })
        .collect();
    assert!(shown.is_empty(), "{shown:#?}");
}
