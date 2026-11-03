//! TS-17C — ripples and routing (M10): the stitch types and tools of M10 that sew along lines, each step
//! drawing its part.
//!
//! M10.4's part: the word ROUTE in satin letters 20 mm tall, their strokes satin columns 3 mm wide drawn
//! with rails and rungs, each with a centre walk underlay ([`LETTERS`]). Above, the letters are sewn as
//! drawn, column by column in paint order. Below, the same letters, 30 mm lower, are sewn as
//! `stitchcraft_engine::route` routes them with Auto-Route Satin Columns' options at their defaults
//! (`REQ-ROUTE-001` to `REQ-ROUTE-007`): cut into pieces, run under the satins that cover the runs, and
//! trimmed where a jump between letters is long. The R, the U and the bowl are curved, the O is a ring of 2
//! circles, and the T and the E have strokes that branch, so the route cuts columns and runs under them.
//! The differential routes the letters above with Ink/Stitch's tool too, and compares what both sew
//! (`REQ-ROUTE-008`).

use super::designed::{Drawing, KAPPA, Start};
use super::sketch::SheetError;
use super::{BLUE, thread};
use crate::design::{Segment, Subpath};
use crate::route::{Options, Route, Tool};
use stitchcraft_core::Point;
use stitchcraft_plan::Thread;

/// How far the routed letters lie below the letters as drawn (mm).
pub const BELOW: f64 = 30.0;

/// The letters' columns, by name, in the order they are drawn: the word ROUTE.
pub const LETTERS: [&str; 11] = ["r-stem", "r-bowl", "r-leg", "o", "u", "t-bar", "t-stem", "e-stem", "e-top", "e-middle", "e-bottom"];

/// Each column's settings: a centre walk underlay, as lettering's columns have.
const SETTINGS: [(&str, &str); 1] = [("center_walk_underlay", "true")];

/// Draws TS-17C.
pub(super) fn drawing() -> Result<Drawing, SheetError> {
    let blue = thread(BLUE)?;
    let mut d = Drawing::new("ts17c");
    letters(&mut d, "drawn", 0.0, &blue)?;
    let mut routed = Drawing::new("ts17c");
    letters(&mut routed, "routed", BELOW, &blue)?;
    d.routed(&routed, &Route { tool: Tool::Satins, options: Options::default(), start: None, end: None })?;
    Ok(d)
}

/// The letters ROUTE, their columns named after `half`, `down` mm below the top, sewn with `thread`.
pub(super) fn letters(d: &mut Drawing, half: &str, down: f64, thread: &Thread) -> Result<(), SheetError> {
    let mut put = Put { d, half, down, thread };
    // R: a stem, a bowl from the stem's middle round to its waist, and a leg from the bowl down to the right.
    put.column("r-stem", [put.line((0.0, 0.0), (0.0, 20.0))?, put.line((3.0, 0.0), (3.0, 20.0))?], &[[(-0.4, 15.0), (3.4, 15.0)]])?;
    let bowl_rungs = [[(4.25, -0.4), (4.25, 3.4)], [(9.35, 5.75), (13.15, 5.75)], [(4.25, 8.1), (4.25, 11.9)]];
    put.column("r-bowl", [put.bowl(5.75, 0.0)?, put.bowl(2.75, 3.0)?], &bowl_rungs)?;
    let [a, b, rung] = across((6.5, 10.5), (11.5, 20.0));
    put.column("r-leg", [put.line(a[0], a[1])?, put.line(b[0], b[1])?], &[rung])?;
    // O: a ring of 2 circles 20 and 14 mm across, from their tops, their nodes pairing up.
    put.d.ring(&format!("{half}:o"), ((26.0, 10.0 + down), Start::Top), (10.0, 7.0), thread, &SETTINGS)?;
    // U: 2 stems joined by a half circle.
    let u_rungs = [[(38.6, 7.0), (42.4, 7.0)], [(45.0, 16.6), (45.0, 20.4)], [(47.6, 7.0), (51.4, 7.0)]];
    put.column("u", [put.u(39.0, 6.0)?, put.u(42.0, 3.0)?], &u_rungs)?;
    // T: a bar, and a stem from inside it.
    put.column("t-bar", [put.line((54.0, 0.0), (68.0, 0.0))?, put.line((54.0, 3.0), (68.0, 3.0))?], &[[(61.0, -0.4), (61.0, 3.4)]])?;
    put.column("t-stem", [put.line((59.5, 1.5), (59.5, 20.0))?, put.line((62.5, 1.5), (62.5, 20.0))?], &[[(59.1, 11.15), (62.9, 11.15)]])?;
    // E: a stem, and 3 arms from inside it.
    put.column("e-stem", [put.line((71.0, 0.0), (71.0, 20.0))?, put.line((74.0, 0.0), (74.0, 20.0))?], &[[(70.6, 10.0), (74.4, 10.0)]])?;
    for (name, top, end) in [("e-top", 0.0, 82.0), ("e-middle", 8.5, 80.0), ("e-bottom", 17.0, 82.0)] {
        let rails = [put.line((72.5, top), (end, top))?, put.line((72.5, top + 3.0), (end, top + 3.0))?];
        put.column(name, rails, &[[(77.5, top - 0.4), (77.5, top + 3.4)]])?;
    }
    Ok(())
}

/// Where the letters are drawn: into `d`, named after `half`, `down` mm below the top, with `thread`.
struct Put<'a> {
    d: &'a mut Drawing,
    half: &'a str,
    down: f64,
    thread: &'a Thread,
}

impl Put<'_> {
    /// The point `(x, y)` of a letter, moved down.
    fn at(&self, (x, y): (f64, f64)) -> Result<Point, SheetError> {
        Ok(Point::new(x, y + self.down)?)
    }

    /// The straight line from `from` to `to`.
    fn line(&self, from: (f64, f64), to: (f64, f64)) -> Result<Subpath, SheetError> {
        Ok(Subpath { start: self.at(from)?, segments: vec![Segment::Line(self.at(to)?)], closed: false })
    }

    /// The column `name` between `rails`, with a rung across each pair of `rungs`.
    fn column(&mut self, name: &str, rails: [Subpath; 2], rungs: &[[(f64, f64); 2]]) -> Result<(), SheetError> {
        let mut subpaths: Vec<Subpath> = rails.into();
        for &[a, b] in rungs {
            subpaths.push(self.line(a, b)?);
        }
        self.d.satin_drawn(&format!("{}:{name}", self.half), subpaths, self.thread, &SETTINGS)
    }

    /// A rail of the R's bowl: from the stem's middle at `top` along to x = 7, round a half circle of
    /// `radius` about (7, 5.75), and back to the stem's middle.
    fn bowl(&self, radius: f64, top: f64) -> Result<Subpath, SheetError> {
        let ((cx, cy), k) = ((7.0, 5.75), KAPPA * radius);
        let segments = vec![
            Segment::Line(self.at((cx, cy - radius))?),
            Segment::Cubic(self.at((cx + k, cy - radius))?, self.at((cx + radius, cy - k))?, self.at((cx + radius, cy))?),
            Segment::Cubic(self.at((cx + radius, cy + k))?, self.at((cx + k, cy + radius))?, self.at((cx, cy + radius))?),
            Segment::Line(self.at((1.5, cy + radius))?),
        ];
        Ok(Subpath { start: self.at((1.5, top))?, segments, closed: false })
    }

    /// A rail of the U: down from the top at `left`, round a half circle of `radius` about (45, 14), and up
    /// to the top again.
    fn u(&self, left: f64, radius: f64) -> Result<Subpath, SheetError> {
        let ((cx, cy), k) = ((45.0, 14.0), KAPPA * radius);
        let right = 2.0 * cx - left;
        let segments = vec![
            Segment::Line(self.at((left, cy))?),
            Segment::Cubic(self.at((cx - radius, cy + k))?, self.at((cx - k, cy + radius))?, self.at((cx, cy + radius))?),
            Segment::Cubic(self.at((cx + k, cy + radius))?, self.at((cx + radius, cy + k))?, self.at((right, cy))?),
            Segment::Line(self.at((right, 0.0))?),
        ];
        Ok(Subpath { start: self.at((left, 0.0))?, segments, closed: false })
    }
}

/// The rails of a column 3 mm wide along the straight line from `from` to `to`, and a rung across its
/// middle reaching 0.4 mm past them.
fn across(from: (f64, f64), to: (f64, f64)) -> [[(f64, f64); 2]; 3] {
    let (dx, dy) = (to.0 - from.0, to.1 - from.1);
    let length = (dx * dx + dy * dy).sqrt();
    let (nx, ny) = (-dy / length, dx / length);
    let side = |by: f64| [(from.0 + nx * by, from.1 + ny * by), (to.0 + nx * by, to.1 + ny * by)];
    let middle = ((from.0 + to.0) / 2.0, (from.1 + to.1) / 2.0);
    [side(1.5), side(-1.5), [(middle.0 + nx * 1.9, middle.1 + ny * 1.9), (middle.0 - nx * 1.9, middle.1 - ny * 1.9)]]
}
