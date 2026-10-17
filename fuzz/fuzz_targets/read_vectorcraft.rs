//! The VectorCraft reader on any bytes, plain and compressed: no panic, only its own refusals, and a valid
//! design. The body, and why it lives elsewhere: `stitchcraft_testkit::fuzz::read_vectorcraft`.
#![no_main]

libfuzzer_sys::fuzz_target!(|data: &[u8]| stitchcraft_testkit::fuzz::read_vectorcraft(data));
