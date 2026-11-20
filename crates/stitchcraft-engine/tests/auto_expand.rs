//! Expand (roadmap step M13.26, `REQ-AUTO-142` to `REQ-AUTO-144`; `docs/src/design/image-to-embroidery.md`
//! › Expand): an automatic design becomes the ordinary elements digitizing makes of it, which a person edits
//! freely, and it sews as it did. The cases are planned: phase 1 recorded the decision, and the build phase
//! writes `auto::expand` and the setting `digitized_as`.
//!
//! [`expanded`] stands in for `auto::expand` until then. It is what phase 1's prototype measured: the elements
//! [`digitized`] gives, with nothing to say what ordinary settings can't, and no warning.

// Test code may unwrap and index (clippy.toml allows it in tests).
#![allow(clippy::unwrap_used, clippy::panic, clippy::indexing_slicing)]

use stitchcraft_core::{Budget, Diagnostic, ElementId, Severity};
use stitchcraft_engine::auto::digitized;
use stitchcraft_engine::design::mark::Mark;
use stitchcraft_engine::design::{Design, DesignSettings, Element};
use stitchcraft_plan::{Role, StitchPlan};
use stitchcraft_testkit::designs::{BLUE, RED, fill, in_place};
use stitchcraft_testkit::pictures::{self, Trace};

/// The setting that says an element sews as digitizing made it (`REQ-AUTO-143`).
const DIGITIZED_AS: &str = "digitized_as";

/// `design` expanded, and what was said of it. Stands in for `auto::expand` (see the module's docs).
fn expanded(design: &Design) -> (Design, Vec<Diagnostic>) {
    let (elements, _) = digitized(design, &Budget::DEFAULT);
    let elements: Vec<Element> = elements.into_owned().into_iter().map(|element| Element { automatic: false, lettered: None, ..element }).collect();
    (Design::new(elements, design.settings).unwrap(), Vec::new())
}

/// The designs the plan is compared on: one of each kind of the set, small enough for a case.
fn designs() -> Vec<(&'static str, Design)> {
    let badges = pictures::badges(Trace::Abutting);
    vec![
        ("badges", Design::new(badges.elements, DesignSettings::default()).unwrap()),
        ("pear", pictures::pear()),
        ("lettered patch", pictures::lettered_patch()),
    ]
}

/// Each stitch of `plan`: its thread's block, where it is, what kind and which part of its element's
/// stitching.
fn stitches(plan: &StitchPlan) -> Vec<(usize, f64, f64, String, Role)> {
    let mut all = Vec::new();
    for (index, block) in plan.blocks.iter().enumerate() {
        for stitch in &block.stitches {
            all.push((index, stitch.at.x(), stitch.at.y(), format!("{:?}", stitch.kind), stitch.origin.role));
        }
    }
    all
}

#[test]
#[ignore = "planned: M13.26 builds Expand"]
fn req_auto_142_an_expanded_design_plans_to_the_automatic_plan() {
    for (name, design) in designs() {
        let automatic = in_place(&design).plan.unwrap();
        let (design, _) = expanded(&design);
        let plan = in_place(&design).plan.unwrap();
        let threads = |plan: &StitchPlan| plan.blocks.iter().map(|block| block.thread.clone()).collect::<Vec<_>>();
        assert_eq!(threads(&plan), threads(&automatic), "{name}: threads");
        // Every stitch at the same point, of the same kind and part, in the same order: the machine file is
        // the same byte for byte.
        let (got, want) = (stitches(&plan), stitches(&automatic));
        let first = got.iter().zip(&want).position(|(a, b)| a != b);
        assert!(got.len() == want.len() && first.is_none(), "{name}: {} stitches against {}, first different {first:?}", got.len(), want.len());
    }
}

#[test]
#[ignore = "planned: M13.26 builds Expand"]
fn req_auto_142_an_expanded_design_has_ordinary_elements_named_after_their_shapes() {
    for (name, design) in designs() {
        let (expanded, _) = expanded(&design);
        for element in expanded.elements() {
            assert!(!element.automatic, "{name}: {} is automatic", element.id);
            assert!(element.params.iter().all(|(key, _)| !key.starts_with("auto_")), "{name}: {} keeps a setting of the Auto effect", element.id);
            // Each part is named after its shape, as the notices name it, so the person finds it in the Layers
            // panel.
            assert!(element.name.as_deref().is_some_and(|named| !named.is_empty()), "{name}: {} has no name", element.id);
        }
    }
}

#[test]
#[ignore = "planned: M13.26 builds the setting digitized_as"]
fn req_auto_143_a_fill_set_as_base_sews_only_underlay() {
    let base = fill("base", &[&[(0.0, 0.0), (20.0, 0.0), (20.0, 20.0), (0.0, 20.0)]], &RED, &[(DIGITIZED_AS, "base")]);
    let design = Design::new(vec![base], DesignSettings::default()).unwrap();
    let outcome = in_place(&design);
    assert!(!outcome.diagnostics.iter().any(|said| matches!(said.severity(), Severity::Error | Severity::Warning)), "{:?}", outcome.diagnostics);
    let plan = outcome.plan.unwrap();
    let roles: Vec<Role> = plan.sewn_stitches().into_iter().map(|stitch| stitch.role).filter(|role| *role != Role::Lock).collect();
    assert!(!roles.is_empty() && roles.iter().all(|role| *role == Role::Underlay), "{roles:?}");
}

#[test]
#[ignore = "planned: M13.26 builds the setting digitized_as"]
fn req_auto_143_the_needle_runs_under_a_later_shape_between_2_parts_set_by_hand() {
    // 2 red squares 10 mm wide and 20 mm apart and a blue bar over them, as `tests/auto_runs.rs` draws them,
    // here set by hand as parts digitizing made: the red thread runs under the bar from one to the other.
    let square =
        |id: &str, left: f64| fill(id, &[&[(left, 0.0), (left + 10.0, 0.0), (left + 10.0, 10.0), (left, 10.0)]], &RED, &[(DIGITIZED_AS, "part")]);
    let bar = fill("bar", &[&[(8.0, 2.0), (32.0, 2.0), (32.0, 8.0), (8.0, 8.0)]], &BLUE, &[(DIGITIZED_AS, "part")]);
    let design = Design::new(vec![square("left", 0.0), square("right", 30.0), bar], DesignSettings::default()).unwrap();
    let plan = in_place(&design).plan.unwrap();
    let runs = plan.sewn_stitches().into_iter().filter(|stitch| stitch.role == Role::Travel).count();
    // The plan jumps to its first stitch and at its thread change alone.
    assert_eq!((plan.stats().jumps, runs > 0), (2, true), "{:?}", plan.stats());
    // Without the setting, the shapes are set by hand and the thread jumps between the squares.
    let plain: Vec<Element> = design
        .elements()
        .iter()
        .cloned()
        .map(|mut element| {
            element.params.remove(DIGITIZED_AS);
            element
        })
        .collect();
    let plain = Design::new(plain, DesignSettings::default()).unwrap();
    assert!(in_place(&plain).plan.unwrap().stats().jumps > 2);
}

#[test]
#[ignore = "planned: M13.26 builds Expand"]
fn req_auto_144_an_expanded_design_no_longer_follows_its_marks_and_names_what_it_lost() {
    let design = pictures::lettered_patch();
    let (expanded, said) = expanded(&design);
    let before = in_place(&expanded).plan.unwrap();
    // A mark drawn over the expanded design changes nothing: it has no automatic shape left to digitize.
    let mut marked = expanded.clone();
    let bounds = expanded.bounds().unwrap();
    let (min, max) = ((bounds.min().x(), bounds.min().y()), (bounds.max().x(), bounds.max().y()));
    let mark = fill("mark", &[&[min, (max.0, min.1), max, (min.0, max.1)]], &RED, &[]);
    marked.marks.push(Mark { id: ElementId::new("stitch:keep").unwrap(), shape: mark.shape });
    assert_eq!(in_place(&marked).plan.unwrap(), before);
    // The lettering placed for the patch's marker became its columns, and one warning names it.
    let warnings: Vec<&Diagnostic> = said.iter().filter(|said| said.severity() == Severity::Warning).collect();
    assert_eq!(warnings.len(), 1, "{said:?}");
    assert!(warnings[0].message.contains("lettering"), "{}", warnings[0].message);
}
