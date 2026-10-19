//! The stitch budget bounds what generators make (`REQ-GEN-004`). A few settings multiply an element's
//! needle points — the smallest spacing, repeats, bean stitch, a short longest stitch — so every point a
//! generator makes along the shape costs a stitch as it is made, and making stops at the stitches the
//! design has left, long before memory runs out. Once an element is sewn, it has spent a stitch for each
//! needle point it gives, no more and no fewer, and `plan` gives each element what the ones before it left.
//!
//! Each case that stops compares the work spent with the work of sewing the same element without a stitch
//! limit: stopping at the stitches left spends a small part of it.

// Test code may unwrap and index (clippy.toml allows it in tests).
#![allow(clippy::unwrap_used, clippy::panic)]

use stitchcraft_core::rng::SplitMix64;
use stitchcraft_core::{Budget, Code, ElementId, Exhausted, Meter, Mm};
use stitchcraft_engine::design::{Design, DesignSettings, FillRule, Path};
use stitchcraft_engine::generate::generate;
use stitchcraft_engine::generators::manual::manual_stitch;
use stitchcraft_engine::generators::passes::RepeatParams;
use stitchcraft_engine::generators::pull::PullParams;
use stitchcraft_engine::generators::running::{RunningParams, running_stitch};
use stitchcraft_engine::generators::satin::{SatinParams, satin_stitch};
use stitchcraft_engine::generators::tatami::rows::Grid;
use stitchcraft_engine::generators::tatami::sew::{Fill, Layer, Pull, Travel, tatami_fill};
use stitchcraft_engine::generators::tatami::stitches::Stitching;
use stitchcraft_engine::generators::zigzag::{ZigzagStrokeParams, zigzag_stroke};
use stitchcraft_engine::generators::{Neighbours, Stitched};
use stitchcraft_engine::normalize::region::build;
use stitchcraft_engine::normalize::satin::{Shape, recognize};
use stitchcraft_engine::plan;
use stitchcraft_params::{ParamSet, StitchType};
use stitchcraft_plan::profiles::REFERENCE;
use stitchcraft_testkit::designs::{RED, line, messages, polylines, rings};
use stitchcraft_testkit::satins::{ladder, satin_lengths};

/// The machine's shortest stitch, as the engine gives it to a stroke on the reference machine.
const SHORTEST: f64 = 0.3;

/// A generator sewing with a meter it is given.
type Generator<'a> = dyn Fn(&mut Meter) -> Result<Stitched, Exhausted> + 'a;

/// What one generator sewed with a meter of `stitches` and the default work: the needle points it gave,
/// or why it stopped, then the work and the stitches it spent.
struct Spent {
    sewn: Result<usize, Exhausted>,
    work: u64,
    stitches: u32,
}

/// What `generator` sews with a meter of `stitches` and the default work.
fn spent(stitches: u32, generator: impl FnOnce(&mut Meter) -> Result<Stitched, Exhausted>) -> Spent {
    let mut meter = Budget { max_stitches: stitches, ..Budget::DEFAULT }.meter();
    let sewn = generator(&mut meter).map(|stitched| stitched.runs.iter().map(Vec::len).sum());
    Spent { sewn, work: Budget::DEFAULT.max_work - meter.work_left(), stitches: stitches - meter.stitches_left() }
}

/// `generator` sewn with no stitch limit to speak of: it gives `points` needle points, having spent a stitch
/// for each. With a meter of `limit` stitches, far fewer, it stops for the stitches, having spent less than
/// a quarter of the work.
fn stops_early(points: usize, limit: u32, generator: impl Fn(&mut Meter) -> Result<Stitched, Exhausted>) {
    let free = spent(u32::MAX, &generator);
    assert_eq!(free.sewn, Ok(points));
    assert_eq!(usize::try_from(free.stitches).unwrap(), points, "a stitch spent for each needle point");
    let limited = spent(limit, &generator);
    assert_eq!(limited.sewn, Err(Exhausted::Stitches));
    assert!(limited.work < free.work / 4, "making stops at the stitches left: {} of {} units of work", limited.work, free.work);
}

fn set(params: &[(&str, &str)]) -> ParamSet {
    params.iter().copied().collect()
}

fn running(path: &Path, params: &ParamSet, meter: &mut Meter) -> Result<Stitched, Exhausted> {
    let (running, passes) = (RunningParams::from_set(params).unwrap().params, RepeatParams::from_set(params).unwrap().params);
    running_stitch(path, &running, &passes, Mm::new(SHORTEST).unwrap(), &SplitMix64::new(1), meter)
}

fn zigzag(path: &Path, width: f64, params: &ParamSet, meter: &mut Meter) -> Result<Stitched, Exhausted> {
    let (zigzag, passes) = (ZigzagStrokeParams::from_set(params).unwrap().params, RepeatParams::from_set(params).unwrap().params);
    zigzag_stroke(path, Mm::new(width).unwrap(), &zigzag, &passes, Mm::new(SHORTEST).unwrap(), meter)
}

fn manual(path: &Path, longest: Option<f64>, params: &ParamSet, meter: &mut Meter) -> Result<Stitched, Exhausted> {
    let passes = RepeatParams::from_set(params).unwrap().params;
    manual_stitch(path, longest.map(|mm| Mm::new(mm).unwrap()), &passes, Mm::new(SHORTEST).unwrap(), meter)
}

/// The satin column `path` sewn with `params`, as the satin cases sew one, with nothing before or after it.
fn satin(path: &Path, params: &ParamSet, meter: &mut Meter) -> Result<Stitched, Exhausted> {
    let Ok(Shape::Rails(satin)) = recognize(path, &mut Budget::DEFAULT.meter()).unwrap().shape else { panic!("not rails") };
    let column = SatinParams::from_set_as(params, StitchType::SatinColumn).unwrap().params;
    let pull = PullParams::from_set_as(params, StitchType::SatinColumn).unwrap().params;
    satin_stitch(&satin, &column, &pull, satin_lengths(params), &Neighbours::default(), &mut SplitMix64::new(1), meter)
}

/// Rows at 0°, `spacing` apart, in stitches of `length`.
fn rows(spacing: f64, length: f64) -> Layer {
    let stitching = Stitching { grid: Grid { angle: 0.0, spacing, end_spacing: None }, length, staggers: 4.0, skip_last: false, jitter: None };
    let travel = Travel { length: 2.0, tolerance: 0.2, min_stitch: SHORTEST, underpath: true, jump: 3.0 };
    Layer { stitching, travel, pull: Pull::default(), gap_rows: 0 }
}

/// A tatami fill of `top` rows over a rectangle `width` by `height` millimetres, after `underlay`.
fn tatami((width, height): (f64, f64), top: Layer, underlay: &[Layer], meter: &mut Meter) -> Result<Stitched, Exhausted> {
    let rectangle = rings(&[&[(0.0, 0.0), (width, 0.0), (width, height), (0.0, height), (0.0, 0.0)]]);
    let region = build(&rectangle, FillRule::EvenOdd, &mut Budget::DEFAULT.meter()).unwrap().region;
    let fill = Fill { top, expand: 0.0, underlay: underlay.to_vec(), inset: 0.0, force_locks: false };
    tatami_fill(&region, &fill, None, None, &mut SplitMix64::new(1), meter)
}

#[test]
fn req_gen_004_repeats_and_bean_stitch_stop_at_the_stitches_left() {
    // 1 mm of zigzag at the smallest spacing, 201 needle points, sewn 100 times with bean stitch 10:
    // 420,001 needle points. 10 mm of it would be 4.2 million and a metre 420 million, gigabytes.
    let dash = polylines(&[&[(0.0, 0.0), (1.0, 0.0)]]);
    let bold = set(&[("zigzag_spacing_mm", "0.01"), ("repeats", "100"), ("bean_stitch_repeats", "10")]);
    stops_early(420_001, 10_000, |meter| zigzag(&dash, 1.0, &bold, meter));
    // A running stitch's passes alike: 60 mm in 0.6 mm stitches, 101 needle points, 210,001 sewn.
    let line = polylines(&[&[(0.0, 0.0), (60.0, 0.0)]]);
    let bold = set(&[("running_stitch_length_mm", "0.6"), ("repeats", "100"), ("bean_stitch_repeats", "10")]);
    stops_early(210_001, 10_000, |meter| running(&line, &bold, meter));
    // And hand-placed stitches' bean stitch: 101 nodes 1 mm apart, each stitch sewn 21 times.
    let nodes: Vec<(f64, f64)> = (0..=100).map(|x| (f64::from(x), 0.0)).collect();
    let stitches = polylines(&[&nodes]);
    stops_early(2_101, 100, |meter| manual(&stitches, None, &set(&[("bean_stitch_repeats", "10")]), meter));
}

#[test]
fn req_gen_004_a_long_pass_stops_at_the_stitches_left() {
    // 6 m of running stitch in 0.6 mm stitches: 10,001 needle points in one pass.
    let long = polylines(&[&[(0.0, 0.0), (6000.0, 0.0)]]);
    stops_early(10_001, 1_000, |meter| running(&long, &set(&[("running_stitch_length_mm", "0.6")]), meter));
    // 100 mm of zigzag at the smallest spacing: 20,001 needle points, known before any is placed.
    let line = polylines(&[&[(0.0, 0.0), (100.0, 0.0)]]);
    stops_early(20_001, 1_000, |meter| zigzag(&line, 1.0, &set(&[("zigzag_spacing_mm", "0.01")]), meter));
    // A 1 m stitch placed by hand, split at the shortest longest stitch, 0.1 mm, into the most parts no
    // shorter than the shortest stitch: 3,334 needle points.
    let stitch = polylines(&[&[(0.0, 0.0), (1000.0, 0.0)]]);
    stops_early(3_334, 1_000, |meter| manual(&stitch, Some(0.1), &ParamSet::default(), meter));
}

#[test]
fn req_gen_004_a_satin_column_stops_at_the_stitches_left() {
    // A column 100 mm long at the smallest spacing: 10,001 pairs of needle points.
    let column = ladder(100.0, 4.0, &[]);
    let dense = set(&[("zigzag_spacing_mm", "0.01"), ("short_stitch_distance_mm", "0")]);
    stops_early(20_002, 1_000, |meter| satin(&column, &dense, meter));
    // A column 50 mm wide, its stitches split at the shortest longest stitch: 11 pairs, 500 needle points
    // across each.
    let wide = ladder(10.0, 50.0, &[]);
    let split = set(&[("zigzag_spacing_mm", "1"), ("max_stitch_length_mm", "0.1"), ("short_stitch_distance_mm", "0")]);
    let points = spent(u32::MAX, |meter| satin(&wide, &split, meter)).sewn.unwrap();
    assert!(points > 5_000, "{points}");
    stops_early(points, 500, |meter| satin(&wide, &split, meter));
    // Its underlays, each walk a running stitch: a centre walk 6 m long at the default 2.5 mm stitches.
    let long = ladder(6000.0, 4.0, &[]);
    let walked = set(&[("zigzag_spacing_mm", "10"), ("center_walk_underlay", "true"), ("short_stitch_distance_mm", "0")]);
    let points = spent(u32::MAX, |meter| satin(&long, &walked, meter)).sewn.unwrap();
    stops_early(points, 1_000, |meter| satin(&long, &walked, meter));
}

#[test]
fn req_gen_004_a_tatami_fill_stops_at_the_stitches_left() {
    // A 1 m x 10 mm rectangle in rows 1 mm apart, in stitches of the shortest stitch, 0.3 mm: 11 rows of
    // 3,334 needle points or so.
    let long = (1000.0, 10.0);
    let points = spent(u32::MAX, |meter| tatami(long, rows(1.0, 0.3), &[], meter)).sewn.unwrap();
    assert!(points > 33_000, "{points}");
    stops_early(points, 1_000, |meter| tatami(long, rows(1.0, 0.3), &[], meter));
    // A 100 mm square in rows at the smallest spacing, each a stitch long: the rows alone are 1,001
    // segments, which are laid out before any is sewn.
    let square = (100.0, 100.0);
    let points = spent(u32::MAX, |meter| tatami(square, rows(0.1, 100.0), &[], meter)).sewn.unwrap();
    stops_early(points, 1_000, |meter| tatami(square, rows(0.1, 100.0), &[], meter));
    // Its underlay alike, under rows that make few needle points.
    let points = spent(u32::MAX, |meter| tatami(long, rows(5.0, 100.0), &[rows(1.0, 0.3)], meter)).sewn.unwrap();
    stops_early(points, 1_000, |meter| tatami(long, rows(5.0, 100.0), &[rows(1.0, 0.3)], meter));
}

#[test]
fn req_gen_004_an_element_spends_a_stitch_for_each_needle_point_it_gives() {
    // Every generator, on shapes whose points it drops, merges or lists again: what it spent comes to the
    // needle points it gives.
    let square = rings(&[&[(0.0, 0.0), (10.0, 0.0), (10.0, 10.0), (0.0, 10.0), (0.0, 0.0)]]);
    let curl = polylines(&[&[(0.0, 0.0), (0.1, 0.0), (0.1, 0.1), (5.0, 0.1), (5.0, 0.1), (9.0, 3.0)]]);
    let cases: Vec<(&str, Box<Generator>)> = vec![
        ("running", Box::new(|m| running(&curl, &set(&[("repeats", "3"), ("bean_stitch_repeats", "1 0")]), m))),
        ("running ring", Box::new(|m| running(&square, &set(&[("repeats", "2")]), m))),
        ("zigzag", Box::new(|m| zigzag(&square, 2.0, &set(&[("repeats", "2"), ("bean_stitch_repeats", "1")]), m))),
        ("manual", Box::new(|m| manual(&curl, Some(1.0), &set(&[("bean_stitch_repeats", "2 0")]), m))),
        (
            "satin",
            Box::new(|m| {
                let all =
                    [("center_walk_underlay", "true"), ("contour_underlay", "true"), ("zigzag_underlay", "true"), ("max_stitch_length_mm", "1.5")];
                satin(&ladder(20.0, 4.0, &[10.0]), &set(&all), m)
            }),
        ),
        ("e-stitch", Box::new(|m| satin(&ladder(20.0, 4.0, &[]), &set(&[("satin_method", "e_stitch"), ("max_stitch_length_mm", "1.5")]), m))),
        ("s-stitch", Box::new(|m| satin(&ladder(20.0, 4.0, &[]), &set(&[("satin_method", "s_stitch"), ("max_stitch_length_mm", "1.5")]), m))),
        ("zigzag satin", Box::new(|m| satin(&ladder(20.0, 4.0, &[]), &set(&[("satin_method", "zigzag"), ("max_stitch_length_mm", "1.5")]), m))),
        ("tatami", Box::new(|m| tatami((20.0, 20.0), rows(0.4, 3.0), &[rows(2.0, 3.0)], m))),
    ];
    for (name, generator) in &cases {
        let sewn = spent(u32::MAX, generator);
        let points = sewn.sewn.unwrap();
        assert!(points > 10, "{name}: {points}");
        assert_eq!(usize::try_from(sewn.stitches).unwrap(), points, "{name}");
        // One stitch fewer than it gives is not enough.
        let limit = u32::try_from(points).unwrap();
        assert_eq!(spent(limit - 1, generator).sewn, Err(Exhausted::Stitches), "{name}");
    }
    // A stroke along a straight line drops and copies nothing it made but the passes it sews again: as
    // many stitches as it gives are enough, every pass and bean stitch spent once.
    let line = polylines(&[&[(0.0, 0.0), (10.0, 0.0)]]);
    let bold = set(&[("repeats", "3"), ("bean_stitch_repeats", "2 0")]);
    let strokes: [(&str, &Generator); 3] = [
        ("running", &|m| running(&line, &bold, m)),
        ("zigzag", &|m| zigzag(&line, 1.0, &bold, m)),
        ("manual", &|m| manual(&line, Some(2.0), &bold, m)),
    ];
    for (name, generator) in strokes {
        let points = spent(u32::MAX, generator).sewn.unwrap();
        assert_eq!(spent(u32::try_from(points).unwrap(), generator).sewn, Ok(points), "{name}");
    }
}

#[test]
fn req_gen_004_each_element_is_given_what_the_ones_before_it_left() {
    // 3 lines of 41 needle points, and a stitch type not sewn yet after them, which only says so.
    let design = |stitches: u32| {
        let elements = vec![
            line("a", (0.0, 0.0), 100.0, &RED, &[]),
            line("b", (0.0, 5.0), 100.0, &RED, &[]),
            line("c", (0.0, 10.0), 100.0, &RED, &[]),
            line("d", (0.0, 15.0), 10.0, &RED, &[("stroke_method", "ripple_stitch")]),
        ];
        let design = Design::new(elements, DesignSettings::default()).unwrap();
        plan(&design, REFERENCE, &Budget { max_stitches: stitches, ..Budget::DEFAULT })
    };
    let whole = design(Budget::DEFAULT.max_stitches);
    assert!(whole.plan.is_some());
    assert_eq!(whole.diagnostics.iter().map(|d| d.code).collect::<Vec<_>>(), [Code::StitchTypeNotYet]);
    // 100 stitches: a and b leave 18, too few for c. Planning stops there, before d is looked at.
    let short = design(100);
    assert_eq!(short.plan, None);
    assert_eq!(messages(&short), ["error SC-E0004: The design needs more than 100 stitches, the limit for one design."]);
    assert_eq!(short.diagnostics[0].element, Some(ElementId::new("c").unwrap()));
}

#[test]
fn req_gen_004_an_element_generated_alone_with_too_few_stitches_is_skipped_and_says_so() {
    // A preview generates one element at a time: a line of 41 needle points with a budget of 40 stitches
    // sews nothing, and SC-E0004 names it. 41 are enough.
    let element = line("a", (0.0, 0.0), 100.0, &RED, &[]);
    let alone =
        |max_stitches| generate(&element, &DesignSettings::default(), REFERENCE, &Neighbours::default(), &Budget { max_stitches, ..Budget::DEFAULT });
    let few = alone(40);
    assert_eq!(few.generated, None);
    let said: Vec<String> = few.diagnostics.iter().map(ToString::to_string).collect();
    assert_eq!(said, ["error SC-E0004: The design needs more than 40 stitches, the limit for one design."]);
    assert_eq!(few.diagnostics[0].element, Some(ElementId::new("a").unwrap()));
    let enough = alone(41);
    assert_eq!(enough.generated.map(|generated| generated.groups.iter().map(Vec::len).sum::<usize>()), Some(41));
    assert!(enough.diagnostics.is_empty(), "{:?}", enough.diagnostics);
}
