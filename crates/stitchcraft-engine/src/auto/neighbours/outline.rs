//! The outline round a fill (`REQ-AUTO-051`): a running stitch along the edge of each automatic area where the
//! edge shows, sewn after the area's rows in the area's own thread.
//!
//! Design: `docs/src/design/image-to-embroidery.md` › An outline round a fill. A tatami fill's rows end at its
//! edge, each a stitch's width from the next, and the thread pulls each row's end in. On a small shape with
//! points, a star 5 mm across, the rows' ends are most of what shows, and its points sew blunt. A digitizer
//! runs a stitch round such a shape after filling it, which draws the edge as one line. A design asks for it
//! with `auto_fill_outline`.
//!
//! - **Where.** The edge of an area is the rings of its region as the layers and the specks left it, before
//!   it reaches under its neighbours: what shows of it. A shape sewn after the area that touches its edge
//!   covers the edge there, by the overlap the area reaches under it, and a run there would lie under that
//!   shape's stitches and thicken its edge. So the outline leaves out the edge within [`TOUCHING`] of each
//!   shape sewn after the area, and of each other element of the area's own shape, whose columns and lines
//!   are the same shape going on. A line digitized from a stroke goes over an area and covers none of it.
//! - **How.** Each edge is cut into pieces no longer than [`STEP`], and a piece shows when its middle lies in
//!   none of those shapes. The pieces that show in a row are one line, a ring's when all of it shows, and a
//!   line shorter than [`SHORTEST`] is left out: it would sew a knot beside its neighbour's edge.
//! - **Work.** Each area has the budget's work to itself: a unit for each shape it looks past, the work of
//!   growing those near, and for each piece a unit for each corner of the shapes whose bounds hold its middle.
//!   An area that needs more sews no outline, and that is said (`SC-I0904`).

use stitchcraft_core::{Budget, Code, Diagnostic, ElementId, Meter, Point};

use super::{Failed, Kind, Shapes, TOUCHING, envelope, near};
use crate::normalize::region::Polygon;
use crate::normalize::region::geom::{self, Envelope, Location};
use crate::normalize::strips::Line;

/// The longest piece an edge is cut into to find where it shows, in millimetres: an outline starts and ends
/// within this of where a neighbour takes the edge over, a fifth of a running stitch.
pub(super) const STEP: f64 = 0.5;

/// The shortest line of an outline that sews, in millimetres: 2 running stitches. A shorter one is a knot.
pub(super) const SHORTEST: f64 = 3.0;

/// For each automatic area of `shapes` whose edge shows somewhere, in the order they sew, its id and the lines
/// of its outline, each area with `budget`'s work to itself; and `SC-I0904` for each area that needed more,
/// which sews no outline.
pub(super) fn outlines(shapes: &mut Shapes, budget: &Budget) -> (Vec<(ElementId, Vec<Line>)>, Vec<Diagnostic>) {
    let (mut found, mut said) = (Vec::new(), Vec::new());
    for index in 0..shapes.len() {
        let (Some(id), Some(region)) = (shapes.id(index), shapes.region(index).map(<[Polygon]>::to_vec)) else { continue };
        match shown(shapes, index, &region, &mut budget.meter()) {
            Ok(lines) if lines.is_empty() => {}
            Ok(lines) => found.push((id, lines)),
            Err(failed) => {
                let (finding, consequence) = ("where this area's edge meets the shapes sewn after it", "it sews no outline");
                said.push(failed.said(Code::AutoUnder, finding, consequence, budget.max_work, id));
            }
        }
    }
    (found, said)
}

/// The lines of the outline of the automatic area `index` of `shapes`, whose region is `region`: its edge
/// outside the shapes sewn after it and the other elements of its own shape, with `meter`'s work.
fn shown(shapes: &mut Shapes, index: usize, region: &[Polygon], meter: &mut Meter) -> Result<Vec<Line>, Failed> {
    let Some(bounds) = envelope(region) else { return Ok(Vec::new()) };
    let mut covering = Vec::new();
    for other in 0..shapes.len() {
        meter.charge(1)?;
        let own = other != index && shapes.of_shape.get(other) == shapes.of_shape.get(index);
        let covers = (other > index || own) && shapes.kind(other) != Some(Kind::Line);
        if !covers || !shapes.bounds(other).is_some_and(|theirs| near(&bounds, TOUCHING, &theirs)) {
            continue;
        }
        covering.extend(shapes.grown(other, meter)?.unwrap_or_default());
    }
    let covering: Vec<(Envelope, &Polygon)> = covering.iter().map(|part| (Envelope::of(&part.outline), part)).collect();
    let mut lines = Vec::new();
    for ring in region.iter().flat_map(Polygon::rings) {
        lines.extend(uncovered(ring, &covering, meter)?);
    }
    Ok(lines)
}

/// Whether `p` lies in one of `parts`, each with its bounds, on its edge included, with `meter`'s work.
fn covered(p: Point, parts: &[(Envelope, &Polygon)], meter: &mut Meter) -> Result<bool, Failed> {
    let here = Envelope::of(&[p]);
    for (bounds, part) in parts {
        meter.charge(1)?;
        if !bounds.covers(&here) {
            continue;
        }
        meter.charge(u64::try_from(part.rings().map(<[Point]>::len).sum::<usize>()).unwrap_or(u64::MAX))?;
        let inside = geom::locate_in_ring(p, &part.outline) != Location::Exterior;
        if inside && !part.holes.iter().any(|hole| geom::locate_in_ring(p, hole) == Location::Interior) {
            return Ok(true);
        }
    }
    Ok(false)
}

/// The lines along the closed `ring` that lie outside `parts`: the ring cut into pieces no longer than
/// [`STEP`], those whose middles `parts` don't hold joined in rows, the ring whole when all of it shows, and
/// the lines shorter than [`SHORTEST`] left out. A line keeps the ring's corners, and its ends lie where a
/// piece ends.
fn uncovered(ring: &[Point], parts: &[(Envelope, &Polygon)], meter: &mut Meter) -> Result<Vec<Line>, Failed> {
    // Each piece: where it ends, whether that is a corner of the ring, and whether it shows.
    let mut pieces = Vec::new();
    let Some(&start) = ring.first() else { return Ok(Vec::new()) };
    for pair in ring.windows(2) {
        let (Some(&a), Some(&b)) = (pair.first(), pair.last()) else { continue };
        let count = (a.distance(b) / STEP).ceil().max(1.0);
        let mut from = a;
        let mut k = 1.0;
        while k <= count {
            let to = if k == count { b } else { a.lerp(b, k / count) };
            pieces.push((to, k == count, !covered(from.lerp(to, 0.5), parts, meter)?));
            (from, k) = (to, k + 1.0);
        }
    }
    let Some(hidden) = pieces.iter().position(|&(_, _, shows)| !shows) else {
        // All of it shows: the ring, by its corners.
        let points: Vec<Point> = pieces.iter().filter(|&&(_, corner, _)| corner).map(|&(to, ..)| to).collect();
        let line = Line { points, closed: true };
        return Ok(if line.length() >= SHORTEST { vec![line] } else { Vec::new() });
    };
    // From the piece after a hidden one round to it again, so that no line is cut where the ring starts.
    let mut lines = Vec::new();
    let mut points: Vec<Point> = Vec::new();
    let mut from = pieces.get(hidden).map_or(start, |&(to, ..)| to);
    for step in 1..=pieces.len() {
        let Some(&(to, corner, shows)) = pieces.get((hidden + step) % pieces.len()) else { continue };
        if shows {
            if points.is_empty() {
                points.push(from);
            }
            if corner {
                points.push(to);
            }
        } else if !points.is_empty() {
            if points.last() != Some(&from) {
                points.push(from);
            }
            lines.push(Line { points: std::mem::take(&mut points), closed: false });
        }
        from = to;
    }
    lines.retain(|line| line.length() >= SHORTEST);
    Ok(lines)
}

#[cfg(test)]
mod tests {
    use stitchcraft_core::{Budget, Mm};
    use stitchcraft_params::ParamSet;

    use super::super::tests::{filled, square};
    use super::super::{AutoParams, Sewn};
    use super::*;
    use crate::design::{Cap, DesignSettings, Element, Join, Path, Segment, Shape, Subpath};

    fn p(x: f64, y: f64) -> Point {
        Point::new(x, y).unwrap()
    }

    /// An automatic area `id`, `part`, of the shape at `shape` in the design.
    fn area(id: &str, part: Polygon, shape: usize) -> Sewn {
        Sewn { element: filled(id, &[part]), area: Some(AutoParams::defaults()), automatic: true, reach: None, shape, part: None }
    }

    /// An area's id, and each line of its outline: its length to the micrometre, and whether it closes.
    type Outlined = (String, Vec<(f64, bool)>);

    /// The outlines of `sewn`, by area, found with `budget`, and how much was said.
    fn outlined(sewn: Vec<Sewn>, budget: &Budget) -> (Vec<Outlined>, usize) {
        let mut shapes = Shapes::new(sewn, &DesignSettings::default(), &Budget::DEFAULT);
        let (found, said) = outlines(&mut shapes, budget);
        assert!(said.iter().all(|said| said.code == Code::AutoUnder && said.message.ends_with("so it sews no outline.")), "{said:?}");
        let lines = |lines: &[Line]| lines.iter().map(|line| ((line.length() * 1000.0).round() / 1000.0, line.closed)).collect();
        (found.iter().map(|(id, found)| (id.as_str().to_string(), lines(found))).collect(), said.len())
    }

    /// [`outlined`] with the default budget, of which nothing is said.
    fn outlines_of(sewn: Vec<Sewn>) -> Vec<Outlined> {
        let (found, said) = outlined(sewn, &Budget::DEFAULT);
        assert_eq!(said, 0);
        found
    }

    fn one(id: &str, length: f64, closed: bool) -> Outlined {
        (id.to_string(), vec![(length, closed)])
    }

    #[test]
    fn req_auto_051_an_area_s_outline_runs_round_its_edge_where_no_shape_sewn_after_it_covers_it() {
        let (left, right) = (square((0.0, 0.0), (10.0, 10.0)), square((10.0, 0.0), (20.0, 10.0)));
        // Alone, a square is outlined whole: its ring, by its 4 corners.
        let mut shapes = Shapes::new(vec![area("a", left.clone(), 0)], &DesignSettings::default(), &Budget::DEFAULT);
        let (found, said) = outlines(&mut shapes, &Budget::DEFAULT);
        let [(id, lines)] = found.as_slice() else { panic!("{found:?}") };
        assert_eq!((id.as_str(), said.len()), ("a", 0));
        assert_eq!(lines, &[Line { points: vec![p(0.0, 10.0), p(10.0, 10.0), p(10.0, 0.0), p(0.0, 0.0)], closed: true }]);
        // Beside a square sewn after it, it leaves out the edge they share: the later square covers that
        // edge, and is outlined whole itself.
        let pair = outlines_of(vec![area("a", left.clone(), 0), area("b", right.clone(), 1)]);
        assert_eq!(pair, [one("a", 30.0, false), one("b", 40.0, true)]);
        // 2 elements of one shape leave the edge between them out both: the shape goes on across it.
        let own = outlines_of(vec![area("a", left.clone(), 0), area("b", right, 0)]);
        assert_eq!(own, [one("a", 30.0, false), one("b", 30.0, false)]);
        // A line that starts where the ring does isn't cut there: a square that covers 0.8 mm of the first
        // side's middle hides the 2 pieces of 0.5 mm whose middles it holds, and leaves one line of 39 mm, from
        // one of its sides round to the other.
        let notch = square((-1.0, 4.6), (1.0, 5.4));
        let mut shapes = Shapes::new(vec![area("a", left.clone(), 0), area("n", notch, 1)], &DesignSettings::default(), &Budget::DEFAULT);
        let (found, _) = outlines(&mut shapes, &Budget::DEFAULT);
        let line = &found[0].1[0];
        assert_eq!((found[0].1.len(), line.closed, line.points.first(), line.points.last()), (1, false, Some(&p(0.0, 5.5)), Some(&p(0.0, 4.5))));
        assert!((line.length() - 39.0).abs() < 1e-9 && line.points.len() == 6, "{line:?}");
        // A shape far from it covers none of it, and neither does a line digitized from a stroke, which
        // goes over the area.
        let far = square((50.0, 50.0), (60.0, 60.0));
        assert_eq!(outlines_of(vec![area("a", left.clone(), 0), area("f", far, 1)])[0], one("a", 40.0, true));
        let along = Subpath { start: p(10.0, -5.0), segments: vec![Segment::Line(p(10.0, 15.0))], closed: false };
        let stroke = Shape::Stroke { path: Path { subpaths: vec![along] }, width: Mm::new(0.4).unwrap(), join: Join::Round, cap: Cap::Round };
        let line = Sewn {
            element: Element { shape: stroke, params: ParamSet::default(), ..filled("l", std::slice::from_ref(&left)) },
            area: None,
            automatic: true,
            reach: Some(1.0),
            shape: 1,
            part: None,
        };
        assert_eq!(outlines_of(vec![area("a", left, 0), line]), [one("a", 40.0, true)]);
    }

    #[test]
    fn req_auto_051_an_outline_shorter_than_2_stitches_is_left_out() {
        // A square 1 mm across has a ring of 4 mm, which sews, and one 0.7 mm across a ring of 2.8 mm, which
        // doesn't: the area has no outline.
        assert_eq!(outlines_of(vec![area("a", square((0.0, 0.0), (1.0, 1.0)), 0)]), [one("a", 4.0, true)]);
        assert_eq!(outlines_of(vec![area("a", square((0.0, 0.0), (0.7, 0.7)), 0)]), []);
        // A ring exactly 3 mm long sews.
        assert_eq!(outlines_of(vec![area("a", square((0.0, 0.0), (0.75, 0.75)), 0)]), [one("a", 3.0, true)]);
        // A square 2.5 mm across in the mouth of a U sewn after it shows one side: 2.5 mm, left out.
        let u = Polygon {
            outline: vec![p(-5.0, -5.0), p(-5.0, 2.5), p(0.0, 2.5), p(0.0, 0.0), p(2.5, 0.0), p(2.5, 2.5), p(7.5, 2.5), p(7.5, -5.0), p(-5.0, -5.0)],
            holes: Vec::new(),
        };
        let small = square((0.0, 0.0), (2.5, 2.5));
        assert_eq!(outlines_of(vec![area("a", small, 0), area("u", u.clone(), 1)]).iter().map(|(id, _)| id.as_str()).collect::<Vec<_>>(), ["u"]);
        // In a mouth 3 mm wide the side of 3 mm sews.
        let wide = Polygon { outline: u.outline.iter().map(|q| if q.x() == 2.5 { p(3.0, q.y()) } else { *q }).collect(), holes: Vec::new() };
        let found = outlines_of(vec![area("a", square((0.0, 0.0), (3.0, 2.5)), 0), area("u", wide, 1)]);
        assert_eq!(found[0], one("a", 3.0, false));
    }

    #[test]
    fn req_auto_051_an_edge_in_a_hole_of_a_later_shape_shows() {
        // A square in the hole of a frame sewn after it, 1 mm clear of it all round, is outlined whole: the
        // frame's bounds hold it, and its hole does too. A square that fills the hole shows no edge.
        let frame = |hole: Polygon| Polygon { holes: vec![hole.outline.iter().rev().copied().collect()], ..square((0.0, 0.0), (10.0, 10.0)) };
        let clear = outlines_of(vec![area("a", square((4.0, 4.0), (6.0, 6.0)), 0), area("f", frame(square((3.0, 3.0), (7.0, 7.0))), 1)]);
        assert_eq!(clear[0], one("a", 8.0, true));
        let filling = outlines_of(vec![area("a", square((4.0, 4.0), (6.0, 6.0)), 0), area("f", frame(square((4.0, 4.0), (6.0, 6.0))), 1)]);
        assert_eq!(filling.iter().map(|(id, _)| id.as_str()).collect::<Vec<_>>(), ["f"]);
        // A part with no ring at all has no outline.
        assert_eq!(uncovered(&[], &[], &mut Budget::DEFAULT.meter()).map_err(|_| ()), Ok(Vec::new()));
    }

    #[test]
    fn req_auto_051_an_area_whose_outline_needs_more_work_than_its_budget_allows_sews_none_and_says_so() {
        let sewn = || vec![area("a", square((0.0, 0.0), (10.0, 10.0)), 0), area("b", square((10.0, 0.0), (20.0, 10.0)), 1)];
        let full = outlined(sewn(), &Budget::DEFAULT);
        let with = |work: u64| outlined(sewn(), &Budget { max_work: work, ..Budget::DEFAULT });
        let least = (0..1 << 20).find(|work| with(*work) == full).unwrap();
        // With less, the first area, which looks at the second, has no outline, and that is said of it; the
        // second, which needs less, may have its own.
        for work in 0..least {
            let (found, said) = with(work);
            assert!(said >= 1 && found.iter().all(|(id, _)| id == "b"), "{work}: {found:?}");
        }
        assert!(least > 80, "{least}");
        let mut shapes = Shapes::new(sewn(), &DesignSettings::default(), &Budget::DEFAULT);
        let (_, said) = outlines(&mut shapes, &Budget { max_work: 0, ..Budget::DEFAULT });
        assert_eq!(said[0].element.as_ref().map(|id| id.as_str()), Some("a"));
        assert_eq!(
            said[0].to_string(),
            "info SC-I0904: Finding where this area's edge meets the shapes sewn after it needed more than the work budget of 0 units, so it \
             sews no outline."
        );
    }
}
