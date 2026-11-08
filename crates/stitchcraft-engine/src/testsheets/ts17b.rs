//! TS-17B — the new fills, for MC-12: the tartans of M10.6 and M10.7 so far ([`tartans`]), each a tartan fill
//! 40 mm square with a sett of its own, as Ink/Stitch's tartan editor saves it, and the linear gradients of
//! M10.12 below them ([`gradients`]).
//!
//! Drawn as a design (`Drawn::Designed`): from M10.7 VectorCraft's fill dialog holds a tartan's sett, so the
//! Test Sheet filter draws the sheet too, each gradient as a gradient paint. Each sample is trimmed after each
//! colour. A tartan sews its underlay in a thread of its own, a colour of its sett other than its first
//! stripe's: sewn in that colour, the underlay and the first colour would be one colour block, and the jump
//! between them would leave its thread under the fill on the reference machine (MC-1). A gradient's underlay
//! sews in its first stop's colour, as Ink/Stitch sews it, and its first colour runs on from it along the
//! outline, without a jump.
//!
//! - **Top row**: a sett mirrored at its ends, one repeated, its code the list Ink/Stitch's editor writes for
//!   a repeated sett, and a warp and a weft that differ.
//! - **Middle row**: a sett with stripes narrow enough to sew as runs along their middles, one turned 30°
//!   and scaled by 150 %, and a herringbone 3 mm wide (M10.7), its rows in bands that slant one way and then
//!   the other.
//! - **Bottom row**: linear gradient fills of 2 stops and of 3, the middle stop of the last transparent, each
//!   at a slant of its own (M10.12).
//!
//! Every stripe's colour is one of Brother's threads, so that both programs' PES files name each thread as
//! itself and the differential compares them as they are. A stripe sews in a thread of its colour alone,
//! unnamed, and so does each tartan's own thread here: a named and an unnamed thread of one colour are 2
//! threads, which a PES file writes as 2 entries of Brother's palette (`REQ-THREAD-002`), the second of them
//! another colour. One colour is then one thread across the sheet, as `REQ-RND-001`'s read-back expects. A
//! gradient's own thread is its first stop's colour, unnamed, which the SVG and VectorCraft readers give it
//! back as.
//!
//! The gradients slant, so that no gradient's rows fit its reach in a whole number of row spacings, nor
//! does a stop lie half way between 2 rows: Ink/Stitch counts them in pixels, and its rounding there could
//! give it a row more (`docs/src/design/algorithms/gradient.md`).

use stitchcraft_plan::Thread;

use super::designed::{Drawing, point, polygon};
use super::sketch::SheetError;
use super::{BLACK, BLUE, DEEP_GOLD, EMERALD_GREEN, PRUSSIAN_BLUE, RED, TANGERINE, WHITE, thread};
use crate::design::{Gradient, GradientStop};

/// The side of each tartan, and how far apart their corners lie (mm).
const SIDE: f64 = 40.0;
const PITCH: f64 = 45.0;

/// TS-17B's tartans: each one's name, the Brother PEC index of its own thread, which its underlay sews in,
/// its sett as Ink/Stitch's tartan editor saves it, and its herringbone's width in millimetres, 0 for none.
///
/// - **mirrored**: red, white, Prussian blue and deep gold, mirrored at its first and last stripes, the weft
///   as the warp.
/// - **repeated**: emerald green, black and white, repeated.
/// - **apart**: a warp of red and black, and a weft of blue and white.
/// - **runs**: blue and black, with a white stripe 0.5 mm wide and a red one 0.8 mm wide, each sewn as a run
///   along its middle after the fills.
/// - **turned**: deep gold, black and tangerine, turned 30° and scaled by 150 %.
/// - **herringbone**: blue, white, red and black, mirrored, in a herringbone 3 mm wide.
const TARTANS: [(&str, u8, &str, &str); 6] = [
    (
        "mirrored",
        PRUSSIAN_BLUE,
        r#"{"symmetry": true, "rotate": 0.0, "scale": 100, "offset_x": 0.0, "offset_y": 0.0, "palette": "(#ed171f)/4.0 (#f0f0f0)1.5 (#0e1f7c)3.0 (#e8a900)/2.0", "min_stripe_width": 1.0}"#,
        "0",
    ),
    (
        "repeated",
        BLACK,
        r#"{"symmetry": false, "rotate": 0.0, "scale": 100, "offset_x": 0.0, "offset_y": 0.0, "palette": "...['(#00673e)5.0', '(#000000)1.5', '(#00673e)2.0', '(#f0f0f0)2.0']...", "min_stripe_width": 1.0}"#,
        "0",
    ),
    (
        "apart",
        BLUE,
        r#"{"symmetry": true, "rotate": 0.0, "scale": 100, "offset_x": 0.0, "offset_y": 0.0, "palette": "(#ed171f)/3.0 (#000000)2.0 (#ed171f)/1.5 | (#0a55a3)/2.5 (#f0f0f0)2.0 (#0a55a3)/2.0", "min_stripe_width": 1.0}"#,
        "0",
    ),
    (
        "runs",
        BLACK,
        r#"{"symmetry": true, "rotate": 0.0, "scale": 100, "offset_x": 0.0, "offset_y": 0.0, "palette": "(#0a55a3)/4.0 (#f0f0f0)0.5 (#0a55a3)3.0 (#ed171f)0.8 (#000000)/4.0", "min_stripe_width": 1.0}"#,
        "0",
    ),
    (
        "turned",
        TANGERINE,
        r#"{"symmetry": true, "rotate": 30.0, "scale": 150, "offset_x": 0.0, "offset_y": 0.0, "palette": "(#e8a900)/2.0 (#000000)1.5 (#fe9e32)/2.5", "min_stripe_width": 1.0}"#,
        "0",
    ),
    (
        "herringbone",
        BLACK,
        r#"{"symmetry": true, "rotate": 0.0, "scale": 100, "offset_x": 0.0, "offset_y": 0.0, "palette": "(#0a55a3)/3.0 (#f0f0f0)1.5 (#ed171f)2.0 (#000000)/3.0", "min_stripe_width": 1.0}"#,
        "3",
    ),
];

/// A stop of a gradient: its offset, the Brother PEC index of its colour, and its opacity.
type Stop = (f64, u8, f64);

/// A gradient of the sheet: its sample's name, its stops, and its start and end from its square's top left
/// corner.
type Sample = (&'static str, &'static [Stop], (f64, f64), (f64, f64));

/// TS-17B's gradients, in its bottom row: each one's name, its stops, and its start and end, from its
/// square's top left corner. Each offset is a binary fraction, which VectorCraft's stops, of 32 bits, keep.
///
/// - **two**: red to blue, up to the right, from near the square's bottom left corner to near its top right.
/// - **three**: deep gold, white and emerald green, down to the right and steeper.
/// - **clear**: tangerine to Prussian blue, down to the left, through a transparent white half way.
const GRADIENTS: [Sample; 3] = [
    ("two", &[(0.0, RED, 1.0), (1.0, BLUE, 1.0)], (2.0, 36.0), (38.0, 6.0)),
    ("three", &[(0.0, DEEP_GOLD, 1.0), (0.5, WHITE, 1.0), (1.0, EMERALD_GREEN, 1.0)], (6.0, 4.0), (30.0, 37.0)),
    ("clear", &[(0.0, TANGERINE, 1.0), (0.5, WHITE, 0.0), (1.0, PRUSSIAN_BLUE, 1.0)], (31.0, 3.0), (11.0, 39.0)),
];

/// Draws TS-17B: the tartans 40 mm square, 3 to a row, each trimmed after, the herringbone of M10.7 last in
/// the second row, and the gradients below them. The guided fills of M10.9 and M10.10 come below the
/// gradients.
pub(super) fn samples() -> Result<Drawing, SheetError> {
    let mut d = tartans()?;
    gradients(&mut d)?;
    Ok(d)
}

/// The tartans, 3 to a row from the top left.
fn tartans() -> Result<Drawing, SheetError> {
    let mut d = Drawing::new("ts17b");
    for (i, (name, own, sett, herringbone)) in TARTANS.iter().enumerate() {
        let (x, y) = ((i % 3) as f64 * PITCH, (i / 3) as f64 * PITCH);
        let outline = polygon(&[(x, y), (x + SIDE, y), (x + SIDE, y + SIDE), (x, y + SIDE)])?;
        // The Brother thread's colour alone, as the stripes' threads are (the module's docs).
        let own = Thread::new(thread(*own)?.color);
        let mut params = vec![("fill_method", "tartan_fill"), ("tartan", *sett), ("trim_after", "true")];
        if *herringbone != "0" {
            params.push(("herringbone_width_mm", *herringbone));
        }
        d.fill(name, vec![outline], &own, &params)?;
    }
    Ok(d)
}

/// The gradients, each a linear gradient fill 40 mm square in the third row, in the thread of its first
/// stop's colour alone.
fn gradients(d: &mut Drawing) -> Result<(), SheetError> {
    let y = 2.0 * PITCH;
    for (i, (name, stops, (x1, y1), (x2, y2))) in GRADIENTS.iter().enumerate() {
        let x = i as f64 * PITCH;
        let outline = polygon(&[(x, y), (x + SIDE, y), (x + SIDE, y + SIDE), (x, y + SIDE)])?;
        let mut sewn = Vec::new();
        for &(offset, index, opacity) in stops.iter() {
            sewn.push(GradientStop { offset, colour: thread(index)?.color, opacity });
        }
        let own = Thread::new(sewn.first().map_or(thread(BLACK)?.color, |stop| stop.colour));
        d.fill(name, vec![outline], &own, &[("fill_method", "linear_gradient_fill"), ("trim_after", "true")])?;
        d.gradient(Gradient { start: point((x + x1, y + y1))?, end: point((x + x2, y + y2))?, stops: sewn });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use stitchcraft_core::{Point, math};
    use stitchcraft_plan::palette::BROTHER_PEC;
    use stitchcraft_plan::{Role, StitchKind};

    #[test]
    fn ts17b_sews_each_sample_s_underlay_then_each_colour_once() {
        let plan = crate::testsheets::find("TS-17B").unwrap().plan().unwrap();
        // Each colour block's sample and thread.
        let blocks: Vec<(String, String)> = plan
            .blocks
            .iter()
            .map(|block| {
                let element = block.stitches.iter().find_map(|s| s.origin.element).unwrap();
                let name = plan.elements[element.index()].as_str().strip_prefix("ts17b:").unwrap().to_string();
                (name, block.thread.color.to_string())
            })
            .collect();
        // Each tartan's underlay in its own thread, then the warp's colours and the weft's in the order the
        // stripes first reach them, and the narrow stripes' runs last. Each gradient's underlay in its first
        // colour, in the block of that colour's rows, then its other colours in the stops' order, a transparent
        // one's left out.
        let expected = [
            ("mirrored", "#0e1f7c"),
            ("mirrored", "#ed171f"),
            ("mirrored", "#f0f0f0"),
            ("mirrored", "#0e1f7c"),
            ("mirrored", "#e8a900"),
            ("repeated", "#000000"),
            ("repeated", "#00673e"),
            ("repeated", "#000000"),
            ("repeated", "#f0f0f0"),
            ("apart", "#0a55a3"),
            ("apart", "#ed171f"),
            ("apart", "#000000"),
            ("apart", "#0a55a3"),
            ("apart", "#f0f0f0"),
            ("runs", "#000000"),
            ("runs", "#0a55a3"),
            ("runs", "#000000"),
            ("runs", "#f0f0f0"),
            ("runs", "#ed171f"),
            ("turned", "#fe9e32"),
            ("turned", "#e8a900"),
            ("turned", "#000000"),
            ("turned", "#fe9e32"),
            ("herringbone", "#000000"),
            ("herringbone", "#ed171f"),
            ("herringbone", "#000000"),
            ("herringbone", "#0a55a3"),
            ("herringbone", "#f0f0f0"),
            ("two", "#ed171f"),
            ("two", "#0a55a3"),
            ("three", "#e8a900"),
            ("three", "#f0f0f0"),
            ("three", "#00673e"),
            ("clear", "#fe9e32"),
            ("clear", "#0e1f7c"),
        ];
        let found: Vec<(&str, &str)> = blocks.iter().map(|(name, colour)| (name.as_str(), colour.as_str())).collect();
        assert_eq!(found, expected);
        // One colour is one thread, so the PES file writes each block as the Brother entry of its colour: a
        // tartan's own thread named, beside its stripes' unnamed threads, wrote a second Prussian Blue as
        // Purple and a second Black as Dark Gray.
        let entries = BROTHER_PEC.assign(plan.color_entries().iter().map(|entry| entry.thread)).unwrap().entries;
        let written: Vec<String> = entries.iter().map(|entry| entry.color.to_string()).collect();
        assert_eq!(written, expected.map(|(_, colour)| colour.to_string()));
        let top = |block: usize| -> Vec<Point> {
            plan.blocks[block].stitches.iter().filter(|s| s.kind == StitchKind::Normal && s.origin.role == Role::Top).map(|s| s.at).collect()
        };
        // The runs lie along their stripes, upright for the warp and level for the weft: no stitch of theirs
        // slants as the rows do.
        for block in [17, 18] {
            let runs = top(block);
            assert!(runs.len() > 100, "{}", runs.len());
            assert!(runs.windows(2).all(|w| (w[0].x() - w[1].x()).abs() < 0.01 || (w[0].y() - w[1].y()).abs() < 0.01));
        }
        // The turned sample's stripes are turned 30° clockwise as the canvas shows them, their edges at 30° and
        // 120° to its x axis, which points right with y down. Most stitches of its gold rows run at 45° to
        // them, at 165°, and most others join the rows along the stripes' edges.
        let rows = top(20);
        let slant = |a: Point, b: Point| math::to_degrees(math::atan2(b.y() - a.y(), b.x() - a.x())).rem_euclid(180.0);
        let at = |angles: &[f64]| rows.windows(2).filter(|w| angles.iter().any(|&angle| (slant(w[0], w[1]) - angle).abs() < 0.5)).count();
        assert!(at(&[165.0]) * 2 > rows.len(), "{} of {}", at(&[165.0]), rows.len());
        assert!(at(&[30.0, 120.0]) * 4 > rows.len(), "{} of {}", at(&[30.0, 120.0]), rows.len());
        // The herringbone's rows slant at 45° to its stripes, down to the right in some bands and up to the
        // right in the others, and each colour sews both.
        for block in 24..28 {
            let rows = top(block);
            let count = |angle: f64| rows.windows(2).filter(|w| w[0].distance(w[1]) > 0.5 && (slant(w[0], w[1]) - angle).abs() < 0.5).count();
            assert!(count(45.0) > 20 && count(135.0) > 20, "{block}: {} and {}", count(45.0), count(135.0));
        }
        // The clear sample's gradient runs down to the left, 20 mm left for each 36 mm down. Its rows lie square
        // to it, at 29.05° to the x axis, and its underlay along it, at 119.05°: most of tangerine's stitches run
        // one way or the other, the rest along the square's sides between rows. Tangerine's rows lie on the
        // gradient's start's side of blue's.
        let tangerine = top(33);
        let count = |angle: f64| tangerine.windows(2).filter(|w| (slant(w[0], w[1]) - angle).abs() < 0.5).count();
        let (rows, underlay) = (count(29.054_604), count(119.054_604));
        assert!(rows > 300 && underlay > 300 && (rows + underlay) * 4 > tangerine.len() * 3, "{rows} and {underlay} of {}", tangerine.len());
        let along = |points: &[Point]| points.iter().map(|q| -20.0 * q.x() + 36.0 * q.y()).sum::<f64>() / points.len() as f64;
        assert!(along(&tangerine) < along(&top(34)));
        // The gradients lie side by side, 45 mm apart: the first block of each, which holds its underlay over
        // the whole square, reaches the square's top left corner.
        let corner = |block: usize| {
            let at = plan.blocks[block].stitches.iter().filter(|s| s.kind == StitchKind::Normal).map(|s| s.at);
            at.fold((f64::INFINITY, f64::INFINITY), |(x, y), q| (x.min(q.x()), y.min(q.y())))
        };
        let two = corner(28);
        for (block, x) in [(30, 45.0), (33, 90.0)] {
            let (cx, cy) = corner(block);
            assert!((cx - two.0 - x).abs() < 0.5 && (cy - two.1).abs() < 0.5, "{block}: ({cx}, {cy}) beside {two:?}");
        }
    }
}
