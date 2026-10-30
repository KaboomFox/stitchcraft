//! The order the elements are sewn in (`REQ-AUTO-010`, `REQ-AUTO-011`), said with `SC-I0907`.
//!
//! Sewn in paint order, a traced picture changes thread far more often than it needs to. VectorCraft's Auto
//! Grouping draws each shape together with the shapes inside it, so a row of badges goes red, blue, gold,
//! red, blue, gold, and on a home machine each change of thread is a stop to rethread it. So the elements
//! are put in an order of their own, by 4 rules:
//!
//! 1. **What overlaps keeps its paint order.** Of 2 shapes whose paint overlaps, the lower sews first:
//!    sewn the other way, it would cover the upper one. An area paints its region, a satin column the area
//!    between its rails as they are sewn, and a line the area its stroke paints as drawn. Shapes that only
//!    meet, as abutting paths do, may sew in either order. Rounding leaves a sliver no wider than
//!    [`TOUCHING`] between edges drawn as one, and shapes overlap only where their overlap is wider.
//! 2. **A line after the fill it borders.** A satin column or a line that touches a fill without overlapping
//!    it sews after the fill and covers its edge, unless shapes that overlap hold it before: rule 1 comes
//!    first.
//! 3. **Few thread changes.** The next element has the thread that is sewing, as long as an element of that
//!    thread is free to sew, every element that must sew before it sewn. Otherwise the thread changes to
//!    that of the first free element in paint order. Threads are the same when their colours are, as plan
//!    assembly counts thread changes.
//! 4. **The nearest next.** Of the free elements of that thread, the next is the one whose box lies nearest
//!    the box of the one sewn last, then the one whose box's centre does, then the one drawn first. Each
//!    element then starts and ends at its points nearest its neighbours (`REQ-GEN-003`).
//!
//! The elements with settings keep their order among themselves, and the automatic ones move round them,
//! unless the design asks for every element ordered (`--sort-colours`, `Design::sort_colours`). The parts
//! of a shape sewn as several elements keep their order too ([`Moving::shape`]). Nothing
//! moves across an element with a stop after it, so a stop comes where it was drawn: the elements drawn
//! before it sew before it, and those drawn after it after it.
//!
//! Shapes are near when their boxes come within [`TOUCHING`] of each other: an automatic area's box is round
//! its region, and any other shape's round its path's points, grown for a stroke as far as its corners and
//! caps reach ([`drawn`]). What a shape paints is found only when it is near an element that moves, and an
//! element whose paint can't be found keeps its place in paint order beside the shapes near it.
//!
//! **Work.** Each element has the budget's work to itself to find how it lies beside the shapes drawn after
//! it, and costs a unit for each of them and the work of those near it. One that needs more keeps its place
//! in paint order as a stop does: nothing moves across it. Choosing the order from what was found has the
//! budget's work once more. An order that needs more sews the elements not placed by then in paint order,
//! after the others. Both are said.

use std::collections::BTreeSet;
use std::f64::consts::SQRT_2;

use stitchcraft_core::{Budget, Code, Diagnostic, ElementId, Exhausted, Meter, Point};
use stitchcraft_plan::Rgb;

use super::{Failed, Kind, Shapes, TOUCHING, envelope, joined, near, no_area};
use crate::design::{Element, Join, Shape};
use crate::generate::stops_after;
use crate::normalize::region::Polygon;
use crate::normalize::region::geom::Envelope;
use crate::normalize::region::ops::{area_of, buffered, intersection};

/// What becomes of an element whose neighbours need more work to find than the budget allows: it keeps its
/// place, as an element with a stop after it does.
const PLACE: &str = "it keeps its place in paint order: the elements drawn before it sew before it, and those drawn after it after it";

/// How an element may move round the others.
#[derive(Clone, Copy, Debug)]
pub(super) struct Moving {
    /// Whether it may: an automatic element, or any element of a design that asks for every element ordered.
    pub(super) free: bool,
    /// The place in the design of the shape it is a part of: the parts of a shape sewn as several elements,
    /// such as a line's satin columns, keep their order, so that a trim or a stop after the shape still comes
    /// after its last part.
    pub(super) shape: usize,
}

/// `shapes` put in the order they sew in, each element moving round the others as `moving` says, and what
/// is said of it: `SC-I0907` when the order isn't paint order, and for each element left in paint order.
pub(super) fn ordered(shapes: &mut Shapes, moving: &[Moving], budget: &Budget) -> Vec<Diagnostic> {
    let live: Vec<usize> = (0..shapes.len()).filter(|&index| !shapes.left_out(index)).collect();
    if !live.iter().any(|&index| moving.get(index).is_some_and(|moving| moving.free)) {
        return Vec::new();
    }
    let reaches: Vec<Option<Envelope>> = (0..shapes.len()).map(|index| reach(shapes, index)).collect();
    let facts: Vec<Facts> = shapes.elements.iter().zip(reaches).map(|(element, reach)| Facts::of(element, reach)).collect();
    let mut order = Order { facts: &facts, moving, graph: Graph::new(shapes.len()), borders: Vec::new(), kept: BTreeSet::new() };
    let stop = |index: usize| facts.get(index).is_some_and(|facts| facts.stop);
    let mut said = Vec::new();
    for run in runs(&live, stop) {
        order.relate(shapes, &run, budget, &mut said);
    }
    // Nothing moves across an element whose neighbours needed more work than the budget allows either.
    let runs = runs(&live, |index| stop(index) || order.kept.contains(&index));
    let mut sewing = Sewing::default();
    let spent = order.sew(&runs, &mut sewing, &mut budget.meter()).is_err();
    // What wasn't placed sews in paint order after the rest: what the work left, and so no element is lost.
    let placed: BTreeSet<usize> = sewing.order.iter().copied().collect();
    let rest: Vec<usize> = live.iter().copied().filter(|index| !placed.contains(index)).collect();
    if spent {
        said.extend(rest.first().and_then(|&index| shapes.id(index)).map(|id| unplaced(budget.max_work, id)));
    }
    sewing.order.extend(rest);
    if sewing.order != live {
        said.push(summary(changes(&live, &facts), changes(&sewing.order, &facts)));
    }
    // The elements left out go last, where they are left out.
    sewing.order.extend((0..shapes.len()).filter(|&index| shapes.left_out(index)));
    shapes.reorder(&sewing.order);
    said
}

/// What the order needs to know of an element.
struct Facts {
    /// Its thread's colour.
    colour: Rgb,
    /// Whether it has a stop after it, which nothing moves across.
    stop: bool,
    /// The box round it ([`reach`]); `None` for a path of no points.
    reach: Option<Envelope>,
}

impl Facts {
    /// What the order needs to know of `element`, whose box is `reach`.
    fn of(element: &Element, reach: Option<Envelope>) -> Facts {
        Facts { colour: element.thread.color, stop: stops_after(element), reach }
    }
}

/// The box round element `index` of `shapes`, found without work: round an automatic area's region as it now
/// is, its specks merged, round the part a part's line or column sews along, and round any other shape's path
/// ([`drawn`]), whose paint is found only for the shapes near an element that moves.
fn reach(shapes: &Shapes, index: usize) -> Option<Envelope> {
    let path = shapes.elements.get(index).and_then(drawn);
    shapes.known(index).and_then(envelope).or(path)
}

/// The box round the points of `element`'s path, which hold its curves, grown for a stroke by half its
/// width times as much as its corners and caps reach: round what it paints. `None` for no points. A miter
/// limited to `L` reaches `L` times half the width from its corner, and clipped there, as a satin column's
/// rails clip it, its corners up to √(L² + 1) times. Any other corner or cap reaches no further than a square
/// cap's corner, √2 times, which a miter limited to 1 reaches too.
fn drawn(element: &Element) -> Option<Envelope> {
    let points: Vec<Point> = element.shape.path().points().collect();
    let grown = match element.shape {
        Shape::Stroke { width, join: Join::Miter { limit }, .. } => width.get() / 2.0 * (limit * limit + 1.0).sqrt(),
        Shape::Stroke { width, .. } => width.get() / 2.0 * SQRT_2,
        Shape::Fill { .. } => 0.0,
    };
    (!points.is_empty()).then(|| {
        let around = Envelope::of(&points);
        Envelope { min_x: around.min_x - grown, min_y: around.min_y - grown, max_x: around.max_x + grown, max_y: around.max_y + grown }
    })
}

/// `live`, the elements not left out, in paint order, cut into runs that each sew in an order of their own:
/// an element that `cut` says nothing moves across, as nothing moves across a stop, is a run of its own.
fn runs(live: &[usize], cut: impl Fn(usize) -> bool) -> Vec<Vec<usize>> {
    let mut runs = vec![Vec::new()];
    for &index in live {
        if cut(index) {
            runs.extend([vec![index], Vec::new()]);
        } else if let Some(run) = runs.last_mut() {
            run.push(index);
        }
    }
    runs.retain(|run| !run.is_empty());
    runs
}

/// The order as it is found.
struct Order<'o> {
    /// What it needs to know of each element.
    facts: &'o [Facts],
    /// How each element may move round the others.
    moving: &'o [Moving],
    /// What must sew before what.
    graph: Graph,
    /// Each line that borders a fill, as the fill and the line, in the order they were found.
    borders: Vec<(usize, usize)>,
    /// The elements whose neighbours need more work to find than the budget allows, which keep their place.
    kept: BTreeSet<usize>,
}

/// How a shape lies beside one drawn before it.
#[derive(Debug, PartialEq)]
enum Beside {
    /// It overlaps that shape, or what one of them paints can't be found: it sews after it.
    After(usize),
    /// A line borders a fill: the fill, then the line. The fill sews first unless what overlaps holds the
    /// line before it.
    Border(usize, usize),
}

impl Order<'_> {
    /// Whether element `index` may move.
    fn movable(&self, index: usize) -> bool {
        self.moving.get(index).is_some_and(|moving| moving.free)
    }

    /// Whether elements `a` and `b` are parts of one shape.
    fn parts(&self, a: usize, b: usize) -> bool {
        let shape = |index: usize| self.moving.get(index).map(|moving| moving.shape);
        shape(a) == shape(b)
    }

    /// Whether elements `a` and `b` may sew out of paint order, one of them moving, and their boxes come
    /// within [`TOUCHING`] of each other, so that what they paint may meet.
    fn near(&self, a: usize, b: usize) -> bool {
        let reach = |index: usize| self.facts.get(index).and_then(|facts| facts.reach);
        (self.movable(a) || self.movable(b)) && reach(a).zip(reach(b)).is_some_and(|(a, b)| near(&a, TOUCHING, &b))
    }

    /// Finds how the elements of `run` lie beside each other, each with `budget`'s work to itself, and what
    /// must sew before what: the elements with settings in their order, the parts of a shape in theirs, and
    /// of 2 that overlap the one drawn first. An element whose neighbours need more work is [`Order::kept`]
    /// in its place, which is said in `said`.
    fn relate(&mut self, shapes: &mut Shapes, run: &[usize], budget: &Budget, said: &mut Vec<Diagnostic>) {
        let pairs = |items: &[usize]| items.iter().copied().zip(items.iter().copied().skip(1)).collect::<Vec<_>>();
        let fixed: Vec<usize> = run.iter().copied().filter(|&index| !self.movable(index)).collect();
        let parts: Vec<(usize, usize)> = pairs(run).into_iter().filter(|&(a, b)| self.parts(a, b)).collect();
        for (from, to) in pairs(&fixed).into_iter().chain(parts) {
            self.graph.add(from, to);
        }
        for (at, &index) in run.iter().enumerate() {
            let later = run.get(at + 1..).unwrap_or_default();
            match self.beside(shapes, index, later, &mut budget.meter()) {
                Ok(found) => {
                    for beside in found {
                        match beside {
                            Beside::After(other) => self.graph.add(index, other),
                            Beside::Border(fill, line) => self.borders.push((fill, line)),
                        }
                    }
                }
                Err(failed) => {
                    self.kept.insert(index);
                    let finding = "where this element meets the shapes drawn after it";
                    said.extend(shapes.id(index).map(|id| failed.said(Code::AutoOrder, finding, PLACE, budget.max_work, id)));
                }
            }
        }
    }

    /// How each of `later`, the elements drawn after `index` in its run, lies beside it, with `meter`'s work:
    /// a unit for each, and the work of those near it.
    fn beside(&self, shapes: &mut Shapes, index: usize, later: &[usize], meter: &mut Meter) -> Result<Vec<Beside>, Failed> {
        let mut found = Vec::new();
        for &other in later {
            meter.charge(1)?;
            if self.near(index, other) {
                found.extend(lie(shapes, index, other, meter)?);
            }
        }
        Ok(found)
    }

    /// Puts the elements of `runs` in the order they sew, after those `sewing` holds, with `meter`'s work:
    /// first each line after the fills it borders, where what overlaps allows it and no element kept in its
    /// place lies between them, then each run in turn.
    fn sew(&mut self, runs: &[Vec<usize>], sewing: &mut Sewing, meter: &mut Meter) -> Result<(), Exhausted> {
        for (fill, line) in std::mem::take(&mut self.borders) {
            if self.kept.range(fill.min(line)..=fill.max(line)).next().is_none() && !self.graph.reaches(line, fill, meter)? {
                self.graph.add(fill, line);
            }
        }
        for run in runs {
            self.run(run, sewing, meter)?;
        }
        Ok(())
    }

    /// Puts the elements of `run` after those `sewing` holds, with `meter`'s work: of the elements free to
    /// sew, those of the thread sewing, or else those of the first one's thread, and of those the one
    /// nearest the last sewn.
    fn run(&mut self, run: &[usize], sewing: &mut Sewing, meter: &mut Meter) -> Result<(), Exhausted> {
        let mut free: BTreeSet<usize> = run.iter().copied().filter(|&index| self.graph.waiting(index) == 0).collect();
        while let Some(&first) = free.first() {
            let colour = |index: usize| self.facts.get(index).map(|facts| facts.colour);
            let (mut same, mut changed) = (Vec::new(), Vec::new());
            for &index in &free {
                meter.charge(1)?;
                if colour(index) == sewing.thread {
                    same.push(index);
                }
                if colour(index) == colour(first) {
                    changed.push(index);
                }
            }
            if same.is_empty() {
                (same, sewing.thread) = (changed, colour(first));
            }
            let Some(next) = self.nearest(&same, sewing.last) else { break };
            free.remove(&next);
            free.extend(self.graph.release(next));
            sewing.order.push(next);
            sewing.last = Some(next);
        }
        Ok(())
    }

    /// Of `pool`, the element whose box lies nearest the box of `last`, then whose box's centre does, then
    /// the one drawn first; the one drawn first when nothing was sewn before, or a box is missing.
    fn nearest(&self, pool: &[usize], last: Option<usize>) -> Option<usize> {
        let reach = |index: usize| self.facts.get(index).and_then(|facts| facts.reach);
        let from = last.and_then(reach);
        let key = |index: usize| from.zip(reach(index)).map_or([f64::INFINITY; 2], |(from, to)| [gap(&from, &to), apart(&from, &to)]);
        pool.iter().copied().min_by(|&a, &b| {
            let ([gap_a, apart_a], [gap_b, apart_b]) = (key(a), key(b));
            gap_a.total_cmp(&gap_b).then(apart_a.total_cmp(&apart_b)).then(a.cmp(&b))
        })
    }
}

/// How `second`, drawn after `first`, lies beside it, with `meter`'s work: after it when they overlap, or
/// when what one of them paints can't be found, each with the budget's work to itself; a line bordering a
/// fill; or `None`, either way round.
fn lie(shapes: &mut Shapes, first: usize, second: usize, meter: &mut Meter) -> Result<Option<Beside>, Failed> {
    shapes.find(first);
    shapes.find(second);
    let (Some((a, painted_first)), Some((b, painted_second))) = (shapes.paint(first), shapes.paint(second)) else {
        return Ok(Some(Beside::After(second)));
    };
    if overlap(painted_first, painted_second, meter)? {
        return Ok(Some(Beside::After(second)));
    }
    let (fill, line) = match (a, b) {
        (Kind::Area, Kind::Column | Kind::Line | Kind::Part) => (first, second),
        (Kind::Column | Kind::Line | Kind::Part, Kind::Area) => (second, first),
        _ => return Ok(None),
    };
    Ok(touch(shapes, fill, line, meter)?.then_some(Beside::Border(fill, line)))
}

/// Whether `first` and `second`, what 2 shapes paint, overlap by more than rounding leaves where edges
/// drawn as one meet, with `meter`'s work: whether where they overlap is wider than [`TOUCHING`] somewhere,
/// as a sliver of rounding is nowhere, so that something of it is left shrunk by half that on every side.
/// Where they overlap is measured, not either shape, so that a part of a shape thinner than [`TOUCHING`]
/// that lies across the other overlaps it too.
fn overlap(first: &[Polygon], second: &[Polygon], meter: &mut Meter) -> Result<bool, Failed> {
    let both = joined(intersection(first, second, meter))?;
    if no_area(area_of(&both)) {
        return Ok(false);
    }
    Ok(!no_area(area_of(&joined(buffered(&both, -TOUCHING / 2.0, meter))?)))
}

/// Whether what the line `line` paints comes within [`TOUCHING`] of what the fill `fill` paints.
fn touch(shapes: &mut Shapes, fill: usize, line: usize, meter: &mut Meter) -> Result<bool, Failed> {
    let grown = shapes.grown(fill, meter)?.unwrap_or_default();
    let line = shapes.paint(line).map(|(_, parts)| parts).unwrap_or_default();
    Ok(!no_area(area_of(&joined(intersection(&grown, line, meter))?)))
}

/// The square of the distance between boxes `a` and `b`: 0 when they overlap.
fn gap(a: &Envelope, b: &Envelope) -> f64 {
    let x = (b.min_x - a.max_x).max(a.min_x - b.max_x).max(0.0);
    let y = (b.min_y - a.max_y).max(a.min_y - b.max_y).max(0.0);
    x * x + y * y
}

/// The square of the distance between the centres of boxes `a` and `b`.
fn apart(a: &Envelope, b: &Envelope) -> f64 {
    let x = (a.min_x + a.max_x - b.min_x - b.max_x) / 2.0;
    let y = (a.min_y + a.max_y - b.min_y - b.max_y) / 2.0;
    x * x + y * y
}

/// The order as far as it is placed.
#[derive(Default)]
struct Sewing {
    /// The elements placed, in the order they sew.
    order: Vec<usize>,
    /// The thread sewing: the last placed element's; `None` before the first.
    thread: Option<Rgb>,
    /// The last placed.
    last: Option<usize>,
}

/// What must sew before what.
struct Graph {
    /// The elements each element must sew before.
    after: Vec<BTreeSet<usize>>,
    /// How many elements not placed yet each element must sew after.
    waiting: Vec<usize>,
}

impl Graph {
    /// A graph of `count` elements, any of which may sew before any other.
    fn new(count: usize) -> Graph {
        Graph { after: vec![BTreeSet::new(); count], waiting: vec![0; count] }
    }

    /// `from` sews before `to`.
    fn add(&mut self, from: usize, to: usize) {
        if self.after.get_mut(from).is_some_and(|after| after.insert(to))
            && let Some(waiting) = self.waiting.get_mut(to)
        {
            *waiting += 1;
        }
    }

    /// How many elements not placed yet `index` must sew after.
    fn waiting(&self, index: usize) -> usize {
        self.waiting.get(index).copied().unwrap_or(0)
    }

    /// Places `from`, and gives the elements it held back that now wait for none.
    fn release(&mut self, from: usize) -> Vec<usize> {
        let Graph { after, waiting } = self;
        let mut freed = Vec::new();
        for &to in after.get(from).into_iter().flatten() {
            let Some(count) = waiting.get_mut(to) else { continue };
            *count = count.saturating_sub(1);
            if *count == 0 {
                freed.push(to);
            }
        }
        freed
    }

    /// Whether `to` must sew after `from`, through the elements between them, with `meter`'s work: a unit for
    /// each edge followed.
    fn reaches(&self, from: usize, to: usize, meter: &mut Meter) -> Result<bool, Exhausted> {
        let (mut seen, mut stack) = (BTreeSet::from([from]), vec![from]);
        while let Some(at) = stack.pop() {
            for &next in self.after.get(at).into_iter().flatten() {
                meter.charge(1)?;
                if next == to {
                    return Ok(true);
                }
                if seen.insert(next) {
                    stack.push(next);
                }
            }
        }
        Ok(false)
    }
}

/// How many times the thread changes from one element of `order` to the next.
fn changes(order: &[usize], facts: &[Facts]) -> usize {
    let colour = |index: usize| facts.get(index).map(|facts| facts.colour);
    order.windows(2).filter(|pair| matches!(pair, [a, b] if colour(*a) != colour(*b))).count()
}

/// `SC-I0907` saying the elements sew in an order of their own, with `after` thread changes where paint
/// order has `before`.
fn summary(before: usize, after: usize) -> Diagnostic {
    let changes = |count: usize| if count == 1 { "1 thread change".to_string() } else { format!("{count} thread changes") };
    let message = format!("The elements are sewn in an order of their own, with {} where paint order has {before}.", changes(after));
    Diagnostic::new(Code::AutoOrder, message)
}

/// `SC-I0907` for the element `id`, the first in paint order of those not placed when choosing the order
/// needed more than the `work` the budget allows.
fn unplaced(work: u64, id: ElementId) -> Diagnostic {
    let message = format!(
        "Choosing the order the elements sew in needed more than the work budget of {work} units, so this element and the others not \
         placed by then sew in paint order, after those placed."
    );
    Diagnostic::new(Code::AutoOrder, message).with_element(id)
}

#[cfg(test)]
mod tests;
