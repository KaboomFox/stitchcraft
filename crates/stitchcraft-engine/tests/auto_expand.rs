//! Expand (roadmap step M13.26, `REQ-AUTO-142` to `REQ-AUTO-144`; `docs/src/design/image-to-embroidery.md`
//! › Expand): an automatic design becomes the ordinary elements digitizing makes of it, which a person edits
//! freely, and it sews as it did. `apps/stitchcraft-cli/tests/expand.rs` holds the cases of the documents
//! `stitch expand` writes, and the build's measure of the whole set is in the design page.

// Test code may unwrap and index (clippy.toml allows it in tests).
#![allow(clippy::unwrap_used, clippy::panic, clippy::indexing_slicing)]

use stitchcraft_core::{Budget, Code, Diagnostic, ElementId, Point, Severity, math};
use stitchcraft_engine::auto::expand;
use stitchcraft_engine::common::DIGITIZED_AS;
use stitchcraft_engine::design::mark::Mark;
use stitchcraft_engine::design::{Design, DesignSettings, Element, Lettered};
use stitchcraft_engine::plan;
use stitchcraft_plan::profiles::REFERENCE;
use stitchcraft_plan::{Role, StitchPlan};
use stitchcraft_testkit::designs::{BLUE, RED, along, fill, in_place, polylines};
use stitchcraft_testkit::pictures::{self, Trace};

/// `design` expanded, and what was said of it.
fn expanded(design: &Design) -> (Design, Vec<Diagnostic>) {
    expand(design, REFERENCE, &Budget::DEFAULT).unwrap()
}

/// A satin column of a font from `a` to `b`, 1.6 mm wide, with a contour underlay and 0.2 mm of pull
/// compensation, marked as lettering digitizing placed (`tests/auto_lettering.rs` draws them so).
fn letter(id: &str, (ax, ay): (f64, f64), (bx, by): (f64, f64)) -> Element {
    let length = math::hypot(bx - ax, by - ay);
    let (nx, ny) = (-(by - ay) / length * 0.8, (bx - ax) / length * 0.8);
    let rails = polylines(&[&[(ax + nx, ay + ny), (bx + nx, by + ny)], &[(ax - nx, ay - ny), (bx - nx, by - ny)]]);
    let params = [("satin_column", "true"), ("contour_underlay", "true"), ("pull_compensation_mm", "0.2")];
    Element { lettered: Some(Lettered::Digitized), ..along(id, rails, &BLUE, &params) }
}

/// The lettered patch with an x of 3 lettering columns that digitizing placed on its field, where they cross.
fn lettering_placed() -> Design {
    let patch = pictures::lettered_patch();
    let bounds = patch.bounds().unwrap();
    let (cx, cy) = (bounds.min().x() + 8.0, bounds.max().y() - 5.0);
    let ray = |k: usize| {
        let (s, c) = math::sin_cos((60.0 * k as f64).to_radians());
        letter(&format!("x{k}"), (cx - 3.0 * c, cy - 3.0 * s), (cx + 3.0 * c, cy + 3.0 * s))
    };
    let elements: Vec<Element> = patch.elements().iter().cloned().chain((0..3).map(ray)).collect();
    Design::new(elements, patch.settings).unwrap()
}

/// The designs the plan is compared on: one of each kind of the set, small enough for a case.
fn designs() -> Vec<(&'static str, Design)> {
    let badges = pictures::badges(Trace::Abutting);
    vec![
        ("badges", Design::new(badges.elements, DesignSettings::default()).unwrap()),
        ("pear", pictures::pear()),
        ("lettered patch", pictures::lettered_patch()),
        ("lettering placed", lettering_placed()),
    ]
}

/// The plan of `design` with its origin at `middle` and its stitches where it draws its shapes, as `in_place`
/// plans a design round the middle of its own bounds.
fn placed(design: &Design, middle: Point) -> StitchPlan {
    let mut centred = Design::new(design.elements().to_vec(), DesignSettings { origin: Some(middle), ..design.settings }).unwrap();
    centred.marks.clone_from(&design.marks);
    centred.guides.clone_from(&design.guides);
    let mut plan = plan(&centred, REFERENCE, &Budget::DEFAULT).plan.unwrap();
    for stitch in plan.blocks.iter_mut().flat_map(|block| &mut block.stitches) {
        stitch.at = Point::new(stitch.at.x() + middle.x(), stitch.at.y() + middle.y()).unwrap();
    }
    plan
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
fn req_auto_142_an_expanded_design_plans_to_the_automatic_plan() {
    for (name, design) in designs() {
        // Both are planned round the middle of the drawing. The expanded design's own bounds differ, as its
        // areas reach under their neighbours, and an origin of their own would move the fills' rows.
        let bounds = design.bounds().unwrap();
        let middle = Point::new((bounds.min().x() + bounds.max().x()) / 2.0, (bounds.min().y() + bounds.max().y()) / 2.0).unwrap();
        let automatic = placed(&design, middle);
        let (expanded, _) = expanded(&design);
        let plan = placed(&expanded, middle);
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
fn req_auto_142_an_expanded_design_has_ordinary_elements_named_after_their_shapes() {
    for (name, design) in designs() {
        let (expanded, _) = expanded(&design);
        for element in expanded.elements() {
            assert!(!element.automatic && element.lettered.is_none(), "{name}: {} is automatic or lettered", element.id);
            assert!(element.params.iter().all(|(key, _)| !key.starts_with("auto_")), "{name}: {} keeps a setting of the Auto effect", element.id);
            // Each part is named after its shape, as the notices name it, so the person finds it in the Layers
            // panel. The lettering keeps the name it had, none here.
            let part = matches!(element.params.get(DIGITIZED_AS), Some("part" | "base"));
            assert!(!part || element.name.as_deref().is_some_and(|named| !named.is_empty()), "{name}: {} has no name", element.id);
        }
    }
}

#[test]
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
fn req_auto_143_the_needle_runs_under_a_later_shape_between_2_parts_set_by_hand() {
    // 2 red squares 10 mm wide and 20 mm apart and a blue bar over both, set by hand as parts digitizing
    // made: the red thread runs under the bar from one to the other, wherever the first ends and the second
    // starts.
    let square =
        |id: &str, left: f64| fill(id, &[&[(left, 0.0), (left + 10.0, 0.0), (left + 10.0, 10.0), (left, 10.0)]], &RED, &[(DIGITIZED_AS, "part")]);
    let bar = fill("bar", &[&[(-1.0, -1.0), (41.0, -1.0), (41.0, 11.0), (-1.0, 11.0)]], &BLUE, &[(DIGITIZED_AS, "part")]);
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
fn req_auto_144_an_expanded_design_no_longer_follows_its_marks_and_names_what_it_lost() {
    let design = lettering_placed();
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
    assert_eq!(warnings[0].code, Code::AutoExpandLost);
    assert!(warnings[0].message.contains("The lettering of `x0`, `x1` and `x2` became the columns it sews"), "{}", warnings[0].message);
    // A part a person edits sews as its own settings say: a fill turned to another angle sews other rows.
    let mut edited = expanded.elements().to_vec();
    let at = edited.iter().position(|element| element.params.get("angle").is_some()).unwrap();
    edited[at].params.set("angle", "17");
    let edited = Design::new(edited, expanded.settings).unwrap();
    assert_ne!(in_place(&edited).plan.unwrap(), before);
}

#[test]
fn req_auto_144_named_lettering_and_a_shape_set_by_hand_keep_their_names_and_settings() {
    // The lettering's columns named as a text object names them, and a square set by hand beside the patch.
    let design = lettering_placed();
    let mut elements: Vec<Element> = design.elements().to_vec();
    for element in elements.iter_mut().filter(|element| element.lettered.is_some()) {
        element.name = Some("Camp Elk".to_string());
    }
    elements.push(fill("by-hand", &[&[(-40.0, 0.0), (-30.0, 0.0), (-30.0, 10.0), (-40.0, 10.0)]], &BLUE, &[("angle", "30")]));
    let design = Design::new(elements, design.settings).unwrap();
    let (expanded, said) = expanded(&design);
    let by_hand = expanded.elements().iter().find(|element| element.id.as_str() == "by-hand").unwrap();
    assert_eq!((by_hand.params.get(DIGITIZED_AS), by_hand.params.get("angle")), (None, Some("30")));
    let warning = said.iter().find(|said| said.code == Code::AutoExpandLost).unwrap();
    assert!(warning.message.contains("The lettering of \"Camp Elk\" became the columns it sews"), "{}", warning.message);
}

#[test]
fn req_auto_144_the_shapes_digitizing_has_no_work_for_are_named_once_each() {
    let design = pictures::lettered_patch();
    let (expanded, said) = expand(&design, REFERENCE, &Budget { max_work: 1, ..Budget::DEFAULT }).unwrap();
    assert!(expanded.elements().is_empty());
    let warning = said.iter().find(|said| said.code == Code::AutoExpandLost).unwrap();
    assert!(warning.message.contains("`patch-border`, `patch-field`, `m`, `a`, `t` and 2 more needed more work"), "{}", warning.message);
}

#[test]
fn req_auto_142_a_design_with_smoothed_outlines_expands_to_its_own_plan() {
    // Smoothing changes the parts digitizing makes (`REQ-AUTO-131`): Expand writes the parts of the design
    // as it is set, smoothed or not, and drops the setting with the Auto effect's others.
    let pear = pictures::pear();
    for smooth in ["0", "0.2"] {
        let elements: Vec<Element> = pear
            .elements()
            .iter()
            .cloned()
            .map(|mut element| {
                element.params.set("auto_smooth_mm", smooth);
                element
            })
            .collect();
        let design = Design::new(elements, pear.settings).unwrap();
        let bounds = design.bounds().unwrap();
        let middle = Point::new((bounds.min().x() + bounds.max().x()) / 2.0, (bounds.min().y() + bounds.max().y()) / 2.0).unwrap();
        let (expanded, _) = expanded(&design);
        assert!(expanded.elements().iter().all(|element| element.params.iter().all(|(key, _)| !key.starts_with("auto_"))), "{smooth}");
        assert_eq!(stitches(&placed(&expanded, middle)), stitches(&placed(&design, middle)), "{smooth}");
    }
}
