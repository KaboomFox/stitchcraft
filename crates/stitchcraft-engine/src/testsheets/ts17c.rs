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

use super::designed::{Drawing, KAPPA, Start};
use super::sketch::SheetError;
use super::{BLUE, RED, thread};
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
    Ok(d)
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
    let hook = Subpath { start: point(26.0, 82.0)?, segments: vec![Segment::Cubic(point(26.0, 82.0 - k)?, point(36.0 - k, 72.0)?, point(36.0, 72.0)?)], closed: false };
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
