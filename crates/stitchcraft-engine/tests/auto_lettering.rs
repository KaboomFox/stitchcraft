//! The thickness cap and lettering, in the engine (`REQ-AUTO-119` to `REQ-AUTO-124`,
//! `docs/src/design/image-to-embroidery.md` › The cap and lettering): planning reads the mark the lettering
//! passes give the elements they place. `stitchcraft-svg`'s `tests/lettering_cap.rs` holds the cases on
//! lettering set from a font; these plan satin columns drawn as a font draws them and marked by hand.

// Test code may unwrap and index (clippy.toml allows it in tests).
#![allow(clippy::unwrap_used, clippy::panic, clippy::indexing_slicing)]

use stitchcraft_core::{Code, Point, math};
use stitchcraft_engine::PlanOutcome;
use stitchcraft_engine::design::{Design, DesignSettings, Element, Lettered};
use stitchcraft_plan::{Role, StitchKind};
use stitchcraft_testkit::designs::{RED, along, in_place, polylines};
use stitchcraft_testkit::fabric::{self, Limits};

/// A satin column of a font from `a` to `b`, 1.6 mm wide, with a contour underlay and 0.2 mm of pull
/// compensation, as hand-digitized fonts set theirs: its 2 rails.
fn column(id: &str, (ax, ay): (f64, f64), (bx, by): (f64, f64), lettered: Option<Lettered>) -> Element {
    let length = math::hypot(bx - ax, by - ay);
    let (nx, ny) = (-(by - ay) / length * 0.8, (bx - ax) / length * 0.8);
    let rails = polylines(&[&[(ax + nx, ay + ny), (bx + nx, by + ny)], &[(ax - nx, ay - ny), (bx - nx, by - ny)]]);
    let params = [("satin_column", "true"), ("contour_underlay", "true"), ("pull_compensation_mm", "0.2")];
    Element { lettered, ..along(id, rails, &RED, &params) }
}

/// An x of 3 columns 10 mm long that cross at one point, 60° apart, marked `lettered`.
fn x(lettered: Option<Lettered>) -> Design {
    let ray = |k: usize| {
        let (s, c) = math::sin_cos((60.0 * k as f64).to_radians());
        column(&format!("x{k}"), (10.0 - 5.0 * c, 10.0 - 5.0 * s), (10.0 + 5.0 * c, 10.0 + 5.0 * s), lettered)
    };
    Design::new((0..3).map(ray).collect(), DesignSettings::default()).unwrap()
}

/// What `outcome` says with `code`.
fn said(outcome: &PlanOutcome, code: Code) -> usize {
    outcome.diagnostics.iter().filter(|said| said.code == code).count()
}

/// The needle points of `outcome`'s plan, where they lie and what kind they are.
fn stitches(outcome: &PlanOutcome) -> Vec<(Point, StitchKind)> {
    outcome.plan.as_ref().unwrap().stitches().map(|stitch| (stitch.at, stitch.kind)).collect()
}

#[test]
fn req_auto_124_a_persons_own_lettering_is_measured_and_warned_of_and_never_changed() {
    let (set, unmarked) = (in_place(&x(Some(Lettered::Set))), in_place(&x(None)));
    assert!(fabric::thickness(unmarked.plan.as_ref().unwrap(), &Limits::DEFAULT).warns(), "the crossing sews above the warning's limit");
    // Unmarked, it is a design of shapes with settings, which is not measured (`REQ-AUTO-062`).
    assert_eq!((said(&unmarked, Code::AutoThick), said(&unmarked, Code::AutoThinned)), (0, 0));
    // Set by a person, it is measured by the default limits and warned of, and nothing is taken.
    assert_eq!((said(&set, Code::AutoThick), said(&set, Code::AutoThinned)), (1, 0), "{:?}", set.diagnostics);
    assert_eq!(stitches(&set), stitches(&unmarked));
}

#[test]
fn req_auto_119_lettering_that_digitizing_places_is_in_the_reach_of_the_last_pass_and_the_cap() {
    let (placed, set) = (in_place(&x(Some(Lettered::Digitized))), in_place(&x(Some(Lettered::Set))));
    assert!(said(&placed, Code::AutoThinned) > 0, "{:?}", placed.diagnostics);
    assert_ne!(stitches(&placed), stitches(&set));
}

#[test]
fn req_auto_120_a_lettering_column_sewn_whole_tags_its_underlay() {
    let roles = |lettered: Option<Lettered>| {
        let design = Design::new(vec![column("bar", (0.0, 0.0), (10.0, 0.0), lettered)], DesignSettings::default()).unwrap();
        let plan = in_place(&design).plan.unwrap();
        let sewn = plan.stitches().filter(|stitch| stitch.kind == StitchKind::Normal && stitch.origin.role != Role::Lock);
        sewn.map(|stitch| stitch.origin.role).collect::<Vec<_>>()
    };
    // The contour underlay sews first, round the column and back to its start in 10 needle points: they are
    // underlay, and every point after them top. The locks keep their own role.
    let unmarked = roles(None);
    for lettered in [Lettered::Digitized, Lettered::Set] {
        let placed = roles(Some(lettered));
        let underlay = placed.iter().take_while(|role| **role == Role::Underlay).count();
        assert_eq!(underlay, 10, "{placed:?}");
        assert!(placed[underlay..].iter().all(|role| *role == Role::Top), "{placed:?}");
        assert_eq!(placed.len(), unmarked.len());
    }
    // A column that isn't lettering keeps every stitch top, as the cap's rules for digitized columns were measured.
    assert!(unmarked.iter().all(|role| *role == Role::Top), "{unmarked:?}");
}
