//! What a PES file says beside the stitches: which Brother thread each colour entry is (REQ-THREAD-002),
//! where the design lies from the start (REQ-FMT-011), and the pictures machines show (REQ-FMT-012).

// Test code may unwrap and index (clippy.toml allows it inside #[test] functions; these helpers are test
// code too).
#![allow(clippy::unwrap_used, clippy::indexing_slicing)]

use proptest::prelude::*;
use stitchcraft_core::Point;
use stitchcraft_formats::{EncodeError, decode, encode};
use stitchcraft_plan::{FormatId, PlanBuilder, Provenance, Rgb, Role, StitchPlan, Thread};
use stitchcraft_testkit::equivalence::events;
use stitchcraft_testkit::strategies;

/// Where the PEC block starts in every PES v1 file StitchCraft writes.
const PEC: usize = 22;
/// Bytes per thumbnail: 38 rows of 6.
const PICTURE: usize = 228;

fn p(x: f64, y: f64) -> Point {
    Point::new(x, y).unwrap()
}

fn golden(name: &str) -> Vec<u8> {
    std::fs::read(format!("{}/../../conformance/golden/{name}", env!("CARGO_MANIFEST_DIR"))).unwrap()
}

fn pes(plan: &StitchPlan) -> Vec<u8> {
    encode(plan, FormatId::PesV1, "test").unwrap().bytes
}

/// A block of `thread` for each of `xs`: a jump to (x, 0), and stitches there and 2 mm to the right.
fn blocks(threads: &[Thread], xs: &[f64]) -> StitchPlan {
    let mut b = PlanBuilder::new(threads[0].clone());
    for (i, (thread, &x)) in threads.iter().zip(xs).enumerate() {
        if i > 0 {
            b.trim(None);
            b.change_thread(thread.clone());
        }
        b.jump(p(x, 0.0), Provenance::plan(Role::Travel));
        b.stitch(p(x, 0.0), Provenance::plan(Role::Top));
        b.stitch(p(x + 2.0, 0.0), Provenance::plan(Role::Top));
    }
    b.finish()
}

/// The palette index of each colour entry in the PEC header of `bytes`.
fn indices(bytes: &[u8]) -> Vec<u8> {
    let entries = usize::from(bytes[PEC + 48]) + 1;
    bytes[PEC + 49..PEC + 49 + entries].to_vec()
}

#[test]
fn req_thread_002_distinct_threads_get_distinct_entries_and_read_back_as_threads() {
    // Both reds' nearest Brother colour is Red, #ED171F: the first is Red itself. Written with one
    // entry for both, they would read back as one block with a stop, and the machine would show Red twice.
    let reds = [Thread::new(Rgb::from_hex(0xD81B24)), Thread::new(Rgb::from_hex(0xED171F))];
    let bytes = pes(&blocks(&reds, &[0.0, 5.0]));
    assert_eq!(indices(&bytes), [28, 5], "the nearer takes Red, the other its next nearest, Vermilion");
    let back = decode(&bytes).unwrap().plan;
    assert_eq!((back.blocks.len(), back.stats().stops), (2, 0));
    // Equal threads share their entry: a change to the same thread is a stop in PES.
    let again = [reds[0].clone(), reds[1].clone(), reds[0].clone()];
    assert_eq!(indices(&pes(&blocks(&again, &[0.0, 5.0, 10.0]))), [28, 5, 28]);
}

#[test]
fn req_thread_002_an_applique_entry_read_from_a_pes_file_is_written_back_as_it() {
    // Entry 62 is "Applique Material", which colour matching never picks.
    let mut bytes = golden("formats/one-stitch.pes");
    bytes[PEC + 49] = 62;
    let read = decode(&bytes).unwrap();
    assert_eq!(read.plan.blocks[0].thread.name.as_deref(), Some("Applique Material"));
    let written = encode(&read.plan, FormatId::PesV1, &read.name).unwrap();
    assert_eq!(indices(&written.bytes), [62]);
    assert!(written.notes.is_empty(), "{:?}", written.notes);
}

/// One block that starts with a jump to the first of `points` and sews them.
fn sewn(points: &[(f64, f64)]) -> StitchPlan {
    let mut b = PlanBuilder::new(Thread::new(Rgb::from_hex(0x0A55A3)));
    b.jump(p(points[0].0, points[0].1), Provenance::plan(Role::Travel));
    for &(x, y) in points {
        b.stitch(p(x, y), Provenance::plan(Role::Top));
    }
    b.finish()
}

#[test]
fn req_fmt_011_a_design_whose_corner_the_origin_field_cannot_reach_is_refused() {
    // The field holds the start as seen from the design's top-left corner, in 12-bit two's complement:
    // −204.8 mm to 204.7 mm. A corner 204.7 mm left of the start, or above it, fits and reads back where it
    // was written. One 204.8 mm right of it, or below it, fits too.
    for points in [[(-204.7, 0.0), (-202.7, 1.0)], [(0.0, -204.7), (1.0, -202.7)]] {
        let plan = sewn(&points);
        assert_eq!(events(&decode(&pes(&plan)).unwrap().plan), events(&plan), "{points:?}");
    }
    for points in [[(204.8, 0.0), (206.8, 1.0)], [(0.0, 204.8), (1.0, 206.8)]] {
        assert!(encode(&sewn(&points), FormatId::PesV1, "far").is_ok(), "{points:?}");
    }
    // A step farther, it would wrap round, and the design would read back elsewhere.
    for corner in [-204.8, 204.9, -250.0, 210.0] {
        for points in [[(corner, 0.0), (corner + 2.0, 1.0)], [(0.0, corner), (1.0, corner + 2.0)]] {
            let refused = encode(&sewn(&points), FormatId::PesV1, "far");
            assert!(matches!(refused, Err(EncodeError::TooLarge { .. })), "{points:?}: {refused:?}");
        }
    }
}

#[test]
fn the_hoop_indicator_is_1_for_a_design_wider_or_taller_than_100_mm() {
    // Byte 12 of the PES section: 0 for the 100 × 100 mm hoop, 1 for the 130 × 180 mm one. The designs
    // straddle the start, so their sizes are not where their corners are.
    for (points, large) in
        [([(-50.0, -5.0), (60.0, 5.0)], 1), ([(-5.0, -50.0), (5.0, 60.0)], 1), ([(-50.0, -5.0), (50.0, 5.0)], 0), ([(-5.0, -50.0), (5.0, 50.0)], 0)]
    {
        assert_eq!(pes(&sewn(&points))[12..14], [large, 0], "{points:?}");
    }
}

/// Where the thumbnails start in a PES v1 file StitchCraft wrote: the PEC end mark is the byte before.
fn thumbnails_at(bytes: &[u8]) -> usize {
    let offset = usize::from(bytes[PEC + 514]) | usize::from(bytes[PEC + 515]) << 8 | usize::from(bytes[PEC + 516]) << 16;
    PEC + 512 + offset
}

/// Row `y` of thumbnail `picture`, `#` for a set pixel.
fn row(bytes: &[u8], picture: usize, y: usize) -> String {
    let at = thumbnails_at(bytes) + picture * PICTURE + y * 6;
    (0..48).map(|x| if bytes[at + x / 8] >> (x % 8) & 1 == 1 { '#' } else { '.' }).collect()
}

#[test]
fn req_fmt_012_a_design_that_starts_by_sewing_leaves_the_top_row_blank_after_the_end_mark() {
    // No jump first: the needle goes down at (30, 30) mm, and no thread lies between the start and there.
    // Drawn, the move from the start crossed the frame into the top row, the bytes right after the stitch
    // data's end mark, and readers that end the stitch data only at `FF 00` read on into the pictures.
    let mut b = PlanBuilder::new(Thread::new(Rgb::from_hex(0x0A55A3)));
    for (x, y) in [(30.0, 30.0), (32.0, 30.0), (32.0, 32.0), (30.0, 32.0)] {
        b.stitch(p(x, y), Provenance::plan(Role::Top));
    }
    let bytes = pes(&b.finish());
    let at = thumbnails_at(&bytes);
    assert_eq!(bytes[at - 1..=at], [0xFF, 0x00]);
    assert_eq!(row(&bytes, 0, 0), ".".repeat(48));
}

proptest! {
    #![proptest_config(strategies::config(256))]

    /// Random plans, which start by sewing anywhere within 100 mm of the start: every picture keeps its
    /// top and bottom rows blank, since stitches are drawn only inside the frame.
    #[test]
    fn req_fmt_012_stitches_are_drawn_only_inside_the_frame(plan in strategies::plan(120, 100.0)) {
        let bytes = pes(&plan);
        for picture in 0..=plan.color_entries().len() {
            prop_assert_eq!(row(&bytes, picture, 0), ".".repeat(48));
            prop_assert_eq!(row(&bytes, picture, 37), ".".repeat(48));
        }
        prop_assert_eq!(bytes[thumbnails_at(&bytes)], 0);
    }
}
