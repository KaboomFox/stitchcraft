//! The order the elements are sewn in (`REQ-AUTO-010`, `REQ-AUTO-011`), said with `SC-I0907`.
//!
//! Sewn in paint order, a traced picture changes thread far more often than it needs to. VectorCraft's Auto
//! Grouping draws each shape together with the shapes inside it, so a row of badges goes red, blue, gold,
//! red, blue, gold, and on a home machine each change of thread is a stop to rethread it. So the elements
//! are put in an order of their own, by 6 rules:
//!
//! 1. **What overlaps keeps its paint order.** Of 2 shapes whose paint overlaps, the lower sews first:
//!    sewn the other way, it would cover the upper one. An area paints its region, a satin column the area
//!    between its rails as they are sewn, and a line the area its stroke paints as drawn. Shapes that only
//!    meet, as abutting paths do, may sew in either order. Rounding leaves a sliver no wider than
//!    [`TOUCHING`] between edges drawn as one, and shapes overlap only where their overlap is wider.
//! 2. **A line after the fill it borders.** A line that touches a fill without overlapping it sews after
//!    the fill, unless shapes that overlap hold it before: rule 1 comes first. A running stitch is as wide
//!    as its thread and can't reach under the fill, and a fill sewn after it would bury it.
//! 3. **The fewest thread changes** (`REQ-AUTO-037`). Within rules 1 and 2 the elements sew in batches of
//!    one thread, in the row of batches that changes thread least of any ([`fewest`]). Threads are the same
//!    when their colours are, as plan assembly counts thread changes. An element sewn in several threads
//!    moves as one, its colours together and in their order: it has the thread it starts in, and leaves the
//!    one it ends in sewing (`REQ-AUTO-024`).
//! 4. **A satin column after the fill it borders, where that costs no thread change.** A column that
//!    automatic digitizing made covers the fill's edge when it sews after the fill, and reaches under the
//!    fill when it sews first (`REQ-AUTO-029`), so the fabric is covered either way. Of the rows with the
//!    fewest changes, the one that sews more such columns after their fills is taken, and a column waits
//!    for a later batch of its thread where there is one. Any other satin column goes by rule 2.
//! 5. **Details last, where that costs no thread change** (`REQ-AUTO-096`, `REQ-AUTO-097`). An automatic
//!    area that the shapes round it close in on ([`Order::closed_in`]) is a detail of each larger shape it
//!    meets, as an eye is of the fur round it. It sews after them, as a column does after its fill: of the
//!    rows with the fewest changes, the one that sews fewer details early is taken, and a detail waits for a
//!    later batch of its thread. A line or a column round a detail still goes by rules 2 and 4: it covers
//!    the detail's edge, as an outline does, and the detail reaches under a column. Which shape is a detail goes by geometry alone, never by what the picture shows. `SC-I0907` says how
//!    many details still sew before a larger shape round them.
//! 6. **The nearest next.** Of the free elements of a batch, the next is the one whose box lies nearest
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
//! in paint order as a stop does: nothing moves across it. Finding the row of batches has the budget's work
//! once. When it needs more, the elements from that run on sew as they did before M13.22: each thread while
//! an element of it is free, then the thread of the first free element in paint order, every satin column
//! after the fills it borders. Placing the elements has the budget's work once more. An order that needs more
//! sews the elements not placed by then in paint order, after the others. Each is said.

use std::collections::{BTreeMap, BTreeSet};
use std::f64::consts::SQRT_2;

use stitchcraft_core::{Budget, Code, Diagnostic, ElementId, Exhausted, Meter, Point};
use stitchcraft_plan::Rgb;

use super::{Failed, Kind, Shapes, TOUCHING, envelope, joined, near, no_area};
use crate::design::{Element, Join, Shape};
use crate::generate::{colours, stops_after};
use crate::normalize::region::Polygon;
use crate::normalize::region::geom::Envelope;
use crate::normalize::region::ops::{area_of, buffered, difference, intersection, union};

/// What becomes of an element whose neighbours need more work to find than the budget allows: it keeps its
/// place, as an element with a stop after it does.
const PLACE: &str = "it keeps its place in paint order: the elements drawn before it sew before it, and those drawn after it after it";

mod fewest;

/// The share of the strip round an automatic area that the other shapes must cover for them to close in
/// on it, so that it is a detail of the larger ones it meets ([`Order::closed_in`]): an eye or a mark on a
/// patch is closed in whole, and a patch at the picture's edge is open along it.
const CLOSED_IN: f64 = 0.75;

/// How an element may move round the others.
#[derive(Clone, Copy, Debug)]
pub(super) struct Moving {
    /// Whether it may: an automatic element, or any element of a design that asks for every element ordered.
    pub(super) free: bool,
    /// Whether automatic digitizing made it: the base layer sews before the first such element.
    pub(super) made: bool,
    /// The place in the design of the shape it is a part of: the parts of a shape sewn as several elements,
    /// such as a line's satin columns, keep their order, so that a trim or a stop after the shape still comes
    /// after its last part.
    pub(super) shape: usize,
}

/// `shapes` put in the order they sew in, each element moving round the others as `moving` says, and what
/// is said of it: `SC-I0907` when the order isn't paint order, and for each element left in paint order.
/// `under` is the thread of the base layer, if there is one: it sews before the first element that automatic
/// digitizing made, and is sewing when that element's run starts.
pub(super) fn ordered(shapes: &mut Shapes, moving: &[Moving], budget: &Budget, under: Option<Rgb>) -> Vec<Diagnostic> {
    let live: Vec<usize> = (0..shapes.len()).filter(|&index| !shapes.left_out(index)).collect();
    if !live.iter().any(|&index| moving.get(index).is_some_and(|moving| moving.free)) {
        return Vec::new();
    }
    let reaches: Vec<Option<Envelope>> = (0..shapes.len()).map(|index| reach(shapes, index)).collect();
    let facts: Vec<Facts> = shapes.elements.iter().zip(reaches).map(|(element, reach)| Facts::of(element, reach)).collect();
    let (graph, kept) = (Graph::new(shapes.len()), BTreeSet::new());
    let mut order = Order {
        facts: &facts,
        moving,
        graph,
        borders: Vec::new(),
        wished: Vec::new(),
        details: Vec::new(),
        closed: BTreeMap::new(),
        kept,
        unsought: None,
        under,
    };
    let stop = |index: usize| facts.get(index).is_some_and(|facts| facts.stop);
    let mut said = Vec::new();
    for run in runs(&live, stop) {
        order.relate(shapes, &run, budget, &mut said);
    }
    // Nothing moves across an element whose neighbours needed more work than the budget allows either.
    let runs = runs(&live, |index| stop(index) || order.kept.contains(&index));
    let mut sewing = Sewing::default();
    let spent = order.sew(&runs, &mut sewing, budget).is_err();
    said.extend(order.unsought.and_then(|index| shapes.id(index)).map(|id| unsought(budget.max_work, id)));
    // What wasn't placed sews in paint order after the rest: what the work left, and so no element is lost.
    let rest: Vec<usize> = live.iter().copied().filter(|index| !sewing.placed.contains(index)).collect();
    if spent {
        said.extend(rest.first().and_then(|&index| shapes.id(index)).map(|id| unplaced(budget.max_work, id)));
    }
    sewing.order.extend(rest);
    if sewing.order != live {
        said.push(summary(changes(&live, &facts), changes(&sewing.order, &facts)));
    }
    said.extend(early(&order.details, &sewing.order));
    // The elements left out go last, where they are left out.
    sewing.order.extend((0..shapes.len()).filter(|&index| shapes.left_out(index)));
    shapes.reorder(&sewing.order);
    said
}

/// What the order needs to know of an element.
struct Facts {
    /// The colours of its threads, in the order it sews them ([`colours`]).
    colours: Vec<Rgb>,
    /// Whether it has a stop after it, which nothing moves across.
    stop: bool,
    /// The box round it ([`reach`]); `None` for a path of no points.
    reach: Option<Envelope>,
}

impl Facts {
    /// What the order needs to know of `element`, whose box is `reach`.
    fn of(element: &Element, reach: Option<Envelope>) -> Facts {
        Facts { colours: colours(element), stop: stops_after(element), reach }
    }

    /// The colour of the thread it starts in, which must be sewing for it to sew next without a change.
    fn first(&self) -> Option<Rgb> {
        self.colours.first().copied()
    }

    /// The colour of the thread it ends in, which is sewing once it is sewn.
    fn last(&self) -> Option<Rgb> {
        self.colours.last().copied()
    }

    /// How many times its thread changes as it is sewn, between colours one after the other that differ.
    fn changes(&self) -> usize {
        self.colours.windows(2).filter(|pair| matches!(pair, [a, b] if a != b)).count()
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
pub(super) fn drawn(element: &Element) -> Option<Envelope> {
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
    /// Each line that borders a fill and sews after it, as the fill and the line, in the order they were
    /// found.
    borders: Vec<(usize, usize)>,
    /// Each satin column that automatic digitizing made and that borders a fill, as the fill and the column,
    /// in the order they were found: it sews after the fill where that costs no thread change.
    wished: Vec<(usize, usize)>,
    /// Each detail, an automatic area that the shapes round it close in on ([`Self::closed_in`]), with each
    /// shape it meets that is larger than it is, as that shape and the detail, in the order they were found:
    /// it sews after that shape where that costs no thread change.
    details: Vec<(usize, usize)>,
    /// Whether each automatic area looked at is closed in, once found.
    closed: BTreeMap<usize, bool>,
    /// The elements whose neighbours need more work to find than the budget allows, which keep their place.
    kept: BTreeSet<usize>,
    /// The first element of the run that finding the fewest thread changes needed more work for than the
    /// budget allows, if any.
    unsought: Option<usize>,
    /// The thread of the base layer, until the run of the first element that automatic digitizing made
    /// starts.
    under: Option<Rgb>,
}

/// How a shape lies beside one drawn before it.
#[derive(Debug, PartialEq)]
enum Beside {
    /// It overlaps that shape, or what one of them paints can't be found: it sews after it.
    After(usize),
    /// A line borders a fill: the fill, then the line. The fill sews first unless what overlaps holds the
    /// line before it.
    Border(usize, usize),
    /// 2 automatic areas meet without overlapping: the one drawn first, then the other. Either may be a
    /// detail of the other.
    Meet(usize, usize),
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
            let mut meter = budget.meter();
            let found = self.beside(shapes, index, later, &mut meter);
            match found.and_then(|found| self.settle(shapes, index, found, &mut meter)) {
                Ok(()) => {}
                Err(failed) => {
                    self.kept.insert(index);
                    let finding = "where this element meets the shapes drawn after it";
                    said.extend(shapes.id(index).map(|id| failed.said(Code::AutoOrder, finding, PLACE, budget.max_work, id)));
                }
            }
        }
    }

    /// What `found`, how the shapes drawn after `index` lie beside it, makes them sew before what, with `meter`'s work: a
    /// shape sews after one it overlaps, a line after the fill it borders, a satin column after the fill it
    /// borders where that costs no thread change, and a detail after a larger area it meets where that costs
    /// none ([`Self::detail`]).
    fn settle(&mut self, shapes: &mut Shapes, index: usize, found: Vec<Beside>, meter: &mut Meter) -> Result<(), Failed> {
        for beside in found {
            match beside {
                Beside::After(other) => self.graph.add(index, other),
                Beside::Border(fill, line) if shapes.column(line).is_some() => self.wished.push((fill, line)),
                Beside::Border(fill, line) => self.borders.push((fill, line)),
                Beside::Meet(first, second) => {
                    if self.detail(shapes, second, first, meter)? {
                        self.details.push((first, second));
                    } else if self.detail(shapes, first, second, meter)? {
                        self.details.push((second, first));
                    }
                }
            }
        }
        Ok(())
    }

    /// Whether the automatic area `detail` is a detail of `other`, a shape it meets, with `meter`'s work:
    /// the shapes round it close in on it ([`Self::closed_in`]), and it is smaller than `other` paints.
    fn detail(&mut self, shapes: &mut Shapes, detail: usize, other: usize, meter: &mut Meter) -> Result<bool, Failed> {
        let size = |index: usize| shapes.paint(index).map(|(_, parts)| area_of(parts));
        let (true, Some(own), Some(theirs)) = (shapes.is_area(detail), size(detail), size(other)) else { return Ok(false) };
        Ok(own < theirs && self.closed_in(shapes, detail, meter)?)
    }

    /// Whether the shapes round the automatic area `index` close in on it, with `meter`'s work, found once:
    /// a strip [`TOUCHING`] wide round its region lies under what the other shapes paint, grown by
    /// [`TOUCHING`], for [`CLOSED_IN`] of its area or more. It costs a unit for each shape looked at, and the
    /// work of those near it.
    fn closed_in(&mut self, shapes: &mut Shapes, index: usize, meter: &mut Meter) -> Result<bool, Failed> {
        if let Some(&known) = self.closed.get(&index) {
            return Ok(known);
        }
        let found = enclosed(shapes, self.facts, index, meter)?;
        self.closed.insert(index, found);
        Ok(found)
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

    /// Puts the elements of `runs` in the order they sew, after those `sewing` holds. First each line sews
    /// after the fills it borders, where what overlaps allows it and no element kept in its place lies between
    /// them. Then each run in turn sews in the batches that change thread least ([`fewest`]), found with
    /// `budget`'s work once for all the runs, and the columns that those batches let sew after the fills they
    /// border do. From a run that needs more work on, the columns sew after their fills as the lines do, and
    /// each thread sews while it lasts ([`Self::run`]). Placing the elements has `budget`'s work once more.
    fn sew(&mut self, runs: &[Vec<usize>], sewing: &mut Sewing, budget: &Budget) -> Result<(), Exhausted> {
        let mut placing = budget.meter();
        for (fill, line) in std::mem::take(&mut self.borders) {
            self.border(fill, line, &mut placing)?;
        }
        let mut seeking = Some(budget.meter());
        for run in runs {
            // The base layer sews before the first element that automatic digitizing made, so its thread is
            // sewing when that element's run starts. Where elements with settings of the same run sew before
            // that element, the base layer sews between them and it, and may change thread once more than
            // the batches found count.
            if run.iter().any(|&index| self.moving.get(index).is_some_and(|moving| moving.made)) {
                sewing.thread = self.under.take().or(sewing.thread);
            }
            let at = |place: usize| run.get(place).copied();
            match seeking.as_mut().map(|meter| fewest::fewest(&self.sought(run, sewing.thread), meter)) {
                Some(Ok(found)) => {
                    for (fill, column) in found.granted.iter().filter_map(|&(fill, column)| at(fill).zip(at(column))) {
                        self.graph.add(fill, column);
                    }
                    for batch in &found.batches {
                        let batch: Vec<usize> = batch.iter().filter_map(|&place| at(place)).collect();
                        self.run(&batch, sewing, &mut placing)?;
                    }
                }
                Some(Err(_)) => {
                    (seeking, self.unsought) = (None, run.first().copied());
                    // The columns of the runs before are placed, after their fills or not as their batches
                    // were found.
                    for (fill, column) in std::mem::take(&mut self.wished) {
                        if !sewing.placed.contains(&column) {
                            self.border(fill, column, &mut placing)?;
                        }
                    }
                }
                None => {}
            }
            // The elements that no batch placed: all of them when none were found.
            let rest: Vec<usize> = run.iter().copied().filter(|index| !sewing.placed.contains(index)).collect();
            self.run(&rest, sewing, &mut placing)?;
        }
        Ok(())
    }

    /// The line `line` sews after the fill `fill` it borders, unless an element kept in its place lies
    /// between them or what must sew before what already holds the line before the fill; with `meter`'s work.
    fn border(&mut self, fill: usize, line: usize, meter: &mut Meter) -> Result<(), Exhausted> {
        if self.kept.range(fill.min(line)..=fill.max(line)).next().is_none() && !self.graph.reaches(line, fill, meter)? {
            self.graph.add(fill, line);
        }
        Ok(())
    }

    /// `run` as the search for the fewest thread changes takes it ([`fewest::Run`]), `sewing` the thread
    /// sewing when it starts: each element by its place in the run, with the elements of the run that must
    /// sew before it, and the wishes whose fill and column are both of the run.
    fn sought(&self, run: &[usize], sewing: Option<Rgb>) -> fewest::Run {
        let places: BTreeMap<usize, usize> = run.iter().enumerate().map(|(place, &index)| (index, place)).collect();
        let threads = |index: &usize| self.facts.get(*index).map_or([None; 2], |facts| [facts.first(), facts.last()]);
        let mut before = vec![Vec::new(); run.len()];
        for (place, index) in run.iter().enumerate() {
            for later in self.graph.after.get(*index).into_iter().flatten().filter_map(|later| places.get(later)) {
                if let Some(waiting) = before.get_mut(*later) {
                    waiting.push(place);
                }
            }
        }
        let place = |index: &usize| places.get(index).copied();
        let wished = self.wished.iter().chain(&self.details).filter_map(|(first, then)| place(first).zip(place(then))).collect();
        fewest::Run { threads: run.iter().map(threads).collect(), before, wished, sewing }
    }

    /// Puts the elements of `run`, a batch or a run, after those `sewing` holds, with `meter`'s work: of the
    /// elements free to sew, those that start in the thread sewing, or else those that start in the first
    /// one's thread, and of those the one nearest the last sewn, whose last thread is then sewing. An element
    /// that isn't of `run` and that one placed here frees waits for its own batch or run, so that none moves
    /// across what ends a run, a stop or an element kept in its place, and none is placed twice.
    fn run(&mut self, run: &[usize], sewing: &mut Sewing, meter: &mut Meter) -> Result<(), Exhausted> {
        let members: BTreeSet<usize> = run.iter().copied().collect();
        let mut free: BTreeSet<usize> = run.iter().copied().filter(|&index| self.graph.waiting(index) == 0).collect();
        while let Some(&first) = free.first() {
            let starts = |index: usize| self.facts.get(index).and_then(Facts::first);
            let (mut same, mut changed) = (Vec::new(), Vec::new());
            for &index in &free {
                meter.charge(1)?;
                if starts(index) == sewing.thread {
                    same.push(index);
                }
                if starts(index) == starts(first) {
                    changed.push(index);
                }
            }
            if same.is_empty() {
                same = changed;
            }
            let Some(next) = self.nearest(&same, sewing.last) else { break };
            free.remove(&next);
            free.extend(self.graph.release(next).into_iter().filter(|index| members.contains(index)));
            sewing.order.push(next);
            sewing.placed.insert(next);
            sewing.last = Some(next);
            sewing.thread = self.facts.get(next).and_then(Facts::last);
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
        (Kind::Area, Kind::Area) if shapes.is_area(first) && shapes.is_area(second) => {
            return Ok(touch(shapes, first, second, meter)?.then_some(Beside::Meet(first, second)));
        }
        _ => return Ok(None),
    };
    Ok(touch(shapes, fill, line, meter)?.then_some(Beside::Border(fill, line)))
}

/// Whether `first` and `second`, what 2 shapes paint, overlap by more than rounding leaves where edges
/// drawn as one meet, with `meter`'s work: whether where they overlap is wider than [`TOUCHING`] somewhere,
/// as a sliver of rounding is nowhere, so that something of it is left shrunk by half that on every side.
/// Where they overlap is measured, not either shape, so that a part of a shape thinner than [`TOUCHING`]
/// that lies across the other overlaps it too.
pub(super) fn overlap(first: &[Polygon], second: &[Polygon], meter: &mut Meter) -> Result<bool, Failed> {
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

/// Whether the shapes round the automatic area `index` of `shapes` close in on it ([`Order::closed_in`]),
/// each shape's box in `facts`, with `meter`'s work.
fn enclosed(shapes: &mut Shapes, facts: &[Facts], index: usize, meter: &mut Meter) -> Result<bool, Failed> {
    let Some(region) = shapes.paint(index).map(|(_, parts)| parts.to_vec()) else { return Ok(false) };
    let rim = joined(difference(&joined(buffered(&region, TOUCHING, meter))?, &region, meter))?;
    let all = area_of(&rim);
    let reach = |other: usize| facts.get(other).and_then(|facts| facts.reach);
    let Some(bounds) = reach(index).filter(|_| !no_area(all)) else { return Ok(false) };
    let mut around = Vec::new();
    for other in 0..shapes.len() {
        meter.charge(1)?;
        if other == index || shapes.left_out(other) || !reach(other).is_some_and(|other| near(&bounds, 2.0 * TOUCHING, &other)) {
            continue;
        }
        around.extend(shapes.grown(other, meter)?.unwrap_or_default());
    }
    let covered = joined(intersection(&rim, &joined(union([around.as_slice()], meter))?, meter))?;
    Ok(area_of(&covered) >= CLOSED_IN * all)
}

/// `SC-I0907` saying how many of the `details`, each the shape it meets and the detail, sew before that
/// shape in `order`; `None` when none does.
fn early(details: &[(usize, usize)], order: &[usize]) -> Option<Diagnostic> {
    let at = |index: usize| order.iter().position(|&sewn| sewn == index);
    let early: BTreeSet<usize> = details.iter().filter(|&&(shape, detail)| at(detail) < at(shape)).map(|&(_, detail)| detail).collect();
    let message = match early.len() {
        0 => return None,
        1 => "1 detail sews before a larger area round it: sewn after it, it would change thread more often, or break \
              what must sew before what."
            .to_string(),
        n => format!(
            "{n} details sew before a larger area round them: sewn after it, they would change thread more often, or break \
             what must sew before what."
        ),
    };
    Some(Diagnostic::new(Code::AutoOrder, message))
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
    /// The elements placed.
    placed: BTreeSet<usize>,
    /// The thread sewing: the one the last placed element ends in; `None` before the first.
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

/// How many times the thread changes as the elements of `order` are sewn: from one element to the next, and
/// inside the elements sewn in several threads, as the plan changes it.
fn changes(order: &[usize], facts: &[Facts]) -> usize {
    let of = |index: usize| facts.get(index);
    let between = order.windows(2).filter(|pair| matches!(pair, [a, b] if of(*a).and_then(Facts::last) != of(*b).and_then(Facts::first)));
    between.count() + order.iter().filter_map(|&index| of(index)).map(Facts::changes).sum::<usize>()
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

/// `SC-I0907` for the element `id`, the first of the run that finding the fewest thread changes needed more
/// than the `work` the budget allows for.
fn unsought(work: u64, id: ElementId) -> Diagnostic {
    let message = format!(
        "Finding the order with the fewest thread changes needed more than the work budget of {work} units, so from this element on each \
         thread sews while an element of it is free to sew, then the thread of the first free element in paint order, and every satin \
         column sews after the fills it borders."
    );
    Diagnostic::new(Code::AutoOrder, message).with_element(id)
}

#[cfg(test)]
mod tests;
