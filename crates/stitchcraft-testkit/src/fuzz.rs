//! What must hold for *any* bytes given to a reader (REQ-FMT-006, REQ-SVG-002): the bodies of the fuzz
//! targets in `fuzz/`.
//!
//! They live here rather than in the fuzz crate so that every pull request runs them on stable Rust and
//! all three operating systems (on the golden files, every shortening of them and random bytes), while
//! the nightly job feeds them inputs found by coverage-guided fuzzing (`cargo +nightly fuzz`, which
//! needs nightly Rust and libFuzzer). A body panics when a property breaks: that is how libFuzzer learns
//! about it. Memory and time limits are libFuzzer's (`-rss_limit_mb`, `-timeout`), set by the job.
//!
//! The properties:
//!
//! - **Readers never panic** and what they return respects their caps: at most
//!   [`MAX_RECORDS`] entries, every position within ±10 m.
//! - **StitchCraft reads back what it writes from anything it read.** A plan read from any file is
//!   written in every format without a panic. Written as PES, it reads back to a plan that makes the
//!   machine do the same thing ([`events`]). DST is only checked for not panicking: it spells trims as
//!   jump runs, so small jumps next to a trim in a hostile file can legitimately read back differently.
//! - **Previews never panic** on anything read, in either style; refusals such as a spent budget are
//!   fine.
//! - **The SVG reader never panics** and refuses only with its own codes: `SC-E0801` for a file that is
//!   not SVG, `SC-E0004` for a spent budget. What it reads is a valid design (`SC-E0009` would be a bug in
//!   the reader), and everything it says about the file is a warning.

use stitchcraft_core::{Budget, Code, Severity, units::MACHINE_LIMIT};
use stitchcraft_formats::decode::MAX_RECORDS;
use stitchcraft_formats::{decode, dst, encode, pes};
use stitchcraft_plan::{FormatId, StitchPlan};
use stitchcraft_render::raster::MARGIN_MM;
use stitchcraft_render::{Scene, Settings, Style};

use crate::equivalence::events;

/// The PES/PEC reader on `data`.
pub fn read_pes(data: &[u8]) {
    if let Ok(decoded) = pes::decode(data) {
        within_caps(&decoded.plan);
    }
}

/// The DST reader on `data`.
pub fn read_dst(data: &[u8]) {
    if let Ok(decoded) = dst::decode(data) {
        within_caps(&decoded.plan);
    }
}

/// Any file, end to end: read it, write it in every format, read the PES file back, preview it.
pub fn read_write_preview(data: &[u8]) {
    let Ok(decoded) = decode(data) else { return };
    within_caps(&decoded.plan);
    for format in FormatId::ALL {
        let Ok(bytes) = encode(&decoded.plan, *format, &decoded.name) else { continue };
        if *format == FormatId::PesV1 {
            let back = decode(&bytes).unwrap_or_else(|e| panic!("StitchCraft cannot read the PES file it wrote: {e}"));
            assert_eq!(events(&back.plan), events(&decoded.plan), "the PES file makes the machine do something else");
        }
    }
    // Both styles, at a scale that keeps the image near 96 pixels so each input stays fast: filling and
    // encoding cost one step per pixel, whatever the stitches. Refusing (budget spent) is fine.
    let mut meter = Budget { max_stitches: 100_000, max_work: 2_000_000 }.meter();
    let Ok(scene) = Scene::of(&decoded.plan, &mut meter) else { return };
    let Some((lo, hi)) = scene.bounds() else { return };
    let extent_mm = f64::from(hi.x.abs_diff(lo.x).max(hi.y.abs_diff(lo.y))) / 10.0 + 2.0 * f64::from(MARGIN_MM);
    #[allow(clippy::cast_possible_truncation)] // A ratio clamped to the scale range.
    let scale = (96.0 / extent_mm).clamp(f64::from(Settings::MIN_SCALE), 4.0) as f32;
    for style in Style::ALL {
        if let Some(settings) = Settings::new(*style, scale) {
            let _ = scene.render(settings, &mut meter);
        }
    }
}

/// The SVG reader on `data`, with a budget small enough to keep each input fast.
pub fn read_svg(data: &[u8]) {
    let budget = Budget { max_stitches: 100_000, max_work: 2_000_000 };
    match stitchcraft_svg::read(data, &budget) {
        Ok(svg) => {
            for warning in &svg.warnings {
                assert_eq!(warning.severity(), Severity::Warning, "{warning}");
            }
            let limit = f64::from(MACHINE_LIMIT) / 10.0;
            for point in svg.design.elements().iter().flat_map(|e| e.shape.path().points()) {
                assert!(point.x().abs() <= limit && point.y().abs() <= limit, "a point beyond ±10 m: {point:?}");
            }
        }
        Err(refusal) => assert!(matches!(refusal.code, Code::SvgUnreadable | Code::BudgetExhausted), "{refusal}"),
    }
}

/// A plan a reader returned respects the readers' caps.
fn within_caps(plan: &StitchPlan) {
    let entries = plan.stitches().count();
    assert!(entries <= MAX_RECORDS, "{entries} entries, more than the cap of {MAX_RECORDS}");
    let limit = f64::from(MACHINE_LIMIT) / 10.0;
    for s in plan.stitches() {
        assert!(s.at.x().abs() <= limit && s.at.y().abs() <= limit, "a position beyond ±10 m: {:?}", s.at);
    }
}

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;
    use crate::strategies;

    fn golden_files() -> Vec<Vec<u8>> {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../conformance/golden");
        let mut files = Vec::new();
        for dir in ["formats", "testsheets"] {
            let mut paths: Vec<_> = std::fs::read_dir(root.join(dir)).unwrap().map(|e| e.unwrap().path()).collect();
            paths.sort();
            files.extend(paths.iter().map(|p| std::fs::read(p).unwrap()));
        }
        assert!(files.len() >= 10, "the golden machine files are the seed corpus");
        files
    }

    /// The seed corpus of the nightly fuzzing, and every shortening of it, pass every body.
    #[test]
    fn req_fmt_006_the_seed_corpus_and_its_shortenings_pass_every_fuzz_body() {
        for bytes in golden_files() {
            let step = (bytes.len() / 300).max(1);
            for len in (0..=bytes.len()).step_by(step) {
                let part = &bytes[..len];
                read_pes(part);
                read_dst(part);
                read_write_preview(part);
            }
            read_write_preview(&bytes);
        }
    }

    fn svg_fixtures() -> Vec<Vec<u8>> {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../conformance/fixtures/svg");
        let mut paths: Vec<_> = std::fs::read_dir(root).unwrap().map(|e| e.unwrap().path()).collect();
        paths.sort();
        let files: Vec<_> = paths.iter().map(|p| std::fs::read(p).unwrap()).collect();
        assert!(files.len() >= 4, "the SVG fixtures are the seed corpus of `read_svg`");
        files
    }

    /// The SVG seed corpus, and every shortening of it, pass the SVG fuzz body.
    #[test]
    fn req_svg_002_the_svg_seed_corpus_and_its_shortenings_pass_the_fuzz_body() {
        for bytes in svg_fixtures() {
            let step = (bytes.len() / 300).max(1);
            for len in (0..=bytes.len()).step_by(step) {
                read_svg(&bytes[..len]);
            }
            read_svg(&bytes);
        }
    }

    proptest! {
        #![proptest_config(strategies::config(256))]

        /// The SVG fixtures with random bytes overwritten, so the reader gets deep into real files.
        #[test]
        fn req_svg_002_damaged_svg_files_pass_the_fuzz_body(
            which in any::<prop::sample::Index>(),
            edits in proptest::collection::vec((any::<prop::sample::Index>(), any::<u8>()), 1..16),
        ) {
            let fixtures = svg_fixtures();
            let mut data = which.get(&fixtures).clone();
            for (at, byte) in edits {
                let i = at.index(data.len());
                data[i] = byte;
            }
            read_svg(&data);
        }

        /// Random bytes behind each format's magic, so the readers get past their first check.
        #[test]
        fn req_fmt_006_random_bytes_pass_every_fuzz_body(
            magic in prop_oneof![Just(&b"#PES0001"[..]), Just(&b"#PEC0001"[..]), Just(&b"LA:"[..])],
            body in proptest::collection::vec(any::<u8>(), 0..2048),
        ) {
            let mut data = magic.to_vec();
            data.extend_from_slice(&body);
            read_pes(&data);
            read_dst(&data);
            read_write_preview(&data);
        }
    }
}
