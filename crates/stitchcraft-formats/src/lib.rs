//! Machine embroidery formats (layer L2): PES/PEC for Brother and DST first.
//!
//! Writers quantize once and respect each format's per-record limits; readers are hostile-input parsers
//! that check every length before use and never panic.
//!
//! Status: writers planned for roadmap milestone M1, readers for M2. Design: `docs/src/design/formats.md`.
#![forbid(unsafe_code)]
#![deny(clippy::indexing_slicing)]
