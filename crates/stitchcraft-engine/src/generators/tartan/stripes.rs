//! A tartan laid over one piece of a fill: its stripes, its rows, and the lines of each colour that the
//! rows make in its stripes, as Ink/Stitch lays them (`docs/src/design/algorithms/tartan.md` › Stripes and
//! rows, read at `d59c9ab`).
//!
//! **Why in the sett's own frame.** Ink/Stitch turns each stripe and each row about the middle of the
//! piece's bounds, then cuts them against each other. Turned back by the sett's angle, the stripes are
//! bands square to the axes, a warp band upright and a weft band level, and a row is a straight line at
//! `tartan_angle`, so where it crosses a band is worked out exactly, slab by slab. The piece is turned into
//! that frame once, the lines are made and cut there, and they are turned back with the sett.
//!
//! **The box.** The stripes and rows cover a square round the middle of the piece's bounds, as wide either
//! way as the radius of the smallest circle round the piece, plus a margin: the wider of the 2 directions'
//! setts, or 2 rows per thread of rows, whichever is more. Its left and top sides move out by the offsets,
//! and the warp's stripes start at its left side and the weft's at its top, in turn until they pass the
//! other side. Bands of one colour that meet are one band.
//!
//! **The rows** run across the box at `tartan_angle` to the stripes, from its turned left side to its turned
//! right side: the side's length is cut into equal steps no longer than the row spacing, and a row starts at
//! each. Along each, needle points lie the longest stitch apart from a `staggers`-th of a stitch further on
//! than the row before's, and its ends are points too. Rows go to the warp and to the weft in turns of
//! `rows_per_thread`. Each direction's rows are taken from the end of the box's left side Ink/Stitch sorts
//! first, and every other one runs back.
//!
//! **A band's lines.** Each of the direction's rows that crosses the band gives the stretch of it inside
//! the band, but one no longer than half a pixel. Each stretch is joined to the next by a stitch along the
//! band's edge, unless either end lies on the piece's rings, so the band is one zigzag of rows. The zigzag is
//! cut where it leaves the piece, and a line that ends inside the piece runs on along the band's edge to the
//! nearest ring.
//!
//! **A run** lies along a narrow stripe's middle across the box, cut where it leaves the piece, each piece in
//! equal stitches no longer than the running stitch, or for the weft the longest stitch, as Ink/Stitch sews
//! them. A piece of 0.5 mm or less is left out.
//!
//! **What it costs.** Every step of the setts, every row and its needle points, every band each row is
//! tested against, and every point of a line is charged to the meter before memory is taken for it.

use std::cmp::Ordering;
use std::collections::BTreeMap;

use stitchcraft_core::math::{sin_cos, to_radians};
use stitchcraft_core::rng::SplitMix64;
use stitchcraft_core::{Exhausted, Meter, Point};
use stitchcraft_plan::Rgb;

use super::Tartan;
use super::sett::{Item, PX};
use crate::generators::made;
use crate::normalize::near::nearest_to_point;
use crate::normalize::region::Polygon;
use crate::normalize::region::cut::inside;

/// How near the piece's rings a stretch's end may lie and still be joined to the next stretch, in
/// millimetres: 0.005 pixel, as Ink/Stitch tests it.
const ON_RINGS: f64 = 0.005 * PX;
/// How far inside the piece a line's end must lie to run on to its rings, in millimetres: 0.05 pixel.
const INSIDE: f64 = 0.05 * PX;
/// The shortest stretch of row kept, in millimetres: half a pixel.
const SHORTEST_STRETCH: f64 = 0.5 * PX;
/// The shortest run kept, in millimetres.
const SHORTEST_RUN: f64 = 0.5;

/// A turn about a point, as Ink/Stitch's geometry library turns: by an angle in degrees, counter-clockwise
/// in y-up axes, its cosine and sine taken as 0 within 2.5 × 10⁻¹⁶ of it, so that quarter turns are exact.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Turn {
    about: Point,
    cos: f64,
    sin: f64,
}

impl Turn {
    /// The turn by `degrees` about `about`.
    pub(crate) fn new(about: Point, degrees: f64) -> Turn {
        let (sin, cos) = sin_cos(to_radians(degrees));
        let snapped = |value: f64| if value.abs() < 2.5e-16 { 0.0 } else { value };
        Turn { about, cos: snapped(cos), sin: snapped(sin) }
    }

    /// The turn that undoes this one.
    pub(crate) fn back(self) -> Turn {
        Turn { sin: -self.sin, ..self }
    }

    /// Where the turn takes `p`.
    pub(crate) fn apply(self, p: Point) -> Point {
        let (dx, dy) = (p.x() - self.about.x(), p.y() - self.about.y());
        Point::new(self.about.x() + dx * self.cos - dy * self.sin, self.about.y() + dx * self.sin + dy * self.cos).unwrap_or(p)
    }

    /// Where it takes the direction (`x`, `y`).
    fn direction(self, (x, y): (f64, f64)) -> (f64, f64) {
        (x * self.cos - y * self.sin, x * self.sin + y * self.cos)
    }
}

/// Each colour's lines over a piece, the warp's first, in the order a direction's stripes first reach the
/// colour, and the runs the same way.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct Lines {
    /// Each colour's lines along its stripes' rows, in the sett's frame turned back to the piece's.
    pub(crate) fills: Vec<(Rgb, Vec<Vec<Point>>)>,
    /// Each colour's runs, each in its equal stitches.
    pub(crate) runs: Vec<(Rgb, Vec<Vec<Point>>)>,
}

/// The tartan's lines over `piece`, as the module's docs say. Work is charged to `meter` for every step of
/// the setts, every row and its needle points, every band each row is tested against, and as cutting the
/// lines to the piece costs, and a stitch for every needle point made.
pub(crate) fn lay(piece: &Polygon, tartan: &Tartan, meter: &mut Meter) -> Result<Lines, Exhausted> {
    let Some(frame) = Frame::of(piece, tartan, meter)? else { return Ok(Lines::default()) };
    let turn = Turn::new(frame.middle, tartan.sett.rotate);
    let back = turn.back();
    let turned = Polygon {
        outline: piece.outline.iter().map(|&p| back.apply(p)).collect(),
        holes: piece.holes.iter().map(|hole| hole.iter().map(|&p| back.apply(p)).collect()).collect(),
    };
    let Some(reach) = bounds(&turned.outline, meter)? else { return Ok(Lines::default()) };
    let rows = Rows::new(&frame, tartan, meter)?;
    let mut lines = Lines::default();
    for (direction, items) in [(Direction::Warp, &tartan.warp), (Direction::Weft, &tartan.weft)] {
        let walked = walk(items, direction, &frame, meter)?;
        let laying = Laying { frame: &frame, tartan, piece: &turned, reach, rings: turned.rings().collect(), direction, turn };
        for (colour, bands) in &walked.bands {
            let mut colour_lines = Vec::new();
            for &band in bands {
                colour_lines.extend(laying.band(band, &rows, meter)?);
            }
            add(&mut lines.fills, *colour, colour_lines);
        }
        for (colour, at) in &walked.runs {
            let mut runs = Vec::new();
            for &x in at {
                runs.extend(laying.run(x, meter)?);
            }
            add(&mut lines.runs, *colour, runs);
        }
    }
    Ok(lines)
}

/// Adds `more` lines of `colour` to `to`, after the colour's own, or as a colour of their own after the rest:
/// a colour without lines is left out.
fn add(to: &mut Vec<(Rgb, Vec<Vec<Point>>)>, colour: Rgb, more: Vec<Vec<Point>>) {
    if more.is_empty() {
        return;
    }
    match to.iter_mut().find(|(c, _)| *c == colour) {
        Some((_, lines)) => lines.extend(more),
        None => to.push((colour, more)),
    }
}

/// The box the stripes and rows cover, in the sett's frame, and the middle they turn about.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Frame {
    min: [f64; 2],
    max: [f64; 2],
    middle: Point,
}

impl Frame {
    /// The box round `piece` as the module's docs say; `None` for a piece without points.
    fn of(piece: &Polygon, tartan: &Tartan, meter: &mut Meter) -> Result<Option<Frame>, Exhausted> {
        let Some((lo, hi)) = bounds(&piece.outline, meter)? else { return Ok(None) };
        let Ok(middle) = Point::new((lo[0] + hi[0]) / 2.0, (lo[1] + hi[1]) / 2.0) else { return Ok(None) };
        let radius = enclosing_radius(&piece.outline, meter)?;
        let reach = radius + tartan.margin.max(2.0 * tartan.spacing * f64::from(tartan.rows_per_thread));
        let [ox, oy] = tartan.sett.offset;
        Ok(Some(Frame { min: [middle.x() - reach - ox, middle.y() - reach - oy], max: [middle.x() + reach, middle.y() + reach], middle }))
    }
}

/// The least x and y of `points` and the greatest, `None` for no points. A unit of `meter` for each point.
fn bounds(points: &[Point], meter: &mut Meter) -> Result<Option<([f64; 2], [f64; 2])>, Exhausted> {
    meter.charge(u64::try_from(points.len()).unwrap_or(u64::MAX))?;
    let Some(first) = points.first() else { return Ok(None) };
    let (mut lo, mut hi) = ([first.x(), first.y()], [first.x(), first.y()]);
    for p in points {
        lo = [lo[0].min(p.x()), lo[1].min(p.y())];
        hi = [hi[0].max(p.x()), hi[1].max(p.y())];
    }
    Ok(Some((lo, hi)))
}

/// Which stripes: the warp's, upright bands laid from the box's left side, or the weft's, level ones from its
/// top.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Direction {
    Warp,
    Weft,
}

impl Direction {
    /// The axis its bands are laid along: x for the warp, y for the weft.
    const fn axis(self) -> usize {
        match self {
            Direction::Warp => 0,
            Direction::Weft => 1,
        }
    }

    /// The way along its bands' edges.
    const fn along(self) -> (f64, f64) {
        match self {
            Direction::Warp => (0.0, 1.0),
            Direction::Weft => (1.0, 0.0),
        }
    }
}

/// What a direction's full sett lays across the box: each colour's bands, those that meet made one, and its
/// runs, each colour in the order the sett first reaches it, by its bands or its runs, as Ink/Stitch keeps
/// one list of a direction's colours.
#[derive(Debug, Default)]
struct Walked {
    bands: Vec<(Rgb, Vec<[f64; 2]>)>,
    runs: Vec<(Rgb, Vec<f64>)>,
}

/// The bands and runs `items` lay along `direction`'s axis of `frame`, from its least side to past its
/// greatest, the full sett taken in turn. Nothing for a sett without width. One unit of `meter` for each step.
fn walk(items: &[Item], direction: Direction, frame: &Frame, meter: &mut Meter) -> Result<Walked, Exhausted> {
    let total: f64 = items.iter().map(|item| if let Item::Band { width, .. } = item { *width } else { 0.0 }).sum();
    let axis = direction.axis();
    let (Some(&start), Some(&end)) = (frame.min.get(axis), frame.max.get(axis)) else { return Ok(Walked::default()) };
    if total.partial_cmp(&0.0) != Some(Ordering::Greater) {
        return Ok(Walked::default());
    }
    let (mut order, mut bands, mut runs) = (Vec::new(), BTreeMap::<Rgb, Vec<[f64; 2]>>::new(), BTreeMap::<Rgb, Vec<f64>>::new());
    let mut at = start;
    for item in items.iter().cycle() {
        meter.charge(1)?;
        if at > end {
            break;
        }
        match *item {
            Item::Band { colour: None, width } => at += width,
            Item::Band { colour: Some(colour), width } => {
                listed(&mut order, colour);
                let laid = bands.entry(colour).or_default();
                match laid.last_mut() {
                    Some(last) if last[1] == at => last[1] = at + width,
                    _ => laid.push([at, at + width]),
                }
                at += width;
            }
            Item::Run(colour) => {
                listed(&mut order, colour);
                runs.entry(colour).or_default().push(at);
            }
        }
    }
    let bands = order.iter().filter_map(|colour| bands.remove(colour).map(|laid| (*colour, laid))).collect();
    let runs = order.iter().filter_map(|colour| runs.remove(colour).map(|at| (*colour, at))).collect();
    Ok(Walked { bands, runs })
}

/// Adds `colour` to `order` when it isn't there.
fn listed(order: &mut Vec<Rgb>, colour: Rgb) {
    if !order.contains(&colour) {
        order.push(colour);
    }
}

/// A tartan's rows across the box, in the sett's frame.
struct Rows {
    /// Where each row starts, in the order across the box.
    starts: Vec<Point>,
    /// The way each runs, and how long it is.
    along: (f64, f64),
    length: f64,
    /// Along each row, how far its needle points lie from its start, its ends included, in order.
    points: Vec<Vec<f64>>,
    /// Each direction's rows in the order taken, each with whether it runs back: the warp's, then the weft's.
    taken: [Vec<(usize, bool)>; 2],
}

impl Rows {
    /// The rows of `tartan` across `frame`, as the module's docs say.
    fn new(frame: &Frame, tartan: &Tartan, meter: &mut Meter) -> Result<Rows, Exhausted> {
        let [minx, miny] = frame.min;
        let [maxx, maxy] = frame.max;
        let (height, length) = (maxy - miny, maxx - minx);
        // The left side's length in equal steps no longer than the row spacing: at least 1, and as many as
        // the meter allows, which refuses a count `as` saturates.
        let count = steps((height / tartan.spacing).ceil(), 1);
        meter.charge(u64::try_from(count).unwrap_or(u64::MAX))?;
        let rows = Turn::new(frame.middle, tartan.angle);
        let along = rows.direction((1.0, 0.0));
        let mut starts = Vec::with_capacity(count + 1);
        let mut points = Vec::with_capacity(count + 1);
        for k in 0..=count {
            let y = miny + (k as f64 / count as f64) * height;
            starts.push(rows.apply(Point::new(minx, y).unwrap_or(frame.middle)));
            points.push(needle_points(k, length, tartan, meter)?);
        }
        // Ink/Stitch sorts each direction's rows by their starts, x first, which the left side turned by the
        // sett's angle and the rows' runs along one way or the other.
        let left = Turn::new(frame.middle, tartan.sett.rotate + tartan.angle).direction((0.0, 1.0));
        let forwards = left.0 > 0.0 || (left.0 == 0.0 && left.1 > 0.0);
        let per = usize::try_from(tartan.rows_per_thread.max(1)).unwrap_or(1);
        let mut taken = [Vec::new(), Vec::new()];
        let order: Vec<usize> = if forwards { (0..=count).collect() } else { (0..=count).rev().collect() };
        for k in order {
            let direction = usize::from(k % (2 * per) >= per);
            if let Some(list) = taken.get_mut(direction) {
                let back = list.len() % 2 == 1;
                list.push((k, back));
            }
        }
        Ok(Rows { starts, along, length, points, taken })
    }

    /// Row `k`'s point `t` along it.
    fn at(&self, k: usize, t: f64) -> Option<Point> {
        let start = self.starts.get(k)?;
        Point::new(start.x() + t * self.along.0, start.y() + t * self.along.1).ok()
    }
}

/// How far along row `k`, `length` long, its needle points lie: its start, then the longest stitch apart
/// from a `staggers`-th of a stitch further on than the row before's, as Ink/Stitch places them, then its
/// end. Without 2 such points between its ends, its ends alone. A unit of `meter` for each point, spent
/// before they are made.
fn needle_points(k: usize, length: f64, tartan: &Tartan, meter: &mut Meter) -> Result<Vec<f64>, Exhausted> {
    let staggers = if tartan.staggers == 0.0 { 1.0 } else { tartan.staggers };
    let first = (k as f64 / staggers).rem_euclid(1.0) * tartan.longest;
    let count = steps(((length - first) / tartan.longest).ceil(), 0);
    meter.charge(u64::try_from(count).unwrap_or(u64::MAX).saturating_add(2))?;
    let mut ts = Vec::with_capacity(count.saturating_add(2));
    ts.push(0.0);
    if count >= 2 {
        ts.extend((0..count).map(|j| first + j as f64 * tartan.longest).filter(|&t| t > 0.0));
    }
    ts.push(length);
    Ok(ts)
}

/// `steps`, a whole number of steps, as a count: `least` for one that is no number or less than it, and
/// `usize::MAX` for one past it, as `as` saturates, which no meter allows.
fn steps(steps: f64, least: usize) -> usize {
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let count = steps as usize;
    if steps.is_nan() { least } else { count.max(least) }
}

/// What laying a direction's bands and runs over a piece needs.
struct Laying<'l> {
    frame: &'l Frame,
    tartan: &'l Tartan,
    /// The piece, in the sett's frame, its bounds and its rings.
    piece: &'l Polygon,
    reach: ([f64; 2], [f64; 2]),
    rings: Vec<&'l [Point]>,
    direction: Direction,
    /// The sett's turn, which takes the lines back to the piece's frame.
    turn: Turn,
}

/// A stretch of a row inside a band: its points from the end it is sewn from.
type Stretch = Vec<Point>;

impl Laying<'_> {
    /// The lines `band`, from one side to the other along the direction's axis, makes of the direction's
    /// rows over the piece, turned back to the piece's frame.
    fn band(&self, band: [f64; 2], rows: &Rows, meter: &mut Meter) -> Result<Vec<Vec<Point>>, Exhausted> {
        let taken = rows.taken.get(self.direction.axis()).map(Vec::as_slice).unwrap_or_default();
        let mut stretches: Vec<Stretch> = Vec::new();
        for &(k, back) in taken {
            meter.charge(1)?;
            if let Some(stretch) = self.stretch(band, rows, k, back) {
                stretches.push(stretch);
            }
        }
        let (first, last) = self.near(&stretches, meter)?;
        let mut zigzags: Vec<Vec<Point>> = vec![Vec::new()];
        for stretch in stretches.get(first..=last).unwrap_or_default() {
            let Some(zigzag) = zigzags.last_mut() else { break };
            match (zigzag.last().copied(), stretch.first().copied()) {
                (Some(end), Some(start)) if end == start => zigzag.extend(stretch.iter().skip(1)),
                (Some(end), Some(start)) if self.on_rings(end, meter)? || self.on_rings(start, meter)? => zigzags.push(stretch.clone()),
                (Some(end), Some(start)) => {
                    let join = segmentized(end, start, self.tartan.run_length, meter)?;
                    zigzag.extend(join.get(1..join.len().saturating_sub(1)).unwrap_or_default());
                    zigzag.extend(stretch);
                }
                _ => zigzag.extend(stretch),
            }
        }
        let mut lines = Vec::new();
        for zigzag in zigzags.iter().filter(|zigzag| zigzag.len() >= 2) {
            for mut line in inside(zigzag, std::slice::from_ref(self.piece), meter)? {
                self.run_on(&mut line, meter)?;
                made(meter, line.len())?;
                lines.push(line.into_iter().map(|p| self.turn.apply(p)).collect());
            }
        }
        Ok(lines)
    }

    /// The stretch of row `k` inside `band`, from the end the row runs from, or back; `None` where the row
    /// misses the band or crosses it for half a pixel or less.
    fn stretch(&self, band: [f64; 2], rows: &Rows, k: usize, back: bool) -> Option<Stretch> {
        let start = rows.starts.get(k)?;
        let (mut lo, mut hi) = (0.0_f64, rows.length);
        for axis in 0..2 {
            let (from, way) = if axis == 0 { (start.x(), rows.along.0) } else { (start.y(), rows.along.1) };
            let (least, most) = if axis == self.direction.axis() {
                (band[0], band[1])
            } else {
                (self.frame.min.get(axis).copied()?, self.frame.max.get(axis).copied()?)
            };
            if way == 0.0 {
                if from < least || from > most {
                    return None;
                }
                continue;
            }
            let (a, b) = ((least - from) / way, (most - from) / way);
            (lo, hi) = (lo.max(a.min(b)), hi.min(a.max(b)));
        }
        if (hi - lo).partial_cmp(&SHORTEST_STRETCH) != Some(Ordering::Greater) {
            return None;
        }
        let ts = rows.points.get(k)?;
        let inner = ts.iter().copied().filter(|&t| t > lo && t < hi);
        let mut stretch: Vec<Point> = std::iter::once(lo).chain(inner).chain(std::iter::once(hi)).filter_map(|t| rows.at(k, t)).collect();
        if back {
            stretch.reverse();
        }
        Some(stretch)
    }

    /// Which stretches can reach the piece, as the first and last of them: those that come near its box, and
    /// one more either side, whose joins may cross it. Those farther off lie, with their joins, outside it.
    /// A unit of `meter` for each point of each stretch.
    fn near(&self, stretches: &[Stretch], meter: &mut Meter) -> Result<(usize, usize), Exhausted> {
        let (lo, hi) = self.reach;
        meter.charge(u64::try_from(stretches.iter().map(Vec::len).sum::<usize>()).unwrap_or(u64::MAX))?;
        let reaches = |stretch: &Stretch| {
            let (mut a, mut b) = ([f64::INFINITY; 2], [f64::NEG_INFINITY; 2]);
            for p in stretch {
                a = [a[0].min(p.x()), a[1].min(p.y())];
                b = [b[0].max(p.x()), b[1].max(p.y())];
            }
            a[0] <= hi[0] && b[0] >= lo[0] && a[1] <= hi[1] && b[1] >= lo[1]
        };
        let first = stretches.iter().position(reaches);
        let last = stretches.iter().rposition(reaches);
        Ok(match (first, last) {
            (Some(first), Some(last)) => (first.saturating_sub(1), (last + 1).min(stretches.len().saturating_sub(1))),
            _ => (1, 0),
        })
    }

    /// Whether `p` lies within [`ON_RINGS`] of the piece's rings.
    fn on_rings(&self, p: Point, meter: &mut Meter) -> Result<bool, Exhausted> {
        self.within(p, ON_RINGS, meter)
    }

    /// `line`, an end of which lies inside the piece, more than [`INSIDE`] from its rings, run on from that
    /// end along the band's edge to the nearest point where the edge's line meets a ring, in stitches no
    /// longer than the running stitch.
    fn run_on(&self, line: &mut Vec<Point>, meter: &mut Meter) -> Result<(), Exhausted> {
        for end in [false, true] {
            let Some(&p) = (if end { line.last() } else { line.first() }) else { return Ok(()) };
            if !self.piece.covers(p) || self.within(p, INSIDE, meter)? {
                continue;
            }
            let Some(to) = self.crossing(p, meter)? else { continue };
            let way = segmentized(p, to, self.tartan.run_length, meter)?;
            if end {
                line.extend(way.into_iter().skip(1));
            } else {
                let mut joined: Vec<Point> = way.into_iter().rev().collect();
                joined.pop();
                joined.append(line);
                *line = joined;
            }
        }
        Ok(())
    }

    /// Whether `p` lies within `reach` of the piece's rings, or the piece has none.
    fn within(&self, p: Point, reach: f64, meter: &mut Meter) -> Result<bool, Exhausted> {
        Ok(nearest_to_point(&self.rings, p, meter)?.is_none_or(|(_, distance)| distance <= reach))
    }

    /// The point nearest `p` where the line through it along the band's edge meets a ring of the piece.
    fn crossing(&self, p: Point, meter: &mut Meter) -> Result<Option<Point>, Exhausted> {
        let (ux, uy) = self.direction.along();
        let mut best: Option<(f64, Point)> = None;
        for ring in &self.rings {
            for side in ring.windows(2) {
                meter.charge(1)?;
                let &[a, b] = side else { continue };
                let (dx, dy) = (b.x() - a.x(), b.y() - a.y());
                let denominator = ux * dy - uy * dx;
                if denominator == 0.0 {
                    continue;
                }
                let (ax, ay) = (a.x() - p.x(), a.y() - p.y());
                let s = (ax * dy - ay * dx) / denominator;
                let r = (ax * uy - ay * ux) / denominator;
                if !(0.0..=1.0).contains(&r) {
                    continue;
                }
                let Ok(at) = Point::new(p.x() + s * ux, p.y() + s * uy) else { continue };
                if best.is_none_or(|(distance, _)| s.abs() < distance) {
                    best = Some((s.abs(), at));
                }
            }
        }
        Ok(best.map(|(_, at)| at))
    }

    /// The pieces of the run along the line at `at` on the direction's axis, across the box, inside the piece,
    /// each in equal stitches, turned back to the piece's frame: those longer than [`SHORTEST_RUN`].
    fn run(&self, at: f64, meter: &mut Meter) -> Result<Vec<Vec<Point>>, Exhausted> {
        let [minx, miny] = self.frame.min;
        let [maxx, maxy] = self.frame.max;
        let (a, b) = match self.direction {
            Direction::Warp => ((at, miny), (at, maxy)),
            Direction::Weft => ((minx, at), (maxx, at)),
        };
        let (Ok(a), Ok(b)) = (Point::new(a.0, a.1), Point::new(b.0, b.1)) else { return Ok(Vec::new()) };
        let stitch = if self.direction == Direction::Warp { self.tartan.run_length } else { self.tartan.longest };
        let mut runs = Vec::new();
        for piece in inside(&[a, b], std::slice::from_ref(self.piece), meter)? {
            let long: f64 = piece.windows(2).filter_map(|pair| if let &[p, q] = pair { Some(p.distance(q)) } else { None }).sum();
            if long <= SHORTEST_RUN {
                continue;
            }
            let mut run: Vec<Point> = Vec::new();
            for pair in piece.windows(2) {
                let &[p, q] = pair else { continue };
                run.extend(segmentized(p, q, stitch, meter)?.into_iter().skip(usize::from(!run.is_empty())));
            }
            made(meter, run.len())?;
            runs.push(run.into_iter().map(|p| self.turn.apply(p)).collect());
        }
        Ok(runs)
    }
}

/// The straight line from `a` to `b` in equal steps no longer than `longest`, both ends included. A unit of
/// `meter` for each point, spent before they are made.
fn segmentized(a: Point, b: Point, longest: f64, meter: &mut Meter) -> Result<Vec<Point>, Exhausted> {
    let count = steps((a.distance(b) / longest).ceil(), 1);
    meter.charge(u64::try_from(count).unwrap_or(u64::MAX).saturating_add(1))?;
    Ok((0..=count).map(|i| if i == count { b } else { a.lerp(b, i as f64 / count as f64) }).collect())
}

/// The radius of the smallest circle round `points`, as Ink/Stitch's geometry library measures a piece: the
/// circle through 2 or 3 of the points of their convex hull. The hull's points are taken in an order the
/// same shuffle gives every time, which keeps the search short. A unit of `meter` for each point and each
/// step of the search.
pub(crate) fn enclosing_radius(points: &[Point], meter: &mut Meter) -> Result<f64, Exhausted> {
    let mut hull = convex_hull(points, meter)?;
    let mut rng = SplitMix64::new(0x7a27_a4f1);
    for i in (1..hull.len()).rev() {
        let j = usize::try_from(rng.next_u64() % (i as u64 + 1)).unwrap_or(0);
        hull.swap(i, j);
    }
    let Some(&first) = hull.first() else { return Ok(0.0) };
    let mut circle = Circle { middle: first, radius: 0.0 };
    for (i, &p) in hull.iter().enumerate() {
        meter.charge(1)?;
        if circle.holds(p) {
            continue;
        }
        circle = Circle { middle: p, radius: 0.0 };
        for (j, &q) in hull.iter().enumerate().take(i) {
            meter.charge(1)?;
            if circle.holds(q) {
                continue;
            }
            circle = Circle::across(p, q);
            for &r in hull.iter().take(j) {
                meter.charge(1)?;
                if !circle.holds(r) {
                    circle = Circle::through(p, q, r);
                }
            }
        }
    }
    Ok(circle.radius)
}

/// A circle, by its middle and its radius.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Circle {
    middle: Point,
    radius: f64,
}

impl Circle {
    /// Whether `p` lies in the circle, allowing for the rounding of the arithmetic that found it.
    fn holds(self, p: Point) -> bool {
        self.middle.distance(p) <= self.radius * (1.0 + 1e-12) + 1e-12
    }

    /// The circle whose diameter runs from `a` to `b`.
    fn across(a: Point, b: Point) -> Circle {
        Circle { middle: a.lerp(b, 0.5), radius: a.distance(b) / 2.0 }
    }

    /// The circle through `a`, `b` and `c`; for 3 points on a line, the circle across the 2 farthest apart.
    fn through(a: Point, b: Point, c: Point) -> Circle {
        let (bx, by, cx, cy) = (b.x() - a.x(), b.y() - a.y(), c.x() - a.x(), c.y() - a.y());
        let d = 2.0 * (bx * cy - by * cx);
        let (b2, c2) = (bx * bx + by * by, cx * cx + cy * cy);
        let (ux, uy) = ((cy * b2 - by * c2) / d, (bx * c2 - cx * b2) / d);
        match Point::new(a.x() + ux, a.y() + uy) {
            Ok(middle) if d != 0.0 => Circle { middle, radius: middle.distance(a) },
            _ => [Circle::across(a, b), Circle::across(a, c), Circle::across(b, c)]
                .into_iter()
                .max_by(|x, y| x.radius.total_cmp(&y.radius))
                .unwrap_or(Circle::across(a, b)),
        }
    }
}

/// The corners of the convex hull of `points`, each once, without the points along its sides, by Andrew's
/// monotone chain. A unit of `meter` for each point.
pub(crate) fn convex_hull(points: &[Point], meter: &mut Meter) -> Result<Vec<Point>, Exhausted> {
    meter.charge(u64::try_from(points.len()).unwrap_or(u64::MAX))?;
    let mut sorted = points.to_vec();
    sorted.sort_by(|a, b| a.x().total_cmp(&b.x()).then(a.y().total_cmp(&b.y())));
    sorted.dedup();
    if sorted.len() < 3 {
        return Ok(sorted);
    }
    let (mut lower, mut upper) = (Vec::new(), Vec::new());
    for &p in &sorted {
        turned_left(&mut lower, p);
    }
    for &p in sorted.iter().rev() {
        turned_left(&mut upper, p);
    }
    lower.pop();
    upper.pop();
    lower.extend(upper);
    Ok(lower)
}

/// Adds `p` to `chain`, a chain of a hull's corners, once the corners before it that it doesn't turn left at
/// are taken off.
fn turned_left(chain: &mut Vec<Point>, p: Point) {
    while let (Some(&b), Some(&a)) = (chain.last(), chain.len().checked_sub(2).and_then(|i| chain.get(i))) {
        let cross = (b.x() - a.x()) * (p.y() - a.y()) - (b.y() - a.y()) * (p.x() - a.x());
        if cross > 0.0 {
            break;
        }
        chain.pop();
    }
    chain.push(p);
}

#[cfg(test)]
mod tests;
