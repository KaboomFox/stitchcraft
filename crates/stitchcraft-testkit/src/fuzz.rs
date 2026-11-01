//! What must hold for *any* bytes given to a reader (REQ-FMT-006, REQ-SVG-002, REQ-VC-001): the bodies of
//! the fuzz targets in `fuzz/`.
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
//!   written in every format without a panic, and reads back to a plan that makes the machine do the same
//!   thing: what the plan tells a machine in that format ([`format_events`]: PES asks for a cut before
//!   every jump, and DST machines cut the thread before long runs of jumps).
//! - **Previews never panic** on anything read, in either style; refusals such as a spent budget are
//!   fine.
//! - **The SVG reader never panics** and refuses only with its own codes: `SC-E0801` for a file that is
//!   not SVG, `SC-E0004` for a spent budget. What it reads is a valid design (`SC-E0009` would be a bug in
//!   the reader), and everything it says about the file is a warning.
//! - **So does the VectorCraft reader**, with `SC-E0806` for a file that is not a VectorCraft document. The
//!   same bytes compressed read the same.

use std::io::Write;

use flate2::Compression;
use flate2::write::GzEncoder;
use stitchcraft_core::{Budget, Code, Severity, units::MACHINE_LIMIT};
use stitchcraft_formats::decode::MAX_RECORDS;
use stitchcraft_formats::{decode, dst, encode, pes};
use stitchcraft_plan::{FormatId, StitchPlan, TrimSupport};
use stitchcraft_render::raster::MARGIN_MM;
use stitchcraft_render::{Scene, Settings, Style};

use crate::equivalence::{events, format_events};

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
        let Ok(encoded) = encode(&decoded.plan, *format, &decoded.name) else { continue };
        let back = decode(&encoded.bytes).unwrap_or_else(|e| panic!("StitchCraft cannot read the {} file it wrote: {e}", format.name()));
        assert_eq!(events(&back.plan), format_events(&decoded.plan, *format), "the {} file makes the machine do something else", format.name());
    }
    // Both styles, at a scale that keeps the image near 96 pixels so each input stays fast: filling and
    // encoding cost one step per pixel, whatever the stitches. Refusing (budget spent) is fine.
    let mut meter = Budget { max_stitches: 100_000, max_work: 2_000_000 }.meter();
    // A machine that obeys the trim command marks every trim the file holds.
    let Ok(scene) = Scene::of(&decoded.plan, TrimSupport::Command, &mut meter) else { return };
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
            // What the reader says about a file it reads is a warning or a note, never an error.
            for said in &svg.warnings {
                assert_ne!(said.severity(), Severity::Error, "{said}");
            }
            let limit = f64::from(MACHINE_LIMIT) / 10.0;
            for point in svg.design.elements().iter().flat_map(|e| e.shape.path().points()) {
                assert!(point.x().abs() <= limit && point.y().abs() <= limit, "a point beyond ±10 m: {point:?}");
            }
        }
        Err(refusal) => assert!(matches!(refusal.code, Code::SvgUnreadable | Code::BudgetExhausted), "{refusal}"),
    }
}

/// The VectorCraft reader on `data`, as it is and compressed.
pub fn read_vectorcraft(data: &[u8]) {
    let budget = Budget { max_stitches: 100_000, max_work: 2_000_000 };
    let plain = stitchcraft_vectorcraft::read(data, &budget);
    match &plain {
        Ok(read) => {
            for said in &read.warnings {
                assert_ne!(said.severity(), Severity::Error, "{said}");
            }
            let limit = f64::from(MACHINE_LIMIT) / 10.0;
            for point in read.design.elements().iter().flat_map(|e| e.shape.path().points()) {
                assert!(point.x().abs() <= limit && point.y().abs() <= limit, "a point beyond ±10 m: {point:?}");
            }
        }
        Err(refusal) => assert!(matches!(refusal.code, Code::VcUnreadable | Code::BudgetExhausted), "{refusal}"),
    }
    // The part of the file a bug-report bundle keeps reads as the file does.
    assert_eq!(stitchcraft_vectorcraft::read(stitchcraft_vectorcraft::read_part(data), &budget), plain, "its part reads otherwise");
    // Compressed, the same bytes say the same, unless the budget runs out unpacking them. Bytes that are
    // compressed already would be compressed twice, which no file is.
    let mut encoder = GzEncoder::new(Vec::new(), Compression::fast());
    if !data.starts_with(&[0x1f, 0x8b])
        && encoder.write_all(data).is_ok()
        && let Ok(packed) = encoder.finish()
    {
        match (stitchcraft_vectorcraft::read(&packed, &budget), plain) {
            (Err(e), _) if e.code == Code::BudgetExhausted => {}
            (packed, plain) => assert_eq!(packed, plain, "compressed, the file reads otherwise"),
        }
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
    use stitchcraft_plan::{PlanBuilder, Provenance, Rgb, Role, Thread};

    use super::*;
    use crate::strategies;

    // The seed corpora are every file of their folders.
    #[allow(clippy::disallowed_methods)]
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

    /// Designs farther from their start than a PES file's origin field reaches (REQ-FMT-011) pass every
    /// body: the PES writer refuses them. A DST file of a design 411 mm left of its start, as large frames
    /// sew them, and a golden PES file whose origin field's first byte is changed, so that it reads as a
    /// jump of −184.8 mm and the design reaches −204.8 mm.
    #[test]
    fn req_fmt_011_designs_far_from_their_start_pass_every_fuzz_body() {
        let p = |x: f64| stitchcraft_core::Point::new(x, 0.0).unwrap();
        let mut far = PlanBuilder::new(Thread::new(Rgb::new(0, 0, 0)));
        far.jump(p(-411.4), Provenance::plan(Role::Travel));
        for x in [-411.4, -409.4, -407.4] {
            far.stitch(p(x), Provenance::plan(Role::Top));
        }
        let dst = encode(&far.finish(), FormatId::Dst, "far").unwrap().bytes;
        let mut changed = include_bytes!("../../../conformance/golden/formats/every-command.pes").to_vec();
        changed[22 + 528] = 0x98;
        for bytes in [dst, changed] {
            read_write_preview(&bytes);
        }
    }

    #[allow(clippy::disallowed_methods)]
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

    #[allow(clippy::disallowed_methods)]
    fn vectorcraft_fixtures() -> Vec<Vec<u8>> {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../conformance/fixtures/vectorcraft");
        let mut paths: Vec<_> = std::fs::read_dir(root).unwrap().map(|e| e.unwrap().path()).collect();
        paths.sort();
        let files: Vec<_> = paths.iter().map(|p| std::fs::read(p).unwrap()).collect();
        assert!(files.len() >= 4, "the VectorCraft fixtures are the seed corpus of `read_vectorcraft`");
        files
    }

    /// The VectorCraft seed corpus, and every shortening of it, pass the VectorCraft fuzz body.
    #[test]
    fn req_vc_001_the_vectorcraft_seed_corpus_and_its_shortenings_pass_the_fuzz_body() {
        for bytes in vectorcraft_fixtures() {
            let step = (bytes.len() / 200).max(1);
            for len in (0..=bytes.len()).step_by(step) {
                read_vectorcraft(&bytes[..len]);
            }
            read_vectorcraft(&bytes);
        }
    }

    proptest! {
        #![proptest_config(strategies::config(256))]

        /// The VectorCraft fixtures with random bytes overwritten, so the reader gets deep into real files.
        #[test]
        fn req_vc_001_damaged_vectorcraft_files_pass_the_fuzz_body(
            which in any::<prop::sample::Index>(),
            edits in proptest::collection::vec((any::<prop::sample::Index>(), any::<u8>()), 1..16),
        ) {
            let fixtures = vectorcraft_fixtures();
            let mut data = which.get(&fixtures).clone();
            for (at, byte) in edits {
                let i = at.index(data.len());
                data[i] = byte;
            }
            read_vectorcraft(&data);
        }

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
