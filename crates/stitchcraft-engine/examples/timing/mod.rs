//! What the engine's benchmark examples share: the median time of a run. A file of its own folder, so that
//! Cargo does not take it for an example.

// It times a benchmark, which library code never may (clippy.toml).
#![allow(clippy::disallowed_types)]

use std::time::Instant;

/// The median of `runs` timings of `run`, in milliseconds, or 0 for no run.
pub fn median_ms<T>(runs: usize, mut run: impl FnMut() -> T) -> f64 {
    let mut times: Vec<f64> = (0..runs)
        .map(|_| {
            let start = Instant::now();
            std::hint::black_box(run());
            start.elapsed().as_secs_f64() * 1000.0
        })
        .collect();
    times.sort_by(f64::total_cmp);
    times.get(times.len() / 2).copied().unwrap_or_default()
}
