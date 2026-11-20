//! Outlines smoothed where thread can't show their wobble (`REQ-AUTO-131` to `REQ-AUTO-134`, `SC-I0921`).
//!
//! A traced picture's outlines carry many points and wobble by less than a thread is wide: a staircase of
//! pixels, a jag of a tenth of a millimetre. Thread can't show that wobble, and sewing it makes short stitches
//! and ragged edges. With `auto_smooth_mm` set, an automatic area's outline is smoothed within that distance
//! before anything is digitized (`docs/src/design/image-to-embroidery.md` › Smoothing in StitchCraft).
//!
//! The pass works on edges, not on outlines one at a time, because the areas of a traced picture meet along
//! edges drawn twice, once in each area's outline: smoothed apart, the 2 copies would move apart and open a
//! sliver. So the outlines are cut into edges where they part or meet a third (their junctions), each edge is
//! smoothed once, and every area along it takes the same line. An edge that an area set off shares, or a shape
//! with settings, stays as drawn on both sides, so a shape a person drew keeps its outline unless asked.
//!
//! Within an edge, the corners stay where they are, the pieces between them are averaged by a Gaussian, and
//! each piece moves no farther than the setting, each ring's area by no more than a tenth, and no line comes to
//! cross another of its area: where one would, the edge is smoothed less, and at last stays as drawn. The pass
//! is off by default, since smoothing an outline changes which parts sew as satin columns, and not every change
//! is better.

use std::collections::{BTreeMap, BTreeSet};
use std::iter::once;

use stitchcraft_core::{Budget, Code, Diagnostic, Exhausted, Fix, Meter, Point};

use super::AutoParams;
use crate::design::{Design, Element, Path, Segment, Shape, Subpath};
use crate::normalize::stroke;

mod line;
#[cfg(test)]
mod tests;

/// How close to its smoothed line the fewest points of an edge draw it, in millimetres.
pub(super) const KEPT: f64 = 0.02;
/// The most a ring's area may change, as a share of its area: no speck or narrow part vanishes.
const AREA: f64 = 0.1;
/// What the deviation is multiplied by when a piece would move too far or a ring's area change too much.
const SHRINK: f64 = 0.7;
/// How many times the deviation shrinks before a piece, or an edge, stays as drawn.
const SHRINKS: u32 = 8;
/// How many times the deviation of an edge that would cross a line halves before it stays as drawn.
const HALVINGS: u32 = 8;
/// How far before and after a point its direction is measured to find a corner, in settings.
const REACH: f64 = 4.0;
/// How many samples a piece is resampled at in each setting's length.
const SAMPLES: f64 = 5.0;

/// A point as its exact coordinates, for finding the same point in 2 outlines. `-0.0` is `0.0`.
type At = (u64, u64);

/// The elements whose rings draw each segment, by its key ([`Step::key`]), as many times as they do.
type Owners = BTreeMap<Vec<At>, Vec<usize>>;

/// The exact coordinates of `p`.
fn at(p: Point) -> At {
    ((p.x() + 0.0).to_bits(), (p.y() + 0.0).to_bits())
}

/// The units a meter is charged for `count` things.
fn units(count: usize) -> u64 {
    u64::try_from(count).unwrap_or(u64::MAX)
}

/// `base` multiplied by itself `times` times.
fn times(base: f64, times: u32) -> f64 {
    (0..times).fold(1.0, |product, _| product * base)
}

/// A segment of a ring, from where the one before it ends.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Step {
    /// Where it starts.
    from: Point,
    /// The segment.
    segment: Segment,
}

impl Step {
    /// Its start, control points and end.
    fn points(self) -> impl Iterator<Item = Point> {
        once(self.from).chain(self.segment.points())
    }

    /// The same segment drawn the other way.
    fn reversed(self) -> Step {
        let segment = match self.segment {
            Segment::Line(_) => Segment::Line(self.from),
            Segment::Quad(control, _) => Segment::Quad(control, self.from),
            Segment::Cubic(first, second, _) => Segment::Cubic(second, first, self.from),
        };
        Step { from: self.segment.end(), segment }
    }

    /// The segment's points either way round, whichever way sorts first, and whether that is this way: the
    /// same for the segment in 2 outlines, whichever way each draws it.
    fn key(self) -> (Vec<At>, bool) {
        let forward: Vec<At> = self.points().map(at).collect();
        let backward: Vec<At> = forward.iter().rev().copied().collect();
        if backward < forward { (backward, false) } else { (forward, true) }
    }
}

/// The segments of `subpath` that move, as a ring: with the line that closes it, since a fill closes every
/// subpath. A unit of work for each segment.
fn steps(subpath: &Subpath, meter: &mut Meter) -> Result<Vec<Step>, Exhausted> {
    meter.charge(units(subpath.segments.len() + 1))?;
    let mut steps = Vec::with_capacity(subpath.segments.len() + 1);
    let mut from = subpath.start;
    for &segment in &subpath.segments {
        let step = Step { from, segment };
        if step.points().any(|p| p != from) {
            steps.push(step);
        }
        from = segment.end();
    }
    if from != subpath.start {
        steps.push(Step { from, segment: Segment::Line(subpath.start) });
    }
    Ok(steps)
}

/// A subpath of an area that may be smoothed, as a ring of steps.
struct Ring {
    /// The element's index in the design.
    element: usize,
    /// The subpath's index in the element's path.
    subpath: usize,
    /// Its steps.
    steps: Vec<Step>,
    /// Each step's key ([`Step::key`]).
    keys: Vec<Vec<At>>,
}

/// An edge: a run of segments between junctions that one or more rings share, or a whole ring.
struct Edge {
    /// The elements whose rings draw it, as many times as they do.
    owners: Vec<usize>,
    /// Whether it is a whole ring, with no junction.
    closed: bool,
    /// Its points as drawn, flattened, the way round it sorts first.
    drawn: Vec<Point>,
    /// How far it may move: the least setting of its areas; `None` when one of them is set off, has settings
    /// or ran out of work, and the edge stays as drawn.
    tolerance: Option<f64>,
    /// How many times a ring's area made its deviation shrink.
    shrinks: u32,
    /// How many times a crossing made its deviation halve.
    halvings: u32,
    /// Its smoothed line, the way round [`Edge::drawn`] goes; `None` while it stays as drawn.
    line: Option<Vec<Point>>,
}

impl Edge {
    /// The deviation it is smoothed with now, `None` once it stays as drawn.
    fn deviation(&self) -> Option<f64> {
        let tolerance = self.tolerance?;
        (self.shrinks <= SHRINKS && self.halvings <= HALVINGS).then(|| tolerance * times(SHRINK, self.shrinks) * times(0.5, self.halvings))
    }

    /// Its points now, as smoothed or as drawn, the way round a ring goes along it.
    fn oriented(&self, reversed: bool, drawn: bool) -> Vec<Point> {
        let points = self.line.as_ref().filter(|_| !drawn).unwrap_or(&self.drawn);
        match (reversed, self.closed) {
            (false, _) => points.clone(),
            (true, false) => points.iter().rev().copied().collect(),
            (true, true) => points.first().into_iter().chain(points.iter().skip(1).rev()).copied().collect(),
        }
    }
}

/// A ring's stretch along one edge.
struct Run {
    /// The edge's index.
    edge: usize,
    /// Whether the ring goes along it the other way round from [`Edge::drawn`].
    reversed: bool,
    /// The ring's steps along it, in the ring's order.
    steps: Vec<usize>,
}

/// Why an attempt stopped: the element whose work ran out, or the work of the shapes beside the areas.
#[derive(Debug)]
enum Ran {
    /// The work of the area at this index ran out.
    Out(usize),
    /// The work of finding what the other shapes share with the areas ran out.
    Rest,
}

/// `design` with the outline of each automatic area whose `auto_smooth_mm`, by `rules`, is more than 0 smoothed
/// (`REQ-AUTO-131` to `REQ-AUTO-134`), and `SC-I0921` in `said`; `None` when no area asks for it or none
/// changed. Each area has `budget`'s work to itself, and charges for each edge it shares. An area whose
/// work runs out keeps its outline, and its edges stay as drawn for the areas beside it too.
pub(super) fn outlines(design: &Design, rules: &[Option<AutoParams>], budget: &Budget, said: &mut Vec<Diagnostic>) -> Option<Design> {
    let settings: BTreeMap<usize, f64> = design
        .elements()
        .iter()
        .zip(rules)
        .enumerate()
        .filter_map(|(index, (element, rules))| {
            let setting = rules.as_ref()?.auto_smooth_mm.get();
            (element.automatic && matches!(element.shape, Shape::Fill { .. }) && setting > 0.0).then_some((index, setting))
        })
        .collect();
    if settings.is_empty() {
        return None;
    }
    // An area whose work runs out is left as drawn, and the others are smoothed again without it. Each attempt
    // freezes one more area, or all of them, so this ends.
    let mut frozen = BTreeSet::new();
    let paths = loop {
        match attempt(design, &settings, &frozen, budget) {
            Ok(paths) => break paths,
            Err(Ran::Out(index)) if !frozen.contains(&index) => {
                frozen.insert(index);
            }
            Err(_) => frozen = settings.keys().copied().collect(),
        }
    };
    let elements = design.elements();
    let count = |path: &Path| path.subpaths.iter().map(|subpath| subpath.segments.len() + 1).sum::<usize>();
    let (mut before, mut after) = (0, 0);
    for index in settings.keys().filter(|index| !frozen.contains(index)) {
        let Some(Shape::Fill { path, .. }) = elements.get(*index).map(|element| &element.shape) else { continue };
        before += count(path);
        after += paths.get(index).map_or_else(|| count(path), count);
    }
    said.push(notice(&settings, &frozen, (before, after), elements));
    if paths.is_empty() {
        return None;
    }
    let mut elements = elements.to_vec();
    for (index, smoothed) in paths {
        if let Some(Element { shape: Shape::Fill { path, .. }, .. }) = elements.get_mut(index) {
            *path = smoothed;
        }
    }
    // Every point of a smoothed outline lies within the setting of its outline as drawn, which the design
    // checked, so an error is a bug.
    design.with_elements(elements).map_err(|error| said.push(error)).ok()
}

/// What `SC-I0921` says of the areas of `elements` with `settings`, those of them `frozen`, which ran out of
/// work, and the outline points the others had and have.
fn notice(settings: &BTreeMap<usize, f64>, frozen: &BTreeSet<usize>, (before, after): (usize, usize), elements: &[Element]) -> Diagnostic {
    let smoothed: Vec<f64> = settings.iter().filter(|(index, _)| !frozen.contains(index)).map(|(_, &setting)| setting).collect();
    let (least, most) = smoothed.iter().fold((f64::INFINITY, 0.0_f64), |(least, most), &s| (least.min(s), most.max(s)));
    let within = if least < most { format!("{least} to {most}") } else { format!("{most}") };
    let mut message = match smoothed.len() {
        0 => String::new(),
        1 => format!("The outline of 1 automatic area was smoothed within {within} mm: it is drawn with {after} points, where it had {before}."),
        n => format!(
            "The outlines of {n} automatic areas were smoothed within {within} mm: they are drawn with {after} points, where they had {before}."
        ),
    };
    let names: Vec<String> = frozen.iter().filter_map(|index| elements.get(*index)).map(|element| format!("`{}`", element.id)).collect();
    if let Some((last, rest)) = names.split_last() {
        let (who, keep) = if rest.is_empty() {
            (format!("The area {last}"), "keeps its outline")
        } else {
            (format!("The areas {} and {last}", rest.join(", ")), "keep their outlines")
        };
        let space = if message.is_empty() { "" } else { " " };
        message.push_str(&format!("{space}{who} needed more work to smooth than the budget allows, and {keep}."));
    }
    let fix = "Smoothing changes which parts sew as satin columns. To sew the outlines as drawn, set auto_smooth_mm to 0.";
    Diagnostic::new(Code::AutoSmoothed, message).with_fix(Fix::Hint(fix.to_string()))
}

/// The work of `owners` among `meters`, those of them still smoothed, charged for `work`: it may spend what the
/// poorest of them has left, and each is then charged what it spent.
fn charged<T>(meters: &mut BTreeMap<usize, Meter>, owners: &[usize], work: impl FnOnce(&mut Meter) -> Result<T, Exhausted>) -> Result<T, Ran> {
    let owners: BTreeSet<usize> = owners.iter().copied().collect();
    let (poorest, left) =
        owners.iter().filter_map(|index| Some((*index, meters.get(index)?.work_left()))).min_by_key(|&(_, left)| left).ok_or(Ran::Rest)?;
    let mut scratch = Budget { max_work: left, max_stitches: 0 }.meter();
    let done = work(&mut scratch).map_err(|_| Ran::Out(poorest))?;
    let spent = left.saturating_sub(scratch.work_left());
    for index in owners {
        if let Some(meter) = meters.get_mut(&index) {
            meter.charge(spent).map_err(|_| Ran::Out(index))?;
        }
    }
    Ok(done)
}

/// The rings of the areas with `settings`, and the elements whose rings draw each segment, the fills beside
/// them too, as many times as they do, charged to each area's meter among `meters`, or else to `rest`.
fn rings(
    design: &Design,
    settings: &BTreeMap<usize, f64>,
    meters: &mut BTreeMap<usize, Meter>,
    rest: &mut Meter,
) -> Result<(Vec<Ring>, Owners), Ran> {
    let elements = design.elements();
    let mut rings = Vec::new();
    let mut owners: Owners = BTreeMap::new();
    for &element in settings.keys() {
        let Some(Shape::Fill { path, .. }) = elements.get(element).map(|element| &element.shape) else { continue };
        // A frozen area's segments are found on the work of the shapes beside the areas.
        let ran = if meters.contains_key(&element) { Ran::Out(element) } else { Ran::Rest };
        let meter = match meters.get_mut(&element) {
            Some(meter) => meter,
            None => &mut *rest,
        };
        for (subpath, drawn) in path.subpaths.iter().enumerate() {
            let Ok(steps) = steps(drawn, meter) else { return Err(ran) };
            let keys: Vec<Vec<At>> = steps.iter().map(|step| step.key().0).collect();
            for key in &keys {
                owners.entry(key.clone()).or_default().push(element);
            }
            rings.push(Ring { element, subpath, steps, keys });
        }
    }
    // The fills beside them: an edge they share with an area stays as drawn.
    for (element, shape) in elements.iter().map(|element| &element.shape).enumerate().filter(|(index, _)| !settings.contains_key(index)) {
        let Shape::Fill { path, .. } = shape else { continue };
        for subpath in &path.subpaths {
            for step in steps(subpath, rest).map_err(|_| Ran::Rest)? {
                if let Some(owners) = owners.get_mut(&step.key().0) {
                    owners.push(element);
                }
            }
        }
    }
    Ok((rings, owners))
}

/// The steps of `ring` cut into its stretches along edges, each with whether it is the whole ring: at each
/// point where 3 or more segments meet, or where the elements that draw the segment before and after it
/// differ. A unit of work for each step.
fn stretches(ring: &Ring, owners: &Owners, degree: &BTreeMap<At, usize>, meter: &mut Meter) -> Result<Vec<(Vec<usize>, bool)>, Exhausted> {
    let n = ring.steps.len();
    meter.charge(units(n))?;
    let owners_of = |i: usize| ring.keys.get(i).and_then(|key| owners.get(key));
    let fixed: Vec<usize> = (0..n)
        .filter(|&i| {
            let meeting = ring.steps.get(i).and_then(|step| degree.get(&at(step.from))).copied().unwrap_or(0);
            meeting != 2 || owners_of((i + n - 1) % n) != owners_of(i)
        })
        .collect();
    if fixed.is_empty() {
        return Ok(vec![((0..n).collect(), true)]);
    }
    let next = fixed.iter().skip(1).chain(fixed.first());
    Ok(fixed
        .iter()
        .zip(next)
        .map(|(&from, &to)| {
            let length = (to + n - from - 1) % n + 1;
            ((0..length).map(|k| (from + k) % n).collect(), false)
        })
        .collect())
}

/// The steps `stretch` of `ring` the way round that sorts first, by their points, and whether that is the other
/// way round from the ring: for a whole ring, from its point that sorts first. The same for the edge in every
/// ring that goes along it, so that each takes the same line.
fn canonical(ring: &Ring, stretch: &[usize], closed: bool) -> (Vec<Step>, bool) {
    let steps: Vec<Step> = stretch.iter().filter_map(|&i| ring.steps.get(i).copied()).collect();
    let n = steps.len();
    let anchor = |i: usize| steps.get(i % n.max(1)).map(|step| at(step.from));
    if !closed {
        let forward: Vec<At> = steps.first().map(|step| step.from).into_iter().chain(steps.iter().map(|step| step.segment.end())).map(at).collect();
        let backward: Vec<At> = forward.iter().rev().copied().collect();
        if backward < forward {
            return (steps.iter().rev().map(|step| step.reversed()).collect(), true);
        }
        return (steps, false);
    }
    let first = (0..n).min_by_key(|&i| anchor(i)).unwrap_or(0);
    let forward: Vec<Option<At>> = (0..n).map(|k| anchor(first + k)).collect();
    let backward: Vec<Option<At>> = (0..n).map(|k| anchor(first + n - k)).collect();
    if backward < forward {
        let reversed = (0..n).filter_map(|k| steps.get((first + 2 * n - 1 - k) % n)).map(|step| step.reversed());
        return (reversed.collect(), true);
    }
    ((0..n).filter_map(|k| steps.get((first + k) % n)).copied().collect(), false)
}

/// The points of `steps` flattened, each segment flattened the way round it sorts first, so that the same edge
/// flattens to the same points in every ring. A whole ring lists each point once. A unit of work for each
/// segment and each halving of a curve.
fn flattened(steps: &[Step], closed: bool, meter: &mut Meter) -> Result<Vec<Point>, Exhausted> {
    let mut points: Vec<Point> = Vec::new();
    for &step in steps {
        let (_, this_way) = step.key();
        let sorted = if this_way { step } else { step.reversed() };
        let mut flat = match sorted.segment {
            Segment::Line(end) => {
                meter.charge(1)?;
                vec![sorted.from, end]
            }
            segment => stroke::flattened(&Subpath { start: sorted.from, segments: vec![segment], closed: false }, meter)?,
        };
        if !this_way {
            flat.reverse();
        }
        for p in flat {
            if points.last() != Some(&p) {
                points.push(p);
            }
        }
    }
    if closed && points.len() > 1 && points.first() == points.last() {
        points.pop();
    }
    Ok(points)
}

/// How many samples a line `length` millimetres long is resampled at with `tolerance`.
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss, reason = "a finite count, at least 1, capped below usize's limit")]
fn samples(length: f64, tolerance: f64) -> Option<usize> {
    let count = (length / (tolerance / SAMPLES)).ceil();
    (1.0..1e12).contains(&count).then_some(count as usize)
}

/// The piece of a line `drawn` smoothed with `deviation` millimetres, moved no farther than `tolerance`: the
/// deviation shrinks by [`SHRINK`] up to [`SHRINKS`] times, and then the piece stays as drawn.
fn piece(drawn: &[Point], closed: bool, tolerance: f64, deviation: f64, meter: &mut Meter) -> Result<Vec<Point>, Exhausted> {
    let length: f64 = line::length(drawn, closed);
    let Some(steps) = samples(length, tolerance).filter(|&steps| !closed || steps >= 3) else { return Ok(drawn.to_vec()) };
    let Some(resampled) = line::resampled(drawn, closed, steps, meter)? else { return Ok(drawn.to_vec()) };
    let step = length / steps as f64;
    let mut deviation = deviation;
    for _ in 0..=SHRINKS {
        if let Some(blurred) = line::blurred(&resampled, closed, deviation / step, meter)?
            && line::apart(drawn, &blurred, closed, REACH * tolerance, meter)? <= tolerance
        {
            return Ok(blurred);
        }
        deviation *= SHRINK;
    }
    Ok(drawn.to_vec())
}

/// The edge drawn as `drawn` smoothed within `tolerance` with `deviation` ([`piece`]), its corners fixed, and
/// drawn with the fewest of its points within [`KEPT`] of it. An edge that is a whole ring without corners is
/// smoothed round, and starts where it did; one with corners starts at its first.
fn smoothed(drawn: &[Point], closed: bool, tolerance: f64, deviation: f64, meter: &mut Meter) -> Result<Vec<Point>, Exhausted> {
    let corners = line::corners(drawn, closed, REACH * tolerance, meter)?;
    let n = drawn.len();
    if closed && corners.is_empty() {
        let round = piece(drawn, true, tolerance, deviation, meter)?;
        return line::fewest_round(&round, KEPT, meter);
    }
    let cuts: Vec<usize> = if closed {
        corners.iter().copied().chain(corners.first().map(|first| first + n)).collect()
    } else {
        once(0).chain(corners).chain(once(n.saturating_sub(1))).collect()
    };
    let mut line: Vec<Point> = Vec::new();
    for pair in cuts.windows(2) {
        let (Some(&from), Some(&to)) = (pair.first(), pair.get(1)) else { continue };
        let points: Vec<Point> = (from..=to).filter_map(|i| drawn.get(i % n.max(1))).copied().collect();
        let smooth = piece(&points, false, tolerance, deviation, meter)?;
        let kept = line::fewest(&smooth, KEPT, meter)?;
        let skip = usize::from(!line.is_empty());
        line.extend(kept.into_iter().skip(skip));
    }
    if closed {
        line.pop();
    }
    Ok(line)
}

/// One attempt at smoothing the areas with `settings`, but those `frozen`: each area's smoothed path, for
/// those whose outline changed, or why the work ran out.
fn attempt(design: &Design, settings: &BTreeMap<usize, f64>, frozen: &BTreeSet<usize>, budget: &Budget) -> Result<BTreeMap<usize, Path>, Ran> {
    let active: BTreeMap<usize, f64> = settings.iter().filter(|(index, _)| !frozen.contains(index)).map(|(&i, &s)| (i, s)).collect();
    if active.is_empty() {
        return Ok(BTreeMap::new());
    }
    let mut meters: BTreeMap<usize, Meter> = active.keys().map(|&index| (index, budget.meter())).collect();
    let mut rest = budget.meter();
    let (rings, owners) = rings(design, settings, &mut meters, &mut rest)?;
    rest.charge(units(owners.len())).map_err(|_| Ran::Rest)?;
    let mut degree: BTreeMap<At, usize> = BTreeMap::new();
    for key in owners.keys() {
        for end in [key.first(), key.last()].into_iter().flatten() {
            *degree.entry(*end).or_default() += 1;
        }
    }
    let mut pass = Pass { edges: Vec::new(), runs: BTreeMap::new(), meters };
    let mut ids: BTreeMap<Vec<At>, usize> = BTreeMap::new();
    for (index, ring) in rings.iter().enumerate().filter(|(_, ring)| active.contains_key(&ring.element)) {
        let meter = pass.meters.get_mut(&ring.element).ok_or(Ran::Out(ring.element))?;
        let mut runs = Vec::new();
        for (stretch, closed) in stretches(ring, &owners, &degree, meter).map_err(|_| Ran::Out(ring.element))? {
            let (steps, reversed) = canonical(ring, &stretch, closed);
            let Some(id) = stretch.iter().filter_map(|&i| ring.keys.get(i)).min().cloned() else { continue };
            let edge = match ids.get(&id) {
                Some(&edge) => edge,
                None => {
                    let drawn = flattened(&steps, closed, meter).map_err(|_| Ran::Out(ring.element))?;
                    let edge_owners = owners.get(&id).cloned().unwrap_or_default();
                    let tolerance =
                        edge_owners.iter().map(|owner| active.get(owner).copied()).try_fold(f64::INFINITY, |least, s| Some(least.min(s?)));
                    pass.edges.push(Edge { owners: edge_owners, closed, drawn, tolerance, shrinks: 0, halvings: 0, line: None });
                    ids.insert(id, pass.edges.len() - 1);
                    pass.edges.len() - 1
                }
            };
            runs.push(Run { edge, reversed, steps: stretch });
        }
        pass.runs.insert(index, runs);
    }
    let all: BTreeSet<usize> = (0..pass.edges.len()).collect();
    pass.smooth(&all)?;
    pass.settle(&rings)?;
    Ok(pass.paths(design, &rings))
}

/// The edges of one attempt, the rings' runs along them, and the areas' meters.
struct Pass {
    /// The edges.
    edges: Vec<Edge>,
    /// Each smoothed ring's runs, by the ring's index.
    runs: BTreeMap<usize, Vec<Run>>,
    /// Each smoothed area's meter, by its index in the design.
    meters: BTreeMap<usize, Meter>,
}

impl Pass {
    /// Smooths the edges `which` with their deviations now.
    fn smooth(&mut self, which: &BTreeSet<usize>) -> Result<(), Ran> {
        for &index in which {
            let Some(edge) = self.edges.get(index) else { continue };
            let (Some(tolerance), deviation) = (edge.tolerance, edge.deviation()) else { continue };
            let line = match deviation {
                Some(deviation) => {
                    let (drawn, closed) = (&edge.drawn, edge.closed);
                    let line = charged(&mut self.meters, &edge.owners, |meter| smoothed(drawn, closed, tolerance, deviation, meter))?;
                    // A line that smoothing leaves as it was, a straight side, stays as drawn, curves and all.
                    (line != *drawn).then_some(line)
                }
                None => None,
            };
            if let Some(edge) = self.edges.get_mut(index) {
                edge.line = line;
            }
        }
        Ok(())
    }

    /// The points of the ring whose runs are `runs` now, or as drawn.
    fn ring(&self, runs: &[Run], drawn: bool) -> Vec<Point> {
        let mut points: Vec<Point> = Vec::new();
        for run in runs {
            let Some(edge) = self.edges.get(run.edge) else { continue };
            let skip = usize::from(!points.is_empty());
            points.extend(edge.oriented(run.reversed, drawn).into_iter().skip(skip));
        }
        if points.len() > 1 && points.first() == points.last() {
            points.pop();
        }
        points
    }

    /// Smooths the edges less until no ring's area changes by more than [`AREA`] and no line crosses another of
    /// its area (`REQ-AUTO-134`). Each round shrinks or halves at least one edge's deviation, or stops.
    fn settle(&mut self, rings: &[Ring]) -> Result<(), Ran> {
        loop {
            let mut dirty: BTreeSet<usize> = BTreeSet::new();
            for (&index, runs) in &self.runs {
                let Some(ring) = rings.get(index) else { continue };
                let (now, drawn) = (self.ring(runs, false), self.ring(runs, true));
                if let Some(meter) = self.meters.get_mut(&ring.element) {
                    meter.charge(units(now.len() + drawn.len())).map_err(|_| Ran::Out(ring.element))?;
                }
                let (now, drawn) = (line::area(&now), line::area(&drawn));
                if (now - drawn).abs() > AREA * drawn.abs() {
                    dirty.extend(runs.iter().map(|run| run.edge).filter(|&edge| self.edges.get(edge).is_some_and(|edge| edge.line.is_some())));
                }
            }
            for &edge in &dirty {
                if let Some(edge) = self.edges.get_mut(edge) {
                    edge.shrinks += 1;
                }
            }
            let crossing = self.crossing(rings)?;
            for &edge in &crossing {
                if let Some(edge) = self.edges.get_mut(edge) {
                    edge.halvings += 1;
                }
            }
            dirty.extend(crossing);
            if dirty.is_empty() {
                return Ok(());
            }
            self.smooth(&dirty)?;
        }
    }

    /// The smoothed edges that cross a line of an area along them, or themselves.
    fn crossing(&mut self, rings: &[Ring]) -> Result<BTreeSet<usize>, Ran> {
        let mut by_area: BTreeMap<usize, BTreeSet<usize>> = BTreeMap::new();
        for (&index, runs) in &self.runs {
            let Some(ring) = rings.get(index) else { continue };
            by_area.entry(ring.element).or_default().extend(runs.iter().map(|run| run.edge));
        }
        let mut crossing = BTreeSet::new();
        for (element, edges) in by_area {
            let edges: Vec<usize> = edges.into_iter().collect();
            let lines: Vec<(Vec<(Point, Point)>, bool)> = edges
                .iter()
                .filter_map(|&edge| self.edges.get(edge))
                .map(|edge| {
                    let points = edge.oriented(false, false);
                    let close = points.last().copied().zip(points.first().copied()).filter(|_| edge.closed);
                    let pairs = points.windows(2).filter_map(|pair| Some((*pair.first()?, *pair.get(1)?))).chain(close).collect();
                    (pairs, edge.line.is_some())
                })
                .collect();
            let Some(meter) = self.meters.get_mut(&element) else { continue };
            let found = line::crossing(&lines, meter).map_err(|_| Ran::Out(element))?;
            crossing.extend(
                found
                    .into_iter()
                    .filter_map(|line| edges.get(line).copied())
                    .filter(|&edge| self.edges.get(edge).is_some_and(|edge| edge.line.is_some())),
            );
        }
        Ok(crossing)
    }

    /// Each smoothed area's path, for those of `design` whose outline changed: a ring along a smoothed edge
    /// drawn with lines through its points, the rest of each ring as drawn.
    fn paths(&self, design: &Design, rings: &[Ring]) -> BTreeMap<usize, Path> {
        let mut paths: BTreeMap<usize, Path> = BTreeMap::new();
        for (&index, runs) in &self.runs {
            let Some(ring) = rings.get(index) else { continue };
            if !runs.iter().any(|run| self.edges.get(run.edge).is_some_and(|edge| edge.line.is_some())) {
                continue;
            }
            let Some(Shape::Fill { path, .. }) = design.elements().get(ring.element).map(|element| &element.shape) else { continue };
            let Some(subpath) = self.subpath(ring, runs) else { continue };
            let path = paths.entry(ring.element).or_insert_with(|| path.clone());
            if let Some(slot) = path.subpaths.get_mut(ring.subpath) {
                *slot = subpath;
            }
        }
        paths
    }

    /// The subpath of `ring` along its `runs` now.
    fn subpath(&self, ring: &Ring, runs: &[Run]) -> Option<Subpath> {
        let mut start = None;
        let mut segments = Vec::new();
        for run in runs {
            let edge = self.edges.get(run.edge)?;
            if edge.line.is_none() {
                for step in run.steps.iter().filter_map(|&i| ring.steps.get(i)) {
                    start.get_or_insert(step.from);
                    segments.push(step.segment);
                }
                continue;
            }
            let points = edge.oriented(run.reversed, false);
            let (first, rest) = points.split_first()?;
            start.get_or_insert(*first);
            segments.extend(rest.iter().map(|&p| Segment::Line(p)));
            if edge.closed {
                segments.push(Segment::Line(*first));
            }
        }
        Some(Subpath { start: start?, segments, closed: true })
    }
}
