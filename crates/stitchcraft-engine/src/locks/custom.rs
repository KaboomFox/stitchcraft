//! Custom locks, read as Ink/Stitch reads them (REQ-LCK-004).
//!
//! `lock_custom_start` and `lock_custom_end` hold either numbers separated by spaces — the steps the needle
//! takes along the stitching, in sizes of `lock_*_scale_mm` — or an SVG path that draws the lock. Which of
//! the two a text is decides how a file sews, so the rule is Ink/Stitch's: a text made only of digits,
//! spaces, dots, commas and minus signs (a newline may end it) is numbers, and anything else a path. The
//! numbers are the pieces between spaces that read as numbers, so `1,5` is not one.
//!
//! StitchCraft adds two rules of its own, and says when they apply instead of dropping a piece in silence:
//! a step longer than 10 m (the data model's reach) is not sewn, and a step of 0 is no step, because it
//! would sew in place. It does not sew drawn custom locks yet: the engine never reads SVG, so the adapter
//! will hand it the path (roadmap M8).

use stitchcraft_core::{Exhausted, Meter};

use crate::design::WORKING_LIMIT_MM;

/// A custom lock's text, read.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Read {
    /// Numbers: the steps in millimetres, and the pieces that are not steps it can sew.
    Steps {
        /// The steps, scaled, in sewing order.
        steps: Vec<f64>,
        /// The pieces left out, as written.
        skipped: Vec<String>,
    },
    /// Not numbers: a lock drawn as an SVG path.
    Drawn,
}

/// `text` read as a custom lock, its steps `scale_mm` long per unit. Every piece costs a unit of `meter`.
pub(crate) fn read(text: &str, scale_mm: f64, meter: &mut Meter) -> Result<Read, Exhausted> {
    let body = text.strip_suffix('\n').unwrap_or(text);
    if !body.chars().all(|c| c.is_ascii_digit() || matches!(c, ' ' | '.' | ',' | '-')) {
        return Ok(Read::Drawn);
    }
    let (mut steps, mut skipped) = (Vec::new(), Vec::new());
    for piece in body.split(' ').filter(|piece| !piece.is_empty()) {
        meter.charge(1)?;
        match piece.parse::<f64>().map(|units| units * scale_mm) {
            Ok(step) if step.abs() <= WORKING_LIMIT_MM => {
                if step != 0.0 {
                    steps.push(step);
                }
            }
            _ => skipped.push(piece.to_string()),
        }
    }
    Ok(Read::Steps { steps, skipped })
}

#[cfg(test)]
mod tests {
    use stitchcraft_core::Budget;

    use super::*;

    fn steps(text: &str, scale_mm: f64) -> Read {
        read(text, scale_mm, &mut Budget::DEFAULT.meter()).unwrap()
    }

    fn sewn(steps: &[f64], skipped: &[&str]) -> Read {
        Read::Steps { steps: steps.to_vec(), skipped: skipped.iter().map(ToString::to_string).collect() }
    }

    #[test]
    fn numbers_are_steps_in_sizes_of_the_scale() {
        assert_eq!(steps("1 1 -1 -1", 0.5), sewn(&[0.5, 0.5, -0.5, -0.5], &[]));
        assert_eq!(steps("  2  -.5 1.\n", 1.0), sewn(&[2.0, -0.5, 1.0], &[]), "spaces repeat, a newline ends it");
        assert_eq!(steps("1 0 -0 -1", 1.0), sewn(&[1.0, -1.0], &[]), "0 is no step");
        assert_eq!(steps("", 1.0), sewn(&[], &[]));
    }

    #[test]
    fn pieces_that_are_not_steps_are_named() {
        assert_eq!(steps("1,5 1 - . 1-2 --1", 1.0), sewn(&[1.0], &["1,5", "-", ".", "1-2", "--1"]));
        // 10 m is the reach of the data model: a step that long is sewn, a longer one is not.
        assert_eq!(steps("1000 1001", 10.0), sewn(&[10_000.0], &["1001"]));
        assert_eq!(steps(&"9".repeat(400), 1.0), sewn(&[], &["9".repeat(400).as_str()]), "too large to be finite");
    }

    #[test]
    fn anything_else_is_a_drawn_lock() {
        assert_eq!(steps("M 0,0 L 1,1", 1.0), Read::Drawn);
        assert_eq!(steps("1\t-1", 1.0), Read::Drawn, "only spaces separate numbers");
        assert_eq!(steps("1 -1\n\n", 1.0), Read::Drawn, "one newline may end the numbers, not two");
    }

    #[test]
    fn the_budget_bounds_the_work() {
        let mut meter = Budget { max_stitches: 1, max_work: 2 }.meter();
        assert_eq!(read("1 1 1", 1.0, &mut meter), Err(Exhausted::Work));
    }
}
