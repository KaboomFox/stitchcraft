//! TS-17A — cross stitch (M10.1), for MC-12: simple crosses on grids of 2 and 3 mm, with 2 and 4 threads,
//! plain and flipped, and a letter whose slanted edges cut their cells in half. M10.2 adds its other crosses
//! below.
//!
//! Drawn as a design (`designed`), so the engine sews each sample as a design's element, with Ink/Stitch's
//! defaults but for what the sample sets. Each sample is trimmed after, and each row is a thread of its own:
//! the reference machine leaves a jump's thread on the fabric (MC-1), which a row sewn after it would sew over.
//!
//! - **Top row**, in red: squares 24 mm wide, on grids of 2 mm with 2 and 4 threads, then of 3 mm with 2 and
//!   4, Ink/Stitch's default.
//! - **Bottom row**, in blue: the 2 mm square with 2 threads and the 3 mm one with 4, flipped; then a K 24 mm
//!   tall on a 2 mm grid with 2 threads, grown by 0.1 mm, as Ink/Stitch's cross-stitch fonts sew their
//!   letters. Its stem lies on the grid's lines, and its arms slant at 45° through the cells' corners, so each
//!   cell along an arm is half covered, and sews a cross by the coverage rule's slack.
//!
//! Every sample lies on the canvas's grid, Ink/Stitch's default, each square's corner and the K's on a line of
//! both grids: a multiple of 6 mm from the origin. So the squares sew whole cells, and the sheet sews the same
//! mirrored or turned half round about the origin. VectorCraft's Test Sheet filter moves it by multiples of
//! 6 mm for the same reason ([`TestSheet::step`](super::TestSheet::step)).

use super::designed::{Drawing, polygon};
use super::sketch::SheetError;
use super::{BLUE, RED, thread};

/// The side of each square, in millimetres: a whole number of cells of both grids.
const SIDE: f64 = 24.0;
/// How far apart the samples of a row lie, from corner to corner, and the rows (mm): multiples of 6 mm.
const PITCH: f64 = 30.0;

/// Each sample's own settings: cross stitch, trimmed after.
const CROSS: [(&str, &str); 2] = [("fill_method", "cross_stitch"), ("trim_after", "true")];

/// A sample's own Ink/Stitch settings.
type Own = &'static [(&'static str, &'static str)];

/// The top row's squares: their names and their own settings.
const PLAIN: [(&str, Own); 4] = [
    ("simple-2-2", &[("pattern_size_mm", "2"), ("cross_thread_count", "2")]),
    ("simple-2-4", &[("pattern_size_mm", "2")]),
    ("simple-3-2", &[("cross_thread_count", "2")]),
    ("simple-3-4", &[]),
];

/// The bottom row's flipped squares: their names and their own settings.
const FLIPPED: [(&str, Own); 2] = [
    ("flipped-2-2", &[("cross_stitch_method", "simple_cross_flipped"), ("pattern_size_mm", "2"), ("cross_thread_count", "2")]),
    ("flipped-3-4", &[("cross_stitch_method", "simple_cross_flipped")]),
];

/// The letter's settings, as Ink/Stitch's cross-stitch fonts set theirs: a 2 mm grid, 2 threads, grown by
/// 0.1 mm.
const LETTER: Own = &[("pattern_size_mm", "2"), ("cross_thread_count", "2"), ("expand_mm", "0.1")];

/// The K's outline from its top left corner, 16 mm wide and 24 tall: a stem 4 mm wide, and arms 4 mm wide
/// across that slant at 45° from its middle, their edges through the corners of a 2 mm grid.
const K: [(f64, f64); 11] =
    [(0.0, 0.0), (4.0, 0.0), (4.0, 8.0), (12.0, 0.0), (16.0, 0.0), (4.0, 12.0), (16.0, 24.0), (12.0, 24.0), (4.0, 16.0), (4.0, 24.0), (0.0, 24.0)];

/// Draws TS-17A.
pub(super) fn crosses() -> Result<Drawing, SheetError> {
    let mut d = Drawing::new("ts17a");
    let red = thread(RED)?;
    for (column, (name, own)) in PLAIN.iter().enumerate() {
        d.fill(name, vec![square(column as f64 * PITCH, 0.0)?], &red, &[&CROSS[..], own].concat())?;
    }
    let blue = thread(BLUE)?;
    for (column, (name, own)) in FLIPPED.iter().enumerate() {
        d.fill(name, vec![square(column as f64 * PITCH, PITCH)?], &blue, &[&CROSS[..], own].concat())?;
    }
    let letter: Vec<(f64, f64)> = K.iter().map(|&(x, y)| (x + 2.0 * PITCH, y + PITCH)).collect();
    d.fill("letter", vec![polygon(&letter)?], &blue, &[&CROSS[..], LETTER].concat())?;
    Ok(d)
}

/// The square `SIDE` wide from `(x, y)`.
fn square(x: f64, y: f64) -> Result<crate::design::Subpath, SheetError> {
    Ok(polygon(&[(x, y), (x + SIDE, y), (x + SIDE, y + SIDE), (x, y + SIDE)])?)
}

#[cfg(test)]
mod tests {
    use std::collections::{BTreeMap, BTreeSet};

    use stitchcraft_plan::{Role, StitchKind};

    /// The top stitches' needle points of each sample of TS-17A, by its name, as the sheet's plan sews them.
    fn samples() -> BTreeMap<String, Vec<(f64, f64)>> {
        let plan = crate::testsheets::find("TS-17A").unwrap().plan().unwrap();
        let mut samples: BTreeMap<String, Vec<(f64, f64)>> = BTreeMap::new();
        for s in plan.stitches().filter(|s| s.kind == StitchKind::Normal && s.origin.role == Role::Top) {
            let element = &plan.elements[s.origin.element.unwrap().index()];
            let name = element.as_str().strip_prefix("ts17a:").unwrap().to_string();
            samples.entry(name).or_default().push((s.at.x(), s.at.y()));
        }
        samples
    }

    /// The cells, by their top left corners in cells, of each cross that `points` sews on a grid of `size`
    /// mm: the cells both of whose diagonals it sews whole.
    fn crossed(points: &[(f64, f64)], size: f64) -> BTreeSet<(i64, i64)> {
        let mut falling = BTreeSet::new();
        let mut rising = BTreeSet::new();
        for w in points.windows(2) {
            let ((ax, ay), (bx, by)) = (w[0], w[1]);
            if ((ax - bx).abs() - size).abs() < 0.01 && ((ay - by).abs() - size).abs() < 0.01 {
                let cell = (((ax.min(bx)) / size).round() as i64, ((ay.min(by)) / size).round() as i64);
                if (bx - ax) * (by - ay) > 0.0 {
                    falling.insert(cell)
                } else {
                    rising.insert(cell)
                };
            }
        }
        falling.intersection(&rising).copied().collect()
    }

    /// The least x and y of `points`, and how wide and tall they reach.
    fn bounds(points: &[(f64, f64)]) -> [f64; 4] {
        let (xs, ys) = (points.iter().map(|p| p.0), points.iter().map(|p| p.1));
        let ([x0, y0], [x1, y1]) = (
            [xs.clone().fold(f64::INFINITY, f64::min), ys.clone().fold(f64::INFINITY, f64::min)],
            [xs.fold(f64::NEG_INFINITY, f64::max), ys.fold(f64::NEG_INFINITY, f64::max)],
        );
        [x0, y0, x1 - x0, y1 - y0]
    }

    #[test]
    fn ts17a_sews_a_cross_in_each_cell_of_its_squares_and_of_the_letter_s_arms() {
        let samples = samples();
        // Each square sews a cross in each of its cells, and no other: 144 on the 2 mm grid and 64 on the 3 mm
        // one, in rows and columns of 12 and 8. Its needle points span it, 24 mm a side, where the sheet puts
        // it: the plan is centred on the hoop, so each is placed from the first square's corner.
        let [x0, y0, ..] = bounds(&samples["simple-2-2"]);
        for (name, size, side, [x, y]) in [
            ("simple-2-2", 2.0, 12, [0.0, 0.0]),
            ("simple-2-4", 2.0, 12, [30.0, 0.0]),
            ("simple-3-2", 3.0, 8, [60.0, 0.0]),
            ("simple-3-4", 3.0, 8, [90.0, 0.0]),
            ("flipped-2-2", 2.0, 12, [0.0, 30.0]),
            ("flipped-3-4", 3.0, 8, [30.0, 30.0]),
        ] {
            let cells = crossed(&samples[name], size);
            let (columns, rows): (BTreeSet<i64>, BTreeSet<i64>) = (cells.iter().map(|c| c.0).collect(), cells.iter().map(|c| c.1).collect());
            assert_eq!((cells.len(), columns.len(), rows.len()), (side * side, side as usize, side as usize), "{name}");
            let [left, top, width, height] = bounds(&samples[name]);
            assert!(near([left - x0, top - y0, width, height], [x, y, 24.0, 24.0]), "{name}: {:?}", [left - x0, top - y0, width, height]);
        }
        // The K: its stem's 2 columns of 12 cells, and along each arm 15: the 5 it covers whole, and the 10 its
        // slanted edges cut in half, which its 0.1 mm of growth covers by 57 %. It spans 16 by 24 mm, right of
        // the flipped squares.
        assert_eq!(crossed(&samples["letter"], 2.0).len(), 24 + 2 * 15);
        let [left, top, width, height] = bounds(&samples["letter"]);
        assert!(near([left - x0, top - y0, width, height], [60.0, 30.0, 16.0, 24.0]), "{:?}", [left - x0, top - y0, width, height]);
    }

    /// Whether `a` and `b` agree within a thousandth of a millimetre.
    fn near(a: [f64; 4], b: [f64; 4]) -> bool {
        a.iter().zip(b).all(|(a, b)| (a - b).abs() < 0.001)
    }
}
