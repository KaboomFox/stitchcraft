//! A satin column's ends stay square to it (`REQ-AUTO-177`), and a line's sharp corners sew mitred
//! (`REQ-AUTO-178`).
//!
//! Design: `docs/src/design/image-to-embroidery.md` › A column's ends. A part's column whose end a slanted or
//! curved edge cuts ends at that edge's farthest corner, one rail running on along the edge to meet the other
//! there. Its last rung ran square to the mean of its rails' ways, one of them the end edge, so the stitches
//! between that rung and the corner spread like a fan along the column, over what lies beside it. Here such
//! an end gets rungs square to the column's way at the end, every [`SPACING`], from where either rail leaves
//! that way to the end's farthest corner: the stitches stay square and grow shorter into the corner, inside
//! the shape's own outline. Only an end after a straight stretch has one way to be square to: a column that
//! bends or wanders into its end, as a letter C cut along its radii or many of a traced picture's columns,
//! keeps its rungs, and so does a ring, which has no end.
//!
//! A line that turns sharply sews a column per leg between its corners, each ending on the corner's bisector
//! and squared there as a slanted end is, instead of one column whose stitches fan round the inside of each
//! corner ([`legs`]).
//!
//! Every step costs a unit of work, the column's being part of the work of digitizing its shape.

use stitchcraft_core::math::hypot;
use stitchcraft_core::{ElementId, Exhausted, Meter, Point};
use stitchcraft_params::ParamSet;

use super::{Style, part, suffixed, underlay, walks_once};
use crate::design::{Element, Join, Path, Segment, Shape};
use crate::generators::satin::drawn::from_rails;
use crate::normalize::strips::{CORNER, SQUARE};

/// The cosine of 20°: a rail runs along the column's way within 20° of it, and an end edge more than 20° from
/// square to the way is slanted.
const STRAIGHT: f64 = 0.939_692_620_785_908_4;

/// The cosine of 35°: a rail's corner that turns by more than 35° near an end folds the end edge into it.
const FOLD: f64 = 0.819_152_044_288_991_8;

/// How near the end, in widths, a folded corner may not lie: a corner nearer is the end's own rounding.
const FOLD_FROM: f64 = 0.3;

/// How far from its end, in widths, an end may close in: a 45° end edge is 1.41 widths long.
const REACH: f64 = 3.0;

/// How far from its end, in widths, the rails give the column's way there.
const WAY_OVER: f64 = 0.5;

/// How far apart the squared rungs lie along the column, in millimetres.
const SPACING: f64 = 0.5;

/// A column's rails, both from its start, and its rungs in order along it.
pub(super) type Rails = ([Vec<Point>; 2], Vec<[Point; 2]>);

/// A way of unit length.
type Way = (f64, f64);

/// The way from `a` to `b` as a unit, `None` for points that coincide.
fn unit(a: Point, b: Point) -> Option<Way> {
    let (dx, dy) = (b.x() - a.x(), b.y() - a.y());
    let length = hypot(dx, dy);
    (length > 1e-12).then(|| (dx / length, dy / length))
}

/// The dot product of 2 ways.
fn dot(u: Way, v: Way) -> f64 {
    u.0 * v.0 + u.1 * v.1
}

/// `rail`'s segments walked from its end back, each as its point nearer the end, its point farther, and how
/// far along the rail its nearer point lies from the end.
fn backwards(rail: &[Point]) -> impl Iterator<Item = (Point, Point, f64)> + '_ {
    let mut gone = 0.0;
    rail.windows(2).rev().filter_map(move |pair| {
        let (&[far, near], before) = (pair, gone) else { return None };
        gone += near.distance(far);
        Some((near, far, before))
    })
}

/// The point `d` along `rail` from its end back, or its start when it is shorter.
fn back(rail: &[Point], d: f64, meter: &mut Meter) -> Result<Option<Point>, Exhausted> {
    for (near, far, gone) in backwards(rail) {
        meter.charge(1)?;
        let length = near.distance(far);
        if gone + length >= d {
            return Ok(Some(near.lerp(far, if length > 0.0 { (d - gone) / length } else { 0.0 })));
        }
    }
    Ok(rail.first().copied())
}

/// How far from `rail`'s end an end edge folded into it turns into the rail: its sharpest corner within
/// [`REACH`] widths of the end and nearer it than the rail's other end, when it turns by more than 35° and lies
/// [`FOLD_FROM`] widths or more from the end; 0 when it has none. A corner nearer the other end is that end's:
/// a short column's other end edge, once it joins this rail, turns into it there.
fn folded(rail: &[Point], width: f64, meter: &mut Meter) -> Result<f64, Exhausted> {
    let mut sharpest: Option<(f64, f64)> = None;
    let mut later: Option<Way> = None;
    let within = (REACH * width).min(length(rail) / 2.0);
    for (near, far, gone) in backwards(rail) {
        meter.charge(1)?;
        if gone > within {
            break;
        }
        let Some(way) = unit(far, near) else { continue };
        if let Some(next) = later
            && sharpest.is_none_or(|(cosine, _)| dot(way, next) < cosine)
        {
            sharpest = Some((dot(way, next), gone));
        }
        later = Some(way);
    }
    Ok(sharpest.filter(|&(cosine, at)| cosine <= FOLD && at >= FOLD_FROM * width).map_or(0.0, |(_, at)| at))
}

/// The length of `rail`.
fn length(rail: &[Point]) -> f64 {
    rail.windows(2).map(|pair| if let [a, b] = pair { a.distance(*b) } else { 0.0 }).sum()
}

/// Where the line square to `way` at station `at` crosses `rail`, walking from its end back no farther than
/// `within`.
fn crossing(rail: &[Point], way: Way, at: f64, within: f64) -> Option<Point> {
    let station = |p: Point| p.x() * way.0 + p.y() * way.1;
    for (near, far, gone) in backwards(rail) {
        let (s_near, s_far) = (station(near), station(far));
        if (s_near - at) * (s_far - at) <= 0.0 && (s_near - s_far).abs() > 1e-12 {
            return Some(near.lerp(far, (at - s_near) / (s_far - s_near)));
        }
        if gone + near.distance(far) > within {
            return None;
        }
    }
    None
}

/// One end of a column, its rails walked so that the end is their last point, `width` wide: the way at the
/// end, the station from which the end closes in, and the rungs that replace the column's own from there.
/// The rails change when a slanted end edge joins the rail that stops first. `None` leaves the end as it is.
#[allow(clippy::type_complexity)]
fn end(rails: &mut [Vec<Point>; 2], width: f64, meter: &mut Meter) -> Result<Option<(Way, f64, Vec<[Point; 2]>)>, Exhausted> {
    // The way at the end, each rail's over its last half width, past an end edge folded into one rail only.
    let fold = [folded(&rails[0], width, meter)?, folded(&rails[1], width, meter)?];
    let skips = if fold[0] > 0.0 && fold[1] > 0.0 { [0.0, 0.0] } else { fold };
    let mut sum = (0.0, 0.0);
    for (rail, &skip) in rails.iter().zip(&skips) {
        let stretch = (WAY_OVER * width).min((length(rail) / 3.0 - skip).max(0.05));
        let (Some(near), Some(far)) = (back(rail, skip, meter)?, back(rail, skip + stretch, meter)?) else { return Ok(None) };
        let Some(way) = unit(far, near) else { return Ok(None) };
        sum = (sum.0 + way.0, sum.1 + way.1);
    }
    let (Some(origin), Ok(tip)) = (Point::new(0.0, 0.0).ok(), Point::new(sum.0, sum.1)) else { return Ok(None) };
    let Some(way) = unit(origin, tip) else { return Ok(None) };
    // A straight stretch before the end: both rails, past the folded edge, within 20° of the way for a width.
    for (rail, &skip) in rails.iter().zip(&skips) {
        for (near, far, gone) in backwards(rail) {
            meter.charge(1)?;
            if gone > skip + width {
                break;
            }
            let Some(along) = unit(far, near) else { continue };
            if gone + near.distance(far) > skip && dot(along, way) < STRAIGHT {
                return Ok(None);
            }
        }
    }
    let station = |p: Point| p.x() * way.0 + p.y() * way.1;
    let (Some(&first), Some(&second)) = (rails[0].last(), rails[1].last()) else { return Ok(None) };
    let mut body = f64::INFINITY;
    // A slanted end edge joins the rail that stops first, from its end to the other's.
    if let Some(edge) = unit(first, second).filter(|_| first.distance(second) > 1e-6) {
        // An edge within 20° of square is one a column ends along ([`SQUARE`]), and its stitches with it.
        if dot(edge, way).abs() <= SQUARE {
            return Ok(None);
        }
        let (lagging, leading) = if station(first) >= station(second) { (1, first) } else { (0, second) };
        body = station(if lagging == 1 { second } else { first });
        if let Some(rail) = rails.get_mut(lagging) {
            rail.push(leading);
        }
    }
    // Each rail, walked from its end back: the station where it last runs within 20° of the way.
    for rail in rails.iter() {
        for (near, far, gone) in backwards(rail) {
            meter.charge(1)?;
            if unit(far, near).is_some_and(|along| dot(along, way) >= STRAIGHT) {
                body = body.min(station(near));
                break;
            }
            if gone + near.distance(far) > REACH * width {
                break;
            }
        }
    }
    let tip = station(first).max(station(second));
    if !body.is_finite() || tip - body < SPACING.min(0.3) {
        return Ok(None);
    }
    let within = (tip - body) + 2.0 * width;
    let mut rungs = Vec::new();
    let mut at = body;
    while at < tip - SPACING / 2.0 {
        meter.charge(1)?;
        if let (Some(p), Some(q)) = (crossing(&rails[0], way, at, within), crossing(&rails[1], way, at, within))
            && p.distance(q) > 1e-3
        {
            rungs.push([p, q]);
        }
        at += SPACING;
    }
    Ok(Some((way, body, rungs)))
}

/// `rails` and `rungs`, a column `width` wide that isn't a ring, with each end that closes in after a
/// straight stretch squared (see the module's docs).
pub(super) fn squared(rails: [Vec<Point>; 2], rungs: Vec<[Point; 2]>, width: f64, meter: &mut Meter) -> Result<Rails, Exhausted> {
    let (mut rails, mut rungs) = (rails, rungs);
    if rails.iter().any(|rail| rail.len() < 2) {
        return Ok((rails, rungs));
    }
    // The column's finish, then its start, the rails walked back for it.
    for start in [false, true] {
        if start {
            rails.iter_mut().for_each(|rail| rail.reverse());
        }
        if let Some((way, body, added)) = end(&mut rails, width, meter)? {
            let station = |p: Point| p.x() * way.0 + p.y() * way.1;
            let tip = rails[0].last().copied();
            // The column's rungs from where the end closes in, near the end: a column that turns back on
            // itself has rungs at the same stations farther along it.
            let near = |p: Point| tip.is_some_and(|tip| tip.distance(p) < (station(tip) - body).abs() + REACH * width);
            meter.charge(u64::try_from(rungs.len()).unwrap_or(u64::MAX))?;
            rungs.retain(|rung| station(rung[0]).max(station(rung[1])) <= body - 0.05 || !near(rung[0]));
            if start {
                rungs.splice(0..0, added.into_iter().rev());
            } else {
                rungs.extend(added);
            }
        }
        if start {
            rails.iter_mut().for_each(|rail| rail.reverse());
        }
    }
    Ok((rails, rungs))
}

/// The legs of a line's column, its rails `rails` made from its centre line `line`, a polyline whose points
/// each rail pairs one for one, `width` wide: a column for each leg between the corners where the line turns
/// by more than 60° ([`CORNER`]), its rails from the inner corner to the
/// miter's point at each, each squared there. Each leg runs backwards, from its end to its start, when
/// `backwards`. Empty when the line has no such corner.
pub(super) fn legs(rails: &[Vec<Point>; 2], line: &[Point], width: f64, backwards: bool, meter: &mut Meter) -> Result<Vec<Rails>, Exhausted> {
    if rails.iter().any(|rail| rail.len() != line.len()) {
        return Ok(Vec::new());
    }
    let mut cuts = vec![0];
    for (k, three) in line.windows(3).enumerate() {
        meter.charge(1)?;
        let &[a, b, c] = three else { continue };
        if let (Some(into), Some(out)) = (unit(a, b), unit(b, c))
            && -dot(into, out) > CORNER
        {
            cuts.push(k + 1);
        }
    }
    if cuts.len() < 2 {
        return Ok(Vec::new());
    }
    cuts.push(line.len().saturating_sub(1));
    let mut made = Vec::with_capacity(cuts.len());
    for pair in cuts.windows(2) {
        let &[from, to] = pair else { continue };
        let leg = |rail: &Vec<Point>| {
            let mut leg = rail.get(from..=to).map(<[Point]>::to_vec).unwrap_or_default();
            if backwards {
                leg.reverse();
            }
            leg
        };
        made.push(squared([leg(&rails[0]), leg(&rails[1])], Vec::new(), width, meter)?);
    }
    Ok(made)
}

/// The elements that sew `element`'s subpath `id` along `line`, a stroke drawn with `style` whose column's
/// rails are `rails` and settings `column`, its other settings `rest`, mitred at each corner where it turns
/// by more than 60° (`REQ-AUTO-178`): a column for each leg, `id` with `~leg` and its number from the line's
/// start. Each leg of a line that walks its middle once (`REQ-AUTO-075`) has no underlay of its own: the line
/// first walks its whole length as `id`, then sews its legs' tops from its far end back, so that no leg travels
/// over another. A wider line's legs sew in turn from its start, each with its underlay. Empty when the line
/// is closed, curved, its corners not drawn sharp, or none turns so far: it then sews as one column.
pub(super) fn mitred(
    element: &Element,
    id: &ElementId,
    line: &Path,
    rails: &[Vec<Point>; 2],
    style: Style,
    (column, rest): (&ParamSet, &ParamSet),
    meter: &mut Meter,
) -> Result<Vec<Element>, Exhausted> {
    let Some(subpath) = line.subpaths.first().filter(|subpath| !subpath.closed && matches!(style.join, Join::Miter { .. })) else {
        return Ok(Vec::new());
    };
    let mut centre = vec![subpath.start];
    for segment in &subpath.segments {
        let Segment::Line(to) = segment else { return Ok(Vec::new()) };
        centre.push(*to);
    }
    let once = walks_once(style.width);
    let mut drawn = Vec::new();
    for (rails, rungs) in legs(rails, &centre, style.width.get(), once, meter)? {
        let Ok(leg) = from_rails(rails, rungs, [0.0, 0.0], meter)? else { return Ok(Vec::new()) };
        drawn.push(leg.path());
    }
    let mut leg_params = column.clone();
    if once {
        for key in underlay(style.width) {
            leg_params.set(*key, "false");
        }
    }
    let mut made = Vec::with_capacity(drawn.len() + 1);
    if once && !drawn.is_empty() {
        made.push(part(
            element,
            id.clone(),
            Shape::Stroke { path: line.clone(), width: style.width, join: style.join, cap: style.cap },
            rest.clone(),
        ));
    }
    for (k, path) in drawn.into_iter().enumerate() {
        let Some(leg_id) = suffixed(id, &format!("~leg{}", k + 1)) else { continue };
        made.push(part(element, leg_id, Shape::Stroke { path, width: style.width, join: style.join, cap: style.cap }, leg_params.clone()));
    }
    // A line that walks once sews its legs' tops from its far end back.
    if once {
        made.get_mut(1..).into_iter().for_each(<[Element]>::reverse);
    }
    Ok(made)
}

#[cfg(test)]
mod tests {
    use super::*;
    use stitchcraft_core::Budget;

    fn p(x: f64, y: f64) -> Point {
        Point::new(x, y).unwrap()
    }

    fn rail(points: &[(f64, f64)]) -> Vec<Point> {
        points.iter().map(|&(x, y)| p(x, y)).collect()
    }

    /// A band 5 mm wide along x from x = 0, its end at x = 40 slanted by 45°: the first rail, along y = 0, runs
    /// on along the end edge to the second's end, as a part's column ends at its end's farthest corner.
    fn slanted() -> [Vec<Point>; 2] {
        [rail(&[(0.0, 0.0), (20.0, 0.0), (40.0, 0.0), (45.0, 5.0)]), rail(&[(0.0, 5.0), (20.0, 5.0), (45.0, 5.0)])]
    }

    /// How far `rung` lies from square to x, in degrees.
    fn lean(rung: &[Point; 2]) -> f64 {
        stitchcraft_core::math::to_degrees(stitchcraft_core::math::atan2((rung[1].x() - rung[0].x()).abs(), (rung[1].y() - rung[0].y()).abs()))
    }

    #[test]
    fn req_auto_177_a_slanted_end_gets_rungs_square_to_the_column_every_half_millimetre() {
        let mut meter = Budget::DEFAULT.meter();
        let (rails, rungs) = squared(slanted(), vec![[p(20.0, 0.0), p(20.0, 5.0)], [p(42.0, 2.0), p(44.0, 5.0)]], 5.0, &mut meter).unwrap();
        // The rung across the slanted edge is replaced; the one in the middle stays first.
        assert_eq!(rungs.first(), Some(&[p(20.0, 0.0), p(20.0, 5.0)]));
        let added = rungs.get(1..).unwrap();
        // From x = 40, where the first rail leaves the way, to the tip at 45: 40, 40.5 … 44.5, short of the tip
        // by half a spacing.
        assert_eq!(added.len(), 10, "{added:?}");
        for (k, rung) in added.iter().enumerate() {
            let x = 40.0 + SPACING * k as f64;
            assert!((rung[0].x() - x).abs() < 1e-9 && (rung[1].x() - x).abs() < 1e-9, "{rung:?} at {x}");
            assert!(lean(rung) < 1e-9);
            // The first rail's point lies on the end edge, the other's on the band's top.
            assert!((rung[0].y() - (x - 40.0)).abs() < 1e-9 && (rung[1].y() - 5.0).abs() < 1e-9, "{rung:?}");
        }
        assert_eq!(rails, slanted(), "the rails already meet at the farthest corner");
    }

    #[test]
    fn req_auto_177_a_slanted_chord_joins_the_rail_that_stops_first() {
        // The rails end at the end edge's 2 corners: the second, which stops first, runs on to the first's end.
        let rails = [rail(&[(0.0, 0.0), (20.0, 0.0), (45.0, 0.0)]), rail(&[(0.0, 5.0), (20.0, 5.0), (40.0, 5.0)])];
        let (rails, rungs) = squared(rails, Vec::new(), 5.0, &mut Budget::DEFAULT.meter()).unwrap();
        assert_eq!(rails[1].last(), Some(&p(45.0, 0.0)));
        assert_eq!(rails[0].len(), 3);
        assert_eq!(rungs.len(), 10);
        assert!(rungs.iter().all(|rung| lean(rung) < 1e-9));
    }

    #[test]
    fn req_auto_177_a_square_end_and_a_bent_one_keep_their_rungs() {
        let square = [rail(&[(0.0, 0.0), (40.0, 0.0)]), rail(&[(0.0, 5.0), (40.0, 5.0)])];
        let rungs = vec![[p(20.0, 0.0), p(20.0, 5.0)]];
        assert_eq!(squared(square.clone(), rungs.clone(), 5.0, &mut Budget::DEFAULT.meter()).unwrap(), (square, rungs.clone()));
        // Within a width of its end the first rail turns 30° from the way.
        let bent = [rail(&[(0.0, 0.0), (36.0, 0.0), (40.0, 2.31)]), rail(&[(0.0, 5.0), (36.0, 5.0), (40.0, 7.31)])];
        assert_eq!(squared(bent.clone(), rungs.clone(), 5.0, &mut Budget::DEFAULT.meter()).unwrap(), (bent, rungs.clone()));
        // A rail of one point has no end to square.
        let point = [rail(&[(0.0, 0.0)]), rail(&[(0.0, 5.0), (40.0, 5.0)])];
        assert_eq!(squared(point.clone(), rungs.clone(), 5.0, &mut Budget::DEFAULT.meter()).unwrap(), (point, rungs));
    }

    #[test]
    fn req_auto_177_a_short_column_slanted_at_both_ends_is_square_at_both() {
        // A leg of a line 5 mm wide turning 90° at both ends, 3 widths long: once its finish's edge joins the
        // first rail, that rail turns into it 15 mm from the start, a corner of the finish, not of the start.
        let rails = [rail(&[(24.5, 17.5), (24.5, 32.5)]), rail(&[(19.5, 22.5), (19.5, 37.5)])];
        let (_, rungs) = squared(rails, Vec::new(), 5.0, &mut Budget::DEFAULT.meter()).unwrap();
        assert_eq!(rungs.len(), 20, "{rungs:?}");
        assert!(rungs.iter().all(|rung| (rung[0].y() - rung[1].y()).abs() < 1e-9), "{rungs:?}");
    }

    #[test]
    fn req_auto_177_squaring_is_the_part_s_work() {
        for units in 0.. {
            let mut meter = Budget { max_work: units, ..Budget::DEFAULT }.meter();
            if squared(slanted(), Vec::new(), 5.0, &mut meter).is_ok() {
                assert!(units > 20, "squared within {units} units");
                break;
            }
        }
    }

    #[test]
    fn req_auto_178_a_line_s_sharp_corners_cut_it_into_legs_squared_on_the_bisector() {
        // A line 2 mm wide along x, turning 90° up at x = 10: its rails pair its points one for one.
        let line = rail(&[(0.0, 0.0), (10.0, 0.0), (10.0, 10.0)]);
        let rails = [rail(&[(0.0, -1.0), (11.0, -1.0), (11.0, 10.0)]), rail(&[(0.0, 1.0), (9.0, 1.0), (9.0, 10.0)])];
        let legs = legs(&rails, &line, 2.0, false, &mut Budget::DEFAULT.meter()).unwrap();
        assert_eq!(legs.len(), 2);
        // The first leg ends on the bisector from (9, 1) to (11, -1), squared: rungs at x = 9 to 10.5.
        let (first, rungs) = &legs[0];
        assert_eq!(first[0].first(), Some(&p(0.0, -1.0)));
        assert_eq!(rungs.len(), 4, "{rungs:?}");
        assert!(rungs.iter().all(|rung| lean(rung) < 1e-9));
        // Run backwards, the first leg starts on the bisector.
        let back = super::legs(&rails, &line, 2.0, true, &mut Budget::DEFAULT.meter()).unwrap();
        assert_eq!(back[0].0[0].first(), Some(&p(11.0, -1.0)));
        // A line turning 60° or less, or one whose rails don't pair its points, has no legs.
        let gentle = rail(&[(0.0, 0.0), (10.0, 0.0), (15.0, 8.0)]);
        let gentle_rails = [rail(&[(0.0, -1.0), (10.6, -1.0), (15.9, 8.2)]), rail(&[(0.0, 1.0), (9.4, 1.0), (14.1, 9.2)])];
        assert!(super::legs(&gentle_rails, &gentle, 2.0, false, &mut Budget::DEFAULT.meter()).unwrap().is_empty());
        assert!(super::legs(&rails, &line[..2], 2.0, false, &mut Budget::DEFAULT.meter()).unwrap().is_empty());
    }

    #[test]
    fn req_auto_178_cutting_a_line_into_legs_is_its_work() {
        let line = rail(&[(0.0, 0.0), (10.0, 0.0), (10.0, 10.0)]);
        let rails = [rail(&[(0.0, -1.0), (11.0, -1.0), (11.0, 10.0)]), rail(&[(0.0, 1.0), (9.0, 1.0), (9.0, 10.0)])];
        for units in 0.. {
            if legs(&rails, &line, 2.0, false, &mut Budget { max_work: units, ..Budget::DEFAULT }.meter()).is_ok() {
                assert!(units > 0);
                break;
            }
        }
    }
}
