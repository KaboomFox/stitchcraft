//! Cases of a part read as strokes (`REQ-AUTO-087`, `REQ-AUTO-088`): the traced word's M and A, a T, an L,
//! a cross, and the parts that keep their centre lines' columns, with the bounds of each measure.

use stitchcraft_core::{Budget, math};

use super::joins::{crossed, end, halved, join, leaves, meeting, square, stops};
use super::*;
use crate::normalize::fixture::part;
use crate::normalize::strips::columns;

/// A way of turning or mirroring a part's points.
type Turn = fn((f64, f64)) -> (f64, f64);

/// A way of turning or mirroring a part's points, from its outline's.
type TurnPoint = fn(Point) -> (f64, f64);

/// The pull compensation of the cases' columns, in millimetres: the profile's default.
const REACH: f64 = 0.4;

/// The narrowest and widest stroke of the cases.
const LIMITS: (f64, f64) = (0.5, 7.0);

fn p(x: f64, y: f64) -> Point {
    Point::new(x, y).unwrap()
}

fn side(a: (f64, f64), b: (f64, f64)) -> Side {
    Side { a: p(a.0, a.1), b: p(b.0, b.1) }
}

/// `ring` scaled by `by`.
fn scaled(ring: &[(f64, f64)], by: f64) -> Vec<(f64, f64)> {
    ring.iter().map(|&(x, y)| (x * by, y * by)).collect()
}

/// The traced word's M, `height` mm tall.
fn m(height: f64) -> Polygon {
    let ring = [
        (0.0, 1.0),
        (0.0, 0.0),
        (0.22, 0.0),
        (0.46, 0.70),
        (0.70, 0.0),
        (0.92, 0.0),
        (0.92, 1.0),
        (0.74, 1.0),
        (0.74, 0.4385),
        (0.5475, 1.0),
        (0.3725, 1.0),
        (0.18, 0.4385),
        (0.18, 1.0),
    ];
    part(&scaled(&ring, height), &[])
}

/// The traced word's A, `height` mm tall.
fn a(height: f64) -> Polygon {
    let outline = scaled(&[(0.0, 1.0), (0.35, 0.0), (0.51, 0.0), (0.86, 1.0), (0.6693, 1.0), (0.5923, 0.78), (0.2677, 0.78), (0.1907, 1.0)], height);
    let hole = scaled(&[(0.43, 0.3163), (0.3307, 0.6), (0.5293, 0.6)], height);
    part(&outline, &[&hole])
}

/// A T: a bar 6 mm long and 1 mm wide on a stem 5 mm long.
fn t() -> Polygon {
    part(&[(0.0, 5.0), (2.5, 5.0), (2.5, 0.0), (3.5, 0.0), (3.5, 5.0), (6.0, 5.0), (6.0, 6.0), (0.0, 6.0)], &[])
}

/// An L: a stem 6 mm long and a foot 5 mm long, both 1 mm wide, its inner corner cut `chamfer` mm along each.
fn l(chamfer: f64) -> Polygon {
    let mut ring = vec![(0.0, 0.0), (5.0, 0.0), (5.0, 1.0)];
    if chamfer > 0.0 {
        ring.extend([(1.0 + chamfer, 1.0), (1.0, 1.0 + chamfer)]);
    } else {
        ring.push((1.0, 1.0));
    }
    ring.extend([(1.0, 6.0), (0.0, 6.0)]);
    part(&ring, &[])
}

/// A cross: a bar 8 mm long across one 7 mm long, both 1 mm wide.
fn cross() -> Polygon {
    part(
        &[
            (0.0, 3.0),
            (3.0, 3.0),
            (3.0, 0.0),
            (4.0, 0.0),
            (4.0, 3.0),
            (8.0, 3.0),
            (8.0, 4.0),
            (4.0, 4.0),
            (4.0, 7.0),
            (3.0, 7.0),
            (3.0, 4.0),
            (0.0, 4.0),
        ],
        &[],
    )
}

/// The columns along `part`'s centre lines.
fn found(part: &Polygon) -> Vec<Column> {
    columns(part, 1.8, &[], LIMITS.0, &mut Budget::DEFAULT.meter()).unwrap().unwrap().unwrap()
}

/// The columns along `part`'s centre lines, twice over: the rule reads a part only when it has 2 or more,
/// and an L's centre line runs round its corner in one.
fn twice(part: &Polygon) -> Vec<Column> {
    let found = found(part);
    found.iter().chain(&found).cloned().collect()
}

/// `part`'s columns sewn as strokes, if it reads as strokes.
fn sewn(part: &Polygon) -> Option<Vec<Column>> {
    strokes(part, &twice(part), REACH, LIMITS, &mut Budget::DEFAULT.meter()).unwrap()
}

/// `part`'s strokes.
fn bands_of(part: &Polygon) -> Vec<Band> {
    let meter = &mut Budget::DEFAULT.meter();
    let mut runs = Vec::new();
    for ring in std::iter::once(&part.outline).chain(&part.holes) {
        runs.extend(sides(ring, meter).unwrap());
    }
    bands(part, &runs, LIMITS, meter).unwrap().unwrap()
}

/// The angle in degrees between a column's stitch from `a` to `b` and the square to `band`.
fn lean(band: &Band, [a, b]: [Point; 2]) -> f64 {
    let along = (band.along(b) - band.along(a)).abs();
    let across = (band.across(b) - band.across(a)).abs();
    math::to_degrees(math::atan2(along, across))
}

/// Whether every rung of `column` lies square to one of `bands` within `most` degrees.
fn square_rungs(column: &Column, bands: &[Band], most: f64) -> bool {
    column.rungs.iter().all(|&rung| bands.iter().any(|band| lean(band, rung) <= most))
}

#[test]
fn req_auto_087_an_m_sews_its_stems_and_middle_strokes_each_as_a_column_square_to_itself() {
    let part = m(10.0);
    let bands = bands_of(&part);
    assert_eq!(bands.len(), 4, "2 stems and 2 middle strokes: {bands:?}");
    let columns = sewn(&part).unwrap();
    assert_eq!(columns.len(), 4);
    for column in &columns {
        assert!(!column.closed);
        assert!(square_rungs(column, &bands, 1.0), "{column:?}");
    }
    // The middle strokes stop against the stems and end under them; at the V they are halved, under none.
    let under: Vec<[Option<usize>; 2]> = columns.iter().map(|column| column.under).collect();
    assert_eq!(under.iter().flatten().flatten().count(), 2, "{under:?}");
}

#[test]
fn req_auto_088_an_a_halves_its_apex_and_its_crossbar_stops_against_its_legs() {
    let part = a(10.0);
    let bands = bands_of(&part);
    assert_eq!(bands.len(), 3, "2 legs and a crossbar: {bands:?}");
    let columns = sewn(&part).unwrap();
    assert_eq!(columns.len(), 3);
    // The crossbar ends under each leg, and the legs, halved at the apex, under nothing.
    let bar = columns.iter().find(|column| column.under.iter().all(Option::is_some)).unwrap();
    assert!(bar.under[0] != bar.under[1]);
    assert_eq!(columns.iter().filter(|column| column.under == [None, None]).count(), 2);
}

#[test]
fn req_auto_088_a_ts_stem_stops_under_its_bar_which_runs_on() {
    let part = t();
    let columns = sewn(&part).unwrap();
    assert_eq!(columns.len(), 2);
    let bands = bands_of(&part);
    let bar = bands.iter().position(|band| band.u.1.abs() < 1e-9).unwrap();
    let stem = 1 - bar;
    // The stem ends inside the bar, past its lower side by the share of the pull compensation its own
    // stitches don't reach across there: all of it, the 2 square.
    let line = &columns[stem].line.points;
    let top = line.iter().map(|q| q.y()).fold(f64::NEG_INFINITY, f64::max);
    assert!((top - 5.0 - REACH).abs() < 1e-6, "{top}");
    assert_eq!(columns[stem].under.iter().flatten().count(), 1);
    // It ends under the bar at the end of its line nearer the bar, its top.
    let rising = line.first().unwrap().y() < line.last().unwrap().y();
    assert_eq!(columns[stem].under, if rising { [None, Some(bar)] } else { [Some(bar), None] });
    assert_eq!(columns[bar].under, [None, None]);
    let length = |column: &Column| column.line.length();
    assert!((length(&columns[bar]) - 6.0).abs() < 0.6, "{}", length(&columns[bar]));
}

#[test]
fn req_auto_088_an_ls_longer_stroke_runs_through_its_corner() {
    let part = l(0.0);
    let bands = bands_of(&part);
    assert_eq!(bands.len(), 2);
    let columns = sewn(&part).unwrap();
    assert_eq!(columns.len(), 2);
    // The stem, 6 mm, runs on to the corner, and the foot, 5 mm, stops against it.
    let stem = bands.iter().position(|band| band.u.0.abs() < 1e-9).unwrap();
    assert_eq!(columns[stem].under, [None, None]);
    assert_eq!(columns[1 - stem].under.iter().flatten().count(), 1);
    let low = columns[stem].line.points.iter().map(|q| q.y()).fold(f64::INFINITY, f64::min);
    assert!(low < 0.5, "{low}");
}

#[test]
fn req_auto_088_of_2_strokes_that_cross_the_longer_runs_through_and_the_other_sews_in_2() {
    let part = cross();
    let columns = sewn(&part).unwrap();
    assert_eq!(columns.len(), 3);
    let through: Vec<&Column> = columns.iter().filter(|column| column.under == [None, None]).collect();
    assert_eq!(through.len(), 1);
    assert!((through[0].line.length() - 8.0).abs() < 0.6, "{}", through[0].line.length());
    // The 2 pieces of the other each end under it.
    assert_eq!(columns.iter().filter(|column| column.under.iter().flatten().count() == 1).count(), 2);
}

#[test]
fn a_part_that_does_not_read_as_strokes_keeps_its_centre_lines_columns() {
    let meter = &mut Budget::DEFAULT.meter();
    // A ring among them: a frame round a hole.
    let frame = part(&[(0.0, 0.0), (6.0, 0.0), (6.0, 6.0), (0.0, 6.0)], &[&[(1.0, 1.0), (5.0, 1.0), (5.0, 5.0), (1.0, 5.0)]]);
    let ringed = found(&frame);
    assert!(ringed.iter().any(|column| column.closed));
    assert_eq!(strokes(&frame, &ringed, REACH, LIMITS, meter).unwrap(), None);
    // Fewer than 2 columns.
    let corner = l(0.0);
    assert_eq!(strokes(&corner, &found(&corner), REACH, LIMITS, meter).unwrap(), None);
    // One stroke, though 2 columns.
    let bar = part(&[(0.0, 0.0), (8.0, 0.0), (8.0, 1.0), (0.0, 1.0)], &[]);
    assert_eq!(strokes(&bar, &twice(&bar), REACH, LIMITS, meter).unwrap(), None);
    // No stroke: a part wider than the widest stroke.
    let t = t();
    assert_eq!(strokes(&t, &found(&t), REACH, (0.5, 0.9), meter).unwrap(), None);
    // Strokes that leave more than a hundredth of the part uncovered: an L with its inner corner filled in.
    assert_eq!(sewn(&l(0.5)), None);
}

#[test]
fn a_corner_filled_in_by_less_than_a_hundredth_of_the_part_still_sews_as_strokes() {
    // 0.045 mm² of 10.045: 0.45 %. At 0.5 mm, 0.125 of 10.125 is 1.2 %.
    assert_eq!(sewn(&l(0.3)).map(|columns| columns.len()), Some(2));
}

#[test]
fn every_case_runs_out_of_work_at_every_charge_on_its_way() {
    // The T reaches every charge: a cross or an L only takes other branches between them.
    for part in [t()] {
        let found = twice(&part);
        let meter = &mut Budget::DEFAULT.meter();
        assert!(strokes(&part, &found, REACH, LIMITS, meter).unwrap().is_some());
        let needed = Budget::DEFAULT.max_work - meter.work_left();
        for work in 0..needed {
            let short = &mut Budget { max_stitches: 0, max_work: work }.meter();
            assert_eq!(strokes(&part, &found, REACH, LIMITS, short), Err(Exhausted::Work), "{work} of {needed} units");
        }
    }
}

#[test]
fn a_ring_is_cut_into_its_straight_runs_from_its_sharpest_corner() {
    let meter = &mut Budget::DEFAULT.meter();
    // A square with a point 0.04 mm off its bottom side, within RUN, and one 0.06 mm off its top, past it.
    // Its sharpest corner is the top's right, where the top's first piece turns 91.7° from the right side.
    let ring: Vec<Point> =
        [(0.0, 0.0), (2.0, 0.04), (4.0, 0.0), (4.0, 4.0), (2.0, 3.94), (0.0, 4.0), (0.0, 0.0)].iter().map(|&(x, y)| p(x, y)).collect();
    let found = sides(&ring, meter).unwrap();
    assert_eq!(found.len(), 5, "{found:?}");
    assert_eq!(found[0].a, p(4.0, 4.0));
    assert!(found.contains(&side((0.0, 0.0), (4.0, 0.0))));
    assert!(found.contains(&side((4.0, 4.0), (2.0, 3.94))));
    // Too few points for a ring.
    assert_eq!(sides(&[p(0.0, 0.0), p(1.0, 0.0), p(0.0, 0.0)], meter).unwrap(), Vec::new());
    // Points at the boundary.
    let edge = [p(0.0, 0.0), p(1.0, RUN), p(2.0, 0.0)];
    assert!(straight(&edge, 0, 2, meter).unwrap());
    let past = [p(0.0, 0.0), p(1.0, RUN + 1e-9), p(2.0, 0.0)];
    assert!(!straight(&past, 0, 2, meter).unwrap());
}

#[test]
fn two_sides_are_a_strokes_within_each_of_its_bounds() {
    let one = side((0.0, 0.0), (5.5, 0.0));
    let at = |degrees: f64| {
        let (sin, cos) = math::sin_cos(math::to_radians(degrees));
        Side { a: p(5.5, 5.0), b: p(5.5 - 5.5 * cos, 5.0 + 5.5 * sin) }
    };
    let (band, overlap) = paired(one, at(0.0), LIMITS).unwrap();
    assert_eq!((band.lo, band.hi, band.from, band.to), (0.0, 5.0, 0.0, 5.5));
    assert_eq!(overlap, (0.0, 5.5));
    // Opposite within 5°, and as far apart at both ends within a tenth.
    assert!(paired(one, at(4.9), LIMITS).is_some());
    assert!(paired(one, at(5.1), LIMITS).is_none());
    assert!(paired(one, side((0.0, 5.0), (5.5, 5.0)), LIMITS).is_none(), "the same way");
    assert!(paired(one, side((5.5, 5.0), (0.0, 5.55)), LIMITS).is_none(), "a tenth farther apart at one end");
    assert!(paired(one, side((5.5, 5.0), (0.0, 5.45)), LIMITS).is_some());
    // As wide as the narrowest and widest.
    let apart = |width: f64| side((5.5, width), (0.0, width));
    assert!(paired(one, apart(5.0), (5.0, 5.0)).is_some());
    assert!(paired(one, apart(5.0), (5.1, 7.0)).is_none());
    assert!(paired(one, apart(5.0), (0.5, 4.9)).is_none());
    // Overlapping by half the width, the longer as long as it and the shorter half as long.
    assert!(paired(side((0.0, 0.0), (1.0, 0.0)), side((1.5, 1.0), (0.5, 1.0)), LIMITS).is_some());
    assert!(paired(side((0.0, 0.0), (1.0, 0.0)), side((1.6, 1.0), (0.6, 1.0)), LIMITS).is_none());
    assert!(paired(side((0.0, 0.0), (1.0, 0.0)), side((0.5, 1.0), (0.0, 1.0)), LIMITS).is_some());
    assert!(paired(side((0.0, 0.0), (0.9, 0.0)), side((0.6, 1.0), (0.0, 1.0)), LIMITS).is_none());
    assert!(paired(side((0.0, 0.0), (1.0, 0.0)), side((0.6, 1.0), (0.2, 1.0)), LIMITS).is_none());
    // Sides with no length.
    assert!(paired(side((0.0, 0.0), (0.0, 0.0)), at(0.0), LIMITS).is_none());
    assert!(paired(one, side((1.0, 1.0), (1.0, 1.0)), LIMITS).is_none());
    // The band runs as far as the 2 reach: from the other's start, to the first's end.
    let (band, overlap) = paired(side((1.0, 0.0), (5.0, 0.0)), side((4.0, -1.0), (0.0, -1.0)), LIMITS).unwrap();
    assert_eq!((band.lo, band.hi, band.from, band.to), (-1.0, 0.0, -1.0, 4.0));
    assert_eq!(overlap, (0.0, 3.0));
}

#[test]
fn a_band_is_held_by_the_part_but_for_a_fiftieth_of_it() {
    let meter = &mut Budget::DEFAULT.meter();
    let square = part(&[(0.0, 0.0), (10.0, 0.0), (10.0, 1.0), (0.0, 1.0)], &[]);
    let band = Band { o: p(0.0, 0.0), u: (1.0, 0.0), n: (0.0, 1.0), lo: 0.0, hi: 1.0, from: 0.0, to: 10.0 };
    assert_eq!(held(&square, &band, (0.0, 10.0), meter).unwrap(), Some(true));
    assert_eq!(held(&square, &band, (0.0, 10.19), meter).unwrap(), Some(true));
    assert_eq!(held(&square, &band, (0.0, 10.25), meter).unwrap(), Some(false));
    let wide = Band { hi: 1.1, ..band };
    assert_eq!(held(&square, &wide, (0.0, 10.0), meter).unwrap(), Some(false));
}

#[test]
fn bands_on_the_same_2_lines_are_one_stroke() {
    let kept = Band { o: p(0.0, 0.0), u: (1.0, 0.0), n: (0.0, 1.0), lo: 0.0, hi: 1.0, from: 0.0, to: 2.0 };
    let later = Band { from: 4.0, to: 6.0, ..kept };
    assert!(same(&kept, &later));
    let back = Band { o: p(6.0, 1.0), u: (-1.0, 0.0), n: (0.0, -1.0), lo: 0.0, hi: 1.0, from: 0.0, to: 2.0 };
    assert!(same(&kept, &back));
    assert!(same(&kept, &Band { lo: 0.09, hi: 1.09, ..later }));
    assert!(!same(&kept, &Band { lo: 0.11, hi: 1.0, ..later }));
    assert!(!same(&kept, &Band { lo: 0.0, hi: 0.89, ..later }));
    let (sin, cos) = math::sin_cos(math::to_radians(2.5));
    assert!(!same(&kept, &Band { u: (cos, sin), n: (-sin, cos), ..later }));
    let (sin, cos) = math::sin_cos(math::to_radians(1.5));
    assert!(same(&kept, &Band { o: p(0.0, 0.0), u: (cos, sin), n: (-sin, cos), from: 0.0, to: 2.0, ..kept }));
    // Turned 3° about its middle, a band lies on the lines but runs another way.
    let (sin, cos) = math::sin_cos(math::to_radians(3.0));
    assert!(!same(&kept, &Band { o: p(1.0, 0.0), u: (cos, sin), n: (-sin, cos), from: -1.0, to: 1.0, ..kept }));
    let mut merged = kept;
    merge(&mut merged, &later);
    assert_eq!((merged.from, merged.to), (0.0, 6.0));
    merge(&mut merged, &Band { o: p(-3.0, 0.0), ..kept });
    assert_eq!((merged.from, merged.to), (-3.0, 6.0));
}

#[test]
fn strokes_that_do_not_meet_have_no_join() {
    let meter = &mut Budget::DEFAULT.meter();
    let bar = part(&[(0.0, 0.0), (10.0, 0.0), (10.0, 1.0), (0.0, 1.0)], &[]);
    let one = Band { o: p(0.0, 0.0), u: (1.0, 0.0), n: (0.0, 1.0), lo: 0.0, hi: 1.0, from: 0.0, to: 10.0 };
    let far = Band { o: p(0.0, 5.0), ..one };
    assert_eq!(joins(&bar, &[one, far], REACH, meter).unwrap(), None);
    assert_eq!(meeting(&bar, &one, &far, meter).unwrap(), Some(None));
    // Bands that meet in less than a sliver: 0.009 mm², under 0.02 w² = 0.02.
    let corner = Band { o: p(10.99, -5.0), u: (0.0, 1.0), n: (-1.0, 0.0), lo: 0.0, hi: 1.0, from: 0.0, to: 4.9 };
    assert!(meeting(&bar, &one, &corner, meter).unwrap().unwrap().is_none());
    // Bands 2 and 3 mm wide meet in a sliver under 0.02 × 2 × 3 = 0.12 mm²: 0.1 mm².
    let wide = part(&[(0.0, 0.0), (10.0, 0.0), (10.0, 2.0), (0.0, 2.0)], &[]);
    let two = Band { hi: 2.0, ..one };
    let three = Band { o: p(12.95, -5.0), u: (0.0, 1.0), n: (-1.0, 0.0), lo: 0.0, hi: 3.0, from: 0.0, to: 4.9 };
    assert!(meeting(&wide, &two, &three, meter).unwrap().unwrap().is_none());
    let more = Band { o: p(13.0, -5.0), ..three };
    assert!(meeting(&wide, &two, &more, meter).unwrap().unwrap().is_none());
    let past = Band { o: p(12.9, -5.0), ..three };
    assert!(meeting(&wide, &two, &past, meter).unwrap().unwrap().is_some());
}

#[test]
fn of_2_strokes_that_end_alike_the_squarer_runs_through() {
    // The M's stem against its middle stroke: the stem's top is square, the stroke's is on the slant.
    let part = m(10.0);
    let bands = bands_of(&part);
    let meter = &mut Budget::DEFAULT.meter();
    let stem = bands.iter().position(|band| band.u.0.abs() < 1e-9).unwrap();
    let stroke =
        bands.iter().position(|band| band.u.0.abs() > 1e-3 && meeting(&part, &bands[stem], band, meter).unwrap().unwrap().is_some()).unwrap();
    let met = meeting(&part, &bands[stem], &bands[stroke], meter).unwrap().unwrap().unwrap();
    let straight = square(&part, &bands[stem], &met).unwrap();
    let slanted = square(&part, &bands[stroke], &met).unwrap();
    assert!(straight + TIE < slanted, "{straight} {slanted}");
    let cuts = join(&part, (stem, &bands[stem]), (stroke, &bands[stroke]), &met, REACH).unwrap();
    assert_eq!(cuts.len(), 1);
    assert_eq!(cuts[0].0, stroke);
    assert_eq!(cuts[0].2.map(|(_, under)| under), Some(stem));
    let back = join(&part, (stroke, &bands[stroke]), (stem, &bands[stem]), &met, REACH).unwrap();
    assert_eq!(back[0].0, stroke);
}

#[test]
fn a_stroke_stops_its_reach_past_the_other_along_its_own_line() {
    let running = Band { o: p(0.0, 5.0), u: (1.0, 0.0), n: (0.0, 1.0), lo: 0.0, hi: 1.0, from: 0.0, to: 6.0 };
    let below = Band { o: p(2.5, 0.0), u: (0.0, 1.0), n: (-1.0, 0.0), lo: -1.0, hi: 0.0, from: 0.0, to: 5.0 };
    // Square to it, its own stitches reach none of it across the other's side: the cut lies its whole reach
    // into the other.
    assert_eq!(stops(&below, &running, REACH), Some(Cut::Keep { p: p(0.0, 5.0 + REACH), m: (0.0, -1.0) }));
    let above = Band { o: p(2.5, 6.0), lo: -1.0, hi: 0.0, ..below };
    assert_eq!(stops(&above, &running, REACH), Some(Cut::Keep { p: p(0.0, 6.0 - REACH), m: (0.0, 1.0) }));
    // At 60°, its stitches reach half of it: the cut lies 0.2 mm into the other.
    let (sin, cos) = math::sin_cos(math::to_radians(60.0));
    let slanted = Band { u: (cos, sin), n: (-sin, cos), ..below };
    let Some(Cut::Keep { p: at, m }) = stops(&slanted, &running, REACH) else { panic!() };
    assert!((at.y() - 5.2).abs() < 1e-9 && m == (0.0, -1.0), "{at:?}");
    // A stroke whose middle lies inside the other's band doesn't stop against it.
    let inside = Band { o: p(2.5, 5.2), u: (0.0, 1.0), n: (-1.0, 0.0), lo: -1.0, hi: 0.0, from: 0.0, to: 0.5 };
    assert_eq!(stops(&inside, &running, REACH), None);
}

#[test]
fn a_halving_line_cuts_2_strokes_at_the_point_their_lines_cross() {
    let (sin, cos) = math::sin_cos(math::to_radians(60.0));
    // A V: 2 strokes from (0, 0) up at 60° each way.
    let left = Band { o: p(0.0, 0.0), u: (-cos, sin), n: (-sin, -cos), lo: -0.5, hi: 0.5, from: 0.0, to: 5.0 };
    let right = Band { o: p(0.0, 0.0), u: (cos, sin), n: (-sin, cos), lo: -0.5, hi: 0.5, from: 0.0, to: 5.0 };
    assert_eq!(crossed(&left, &right).map(|q| (q.x().abs() < 1e-12, q.y().abs() < 1e-12)), Some((true, true)));
    let cuts = halved((0, &left), (1, &right), REACH).unwrap();
    assert_eq!(cuts.len(), 2);
    // The halving line is upright, each stroke reaching max(0, 0.2 - 0.4 cos 30°) = 0 past it.
    for (stroke, cut, under) in cuts {
        let Cut::Keep { p: at, m } = cut else { panic!() };
        assert_eq!(under, None);
        assert!(at.x().abs() < 1e-12 && at.y().abs() < 1e-12, "{at:?}");
        assert!((m.0 - if stroke == 0 { -1.0 } else { 1.0 }).abs() < 1e-12 && m.1.abs() < 1e-12, "{m:?}");
    }
    // Parallel lines don't cross.
    assert_eq!(crossed(&left, &Band { o: p(3.0, 0.0), ..left }), None);
    // A ray finds the nearest edge it crosses, none when it leaves no edge.
    let bar = part(&[(0.0, 0.0), (10.0, 0.0), (10.0, 1.0), (0.0, 1.0)], &[]);
    assert_eq!(leaves(&bar, p(2.0, 0.5), (1.0, 0.0)), Some(8.0));
    assert_eq!(leaves(&bar, p(20.0, 0.5), (1.0, 0.0)), None);
}

#[test]
fn a_pieces_column_has_its_rails_along_the_stroke_and_its_rungs_square_to_it() {
    let meter = &mut Budget::DEFAULT.meter();
    let bar = part(&[(0.0, 0.0), (6.0, 0.0), (6.0, 1.0), (0.0, 1.0)], &[]);
    let band = Band { o: p(0.0, 0.0), u: (1.0, 0.0), n: (0.0, 1.0), lo: 0.0, hi: 1.0, from: 0.0, to: 6.0 };
    let (rails, rungs, line) = column(&bar, &bar, &band, meter).unwrap().unwrap();
    assert_eq!(rails[0].len(), 2);
    assert_eq!(rails[1].len(), 2);
    assert!(rungs.is_empty());
    // A point every half width: 12 pieces.
    assert_eq!(line.points.len(), 13);
    assert_eq!(line.points[0], p(0.0, 0.5));
    assert_eq!(line.points[12], p(6.0, 0.5));
    // A flat end on the slant, within 20°: a rung past its far corner, twice as far as the corners lie apart.
    let slant = part(&[(0.0, 0.0), (6.0, 0.0), (6.0, 1.0), (0.3, 1.0)], &[]);
    let (_, rungs, _) = column(&slant, &slant, &band, meter).unwrap().unwrap();
    assert_eq!(rungs.len(), 1);
    assert!(rungs[0].iter().all(|q| (q.x() - 0.9).abs() < 1e-9), "{rungs:?}");
    // On the slant past 20°, the end is a point.
    let point = part(&[(0.0, 0.0), (6.0, 0.0), (6.0, 1.0), (0.4, 1.0)], &[]);
    let (rails, _, _) = column(&point, &point, &band, meter).unwrap().unwrap();
    assert_eq!(rails.iter().map(|rail| rail[0]).collect::<Vec<_>>(), vec![p(0.0, 0.0), p(0.0, 0.0)]);
    // A rail that runs back along the stroke.
    let hook = part(&[(0.0, 0.0), (6.0, 0.0), (6.0, 1.0), (3.0, 1.0), (2.0, 3.0), (1.0, 1.0), (0.0, 1.0)], &[]);
    let back = part(&[(0.0, 0.0), (6.0, 0.0), (6.0, 1.0), (2.0, 1.0), (2.9, 2.0), (1.0, 2.0), (0.0, 1.0)], &[]);
    assert!(column(&hook, &hook, &band, meter).unwrap().is_some());
    assert_eq!(column(&back, &back, &band, meter).unwrap(), None);
}

#[test]
fn a_rail_runs_on_but_for_a_twentieth_of_a_millimetre_back() {
    let band = Band { o: p(0.0, 0.0), u: (1.0, 0.0), n: (0.0, 1.0), lo: 0.0, hi: 1.0, from: 0.0, to: 6.0 };
    assert!(onward(&[p(0.0, 0.0), p(2.0, 0.0), p(2.0 - BACK, 1.0), p(3.0, 0.0)], &band));
    assert!(!onward(&[p(0.0, 0.0), p(2.0, 0.0), p(2.0 - BACK - 1e-9, 1.0), p(3.0, 0.0)], &band));
}

#[test]
fn a_pieces_points_snap_to_the_parts_corners_and_lose_their_steps() {
    let meter = &mut Budget::DEFAULT.meter();
    let bar = part(&[(0.0, 0.0), (6.0, 0.0), (6.0, 1.0), (0.0, 1.0)], &[]);
    let near = CLOSE * 0.9;
    let ring = [p(near, 0.0), p(3.0, 0.0), p(3.0 + near, 0.0), p(6.0, 0.0), p(6.0, 1.0), p(0.0, 1.0), p(0.0, near * 0.5)];
    assert_eq!(cleaned(&ring, &bar, meter).unwrap(), vec![p(0.0, 0.0), p(3.0, 0.0), p(6.0, 0.0), p(6.0, 1.0), p(0.0, 1.0)]);
    let far = [p(CLOSE, 0.0), p(6.0, 0.0), p(6.0, 1.0)];
    assert_eq!(cleaned(&far, &bar, meter).unwrap(), far.to_vec());
}

#[test]
fn the_helpers_of_a_column_find_its_ends_and_walk_its_ring() {
    assert_eq!(extreme(&[2.0, 1.0, 1.0, 3.0, 3.0], false), Some(1));
    assert_eq!(extreme(&[2.0, 1.0, 1.0, 3.0, 3.0], true), Some(3));
    assert_eq!(extreme(&[], true), None);
    assert_eq!(run_round(&[0.0, 5.0, 5.0, 0.0, 0.0], 0, |t| t < 1.0), (3, 0));
    assert_eq!(run_round(&[0.0, 0.0, 5.0, 0.0], 1, |t| t < 1.0), (3, 1));
    assert_eq!(run_round(&[0.0, 0.0, 0.0], 1, |t| t < 1.0), (2, 1));
    let ring = [p(0.0, 0.0), p(1.0, 0.0), p(1.0, 1.0), p(0.0, 1.0)];
    assert_eq!(walk(&ring, 1, 3, 1), vec![p(1.0, 0.0), p(1.0, 1.0), p(0.0, 1.0)]);
    assert_eq!(walk(&ring, 1, 3, 3), vec![p(1.0, 0.0), p(0.0, 0.0), p(0.0, 1.0)]);
    assert_eq!(walk(&ring, 1, 9, 1), Vec::new());
    let band = Band { o: p(0.0, 0.0), u: (1.0, 0.0), n: (0.0, 1.0), lo: 0.0, hi: 1.0, from: 0.0, to: 6.0 };
    let rail = [p(0.0, 0.0), p(2.0, 1.0), p(2.0, 2.0), p(4.0, 0.0)];
    assert_eq!(meet(&rail, &band, 1.0), Some(p(1.0, 0.5)));
    assert_eq!(meet(&rail, &band, 3.0), Some(p(3.0, 1.0)));
    assert_eq!(meet(&rail, &band, 5.0), None);
    assert_eq!(spanned(&ring, &band, 0.5), Some((0.0, 1.0)));
    assert_eq!(spanned(&ring, &band, 2.0), None);
}

#[test]
fn rungs_lie_square_at_every_inner_corner_and_none_crowd() {
    let meter = &mut Budget::DEFAULT.meter();
    let band = Band { o: p(0.0, 0.0), u: (1.0, 0.0), n: (0.0, 1.0), lo: 0.0, hi: 1.0, from: 0.0, to: 6.0 };
    let one = [p(0.0, 0.0), p(2.0, 0.0), p(2.0 + RUNGS_APART / 2.0, 0.0), p(6.0, 0.0)];
    let other = [p(0.0, 1.0), p(4.0, 1.0), p(6.0, 1.0)];
    assert_eq!(rungs(&one, &other, &band, meter).unwrap(), vec![[p(2.0, 0.0), p(2.0, 1.0)], [p(4.0, 0.0), p(4.0, 1.0)]]);
    // A rung within RUNGS_APART of a rail's end is left out.
    let end = [p(0.0, 0.0), p(RUNGS_APART / 2.0, 0.0), p(6.0, 0.0)];
    let kept = rungs(&end, &other[..], &band, meter).unwrap();
    assert_eq!(kept.len(), 1);
    assert!(kept[0][0].distance(p(4.0, 0.0)) < 1e-9 && kept[0][1] == p(4.0, 1.0), "{kept:?}");
}

#[test]
fn the_shapes_helpers_clip_narrow_and_close() {
    let square = [p(0.0, 0.0), p(2.0, 0.0), p(2.0, 2.0), p(0.0, 2.0)];
    assert_eq!(clip(&square, p(1.0, 0.0), (1.0, 0.0)), vec![p(1.0, 0.0), p(2.0, 0.0), p(2.0, 2.0), p(1.0, 2.0)]);
    assert_eq!(clip(&square, p(3.0, 0.0), (1.0, 0.0)), Vec::new());
    let band = Band { o: p(0.0, 0.0), u: (1.0, 0.0), n: (0.0, 1.0), lo: 0.0, hi: 1.0, from: 0.0, to: 6.0 };
    assert_eq!(narrowed(&band, 0.5), None);
    assert_eq!(ops::area_of(&[narrowed(&band, 0.25).unwrap()]), 0.5 * 8.0);
    let ring = polygon(vec![p(0.0, 0.0), p(0.0, 1.0), p(1.0, 1.0)]);
    assert!(geom::signed_area(&ring.outline) < 0.0);
    assert_eq!(ring.outline.len(), 4);
    assert_eq!(polygon(Vec::new()).outline, Vec::new());
    assert_eq!(unit_of((0.0, 0.0)), None);
    assert_eq!(unit_of((f64::INFINITY, 0.0)), None);
    assert_eq!(unit_of((3.0, 4.0)), Some((0.6, 0.8)));
    assert_eq!(ordered(2.0, 1.0), (1.0, 2.0));
    assert_eq!(sine((1.0, 0.0), (0.0, -1.0)), 1.0);
    assert_eq!(cosine((1.0, 0.0), (-1.0, 0.0)), 1.0);
    assert_eq!(band.width(), 1.0);
    assert_eq!(band.middle(), Some(p(3.0, 0.5)));
    assert_eq!(band.reach().map(|points| points[0]), Some(p(-1.0, -EDGE)));
}

#[test]
fn a_stroke_wider_than_its_reach_over_tan_20_degrees_stops_at_the_others_side() {
    // A T 3 mm wide: its stem's piece reaches 0.4 mm, the pull compensation, past the bar's side, less than
    // its flat end's 20° allows along it (3 tan 20° = 1.09 mm), so the end takes in that stub and the rails
    // end on the bar's side. The bar's stitches, which run along the stem there, reach 0.4 mm over the seam
    // with their own pull compensation. The 1 mm T's stem, narrower than 0.4 / tan 20° = 1.1 mm, reaches in.
    let wide = part(&[(0.0, 0.0), (20.0, 0.0), (20.0, 3.0), (11.5, 3.0), (11.5, 15.0), (8.5, 15.0), (8.5, 3.0), (0.0, 3.0)], &[]);
    let low = |part: &Polygon| {
        let columns = sewn(part).unwrap();
        let stem = columns.iter().find(|column| column.under.iter().any(Option::is_some)).unwrap();
        stem.rails.iter().flatten().map(|q| q.y()).fold(f64::INFINITY, f64::min)
    };
    assert!((low(&wide) - 3.0).abs() < 1e-6, "{}", low(&wide));
    let high = |part: &Polygon| {
        let columns = sewn(part).unwrap();
        let stem = columns.iter().find(|column| column.under.iter().any(Option::is_some)).unwrap();
        stem.rails.iter().flatten().map(|q| q.y()).fold(f64::NEG_INFINITY, f64::max)
    };
    assert!((high(&t()) - 5.0 - REACH).abs() < 1e-6, "{}", high(&t()));
}

#[test]
fn a_stroke_runs_through_when_the_part_goes_on_past_the_join_by_half_its_width() {
    // A stem 1 mm wide whose left side runs on, and an arm 6 mm long off its right side, 0.7 mm above the
    // stem's foot: the stem goes on past the join by 0.7 mm, more than half its width, and runs through.
    // Had it not, the 2 would end alike there, and the arm, the longer, would run through.
    // Drawn the other way up too, so that the stub lies at the stem's either end.
    let ring = [(3.0, 2.3), (4.0, 2.3), (4.0, 3.0), (10.0, 3.0), (10.0, 4.0), (4.0, 4.0), (4.0, 7.0), (3.0, 7.0)];
    let flipped: Vec<(f64, f64)> = ring.iter().map(|&(x, y)| (x, 9.3 - y)).collect();
    for part in [part(&ring, &[]), part(&flipped, &[])] {
        let bands = bands_of(&part);
        assert_eq!(bands.len(), 2, "{bands:?}");
        let stem = bands.iter().position(|band| band.u.0.abs() < 1e-9).unwrap();
        let columns = sewn(&part).unwrap();
        assert_eq!(columns[stem].under, [None, None]);
        assert!(columns[1 - stem].under.contains(&Some(stem)), "{:?}", columns[1 - stem].under);
    }
}

#[test]
fn of_2_strokes_that_cross_either_way_round_the_longer_runs_through() {
    let part = cross();
    let bands = bands_of(&part);
    let meter = &mut Budget::DEFAULT.meter();
    let met = meeting(&part, &bands[0], &bands[1], meter).unwrap().unwrap().unwrap();
    let long = usize::from(bands[1].to - bands[1].from > bands[0].to - bands[0].from);
    for (a, b) in [(0, 1), (1, 0)] {
        let cuts = join(&part, (a, &bands[a]), (b, &bands[b]), &met, REACH).unwrap();
        assert_eq!(cuts.len(), 1);
        assert_eq!(cuts[0].0, 1 - long);
        assert!(matches!(cuts[0].1, Cut::Crossed { of, .. } if of == long), "{cuts:?}");
    }
}

#[test]
fn a_stroke_cut_away_whole_or_left_without_its_middle_keeps_the_centre_lines() {
    let meter = &mut Budget::DEFAULT.meter();
    let bar = part(&[(0.0, 0.0), (10.0, 0.0), (10.0, 1.0), (0.0, 1.0)], &[]);
    let band = Band { o: p(0.0, 0.0), u: (1.0, 0.0), n: (0.0, 1.0), lo: 0.0, hi: 1.0, from: 0.0, to: 10.0 };
    // A cut that keeps nothing of the band.
    let gone = Joined { cuts: vec![Cut::Keep { p: p(20.0, 0.0), m: (1.0, 0.0) }], under: Vec::new() };
    assert_eq!(pieces(&bar, &[band], &[gone], meter).unwrap(), None);
    // A stroke that crosses all of it.
    let crossed = Joined { cuts: vec![Cut::Crossed { of: 1, by: 0.0 }], under: Vec::new() };
    let over = Band { lo: -1.0, hi: 2.0, ..band };
    assert_eq!(pieces(&bar, &[band, over], &[crossed, Joined::default()], meter).unwrap(), None);
}

#[test]
fn bands_lie_on_the_same_lines_within_a_tenth_of_their_own_width() {
    let kept = Band { o: p(0.0, 0.0), u: (1.0, 0.0), n: (0.0, 1.0), lo: 0.0, hi: 2.0, from: 0.0, to: 2.0 };
    assert!(same(&kept, &Band { lo: 0.19, hi: 2.19, from: 4.0, to: 6.0, ..kept }));
    assert!(!same(&kept, &Band { lo: 0.21, hi: 2.0, from: 4.0, to: 6.0, ..kept }));
}

#[test]
fn a_rung_is_left_out_when_either_of_its_points_lies_near_a_rails_end() {
    let meter = &mut Budget::DEFAULT.meter();
    let band = Band { o: p(0.0, 0.0), u: (1.0, 0.0), n: (0.0, 1.0), lo: 0.0, hi: 1.0, from: 0.0, to: 6.0 };
    // The rung at 4 mm, from the other rail's corner, meets the first rail 0.02 mm from its end.
    let short = [p(0.0, 0.0), p(4.02, 0.0)];
    let long = [p(0.0, 1.0), p(4.0, 1.0), p(6.0, 1.0)];
    // The ends' rung, past the short rail's end on the slant, is the only one left.
    let at_4 = |found: Vec<[Point; 2]>| found.iter().any(|rung| rung.iter().any(|q| (q.x() - 4.0).abs() < 0.1));
    assert!(!at_4(rungs(&short, &long, &band, meter).unwrap()));
    assert!(!at_4(rungs(&long, &short, &band, meter).unwrap()));
    // Farther from the end, it is kept.
    let longer = [p(0.0, 0.0), p(4.2, 0.0)];
    assert!(at_4(rungs(&longer, &long, &band, meter).unwrap()));
}

#[test]
fn of_2_strokes_that_end_alike_at_a_blunt_corner_either_way_round_the_longer_runs_through() {
    let part = l(0.0);
    let bands = bands_of(&part);
    let meter = &mut Budget::DEFAULT.meter();
    let met = meeting(&part, &bands[0], &bands[1], meter).unwrap().unwrap().unwrap();
    let long = usize::from(bands[1].to - bands[1].from > bands[0].to - bands[0].from);
    for (a, b) in [(0, 1), (1, 0)] {
        let cuts = join(&part, (a, &bands[a]), (b, &bands[b]), &met, REACH).unwrap();
        assert_eq!(cuts.len(), 1);
        assert_eq!(cuts[0].0, 1 - long);
        assert_eq!(cuts[0].2.map(|(_, under)| under), Some(long));
    }
}

#[test]
fn a_piece_holds_the_bands_middle_line_where_it_lies_along_the_band() {
    let band = Band { o: p(0.0, 0.0), u: (1.0, 0.0), n: (0.0, 1.0), lo: 0.0, hi: 1.0, from: 0.0, to: 9.0 };
    // The 9 points lie at 0.5, 1.5 … 8.5 mm along, half way across.
    assert!(cores(&band, &part(&[(8.0, 0.0), (9.0, 0.0), (9.0, 1.0), (8.0, 1.0)], &[])));
    assert!(!cores(&band, &part(&[(-1.0, 0.0), (0.4, 0.0), (0.4, 1.0), (-1.0, 1.0)], &[])));
    assert!(!cores(&band, &part(&[(0.0, 0.6), (9.0, 0.6), (9.0, 1.0), (0.0, 1.0)], &[])));
}

#[test]
fn a_flat_end_square_to_its_stroke_gets_no_rung_past_it() {
    let meter = &mut Budget::DEFAULT.meter();
    let band = Band { o: p(0.0, 0.0), u: (1.0, 0.0), n: (0.0, 1.0), lo: 0.0, hi: 1.0, from: 0.0, to: 6.0 };
    // The corners lie 0.04 mm apart along the stroke, within 2 × RUNGS_APART: square.
    assert_eq!(rungs(&[p(3.0, 0.0), p(6.0, 0.0)], &[p(3.04, 1.0), p(6.0, 1.0)], &band, meter).unwrap(), Vec::<[Point; 2]>::new());
}

#[test]
fn rungs_crowd_when_their_points_on_the_first_rail_lie_close() {
    let meter = &mut Budget::DEFAULT.meter();
    let band = Band { o: p(0.0, 0.0), u: (1.0, 0.0), n: (0.0, 1.0), lo: 0.0, hi: 3.0, from: 0.0, to: 6.0 };
    // The other rail climbs steeply past 2 mm: its points of the rungs at 2 and 2.025 mm lie far apart,
    // their points on the first rail 0.025 mm apart.
    let one = [p(0.0, 0.0), p(2.0, 0.0), p(2.025, 0.0), p(6.0, 0.0)];
    let other = [p(0.0, 1.0), p(2.0, 1.0), p(2.03, 3.0), p(6.0, 3.0)];
    let found = rungs(&one, &other, &band, meter).unwrap();
    assert_eq!(found.iter().filter(|rung| rung[0].x() < 3.0).count(), 1, "{found:?}");
    // And with the rails the other way round, their points on the second rail lie close.
    let found = rungs(&other, &one, &band, meter).unwrap();
    assert_eq!(found.iter().filter(|rung| rung[1].x() < 3.0).count(), 1, "{found:?}");
}

#[test]
fn a_rail_meets_a_place_along_the_stroke_on_its_first_piece_that_reaches_it() {
    let band = Band { o: p(0.0, 0.0), u: (1.0, 0.0), n: (0.0, 1.0), lo: 0.0, hi: 1.0, from: 0.0, to: 6.0 };
    // The first piece ends at 1 mm along: that end is the point, though the third piece starts there too.
    assert_eq!(meet(&[p(2.0, 0.0), p(1.0, 0.0), p(1.0, 1.0), p(0.0, 2.0)], &band, 1.0), Some(p(1.0, 0.0)));
}

#[test]
fn of_2_strokes_that_end_alike_at_an_ls_corner_the_longer_runs_through_whichever_is_the_foot() {
    // A foot 7 mm long on a stem 6 mm long: the foot runs through.
    let part = part(&[(0.0, 0.0), (7.0, 0.0), (7.0, 1.0), (1.0, 1.0), (1.0, 6.0), (0.0, 6.0)], &[]);
    let bands = bands_of(&part);
    let meter = &mut Budget::DEFAULT.meter();
    let met = meeting(&part, &bands[0], &bands[1], meter).unwrap().unwrap().unwrap();
    let foot = bands.iter().position(|band| band.u.1.abs() < 1e-9).unwrap();
    for (a, b) in [(0, 1), (1, 0)] {
        let cuts = join(&part, (a, &bands[a]), (b, &bands[b]), &met, REACH).unwrap();
        assert_eq!(cuts[0].0, 1 - foot, "{bands:?}");
    }
}

#[test]
fn how_square_an_end_lies_is_measured_from_the_bands_middle() {
    // A bar from 2 to 8 mm whose right end runs back on the slant, 0.8 mm over its width less 0.2 mm.
    let bar = part(&[(2.0, 0.0), (8.0, 0.0), (7.0, 1.0), (2.0, 1.0)], &[]);
    // The band reaches back to -3 mm, as a stroke's band does past a join: its middle lies at 2.5 mm.
    let band = Band { o: p(0.0, 0.0), u: (1.0, 0.0), n: (0.0, 1.0), lo: 0.0, hi: 1.0, from: -3.0, to: 8.0 };
    let measured = square(&bar, &band, &[p(7.5, 0.5)]).unwrap();
    assert!((measured - 0.8).abs() < 1e-9, "{measured}");
}

#[test]
fn a_halving_line_keeps_each_strokes_middle_on_its_side_with_an_upright_stroke() {
    let (sin, cos) = math::sin_cos(math::to_radians(30.0));
    let upright = Band { o: p(0.0, 0.0), u: (0.0, 1.0), n: (-1.0, 0.0), lo: -0.5, hi: 0.5, from: 0.0, to: 5.0 };
    let leaning = Band { o: p(0.0, 0.0), u: (sin, cos), n: (-cos, sin), lo: -0.5, hi: 0.5, from: 0.0, to: 5.0 };
    // And a level stroke with one rising at 30° from it.
    let level = Band { u: (1.0, 0.0), n: (0.0, 1.0), ..upright };
    let rising = Band { u: (cos, sin), n: (-sin, cos), ..upright };
    for bands in [[upright, leaning], [level, rising]] {
        for (stroke, cut, _) in halved((0, &bands[0]), (1, &bands[1]), REACH).unwrap() {
            let Cut::Keep { p: at, m } = cut else { panic!() };
            let middle = bands[stroke].middle().unwrap();
            assert!((middle.x() - at.x()) * m.0 + (middle.y() - at.y()) * m.1 > 0.0, "{stroke}: {at:?} {m:?}");
        }
    }
}

#[test]
fn a_stroke_that_stops_ends_under_the_other_at_its_end_nearer_it_however_the_part_lies() {
    let ring = [(0.0, 5.0), (2.5, 5.0), (2.5, 0.0), (3.5, 0.0), (3.5, 5.0), (6.0, 5.0), (6.0, 6.0), (0.0, 6.0)];
    let turns: [Turn; 4] = [|(x, y)| (x, y), |(x, y)| (x, 6.0 - y), |(x, y)| (6.0 - x, y), |(x, y)| (y, x)];
    for turn in turns {
        let turned: Vec<(f64, f64)> = ring.iter().map(|&q| turn(q)).collect();
        let part = part(&turned, &[]);
        let bands = bands_of(&part);
        let columns = sewn(&part).unwrap();
        let stem = columns.iter().position(|column| column.under.iter().any(Option::is_some)).unwrap();
        let bar = &bands[1 - stem];
        let (line, under) = (&columns[stem].line.points, columns[stem].under);
        let end = if under[0].is_some() { line.first() } else { line.last() };
        let across = bar.across(*end.unwrap()) - (bar.lo + bar.hi) / 2.0;
        assert!(across.abs() <= bar.width(), "{turned:?}: {under:?} {line:?}");
    }
}

#[test]
fn each_piece_of_a_crossed_stroke_ends_under_the_other_at_its_end_beside_it() {
    // The cross turned every way, so that its strokes' bands are measured from either end.
    let turns: [TurnPoint; 4] = [|q| (q.x(), q.y()), |q| (q.x(), 7.0 - q.y()), |q| (8.0 - q.x(), q.y()), |q| (q.y(), q.x())];
    for turn in turns {
        let ring: Vec<(f64, f64)> = cross().outline.iter().map(|&q| turn(q)).collect();
        let part = part(&ring, &[]);
        let bands = bands_of(&part);
        let through = &bands[usize::from(bands[1].to - bands[1].from > bands[0].to - bands[0].from)];
        let columns = sewn(&part).unwrap();
        for column in columns.iter().filter(|column| column.under != [None, None]) {
            let line = &column.line.points;
            let end = if column.under[0].is_some() { line.first() } else { line.last() };
            let across = through.across(*end.unwrap()) - (through.lo + through.hi) / 2.0;
            assert!(across.abs() <= through.width(), "{:?}: {line:?}", column.under);
        }
    }
}

#[test]
fn a_column_ends_under_another_at_its_end_nearer_the_join_wherever_its_band_is_measured_from() {
    let meter = &mut Budget::DEFAULT.meter();
    let bar = part(&[(0.0, 0.0), (6.0, 0.0), (6.0, 1.0), (0.0, 1.0)], &[]);
    // Measured from 100 mm before it: the bar lies from 100 to 106 mm along, and the join at its start.
    let band = Band { o: p(-100.0, 0.0), u: (1.0, 0.0), n: (0.0, 1.0), lo: 0.0, hi: 1.0, from: 100.0, to: 106.0 };
    // A join half way between its ends is at its start.
    for (at, under) in [(100.0, [Some(0), None]), (106.0, [None, Some(0)]), (103.0, [Some(0), None])] {
        let joined = Joined { cuts: Vec::new(), under: vec![(at, 0)] };
        let columns = columns_of(&bar, &[band], &[joined], &[vec![bar.clone()]], meter).unwrap().unwrap();
        assert_eq!(columns[0].under, under);
    }
}

/// `band` turned `degrees` about the origin.
fn turned(band: Band, degrees: f64) -> Band {
    let (sin, cos) = math::sin_cos(math::to_radians(degrees));
    let turn = |(x, y): (f64, f64)| (x * cos - y * sin, x * sin + y * cos);
    let (x, y) = turn((band.o.x(), band.o.y()));
    Band { o: p(x, y), u: turn(band.u), n: turn(band.n), ..band }
}

#[test]
fn a_rings_runs_close_it_from_its_first_point_when_that_is_the_sharpest() {
    let meter = &mut Budget::DEFAULT.meter();
    // A square with a point half way along its bottom: its corners turn alike, so the first is the start.
    let ring: Vec<Point> = [(0.0, 0.0), (2.0, 0.0), (4.0, 0.0), (4.0, 4.0), (0.0, 4.0), (0.0, 0.0)].iter().map(|&(x, y)| p(x, y)).collect();
    let expected = vec![side((0.0, 0.0), (4.0, 0.0)), side((4.0, 0.0), (4.0, 4.0)), side((4.0, 4.0), (0.0, 4.0)), side((0.0, 4.0), (0.0, 0.0))];
    assert_eq!(sides(&ring, meter).unwrap(), expected);
    // A circle of 200 points, each turning 1.8°: its runs are arcs, and the last ends where the first starts.
    let circle: Vec<Point> = (0..=200)
        .map(|k| {
            let (sin, cos) = math::sin_cos(math::to_radians(f64::from(k % 200) * 1.8));
            p(5.0 * cos, 5.0 * sin)
        })
        .collect();
    let found = sides(&circle, meter).unwrap();
    assert!(found.len() > 2, "{found:?}");
    for (one, next) in found.iter().zip(found.iter().cycle().skip(1)) {
        assert_eq!(one.b, next.a, "{found:?}");
    }
}

#[test]
fn reading_runs_and_pairing_them_charges_a_unit_for_each_point_and_each_pair() {
    // One unit for each point between a run's ends.
    let edge = [p(0.0, 0.0), p(1.0, 0.0), p(2.0, 0.0)];
    assert!(straight(&edge, 0, 2, &mut Budget { max_stitches: 0, max_work: 1 }.meter()).unwrap());
    // One for each pair of runs: 4 runs the same way make 6 pairs and no stroke.
    let bar = part(&[(0.0, 0.0), (10.0, 0.0), (10.0, 1.0), (0.0, 1.0)], &[]);
    let runs: Vec<Side> = (0..4).map(|k| side((0.0, f64::from(k)), (5.0, f64::from(k)))).collect();
    assert_eq!(bands(&bar, &runs, LIMITS, &mut Budget { max_stitches: 0, max_work: 6 }.meter()), Ok(Some(Vec::new())));
    assert_eq!(bands(&bar, &runs, LIMITS, &mut Budget { max_stitches: 0, max_work: 5 }.meter()), Err(Exhausted::Work));
}

#[test]
fn two_sides_are_a_strokes_at_exactly_their_bounds() {
    // Opposite at exactly 5°.
    let sin = (1.0 - OPPOSITE * OPPOSITE).sqrt();
    let one = side((0.0, -5.0), (5.5, -5.0));
    let other = side((4.0 * OPPOSITE, 0.0), (0.0, 4.0 * sin));
    assert_eq!(dot(unit(one.a, one.b).unwrap(), unit(other.a, other.b).unwrap()), -OPPOSITE);
    assert!(paired(one, other, LIMITS).is_some());
    // Ends apart by exactly a tenth of the width, 10 mm, and by a little more.
    let long = side((0.0, 0.0), (100.0, 0.0));
    let wide = (0.5, 20.0);
    assert!(paired(long, side((100.0, 9.5), (0.0, 10.5)), wide).is_some());
    assert!(paired(long, side((100.0, 9.49), (0.0, 10.51)), wide).is_none());
}

#[test]
fn bands_lie_on_the_same_lines_at_exactly_2_degrees_whichever_way_they_lie_measured_at_their_middle() {
    let kept = Band { o: p(0.0, 0.0), u: (1.0, 0.0), n: (0.0, 1.0), lo: 0.0, hi: 1.0, from: 0.0, to: 2.0 };
    // Turned exactly 2° about kept's middle.
    let s = (1.0 - SAME_WAY * SAME_WAY).sqrt();
    assert!(same(&kept, &Band { o: p(1.0, 0.0), u: (SAME_WAY, s), n: (-s, SAME_WAY), from: -1.0, to: 1.0, ..kept }));
    // 3° about it, the 2 turned 30° together.
    let (sin, cos) = math::sin_cos(math::to_radians(3.0));
    let off = Band { o: p(1.0, 0.0), u: (cos, sin), n: (-sin, cos), from: -1.0, to: 1.0, ..kept };
    assert!(!same(&turned(kept, 30.0), &turned(off, 30.0)));
    assert!(same(&turned(kept, 30.0), &turned(kept, 30.0)));
    // A band 1.5° off kept's lines that crosses them at its middle, 10 mm along it: its ends lie 0.2 mm off.
    let (sin, cos) = math::sin_cos(math::to_radians(1.5));
    let long = Band { o: p(-10.0 * cos, -10.0 * sin), u: (cos, sin), n: (-sin, cos), lo: 0.0, hi: 1.0, from: 2.0, to: 18.0 };
    assert!(same(&kept, &long));
    // Merged, its ends count where its line half way across meets kept's.
    let slanted = Band { o: p(0.0, 0.0), lo: 1.0, hi: 3.0, from: -3.0, to: 6.0, ..long };
    let mut merged = kept;
    merge(&mut merged, &slanted);
    assert!((merged.from + 3.0 * cos + 2.0 * sin).abs() < 1e-12 && (merged.to - 6.0 * cos + 2.0 * sin).abs() < 1e-12, "{merged:?}");
}

#[test]
fn a_piece_of_a_stroke_past_its_sides_ends_is_left_to_the_others() {
    let meter = &mut Budget::DEFAULT.meter();
    // A bar to 10.6 mm whose stroke's sides end at 10.4 mm, crossed by an upright stroke from 10 to 10.5 mm:
    // the bar's piece past that holds none of its middle line, and it is less than a hundredth of the part.
    let bar = part(&[(0.0, 0.0), (10.6, 0.0), (10.6, 1.0), (0.0, 1.0)], &[]);
    let level = Band { o: p(0.0, 0.0), u: (1.0, 0.0), n: (0.0, 1.0), lo: 0.0, hi: 1.0, from: 0.0, to: 10.4 };
    let upright = Band { o: p(10.0, 0.0), u: (0.0, 1.0), n: (-1.0, 0.0), lo: -0.5, hi: 0.0, from: 0.0, to: 1.0 };
    let crossed = Joined { cuts: vec![Cut::Crossed { of: 1, by: 0.0 }], under: Vec::new() };
    let found = pieces(&bar, &[level, upright], &[crossed, Joined::default()], meter).unwrap().unwrap();
    assert_eq!(found[0].len(), 1, "{found:?}");
    assert!(found[0][0].outline.iter().all(|q| q.x() <= 10.0 + 1e-9), "{found:?}");
}

#[test]
fn a_column_with_pointed_ends_and_one_too_short_for_rails() {
    let meter = &mut Budget::DEFAULT.meter();
    let band = Band { o: p(0.0, 0.0), u: (1.0, 0.0), n: (0.0, 1.0), lo: 0.0, hi: 1.0, from: 0.0, to: 6.0 };
    // Pointed at both ends: the rails hold every point of the ring, the 2 ends twice.
    let pointed = part(&[(0.0, 0.5), (1.0, 0.0), (5.0, 0.0), (6.0, 0.5), (5.0, 1.0), (1.0, 1.0)], &[]);
    let (rails, _, _) = column(&pointed, &pointed, &band, meter).unwrap().unwrap();
    assert_eq!(rails[0].len() + rails[1].len(), 8);
    // Shorter along the stroke than a flat end may slant: its ends take in the whole ring.
    let short = part(&[(0.0, 0.0), (0.3, 0.0), (0.3, 1.0), (0.0, 1.0)], &[]);
    assert_eq!(column(&short, &short, &band, meter).unwrap(), None);
    // Shorter than a rail may run back: both ends take in the whole ring, and the rails, which each run
    // round past the other end, hold more points than it has.
    let tiny = part(&[(0.0, 0.0), (0.02, 0.0), (0.04, 0.0), (0.04, 1.0), (0.02, 1.0), (0.0, 1.0)], &[]);
    assert_eq!(column(&tiny, &tiny, &band, meter).unwrap(), None);
}

#[test]
fn a_pieces_ring_keeps_a_lone_point_and_a_last_point_exactly_close_from_its_first() {
    let meter = &mut Budget::DEFAULT.meter();
    let bar = part(&[(0.0, 0.0), (6.0, 0.0), (6.0, 1.0), (0.0, 1.0)], &[]);
    assert_eq!(cleaned(&[p(3.0, 0.5)], &bar, meter).unwrap(), vec![p(3.0, 0.5)]);
    let ring = [p(0.0, 0.5), p(3.0, 0.25), p(3.0, 0.75), p(CLOSE, 0.5)];
    assert_eq!(ring[0].distance(ring[3]), CLOSE);
    assert_eq!(cleaned(&ring, &bar, meter).unwrap(), ring.to_vec());
    // A run on from its start, and one round a ring that holds everywhere.
    assert_eq!(run_round(&[0.0, 0.0, 0.0, 5.0], 0, |t| t < 1.0), (0, 2));
    assert_eq!(run_round(&[0.0, 0.0, 0.0, 0.0], 2, |t| t < 1.0), (3, 2));
}

#[test]
fn rungs_at_exactly_their_bounds() {
    let meter = &mut Budget::DEFAULT.meter();
    let band = Band { o: p(0.0, 0.0), u: (1.0, 0.0), n: (0.0, 1.0), lo: 0.0, hi: 1.0, from: 0.0, to: 6.0 };
    let none = Vec::<[Point; 2]>::new();
    // A flat end whose corners lie exactly 2 × RUNGS_APART apart along the stroke is square.
    assert_eq!(rungs(&[p(0.1, 0.0), p(6.0, 0.0)], &[p(0.0, 1.0), p(6.0, 1.0)], &band, meter).unwrap(), none);
    // A rung exactly RUNGS_APART from the rails' ends is kept.
    assert_eq!(p(0.0, 0.0).distance(p(RUNGS_APART, 0.0)), RUNGS_APART);
    let one = [p(0.0, 0.0), p(0.05, 0.0), p(6.0, 0.0)];
    let other = [p(0.0, 1.0), p(0.05, 1.0), p(6.0, 1.0)];
    assert_eq!(rungs(&one, &other, &band, meter).unwrap(), vec![[p(0.05, 0.0), p(0.05, 1.0)]]);
    // Rungs exactly RUNGS_APART apart on both rails don't crowd.
    assert_eq!(p(0.05, 0.0).distance(p(0.1, 0.0)), RUNGS_APART);
    let one = [p(0.0, 0.0), p(0.05, 0.0), p(0.1, 0.0), p(6.0, 0.0)];
    let other = [p(0.0, 1.0), p(0.05, 1.0), p(0.1, 1.0), p(6.0, 1.0)];
    assert_eq!(rungs(&one, &other, &band, meter).unwrap(), vec![[p(0.05, 0.0), p(0.05, 1.0)], [p(0.1, 0.0), p(0.1, 1.0)]]);
    // A ring of no area keeps its order, and a dot product is exact.
    assert_eq!(polygon(vec![p(0.0, 0.0), p(1.0, 0.0), p(2.0, 0.0)]).outline, vec![p(0.0, 0.0), p(1.0, 0.0), p(2.0, 0.0), p(0.0, 0.0)]);
    assert_eq!(dot((2.0, 3.0), (5.0, 7.0)), 31.0);
    assert_eq!(cosine((0.6, 0.8), (-0.6, -0.8)), 1.0);
}

#[test]
fn strokes_meet_in_exactly_the_least_area_and_their_ends_measure_from_the_join() {
    let meter = &mut Budget::DEFAULT.meter();
    // 5 and 10 mm wide, they meet in the whole of a square of 1 mm², exactly 0.02 × 5 × 10.
    let square_part = part(&[(0.0, 0.0), (1.0, 0.0), (1.0, 1.0), (0.0, 1.0)], &[]);
    let five = Band { o: p(0.0, 0.0), u: (1.0, 0.0), n: (0.0, 1.0), lo: -2.0, hi: 3.0, from: -1.0, to: 2.0 };
    let ten = Band { o: p(0.0, 0.0), u: (0.0, 1.0), n: (-1.0, 0.0), lo: -5.0, hi: 5.0, from: -1.0, to: 2.0 };
    assert_eq!(0.02 * five.width() * ten.width(), 1.0);
    assert!(meeting(&square_part, &five, &ten, meter).unwrap().unwrap().is_some());
    // The end at a join lies half way along it.
    assert_eq!(end(&five, &[p(3.0, 0.0), p(5.0, 1.0)]), 4.0);
    // A stroke whose middle lies on either side of the other's band doesn't stop against it.
    let running = Band { o: p(0.0, 5.0), u: (1.0, 0.0), n: (0.0, 1.0), lo: 0.0, hi: 1.0, from: 0.0, to: 6.0 };
    for y in [3.0, 4.0] {
        let stopping = Band { o: p(2.5, y), u: (0.0, 1.0), n: (-1.0, 0.0), lo: -0.5, hi: 0.5, from: 0.0, to: 4.0 };
        assert_eq!(stops(&stopping, &running, REACH), None, "{y}");
    }
    // A ray from a point of an edge leaves the part where it next crosses one, on the slant too.
    let bar = part(&[(0.0, 0.0), (10.0, 0.0), (10.0, 1.0), (0.0, 1.0)], &[]);
    assert_eq!(leaves(&bar, p(0.0, 0.5), (1.0, 0.0)), Some(10.0));
    let slant = part(&[(0.0, 0.0), (10.0, 0.0), (9.0, 2.0), (0.0, 2.0)], &[]);
    assert!((leaves(&slant, p(5.0, 0.2), (1.0, 0.0)).unwrap() - 4.9).abs() < 1e-12);
    // A band 2 mm wide whose end slants back 0.8 mm between its sides' rays, a tenth of its width in.
    let wide = Band { o: p(0.0, 0.0), u: (1.0, 0.0), n: (0.0, 1.0), lo: 0.0, hi: 2.0, from: 0.0, to: 10.0 };
    assert!((square(&slant, &wide, &[p(9.5, 1.0)]).unwrap() - 0.4).abs() < 1e-12);
}

#[test]
fn a_stroke_that_crosses_another_on_the_slant_loses_its_band_narrowed_by_its_reachs_share() {
    let (sin, cos) = math::sin_cos(math::to_radians(60.0));
    let level = Band { o: p(0.0, 0.0), u: (1.0, 0.0), n: (0.0, 1.0), lo: -0.5, hi: 0.5, from: 0.0, to: 10.0 };
    let slanted = Band { o: p(5.0 - 5.0 * cos, -5.0 * sin), u: (cos, sin), n: (-sin, cos), lo: -0.5, hi: 0.5, from: 0.0, to: 10.0 };
    let met = [p(4.4, -0.5), p(5.6, -0.5), p(5.6, 0.5), p(4.4, 0.5)];
    let cuts = join(&t(), (0, &level), (1, &slanted), &met, REACH).unwrap();
    assert_eq!(cuts.len(), 1);
    assert!(matches!(cuts[0], (1, Cut::Crossed { of: 0, by }, Some((_, 0))) if (by - REACH * sin).abs() < 1e-12), "{cuts:?}");
}

#[test]
fn of_2_strokes_whose_ends_differ_by_exactly_tie_neither_is_squarer() {
    // A level stroke 2 mm wide with a square end, and a longer upright one whose sides' rays from its middle
    // leave the part 0.5 and 0.2 mm on: 0.3 mm over its width of 2, exactly TIE.
    let ring = [
        (0.0, 0.0),
        (8.0, 0.0),
        (8.0, 2.0),
        (17.5, 2.0),
        (17.5, 0.3),
        (18.5, 0.3),
        (18.5, 2.0),
        (19.5, 2.0),
        (19.5, 0.0),
        (20.5, 0.0),
        (20.5, 4.0),
        (0.0, 4.0),
    ];
    let part = part(&ring, &[]);
    let level = Band { o: p(0.0, 0.0), u: (1.0, 0.0), n: (0.0, 1.0), lo: 0.0, hi: 2.0, from: 0.0, to: 8.0 };
    let upright = Band { o: p(20.0, -100.0), u: (0.0, 1.0), n: (-1.0, 0.0), lo: 0.0, hi: 2.0, from: 91.0, to: 110.0 };
    let met = [p(7.5, -8.5), p(8.0, -8.5), p(8.0, -8.0), p(7.5, -8.0)];
    assert_eq!((square(&part, &level, &met), square(&part, &upright, &met)), (Some(0.0), Some(TIE)));
    // At right angles, the shorter stops, whichever is named first.
    let bands = [level, upright];
    for (a, b) in [(0, 1), (1, 0)] {
        let cuts = join(&part, (a, &bands[a]), (b, &bands[b]), &met, REACH).unwrap();
        assert_eq!(cuts.iter().map(|cut| cut.0).collect::<Vec<_>>(), vec![0]);
    }
}

#[test]
fn a_halving_line_at_a_blunt_join_lets_each_stroke_reach_its_share_past_it() {
    // Strokes 150° apart, at 10° and 160° from their join at (2, 3): the halving line runs at 85°, each stroke
    // 75° from it, and each reaches 0.2 - 0.4 cos 75° mm past it. The first runs from its far end in.
    let way = |degrees: f64| {
        let (sin, cos) = math::sin_cos(math::to_radians(degrees));
        (cos, sin)
    };
    let (a, b, h) = (way(10.0), way(160.0), way(85.0));
    let one = Band { o: p(2.0 + 6.0 * a.0, 3.0 + 6.0 * a.1), u: (-a.0, -a.1), n: (a.1, -a.0), lo: -0.5, hi: 0.5, from: 0.0, to: 5.0 };
    let other = Band { o: p(2.0, 3.0), u: b, n: (-b.1, b.0), lo: -0.5, hi: 0.5, from: 1.0, to: 6.0 };
    let past = REACH / 2.0 - REACH * cosine(a, h);
    let m = (-h.1, h.0);
    let expected = [(0, (2.0 + past * m.0, 3.0 + past * m.1), (-m.0, -m.1)), (1, (2.0 - past * m.0, 3.0 - past * m.1), m)];
    for ((stroke, cut, _), (want, at, keeps)) in halved((0, &one), (1, &other), REACH).unwrap().into_iter().zip(expected) {
        assert_eq!(stroke, want);
        assert!(
            matches!(cut, Cut::Keep { p: q, m: kept } if q.distance(p(at.0, at.1)) < 1e-9 && (kept.0 - keeps.0).abs() < 1e-9 && (kept.1 - keeps.1).abs() < 1e-9),
            "{stroke}: {cut:?}"
        );
    }
}
