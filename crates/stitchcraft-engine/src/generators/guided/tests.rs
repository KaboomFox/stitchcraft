//! The guided fill's cases: its copies, their order and stagger, its fallbacks and what it says. The guide's
//! preparation and the cutting of its rows are tested in their modules (`line`, `cut`); whole designs in
//! `tests/fill_guided.rs`.

use stitchcraft_core::Budget;

use super::*;
use crate::design::{Segment as Drawn, Subpath};
use crate::generators::tatami::fixture::{p, rectangle};
use crate::generators::tatami::stitches::Stitching as Rows;

const TRAVEL: Travel = Travel { length: 2.5, tolerance: 0.2, min_stitch: 0.3, underpath: true, jump: 3.0 };

/// The open polyline through `points`, as a path.
fn path(points: &[(f64, f64)]) -> Path {
    let start = p(points[0].0, points[0].1);
    Path { subpaths: vec![Subpath { start, segments: points[1..].iter().map(|&(x, y)| Drawn::Line(p(x, y))).collect(), closed: false }] }
}

/// A guided fill along the polyline `guide`, its copies `spacing` apart, in stitches of 3 mm staggered over 4
/// rows, with no underlay and no tatami fill to fall back to.
fn guided(guide: &[(f64, f64)], spacing: f64) -> Guided {
    Guided {
        guide: Some(path(guide)),
        spacing,
        angle: None,
        smoothness: 0.0,
        stitching: RowStitching { length: 3.0, tolerance: 0.2, jitter: None, staggers: 4.0, position: Position::Flexible },
        skip_last: false,
        beans: vec![0],
        underlay: Vec::new(),
        inset: 0.0,
        expand: 0.0,
        force_locks: false,
        travel: TRAVEL,
        plain: Plain { fill: None, said: Vec::new() },
    }
}

/// The tatami fill a guided fill falls back to without a guide in these cases: rows 1 mm apart at 0°.
fn plain() -> Fill {
    let stitching = Rows { grid: Grid { angle: 0.0, spacing: 1.0, end_spacing: None }, length: 3.0, staggers: 4.0, skip_last: false, jitter: None };
    Fill {
        top: Layer { stitching, travel: TRAVEL, pull: Pull::default(), gap_rows: 0 },
        expand: 0.0,
        underlay: Vec::new(),
        inset: 0.0,
        force_locks: false,
    }
}

/// The rows `guided`'s guide gives over `piece`.
fn rows(piece: &Polygon, guided: &Guided) -> Vec<Vec<Point>> {
    let meter = &mut Budget::DEFAULT.meter();
    let rings = Rings::new(piece, meter).unwrap();
    let guide = guide_of(guided, meter).unwrap().unwrap();
    copies::rows(piece, &rings, &guide.line, guided, meter).unwrap()
}

/// The y of each of `rows`, which must be level.
fn levels(rows: &[Vec<Point>]) -> Vec<f64> {
    rows.iter()
        .map(|row| {
            let y = row[0].y();
            assert!(row.iter().all(|q| (q.y() - y).abs() < 1e-9), "{row:?}");
            (y * 1e9).round() / 1e9
        })
        .collect()
}

fn sew(piece: &Polygon, guided: &Guided) -> Stitched {
    let region = Region { parts: vec![piece.clone()] };
    guided_fill(&region, guided, Some(p(0.0, 0.0)), None, &mut SplitMix64::new(7), &mut Budget::DEFAULT.meter()).unwrap()
}

fn codes(stitched: &Stitched) -> Vec<Code> {
    stitched.warnings.iter().map(|w| w.code).collect()
}

#[test]
fn req_fill_gdd_003_copies_move_square_to_the_line_between_the_guide_s_ends_clockwise_as_drawn() {
    // A guide above a 10 × 4 piece, drawn to the right: its copies move down the page, 1 mm at a time, from
    // the copy 3 mm down, the whole number of spacings the centroid lies from the guide, and those that meet
    // the piece, its edges included, are its rows.
    let piece = rectangle(4.0);
    let right = rows(&piece, &guided(&[(0.0, -1.0), (10.0, -1.0)], 1.0));
    assert_eq!(levels(&right), [0.0, 1.0, 2.0, 3.0, 4.0]);
    // Each is lengthened at both ends by the distance from the guide's end to the piece's farthest corner.
    let reach = 125.0_f64.sqrt();
    let xs: Vec<f64> = right[2].iter().map(|q| q.x()).collect();
    assert_eq!(xs, [-reach, 0.0, 10.0, 10.0 + reach]);
    // Drawn to the left, they move up the page: the piece is behind the guide, and they start 3 copies back.
    let left = rows(&piece, &guided(&[(10.0, -1.0), (0.0, -1.0)], 1.0));
    assert_eq!(levels(&left), [0.0, 1.0, 2.0, 3.0, 4.0]);
    assert_eq!(left[2].first().map(|q| q.x()), Some(10.0 + reach));
    // At guided_fill_angle, 45° clockwise from the x axis: each copy moves 0.71 mm down and 0.71 mm right.
    let turned = rows(&piece, &Guided { angle: Some(45.0), ..guided(&[(0.0, -1.0), (10.0, -1.0)], 1.0) });
    let expected: Vec<f64> = (2..8).map(|k| ((-1.0 + f64::from(k) * std::f64::consts::FRAC_1_SQRT_2) * 1e9).round() / 1e9).collect();
    assert_eq!(levels(&turned), expected);
    assert!((turned[0][0].x() - (-reach + 2.0 * std::f64::consts::FRAC_1_SQRT_2)).abs() < 1e-9, "{:?}", turned[0]);
}

#[test]
fn req_fill_gdd_003_the_copies_start_from_the_guide_where_it_meets_the_piece_and_from_the_centroid_elsewhere() {
    let piece = rectangle(4.0);
    // Across the piece: from the guide itself, a spacing of 1.5 mm each way.
    assert_eq!(levels(&rows(&piece, &guided(&[(0.0, 2.2), (10.0, 2.2)], 1.5))), [0.7, 2.2, 3.7]);
    // Below it, drawn to the right: the copies move down the page, and the centroid, 4 mm up, lies behind
    // the guide, 2 whole spacings: they start 2 copies back, 3 mm up.
    assert_eq!(levels(&rows(&piece, &guided(&[(0.0, 6.0), (10.0, 6.0)], 1.5))), [0.0, 1.5, 3.0]);
    // 3.2 mm away the count is rounded down, to 2 spacings.
    assert_eq!(levels(&rows(&piece, &guided(&[(0.0, -1.2), (10.0, -1.2)], 1.5))), [0.3, 1.8, 3.3]);
}

#[test]
fn req_fill_gdd_004_rows_are_taken_by_their_first_points_along_the_diagonal_of_the_box_round_them() {
    let meter = &mut Budget::DEFAULT.meter();
    let row = |x: f64, y: f64| vec![p(x, y), p(x + 1.0, y)];
    // 3 rows: by how far along the diagonal from (0, 0) to (2, 5) each first point lies, not by x.
    let mut three = vec![row(2.0, 0.0), row(0.0, 5.0), row(1.0, 1.0)];
    copies::sorted(&mut three, meter).unwrap();
    assert_eq!(three, [row(2.0, 0.0), row(1.0, 1.0), row(0.0, 5.0)]);
    // As far along, they keep the order they were found in.
    let mut even = vec![row(1.0, 0.0), row(0.0, 1.0), row(0.0, 0.0)];
    copies::sorted(&mut even, meter).unwrap();
    assert_eq!(even, [row(0.0, 0.0), row(1.0, 0.0), row(0.0, 1.0)]);
    // 2 rows: by x, then by y.
    let mut two = vec![row(1.0, 0.0), row(0.0, 5.0)];
    copies::sorted(&mut two, meter).unwrap();
    assert_eq!(two, [row(0.0, 5.0), row(1.0, 0.0)]);
    let mut level = vec![row(0.0, 5.0), row(0.0, 1.0)];
    copies::sorted(&mut level, meter).unwrap();
    assert_eq!(level, [row(0.0, 1.0), row(0.0, 5.0)]);
}

/// The x of each needle point of the `i`-th row taken along the line from (0, 0) to (20, 0), stitched as
/// `stitching` says.
fn needles(i: usize, stitching: RowStitching) -> Vec<f64> {
    let meter = &mut Budget::DEFAULT.meter();
    let guided = Guided { stitching, ..guided(&[(0.0, 0.0), (1.0, 0.0)], 1.0) };
    let row = stitched::row(&[p(0.0, 0.0), p(20.0, 0.0)], i, &guided, &SplitMix64::new(7), meter).unwrap();
    assert!(row.iter().all(|q| q.y() == 0.0), "{row:?}");
    row.iter().map(|q| (q.x() * 1e9).round() / 1e9).collect()
}

#[test]
fn req_fill_gdd_005_each_row_s_needle_points_start_a_staggers_th_of_a_stitch_further_along() {
    let even = RowStitching { length: 3.0, tolerance: 0.2, jitter: None, staggers: 4.0, position: Position::Flexible };
    // Flexible: the row's first point, then stitches spread evenly from the stagger to its end, here from
    // the first point again.
    let first: Vec<f64> = std::iter::once(0.0).chain((0..=7).map(|k| (f64::from(k) * 20.0 / 7.0 * 1e9).round() / 1e9)).collect();
    assert_eq!(needles(0, even), first);
    let second = needles(1, even);
    assert_eq!(second[..2], [0.0, 0.75]);
    assert_eq!(second.len(), 9, "{second:?}");
    assert_eq!(needles(2, even)[1], 1.5);
    assert_eq!(needles(3, even)[1], 2.25);
    // The fifth row is back where the first started.
    assert_eq!(needles(4, even), needles(0, even));
    // Flexible (stagger): whole stitches from the stagger, the last whatever is left.
    let stagger = RowStitching { position: Position::Stagger, ..even };
    assert_eq!(needles(1, stagger), [0.0, 0.75, 3.75, 6.75, 9.75, 12.75, 15.75, 18.75, 20.0]);
    // Staggers that don't divide a whole turn, and none: every row from its first point (DEV-FILL-033).
    assert_eq!(needles(3, RowStitching { staggers: 2.5, ..stagger })[1], 0.6);
    assert_eq!(needles(3, RowStitching { staggers: 0.0, ..stagger }), needles(0, stagger));
}

#[test]
fn req_fill_gdd_005_a_row_is_smoothed_before_it_is_stitched() {
    // A row with a hairpin turn: smoothed by 1 mm, its needle points keep off the turn's point.
    let meter = &mut Budget::DEFAULT.meter();
    let smooth = Guided { smoothness: 1.0, ..guided(&[(0.0, 0.0), (1.0, 0.0)], 1.0) };
    let copy = [p(0.0, 0.0), p(10.0, 0.0), p(10.0, 1.0), p(0.0, 1.0)];
    let row = stitched::row(&copy, 0, &smooth, &SplitMix64::new(7), meter).unwrap();
    assert!(row.iter().all(|q| q.distance(p(10.0, 0.0)) > 0.2), "{row:?}");
    let sharp = stitched::row(&copy, 0, &guided(&[(0.0, 0.0), (1.0, 0.0)], 1.0), &SplitMix64::new(7), meter).unwrap();
    assert!(sharp.contains(&p(10.0, 0.0)), "{sharp:?}");
    assert!(stitched::row(&[], 0, &smooth, &SplitMix64::new(7), meter).unwrap().is_empty());
}

#[test]
fn req_fill_gdd_001_a_guided_fill_sews_its_rows_along_copies_of_its_guide() {
    // A guide with a peak over the middle of a 10 × 4 piece: the rows slant with it, up to the peak and down
    // from it.
    let piece = rectangle(4.0);
    let sewn = sew(&piece, &guided(&[(0.0, -1.0), (5.0, -2.0), (10.0, -1.0)], 0.5));
    assert_eq!(codes(&sewn), []);
    let points: Vec<Point> = sewn.runs.concat();
    assert!(points.len() > 40, "{}", points.len());
    let slant = |a: Point, b: Point| (b.y() - a.y()) / (b.x() - a.x());
    // The rows' stitches, more than 1 mm across: none slants more than the guide, and many as much, those
    // that don't reach over its peak.
    let rows: Vec<f64> =
        points.windows(2).filter(|w| (w[1].x() - w[0].x()).abs() > 1.0).map(|w| slant(w[0], w[1]).abs()).filter(|s| *s < 0.5).collect();
    assert!(rows.iter().all(|s| *s <= 0.2 + 1e-9), "{rows:?}");
    assert!(rows.iter().filter(|s| (*s - 0.2).abs() < 1e-9).count() >= 10, "{rows:?}");
}

#[test]
fn diag_sc_w0331_a_guided_fill_without_a_guide_line_sews_as_a_tatami_fill() {
    let piece = rectangle(4.0);
    let region = Region { parts: vec![piece.clone()] };
    let tatami = tatami_fill(&region, &plain(), Some(p(0.0, 0.0)), None, &mut SplitMix64::new(7), &mut Budget::DEFAULT.meter()).unwrap();
    let said = Diagnostic::new(Code::ParamClamped, "A word reading the tatami fill's settings said.");
    let without = Guided { guide: None, plain: Plain { fill: Some(plain()), said: vec![said.clone()] }, ..guided(&[(0.0, 0.0)], 1.0) };
    let sewn = sew(&piece, &without);
    assert_eq!(sewn.runs, tatami.runs);
    assert_eq!(sewn.warnings[0].message, "This guided fill has no guide line, so it is sewn as a tatami fill with its own settings.");
    assert_eq!(sewn.warnings[1], said);
    // A guide of no length: drawn, but its ends at one point (DEV-FILL-034).
    let dot = Guided { guide: Some(path(&[(5.0, 5.0), (5.0, 5.0)])), ..without.clone() };
    let sewn = sew(&piece, &dot);
    assert_eq!((codes(&sewn), sewn.runs), (vec![Code::FillGuidedNoGuide, Code::ParamClamped], tatami.runs));
    assert_eq!(
        sewn.warnings[0].message,
        "This guided fill's guide line starts and ends at one point, so the fill is sewn as a tatami fill with its own settings."
    );
    // Settings that don't read as a tatami fill's sew nothing, and say so.
    let unread = sew(&piece, &Guided { plain: Plain { fill: None, said: vec![said] }, ..without });
    assert_eq!((unread.runs, unread.warnings.len()), (Vec::<Vec<Point>>::new(), 2));
}

#[test]
fn diag_sc_w0332_a_part_no_copy_of_the_guide_meets_sews_as_a_tatami_fill_at_the_angle_of_the_guide_s_ends() {
    // A strip 10 × 0.4 between the copies 1 mm apart of a guide 1 mm above it: none meets it. It falls back to
    // the tatami fill's rows at the angle of the guide's ends, 1 mm apart, which miss it too, and it is sewn
    // round its outline (SC-W0305).
    let strip = Polygon { outline: vec![p(0.0, 0.3), p(0.0, 0.7), p(10.0, 0.7), p(10.0, 0.3), p(0.0, 0.3)], holes: Vec::new() };
    let missed = sew(&strip, &guided(&[(0.0, -1.0), (10.0, -1.0)], 1.0));
    assert_eq!(codes(&missed), [Code::FillGuidedMissed, Code::FillTooThin]);
    assert_eq!(
        missed.warnings[0].message,
        "No copy of this guided fill's guide meets it, so it is sewn as a tatami fill at the angle of the line between the guide's ends."
    );
    // A strip from y = 1.5 to 2.5, between copies 2 mm apart at 0.7 and 2.7: the tatami row at y = 2, at 0°,
    // the angle of the guide's ends, crosses it.
    let strip = Polygon { outline: vec![p(0.0, 1.5), p(0.0, 2.5), p(10.0, 2.5), p(10.0, 1.5), p(0.0, 1.5)], holes: Vec::new() };
    let mut sewn = sew(&strip, &guided(&[(0.0, -1.3), (10.0, -1.3)], 2.0));
    assert_eq!(codes(&sewn), [Code::FillGuidedMissed]);
    let points = sewn.runs.remove(0);
    assert!(points.windows(2).any(|w| w[0].distance(w[1]) > 2.0 && (w[0].y() - 2.0).abs() < 1e-9 && (w[1].y() - 2.0).abs() < 1e-9), "{points:?}");
}

#[test]
fn diag_sc_w0332_says_how_many_parts_no_copy_meets() {
    let tally = |pieces, missed, outlined| Tally { pieces, missed, outlined }.said().map(|d| (d.code, d.message)).collect::<Vec<_>>();
    assert_eq!(tally(3, 0, 0), []);
    let one = "No copy of this guided fill's guide meets 1 of its parts, so that part is sewn as a tatami fill at the angle of the line \
               between the guide's ends.";
    assert_eq!(tally(3, 1, 0), [(Code::FillGuidedMissed, one.to_string())]);
    let two = "No copy of this guided fill's guide meets 2 of its parts, so each is sewn as a tatami fill at the angle of the line between \
               the guide's ends.";
    let said = tally(3, 2, 1);
    assert_eq!(said[0], (Code::FillGuidedMissed, two.to_string()));
    assert_eq!(said.iter().map(|(code, _)| *code).collect::<Vec<_>>(), [Code::FillGuidedMissed, Code::FillTooThin]);
}

#[test]
fn req_fill_gdd_007_bean_stitch_sews_each_stitch_of_the_rows_back_and_forth_but_the_travel() {
    let piece = rectangle(4.0);
    let once = guided(&[(0.0, -1.0), (10.0, -1.0)], 1.0);
    let beaned = sew(&piece, &Guided { beans: vec![1], ..once.clone() });
    let plain = sew(&piece, &once);
    // Each stitch of the 3 rows inside the piece, 4 each, goes back and forth; the travel stitches, along
    // the sides, don't.
    let (sewn, beaned) = (plain.runs.concat(), beaned.runs.concat());
    assert!(beaned.len() > sewn.len() + 20, "{} {}", beaned.len(), sewn.len());
    assert!(beaned.windows(3).filter(|w| w[0] == w[2] && w[0] != w[1]).count() >= 12);
    // Short stitches are left out before the bean stitch, so none of its passes is shorter than the shortest.
    assert!(beaned.windows(2).all(|w| w[0] == w[1] || w[0].distance(w[1]) >= TRAVEL.min_stitch - 1e-9), "{beaned:?}");
}

#[test]
fn a_preview_shows_the_rows_or_the_tatami_fill_s_rows_where_the_guide_is_no_use() {
    let meter = &mut Budget::DEFAULT.meter();
    let rng = SplitMix64::new(7);
    let region = Region { parts: vec![rectangle(4.0)] };
    // The rows inside the piece: the copies at 1, 2 and 3 mm, cut by it.
    let along = guided(&[(0.0, -1.0), (10.0, -1.0)], 1.0);
    let segments = top_path(&region, &along, &rng, meter).unwrap();
    let on = |y: f64| [1.0, 2.0, 3.0].iter().any(|row| (y - row).abs() < 1e-9);
    assert!(segments.len() > 9 && segments.iter().all(|s| on(s.start.y()) && on(s.end.y())), "{segments:?}");
    // Without a guide, the tatami fill's rows, or none without one to fall back to.
    let without = Guided { guide: None, plain: Plain { fill: Some(plain()), said: Vec::new() }, ..along.clone() };
    assert_eq!(top_path(&region, &without, &rng, meter).unwrap(), top_rows(&region, &plain(), meter).unwrap());
    assert_eq!(top_path(&region, &Guided { plain: Plain { fill: None, said: Vec::new() }, ..without }, &rng, meter).unwrap(), []);
    // A part no copy meets shows the tatami rows it falls back to.
    let strip =
        Region { parts: vec![Polygon { outline: vec![p(0.0, 1.5), p(0.0, 2.5), p(10.0, 2.5), p(10.0, 1.5), p(0.0, 1.5)], holes: Vec::new() }] };
    let fallback = top_path(&strip, &guided(&[(0.0, -1.3), (10.0, -1.3)], 2.0), &rng, meter).unwrap();
    assert!(!fallback.is_empty() && fallback.iter().all(|s| (s.start.y() - 2.0).abs() < 1e-9 && (s.end.y() - 2.0).abs() < 1e-9), "{fallback:?}");
}

#[test]
fn a_guided_fill_is_charged_to_the_budget() {
    let region = Region { parts: vec![rectangle(4.0)] };
    let along = guided(&[(0.0, -1.0), (5.0, -2.0), (10.0, -1.0)], 0.5);
    let sewn = |work: u64| {
        let budget = Budget { max_stitches: 10_000, max_work: work };
        guided_fill(&region, &along, None, None, &mut SplitMix64::new(7), &mut budget.meter())
    };
    assert!(sewn(Budget::DEFAULT.max_work).is_ok());
    assert!(sewn(100).is_err());
    // A preview's budget holds any number of needle points.
    let preview = |work: u64| top_path(&region, &along, &SplitMix64::new(7), &mut Budget { max_stitches: u32::MAX, max_work: work }.meter());
    assert!(preview(Budget::DEFAULT.max_work).is_ok());
    assert!(preview(100).is_err());
}
