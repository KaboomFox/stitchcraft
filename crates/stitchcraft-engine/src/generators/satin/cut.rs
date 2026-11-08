//! A satin column cut into pieces, each of which sews the pairs the column sews along it
//! (`docs/src/design/algorithms/satin.md` › Cut in 2), as routing cuts a column (`crate::route`).
//!
//! **Why here.** A piece is a satin column of its own, drawn with its share of the column's rails and rungs
//! and sewn with the column's settings, so the cut is a satin column's business: it takes the column's
//! rails as they are sewn, its rungs and its cuts, the pairs at the zigzag spacing with no compensation and
//! nothing random, as the column's start and end find their places.
//!
//! **The centre line.** Ink/Stitch's tools measure a column along the line through the middles of its cuts
//! (read at `d59c9ab`), and so does routing: [`Cuttable::line`].
//!
//! **Where a piece is cut.** A place along the column, given as a fraction of its centre line, takes a cut:
//! the cuts' points listed in order, first rail's then second rail's of each, the point at that fraction of
//! the list, rounded to the nearest, halves to the even one, and that point's cut ([`Cuttable::cut_at`]).
//! Each rail is cut where it comes nearest its point of the cut. A piece is the rails' parts between its 2
//! cuts, or the rails' ends at the column's ends.
//!
//! **Rungs.** A piece keeps the column's rungs whose points both lie on its parts of the rails, a rung added
//! across a column's nodes among them, but for a rung across its start or its end, whose points that end
//! pairs anyway. Kept, it would lie on the rung added there. The piece is drawn as a column drawn with its
//! rails and rungs is ([`drawing`]): a rung added at an end where Ink/Stitch's rule would not tell its rails
//! from its rungs.
//! Ink/Stitch adds a rung 0.3 CSS pixels from the piece's start instead, and 2 more to a piece with none
//! (`DEV-ROUTE-001`). A rung that misses a rail is left out of every piece, as Ink/Stitch leaves it out:
//! its line meets no part of that rail.
//!
//! **Drawn as the column is.** The piece's rails are drawn in the column's order, and a rail that the
//! column's `reverse_rails` turns by name is drawn turned, so that the piece's settings, the column's, swap
//! and turn them as the column's swap and turn its own. With `automatic`, the rails are drawn running the
//! same way, which the setting leaves. Ink/Stitch draws a piece with the rails in the order they are sewn,
//! which its swap then swaps back (`DEV-ROUTE-002`). A piece whose rails both have no length sews nothing,
//! and is none. One whose rail is shorter than the zigzag spacing is kept, where Ink/Stitch makes none
//! (`DEV-ROUTE-004`).

use stitchcraft_core::{Exhausted, Meter, Point};

use crate::design::Path;
use crate::generators::pull::offset;
use crate::generators::satin::SatinParams;
use crate::generators::satin::column::{added_rungs, nodes_started_together, oriented, sections};
use crate::generators::satin::compensation::Processor;
use crate::generators::satin::drawn::{drawing, reads_back};
use crate::generators::satin::pairs::{Pair, pairs};
use crate::normalize::along::Along;
use crate::normalize::near::{Look, along_to_segment};
use crate::normalize::satin::{Pairing, Satin};

/// A satin column ready to be cut into pieces.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Cuttable {
    /// The rails as sewn: swapped, rings started together and turned as the settings say, without push
    /// compensation, as Ink/Stitch cuts them.
    pub(crate) rails: [Vec<Point>; 2],
    /// The rails' places, first and second as sewn, that a `reverse_rails` naming them turns: a piece is
    /// drawn with them turned back, and turned again by its own setting.
    named: [bool; 2],
    /// Whether the rails are swapped.
    swapped: bool,
    /// How far along each rail each of its points lies.
    at: [Vec<f64>; 2],
    /// The points each rung pairs, on the first rail and on the second, and how far along each rail they lie.
    rungs: Vec<([Point; 2], [f64; 2])>,
    /// Its cuts: the pairs at the zigzag spacing, with no compensation and nothing random.
    pub(crate) cuts: Vec<Pair>,
}

/// A piece of a column drawn with its rails and rungs.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Piece {
    /// The path: the rails, then a line across them for each rung.
    pub(crate) path: Path,
    /// Whether it reads back with the rails it was drawn with, pairing the points the column's rungs pair.
    pub(crate) reads_back: bool,
}

impl Cuttable {
    /// `satin`, sewn as `params` say, ready to be cut. Turning its rails, making its rungs and placing its
    /// cuts charge `meter` as sewing it does.
    pub(crate) fn of(satin: &Satin, params: &SatinParams, meter: &mut Meter) -> Result<Cuttable, Exhausted> {
        let (rails, turned) = oriented(satin, params, meter)?;
        let named = match params.reverse_rails {
            "automatic" | "none" => [false; 2],
            _ => turned,
        };
        let swap = params.swap_satin_rails;
        let pairs_on = match &satin.pairing {
            Pairing::Rungs(rungs) => {
                let crossing = rungs.iter().filter(|rung| rung.missed.is_none());
                crossing.map(|rung| if swap { [rung.points[1], rung.points[0]] } else { rung.points }).collect()
            }
            Pairing::Nodes => {
                let nodes = nodes_started_together(satin, swap, meter)?;
                let mut pairs = Vec::new();
                for rung in added_rungs(&nodes, turned, &mut Vec::new()) {
                    let [first, second] = &rails;
                    pairs.push([nearest(first, rung, meter)?, nearest(second, rung, meter)?]);
                }
                pairs
            }
        };
        let at = [Along::new(&rails[0], meter)?.at, Along::new(&rails[1], meter)?.at];
        let sections = sections(satin, params, &mut Vec::new(), meter)?;
        let cuts = pairs(&sections, params.zigzag_spacing_mm.get(), &mut Processor::inset([0.0; 2], [0.0; 2]), meter)?;
        let mut cuttable = Cuttable { rails, named, swapped: swap, at, rungs: Vec::with_capacity(pairs_on.len()), cuts };
        for [a, b] in pairs_on {
            let along = [cuttable.project(0, a, meter)?, cuttable.project(1, b, meter)?];
            cuttable.rungs.push(([a, b], along));
        }
        Ok(cuttable)
    }

    /// The rail `rail`, 0 or 1, measured.
    fn along(&self, rail: usize) -> Along<'_> {
        let points = self.rails.get(rail).map_or(&[][..], Vec::as_slice);
        Along { points, at: self.at.get(rail).cloned().unwrap_or_default() }
    }

    /// How far along the rail `rail` it comes nearest `point`, the first of equally near places.
    fn project(&self, rail: usize, point: Point, meter: &mut Meter) -> Result<f64, Exhausted> {
        self.along(rail).project(point, meter)
    }

    /// The length of each rail.
    pub(crate) fn lengths(&self) -> [f64; 2] {
        [self.along(0).length(), self.along(1).length()]
    }

    /// The column's centre line: the middle of each cut, in order, where its 2 points are moved in to meet,
    /// as pull compensation moves them.
    pub(crate) fn line(&self) -> Vec<Point> {
        self.cuts.iter().map(|&cut| offset(cut, [0.0; 2], [-0.5; 2])[0]).collect()
    }

    /// How far along each rail the cut at `fraction` of the centre line cuts it ([`Self::cut_index`]): where
    /// the rail comes nearest the cut's point on it. `None` for a column without cuts.
    pub(crate) fn cut_at(&self, fraction: f64, meter: &mut Meter) -> Result<Option<[f64; 2]>, Exhausted> {
        let Some([a, b]) = self.cuts.get(self.cut_index(fraction)).copied() else { return Ok(None) };
        Ok(Some([self.project(0, a, meter)?, self.project(1, b, meter)?]))
    }

    /// The cut at `fraction` of the centre line, as Ink/Stitch's tools take it: of the cuts' points, 2 to a
    /// cut, the one at that fraction of their count, rounded to the nearest, halves to the even, and its cut,
    /// the last at most.
    pub(crate) fn cut_index(&self, fraction: f64) -> usize {
        let points = 2.0 * self.cuts.len() as f64;
        // The fraction is from 0 to 1, so the index is a whole number from 0 to the points' count.
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let index = (points * fraction.clamp(0.0, 1.0)).round_ties_even() as usize;
        (index / 2).min(self.cuts.len().saturating_sub(1))
    }

    /// The piece of the column from `from` to `to` along the rails, millimetres along each, its first rail's
    /// then its second's, turned to run back when `back`, with the column's push compensation `push`, drawn
    /// with its rails and rungs; `None` when both rails' parts have no length, which sews nothing. Drawing it
    /// and reading it back charge `meter`.
    pub(crate) fn piece(&self, from: [f64; 2], to: [f64; 2], back: bool, push: [f64; 2], meter: &mut Meter) -> Result<Option<Piece>, Exhausted> {
        if (0..2).all(|i| to[i] <= from[i]) {
            return Ok(None);
        }
        let mut parts = [0, 1].map(|i| self.along(i).part(from[i], to[i].max(from[i])));
        // A rung across the piece's start or its end, on both rails, is that end's, which pairs the same points.
        let inside = |at: [f64; 2]| (0..2).all(|i| from[i] <= at[i] && at[i] <= to[i]) && at != from && at != to;
        let rungs: Vec<[Point; 2]> = self.rungs.iter().filter(|(_, at)| inside(*at)).map(|(points, _)| *points).collect();
        if back {
            parts.iter_mut().for_each(|part| part.reverse());
        }
        let Some((mut column, pairs)) = drawing(parts, rungs, push) else { return Ok(None) };
        let reads_back = reads_back(&column, &pairs, meter)? && !column.cuts_lengthened_end;
        for (rail, turn) in column.rails.iter_mut().zip(self.named) {
            if turn {
                rail.reverse();
            }
        }
        if self.swapped {
            column.rails.swap(0, 1);
        }
        Ok(Some(Piece { path: column.path(), reads_back }))
    }
}

/// The point of `rail` nearest the rung `rung`, measured from the rung, as a column measures a rung added
/// across its nodes. 2 units of `meter`'s work per side of the rail.
fn nearest(rail: &[Point], rung: (Point, Point), meter: &mut Meter) -> Result<Point, Exhausted> {
    let at = along_to_segment(rail, rung, Look::FromSegment, meter)?;
    Ok(Along::new(rail, meter)?.point(at))
}

#[cfg(test)]
mod tests;
