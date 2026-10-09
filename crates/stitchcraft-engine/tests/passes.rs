//! The running stitch's passes and random lengths, through the engine's public API: bean stitch
//! (`REQ-RUN-004`), repeats (`REQ-RUN-005`) and random stitch length (`REQ-RUN-007`).

// Test code may unwrap and index (clippy.toml allows it in tests).
#![allow(clippy::unwrap_used)]

use proptest::prelude::*;
use stitchcraft_core::rng::SplitMix64;
use stitchcraft_core::{Budget, Mm, Point};
use stitchcraft_engine::design::{Path, Segment, Subpath};
use stitchcraft_engine::generators::passes::RepeatParams;
use stitchcraft_engine::generators::running::{RunningParams, running_stitch};
use stitchcraft_params::ParamSet;

fn p(x: f64, y: f64) -> Point {
    Point::new(x, y).unwrap()
}

/// A straight line from the origin `length` mm along x.
fn line(length: f64) -> Path {
    Path { subpaths: vec![Subpath { start: p(0.0, 0.0), segments: vec![Segment::Line(p(length, 0.0))], closed: false }] }
}

/// The one run of `path`, stitched with `settings` (Ink/Stitch keys and values) for element `id`, with a
/// shortest stitch of 0.3 mm.
fn sew(path: &Path, settings: &[(&str, &str)], id: &str) -> Vec<Point> {
    let set: ParamSet = settings.iter().copied().collect();
    let running = RunningParams::from_set(&set).unwrap().params;
    let passes = RepeatParams::from_set(&set).unwrap().params;
    let mut rng = SplitMix64::for_element(id, running.random_seed.unwrap_or(0));
    let stitched = running_stitch(path, &running, &passes, Mm::new(0.3).unwrap(), &mut rng, &mut Budget::DEFAULT.meter()).unwrap();
    assert_eq!(stitched.runs.len(), 1);
    stitched.runs.into_iter().next().unwrap()
}

/// The stitches of `run` as pairs of x coordinates (every case here is along the x axis).
fn stitches(run: &[Point]) -> Vec<(f64, f64)> {
    run.windows(2).map(|s| (s[0].x(), s[1].x())).collect()
}

/// How many times each stretch of the line (between two stitch points of one pass, from `cuts`) is sewn.
fn coverage(run: &[Point], cuts: &[f64]) -> Vec<usize> {
    cuts.windows(2)
        .map(|c| stitches(run).iter().filter(|(a, b)| (a.min(*b) - c[0]).abs() < 1e-9 && (a.max(*b) - c[1]).abs() < 1e-9).count())
        .collect()
}

#[test]
fn req_run_004_bean_stitch_sews_each_stitch_2b_plus_1_times() {
    // Four stitches of 2.5 mm.
    let cuts = [0.0, 2.5, 5.0, 7.5, 10.0];
    let plain = sew(&line(10.0), &[], "e");
    assert_eq!(coverage(&plain, &cuts), [1, 1, 1, 1]);
    // b = 1 triples each stitch, there and back and there again; b = 2 sews it five times.
    let triple = sew(&line(10.0), &[("bean_stitch_repeats", "1")], "e");
    assert_eq!((stitches(&triple).len(), coverage(&triple, &cuts)), (12, vec![3, 3, 3, 3]));
    assert_eq!(&stitches(&triple)[..3], [(0.0, 2.5), (2.5, 0.0), (0.0, 2.5)]);
    let five = sew(&line(10.0), &[("bean_stitch_repeats", "2")], "e");
    assert_eq!((stitches(&five).len(), coverage(&five, &cuts)), (20, vec![5, 5, 5, 5]));
    // A list is taken in turn along the stitches.
    let alternate = sew(&line(10.0), &[("bean_stitch_repeats", "1 0")], "e");
    assert_eq!(coverage(&alternate, &cuts), [3, 1, 3, 1]);
    // Across repeats the list runs on, the turnaround taking one step: each stretch is sewn the same way
    // there and back.
    let both = sew(&line(10.0), &[("bean_stitch_repeats", "1 0"), ("repeats", "2")], "e");
    assert_eq!(coverage(&both, &cuts), [6, 2, 6, 2]);
}

#[test]
fn req_run_005_repeats_alternate_direction_without_a_stitch_in_place() {
    for (repeats, end) in [(1, 10.0), (2, 0.0), (3, 10.0), (4, 0.0)] {
        let run = sew(&line(10.0), &[("repeats", &repeats.to_string())], "e");
        assert_eq!((run.first().unwrap().x(), run.last().unwrap().x()), (0.0, end), "{repeats} repeats");
        assert_eq!(stitches(&run).len(), 4 * repeats);
        assert!(stitches(&run).iter().all(|(a, b)| a != b), "a stitch in place: {run:?}");
    }
}

#[test]
fn req_run_007_random_lengths_repeat_from_the_seed() {
    let random = [("enable_random_stitch_length", "true"), ("random_stitch_length_jitter_percent", "40")];
    let first = sew(&line(100.0), &random, "e");
    let lengths: Vec<f64> = first.windows(2).map(|s| s[0].distance(s[1])).collect();
    assert!(lengths.iter().any(|l| (l - lengths[0]).abs() > 0.1), "the lengths differ: {lengths:?}");
    assert!(lengths.iter().all(|l| *l <= 2.5 * 1.4 + 1e-9 && *l >= 0.3 - 1e-9), "{lengths:?}");
    // The same element and seed: the same stitches. Another element or another seed: other stitches.
    assert_eq!(sew(&line(100.0), &random, "e"), first);
    assert_ne!(sew(&line(100.0), &random, "f"), first);
    let seeded = [random[0], random[1], ("random_seed", "7")];
    assert_ne!(sew(&line(100.0), &seeded, "e"), first);
    assert_eq!(sew(&line(100.0), &seeded, "e"), sew(&line(100.0), &seeded, "e"));
    // Off, the seed changes nothing.
    assert_eq!(sew(&line(100.0), &[("random_seed", "7")], "e"), sew(&line(100.0), &[], "f"));
}

proptest! {
    #![proptest_config(stitchcraft_testkit::strategies::config(256))]

    /// Any line, list of bean counts and number of repeats: each stretch between two stitch points of a
    /// pass is sewn 2b + 1 times on each pass, b taken in turn with each turnaround a step, and every
    /// stitch joins two neighbouring points of the pass.
    #[test]
    fn req_run_004_counts_follow_the_list_across_repeats(length in 1.0..40.0_f64, beans in prop::collection::vec(0_u32..4, 1..4), repeats in 1_usize..5) {
        let list = beans.iter().map(u32::to_string).collect::<Vec<_>>().join(" ");
        let pass = sew(&line(length), &[], "e");
        let run = sew(&line(length), &[("bean_stitch_repeats", &list), ("repeats", &repeats.to_string())], "e");
        let cuts: Vec<f64> = pass.iter().map(|q| q.x()).collect();
        let n = cuts.len() - 1;
        let mut expected = vec![0_usize; n];
        for k in 0..repeats {
            for j in 0..n {
                let step = k * (n + 1) + j;
                let stretch = if k % 2 == 0 { j } else { n - 1 - j };
                expected[stretch] += 2 * beans[step % beans.len()] as usize + 1;
            }
        }
        prop_assert_eq!(coverage(&run, &cuts), expected);
        prop_assert_eq!(stitches(&run).len(), coverage(&run, &cuts).iter().sum::<usize>(), "every stitch joins neighbours");
    }

    /// Random lengths on any line keep every stitch between the shortest stitch and the longest length
    /// times (1 + jitter) (`REQ-RUN-001`), and end where the line does.
    #[test]
    fn req_run_001_random_lengths_stay_within_bounds(length in 0.5..60.0_f64, jitter in 0.0..100.0_f64, stitch in 0.7..6.0_f64, seed in any::<u32>()) {
        let (jitter, stitch, seed) = (format!("{jitter:.1}"), format!("{stitch:.2}"), seed.to_string());
        let run = sew(&line(length), &[("enable_random_stitch_length", "true"), ("random_stitch_length_jitter_percent", &jitter), ("running_stitch_length_mm", &stitch), ("random_seed", &seed)], "e");
        let longest = stitch.parse::<f64>().unwrap() * (1.0 + jitter.parse::<f64>().unwrap() / 100.0);
        for s in run.windows(2) {
            let l = s[0].distance(s[1]);
            prop_assert!(l >= 0.3 - 1e-9 && l <= longest + 1e-9, "{l} not within [0.3, {longest}]");
        }
        prop_assert_eq!(run.last().unwrap().x(), length);
    }
}
