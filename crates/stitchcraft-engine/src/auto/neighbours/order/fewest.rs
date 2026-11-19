//! The order of a run of elements that changes thread least (`REQ-AUTO-037`).
//!
//! Each change of thread is a rethread by hand on a machine with one needle. Taking each thread while it
//! lasts and then the thread of the first free element, as the order did before M13.22, comes back to a
//! thread again and again: the owner's traced dogs changed thread 12 to 28 times where 8 to 12 are needed.
//! Finding the fewest changes under rules of what sews before what is hard in general, and a design has few
//! threads, so the order is searched for:
//!
//! - **Batches.** Once a thread is sewing, every element of that thread that is free sews, and those they
//!   free: sewing more now never costs a change later. So an order is a row of batches, each the elements
//!   of one thread free at its turn, and only the row of threads is searched.
//! - **The search.** It goes from the row with the fewest changes so far plus the fewest still needed, one
//!   for each thread that has elements left, and so its first whole row has the fewest changes of any. An
//!   element sewn in several threads is a batch of its own, which costs a change when the thread it starts
//!   in isn't sewing, and the batch of the thread it ends in follows it.
//! - **Wishes.** A satin column would sew after the fill it borders, and it reaches under that fill when it
//!   sews first, so this is a wish and no rule. Of 2 rows with as few changes, the search takes the one that
//!   grants more wishes. Then each wish that the found row of threads allows is granted, in the order the
//!   wishes come, and the column waits for a later batch of its thread. The row allows a wish when it
//!   still sews every element with it: it has the fewest changes of any, so no batch of it is then empty.
//! - **Nests** (`REQ-AUTO-108`). A wish moves its second element later, and what sewed on top of that
//!   element would then lie under it: a detail of a detail, or a column that borders it. So a wish goes with
//!   its nest, the wishes that something sews after its second element, and theirs in turn, and a nest is
//!   granted whole or not at all. Of 2 rows with as few changes, the search first takes the one that misses
//!   fewer inner wishes, those that some other wish moves the first element of, so that what sews on top of
//!   a detail stays on top wherever a row allows it.
//!
//! Rows that are equal in both are taken in the order they are found: the threads of the free elements in
//! paint order.
//!
//! **Work.** A unit for each element looked at, and [`STATE`] units for each row kept, with the meter it is
//! given. A search that needs more gives none, and the caller sews the run as it did before, and says so.

use std::cmp::Reverse;
use std::collections::{BTreeMap, BTreeSet, BinaryHeap};

use stitchcraft_core::{Exhausted, Meter};
use stitchcraft_plan::Rgb;

/// The work a row of batches kept by the search costs beyond the elements it looks at, so that no design
/// makes the search hold more rows than memory has room for: the default budget has 500,000 of them.
pub(super) const STATE: u64 = 1_000;

/// The thread an element starts or ends in: its colour, or `None` for an element with no thread.
type Thread = Option<Rgb>;

/// A run of elements that sews in an order of its own, each element known by its place in the run.
pub(super) struct Run {
    /// The thread each element starts in, and the one it ends in.
    pub(super) threads: Vec<[Thread; 2]>,
    /// For each element, the elements of the run that must sew before it.
    pub(super) before: Vec<Vec<usize>>,
    /// The wishes: for each, the element that would sew first, and the one that would sew after it.
    pub(super) wished: Vec<(usize, usize)>,
    /// The thread sewing when the run starts.
    pub(super) sewing: Thread,
}

/// The order found for a run.
#[derive(Debug, PartialEq)]
pub(super) struct Found {
    /// The batches in the order they sew, each the places in the run of elements that sew one after
    /// another, in any order that the rules and the wishes granted allow.
    pub(super) batches: Vec<Vec<usize>>,
    /// The wishes granted: each sews its first element before its second.
    pub(super) granted: Vec<(usize, usize)>,
}

/// The batches `run` sews in with the fewest thread changes, and the wishes they grant, with `meter`'s
/// work.
pub(super) fn fewest(run: &Run, meter: &mut Meter) -> Result<Found, Exhausted> {
    let search = Search::new(run);
    let moves = search.row(meter)?;
    let mut before = run.before.clone();
    let mut batches = search.play(&before, &moves, meter)?.unwrap_or_default();
    // Each wish in turn, kept when the row still sews every element. A wish that a rule or a wish before it
    // forbids makes elements wait for each other, and no row sews those.
    // A wish is granted with its nest, whole or not at all (`REQ-AUTO-108`).
    let mut granted = Vec::new();
    for at in 0..run.wished.len() {
        let nest = nest(&run.wished, at, meter)?;
        let wished = nest.iter().fold(before.clone(), |before, &(first, then)| with(before, first, then));
        if let Some(played) = search.play(&wished, &moves, meter)? {
            (before, batches) = (wished, played);
            granted.extend(nest.into_iter().filter(|wish| !granted.contains(wish)).collect::<Vec<_>>());
        }
    }
    Ok(Found { batches, granted })
}

/// The nest of the wish at `at` of `wished`: that wish, each wish whose first element is the second element
/// of a wish of the nest, and so on, in the order found, with `meter`'s work: a unit for each wish looked
/// at.
fn nest(wished: &[(usize, usize)], at: usize, meter: &mut Meter) -> Result<Vec<(usize, usize)>, Exhausted> {
    let mut nest: Vec<(usize, usize)> = wished.get(at).copied().into_iter().collect();
    let mut next = 0;
    while let Some(&(_, moved)) = nest.get(next) {
        next += 1;
        for &wish in wished {
            meter.charge(1)?;
            if wish.0 == moved && !nest.contains(&wish) {
                nest.push(wish);
            }
        }
    }
    Ok(nest)
}

/// `before` with `first` among the elements that sew before `then`.
fn with(mut before: Vec<Vec<usize>>, first: usize, then: usize) -> Vec<Vec<usize>> {
    if let Some(of_then) = before.get_mut(then) {
        of_then.push(first);
    }
    before
}

/// The elements sewn so far, a bit for each.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct Done(Vec<u64>);

impl Done {
    /// None of `count` elements.
    fn none(count: usize) -> Done {
        Done(vec![0; count.div_ceil(64)])
    }

    /// Whether element `index` is sewn.
    fn has(&self, index: usize) -> bool {
        self.0.get(index / 64).is_some_and(|word| word >> (index % 64) & 1 == 1)
    }

    /// Element `index` is sewn.
    fn add(&mut self, index: usize) {
        if let Some(word) = self.0.get_mut(index / 64) {
            *word |= 1 << (index % 64);
        }
    }
}

/// What sews next in a row of batches.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Move {
    /// The batch of a thread: every element sewn in that thread alone that is free.
    Thread(Thread),
    /// An element sewn in several threads, and then the batch of the thread it ends in.
    Element(usize),
}

/// A row of batches as far as the search has it.
struct State {
    /// The elements sewn.
    done: Done,
    /// How many they are.
    sewn: usize,
    /// The thread sewing.
    thread: Thread,
    /// How many times the thread changed, how many inner wishes are missed, and how many wishes.
    cost: [usize; 3],
    /// The row it continues, as its place among the states, and what it sews after that row.
    from: Option<(usize, Move)>,
}

/// The search of a run.
struct Search<'r> {
    /// The run.
    run: &'r Run,
    /// The elements sewn in one thread alone, by that thread, in paint order.
    plain: BTreeMap<Thread, Vec<usize>>,
    /// For each element, the elements wished before it.
    wishes: Vec<Vec<usize>>,
    /// For each element, the elements wished before it by an inner wish: one whose first element is the
    /// second element of another wish.
    inner: Vec<Vec<usize>>,
}

impl<'r> Search<'r> {
    /// The search of `run`.
    fn new(run: &'r Run) -> Search<'r> {
        let mut plain: BTreeMap<Thread, Vec<usize>> = BTreeMap::new();
        for (index, [first, last]) in run.threads.iter().enumerate() {
            if first == last {
                plain.entry(*first).or_default().push(index);
            }
        }
        let (mut wishes, mut inner) = (vec![Vec::new(); run.threads.len()], vec![Vec::new(); run.threads.len()]);
        for &(first, then) in &run.wished {
            wishes = with(wishes, first, then);
            if run.wished.iter().any(|&(_, moved)| moved == first) {
                inner = with(inner, first, then);
            }
        }
        Search { run, plain, wishes, inner }
    }

    /// Whether element `index` is free to sew by `before`, the elements `done` sewn: not sewn itself, and
    /// every element before it sewn. A unit of `meter`'s work for each of those looked at.
    fn free(before: &[Vec<usize>], done: &Done, index: usize, meter: &mut Meter) -> Result<bool, Exhausted> {
        if done.has(index) {
            return Ok(false);
        }
        for &earlier in before.get(index).into_iter().flatten() {
            meter.charge(1)?;
            if !done.has(earlier) {
                return Ok(false);
            }
        }
        Ok(true)
    }

    /// Sews the batch of `thread` by `before` and gives it: the elements sewn in that thread alone that are
    /// free, and those they free, added to `done`. A unit of `meter`'s work for each element looked at.
    fn batch(&self, before: &[Vec<usize>], done: &mut Done, thread: Thread, meter: &mut Meter) -> Result<Vec<usize>, Exhausted> {
        let of_thread = self.plain.get(&thread).map(Vec::as_slice).unwrap_or_default();
        let mut batch = Vec::new();
        loop {
            let sewn = batch.len();
            for &index in of_thread {
                meter.charge(1)?;
                if Self::free(before, done, index, meter)? {
                    done.add(index);
                    batch.push(index);
                }
            }
            if batch.len() == sewn {
                return Ok(batch);
            }
        }
    }

    /// The thread that `step` starts in.
    fn starts(&self, step: Move) -> Thread {
        match step {
            Move::Thread(thread) => thread,
            Move::Element(index) => self.run.threads.get(index).and_then(|[first, _]| *first),
        }
    }

    /// Sews `step` by `before` after the elements `done`, `sewing` the thread sewing, and gives the batches
    /// it sews, or `None` when it sews nothing: a thread with no free element, or an element that isn't
    /// free.
    fn step(
        &self,
        before: &[Vec<usize>],
        done: &mut Done,
        sewing: &mut Thread,
        step: Move,
        meter: &mut Meter,
    ) -> Result<Option<Vec<Vec<usize>>>, Exhausted> {
        match step {
            Move::Thread(thread) => {
                *sewing = thread;
                let batch = self.batch(before, done, thread, meter)?;
                Ok((!batch.is_empty()).then(|| vec![batch]))
            }
            Move::Element(index) => {
                if !Self::free(before, done, index, meter)? {
                    return Ok(None);
                }
                done.add(index);
                *sewing = self.run.threads.get(index).and_then(|[_, last]| *last);
                let batch = self.batch(before, done, *sewing, meter)?;
                Ok(Some(if batch.is_empty() { vec![vec![index]] } else { vec![vec![index], batch] }))
            }
        }
    }

    /// What may sew next after the elements `done`: the thread of each free element sewn in one thread, once,
    /// and each free element sewn in several, in paint order.
    fn steps(&self, done: &Done, meter: &mut Meter) -> Result<Vec<Move>, Exhausted> {
        let (mut steps, mut threads) = (Vec::new(), BTreeSet::new());
        for (index, [first, last]) in self.run.threads.iter().enumerate() {
            meter.charge(1)?;
            if !Self::free(&self.run.before, done, index, meter)? {
                continue;
            }
            if first != last {
                steps.push(Move::Element(index));
            } else if threads.insert(*first) {
                steps.push(Move::Thread(*first));
            }
        }
        Ok(steps)
    }

    /// The fewest thread changes that the elements not `done` still need: one for each thread that an
    /// element sewn in that thread alone is left of, less one for each element sewn in several threads that
    /// is left, which may leave such a thread sewing.
    fn left(&self, done: &Done, meter: &mut Meter) -> Result<usize, Exhausted> {
        let (mut threads, mut several) = (BTreeSet::new(), 0_usize);
        for (index, [first, last]) in self.run.threads.iter().enumerate() {
            meter.charge(1)?;
            if done.has(index) {
                continue;
            }
            if first == last {
                threads.insert(*first);
            } else {
                several += 1;
            }
        }
        Ok(threads.len().saturating_sub(several))
    }

    /// How many inner wishes and how many wishes `batches`, sewn one after another after the elements
    /// `done`, miss: an element of theirs wished after one that isn't sewn by the end of its batch.
    fn missed(&self, mut done: Done, batches: &[Vec<usize>], meter: &mut Meter) -> Result<[usize; 2], Exhausted> {
        let mut missed = [0, 0];
        for batch in batches {
            for &index in batch {
                done.add(index);
            }
            for (count, wishes) in missed.iter_mut().zip([&self.inner, &self.wishes]) {
                for &earlier in batch.iter().filter_map(|&index| wishes.get(index)).flatten() {
                    meter.charge(1)?;
                    *count += usize::from(!done.has(earlier));
                }
            }
        }
        Ok(missed)
    }

    /// The row of batches with the fewest thread changes, and of those the fewest inner wishes missed and
    /// then the fewest wishes, as what sews after the batch of the thread sewing at the start.
    fn row(&self, meter: &mut Meter) -> Result<Vec<Move>, Exhausted> {
        let count = self.run.threads.len();
        let mut done = Done::none(count);
        let first = self.batch(&self.run.before, &mut done, self.run.sewing, meter)?;
        let [inner, missed] = self.missed(Done::none(count), std::slice::from_ref(&first), meter)?;
        let left = self.left(&done, meter)?;
        let mut best = BTreeMap::from([((done.clone(), self.run.sewing), 0_usize)]);
        let mut states = vec![State { done, sewn: first.len(), thread: self.run.sewing, cost: [0, inner, missed], from: None }];
        // The row to go on from: the fewest changes with those still needed, then the fewest inner wishes
        // missed and the fewest wishes, then the most changes made, which is nearest its end, then the row
        // found first.
        let mut open = BinaryHeap::from([Reverse((left, inner, missed, Reverse(0), 0))]);
        while let Some(Reverse((_, _, _, _, at))) = open.pop() {
            let Some(State { done, sewn, thread, cost, .. }) = states.get(at) else { continue };
            let (done, sewn, thread, cost) = (done.clone(), *sewn, *thread, *cost);
            // A row that a better one to the same elements and thread was found after is left.
            if best.get(&(done.clone(), thread)) != Some(&at) {
                continue;
            }
            if sewn == count {
                return Ok(self.steps_to(&states, at));
            }
            for step in self.steps(&done, meter)? {
                meter.charge(STATE)?;
                let (mut next, mut sewing) = (done.clone(), thread);
                let Some(batches) = self.step(&self.run.before, &mut next, &mut sewing, step, meter)? else { continue };
                let [changes, inner, missed] = cost;
                let [more_inner, more] = self.missed(done.clone(), &batches, meter)?;
                let cost = [changes + usize::from(self.starts(step) != thread), inner + more_inner, missed + more];
                let known = best.get(&(next.clone(), sewing)).and_then(|&known| states.get(known)).map(|known| known.cost);
                if known.is_some_and(|known| known <= cost) {
                    continue;
                }
                let left = self.left(&next, meter)?;
                best.insert((next.clone(), sewing), states.len());
                open.push(Reverse((cost[0] + left, cost[1], cost[2], Reverse(cost[0]), states.len())));
                let sewn = sewn + batches.iter().map(Vec::len).sum::<usize>();
                states.push(State { done: next, sewn, thread: sewing, cost, from: Some((at, step)) });
            }
        }
        // No row sews every element only when elements wait for each other, which the order's rules never
        // make them do: then no row is given, and playing none sews what it can.
        Ok(Vec::new())
    }

    /// What the row that ends in the state `at` of `states` sews, in order.
    fn steps_to(&self, states: &[State], at: usize) -> Vec<Move> {
        let mut steps = Vec::new();
        let mut at = Some(at);
        while let Some((earlier, step)) = at.and_then(|at| states.get(at)).and_then(|state| state.from) {
            steps.push(step);
            at = Some(earlier);
        }
        steps.reverse();
        steps
    }

    /// The batches that the row `steps` sews by `before`, after the batch of the thread sewing at the
    /// start; `None` when a step of it sews nothing or the row leaves an element unsewn.
    fn play(&self, before: &[Vec<usize>], steps: &[Move], meter: &mut Meter) -> Result<Option<Vec<Vec<usize>>>, Exhausted> {
        let count = self.run.threads.len();
        let (mut done, mut sewing) = (Done::none(count), self.run.sewing);
        let mut batches = vec![self.batch(before, &mut done, sewing, meter)?];
        for &step in steps {
            let Some(sewn) = self.step(before, &mut done, &mut sewing, step, meter)? else { return Ok(None) };
            batches.extend(sewn);
        }
        batches.retain(|batch| !batch.is_empty());
        Ok((batches.iter().map(Vec::len).sum::<usize>() == count).then_some(batches))
    }
}

#[cfg(test)]
mod tests;
