//! The SVG reader on any bytes: no panic, only its own refusals, and a valid design.
//! The body, and why it lives elsewhere: `stitchcraft_testkit::fuzz::read_svg`.
#![no_main]

libfuzzer_sys::fuzz_target!(|data: &[u8]| stitchcraft_testkit::fuzz::read_svg(data));
