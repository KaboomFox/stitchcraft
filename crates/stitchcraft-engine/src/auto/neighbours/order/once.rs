//! Each thread threaded once (`REQ-AUTO-185`, `REQ-AUTO-187`, M13.63): the thread the base layer sews in, and
//! `SC-W0925` where the shapes as drawn let no order thread each thread once.
//!
//! A machine with one needle is rethreaded by hand at each change of thread, so a design of 6 threads should
//! be threaded 6 times. The base layer sews before every automatic element, and its thread is sewing when
//! they start. In the thread of the largest area, as it was chosen before M13.63, it threaded that thread
//! twice where its shapes lie over another thread's and can't sew first: a flower's pink head drawn over its
//! green stem sewed pink, green, pink. So the order is searched for twice, once from the largest area's thread
//! and once with no thread sewing, and the base layer takes the largest area's thread unless the free order
//! changes thread less, or as little and sews more lines and columns after their fills. Then it takes the
//! thread that order starts with.
//!
//! Where a shade is drawn both under and over another, as a roof over a chimney's outline that is drawn over
//! the chimney, the shapes that overlap keep their paint order (rule 1), and the 2 threads must each sew
//! twice. Cutting the lower shapes away would mend it, but it chooses on its own which shape loses its depth,
//! and measured it made a traced cat sew 4.23 fills thick. So `SC-W0925` names the shapes, and the person
//! changes the drawing.

use std::cmp::{Ordering, Reverse};
use std::collections::{BTreeMap, BTreeSet};

use stitchcraft_core::{Budget, Code, Diagnostic, Exhausted, Fix, Meter};
use stitchcraft_plan::Rgb;

use super::fewest::{self, Found};
use super::{Facts, Order, Shapes};
use crate::normalize::region::ops::area_of;

/// The batches of `run` with the fewest thread changes, and the thread the base layer sews in: `base`, the
/// largest area's thread, unless the row found with no thread sewing changes thread less, counting a change
/// into its first batch from the thread it starts in, or as little and grants more wishes. Then it is that
/// row's first thread. With `meter`'s work: both searches.
pub(super) fn based(order: &Order<'_>, run: &[usize], base: Rgb, meter: &mut Meter) -> Result<(Found, Option<Rgb>), Exhausted> {
    let given = fewest::fewest(&order.sought(run, Some(base)), meter)?;
    let free = fewest::fewest(&order.sought(run, None), meter)?;
    let starts = threads(order, run, &free).first().and_then(|&[first, _]| first);
    match starts {
        Some(starts) if cost(order, run, &free, starts) < cost(order, run, &given, base) => Ok((free, Some(starts))),
        _ => Ok((given, Some(base))),
    }
}

/// What the row `found` of `run` costs, sewn from the thread `sewing`: its thread changes, then the fewer wishes
/// it grants.
fn cost(order: &Order<'_>, run: &[usize], found: &Found, sewing: Rgb) -> (usize, Reverse<usize>) {
    let mut changes = 0;
    let mut last = Some(sewing);
    for [first, end] in threads(order, run, found) {
        changes += usize::from(first != last);
        last = end;
    }
    (changes, Reverse(found.granted.len()))
}

/// The thread each batch of `found`, of the elements of `run`, starts in and ends in.
fn threads(order: &Order<'_>, run: &[usize], found: &Found) -> Vec<[Option<Rgb>; 2]> {
    let facts = |place: Option<&usize>| place.and_then(|&place| run.get(place)).and_then(|&index| order.facts.get(index));
    found.batches.iter().map(|batch| [facts(batch.first()).and_then(Facts::first), facts(batch.last()).and_then(Facts::last)]).collect()
}

/// What is said of the threads that the shapes as drawn hold both before and after another ([`twice`]), and
/// of the lines sewn before a fill beside them ([`buried`]) in the order `sewn`, with the work of `budget`:
/// `SC-I0907` when that needs more, and no shape is named.
pub(super) fn told(order: &Order<'_>, sewn: &[usize], shapes: &Shapes, budget: &Budget) -> Vec<Diagnostic> {
    let meter = &mut budget.meter();
    let said = twice(order, shapes, meter).and_then(|mut said| {
        said.extend(buried(order, sewn, shapes, meter)?);
        Ok(said)
    });
    said.unwrap_or_else(|_| vec![untold(budget.max_work)])
}

/// How many of the lines sewn before a fill beside them a notice names.
const NAMED: usize = 3;

/// `SC-I0926` counting the lines that sew before a fill of another thread they border, in the order `sewn`, so
/// that each thread is threaded once: the fill's rows end on such a line and cover up to half its thread on
/// that side (`REQ-AUTO-188`). It names the [`NAMED`] with such fills on the most sides, of those the ones
/// that paint least, as the lines most of whose thread may be covered; `None` when no line sews so. With
/// `meter`'s work: a unit for each line and fill they border, and for each line named.
fn buried(order: &Order<'_>, sewn: &[usize], shapes: &Shapes, meter: &mut Meter) -> Result<Option<Diagnostic>, Exhausted> {
    meter.charge((sewn.len() + order.lined.len()) as u64)?;
    let at: BTreeMap<usize, usize> = sewn.iter().enumerate().map(|(place, &index)| (index, place)).collect();
    let thread = |index: usize| order.facts.get(index).and_then(Facts::first);
    let mut sides: BTreeMap<usize, usize> = BTreeMap::new();
    for &(fill, line) in order.lined.iter().filter(|&&(fill, line)| thread(fill) != thread(line)) {
        if let (Some(line_at), Some(fill_at)) = (at.get(&line), at.get(&fill)) {
            *sides.entry(line).or_insert(0) += usize::from(line_at.cmp(fill_at) == Ordering::Less);
        }
    }
    sides.retain(|_, count| *count > 0);
    meter.charge(sides.len() as u64)?;
    let mut worst: Vec<(Reverse<usize>, f64, usize)> =
        sides.iter().map(|(&line, &count)| (Reverse(count), shapes.paint(line).map_or(0.0, |(_, parts)| area_of(parts)), line)).collect();
    worst.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.total_cmp(&b.1)).then(a.2.cmp(&b.2)));
    let named: Vec<String> = worst.iter().take(NAMED).filter_map(|&(_, _, line)| name(shapes, line)).map(|name| format!("\"{name}\"")).collect();
    let count = sides.len();
    let lines = if count == 1 { "1 line sews".to_string() } else { format!("{count} lines sew") };
    let message = format!(
        "{lines} before a fill of another thread beside them, so that each thread is threaded once: the fill's rows end on the line and \
         may cover up to half its thread on that side. Those with such fills on the most sides, the smallest first: {}.",
        named.join(", ")
    );
    let fix = Fix::Hint(
        "To sew every line after the fills beside it, turn `auto_thread_once` off: some threads are then threaded more than once.".to_string(),
    );
    let said = |(_, _, line): &(Reverse<usize>, f64, usize)| {
        shapes.id(*line).map(|id| Diagnostic::new(Code::AutoLinesFirst, message.clone()).with_element(id).with_fix(fix.clone()))
    };
    Ok(worst.first().and_then(said))
}

/// `SC-I0907` when finding the threads that the shapes as drawn hold both before and after another needed more
/// than the `work` the budget allows: `SC-W0925` isn't said.
fn untold(work: u64) -> Diagnostic {
    let message = format!(
        "Finding the threads that the shapes as drawn sew both before and after another thread needed more than the work budget of {work} \
         units, so no shape is named for a thread threaded more than once."
    );
    Diagnostic::new(Code::AutoOrder, message)
}

/// `SC-W0925` for each group of threads of a run that its overlapping shapes hold both before and after each
/// other, so that a thread of the group is sewn in more than one batch: said once a group, of a shape of the first
/// such thread lying under a shape of another thread of the group, and naming a shape of the group lying under
/// a shape of that thread. With `meter`'s work: a unit for each element of a batch, each thread of the run's
/// elements, each pair of overlapping shapes, and each pair of threads looked at by [`reaches`].
fn twice(order: &Order<'_>, shapes: &Shapes, meter: &mut Meter) -> Result<Vec<Diagnostic>, Exhausted> {
    let mut said = Vec::new();
    let colours = |index: &usize| order.facts.get(*index).into_iter().flat_map(|facts| facts.colours.iter().copied());
    for (run, batches) in &order.found {
        // How many batches each thread is sewn in.
        let mut times: BTreeMap<Rgb, usize> = BTreeMap::new();
        for batch in batches {
            meter.charge(batch.len() as u64)?;
            for colour in batch.iter().flat_map(colours).collect::<BTreeSet<Rgb>>() {
                *times.entry(colour).or_insert(0) += 1;
            }
        }
        // The threads in the order their first elements come in the run. A thread that shapes hold both before
        // and after another is sewn in more than one batch.
        let mut threads: Vec<Rgb> = Vec::new();
        for colour in run.iter().flat_map(colours) {
            meter.charge(1)?;
            if !threads.contains(&colour) {
                threads.push(colour);
            }
        }
        // Each pair of threads that overlapping shapes of the run hold one before the other, with those shapes.
        meter.charge(order.overlaps.len() as u64)?;
        let thread = |index: usize, end: fn(&Facts) -> Option<Rgb>| order.facts.get(index).and_then(end);
        let mut held: BTreeMap<(Rgb, Rgb), Vec<(usize, usize)>> = BTreeMap::new();
        for (lower, upper, from, to) in order
            .overlaps
            .iter()
            .filter(|(lower, upper)| run.contains(lower) && run.contains(upper))
            .filter_map(|&(lower, upper)| Some((lower, upper, thread(lower, Facts::last)?, thread(upper, Facts::first)?)))
            .filter(|(_, _, from, to)| from != to)
        {
            held.entry((from, to)).or_default().push((lower, upper));
        }
        let mut told: BTreeSet<Rgb> = BTreeSet::new();
        for thread in threads {
            let mut group = BTreeSet::new();
            for &other in times.keys() {
                if reaches(&held, thread, other, meter)? && reaches(&held, other, thread, meter)? {
                    group.insert(other);
                }
            }
            if !group.is_empty() && !told.contains(&thread) {
                told.extend(group.iter().copied());
                said.extend(named(shapes, &held, &group, thread, &times));
            }
        }
    }
    Ok(said)
}

/// Whether the thread `to` must sew after the thread `from`, through the pairs `held`, with `meter`'s work: a
/// unit for each pair looked at.
fn reaches(held: &BTreeMap<(Rgb, Rgb), Vec<(usize, usize)>>, from: Rgb, to: Rgb, meter: &mut Meter) -> Result<bool, Exhausted> {
    let (mut seen, mut stack) = (BTreeSet::from([from]), vec![from]);
    while let Some(at) = stack.pop() {
        meter.charge(held.len() as u64)?;
        for &(_, next) in held.keys().filter(|(start, _)| *start == at) {
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

/// The name of element `index` of `shapes`, or its id when it has none.
fn name(shapes: &Shapes, index: usize) -> Option<String> {
    let element = shapes.elements.get(index)?;
    Some(element.name.clone().unwrap_or_else(|| element.id.as_str().to_string()))
}

/// `SC-W0925` of the `group` of threads, `thread` one of them, each sewn `times` times: of the first shape of
/// `thread` lying under a shape of another thread of the group, naming that shape and a shape of the group
/// lying under a shape of `thread`, one of the first shape's thread when there is one.
fn named(
    shapes: &Shapes,
    held: &BTreeMap<(Rgb, Rgb), Vec<(usize, usize)>>,
    group: &BTreeSet<Rgb>,
    thread: Rgb,
    times: &BTreeMap<Rgb, usize>,
) -> Option<Diagnostic> {
    let inside = |pair: &&(Rgb, Rgb)| group.contains(&pair.0) && group.contains(&pair.1);
    let (&(_, over), &(lower, upper)) = held
        .iter()
        .filter(|(pair, _)| pair.0 == thread && inside(pair))
        .flat_map(|(pair, shapes)| shapes.iter().map(move |shape| (pair, shape)))
        .min_by_key(|(_, shape)| **shape)?;
    let (_, &(under, on)) = held
        .iter()
        .filter(|(pair, _)| pair.1 == thread && inside(pair))
        .flat_map(|(pair, shapes)| shapes.iter().map(move |shape| (pair, shape)))
        .min_by_key(|(pair, shape)| (pair.0 != over, **shape))?;
    let name = |index: usize| name(shapes, index);
    let threaded: Vec<String> = group
        .iter()
        .map(|colour| match times.get(colour).copied().unwrap_or(0) {
            1 => format!("{colour} once"),
            count => format!("{colour} {count} times"),
        })
        .collect();
    let message = format!(
        "This shape lies under \"{}\", and \"{}\" lies under \"{}\": their threads lie both under and over each other, so no order threads \
         each thread once. The threads are threaded {}.",
        name(upper)?,
        name(under)?,
        name(on)?,
        threaded.join(", ")
    );
    let fix = Fix::Hint(
        "To thread each thread once, change the drawing so that each shade lies at one depth: cut away what the upper shape hides of the \
         lower one, or join the shapes of one shade."
            .to_string(),
    );
    Some(Diagnostic::new(Code::AutoThreadedAgain, message).with_element(shapes.id(lower)?).with_fix(fix))
}

/// Whether `sewn`, the order found with `auto_thread_once` on, is one that the strict rules allow too, those of
/// `auto_thread_once` off (`REQ-AUTO-186`): no line sews before a fill it borders, the base layer keeps the
/// thread it was given, and no satin column sews after a part's running line it was held after by rule 4
/// widened. Such an order is a strict order with as few thread changes, so the thickness guard has nothing to
/// weigh (`REQ-AUTO-189`); the strict search may break a tie between such orders another way, which the set
/// measures. A line kept before its fill by what must sew before what counts
/// too: the strict rules would allow it, but it is not looked into. A unit of work for each element and pair,
/// out of `budget`'s work; one out of work counts the order as one the strict rules don't allow.
pub(super) fn allowed(order: &Order<'_>, sewn: &[usize], budget: &Budget) -> bool {
    let mut meter = budget.meter();
    for count in [sewn.len(), order.lined.len(), order.held.len()] {
        if meter.charge(count as u64).is_err() {
            return false;
        }
    }
    let at: BTreeMap<usize, usize> = sewn.iter().enumerate().map(|(place, &index)| (index, place)).collect();
    // Two elements never share a place.
    let before = |first: usize, second: usize| at.get(&first).zip(at.get(&second)).is_some_and(|(first, second)| first.cmp(second) == Ordering::Less);
    order.chosen.is_none()
        && !order.lined.iter().any(|&(fill, line)| before(line, fill))
        && !order.held.iter().any(|&(line, column)| before(line, column))
}

#[cfg(test)]
mod tests;
