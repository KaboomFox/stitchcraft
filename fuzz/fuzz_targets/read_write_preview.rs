//! Any file end to end: read, written in every format, PES read back unchanged in what the machine does, previewed.
//! The body, and why it lives elsewhere: `stitchcraft_testkit::fuzz::read_write_preview`.
#![no_main]

libfuzzer_sys::fuzz_target!(|data: &[u8]| stitchcraft_testkit::fuzz::read_write_preview(data));
