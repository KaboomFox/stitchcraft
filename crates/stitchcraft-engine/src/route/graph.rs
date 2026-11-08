//! The graph a route is found in (`docs/src/design/routing.md` › Auto-satin): places, the points where
//! pieces end, joined by links, the pieces themselves and the jumps that join pieces lying apart.
//!
//! **Why a graph of its own, in its order.** Ink/Stitch finds its route with networkx (read at `d59c9ab`),
//! whose searches meet each place's links in the order they were added. Where 2 ways are as short, the
//! first met is taken. StitchCraft keeps the same order, so that both programs take the same way: places
//! in the order the pieces' ends are added, element by element and along each, and each place's links in
//! the order they were added, the jumps after the pieces. 2 ends at the same point, to the last bit, are
//! one place. A place belongs to the last element whose piece ends there.
//!
//! **The searches** are published algorithms, written from their descriptions:
//!
//! - The jumps that join the groups of pieces lying apart, with Preserve order off, are a minimum spanning
//!   tree of the groups (J. B. Kruskal, *On the shortest spanning subtree of a graph*, 1956), as networkx's
//!   `k_edge_augmentation` gives it for a graph made connected once. 2 groups can be joined by the jump
//!   between their nearest places, the first place of the first group and the first of the second of
//!   equally near pairs. Of jumps as short, the one between earlier groups is taken first, a group being as
//!   early as its first place. The jumps are added group by group, each group's in the order the tree took
//!   them, as networkx lists the tree's edges.
//! - The way from the start to the end with the fewest links is found by a breadth-first search from both
//!   ends at once, as networkx's `bidirectional_shortest_path` searches: the side with fewer places to look
//!   from goes next, the start's side when they are as many, and the ways meet at the first place both
//!   have reached.
//! - The rest is sewn depth first from each place along the way, as networkx's `dfs_labeled_edges` walks a
//!   graph without directions (R. Tarjan, *Depth-first search and linear graph algorithms*, 1972): out
//!   along each link to a place not yet reached and back, and there and back along a link to a place
//!   already reached. Ink/Stitch searches a copy of its graph without directions, whose places list their
//!   links in a new order: first those to places added before, in their order, then their own in theirs.
//!
//! Every link looked at charges a unit of the meter's work.

use std::collections::btree_map::Entry;
use std::collections::{BTreeMap, BTreeSet};

use stitchcraft_core::{Exhausted, Meter, Point};

/// No link: what a search's start was reached by.
const NONE: usize = usize::MAX;

/// A place: a point where pieces end.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct Place {
    /// Where it is.
    pub(super) at: Point,
    /// The element whose piece ends there, the last added of those that do.
    pub(super) element: usize,
}

/// A link between 2 places.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Link {
    /// The piece `index` along the element `element`, from the first place to the second.
    Piece {
        /// The element it is a piece of.
        element: usize,
        /// Its place along it, from 0.
        index: usize,
        /// The places at its start and at its end.
        ends: [usize; 2],
    },
    /// A jump between 2 places.
    Jump {
        /// The places it joins.
        ends: [usize; 2],
    },
}

/// A way through the graph: its places in order, and the links between them.
pub(super) type Way = (Vec<usize>, Vec<usize>);

/// A step of a route: along `link` from the place `from` to the place `to`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Step {
    /// Where it starts.
    pub(super) from: usize,
    /// Where it ends.
    pub(super) to: usize,
    /// The link it follows.
    pub(super) link: usize,
}

/// What a depth-first search meets, as networkx labels it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Met {
    /// A link out to a place not yet reached.
    Forward,
    /// The link back from a place left, to the place it was reached from.
    Reverse,
    /// A link to a place already reached.
    Nontree,
}

/// The places and links of a route.
#[derive(Clone, Debug, Default)]
pub(super) struct Graph {
    places: Vec<Place>,
    /// Each place's number by its point's coordinates, as bits: 0 and -0 are one.
    index: BTreeMap<(u64, u64), usize>,
    links: Vec<Link>,
    /// Each place's links out, with the place each leads to, in the order they were added.
    out: Vec<Vec<(usize, usize)>>,
    /// Each place's links in, with the place each comes from, in the order they were added; with no
    /// directions, its links out.
    into: Vec<Vec<(usize, usize)>>,
    /// Whether a jump goes one way only, as with Preserve order.
    directed: bool,
}

impl Graph {
    /// A graph with nothing in it, whose jumps go one way only when `directed`.
    pub(super) fn new(directed: bool) -> Graph {
        Graph { directed, ..Graph::default() }
    }

    /// The places.
    pub(super) fn places(&self) -> &[Place] {
        &self.places
    }

    /// The link `link`.
    pub(super) fn link(&self, link: usize) -> Option<Link> {
        self.links.get(link).copied()
    }

    /// The place at `at`, added for `element` when there is none, which it now belongs to.
    fn place(&mut self, at: Point, element: usize) -> usize {
        let key = |v: f64| (v + 0.0).to_bits();
        let count = self.places.len();
        let place = *self.index.entry((key(at.x()), key(at.y()))).or_insert(count);
        if place == count {
            self.places.push(Place { at, element });
            self.out.push(Vec::new());
            self.into.push(Vec::new());
        } else if let Some(found) = self.places.get_mut(place) {
            found.element = element;
        }
        place
    }

    /// Links `from` to `to` by `link`, a way each way for a piece; a jump of a graph with directions goes one
    /// way.
    fn join(&mut self, from: usize, to: usize, link: usize, both_ways: bool) {
        let mut add = |a: usize, b: usize| {
            if let Some(out) = self.out.get_mut(a) {
                out.push((b, link));
            }
            let into = if self.directed { self.into.get_mut(b) } else { self.out.get_mut(b).filter(|_| a != b) };
            if let Some(into) = into {
                into.push((a, link));
            }
        };
        add(from, to);
        if self.directed && both_ways {
            add(to, from);
        }
    }

    /// Adds the piece `index` along `element`, from `start` to `end`.
    pub(super) fn piece(&mut self, element: usize, index: usize, [start, end]: [Point; 2]) {
        let ends = [self.place(start, element), self.place(end, element)];
        self.links.push(Link::Piece { element, index, ends });
        self.join(ends[0], ends[1], self.links.len() - 1, true);
    }

    /// Adds a jump from `from` to `to`, which a graph with directions takes only that way, unless a link leads
    /// that way already: Ink/Stitch's graph holds one link from a place to another, and a jump added where one
    /// is takes its place in it. Each link looked at charges `meter` a unit.
    fn jump(&mut self, from: usize, to: usize, meter: &mut Meter) -> Result<(), Exhausted> {
        if !self.linked(from, to, meter)? {
            self.links.push(Link::Jump { ends: [from, to] });
            self.join(from, to, self.links.len() - 1, false);
        }
        Ok(())
    }

    /// Adds a jump between `a` and `b` that the route can take either way, as it takes a piece, unless links
    /// lead both ways already. Each link looked at charges `meter` a unit.
    fn jump_both_ways(&mut self, a: usize, b: usize, meter: &mut Meter) -> Result<(), Exhausted> {
        if !(self.linked(a, b, meter)? && self.linked(b, a, meter)?) {
            self.links.push(Link::Jump { ends: [a, b] });
            self.join(a, b, self.links.len() - 1, true);
        }
        Ok(())
    }

    /// Whether a link leads from `from` to `to`. Each link looked at charges `meter` a unit.
    fn linked(&self, from: usize, to: usize, meter: &mut Meter) -> Result<bool, Exhausted> {
        for &(next, _) in self.links_out(from) {
            meter.charge(1)?;
            if next == to {
                return Ok(true);
            }
        }
        Ok(false)
    }

    /// The links into `place`, with the place each comes from.
    fn links_into(&self, place: usize) -> &[(usize, usize)] {
        let lists = if self.directed { &self.into } else { &self.out };
        lists.get(place).map_or(&[], Vec::as_slice)
    }

    /// The links out of `place`, with the place each leads to.
    fn links_out(&self, place: usize) -> &[(usize, usize)] {
        self.out.get(place).map_or(&[], Vec::as_slice)
    }

    /// The places where `element`'s pieces end, in order. Each link looked at charges `meter` a unit.
    pub(super) fn places_on(&self, element: usize, meter: &mut Meter) -> Result<Vec<usize>, Exhausted> {
        let mut on = Vec::new();
        for link in &self.links {
            meter.charge(1)?;
            if let Link::Piece { element: of, ends, .. } = link
                && *of == element
            {
                on.extend(ends);
            }
        }
        on.sort_unstable();
        on.dedup();
        Ok(on)
    }

    /// The groups of places joined by links, each in order, in the order of their first places.
    fn groups(&self, meter: &mut Meter) -> Result<Vec<Vec<usize>>, Exhausted> {
        let mut group_of = vec![NONE; self.places.len()];
        let mut groups: Vec<Vec<usize>> = Vec::new();
        for first in 0..self.places.len() {
            if group_of.get(first) != Some(&NONE) {
                continue;
            }
            let number = groups.len();
            let (mut members, mut next) = (vec![first], vec![first]);
            if let Some(slot) = group_of.get_mut(first) {
                *slot = number;
            }
            while let Some(place) = next.pop() {
                for &(other, _) in self.links_out(place).iter().chain(self.links_into(place)) {
                    meter.charge(1)?;
                    if let Some(slot) = group_of.get_mut(other).filter(|slot| **slot == NONE) {
                        *slot = number;
                        members.push(other);
                        next.push(other);
                    }
                }
            }
            members.sort_unstable();
            groups.push(members);
        }
        Ok(groups)
    }

    /// Joins the groups of places that lie apart by the jumps of least length in all, a minimum spanning tree
    /// of the groups (see the module's docs). Each pair of places measured charges `meter` a unit.
    pub(super) fn join_apart(&mut self, meter: &mut Meter) -> Result<(), Exhausted> {
        let groups = self.groups(meter)?;
        let mut jumps: Vec<(f64, usize, usize, usize, usize)> = Vec::new();
        for (i, first) in groups.iter().enumerate() {
            for (j, second) in groups.iter().enumerate().skip(i + 1) {
                if let Some((length, a, b)) = self.nearest(first, second, meter)? {
                    jumps.push((length, i, j, a, b));
                }
            }
        }
        jumps.sort_by(|a, b| a.0.total_cmp(&b.0));
        let mut joined: Vec<usize> = (0..groups.len()).collect();
        let mut taken: Vec<(usize, usize, usize, usize)> = Vec::new();
        for (_, i, j, a, b) in jumps {
            let (root_i, root_j) = (root(&mut joined, i, meter)?, root(&mut joined, j, meter)?);
            if root_i != root_j {
                if let Some(slot) = joined.get_mut(root_j) {
                    *slot = root_i;
                }
                taken.push((i, j, a, b));
            }
        }
        // Group by group, each group's jumps to later groups in the order the tree took them.
        for group in 0..groups.len() {
            for &(i, _, a, b) in &taken {
                meter.charge(1)?;
                if i == group {
                    self.jump(a, b, meter)?;
                }
            }
        }
        Ok(())
    }

    /// The nearest places of `first` and `second`, and how far apart they are: the first place of `first`
    /// and then of `second` of equally near ones; `None` when one of them has none.
    fn nearest(&self, first: &[usize], second: &[usize], meter: &mut Meter) -> Result<Option<(f64, usize, usize)>, Exhausted> {
        let mut best: Option<(f64, usize, usize)> = None;
        for &a in first {
            for &b in second {
                meter.charge(1)?;
                let length = self.distance(a, b);
                if best.is_none_or(|(least, ..)| length < least) {
                    best = Some((length, a, b));
                }
            }
        }
        Ok(best)
    }

    /// How far apart places `a` and `b` are.
    fn distance(&self, a: usize, b: usize) -> f64 {
        self.places.get(a).zip(self.places.get(b)).map_or(f64::INFINITY, |(a, b)| a.at.distance(b.at))
    }

    /// Joins each of `elements` to the next by the shortest jump between their places, from the first's to
    /// the second's, which the route can take only forwards: Preserve order.
    pub(super) fn join_in_order(&mut self, elements: &[usize], meter: &mut Meter) -> Result<(), Exhausted> {
        for pair in elements.windows(2) {
            let &[from, to] = pair else { continue };
            let (from, to) = (self.places_on(from, meter)?, self.places_on(to, meter)?);
            if let Some((_, a, b)) = self.nearest(&from, &to, meter)? {
                self.jump(a, b, meter)?;
            }
        }
        Ok(())
    }

    /// Joins the parts of `element` that its own pieces don't join, each to each, by the shortest jump between
    /// their places, which the route can take either way: with Preserve order, the subpaths of a stroke that
    /// lie apart. The parts come in the order of their first places, and each pair is joined at its nearest
    /// places, the first of equally near ones ([`Graph::nearest`]). Each link and each pair of places looked
    /// at charges `meter` a unit.
    pub(super) fn join_parts(&mut self, element: usize, meter: &mut Meter) -> Result<(), Exhausted> {
        let mut next: BTreeMap<usize, Vec<usize>> = BTreeMap::new();
        for link in &self.links {
            meter.charge(1)?;
            if let Link::Piece { element: of, ends: [a, b], .. } = *link
                && of == element
            {
                next.entry(a).or_default().push(b);
                next.entry(b).or_default().push(a);
            }
        }
        let mut seen = BTreeSet::new();
        let mut parts: Vec<Vec<usize>> = Vec::new();
        for &first in next.keys() {
            if !seen.insert(first) {
                continue;
            }
            let (mut part, mut todo) = (vec![first], vec![first]);
            while let Some(place) = todo.pop() {
                for &other in next.get(&place).map_or(&[][..], Vec::as_slice) {
                    meter.charge(1)?;
                    if seen.insert(other) {
                        part.push(other);
                        todo.push(other);
                    }
                }
            }
            part.sort_unstable();
            parts.push(part);
        }
        let mut jumps = Vec::new();
        for (i, first) in parts.iter().enumerate() {
            for second in parts.iter().skip(i + 1) {
                jumps.extend(self.nearest(first, second, meter)?.map(|(_, a, b)| (a, b)));
            }
        }
        for (a, b) in jumps {
            self.jump_both_ways(a, b, meter)?;
        }
        Ok(())
    }

    /// The place a route starts or ends at: of `among`, or else of every place, the one nearest `point`, or
    /// without one the leftmost, or the rightmost when not `leftmost`; the first of equally placed ones.
    pub(super) fn end(&self, point: Option<Point>, leftmost: bool, among: Option<&[usize]>) -> Option<usize> {
        let all: Vec<usize> = (0..self.places.len()).collect();
        let among = among.unwrap_or(&all);
        let score = |place: &usize| {
            let at = self.places.get(*place).map_or(Point::ORIGIN, |place| place.at);
            match point {
                Some(point) => at.distance(point),
                None if leftmost => at.x(),
                None => -at.x(),
            }
        };
        among.iter().copied().reduce(|best, place| if score(&place) < score(&best) { place } else { best })
    }

    /// The way from `start` to `end` with the fewest links, its places and the links between them, found as
    /// the module's docs say; `None` when there is none. Each link looked at charges `meter` a unit.
    pub(super) fn shortest(&self, start: usize, end: usize, meter: &mut Meter) -> Result<Option<Way>, Exhausted> {
        if start == end {
            return Ok(Some((vec![start], Vec::new())));
        }
        // How each place reached was reached: the place before and the link, from the start's side and from
        // the end's.
        let mut before: BTreeMap<usize, (usize, usize)> = BTreeMap::from([(start, (NONE, NONE))]);
        let mut after: BTreeMap<usize, (usize, usize)> = BTreeMap::from([(end, (NONE, NONE))]);
        let (mut forward, mut back) = (vec![start], vec![end]);
        let met = 'search: loop {
            if forward.is_empty() || back.is_empty() {
                return Ok(None);
            }
            if forward.len() <= back.len() {
                for place in std::mem::take(&mut forward) {
                    for &(next, link) in self.links_out(place) {
                        meter.charge(1)?;
                        if let Entry::Vacant(slot) = before.entry(next) {
                            slot.insert((place, link));
                            forward.push(next);
                        }
                        if after.contains_key(&next) {
                            break 'search next;
                        }
                    }
                }
            } else {
                for place in std::mem::take(&mut back) {
                    for &(previous, link) in self.links_into(place) {
                        meter.charge(1)?;
                        if let Entry::Vacant(slot) = after.entry(previous) {
                            slot.insert((place, link));
                            back.push(previous);
                        }
                        if before.contains_key(&previous) {
                            break 'search previous;
                        }
                    }
                }
            }
        };
        let (mut places, mut links) = (vec![met], Vec::new());
        let mut at = met;
        while let Some(&(place, link)) = before.get(&at).filter(|(place, _)| *place != NONE) {
            places.push(place);
            links.push(link);
            at = place;
        }
        places.reverse();
        links.reverse();
        at = met;
        while let Some(&(place, link)) = after.get(&at).filter(|(place, _)| *place != NONE) {
            places.push(place);
            links.push(link);
            at = place;
        }
        Ok(Some((places, links)))
    }

    /// The links of each place in a copy of the graph without directions, as networkx copies one: places
    /// in their order, each listing first its links with places before it, in those places' order, then its
    /// own links out, in theirs. A piece is one link, whichever way it is listed.
    fn undirected(&self, meter: &mut Meter) -> Result<Vec<Vec<(usize, usize)>>, Exhausted> {
        let mut lists: Vec<Vec<(usize, usize)>> = vec![Vec::new(); self.places.len()];
        let mut listed = vec![false; self.links.len()];
        for place in 0..self.places.len() {
            for &(other, link) in self.links_out(place) {
                meter.charge(1)?;
                let Some(slot) = listed.get_mut(link).filter(|listed| !**listed) else { continue };
                *slot = true;
                if let Some(list) = lists.get_mut(place) {
                    list.push((other, link));
                }
                if let Some(list) = lists.get_mut(other).filter(|_| other != place) {
                    list.push((place, link));
                }
            }
        }
        Ok(lists)
    }

    /// The route along the way `places`, joined by `links`, that sews every link: at each place of the way,
    /// each link that can be reached from there without the way's own is walked depth first, out and back
    /// (see the module's docs). Each link looked at charges `meter` a unit.
    pub(super) fn walk(&self, places: &[usize], links: &[usize], meter: &mut Meter) -> Result<Vec<Step>, Exhausted> {
        let lists = self.undirected(meter)?;
        let mut gone = vec![false; self.links.len()];
        for &link in links {
            take(&mut gone, link);
        }
        let mut steps = Vec::new();
        for (k, &place) in places.iter().enumerate() {
            if let (Some(&from), Some(&link)) = (places.get(k.wrapping_sub(1)), links.get(k.wrapping_sub(1))) {
                steps.push(Step { from, to: place, link });
            }
            for (from, to, link, met) in depth_first(&lists, &gone, place, meter)? {
                match met {
                    _ if from == to => {}
                    Met::Forward => {
                        steps.push(Step { from, to, link });
                        take(&mut gone, link);
                    }
                    Met::Reverse => steps.push(Step { from: to, to: from, link }),
                    Met::Nontree => {
                        if take(&mut gone, link) {
                            steps.push(Step { from, to, link });
                            steps.push(Step { from: to, to: from, link });
                        }
                    }
                }
            }
        }
        Ok(steps)
    }
}

/// Marks `link` gone from the links still to sew: whether it was there to take.
fn take(gone: &mut [bool], link: usize) -> bool {
    gone.get_mut(link).is_some_and(|slot| !std::mem::replace(slot, true))
}

/// The root of `group` among the groups `joined` has joined, each pointing to one it was joined to, and
/// each group on the way pointed past the one it points to (path halving, R. E. Tarjan and J. van Leeuwen,
/// *Worst-case analysis of set union algorithms*, 1984). Each step charges `meter` a unit.
fn root(joined: &mut [usize], mut group: usize, meter: &mut Meter) -> Result<usize, Exhausted> {
    while let Some(&up) = joined.get(group).filter(|up| **up != group) {
        meter.charge(1)?;
        let past = joined.get(up).copied().unwrap_or(up);
        if let Some(slot) = joined.get_mut(group) {
            *slot = past;
        }
        group = past;
    }
    Ok(group)
}

/// What a depth-first search from `start` meets among `lists`, the links of each place, but those `gone`,
/// in order, as networkx's `dfs_labeled_edges` labels them: for each link met, the place it is met from,
/// the place it leads to, the link and how it is met. A link back from a place left is met from the place
/// it was reached from. Each link looked at charges `meter` a unit.
fn depth_first(lists: &[Vec<(usize, usize)>], gone: &[bool], start: usize, meter: &mut Meter) -> Result<Vec<(usize, usize, usize, Met)>, Exhausted> {
    let mut reached = vec![false; lists.len()];
    if let Some(slot) = reached.get_mut(start) {
        *slot = true;
    }
    let mut met = Vec::new();
    // Each place being walked from: the place, how many of its links are looked at, and the link it was
    // reached by.
    let mut stack: Vec<(usize, usize, usize)> = vec![(start, 0, NONE)];
    while let Some(&(place, looked, by)) = stack.last() {
        let list = lists.get(place).map_or(&[][..], Vec::as_slice);
        let mut next = None;
        let mut at = looked;
        while let Some(&(other, link)) = list.get(at) {
            at += 1;
            meter.charge(1)?;
            if gone.get(link).copied().unwrap_or(true) {
                continue;
            }
            if reached.get(other).copied().unwrap_or(true) {
                met.push((place, other, link, Met::Nontree));
            } else {
                met.push((place, other, link, Met::Forward));
                if let Some(slot) = reached.get_mut(other) {
                    *slot = true;
                }
                next = Some((other, link));
                break;
            }
        }
        if let Some(top) = stack.last_mut() {
            top.1 = at;
        }
        match next {
            Some((other, link)) => stack.push((other, 0, link)),
            None => {
                stack.pop();
                if let Some(&(parent, ..)) = stack.last() {
                    met.push((parent, place, by, Met::Reverse));
                }
            }
        }
    }
    Ok(met)
}

#[cfg(test)]
mod tests;
