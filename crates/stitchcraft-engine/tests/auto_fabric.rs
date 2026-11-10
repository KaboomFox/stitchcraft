//! What automatic digitizing leaves of a picture (`REQ-AUTO-025`, `REQ-AUTO-026`): no fabric bare where the
//! picture has a colour, and no thread that shows where it should not. Each test picture and the traced pear
//! that the docs show are planned with their stitches where they draw their shapes, and measured as
//! `stitchcraft_testkit::fabric` measures any design. So are the pieces of the traced dogs whose fabric showed,
//! each a case of the rule that covers it now: the whole dogs are `tests/auto_photos.rs`'s, in the release
//! build.

// Test code may unwrap and index (clippy.toml allows it in tests).
#![allow(clippy::unwrap_used, clippy::panic, clippy::indexing_slicing)]

use stitchcraft_engine::design::{Design, DesignSettings};
use stitchcraft_plan::StitchPlan;
use stitchcraft_testkit::designs::{RED, automatic, fill, in_place, rules};
use stitchcraft_testkit::fabric::{self, ASTRAY};
use stitchcraft_testkit::pictures::{self, Trace};

/// A strand of fur as a traced photo has it, alone: 0.6 mm wide for 6 mm, widening over 1 mm to a head
/// 1.4 mm wide and 3 mm long, under 1 mm wide on average along its middle. Run along its middle, it left the
/// head's sides bare.
fn strand() -> Design {
    let club = [(0.0, -0.3), (6.0, -0.3), (7.0, -0.7), (10.0, -0.7), (10.0, 0.7), (7.0, 0.7), (6.0, 0.3), (0.0, 0.3)];
    Design::new(vec![automatic(fill("strand", &[&club], &RED, &[]))], DesignSettings::default()).unwrap()
}

/// Each test picture, the strand, the traced pear and the pieces of the traced dogs by name, digitized
/// automatically and planned in place. The dog on turf's paw left a wedge of 1.8 mm² bare beside a sliver of
/// its outline, which its column's rail ran out along and back, and 4 shapes of the running dog's ear left
/// 1.4 mm² bare inside a column whose line ran along its other side.
fn planned() -> Vec<(String, Design, StitchPlan)> {
    let mut designs: Vec<(String, Design)> = pictures::all()
        .into_iter()
        .chain([Trace::Abutting, Trace::Stacked].into_iter().flat_map(|trace| [pictures::emblem(trace), pictures::button(trace)]))
        .map(|picture| (picture.name, Design::new(picture.elements, DesignSettings::default()).unwrap()))
        .collect();
    designs.push(("strand".to_string(), strand()));
    designs.push(("pear".to_string(), pictures::pear()));
    designs.push(("paw".to_string(), pictures::dog_shapes("dog-on-turf-6", &["vc:69:fill"])));
    designs.push(("ear".to_string(), pictures::dog_shapes("dog-running-6", &["vc:5:fill", "vc:6:fill", "vc:28:fill", "vc:50:fill"])));
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
    let (mut shown, mut apart) = (Vec::new(), Vec::new());
    for (name, design, plan) in planned() {
        let astray = fabric::astray(&design, &plan);
        if astray.longest >= ASTRAY {
            shown.push(format!("{name}: {:?}, the longest {:.3} mm", astray.stitches, astray.longest));
        }
        apart.push((name, astray.moves.len(), astray.jumps.len()));
    }
    assert!(shown.is_empty(), "{shown:#?}");
    // The moves sewn on from one element to the next, and the jump threads that the PE800 leaves, are as the
    // design page records them (image-to-embroidery.md › Sewing order): the button's dots sew on from one to
    // the next across its yellow, and each of the pear's 2 darker bands jumps to the stem between its ends.
    let recorded = [
        ("stripes", 0, 0),
        ("badge-abutting", 0, 0),
        ("badge-stacked", 0, 0),
        ("badges-abutting", 0, 4),
        ("badges-stacked", 0, 4),
        ("emblem-abutting", 0, 2),
        ("button-abutting", 6, 0),
        ("emblem-stacked", 0, 2),
        ("button-stacked", 6, 0),
        ("strand", 0, 0),
        ("pear", 0, 4),
        ("paw", 0, 1),
        ("ear", 1, 8),
    ];
    assert_eq!(apart, recorded.map(|(name, moves, jumps)| (name.to_string(), moves, jumps)));
}
