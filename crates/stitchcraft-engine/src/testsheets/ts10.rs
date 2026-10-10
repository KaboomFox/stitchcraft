//! TS-10 — hoop size: a frame as large as one hoop's field, with a centre cross and an "F".
//!
//! Three sheets, one for each hoop of the reference machine: the 4 × 4 in (TS-10A), the 5 × 7 in (TS-10B)
//! and the small hoop (TS-10C). Each frame is its profile's whole field, so the sheets test the profiles:
//! does the machine take each design from a PES v1 file in its hoop, and sew it to size? A machine that
//! asks for a larger hoop says the profile's field is too large, or the wrong way round. The "F" in the
//! top-left corner shows which way up the design went.

use stitchcraft_plan::{MachineProfile, StitchPlan};

use super::sketch::{SheetError, Sketch, letter_f};
use super::{BLUE, thread};

/// How tall the "F" is (mm) when the field is at least 30 mm each way. A smaller field gets an F half its
/// shorter side, which keeps it clear of the centre cross.
const LETTER: f64 = 15.0;
/// How far the "F" stands in from the frame's corner (mm) when the field is at least 100 mm each way, and
/// a tenth of the shorter side when it is smaller.
const INSET: f64 = 10.0;

/// Draws the TS-10 frame for `profile`'s hoop.
pub(super) fn build(sheet: &'static str, profile: &MachineProfile) -> Result<StitchPlan, SheetError> {
    let (width, height) = (profile.hoop.width.get(), profile.hoop.height.get());
    let (x, y) = (width / 2.0, height / 2.0);
    let shorter = width.min(height);
    let (letter, inset) = (LETTER.min(shorter / 2.0), INSET.min(shorter / 10.0));
    let mut s = Sketch::new(sheet, thread(BLUE)?);
    s.part("frame")?;
    s.move_to((-x, -y), false)?;
    s.polyline(&[(x, -y), (x, y), (-x, y), (-x, -y)])?;

    s.part("centre-cross")?;
    s.move_to((-5.0, 0.0), true)?;
    s.line_to((5.0, 0.0))?;
    s.move_to((0.0, -5.0), true)?;
    s.line_to((0.0, 5.0))?;

    s.part("letter-f")?;
    letter_f(&mut s, (-x + inset, -y + inset + letter), letter, true)?;
    s.trim();
    Ok(s.finish())
}

#[cfg(test)]
mod tests {
    use stitchcraft_plan::StitchKind;
    use stitchcraft_plan::profiles::{BROTHER_PE800_SMALL, REFERENCE};

    use super::*;

    /// The points of `plan`'s part named `part` that the needle sews.
    fn part(plan: &StitchPlan, part: &str) -> Vec<(f64, f64)> {
        let index = plan.elements.iter().position(|e| e.as_str().ends_with(part)).unwrap();
        let mine = |s: &&stitchcraft_plan::Stitch| s.kind == StitchKind::Normal && s.origin.element.map(|e| e.index()) == Some(index);
        plan.stitches().filter(mine).map(|s| (s.at.x(), s.at.y())).collect()
    }

    #[test]
    fn the_f_stands_in_the_top_left_corner_clear_of_the_cross() {
        // A large field: the F is 15 mm tall, 10 mm in from the corner.
        let large = build("large", REFERENCE).unwrap();
        let f = part(&large, ":letter-f");
        let (left, top) = (f.iter().map(|p| p.0).fold(f64::INFINITY, f64::min), f.iter().map(|p| p.1).fold(f64::INFINITY, f64::min));
        assert_eq!((left, top), (-55.0, -80.0));
        assert_eq!(f.iter().map(|p| p.1).fold(f64::NEG_INFINITY, f64::max), -65.0);
        // The small hoop's 20 mm: a 10 mm F, 2 mm in, and left of the cross's upright and above its arms.
        let small = build("small", &BROTHER_PE800_SMALL).unwrap();
        let f = part(&small, ":letter-f");
        assert!(f.iter().all(|&(x, y)| (-8.0..=-2.0).contains(&x) && (-28.0..=-18.0).contains(&y)), "{f:?}");
        assert!(f.iter().any(|&(x, y)| x == -8.0 && y == -28.0) && f.iter().any(|&(x, _)| x == -2.0));
    }
}
