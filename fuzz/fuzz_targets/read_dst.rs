//! The DST reader on any bytes: no panic, and caps respected.
//! The body, and why it lives elsewhere: `stitchcraft_testkit::fuzz::read_dst`.
#![no_main]

libfuzzer_sys::fuzz_target!(|data: &[u8]| stitchcraft_testkit::fuzz::read_dst(data));
