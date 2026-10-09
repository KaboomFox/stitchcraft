//! TS-04 — lock stitches: does each lock hold when its tail is pulled, and can it be seen?
//!
//! Drawn as a design (`designed`): a row for each lock shape, in the order of the lock table (custom locks
//! aside, which have no shape of their own), each of three lines of about 30 mm with that lock at both
//! ends and a trim after it, so the lock is all that holds each end. The sizes grow to the right: the half
//! stitch is sized from the first stitch, so its lines have first stitches of 1.5, 2.5 and 4 mm; locks
//! made of steps are sized by `lock_*_scale_mm` (0.5, 0.7, 1.0 mm), drawn ones by
//! `lock_*_scale_percent` (70, 100, 150 %).

use stitchcraft_plan::StitchPlan;

use super::designed::Drawing;
use super::sketch::SheetError;
use super::{RED, thread};
use crate::locks::{LOCKS, SIZED_IN_MM, SIZED_IN_PERCENT};

/// The half stitch's first stitches, small to large (mm), each with a line length that is a whole
/// number of them.
pub const FIRST_STITCHES: [(f64, f64); 3] = [(1.5, 30.0), (2.5, 30.0), (4.0, 28.0)];
/// The sizes of locks made of steps, small to large (`lock_*_scale_mm`).
pub const STEP_SIZES: [&str; 3] = ["0.5", "0.7", "1.0"];
/// The sizes of drawn locks, small to large (`lock_*_scale_percent`).
pub const DRAWN_SIZES: [&str; 3] = ["70", "100", "150"];
/// The lines' length, but the half stitch's (mm).
const LINE: f64 = 30.0;
/// The distance between columns and between rows (mm).
const COLUMN: f64 = 40.0;
const ROW: f64 = 8.0;

/// The lock shapes, row by row, top to bottom.
pub fn rows() -> impl Iterator<Item = &'static str> {
    LOCKS.iter().map(|lock| lock.id).filter(|&id| id != "custom")
}

/// Draws TS-04.
pub(super) fn build() -> Result<StitchPlan, SheetError> {
    let red = thread(RED)?;
    let mut d = Drawing::new("ts04");
    let mut y = 0.0;
    for lock in rows() {
        let columns = [0.0, COLUMN, 2.0 * COLUMN].into_iter().zip(STEP_SIZES).zip(DRAWN_SIZES).zip(FIRST_STITCHES);
        for (column, (((x, step), drawn), (first, half_stitch_line))) in columns.enumerate() {
            let (size, line): (Vec<(&str, String)>, f64) = if SIZED_IN_MM.contains(&lock) {
                (vec![("lock_start_scale_mm", step.to_string()), ("lock_end_scale_mm", step.to_string())], LINE)
            } else if SIZED_IN_PERCENT.contains(&lock) {
                (vec![("lock_start_scale_percent", drawn.to_string()), ("lock_end_scale_percent", drawn.to_string())], LINE)
            } else {
                (vec![("running_stitch_length_mm", first.to_string())], half_stitch_line)
            };
            let mut params = vec![("lock_start", lock), ("lock_end", lock), ("trim_after", "true")];
            params.extend(size.iter().map(|(key, value)| (*key, value.as_str())));
            d.polyline(&format!("{lock}-{column}"), &[(x, y), (x + line, y)], &red, &params)?;
        }
        y += ROW;
    }
    d.plan()
}
