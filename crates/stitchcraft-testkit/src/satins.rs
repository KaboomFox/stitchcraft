//! Satin columns for the engine's satin cases: straight rails to sew, a column sewn through the satin
//! generator as an element is, and its needle points read back as pairs across it, with how far apart
//! they are as the satin design page measures it (`docs/src/design/algorithms/satin.md` › Top stitches).

use stitchcraft_core::rng::SplitMix64;
use stitchcraft_core::{Budget, Point};
use stitchcraft_engine::common::CommonParams;
use stitchcraft_engine::design::Path;
use stitchcraft_engine::generators::satin::{SatinParams, satin_stitch};
use stitchcraft_engine::normalize::satin::{Shape, recognize};
use stitchcraft_params::ParamSet;

use crate::designs::polylines;

/// The id the columns are sewn as, which seeds their random variation as an element's id does.
pub const SATIN_ID: &str = "satin";

/// The needle points of the satin column `path`, sewn with the parameters `params` (Ink/Stitch keys and
/// values) as the element [`SATIN_ID`] is, and its warnings.
pub fn sewn_satin(path: &Path, params: &[(&str, &str)]) -> (Vec<Point>, Vec<String>) {
    let mut meter = Budget::DEFAULT.meter();
    let Ok(Shape::Rails(satin)) = recognize(path, &mut meter).unwrap().shape else { panic!("not rails: {path:?}") };
    let set: ParamSet = params.iter().copied().collect();
    let mut rng = SplitMix64::for_element(SATIN_ID, CommonParams::from_set(&set).unwrap().params.random_seed.unwrap_or(0));
    let stitched = satin_stitch(&satin, &SatinParams::from_set(&set).unwrap().params, &mut rng, &mut meter).unwrap();
    assert_eq!(stitched.runs.len(), 1);
    (stitched.runs.into_iter().flatten().collect(), stitched.warnings.iter().map(ToString::to_string).collect())
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

/// Two straight rails along x, `length` long and `width` apart, with rungs across them at `rungs`.
pub fn ladder(length: f64, width: f64, rungs: &[f64]) -> Path {
    let lower = [(0.0, 0.0), (length, 0.0)];
    let upper = [(0.0, width), (length, width)];
    let rungs: Vec<[(f64, f64); 2]> = rungs.iter().map(|&x| [(x, -1.0), (x, width + 1.0)]).collect();
    let mut parts: Vec<&[(f64, f64)]> = vec![&lower, &upper];
    parts.extend(rungs.iter().map(|rung| &rung[..]));
    polylines(&parts)
}
