//! TS-17B — the new fills, for MC-12 ([`sheet`]): the tartans of M10.6 and M10.7, each a tartan fill 40 mm
//! square with a sett of its own, as Ink/Stitch's tartan editor saves it, and from M10.9 a leaf whose guided
//! fill copies an S-shaped guide ([`guided`]).
//!
//! Drawn as a design (`Drawn::Designed`). From M10.7 VectorCraft's fill dialog holds a tartan's sett, and the
//! Test Sheet filter drew the sheet too. From M10.9 the leaf's guide is a marker a host would group with the
//! leaf, which a filter can't, so only `stitch testsheet` draws the sheet (`TestSheet::uses_file_features`).
//! Each tartan is trimmed after each colour, and sews its underlay in a thread of its own, a colour of its
//! sett other than its first stripe's: sewn in that colour, the underlay and the first colour would be one
//! colour block, and the jump between them would leave its thread under the fill on the reference machine
//! (MC-1).
//!
//! - **Top row**: a sett mirrored at its ends, one repeated, its code the list Ink/Stitch's editor writes for
//!   a repeated sett, and a warp and a weft that differ.
//! - **Second row**: a sett with stripes narrow enough to sew as runs along their middles, one turned 30°
//!   and scaled by 150 %, and a herringbone 3 mm wide (M10.7), its rows in bands that slant one way and then
//!   the other.
//! - **Fourth row**, from y = 135: the leaf, 48 mm from tip to tip, from its bottom left to its top right
//!   corner of the 40 mm square from x = 0, its rows copies of a guide that bends one way and then the other
//!   between the tips (`REQ-FILL-GDD-008`). The third row is the gradient fills', and the rest of the fourth
//!   the leaves that M10.10 offsets and buffers.
//!
//! Every stripe's colour is one of Brother's threads, so that both programs' PES files name each thread as
//! itself and the differential compares them as they are. A stripe sews in a thread of its colour alone,
//! unnamed, and so does each tartan's own thread here: a named and an unnamed thread of one colour are 2
//! threads, which a PES file writes as 2 entries of Brother's palette (`REQ-THREAD-002`), the second of them
//! another colour. One colour is then one thread across the sheet, as `REQ-RND-001`'s read-back expects.

use stitchcraft_core::Point;
use stitchcraft_plan::Thread;

use super::designed::{Drawing, polygon};
use super::sketch::SheetError;
use super::{BLACK, BLUE, EMERALD_GREEN, PRUSSIAN_BLUE, TANGERINE, thread};
use crate::design::{Segment, Subpath};

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

/// Draws TS-17B: the tartans 40 mm square, 3 to a row, each trimmed after, the herringbone of M10.7 last in
/// the second row, and the guided leaf in the fourth ([`guided`]). The gradient fills come between.
pub(super) fn sheet() -> Result<Drawing, SheetError> {
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
    guided(&mut d)?;
    Ok(d)
}

/// The leaf's tips, at the bottom left and the top right of its square (mm).
const TIPS: [(f64, f64); 2] = [(3.0, 172.0), (37.0, 138.0)];

/// The leaf's sides' control points, from the bottom tip over the top left side and back over the bottom right,
/// 11 mm from the line between the tips at a quarter and three quarters of the way, and its guide's, 9 mm from
/// that line on one side and then the other at a third and two thirds of the way (mm).
const SIDES: [[(f64, f64); 2]; 2] = [[(3.72, 155.72), (20.72, 138.72)], [(36.28, 154.28), (19.28, 171.28)]];
const GUIDE: [(f64, f64); 2] = [(7.86, 154.42), (32.14, 155.58)];

/// Draws the guided leaf of M10.9 in emerald green, the colour alone as the stripes' threads are (the module's
/// docs): a guided fill with its defaults, its copies 0.25 mm apart over the tatami fill's underlay, trimmed
/// after, and its guide line from tip to tip, which bends with the S of a stem.
pub(super) fn guided(d: &mut Drawing) -> Result<(), SheetError> {
    let at = |(x, y): (f64, f64)| Point::new(x, y);
    let curve = |[a, b]: [(f64, f64); 2], to: (f64, f64)| -> Result<Segment, SheetError> { Ok(Segment::Cubic(at(a)?, at(b)?, at(to)?)) };
    let [bottom, top] = TIPS;
    let [left, right] = SIDES;
    let outline = Subpath { start: at(bottom)?, segments: vec![curve(left, top)?, curve(right, bottom)?], closed: true };
    let green = Thread::new(thread(EMERALD_GREEN)?.color);
    d.fill("guided:copied", vec![outline], &green, &[("fill_method", "guided_fill"), ("trim_after", "true")])?;
    let guide = Subpath { start: at(bottom)?, segments: vec![curve(GUIDE, top)?], closed: false };
    d.guide(vec![guide], 0.3, &[])
}

#[cfg(test)]
mod tests {
    use stitchcraft_core::{Point, math};
    use stitchcraft_plan::palette::BROTHER_PEC;
    use stitchcraft_plan::{Role, StitchKind};

    #[test]
    fn ts17b_s_guided_leaf_sews_its_rows_along_its_guide_without_a_word() {
        // The leaf's rows are copies of its S-shaped guide, which leaves its tips at 105.5° to the x axis, as the
        // canvas shows it with y down, and crosses its middle at 150.6°: its rows' stitches slant from one to the
        // other, where a tatami fill at the angle of the line between the tips, 135°, would sew them all at that.
        let design = crate::testsheets::find("TS-17B").unwrap().design().unwrap().unwrap();
        let leaf = design.elements().iter().find(|e| e.id.as_str() == "ts17b:guided:copied").unwrap();
        assert!(leaf.hints.guide.is_some());
        let plan = crate::testsheets::find("TS-17B").unwrap().plan().unwrap();
        let block = plan.blocks.last().unwrap();
        let top: Vec<Point> = block.stitches.iter().filter(|s| s.kind == StitchKind::Normal && s.origin.role == Role::Top).map(|s| s.at).collect();
        let slant = |a: Point, b: Point| math::to_degrees(math::atan2(b.y() - a.y(), b.x() - a.x())).rem_euclid(180.0);
        let long: Vec<f64> = top.windows(2).filter(|w| w[0].distance(w[1]) > 1.5).map(|w| slant(w[0], w[1])).collect();
        let spread = |from: f64, to: f64| long.iter().filter(|a| (from..to).contains(*a)).count();
        assert!(long.len() > 500, "{}", long.len());
        assert!(spread(100.0, 125.0) > 50 && spread(145.0, 160.0) > 200, "{} {}", spread(100.0, 125.0), spread(145.0, 160.0));
    }

    #[test]
    fn ts17b_sews_each_tartan_s_underlay_in_its_own_thread_then_each_colour_once() {
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
        // Each sample's underlay in its own thread, then the warp's colours and the weft's in the order the
        // stripes first reach them, and the narrow stripes' runs last.
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
            ("guided:copied", "#00673e"),
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
    }
}
