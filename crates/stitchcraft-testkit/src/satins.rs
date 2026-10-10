//! Satin columns for the engine's satin cases: straight rails to sew, a column sewn through the satin
//! generator as an element is, and its needle points read back as pairs across it, with how far apart
//! they are as the satin design page measures it (`docs/src/design/algorithms/satin.md` › Top stitches).

use stitchcraft_core::rng::SplitMix64;
use stitchcraft_core::{Budget, Point};
use stitchcraft_engine::common::CommonParams;
use stitchcraft_engine::design::{Path, Segment, Subpath};
use stitchcraft_engine::generators::running::RunningParams;
use stitchcraft_engine::generators::satin::{SatinLengths, SatinParams, satin_stitch};
use stitchcraft_engine::normalize::satin::{Shape, recognize};
use stitchcraft_params::ParamSet;
use stitchcraft_plan::profiles::BROTHER_200X200;

use crate::designs::{p, polylines};

/// The id the columns are sewn as, which seeds their random variation as an element's id does.
pub const SATIN_ID: &str = "satin";

/// Short stitches off: for cases about where pairs go and how they are widened, whose needle points may
/// crowd on a rail. Short stitches have cases of their own.
pub const NO_SHORT_STITCHES: (&str, &str) = ("short_stitch_distance_mm", "0");

/// The needle points of the satin column `path`, sewn with the parameters `params` (Ink/Stitch keys and
/// values) as the element [`SATIN_ID`] is, on the reference machine, and its warnings.
pub fn sewn_satin(path: &Path, params: &[(&str, &str)]) -> (Vec<Point>, Vec<String>) {
    let mut meter = Budget::DEFAULT.meter();
    let Ok(Shape::Rails(satin)) = recognize(path, &mut meter).unwrap().shape else { panic!("not rails: {path:?}") };
    let set: ParamSet = params.iter().copied().collect();
    let seed = CommonParams::from_set(&set).unwrap().params.random_seed;
    let mut rng = SplitMix64::for_element(SATIN_ID, seed.unwrap_or(0));
    let stitched = satin_stitch(&satin, &SatinParams::from_set(&set).unwrap().params, satin_lengths(&set), &mut rng, &mut meter).unwrap();
    assert_eq!(stitched.runs.len(), 1);
    (stitched.runs.into_iter().flatten().collect(), stitched.warnings.iter().map(ToString::to_string).collect())
}

/// The stitch lengths of a satin column whose element sets `set`, as the engine gives them on the reference
/// machine: its shortest stitch, its longest, and the first of the running stitch's lengths for its travel.
pub fn satin_lengths(set: &ParamSet) -> SatinLengths {
    let max_stitch = CommonParams::from_set(set).unwrap().params.max_stitch_length_mm;
    let travel = RunningParams::from_set(set).unwrap().params.running_stitch_length_mm[0];
    SatinLengths { min_stitch: BROTHER_200X200.min_stitch, max_stitch, travel }
}

/// The needle points in pairs across the column: the first rail's, then the second's.
pub fn across(points: &[Point]) -> Vec<[Point; 2]> {
    points.chunks(2).map(|pair| [pair[0], pair[1]]).collect()
}

/// How far each pair is from the one before, across the column, as `satin.md` measures it: at a right
/// angle to the previous pair, at whichever end is farther.
pub fn gaps(pairs: &[[Point; 2]]) -> Vec<f64> {
    pairs
        .windows(2)
        .map(|w| {
            let ([c, d], [a, b]) = (w[0], w[1]);
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
