// Test data: small counts of rows, from widths given in whole steps.
#![allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]

use stitchcraft_core::{Budget, ElementId};
use stitchcraft_plan::{PlanBuilder, Provenance, Rgb, Thread};

use super::*;

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

/// `times` stitches back and forth between `a` and `b`.
fn knot(a: (f64, f64), b: (f64, f64), times: usize) -> Vec<(f64, f64)> {
    (0..=times).map(|k| if k % 2 == 0 { a } else { b }).collect()
}

/// The rows of a fill from x = `from` to x = `to`, `rows` of them `apart` from y = 0, each in 2 stitches.
fn rows(from: f64, to: f64, rows: usize, apart: f64) -> Vec<(f64, f64)> {
    let mut points = Vec::new();
    for row in 0..rows {
        let y = row as f64 * apart;
        let (a, b) = if row % 2 == 0 { (from, to) } else { (to, from) };
        points.extend([(a, y), ((a + b) / 2.0, y), (b, y)]);
    }
    points
}

/// The rows of a fill from y = -1 to y = 4, from x = `from`, 0.25 mm apart, as wide as `width`.
fn columns(from: f64, width: f64) -> Vec<(f64, f64)> {
    let count = (width / 0.25) as usize;
    (0..count)
        .flat_map(|k| {
            let x = from + k as f64 * 0.25;
            if k % 2 == 0 { [(x, -1.0), (x, 1.5), (x, 4.0)] } else { [(x, 4.0), (x, 1.5), (x, -1.0)] }
        })
        .collect()
}

const NONE: &BTreeSet<ElementRef> = &BTreeSet::new();

fn rules(base: &BTreeSet<ElementRef>) -> Rules<'_> {
    Rules { cap: 4.0, limit: 4.0, base, columns: NONE, overlap: 1.0, shortest: 0.3, longest: 12.1 }
}

/// `plan` thinned with `rules`, its measure kept up to date, and the measure of what the pass left.
fn thin(plan: &mut StitchPlan, rules: &Rules) -> (Option<Thinned>, Thickness, Thickness) {
    let mut thickness = Thickness::of(plan, &mut Budget::DEFAULT.meter()).unwrap().unwrap();
    let thinned = thinned(plan, &mut thickness, rules, &mut Budget::DEFAULT.meter()).unwrap();
    let again = Thickness::of(plan, &mut Budget::DEFAULT.meter()).unwrap().unwrap();
    (thinned, thickness, again)
}

/// The entries of `plan` of `role`, by their needle points.
fn of(plan: &StitchPlan, role: Role) -> Vec<(f64, f64)> {
    plan.stitches()
        .filter(|stitch| stitch.kind == StitchKind::Normal && stitch.origin.role == role)
        .map(|stitch| (stitch.at.x(), stitch.at.y()))
        .collect()
}

/// A thick place round (0.5, 0.5): a stitch sewn 30 times over, 7.5 fills.
fn thick() -> Part<'static> {
    ("thick", Role::Top, knot((0.0, 0.5), (1.0, 0.5), 30))
}

#[test]
fn req_auto_074_locks_keep_one_back_and_forth_where_a_spot_is_above_the_cap() {
    let lock = knot((0.5, 0.5), (0.8, 0.5), 4);
    let mut sewn = plan(&[thick(), ("tied", Role::Lock, lock.clone()), ("tied", Role::Top, vec![(0.5, 3.0), (0.5, 6.0)])]);
    let (thinned, kept, measured) = thin(&mut sewn, &rules(NONE));
    let thinned = thinned.unwrap();
    // Of 4 stitches of 0.3 mm, 2 are left: out and back.
    assert_eq!(of(&sewn, Role::Lock), [(0.5, 0.5), (0.8, 0.5), (0.5, 0.5)]);
    assert_eq!(thinned.times, [0, 1, 0, 0]);
    assert!((thinned.taken[1] - 0.6).abs() < 1e-9, "{:?}", thinned.taken);
    // The measure kept is the plan's as it is left.
    assert_eq!(kept.sewn(), measured.sewn());
    assert!((kept.most(Measure::Fills).value - measured.most(Measure::Fills).value).abs() < 1e-9);
    assert!(thinned.left.area <= thinned.above.area && thinned.thickest.value > rules(NONE).cap);
    assert_eq!(kinds(&thinned), "0.6 mm of lock stitches in 1 place");
    // Away from a thick place a lock is kept whole, and a plan within the cap is not gone through.
    let mut far = plan(&[thick(), ("tied", Role::Lock, knot((5.5, 0.5), (5.8, 0.5), 4)), ("tied", Role::Top, vec![(5.5, 3.0)])]);
    let (thinned, _, _) = thin(&mut far, &rules(NONE));
    assert_eq!((thinned.unwrap().times, of(&far, Role::Lock).len()), ([0; 4], 5));
    let mut thin_plan = plan(&[("thin", Role::Top, knot((0.0, 0.5), (1.0, 0.5), 10)), ("tied", Role::Lock, lock)]);
    assert_eq!(thin(&mut thin_plan, &rules(NONE)).0, None);
}

#[test]
fn req_auto_074_travel_out_and_back_goes_and_a_top_stitch_out_and_back_stays() {
    let travel = vec![(0.5, 0.4), (0.5, -3.0), (3.0, -3.0)];
    let mut sewn =
        plan(&[thick(), ("way", Role::Top, vec![(0.5, -3.0)]), ("way", Role::Travel, travel), ("bean", Role::Top, knot((0.2, 0.0), (0.2, 1.0), 2))]);
    let (thinned, _, _) = thin(&mut sewn, &rules(NONE));
    let thinned = thinned.unwrap();
    assert_eq!(of(&sewn, Role::Travel), [(3.0, -3.0)]);
    assert_eq!(thinned.times, [1, 0, 0, 0]);
    assert!((thinned.taken[0] - 6.8).abs() < 1e-9, "{:?}", thinned.taken);
    assert_eq!(of(&sewn, Role::Top).iter().filter(|(x, _)| *x == 0.2).count(), 3);
    assert_eq!(kinds(&thinned), "6.8 mm of travel sewn out and back in 1 place");
    // Where the stitch that would join the way's ends is shorter than the machine's shortest, 2.5 mm against
    // 2.6 here, the way stays.
    let way = || plan(&[thick(), ("way", Role::Top, vec![(0.5, -3.0)]), ("way", Role::Travel, vec![(0.5, 0.4), (0.5, -3.0), (3.0, -3.0)])]);
    assert_eq!(thin(&mut way(), &Rules { shortest: 2.6, ..rules(NONE) }).0.unwrap().times, [0; 4]);
    assert_eq!(thin(&mut way(), &Rules { shortest: 2.5, ..rules(NONE) }).0.unwrap().times, [1, 0, 0, 0]);
    // And where it would be longer than the machine's longest, 2.5 mm against 2.4, it stays too.
    assert_eq!(thin(&mut way(), &Rules { longest: 2.4, ..rules(NONE) }).0.unwrap().times, [0; 4]);
    assert_eq!(thin(&mut way(), &Rules { longest: 2.5, ..rules(NONE) }).0.unwrap().times, [1, 0, 0, 0]);
    // A way that comes back 0.2 mm off, further than a lane, is no way out and back.
    let mut wide = plan(&[thick(), ("way", Role::Top, vec![(0.5, -3.0)]), ("way", Role::Travel, vec![(0.5, 0.4), (0.7, -3.0), (3.0, -3.0)])]);
    assert_eq!(thin(&mut wide, &rules(NONE)).0.unwrap().times, [0; 4]);
}

/// A fill's rows from x = 0 to x = 6, 0.25 mm apart, under a later fill from x = 4 to x = 8, and a thick
/// place on the seam round (5.5, 1.5), sewn last as underlay, which hides nothing.
fn seam(role: Role, apart: f64, covered: bool) -> StitchPlan {
    let mut parts = vec![("under", role, rows(0.0, 6.0, (3.0 / apart) as usize + 1, apart))];
    if covered {
        parts.push(("over", Role::Top, columns(4.0, 4.0)));
    }
    parts.push(("thick", Role::Underlay, knot((5.0, 1.5), (6.0, 1.5), 16)));
    plan(&parts)
}

/// The row ends of `plan`'s first element on the right, by their x.
fn right_ends(plan: &StitchPlan, role: Role) -> Vec<f64> {
    plan.blocks[0]
        .stitches
        .iter()
        .filter(|stitch| stitch.origin.element == ElementRef::from_index(0) && stitch.origin.role == role)
        .map(|stitch| stitch.at.x())
        .filter(|x| *x > 4.5)
        .collect()
}

#[test]
fn req_auto_074_row_ends_under_a_later_shape_move_back_by_the_overlap_where_a_spot_is_above_the_cap() {
    let mut sewn = seam(Role::Top, 0.25, true);
    let before = Thickness::of(&sewn, &mut Budget::DEFAULT.meter()).unwrap().unwrap().most(Measure::Fills).value;
    let (thinned, kept, measured) = thin(&mut sewn, &rules(NONE));
    let thinned = thinned.unwrap();
    assert!(thinned.times[3] > 0 && thinned.times[..3] == [0, 0, 0], "{thinned:?}");
    let ends = right_ends(&sewn, Role::Top);
    // Near the thick place the ends moved back by the overlap, 1 mm; further off they stayed at x = 6.
    assert!(ends.contains(&5.0) && ends.contains(&6.0), "{ends:?}");
    assert!(ends.iter().all(|x| *x == 5.0 || *x == 6.0), "{ends:?}");
    // Each move takes 2 mm of thread: 1 mm of each row.
    assert!((thinned.taken[3] - 2.0 * thinned.times[3] as f64).abs() < 1e-6, "{thinned:?}");
    assert!(thinned.above.area > 0.0 && thinned.left.area < thinned.above.area, "{thinned:?}");
    let after = measured.most(Measure::Fills).value;
    assert!(after < before && (kept.most(Measure::Fills).value - after).abs() < 1e-9, "{before} {after}");
    // Without the later shape over them the row ends show, and stay.
    let mut shown = seam(Role::Top, 0.25, false);
    let (thinned, _, _) = thin(&mut shown, &rules(NONE));
    assert_eq!(thinned.unwrap().times, [0; 4]);
    assert!(right_ends(&shown, Role::Top).iter().all(|x| *x == 6.0));
    // A turn shorter than the machine's shortest stitch, 0.25 mm here, may stay as short, and the rows keep
    // the shortest stitch: with 2.5 mm of 3 kept, they move by half a millimetre.
    let mut kept = seam(Role::Top, 0.25, true);
    thin(&mut kept, &Rules { shortest: 2.5, ..rules(NONE) });
    let ends = right_ends(&kept, Role::Top);
    assert!(ends.contains(&5.5) && ends.iter().all(|x| *x == 5.5 || *x == 6.0), "{ends:?}");
    // With less overlap they move less far.
    let mut short = seam(Role::Top, 0.25, true);
    thin(&mut short, &Rules { overlap: 0.5, ..rules(NONE) });
    assert!(right_ends(&short, Role::Top).iter().all(|x| *x == 5.5 || *x == 6.0));
}

#[test]
fn req_auto_114_a_satin_column_hides_no_row_end_of_the_pass() {
    // The same seam, the later shape a satin column: below the warning's limit the rows' ends under it keep
    // its edge covered once the fabric pulls it in, and stay.
    let mut sewn = seam(Role::Top, 0.25, true);
    let column: BTreeSet<ElementRef> = ElementRef::from_index(1).into_iter().collect();
    let (thinned, _, _) = thin(&mut sewn, &Rules { columns: &column, limit: 50.0, ..rules(NONE) });
    assert_eq!(thinned.unwrap().times[3], 0);
    assert!(right_ends(&sewn, Role::Top).iter().all(|x| *x == 6.0));
    // At it, they stay too.
    let mut at = seam(Role::Top, 0.25, true);
    let most = Thickness::of(&at, &mut Budget::DEFAULT.meter()).unwrap().unwrap().most(Measure::Fills).value;
    let (thinned, _, _) = thin(&mut at, &Rules { columns: &column, cap: 2.0, limit: most, ..rules(NONE) });
    assert_eq!(thinned.unwrap().times[3], 0);
    // Above it, as a last resort, the column hides them as any later shape does (`REQ-AUTO-112`).
    let mut hot = seam(Role::Top, 0.25, true);
    let (thinned, _, _) = thin(&mut hot, &Rules { columns: &column, ..rules(NONE) });
    assert!(thinned.unwrap().times[3] > 0);
    assert!(right_ends(&hot, Role::Top).contains(&5.0));
}

#[test]
fn req_auto_074_the_base_layer_s_row_ends_move_first_and_only_the_base_layer_s() {
    let base: BTreeSet<ElementRef> = ElementRef::from_index(0).into_iter().collect();
    let mut sewn = seam(Role::Underlay, 0.8, true);
    let (thinned, _, _) = thin(&mut sewn, &rules(&base));
    let thinned = thinned.unwrap();
    assert!(thinned.times[2] > 0, "{thinned:?}");
    assert!(right_ends(&sewn, Role::Underlay).contains(&5.0));
    // An element's own underlay is not the base layer's.
    let mut own = seam(Role::Underlay, 0.8, true);
    let (thinned, _, _) = thin(&mut own, &rules(NONE));
    assert_eq!(thinned.unwrap().times[2], 0);
    assert!(right_ends(&own, Role::Underlay).iter().all(|x| *x == 6.0));
}

#[test]
fn req_auto_074_the_pass_stops_when_the_work_runs_out() {
    for sewn in [seam(Role::Top, 0.25, true), plan(&[thick(), ("tied", Role::Lock, knot((0.5, 0.5), (0.8, 0.5), 4))])] {
        let thickness = Thickness::of(&sewn, &mut Budget::DEFAULT.meter()).unwrap().unwrap();
        let run = |max_work: u64| {
            thinned(&mut sewn.clone(), &mut thickness.clone(), &rules(NONE), &mut Budget { max_work, ..Budget::DEFAULT }.meter()).is_ok()
        };
        let enough = (0..1_000_000u64).find(|work| run(*work)).unwrap();
        assert!((0..enough).all(|work| !run(work)), "{enough}");
    }
}

#[test]
fn req_auto_074_a_way_back_a_lane_off_goes_and_one_further_off_stays() {
    // The way comes back 0.1 mm from where it was 2 needle points before, a lane: it went out and back.
    let way = |back: f64| plan(&[thick(), ("way", Role::Top, vec![(0.1, -3.0)]), ("way", Role::Travel, vec![(0.1, 0.4), (back, -3.0), (3.0, -3.0)])]);
    assert_eq!(thin(&mut way(0.2), &rules(NONE)).0.unwrap().times, [1, 0, 0, 0]);
    assert_eq!(thin(&mut way(0.3), &rules(NONE)).0.unwrap().times, [0; 4]);
}

#[test]
fn req_auto_074_rows_of_2_shapes_in_turn_keep_their_ends() {
    // The rows of the seam, every other one another shape's in the same role: a turn between 2 shapes is
    // no row's end.
    let points = rows(0.0, 6.0, 13, 0.25);
    let mut parts: Vec<Part> = points.chunks(3).enumerate().map(|(k, row)| (if k % 2 == 0 { "a" } else { "b" }, Role::Top, row.to_vec())).collect();
    parts.push(("over", Role::Top, columns(4.0, 4.0)));
    parts.push(("thick", Role::Underlay, knot((5.0, 1.5), (6.0, 1.5), 16)));
    let mut sewn = plan(&parts);
    assert_eq!(thin(&mut sewn, &rules(NONE)).0.unwrap().times, [0; 4]);
}

#[test]
fn req_auto_074_a_row_s_end_moves_by_what_its_last_stitch_leaves_of_the_shortest_and_no_less_than_a_step() {
    // Rows whose last stitch to the right is 1.5 mm and whose next row's first is 3 mm: with a shortest
    // stitch of 0.9 mm, the end moves back by 0.6 mm, what the shorter leaves.
    let mut points = Vec::new();
    for row in 0..13 {
        let y = f64::from(row) * 0.25;
        points.extend(if row % 2 == 0 { [(0.0, y), (4.5, y), (6.0, y)] } else { [(6.0, y), (3.0, y), (0.0, y)] });
    }
    let sewn = || {
        plan(&[
            ("under", Role::Top, points.clone()),
            ("over", Role::Top, columns(4.0, 4.0)),
            ("thick", Role::Underlay, knot((5.0, 1.5), (6.0, 1.5), 16)),
        ])
    };
    let mut moved = sewn();
    thin(&mut moved, &Rules { shortest: 0.9, ..rules(NONE) });
    let ends = right_ends(&moved, Role::Top);
    assert!(ends.iter().any(|x| (x - 5.4).abs() < 1e-9) && ends.iter().all(|x| (x - 5.4).abs() < 1e-9 || *x == 6.0), "{ends:?}");
    // A move shorter than a step of the grid, 0.03 mm, is not made, and not counted.
    let mut kept = seam(Role::Top, 0.25, true);
    assert_eq!(thin(&mut kept, &Rules { shortest: 2.97, ..rules(NONE) }).0.unwrap().times, [0; 4]);
}

#[test]
fn a_run_ends_at_its_block_s_first_entry_and_a_jump_and_a_top_stitch_after_a_jump_covers_nothing() {
    let mut sewn = plan(&[thick(), ("over", Role::Top, vec![(0.5, 3.0), (0.5, 6.0)])]);
    let mut thickness = Thickness::of(&sewn, &mut Budget::DEFAULT.meter()).unwrap().unwrap();
    let rules = rules(NONE);
    let pass = Pass { plan: &mut sewn, thickness: &mut thickness, rules: &rules, gone: BTreeSet::new(), taken: [0.0; 4], times: [0; 4] };
    let mut meter = Budget::DEFAULT.meter();
    // Entry 0 is the jump to the first stitch, entry 1 that stitch: nothing is before them.
    assert_eq!(pass.beside((0, 0), false, &mut meter).unwrap(), None);
    assert_eq!(pass.beside((0, 1), false, &mut meter).unwrap(), None);
    assert_eq!(pass.beside((0, 1), true, &mut meter).unwrap(), Some(2));
    // The top stitch after the jump to `over` lays no thread of its own.
    let over = pass.plan.blocks[0].stitches.iter().position(|stitch| stitch.at == p(0.5, 3.0) && stitch.kind == StitchKind::Normal).unwrap();
    let mut cover = [pass.thickness.blank(), pass.thickness.blank()];
    pass.covering(&mut cover, &[(0, over)], &mut meter).unwrap();
    assert!(cover.iter().all(|cover| cover.most(Measure::Fills).value == 0.0));
    pass.covering(&mut cover, &[(0, over + 1)], &mut meter).unwrap();
    assert!(cover.iter().all(|cover| cover.most(Measure::Fills).value > 0.0));
}

/// Whether `kept`, the measure the pass kept up to date, measures as `measured`, the plan's measured again,
/// at every half millimetre round the origin.
fn measures_as(kept: &Thickness, measured: &Thickness) -> bool {
    (-12..=12).all(|x| {
        (-12..=12).all(|y| {
            let place = p(f64::from(x) * 0.5, f64::from(y) * 0.5);
            [Measure::Fills, Measure::Points].into_iter().all(|measure| (kept.at(measure, place) - measured.at(measure, place)).abs() < 1e-9)
        })
    })
}

#[test]
fn req_auto_074_a_lock_after_a_top_stitch_joins_it_with_a_lock_s_shortest_stitch() {
    // The lock's first point comes back to the top stitch before it, 0.1 mm off, and the stitch that would
    // join them, 0.32 mm, is shorter than the machine's shortest, 0.35 mm, but no shorter than a lock's.
    let tied = || {
        let lock = vec![(0.8, 0.5), (0.5, 0.6), (0.8, 0.6), (0.5, 0.5)];
        plan(&[thick(), ("tied", Role::Top, vec![(0.5, 0.5)]), ("tied", Role::Lock, lock), ("tied", Role::Top, vec![(0.5, 3.0)])])
    };
    let mut sewn = tied();
    let (thinned, kept, measured) = thin(&mut sewn, &Rules { shortest: 0.35, ..rules(NONE) });
    assert_eq!(thinned.unwrap().times, [0, 1, 0, 0]);
    assert_eq!(of(&sewn, Role::Lock), [(0.8, 0.6), (0.5, 0.5)]);
    assert!(measures_as(&kept, &measured));
}

#[test]
fn req_auto_074_travel_out_and_back_after_a_lock_measures_as_the_plan_does() {
    // The way out and back starts at a lock's last point: the stitch that joins it to the next top stitch
    // comes out of a lock, and no needle point is counted for it, as the plan's measure counts none.
    let mut sewn = plan(&[
        thick(),
        ("tied", Role::Top, vec![(0.5, 2.0)]),
        ("tied", Role::Lock, knot((0.5, 0.5), (0.8, 0.5), 2)),
        ("tied", Role::Travel, vec![(0.5, -3.0), (0.5, 0.4)]),
        ("tied", Role::Top, vec![(0.5, 1.0), (0.5, 4.0)]),
    ]);
    let (thinned, kept, measured) = thin(&mut sewn, &rules(NONE));
    assert_eq!(thinned.unwrap().times, [1, 0, 0, 0]);
    assert!(of(&sewn, Role::Travel).is_empty());
    assert!(measures_as(&kept, &measured));
}

#[test]
fn req_auto_074_2_rows_run_opposite_ways_within_about_25_degrees() {
    let stitch = |(x0, y0): (f64, f64), (x1, y1): (f64, f64)| SewnStitch {
        block: 0,
        index: 1,
        from: p(x0, y0),
        to: p(x1, y1),
        role: Role::Top,
        from_role: Role::Top,
    };
    // The cosine of the angle between them is -0.9 or less: turned 20 or 24 degrees from straight back, they
    // run opposite ways, and turned 26 or 30 degrees they don't.
    let a = stitch((1.0, 2.0), (4.0, 6.0));
    assert!(opposite(&a, &stitch((3.0, -1.0), (2.4196, -2.9139))));
    assert!(!opposite(&a, &stitch((3.0, -1.0), (2.7608, -2.9856))));
    let b = stitch((-2.0, 4.0), (2.3301, 1.5));
    assert!(opposite(&b, &stitch((5.0, 6.0), (-1.9617, 6.7317))));
    assert!(!opposite(&b, &stitch((5.0, 6.0), (-1.9829, 6.4883))));
}

/// The seam's rows, every other one running back from x = 6 turned by `tilt` degrees from straight back.
fn tilted(tilt: f64) -> StitchPlan {
    let (dx, dy) = (stitchcraft_core::math::cos(tilt.to_radians()), stitchcraft_core::math::sin(tilt.to_radians()));
    let mut points = Vec::new();
    for row in 0..13 {
        let y = f64::from(row) * 0.25;
        points.extend(if row % 2 == 0 {
            [(0.0, y), (3.0, y), (6.0, y)]
        } else {
            [(6.0, y), (6.0 - 3.0 * dx, y + 3.0 * dy), (6.0 - 6.0 * dx, y + 6.0 * dy)]
        });
    }
    plan(&[("under", Role::Top, points), ("over", Role::Top, columns(4.0, 4.0)), ("thick", Role::Underlay, knot((5.0, 1.5), (6.0, 1.5), 16))])
}

#[test]
fn req_auto_074_a_row_s_end_moves_where_its_turn_is_1_mm_or_shorter_and_the_rows_run_opposite_ways() {
    let moves = |sewn: &mut StitchPlan, rules: &Rules| thin(sewn, rules).0.unwrap().times[3];
    // Rows 1 mm apart turn by 1 mm, and their ends move where the thick place lies on their first turn; 1.1
    // mm apart they don't.
    let apart = |apart: f64| {
        let thick = ("thick", Role::Underlay, knot((5.0, apart / 2.0), (6.0, apart / 2.0), 16));
        plan(&[("under", Role::Top, rows(0.0, 6.0, 4, apart)), ("over", Role::Top, columns(4.0, 4.0)), thick])
    };
    assert!(moves(&mut apart(1.0), &rules(NONE)) > 0);
    assert_eq!(moves(&mut apart(1.1), &rules(NONE)), 0);
    // Rows that run back turned by 20 degrees move, and by 30 degrees they don't.
    assert!(moves(&mut tilted(20.0), &rules(NONE)) > 0);
    assert_eq!(moves(&mut tilted(30.0), &rules(NONE)), 0);
    // A move as long as a step of the grid, 0.1 mm, is made.
    assert!(moves(&mut seam(Role::Top, 0.25, true), &Rules { overlap: 0.1, ..rules(NONE) }) > 0);
}

#[test]
fn a_stitch_is_a_normal_entry_not_taken_out_and_the_next_one_passes_those_taken_out() {
    let mut sewn = plan(&[thick()]);
    let mut thickness = Thickness::of(&sewn, &mut Budget::DEFAULT.meter()).unwrap().unwrap();
    let rules = rules(NONE);
    let gone: BTreeSet<Entry> = [(0, 2)].into_iter().collect();
    let pass = Pass { plan: &mut sewn, thickness: &mut thickness, rules: &rules, gone, taken: [0.0; 4], times: [0; 4] };
    let mut meter = Budget::DEFAULT.meter();
    // Entry 0 is a jump, and entry 2 is taken out.
    assert!(pass.stitch((0, 0)).is_none() && pass.stitch((0, 2)).is_none() && pass.stitch((0, 1)).is_some());
    assert_eq!(pass.beside((0, 1), true, &mut meter).unwrap(), Some(3));
    assert_eq!(pass.beside((0, 3), false, &mut meter).unwrap(), Some(1));
}

#[test]
fn req_auto_074_a_change_takes_no_spot_at_the_warning_s_limit_or_under_above_it() {
    // A thick place round (0.5, 0.5), 7.5 fills, and a thinner one round (3.5, 0.5), 3.75 fills. A change that
    // takes 2 stitches of the thick place and adds 2 that run from it across the thinner one keeps the thick
    // place as it was and lays the thinner one at 4.25 fills: above the warning's limit, 4 here, where it was
    // under it. That the stitches added cross a spot thicker still doesn't allow it.
    let mut sewn = plan(&[thick(), ("thin", Role::Top, knot((3.0, 0.5), (4.0, 0.5), 15))]);
    let mut thickness = Thickness::of(&sewn, &mut Budget::DEFAULT.meter()).unwrap().unwrap();
    let rules = rules(NONE);
    let mut meter = Budget::DEFAULT.meter();
    let removed: Vec<SewnStitch> = thickness.sewn().iter().filter(|stitch| stitch.role == Role::Top).take(2).copied().collect();
    let at = |x: f64| thickness.at(Measure::Fills, p(x, 0.5));
    let (thick_was, thin_was) = (at(0.5), at(3.5));
    assert!(thick_was > 7.0 && (3.5..4.0).contains(&thin_was), "{thick_was} {thin_was}");
    let added: Vec<SewnStitch> =
        removed.iter().map(|stitch| SewnStitch { from: thickness.placed(p(0.0, 0.5)), to: thickness.placed(p(4.0, 0.5)), ..*stitch }).collect();
    let mut pass = Pass::new(&mut sewn, &mut thickness, &rules);
    assert!(!pass.changed(&removed, &added, &mut meter).unwrap());
    // The measure stays as it was.
    let (thick_now, thin_now) = (pass.thickness.at(Measure::Fills, p(0.5, 0.5)), pass.thickness.at(Measure::Fills, p(3.5, 0.5)));
    assert!((thick_now - thick_was).abs() < 1e-9 && (thin_now - thin_was).abs() < 1e-9);
}
