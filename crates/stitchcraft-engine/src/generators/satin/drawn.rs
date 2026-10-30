//! A satin column drawn as one path, made into one drawn with rails and rungs that sews as it does: what a
//! host's Stroke to Satin tool makes of a line (`REQ-VC-014`), as Ink/Stitch's Stroke to Satin does.
//!
//! **What it makes.** The rails and rungs the column makes when it is sewn (`normalize::centre_line`), as
//! subpaths of straight lines: the 2 rails, then a line across them for each rung, reaching past each point
//! it pairs by [`REACH`] of the distance between them, so that it crosses each rail rather than ending on
//! it. Read back as any satin column drawn with rails and rungs is ([`recognize`]), they give the same
//! rails, and pair the same points to within a millionth of a millimetre, so the column sews as the line
//! did.
//!
//! **A rung at the start.** Ink/Stitch's rule tells the rails from the rungs by where they meet only with 1
//! rung, or 3 or more. With exactly 2 it takes the 2 longest subpaths for the rails, which on a column
//! shorter than it is wide are its rungs (`DEV-SAT-007`), and with none a path of 2 subpaths is 2 rails
//! whose nodes pair up. So a column made with 2 rungs or none is drawn with one more, which pairs the
//! rails' first points: the section it ends has no length, and the column sews as before.
//!
//! **What it refuses** ([`Unmade`]): a line whose rails can't be made, as `SC-E0214` says of it sewn, or
//! whose rails leave parts of it out, as `SC-W0213` says, since the column drawn would sew less than the
//! line; and a column that would still be read back otherwise, as one whose rails cross each other is.
//!
//! **Narrow lines.** A line no wider than the design's narrowest satin stroke is sewn as a stroke by the
//! satin effect (`REQ-SAT-015`). Made into rails and rungs it is a satin column as narrow as it is drawn,
//! which is what the tool is asked for: a host's filter isn't given the design's settings.

use std::fmt;

use stitchcraft_core::{Exhausted, Meter, Mm, Point};

use crate::design::{Join, Path, Segment, Subpath};
use crate::normalize::centre_line::{Made, rails_and_rungs};
use crate::normalize::satin::{Pairing, SAME_POINT, Satin, Shape, recognize};

/// How far past each point it pairs a rung drawn reaches, as a fraction of the distance between them.
pub const REACH: f64 = 0.01;

/// A satin column drawn with its rails and rungs, in millimetres.
#[derive(Clone, Debug, PartialEq)]
pub struct Column {
    /// The 2 rails, in the order the path draws them, both from the line's start.
    pub rails: [Vec<Point>; 2],
    /// The rungs as drawn: each a straight line across both rails, from past the first to past the second.
    pub rungs: Vec<[Point; 2]>,
}

impl Column {
    /// The column as a path: its rails, then its rungs, each an open subpath of straight lines.
    pub fn path(&self) -> Path {
        let lines = self.rails.iter().map(Vec::as_slice).chain(self.rungs.iter().map(<[Point; 2]>::as_slice));
        Path { subpaths: lines.filter_map(open).collect() }
    }
}

/// The open subpath through `points`; `None` without any.
fn open(points: &[Point]) -> Option<Subpath> {
    let (&start, rest) = points.split_first()?;
    Some(Subpath { start, segments: rest.iter().map(|&point| Segment::Line(point)).collect(), closed: false })
}

/// Why a line makes no satin column drawn with rails and rungs that sews as it does.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Unmade {
    /// Its rails can't be made: the line is too short, or turns too tightly for the column's width.
    NoRails,
    /// So many parts of it cross themselves or turn more tightly than the column is wide, which the column
    /// would leave out.
    PartsLeftOut(usize),
    /// The column made of it would be read back with other rails or rungs.
    ReadOtherwise,
}

impl fmt::Display for Unmade {
    /// What is wrong, as the end of a sentence that names the line.
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Unmade::NoRails => f.write_str("is too short or turns too tightly for the stroke's width to make the rails of a satin column"),
            Unmade::PartsLeftOut(1) => f.write_str(
                "has a part that crosses itself or turns more tightly than the column is wide, which a satin column made of it would \
                 leave out",
            ),
            Unmade::PartsLeftOut(parts) => write!(
                f,
                "has {parts} parts that cross themselves or turn more tightly than the column is wide, which a satin column made of it \
                 would leave out"
            ),
            Unmade::ReadOtherwise => {
                f.write_str("would make a satin column that is read back with other rails or rungs, as one whose rails cross each other is")
            }
        }
    }
}

/// The column drawn with rails and rungs that sews as `line` does, a satin column drawn as one path, `width`
/// wide with `join` at its corners; or why there is none. Making and checking it charges `meter`.
pub fn from_line(line: &Subpath, width: Mm, join: Join, meter: &mut Meter) -> Result<Result<Column, Unmade>, Exhausted> {
    let Ok(Shape::CentreLine { line, closed }) = recognize(&Path { subpaths: vec![line.clone()] }, meter)?.shape else {
        return Ok(Err(Unmade::NoRails));
    };
    let Some(Made { rails, rungs, left_out }) = rails_and_rungs(&line, closed, width.get(), join, meter)? else {
        return Ok(Err(Unmade::NoRails));
    };
    if left_out > 0 {
        return Ok(Err(Unmade::PartsLeftOut(left_out)));
    }
    from_rails(rails, rungs, meter)
}

/// The column drawn with `rails`, both from the column's start, and a rung across each pair of points
/// `rungs` gives, on the first rail and then the second, in order; or [`Unmade::ReadOtherwise`] when it
/// would be read back with other rails or rungs. Checking it charges `meter`.
///
/// Ink/Stitch's rule tells the rails from the rungs by where they meet: with 1 rung, the rails are the 2
/// that meet one other; with 2 or more, those that meet more than 2. Rails that meet each other, as a pointed
/// column's do at its tip, meet one more. So a rung that pairs the rails' first points is added where the
/// count would leave the rails unknown: to none or 2 rungs between rails apart, and to none or 1 between
/// rails that meet, and one that pairs their last points when that is still too few.
pub fn from_rails(rails: [Vec<Point>; 2], rungs: Vec<[Point; 2]>, meter: &mut Meter) -> Result<Result<Column, Unmade>, Exhausted> {
    let reaching = |[a, b]: [Point; 2]| {
        let past = |from: Point, to: Point| Point::new(to.x() + REACH * (to.x() - from.x()), to.y() + REACH * (to.y() - from.y())).ok();
        Some([past(b, a)?, past(a, b)?])
    };
    let apart = |a: Option<&Point>, b: Option<&Point>| a.zip(b).filter(|(a, b)| a.distance(**b) >= SAME_POINT).map(|(a, b)| [*a, *b]);
    let [first, second] = &rails;
    let (start, end) = (apart(first.first(), second.first()), apart(first.last(), second.last()));
    let meet = start.is_none() || end.is_none();
    let known = |count: usize| if meet { count >= 2 } else { count == 1 || count >= 3 };
    let mut pairs = rungs;
    if !known(pairs.len())
        && let Some(start) = start
    {
        pairs.insert(0, start);
    }
    if !known(pairs.len())
        && let Some(end) = end
    {
        pairs.push(end);
    }
    let Some(drawn) = pairs.iter().copied().map(reaching).collect::<Option<Vec<_>>>() else { return Ok(Err(Unmade::ReadOtherwise)) };
    let column = Column { rails, rungs: drawn };
    // Read back, the column must give its rails and pair the points the line's rungs pair.
    let found = recognize(&column.path(), meter)?;
    let same = match found.shape {
        Ok(Shape::Rails(Satin { rails: found_rails, pairing: Pairing::Rungs(found_pairs), .. })) => {
            found.warnings.is_empty()
                && found_rails.iter().zip(&column.rails).all(|(found, made)| alike(found, made))
                && alike(&found_pairs.concat(), &pairs.concat())
        }
        _ => false,
    };
    Ok(if same { Ok(column) } else { Err(Unmade::ReadOtherwise) })
}

/// Whether `a` and `b` are as many points, each within a millionth of a millimetre of the other's.
fn alike(a: &[Point], b: &[Point]) -> bool {
    a.len() == b.len() && a.iter().zip(b).all(|(a, b)| a.distance(*b) < SAME_POINT)
}

#[cfg(test)]
mod tests {
    use stitchcraft_core::Budget;

    use super::*;

    fn p(x: f64, y: f64) -> Point {
        Point::new(x, y).unwrap()
    }

    /// A column read back gives the rails it was made with, and pairs the same points, only when every point
    /// is where it was made, to within a millionth of a millimetre (the integration tests make columns that
    /// are read back so, and ones that are read back otherwise).
    #[test]
    fn points_are_alike_when_as_many_and_each_within_a_millionth_of_a_millimetre() {
        let made = [p(0.0, 0.0), p(5.0, 0.0)];
        assert!(alike(&made, &[p(0.0, 0.0), p(5.0, SAME_POINT / 2.0)]));
        // Exactly a millionth apart is another point.
        assert!(!alike(&made, &[p(0.0, 0.0), p(5.0, SAME_POINT)]));
        assert!(!alike(&made, &[p(0.0, 0.0), p(5.0, 1.0)]));
        // A point more, or one fewer, is another line, whatever the points they share.
        assert!(!alike(&made, &[p(0.0, 0.0), p(5.0, 0.0), p(9.0, 0.0)]));
        assert!(!alike(&made, &[p(0.0, 0.0)]));
    }

    /// Ink/Stitch's rule tells rails that meet from rungs only by 2 rungs or more, and rails apart only by 1, or
    /// 3 or more: a column drawn with rails and rungs gets a rung pairing its rails' first points, or their
    /// last, where the count would leave its rails unknown, and read back it gives its rails and rungs.
    #[test]
    fn a_rung_is_added_at_an_end_where_the_count_would_leave_the_rails_unknown() {
        let drawn = |rails: [Vec<Point>; 2], rungs: Vec<[Point; 2]>| from_rails(rails, rungs, &mut Budget::DEFAULT.meter()).unwrap().unwrap();
        // Pointed at its start, its rails meeting there, with 1 rung: one more pairs their last points, drawn
        // past them.
        let pointed = [vec![p(0.0, 0.0), p(10.0, -1.5)], vec![p(0.0, 0.0), p(10.0, 1.5)]];
        let column = drawn(pointed.clone(), vec![[p(5.0, -0.75), p(5.0, 0.75)]]);
        assert_eq!(column.rails, pointed);
        let [_, [a, b]] = column.rungs.as_slice() else { panic!("{:?}", column.rungs) };
        assert!(a.x() == 10.0 && b.x() == 10.0 && a.y() < -1.5 && b.y() > 1.5, "{:?}", column.rungs);
        // Apart at both ends, with 2 rungs: one more pairs their first points.
        let apart = [vec![p(0.0, -1.5), p(10.0, -1.5)], vec![p(0.0, 1.5), p(10.0, 1.5)]];
        let column = drawn(apart, vec![[p(3.0, -1.5), p(3.0, 1.5)], [p(6.0, -1.5), p(6.0, 1.5)]]);
        let [first, ..] = column.rungs.as_slice() else { panic!("{:?}", column.rungs) };
        assert!(column.rungs.len() == 3 && first.iter().all(|q| q.x() == 0.0), "{:?}", column.rungs);
    }
}
