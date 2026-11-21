//! M13.57 probe hooks (NOT FOR THE REPOSITORY): what the pipeline knows of each element, the plan before the
//! cap, and per-element setting overrides and columns sewn as lines, for prototypes measured by
//! `m1357_probe`.
#![allow(missing_docs, clippy::unwrap_used)]

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Mutex;

use stitchcraft_core::ElementId;
use stitchcraft_plan::StitchPlan;

/// Each generated element's kind: base, letter, column, fill, run, person; and a key of its path's bounds.
pub static KINDS: Mutex<BTreeMap<ElementId, (&'static str, String)>> = Mutex::new(BTreeMap::new());
/// The plan before the last pass and the cap.
pub static PRE: Mutex<Option<StitchPlan>> = Mutex::new(None);
/// Settings set on elements (by id) after digitizing, before generation.
pub static OVERRIDES: Mutex<BTreeMap<ElementId, Vec<(String, String)>>> = Mutex::new(BTreeMap::new());
/// Columns (by the key of their path's bounds) that a part sews as a running line instead.
pub static LINES: Mutex<BTreeSet<String>> = Mutex::new(BTreeSet::new());
/// How many of each element's first group's top points are its underlay.
pub static UNDERLAY: Mutex<BTreeMap<ElementId, usize>> = Mutex::new(BTreeMap::new());
/// Candidate f: how far a column reaches past a rail along a later satin column.
pub static F: Mutex<Option<f64>> = Mutex::new(None);
/// How many satin rail points the crowded rule moved in, and how many it looked at (since the last reset).
pub static SHORTENED: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
pub static RAIL_POINTS: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
/// Whether the cap is skipped.
pub static NOCAP: Mutex<bool> = Mutex::new(false);

/// The key of the bounds of `points`.
pub fn key(points: impl Iterator<Item = stitchcraft_core::Point>) -> String {
    match stitchcraft_core::Rect::around(points) {
        Some(r) => format!("{:.2},{:.2},{:.2},{:.2}", r.min().x(), r.min().y(), r.max().x(), r.max().y()),
        None => String::new(),
    }
}
