//! TS-11 — new stitch types (M7), for MC-6. TS-11A sews the new strokes and satins of M7.2 to M7.5, TS-11B
//! the contour fills (M7.7) and the circular fills (M7.9), and TS-11C the meander fills (M7.8).
//!
//! Drawn as designs (`designed`), so the engine sews each sample as a design's element, with Ink/Stitch's
//! defaults but for what the sample sets, over the default underlay. Each sample is trimmed after. The
//! reference machine cuts the thread at a colour change, and leaves a jump's thread on the fabric (MC-1),
//! which a row sewn after it would sew over. So each row of TS-11B and TS-11C is a thread of its own, and
//! TS-11A's rows, a band of them to a thread, turn back on each other.
//!
//! TS-11A, a band to a thread. Most samples are hooks, straight and then turning a quarter circle, so each
//! sews along a straight and round a curve, in rows that turn back on each other ([`snake`]):
//!
//! - **Zigzag strokes**, in green: widths of 1, 2 and 4 mm down the rows, spacings of 0.4, 1 and 2 mm across
//!   them, and beside the last row a straight line 4 mm wide whose stitches turn 45°.
//! - **E-stitches** for appliqué, in red: the sides of a square and 4 rings, 3 or 5 mm wide at spacings of 2
//!   or 3 mm, each spine on the appliqué's edge and its spikes in over it. The machine first sews a line 1 mm
//!   inside each edge and stops for the appliqué fabric, then sews the line again to tack the fabric down and
//!   stops for it to be trimmed 1 mm outside the line, where the spines then sew.
//! - **S-stitches**, in blue: 6 mm wide, and 10 mm wide split at 3 mm by the default method and by the
//!   staggered one, a row at a spacing of 0.4 mm and a row at 0.6 mm.
//! - **Zigzag satins**, in gold, each beside a satin column of the same width and spacing: 4 mm wide in the
//!   first row and 8 mm in the second, at spacings of 0.4 and 1 mm, then one 8 mm wide at 1 mm over a contour
//!   underlay.
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

use super::designed::{AS_DRAWN, Drawing, Start, Way, hook, open, polygon, round};
use super::sketch::SheetError;
use super::{BLUE, DEEP_GOLD, EMERALD_GREEN, RED, thread};
use crate::design::Subpath;

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

/// Each element of a sheet trimmed after it, so the samples stand apart.
const TRIMMED: [(&str, &str); 1] = [("trim_after", "true")];

/// A sample's own Ink/Stitch settings.
type Own = &'static [(&'static str, &'static str)];

/// Draws TS-11A.
pub(super) fn strokes() -> Result<Drawing, SheetError> {
    let mut d = Drawing::new("ts11a");
    zigzags(&mut d)?;
    e_stitches(&mut d)?;
    s_stitches(&mut d)?;
    zigzag_satins(&mut d)?;
    Ok(d)
}

/// The hooks of rows laid out by [`snake`]: each row's in the order they sew, each with its column, its start
/// and the way it runs.
type Laid<T> = Vec<Vec<(T, (f64, f64), Way)>>;

/// Lays out rows of hooks that turn back on each other, each hook `straight` mm long before it turns and
/// `pitch` apart along its row. The first row runs east from `left`, and each after it back the other way;
/// each row is `(y, radius)`, with a hook for each of `columns`, from the left. Gives the hooks as they sew,
/// and where the last ends, or `(left, 0)` with none.
///
/// The needle comes to every hook from behind it, where none of the hook's stitches lie: the reference
/// machine leaves a jump's thread on the fabric (MC-1), and a hook that sewed over it would catch it. Along a
/// row, each hook starts up and on from where the one before it ended. A row's first hook starts as far on
/// from where the row before ended as it lies below it, 45° down from there, so its first stitches don't lie
/// along the thread.
fn snake<T: Copy>(left: f64, (straight, pitch): (f64, f64), columns: &[T], rows: &[(f64, f64)]) -> (Laid<T>, (f64, f64)) {
    let (mut laid, mut way, mut end) = (Vec::with_capacity(rows.len()), Way::East, None);
    for &(y, radius) in rows {
        let s = way.sign();
        let mut x = end.map_or(left, |(x, bottom): (f64, f64)| x + s * (y - bottom));
        let order: Vec<T> = match way {
            Way::East => columns.to_vec(),
            Way::West => columns.iter().rev().copied().collect(),
        };
        let mut row = Vec::with_capacity(order.len());
        for column in order {
            row.push((column, (x, y), way));
            end = Some((x + s * (straight + radius), y + radius));
            x += s * pitch;
        }
        laid.push(row);
        way = way.back();
    }
    (laid, end.unwrap_or((left, 0.0)))
}

/// The rails of a column `width` mm wide along a hook of `(straight, radius)` from `start` the `way` it runs:
/// the rail outside its turn, then the one inside it.
fn column(start: (f64, f64), way: Way, size: (f64, f64), width: f64) -> Result<[Subpath; 2], SheetError> {
    Ok([hook(start, way, size, -width / 2.0)?, hook(start, way, size, width / 2.0)?])
}

/// TS-11A's zigzag strokes: hooks 18 mm long turning round 6, 32 mm apart from 6 mm in, a row at each width,
/// its height down the sheet with its width, and a column at each spacing (mm).
const ZIGZAG_HOOK: (f64, f64) = (18.0, 6.0);
const ZIGZAG_ROWS: [(f64, f64); 3] = [(4.0, 1.0), (16.0, 2.0), (28.0, 4.0)];
const ZIGZAG_SPACINGS: [&str; 3] = ["0.4", "1", "2"];

/// TS-11A's zigzag strokes: a hook at each width and spacing, then a straight line beside the last row, as
/// wide as its hooks, whose stitches turn 45°.
fn zigzags(d: &mut Drawing) -> Result<(), SheetError> {
    let green = thread(EMERALD_GREEN)?;
    let with = |spacing: &'static str, own: &[(&'static str, &'static str)]| {
        [&[("stroke_method", "zigzag_stitch"), ("zigzag_spacing_mm", spacing)][..], own, &TRIMMED].concat()
    };
    let (straight, radius) = ZIGZAG_HOOK;
    let (laid, (x, bottom)) = snake(6.0, (straight, 32.0), &ZIGZAG_SPACINGS, &ZIGZAG_ROWS.map(|(y, _)| (y, radius)));
    for (row, (_, width)) in laid.into_iter().zip(ZIGZAG_ROWS) {
        for (spacing, start, way) in row {
            let name = format!("zigzag-stroke-{width}-{spacing}");
            d.wide(&name, hook(start, way, ZIGZAG_HOOK, 0.0)?, width, &green, &with(spacing, &[]))?;
        }
    }
    // On the last row's height, 45° on from where it ended.
    let [.., (y, width)] = ZIGZAG_ROWS;
    let from = x + (bottom - y);
    d.wide("zigzag-stroke-45", open(&[(from, y), (from + 24.0, y)])?, width, &green, &with("1", &[("zigzag_angle", "45")]))
}

/// TS-11A's appliqué square, its top left corner and side, and its sides' E-stitches clockwise from the top,
/// each its width and spacing (mm).
const SQUARE: ((f64, f64), f64) = ((0.0, 42.0), 26.0);
const SIDES: [(&str, f64, &str); 4] = [("top", 3.0, "2"), ("right", 3.0, "3"), ("bottom", 5.0, "2"), ("left", 5.0, "3")];
/// TS-11A's appliqué circles: their middles' height down the sheet and their radius, then each one's middle
/// across it and its E-stitch's width and spacing (mm).
const RING_ROW: (f64, f64) = (55.0, 9.0);
const RINGS: [(f64, f64, &str); 4] = [(40.0, 3.0, "2"), (63.0, 3.0, "3"), (86.0, 5.0, "2"), (109.0, 5.0, "3")];
/// How far inside an appliqué's edge the line that places it and tacks it down runs (mm).
const TACK: f64 = 1.0;

/// TS-11A's E-stitches for appliqué: the lines that place the fabric and tack it down, each time followed by
/// a stop, then the sides of the square and the rings round the circles, each from its left.
fn e_stitches(d: &mut Drawing) -> Result<(), SheetError> {
    let red = thread(RED)?;
    let ((x, y), side) = SQUARE;
    let (cy, radius) = RING_ROW;
    let (near, far) = (TACK, side - TACK);
    let inset = [(x + near, y + near), (x + far, y + near), (x + far, y + far), (x + near, y + far), (x + near, y + near)];
    for step in ["placement", "tack-down"] {
        d.polyline(&format!("{step}-square"), &inset, &red, &TRIMMED)?;
        for (cx, ..) in RINGS {
            d.circle(&format!("{step}-circle-{cx}"), (cx, cy), radius - TACK, Start::Left, &red, &TRIMMED)?;
        }
        d.stop();
    }
    // Each side runs clockwise along its edge, its spine on the edge and its other rail inside it, as wide
    // apart as the side is. The top side runs the whole edge, and each side after it starts where the one
    // before covers its edge, so the sides meet without overlapping, as a pinwheel's sails do: mitred,
    // their spikes would lean far along the column at the corners.
    let [(_, top, _), (_, right, _), (_, bottom, _), (_, left, _)] = SIDES;
    let (far_x, far_y) = (x + side, y + side);
    let sides = [
        [[(x, y), (far_x, y)], [(x, y + top), (far_x, y + top)]],
        [[(far_x, y + top), (far_x, far_y)], [(far_x - right, y + top), (far_x - right, far_y)]],
        [[(far_x - right, far_y), (x, far_y)], [(far_x - right, far_y - bottom), (x, far_y - bottom)]],
        [[(x, far_y - bottom), (x, y + top)], [(x + left, far_y - bottom), (x + left, y + top)]],
    ];
    let with = |spacing: &'static str| [&[("satin_method", "e_stitch"), ("zigzag_spacing_mm", spacing)][..], &AS_DRAWN].concat();
    for (rails, (name, _, spacing)) in sides.into_iter().zip(SIDES) {
        d.satin(&format!("e-stitch-{name}"), rails, &red, &with(spacing))?;
    }
    for (cx, width, spacing) in RINGS {
        d.ring(&format!("e-stitch-{width}-{spacing}"), ((cx, cy), Start::Left), (radius, radius - width), &red, &with(spacing))?;
    }
    Ok(())
}

/// TS-11A's S-stitches: hooks 20 mm long turning round 8, 40 mm apart from 13 mm in, a row at each spacing,
/// its height down the sheet with its spacing (mm).
const S_HOOK: (f64, f64) = (20.0, 8.0);
const S_ROWS: [(f64, &str); 2] = [(80.0, "0.4"), (96.0, "0.6")];

/// TS-11A's S-stitches: hooks 6 mm wide, and 10 mm wide split at 3 mm by the default method and the staggered
/// one, a row at each spacing.
fn s_stitches(d: &mut Drawing) -> Result<(), SheetError> {
    let blue = thread(BLUE)?;
    let samples: [(&str, f64, Own); 3] = [
        ("6", 6.0, &[]),
        ("10-default", 10.0, &[("max_stitch_length_mm", "3"), ("split_method", "default")]),
        ("10-staggered", 10.0, &[("max_stitch_length_mm", "3"), ("split_method", "staggered")]),
    ];
    let (straight, radius) = S_HOOK;
    let (laid, _) = snake(13.0, (straight, 40.0), &samples, &S_ROWS.map(|(y, _)| (y, radius)));
    for (r, (row, (_, spacing))) in laid.into_iter().zip(S_ROWS).enumerate() {
        for (k, ((name, width, split), start, way)) in row.into_iter().enumerate() {
            let params = [&[("satin_method", "s_stitch"), ("zigzag_spacing_mm", spacing)][..], split, &AS_DRAWN].concat();
            // An S-stitch starts on its second rail. Inside its turn, it starts next to where the hook before it
            // ended. The first of a row after the first starts outside, nearest the row above, where the needle
            // comes from: starting inside, its first stitches would cross the jump's thread.
            let [outside, inside] = column(start, way, S_HOOK, width)?;
            let rails = if r > 0 && k == 0 { [inside, outside] } else { [outside, inside] };
            d.satin_between(&format!("s-stitch-{name}-{spacing}"), rails, &blue, &params)?;
        }
    }
    Ok(())
}

/// TS-11A's zigzag satins and satin columns: hooks 14 mm long, 30 mm apart from 12 mm in, a row at each width,
/// its height down the sheet with its width and its hooks' radius, then the zigzag satin over a contour
/// underlay's height (mm).
const SATIN_STRAIGHT: f64 = 14.0;
const SATIN_ROWS: [(f64, f64, f64); 2] = [(114.0, 4.0, 6.0), (127.0, 8.0, 7.0)];
const UNDERLAID: f64 = 141.0;

/// TS-11A's zigzag satins, each beside a satin column of the same width and spacing, a row at each width,
/// then, after a stop, a zigzag satin over a contour underlay 45° on from where the rows ended.
fn zigzag_satins(d: &mut Drawing) -> Result<(), SheetError> {
    let gold = thread(DEEP_GOLD)?;
    let zigzag: Own = &[("satin_method", "zigzag")];
    let samples: [(&str, &str, Own); 4] =
        [("zigzag-satin", "0.4", zigzag), ("satin-column", "0.4", &[]), ("zigzag-satin", "1", zigzag), ("satin-column", "1", &[])];
    let (laid, (x, bottom)) = snake(12.0, (SATIN_STRAIGHT, 30.0), &samples, &SATIN_ROWS.map(|(y, _, radius)| (y, radius)));
    for (row, (_, width, radius)) in laid.into_iter().zip(SATIN_ROWS) {
        for ((name, spacing, method), start, way) in row {
            // A satin column and a zigzag satin start on their first rail, here the one outside the turn.
            let params = [method, &[("zigzag_spacing_mm", spacing)], &AS_DRAWN].concat();
            d.satin_between(&format!("{name}-{width}-{spacing}"), column(start, way, (SATIN_STRAIGHT, radius), width)?, &gold, &params)?;
        }
    }
    // The last starts after a stop: its underlay starts inside the column, and on any way the needle could come
    // to it, the column's own first stitch would come down on the jump's thread.
    d.stop();
    let [.., (_, width, radius)] = SATIN_ROWS;
    let params = [zigzag, &[("zigzag_spacing_mm", "1"), ("contour_underlay", "true")], &AS_DRAWN].concat();
    let start = (x + (UNDERLAID - bottom), UNDERLAID);
    d.satin_between("zigzag-satin-underlay", column(start, Way::East, (SATIN_STRAIGHT, radius), width)?, &gold, &params)
}

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
    let rectangle = polygon(&[(x, y), (x + 36.0, y), (x + 36.0, y + 24.0), (x, y + 24.0)])?;
    let zigzag = with(&[("meander_scale_percent", "200"), ("zigzag_spacing_mm", "0.6"), ("zigzag_width_mm", "2.5"), ("clip", "true")]);
    d.fill("zigzag", vec![rectangle], &red, &zigzag)?;
    let turned = with(&[("meander_scale_percent", "50"), ("meander_angle", "30"), ("smoothness_mm", "0.5")]);
    d.fill("turned", vec![polygon(&leaf((right, bottom), 19.0, 10.0))?], &red, &turned)?;
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
        // and y 2 to 168 (the circular disc). Each sample's needle points span its shape to within half a
        // millimetre: a spiral turns in from the outline as it goes round, and the smoothed leaf, left out here,
        // is rounded off at its tips.
        let plan = crate::testsheets::find("TS-11B").unwrap().plan().unwrap();
        let mid = plan.bounds().unwrap().center();
        let (dx, dy) = (mid.x() - 125.021130326 / 2.0, mid.y() - 85.0);
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
            ("circular", [left - DISC, CIRCLES - DISC, left + DISC, CIRCLES + DISC]),
            ("circular-star", bounds(star((middle, CIRCLES)))),
            ("circular-fading", [right - 18.0, CIRCLES - 12.0, right + 18.0, CIRCLES + 12.0]),
        ];
        for (name, drawn) in expected {
            let found = bounds(sewn(name));
            assert!(found.iter().zip(drawn).all(|(f, d)| (f - d).abs() < 0.5), "{name}: sewn {found:?}, drawn {drawn:?}");
        }
        // The hole of inner to outer, 12 mm across, lies 4 mm right of the disc's middle and 3 mm above it: no
        // needle point inside it, and its own ring round it. The circular disc's hole lies as far from its
        // middle, and the spiral's pieces end on it.
        let (hole, (hx, hy)) = HOLE;
        for (name, (x0, y0)) in [("inner-to-outer", (left + hx, top + hy)), ("circular", (left + hx, CIRCLES + hy))] {
            let nearest = sewn(name).iter().map(|&(x, y)| ((x - x0) * (x - x0) + (y - y0) * (y - y0)).sqrt()).fold(f64::INFINITY, f64::min);
            assert!((nearest - hole).abs() < 0.1, "{name}: {nearest}");
        }
    }

    #[test]
    fn ts11c_sews_each_sample_where_its_drawing_puts_it() {
        // The sheet is planned with its middle on the hoop's: its drawing spans x from the star's left point to the
        // leaf's tip, and y from the top discs to the star's lower points. Each sample's needle points lie in its
        // shape's bounds, its underlay reaching to within a millimetre of each side of them.
        let plan = crate::testsheets::find("TS-11C").unwrap().plan().unwrap();
        let [left, middle, right] = ACROSS;
        let [top, centre, bottom] = DOWN;
        let (star, leaf) = (bounds(star((left, bottom))), bounds(leaf((right, bottom), 19.0, 10.0)));
        let mid = plan.bounds().unwrap().center();
        let (dx, dy) = (mid.x() - (star[0] + leaf[2]) / 2.0, mid.y() - (top - DISC + star[3]) / 2.0);
        let sewn = |name: &str| -> Vec<(f64, f64)> {
            let element = plan.elements.iter().position(|e| e.as_str() == format!("ts11c:{name}")).unwrap();
            let mine = |s: &&stitchcraft_plan::Stitch| {
                s.kind == StitchKind::Normal && s.origin.role != Role::Lock && s.origin.element.map(|e| e.index()) == Some(element)
            };
            plan.stitches().filter(mine).map(|s| (s.at.x() - dx, s.at.y() - dy)).collect()
        };
        let disc = |x: f64, y: f64| [x - DISC, y - DISC, x + DISC, y + DISC];
        let expected = [
            ("honeycomb", disc(left, top)),
            ("squares", disc(middle, top)),
            ("triangles", disc(right, top)),
            ("octagons-squares", disc(left, centre)),
            ("hexagons-triangles", disc(middle, centre)),
            ("hexagons-squares-triangles", disc(right, centre)),
            ("squares-triangles", star),
            ("zigzag", [middle - 18.0, bottom - 12.0, middle + 18.0, bottom + 12.0]),
            ("turned", leaf),
        ];
        for (name, drawn) in expected {
            let [x0, y0, x1, y1] = bounds(sewn(name));
            let inside = x0 > drawn[0] - 0.1 && y0 > drawn[1] - 0.1 && x1 < drawn[2] + 0.1 && y1 < drawn[3] + 0.1;
            let reaching = x0 < drawn[0] + 1.0 && y0 < drawn[1] + 1.0 && x1 > drawn[2] - 1.0 && y1 > drawn[3] - 1.0;
            assert!(inside && reaching, "{name}: sewn {:?}, drawn {drawn:?}", [x0, y0, x1, y1]);
        }
    }
}
