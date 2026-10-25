//! Test sheets: designs drawn in code for machine checkpoints (`docs/src/plan/machine-testing.md`).
//!
//! Each sheet answers specific questions about a machine — orientation, scale, which trim encoding it
//! obeys, which hoop sizes it accepts, which stitch lengths and locks hold — and lists what to look at
//! after sewing it. Each is for one hoop of the reference machine, its profile: the 5 × 7 in for most, and
//! the others for the TS-10 sheets that test them. The command line writes them (`stitch testsheet TS-01
//! -o TS-01.pes`, checked against the sheet's profile unless `--profile` names another). The tests below
//! check that every sheet is a valid plan for its profile and fits its hoop, so a sheet can never ask a
//! machine to do something StitchCraft's own rules forbid.
//!
//! The MC-1 sheets are drawn stitch by stitch (`sketch`), to test the machine and the file formats; from
//! MC-2 they are drawn as designs and planned by the engine (`designed`), to test its stitches too. The MC-3
//! sheets sew satin columns: a ladder of widths and spacings, and the underlays side by side. The MC-4 sheets
//! sew tatami fills: inside outlines at 3 pull compensations, at 5 spacings and 3 angles, and round holes
//! and a part apart, with and without underlay and gap-filling rows.
//! Sheets use exact Brother PEC thread colours, so a Brother machine shows the thread names the
//! expected-result sheet lists.

mod designed;
mod sketch;
mod ts01;
mod ts02;
mod ts02b;
mod ts03;
mod ts04;
mod ts05;
mod ts06;
mod ts07;
mod ts08;
mod ts09;
mod ts10;

use designed::Drawing;
pub use sketch::{STITCH_LEN, SheetError};
use stitchcraft_core::Code;
use stitchcraft_plan::palette::BROTHER_PEC;
use stitchcraft_plan::profiles::{BROTHER_PE800_4X4, BROTHER_PE800_SMALL, REFERENCE};
use stitchcraft_plan::{MachineProfile, StitchPlan, Thread};

use crate::design::Design;

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
    /// The profile of the hoop to sew it in.
    pub profile: &'static MachineProfile,
    drawn: Drawn,
}

/// How a sheet is drawn.
#[derive(Clone, Copy, Debug)]
enum Drawn {
    /// Stitch by stitch, as a plan (`sketch`).
    Sketched(fn() -> Result<StitchPlan, SheetError>),
    /// As a design the engine plans (`designed`), which says exactly the diagnostics of these codes.
    Designed(fn() -> Result<Drawing, SheetError>, &'static [Code]),
}

impl TestSheet {
    /// The sheet's stitch plan, centred on the machine origin.
    pub fn plan(&self) -> Result<StitchPlan, SheetError> {
        match self.drawn {
            Drawn::Sketched(build) => build(),
            Drawn::Designed(drawing, said) => drawing()?.plan_saying(said),
        }
    }

    /// Whether the sheet is drawn as a design, which [`TestSheet::design`] gives, rather than stitch by stitch.
    pub const fn designed(&self) -> bool {
        matches!(self.drawn, Drawn::Designed(..))
    }

    /// The sheet as the design the engine plans, with the design settings' defaults, as a host draws it into
    /// a document (`REQ-VC-015`); `None` for a sheet drawn stitch by stitch, which is no design.
    pub fn design(&self) -> Option<Result<Design, SheetError>> {
        match self.drawn {
            Drawn::Sketched(_) => None,
            Drawn::Designed(drawing, _) => Some(drawing().and_then(Drawing::design)),
        }
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
        profile: REFERENCE,
        drawn: Drawn::Sketched(ts01::build),
    },
    TestSheet {
        id: "TS-02",
        title: "Colour changes, a stop, jumps and trims",
        checks: &[
            "The machine stops for red → blue and blue → green, and once more in the middle of the green line (the stop).",
            "Left half (red, a trim before every jump): for each row (gaps of 2, 5, 15, 30 mm, top to bottom), was the thread between the two dashes cut?",
            "Right half (blue, jumps without trims, which a PES file flags as trims too): did every dash sew? For each row, and between rows, was the thread cut?",
            "Any loose loops, knots or bird's nests on the back, and where.",
        ],
        profile: REFERENCE,
        drawn: Drawn::Sketched(ts02::build),
    },
    TestSheet {
        id: "TS-02B",
        title: "TS-02 drawn as a design: trims elements ask for",
        checks: &[
            "The machine stops for red → blue and blue → green, and once more in the middle of the green line (the stop), and cuts the thread at each.",
            "Left half (red; every dash but the last asks for a trim after it): the machine cuts no jump, so its rows (gaps of 2, 5, 15, 30 mm, top to bottom) are joined by threads. Did every dash stay sewn while the machine went on to the next?",
            "Right half (blue; no trims): the 2 mm gap is sewn across, and threads join the other dashes and the rows. Did every dash stay sewn?",
            "Each dash starts and ends with a small lock. Snip the threads between the dashes, then pull each tail gently: does the dash hold?",
            "Any loose loops, knots or bird's nests on the back, and where.",
        ],
        profile: REFERENCE,
        drawn: Drawn::Designed(ts02b::drawing, &[]),
    },
    TestSheet {
        id: "TS-03",
        title: "Running stitch: lengths, bean stitch, curves, the shortest stitch",
        checks: &[
            "Running stitch lines (top five; 1.5, 2.0, 2.5, 3.0 and 4.0 mm): the stitches of each line are even; ten stitches, counted inside the small lock at each end, measure 15, 20, 25, 30 and 40 mm.",
            "Bean stitch lines (next two; each stitch sewn three and five times): solid and raised, with no gaps.",
            "Circles (6 mm across; tolerance 0.1, 0.2 and 0.5 mm, left to right): round, with fewer and straighter stitches to the right.",
            "Short stitches placed by hand (bottom five; 0.3, 0.4, 0.5, 0.7 and 1.0 mm, each with a small back-and-forth lock at both ends): which lines sew cleanly, with no thread breaks, knots or bunching on the back? The shortest clean one is the machine's shortest stitch; if the 0.3 mm line is clean, it is 0.3 mm or less: the sheet has no shorter line.",
        ],
        profile: REFERENCE,
        drawn: Drawn::Designed(ts03::drawing, &[]),
    },
    TestSheet {
        id: "TS-04",
        title: "Lock stitches: do they hold, and do they show?",
        checks: &[
            "Rows, top to bottom: half stitch, arrow, back and forth, bowtie, cross, star, simple, triangle, zigzag. Each line has its lock at both ends and was trimmed after, but the machine cuts no jump: snip the threads between the lines, then pull each tail gently. Does the lock hold, or does the line come undone?",
            "Columns, left to right, are small, medium and large: the half stitch on first stitches of 1, 1.5 and 2.5 mm, back and forth at 0.5, 0.7 and 1.0 mm, the others at 70, 100 and 150 %.",
            "Rows 1, 3, 5, 7 and 9 are sewn left to right and the others right to left: a line's start is the end it was sewn from.",
            "Which locks show from the front, and how much (1 hidden to 5 obvious)?",
            "Any thread breaks or knots at the locks, and where.",
        ],
        profile: REFERENCE,
        drawn: Drawn::Designed(ts04::drawing, &[]),
    },
    TestSheet {
        id: "TS-05",
        title: "Satin width ladder: spacing, width and pull",
        checks: &[
            "Rows, top to bottom, are at zigzag spacings of 0.3, 0.4 and 0.5 mm, and the columns of each row are 1 to 10 mm wide, left to right.",
            "Coverage: in which columns does the fabric show between the stitches? Rate each row 1 (bare) to 5 (solid).",
            "Width: measure the 2, 6 and 10 mm columns of each row across, halfway down. How much narrower than drawn are they?",
            "Edges: straight and crisp, or wavy? From which width on do the long stitches lie loose or catch?",
            "Any puckering round the columns, thread breaks, or loops on the back, and where.",
        ],
        profile: REFERENCE,
        drawn: Drawn::Designed(ts05::drawing, &[]),
    },
    TestSheet {
        id: "TS-06",
        title: "Satin underlays side by side",
        checks: &[
            "Columns, left to right: no underlay, a centre walk, a contour, a zigzag, and a contour with a zigzag, each 6 mm wide.",
            "Edges: rate each column 1 (ragged) to 5 (crisp).",
            "Loft: rate each column 1 (flat) to 5 (full and raised).",
            "Does any underlay show past the top stitches, at the edges or at the ends?",
            "Measure each column across, halfway down, and note any puckering round it.",
        ],
        profile: REFERENCE,
        drawn: Drawn::Designed(ts06::drawing, &[]),
    },
    TestSheet {
        id: "TS-07",
        title: "Fill and outline registration at 3 pull compensations",
        checks: &[
            "Circles: top left with no pull compensation, top right with 0.2 mm, bottom with 0.4 mm. Each red fill is 60 mm across, with a black running stitch along its edge.",
            "At 12 points round each circle, as the hours of a clock: does fabric show between the fill and the outline, or does the fill reach past it? Measure the largest gap and the largest overlap of each circle.",
            "Measure each fill across, left to right and top to bottom. How much less than 60 mm is it, and how much of that does the compensation make up?",
            "Any puckering round the circles, and where.",
        ],
        profile: REFERENCE,
        drawn: Drawn::Designed(ts07::drawing, &[]),
    },
    TestSheet {
        id: "TS-08",
        title: "Fill density and angle",
        checks: &[
            "Rows, top to bottom, are at row spacings of 0.25, 0.3, 0.4, 0.5 and 0.6 mm, and the columns at angles of 0°, 45° and 90°, left to right. Each square is 30 mm wide.",
            "Coverage: in which squares does the fabric show between the rows? Rate each row of squares 1 (bare) to 5 (solid).",
            "At 0.25 mm the rows lie nearer each other than the machine's shortest stitch of 0.3 mm. In the 0° and 90° squares, whose edges meet the rows square on, each row's first needle point is left out: does a sliver of fabric show at the rows' starts, along those edges? The 45° square's rows end 0.35 mm apart along its edges and keep their starts.",
            "Stiffness: rate each row of squares 1 (soft) to 5 (stiff as card). Is there puckering round any square, or furrows where needle points line up?",
            "Measure the 0° and the 90° square at 0.4 mm across, both ways. How much less than 30 mm are they?",
        ],
        profile: REFERENCE,
        drawn: Drawn::Designed(ts08::drawing, &[]),
    },
    TestSheet {
        id: "TS-09",
        title: "Fill underlay, travel and gap-filling rows round holes",
        checks: &[
            "Top to bottom: without underlay, with Ink/Stitch's default underlay, and with the underlay and 2 gap-filling rows. Each is a 50 × 30 mm rectangle with a square hole and a round hole, and a bar 6 mm to its right that belongs to the same fill.",
            "Travel: does any running stitch between rows show on top of the fill? Where?",
            "The bar: was the thread to it cut, or does a jump thread run to it? Does the fill hold at both ends of that jump?",
            "Holes: are their edges clean, with no row across them and no fabric showing round them?",
            "Where the rows round a hole meet again, does a gap open along a row? Does the bottom copy, with gap-filling rows, close it?",
            "How the 3 copies differ in stiffness, and any puckering.",
        ],
        profile: REFERENCE,
        drawn: Drawn::Designed(ts09::drawing, &[Code::FillPartsApart; 3]),
    },
    TestSheet {
        id: "TS-10A",
        title: "Hoop size: the 4 × 4 in hoop, a 100 × 100 mm frame",
        checks: &[
            "With the 4 × 4 in hoop on the machine: does the machine take the file and sew it, without asking for a larger hoop?",
            "The frame measures 100.0 × 100.0 mm (± 0.5 mm), the F at the top left.",
        ],
        profile: &BROTHER_PE800_4X4,
        drawn: Drawn::Sketched(|| ts10::build("ts10a", &BROTHER_PE800_4X4)),
    },
    TestSheet {
        id: "TS-10B",
        title: "Hoop size: the 5 × 7 in hoop, a 130 × 180 mm frame",
        checks: &[
            "With the 5 × 7 in hoop on the machine: does the machine take the file and sew it?",
            "The frame measures 130.0 × 180.0 mm (± 0.5 mm), tall side top to bottom, the F at the top left.",
        ],
        profile: REFERENCE,
        drawn: Drawn::Sketched(|| ts10::build("ts10b", REFERENCE)),
    },
    TestSheet {
        id: "TS-10C",
        title: "Hoop size: the small hoop, a 20 × 60 mm frame",
        checks: &[
            "With the small hoop on the machine: does the machine take the file and sew it, without asking for a larger hoop?",
            "If it asks for a larger hoop, turn the design a quarter turn on the machine's screen. Does it take it then?",
            "The frame measures 20.0 × 60.0 mm (± 0.5 mm), tall side top to bottom, the F at the top left.",
        ],
        profile: &BROTHER_PE800_SMALL,
        drawn: Drawn::Sketched(|| ts10::build("ts10c", &BROTHER_PE800_SMALL)),
    },
];

/// The sheet with id `id` (any case).
pub fn find(id: &str) -> Option<&'static TestSheet> {
    SHEETS.iter().find(|sheet| sheet.id.eq_ignore_ascii_case(id))
}

#[cfg(test)]
mod tests {
    use stitchcraft_plan::invariants::check;
    use stitchcraft_plan::{Role, StitchKind};

    use super::*;

    #[test]
    fn req_vc_015_a_sheet_drawn_as_a_design_has_the_design_it_plans() {
        let mut designed = Vec::new();
        for sheet in SHEETS {
            let Some(design) = sheet.design() else {
                assert!(!sheet.designed(), "{}", sheet.id);
                continue;
            };
            let planned = crate::plan(&design.unwrap(), sheet.profile, &stitchcraft_core::Budget::DEFAULT).plan;
            assert_eq!(planned, Some(sheet.plan().unwrap()), "{}", sheet.id);
            assert!(sheet.designed(), "{}", sheet.id);
            designed.push(sheet.id);
        }
        // The sheets drawn stitch by stitch have no design.
        assert_eq!(designed, ["TS-02B", "TS-03", "TS-04", "TS-05", "TS-06", "TS-07", "TS-08", "TS-09"]);
    }

    #[test]
    fn every_sheet_is_a_valid_plan_for_its_profile_and_fits_its_hoop() {
        for sheet in SHEETS {
            let plan = sheet.plan().unwrap();
            assert_eq!(check(&plan, sheet.profile), Vec::new(), "{} on {}", sheet.id, sheet.profile.id);
            assert_eq!(plan.bounds().and_then(|b| sheet.profile.check_fit(b)), None, "{} on {}", sheet.id, sheet.profile.id);
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
        assert_eq!(size("TS-02"), (120.0, 70.0, 0.0, 0.0));
        assert_eq!(size("TS-02B"), (120.0, 70.0, 0.0, 0.0));
        assert_eq!(size("TS-03"), (60.0, 78.0, 0.0, 0.0));
        assert_eq!(size("TS-04"), (110.0, 64.525, 0.0, 0.0), "the zigzags at 150 % reach 0.525 mm across their line");
        // A satin's needle points lie on its rails to within rounding.
        let about = |id: &str| {
            let (w, h, x, y) = size(id);
            [w, h, x, y].map(|v| (v * 1e9).round() / 1e9)
        };
        assert_eq!(about("TS-05"), [91.0, 76.0, 0.0, 0.0]);
        assert_eq!(about("TS-06"), [62.0, 30.0, 0.0, 0.0]);
        // The top-right circle's rows reach 0.2 mm past its edge either side.
        assert_eq!(about("TS-07"), [126.2, 126.0, 0.0, 0.0]);
        assert_eq!(about("TS-08"), [100.0, 170.0, 0.0, 0.0]);
        assert_eq!(about("TS-09"), [66.0, 114.0, 0.0, 0.0]);
        // Each TS-10 frame is its hoop's whole field, as its title and checks say, and fits no smaller hoop.
        let hoops = [("ts-10a", "4 × 4 in", &BROTHER_PE800_4X4), ("TS-10B", "5 × 7 in", REFERENCE), ("TS-10C", "small", &BROTHER_PE800_SMALL)];
        for (id, hoop, profile) in hoops {
            let (width, height) = (profile.hoop.width.get(), profile.hoop.height.get());
            assert_eq!(size(id), (width, height, 0.0, 0.0), "{id}");
            let sheet = find(id).unwrap();
            assert_eq!(sheet.profile.id, profile.id, "{id}");
            assert!(sheet.title.contains(&format!("the {hoop} hoop, a {width} × {height} mm frame")), "{id}: {}", sheet.title);
            assert!(sheet.checks.iter().any(|c| c.contains(&format!("{width:.1} × {height:.1} mm"))), "{id}");
        }
        assert!(BROTHER_PE800_4X4.check_fit(find("TS-10B").unwrap().plan().unwrap().bounds().unwrap()).is_some());
        assert!(BROTHER_PE800_SMALL.check_fit(find("TS-10A").unwrap().plan().unwrap().bounds().unwrap()).is_some());
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
    fn ts02_and_ts02b_checks_name_the_gaps_they_sew() {
        let gaps = ts02::JUMPS.iter().map(ToString::to_string).collect::<Vec<_>>().join(", ");
        for sheet in ["TS-02", "TS-02B"] {
            let checks = find(sheet).unwrap().checks;
            assert!(checks.iter().any(|c| c.contains(&format!("(gaps of {gaps} mm, top to bottom)"))), "{sheet}: {checks:?}");
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
            let off = |p: &stitchcraft_core::Point| {
                let (dx, dy) = (p.x() - centre.0, p.y() - centre.1);
                (dx * dx + dy * dy).sqrt() - 3.0
            };
            assert!(points.iter().all(|p| off(p).abs() < 0.05), "{tolerance}: {points:?}");
            centres.push(centre.0);
            counts.push(points.len());
        }
        assert!(centres.windows(2).all(|w| w[0] < w[1]), "{centres:?}");
        assert!(counts[0] >= counts[1] && counts[1] > counts[2], "{counts:?}");
        // Hand-placed stitches of exactly their length, 20 of them. Each line starts straight below where the
        // stitching before it ended, the first below the last circle's end, so the jump to it runs square to it.
        let mut before = sewn("TS-03", &format!("circle-{}", ts03::TOLERANCES[0]));
        for length in ts03::SHORT {
            let points = sewn("TS-03", &format!("short-{length}"));
            assert_eq!(points.len(), 21, "{length}");
            assert!(points.windows(2).all(|p| (p[0].distance(p[1]) - length).abs() < 1e-9), "{length}: {points:?}");
            assert!((points[0].x() - before[before.len() - 1].x()).abs() < 1e-9, "{length}: {:?} after {:?}", points[0], before.last());
            before = points;
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
        // Rows top to bottom in the table's order, columns left to right, the rows sewn left to right and right
        // to left in turn.
        let mut previous = f64::NEG_INFINITY;
        for (row, lock) in ts04::rows().enumerate() {
            let lines: Vec<_> = (0..3).map(|column| sewn("TS-04", &format!("{lock}-{column}"))).collect();
            let lefts: Vec<f64> = lines.iter().map(|points| across(points).0).collect();
            assert!(lefts.windows(2).all(|w| w[1] - w[0] > 30.0), "{lock}: {lefts:?}");
            let rightwards = ts04::rightwards(row);
            assert!(lines.iter().all(|points| points.windows(2).all(|w| (w[0].x() < w[1].x()) == rightwards)), "{lock}");
            assert!(lines[0][0].y() > previous, "{lock}");
            previous = lines[0][0].y();
        }
        // The half stitch's lines have the first stitches the second check names.
        for (column, first) in ts04::FIRST_STITCHES.iter().enumerate() {
            let points = sewn("TS-04", &format!("half_stitch-{column}"));
            assert!((points[0].distance(points[1]) - first).abs() < 1e-9, "{column}");
        }
        let firsts = listed(&ts04::FIRST_STITCHES.map(|first| first.to_string()));
        let second = find("TS-04").unwrap().checks[1];
        assert!(second.contains(&format!("the half stitch on first stitches of {firsts} mm")), "{second}");
        // The checks say which rows run left to right, so that the reader knows each line's start.
        let rightwards: Vec<String> = (0..labels.len()).filter(|&row| ts04::rightwards(row)).map(|row| (row + 1).to_string()).collect();
        let third = find("TS-04").unwrap().checks[2];
        assert!(third.starts_with(&format!("Rows {} are sewn left to right", listed(&rightwards))), "{third}");
    }

    /// `items`, 2 or more, as a list in words: `1, 2 and 3`.
    fn listed(items: &[String]) -> String {
        let (last, rest) = items.split_last().unwrap();
        format!("{} and {last}", rest.join(", "))
    }

    /// The left and right edges of `points`.
    fn across(points: &[stitchcraft_core::Point]) -> (f64, f64) {
        points.iter().fold((f64::INFINITY, f64::NEG_INFINITY), |(left, right), p| (left.min(p.x()), right.max(p.x())))
    }

    /// The top and bottom edges of `points`.
    fn down(points: &[stitchcraft_core::Point]) -> (f64, f64) {
        points.iter().fold((f64::INFINITY, f64::NEG_INFINITY), |(top, bottom), p| (top.min(p.y()), bottom.max(p.y())))
    }

    #[test]
    fn ts05_sews_its_rows_and_columns_as_its_checks_name() {
        let mut rows = Vec::new();
        for (row, spacing) in ts05::SPACINGS.into_iter().enumerate() {
            let mut lefts = Vec::new();
            for (column, width) in ts05::WIDTHS.into_iter().enumerate() {
                let points = sewn("TS-05", &format!("{spacing}-{width}"));
                let (left, right) = across(&points);
                assert!((right - left - width).abs() < 1e-9, "{spacing}-{width}: as wide as drawn");
                // Every other needle point is on the left rail, the spacing on from the one before, but for the
                // last, which is at the column's end.
                let rail: Vec<f64> = points.iter().filter(|p| (p.x() - left).abs() < 1e-9).map(|p| p.y()).collect();
                let gaps: Vec<f64> = rail.windows(2).map(|w| (w[1] - w[0]).abs()).collect();
                assert!(gaps[..gaps.len() - 1].iter().all(|gap| (gap - spacing).abs() < 1e-6), "{spacing}-{width}: {gaps:?}");
                // The rows run left to right and right to left in turn, and the columns, in the order they are
                // sewn, down and up in turn.
                let sewn_at = row * ts05::WIDTHS.len() + if row.is_multiple_of(2) { column } else { ts05::WIDTHS.len() - 1 - column };
                let downwards = points.first().map(|p| p.y()) < points.last().map(|p| p.y());
                assert_eq!(downwards, sewn_at.is_multiple_of(2), "{spacing}-{width}");
                lefts.push(left);
            }
            assert!(lefts.windows(2).all(|w| w[0] < w[1]), "{spacing}: left to right by width");
            rows.push(down(&sewn("TS-05", &format!("{spacing}-1"))).0);
        }
        assert!(rows.windows(2).all(|w| w[0] < w[1]), "top to bottom by spacing: {rows:?}");
    }

    #[test]
    fn ts06_sews_its_underlays_before_the_same_top_stitches_left_to_right() {
        // How each column is sewn, by its place from the left: down or up in turn, in the order they are sewn,
        // each from the side the needle comes from.
        let mut ways = [(true, true); ts06::UNDERLAYS.len()];
        let mut before = None;
        for (n, &place) in ts06::SEWN.iter().enumerate() {
            ways[place] = (n.is_multiple_of(2), before.is_none_or(|before| before < place));
            before = Some(place);
        }
        // A column's needle points from the corner it starts at, as if sewn down from the left: sewn up or from
        // the right, a column is the mirror image of one sewn down from the left.
        let from_its_start = |name: &str, place: usize| {
            let points = sewn("TS-06", name);
            let ((left, right), (top, bottom), (sewn_down, from_left)) = (across(&points), down(&points), ways[place]);
            let x = |p: &stitchcraft_core::Point| if from_left { p.x() - left } else { right - p.x() };
            let y = |p: &stitchcraft_core::Point| if sewn_down { p.y() - top } else { bottom - p.y() };
            points.iter().map(|p| (x(p), y(p))).collect::<Vec<_>>()
        };
        let plain = from_its_start("none", 0);
        let mut previous = f64::NEG_INFINITY;
        for (place, (name, _)) in ts06::UNDERLAYS.into_iter().enumerate() {
            let (left, right) = across(&sewn("TS-06", name));
            assert!((right - left - ts06::WIDTH).abs() < 1e-9 && left > previous, "{name}: as wide as drawn, right of the last");
            previous = left;
            // The top stitches come last, as the column without underlay sews them.
            let points = from_its_start(name, place);
            let top = &points[points.len() - plain.len()..];
            let same = top.iter().zip(&plain).all(|(a, b)| (a.0 - b.0).abs() < 1e-9 && (a.1 - b.1).abs() < 1e-9);
            assert!(same && (name == "none") == (points.len() == plain.len()), "{name}");
        }
    }

    #[test]
    fn ts07_sews_its_fills_then_their_outlines_with_the_compensations_its_checks_name() {
        let plan = find("TS-07").unwrap().plan().unwrap();
        assert_eq!(plan.blocks.iter().map(|b| b.thread.name.as_deref()).collect::<Vec<_>>(), [Some("Red"), Some("Black")]);
        let mut shift: Option<(f64, f64)> = None;
        let mut centres = Vec::new();
        for ((pull, (x, y)), start) in ts07::CIRCLES.into_iter().zip(ts07::STARTS) {
            // Each outline starts where the drawing starts it, and its needle points lie on the circle, within
            // the curves' few thousandths of a millimetre.
            let outline = sewn("TS-07", &format!("outline-{pull}"));
            let (dx, dy) = match start {
                designed::Start::Right => (-ts07::RADIUS, 0.0),
                designed::Start::Left => (ts07::RADIUS, 0.0),
                designed::Start::Top => (0.0, ts07::RADIUS),
            };
            let centre = stitchcraft_core::Point::new(outline[0].x() + dx, outline[0].y() + dy).unwrap();
            assert!(outline.iter().all(|q| (q.distance(centre) - ts07::RADIUS).abs() < 0.05), "{pull}");
            // Each circle where the drawing puts it, all moved alike to centre the sheet.
            let moved = *shift.get_or_insert((centre.x() - x, centre.y() - y));
            assert!((centre.x() - x - moved.0).abs() < 1e-9 && (centre.y() - y - moved.1).abs() < 1e-9, "{pull}");
            // The rows through the circle's middle reach its compensation past its edge, and none reaches farther.
            // Row starts the machine's shortest stitch leaves out (DEV-FIN-001) can only fall short.
            let reach = sewn("TS-07", &format!("fill-{pull}")).iter().map(|q| q.distance(centre)).fold(0.0, f64::max);
            let compensation: f64 = pull.parse().unwrap();
            assert!((reach - ts07::RADIUS - compensation).abs() < 0.01, "{pull}: {reach}");
            centres.push(centre);
        }
        // Top left, top right, and below between them, as the first check says, and no 2 circles within a
        // millimetre of each other, their compensations and all.
        let [left, right, below] = centres[..] else { panic!("3 circles") };
        assert!(left.y() == right.y() && left.x() < right.x() && below.y() > left.y(), "{centres:?}");
        assert!((below.x() - (left.x() + right.x()) / 2.0).abs() < 1e-9, "{centres:?}");
        let apart = [(left, right), (left, below), (right, below)].map(|(a, b)| a.distance(b));
        assert!(apart.iter().all(|d| *d > 2.0 * (ts07::RADIUS + 0.4) + 1.0), "{apart:?}");
        // The first check names the compensations in the circles' order.
        let first = find("TS-07").unwrap().checks[0];
        let named = |pull: &str| if pull == "0" { "no pull compensation".to_string() } else { format!("{pull} mm") };
        let at: Vec<usize> = ts07::CIRCLES.iter().map(|(pull, _)| first.find(&named(pull)).unwrap()).collect();
        assert!(at.windows(2).all(|w| w[0] < w[1]), "{first}");
    }

    #[test]
    fn ts08_sews_its_squares_by_spacing_and_angle_as_its_checks_name() {
        let mut tops = Vec::new();
        for spacing in ts08::SPACINGS {
            let mut lefts = Vec::new();
            let mut counts = Vec::new();
            for angle in ts08::ANGLES {
                let points = sewn("TS-08", &format!("{spacing}-{angle}"));
                let (left, right) = across(&points);
                assert!(right - left <= ts08::SIDE + 1e-9, "{spacing}-{angle}: within its square");
                lefts.push(left);
                counts.push(points.len());
            }
            assert!(lefts.windows(2).all(|w| w[1] - w[0] > ts08::SIDE), "{spacing}: left to right by angle");
            tops.push((sewn("TS-08", &format!("{spacing}-0"))[0].y(), counts));
        }
        assert!(tops.windows(2).all(|w| w[0].0 < w[1].0), "top to bottom by spacing");
        // Wider spacings sew fewer stitches in each column.
        for column in 0..ts08::ANGLES.len() {
            assert!(tops.windows(2).all(|w| w[0].1[column] > w[1].1[column]), "{column}: {tops:?}");
        }
        // The first check names the spacings and the angles in the order they are sewn.
        let spacings = listed(&ts08::SPACINGS.map(String::from));
        let angles = listed(&ts08::ANGLES.map(|a| format!("{a}°")));
        let first = find("TS-08").unwrap().checks[0];
        assert!(first.contains(&format!("spacings of {spacings} mm")) && first.contains(&format!("angles of {angles}")), "{first}");
    }

    #[test]
    fn ts09_sews_each_copy_round_its_holes_and_says_it_is_in_parts() {
        // Each copy in its 2 parts, with a jump to the bar, locked wherever its stitching starts or ends, and
        // trimmed after. The sheet draws only as it says SC-W0307 for each.
        let squeeze =
            |block: &String| block.chars().fold(String::new(), |out, c| if c == 's' && out.ends_with('s') { out } else { out + &c.to_string() });
        assert_eq!(shape("TS-09").iter().map(squeeze).collect::<Vec<_>>(), ["JllllsllllJllllsllllT".repeat(3)]);
        // Copies top to bottom from the drawing's top-left corner, which centring the sheet moves.
        let bounds = find("TS-09").unwrap().plan().unwrap().bounds().unwrap();
        let corner = (bounds.center().x() - bounds.width() / 2.0, bounds.center().y() - bounds.height() / 2.0);
        let ([(left, top), (right, bottom)], ((x, y), radius)) = (ts09::SQUARE, ts09::ROUND);
        let mut counts = Vec::new();
        for (k, (name, _)) in ts09::COPIES.into_iter().enumerate() {
            let at = |dx: f64, dy: f64| stitchcraft_core::Point::new(corner.0 + dx, corner.1 + k as f64 * (ts09::TALL + ts09::GAP) + dy).unwrap();
            let (from, to) = (at(0.0, 0.0), at(ts09::BAR.1, ts09::TALL));
            // Every needle point in its copy, and none in a hole or between the rectangle and the bar, by more
            // than a hundredth of a millimetre.
            let points = sewn("TS-09", name);
            let between = |q: &stitchcraft_core::Point, a: stitchcraft_core::Point, b: stitchcraft_core::Point, by: f64| {
                (a.x() + by..=b.x() - by).contains(&q.x()) && (a.y() + by..=b.y() - by).contains(&q.y())
            };
            assert!(points.iter().all(|q| between(q, from, to, -1e-9)), "{name}: in its copy");
            let square = |q| between(q, at(left, top), at(right, bottom), 0.01);
            let circle = |q: &stitchcraft_core::Point| q.distance(at(x, y)) < radius - 0.01;
            let gap = |q| between(q, at(ts09::WIDE, 0.0), at(ts09::BAR.0, ts09::TALL), 0.01);
            assert!(!points.iter().any(|q| square(q) || circle(q) || gap(q)), "{name}: in a hole or the gap");
            counts.push(points.len());
        }
        // Underlay adds stitches, and gap-filling rows add more.
        assert!(counts[0] < counts[1] && counts[1] < counts[2], "{counts:?}");
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

    /// The sheets sewn at MC-1 as they are: drawn stitch by stitch, to test the machine and the file format.
    const SEWN_AT_MC1: [&str; 5] = ["TS-01", "TS-02", "TS-10A", "TS-10B", "TS-10C"];

    /// How near a needle may come down to a thread on the fabric without catching it (mm): about half a 75/11
    /// needle, 0.75 mm across, and half a 40 wt thread.
    const CATCHES: f64 = 0.5;

    /// How far `p` lies from the segment from `a` to `b`, which are apart, and how far before `b` the point of
    /// the segment nearest it is.
    fn off(p: stitchcraft_core::Point, a: stitchcraft_core::Point, b: stitchcraft_core::Point) -> (f64, f64) {
        let (ax, ay, px, py) = (b.x() - a.x(), b.y() - a.y(), p.x() - a.x(), p.y() - a.y());
        let t = ((px * ax + py * ay) / (ax * ax + ay * ay)).clamp(0.0, 1.0);
        let (dx, dy) = (px - t * ax, py - t * ay);
        ((dx * dx + dy * dy).sqrt(), (1.0 - t) * a.distance(b))
    }

    /// How far before `b` the stitch from `p` to `q` crosses the segment from `a` to `b`, if it crosses it.
    fn crossing(p: stitchcraft_core::Point, q: stitchcraft_core::Point, a: stitchcraft_core::Point, b: stitchcraft_core::Point) -> Option<f64> {
        let (rx, ry, sx, sy, wx, wy) = (q.x() - p.x(), q.y() - p.y(), b.x() - a.x(), b.y() - a.y(), a.x() - p.x(), a.y() - p.y());
        let across = rx * sy - ry * sx;
        // How far along the stitch and along the segment they meet, as fractions of each. Parallel ones meet
        // nowhere: the fractions are infinite or not numbers, and fail the test.
        let (t, u) = ((wx * sy - wy * sx) / across, (wx * ry - wy * rx) / across);
        (0.0 < t && t < 1.0 && 0.0 < u && u < 1.0).then_some((1.0 - u) * a.distance(b))
    }

    #[test]
    fn no_sheet_sews_over_a_jump_thread_the_machine_leaves() {
        // The reference machine cuts the thread at colour changes and stops, and at no jump, a trim's or not
        // (MC-1, formats.md). A jump leaves its thread on the fabric, straight from where the needle came up to
        // where it goes down again, until the operator snips it at the next stop or after sewing. A needle that
        // comes down within reach of it sews it down, and so does a stitch that lies across it: then more than
        // the lock holds the tail a pull test pulls. Round the hole where it goes down, the thread is held
        // already, and its tie-in, which ties in its own thread, may lie along it.
        let mut sewn_over = Vec::new();
        for sheet in SHEETS.iter().filter(|sheet| !SEWN_AT_MC1.contains(&sheet.id)) {
            for block in &sheet.plan().unwrap().blocks {
                // The threads on the fabric, each from where the needle came up to where it went down again.
                let mut threads = Vec::new();
                // Where the needle went down last since the thread was cut, and the jump under way from there.
                let (mut needle, mut jump) = (None, None);
                let mut tying_in = false;
                for s in &block.stitches {
                    match s.kind {
                        StitchKind::Stop => (threads, needle, jump) = (Vec::new(), None, None),
                        StitchKind::Trim => {}
                        StitchKind::Jump => jump = jump.map(|(from, _)| from).or(needle).map(|from| (from, s.at)),
                        StitchKind::Normal => {
                            // The needle lands where it jumped to, or sews a stitch from where it went down last.
                            let landed = jump.take();
                            let stitch = needle.filter(|_| landed.is_none());
                            if let Some(thread) = landed.filter(|(from, to)| from != to) {
                                threads.push(thread);
                                tying_in = true;
                            }
                            tying_in &= s.origin.role == Role::Lock;
                            // Each thread on the fabric: how far the needle comes down from it and how far before it
                            // lands, how far before it lands the stitch crosses it, and whether this is the tie-in
                            // where the newest one lands.
                            let near = threads.iter().enumerate().map(|(k, &(a, b))| {
                                let crossed = stitch.and_then(|from| crossing(from, s.at, a, b));
                                (sheet.id, a, b, s.at, off(s.at, a, b), crossed, tying_in && k + 1 == threads.len())
                            });
                            sewn_over.extend(near.filter(|&(.., (apart, before), crossed, own_tie_in)| {
                                !own_tie_in && (apart < CATCHES && before >= CATCHES || crossed.is_some_and(|at| at >= CATCHES))
                            }));
                            needle = Some(s.at);
                        }
                    }
                }
            }
        }
        // The count is taken here, so that every line of the test runs whether it passes or not. Each stitch is
        // listed as (sheet, thread from, thread to, needle, (mm off the thread, mm before it lands), mm before it
        // lands where the stitch crosses it, tie-in).
        let found = sewn_over.len();
        assert!(found == 0, "{found} stitches on a jump thread:\n{sewn_over:#?}");
    }

    #[test]
    fn sheets_are_deterministic() {
        for sheet in SHEETS {
            assert_eq!(sheet.plan().unwrap(), sheet.plan().unwrap(), "{}", sheet.id);
        }
    }
}
