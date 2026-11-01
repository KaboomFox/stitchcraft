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
//! **A rung at an end.** Ink/Stitch's rule tells the rails from the rungs by where they meet only with 1
//! rung, or 3 or more. With exactly 2 it takes the 2 longest subpaths for the rails, which on a column
//! shorter than it is wide are its rungs (`DEV-SAT-007`), and with none a path of 2 subpaths is 2 rails
//! whose nodes pair up. So a column made with 2 rungs or none is drawn with one more, which pairs the
//! rails' first points: the section it ends has no length, and the column sews as before. Push
//! compensation lengthens the rails before they are cut, though, and a rung across an end it lengthens cuts
//! what it adds there off as a section of its own, whose points pair otherwise. So the rung pairs the rails'
//! last points where push compensation lengthens the column's start, and an end it lengthens takes the
//! rung only when no other can ([`Column::cuts_lengthened_end`]).
//!
//! **What it refuses** ([`Unmade`]): a line whose rails can't be made, as `SC-E0214` says of it sewn, or
//! whose rails leave parts of it out, as `SC-W0213` says, since the column drawn would sew less than the
//! line; a column that would still be read back otherwise, as one whose rails cross each other is; and one
//! that would need a rung across an end that push compensation lengthens, which would sew otherwise.
//!
//! **Narrow lines.** A line no wider than the design's narrowest satin stroke is sewn as a stroke by the
//! satin effect (`REQ-SAT-015`). Made into rails and rungs it is a satin column as narrow as it is drawn,
//! which is what the tool is asked for: a host's filter isn't given the design's settings.

use std::fmt;

use stitchcraft_core::{Exhausted, Meter, Mm, Point};

use crate::design::{Join, Path, Segment, Subpath};
use crate::generators::satin::compensation::lengthened;
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
    /// Whether a rung added to tell the rails from the rungs lies across an end that push compensation
    /// lengthens, where it cuts what that adds off as a section of its own: no other end could take it.
    pub cuts_lengthened_end: bool,
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
    /// The column made of it would need a rung across an end that its push compensation lengthens, which
    /// would sew what that adds as a section of its own.
    Lengthened,
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
            Unmade::Lengthened => f.write_str(
                "would make a satin column that needs a rung across an end its push compensation lengthens, which would sew what that \
                 adds as a part of its own",
            ),
        }
    }
}

/// The column drawn with rails and rungs that sews as `line` does, a satin column drawn as one path, `width`
/// wide with `join` at its corners, whose push compensation, which the column keeps, is `push` millimetres at
/// its start and its end; or why there is none. Making and checking it charges `meter`.
pub fn from_line(line: &Subpath, width: Mm, join: Join, push: [f64; 2], meter: &mut Meter) -> Result<Result<Column, Unmade>, Exhausted> {
    let Ok(Shape::CentreLine { line, closed }) = recognize(&Path { subpaths: vec![line.clone()] }, meter)?.shape else {
        return Ok(Err(Unmade::NoRails));
    };
    let Some(Made { rails, rungs, left_out }) = rails_and_rungs(&line, closed, width.get(), join, meter)? else {
        return Ok(Err(Unmade::NoRails));
    };
    if left_out > 0 {
        return Ok(Err(Unmade::PartsLeftOut(left_out)));
    }
    Ok(from_rails(rails, rungs, push, meter)?.and_then(|column| if column.cuts_lengthened_end { Err(Unmade::Lengthened) } else { Ok(column) }))
}

/// The column drawn with `rails`, both from the column's start, and a rung across each pair of points
/// `rungs` gives, on the first rail and then the second, in order, whose push compensation is `push`
/// millimetres at its start and its end; or [`Unmade::ReadOtherwise`] when it would be read back with other
/// rails or rungs. Checking it charges `meter`.
///
/// Ink/Stitch's rule tells the rails from the rungs by where they meet: with 1 rung, the rails are the 2
/// that meet one other; with 2 or more, those that meet more than 2. Rails that meet each other, as a pointed
/// column's do at its tip, meet one more. So a rung across an end where the rails are apart is added where
/// the count would leave the rails unknown: to none or 2 rungs between rails apart, and to none or 1 between
/// rails that meet, and one across the other end too when that is still too few. Ends that `push` doesn't
/// lengthen take one first, the start before the end, and then ends it lengthens
/// ([`Column::cuts_lengthened_end`]).
pub fn from_rails(rails: [Vec<Point>; 2], rungs: Vec<[Point; 2]>, push: [f64; 2], meter: &mut Meter) -> Result<Result<Column, Unmade>, Exhausted> {
    let reaching = |[a, b]: [Point; 2]| {
        let past = |from: Point, to: Point| Point::new(to.x() + REACH * (to.x() - from.x()), to.y() + REACH * (to.y() - from.y())).ok();
        Some([past(b, a)?, past(a, b)?])
    };
    let apart = |a: Option<&Point>, b: Option<&Point>| a.zip(b).filter(|(a, b)| a.distance(**b) >= SAME_POINT).map(|(a, b)| [*a, *b]);
    let [first, second] = &rails;
    let ends = [apart(first.first(), second.first()), apart(first.last(), second.last())];
    let meet = ends.iter().any(Option::is_none);
    let known = |count: usize| if meet { count >= 2 } else { count == 1 || count >= 3 };
    let (mut pairs, mut cuts_lengthened_end) = (rungs, false);
    // Ends push compensation leaves as they are, then ends it lengthens, which only the last rung added can
    // be across.
    for lengthening in [false, true] {
        for (at, (end, lengthened)) in ends.into_iter().zip(lengthened(push)).enumerate() {
            let Some(end) = end.filter(|_| lengthened == lengthening && !known(pairs.len())) else { continue };
            if at == 0 {
                pairs.insert(0, end);
            } else {
                pairs.push(end);
            }
            cuts_lengthened_end = lengthening;
        }
    }
    let Some(drawn) = pairs.iter().copied().map(reaching).collect::<Option<Vec<_>>>() else { return Ok(Err(Unmade::ReadOtherwise)) };
    let column = Column { rails, rungs: drawn, cuts_lengthened_end };
    // Read back, the column must give its rails and pair the points the line's rungs pair.
    let found = recognize(&column.path(), meter)?;
    let same = match found.shape {
        Ok(Shape::Rails(Satin { rails: found_rails, pairing: Pairing::Rungs(found_pairs), .. })) => {
            found.warnings.is_empty()
                && found_rails.iter().zip(&column.rails).all(|(found, made)| alike(found, made))
                && alike(&found_pairs.iter().flat_map(|rung| rung.points).collect::<Vec<_>>(), &pairs.concat())
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
        let drawn =
            |rails: [Vec<Point>; 2], rungs: Vec<[Point; 2]>| from_rails(rails, rungs, [0.0; 2], &mut Budget::DEFAULT.meter()).unwrap().unwrap();
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
        // Rails that lie on each other, a column of no width, meet all along and get no rung: drawn, they
        // aren't read back as rails at all.
        let flat = [vec![p(0.0, 0.0), p(10.0, 0.0)], vec![p(0.0, 0.0), p(10.0, 0.0)]];
        assert_eq!(from_rails(flat, Vec::new(), [0.0; 2], &mut Budget::DEFAULT.meter()).unwrap(), Err(Unmade::ReadOtherwise));
    }

    /// Push compensation lengthens the rails before they are cut, so a rung across an end it lengthens would
    /// cut what it adds off as a section of its own: an end it leaves as it is takes the rung added, the start
    /// first, and one it lengthens only when no other can, which the column says.
    #[test]
    fn a_rung_is_added_across_an_end_that_push_compensation_does_not_lengthen() {
        let meter = &mut Budget::DEFAULT.meter();
        let apart = [vec![p(0.0, -1.5), p(10.0, -1.5)], vec![p(0.0, 1.5), p(10.0, 1.5)]];
        let rungs = vec![[p(3.0, -1.5), p(3.0, 1.5)], [p(6.0, -1.5), p(6.0, 1.5)]];
        let mut drawn = |push: [f64; 2]| from_rails(apart.clone(), rungs.clone(), push, meter).unwrap().unwrap();
        let across = |column: &Column| (column.rungs.iter().map(|[a, _]| a.x()).collect::<Vec<_>>(), column.cuts_lengthened_end);
        for push in [[0.0, 0.0], [0.5, 0.5], [0.0, -0.4]] {
            assert_eq!(across(&drawn(push)), (vec![0.0, 3.0, 6.0], false), "{push:?}: at the start");
        }
        assert_eq!(across(&drawn([-0.4, 0.0])), (vec![3.0, 6.0, 10.0], false), "at the end");
        assert_eq!(across(&drawn([-0.4, -0.4])), (vec![0.0, 3.0, 6.0], true), "at the start, lengthened");
        // Rails that meet at their start, as a pointed column's do, take it at their end, lengthened or not.
        let pointed = [vec![p(0.0, 0.0), p(10.0, -1.5)], vec![p(0.0, 0.0), p(10.0, 1.5)]];
        let column = from_rails(pointed, vec![[p(5.0, -0.75), p(5.0, 0.75)]], [0.0, -0.4], meter).unwrap().unwrap();
        assert_eq!(across(&column), (vec![5.0, 10.0], true));
    }
}
