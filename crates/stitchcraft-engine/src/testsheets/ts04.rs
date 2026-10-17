//! TS-04 — lock stitches: does each lock hold when its tail is pulled, and can it be seen?
//!
//! Drawn as a design (`designed`): a row for each lock shape, in the order of the lock table (custom locks
//! aside, which have no shape of their own), each of three lines of 30 mm with that lock at both ends and a
//! trim after it, so the lock is all that holds each end once the jump thread is snipped. The sizes grow to
//! the right: the half stitch is sized from the first stitch, half of it up to 1 mm, so its lines have first
//! stitches of 1, 1.5 and 2.5 mm, the last as long as the default running stitch's; locks made of steps are
//! sized by `lock_*_scale_mm` (0.5, 0.7, 1.0 mm), drawn ones by `lock_*_scale_percent` (70, 100, 150 %).
//!
//! The rows run left to right and right to left in turn, so each starts below where the one before ended.
//! The reference machine cuts no jump (MC-1): a row sewn from the same side as the one before would start
//! along the jump thread from that row's end, and sew it down under its first lock.

use stitchcraft_plan::StitchPlan;

use super::designed::Drawing;
use super::sketch::SheetError;
use super::{RED, thread};
use crate::locks::{LOCKS, SIZED_IN_MM, SIZED_IN_PERCENT};

/// The half stitch's first stitches, small to large (mm): every line's length is a whole number of them.
pub const FIRST_STITCHES: [f64; 3] = [1.0, 1.5, 2.5];
/// The sizes of locks made of steps, small to large (`lock_*_scale_mm`).
pub const STEP_SIZES: [&str; 3] = ["0.5", "0.7", "1.0"];
/// The sizes of drawn locks, small to large (`lock_*_scale_percent`).
pub const DRAWN_SIZES: [&str; 3] = ["70", "100", "150"];
/// The lines' length (mm).
const LINE: f64 = 30.0;
/// The distance between columns and between rows (mm).
const COLUMN: f64 = 40.0;
const ROW: f64 = 8.0;

/// The lock shapes, row by row, top to bottom.
pub fn rows() -> impl Iterator<Item = &'static str> {
    LOCKS.iter().map(|lock| lock.id).filter(|&id| id != "custom")
}

/// Whether the `row`th row from the top is sewn left to right: the even ones are.
pub fn rightwards(row: usize) -> bool {
    row.is_multiple_of(2)
}

/// Draws TS-04.
pub(super) fn build() -> Result<StitchPlan, SheetError> {
    let red = thread(RED)?;
    let mut d = Drawing::new("ts04");
    let mut y = 0.0;
    for (row, lock) in rows().enumerate() {
        let mut columns: Vec<_> = [0.0, COLUMN, 2.0 * COLUMN].into_iter().zip(STEP_SIZES).zip(DRAWN_SIZES).zip(FIRST_STITCHES).enumerate().collect();
        if !rightwards(row) {
            columns.reverse();
        }
        for (column, (((x, step), drawn), first)) in columns {
            let size: Vec<(&str, String)> = if SIZED_IN_MM.contains(&lock) {
                vec![("lock_start_scale_mm", step.to_string()), ("lock_end_scale_mm", step.to_string())]
            } else if SIZED_IN_PERCENT.contains(&lock) {
                vec![("lock_start_scale_percent", drawn.to_string()), ("lock_end_scale_percent", drawn.to_string())]
            } else {
                vec![("running_stitch_length_mm", first.to_string())]
            };
            let mut params = vec![("lock_start", lock), ("lock_end", lock), ("trim_after", "true")];
            params.extend(size.iter().map(|(key, value)| (*key, value.as_str())));
            let ends = if rightwards(row) { [(x, y), (x + LINE, y)] } else { [(x + LINE, y), (x, y)] };
            d.polyline(&format!("{lock}-{column}"), &ends, &red, &params)?;
        }
        y += ROW;
    }
    d.plan()
}
