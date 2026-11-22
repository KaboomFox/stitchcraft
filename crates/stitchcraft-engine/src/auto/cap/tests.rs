// Test data: small counts of rows, from sizes given in whole steps.
#![allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]

use stitchcraft_core::{Budget, Code, Diagnostic, ElementId};
use stitchcraft_plan::thickness::Limits;
use stitchcraft_plan::{PlanBuilder, Provenance, Rgb, Stitch, StitchKind, Thread};

use super::*;
use crate::auto::thick::{Held, finished};

fn p(x: f64, y: f64) -> Point {
    Point::new(x, y).unwrap()
}

/// An element's id, the role of its stitches, and their needle points; the first part of each element
/// starts with a jump to its first point.
type Part<'a> = (&'a str, Role, Vec<(f64, f64)>);

/// A plan of one block of `parts`, sewn in order.
fn plan(parts: &[Part]) -> StitchPlan {
    let mut builder = PlanBuilder::new(Thread::new(Rgb::new(0, 0, 0)));
    let mut ids: Vec<(&str, ElementRef)> = Vec::new();
    for (id, role, points) in parts {
        let element = match ids.iter().find(|(known, _)| known == id) {
            Some((_, element)) => *element,
            None => {
                let element = builder.element(ElementId::new(*id).unwrap()).unwrap();
                ids.push((id, element));
                let (x, y) = points[0];
                builder.jump(p(x, y), Provenance::new(element, Role::Travel));
                element
            }
        };
        for (x, y) in points {
            builder.stitch(p(*x, *y), Provenance::new(element, *role));
        }
    }
    builder.finish()
}

/// A zigzag from x = 0 to x = `width` and back, from y = 0 to y = `width`, its needle points `apart` above
/// each other on alternate sides: a fill of `0.25 / apart` fills, whose stitches turn at each point.
fn zigzag(width: f64, apart: f64) -> Vec<(f64, f64)> {
    let count = (width / apart).round() as usize;
    (0..=count).map(|k| (if k % 2 == 0 { 0.0 } else { width }, k as f64 * apart)).collect()
}

/// Rows from x = 0 to x = `width` and back, `apart` above each other, each in 1 stitch, with a turn of
/// `apart` between them.
fn rows(width: f64, apart: f64) -> Vec<(f64, f64)> {
    let count = (width / apart).round() as usize;
    (0..=count)
        .flat_map(|k| {
            if k % 2 == 0 { [(0.0, k as f64 * apart), (width, k as f64 * apart)] } else { [(width, k as f64 * apart), (0.0, k as f64 * apart)] }
        })
        .collect()
}

const NONE: &BTreeSet<ElementRef> = &BTreeSet::new();

/// No lettering's columns.
const NO_PULL: &BTreeMap<ElementRef, crate::auto::cap::Pull> = &BTreeMap::new();
const NO_PULLS: &BTreeMap<ElementRef, crate::auto::cap::Pullable> = &BTreeMap::new();

fn rules() -> Rules<'static> {
    Rules { cap: 4.0, limit: 4.0, base: NONE, columns: NONE, lettering: NO_PULL, pulls: NO_PULLS, overlap: 0.0, shortest: 0.3, longest: 12.1 }
}

/// Every element of `plan`, as digitizing made them all.
fn all(plan: &StitchPlan) -> BTreeSet<ElementRef> {
    (0..plan.elements.len()).filter_map(ElementRef::from_index).collect()
}

/// `plan` capped with `rules`, its measure kept up to date, and the measure of what the cap left.
fn cap(plan: &mut StitchPlan, rules: &Rules) -> (Option<Capped>, Thickness, Thickness) {
    let mut thickness = Thickness::of(plan, &mut Budget::DEFAULT.meter()).unwrap().unwrap();
    let capped = capped(plan, &mut thickness, rules, &all(plan), &Budget::DEFAULT).unwrap();
    let again = Thickness::of(plan, &mut Budget::DEFAULT.meter()).unwrap().unwrap();
    (capped, thickness, again)
}

/// How many needle points of `plan` have `role`.
fn count(plan: &StitchPlan, role: Role) -> usize {
    plan.stitches().filter(|stitch| stitch.kind == StitchKind::Normal && stitch.origin.role == role).count()
}

/// Whether `kept`, the measure the cap kept up to date, measures as `measured`, the plan's measured again,
/// at every half millimetre round the origin, and has its stitches.
fn measures_as(kept: &Thickness, measured: &Thickness) -> bool {
    kept.sewn() == measured.sewn()
        && (-12..=12).all(|x| {
            (-12..=12).all(|y| {
                let place = p(f64::from(x) * 0.5, f64::from(y) * 0.5);
                [Measure::Fills, Measure::Points].into_iter().all(|measure| (kept.at(measure, place) - measured.at(measure, place)).abs() < 1e-9)
            })
        })
}

#[test]
fn req_auto_100_underlay_under_later_top_stitches_goes_first() {
    // A fill of underlay under 4 fills of top stitches, sewn after it: 5 fills.
    let sewn = plan(&[("base", Role::Underlay, zigzag(4.0, 0.25)), ("top", Role::Top, zigzag(4.0, 0.0625))]);
    let mut capped_plan = sewn.clone();
    let (capped, kept, measured) = cap(&mut capped_plan, &rules());
    let capped = capped.unwrap();
    assert_eq!(capped.times, [8, 0, 0, 0], "{capped:?}");
    assert!((capped.taken[0] - 60.129_848_823).abs() < 1e-6 && capped.taken[1..] == [0.0, 0.0, 0.0], "{capped:?}");
    assert!(count(&capped_plan, Role::Underlay) < count(&sewn, Role::Underlay));
    assert_eq!(count(&capped_plan, Role::Top), count(&sewn, Role::Top));
    assert!(measures_as(&kept, &measured));
    assert!((capped.thickest.value - measured.most(Measure::Fills).value).abs() < 1e-9);
    assert!(capped.left.area < measured.over(Measure::Fills, 4.0, &mut Budget::DEFAULT.meter()).unwrap().area + 1e-9);
    assert!(kinds(&capped).starts_with(&format!("{:.1} mm of underlay and travel in {} places", capped.taken[0], capped.times[0])));
    // Each needle point left is one of the plan's, in its order, and the jump stays.
    let left: Vec<Stitch> = capped_plan.stitches().copied().collect();
    let mut all = sewn.stitches();
    assert!(left.iter().all(|stitch| all.any(|other| other == stitch)));
    assert_eq!(capped_plan.stitches().filter(|stitch| stitch.kind == StitchKind::Jump).count(), 2);
}

#[test]
fn req_auto_100_underlay_sewn_after_the_top_stitches_stays() {
    // Nothing is sewn after the underlay, and the top stitches are too close together to join.
    let mut sewn = plan(&[("top", Role::Top, zigzag(4.0, 0.0625)), ("base", Role::Underlay, zigzag(4.0, 0.25))]);
    let was = sewn.clone();
    let (capped, _, _) = cap(&mut sewn, &rules());
    let capped = capped.unwrap();
    assert!(!capped.took() && capped.left.area > 0.0 && capped.thickest.value > 4.0, "{capped:?}");
    assert_eq!(sewn, was);
    assert_eq!(kinds(&capped), "");
}

#[test]
fn req_auto_100_only_the_elements_digitizing_made_lose_needle_points() {
    // The same 5 fills, of which digitizing made the top stitches only: the underlay is a shape's own,
    // sewn as its settings say.
    let mut sewn = plan(&[("base", Role::Underlay, zigzag(4.0, 0.25)), ("top", Role::Top, zigzag(4.0, 0.0625))]);
    let was = sewn.clone();
    let mut thickness = Thickness::of(&sewn, &mut Budget::DEFAULT.meter()).unwrap().unwrap();
    let made: BTreeSet<ElementRef> = ElementRef::from_index(1).into_iter().collect();
    let capped = capped(&mut sewn, &mut thickness, &rules(), &made, &Budget::DEFAULT).unwrap().unwrap();
    assert!(!capped.took(), "{capped:?}");
    assert_eq!(sewn, was);
}

#[test]
fn req_auto_100_a_shape_s_top_under_a_later_shape_goes_second() {
    let mut sewn = plan(&[("under", Role::Top, zigzag(4.0, 0.25)), ("over", Role::Top, zigzag(4.0, 0.0625))]);
    let (capped, kept, measured) = cap(&mut sewn, &rules());
    let capped = capped.unwrap();
    assert_eq!(capped.times, [0, 8, 0, 0], "{capped:?}");
    assert!((capped.taken[1] - 60.129_848_823).abs() < 1e-6, "{capped:?}");
    assert!(measures_as(&kept, &measured));
    assert!(kinds(&capped).starts_with(&format!("{:.1} mm of top stitches under later shapes in ", capped.taken[1])));
    // Sewn the other way round, the shape below is the later one, and its points can't go.
    let mut turned = plan(&[("over", Role::Top, zigzag(4.0, 0.0625)), ("under", Role::Top, zigzag(4.0, 0.25))]);
    assert!(!cap(&mut turned, &rules()).0.unwrap().took());
}

#[test]
fn req_auto_100_a_shape_s_own_top_where_it_is_doubled_goes_last() {
    // One shape sews the same zigzag 5 times: 5 fills, each pass covering the others' places.
    let points: Vec<(f64, f64)> = (0..5).flat_map(|_| zigzag(4.0, 0.25)).collect();
    let sewn = plan(&[("doubled", Role::Top, points)]);
    let mut capped_plan = sewn.clone();
    // Below the warning's limit a joined stitch lays thread only where the zigzag's own does: 2 points at its
    // turns stay.
    let (capped, kept, measured) = cap(&mut capped_plan, &Rules { limit: 50.0, ..rules() });
    let capped = capped.unwrap();
    assert_eq!(capped.times, [0, 0, 20, 0], "{capped:?}");
    assert!((capped.taken[2] - 143.808_390_956).abs() < 1e-6, "{capped:?}");
    assert!(measures_as(&kept, &measured));
    assert!(measured.most(Measure::Fills).value < Thickness::of(&sewn, &mut Budget::DEFAULT.meter()).unwrap().unwrap().most(Measure::Fills).value);
    assert!(kinds(&capped).starts_with(&format!("{:.1} mm of a shape's own doubled top stitches in ", capped.taken[2])));
}

#[test]
fn req_auto_100_every_kind_is_said_in_order() {
    let thickest =
        Thickness::of(&plan(&[("one", Role::Top, zigzag(4.0, 0.25))]), &mut Budget::DEFAULT.meter()).unwrap().unwrap().most(Measure::Fills);
    let capped = Capped { taken: [1.0, 2.0, 3.04, 0.0], times: [1, 2, 3, 0], left: Over::default(), thickest, ran_out: false, pulled: [0; 2] };
    let all = "1.0 mm of underlay and travel in 1 place, 2.0 mm of top stitches under later shapes in 2 places, 3.0 mm of a shape's own doubled top stitches in 3 places";
    assert_eq!(kinds(&capped), all);
    // The points pulled in come first (`REQ-AUTO-172`).
    assert_eq!(done(&capped), format!("took {all}"));
    let one = Capped { taken: [1.0, 0.0, 0.0, 0.0], times: [1, 0, 0, 0], pulled: [1, 1], ..capped };
    assert_eq!(done(&one), "pulled in 1 point of a satin column in 1 place and took 1.0 mm of underlay and travel in 1 place");
    let only = Capped { taken: [0.0; 4], times: [0; 4], pulled: [3, 2], ..capped };
    assert!(only.took());
    assert_eq!(done(&only), "pulled in 3 points of satin columns in 2 places");
}

#[test]
fn req_auto_100_a_joined_stitch_is_no_longer_than_its_element_s_longest() {
    // The underlay's rows are its longest stitches, 4 mm: a row and its turn joined are longer.
    let mut sewn = plan(&[("base", Role::Underlay, rows(4.0, 0.25)), ("top", Role::Top, zigzag(4.0, 0.0625))]);
    let (capped, _, _) = cap(&mut sewn, &rules());
    assert_eq!(capped.unwrap().times[0], 0);
}

#[test]
fn req_auto_100_a_lock_s_needle_points_and_the_one_after_a_lock_stay() {
    let lock = vec![(0.0, 0.0), (0.3, 0.0), (0.0, 0.0), (0.3, 0.0), (0.0, 0.0)];
    let sewn = plan(&[("base", Role::Lock, lock), ("base", Role::Underlay, zigzag(4.0, 0.25)), ("top", Role::Top, zigzag(4.0, 0.0625))]);
    let mut capped_plan = sewn.clone();
    let (capped, _, _) = cap(&mut capped_plan, &rules());
    assert!(capped.unwrap().times[0] > 0);
    let locks = |plan: &StitchPlan| plan.stitches().filter(|stitch| stitch.origin.role == Role::Lock).copied().collect::<Vec<_>>();
    assert_eq!(locks(&capped_plan), locks(&sewn));
    // The underlay's first point, after the lock, is the plan's.
    let first = |plan: &StitchPlan| plan.stitches().find(|stitch| stitch.origin.role == Role::Underlay).copied();
    assert_eq!(first(&capped_plan), first(&sewn));
    // A plan whose spots are all within the cap is not gone through.
    let mut thin = plan(&[("one", Role::Top, zigzag(4.0, 0.25))]);
    assert_eq!(cap(&mut thin, &rules()).0, None);
    // A cap above every spot takes nothing.
    let mut five = sewn;
    assert_eq!(cap(&mut five, &Rules { cap: 50.0, ..rules() }).0, None);
}

#[test]
fn req_auto_100_a_point_on_a_straight_run_takes_no_thread_and_stays() {
    // The top stitches run straight across and back, in stitches of 0.5 mm, 24 times: 6 fills. A point
    // between 2 of them takes no thread.
    let line: Vec<(f64, f64)> = (0..=8).map(|k| (f64::from(k) * 0.5, 1.0)).collect();
    let back: Vec<(f64, f64)> = line.iter().rev().copied().collect();
    let points: Vec<(f64, f64)> = (0..24).flat_map(|k| if k % 2 == 0 { line.clone() } else { back.clone() }).collect();
    let mut sewn = plan(&[("line", Role::Top, points)]);
    let was = sewn.clone();
    let (capped, _, _) = cap(&mut sewn, &rules());
    assert!(capped.unwrap().thickest.value > 4.0);
    // Only the turns at the line's ends may go.
    let kept: Vec<(f64, f64)> =
        sewn.stitches().filter(|stitch| stitch.kind == StitchKind::Normal).map(|stitch| (stitch.at.x(), stitch.at.y())).collect();
    let all: Vec<(f64, f64)> =
        was.stitches().filter(|stitch| stitch.kind == StitchKind::Normal).map(|stitch| (stitch.at.x(), stitch.at.y())).collect();
    let inner = |points: &[(f64, f64)]| points.iter().filter(|(x, _)| *x > 0.0 && *x < 4.0).count();
    assert_eq!(inner(&kept), inner(&all));
}

/// The least work of a budget with which `sewn`, measured as `thickness`, is capped with `rules` without
/// running out, of every element's points.
fn least(sewn: &StitchPlan, thickness: &Thickness, rules: &Rules) -> u64 {
    let whole = |max_work: u64| {
        let budget = Budget { max_work, ..Budget::DEFAULT };
        capped(&mut sewn.clone(), &mut thickness.clone(), rules, &all(sewn), &budget).is_ok_and(|capped| capped.is_some_and(|capped| !capped.ran_out))
    };
    let (mut short, mut enough) = (0, Budget::DEFAULT.max_work);
    while enough - short > 1 {
        let middle = short + (enough - short) / 2;
        if whole(middle) { enough = middle } else { short = middle }
    }
    enough
}

#[test]
fn req_auto_101_a_cap_out_of_work_keeps_the_points_it_took_measured_again() {
    let sewn =
        plan(&[("under", Role::Top, zigzag(1.5, 0.25)), ("over", Role::Top, zigzag(1.5, 0.0625)), ("over", Role::Underlay, zigzag(1.5, 0.25))]);
    let thickness = Thickness::of(&sewn, &mut Budget::DEFAULT.meter()).unwrap().unwrap();
    let enough = least(&sewn, &thickness, &rules());
    let (mut whole, mut full) = (sewn.clone(), thickness.clone());
    let all_of = capped(&mut whole, &mut full, &rules(), &all(&sewn), &Budget::DEFAULT).unwrap().unwrap();
    assert!(!all_of.ran_out && all_of.took(), "{all_of:?}");
    // With less work at all, the cap runs out: before it takes a point it says nothing of the plan, and
    // after, it keeps the points it took, no more than with the work it needs, and the plan they leave
    // measured again.
    let mut kept = 0;
    for max_work in 0..enough {
        let (mut plan, mut measure) = (sewn.clone(), thickness.clone());
        let Ok(Some(capped)) = capped(&mut plan, &mut measure, &rules(), &all(&sewn), &Budget { max_work, ..Budget::DEFAULT }) else { continue };
        assert!(capped.ran_out, "{max_work}: {capped:?}");
        let measured = Thickness::of(&plan, &mut Budget::DEFAULT.meter()).unwrap().unwrap();
        assert!(measures_as(&measure, &measured), "{max_work}");
        assert_eq!(capped.thickest, measured.most(Measure::Fills), "{max_work}");
        assert!(capped.times.iter().sum::<usize>() <= all_of.times.iter().sum::<usize>(), "{max_work}: {capped:?}");
        kept += usize::from(capped.took());
    }
    assert!(kept > 0, "{enough}");
}

/// `copies` columns, each sewing a zigzag 4 mm wide 5 times over itself, 5 fills, 20 mm apart along x: the
/// plan, and the columns.
fn columns_apart(copies: usize) -> (StitchPlan, BTreeSet<ElementRef>) {
    let ids: Vec<String> = (0..copies).map(|copy| format!("column-{copy}")).collect();
    let parts: Vec<Part> = ids
        .iter()
        .enumerate()
        .map(|(copy, id)| {
            let points = (0..5).flat_map(|_| zigzag(4.0, 0.25)).map(|(x, y)| (x + 20.0 * copy as f64, y)).collect();
            (id.as_str(), Role::Top, points)
        })
        .collect();
    (plan(&parts), (0..copies).filter_map(ElementRef::from_index).collect())
}

#[test]
fn req_auto_101_the_cap_s_work_grows_as_the_design_does() {
    // Columns far apart above the warning's limit: 4 of them take 4 times the thread of one, for about 4
    // times the work, not 16, since a point is judged only for a spot its stitches lay thread at.
    let work = |copies: usize| {
        let (mut sewn, columns) = columns_apart(copies);
        let mut thickness = Thickness::of(&sewn, &mut Budget::DEFAULT.meter()).unwrap().unwrap();
        let rules = Rules { cap: 3.0, limit: 4.0, columns: &columns, ..rules() };
        let enough = least(&sewn, &thickness, &rules);
        let capped = capped(&mut sewn, &mut thickness, &rules, &columns, &Budget::DEFAULT).unwrap().unwrap();
        (capped.times[3], enough)
    };
    let ((one, alone), (four, together)) = (work(1), work(4));
    assert!(one > 0 && four == 4 * one, "{one} and {four}");
    assert!(together <= 5 * alone, "{alone} and {together}");
}

/// What `finished` says of `sewn` with `budget`, and the plan it leaves.
fn said(sewn: &StitchPlan, budget: &Budget) -> (Vec<Diagnostic>, StitchPlan) {
    let held = Held { limits: Limits::DEFAULT, overlap: 0.0, cap: 4.0 };
    let mut kept = sewn.clone();
    let made: BTreeSet<ElementId> = sewn.elements.iter().cloned().collect();
    let said = finished(
        &mut kept,
        &held,
        (&[], &made, &BTreeSet::new(), &BTreeMap::new(), &BTreeMap::new()),
        stitchcraft_plan::profiles::REFERENCE,
        &crate::names::Names::default(),
        budget,
    );
    (said, kept)
}

/// The work `work` takes.
fn used(work: &dyn Fn(&mut Meter) -> bool) -> u64 {
    let mut meter = Budget::DEFAULT.meter();
    assert!(work(&mut meter));
    Budget::DEFAULT.max_work - meter.work_left()
}

#[test]
fn diag_sc_i1119_says_what_the_cap_took_after_a_pass_that_took_nothing() {
    let sewn = plan(&[("base", Role::Underlay, zigzag(4.0, 0.25)), ("top", Role::Top, zigzag(4.0, 0.0625))]);
    let (said, kept) = said(&sewn, &Budget::DEFAULT);
    let note = said.iter().find(|said| said.code == Code::AutoThinned).unwrap();
    assert!(note.message.contains("sewed 4 fills or more. The last pass took nothing there, and the cap took "), "{}", note.message);
    assert!(note.message.contains(" mm of underlay and travel in "), "{}", note.message);
    assert!(count(&kept, Role::Underlay) < count(&sewn, Role::Underlay));
    // The warning that follows gives the plan as the cap left it.
    let measured = Thickness::of(&kept, &mut Budget::DEFAULT.meter()).unwrap().unwrap().most(Measure::Fills).value;
    assert!(note.message.contains(&format!("the thickest spot, {measured:.1} fills at ")), "{}", note.message);
    let warned = said.iter().find(|said| said.code == Code::AutoThick).unwrap();
    assert!(warned.message.starts_with(&format!("Thread is up to {measured:.1} fills thick at ")), "{}", warned.message);
}

#[test]
fn diag_sc_i1119_says_when_the_cap_ran_out_of_work_and_keeps_what_the_pass_left() {
    let sewn = plan(&[("base", Role::Underlay, zigzag(4.0, 0.25)), ("top", Role::Top, zigzag(4.0, 0.0625))]);
    let thickness = Thickness::of(&sewn, &mut Budget::DEFAULT.meter()).unwrap().unwrap();
    let measure = used(&|meter| Thickness::of(&sewn, meter).is_ok());
    let pass = used(&|meter| super::super::thin::thinned(&mut sewn.clone(), &mut thickness.clone(), &rules(), meter).is_ok());
    let budget = Budget { max_work: measure.max(pass), ..Budget::DEFAULT };
    let (said, kept) = said(&sewn, &budget);
    let note = said.iter().find(|said| said.code == Code::AutoThinned).unwrap();
    assert!(
        note.message.contains("The last pass took nothing there. The cap needs more work than the budget allows, and took nothing. Left: "),
        "{}",
        note.message
    );
    assert_eq!(kept, sewn);
}

#[test]
fn diag_sc_i1119_says_what_the_cap_took_before_it_ran_out_of_work_and_keeps_it() {
    let sewn = plan(&[("base", Role::Underlay, zigzag(4.0, 0.25)), ("top", Role::Top, zigzag(4.0, 0.0625))]);
    let thickness = Thickness::of(&sewn, &mut Budget::DEFAULT.meter()).unwrap().unwrap();
    // The pass takes nothing here, so the cap works on the plan as it is: with one unit less than it needs, it
    // runs out after it took points, and keeps them.
    let pass = used(&|meter| super::super::thin::thinned(&mut sewn.clone(), &mut thickness.clone(), &rules(), meter).is_ok());
    let short = least(&sewn, &thickness, &rules()) - 1;
    assert!(short >= pass, "{short} and {pass}");
    let (said, kept) = said(&sewn, &Budget { max_work: short, ..Budget::DEFAULT });
    let note = said.iter().find(|said| said.code == Code::AutoThinned).unwrap();
    assert!(note.message.contains("The last pass took nothing there, and the cap took "), "{}", note.message);
    assert!(note.message.contains(" before it ran out of work. Left: "), "{}", note.message);
    assert!(count(&kept, Role::Underlay) < count(&sewn, Role::Underlay));
    let measured = Thickness::of(&kept, &mut Budget::DEFAULT.meter()).unwrap().unwrap().most(Measure::Fills).value;
    assert!(note.message.contains(&format!("the thickest spot, {measured:.1} fills at ")), "{}", note.message);
}

#[test]
fn diag_sc_i1119_says_only_what_the_pass_took_when_the_cap_takes_nothing() {
    // Lock stitches beyond one back and forth, on top stitches sewn 30 times over a stitch: the pass takes
    // the locks, and no top point may go, as a joined stitch would have no length.
    let thick: Vec<(f64, f64)> = (0..=30).map(|k| if k % 2 == 0 { (0.0, 0.5) } else { (1.0, 0.5) }).collect();
    let lock = vec![(0.5, 0.5), (0.8, 0.5), (0.5, 0.5), (0.8, 0.5), (0.5, 0.5)];
    let sewn = plan(&[("thick", Role::Top, thick), ("tied", Role::Lock, lock)]);
    let (said, kept) = said(&sewn, &Budget::DEFAULT);
    let note = said.iter().find(|said| said.code == Code::AutoThinned).unwrap();
    assert!(note.message.contains("The last pass took ") && note.message.contains(" mm of lock stitches in "), "{}", note.message);
    assert!(!note.message.contains("the cap"), "{}", note.message);
    assert!(count(&kept, Role::Lock) < count(&sewn, Role::Lock));
}

#[test]
fn req_auto_100_a_point_left_by_a_round_goes_in_the_next_once_the_one_before_it_has_gone() {
    // An underlay run under 5 fills of later top stitches: its third point lies on a straight stretch, so it
    // takes no thread until the turn before it goes, which the walk from the plan's end reaches after it.
    let run = vec![(0.0, 1.0), (4.0, 1.0), (2.0, 1.25), (0.0, 1.5)];
    let mut sewn = plan(&[("base", Role::Underlay, run), ("top", Role::Top, zigzag(4.0, 0.05))]);
    let (capped, kept, measured) = cap(&mut sewn, &rules());
    assert_eq!(capped.unwrap().times, [2, 0, 0, 0]);
    assert!(measures_as(&kept, &measured));
    let left: Vec<(f64, f64)> = sewn
        .stitches()
        .filter(|stitch| stitch.kind == StitchKind::Normal && stitch.origin.role == Role::Underlay)
        .map(|stitch| (stitch.at.x(), stitch.at.y()))
        .collect();
    assert_eq!(left, [(0.0, 1.0), (0.0, 1.5)]);
}

#[test]
fn req_auto_100_a_top_point_before_its_shape_s_lock_stays() {
    // The shape below ends in the middle of the square and ties off there, under a later shape: its last top
    // point, before the lock, stays.
    let mut under = zigzag(4.0, 0.25);
    under.push((2.0, 2.0));
    let lock = vec![(1.7, 2.0), (2.0, 2.0), (1.7, 2.0), (2.0, 2.0)];
    let mut sewn = plan(&[("under", Role::Top, under), ("under", Role::Lock, lock), ("over", Role::Top, zigzag(4.0, 0.0625))]);
    let (capped, _, _) = cap(&mut sewn, &rules());
    assert!(capped.unwrap().times[1] > 0);
    assert!(sewn.stitches().any(|stitch| stitch.origin.role == Role::Top && stitch.at == p(2.0, 2.0)));
}

#[test]
fn req_auto_100_underlay_covers_nothing() {
    // A shape's top stitches under a later shape's underlay alone: underlay is no cover, so nothing goes.
    let mut sewn = plan(&[("under", Role::Top, zigzag(4.0, 0.25)), ("over", Role::Underlay, zigzag(4.0, 0.0625))]);
    let was = sewn.clone();
    assert!(!cap(&mut sewn, &rules()).0.unwrap().took());
    assert_eq!(sewn, was);
}

#[test]
fn req_auto_100_a_point_is_looked_at_when_its_stitch_out_comes_near_a_spot_above_the_cap() {
    // A run that comes from far off into a thick place: the stitch into its second point lies far from it,
    // the stitch out of it reaches it.
    let run = vec![(12.0, 0.5), (8.0, 0.5), (0.5, 0.5)];
    let sewn =
        plan(&[("thick", Role::Top, (0..=30).map(|k| if k % 2 == 0 { (0.0, 0.5) } else { (1.0, 0.5) }).collect()), ("run", Role::Underlay, run)]);
    let thickness = Thickness::of(&sewn, &mut Budget::DEFAULT.meter()).unwrap().unwrap();
    let near = thickness.near(4.0, NEAR, &mut Budget::DEFAULT.meter()).unwrap();
    let at = |x: f64| {
        sewn.blocks[0]
            .stitches
            .iter()
            .position(|stitch| stitch.kind == StitchKind::Normal && stitch.origin.role == Role::Underlay && stitch.at.x() == x)
            .unwrap()
    };
    let points = candidates(thickness.sewn(), &near, |_| true);
    assert!(points.contains(&(0, at(8.0))) && points.contains(&(0, at(0.5))) && !points.contains(&(0, at(12.0))), "{points:?}");
    // Only the points of the elements digitizing made.
    let run = ElementRef::from_index(1);
    let element = |stitch: &SewnStitch| sewn.blocks[stitch.block].stitches[stitch.index].origin.element;
    let points = candidates(thickness.sewn(), &near, |stitch| element(stitch) == run);
    assert_eq!(points, [(0, at(8.0)), (0, at(0.5))]);
}

#[test]
fn ribbons_index_the_cells_a_stitch_covers_and_charge_one_unit_each() {
    let mut ribbons = Ribbons::default();
    let ribbon = |from: (f64, f64), to: (f64, f64), at: Entry| Ribbon::of((p(from.0, from.1), p(to.0, to.1)), (0.0, 0.0), at, None);
    let mut meter = Budget::DEFAULT.meter();
    ribbons.add(ribbon((2.5, 3.5), (4.5, 3.5), (0, 1)), &mut meter).unwrap();
    assert_eq!(Budget::DEFAULT.max_work - meter.work_left(), 3);
    assert_eq!(ribbons.cells.keys().copied().collect::<Vec<_>>(), [(2, 3), (3, 3), (4, 3)]);
    ribbons.add(ribbon((2.5, 3.6), (4.5, 3.6), (0, 2)), &mut meter).unwrap();
    // A place under both: each place along a stitch of 0.1 mm, 3 of them, looks at the 2 stitches of its cell.
    let along = SewnStitch { block: 0, index: 9, from: p(3.2, 3.5), to: p(3.3, 3.5), role: Role::Underlay, from_role: Role::Underlay };
    let mut meter = Budget::DEFAULT.meter();
    assert!(ribbons.covers(&along, |_| true, false, &mut meter).unwrap());
    assert_eq!(Budget::DEFAULT.max_work - meter.work_left(), 9);
    // Ended, a stitch covers nothing.
    ribbons.kill((0, 1));
    ribbons.kill((0, 2));
    assert!(!ribbons.covers(&along, |_| true, false, &mut Budget::DEFAULT.meter()).unwrap());
    // A place in no cell is not covered.
    let away = SewnStitch { from: p(9.0, 9.0), to: p(9.1, 9.0), ..along };
    assert!(!ribbons.covers(&away, |_| true, false, &mut Budget::DEFAULT.meter()).unwrap());
}

#[test]
fn req_auto_100_a_stitch_taken_out_covers_nothing_after() {
    // A shape sews a turn twice over 4 fills of an earlier one: the second turn's point goes, as the first
    // covers it, and then the first's stays, as the second no longer covers it.
    let twice = vec![(1.0, 1.5), (3.0, 2.0), (1.0, 2.5), (1.0, 1.5), (3.0, 2.0), (1.0, 2.5)];
    let mut sewn = plan(&[("base", Role::Top, zigzag(4.0, 0.0625)), ("twice", Role::Top, twice)]);
    let (capped, kept, measured) = cap(&mut sewn, &rules());
    assert_eq!(capped.unwrap().times, [0, 0, 1, 0]);
    assert!(measures_as(&kept, &measured));
    let turns = sewn.stitches().filter(|stitch| stitch.kind == StitchKind::Normal && stitch.at == p(3.0, 2.0)).count();
    assert_eq!(turns, 1);
}

/// A stitch of block 0 ending at entry `index`, from `from` to `to`, of `role`.
fn sewn(index: usize, from: (f64, f64), to: (f64, f64), role: Role) -> SewnStitch {
    SewnStitch { block: 0, index, from: p(from.0, from.1), to: p(to.0, to.1), role, from_role: role }
}

#[test]
fn req_auto_114_the_thread_runs_straight_on_within_10_degrees_from_where_it_ends() {
    let first = sewn(1, (0.0, 0.0), (2.0, 0.0), Role::Top);
    let turned = |degrees: f64| {
        let t = degrees.to_radians();
        sewn(2, (2.0, 0.0), (2.0 + 2.0 * math::cos(t), 2.0 * math::sin(t)), Role::Top)
    };
    assert!(straight_on(&first, &turned(0.0)) && straight_on(&first, &turned(9.9)));
    assert!(!straight_on(&first, &turned(10.1)) && !straight_on(&first, &turned(90.0)));
    // Only from where it ends, in its block, and with a length.
    assert!(!straight_on(&first, &sewn(2, (2.1, 0.0), (4.0, 0.0), Role::Top)));
    assert!(!straight_on(&first, &SewnStitch { block: 1, ..sewn(2, (2.0, 0.0), (4.0, 0.0), Role::Top) }));
    assert!(!straight_on(&first, &sewn(2, (2.0, 0.0), (2.0, 0.0), Role::Top)));
}

#[test]
fn req_auto_114_the_pull_moves_a_stitch_s_ends_in() {
    let (a, b) = (p(0.0, 0.0), p(4.0, 0.0));
    assert_eq!(pulled(a, b, 1.0, 1.0), Some((p(1.0, 0.0), p(3.0, 0.0))));
    assert_eq!(pulled(a, b, 0.0, 1.5), Some((a, p(2.5, 0.0))));
    assert_eq!(pulled(a, b, 2.5, 0.0), Some((p(2.5, 0.0), b)));
    assert_eq!(pulled(a, b, 2.0, 2.0), None);
    assert!(pulled(a, b, 1.99, 2.0).is_some());
    let ribbon = Ribbon::of((a, b), (0.5, 1.0), (0, 3), None);
    assert_eq!(
        (ribbon.pulled, ribbon.whole, ribbon.cut, ribbon.at, ribbon.alive),
        (Some((p(0.5, 0.0), p(3.0, 0.0))), (a, b), (0.5, 1.0), (0, 3), true)
    );
}

/// Rows 0.25 mm apart from x = 0 to x = 4, each in a stitch of 3 mm and one of 1 mm, under a zigzag of 4 fills
/// from x = `start` to x = 6: the row ends at x = 4 lie under the zigzag.
fn rows_under(start: f64) -> StitchPlan {
    let rows: Vec<(f64, f64)> = (0..=16)
        .flat_map(|k| {
            let y = f64::from(k) * 0.25;
            if k % 2 == 0 { vec![(0.0, y), (3.0, y), (4.0, y)] } else { vec![(4.0, y), (3.0, y), (0.0, y)] }
        })
        .collect();
    let over: Vec<(f64, f64)> = zigzag(6.0 - start, 0.0625).into_iter().map(|(x, y)| (x + start, y)).collect();
    plan(&[("under", Role::Top, rows), ("over", Role::Top, over)])
}

#[test]
fn req_auto_114_a_row_end_stays_where_a_later_column_pulled_in_leaves_the_row_before_it_bare() {
    // The zigzag is a satin column, whose stitches cover only as the fabric pulls it in.
    let column: BTreeSet<ElementRef> = ElementRef::from_index(1).into_iter().collect();
    let pull = Rules { overlap: 1.0, columns: &column, ..rules() };
    // The zigzag from x = 0.5, pulled in to x = 1.5, covers the last millimetre of each row's long stitch,
    // which turns into the joined stitch once the row's end goes: the ends at x = 4 go.
    let (capped, _, _) = cap(&mut rows_under(0.5), &pull);
    assert!(capped.unwrap().times[1] > 0);
    // From x = 2.2, pulled in to x = 3.2, it covers the row's last stitch but not the millimetre before it.
    let (capped, _, _) = cap(&mut rows_under(2.2), &pull);
    assert_eq!(capped.unwrap().times[1], 0);
    // As a fill's rows, which hide what they lie on, it takes them.
    let (capped, _, _) = cap(&mut rows_under(2.2), &rules());
    assert!(capped.unwrap().times[1] > 0);
}

#[test]
fn req_auto_112_a_joined_stitch_that_turns_45_degrees_or_more_is_a_notch() {
    let into = sewn(1, (0.0, 0.0), (2.0, 0.0), Role::Top);
    let towards = |x: f64, y: f64| Change::of(&into, &sewn(2, (0.0, 0.0), (x, y), Role::Top));
    let (on, turned, across, along) = (towards(3.0, 0.0), towards(2.0, 1.0), towards(1.0, 0.99), towards(0.0, 2.0));
    assert!(!on.notch && (on.cosine - 1.0).abs() < 1e-12);
    assert!(!turned.notch && !across.notch && along.notch);
    assert!(towards(1.0, 1.01).notch);
    // No notch beats a notch, and then the smaller turn.
    assert!(on.less_than(turned) && turned.less_than(across) && across.less_than(along) && turned.less_than(along));
    assert!(!along.less_than(turned) && !turned.less_than(turned) && !along.less_than(along));
    assert!(towards(0.0, 2.0).less_than(towards(-1.0, 1.0)));
}

#[test]
fn req_auto_112_a_column_s_points_go_only_above_the_warning_s_limit_and_until_it() {
    // One column sews the same zigzag 5 times: 5 fills, each pass covering the others' places as sewn.
    let points: Vec<(f64, f64)> = (0..5).flat_map(|_| zigzag(4.0, 0.25)).collect();
    let column: BTreeSet<ElementRef> = ElementRef::from_index(0).into_iter().collect();
    let as_column = Rules { cap: 3.0, limit: 4.0, columns: &column, ..rules() };
    let mut sewn = plan(&[("column", Role::Top, points.clone())]);
    let (capped, _, measured) = cap(&mut sewn, &as_column);
    let capped = capped.unwrap();
    assert!(capped.times[..3] == [0, 0, 0] && capped.times[3] > 0, "{capped:?}");
    let left = measured.most(Measure::Fills).value;
    assert!(left <= 4.0 && left > 3.5, "{left}");
    // Under the warning's limit the column stays as it is.
    let mut kept = plan(&[("column", Role::Top, points)]);
    let (capped, _, _) = cap(&mut kept, &Rules { limit: 50.0, ..as_column });
    assert!(capped.is_none_or(|capped| capped.times == [0; 4]));
}

#[test]
fn req_auto_112_a_joined_stitch_s_turn_is_measured_from_the_stitch_into_the_point() {
    // From (1, 1): along x, and then up 27 degrees and up 63 degrees, wherever the stitches start.
    let into = sewn(1, (1.0, 1.0), (2.0, 1.0), Role::Top);
    let up = |to: (f64, f64)| Change::of(&into, &sewn(2, (1.0, 1.0), to, Role::Top));
    let (half, steep) = (up((3.0, 2.0)), up((1.5, 2.0)));
    assert!((half.cosine - 2.0 / 5.0_f64.sqrt()).abs() < 1e-12 && !half.notch, "{half:?}");
    assert!((steep.cosine - 0.5 / 1.25_f64.sqrt()).abs() < 1e-12 && steep.notch, "{steep:?}");
    // Across the other way the cosine is the same, and from a stitch of no length it is a notch.
    let down = Change::of(&sewn(1, (2.0, 2.0), (2.0, 3.0), Role::Top), &sewn(2, (2.0, 2.0), (1.0, 4.0), Role::Top));
    assert!((down.cosine - 2.0 / 5.0_f64.sqrt()).abs() < 1e-12 && !down.notch, "{down:?}");
    assert!(Change::of(&sewn(1, (1.0, 1.0), (1.0, 1.0), Role::Top), &sewn(2, (1.0, 1.0), (2.0, 1.0), Role::Top)).notch);
}

#[test]
fn req_auto_112_the_last_resort_takes_the_least_change_of_the_points_that_lower_the_spot() {
    let change = |notch: bool, cosine: f64, lowered: f64| Change { notch, cosine, lowered };
    // The least change of those that lower the spot by half the most or more: 0.06 of 0.1 counts, 0.04 not.
    let may = [(change(true, 0.2, 0.1), (0, 1)), (change(false, 0.9, 0.06), (0, 2)), (change(false, 0.99, 0.04), (0, 3))];
    assert_eq!(chosen(&may), Some((0, 2)));
    assert_eq!(chosen(&[(change(true, 0.2, 0.1), (0, 1)), (change(false, 0.9, 0.05), (0, 2))]), Some((0, 2)));
    assert_eq!(chosen(&[(change(true, 0.2, 3.0), (0, 1)), (change(false, 0.9, 1.6), (0, 2))]), Some((0, 2)));
    // None lowers it, or there is none.
    assert_eq!(chosen(&[(change(false, 0.9, 0.0), (0, 2))]), None);
    assert_eq!(chosen(&[]), None);
}

#[test]
fn req_auto_112_the_last_resort_takes_no_point_of_a_column_that_digitizing_did_not_make() {
    // A column sewn 5 times over itself, 5 fills: a satin column of a shape's own settings keeps its stitches.
    let points: Vec<(f64, f64)> = (0..5).flat_map(|_| zigzag(4.0, 0.25)).collect();
    let mut sewn = plan(&[("column", Role::Top, points)]);
    let was = sewn.clone();
    let mut thickness = Thickness::of(&sewn, &mut Budget::DEFAULT.meter()).unwrap().unwrap();
    let columns: BTreeSet<ElementRef> = ElementRef::from_index(0).into_iter().collect();
    let rules = Rules { columns: &columns, ..rules() };
    assert!(capped(&mut sewn, &mut thickness, &rules, &BTreeSet::new(), &Budget::DEFAULT).unwrap().is_none());
    assert_eq!(sewn, was);
    // Made by digitizing, it loses points down to the limit.
    let made = columns.clone();
    let mut thickness = Thickness::of(&sewn, &mut Budget::DEFAULT.meter()).unwrap().unwrap();
    let capped = capped(&mut sewn, &mut thickness, &rules, &made, &Budget::DEFAULT).unwrap().unwrap();
    assert!(capped.times[3] > 0, "{capped:?}");
}

#[test]
fn req_auto_121_a_lettering_column_s_pull_is_its_own_compensation_and_0_2_mm_at_least() {
    // 0.3 mm and 10 % of a stitch 2 mm long.
    assert!((Pull { mm: 0.3, share: 0.1 }.at(2.0) - 0.5).abs() < 1e-12);
    assert!((Pull { mm: 0.0, share: 0.0 }.at(5.0) - LEAST_PULL).abs() < 1e-12);
    // The larger of the 2 sides' each: 0.3 mm and 20 %.
    let set: stitchcraft_params::ParamSet = [("pull_compensation_mm", "0.1 0.3"), ("pull_compensation_percent", "20 10")].into_iter().collect();
    let pull = Pull::of(&crate::generators::pull::PullParams::from_set(&set).unwrap().params);
    assert_eq!(pull, Pull { mm: 0.3, share: 0.2 });
}

/// The elements at `indices`, each a lettering column pulled in by `mm`.
fn lettering(indices: &[usize], mm: f64) -> BTreeMap<ElementRef, Pull> {
    indices.iter().filter_map(|index| ElementRef::from_index(*index)).map(|element| (element, Pull { mm, share: 0.0 })).collect()
}

/// A zigzag `width` wide from x = `left`, as [`zigzag`] draws it.
fn zigzag_from(left: f64, width: f64, apart: f64) -> Vec<(f64, f64)> {
    zigzag(width, apart).into_iter().map(|(x, y)| (x + left, y)).collect()
}

#[test]
fn req_auto_122_a_lettering_column_s_underlay_goes_only_where_another_lettering_column_covers_it() {
    // A column's underlay, 2 mm wide inside its 4 mm, under its own top stitches: 5 fills.
    let own = || vec![("column", Role::Underlay, zigzag_from(1.0, 2.0, 0.25)), ("column", Role::Top, zigzag(4.0, 0.0625))];
    let columns: BTreeSet<ElementRef> = (0..2).filter_map(ElementRef::from_index).collect();
    let (one, both) = (lettering(&[0], 0.2), lettering(&[0, 1], 0.2));
    // A lettering column's own top holds its underlay up: it stays.
    let sewn = plan(&own());
    let mut kept = sewn.clone();
    let (capped, _, _) = cap(&mut kept, &Rules { columns: &columns, lettering: &one, ..rules() });
    assert!(capped.as_ref().is_none_or(|capped| capped.times[0] == 0), "{capped:?}");
    assert_eq!(count(&kept, Role::Underlay), count(&sewn, Role::Underlay));
    // Another lettering column over it as well: it goes.
    let mut crossed = own();
    crossed.push(("other", Role::Top, zigzag(4.0, 0.0625)));
    let sewn = plan(&crossed);
    let mut capped_plan = sewn.clone();
    let (capped, _, _) = cap(&mut capped_plan, &Rules { columns: &columns, lettering: &both, ..rules() });
    assert!(capped.unwrap().times[0] > 0);
    assert!(count(&capped_plan, Role::Underlay) < count(&sewn, Role::Underlay));
    // A column that isn't lettering loses its underlay under its own top.
    let mut capped_plan = plan(&own());
    let (capped, _, _) = cap(&mut capped_plan, &Rules { columns: &columns, ..rules() });
    assert!(capped.unwrap().times[0] > 0);
}

#[test]
fn req_auto_121_above_the_warning_s_limit_a_lettering_column_still_covers_only_as_pulled_in() {
    // A running stitch 3.8 mm wide, its points 0.5 mm apart, under a column 4 mm wide sewn after it: above 4
    // fills, where the cap judges cover as sewn. Its turns lie 0.1 mm inside the column's edges, and a thread
    // 0.4 mm thick: a column pulled in by 0.6 mm leaves them bare.
    let sewn = plan(&[("run", Role::Top, zigzag_from(0.1, 3.8, 0.5)), ("column", Role::Top, zigzag(4.0, 0.0625))]);
    let column: BTreeSet<ElementRef> = ElementRef::from_index(1).into_iter().collect();
    let pulled = lettering(&[1], 0.6);
    let thickest = Thickness::of(&sewn, &mut Budget::DEFAULT.meter()).unwrap().unwrap().most(Measure::Fills).value;
    assert!(thickest > 4.0, "{thickest}");
    // A lettering column, pulled in, leaves the turns bare: they stay.
    let mut kept = sewn.clone();
    let (capped, _, _) = cap(&mut kept, &Rules { columns: &column, lettering: &pulled, ..rules() });
    assert!(capped.as_ref().is_none_or(|capped| !capped.took()), "{capped:?}");
    assert_eq!(kept, sewn);
    // Another column, judged as sewn, covers them: they go.
    let mut capped_plan = sewn.clone();
    let (capped, _, _) = cap(&mut capped_plan, &Rules { columns: &column, ..rules() });
    assert!(capped.unwrap().took());
    assert!(count(&capped_plan, Role::Top) < count(&sewn, Role::Top));
}

/// A column that fans: its stitches run from points 0.4 mm apart along x = 0 to points `crowd` mm apart
/// along x = 4, 21 of them, so that the second rail's points crowd and its thread piles up there.
fn fan(crowd: f64) -> Vec<(f64, f64)> {
    (0..=20).map(|k| if k % 2 == 0 { (4.0, crowd / 2.0 * f64::from(k)) } else { (0.0, 0.2 * f64::from(k)) }).collect()
}

/// The fan's column, its element, and its shares when the cap may pull its points in.
fn fanned(crowd: f64) -> (StitchPlan, BTreeSet<ElementRef>, BTreeMap<ElementRef, Pullable>) {
    let sewn = plan(&[("fan", Role::Top, fan(crowd))]);
    let column: BTreeSet<ElementRef> = ElementRef::from_index(0).into_iter().collect();
    let pulls = column.iter().map(|element| (*element, Pullable { shares: vec![0.25, 0.5], spacing: 0.4 })).collect();
    (sewn, column, pulls)
}

#[test]
fn req_auto_171_the_cap_pulls_a_fan_s_crowded_points_in_above_the_limit() {
    let (mut sewn, column, pulls) = fanned(0.05);
    let before = Thickness::of(&sewn, &mut Budget::DEFAULT.meter()).unwrap().unwrap().most(Measure::Fills).value;
    assert!(before > 4.0, "{before}");
    let rules = Rules { cap: 3.0, limit: 4.0, columns: &column, pulls: &pulls, ..rules() };
    let mut pulled = sewn.clone();
    let (capped, kept, measured) = cap(&mut pulled, &rules);
    let capped = capped.unwrap();
    assert!(capped.pulled[0] > 0 && capped.pulled[1] > 0 && capped.took(), "{capped:?}");
    assert!(measures_as(&kept, &measured));
    assert!(measured.most(Measure::Fills).value < before);
    // Only the crowded rail's points move, in along their stitches, by half of them at the most; the run
    // keeps every point, in its order.
    let at = |plan: &StitchPlan| -> Vec<Point> { plan.stitches().filter(|s| s.kind == StitchKind::Normal).map(|s| s.at).collect() };
    let (was, now) = (at(&sewn), at(&pulled));
    let moved: Vec<(Point, Point)> = was.iter().zip(&now).filter(|(a, b)| a != b).map(|(a, b)| (*a, *b)).collect();
    assert_eq!(moved.len(), capped.pulled[0], "{moved:?}");
    assert!(moved.iter().all(|(a, b)| a.x() == 4.0 && b.x() < 4.0 && b.x() >= 2.0 - 0.05), "{moved:?}");
    // Without the shares, or under the warning's limit, nothing is pulled in.
    let (capped, _, _) = cap(&mut sewn.clone(), &Rules { pulls: NO_PULLS, ..rules });
    assert!(capped.is_none_or(|capped| capped.pulled == [0, 0]));
    let (capped, _, _) = cap(&mut sewn, &Rules { limit: 50.0, ..rules });
    assert!(capped.is_none_or(|capped| capped.pulled == [0, 0]));
}

#[test]
fn req_auto_171_a_cap_out_of_work_while_it_pulls_points_in_keeps_what_it_pulled() {
    let (sewn, column, pulls) = fanned(0.05);
    let rules = Rules { cap: 3.0, limit: 4.0, columns: &column, pulls: &pulls, ..rules() };
    let thickness = Thickness::of(&sewn, &mut Budget::DEFAULT.meter()).unwrap().unwrap();
    let enough = least(&sewn, &thickness, &rules);
    let mut pulled_some = 0;
    for max_work in 0..enough {
        let (mut plan, mut measure) = (sewn.clone(), thickness.clone());
        let Ok(Some(capped)) = capped(&mut plan, &mut measure, &rules, &all(&sewn), &Budget { max_work, ..Budget::DEFAULT }) else { continue };
        assert!(capped.ran_out, "{max_work}: {capped:?}");
        let measured = Thickness::of(&plan, &mut Budget::DEFAULT.meter()).unwrap().unwrap();
        assert!(measures_as(&measure, &measured), "{max_work}");
        pulled_some += usize::from(capped.pulled[0] > 0);
    }
    assert!(pulled_some > 0, "{enough}");
}

/// The spacing of a column whose points 0.4 mm apart lie just as far apart as crowds: 5 % past it.
const SPACED: f64 = 0.4 / (1.0 + crate::generators::satin::OFF);

/// The fan with `crowd`, ready for the cap, with `rules`: its plan and measure, given to `check`.
fn judged(crowd: f64, shares: &[f64], check: impl Fn(&mut Cap, Point, &mut Meter)) {
    spaced(crowd, shares, SPACED, check);
}

/// [`judged`], for a column with zigzag spacing `spacing`.
fn spaced(crowd: f64, shares: &[f64], spacing: f64, check: impl Fn(&mut Cap, Point, &mut Meter)) {
    let (mut sewn, column, _) = fanned(crowd);
    let pulls: BTreeMap<ElementRef, Pullable> = column.iter().map(|element| (*element, Pullable { shares: shares.to_vec(), spacing })).collect();
    let rules = Rules { cap: 3.0, limit: 4.0, columns: &column, pulls: &pulls, ..rules() };
    let mut thickness = Thickness::of(&sewn, &mut Budget::DEFAULT.meter()).unwrap().unwrap();
    let spot = thickness.most(Measure::Fills).at;
    let mut cap = Cap {
        pass: Pass::new(&mut sewn, &mut thickness, &rules),
        ribbons: Ribbons::default(),
        longest: BTreeMap::new(),
        taken: [0.0; 4],
        times: [0; 4],
        pulled: [0; 2],
    };
    check(&mut cap, spot, &mut Budget::DEFAULT.meter());
}

#[test]
fn req_auto_171_a_point_is_crowded_closer_than_its_column_s_spacing_to_its_rail_s_neighbour() {
    // The fan's points sit at entries 1 to 21 of its block, after its jump: entry 11 is on the crowded
    // rail, between 2 others there, and entry 12 on the open rail, 0.4 mm from its neighbours: 5 % past the
    // spacing, so not crowded.
    let crowded = (0, 11);
    let open = (0, 12);
    let moves = BTreeMap::new();
    for (crowd, pulled) in [(0.05, true), (0.3, true), (0.4, false), (0.5, false)] {
        judged(crowd, &[0.25, 0.5], |cap, _, meter| {
            let near = cap.pass.thickness.point(cap.pass.stitch(crowded).unwrap().0);
            let first = cap.pulled(crowded, 0, &moves, near, meter).unwrap();
            assert_eq!(first.is_some(), pulled, "{crowd}");
            assert!(cap.pulled(open, 0, &moves, near, meter).unwrap().is_none(), "{crowd}");
        });
    }
    // Points 0.4 mm apart in a column spaced 0.39 mm lie within 5 % of it, as a bend's inside keeps them
    // (`REQ-SAT-030`): crowded. In one spaced 0.37 mm they lie more than 5 % past it.
    for (spacing, pulled) in [(0.39, true), (0.37, false)] {
        spaced(0.4, &[0.25, 0.5], spacing, |cap, _, meter| {
            let near = cap.pass.thickness.point(cap.pass.stitch(crowded).unwrap().0);
            assert_eq!(cap.pulled(crowded, 0, &moves, near, meter).unwrap().is_some(), pulled, "{spacing}");
            assert_eq!(cap.pulled(open, 0, &moves, near, meter).unwrap().is_some(), pulled, "{spacing}");
        });
    }
    judged(0.05, &[0.25, 0.5], |cap, _, meter| {
        let here = cap.pass.stitch(crowded).unwrap().0;
        let near = cap.pass.thickness.point(here);
        // It moves a quarter of the way to the middle of its stitches' other ends, from where it first was.
        let pulled = cap.pulled(crowded, 0, &moves, near, meter).unwrap().unwrap();
        let (into, out) = (pulled.removed[0].from, pulled.removed[1].to);
        let wanted = here.lerp(into.lerp(out, 0.5), 0.25);
        assert!(pulled.to.distance(wanted) < 0.11, "{:?} against {wanted:?}", pulled.to);
        assert_eq!(pulled.from, here);
        let first = here.lerp(into.lerp(out, 0.5), 0.1);
        let again: BTreeMap<Entry, (usize, Point)> = [(crowded, (1, first))].into_iter().collect();
        let later = cap.pulled(crowded, 1, &again, near, meter).unwrap().unwrap();
        assert!(later.to.distance(first.lerp(into.lerp(out, 0.5), 0.5)) < 0.11, "{:?}", later.to);
        // Its shares used up, it stays.
        assert!(cap.pulled(crowded, 2, &again, near, meter).unwrap().is_none());
        // A spot farther than 1 mm from both its stitches is none of its business.
        let far = cap.pass.thickness.point(p(here.x() + 0.0, here.y() + 30.0));
        assert!(cap.pulled(crowded, 0, &moves, far, meter).unwrap().is_none());
        // Both its neighbours on its rail moved: it stays, unless it moved too.
        let both: BTreeMap<Entry, (usize, Point)> = [((0, 9), (1, here)), ((0, 13), (1, here))].into_iter().collect();
        assert!(cap.pulled(crowded, 0, &both, near, meter).unwrap().is_none());
        let one: BTreeMap<Entry, (usize, Point)> = [((0, 9), (1, here))].into_iter().collect();
        assert!(cap.pulled(crowded, 0, &one, near, meter).unwrap().is_some());
        let with: BTreeMap<Entry, (usize, Point)> = [((0, 9), (1, here)), ((0, 13), (1, here)), (crowded, (1, here))].into_iter().collect();
        assert!(cap.pulled(crowded, 1, &with, near, meter).unwrap().is_some());
        // How much it lowers the spot, and the measure as it was after.
        let was = cap.pass.thickness.at(Measure::Fills, near);
        let lowered = cap.lowers(&pulled, was, near, meter).unwrap();
        assert!(lowered > 0.0 && lowered < was, "{lowered}");
        assert_eq!(cap.pass.thickness.at(Measure::Fills, near), was);
    });
    // A column whose shares are none moves no point.
    judged(0.05, &[], |cap, spot, meter| assert!(cap.pulled(crowded, 0, &moves, spot, meter).unwrap().is_none()));
}

/// Whether the cap would pull in entry `point` of `sewn`, a column's whose shares and spacing are the
/// defaults, for the spot at `spot`, with the machine's shortest stitch `shortest`.
fn pulls_in(sewn: &mut StitchPlan, point: Entry, spot: (f64, f64), shortest: f64) -> bool {
    let column: BTreeSet<ElementRef> = ElementRef::from_index(0).into_iter().collect();
    let pulls: BTreeMap<ElementRef, Pullable> = column.iter().map(|element| (*element, Pullable { shares: vec![0.25, 0.5], spacing: 0.4 })).collect();
    let rules = Rules { cap: 3.0, limit: 4.0, columns: &column, pulls: &pulls, shortest, ..rules() };
    let mut thickness = Thickness::of(sewn, &mut Budget::DEFAULT.meter()).unwrap().unwrap();
    let cap = Cap {
        pass: Pass::new(sewn, &mut thickness, &rules),
        ribbons: Ribbons::default(),
        longest: BTreeMap::new(),
        taken: [0.0; 4],
        times: [0; 4],
        pulled: [0; 2],
    };
    cap.pulled(point, 0, &BTreeMap::new(), p(spot.0, spot.1), &mut Budget::DEFAULT.meter()).unwrap().is_some()
}

#[test]
fn req_auto_171_a_point_whose_stitches_pass_1_mm_from_the_spot_or_nearer_is_looked_at() {
    // The point at (4, 0), entry 3, crowds its rail's point 0.25 mm before it. Its stitch in runs along
    // y = 0 and its stitch out slants up to y = 0.25: a spot 1 mm below the first is near enough, and one
    // farther from both is not, whichever is the nearer.
    let level_in = || plan(&[("column", Role::Top, vec![(4.0, -0.25), (0.0, 0.0), (4.0, 0.0), (0.0, 0.25), (4.0, 0.25)])]);
    assert!(pulls_in(&mut level_in(), (0, 3), (2.0, -1.0), 0.3));
    assert!(!pulls_in(&mut level_in(), (0, 3), (2.0, -1.5), 0.3));
    let level_out = || plan(&[("column", Role::Top, vec![(4.0, -0.25), (0.0, -0.25), (4.0, 0.0), (0.0, 0.0), (4.0, 0.25)])]);
    assert!(pulls_in(&mut level_out(), (0, 3), (2.0, 1.0), 0.3));
    assert!(pulls_in(&mut level_out(), (0, 3), (2.0, 0.5), 0.3));
    assert!(!pulls_in(&mut level_out(), (0, 3), (2.0, 1.5), 0.3));
    // A stitch moved in no shorter than the machine's shortest: a quarter of 4 mm in leaves 3 mm.
    assert!(!pulls_in(&mut level_in(), (0, 3), (2.0, -1.0), 3.5));
}

#[test]
fn req_auto_171_a_point_between_its_own_column_s_top_stitches_only_is_looked_at() {
    // A column whose top starts after its underlay and runs straight on into another column's top, with no
    // jump: its first top point comes from its underlay, and its last goes to the other column.
    let mut builder = PlanBuilder::new(Thread::new(Rgb::new(0, 0, 0)));
    let (one, two) = (builder.element(ElementId::new("one").unwrap()).unwrap(), builder.element(ElementId::new("two").unwrap()).unwrap());
    builder.jump(p(4.0, -1.0), Provenance::new(one, Role::Travel));
    for (x, y) in [(4.0, -1.0), (0.0, -1.0)] {
        builder.stitch(p(x, y), Provenance::new(one, Role::Underlay));
    }
    for (x, y) in [(4.0, -0.5), (0.0, -0.25), (4.0, -0.25), (0.0, 0.0), (4.0, 0.0)] {
        builder.stitch(p(x, y), Provenance::new(one, Role::Top));
    }
    for (x, y) in [(0.0, 0.25), (4.0, 0.25)] {
        builder.stitch(p(x, y), Provenance::new(two, Role::Top));
    }
    let mut sewn = builder.finish();
    // Entries: the jump, 2 of underlay, then the top from entry 3 to 7, and the other column's.
    assert!(pulls_in(&mut sewn.clone(), (0, 5), (2.0, -0.25), 0.3), "a point between 2 of its own top stitches");
    assert!(!pulls_in(&mut sewn.clone(), (0, 3), (2.0, -0.5), 0.3), "from its underlay");
    assert!(!pulls_in(&mut sewn, (0, 7), (2.0, 0.0), 0.3), "to the other column");
}
