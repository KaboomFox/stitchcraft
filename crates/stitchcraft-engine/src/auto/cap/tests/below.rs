//! The cap below the limit (`REQ-AUTO-194`, `REQ-AUTO-195`): it goes on past a spot it can't lower, and with
//! `auto_short_stitches` `knots` it pulls in a column's knots, and points whose new end a later shape covers,
//! below the warning's limit, each by its first share, keeping its measure up to date as the plan measured
//! again has it.

use super::*;
use crate::auto::cap::hot::Hot;

/// The shares of a column pulled in by its insets, 25 and 50.
const SHARES: [f64; 2] = [0.25, 0.5];

/// The columns of `column`, whose points the cap pulls in, below the limit too when `knots`.
fn pullable(column: &BTreeSet<ElementRef>, knots: bool) -> BTreeMap<ElementRef, Pullable> {
    column.iter().map(|element| (*element, Pullable { shares: SHARES.to_vec(), spacing: 0.4, knots })).collect()
}

/// The needle points of `plan` that moved, as they were and as they are in `now`.
fn moved(plan: &StitchPlan, now: &StitchPlan) -> Vec<(Point, Point)> {
    let at = |plan: &StitchPlan| -> Vec<Point> { plan.stitches().filter(|s| s.kind == StitchKind::Normal).map(|s| s.at).collect() };
    at(plan).into_iter().zip(at(now)).filter(|(a, b)| a != b).collect()
}

/// A pile of thread at x = `x`, 3 mm long, sewn 60 times: far thicker than the limit.
fn pile(x: f64) -> Vec<(f64, f64)> {
    (0..60).map(|k| (x, if k % 2 == 0 { 0.0 } else { 3.0 })).collect()
}

/// `check` run on the cap of `sewn` with `rules`, before it took anything, with every entry of its first
/// block as the points it may move or take.
fn on_cap(sewn: &mut StitchPlan, rules: &Rules, check: impl FnOnce(&mut Cap, &[Entry])) {
    let mut thickness = Thickness::of(sewn, &mut Budget::DEFAULT.meter()).unwrap().unwrap();
    let points: Vec<Entry> = (0..sewn.blocks[0].stitches.len()).map(|index| (0, index)).collect();
    // Every top stitch covers as sewn.
    // Every top stitch covers as sewn, and the longest stitch of each element is known, as the cap finds them.
    let (mut ribbons, mut longest) = (Ribbons::default(), BTreeMap::new());
    for stitch in thickness.sewn() {
        let element = sewn.blocks[stitch.block].stitches[stitch.index].origin.element;
        let most: &mut f64 = longest.entry((element, stitch.role == Role::Top)).or_default();
        *most = most.max(stitch.length());
        if stitch.role == Role::Top {
            ribbons
                .add(Ribbon::of((stitch.from, stitch.to), (0.0, 0.0), (stitch.block, stitch.index), element), &mut Budget::DEFAULT.meter())
                .unwrap();
        }
    }
    let mut cap = Cap {
        pass: Pass::new(sewn, &mut thickness, rules),
        ribbons,
        longest,
        taken: [0.0; 4],
        times: [0; 4],
        pulled: [0; 2],
        moves: BTreeMap::new(),
        refused: BTreeSet::new(),
        places: BTreeSet::new(),
    };
    check(&mut cap, &points);
}

#[test]
fn req_auto_195_below_the_limit_the_cap_pulls_a_knot_in_by_its_first_share() {
    // The fan's crowded rail has its points 0.08 mm apart, closer than half the spacing: a knot. It sews 3.90
    // fills, between the cap and the limit.
    let (mut sewn, column, _) = fanned(0.08);
    let before = Thickness::of(&sewn, &mut Budget::DEFAULT.meter()).unwrap().unwrap().most(Measure::Fills).value;
    assert!(before > 3.0 && before <= 4.0, "{before}");
    let pulls = pullable(&column, true);
    let rules = Rules { cap: 3.0, limit: 4.0, columns: &column, pulls: &pulls, ..rules() };
    let mut pulled = sewn.clone();
    let (capped, kept, measured) = cap(&mut pulled, &rules);
    let capped = capped.unwrap();
    assert!(capped.pulled[0] > 0 && capped.took() && !capped.ran_out, "{capped:?}");
    assert!(measures_as(&kept, &measured));
    assert!(measured.most(Measure::Fills).value <= 3.0, "{}", measured.most(Measure::Fills).value);
    assert_eq!(capped.thickest, measured.most(Measure::Fills));
    // Each moved point moved once, a quarter of the way from the crowded rail at x = 4 to the open one at 0.
    let moves = moved(&sewn, &pulled);
    assert_eq!(moves.len(), capped.pulled[0]);
    assert!(moves.iter().all(|(a, b)| a.x() == 4.0 && (b.x() - 3.0).abs() <= 0.05), "{moves:?}");
    // With `above_limit` nothing is pulled in under the limit.
    let pulls = pullable(&column, false);
    let (capped, _, _) = cap(&mut sewn.clone(), &Rules { pulls: &pulls, ..rules });
    assert!(capped.as_ref().is_none_or(|capped| capped.pulled == [0, 0]), "{capped:?}");
    // Nor at the cap or under it.
    let (capped, _, _) = cap(&mut sewn, &Rules { cap: before, ..rules });
    assert!(capped.as_ref().is_none_or(|capped| capped.pulled == [0, 0]), "{capped:?}");
}

#[test]
fn req_auto_195_below_the_limit_a_point_off_a_knot_moves_only_under_a_later_shape() {
    // The fan's crowded rail has its points 0.25 mm apart: crowded, as the inside of a bend, but no knot.
    // Its points, moved a quarter of the way in, end near x = 3. A run of another shape at x = 3.05 covers
    // those ends when it sews after the fan, and not before it, nor as the fan's own.
    let fan = fan(0.25);
    let over: Vec<(f64, f64)> = vec![(3.05, -1.0), (3.05, 5.0)];
    let pulled_in = |parts: &[Part], made: &[&str]| -> (usize, StitchPlan, StitchPlan) {
        let sewn = plan(parts);
        let column: BTreeSet<ElementRef> =
            made.iter().filter_map(|id| sewn.elements.iter().position(|e| e.as_str() == *id)).filter_map(ElementRef::from_index).collect();
        let pulls = pullable(&column, true);
        let rules = Rules { cap: 1.5, limit: 2.5, columns: &column, pulls: &pulls, ..rules() };
        let mut pulled = sewn.clone();
        let (capped, _, measured) = cap(&mut pulled, &rules);
        assert!(measured.most(Measure::Fills).value <= 2.5, "{}", measured.most(Measure::Fills).value);
        (capped.map_or(0, |capped| capped.pulled[0]), sewn, pulled)
    };
    let (alone, _, _) = pulled_in(&[("fan", Role::Top, fan.clone())], &["fan"]);
    assert_eq!(alone, 0);
    let (covered, sewn, pulled) = pulled_in(&[("fan", Role::Top, fan.clone()), ("over", Role::Top, over.clone())], &["fan"]);
    assert!(covered > 0, "{covered}");
    let moves = moved(&sewn, &pulled);
    assert!(moves.iter().all(|(a, b)| a.x() == 4.0 && (b.x() - 3.0).abs() <= 0.05), "{moves:?}");
    let (before, _, _) = pulled_in(&[("over", Role::Top, over.clone()), ("fan", Role::Top, fan.clone())], &["fan"]);
    assert_eq!(before, 0);
    let (own, _, _) = pulled_in(&[("fan", Role::Top, fan), ("fan", Role::Top, over)], &["fan"]);
    assert_eq!(own, 0);
}

#[test]
fn req_auto_195_a_pull_below_the_limit_makes_no_spot_above_the_limit_thicker() {
    // A pull lays thread in the spots beside the one it lowers. Held at a level, no spot it lays thread in
    // ends thicker than the larger of what it was and that level: at 0 every spot that gains thread refuses
    // it, and the measure stays as it was; at 50 none does.
    judged(0.05, &SHARES, |cap, _, meter| {
        let crowded = (0, 11);
        let near = cap.pass.thickness.point(cap.pass.stitch(crowded).unwrap().0);
        let pulled = cap.pulled(crowded, 0, &BTreeMap::new(), near, meter).unwrap().unwrap();
        let was = cap.pass.thickness.clone();
        assert!(!cap.pass.held(&pulled.removed, &pulled.added, Some(0.0), meter).unwrap());
        assert!(measures_as(cap.pass.thickness, &was));
        assert!(cap.pass.held(&pulled.removed, &pulled.added, Some(50.0), meter).unwrap());
        assert!(!measures_as(cap.pass.thickness, &was));
    });
    judged(0.05, &SHARES, |cap, _, meter| {
        let crowded = (0, 11);
        let near = cap.pass.thickness.point(cap.pass.stitch(crowded).unwrap().0);
        let pulled = cap.pulled(crowded, 0, &BTreeMap::new(), near, meter).unwrap().unwrap();
        assert!(cap.pass.held(&pulled.removed, &pulled.added, None, meter).unwrap());
    });
}

#[test]
fn req_auto_194_the_pull_in_goes_on_past_a_spot_no_point_lowers() {
    // A pile no point of the fan lays thread in is the thickest spot: the pull-in stops there, or goes on to
    // the fan's spot with `skip`.
    let mut sewn = plan(&[("pile", Role::Top, pile(30.0)), ("fan", Role::Top, fan(0.05))]);
    let column: BTreeSet<ElementRef> = ElementRef::from_index(1).into_iter().collect();
    let pulls = pullable(&column, false);
    let rules = Rules { cap: 3.0, limit: 4.0, columns: &column, pulls: &pulls, ..rules() };
    for (skip, pulls) in [(false, false), (true, true)] {
        on_cap(&mut sewn.clone(), &rules, |cap, points| {
            cap.pull_in(points, 4.0, skip, &mut Budget::DEFAULT.meter()).unwrap();
            assert_eq!(cap.pulled[0] > 0, pulls, "{skip}");
        });
    }
    // Each spot costs a unit of work for each spot of the measure as it is looked for.
    on_cap(&mut sewn, &rules, |cap, points| {
        let (columns, rows) = cap.pass.thickness.size();
        let mut meter = Budget { max_work: units(columns * rows), ..Budget::DEFAULT }.meter();
        assert!(cap.pull_in(points, 4.0, true, &mut meter).is_err());
    });
}

#[test]
fn req_auto_194_the_last_resort_goes_on_past_a_spot_no_point_lowers() {
    // A pile of a shape sewn by its own settings, which the cap may not touch, is the thickest spot. A column
    // sewing 5 fills beside it loses top points with `skip`, not without it; and the cap, which runs both,
    // takes them and leaves the column under the limit.
    let column_points: Vec<(f64, f64)> = (0..5).flat_map(|_| zigzag(4.0, 0.25)).collect();
    let sewn = plan(&[("pile", Role::Top, pile(30.0)), ("column", Role::Top, column_points)]);
    let column: BTreeSet<ElementRef> = ElementRef::from_index(1).into_iter().collect();
    let rules = Rules { cap: 3.0, limit: 4.0, columns: &column, ..rules() };
    for (skip, takes) in [(false, false), (true, true)] {
        on_cap(&mut sewn.clone(), &rules, |cap, points| {
            cap.last_resort(points, skip, &mut Budget::DEFAULT.meter()).unwrap();
            assert_eq!(cap.times[Kind::Column.at()] > 0, takes, "{skip}");
        });
    }
    let mut capped_plan = sewn.clone();
    let mut thickness = Thickness::of(&capped_plan, &mut Budget::DEFAULT.meter()).unwrap().unwrap();
    let done = capped(&mut capped_plan, &mut thickness, &rules, &column, &Budget::DEFAULT).unwrap().unwrap();
    assert!(done.times[Kind::Column.at()] > 0, "{done:?}");
    let at = |plan: &StitchPlan| Thickness::of(plan, &mut Budget::DEFAULT.meter()).unwrap().unwrap().at(Measure::Fills, p(2.0, 1.5));
    assert!(at(&sewn) > 4.0 && at(&capped_plan) <= 4.0, "{} {}", at(&sewn), at(&capped_plan));
}

#[test]
fn req_auto_195_a_cap_out_of_work_while_it_pulls_in_below_the_limit_keeps_a_plan_it_measures_again() {
    let (sewn, column, _) = fanned(0.08);
    let pulls = pullable(&column, true);
    let rules = Rules { cap: 3.0, limit: 4.0, columns: &column, pulls: &pulls, ..rules() };
    let thickness = Thickness::of(&sewn, &mut Budget::DEFAULT.meter()).unwrap().unwrap();
    let enough = least(&sewn, &thickness, &rules);
    let mut ran_out = 0;
    for max_work in 0..enough {
        let (mut plan, mut measure) = (sewn.clone(), thickness.clone());
        let Ok(Some(capped)) = capped(&mut plan, &mut measure, &rules, &all(&sewn), &Budget { max_work, ..Budget::DEFAULT }) else { continue };
        assert!(capped.ran_out, "{max_work}: {capped:?}");
        let measured = Thickness::of(&plan, &mut Budget::DEFAULT.meter()).unwrap().unwrap();
        assert!(measures_as(&measure, &measured), "{max_work}");
        assert_eq!(capped.thickest, measured.most(Measure::Fills), "{max_work}");
        ran_out += 1;
    }
    assert!(ran_out > 0, "{enough}");
}

#[test]
fn req_auto_195_the_pull_in_holds_spots_above_the_limit_only_below_it() {
    use super::super::pull::held_at;
    assert_eq!((held_at(3.25, 4.0), held_at(4.0, 4.0), held_at(4.5, 4.0)), (Some(4.0), None, None));
}

#[test]
fn req_auto_171_a_point_is_chosen_only_over_one_that_lowers_the_spot_less() {
    use super::super::pull::better;
    assert!(!better(0.0, None) && !better(-0.1, None) && better(0.1, None));
    assert!(!better(0.1, Some(0.1)) && !better(0.05, Some(0.1)) && better(0.2, Some(0.1)));
}

/// A fan as [`fan`] makes, `width` mm wide: its crowded rail at x = `width`, its open rail at x = 0.
fn wide_fan(crowd: f64, width: f64) -> Vec<(f64, f64)> {
    fan(crowd).into_iter().map(|(x, y)| (x / 4.0 * width, y)).collect()
}

#[test]
fn req_auto_195_a_later_shape_covers_a_pulled_end_only_where_it_covers_its_first_tenth_of_a_millimetre() {
    // The fan 8 mm wide has its crowded rail's points 0.25 mm apart: no knot. A point pulled in a quarter of
    // the way ends near x = 6, 2 mm from where it was. A run of another shape sewn later at x = 5.95 covers
    // 0.2 mm round it, to x = 6.15: the first 0.1 mm of the way back from the new end, and no more of it.
    let pulled = |over_x: f64| {
        let over: Vec<(f64, f64)> = vec![(over_x, -1.0), (over_x, 5.0)];
        let mut sewn = plan(&[("fan", Role::Top, wide_fan(0.25, 8.0)), ("over", Role::Top, over)]);
        let column: BTreeSet<ElementRef> = ElementRef::from_index(0).into_iter().collect();
        let pulls = pullable(&column, true);
        let rules = Rules { cap: 1.5, limit: 2.5, columns: &column, pulls: &pulls, ..rules() };
        let (capped, _, measured) = cap(&mut sewn, &rules);
        assert!(measured.most(Measure::Fills).value <= 2.5);
        capped.map_or(0, |capped| capped.pulled[0])
    };
    assert!(pulled(5.95) > 0);
    // At x = 5.8 the run covers to 6.0 only: the new end's first 0.1 mm shows.
    assert_eq!(pulled(5.8), 0);
}

#[test]
fn req_auto_194_the_spots_gone_on_to_are_those_near_the_points_stitches_thickest_first() {
    let mut sewn = plan(&[("pile", Role::Top, pile(30.0)), ("fan", Role::Top, fan(0.05))]);
    let column: BTreeSet<ElementRef> = ElementRef::from_index(1).into_iter().collect();
    let pulls = pullable(&column, false);
    let rules = Rules { cap: 3.0, limit: 4.0, columns: &column, pulls: &pulls, ..rules() };
    on_cap(&mut sewn, &rules, |cap, points| {
        let fan: Vec<Entry> = points
            .iter()
            .copied()
            .filter(|point| cap.pass.stitch(*point).is_some_and(|(_, _, element)| column.contains(&element.unwrap())))
            .collect();
        let mut meter = Budget::DEFAULT.meter();
        let mut hot = Hot::of(&cap.pass, &cap.ribbons, &fan, 1.0, &mut meter).unwrap();
        // The pile is thicker, but no stitch of the fan comes near it: the fan's own thickest spot comes first.
        let (first, spot) = hot.next(cap.pass.thickness, &mut meter).unwrap().unwrap();
        assert!(spot.value < cap.pass.thickness.most(Measure::Fills).value, "{spot:?}");
        let alone = Thickness::of(&plan(&[("fan", Role::Top, super::fan(0.05))]), &mut meter).unwrap().unwrap().most(Measure::Fills).value;
        assert!((spot.value - alone).abs() < 1e-9, "{spot:?} {alone}");
        // It stays until it is stuck, and its points are the fan's whose stitches come near it, in the plan's
        // order: every point the cap may move for it is among them.
        assert_eq!(hot.next(cap.pass.thickness, &mut meter).unwrap().map(|(at, _)| at), Some(first));
        let near = hot.near(&cap.pass, &cap.ribbons, spot.at, &mut meter).unwrap();
        assert!(!near.is_empty() && near.windows(2).all(|pair| pair[0] < pair[1]) && near.iter().all(|point| fan.contains(point)));
        let movable: Vec<Entry> =
            fan.iter().copied().filter(|point| cap.pulled(*point, 0, &BTreeMap::new(), spot.at, &mut meter).unwrap().is_some()).collect();
        assert!(!movable.is_empty() && movable.iter().all(|point| near.contains(point)), "{movable:?} {near:?}");
        assert!(hot.near(&cap.pass, &cap.ribbons, Point::new(500.0, 500.0).unwrap(), &mut meter).unwrap().is_empty());
        hot.stuck(first);
        let (second, next) = hot.next(cap.pass.thickness, &mut meter).unwrap().unwrap();
        assert!(second != first && next.value <= spot.value && next.value > 1.0);
        // A spot whose thread falls takes its new place; under the level it is done.
        for stitch in cap.pass.thickness.sewn().to_vec().iter().filter(|stitch| fan.contains(&(stitch.block, stitch.index))) {
            cap.pass.thickness.remove(stitch, &mut meter).unwrap();
        }
        assert_eq!(hot.next(cap.pass.thickness, &mut meter).unwrap(), None);
        // A stitch laid again puts its spots above the level back, but for those stuck.
        let stitch = cap.pass.thickness.sewn().iter().find(|stitch| fan.contains(&(stitch.block, stitch.index))).copied().unwrap();
        for _ in 0..10 {
            cap.pass.thickness.add(&stitch, &mut meter).unwrap();
        }
        hot.add(cap.pass.thickness, &[stitch], &mut meter).unwrap();
        let back = hot.next(cap.pass.thickness, &mut meter).unwrap().unwrap();
        assert!(back.0 != first && back.1.value > 1.0);
        // Nothing above a level as high as the thickest spot.
        assert_eq!(Hot::of(&cap.pass, &cap.ribbons, &fan, 1000.0, &mut meter).unwrap().next(cap.pass.thickness, &mut meter).unwrap(), None);
        // A unit of work for each point and each spot looked at.
        assert!(Hot::of(&cap.pass, &cap.ribbons, &fan, 1.0, &mut Budget { max_work: 0, ..Budget::DEFAULT }.meter()).is_err());
        assert!(hot.next(cap.pass.thickness, &mut Budget { max_work: 0, ..Budget::DEFAULT }.meter()).is_err());
        assert!(hot.near(&cap.pass, &cap.ribbons, back.1.at, &mut Budget { max_work: 0, ..Budget::DEFAULT }.meter()).is_err());
    });
}
