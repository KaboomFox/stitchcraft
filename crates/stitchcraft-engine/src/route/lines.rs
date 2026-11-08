//! The strokes an auto-run route takes, read and cut into pieces (`docs/src/design/routing.md` › Auto-run,
//! steps 1 to 3), as Ink/Stitch's Auto-Route Running Stitch cuts them (read at `d59c9ab`).
//!
//! **What is routed.** Every element that sews as a stroke, whatever its stroke method: a running stitch,
//! a bean stitch, a manual stitch, a ripple or a zigzag. A satin column, a fill, an element digitized
//! automatically, one whose settings don't read and one of no length are left as they are, and `SC-I1002`
//! names each.
//!
//! **Lines.** Each subpath is a line: flattened within 0.1 CSS pixels, as Ink/Stitch flattens a stroke, or
//! a manual stitch's nodes, as it sews them, a point that repeats the one before left out.
//!
//! **Cuts**, with Add nodes at intersections ([`Options::break_up`]). A line is first cut where it crosses or
//! touches itself, into lines in order along it, as the geometry library Ink/Stitch uses nodes one. Then the
//! lines are compared 2 by 2 in order, a stroke's subpaths and their parts among them. A line is cut where it
//! crosses, touches or runs along another, and, where the 2 pass within [`NEAR`] of each other without
//! touching, at its point nearest the other, found as that library finds it (`normalize::near`). Without the
//! option, a line is cut nowhere.
//!
//! **Where lines meet.** A crossing is one point, which both lines are cut at, and a cut within a millionth
//! of a millimetre of a point seen before, a line's own or another cut, is that point. So lines that cross
//! meet at one place, and 3 lines through one point do too. Ink/Stitch cuts each line at the crossing's
//! projection onto it, and its graph joins 2 lines only where their cuts agree to 16 digits
//! (`DEV-ROUTE-007`). 2 lines that meet in several stretches, or in stretches and points, are cut at each,
//! where Ink/Stitch's tool cuts them at none, and a stretch 0.1 CSS pixels long or shorter between 2 cuts is
//! kept, where Ink/Stitch leaves it out (`DEV-ROUTE-008`).
//!
//! **Pieces.** Each line is cut at its cuts into stretches, and each stretch at its points into straight
//! pieces, each from a point to the next, in order along the line, as Ink/Stitch's graph links the points of
//! its stretches. A stroke's pieces come line by line, in the order of its subpaths.
//!
//! **A cap.** The lines are compared 2 by 2, n (n − 1) / 2 pairs for n lines, and 2 lines whose boxes lie
//! within [`NEAR`] of each other side by side. Beyond [`MOST_LINES`] lines, no line is cut by another, and
//! `SC-W1004` says so. Each pair of lines and each pair of sides compared charges the meter a unit.
//!
//! [`Options::break_up`]: crate::route::Options::break_up

use stitchcraft_core::exact::{Side, side};
use stitchcraft_core::units::MM_PER_SVG_PX;
use stitchcraft_core::{Code, Diagnostic, Exhausted, Fix, Meter, Point};
use stitchcraft_params::{Family, StitchType};

use crate::design::{DesignSettings, Element, Shape};
use crate::generate::{cut_line, stitch_type_of};
use crate::generators::passes::RepeatParams;
use crate::normalize::near::{self, Snap, boxes_meet, in_box};
use crate::normalize::satin::SAME_POINT;
use crate::route::read::left;

/// How near 2 lines pass for each to be cut where it comes nearest the other, in millimetres: 50 CSS
/// pixels, 13.2 mm, Ink/Stitch's. Lines exactly that far apart are cut.
pub const NEAR: f64 = 50.0 * MM_PER_SVG_PX;

/// The most lines an auto-run route cuts where they cross and pass near each other: beyond it, comparing
/// every pair would cost too much work, and no line is cut by another (`SC-W1004`).
pub const MOST_LINES: usize = 1_000;

/// A stroke an auto-run route takes, read and cut.
#[derive(Clone, Debug, PartialEq)]
pub(super) struct Stroke {
    /// Its place among the elements given.
    pub(super) element: usize,
    /// How it sews.
    pub(super) stitch_type: StitchType,
    /// How many times it sews along its path, as its settings read.
    pub(super) repeats: u32,
    /// Its pieces, each a straight stretch from a point to the next, in order along its lines, which come
    /// in the order of its subpaths.
    pub(super) pieces: Vec<[Point; 2]>,
}

/// A line of a stroke, and where it is cut.
#[derive(Clone, Debug, PartialEq)]
struct Line {
    /// The stroke it belongs to, by its place among those taken.
    stroke: usize,
    /// Its points, at least 2, none the same as the one before.
    points: Vec<Point>,
    /// How far along it each point lies.
    along: Vec<f64>,
    /// Its box: the least x and y of its points, then the greatest.
    bounds: [f64; 4],
    /// Where it is cut: how far along it, and the point.
    cuts: Vec<(f64, Point)>,
}

impl Line {
    /// The line of `stroke` through `points`, cut nowhere yet.
    fn new(stroke: usize, points: Vec<Point>) -> Line {
        let mut along = Vec::with_capacity(points.len());
        let mut length = 0.0;
        let mut bounds = [f64::INFINITY, f64::INFINITY, f64::NEG_INFINITY, f64::NEG_INFINITY];
        for (k, p) in points.iter().enumerate() {
            if let Some(before) = k.checked_sub(1).and_then(|before| points.get(before)) {
                length += before.distance(*p);
            }
            along.push(length);
            bounds = [bounds[0].min(p.x()), bounds[1].min(p.y()), bounds[2].max(p.x()), bounds[3].max(p.y())];
        }
        Line { stroke, points, along, bounds, cuts: Vec::new() }
    }

    /// Its sides, each with its place along it: from each point to the next.
    fn sides(&self) -> impl Iterator<Item = (usize, (Point, Point))> + '_ {
        self.points.windows(2).enumerate().filter_map(|(k, pair)| Some((k, (*pair.first()?, *pair.get(1)?))))
    }

    /// Cuts it at `p`, a point of its side `side`.
    fn cut(&mut self, side: usize, p: Point) {
        let (Some(&start), Some(&from), Some(&to)) = (self.points.get(side), self.along.get(side), self.along.get(side + 1)) else { return };
        let at = if self.points.get(side + 1) == Some(&p) { to } else { (from + start.distance(p)).min(to) };
        self.cuts.push((at, p));
    }

    /// The line cut at its cuts: the stretch from each cut, or its start, to the next, or its end, in order,
    /// each its points; a stretch whose points are all one is left out, and a cut at either end changes
    /// nothing.
    fn stretches(&self) -> Vec<Vec<Point>> {
        let (Some(&first), Some(&last), Some(&length)) = (self.points.first(), self.points.last(), self.along.last()) else { return Vec::new() };
        let mut cuts: Vec<(f64, Point)> = self.cuts.iter().copied().filter(|(at, _)| *at > 0.0 && *at < length).collect();
        cuts.sort_by(|a, b| a.0.total_cmp(&b.0));
        cuts.dedup_by(|later, earlier| later.0 == earlier.0);
        let bounds: Vec<(f64, Point)> = std::iter::once((0.0, first)).chain(cuts).chain([(length, last)]).collect();
        let mut stretches = Vec::with_capacity(bounds.len());
        let mut next = 0;
        for pair in bounds.windows(2) {
            let &[(from, start), (to, end)] = pair else { continue };
            let mut points = vec![start];
            while let Some((&at, &p)) = self.along.get(next).zip(self.points.get(next))
                && at < to
            {
                next += 1;
                if at > from && points.last() != Some(&p) {
                    points.push(p);
                }
            }
            if points.last() != Some(&end) {
                points.push(end);
            }
            if points.len() >= 2 {
                stretches.push(points);
            }
        }
        stretches
    }
}

/// The strokes of `elements` an auto-run route takes, in order, each read with the design's `settings`, and
/// with `break_up` cut where the module's docs say; what is said of the others goes to `said`. Reading and
/// cutting charge `meter`.
pub(super) fn strokes(
    elements: &[Element],
    settings: &DesignSettings,
    break_up: bool,
    said: &mut Vec<Diagnostic>,
    meter: &mut Meter,
) -> Result<Vec<Stroke>, Exhausted> {
    let mut strokes = Vec::new();
    let mut lines: Vec<Line> = Vec::new();
    let mut snap: Snap<Point> = Snap::new(SAME_POINT);
    for (index, element) in elements.iter().enumerate() {
        let (stitch_type, repeats, own) = match read(element, settings, meter)? {
            Ok(read) => read,
            Err(why) => {
                said.push(left(element, why));
                continue;
            }
        };
        let stroke = strokes.len();
        for points in own {
            for &p in &points {
                meter.charge(1)?;
                snap.snap(p, || p);
            }
            if break_up {
                lines.extend(parts(Line::new(stroke, points), &mut snap, meter)?.into_iter().map(|part| Line::new(stroke, part)));
            } else {
                lines.push(Line::new(stroke, points));
            }
        }
        strokes.push(Stroke { element: index, stitch_type, repeats, pieces: Vec::new() });
    }
    if break_up && lines.len() > MOST_LINES {
        said.push(too_many(lines.len()));
    } else if break_up {
        cut(&mut lines, &mut snap, meter)?;
    }
    for line in &lines {
        let Some(stroke) = strokes.get_mut(line.stroke) else { continue };
        for stretch in line.stretches() {
            for pair in stretch.windows(2) {
                meter.charge(1)?;
                let &[a, b] = pair else { continue };
                stroke.pieces.push([a, b]);
            }
        }
    }
    Ok(strokes)
}

/// What a stroke is read as.
type Read = (StitchType, u32, Vec<Vec<Point>>);

/// `element` read as an auto-run route takes it: its stitch type, its repeats and its lines; or why it is
/// left as it is, the end of a sentence.
fn read(element: &Element, settings: &DesignSettings, meter: &mut Meter) -> Result<Result<Read, &'static str>, Exhausted> {
    let Shape::Stroke { path, .. } = &element.shape else { return Ok(Err("it is a fill, which a route doesn't take")) };
    if element.automatic {
        return Ok(Err("it is digitized automatically, which chooses its own stitches"));
    }
    let Some(stitch_type) = stitch_type_of(element, settings) else { return Ok(Err("its settings don't read")) };
    if stitch_type.family() == Family::Satin {
        return Ok(Err("it is a satin column, which Auto-Route Running Stitch doesn't take"));
    }
    let Ok(passes) = RepeatParams::from_set_as(&element.params, stitch_type) else { return Ok(Err("its settings don't read")) };
    let line = cut_line(stitch_type);
    let mut lines = Vec::new();
    for subpath in &path.subpaths {
        let mut points = line(subpath, meter)?;
        points.dedup();
        if points.len() >= 2 {
            lines.push(points);
        }
    }
    if lines.is_empty() {
        return Ok(Err("it has no length"));
    }
    Ok(Ok((stitch_type, passes.params.repeats, lines)))
}

/// `line` cut where it crosses or touches itself, into lines in order along it, the points where it does
/// taken through `snap`. Each pair of its sides compared charges `meter` a unit.
fn parts(mut line: Line, snap: &mut Snap<Point>, meter: &mut Meter) -> Result<Vec<Vec<Point>>, Exhausted> {
    let sides: Vec<(usize, (Point, Point))> = line.sides().collect();
    for (i, &(k, first)) in sides.iter().enumerate() {
        for &(l, second) in sides.iter().skip(i + 1) {
            meter.charge(1)?;
            for p in meetings(first, second) {
                // Sides next to each other meet at the point between them, which is no cut.
                if l == k + 1 && p == first.1 {
                    continue;
                }
                let p = snap.snap(p, || p);
                line.cut(k, p);
                line.cut(l, p);
            }
        }
    }
    Ok(line.stretches())
}

/// Cuts `lines` where each crosses, touches or runs along another, and where 2 pass within [`NEAR`] of each
/// other, at the point of each nearest the other, the points taken through `snap`. Each pair of lines and
/// each pair of sides compared charges `meter` a unit.
fn cut(lines: &mut [Line], snap: &mut Snap<Point>, meter: &mut Meter) -> Result<(), Exhausted> {
    for i in 0..lines.len() {
        for j in (i + 1)..lines.len() {
            meter.charge(1)?;
            let (Some(first), Some(second)) = (lines.get(i), lines.get(j)) else { continue };
            if boxes_apart(first.bounds, second.bounds) > NEAR {
                continue;
            }
            let Some(cuts) = compared(first, second, meter)? else { continue };
            for (k, l, p, q) in cuts {
                let (p, q) = (snap.snap(p, || p), snap.snap(q, || q));
                if let Some(first) = lines.get_mut(i) {
                    first.cut(k, p);
                }
                if let Some(second) = lines.get_mut(j) {
                    second.cut(l, q);
                }
            }
        }
    }
    Ok(())
}

/// Where `first` and `second` are cut as the module's docs say, each cut its side of `first`, its side of
/// `second`, and their points on each; `None` when they lie farther apart than [`NEAR`]. Where they meet,
/// the points where they do; otherwise their nearest points, measured from `first`. Each pair of sides
/// compared charges `meter` a unit.
fn compared(first: &Line, second: &Line, meter: &mut Meter) -> Result<Option<Vec<(usize, usize, Point, Point)>>, Exhausted> {
    let mut distance = f64::INFINITY;
    let mut nearest = None;
    let mut met = Vec::new();
    for (k, s) in first.sides() {
        for (l, t) in second.sides() {
            meter.charge(1)?;
            let apart = near::apart(s.0, s.1, t.0, t.1);
            if apart < distance {
                distance = apart;
                let (p, q, _) = near::between_segments(s, t);
                nearest = Some((k, l, p, q));
            }
            met.extend(meetings(s, t).into_iter().map(|p| (k, l, p, p)));
        }
    }
    if distance > NEAR {
        return Ok(None);
    }
    Ok(Some(if met.is_empty() { nearest.into_iter().collect() } else { met }))
}

/// How far apart the boxes `a` and `b` are, each the least x and y, then the greatest: 0 where they meet.
fn boxes_apart(a: [f64; 4], b: [f64; 4]) -> f64 {
    let dx = (b[0] - a[2]).max(a[0] - b[2]).max(0.0);
    let dy = (b[1] - a[3]).max(a[1] - b[3]).max(0.0);
    (dx * dx + dy * dy).sqrt()
}

/// Where the sides from `a` to `b` and from `c` to `d` meet, each point once: where they cross; the ends of
/// one that lie on the other, where they touch; or the ends of the stretch along which they lie together.
/// Whether a point lies on a side's line is decided exactly ([`side`]).
fn meetings((a, b): (Point, Point), (c, d): (Point, Point)) -> Vec<Point> {
    if !boxes_meet(a, b, c, d) {
        return Vec::new();
    }
    let (sc, sd, sa, sb) = (side(a, b, c), side(a, b, d), side(c, d, a), side(c, d, b));
    if (sc == sd && sc != Side::On) || (sa == sb && sa != Side::On) {
        return Vec::new();
    }
    let mut met: Vec<Point> = Vec::with_capacity(2);
    if [sc, sd, sa, sb].contains(&Side::On) {
        // An end on the other side's line: the sides meet only at such ends, unless they lie on one line.
        for (p, on, (from, to)) in [(c, sc, (a, b)), (d, sd, (a, b)), (a, sa, (c, d)), (b, sb, (c, d))] {
            if on == Side::On && in_box(from, to, p) && !met.contains(&p) {
                met.push(p);
            }
        }
        return met;
    }
    // A crossing: each side's ends lie either side of the other's line, which are not parallel.
    let across = (b.x() - a.x()) * (d.y() - c.y()) - (b.y() - a.y()) * (d.x() - c.x());
    let t = ((c.x() - a.x()) * (d.y() - c.y()) - (c.y() - a.y()) * (d.x() - c.x())) / across;
    met.extend(Point::new(a.x() + t * (b.x() - a.x()), a.y() + t * (b.y() - a.y())).ok());
    met
}

/// `SC-W1004`: there are `count` lines, more than [`MOST_LINES`], so none is cut by another.
fn too_many(count: usize) -> Diagnostic {
    let message = format!(
        "There are {count} lines to route, more than the {MOST_LINES} that are cut where they cross and pass near each other, so they are \
         routed as drawn, each cut only where it crosses itself."
    );
    let hint = "Route fewer running stitches at a time, or turn Add nodes at intersections off and draw nodes where the lines meet.";
    Diagnostic::new(Code::RouteTooManyLines, message).with_fix(Fix::Hint(hint.to_string()))
}

#[cfg(test)]
mod tests;
