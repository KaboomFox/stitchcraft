//! The warnings every satin method gives about how its stitches lie (`REQ-SAT-022`, `REQ-SAT-023`):
//! stitches that lean far along the column, which a rung would set straight (`SC-W0208`), and top
//! stitches longer than 12 mm (`SC-W0209`). Ink/Stitch gives neither.

// Test code may unwrap, panic and index (clippy.toml allows it in tests).
#![allow(clippy::unwrap_used, clippy::panic)]

use stitchcraft_core::{Code, Diagnostic, Point};
use stitchcraft_engine::design::Path;
use stitchcraft_engine::generators::{Approach, Neighbours};
use stitchcraft_testkit::designs::{p, polylines};
use stitchcraft_testkit::satins::{NO_SHORT_STITCHES, across, ladder, satin_diagnostics};

/// The satin methods StitchCraft sews.
const METHODS: [&str; 2] = ["satin_column", "e_stitch"];

/// The warnings of the satin column `path` sewn by `method` with `params`, and no short stitches.
fn warnings(path: &Path, method: &str, params: &[(&str, &str)]) -> Vec<Diagnostic> {
    let mut params = params.to_vec();
    params.extend([("satin_method", method), NO_SHORT_STITCHES]);
    satin_diagnostics(path, &params, &Neighbours::default()).1
}

fn codes(warnings: &[Diagnostic]) -> Vec<Code> {
    warnings.iter().map(|d| d.code).collect()
}

/// Where `warning` is, as x and y.
fn place(warning: &Diagnostic) -> (f64, f64) {
    let at: Point = warning.at.unwrap();
    (at.x(), at.y())
}

#[test]
fn req_sat_022_stitches_that_lean_along_the_column_say_where_to_add_a_rung() {
    // Rails of 2 nodes, the second 8 mm ahead of the first: every stitch goes 8 mm along the column for 4
    // mm across it, 63° from square.
    let ahead = polylines(&[&[(0.0, 0.0), (20.0, 0.0)], &[(8.0, 4.0), (28.0, 4.0)]]);
    let (points, found) = satin_diagnostics(&ahead, &[NO_SHORT_STITCHES], &Neighbours::default());
    assert_eq!(codes(&found), [Code::SatinStitchesLean], "{found:?}");
    let pairs = across(&points);
    assert!(found[0].message.starts_with(&format!("{} stitches", pairs.len())) && found[0].message.contains("63.4°"), "{}", found[0].message);
    // At the middle of the column's middle pair, the later of 2: with nothing to compensate, the pairs
    // sewn are those that cut the column.
    let [a, b] = pairs[pairs.len() / 2];
    assert_eq!(found[0].at, Some(a.lerp(b, 0.5)));
    // An E-stitch places the same pairs, and leans alike.
    assert_eq!(warnings(&ahead, "e_stitch", &[]), found);
}

#[test]
fn req_sat_022_each_stretch_of_leaning_stitches_gets_its_own_warning() {
    // The middle nodes pair up square to the column. Towards each end the second rail's nodes lie 4 mm
    // nearer the middle, and the stitches lean up to 56.3°, more than 45° where they go more than 4 mm
    // along the column for their 4 mm across: in the first and last third.
    let tapered = polylines(&[&[(0.0, 0.0), (10.0, 0.0), (20.0, 0.0)], &[(6.0, 4.0), (10.0, 4.0), (14.0, 4.0)]]);
    for method in METHODS {
        let found = warnings(&tapered, method, &[]);
        assert_eq!(codes(&found), [Code::SatinStitchesLean, Code::SatinStitchesLean], "{method}: {found:?}");
        assert!(found.iter().all(|d| d.message.contains("56.3°")), "{found:?}");
        let (first, last) = (place(&found[0]), place(&found[1]));
        assert!(first.0 < 6.0 && last.0 > 14.0, "{method}: at {first:?} and {last:?}");
    }
    // Square stitches give none.
    for method in METHODS {
        assert_eq!(warnings(&ladder(20.0, 4.0, &[]), method, &[]), []);
    }
}

#[test]
fn req_sat_022_stitches_at_exactly_45_degrees_do_not_lean_too_far() {
    // Rails 4 mm apart, the second 4 mm ahead: every stitch goes as far along the column as across it, and
    // rounding tips none of them over, along x or turned 30°. A tenth of a degree more is too far, and the
    // whole column is one stretch.
    let (cos, sin) = (3.0_f64.sqrt() / 2.0, 0.5);
    for length in [20.0, 100.0] {
        for turned in [false, true] {
            let at = |x: f64, y: f64| if turned { (x * cos - y * sin, x * sin + y * cos) } else { (x, y) };
            let ahead = |by: f64| polylines(&[&[at(0.0, 0.0), at(length, 0.0)], &[at(by, 4.0), at(length + by, 4.0)]]);
            for method in METHODS {
                assert_eq!(warnings(&ahead(4.0), method, &[]), [], "{method}, {length} mm, turned: {turned}");
                let found = warnings(&ahead(4.01), method, &[]);
                assert_eq!(codes(&found), [Code::SatinStitchesLean], "{method}, {length} mm, turned: {turned}");
                assert!(found[0].message.contains("45.1°"), "{}", found[0].message);
            }
        }
    }
}

#[test]
fn req_sat_022_square_stitches_where_a_rail_steps_out_do_not_lean() {
    // The second rail steps out from 4 mm to 8 mm over 0.4 mm, and every stitch still goes straight across:
    // a rung there would change nothing. So does a column that flares on one side over its last 0.4 mm.
    let step = polylines(&[&[(0.0, 0.0), (10.0, 0.0), (10.4, 0.0), (20.0, 0.0)], &[(0.0, 4.0), (10.0, 4.0), (10.4, 8.0), (20.0, 8.0)]]);
    let flare = polylines(&[&[(0.0, 0.0), (19.6, 0.0), (20.0, 0.0)], &[(0.0, 10.4), (19.6, 10.4), (20.0, 11.5)]]);
    for method in METHODS {
        assert_eq!(warnings(&step, method, &[]), [], "{method}");
        assert_eq!(warnings(&flare, method, &[]), [], "{method}");
    }
}

#[test]
fn req_sat_023_a_top_stitch_longer_than_12_mm_snags_and_says_so() {
    // A column 13 mm wide. The satin's slanted stitches back are its longest, a little over 13 mm.
    let wide = ladder(10.0, 13.0, &[]);
    for (method, length) in [("satin_column", "13.01 mm"), ("e_stitch", "13 mm")] {
        let found = warnings(&wide, method, &[]);
        assert_eq!(codes(&found), [Code::SatinStitchLong], "{method}: {found:?}");
        assert!(found[0].message.contains(length), "{}", found[0].message);
        let (_, y) = place(&found[0]);
        assert!((y - 6.5).abs() < 1e-9, "{method}: at the stitch's middle, y = {y}");
        // A longest stitch that splits them leaves none.
        assert_eq!(warnings(&wide, method, &[("max_stitch_length_mm", "7")]), [], "{method}");
    }
    // Exactly 12 mm is not longer than 12 mm.
    assert_eq!(warnings(&ladder(10.0, 12.0, &[]), "e_stitch", &[]), []);
    // An E-stitch's spine counts too: spikes 10 mm apart, give or take half that at random, which puts some
    // more than 12 mm apart. The warning names the longest stitch, the first of several as long.
    let params = [("satin_method", "e_stitch"), ("zigzag_spacing_mm", "10"), ("random_zigzag_spacing_percent", "50"), NO_SHORT_STITCHES];
    let (points, far) = satin_diagnostics(&ladder(80.0, 4.0, &[]), &params, &Neighbours::default());
    let stitches: Vec<(f64, Point)> = points.windows(2).map(|w| (w[0].distance(w[1]), w[0].lerp(w[1], 0.5))).collect();
    let longest = stitches.iter().fold(stitches[0], |kept, &s| if s.0 > kept.0 { s } else { kept });
    assert!(longest.0 > 12.0, "some spine stitch is longer than 12 mm: {longest:?}");
    assert_eq!(codes(&far), [Code::SatinStitchLong], "{far:?}");
    assert_eq!(far[0].at, Some(longest.1));
}

#[test]
fn req_sat_023_a_stitch_the_end_cuts_in_2_is_sewn_as_2() {
    // A column 10.4 mm wide that flares to 11.5 mm over its last 0.4 mm, widened 0.3 mm on each side: only
    // its last stitch across is longer than 12 mm, 12.1 mm.
    let flare = polylines(&[&[(0.0, 0.0), (19.6, 0.0), (20.0, 0.0)], &[(0.0, 10.4), (19.6, 10.4), (20.0, 11.5)]]);
    let params = [("pull_compensation_mm", "0.3"), NO_SHORT_STITCHES];
    let (_, alone) = satin_diagnostics(&flare, &params, &Neighbours::default());
    assert_eq!(codes(&alone), [Code::SatinStitchLong], "{alone:?}");
    assert!(alone[0].message.contains("12.1 mm"), "{}", alone[0].message);
    // Before an element near its flared corner, the column ends there, and the end cuts that stitch in 2.
    // No stitch it sews is longer than 12 mm, and it names none.
    let next = Neighbours { needle: None, next: Some(Approach::Point(p(21.0, 12.0))) };
    let (points, cut) = satin_diagnostics(&flare, &params, &next);
    let longest = points.windows(2).map(|w| w[0].distance(w[1])).fold(0.0, f64::max);
    assert!(longest < 12.0, "the longest stitch sewn is {longest} mm");
    assert_eq!(cut, []);
}

#[test]
fn req_sat_023_the_underlays_and_travel_do_not_count() {
    // A centre walk of 15 mm stitches under a column 4 mm wide.
    let found =
        warnings(&ladder(30.0, 4.0, &[]), "satin_column", &[("center_walk_underlay", "true"), ("center_walk_underlay_stitch_length_mm", "15")]);
    assert_eq!(found, []);
}

#[test]
fn diag_sc_w0208_leaning_stitches_name_their_place() {
    let ahead = polylines(&[&[(0.0, 0.0), (20.0, 0.0)], &[(8.0, 4.0), (28.0, 4.0)]]);
    let found = warnings(&ahead, "satin_column", &[]);
    assert_eq!(codes(&found), [Code::SatinStitchesLean]);
    assert!(found[0].fix.is_some(), "it says what to do");
}

#[test]
fn diag_sc_w0209_a_long_stitch_names_its_place() {
    let found = warnings(&ladder(10.0, 13.0, &[]), "satin_column", &[]);
    assert_eq!(codes(&found), [Code::SatinStitchLong]);
    assert!(found[0].fix.is_some(), "it says what to do");
}
