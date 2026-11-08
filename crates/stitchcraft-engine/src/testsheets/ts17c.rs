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
//!
//! M10.13's part, below the letters: a network of 6 running stitches in red ([`LINES`]), each sewn there and
//! back with bean stitch. On the left it is sewn as drawn, line by line. On the right, [`BESIDE`] mm over,
//! the same lines are sewn as `stitchcraft_engine::route` routes them with Auto-Route Running Stitch, its
//! Trim jump stitches on (`REQ-ROUTE-009` to `REQ-ROUTE-012`). A rail crosses a post, and a brace and a
//! hook, a curve, stand on them, so the route sews a loop and branches, the branches under the others
//! first. A line that crosses itself lies apart, and a tick lies 0.5 mm off the rail, so the route jumps to
//! both, and trims after the long jump only. The lines meet where both are drawn through a node, as
//! Ink/Stitch's help says to draw lines for routing: its graph joins 2 lines it cuts elsewhere only where
//! its cuts agree to the last digits (`DEV-ROUTE-007`). The other lines pass within 13.2 mm of each other,
//! so the route cuts them at their nearest points. The differential routes the lines on the left with
//! Ink/Stitch's tool too (`REQ-ROUTE-013`).
//!
//! M10.8's part, below the network: 3 ripple stitches in green ([`ripples`]), one of each kind
//! (`REQ-RIP-001`). On the left, a rainbow: a column drawn with rails and rungs between 2 arches, whose 7 lines
//! run back and forth from the outer arch to the inner one. In the middle, a wave whose 8 copies shrink to a
//! target point above it, as Ink/Stitch's target point command places it, each meeting the next at a point.
//! On the right, a circle that spirals in to its middle. The differential sews each in Ink/Stitch too
//! (`REQ-RIP-011`).

use super::designed::{Drawing, KAPPA, Start};
use super::sketch::SheetError;
use super::{BLUE, EMERALD_GREEN, RED, thread};
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

/// How far right of the network as drawn its routed copy lies (mm).
pub const BESIDE: f64 = 44.0;

/// The network's lines, by name, in the order they are drawn.
pub const LINES: [&str; 6] = ["rail", "post", "brace", "hook", "loop", "tick"];

/// Each line's settings: a running stitch 3 mm long, sewn there and back, and each stitch 3 times over.
const LINE_SETTINGS: [(&str, &str); 3] = [("running_stitch_length_mm", "3"), ("repeats", "2"), ("bean_stitch_repeats", "1")];

/// The ripple stitches, by name, in the order they are drawn.
pub const RIPPLES: [&str; 3] = ["column", "towards", "spiral"];

/// The setting that sews a stroke as a ripple stitch.
const RIPPLE: (&str, &str) = ("stroke_method", "ripple_stitch");

/// Draws TS-17C.
pub(super) fn drawing() -> Result<Drawing, SheetError> {
    let blue = thread(BLUE)?;
    let mut d = Drawing::new("ts17c");
    letters(&mut d, "drawn", 0.0, &blue)?;
    let mut routed = Drawing::new("ts17c");
    letters(&mut routed, "routed", BELOW, &blue)?;
    d.routed(&routed, &Route { tool: Tool::Satins, options: Tool::Satins.defaults(), start: None, end: None })?;
    let red = thread(RED)?;
    network(&mut d, "lines-drawn", 0.0, &red)?;
    let mut lines = Drawing::new("ts17c");
    network(&mut lines, "lines-routed", BESIDE, &red)?;
    let trimmed = Options { trim: true, ..Tool::Runs.defaults() };
    d.routed(&lines, &Route { tool: Tool::Runs, options: trimmed, start: None, end: None })?;
    ripples(&mut d, &thread(EMERALD_GREEN)?)?;
    Ok(d)
}

/// The ripple stitches below the network, sewn with `thread`: a rainbow between 2 arches, a wave that shrinks
/// to a point above it and a circle that spirals in to its middle.
pub(super) fn ripples(d: &mut Drawing, thread: &Thread) -> Result<(), SheetError> {
    let point = |x: f64, y: f64| Point::new(x, y);
    // An arch: half a circle of radius `r` about (14, 112), from its left end over the top.
    let arch = |r: f64| -> Result<Subpath, SheetError> {
        let (cx, cy, k) = (14.0, 112.0, KAPPA * r);
        let segments = vec![
            Segment::Cubic(point(cx - r, cy - k)?, point(cx - k, cy - r)?, point(cx, cy - r)?),
            Segment::Cubic(point(cx + k, cy - r)?, point(cx + r, cy - k)?, point(cx + r, cy)?),
        ];
        Ok(Subpath { start: point(cx - r, cy)?, segments, closed: false })
    };
    // The rainbow: arches of radius 14 and 7, the rails, and a rung across their tops reaching 0.4 mm past them.
    let rung = Subpath { start: point(14.0, 97.6)?, segments: vec![Segment::Line(point(14.0, 105.4)?)], closed: false };
    d.strokes("ripple:column", vec![arch(14.0)?, arch(7.0)?, rung], thread, &[RIPPLE, ("line_count", "7")])?;
    // The wave, 24 mm long, and its target point 20 mm above its middle.
    let curve = Segment::Cubic(point(40.0, 108.0)?, point(52.0, 124.0)?, point(58.0, 116.0)?);
    let wave = Subpath { start: point(34.0, 116.0)?, segments: vec![curve], closed: false };
    d.along("ripple:towards", wave, thread, &[RIPPLE, ("line_count", "8"), ("join_style", "1")])?;
    d.target((46.0, 96.0))?;
    // The circle, 22 mm across, from its top.
    d.circle("ripple:spiral", (71.0, 107.0), 11.0, Start::Top, thread, &[RIPPLE, ("line_count", "6")])
}

/// The network of lines, named after `half`, `right` mm right of where it is drawn first, sewn with
/// `thread`. A rail along the bottom, through nodes where the others meet it; a post that crosses it, through
/// a node where the brace leaves it; the brace from the post down to the rail, and a hook, a quarter circle,
/// up from the rail to the right. Apart from them, a line that crosses itself, and a tick 0.5 mm below the
/// rail.
pub(super) fn network(d: &mut Drawing, half: &str, right: f64, thread: &Thread) -> Result<(), SheetError> {
    let at = |points: &[(f64, f64)]| points.iter().map(|&(x, y)| (x + right, y)).collect::<Vec<_>>();
    let name = |line: &str| format!("{half}:{line}");
    d.polyline(&name("rail"), &at(&[(0.0, 82.0), (8.0, 82.0), (20.0, 82.0), (26.0, 82.0), (38.0, 82.0)]), thread, &LINE_SETTINGS)?;
    d.polyline(&name("post"), &at(&[(8.0, 62.0), (8.0, 68.0), (8.0, 82.0), (8.0, 85.0)]), thread, &LINE_SETTINGS)?;
    d.polyline(&name("brace"), &at(&[(8.0, 68.0), (20.0, 82.0)]), thread, &LINE_SETTINGS)?;
    // A quarter circle of radius 10 about (36, 82), from the rail up to (36, 72).
    let k = 10.0 * KAPPA;
    let point = |x: f64, y: f64| Point::new(x + right, y);
    let hook = Subpath {
        start: point(26.0, 82.0)?,
        segments: vec![Segment::Cubic(point(26.0, 82.0 - k)?, point(36.0 - k, 72.0)?, point(36.0, 72.0)?)],
        closed: false,
    };
    d.along(&name("hook"), hook, thread, &LINE_SETTINGS)?;
    d.polyline(&name("loop"), &at(&[(14.0, 61.0), (24.0, 67.0), (24.0, 61.0), (17.0, 66.0)]), thread, &LINE_SETTINGS)?;
    d.polyline(&name("tick"), &at(&[(34.0, 82.5), (35.0, 85.0)]), thread, &LINE_SETTINGS)
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::design::Element;

    /// The columns of the letters `letters` draws `down` mm below the top, named after `half`.
    fn columns(half: &str, down: f64) -> Vec<Element> {
        let mut d = Drawing::new("ts17c");
        letters(&mut d, half, down, &thread(BLUE).unwrap()).unwrap();
        d.design().unwrap().elements().to_vec()
    }

    /// The subpaths of `element` as an SVG path draws them, each number to a thousandth of a millimetre: `M`
    /// and its start, `L` and a line's end, `C` and a curve's 2 control points and its end, and `Z` when it
    /// closes.
    fn svg(element: &Element) -> Vec<String> {
        let at = |p: Point| format!("{},{}", (p.x() * 1000.0).round() / 1000.0 + 0.0, (p.y() * 1000.0).round() / 1000.0 + 0.0);
        let segment = |piece: &Segment| match piece {
            Segment::Cubic(a, b, end) => format!(" C{} {} {}", at(*a), at(*b), at(*end)),
            _ => format!(" L{}", at(piece.end())),
        };
        let subpath =
            |s: &Subpath| format!("M{}{}{}", at(s.start), s.segments.iter().map(segment).collect::<String>(), if s.closed { " Z" } else { "" });
        element.shape.path().subpaths.iter().map(subpath).collect()
    }

    #[test]
    fn ts17c_draws_the_word_route_in_satin_columns_3_mm_wide_with_rails_and_rungs() {
        // Each column, in the order it is drawn: its 2 rails, then its rungs.
        let drawn: [(&str, &[&str]); 11] = [
            ("r-stem", &["M0,0 L0,20", "M3,0 L3,20", "M-0.4,15 L3.4,15"]),
            // From the stem's middle along to x = 7, round half circles of 5.75 and 2.75 mm about (7, 5.75), each
            // quarter a curve whose control points lie 0.552 of the radius from its ends, and back.
            (
                "r-bowl",
                &[
                    "M1.5,0 L7,0 C10.176,0 12.75,2.574 12.75,5.75 C12.75,8.926 10.176,11.5 7,11.5 L1.5,11.5",
                    "M1.5,3 L7,3 C8.519,3 9.75,4.231 9.75,5.75 C9.75,7.269 8.519,8.5 7,8.5 L1.5,8.5",
                    "M4.25,-0.4 L4.25,3.4",
                    "M9.35,5.75 L13.15,5.75",
                    "M4.25,8.1 L4.25,11.9",
                ],
            ),
            // 1.5 mm either side of the line from (6.5, 10.5) to (11.5, 20), and a rung across its middle reaching
            // 0.4 mm past them, all square to the line.
            ("r-leg", &["M5.173,11.199 L10.173,20.699", "M7.827,9.801 L12.827,19.301", "M7.319,16.135 L10.681,14.365"]),
            (
                "o",
                &[
                    "M26,0 C31.523,0 36,4.477 36,10 C36,15.523 31.523,20 26,20 C20.477,20 16,15.523 16,10 C16,4.477 20.477,0 26,0 Z",
                    "M26,3 C29.866,3 33,6.134 33,10 C33,13.866 29.866,17 26,17 C22.134,17 19,13.866 19,10 C19,6.134 22.134,3 26,3 Z",
                ],
            ),
            // Down from the top at x = 39 and 42, round half circles of 6 and 3 mm about (45, 14), and up again at
            // x = 51 and 48, the mirror image.
            (
                "u",
                &[
                    "M39,0 L39,14 C39,17.314 41.686,20 45,20 C48.314,20 51,17.314 51,14 L51,0",
                    "M42,0 L42,14 C42,15.657 43.343,17 45,17 C46.657,17 48,15.657 48,14 L48,0",
                    "M38.6,7 L42.4,7",
                    "M45,16.6 L45,20.4",
                    "M47.6,7 L51.4,7",
                ],
            ),
            ("t-bar", &["M54,0 L68,0", "M54,3 L68,3", "M61,-0.4 L61,3.4"]),
            ("t-stem", &["M59.5,1.5 L59.5,20", "M62.5,1.5 L62.5,20", "M59.1,11.15 L62.9,11.15"]),
            ("e-stem", &["M71,0 L71,20", "M74,0 L74,20", "M70.6,10 L74.4,10"]),
            ("e-top", &["M72.5,0 L82,0", "M72.5,3 L82,3", "M77.5,-0.4 L77.5,3.4"]),
            ("e-middle", &["M72.5,8.5 L80,8.5", "M72.5,11.5 L80,11.5", "M77.5,8.1 L77.5,11.9"]),
            ("e-bottom", &["M72.5,17 L82,17", "M72.5,20 L82,20", "M77.5,16.6 L77.5,20.4"]),
        ];
        assert_eq!(drawn.map(|(name, _)| name), LETTERS);
        let columns = columns("drawn", 0.0);
        let found: Vec<(String, Vec<String>)> = columns.iter().map(|column| (column.id.to_string(), svg(column))).collect();
        let want: Vec<(String, Vec<String>)> =
            drawn.iter().map(|(name, subpaths)| (format!("ts17c:drawn:{name}"), subpaths.iter().map(ToString::to_string).collect())).collect();
        assert_eq!(found, want);
        // Each a satin column with a centre walk underlay, in blue.
        let blue = thread(BLUE).unwrap();
        let settings = [("center_walk_underlay", "true"), ("satin_column", "true")];
        assert!(columns.iter().all(|c| c.params.iter().eq(settings) && c.thread == blue && c.name.is_none()), "{columns:?}");
    }

    /// The lines `network` draws `right` mm right of where they are drawn on the left, named after `half`.
    fn lines(half: &str, right: f64) -> Vec<Element> {
        let mut d = Drawing::new("ts17c");
        network(&mut d, half, right, &thread(RED).unwrap()).unwrap();
        d.design().unwrap().elements().to_vec()
    }

    /// TS-17C's elements whose ids start with `prefix`.
    fn sheet_elements(prefix: &str) -> Vec<Element> {
        let design = super::super::find("TS-17C").unwrap().design().unwrap().unwrap();
        design.elements().iter().filter(|element| element.id.as_str().starts_with(prefix)).cloned().collect()
    }

    #[test]
    fn ts17c_s_network_is_6_bean_stitched_lines_that_meet_at_their_nodes() {
        let drawn: [(&str, &str); 6] = [
            ("rail", "M0,82 L8,82 L20,82 L26,82 L38,82"),
            ("post", "M8,62 L8,68 L8,82 L8,85"),
            ("brace", "M8,68 L20,82"),
            // A quarter circle of radius 10 about (36, 82), its control points 0.552 of the radius from its ends.
            ("hook", "M26,82 C26,76.477 30.477,72 36,72"),
            ("loop", "M14,61 L24,67 L24,61 L17,66"),
            ("tick", "M34,82.5 L35,85"),
        ];
        assert_eq!(drawn.map(|(name, _)| name), LINES);
        let lines = lines("lines-drawn", 0.0);
        let found: Vec<(String, Vec<String>)> = lines.iter().map(|line| (line.id.to_string(), svg(line))).collect();
        let want: Vec<(String, Vec<String>)> =
            drawn.iter().map(|(name, path)| (format!("ts17c:lines-drawn:{name}"), vec![path.to_string()])).collect();
        assert_eq!(found, want);
        // Each a running stitch 3 mm long, sewn there and back and bean-stitched, in red.
        let red = thread(RED).unwrap();
        assert!(lines.iter().all(|line| LINE_SETTINGS.iter().all(|&(key, value)| line.params.get(key) == Some(value)) && line.thread == red));
        // The sheet sews them on the left as drawn, below the letters.
        assert_eq!(sheet_elements("ts17c:lines-drawn:"), lines);
    }

    #[test]
    fn ts17c_s_network_on_the_right_is_routed_as_ink_stitch_routes_it() {
        // As Ink/Stitch's Auto-Route Running Stitch routes the lines on the left with Trim jump stitches on, run
        // as a black box at d59c9ab: 18 elements, 6 of them runs under the others, and a trim after the run up
        // the post, before the jump to the line that crosses itself, and after that line, before the jump back.
        let routed = sheet_elements("ts17c:lines-routed:");
        let sewn: Vec<(String, bool)> =
            routed.iter().map(|piece| (piece.name.clone().unwrap_or_default(), piece.params.get("trim_after") == Some("true"))).collect();
        let under = [2, 4, 7, 10, 13, 16];
        let name = |n: usize| if under.contains(&n) { format!("AutoRun Underpath {n}") } else { format!("AutoRun {n}") };
        assert_eq!(sewn, (1..=18).map(|n| (name(n), n == 2 || n == 5)).collect::<Vec<_>>());
        // 44 mm right of the lines as drawn, from the rail's left end to its right end.
        let points: Vec<Point> = routed.iter().flat_map(|piece| piece.shape.path().points().collect::<Vec<_>>()).collect();
        let at = |x: f64, y: f64| Point::new(x + BESIDE, y).unwrap();
        assert_eq!((points.first(), points.last()), (Some(&at(0.0, 82.0)), Some(&at(38.0, 82.0))));
        let (xs, ys): (Vec<f64>, Vec<f64>) = points.iter().map(|q| (q.x(), q.y())).unzip();
        let span = |vs: &[f64]| (vs.iter().copied().fold(f64::INFINITY, f64::min), vs.iter().copied().fold(f64::NEG_INFINITY, f64::max));
        assert_eq!((span(&xs), span(&ys)), ((BESIDE, BESIDE + 38.0), (61.0, 85.0)));
    }

    #[test]
    fn ts17c_s_ripples_are_a_rainbow_a_wave_towards_its_target_and_a_spiral() {
        let drawn: [(&str, &[&str]); 3] = [
            // Arches of radius 14 and 7 about (14, 112), each quarter a curve whose control points lie 0.552 of the
            // radius from its ends, and a rung across their tops.
            (
                "column",
                &[
                    "M0,112 C0,104.268 6.268,98 14,98 C21.732,98 28,104.268 28,112",
                    "M7,112 C7,108.134 10.134,105 14,105 C17.866,105 21,108.134 21,112",
                    "M14,97.6 L14,105.4",
                ],
            ),
            ("towards", &["M34,116 C40,108 52,124 58,116"]),
            // A circle of radius 11 about (71, 107), clockwise from its top.
            (
                "spiral",
                &["M71,96 C77.075,96 82,100.925 82,107 C82,113.075 77.075,118 71,118 C64.925,118 60,113.075 60,107 C60,100.925 64.925,96 71,96 Z"],
            ),
        ];
        assert_eq!(drawn.map(|(name, _)| name), RIPPLES);
        let ripples = sheet_elements("ts17c:ripple:");
        let found: Vec<(String, Vec<String>)> = ripples.iter().map(|ripple| (ripple.id.to_string(), svg(ripple))).collect();
        let want: Vec<(String, Vec<String>)> =
            drawn.iter().map(|(name, subpaths)| (format!("ts17c:ripple:{name}"), subpaths.iter().map(ToString::to_string).collect())).collect();
        assert_eq!(found, want);
        // Each a ripple stitch in green, with its own count; the wave's lines meet at points, and it aims 20 mm
        // above its middle.
        let settings: [&[(&str, &str)]; 3] =
            [&[("line_count", "7"), RIPPLE], &[("join_style", "1"), ("line_count", "8"), RIPPLE], &[("line_count", "6"), RIPPLE]];
        assert!(ripples.iter().zip(settings).all(|(ripple, settings)| ripple.params.iter().eq(settings.iter().copied())), "{ripples:?}");
        let green = thread(EMERALD_GREEN).unwrap();
        assert!(ripples.iter().all(|ripple| ripple.thread == green));
        let targets: Vec<Option<Point>> = ripples.iter().map(|ripple| ripple.hints.target).collect();
        assert_eq!(targets, [None, Some(Point::new(46.0, 96.0).unwrap()), None]);
        // Below the network, whose lowest point lies at y = 85, and last on the sheet.
        let ys: Vec<f64> = ripples.iter().flat_map(|ripple| ripple.shape.path().points().map(|q| q.y()).collect::<Vec<_>>()).collect();
        assert!(ys.iter().all(|&y| y > 85.0 + 10.0), "{ys:?}");
        let design = super::super::find("TS-17C").unwrap().design().unwrap().unwrap();
        assert_eq!(design.elements()[design.elements().len() - RIPPLES.len()..], ripples);
    }

    #[test]
    fn ts17c_s_routed_letters_are_the_drawn_ones_30_mm_lower() {
        let (drawn, routed) = (columns("drawn", 0.0), columns("routed", BELOW));
        assert_eq!(routed.iter().map(|column| column.id.to_string()).collect::<Vec<_>>(), LETTERS.map(|name| format!("ts17c:routed:{name}")));
        for (above, below) in drawn.iter().zip(&routed) {
            let points = |column: &Element| column.shape.path().points().collect::<Vec<_>>();
            let (above_points, below_points) = (points(above), points(below));
            assert_eq!(above_points.len(), below_points.len(), "{}", below.id);
            let lower = |(p, q): (&Point, &Point)| (q.x() - p.x()).abs() < 1e-9 && (q.y() - p.y() - BELOW).abs() < 1e-9;
            assert!(above_points.iter().zip(&below_points).all(lower), "{}: {below_points:?}", below.id);
            assert_eq!((above.params.clone(), &above.thread), (below.params.clone(), &below.thread));
        }
    }

    #[test]
    fn ts17c_s_routed_half_trims_after_each_letter_and_nowhere_else() {
        // The letters as drawn, then the pieces of the route of those below, each named after its column. The
        // route sews each letter in one go, and the jump to the next letter, more than 1 mm long, trims.
        let design = super::super::find("TS-17C").unwrap().design().unwrap().unwrap();
        let (drawn, rest) = design.elements().split_at(LETTERS.len());
        assert_eq!(drawn, columns("drawn", 0.0).as_slice());
        let pieces: Vec<&Element> = rest.iter().take_while(|piece| piece.id.as_str().starts_with("ts17c:routed:")).collect();
        let letter = |piece: &Element| piece.id.as_str().strip_prefix("ts17c:routed:").and_then(|name| name.chars().next());
        let sewn: Vec<(Option<char>, bool)> = pieces.iter().map(|piece| (letter(piece), piece.params.get("trim_after") == Some("true"))).collect();
        for (k, &(of, trims)) in sewn.iter().enumerate() {
            assert_eq!(trims, sewn.get(k + 1).is_none_or(|next| next.0 != of), "piece {k} of {sewn:?}");
        }
        let mut order: Vec<Option<char>> = sewn.iter().map(|piece| piece.0).collect();
        order.dedup();
        assert_eq!(order, ['r', 'o', 'u', 't', 'e'].map(Some));
    }
}
