//! TS-13F — how thick a design may sew: a ladder of squares of 1 to 6 fills on each other, bare and under a
//! satin outline, and fills cut into small patches that reach under each other.
//!
//! The measure of how thick a design sews (`crate::thick`) warns at 3 fills of thread over one spot and at 6,
//! starting values from public advice that no sew-out has confirmed. The sheet lets a sew-out set them: the
//! first square that breaks thread or thumps is the limit that fails.
//!
//! Drawn as a design (`designed`), none of it digitized automatically, so that each square sews exactly the
//! fills its place in the ladder names, whatever the rules of automatic digitizing become. Every square and
//! block lies on one open layer of rows, sewn as the base layer of a design digitized automatically is by
//! default ([`BASE`]), and its fills have no underlay of their own, as that design's fills have none.
//!
//! - **Top row:** squares 15 mm a side of 1 to 6 fills, left to right, each fill's rows turned 30° from
//!   those of the fill under it.
//! - **Middle row:** the same squares, sewn right to left, each with a satin outline 2 mm wide along its
//!   edge, sewn after all the fills in a thread of its own.
//! - **Bottom row:** 3 blocks 12 mm a side, each one fill cut into square patches 1.5, 2 and 3 mm across.
//!   Each patch reaches 1 mm under the patch sewn after it in its row and under the one below it, as an
//!   area of a traced photo reaches under its neighbours. A block's rows of patches sew top to bottom, left
//!   to right and right to left in turn.

use super::designed::{Drawing, polygon};
use super::sketch::SheetError;
use super::{BLACK, BLUE, thread};

/// The most fills on one square, and the squares in a row.
pub const MOST: usize = 6;
/// How wide each square is (mm).
pub const SIDE: f64 = 15.0;
/// The space between the squares of a row (mm). With 5 mm the sheet's left side, drawn into a Letter page
/// by VectorCraft's Test Sheet filter, lay at 49 mm, which came back from the document's points 7e-15 mm
/// off. A fill's rows lie on quarters of a millimetre, halfway between 2 of a PES file's tenths, so that
/// much moved a needle point of every upright row by 0.1 mm against `stitch testsheet`'s file. With 4 mm
/// the 2 files are the same, which the plug-in's case of the sheets checks.
const GAP: f64 = 4.0;
/// How wide the satin outline of a square of the middle row is (mm).
pub const OUTLINE: f64 = 2.0;
/// The top of the middle row (mm).
const MIDDLE: f64 = 22.0;
/// The top of the bottom row (mm).
const BOTTOM: f64 = 45.0;
/// How wide the patches of each block of the bottom row are (mm), left to right, and how many lie along a
/// side of the block.
pub const PATCHES: [(f64, usize); 3] = [(1.5, 8), (2.0, 6), (3.0, 4)];
/// How far a patch reaches under the patch sewn after it in its row and the one below it (mm): the overlap of a design
/// digitized automatically.
pub const OVERLAP: f64 = 1.0;
/// The left sides of the blocks of the bottom row (mm): under the first, third and fifth squares.
const BLOCKS: [f64; 3] = [0.0, 38.0, 76.0];
/// A fill of the ladder: a fill's default rows, and no underlay.
const FILL: (&str, &str) = ("fill_underlay", "false");
/// The layer under each square and block: one layer of stitches 3 mm long in rows 3 sin 60° mm apart,
/// staggered over 2 rows, across the rows of the first fill, without an underlay. It is what the base layer
/// of a design digitized automatically sews by default, which a test compares.
pub(super) const BASE: [(&str, &str); 6] =
    [("max_stitch_length_mm", "3"), ("row_spacing_mm", "2.5981"), ("staggers", "2"), ("underpath", "false"), FILL, ("angle", "90")];

/// The corners of the square with its top left corner at (`x`, `y`), `side` mm a side, from that corner round.
fn corners((x, y): (f64, f64), side: f64) -> [(f64, f64); 4] {
    [(x, y), (x + side, y), (x + side, y + side), (x, y + side)]
}

/// Draws TS-13F.
pub(super) fn drawing() -> Result<Drawing, SheetError> {
    let (blue, black) = (thread(BLUE)?, thread(BLACK)?);
    let mut d = Drawing::new("ts13f");
    for (row, top) in [("bare", 0.0), ("outlined", MIDDLE)] {
        // The top row sews left to right and the middle row right to left, from under the top row's last
        // square: the reference machine cuts no jump, and a jump thread back across the sheet would be sewn
        // down where it lands.
        let squares: Vec<usize> = if top == 0.0 { (1..=MOST).collect() } else { (1..=MOST).rev().collect() };
        for fills in squares {
            let at = ((fills - 1) as f64 * (SIDE + GAP), top);
            d.fill(&format!("{row}-{fills}-base"), vec![polygon(&corners(at, SIDE))?], &blue, &BASE)?;
            for layer in 0..fills {
                let (name, angle) = (format!("{row}-{fills}-fill-{layer}"), (layer * 30).to_string());
                let last = if layer + 1 == fills { "true" } else { "false" };
                let params = [FILL, ("angle", angle.as_str()), ("trim_after", last)];
                d.fill(&name, vec![polygon(&corners(at, SIDE))?], &blue, &params)?;
            }
        }
    }
    for ((across, count), left) in PATCHES.into_iter().zip(BLOCKS) {
        let side = across * count as f64;
        d.fill(&format!("patches-{across}-base"), vec![polygon(&corners((left, BOTTOM), side))?], &blue, &BASE)?;
        for j in 0..count {
            // The rows of patches sew left to right and right to left in turn, so that no jump thread runs
            // back across the block under the next row.
            let rightwards = j.is_multiple_of(2);
            for step in 0..count {
                let i = if rightwards { step } else { count - 1 - step };
                let (x, y) = (left + i as f64 * across, BOTTOM + j as f64 * across);
                // A patch reaches under the patch sewn after it in its row, and under the one below it. The
                // last patch of a row and the patches of the last row end at the block's edge.
                let (reach_right, reach_left) = (rightwards && step + 1 < count, !rightwards && step + 1 < count);
                let (x0, x1) = (if reach_left { x - OVERLAP } else { x }, if reach_right { x + across + OVERLAP } else { x + across });
                let tall = if j + 1 == count { across } else { across + OVERLAP };
                let patch = polygon(&[(x0, y), (x1, y), (x1, y + tall), (x0, y + tall)])?;
                let last = if j + 1 == count && step + 1 == count { "true" } else { "false" };
                d.fill(&format!("patches-{across}-{j}-{i}"), vec![patch], &blue, &[FILL, ("trim_after", last)])?;
            }
        }
    }
    // The outlines last, in their own thread: each a satin column between a square outside the fills' edge and
    // one inside it.
    for fills in 1..=MOST {
        let (x, y) = ((fills - 1) as f64 * (SIDE + GAP), MIDDLE);
        let half = OUTLINE / 2.0;
        let rail = |inset: f64| polygon(&corners((x + inset, y + inset), SIDE - inset - inset));
        d.satin_between(&format!("outlined-{fills}-outline"), [rail(-half)?, rail(half)?], &black, &[("trim_after", "true")])?;
    }
    Ok(d)
}

#[cfg(test)]
mod tests {
    use stitchcraft_plan::Rgb;

    use super::*;
    use crate::auto::default_base_layer;
    use crate::design::{Segment, Shape};
    use crate::testsheets::find;

    #[test]
    fn ts13f_lays_each_square_on_the_base_layer_a_digitized_design_sews_by_default() {
        let sewn = default_base_layer();
        let ours: stitchcraft_params::ParamSet = BASE.into_iter().collect();
        assert_eq!(ours, sewn);
    }

    #[test]
    fn ts13f_draws_its_squares_by_their_fills_then_its_patches_then_its_outlines() {
        let design = drawing().unwrap().design().unwrap();
        let ids: Vec<&str> = design.elements().iter().map(|e| e.id.as_str()).collect();
        // A square of n fills is a base layer and n fills: 1 + 2 + … + 6 fills and 6 base layers a row.
        let squares = 2 * (MOST + MOST * (MOST + 1) / 2);
        let patches: usize = PATCHES.iter().map(|(_, count)| 1 + count * count).sum();
        assert_eq!((squares, patches, ids.len()), (54, 119, 54 + 119 + MOST));
        assert_eq!(ids[..3], ["ts13f:bare-1-base", "ts13f:bare-1-fill-0", "ts13f:bare-2-base"]);
        assert_eq!(ids[26], "ts13f:bare-6-fill-5");
        assert_eq!(ids[27], "ts13f:outlined-6-base");
        assert_eq!(ids[squares - 1], "ts13f:outlined-1-fill-0");
        assert_eq!(ids[squares..squares + 3], ["ts13f:patches-1.5-base", "ts13f:patches-1.5-0-0", "ts13f:patches-1.5-0-1"]);
        assert_eq!(ids[squares + 8..squares + 11], ["ts13f:patches-1.5-0-7", "ts13f:patches-1.5-1-7", "ts13f:patches-1.5-1-6"]);
        assert_eq!(ids[squares + patches..], [1, 2, 3, 4, 5, 6].map(|n| format!("ts13f:outlined-{n}-outline")));
        // Each fill's rows are turned 30° from those of the fill under it, and only the last of a square trims.
        let of = |id: &str, key: &str| design.elements().iter().find(|e| e.id.as_str() == id).unwrap().params.get(key).map(str::to_string);
        assert_eq!([0, 1, 5].map(|layer| of(&format!("ts13f:bare-6-fill-{layer}"), "angle").unwrap()), ["0", "30", "150"]);
        assert_eq!([0, 4, 5].map(|layer| of(&format!("ts13f:bare-6-fill-{layer}"), "trim_after").unwrap()), ["false", "false", "true"]);
        assert_eq!(of("ts13f:bare-6-fill-5", "fill_underlay").as_deref(), Some("false"));
        // The fills sew in blue and the outlines in black, last.
        let black = thread(BLACK).unwrap().color;
        let threads: Vec<Rgb> = design.elements().iter().map(|e| e.thread.color).collect();
        assert!(threads[..squares + patches].iter().all(|c| *c != black) && threads[squares + patches..].iter().all(|c| *c == black));
        let plan = find("TS-13F").unwrap().plan().unwrap();
        assert_eq!(plan.stats().color_changes, 1);
    }

    #[test]
    fn ts13f_patches_reach_1_mm_under_the_patch_to_their_right_and_the_one_below() {
        let design = drawing().unwrap().design().unwrap();
        let size = |id: &str| {
            let element = design.elements().iter().find(|e| e.id.as_str() == id).unwrap();
            let Shape::Fill { path, .. } = &element.shape else { panic!("{id} is no fill") };
            let ring = &path.subpaths[0];
            // A patch is drawn with straight sides only.
            let ends = ring.segments.iter().filter_map(|segment| if let Segment::Line(to) = segment { Some(*to) } else { None });
            let points: Vec<_> = std::iter::once(ring.start).chain(ends).collect();
            let (xs, ys): (Vec<f64>, Vec<f64>) = points.iter().map(|p| (p.x(), p.y())).unzip();
            let span = |v: &[f64]| v.iter().copied().fold(f64::MIN, f64::max) - v.iter().copied().fold(f64::MAX, f64::min);
            (span(&xs), span(&ys), xs[0], ys[0])
        };
        // The first patch of each block and the last of its first row, the first of its second row, which
        // sews right to left and reaches left, and patches of its last row.
        assert_eq!(size("ts13f:patches-1.5-0-0"), (2.5, 2.5, 0.0, BOTTOM));
        assert_eq!(size("ts13f:patches-1.5-0-7"), (1.5, 2.5, 10.5, BOTTOM));
        assert_eq!(size("ts13f:patches-1.5-1-7"), (2.5, 2.5, 9.5, BOTTOM + 1.5));
        assert_eq!(size("ts13f:patches-1.5-1-0"), (1.5, 2.5, 0.0, BOTTOM + 1.5));
        assert_eq!(size("ts13f:patches-1.5-7-0"), (1.5, 1.5, 0.0, BOTTOM + 10.5));
        assert_eq!(size("ts13f:patches-2-0-0"), (3.0, 3.0, 38.0, BOTTOM));
        assert_eq!(size("ts13f:patches-2-5-5"), (3.0, 2.0, 47.0, BOTTOM + 10.0));
        assert_eq!(size("ts13f:patches-3-0-0"), (4.0, 4.0, 76.0, BOTTOM));
        assert_eq!(size("ts13f:patches-3-3-3"), (4.0, 3.0, 84.0, BOTTOM + 9.0));
        assert_eq!(size("ts13f:patches-3-3-0"), (3.0, 3.0, 76.0, BOTTOM + 9.0));
        // Only a block's last patch trims: the needle sews on from each patch to the next.
        let trims = |id: &str| design.elements().iter().find(|e| e.id.as_str() == id).unwrap().params.get("trim_after") == Some("true");
        assert!(trims("ts13f:patches-3-3-0") && !trims("ts13f:patches-3-0-3") && !trims("ts13f:patches-3-3-3"));
        // An outline's rails lie 1 mm outside its square's edge and 1 mm inside it.
        let outline = design.elements().iter().find(|e| e.id.as_str() == "ts13f:outlined-2-outline").unwrap();
        let Shape::Stroke { path, .. } = &outline.shape else { panic!("no stroke") };
        let starts: Vec<(f64, f64)> = path.subpaths.iter().map(|s| (s.start.x(), s.start.y())).collect();
        assert_eq!(starts, [(18.0, MIDDLE - 1.0), (20.0, MIDDLE + 1.0)]);
        assert_eq!((SIDE, OUTLINE, OVERLAP), (15.0, 2.0, 1.0));
    }
}
