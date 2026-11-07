//! The bars a full cross is made of, where it joins a run, and the loop that sews it
//! (`docs/src/design/algorithms/cross-stitch.md` › A cross).
//!
//! **Bars.** A full cross is a stack of bars through the middle of its cell, sewn from the bottom up: a
//! simple cross's `\` diagonal and its `/` diagonal over it, an upright cross's bar across and its bar
//! down over it, a double cross's `\` diagonal, its upright cross and its `/` diagonal, and a Smyrna
//! cross's 2 diagonals and its upright cross over them. Places are counted in half cells from the cross's
//! middle ([`Offset`]), so that one lattice holds every corner, middle and middle of a side. A flipped
//! cross is a plain one turned a quarter turn, which the caller sees to (`super::Frame`).
//!
//! **Joins.** A cross joins the run that sews it at the ends of some of its bars: a simple or a double
//! cross at its corners, an upright cross at the middles of its sides, and a Smyrna cross at its corners
//! and the middles of its top and bottom sides. The ends of its top bar are its good joins. A loop from
//! one goes in to the middle along the top bar, which covers that stitch later, so nothing of the loop
//! lies on top. Its other joins are bad: a loop from one ends with a stitch from the middle back to it, on
//! top.
//!
//! **The loop** from a join sews each bar `n` times back and forth and ends where it started:
//!
//! - From an end of the top bar, it goes in to the middle, sews each bar below the top one in turn from its
//!   first end, back to the middle after each, and then the top bar from its far end to the start.
//! - From an end of the bottom bar, it sews that bar from the start, ending at its far end, goes to the
//!   middle, sews each bar above in turn from its first end, back to the middle after each, and ends with
//!   the stitch from the middle to the start.
//! - From an end of a bar between, a Smyrna cross's `/` diagonal, it goes in to the middle and sews each
//!   bar in turn, that one from the middle to its far end and back and forth 1 time fewer, ending there,
//!   and ends with the stitch from the middle to the start.
//!
//! A bar's first end is the one the needle goes to first from the middle: the `\` diagonal's top end, or
//! its bottom end from the bottom left corner. The `/` diagonal's top end, but a Smyrna cross's bottom
//! end, except from the bottom right corner. The left end of the bar across. The top end of the bar down,
//! but a Smyrna cross's bottom end from the top left corner. Each end that way, a loop is the one Ink/Stitch
//! sews from that join (read at `d59c9ab`).

/// A place on a cross, in half cells right of and below its middle: a corner, the middle of a side, or the
/// middle itself.
pub(crate) type Offset = [i64; 2];

/// The top left corner.
const TL: Offset = [-1, -1];
/// The top right corner.
const TR: Offset = [1, -1];
/// The bottom right corner.
const BR: Offset = [1, 1];
/// The bottom left corner.
const BL: Offset = [-1, 1];
/// The middle of the top side.
const MT: Offset = [0, -1];
/// The middle of the right side.
const MR: Offset = [1, 0];
/// The middle of the bottom side.
const MB: Offset = [0, 1];
/// The middle of the left side.
const ML: Offset = [-1, 0];

/// A bar through a cross's middle, from one of its ends to the other.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Bar {
    /// The `\` diagonal, from the top left corner to the bottom right one.
    Falling,
    /// The `/` diagonal, from the bottom left corner to the top right one.
    Rising,
    /// The bar across, from the middle of the left side to the middle of the right side.
    Across,
    /// The bar down, from the middle of the top side to the middle of the bottom side.
    Down,
}

impl Bar {
    /// Its 2 ends.
    pub(crate) const fn ends(self) -> [Offset; 2] {
        match self {
            Bar::Falling => [TL, BR],
            Bar::Rising => [BL, TR],
            Bar::Across => [ML, MR],
            Bar::Down => [MT, MB],
        }
    }

    /// Its end that isn't `end`.
    fn other(self, end: Offset) -> Offset {
        let [a, b] = self.ends();
        if end == a { b } else { a }
    }
}

/// The kinds of full cross, each a stack of bars.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Kind {
    /// A `\` diagonal under a `/` one.
    Simple,
    /// A bar across under a bar down: a `+`.
    Upright,
    /// A `\` diagonal, an upright cross over it, and a `/` diagonal over that.
    Double,
    /// A simple cross, and an upright cross over it.
    Smyrna,
}

impl Kind {
    /// Its bars, from the bottom up.
    pub(crate) const fn stack(self) -> &'static [Bar] {
        match self {
            Kind::Simple => &[Bar::Falling, Bar::Rising],
            Kind::Upright => &[Bar::Across, Bar::Down],
            Kind::Double => &[Bar::Falling, Bar::Across, Bar::Down, Bar::Rising],
            Kind::Smyrna => &[Bar::Falling, Bar::Rising, Bar::Across, Bar::Down],
        }
    }

    /// Its joins, clockwise from its top left.
    pub(crate) const fn joins(self) -> &'static [Offset] {
        match self {
            Kind::Simple | Kind::Double => &[TL, TR, BR, BL],
            Kind::Upright => &[MT, MR, MB, ML],
            Kind::Smyrna => &[TL, MT, TR, BR, MB, BL],
        }
    }

    /// Its good joins, the ends of its top bar, in the order the crosses that have one at a place are sewn:
    /// the cross lower down first, and of 2 level with each other the one further right. A cross whose join
    /// at `p` is `o` lies at `p − o`, so the offsets go by their rows, then from the left.
    pub(crate) const fn good(self) -> &'static [Offset] {
        match self {
            Kind::Simple | Kind::Double => &[TR, BL],
            Kind::Upright | Kind::Smyrna => &[MT, MB],
        }
    }

    /// Its bad joins, in the order [`Kind::good`] gives its good ones.
    pub(crate) const fn bad(self) -> &'static [Offset] {
        match self {
            Kind::Simple | Kind::Double => &[TL, BR],
            Kind::Upright => &[ML, MR],
            Kind::Smyrna => &[TL, TR, BL, BR],
        }
    }

    /// Of its joins, the first good one clockwise from its top left: where a run starts without a needle.
    pub(crate) fn first_good(self) -> Offset {
        let good = self.good();
        self.joins().iter().copied().find(|join| good.contains(join)).unwrap_or(MT)
    }

    /// The end of `bar` the needle goes to first from the middle when the bar is sewn between others, in a
    /// loop from `start`.
    fn first_end(self, bar: Bar, start: Offset) -> Offset {
        match bar {
            Bar::Falling if start == BL => BR,
            Bar::Falling => TL,
            Bar::Rising if self == Kind::Smyrna && start != BR => BL,
            Bar::Rising => TR,
            Bar::Across => ML,
            Bar::Down if self == Kind::Smyrna && start == TL => MB,
            Bar::Down => MT,
        }
    }
}

/// The places of the loop that sews the cross of `kind` whose middle is `middle`, from its join `start`
/// back to it, after `start`: each bar `n` times back and forth, as the module's docs give it. Empty for a
/// `start` that is no end of the kind's bars, which no caller passes.
pub(crate) fn loop_from(kind: Kind, middle: [i64; 2], start: Offset, n: u32) -> Vec<[i64; 2]> {
    let stack = kind.stack();
    let Some(at) = stack.iter().position(|bar| bar.ends().contains(&start)) else { return Vec::new() };
    let top = stack.len() - 1;
    let far = stack.get(at).map_or(start, |bar| bar.other(start));
    let mut spots = Vec::with_capacity(stack.len() * (2 * n as usize + 1) + 2);
    let fewer = n.saturating_sub(1);
    if at == top {
        spots.push(middle);
        for &bar in stack.iter().take(top) {
            between(&mut spots, kind, bar, (middle, start), n);
        }
        back_and_forth(&mut spots, middle, [far, start], n);
    } else if at == 0 {
        back_and_forth(&mut spots, middle, [far, start], fewer);
        spots.extend([place(middle, far), middle]);
        for &bar in stack.iter().skip(1) {
            between(&mut spots, kind, bar, (middle, start), n);
        }
        spots.push(place(middle, start));
    } else {
        spots.push(middle);
        for (k, &bar) in stack.iter().enumerate() {
            if k == at {
                spots.push(place(middle, far));
                back_and_forth(&mut spots, middle, [start, far], fewer);
                spots.push(middle);
            } else {
                between(&mut spots, kind, bar, (middle, start), n);
            }
        }
        spots.push(place(middle, start));
    }
    spots
}

/// Where `offset` lies from the middle `middle`.
fn place(middle: [i64; 2], [du, dv]: Offset) -> [i64; 2] {
    [middle[0] + du, middle[1] + dv]
}

/// Adds the bar from `from` to `to` of the cross whose middle is `middle`, `times` times back and forth: to
/// `from`, then to `to`, and again.
fn back_and_forth(spots: &mut Vec<[i64; 2]>, middle: [i64; 2], [from, to]: [Offset; 2], times: u32) {
    for _ in 0..times {
        spots.extend([place(middle, from), place(middle, to)]);
    }
}

/// Adds `bar` of the cross of `kind` sewn between others in the loop from `start`: from the middle, `n`
/// times back and forth from its first end, and back to the middle.
fn between(spots: &mut Vec<[i64; 2]>, kind: Kind, bar: Bar, (middle, start): ([i64; 2], Offset), n: u32) {
    let first = kind.first_end(bar, start);
    back_and_forth(spots, middle, [first, bar.other(first)], n);
    spots.push(middle);
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::*;

    /// The loop of the cross of `kind` at the middle (1, 1), from `start`, as offsets from its middle, with
    /// `n` passes back and forth.
    fn looped(kind: Kind, start: Offset, n: u32) -> Vec<Offset> {
        loop_from(kind, [1, 1], start, n).into_iter().map(|[u, v]| [u - 1, v - 1]).collect()
    }

    const C: Offset = [0, 0];

    /// How many times each half of each bar of `run` is passed: by its end, and only for stitches between
    /// the middle and an end, or between 2 ends of a bar, which pass both its halves.
    fn passes(run: &[Offset]) -> BTreeMap<Offset, usize> {
        let mut passes = BTreeMap::new();
        for pair in run.windows(2) {
            let (a, b) = (pair[0], pair[1]);
            let ends: Vec<Offset> = if a == C {
                vec![b]
            } else if b == C {
                vec![a]
            } else {
                vec![a, b]
            };
            for end in ends {
                *passes.entry(end).or_insert(0) += 1;
            }
        }
        passes
    }

    #[test]
    fn req_fill_crs_003_a_simple_cross_s_loops_are_m10_1_s() {
        assert_eq!(looped(Kind::Simple, TR, 1), [C, TL, BR, C, BL, TR]);
        assert_eq!(looped(Kind::Simple, TR, 2), [C, TL, BR, TL, BR, C, BL, TR, BL, TR]);
        assert_eq!(looped(Kind::Simple, BL, 1), [C, BR, TL, C, TR, BL]);
        assert_eq!(looped(Kind::Simple, TL, 1), [BR, C, TR, BL, C, TL]);
        assert_eq!(looped(Kind::Simple, TL, 2), [BR, TL, BR, C, TR, BL, TR, BL, C, TL]);
        assert_eq!(looped(Kind::Simple, BR, 1), [TL, C, TR, BL, C, BR]);
    }

    #[test]
    fn req_fill_crs_008_an_upright_cross_sews_its_bar_across_then_its_bar_down() {
        // From the middle of its top or bottom side, in to the middle along the bar down, which it sews last.
        assert_eq!(looped(Kind::Upright, MT, 1), [C, ML, MR, C, MB, MT]);
        assert_eq!(looped(Kind::Upright, MB, 2), [C, ML, MR, ML, MR, C, MT, MB, MT, MB]);
        // From the middle of its left or right side, the bar across first, ending on top.
        assert_eq!(looped(Kind::Upright, ML, 1), [MR, C, MT, MB, C, ML]);
        assert_eq!(looped(Kind::Upright, MR, 2), [ML, MR, ML, C, MT, MB, MT, MB, C, MR]);
    }

    #[test]
    fn req_fill_crs_009_a_double_cross_sews_an_upright_cross_between_its_diagonals() {
        assert_eq!(looped(Kind::Double, TR, 1), [C, TL, BR, C, ML, MR, C, MT, MB, C, BL, TR]);
        assert_eq!(looped(Kind::Double, BL, 1), [C, BR, TL, C, ML, MR, C, MT, MB, C, TR, BL]);
        assert_eq!(looped(Kind::Double, TL, 2), [BR, TL, BR, C, ML, MR, ML, MR, C, MT, MB, MT, MB, C, TR, BL, TR, BL, C, TL]);
        assert_eq!(looped(Kind::Double, BR, 1), [TL, C, ML, MR, C, MT, MB, C, TR, BL, C, BR]);
    }

    #[test]
    fn req_fill_crs_009_a_smyrna_cross_sews_an_upright_cross_over_its_diagonals() {
        assert_eq!(looped(Kind::Smyrna, MT, 1), [C, TL, BR, C, BL, TR, C, ML, MR, C, MB, MT]);
        assert_eq!(looped(Kind::Smyrna, MB, 1), [C, TL, BR, C, BL, TR, C, ML, MR, C, MT, MB]);
        assert_eq!(looped(Kind::Smyrna, TL, 1), [BR, C, BL, TR, C, ML, MR, C, MB, MT, C, TL]);
        assert_eq!(looped(Kind::Smyrna, BR, 1), [TL, C, TR, BL, C, ML, MR, C, MT, MB, C, BR]);
        // From an end of its `/` diagonal, a bar between: that bar from the middle to its far end, 1 time back
        // and forth fewer, and the stitch from the middle back to the start last.
        assert_eq!(looped(Kind::Smyrna, TR, 1), [C, TL, BR, C, BL, C, ML, MR, C, MT, MB, C, TR]);
        assert_eq!(looped(Kind::Smyrna, BL, 2), [C, BR, TL, BR, TL, C, TR, BL, TR, C, ML, MR, ML, MR, C, MT, MB, MT, MB, C, BL]);
    }

    #[test]
    fn req_fill_crs_008_each_half_of_each_bar_is_passed_twice_for_each_time_back_and_forth_from_any_join() {
        for kind in [Kind::Simple, Kind::Upright, Kind::Double, Kind::Smyrna] {
            for &start in kind.joins() {
                for n in [1, 2, 3] {
                    let mut run = vec![start];
                    run.extend(looped(kind, start, n));
                    assert_eq!(run.last(), Some(&start), "{kind:?} {start:?} {n}");
                    let counts = passes(&run);
                    let ends: Vec<Offset> = kind.stack().iter().flat_map(|bar| bar.ends()).collect();
                    assert_eq!(counts.len(), ends.len(), "{kind:?} {start:?} {n}: {counts:?}");
                    assert!(ends.iter().all(|end| counts.get(end) == Some(&(2 * n as usize))), "{kind:?} {start:?} {n}: {counts:?}");
                    // A loop from a good join ends along its top bar, from its far end, and nothing of it lies on
                    // top; one from a bad join ends with the stitch from the middle back to the start, on top.
                    let top = *kind.stack().last().unwrap();
                    let before = run[run.len() - 2];
                    if kind.good().contains(&start) {
                        assert_eq!(before, top.other(start), "{kind:?} {start:?}");
                    } else {
                        assert_eq!(before, C, "{kind:?} {start:?}");
                    }
                }
            }
        }
    }

    #[test]
    fn the_joins_are_the_ends_of_the_bars_and_the_good_ones_the_top_bar_s() {
        for kind in [Kind::Simple, Kind::Upright, Kind::Double, Kind::Smyrna] {
            let top = kind.stack().last().unwrap().ends();
            assert!(kind.good().iter().all(|join| top.contains(join)), "{kind:?}");
            assert!(kind.bad().iter().all(|join| !top.contains(join) && kind.joins().contains(join)), "{kind:?}");
            assert_eq!(kind.good().len() + kind.bad().len(), kind.joins().len(), "{kind:?}");
        }
        assert_eq!([Kind::Simple, Kind::Upright, Kind::Double, Kind::Smyrna].map(Kind::first_good), [TR, MT, TR, MT]);
        // A start that no bar of the kind ends at has no loop.
        assert_eq!(looped(Kind::Upright, TL, 1), Vec::<Offset>::new());
        assert_eq!(Bar::Down.other(MT), MB);
        assert_eq!(Bar::Down.other(MB), MT);
    }
}
