//! TS-16 — an Ink/Stitch file (M8): what StitchCraft applies of Ink/Stitch's commands and of the other
//! features of its files, sewn beside Ink/Stitch's own machine file of the sheet (MC-11). Each step of M8
//! that applies one draws its part.
//!
//! M8.4's part: a tatami fill and a satin column, each with start and end positions, which Ink/Stitch's
//! start and end commands give (`REQ-GEN-001`). The differential writes the sheet as Ink/Stitch writes a
//! file, each command's symbol where its position is, and both programs sew that file (`REQ-SVG-008`,
//! `xtask/src/conformance/differential.rs`). `stitch testsheet` plans the sheet's design itself, which the
//! SVG reader reads back from that file unchanged (`REQ-PRM-003`).
//!
//! The red fill, 40 by 30 mm with the default underlay, starts at its top-right corner and ends at its
//! bottom-left one. The blue satin column below it, 40 mm long and 5 mm wide, starts in its middle: the
//! needle runs under the column to its left end and sews it to three quarters along, runs on under it to its
//! right end, sews back, and ends at its end position, three quarters along. The 2 threads part them, so each
//! element's first stitch follows a stop.
//!
//! M8.5's part: a fill and a running stitch cut by clip paths (`REQ-SVG-011`, `REQ-GEN-005`), which the
//! design holds as clips of its elements and the differential writes as clip paths. The green fill, a 30 mm
//! square, is cut by a circle of radius 25 mm round its bottom-right corner, which leaves a quarter of a disc.
//! The black running stitch zigzags across 40 mm, cut by one clip path shaped like a U upside down, which
//! leaves its first 15 mm and its last 15 mm: 2 pieces, each sewn on its own, the needle jumping the gap
//! between them. Each clip path is one shape, which both programs take in alike (`DEV-SVG-016`).
//!
//! M8.8's part: the design's origin and stop position, which Ink/Stitch's `origin` and `stop_position`
//! commands give (`REQ-ASM-005`), and a stop after the blue column. The origin lies on the blue column's top
//! edge, 10 mm in from its left end, off the design's middle, so the needle starts there, at the hoop's
//! centre. Before the stop the frame moves to the stop position, 20 mm right of the red fill's top-right
//! corner and 10 mm above it. The origin lies 60 mm above the sheet's bottom edge, so that the sheet still
//! fits the 5 × 7 in hoop when M8.9 turns it about the origin.
//!
//! M8.9's part: the sheet is written turned right ([`TURN`]), as Ink/Stitch's `rotate_on_export` turns it,
//! about its origin, so it sews 100 mm wide and 40 mm tall, the red fill at the right and the zigzag at the
//! left. Turned, it reaches 60 mm left of the hoop's centre and 50 mm right, 10 mm above and 50 mm below.
//!
//! M8.10's part: a clone of the red fill (`REQ-SVG-012`), turned right by 45° about the fill's middle and
//! moved right of it, its middle at [`CLONE_MIDDLE`]. It sews last, after the zigzag, in red again: a jump
//! from the red fill to it would leave its thread across it, which the reference machine doesn't cut (MC-1).
//! Its rows turn with it (`REQ-SVG-013`): the fill's horizontal rows lie at -45° in the clone, as a fill's
//! angle counts them, 45° right, along its long sides as the fill's lie along its own. The commands tied to
//! the fill go with the clone, so it starts at its corner turned from the fill's top-right one, its right-hand
//! corner, and ends at its left-hand one. Turned right with the sheet, the clone lies below the red fill and
//! reaches 87.25 mm below the hoop's centre, 2.75 mm inside the hoop.
//!
//! The clone lives in the file, not in the design. A clone is how a file draws an element, and once read it
//! is an element like any other: the engine sews nothing otherwise for it, so the design holds the clone's
//! element itself, the fill's shape, positions and settings turned, and `stitch testsheet` sews that. The
//! sheet says beside its design that its file draws that element as a clone ([`super::Cloned`]), and the
//! differential's writer draws it as a `<use>` of the fill, which both programs read. The SVG reader reads
//! the clone back as the element the design holds (`REQ-PRM-003`), which the test kit's tests check. Kept in
//! the design, as M8.4's positions and M8.5's clips are, a clone would be a fact about a file that no host
//! but an SVG file could hold, which every host and the engine would carry without using it.

use stitchcraft_core::math;

use super::designed::{Drawing, polygon, round};
use super::sketch::SheetError;
use super::{BLACK, BLUE, EMERALD_GREEN, RED, thread};
use crate::design::Turn;

/// The fill's corners, left, top, right and bottom (mm).
pub const FILL: [f64; 4] = [0.0, 0.0, 40.0, 30.0];
/// The satin column's rails, at y = 40 and y = 45 from x = 0 to x = 40 (mm).
pub const RAILS: [[(f64, f64); 2]; 2] = [[(0.0, 40.0), (40.0, 40.0)], [(0.0, 45.0), (40.0, 45.0)]];
/// The green fill's square, left, top, right and bottom (mm).
pub const SQUARE: [f64; 4] = [0.0, 55.0, 30.0, 85.0];
/// The circle that cuts it: its centre, the square's bottom-right corner, and its radius (mm).
pub const CIRCLE: ((f64, f64), f64) = ((30.0, 85.0), 25.0);
/// The black zigzag's points, between y = 95 and y = 100 from x = 0 to x = 40 (mm).
pub const ZIGZAG: [(f64, f64); 9] =
    [(0.0, 95.0), (5.0, 100.0), (10.0, 95.0), (15.0, 100.0), (20.0, 95.0), (25.0, 100.0), (30.0, 95.0), (35.0, 100.0), (40.0, 95.0)];
/// The design's origin, on the blue column's top edge 10 mm in from its left end, and the stop position, 20 mm
/// right of the fill's top-right corner and 10 mm above it (mm).
pub const ORIGIN_AND_STOP_POSITION: [(f64, f64); 2] = [(10.0, 40.0), (60.0, -10.0)];
/// The turn the sheet is written at: a quarter turn right, clockwise as it is seen.
pub const TURN: Turn = Turn::RIGHT;
/// How far the red fill's clone turns, right, about the fill's middle (degrees).
pub const CLONE_TURN: f64 = 45.0;
/// Where the clone's middle lies (mm).
pub const CLONE_MIDDLE: (f64, f64) = (72.5, 15.0);
/// The angle of the clone's rows: the fill's, 0°, turned right with it, as a fill's angle counts it.
pub const CLONE_ANGLE: &str = "-45";
/// The clip path that cuts it, a U upside down whose legs reach x = 15 and x = 25 from the outside: it leaves
/// the zigzag from x = 0 to 15 and from 25 to 40 (mm).
pub const GAP: [(f64, f64); 8] = [(-2.0, 90.0), (42.0, 90.0), (42.0, 105.0), (25.0, 105.0), (25.0, 92.0), (15.0, 92.0), (15.0, 105.0), (-2.0, 105.0)];

/// Draws TS-16.
pub(super) fn drawing() -> Result<Drawing, SheetError> {
    let (red, blue) = (thread(RED)?, thread(BLUE)?);
    let mut d = Drawing::new("ts16");
    let [left, top, right, bottom] = FILL;
    let ring = super::designed::polygon(&[(left, top), (right, top), (right, bottom), (left, bottom)])?;
    d.fill("fill", vec![ring], &red, &[])?;
    d.positions((right, top), (left, bottom))?;
    d.satin("column", RAILS, &blue, &[])?;
    d.positions((20.0, 42.5), (30.0, 42.5))?;
    d.stop();
    let [origin, stop_position] = ORIGIN_AND_STOP_POSITION;
    d.origin_and_stop_position(origin, stop_position)?;
    d.turned(TURN);
    let [left, top, right, bottom] = SQUARE;
    d.fill("quarter", vec![polygon(&[(left, top), (right, top), (right, bottom), (left, bottom)])?], &thread(EMERALD_GREEN)?, &[])?;
    let (centre, radius) = CIRCLE;
    d.clip(vec![round(centre, radius)?]);
    d.polyline("zigzag", &ZIGZAG, &thread(BLACK)?, &[])?;
    d.clip(vec![polygon(&GAP)?]);
    // The clone: x' = cos·(x − cx) − sin·(y − cy) + mx, y' = sin·(x − cx) + cos·(y − cy) + my.
    let (sin, cos) = math::sin_cos(math::to_radians(CLONE_TURN));
    let [left, top, right, bottom] = FILL;
    let ((cx, cy), (mx, my)) = (((left + right) / 2.0, (top + bottom) / 2.0), CLONE_MIDDLE);
    d.clone_of("clone", "fill", [cos, sin, -sin, cos, mx - cos * cx + sin * cy, my - sin * cx - cos * cy], CLONE_ANGLE)?;
    Ok(d)
}
