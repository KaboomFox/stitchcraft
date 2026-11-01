//! TS-13 — pictures digitized automatically (M13), for MC-8: the test pictures (`pictures`), each drawn
//! twice, digitized automatically in the top half and in paint order in the bottom half, as the shapes
//! without settings that `stitch plan` sews without `--auto`. Sewn side by side on one piece of fabric, the
//! halves show what digitizing changes, picture by picture.
//!
//! - **TS-13A**, neighbours and sewing order (M13.2, M13.3): a badge traced abutting and one traced stacked,
//!   specks round a disc, a row of badges drawn badge by badge, and a shape that runs on beside another
//!   without touching it.
//! - **TS-13B**, letters and lines traced as areas (M13.4, M13.5): the word MATCH in block letters 15, 10,
//!   6 and 4 mm tall, and a window of 3 coloured areas between lines 0.3, 0.6 and 0.9 mm wide, each with a
//!   junction.
//! - **TS-13C**, lines and shapes in parts (M13.1, M13.6): a cartoon face drawn with lines 1, 2 and 4 mm
//!   wide, with round caps, small rings and hairpins, and a star and a balloon on its string, broad in places
//!   and narrow in others.
//!
//! The half in paint order is drawn first, and its last shape stops the machine: nothing moves across a
//! stop, so the digitized half sews after it in the order automatic digitizing chooses for it alone, and
//! the time, thread changes, jumps and trims of each half can be read on the machine. Drawn as a design
//! (`designed`), the sheet sews as `stitch plan` sews a design, and says exactly what the engine says of
//! its pictures: which shapes it digitized, what it did with the specks and the stacked badge, and so on.

use stitchcraft_core::Code::{
    self, AutoDigitized, AutoOrder, AutoSatinFilled, AutoSpeck, AutoUnder, FillPartsApart, SatinStitchesLean, StitchesMerged,
};

use super::designed::Drawing;
use super::pictures::{self, Picture, Trace};
use super::sketch::SheetError;

/// What the engine says of TS-13A, in order: how many shapes it digitized; each speck round the disc, 6
/// merged into it and 1 left out; the order; the areas that reach under others and the 2 cut away under the
/// stacked badge's upper colours; the specks in paint order, sewn in parts; and the needle points left out
/// where rows end close together.
pub(super) const NEIGHBOURS_SAY: [Code; 12] = [
    AutoDigitized,
    AutoSpeck,
    AutoSpeck,
    AutoSpeck,
    AutoSpeck,
    AutoSpeck,
    AutoSpeck,
    AutoSpeck,
    AutoOrder,
    AutoUnder,
    FillPartsApart,
    StitchesMerged,
];

/// What the engine says of TS-13B, in order: how many shapes it digitized, the order, the areas that reach
/// under others, the stitches that lean where the M's strokes meet at sharp angles, and the needle points
/// left out.
pub(super) const LETTERS_SAY: [Code; 10] = [
    AutoDigitized,
    AutoOrder,
    AutoUnder,
    SatinStitchesLean,
    SatinStitchesLean,
    SatinStitchesLean,
    SatinStitchesLean,
    SatinStitchesLean,
    SatinStitchesLean,
    StitchesMerged,
];

/// What the engine says of TS-13C, in order: how many shapes it digitized, the pupils and hairs that no
/// satin column can follow, filled; the order; the areas that reach under others; the eyes, sewn in parts in
/// both halves; and the needle points left out.
pub(super) const LINES_AND_PARTS_SAY: [Code; 11] = [
    AutoDigitized,
    AutoSatinFilled,
    AutoSatinFilled,
    AutoSatinFilled,
    AutoSatinFilled,
    AutoSatinFilled,
    AutoOrder,
    AutoUnder,
    FillPartsApart,
    FillPartsApart,
    StitchesMerged,
];

/// The check of every TS-13 sheet that asks where a jump thread is sewn down: the reference machine cuts
/// none (MC-1), and the sheets sew their pictures as the engine plans any design.
pub(super) const JUMP_THREADS: &str = "The machine cuts the thread at thread changes and at the stop, but at no jump, and the pictures sew as any design does: a shape sewn after a jump may sew its thread down. Snip the jump threads at each stop and after sewing. Where is one sewn down, and does it show?";

/// A sheet's pictures, each where it lies in the half whose top is at the y given (mm).
type Pictures = fn(f64) -> Result<Vec<Picture>, SheetError>;

/// Draws TS-13A: neighbours and sewing order.
pub(super) fn neighbours() -> Result<Drawing, SheetError> {
    halves("ts13a", 90.0, |top| {
        Ok(vec![
            pictures::badge((20.0, top + 20.0), 0.8, Trace::Abutting)?,
            pictures::badge((66.0, top + 20.0), 0.8, Trace::Stacked)?,
            pictures::specks((110.0, top + 20.0))?,
            pictures::badges((45.0, top + 63.0), 0.6, Trace::Abutting)?,
            pictures::parted((98.0, top + 51.0))?,
        ])
    })
}

/// Draws TS-13B: letters and lines traced as areas.
pub(super) fn letters() -> Result<Drawing, SheetError> {
    halves("ts13b", 80.0, |top| {
        Ok(vec![
            pictures::word((0.0, top), 15.0)?,
            pictures::word((0.0, top + 21.0), 10.0)?,
            pictures::word((0.0, top + 37.0), 6.0)?,
            pictures::word((0.0, top + 49.0), 4.0)?,
            pictures::window((80.0, top), 0.3)?,
            pictures::window((80.0, top + 24.0), 0.6)?,
            pictures::window((80.0, top + 48.0), 0.9)?,
        ])
    })
}

/// Draws TS-13C: lines, and shapes in parts.
pub(super) fn lines_and_parts() -> Result<Drawing, SheetError> {
    halves("ts13c", 60.0, |top| {
        Ok(vec![pictures::face((24.0, top + 26.0))?, pictures::star((70.0, top + 24.0), 16.0)?, pictures::balloon((104.0, top + 11.0))?])
    })
}

/// The sheet `sheet` of `pictures`: drawn as they are, in paint order, `below` mm below the top, and stopped
/// after; then digitized automatically at the top.
fn halves(sheet: &'static str, below: f64, pictures: Pictures) -> Result<Drawing, SheetError> {
    let mut d = Drawing::new(sheet);
    for picture in pictures(below)? {
        d.picture("drawn", picture, false)?;
    }
    d.stop();
    for picture in pictures(0.0)? {
        d.picture("auto", picture, true)?;
    }
    Ok(d)
}

#[cfg(test)]
mod tests {
    use stitchcraft_core::units::to_tenths;
    use stitchcraft_plan::{StitchKind, StitchPlan};

    use super::super::find;
    use crate::common::STOP_AFTER;

    /// The TS-13 sheets.
    const SHEETS: [&str; 3] = ["TS-13A", "TS-13B", "TS-13C"];

    #[test]
    fn ts13_sheets_have_the_sizes_and_stitches_their_cases_and_the_changelog_give() {
        // Each sheet's size, its conformance case's to the micrometre, and its stitches: what MC-8 sews, which
        // changes with any change to what a picture sews.
        for (id, size, stitches) in
            [("TS-13A", [125.992, 168.0], 24_286), ("TS-13B", [110.453, 148.4], 14_401), ("TS-13C", [111.391, 107.74], 10_311)]
        {
            let plan = find(id).unwrap().plan().unwrap();
            let bounds = plan.bounds().unwrap();
            assert_eq!([bounds.width(), bounds.height()].map(|v| (v * 1e3).round() / 1e3), size, "{id}");
            assert_eq!(plan.stats().stitches, stitches, "{id}");
        }
    }

    #[test]
    fn ts13_draws_each_picture_as_drawn_below_and_digitized_above() {
        for id in SHEETS {
            let design = find(id).unwrap().design().unwrap().unwrap();
            let sheet = id.to_ascii_lowercase().replace('-', "");
            let half =
                |name: &str| -> Vec<_> { design.elements().iter().filter(|e| e.id.as_str().starts_with(&format!("{sheet}:{name}:"))).collect() };
            let (drawn, digitized) = (half("drawn"), half("auto"));
            // The same shapes, the half as drawn first, then the digitized half: every element is in one of them.
            assert_eq!(drawn.len() + digitized.len(), design.elements().len(), "{id}");
            let named = |elements: &[&crate::design::Element], half: &str| -> Vec<String> {
                elements.iter().map(|e| e.id.as_str().replacen(&format!(":{half}:"), ":", 1)).collect()
            };
            assert_eq!(named(&drawn, "drawn"), named(&digitized, "auto"), "{id}");
            assert!(design.elements().iter().take(drawn.len()).all(|e| drawn.contains(&e)), "{id}: the half as drawn comes first");
            // The half as drawn has no settings but the stop after its last shape, which nothing digitized has.
            let (last, rest) = drawn.split_last().unwrap();
            assert!(!last.automatic && last.params.get(STOP_AFTER) == Some("true"), "{id}");
            assert!(rest.iter().all(|e| !e.automatic && e.params.iter().next().is_none()), "{id}");
            assert!(digitized.iter().all(|e| e.automatic && e.params.iter().next().is_none()), "{id}");
            // The half as drawn lies below the digitized one, which is at the top.
            let top = |elements: &[&crate::design::Element]| {
                elements.iter().flat_map(|e| e.shape.path().points()).map(|p| p.y()).fold(f64::INFINITY, f64::min)
            };
            assert!(top(&digitized) < 1.0 && top(&drawn) > 50.0, "{id}");
        }
    }

    /// Each half of `plan`'s thread changes and jumps, as the machine file has them: the half as drawn, up to
    /// its stop, and then the digitized one. A colour block the stop falls in is sewn by both, its thread
    /// changed by neither. A jump counts when it moves the needle on the machine's 0.1 mm grid: the writers
    /// leave out a jump that goes nowhere, which plan assembly adds where a part starts at the needle.
    fn halves(plan: &StitchPlan) -> [(usize, usize); 2] {
        let (mut blocks, mut jumps, mut half, mut needle) = ([Vec::new(), Vec::new()], [0, 0], 0, None);
        for (k, block) in plan.blocks.iter().enumerate() {
            for s in &block.stitches {
                if !blocks[half].contains(&k) {
                    blocks[half].push(k);
                }
                let at = Some((to_tenths(s.at.x()), to_tenths(s.at.y())));
                match s.kind {
                    StitchKind::Stop => half = 1,
                    StitchKind::Jump if at != needle => jumps[half] += 1,
                    _ => {}
                }
                needle = at;
            }
        }
        [0, 1].map(|h| (blocks[h].len() - 1, jumps[h]))
    }

    #[test]
    fn ts13_stops_between_its_halves_whose_thread_changes_and_jumps_its_checks_count() {
        for id in SHEETS {
            let sheet = find(id).unwrap();
            let plan = sheet.plan().unwrap();
            // One stop, at the end of the half as drawn: every stitch of its shapes before every stitch of the
            // digitized ones.
            assert_eq!(plan.stats().stops, 1, "{id}");
            let halves_sewn: Vec<bool> =
                plan.stitches().filter_map(|s| s.origin.element).map(|e| plan.elements[e.index()].as_str().contains(":auto:")).collect();
            assert!(halves_sewn.windows(2).all(|w| w[0] <= w[1]), "{id}: the halves are sewn one after the other");
            let [(a, b), (c, d)] = halves(&plan);
            let said = format!("The bottom half changes thread {a} times and jumps {b} times, the top half {c} and {d} times.");
            assert!(sheet.checks.iter().any(|check| check.contains(&said)), "{id}: {said}");
            // Digitized, no sheet changes thread more often. A letter digitized is columns, with jumps between
            // them, where a letter filled sews in one piece: TS-13B jumps more than twice as often digitized.
            assert!(c <= a, "{id}: {said}");
            assert_eq!(d > 2 * b, id == "TS-13B", "{id}: {said}");
        }
    }

    #[test]
    fn ts13a_says_which_of_the_square_and_the_bar_reaches_under_the_other() {
        // Digitized, the bar sews first, in the blue the half as drawn ends with, and reaches 0.4 mm under the
        // square where they meet. The square, sewn after it, keeps its edge (REQ-AUTO-007). As drawn, neither
        // reaches under the other.
        let sheet = find("TS-13A").unwrap();
        let plan = sheet.plan().unwrap();
        // The first stitch of the element `name`, and the least and the greatest x of its stitches.
        let extent = |name: &str| {
            let sewn = plan.stitches().enumerate().filter(|(_, s)| s.kind == StitchKind::Normal);
            let mine: Vec<(usize, f64)> = sewn
                .filter(|(_, s)| s.origin.element.is_some_and(|e| plan.elements[e.index()].as_str() == name))
                .map(|(i, s)| (i, s.at.x()))
                .collect();
            let (first, xs) = (mine.first().unwrap().0, mine.iter().map(|&(_, x)| x));
            (first, xs.clone().fold(f64::INFINITY, f64::min), xs.fold(f64::NEG_INFINITY, f64::max))
        };
        let (bar, square) = (extent("ts13a:auto:parted:bar"), extent("ts13a:auto:parted:square"));
        let (drawn_bar, drawn_square) = (extent("ts13a:drawn:parted:bar"), extent("ts13a:drawn:parted:square"));
        assert!(bar.0 < square.0, "the bar sews first");
        assert!((drawn_bar.1 - bar.1 - 0.4).abs() < 1e-6, "the bar reaches 0.4 mm under the square: {bar:?}, drawn {drawn_bar:?}");
        assert!((square.1 - drawn_square.1).abs() < 1e-6 && (square.2 - drawn_square.2).abs() < 1e-6, "{square:?}, drawn {drawn_square:?}");
        assert!(sheet.checks.iter().any(|check| check.contains("the bar, sewn first, reaches under the square only where they meet")));
    }
}
