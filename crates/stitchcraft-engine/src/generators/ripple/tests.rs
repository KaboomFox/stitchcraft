//! The ripple stitch's unit cases.

use stitchcraft_core::Budget;

use super::*;
use crate::design::{Segment, Subpath};

fn p(x: f64, y: f64) -> Point {
    Point::new(x, y).unwrap()
}

/// A path of an open polyline through each of `parts`.
fn polylines(parts: &[&[(f64, f64)]]) -> Path {
    let subpath = |points: &&[(f64, f64)]| Subpath {
        start: p(points[0].0, points[0].1),
        segments: points[1..].iter().map(|&(x, y)| Segment::Line(p(x, y))).collect(),
        closed: false,
    };
    Path { subpaths: parts.iter().map(subpath).collect() }
}

/// A ripple with `settings`, for a machine whose shortest stitch is 0.3 mm, aiming at `target`.
fn settings(settings: &[(&str, &str)], target: Option<Point>) -> Ripple {
    let set: ParamSet = settings.iter().copied().collect();
    Ripple {
        ripple: RippleParams::from_set(&set).unwrap().params,
        running: RunningParams::from_set(&set).unwrap().params,
        passes: RepeatParams::from_set(&set).unwrap().params,
        satin: SatinParams::from_set_as(&set, StitchType::RippleStitch).unwrap().params,
        min_stitch: Mm::new(0.3).unwrap(),
        target,
        unapplied: unapplied(&set),
        clipped: false,
    }
}

fn sewn(path: &Path, with: &[(&str, &str)], target: Option<Point>) -> Stitched {
    ripple_stitch(path, &settings(with, target), &SplitMix64::new(7), &mut Budget::DEFAULT.meter()).unwrap()
}

fn show(path: &Path, with: &[(&str, &str)], target: Option<Point>) {
    let got = sewn(path, with, target);
    eprintln!("{:?}", got.warnings.iter().map(ToString::to_string).collect::<Vec<_>>());
    for run in &got.runs {
        let (xs, ys): (Vec<f64>, Vec<f64>) = run.iter().map(|q| (q.x(), q.y())).unzip();
        let mid = |v: &[f64]| (v.iter().copied().fold(f64::MAX, f64::min) + v.iter().copied().fold(f64::MIN, f64::max)) / 2.0;
        let (cx, cy) = (mid(&xs), mid(&ys));
        eprintln!("{} stitches", run.len());
        eprintln!("{}", run.iter().map(|q| format!("({:.2},{:.2})", q.x() - cx, q.y() - cy)).collect::<Vec<_>>().join(" "));
    }
}

#[test]
fn probe() {
    let which = std::env::var("PROBE").unwrap_or_default();
    match which.as_str() {
        "a" => show(&polylines(&[&[(5.0, 10.0), (25.0, 10.0)], &[(5.0, 14.0), (25.0, 14.0)], &[(15.0, 9.5), (15.0, 14.5)]]), &[("line_count", "4")], None),
        "b" => {
            let k = 10.0 * 0.552_284_749_8;
            let (c, r) = ((30.0, 30.0), 10.0);
            let circle = Path {
                subpaths: vec![Subpath {
                    start: p(c.0 + r, c.1),
                    segments: vec![
                        Segment::Cubic(p(c.0 + r, c.1 + k), p(c.0 + k, c.1 + r), p(c.0, c.1 + r)),
                        Segment::Cubic(p(c.0 - k, c.1 + r), p(c.0 - r, c.1 + k), p(c.0 - r, c.1)),
                        Segment::Cubic(p(c.0 - r, c.1 - k), p(c.0 - k, c.1 - r), p(c.0, c.1 - r)),
                        Segment::Cubic(p(c.0 + k, c.1 - r), p(c.0 + r, c.1 - k), p(c.0 + r, c.1)),
                    ],
                    closed: true,
                }],
            };
            show(&circle, &[("line_count", "5")], None)
        }
        "c" | "d" => {
            let wave = Path {
                subpaths: vec![Subpath {
                    start: p(10.0, 30.0),
                    segments: vec![Segment::Cubic(p(20.0, 20.0), p(30.0, 40.0), p(50.0, 30.0))],
                    closed: false,
                }],
            };
            let with: &[(&str, &str)] =
                if which == "c" { &[("line_count", "5"), ("join_style", "1")] } else { &[("line_count", "5"), ("staggers", "3")] };
            show(&wave, with, Some(p(30.0, 45.0)))
        }
        _ => {}
    }
}
