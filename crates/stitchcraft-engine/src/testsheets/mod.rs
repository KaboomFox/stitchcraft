//! Test sheets: designs drawn in code for machine checkpoints (`docs/src/plan/machine-testing.md`).
//!
//! Each sheet answers specific questions about a machine — orientation, scale, which trim encoding it
//! obeys, which hoop sizes it accepts, which stitch lengths and locks hold — and lists what to look at
//! after sewing it. The command line writes them (`stitch testsheet TS-01 --profile brother-200x200 -o
//! TS-01.pes`); the tests below check that every sheet is a valid plan for every built-in profile, so a
//! sheet can never ask a machine to do something StitchCraft's own rules forbid.
//!
//! The MC-1 sheets are drawn stitch by stitch (`sketch`), to test the machine and the file formats; from
//! MC-2 they are drawn as designs and planned by the engine (`designed`), to test its stitches too.
//! Sheets use exact Brother PEC thread colours, so a Brother machine shows the thread names the
//! expected-result sheet lists.

mod designed;
mod sketch;
mod ts01;
mod ts02;
mod ts02b;
mod ts03;
mod ts04;
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
        id: "TS-02B",
        title: "TS-02 drawn as a design: trims elements ask for",
        checks: &[
            "The machine stops for red → blue and blue → green, and once more in the middle of the green line (the stop).",
            "Left half (red; every dash but the last asks for a trim after it): for each row (gaps of 2, 5, 15, 40 mm, top to bottom), was the thread between the two dashes cut?",
            "Right half (blue; no trims): the 2 mm gap is sewn across. For the other rows, and between rows, was the jump thread cut?",
            "Each dash starts and ends with a small lock: where the thread was cut, pull the tail gently. Does the dash hold?",
            "Any loose loops, knots or bird's nests on the back, and where.",
        ],
        build: ts02b::build,
    },
    TestSheet {
        id: "TS-03",
        title: "Running stitch: lengths, bean stitch, curves, the shortest stitch",
        checks: &[
            "Running stitch lines (top five; 1.5, 2.0, 2.5, 3.0 and 4.0 mm): the stitches of each line are even; ten stitches measure 15, 20, 25, 30 and 40 mm.",
            "Bean stitch lines (next two; each stitch sewn three and five times): solid and raised, with no gaps.",
            "Circles (6 mm across; tolerance 0.1, 0.2 and 0.5 mm, left to right): round, with fewer and straighter stitches to the right.",
            "Short stitches placed by hand (bottom five; 0.3, 0.4, 0.5, 0.7 and 1.0 mm): which lines sew cleanly, with no thread breaks, knots or bunching on the back? The shortest clean one is the machine's shortest stitch.",
        ],
        build: ts03::build,
    },
    TestSheet {
        id: "TS-04",
        title: "Lock stitches: do they hold, and do they show?",
        checks: &[
            "Rows, top to bottom: half stitch, arrow, back and forth, bowtie, cross, star, simple, triangle, zigzag. Each line has its lock at both ends and was trimmed after: pull each tail gently. Does the lock hold, or does the line come undone?",
            "Columns, left to right, are small, medium and large: the half stitch on first stitches of 1.5, 2.5 and 4 mm, back and forth at 0.5, 0.7 and 1.0 mm, the others at 70, 100 and 150 %.",
            "Which locks show from the front, and how much (1 hidden to 5 obvious)?",
            "Any thread breaks or knots at the locks, and where.",
        ],
        build: ts04::build,
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
        assert_eq!(size("TS-02B"), (140.0, 70.0, 0.0, 0.0));
        assert_eq!(size("TS-03"), (60.0, 78.0, 0.0, 0.0));
        assert_eq!(size("TS-04"), (110.0, 64.525, 0.0, 0.0), "the zigzags at 150 % reach 0.525 mm across their line");
    }

    /// Each block of `sheet`'s plan in words: `J` a jump, `T` a trim, `P` a stop, `l` a lock stitch and `s`
    /// any other stitch.
    fn shape(sheet: &str) -> Vec<String> {
        let plan = find(sheet).unwrap().plan().unwrap();
        let letter = |s: &stitchcraft_plan::Stitch| match (s.kind, s.origin.role) {
            (StitchKind::Jump, _) => 'J',
            (StitchKind::Trim, _) => 'T',
            (StitchKind::Stop, _) => 'P',
            (StitchKind::Normal, Role::Lock) => 'l',
            (StitchKind::Normal, _) => 's',
        };
        plan.blocks.iter().map(|block| block.stitches.iter().map(letter).collect()).collect()
    }

    /// The needle points `sheet` sews for its element named `name`.
    fn sewn(sheet: &str, name: &str) -> Vec<stitchcraft_core::Point> {
        let plan = find(sheet).unwrap().plan().unwrap();
        let id = format!("{}:{name}", sheet.to_ascii_lowercase().replace('-', ""));
        let element = plan.elements.iter().position(|e| e.as_str() == id).unwrap();
        let mine = |s: &&stitchcraft_plan::Stitch| {
            s.kind == StitchKind::Normal && s.origin.role != Role::Lock && s.origin.element.map(|e| e.index()) == Some(element)
        };
        plan.stitches().filter(mine).map(|s| s.at).collect()
    }

    #[test]
    fn ts02_and_ts02b_sew_their_rows_top_to_bottom_and_stop_halfway_along_the_green_line() {
        for sheet in ["TS-02", "TS-02B"] {
            let plan = find(sheet).unwrap().plan().unwrap();
            // Each half's rows start 15 mm apart, top to bottom, in the order of their gaps.
            for block in &plan.blocks[..2] {
                let starts: Vec<f64> = block.stitches.windows(2).filter(|w| w[0].kind == StitchKind::Jump).map(|w| w[1].at.y()).collect();
                let rows: Vec<f64> = starts.iter().copied().fold(Vec::new(), |mut rows, y| {
                    if rows.last() != Some(&y) {
                        rows.push(y);
                    }
                    rows
                });
                assert_eq!((rows.len(), rows.first()), (4, Some(&-35.0)), "{sheet}: {rows:?}");
                assert!(rows.windows(2).all(|w| w[1] - w[0] == 15.0), "{sheet}: {rows:?}");
            }
            // The green line runs left to right (its locks aside) and stops in its middle.
            let line = |s: &&stitchcraft_plan::Stitch| s.kind == StitchKind::Normal && s.origin.role != Role::Lock;
            let green: Vec<_> = plan.blocks[2].stitches.iter().filter(line).map(|s| s.at.x()).collect();
            assert!(green.windows(2).all(|w| w[0] <= w[1]), "{sheet}: {green:?}");
            let stop = plan.blocks[2].stitches.windows(2).find(|w| w[1].kind == StitchKind::Stop).map(|w| w[0].at.x());
            assert_eq!(stop, Some(0.0), "{sheet}");
        }
    }

    #[test]
    fn ts02b_trims_where_its_elements_ask_and_sews_short_gaps_across() {
        // Left (red): every dash locked at both ends and trimmed after, but the last.
        let left = format!("Jllll{}sssssllll", "sssssllllTJllll".repeat(7));
        // Right (blue): no trims; the 2 mm gap within the collapse length is sewn across.
        let right = format!("Jllllssssssssssllll{}", "Jllllsssssllll".repeat(6));
        // Green: a stop halfway, with locks round it, and the last trim.
        let green = "JllllsssssssllllPJllllsssssssllllT".to_string();
        assert_eq!(shape("TS-02B"), [left, right, green]);
    }

    #[test]
    fn ts03_sews_what_its_checks_name() {
        // Running lines at their length, exactly: 60 mm is a whole number of each.
        for length in ts03::LENGTHS {
            let points = sewn("TS-03", &format!("running-{length}"));
            assert!(points.windows(2).all(|p| (p[0].distance(p[1]) - length).abs() < 1e-9), "{length}: {points:?}");
        }
        // Top to bottom: the running lines, the bean lines, the circles, the short stitches.
        let names = ts03::LENGTHS.iter().map(|l| format!("running-{l}")).chain(ts03::BEANS.iter().map(|b| format!("bean-{b}")));
        let names = names.chain(["circle-0.1".to_string()]).chain(ts03::SHORT.iter().map(|l| format!("short-{l}")));
        let tops: Vec<f64> = names.map(|name| sewn("TS-03", &name)[0].y()).collect();
        assert!(tops.windows(2).all(|w| w[0] < w[1]), "{tops:?}");
        // The circles, left to right, are round, and the larger the tolerance, the fewer their stitches.
        let mut centres = Vec::new();
        let mut counts = Vec::new();
        for tolerance in ts03::TOLERANCES {
            // Round the circle and back to where it started: the last point is the first.
            let mut points = sewn("TS-03", &format!("circle-{tolerance}"));
            assert_eq!(points.pop(), points.first().copied(), "{tolerance}");
            let n = points.len() as f64;
            let centre = (points.iter().map(|p| p.x()).sum::<f64>() / n, points.iter().map(|p| p.y()).sum::<f64>() / n);
            let off = |p: &stitchcraft_core::Point| ((p.x() - centre.0).powi(2) + (p.y() - centre.1).powi(2)).sqrt() - 3.0;
            assert!(points.iter().all(|p| off(p).abs() < 0.05), "{tolerance}: {points:?}");
            centres.push(centre.0);
            counts.push(points.len());
        }
        assert!(centres.windows(2).all(|w| w[0] < w[1]), "{centres:?}");
        assert!(counts[0] >= counts[1] && counts[1] > counts[2], "{counts:?}");
        // Hand-placed stitches of exactly their length, 20 of them.
        for length in ts03::SHORT {
            let points = sewn("TS-03", &format!("short-{length}"));
            assert_eq!(points.len(), 21, "{length}");
            assert!(points.windows(2).all(|p| (p[0].distance(p[1]) - length).abs() < 1e-9), "{length}: {points:?}");
        }
        assert!(shape("TS-03")[0].matches('T').count() == 15);
    }

    #[test]
    fn ts04_has_every_lock_shape_at_three_sizes_in_the_order_its_checks_name() {
        // The checks name the rows as the lock table orders them.
        let labels: Vec<String> =
            ts04::rows().map(|id| crate::locks::LOCKS.iter().find(|l| l.id == id).unwrap().label.to_ascii_lowercase()).collect();
        let first = find("TS-04").unwrap().checks[0];
        assert!(first.starts_with(&format!("Rows, top to bottom: {}.", labels.join(", "))), "{first}");
        // A line per shape and size, each trimmed after, so each lock alone holds each end.
        assert_eq!((labels.len(), shape("TS-04")[0].matches('T').count()), (9, 27));
        // Rows top to bottom in the table's order, columns left to right, each line sewn left to right.
        let mut previous = f64::NEG_INFINITY;
        for lock in ts04::rows() {
            let lines: Vec<_> = (0..3).map(|column| sewn("TS-04", &format!("{lock}-{column}"))).collect();
            let starts: Vec<f64> = lines.iter().map(|points| points[0].x()).collect();
            assert!(starts.windows(2).all(|w| w[1] - w[0] > 30.0), "{lock}: {starts:?}");
            assert!(lines.iter().all(|points| points.windows(2).all(|w| w[0].x() < w[1].x())), "{lock}");
            assert!(lines[0][0].y() > previous, "{lock}");
            previous = lines[0][0].y();
        }
        // The half stitch's lines have first stitches of 1.5, 2.5 and 4 mm.
        for (column, (first, _)) in ts04::FIRST_STITCHES.iter().enumerate() {
            let points = sewn("TS-04", &format!("half_stitch-{column}"));
            assert!((points[0].distance(points[1]) - first).abs() < 1e-9, "{column}");
        }
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
