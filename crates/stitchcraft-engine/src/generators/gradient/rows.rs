//! A gradient fill's rows over one piece of it, their needle points, and the row each stop lies at, as
//! Ink/Stitch lays them (`docs/src/design/algorithms/gradient.md` › Rows, Each stop's row, read at
//! `d59c9ab`).
//!
//! **Why along the gradient.** A gradient's colours change along its way and stay the same across it, so
//! the rows lie square to the way and each row can take one colour. Measured along the way, they reach a
//! longest stitch past the piece either side, in equal steps no longer than the row spacing. Measured across,
//! each reaches a row spacing past the piece's outline at both ends, and is cut to the piece later.
//!
//! **Needle points.** Along each row from its first end, the points lie the longest stitch apart, each row's
//! from a `staggers`-th of a stitch further on than the row before's. A row's ends are points too where the
//! nearest point lies more than half a row spacing from them. With random lengths, each row is a running
//! stitch at random lengths from end to end instead, drawn from a stream of the element's generator that the
//! row's number keys (`DEV-FILL-004`).
//!
//! **The stops' rows** are counted in row spacings, as Ink/Stitch counts them, though the steps between rows
//! can be a little shorter: the gradient's start from row 0, and each stop its offset's share of the
//! gradient's length on from the start, rounded halves to even.

use stitchcraft_core::math::{sin_cos, to_radians};
use stitchcraft_core::rng::SplitMix64;
use stitchcraft_core::{Exhausted, Meter, Point};

use crate::design::Gradient;
use crate::generators::running::along_line_varied;
use crate::normalize::region::Polygon;

/// The way a gradient fill's rows lie: `along` the gradient, a unit vector, and `across` it, the way the rows
/// run, `along` turned a quarter turn clockwise as the canvas shows it, y down.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Way {
    pub(crate) along: (f64, f64),
    pub(crate) across: (f64, f64),
}

impl Way {
    /// The way of the gradient from `start` to `end`; along x where they are one point, as Ink/Stitch takes
    /// the angle of a vector of no length.
    pub(crate) fn of_gradient(start: Point, end: Point) -> Way {
        let length = start.distance(end);
        if length > 0.0 { Way::of(((end.x() - start.x()) / length, (end.y() - start.y()) / length)) } else { Way::of((1.0, 0.0)) }
    }

    /// The way at `degrees` turned clockwise from x as the canvas shows it, where a fill without a gradient
    /// fades. A cosine or a sine within 2.5 × 10⁻¹⁶ of 0 is 0, as in Ink/Stitch's geometry library, so that
    /// quarter turns are exact.
    pub(crate) fn at(degrees: f64) -> Way {
        let (sin, cos) = sin_cos(to_radians(degrees));
        let snapped = |value: f64| if value.abs() < 2.5e-16 { 0.0 } else { value };
        Way::of((snapped(cos), snapped(sin)))
    }

    fn of(along: (f64, f64)) -> Way {
        Way { along, across: (-along.1, along.0) }
    }

    /// How far `p` lies along the way, and across it, from the origin.
    fn place(self, p: Point) -> (f64, f64) {
        (p.x() * self.along.0 + p.y() * self.along.1, p.x() * self.across.0 + p.y() * self.across.1)
    }

    /// The point `along` the way and `across` it from the origin.
    fn point(self, along: f64, across: f64) -> Option<Point> {
        Point::new(along * self.along.0 + across * self.across.0, along * self.along.1 + across * self.across.1).ok()
    }
}

/// How a gradient fill's rows are stitched.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Stitching {
    /// The longest stitch, in millimetres: how far apart a row's needle points lie.
    pub(crate) longest: f64,
    /// How many rows the points take to come back to where they started.
    pub(crate) staggers: f64,
    /// With random lengths, how much longer or shorter each stitch may be, as a share of the longest stitch,
    /// the running stitch's tolerance and the shortest stitch; `None` for staggered points.
    pub(crate) random: Option<(f64, f64, f64)>,
}

/// A gradient fill's rows over one piece.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Rows {
    pub(crate) way: Way,
    /// Where row 0 lies along the way, and the step from each row to the next.
    first: f64,
    step: f64,
    /// The last row's number: the rows are 0 to it.
    pub(crate) last: usize,
    /// Where each row's first end and last end lie across the way.
    from: f64,
    to: f64,
    /// The row spacing, which the stops' rows are counted in, and the rows' reach along the way.
    spacing: f64,
    reach: f64,
}

impl Rows {
    /// The rows over `piece` along `way`, at most `spacing` apart, reaching `longest` past it either side
    /// along the way, as the module's docs say; `None` for a piece without points. A unit of `meter` for each
    /// point of its outline and each row.
    pub(crate) fn lay(piece: &Polygon, way: Way, spacing: f64, longest: f64, meter: &mut Meter) -> Result<Option<Rows>, Exhausted> {
        meter.charge(u64::try_from(piece.outline.len()).unwrap_or(u64::MAX))?;
        let mut places = piece.outline.iter().map(|&p| way.place(p));
        let Some((along, across)) = places.next() else { return Ok(None) };
        let (mut least, mut most) = ((along, across), (along, across));
        for (along, across) in places {
            least = (least.0.min(along), least.1.min(across));
            most = (most.0.max(along), most.1.max(across));
        }
        let reach = most.0 - least.0 + 2.0 * longest;
        let steps = whole((reach / spacing).ceil()).max(1);
        let last = usize::try_from(steps).unwrap_or(usize::MAX);
        meter.charge(u64::try_from(last).unwrap_or(u64::MAX).saturating_add(1))?;
        let rows =
            Rows { way, first: least.0 - longest, step: reach / last as f64, last, from: least.1 - spacing, to: most.1 + spacing, spacing, reach };
        Ok(Some(rows))
    }

    /// Row `k`'s first end and its last.
    pub(crate) fn ends(&self, k: usize) -> Option<(Point, Point)> {
        let along = self.first + k as f64 * self.step;
        Some((self.way.point(along, self.from)?, self.way.point(along, self.to)?))
    }

    /// Row `k`'s needle points from its first end, as `stitching` places them, its random lengths drawn from
    /// `rng`'s stream for the row. A unit of `meter` for each point, spent before they are made.
    pub(crate) fn needle_points(&self, k: usize, stitching: &Stitching, rng: &SplitMix64, meter: &mut Meter) -> Result<Vec<Point>, Exhausted> {
        let Some((first_end, last_end)) = self.ends(k) else { return Ok(Vec::new()) };
        if let Some((jitter, tolerance, min_stitch)) = stitching.random {
            return along_line_varied(&[first_end, last_end], (stitching.longest, tolerance, min_stitch), Some(jitter), (rng, k), meter);
        }
        let length = self.to - self.from;
        let longest = stitching.longest;
        // Ink/Stitch reads a staggers of 0 as 1, which lines every row's points up.
        let staggers = if stitching.staggers == 0.0 { 1.0 } else { stitching.staggers };
        let start = (k as f64 / staggers).rem_euclid(1.0) * longest;
        let count = whole(((length - start) / longest).ceil()).max(0);
        meter.charge(u64::try_from(count).unwrap_or(u64::MAX).saturating_add(2))?;
        if count < 2 {
            return Ok(vec![first_end, last_end]);
        }
        let along = self.first + k as f64 * self.step;
        let last = start + (count - 1) as f64 * longest;
        let ends = |near: bool, at: Point| (near).then_some(at);
        let points = ends(start > self.spacing / 2.0, first_end)
            .into_iter()
            .chain((0..count).filter_map(|i| self.way.point(along, self.from + start + i as f64 * longest)))
            .chain(ends(length - last > self.spacing / 2.0, last_end))
            .collect();
        Ok(points)
    }

    /// The row each of `gradient`'s stops lies at, as the module's docs say: the start's row, from the
    /// row spacings it lies past row 0 along the way, no farther than the rows reach, or else minus those
    /// from it to row 0 along the gradient, no more than its length; and each stop its offset's share of
    /// the gradient's length on from it, all rounded halves to even.
    pub(crate) fn stop_rows(&self, gradient: &Gradient) -> Vec<i64> {
        let length = gradient.start.distance(gradient.end);
        let past = self.way.place(gradient.start).0 - self.first;
        let mut start = whole((past.max(0.0).min(self.reach) / self.spacing).round_ties_even());
        if start == 0 {
            start = -whole(((-past).max(0.0).min(length) / self.spacing).round_ties_even());
        }
        gradient.stops.iter().map(|stop| start.saturating_add(whole((length * stop.offset / self.spacing).round_ties_even()))).collect()
    }
}

/// A whole number of rows as an `i64`: `as` saturates past its range, and no meter allows so many.
#[allow(clippy::cast_possible_truncation)]
fn whole(rows: f64) -> i64 {
    rows as i64
}

#[cfg(test)]
mod tests {
    use stitchcraft_core::Budget;
    use stitchcraft_plan::Rgb;

    use super::*;
    use crate::design::GradientStop;

    fn p(x: f64, y: f64) -> Point {
        Point::new(x, y).unwrap()
    }

    /// The rectangle from `(x0, y0)` to `(x1, y1)`.
    fn rectangle(x0: f64, y0: f64, x1: f64, y1: f64) -> Polygon {
        Polygon { outline: vec![p(x0, y0), p(x1, y0), p(x1, y1), p(x0, y1), p(x0, y0)], holes: Vec::new() }
    }

    fn laid(piece: &Polygon, way: Way, spacing: f64, longest: f64) -> Rows {
        Rows::lay(piece, way, spacing, longest, &mut Budget::DEFAULT.meter()).unwrap().unwrap()
    }

    #[test]
    fn req_fill_grd_002_the_rows_lie_square_to_the_gradient_a_longest_stitch_past_the_piece() {
        // A gradient across a square 10 to 50 mm, rows at most 0.3 mm apart and a longest stitch of 4: they
        // reach from 6 to 54 mm along x, 48 mm in 160 equal steps of 0.3 mm, each from 9.7 to 50.3 mm down.
        let rows = laid(&rectangle(10.0, 10.0, 50.0, 50.0), Way::of_gradient(p(10.0, 30.0), p(50.0, 30.0)), 0.3, 4.0);
        assert_eq!(rows.last, 160);
        assert_eq!(rows.ends(0), Some((p(6.0, 9.7), p(6.0, 50.3))));
        let (a, b) = rows.ends(160).unwrap();
        assert!((a.x() - 54.0).abs() < 1e-9 && (a.y() - 9.7).abs() < 1e-9 && (b.y() - 50.3).abs() < 1e-9, "{a:?} {b:?}");
        // A reach that isn't whole steps takes one step more, each shorter: 48 mm at 0.29 mm is 166 steps.
        let rows = laid(&rectangle(10.0, 10.0, 50.0, 50.0), Way::of_gradient(p(10.0, 30.0), p(50.0, 30.0)), 0.29, 4.0);
        assert_eq!(rows.last, 166);
        assert!((rows.step - 48.0 / 166.0).abs() < 1e-12);
        // A gradient pointing up the canvas: the rows run left to right, row 0 below the square.
        let rows = laid(&rectangle(0.0, 0.0, 10.0, 20.0), Way::of_gradient(p(5.0, 20.0), p(5.0, 0.0)), 0.25, 3.0);
        let (a, b) = rows.ends(0).unwrap();
        assert!((a.x() + 0.25).abs() < 1e-9 && (a.y() - 23.0).abs() < 1e-9 && (b.x() - 10.25).abs() < 1e-9, "{a:?} {b:?}");
        assert_eq!(rows.last, 104);
        // A gradient of no length points along x, and so does a fade at 0°; at 90° rows run right to left.
        assert_eq!(Way::of_gradient(p(3.0, 3.0), p(3.0, 3.0)), Way { along: (1.0, 0.0), across: (0.0, 1.0) });
        assert_eq!(Way::at(0.0), Way { along: (1.0, 0.0), across: (0.0, 1.0) });
        assert_eq!(Way::at(90.0), Way { along: (0.0, 1.0), across: (-1.0, 0.0) });
        // A piece without points has no rows, and every point of the outline and every row is charged.
        assert_eq!(
            Rows::lay(&Polygon { outline: Vec::new(), holes: Vec::new() }, Way::at(0.0), 0.25, 3.0, &mut Budget::DEFAULT.meter()).unwrap(),
            None
        );
        let mut meter = Budget { max_stitches: 10, max_work: 100 }.meter();
        assert!(Rows::lay(&rectangle(0.0, 0.0, 50.0, 50.0), Way::at(0.0), 0.25, 3.0, &mut meter).is_err());
    }

    #[test]
    fn req_fill_grd_003_needle_points_lie_a_longest_stitch_apart_staggered_with_the_ends_beyond_half_a_spacing() {
        // Rows across a square 10 mm a side, each 10.5 mm long from 0.25 mm above it, stitches 4 mm long and 4
        // staggers: row 1's points lie 1, 5 and 9 mm from its first end, which is a point too, 1 mm away, and
        // so is its last, 1.5 mm past the last point.
        let rows = laid(&rectangle(0.0, 0.0, 10.0, 10.0), Way::at(0.0), 0.25, 4.0);
        let stitching = Stitching { longest: 4.0, staggers: 4.0, random: None };
        let at = |k: usize| {
            let points = rows.needle_points(k, &stitching, &SplitMix64::new(1), &mut Budget::DEFAULT.meter()).unwrap();
            points.iter().map(|q| ((q.y() + 0.25) * 1e6).round() / 1e6).collect::<Vec<_>>()
        };
        assert_eq!(at(1), [0.0, 1.0, 5.0, 9.0, 10.5]);
        assert_eq!(at(2), [0.0, 2.0, 6.0, 10.0, 10.5]);
        // Row 4's start on its first end is that point, once; row 5's 1 mm on.
        assert_eq!(at(4), [0.0, 4.0, 8.0, 10.5]);
        assert_eq!(at(5), [0.0, 1.0, 5.0, 9.0, 10.5]);
        // An end within half a row spacing of the nearest point, or at it, isn't a point of its own.
        let rows = laid(&rectangle(0.0, 0.0, 10.0, 7.5), Way::at(0.0), 0.25, 2.0);
        let points = |k: usize, staggers: f64| {
            let stitching = Stitching { longest: 2.0, staggers, random: None };
            let points = rows.needle_points(k, &stitching, &SplitMix64::new(1), &mut Budget::DEFAULT.meter()).unwrap();
            points.iter().map(|q| ((q.y() + 0.25) * 1e6).round() / 1e6).collect::<Vec<_>>()
        };
        // 8 mm long: a first point 0.125 mm, half a spacing, from the first end leaves the end out, and a last
        // point 0.125 mm from the last end leaves that out.
        assert_eq!(points(1, 16.0), [0.125, 2.125, 4.125, 6.125, 8.0]);
        assert_eq!(points(15, 16.0), [0.0, 1.875, 3.875, 5.875, 7.875]);
        assert_eq!(points(1, 16.0 * 2.0 / 2.25), [0.0, 0.140625, 2.140625, 4.140625, 6.140625, 8.0]);
        // A staggers of 0 counts as 1, which starts every row on its first end; a negative one staggers back.
        assert_eq!(points(3, 0.0), [0.0, 2.0, 4.0, 6.0, 8.0]);
        assert_eq!(points(1, -4.0), [0.0, 1.5, 3.5, 5.5, 7.5, 8.0]);
        // A row with room for fewer than 2 points is its 2 ends.
        let short = laid(&rectangle(0.0, 0.0, 10.0, 3.5), Way::at(0.0), 0.25, 4.0);
        let stitching = Stitching { longest: 4.0, staggers: 4.0, random: None };
        let ends = short.needle_points(1, &stitching, &SplitMix64::new(1), &mut Budget::DEFAULT.meter()).unwrap();
        assert_eq!(ends.len(), 2);
        assert_eq!(Some((ends[0], ends[1])), short.ends(1));
    }

    #[test]
    fn with_random_lengths_a_row_is_a_running_stitch_from_end_to_end_drawn_from_its_own_stream() {
        let rows = laid(&rectangle(0.0, 0.0, 10.0, 30.0), Way::at(0.0), 0.25, 3.0);
        let stitching = Stitching { longest: 3.0, staggers: 4.0, random: Some((0.2, 0.1, 0.3)) };
        let rng = SplitMix64::new(7);
        let sewn = |k: usize, rng: &SplitMix64| rows.needle_points(k, &stitching, rng, &mut Budget::DEFAULT.meter()).unwrap();
        let row = sewn(3, &rng);
        assert_eq!((row.first().copied(), row.last().copied()), rows.ends(3).map(|(a, b)| (Some(a), Some(b))).unwrap());
        let lengths: Vec<f64> = row.windows(2).map(|w| w[0].distance(w[1])).collect();
        assert!(lengths.iter().all(|&l| l <= 3.0 * 1.2 + 1e-9) && lengths.iter().any(|&l| (l - 3.0).abs() > 0.01), "{lengths:?}");
        // The same row of the same element sews the same, and another row or seed otherwise.
        assert_eq!(sewn(3, &rng), row);
        assert_ne!(sewn(4, &rng).iter().map(|q| q.y()).collect::<Vec<_>>(), row.iter().map(|q| q.y()).collect::<Vec<_>>());
        assert_ne!(sewn(3, &SplitMix64::new(8)), row);
    }

    /// A gradient from `start` to `end` with stops at `offsets`, all red.
    fn gradient(start: Point, end: Point, offsets: &[f64]) -> Gradient {
        let stops = offsets.iter().map(|&offset| GradientStop { offset, colour: Rgb::new(237, 23, 31), opacity: 1.0 }).collect();
        Gradient { start, end, stops }
    }

    #[test]
    fn req_fill_grd_004_each_stop_lies_at_its_offset_s_share_of_the_gradient_in_row_spacings_from_its_start() {
        // Rows from 6 mm along x, 0.3 mm apart: a gradient from 10 to 50 mm starts 4 mm, 13.33 spacings, past
        // row 0, so at row 13, and its stops at 0, ½ and 1 lie at rows 13, 80 and 146.
        let square = rectangle(10.0, 10.0, 50.0, 50.0);
        let way = Way::of_gradient(p(10.0, 30.0), p(50.0, 30.0));
        let rows = laid(&square, way, 0.3, 4.0);
        assert_eq!(rows.stop_rows(&gradient(p(10.0, 30.0), p(50.0, 30.0), &[0.0, 0.5, 1.0])), [13, 80, 146]);
        // Halves round to the even row. Rows 0.25 mm apart from 6 mm: a start 3.375 mm on, 13.5 spacings, lies
        // at row 14, and one 3.625 mm on, 14.5, at row 14 too. On a gradient 32 mm long, a stop half a
        // spacing on from the start lies at the start's row, and one 1.5 spacings on 2 rows on.
        let rows = laid(&square, way, 0.25, 4.0);
        assert_eq!(rows.stop_rows(&gradient(p(9.375, 0.0), p(41.375, 0.0), &[0.0, 1.0 / 256.0, 3.0 / 256.0])), [14, 14, 16]);
        assert_eq!(rows.stop_rows(&gradient(p(9.625, 0.0), p(41.625, 0.0), &[0.0])), [14]);
        let rows = laid(&square, way, 0.3, 4.0);
        // A start before row 0 counts back from it along the gradient, no more than its length, and a start
        // past the rows' reach counts from the rows' far end.
        assert_eq!(rows.stop_rows(&gradient(p(0.0, 0.0), p(40.0, 0.0), &[0.0, 1.0])), [-20, 113]);
        assert_eq!(rows.stop_rows(&gradient(p(-100.0, 0.0), p(-60.0, 0.0), &[0.0, 0.5, 1.0])), [-133, -66, 0]);
        assert_eq!(rows.stop_rows(&gradient(p(100.0, 0.0), p(140.0, 0.0), &[0.0, 1.0])), [160, 293]);
        // Within half a spacing past row 0, the start lies at row 0.
        assert_eq!(rows.stop_rows(&gradient(p(6.1, 0.0), p(46.1, 0.0), &[0.0, 1.0])), [0, 133]);
        // A gradient of no length puts every stop at the start's row.
        assert_eq!(rows.stop_rows(&gradient(p(30.0, 30.0), p(30.0, 30.0), &[0.0, 0.5, 1.0])), [80, 80, 80]);
        assert_eq!(rows.stop_rows(&gradient(p(0.0, 30.0), p(0.0, 30.0), &[0.0, 1.0])), [0, 0]);
    }
}
