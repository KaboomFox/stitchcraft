//! Satin columns stacked on fills and on each other (`REQ-AUTO-167`, `REQ-AUTO-168`, M13.57): how far a
//! column reaches under a later satin column, and the setting that sets it. What the warning of how thick a
//! design sews says of the logo style (`REQ-AUTO-169`) is a case of `tests/auto_photos.rs`, on a traced dog.
//!
//! Design: `docs/src/design/image-to-embroidery.md` › Columns stacked on columns. M13.57 measured that most
//! of the area above the cap, under the logo style, is 2 columns' tops on each other where one reaches the
//! whole overlap, 1 mm, past its rail under the next. A column sewn on top covers that edge with its own pull
//! compensation, so the one under it needs to reach no further than its pull compensation there. The cases
//! draw their shapes in code: strips 3 mm wide that sew as columns, and a square that sews as a fill.

// Test code may unwrap and index (clippy.toml allows it in tests).
#![allow(clippy::unwrap_used, clippy::panic, clippy::indexing_slicing)]

use stitchcraft_core::Budget;
use stitchcraft_engine::auto;
use stitchcraft_engine::design::{Design, DesignSettings, Element};
use stitchcraft_plan::Thread;
use stitchcraft_testkit::designs::{BLUE, RED, automatic, fill, unbased};

/// The setting the cases plan with: how far a column reaches past a rail under a later satin column.
const REACH: &str = "auto_column_reach_mm";

/// An automatic rectangle at `(x, y)`, `wide` by `tall`, of `thread`, with `params`.
fn rect(id: &str, (x, y): (f64, f64), (wide, tall): (f64, f64), thread: &Thread, params: &[(&str, &str)]) -> Element {
    automatic(fill(id, &[&[(x, y), (x + wide, y), (x + wide, y + tall), (x, y + tall)]], thread, params))
}

/// The pull compensation of each rail of the red column, sorted, when the red strip 3 mm wide and 30 mm long
/// is digitized with `beside`, sewn after it along its right rail.
fn reaches(red: &[(&str, &str)], beside: Element) -> Vec<f64> {
    let design = Design::new(unbased(vec![rect("red", (0.0, 0.0), (3.0, 30.0), &RED, red), beside]), DesignSettings::default()).unwrap();
    let (elements, _) = auto::digitized(&design, &Budget::DEFAULT);
    let red = elements.iter().find(|element| element.id.as_str() == "red").unwrap();
    let pull = red.params.get("pull_compensation_mm").unwrap();
    let mut each: Vec<f64> = pull.split_whitespace().map(|value| value.parse().unwrap()).collect();
    if each.len() == 1 {
        each.push(each[0]);
    }
    each.sort_by(f64::total_cmp);
    each
}

/// A blue strip 3 mm wide beside the red one, which sews as a satin column.
fn column() -> Element {
    rect("blue", (3.0, 0.0), (3.0, 30.0), &BLUE, &[])
}

/// `REQ-AUTO-167`: a column reaches its pull compensation, 0.4 mm, past a rail under a later satin column, and
/// still the whole overlap, 1 mm, under a later fill. Before M13.57 it reached 1 mm under both.
#[test]
fn req_auto_167_a_column_reaches_its_pull_compensation_under_a_later_column() {
    assert_eq!(reaches(&[], column()), [0.4, 0.4]);
    // A person's fill drawn over the strip's right millimetre sews after it, and is no column.
    let square = fill("blue", &[&[(2.0, 0.0), (22.0, 0.0), (22.0, 30.0), (2.0, 30.0)]], &BLUE, &[]);
    assert_eq!(reaches(&[], square), [0.4, 1.0]);
}

/// `REQ-AUTO-168`: `auto_column_reach_mm` sets that reach, from 0 to the overlap: 1 sews as before M13.57,
/// and 0 ends the column at its rail there. The other rail keeps its pull compensation.
#[test]
fn req_auto_168_the_setting_sets_the_reach_under_a_later_column() {
    assert_eq!(reaches(&[(REACH, "1")], column()), [0.4, 1.0]);
    assert_eq!(reaches(&[(REACH, "0")], column()), [0.0, 0.4]);
    // Above the overlap, it reads as the overlap.
    assert_eq!(reaches(&[(REACH, "1.5"), ("auto_overlap_mm", "0.8")], column()), [0.4, 0.8]);
}
