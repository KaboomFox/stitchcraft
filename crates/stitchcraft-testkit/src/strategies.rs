//! Random inputs for property tests — with a fixed seed on every pull request.
//!
//! A property test that fails on one CI run and passes on the next is noise, so PR runs use [`SEED`];
//! nightly runs set `PROPTEST_RNG_SEED` to explore fresh inputs, and a failure found there is saved as a
//! regression case (`docs/src/design/conformance.md`).

use proptest::prelude::*;
use proptest::test_runner::{Config, RngSeed};
use stitchcraft_core::Point;
use stitchcraft_plan::{PlanBuilder, Provenance, Rgb, Role, StitchPlan, Thread};

/// The seed of every PR run.
pub const SEED: u64 = 0x5717_C4AF_7E57_0001;

/// A configuration for `cases` cases: fixed seed, unless `PROPTEST_RNG_SEED` asks for another.
pub fn config(cases: u32) -> Config {
    let config = Config { cases, ..Config::default() };
    if std::env::var_os("PROPTEST_RNG_SEED").is_some() { config } else { Config { rng_seed: RngSeed::Fixed(SEED), ..config } }
}

#[derive(Clone, Debug)]
enum Op {
    Stitch(f64, f64),
    Jump(f64, f64),
    Trim,
    Stop,
    Thread(Rgb),
}

/// A random plan: a first stitch, then up to `max_ops` stitches, jumps, trims, stops and thread changes,
/// positions within ±`extent` mm on a 0.01 mm grid. Plans are encodable (they sew at least one stitch and
/// commands happen at the needle) but deliberately not tidy: they need not satisfy the plan invariants.
pub fn plan(max_ops: usize, extent: f64) -> impl Strategy<Value = StitchPlan> {
    let coordinate = move || (-extent..=extent).prop_map(|v: f64| (v * 100.0).round() / 100.0);
    let op = prop_oneof![
        6 => (coordinate(), coordinate()).prop_map(|(x, y)| Op::Stitch(x, y)),
        2 => (coordinate(), coordinate()).prop_map(|(x, y)| Op::Jump(x, y)),
        1 => Just(Op::Trim),
        1 => Just(Op::Stop),
        1 => any::<[u8; 3]>().prop_map(|[r, g, b]| Op::Thread(Rgb::new(r, g, b))),
    ];
    (any::<[u8; 3]>(), coordinate(), coordinate(), proptest::collection::vec(op, 0..max_ops)).prop_map(|([r, g, b], x, y, ops)| {
        let p = |x: f64, y: f64| Point::new(x, y).unwrap();
        let (top, travel) = (Provenance::plan(Role::Top), Provenance::plan(Role::Travel));
        let mut builder = PlanBuilder::new(Thread::new(Rgb::new(r, g, b)));
        builder.stitch(p(x, y), top);
        for op in ops {
            match op {
                Op::Stitch(x, y) => builder.stitch(p(x, y), top),
                Op::Jump(x, y) => builder.jump(p(x, y), travel),
                Op::Trim => builder.trim(None),
                Op::Stop => builder.stop(None),
                Op::Thread(color) => builder.change_thread(Thread::new(color)),
            }
        }
        builder.finish()
    })
}
