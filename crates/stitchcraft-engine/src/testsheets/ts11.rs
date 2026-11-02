//! TS-11 — new stitch types (M7), for MC-6. TS-11B sews the new fills: contour fills (M7.7) so far, and
//! meander and circular fills as M7.8 and M7.9 sew them. TS-11A, the new strokes and satins of M7.2 to
//! M7.5, is drawn before MC-6.
//!
//! Drawn as a design (`designed`), so the engine sews each sample as a design's element, with Ink/Stitch's
//! defaults but for what the sample sets: rings the default 0.25 mm apart, over the default underlay. Each
//! sample is trimmed after, and each row is a thread of its own: the reference machine cuts the thread at a
//! colour change, and leaves a jump's thread on the fabric (MC-1), which the next row would sew over.
//!
//! - **Top row**, the 3 strategies on a disc 36 mm across: inner to outer, round a hole 12 mm across set off
//!   its middle, then a single spiral and a double one.
//! - **Middle row**, the 3 join styles on a star 40 mm across, its rings 0.4 mm apart: round, mitred and
//!   bevelled corners.
//! - **Bottom row**: 2 squares joined by a neck 3 mm wide, where the rings part; a rectangle whose rings
//!   alternate their direction, so the steps between them don't cross; and a leaf smoothed by 0.5 mm.
//!
//! No sample's hole grows to meet its outline at the same ring all round: there the rings that are left
//! turn on the last bits of the arithmetic, in Ink/Stitch's as in StitchCraft's.

use stitchcraft_core::math::sin_cos;

use super::designed::{Drawing, polygon, round};
use super::sketch::SheetError;
use super::{BLUE, EMERALD_GREEN, RED, thread};

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

/// Draws TS-11B.
pub(super) fn fills() -> Result<Drawing, SheetError> {
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
    d.fill(
        "alternating",
        vec![polygon(&[(x, y), (x + 36.0, y), (x + 36.0, y + 24.0), (x, y + 24.0)])?],
        &red,
        &with(&[("avoid_self_crossing", "true")]),
    )?;
    d.fill("smoothed", vec![polygon(&leaf((right, bottom), 19.0, 10.0))?], &red, &with(&[("smoothness_mm", "0.5")]))?;
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
    use super::*;

    #[test]
    fn the_star_s_points_and_the_leaf_s_tips_lie_where_they_are_drawn() {
        let star = star((0.0, 0.0));
        assert_eq!(star.len(), 10);
        assert!((star[0].0).abs() < 1e-9 && (star[0].1 + 20.0).abs() < 1e-9, "the first point is up: {:?}", star[0]);
        assert!(star.iter().skip(1).step_by(2).all(|&(x, y)| ((x * x + y * y).sqrt() - 8.0).abs() < 1e-9));
        let leaf = leaf((0.0, 0.0), 19.0, 10.0);
        assert_eq!(leaf.len(), 48);
        assert_eq!(leaf[0], (-19.0, 0.0));
        assert!((leaf[24].0 - 19.0).abs() < 1e-9 && leaf[24].1.abs() < 1e-9, "{:?}", leaf[24]);
        assert!(leaf.iter().all(|&(_, y)| y.abs() <= 10.0 + 1e-9));
    }
}
