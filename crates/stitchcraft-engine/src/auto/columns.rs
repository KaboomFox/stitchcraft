//! The parts of an automatic area no wider than a satin column, each sewn as satin columns between its own
//! outlines (`REQ-AUTO-016` to `REQ-AUTO-018`).
//!
//! Design: `docs/src/design/image-to-embroidery.md` › Stitch types by width, and
//! `docs/src/design/algorithms/centre-lines.md` › Columns. Filled, a part as wide as a satin column sews rows
//! no longer than it is wide wherever they cross it; a satin column sews it with stitches across, the way a
//! digitizer would. So a part no wider than its area's widest satin column everywhere, and not narrow
//! (`auto::narrow`), sews a column along each of its centre lines (`normalize::strips::columns`):
//!
//! - **Width.** Each column is as wide as the circles that fit inside the part centred on its line, on
//!   average, as a narrow part is. One no wider than the design's narrowest satin stroke is too narrow to
//!   stitch across, and runs along its line as a running stitch instead, tripled from the narrowest triple
//!   run.
//! - **Order.** A column that ends under another sews first, then the longest, and each next the one with
//!   an end nearest where the last one ended, sewn from that end; a ring from its rung nearest.
//! - **Settings.** Each column's underlay is its width's ([`super::underlay`]). One that ends under another
//!   is its area's overlap longer at that end, by its push compensation, and so reaches under it, as an area
//!   reaches under the shapes sewn after it.
//! - **A dot.** A part whose every line is shorter than half its width there is a dot, which stays in the
//!   fill, as one with no line does. Such a part has no way to run, as a disc or a square has none, and its
//!   lines are only what its triangles leave of its middle: corners that lie on one circle tie, and how each
//!   tie breaks depends on where the part lies and how it is drawn, so that of 2 dots alike one would sew a
//!   column across and the other a fill. A disc's or a square's lines are 0.4 times as long as it is wide,
//!   and a rectangle 3 mm wide is a column from 3.5 mm long ([`DOT`]).
//! - **Drawn.** Each column is drawn with its rails and rungs, and read back as a satin column is read, it
//!   must give them ([`drawn::from_rails`]). A part whose triangles make no column round each line, or one of
//!   whose columns can't be drawn so, stays in the area's fill, and `SC-I0902` says so. A rung added to tell
//!   the rails from the rungs goes across an end that doesn't reach under another column where one can take
//!   it; across one that does, the overlap sews as a section of its own.
//!
//! Every step costs a unit of work, a part's columns being part of the work of digitizing its area.

use stitchcraft_core::{Code, Diagnostic, ElementId, Exhausted, Fix, Meter, Mm, Point};

use super::narrow::{Edges, piece, width};
use super::{AutoParams, at_most};
use crate::design::Path;
use crate::generators::mm;
use crate::generators::satin::drawn;
use crate::normalize::region::Polygon;
use crate::normalize::strips::{self, Column, Line};

/// How long, at the least, a part's line is for a column, as a share of the part's width along it: a part
/// whose every line is shorter is a dot, which stays in the fill. Of a disc or a square, which tie, the lines
/// are 0.4 times as long as it is wide, and of a rectangle 3 mm wide, half as long from 3.5 mm long.
pub(super) const DOT: f64 = 0.5;

/// A part sewn as columns, and its columns.
#[derive(Clone, Debug, PartialEq)]
pub(super) struct Columns {
    /// The part, which its columns paint.
    pub(super) part: Polygon,
    /// Its columns, in the order they are sewn.
    pub(super) sewn: Vec<Sewn>,
}

/// A column of a part as it is sewn.
#[derive(Clone, Debug, PartialEq)]
pub(super) enum Sewn {
    /// A satin column: the path that draws its rails and rungs, its width, and how much shorter it is made at
    /// its start, then its end (negative: longer).
    Satin { path: Path, width: Mm, push: [f64; 2] },
    /// A running stitch along the column's centre line, too narrow for satin, and the column's width.
    Running { line: Line, width: Mm },
}

/// What becomes of a part no wider than a satin column.
pub(super) enum Made {
    /// It sews as these columns.
    Columns(Vec<Sewn>),
    /// It has no line, a dot, and stays in the fill.
    Dot,
    /// Its columns can't be drawn, and it stays in the fill.
    Unmade,
    /// Its triangulation lost its shape, a bug, and it stays in the fill.
    Broken,
}

/// The columns of `part`, an area's whose narrowest satin stroke is `narrowest` and whose settings of the
/// rules are `rules`, cut from a broad part along `cuts` (`auto::split`), with `meter`'s work.
pub(super) fn columns(part: &Polygon, narrowest: Mm, rules: &AutoParams, cuts: &[[Point; 2]], meter: &mut Meter) -> Result<Made, Exhausted> {
    let found = match strips::columns(part, piece(part), cuts, meter)? {
        Err(_) => return Ok(Made::Broken),
        Ok(None) => return Ok(Made::Unmade),
        Ok(Some(found)) if found.is_empty() => return Ok(Made::Dot),
        Ok(Some(found)) => found,
    };
    let edges = Edges::of(part, piece(part), meter)?;
    let mut widths = Vec::with_capacity(found.len());
    for column in &found {
        let Some(width) = width(&edges, std::slice::from_ref(&column.line), meter)? else { return Ok(Made::Unmade) };
        widths.push(width);
    }
    if found.iter().zip(&widths).all(|(column, width)| column.walked.length() < DOT * width.get()) {
        return Ok(Made::Dot);
    }
    let mut sewn = Vec::with_capacity(found.len());
    for (index, column) in ordered(&found, meter)? {
        let Some(&width) = widths.get(index) else { return Ok(Made::Unmade) };
        if at_most(width, narrowest) {
            sewn.push(Sewn::Running { line: column.walked, width });
            continue;
        }
        let push = column.under.map(|under| if under.is_some() { -rules.auto_overlap_mm.get() } else { 0.0 });
        let Ok(drawn) = drawn::from_rails(column.rails, column.rungs, push, meter)? else { return Ok(Made::Unmade) };
        sewn.push(Sewn::Satin { path: drawn.path(), width, push });
    }
    Ok(Made::Columns(sewn))
}

/// `columns` in the order they are sewn, each with its place among them, turned to start where it is sewn
/// from, with `meter`'s work: a column that ends under another before it, then the longest first, and each
/// next the one with an end nearest where the last one ended, sewn from that end, or a ring from its rung
/// nearest. Where the columns that end under each other go round, as a pinwheel's bars do, none can sew
/// after every column it ends under: the one that comes first in that order sews first, and the last of them
/// ends on top of it. On a tie, the column found first.
fn ordered(columns: &[Column], meter: &mut Meter) -> Result<Vec<(usize, Column)>, Exhausted> {
    let count = columns.len();
    // How many columns end under each, which must sew before it.
    let mut waiting = vec![0_usize; count];
    for column in columns {
        for &under in column.under.iter().flatten() {
            if let Some(slot) = waiting.get_mut(under) {
                *slot += 1;
            }
        }
    }
    let mut done = vec![false; count];
    let mut out: Vec<(usize, Column)> = Vec::with_capacity(count);
    let mut here: Option<Point> = None;
    // Each step sews one column.
    for _ in 0..count {
        meter.charge(u64::try_from(count).unwrap_or(u64::MAX))?;
        let free = |c: usize| !done.get(c).copied().unwrap_or(true);
        let mut ready: Vec<usize> = (0..count).filter(|&c| free(c) && waiting.get(c) == Some(&0)).collect();
        if ready.is_empty() {
            ready = (0..count).filter(|&c| free(c)).collect();
        }
        let pick = match here {
            None => ready.iter().copied().fold(None, |best: Option<(usize, f64)>, c| {
                let length = columns.get(c).map_or(0.0, |column| column.walked.length());
                if best.is_some_and(|(_, most)| most >= length) { best } else { Some((c, length)) }
            }),
            Some(here) => ready.iter().copied().fold(None, |best: Option<(usize, f64)>, c| {
                let distance = columns.get(c).map_or(f64::INFINITY, |column| nearest_end(column, here));
                if best.is_some_and(|(_, least)| least <= distance) { best } else { Some((c, distance)) }
            }),
        };
        let Some((c, _)) = pick else { break };
        let Some(column) = columns.get(c) else { break };
        let column = match here {
            Some(here) if column.closed => column.restarted(here, meter)?,
            Some(here) if end_nearer(column, here) => column.reversed(),
            _ => column.clone(),
        };
        if let Some(slot) = done.get_mut(c) {
            *slot = true;
        }
        for &under in column.under.iter().flatten() {
            if let Some(slot) = waiting.get_mut(under) {
                *slot = slot.saturating_sub(1);
            }
        }
        here = if column.closed { column.rails[0].first().copied() } else { column.line.points.last().copied() };
        out.push((c, column));
    }
    Ok(out)
}

/// How far `here` is from where `column` can start: its centre line's nearer end, or for a ring, its centre
/// line's nearest point.
fn nearest_end(column: &Column, here: Point) -> f64 {
    let points = &column.line.points;
    let candidates: Vec<&Point> = if column.closed { points.iter().collect() } else { points.first().into_iter().chain(points.last()).collect() };
    candidates.into_iter().map(|p| p.distance(here)).fold(f64::INFINITY, f64::min)
}

/// Whether the open `column`'s centre line ends nearer `here` than it starts.
fn end_nearer(column: &Column, here: Point) -> bool {
    let points = &column.line.points;
    points.first().zip(points.last()).is_some_and(|(start, end)| end.distance(here) < start.distance(here))
}

/// `SC-I0902` for a part of the automatic area `id` no wider than its widest satin column by `rules`, whose
/// satin columns can't be drawn: it stays in the area's fill.
pub(super) fn unmade(id: &ElementId, rules: &AutoParams) -> Diagnostic {
    let message = format!(
        "A part of this area is no wider than a satin column, {} mm, but no satin columns can be drawn along it between its edges, so it is \
         filled.",
        mm(rules.auto_max_satin_width_mm.get())
    );
    Diagnostic::new(Code::AutoSatinFilled, message).with_element(id.clone()).with_fix(Fix::Hint(
        "To sew it as satin columns, draw it with smoother edges, or draw the columns with 2 rails and give them settings.".to_string(),
    ))
}

#[cfg(test)]
mod tests {
    use stitchcraft_core::math::{self, sin_cos};
    use stitchcraft_core::{Budget, Code, ElementId};

    use super::*;
    use crate::normalize::fixture::part;

    /// A band `width` millimetres wide and 20 mm long.
    fn band(width: f64) -> Polygon {
        part(&[(0.0, 0.0), (20.0, 0.0), (20.0, width), (0.0, width)], &[])
    }

    /// `n` steps of a circle of radius `r` about the origin, from `from` radians round `turn` radians, both
    /// ends included.
    fn arc(r: f64, n: usize, from: f64, turn: f64) -> Vec<(f64, f64)> {
        (0..=n)
            .map(|i| {
                let (sin, cos) = sin_cos(from + turn * i as f64 / n as f64);
                (r * cos, r * sin)
            })
            .collect()
    }

    /// The columns of `part` as found, and in the order they are sewn, each with its place among those found.
    fn sewn(part: &Polygon) -> (Vec<Column>, Vec<(usize, Column)>) {
        let mut meter = Budget::DEFAULT.meter();
        let found = strips::columns(part, piece(part), &[], &mut meter).unwrap().unwrap().unwrap();
        let order = ordered(&found, &mut meter).unwrap();
        (found, order)
    }

    /// Where `column`'s centre line starts and ends.
    fn ends(column: &Column) -> (Point, Point) {
        (column.line.points[0], *column.line.points.last().unwrap())
    }

    #[test]
    fn req_auto_016_a_column_no_wider_than_the_narrowest_satin_stroke_runs() {
        // A band 1.5 mm wide is one column: a satin column where the narrowest satin stroke is 1 mm, and a
        // running stitch along its line, as wide as the band, where it is 2 mm.
        let made =
            |narrowest: f64| columns(&band(1.5), Mm::new(narrowest).unwrap(), &AutoParams::defaults(), &[], &mut Budget::DEFAULT.meter()).unwrap();
        let Made::Columns(sewn) = made(1.0) else { panic!("no columns") };
        let [Sewn::Satin { width, push, .. }] = sewn.as_slice() else { panic!("{sewn:?}") };
        assert!((width.get() - 1.5).abs() < 1e-6 && *push == [0.0, 0.0], "{width:?} {push:?}");
        let Made::Columns(sewn) = made(2.0) else { panic!("no columns") };
        let [Sewn::Running { width, line }] = sewn.as_slice() else { panic!("{sewn:?}") };
        assert!((width.get() - 1.5).abs() < 1e-6 && line.points.iter().all(|q| (q.y() - 0.75).abs() < 1e-9), "{width:?} {line:?}");
    }

    #[test]
    fn req_auto_016_each_column_sews_from_its_end_nearest_where_the_last_ended() {
        // An H, its bars and its crossbar 3 mm wide. The crossbar ends under both bars and sews first, then the
        // bar with an end nearest where it ended, from that end, and the other bar from its end nearest where
        // that one ended. The crossbar lies low and high, so that a bar is turned round in one H and not in
        // the other.
        let (mut turned, mut kept) = (0, 0);
        for (low, high) in [(4.0, 7.0), (13.0, 16.0)] {
            let h = part(
                &[
                    (0.0, 0.0),
                    (3.0, 0.0),
                    (3.0, low),
                    (10.0, low),
                    (10.0, 0.0),
                    (13.0, 0.0),
                    (13.0, 20.0),
                    (10.0, 20.0),
                    (10.0, high),
                    (3.0, high),
                    (3.0, 20.0),
                    (0.0, 20.0),
                ],
                &[],
            );
            let (found, order) = sewn(&h);
            let [(crossbar, first), (_, second), (_, third)] = order.as_slice() else { panic!("{order:?}") };
            assert_eq!(found[*crossbar].under.iter().flatten().count(), 2, "{found:?}");
            let here = ends(first).1;
            assert!(nearest_end(second, here) < nearest_end(third, here), "{order:?}");
            for (column, here) in [(second, here), (third, ends(second).1)] {
                let (start, end) = ends(column);
                assert!(start.distance(here) < end.distance(here), "{column:?} from {here:?}");
            }
            for (place, column) in &order {
                if *column == found[*place] { kept += 1 } else { turned += 1 }
            }
        }
        assert!(turned > 0 && kept > 0, "{turned} turned, {kept} kept");
    }

    #[test]
    fn a_column_is_as_far_as_its_nearer_end_and_a_ring_as_its_nearest_point() {
        let [column] = sewn(&band(3.0)).0.try_into().unwrap();
        let (start, end) = ends(&column);
        let (by_start, by_end) = (Point::new(start.x(), start.y() + 4.0).unwrap(), Point::new(end.x(), end.y() - 2.0).unwrap());
        assert!((nearest_end(&column, by_start) - 4.0).abs() < 1e-9 && (nearest_end(&column, by_end) - 2.0).abs() < 1e-9);
        assert!(end_nearer(&column, by_end) && !end_nearer(&column, by_start));
        // As near both ends, it keeps its way.
        let point = |x: f64, y: f64| Point::new(x, y).unwrap();
        let mut level = column;
        level.line = Line { points: vec![point(0.0, 0.0), point(10.0, 0.0)], closed: false };
        assert!(!end_nearer(&level, point(5.0, 3.0)) && end_nearer(&level, point(5.001, 3.0)));
        // A ring 3 mm wide round a hole 10 mm across, its line about 6.5 mm from the centre.
        let ring = part(&arc(8.0, 24, 0.0, std::f64::consts::TAU)[..24], &[&arc(5.0, 24, 0.0, std::f64::consts::TAU)[..24]]);
        let [ring] = sewn(&ring).0.try_into().unwrap();
        let far = nearest_end(&ring, Point::new(20.0, 0.0).unwrap());
        assert!((far - 13.5).abs() < 0.2, "{far}");
    }

    #[test]
    fn columns_that_end_under_each_other_round_a_ring_all_sew() {
        // A pinwheel: a square ring of bars 3 mm wide round a hole 9 mm across, each bar running on 3 mm past
        // the corner where the bar before it ends against its side. Each bar's column ends under the next
        // one's, round the ring, so none can sew after all those it ends under: the longest sews first, the
        // first found of as long, and then each column the last one ends under.
        let pinwheel = part(
            &[
                (-3.0, 0.0),
                (12.0, 0.0),
                (12.0, -3.0),
                (15.0, -3.0),
                (15.0, 12.0),
                (18.0, 12.0),
                (18.0, 15.0),
                (3.0, 15.0),
                (3.0, 18.0),
                (0.0, 18.0),
                (0.0, 3.0),
                (-3.0, 3.0),
            ],
            &[&[(3.0, 3.0), (12.0, 3.0), (12.0, 12.0), (3.0, 12.0)]],
        );
        let (found, order) = sewn(&pinwheel);
        assert_eq!(found.len(), 4);
        assert!(found.iter().all(|column| column.under.iter().flatten().count() == 1), "{found:?}");
        let places: Vec<usize> = order.iter().map(|(place, _)| *place).collect();
        assert_eq!(places[0], 0, "{places:?}");
        for pair in places.windows(2) {
            assert!(found[pair[0]].under.contains(&Some(pair[1])), "{places:?}");
        }
    }

    #[test]
    fn the_longest_column_that_can_sew_first_sews_first() {
        // A plus of bars 3 mm wide: the column through its middle waits for the 3 that end under it, and of
        // those the longest sews first, the first found of as long.
        let plus = part(
            &[
                (8.5, 0.0),
                (11.5, 0.0),
                (11.5, 13.5),
                (25.0, 13.5),
                (25.0, 16.5),
                (11.5, 16.5),
                (11.5, 30.0),
                (8.5, 30.0),
                (8.5, 16.5),
                (0.0, 16.5),
                (0.0, 13.5),
                (8.5, 13.5),
            ],
            &[],
        );
        let (found, order) = sewn(&plus);
        let free: Vec<usize> = (0..found.len()).filter(|&c| found.iter().all(|column| !column.under.contains(&Some(c)))).collect();
        assert!(free.len() > 1, "{found:?}");
        let length = |c: usize| found[c].walked.length();
        let longest = free.iter().copied().fold(free[0], |best, c| if length(c) > length(best) { c } else { best });
        assert_eq!(order[0].0, longest, "{order:?}");
    }

    #[test]
    fn a_ring_sewn_after_another_column_starts_at_its_rung_nearest_where_that_ended() {
        // A ring 3 mm wide round a hole 10 mm across, with a stem 3 mm wide above it, 10 mm long, and one below
        // it, 6 mm long, each ending under the ring. The stems sew first, the longer first, and then the ring,
        // from its rung nearest where the second stem ended.
        let side = math::acos(1.5 / 8.0);
        let mut outline = arc(8.0, 16, side, -2.0 * side);
        outline.extend([(1.5, -14.0), (-1.5, -14.0)]);
        outline.extend(arc(8.0, 16, side - std::f64::consts::PI, -2.0 * side));
        outline.extend([(-1.5, 18.0), (1.5, 18.0)]);
        let ringed = part(&outline, &[&arc(5.0, 24, 0.0, std::f64::consts::TAU)[..24]]);
        let (found, order) = sewn(&ringed);
        let [(_, _), (_, last), (place, ring)] = order.as_slice() else { panic!("{order:?}") };
        assert!(ring.closed && !last.closed, "{order:?}");
        let here = ends(last).1;
        let starts = found[*place].rungs.iter().map(|rung| rung[0]).chain([found[*place].rails[0][0]]);
        let nearest = starts.map(|q| q.distance(here)).fold(f64::INFINITY, f64::min);
        assert!(ring.rails[0][0].distance(here) <= nearest + 1e-9, "{:?} from {here:?}", ring.rails[0][0]);
    }

    #[test]
    fn a_part_no_satin_column_can_be_drawn_along_is_said_to_stay_filled() {
        let id = ElementId::new("letters").unwrap();
        let said = unmade(&id, &AutoParams::defaults());
        assert_eq!((said.code, said.element.as_ref()), (Code::AutoSatinFilled, Some(&id)));
        assert_eq!(
            said.to_string(),
            "info SC-I0902: A part of this area is no wider than a satin column, 7 mm, but no satin columns can be drawn along it between \
             its edges, so it is filled."
        );
    }
}
