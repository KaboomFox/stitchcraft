//! Cases of a part read as strokes (`REQ-AUTO-087`, `REQ-AUTO-088`): the traced word's M and A, a T, an L,
//! a cross, and the parts that keep their centre lines' columns, with the bounds of each measure.

use stitchcraft_core::Budget;

use super::joins::{crossed, halved, join, leaves, meeting, square, stops};
use super::*;
use crate::normalize::fixture::part;
use crate::normalize::strips::columns;

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
    along.atan2(across).to_degrees()
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
    assert!(columns[stem].under.contains(&Some(bar)));
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
    for part in [t(), l(0.0), cross()] {
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
        let (sin, cos) = degrees.to_radians().sin_cos();
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
    let (sin, cos) = 2.5_f64.to_radians().sin_cos();
    assert!(!same(&kept, &Band { u: (cos, sin), n: (-sin, cos), ..later }));
    let (sin, cos) = 1.5_f64.to_radians().sin_cos();
    assert!(same(&kept, &Band { o: p(0.0, 0.0), u: (cos, sin), n: (-sin, cos), from: 0.0, to: 2.0, ..kept }));
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
    let (sin, cos) = 60.0_f64.to_radians().sin_cos();
    let slanted = Band { u: (cos, sin), n: (-sin, cos), ..below };
    let Some(Cut::Keep { p: at, m }) = stops(&slanted, &running, REACH) else { panic!() };
    assert!((at.y() - 5.2).abs() < 1e-9 && m == (0.0, -1.0), "{at:?}");
    // A stroke whose middle lies inside the other's band doesn't stop against it.
    let inside = Band { o: p(2.5, 5.2), u: (0.0, 1.0), n: (-1.0, 0.0), lo: -1.0, hi: 0.0, from: 0.0, to: 0.5 };
    assert_eq!(stops(&inside, &running, REACH), None);
}

#[test]
fn a_halving_line_cuts_2_strokes_at_the_point_their_lines_cross() {
    let (sin, cos) = 60.0_f64.to_radians().sin_cos();
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
