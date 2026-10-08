//! Test sheets: designs drawn in code for machine checkpoints (`docs/src/plan/machine-testing.md`).
//!
//! Each sheet answers specific questions about a machine — orientation, scale, which trim encoding it
//! obeys, which hoop sizes it accepts — and lists what to look at after sewing it. The command line
//! writes them (`stitch testsheet TS-01 --profile brother-200x200 -o TS-01.pes`); the tests below check
//! that every sheet is a valid plan for every built-in profile, so a sheet can never ask a machine to
//! do something StitchCraft's own rules forbid.
//!
//! Sheets use exact Brother PEC thread colours, so a Brother machine shows the thread names the
//! expected-result sheet lists.

mod sketch;
mod ts01;
mod ts02;
mod ts10;

pub use sketch::{STITCH_LEN, SheetError};
use stitchcraft_plan::palette::BROTHER_PEC;
use stitchcraft_plan::{StitchPlan, Thread};

/// Brother PEC indices of the threads the sheets use.
const RED: u8 = 5;
const BLUE: u8 = 2;
const BLACK: u8 = 20;
const EMERALD_GREEN: u8 = 54;

/// The thread with Brother PEC index `index`, named as the machine names it.
fn thread(index: u8) -> Result<Thread, SheetError> {
    let entry = BROTHER_PEC.entry(index).ok_or(SheetError::UnknownThread(index))?;
    Ok(Thread::named(entry.color, entry.name))
}

/// A test sheet.
#[derive(Clone, Copy, Debug)]
pub struct TestSheet {
    /// Its id, as in the machine-testing protocol (`TS-01`).
    pub id: &'static str,
    /// What it tests, in a few words.
    pub title: &'static str,
    /// What to look at and measure after sewing it.
    pub checks: &'static [&'static str],
    build: fn() -> Result<StitchPlan, SheetError>,
}

impl TestSheet {
    /// The sheet's stitch plan, centred on the machine origin.
    pub fn plan(&self) -> Result<StitchPlan, SheetError> {
        (self.build)()
    }
}

/// Every test sheet, in protocol order.
pub static SHEETS: &[TestSheet] = &[
    TestSheet {
        id: "TS-01",
        title: "Orientation and scale",
        checks: &[
            "The F reads normally: not mirrored, not upside down, not turned.",
            "Each arm of the cross measures 100.0 ± 0.5 mm end to end, horizontally and vertically.",
            "The corner squares measure 10.0 mm on every side.",
            "Ticks are 10 mm apart; the long ticks mark the ends and the centre.",
        ],
        build: ts01::build,
    },
    TestSheet {
        id: "TS-02",
        title: "Colour changes, a stop, jumps and trims",
        checks: &[
            "The machine stops for red → blue and blue → green, and once more in the middle of the green line (the stop).",
            "Left half (red, trim-flagged jumps): for each row (gaps of 2, 5, 15, 40 mm, top to bottom), was the thread between the two dashes cut?",
            "Right half (blue, plain jumps): the same question for each row, and for the jumps between rows.",
            "Any loose loops, knots or bird's nests on the back, and where.",
        ],
        build: ts02::build,
    },
    TestSheet {
        id: "TS-10A",
        title: "Hoop size: 150 × 150 mm frame",
        checks: &["The machine accepts the file and shows the design.", "The frame measures 150.0 × 150.0 mm (± 0.5 mm)."],
        build: || ts10::build("ts10a", 150.0, 150.0),
    },
    TestSheet {
        id: "TS-10B",
        title: "Hoop size: 190 × 150 mm frame",
        checks: &[
            "The machine accepts the file and shows the design (StitchCraft warns that it is larger than the comfort zone — expected).",
            "The frame measures 190.0 × 150.0 mm (± 0.5 mm), wide side left to right, the F at the top left.",
        ],
        build: || ts10::build("ts10b", 190.0, 150.0),
    },
    TestSheet {
        id: "TS-10C",
        title: "Hoop size: 150 × 190 mm frame",
        checks: &[
            "The machine accepts the file and shows the design (StitchCraft warns that it is larger than the comfort zone — expected).",
            "The frame measures 150.0 × 190.0 mm (± 0.5 mm), tall side top to bottom, the F at the top left.",
        ],
        build: || ts10::build("ts10c", 150.0, 190.0),
    },
];

/// The sheet with id `id` (any case).
pub fn find(id: &str) -> Option<&'static TestSheet> {
    SHEETS.iter().find(|sheet| sheet.id.eq_ignore_ascii_case(id))
}

#[cfg(test)]
mod tests {
    use stitchcraft_plan::invariants::check;
    use stitchcraft_plan::profiles::BUILTIN;
    use stitchcraft_plan::{Role, StitchKind};

    use super::*;

    #[test]
    fn every_sheet_is_a_valid_plan_for_every_profile() {
        for sheet in SHEETS {
            let plan = sheet.plan().unwrap();
            for profile in BUILTIN {
                assert_eq!(check(&plan, profile), Vec::new(), "{} on {}", sheet.id, profile.id);
            }
            assert!(!sheet.checks.is_empty() && !sheet.title.is_empty());
        }
    }

    #[test]
    fn sheets_have_the_documented_sizes() {
        let size = |id: &str| {
            let b = find(id).unwrap().plan().unwrap().bounds().unwrap();
            (b.width(), b.height(), b.center().x(), b.center().y())
        };
        assert_eq!(size("TS-01"), (120.0, 120.0, 0.0, 0.0));
        assert_eq!(size("ts-10a"), (150.0, 150.0, 0.0, 0.0));
        assert_eq!(size("TS-10B"), (190.0, 150.0, 0.0, 0.0));
        assert_eq!(size("TS-10C"), (150.0, 190.0, 0.0, 0.0));
        assert_eq!(size("TS-02"), (140.0, 70.0, 0.0, 0.0));
    }

    #[test]
    fn ts02_encodes_trims_on_the_left_only_and_has_a_stop() {
        let plan = find("TS-02").unwrap().plan().unwrap();
        assert_eq!(plan.blocks.len(), 3);
        let trims = |b: usize| plan.blocks[b].stitches.iter().filter(|s| s.kind == StitchKind::Trim).count();
        // Left: a trim before each in-row jump and each move between rows (4 + 3).
        assert_eq!(trims(0), 7);
        assert_eq!(trims(1), 0, "the right half must have no trims");
        assert_ne!(plan.blocks[0].stitches.last().map(|s| s.kind), Some(StitchKind::Trim), "a trim here would ride onto the right half's first jump");
        assert_eq!(plan.stats().stops, 1);
        let jumps = |b: usize| plan.blocks[b].stitches.iter().filter(|s| s.kind == StitchKind::Jump && s.origin.role == Role::Travel).count();
        assert_eq!((jumps(0), jumps(1)), (8, 8));
    }

    #[test]
    fn sheets_are_deterministic() {
        for sheet in SHEETS {
            assert_eq!(sheet.plan().unwrap(), sheet.plan().unwrap(), "{}", sheet.id);
        }
    }
}
