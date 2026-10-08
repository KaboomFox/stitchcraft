//! Stitch plans, machine profiles and thread palettes (layer L1).
//!
//! A stitch plan is the ordered list of stitches, jumps, trims, stops and colour changes for a design.
//! The engine produces plans; machine formats encode and decode them; the invariant checker in this crate
//! validates every plan before it can be written.
//!
//! Status: planned for roadmap steps M1.2, M1.3 and M1.6. Design: `docs/src/design/data-model.md`.
#![forbid(unsafe_code)]
