//! Lock stitches' conformance cases (`REQ-LCK-002`, `REQ-LCK-004`) and `SC-W0502`, `SC-W0503`, through the
//! engine's public API.

// Test code may unwrap and index (clippy.toml allows it in tests).
#![allow(clippy::unwrap_used)]

use proptest::prelude::*;
use stitchcraft_core::math::sin_cos;
use stitchcraft_core::{Budget, Code, Exhausted, Point};
use stitchcraft_engine::common::CommonParams;
use stitchcraft_engine::locks::{LOCKS, Lock, SIZED_IN_MM, SIZED_IN_PERCENT, tie_in, tie_off};
use stitchcraft_params::ParamSet;
use stitchcraft_plan::invariants::LOCK_MIN_STITCH;

fn p(x: f64, y: f64) -> Point {
    Point::new(x, y).unwrap()
}

/// The element's settings, as Ink/Stitch keys and values.
fn settings(pairs: &[(&str, &str)]) -> CommonParams {
    let set: ParamSet = pairs.iter().copied().collect();
    CommonParams::from_set(&set).unwrap().params
}

/// Both locks of `group` with the same settings at both ends (`lock` for the shape, the rest with
/// `start` in their keys, which are copied to the end's keys).
fn locks(group: &[Point], lock: &str, start: &[(&str, &str)]) -> (Lock, Lock) {
    let mut pairs: Vec<(String, String)> = vec![("lock_start".into(), lock.into()), ("lock_end".into(), lock.into())];
    for (key, value) in start {
        pairs.push(((*key).to_string(), (*value).to_string()));
        pairs.push((key.replace("start", "end"), (*value).to_string()));
    }
    let set: ParamSet = pairs.iter().map(|(k, v)| (k.as_str(), v.as_str())).collect();
    let params = CommonParams::from_set(&set).unwrap().params;
    let mut meter = Budget::DEFAULT.meter();
    (tie_in(group, &params, &mut meter).unwrap(), tie_off(group, &params, &mut meter).unwrap())
}

/// Needle points every 2.5 mm from (0, 0) to (10, 0).
fn straight() -> Vec<Point> {
    (0..5).map(|i| p(2.5 * f64::from(i), 0.0)).collect()
}

/// The stitches a lock sews, joined to the group: a tie-in into its first point, a tie-off from its last.
fn sewn(group: &[Point], tie_in: &Lock, tie_off: &Lock) -> (Vec<Point>, Vec<Point>) {
    let mut start = tie_in.points.clone();
    start.push(group[0]);
    let mut end = vec![*group.last().unwrap()];
    end.extend(&tie_off.points);
    (start, end)
}

fn lengths(points: &[Point]) -> Vec<f64> {
    points.windows(2).map(|s| s[0].distance(s[1])).collect()
}

fn shortest(points: &[Point]) -> f64 {
    lengths(points).into_iter().fold(f64::INFINITY, f64::min)
}

/// How far the lock reaches from `anchor`.
fn reach(lock: &Lock, anchor: Point) -> f64 {
    lock.points.iter().map(|q| q.distance(anchor)).fold(0.0, f64::max)
}

fn messages(lock: &Lock) -> Vec<String> {
    lock.warnings.iter().map(ToString::to_string).collect()
}

#[test]
fn req_lck_002_every_lock_id_is_accepted_and_sews_a_lock() {
    let group = straight();
    let (first, last) = (group[0], group[4]);
    for lock in LOCKS {
        let (start, end) = locks(&group, lock.id, &[("lock_custom_start", "1 -1 1 -1")]);
        let id = lock.id;
        assert!(start.warnings.is_empty() && end.warnings.is_empty(), "{id}: {:?} {:?}", start.warnings, end.warnings);
        assert!(start.points.len() >= 3 && end.points.len() >= 3, "{id}");
        // Each lock is a loop at its end of the stitching: the tie-in leaves from where the stitching
        // starts and comes back to it, the tie-off ends where the stitching ends, for the trim.
        assert_eq!((start.points[0], *end.points.last().unwrap()), (first, last), "{id}");
        let (into, from) = sewn(&group, &start, &end);
        assert!(shortest(&into) >= LOCK_MIN_STITCH.get() && shortest(&from) >= LOCK_MIN_STITCH.get(), "{id}: {into:?} {from:?}");
        // Small, and on the stitching's side of its end, so the stitching covers it.
        assert!(reach(&start, first) <= 1.4 + 1e-9 && reach(&end, last) <= 1.4 + 1e-9, "{id}");
        assert!(start.points.iter().all(|q| q.x() >= -1e-9) && end.points.iter().all(|q| q.x() <= 10.0 + 1e-9), "{id}");
    }
}

#[test]
fn req_lck_002_locks_scale_with_their_own_parameter_only() {
    let group = straight();
    let anchor = group[0];
    let start_reach = |lock: &str, pairs: &[(&str, &str)]| {
        let mut all = vec![("lock_custom_start", "2 -1 -1")];
        all.extend_from_slice(pairs);
        reach(&locks(&group, lock, &all).0, anchor)
    };
    for lock in LOCKS.iter().map(|l| l.id) {
        let plain = start_reach(lock, &[]);
        let by_mm = start_reach(lock, &[("lock_start_scale_mm", "1.4")]);
        let by_percent = start_reach(lock, &[("lock_start_scale_percent", "200")]);
        let mm_sized = SIZED_IN_MM.contains(&lock);
        let percent_sized = SIZED_IN_PERCENT.contains(&lock) && lock != "custom";
        assert!((by_mm - if mm_sized { 2.0 * plain } else { plain }).abs() < 1e-9, "{lock}: {plain} → {by_mm} by mm");
        assert!((by_percent - if percent_sized { 2.0 * plain } else { plain }).abs() < 1e-9, "{lock}: {plain} → {by_percent} by %");
    }
}

#[test]
fn req_lck_002_the_half_stitch_hides_under_the_first_and_last_stitch() {
    // A slanted group whose first stitch is 1.2 mm long and whose last is 3 mm.
    let group = [p(1.0, 1.0), p(1.72, 1.96), p(3.0, 2.0), p(4.8, 4.4)];
    let (start, end) = locks(&group, "half_stitch", &[("lock_start_scale_mm", "5"), ("lock_start_scale_percent", "300")]);
    // Forth and back over half the first stitch, twice; the scales do not apply.
    let half = p(1.36, 1.48);
    for (got, want) in start.points.iter().zip([p(1.0, 1.0), half, p(1.0, 1.0), half]) {
        assert!(got.distance(want) < 1e-9, "{:?}", start.points);
    }
    // Half of a 3 mm stitch is more than the longest half stitch, 1 mm: back 1 mm along it, twice.
    let back = p(4.2, 3.6);
    for (got, want) in end.points.iter().zip([back, p(4.8, 4.4), back, p(4.8, 4.4)]) {
        assert!(got.distance(want) < 1e-9, "{:?}", end.points);
    }
    assert_eq!((start.points.len(), end.points.len()), (4, 4));
}

#[test]
fn req_lck_002_drawn_locks_turn_with_the_stitching() {
    // Along +y: the arrow's tip is 1.4 mm down the first stitch.
    let group = [p(0.0, 0.0), p(0.0, 3.0)];
    let (start, end) = locks(&group, "arrow", &[]);
    assert!(start.points.iter().any(|q| q.distance(p(0.0, 1.4)) < 1e-9), "{:?}", start.points);
    assert!(end.points.iter().any(|q| q.distance(p(0.0, 1.6)) < 1e-9), "{:?}", end.points);
}

#[test]
fn req_lck_004_custom_numbers_are_steps_into_the_stitching() {
    let group = straight();
    let (start, end) = locks(&group, "custom", &[("lock_custom_start", "2 -1"), ("lock_start_scale_mm", "0.5")]);
    // The tie-in's steps end where the stitching starts: from -0.5, 1 mm forth to 0.5, back 0.5 mm to 0.
    assert_eq!(start.points, [p(-0.5, 0.0), p(0.5, 0.0)]);
    // The tie-off's start where it ends, going back into it: 1 mm back to 9, then 0.5 mm forth to 9.5.
    assert_eq!(end.points, [p(9.0, 0.0), p(9.5, 0.0)]);
    assert!(start.warnings.is_empty() && end.warnings.is_empty());
}

#[test]
fn a_group_without_a_stitch_gets_no_lock() {
    let params = settings(&[]);
    let mut meter = Budget::DEFAULT.meter();
    for group in [vec![], vec![p(1.0, 1.0)], vec![p(1.0, 1.0), p(1.0, 1.0)]] {
        assert_eq!(tie_in(&group, &params, &mut meter).unwrap(), Lock::default());
        assert_eq!(tie_off(&group, &params, &mut meter).unwrap(), Lock::default());
    }
    // A repeated first point is passed over: the lock follows the first real stitch.
    let lock = tie_in(&[p(0.0, 0.0), p(0.0, 0.0), p(2.0, 0.0)], &params, &mut meter).unwrap();
    assert_eq!(lock.points, [p(0.0, 0.0), p(1.0, 0.0), p(0.0, 0.0), p(1.0, 0.0)]);
}

#[test]
fn the_budget_bounds_the_work() {
    let params = settings(&[("lock_start", "custom"), ("lock_custom_start", "1 -1 1 -1 1 -1")]);
    let mut meter = Budget { max_stitches: 1, max_work: 5 }.meter();
    assert_eq!(tie_in(&straight(), &params, &mut meter), Err(Exhausted::Work));
}

#[test]
fn diag_sc_w0502_lock_stitches_shorter_than_0_2_mm_are_lengthened() {
    let group = straight();
    // A drawn lock is enlarged as a whole, until its shortest stitch is 0.2 mm.
    let (start, _) = locks(&group, "zigzag", &[("lock_start_scale_percent", "10")]);
    assert_eq!(
        messages(&start),
        ["warning SC-W0502: The start lock's shortest stitch would be 0.05 mm, shorter than 0.2 mm, so the lock is sewn 4.34 times as large."]
    );
    assert_eq!(start.warnings[0].code, Code::LockStitchLengthened);
    assert_eq!(start.warnings[0].at, Some(group[0]));
    let (into, _) = sewn(&group, &start, &Lock::default());
    assert!((shortest(&into) - 0.2).abs() < 1e-9, "{into:?}");
    // Steps are lengthened one by one.
    let (_, end) = locks(&group, "back_forth", &[("lock_start_scale_mm", "0.1")]);
    assert_eq!(
        messages(&end),
        ["warning SC-W0502: 4 steps of the end lock would be shorter than 0.2 mm, the shortest 0.1 mm, so they are lengthened to 0.2 mm."]
    );
    assert_eq!(end.points, [p(9.8, 0.0), p(10.0, 0.0), p(9.8, 0.0), p(10.0, 0.0)]);
    let (start, _) = locks(&group, "custom", &[("lock_custom_start", "2 -1.9 -0.1")]);
    assert_eq!(
        messages(&start),
        ["warning SC-W0502: A step of the start lock would be 0.07 mm, shorter than 0.2 mm, so it is lengthened to 0.2 mm."]
    );
    assert!(shortest(&sewn(&group, &start, &Lock::default()).0) >= 0.2 - 1e-9);
}

#[test]
fn diag_sc_w0503_custom_locks_that_cannot_be_sewn_as_written_are_named() {
    let group = straight();
    let half_stitch = locks(&group, "half_stitch", &[]).0.points;
    let (drawn, _) = locks(&group, "custom", &[("lock_custom_start", "M 0,0 L 1,1 2,0")]);
    assert_eq!(
        messages(&drawn),
        [
            "warning SC-W0503: The custom start lock is not written as numbers, and StitchCraft cannot sew a lock drawn as an SVG path yet, so the half stitch is sewn instead."
        ]
    );
    assert_eq!((drawn.points, drawn.warnings[0].code), (half_stitch, Code::CustomLockUnusable));
    let (_, empty) = locks(&group, "custom", &[]);
    assert_eq!(messages(&empty), ["warning SC-W0503: The custom end lock has no steps to sew, so the half stitch is sewn instead."]);
    let (one, _) = locks(&group, "custom", &[("lock_custom_start", "1,5 1 -1")]);
    assert_eq!(messages(&one), ["warning SC-W0503: A part of the custom start lock is not a step it can sew (\"1,5\"), so it is left out."]);
    assert_eq!(one.points, [p(0.0, 0.0), p(0.7, 0.0)]);
    let (many, _) = locks(&group, "custom", &[("lock_custom_start", "1,5 1-2 . -- 0")]);
    assert_eq!(
        messages(&many),
        [
            "warning SC-W0503: 4 parts of the custom start lock are not steps it can sew (\"1,5\", \"1-2\", \".\", …), so they are left out.",
            "warning SC-W0503: The custom start lock has no steps to sew, so the half stitch is sewn instead.",
        ]
    );
}

proptest! {
    #![proptest_config(stitchcraft_testkit::strategies::config(256))]

    /// Any group, any lock, any sizes: every lock stitch is at least 0.2 mm long, joins included, and the
    /// same input sews the same lock.
    #[test]
    fn req_lck_002_no_lock_stitch_is_shorter_than_0_2_mm(
        steps in prop::collection::vec((0.3..5.0_f64, 0.0..6.3_f64), 1..8),
        lock in 0..LOCKS.len(),
        scale_mm in 0.1..10.0_f64,
        percent in 10.0..500.0_f64,
        custom in prop::collection::vec(-3.0..3.0_f64, 0..6),
    ) {
        let mut group = vec![p(0.0, 0.0)];
        for (length, angle) in &steps {
            let (sin, cos) = sin_cos(*angle);
            let last = *group.last().unwrap();
            group.push(p(last.x() + length * cos, last.y() + length * sin));
        }
        let custom: Vec<String> = custom.iter().map(|s| format!("{s:.2}")).collect();
        let (scale_mm, percent, custom) = (scale_mm.to_string(), percent.to_string(), custom.join(" "));
        let pairs = [("lock_start_scale_mm", scale_mm.as_str()), ("lock_start_scale_percent", percent.as_str()), ("lock_custom_start", custom.as_str())];
        let id = LOCKS[lock].id;
        let (start, end) = locks(&group, id, &pairs);
        let (into, from) = sewn(&group, &start, &end);
        prop_assert!(shortest(&into) >= LOCK_MIN_STITCH.get() - 1e-9, "{id}: {into:?}");
        prop_assert!(shortest(&from) >= LOCK_MIN_STITCH.get() - 1e-9, "{id}: {from:?}");
        prop_assert_eq!(locks(&group, id, &pairs), (start, end));
    }
}
