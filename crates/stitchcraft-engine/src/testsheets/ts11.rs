//! TS-11 — new stitch types (M7), for MC-6. TS-11B sews the contour fills (M7.7) and the circular fills
//! (M7.9); TS-11C sews the meander fills (M7.8). TS-11A, the new strokes and satins of M7.2 to M7.5, is
//! drawn before MC-6.
//!
//! Drawn as designs (`designed`), so the engine sews each sample as a design's element, with Ink/Stitch's
//! defaults but for what the sample sets, over the default underlay. Each sample is trimmed after, and each
//! row is a thread of its own: the reference machine cuts the thread at a colour change, and leaves a
//! jump's thread on the fabric (MC-1), which the next row would sew over.
//!
//! TS-11B, contour fills, their rings the default 0.25 mm apart:
//!
//! - **Top row**, the 3 strategies on a disc 36 mm across: inner to outer, round a hole 12 mm across set off
//!   its middle, then a single spiral and a double one.
//! - **Middle row**, the 3 join styles on a star 40 mm across, its rings 0.4 mm apart: round, mitred and
//!   bevelled corners.
//! - **Bottom row**: 2 squares joined by a neck 3 mm wide, where the rings part; a rectangle whose rings
//!   alternate their direction, so the steps between them don't cross; and a leaf smoothed by 0.5 mm.
//!
//! - **Fourth row**, circular fills: the disc round its hole set off its middle, the star, and a rectangle
//!   whose circles fade from 0.25 mm apart at its middle to 1 mm at its corners.
//!
//! No sample's hole grows to meet its outline at the same ring all round: there the rings that are left
//! turn on the last bits of the arithmetic, in Ink/Stitch's as in StitchCraft's.
//!
//! TS-11C, meander fills:
//!
//! - **Top and middle rows**, 6 of the 7 tiles on a disc 36 mm across, each side 2 mm long: the honeycomb,
//!   squares and triangles, then octagons and squares, hexagons and triangles, and hexagons, squares and
//!   triangles.
//! - **Bottom row**: the seventh tile, squares and triangles, on the star; a rectangle of hexagons twice as
//!   large, sewn as a zigzag kept inside it; and the leaf in hexagons half as large, turned by 30° and
//!   smoothed by 0.5 mm.

use stitchcraft_core::math::sin_cos;

use super::designed::{Drawing, polygon, round};
use super::sketch::SheetError;
use super::{BLUE, DEEP_GOLD, EMERALD_GREEN, RED, thread};

/// The settings every sample shares: a contour fill, trimmed after.
const CONTOUR: [(&str, &str); 2] = [("fill_method", "contour_fill"), ("trim_after", "true")];
/// The middles of the samples across each row, and of the rows down the sheet (mm).
const ACROSS: [f64; 3] = [20.0, 63.0, 106.0];
const DOWN: [f64; 3] = [20.0, 64.0, 106.0];
/// The discs' radius, and the hole's radius and its middle from the disc's (mm).
const DISC: f64 = 18.0;
const HOLE: (f64, (f64, f64)) = (6.0, (4.0, -3.0));
/// The stars' outer and inner radius (mm).
const STAR: (f64, f64) = (20.0, 8.0);
/// The settings every circular sample shares: a circular fill, trimmed after.
const CIRCULAR: [(&str, &str); 2] = [("fill_method", "circular_fill"), ("trim_after", "true")];
/// The middle of TS-11B's row of circular fills, down the sheet (mm).
const CIRCLES: f64 = 150.0;

/// Draws TS-11B.
pub(super) fn contours() -> Result<Drawing, SheetError> {
    let (green, blue, red) = (thread(EMERALD_GREEN)?, thread(BLUE)?, thread(RED)?);
    let mut d = Drawing::new("ts11b");
    let [left, middle, right] = ACROSS;
    let [top, centre, bottom] = DOWN;
    let with = |own: &[(&'static str, &'static str)]| [&CONTOUR[..], own].concat();
    // Top row: the strategies.
    let (hole, (dx, dy)) = HOLE;
    d.fill("inner-to-outer", vec![round((left, top), DISC)?, round((left + dx, top + dy), hole)?], &green, &with(&[]))?;
    d.fill("single-spiral", vec![round((middle, top), DISC)?], &green, &with(&[("contour_strategy", "1")]))?;
    d.fill("double-spiral", vec![round((right, top), DISC)?], &green, &with(&[("contour_strategy", "2")]))?;
    // Middle row: the join styles.
    for (x, (name, join)) in ACROSS.into_iter().zip([("round", "0"), ("mitred", "1"), ("bevelled", "2")]) {
        d.fill(name, vec![polygon(&star((x, centre)))?], &blue, &with(&[("join_style", join), ("row_spacing_mm", "0.4")]))?;
    }
    // Bottom row: rings that part, rings that alternate, and a smoothed path.
    let (x, y) = (left - 20.0, bottom - 8.0);
    let parts = [
        (0.0, 0.0),
        (16.0, 0.0),
        (16.0, 6.5),
        (24.0, 6.5),
        (24.0, 0.0),
        (40.0, 0.0),
        (40.0, 16.0),
        (24.0, 16.0),
        (24.0, 9.5),
        (16.0, 9.5),
        (16.0, 16.0),
        (0.0, 16.0),
    ];
    let parts: Vec<(f64, f64)> = parts.iter().map(|&(px, py)| (x + px, y + py)).collect();
    d.fill("parting", vec![polygon(&parts)?], &red, &with(&[]))?;
    let (x, y) = (middle - 18.0, bottom - 12.0);
    let rectangle = polygon(&[(x, y), (x + 36.0, y), (x + 36.0, y + 24.0), (x, y + 24.0)])?;
    d.fill("alternating", vec![rectangle], &red, &with(&[("avoid_self_crossing", "true")]))?;
    d.fill("smoothed", vec![polygon(&leaf((right, bottom), 19.0, 10.0))?], &red, &with(&[("smoothness_mm", "0.5")]))?;
    circulars(&mut d)?;
    Ok(d)
}

/// TS-11B's row of circular fills: the disc round its hole, the star, and a rectangle whose circles fade.
fn circulars(d: &mut Drawing) -> Result<(), SheetError> {
    let gold = thread(DEEP_GOLD)?;
    let [left, middle, right] = ACROSS;
    let with = |own: &[(&'static str, &'static str)]| [&CIRCULAR[..], own].concat();
    let (hole, (dx, dy)) = HOLE;
    d.fill("circular", vec![round((left, CIRCLES), DISC)?, round((left + dx, CIRCLES + dy), hole)?], &gold, &with(&[]))?;
    d.fill("circular-star", vec![polygon(&star((middle, CIRCLES)))?], &gold, &with(&[]))?;
    let (x, y) = (right - 18.0, CIRCLES - 12.0);
    let rectangle = polygon(&[(x, y), (x + 36.0, y), (x + 36.0, y + 24.0), (x, y + 24.0)])?;
    d.fill("circular-fading", vec![rectangle], &gold, &with(&[("end_row_spacing_mm", "1")]))?;
    Ok(())
}

/// The settings every meander sample shares: a meander fill, trimmed after.
const MEANDER: [(&str, &str); 2] = [("fill_method", "meander_fill"), ("trim_after", "true")];

/// Draws TS-11C.
pub(super) fn meanders() -> Result<Drawing, SheetError> {
    let (green, blue, red) = (thread(EMERALD_GREEN)?, thread(BLUE)?, thread(RED)?);
    let mut d = Drawing::new("ts11c");
    let [left, middle, right] = ACROSS;
    let [top, centre, bottom] = DOWN;
    let with = |own: &[(&'static str, &'static str)]| [&MEANDER[..], own].concat();
    // Top and middle rows: 6 tiles, a disc each.
    let rows = [
        (top, &green, ["honeycomb", "squares", "triangles"]),
        (centre, &blue, ["octagons-squares", "hexagons-triangles", "hexagons-squares-triangles"]),
    ];
    for (y, thread, tiles) in rows {
        for (x, tile) in ACROSS.into_iter().zip(tiles) {
            d.fill(tile, vec![round((x, y), DISC)?], thread, &with(&[("meander_pattern", tile)]))?;
        }
    }
    // Bottom row: the seventh tile on the star, a zigzag kept inside a rectangle, and a turned and smoothed
    // leaf.
    d.fill("squares-triangles", vec![polygon(&star((left, bottom)))?], &red, &with(&[("meander_pattern", "squares-triangles")]))?;
    let (x, y) = (middle - 18.0, bottom - 12.0);
    d.fill(
        "zigzag",
        vec![polygon(&[(x, y), (x + 36.0, y), (x + 36.0, y + 24.0), (x, y + 24.0)])?],
        &red,
        &with(&[("meander_scale_percent", "200"), ("zigzag_spacing_mm", "0.6"), ("zigzag_width_mm", "2.5"), ("clip", "true")]),
    )?;
    d.fill(
        "turned",
        vec![polygon(&leaf((right, bottom), 19.0, 10.0))?],
        &red,
        &with(&[("meander_scale_percent", "50"), ("meander_angle", "30"), ("smoothness_mm", "0.5")]),
    )?;
    Ok(d)
}

/// A 5-pointed star round `centre`, a point up, its points [`STAR`]'s outer radius from the middle and the
/// corners between them its inner radius.
fn star((x, y): (f64, f64)) -> Vec<(f64, f64)> {
    let (outer, inner) = STAR;
    (0..10)
        .map(|k| {
            let radius = if k % 2 == 0 { outer } else { inner };
            let (sin, cos) = sin_cos(std::f64::consts::PI * (f64::from(k) / 5.0 - 0.5));
            (x + radius * cos, y + radius * sin)
        })
        .collect()
}

/// A leaf round `centre`, `half` long each way along x and `bulge` wide each way along y at its middle:
/// 2 arcs of a sine, 24 points each, meeting at its tips.
fn leaf((x, y): (f64, f64), half: f64, bulge: f64) -> Vec<(f64, f64)> {
    const POINTS: u32 = 24;
    let side = |k: u32| {
        let t = f64::from(k) / f64::from(POINTS);
        let (sin, _) = sin_cos(std::f64::consts::PI * t);
        (x - half + 2.0 * half * t, bulge * sin)
    };
    let upper = (0..POINTS).map(side).map(|(px, h)| (px, y - h));
    let lower = (0..POINTS).map(|k| side(POINTS - k)).map(|(px, h)| (px, y + h));
    upper.chain(lower).collect()
}

#[cfg(test)]
mod tests {
    use stitchcraft_plan::{Role, StitchKind};

    use super::*;

    #[test]
    fn the_star_s_points_and_the_leaf_s_tips_lie_where_they_are_drawn() {
        let star = star((0.0, 0.0));
        assert_eq!(star.len(), 10);
        assert!((star[0].0).abs() < 1e-9 && (star[0].1 + 20.0).abs() < 1e-9, "the first point is up: {:?}", star[0]);
        assert!(star.iter().skip(1).step_by(2).all(|&(x, y)| ((x * x + y * y).sqrt() - 8.0).abs() < 1e-9));
        // Round its middle from the top point towards +x: the corner after it lies 36° past the top, at
        // (5 + 8 cos 54°, -8 sin 54°) round (5, 0).
        let (x, y) = super::star((5.0, 0.0))[1];
        assert!((x - 9.702282).abs() < 1e-6 && (y + 6.472136).abs() < 1e-6, "{x} {y}");
        let leaf = leaf((0.0, 0.0), 19.0, 10.0);
        assert_eq!(leaf.len(), 48);
        assert_eq!(leaf[0], (-19.0, 0.0));
        assert!((leaf[24].0 - 19.0).abs() < 1e-9 && leaf[24].1.abs() < 1e-9, "{:?}", leaf[24]);
        assert!(leaf.iter().all(|&(_, y)| y.abs() <= 10.0 + 1e-9));
    }

    /// The least x and y of `points`, then the greatest.
    fn bounds(points: impl IntoIterator<Item = (f64, f64)>) -> [f64; 4] {
        let start = [f64::INFINITY, f64::INFINITY, f64::NEG_INFINITY, f64::NEG_INFINITY];
        points.into_iter().fold(start, |[x0, y0, x1, y1], (x, y)| [x0.min(x), y0.min(y), x1.max(x), y1.max(y)])
    }

    #[test]
    fn ts11b_sews_each_sample_where_its_drawing_puts_it() {
        // The sheet is planned with its middle on the hoop's: its drawing spans x 0 to 125.02 (the stars' points)
        // and y 2 to 118. Each sample's needle points span its shape to within half a millimetre: a spiral turns
        // in from the outline as it goes round, and the smoothed leaf, left out here, is rounded off at its tips.
        let plan = crate::testsheets::find("TS-11B").unwrap().plan().unwrap();
        let mid = plan.bounds().unwrap().center();
        let (dx, dy) = (mid.x() - 125.021130326 / 2.0, mid.y() - 60.0);
        let sewn = |name: &str| -> Vec<(f64, f64)> {
            let element = plan.elements.iter().position(|e| e.as_str() == format!("ts11b:{name}")).unwrap();
            let mine = |s: &&stitchcraft_plan::Stitch| {
                s.kind == StitchKind::Normal && s.origin.role != Role::Lock && s.origin.element.map(|e| e.index()) == Some(element)
            };
            plan.stitches().filter(mine).map(|s| (s.at.x() - dx, s.at.y() - dy)).collect()
        };
        let [left, middle, right] = ACROSS;
        let [top, centre, bottom] = DOWN;
        let disc = |x: f64| [x - DISC, top - DISC, x + DISC, top + DISC];
        let expected = [
            ("inner-to-outer", disc(left)),
            ("single-spiral", disc(middle)),
            ("double-spiral", disc(right)),
            ("round", bounds(star((left, centre)))),
            ("mitred", bounds(star((middle, centre)))),
            ("bevelled", bounds(star((right, centre)))),
            ("parting", [left - 20.0, bottom - 8.0, left + 20.0, bottom + 8.0]),
            ("alternating", [middle - 18.0, bottom - 12.0, middle + 18.0, bottom + 12.0]),
        ];
        for (name, drawn) in expected {
            let found = bounds(sewn(name));
            assert!(found.iter().zip(drawn).all(|(f, d)| (f - d).abs() < 0.5), "{name}: sewn {found:?}, drawn {drawn:?}");
        }
        // The hole of inner to outer, 12 mm across, lies 4 mm right of the disc's middle and 3 mm above it: no
        // needle point inside it, and its own ring round it.
        let (hx, hy) = (left + 4.0, top - 3.0);
        let nearest = sewn("inner-to-outer").iter().map(|&(x, y)| ((x - hx) * (x - hx) + (y - hy) * (y - hy)).sqrt()).fold(f64::INFINITY, f64::min);
        assert!((nearest - 6.0).abs() < 0.1, "{nearest}");
    }
}
