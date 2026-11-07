//! The runs that sew a part's full crosses (`docs/src/design/algorithms/cross-stitch.md` › Sewing the
//! crosses).
//!
//! **Places.** A run passes the corners of the grid's cells, their middles and the middles of their sides,
//! each a [`Place`] counted in half cells, so that one lattice holds them all: a corner at even numbers both
//! ways, a cell's middle at odd ones. A cross is named by its middle. A dense upright cross's second grid
//! lies half a cell over, so its crosses' middles are corners of the first grid, and their arms end at the
//! middles of its cells' sides, where the first grid's arms end too.
//!
//! **A cross is a loop** from one of its joins back to it, which the `bars` module lays out: the ends of its
//! top bar are good joins, from which nothing of the loop lies on top, and its other joins bad ones.
//!
//! **Crosses that meet sew in one run.** Crosses that share a join form a group: simple, double and Smyrna
//! crosses side by side or corner to corner, upright crosses side by side, and a dense upright cross's 2
//! grids through the arms they share. Each group is one run, from a join back to it. The run starts with a
//! cross at its join, one whose good join it is if it has one, and goes round its loop. At each place along
//! the run, before going on, it sews the loop of each cross not sewn yet whose good join lies there. Of
//! several, the cross lower down goes first, and of 2 level with each other the one further right, each
//! loop sewn the same way as it goes. Where the run has been round and crosses are left, it takes the first
//! place along it where a cross not sewn yet has a bad join, in the same order, sews that loop there, and
//! goes on as before. So few loops end on top, and every cross of the group is sewn, since each join of a
//! cross sewn lies on the run.
//!
//! **Groups in order.** The group with the join nearest the end is sewn last, from that join, so the part
//! ends there. The others go first, each the group with the join nearest the needle, from that join, and
//! with no needle the group of the first cross, row by row from the top, from that cross's first good join
//! clockwise from its top left. A part of one group with both a needle and an end first travels from the
//! join nearest the needle to the join nearest the end, from join to join through the middles of the fewest
//! crosses, and sews from there: the crosses sewn later cover the travel.
//!
//! **Cost.** Every step charges `meter`: each place the run passes, each cross looked for at a place, and
//! each join the travel reaches.

use std::collections::btree_map::Entry;
use std::collections::{BTreeMap, VecDeque};

use stitchcraft_core::{Exhausted, Meter, Point};

use super::bars::{Kind, Offset, loop_from};
use super::grid::Grid;

/// A place on the grid, in half cells: `[u, v]` lies `u` half cells right of the grid's origin and `v` half
/// cells down. Corners lie at even numbers both ways, the middles of cells at odd ones, and the middles of
/// their sides at one of each.
pub(crate) type Place = [i64; 2];

/// `place` moved by `offset`.
fn add([u, v]: Place, [du, dv]: Offset) -> Place {
    [u.saturating_add(du), v.saturating_add(dv)]
}

/// `place` moved back by `offset`: the middle of the cross whose join `offset` lies at `place`.
fn sub([u, v]: Place, [du, dv]: Offset) -> Place {
    [u.saturating_sub(du), v.saturating_sub(dv)]
}

/// The full crosses of a part, of one kind, by their middles, row by row from the top, each row from the
/// left.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Crosses {
    kind: Kind,
    middles: Vec<Place>,
}

impl Crosses {
    /// The crosses of `kind` whose middles are `middles`, in any order.
    pub(crate) fn new(kind: Kind, mut middles: Vec<Place>) -> Crosses {
        middles.sort_by_key(|&[u, v]| (v, u));
        middles.dedup();
        Crosses { kind, middles }
    }

    /// Their middles, row by row from the top, each row from the left.
    pub(crate) fn middles(&self) -> &[Place] {
        &self.middles
    }

    /// Whether there is none.
    pub(crate) fn is_empty(&self) -> bool {
        self.middles.is_empty()
    }

    /// Where the cross whose middle is `middle` lies in [`Crosses::middles`]; `None` when there is none.
    fn find(&self, [u, v]: Place) -> Option<usize> {
        self.middles.binary_search_by(|&[cu, cv]| (cv, cu).cmp(&(v, u))).ok()
    }

    /// The crosses that share a join with the one whose middle is `middle`, by their places and middles, row
    /// by row from the top, each row from the left, each once. One unit of `meter` for each cross looked for.
    fn neighbours(&self, middle: Place, meter: &mut Meter) -> Result<Vec<(usize, Place)>, Exhausted> {
        let joins = self.kind.joins();
        let mut found = Vec::new();
        for &join in joins {
            let place = add(middle, join);
            for &other in joins {
                meter.charge(1)?;
                let neighbour = sub(place, other);
                if neighbour != middle
                    && let Some(k) = self.find(neighbour)
                {
                    found.push((k, neighbour));
                }
            }
        }
        found.sort_by_key(|&(_, [u, v])| (v, u));
        found.dedup();
        Ok(found)
    }
}

/// The runs that sew `crosses` on `grid`, each a list of places from its first, with `repeats` passes back
/// and forth along each bar: one run for each group of crosses that share joins, in the order the module's
/// docs give, from the `needle` and to end nearest `end` where they are known.
pub(crate) fn runs(
    crosses: &Crosses,
    grid: &Grid,
    repeats: u32,
    (needle, end): (Option<Point>, Option<Point>),
    meter: &mut Meter,
) -> Result<Vec<Vec<Place>>, Exhausted> {
    let groups = groups(crosses, meter)?;
    let mut pending = vec![true; crosses.middles.len()];
    let all: Vec<usize> = (0..groups.len()).collect();
    let last = end.map(|end| nearest(crosses, &groups, &all, grid, end, meter)).transpose()?.flatten();
    let mut left: Vec<usize> = all.into_iter().filter(|&group| last.is_none_or(|(sewn_last, _)| sewn_last != group)).collect();
    let mut runs = Vec::with_capacity(groups.len());
    let mut at = needle;
    let first_good = crosses.kind.first_good();
    // Each group is sewn once: a group a turn, until none is left.
    for _ in 0..groups.len() {
        let next = match at {
            Some(p) => nearest(crosses, &groups, &left, grid, p, meter)?,
            None => left.first().and_then(|&group| Some((group, add(*groups.get(group)?.first()?, first_good)))),
        };
        let Some((group, start)) = next else { break };
        left.retain(|&other| other != group);
        runs.push(run(start, crosses, &mut pending, repeats, meter)?);
        at = Some(grid.place(start));
    }
    if let Some((group, start)) = last {
        let mut sewn = vec![start];
        if let (Some(needle), [_]) = (needle, groups.as_slice()) {
            let from = nearest(crosses, &groups, &[group], grid, needle, meter)?.map_or(start, |(_, join)| join);
            sewn = travel(from, start, crosses, meter)?;
        }
        sewn.extend(run(start, crosses, &mut pending, repeats, meter)?.into_iter().skip(1));
        runs.push(sewn);
    }
    Ok(runs)
}

/// The groups of `crosses` that share joins, each its crosses' middles in the order they were reached from
/// its first, the groups in the order of their first crosses, row by row from the top. One unit of `meter`
/// for each cross, and as [`Crosses::neighbours`] charges for each cross of a group.
fn groups(crosses: &Crosses, meter: &mut Meter) -> Result<Vec<Vec<Place>>, Exhausted> {
    let middles = crosses.middles();
    let mut seen = vec![false; middles.len()];
    let mut groups = Vec::new();
    for (k, &middle) in middles.iter().enumerate() {
        meter.charge(1)?;
        if seen.get(k).copied().unwrap_or(true) {
            continue;
        }
        if let Some(mark) = seen.get_mut(k) {
            *mark = true;
        }
        let mut group = vec![middle];
        // Each cross of the group is looked round once, and the group holds no more crosses than there are.
        for next in 0..middles.len() {
            let Some(&from) = group.get(next) else { break };
            for (n, neighbour) in crosses.neighbours(from, meter)? {
                if let Some(mark) = seen.get_mut(n)
                    && !*mark
                {
                    *mark = true;
                    group.push(neighbour);
                }
            }
        }
        groups.push(group);
    }
    Ok(groups)
}

/// Of the groups `among`, the one with the join nearest `p` on `grid`, and that join: by its place in
/// `among`, and the join of its crosses in their order, each cross's joins clockwise from its top left, the
/// first of those equally near. One unit of `meter` for each cross.
fn nearest(
    crosses: &Crosses,
    groups: &[Vec<Place>],
    among: &[usize],
    grid: &Grid,
    p: Point,
    meter: &mut Meter,
) -> Result<Option<(usize, Place)>, Exhausted> {
    let mut best: Option<(f64, usize, Place)> = None;
    for &group in among {
        for &middle in groups.get(group).map_or(&[][..], Vec::as_slice) {
            meter.charge(1)?;
            for &offset in crosses.kind.joins() {
                let join = add(middle, offset);
                let d = grid.place(join).distance(p);
                if best.is_none_or(|(least, ..)| d < least) {
                    best = Some((d, group, join));
                }
            }
        }
    }
    Ok(best.map(|(_, group, join)| (group, join)))
}

/// A run being built: its places linked in sewing order, so that a loop goes in after any of them at once.
struct Linked {
    spots: Vec<Place>,
    next: Vec<Option<usize>>,
}

impl Linked {
    /// The place after `at`.
    fn after(&self, at: usize) -> Option<usize> {
        self.next.get(at).copied().flatten()
    }

    /// Puts `loop_` in after the place `at`.
    fn insert(&mut self, at: usize, loop_: Vec<Place>) {
        let after = self.after(at);
        let mut previous = at;
        for spot in loop_ {
            let new = self.spots.len();
            self.spots.push(spot);
            self.next.push(after);
            if let Some(link) = self.next.get_mut(previous) {
                *link = Some(new);
            }
            previous = new;
        }
    }

    /// The places from the first, in sewing order: each once.
    fn in_order(&self) -> Vec<Place> {
        let mut spots = Vec::with_capacity(self.spots.len());
        let mut at = Some(0);
        for _ in 0..self.spots.len() {
            let Some(k) = at else { break };
            spots.extend(self.spots.get(k).copied());
            at = self.after(k);
        }
        spots
    }
}

/// The run that sews, from the join `start` back to it, the crosses not sewn yet that share joins with a
/// cross at `start`, as the module's docs give it, `pending` saying which are not sewn yet.
fn run(start: Place, crosses: &Crosses, pending: &mut [bool], repeats: u32, meter: &mut Meter) -> Result<Vec<Place>, Exhausted> {
    let kind = crosses.kind;
    let mut linked = Linked { spots: vec![start], next: vec![None] };
    let mut take = |middle: Place| crosses.find(middle).and_then(|k| pending.get_mut(k)).is_some_and(|left| std::mem::replace(left, false));
    if let Some((join, middle)) = kind.good().iter().chain(kind.bad()).map(|&join| (join, sub(start, join))).find(|&(_, middle)| take(middle)) {
        linked.insert(0, loop_from(kind, middle, join, repeats));
    }
    walk(&mut linked, (0, None), kind, &mut take, repeats, meter)?;
    let mut from = linked.after(0);
    // Each turn sews a cross not sewn yet.
    for _ in 0..crosses.middles.len() {
        let Some((k, join, middle)) = first_bad(&linked, from, kind, &mut take, meter)? else { break };
        let after = linked.after(k);
        linked.insert(k, loop_from(kind, middle, join, repeats));
        walk(&mut linked, (k, after), kind, &mut take, repeats, meter)?;
        from = Some(k);
    }
    Ok(linked.in_order())
}

/// A cross found along a run: the place it joins the run at, its join there, and its middle.
type Found = (usize, Offset, Place);

/// The first place along `linked` from the place `from` where `take` gives a cross of `kind` whose bad join
/// lies there: the place, the join and the cross's middle. One unit of `meter` for each place passed.
fn first_bad(
    linked: &Linked,
    from: Option<usize>,
    kind: Kind,
    take: &mut impl FnMut(Place) -> bool,
    meter: &mut Meter,
) -> Result<Option<Found>, Exhausted> {
    let mut at = from;
    while let Some(k) = at {
        meter.charge(1)?;
        if let Some(place) = linked.spots.get(k).copied()
            && let Some((join, middle)) = kind.bad().iter().map(|&join| (join, sub(place, join))).find(|&(_, middle)| take(middle))
        {
            return Ok(Some((k, join, middle)));
        }
        at = linked.after(k);
    }
    Ok(None)
}

/// Goes along `linked` from the place after `from` up to `until`, or to its end: at each place, the loop of
/// each cross of `kind` that `take` gives whose good join lies there goes in after it, the first to be sewn
/// first, and the walk goes on along the loops.
fn walk(
    linked: &mut Linked,
    (from, until): (usize, Option<usize>),
    kind: Kind,
    take: &mut impl FnMut(Place) -> bool,
    repeats: u32,
    meter: &mut Meter,
) -> Result<(), Exhausted> {
    let mut at = linked.after(from);
    while let Some(k) = at.filter(|&k| Some(k) != until) {
        meter.charge(1)?;
        // `after` only gives places that `linked` holds, so the walk never stops here.
        let Some(place) = linked.spots.get(k).copied() else { break };
        for &join in kind.good().iter().rev() {
            let middle = sub(place, join);
            if take(middle) {
                linked.insert(k, loop_from(kind, middle, join, repeats));
            }
        }
        at = linked.after(k);
    }
    Ok(())
}

/// A step of a travel: the join it starts from and the middle of the cross it passes through.
type Step = (Place, Place);

/// The way from the join `from` to the join `to` through `crosses`, its places from `from`: from join to join
/// of a cross through its middle, through the fewest crosses, each join reached first from the join before
/// it in the order the way is searched: the crosses at a join row by row from the top, each row from the
/// left, and each cross's joins clockwise from its top left. One unit of `meter` for each cross looked at.
fn travel(from: Place, to: Place, crosses: &Crosses, meter: &mut Meter) -> Result<Vec<Place>, Exhausted> {
    let joins = crosses.kind.joins();
    // Each join reached, by the join and the cross it was reached from: none for `from`.
    let mut came: BTreeMap<Place, Option<Step>> = BTreeMap::from([(from, None)]);
    let mut queue = VecDeque::from([from]);
    while let Some(join) = queue.pop_front().filter(|_| !came.contains_key(&to)) {
        let mut around: Vec<Place> = joins.iter().map(|&offset| sub(join, offset)).collect();
        around.sort_by_key(|&[u, v]| (v, u));
        for middle in around {
            meter.charge(1)?;
            if crosses.find(middle).is_none() {
                continue;
            }
            for &offset in joins {
                if let Entry::Vacant(unreached) = came.entry(add(middle, offset)) {
                    unreached.insert(Some((join, middle)));
                    queue.push_back(add(middle, offset));
                }
            }
        }
    }
    let mut way = vec![to];
    let mut at = to;
    while let Some(&Some((before, middle))) = came.get(&at) {
        way.extend([middle, before]);
        at = before;
    }
    way.reverse();
    Ok(way)
}

#[cfg(test)]
mod tests;
