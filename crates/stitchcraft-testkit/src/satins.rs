//! Satin columns for the engine's satin cases: straight rails to sew, a column sewn through the satin
//! generator as an element is, and its needle points read back as pairs across it, with how far apart
//! they are as the satin design page measures it (`docs/src/design/algorithms/satin.md` › Top stitches).

use stitchcraft_core::rng::SplitMix64;
use stitchcraft_core::{Budget, Diagnostic, Point};
use stitchcraft_engine::common::CommonParams;
use stitchcraft_engine::design::{DesignSettings, Path, Segment, Subpath};
use stitchcraft_engine::generate::shortest_stitch;
use stitchcraft_engine::generators::Neighbours;
use stitchcraft_engine::generators::pull::PullParams;
use stitchcraft_engine::generators::running::RunningParams;
use stitchcraft_engine::generators::satin::{SatinLengths, SatinParams, satin_stitch};
use stitchcraft_engine::normalize::satin::{Shape, recognize};
use stitchcraft_params::{ParamSet, StitchType};
use stitchcraft_plan::profiles::REFERENCE;

use crate::designs::{p, polylines};

/// The id the columns are sewn as, which seeds their random variation as an element's id does.
pub const SATIN_ID: &str = "satin";

/// Short stitches off: for cases about where pairs go and how they are widened, whose needle points may
/// crowd on a rail. Short stitches have cases of their own.
pub const NO_SHORT_STITCHES: (&str, &str) = ("short_stitch_distance_mm", "0");

/// The needle points of the satin column `path`, sewn with the parameters `params` (Ink/Stitch keys and
/// values) as the element [`SATIN_ID`] is, on the reference machine, with no element before or after it,
/// and its warnings.
pub fn sewn_satin(path: &Path, params: &[(&str, &str)]) -> (Vec<Point>, Vec<String>) {
    sewn_between(path, params, &Neighbours::default())
}

/// The needle points of the satin column `path`, sewn as [`sewn_satin`] sews it, between `neighbours`, and
/// its warnings.
pub fn sewn_between(path: &Path, params: &[(&str, &str)], neighbours: &Neighbours) -> (Vec<Point>, Vec<String>) {
    let (points, warnings) = satin_diagnostics(path, params, neighbours);
    (points, warnings.iter().map(ToString::to_string).collect())
}

/// The needle points of the satin column `path`, sewn as [`sewn_satin`] sews it, between `neighbours`, and
/// its warnings as diagnostics, with their places.
pub fn satin_diagnostics(path: &Path, params: &[(&str, &str)], neighbours: &Neighbours) -> (Vec<Point>, Vec<Diagnostic>) {
    let mut meter = Budget::DEFAULT.meter();
    let Ok(Shape::Rails(satin)) = recognize(path, &mut meter).unwrap().shape else { panic!("not rails: {path:?}") };
    let set: ParamSet = params.iter().copied().collect();
    let seed = CommonParams::from_set_as(&set, SATIN).unwrap().params.random_seed;
    let mut rng = SplitMix64::for_element(SATIN_ID, seed);
    let params = SatinParams::from_set_as(&set, SATIN).unwrap().params;
    let pull = PullParams::from_set_as(&set, SATIN).unwrap().params;
    let stitched = satin_stitch(&satin, &params, &pull, satin_lengths(&set), neighbours, &mut rng, &mut meter).unwrap();
    assert_eq!(stitched.runs.len(), 1);
    (stitched.runs.into_iter().flatten().collect(), stitched.warnings)
}

/// The needle points of a straight column `length` long and `width` wide ([`ladder`], with no rungs), sewn
/// by the satin method `method` with `params`, as points to compare with [`near`]. A column that warns is
/// a case's mistake.
pub fn sewn_ladder<'a>(length: f64, width: f64, method: &'a str, params: &[(&'a str, &'a str)]) -> Vec<(f64, f64)> {
    let mut params = params.to_vec();
    params.push(("satin_method", method));
    let (points, warnings) = sewn_satin(&ladder(length, width, &[]), &params);
    assert!(warnings.is_empty(), "{warnings:?}");
    xy(&points)
}

/// `points` as (x, y), to compare with [`near`].
pub fn xy(points: &[Point]) -> Vec<(f64, f64)> {
    points.iter().map(|q| (q.x(), q.y())).collect()
}

/// Whether `got` are the points `want`, but for rounding.
pub fn near(got: &[(f64, f64)], want: &[(f64, f64)]) -> bool {
    got.len() == want.len() && got.iter().zip(want).all(|(g, w)| (g.0 - w.0).abs() < 1e-9 && (g.1 - w.1).abs() < 1e-9)
}

/// The stitch lengths of a satin column whose element sets `set`, as the engine gives them on the reference
/// machine in a design with the usual settings: its shortest stitch (the element's, when longer than the
/// machine's), its longest, the first of the running stitch's lengths and its tolerance, and its jump length.
pub fn satin_lengths(set: &ParamSet) -> SatinLengths {
    let common = CommonParams::from_set_as(set, SATIN).unwrap().params;
    let running = RunningParams::from_set_as(set, SATIN).unwrap().params;
    let settings = DesignSettings::default();
    SatinLengths {
        min_stitch: shortest_stitch(common.min_stitch_length_mm, &settings, REFERENCE),
        max_stitch: common.max_stitch_length_mm,
        travel: running.running_stitch_length_mm[0],
        tolerance: running.running_stitch_tolerance_mm,
        jump: common.jump_length(settings.collapse_len),
    }
}

/// The stitch type the columns are sewn as, whose family's defaults their settings take.
const SATIN: StitchType = StitchType::SatinColumn;

/// The needle points in pairs across the column: the first rail's, then the second's. A column's points come
/// in pairs, so an odd count is a test's mistake.
pub fn across(points: &[Point]) -> Vec<[Point; 2]> {
    assert!(points.len().is_multiple_of(2), "{} needle points do not make pairs", points.len());
    points.chunks(2).map(|pair| [pair[0], pair[1]]).collect()
}

/// How far each pair is from the one before, across the column, as `satin.md` measures it: at a right
/// angle to the previous pair, at whichever end is farther. A pair with no width has no right angle to
/// measure at, so one before another is a test's mistake.
pub fn gaps(pairs: &[[Point; 2]]) -> Vec<f64> {
    pairs
        .windows(2)
        .map(|w| {
            let ([c, d], [a, b]) = (w[0], w[1]);
            assert!(c.distance(d) > 0.0, "the pair at {c:?} has no width to measure the next one across");
            let (x, y) = (d.x() - c.x(), d.y() - c.y());
            let across = |p: Point, q: Point| ((q.y() - p.y()) * x - (q.x() - p.x()) * y).abs() / c.distance(d);
            across(a, c).max(across(b, d))
        })
        .collect()
}

/// A quarter ring about the origin, from the x axis to the y axis: rails of radius `outer` and `inner`,
/// each one cubic curve.
pub fn quarter_ring(outer: f64, inner: f64) -> Path {
    let arc = |radius: f64| {
        // 4(√2 − 1)/3 of the radius, the usual distance to the control points of a cubic that draws a
        // quarter circle.
        let k = radius * 0.552_284_749_830_793_4;
        Subpath { start: p(radius, 0.0), segments: vec![Segment::Cubic(p(radius, k), p(k, radius), p(0.0, radius))], closed: false }
    };
    Path { subpaths: vec![arc(outer), arc(inner)] }
}

/// Two straight rails along x, `length` long and `width` apart, with rungs across them at `rungs`.
pub fn ladder(length: f64, width: f64, rungs: &[f64]) -> Path {
    let lower = [(0.0, 0.0), (length, 0.0)];
    let upper = [(0.0, width), (length, width)];
    let rungs: Vec<[(f64, f64); 2]> = rungs.iter().map(|&x| [(x, -1.0), (x, width + 1.0)]).collect();
    let mut parts: Vec<&[(f64, f64)]> = vec![&lower, &upper];
    parts.extend(rungs.iter().map(|rung| &rung[..]));
    polylines(&parts)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[should_panic(expected = "do not make pairs")]
    fn an_odd_count_of_needle_points_makes_no_pairs() {
        across(&[p(0.0, 0.0), p(0.0, 1.0), p(1.0, 0.0)]);
    }

    #[test]
    #[should_panic(expected = "has no width")]
    fn a_pair_with_no_width_measures_no_gap() {
        gaps(&[[p(0.0, 0.0), p(0.0, 0.0)], [p(1.0, 0.0), p(1.0, 1.0)]]);
    }

    #[test]
    fn a_satin_s_shortest_stitch_is_the_element_s_when_longer_than_the_machine_s() {
        let lengths = |set: &[(&str, &str)]| satin_lengths(&set.iter().copied().collect()).min_stitch.get();
        assert_eq!([lengths(&[]), lengths(&[("min_stitch_length_mm", "1.5")]), lengths(&[("min_stitch_length_mm", "0.1")])], [0.3, 1.5, 0.3]);
    }
}
