//! SVG adapter (layer L3): SVG documents to the engine's design model.
//!
//! Hosts translate their documents into a [`Design`](stitchcraft_engine::design::Design), so the engine
//! never sees SVG (`docs/src/design/architecture.md` › Hosts, ports and adapters). This crate is the
//! translation for SVG files: [`read`] turns a file into a design and the warnings about what it left out
//! or simplified.
//!
//! What it reads (roadmap step M3.3): paths and the basic shapes, groups and `<switch>`, transforms, the
//! root's size and viewBox (so lengths come out in millimetres), fill and stroke colours with
//! `currentColor` and paint order, and everything that hides an element. Ink/Stitch's `inkstitch:*`
//! settings, commands and clones follow in milestone M8.
//!
//! Its invariants:
//!
//! - **It never fails on content.** Any path data, degenerate arcs included, is read the way SVG viewers
//!   draw it (`REQ-SVG-002`, fuzzed); only a file that is not SVG at all is refused (`SC-E0801`).
//! - **Nothing is dropped silently.** What it leaves out or simplifies is a warning (`SC-W0802`,
//!   `SC-W0804`), unless a viewer would not show it either.
//! - **Positions are exact** to well within a micrometre (`REQ-SVG-001`), and the same on every platform:
//!   transforms are built with `stitchcraft_core::math`, not the platform's trigonometry.
//! - **Work is bounded** by the file size limit, the XML node limit and the budget.
#![forbid(unsafe_code)]

mod document;
mod path;
mod style;
mod transform;

pub use document::{MAX_BYTES, Svg, read};
