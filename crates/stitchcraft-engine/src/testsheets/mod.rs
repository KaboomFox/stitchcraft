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
//! and a part apart, with and without underlay and gap-filling rows. TS-16, for MC-11, gathers what
//! StitchCraft applies of an Ink/Stitch file; the differential writes it as Ink/Stitch writes a file.
//! Sheets use exact Brother PEC thread colours, so a Brother machine shows the thread names the
//! expected-result sheet lists.

mod designed;
pub mod pictures;
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
mod ts11;
mod ts13;
mod ts16;
mod ts17;
pub mod ts17c;

use designed::Drawing;
pub use sketch::{STITCH_LEN, SheetError};
use stitchcraft_core::{Code, ElementId};
use stitchcraft_plan::palette::BROTHER_PEC;
use stitchcraft_plan::profiles::{BROTHER_PE800_4X4, BROTHER_PE800_SMALL, REFERENCE};
use stitchcraft_plan::{MachineProfile, StitchPlan, Thread};

use crate::design::{Design, Hints};

/// Brother PEC indices of the threads the sheets use.
const PRUSSIAN_BLUE: u8 = 1;
const RED: u8 = 5;
const BLUE: u8 = 2;
const DEEP_GOLD: u8 = 11;
const YELLOW: u8 = 13;
const BLACK: u8 = 20;
const WHITE: u8 = 29;
const FLESH_PINK: u8 = 42;
const TANGERINE: u8 = 52;
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
    /// As [`Drawn::Designed`], but the elements' ids choose some of its stitches: a meander fill without a
    /// `random_seed` wanders as its id draws. A host's objects have ids of their own, so the sheet drawn into
    /// a document would sew otherwise, and only `stitch testsheet` writes it.
    DesignedById(fn() -> Result<Drawing, SheetError>, &'static [Code]),
    /// As [`Drawn::Designed`], but the design holds what an Ink/Stitch file gives its elements, which a host
    /// would draw as objects of their own: start and end positions, clips, and the design's origin and stop
    /// position, or keep with the document: its turn on export ([`TestSheet::uses_file_features`]), as TS-16's.
    /// Only `stitch testsheet` and the differential's SVG file draw it.
    FromFile(fn() -> Result<Drawing, SheetError>, &'static [Code]),
    /// As [`Drawn::Designed`], but part of it is routed (`crate::route`), into pieces cut at points along its
    /// columns that a document's points hold only to within a unit in their last place. Drawn into a
    /// document, TS-17C's T sews its first stitch 0.05 mm elsewhere, so only `stitch testsheet` and the
    /// differential's SVG file draw it.
    Routed(fn() -> Result<Drawing, SheetError>, &'static [Code]),
}

impl TestSheet {
    /// The sheet's stitch plan, centred on the machine origin.
    pub fn plan(&self) -> Result<StitchPlan, SheetError> {
        match self.drawn {
            Drawn::Sketched(build) => build(),
            Drawn::Designed(drawing, said) | Drawn::DesignedById(drawing, said) | Drawn::FromFile(drawing, said) | Drawn::Routed(drawing, said) => {
                drawing()?.plan_saying(said)
            }
        }
    }

    /// Whether the sheet is drawn as a design, which [`TestSheet::design`] gives, rather than stitch by stitch.
    pub const fn designed(&self) -> bool {
        matches!(self.drawn, Drawn::Designed(..) | Drawn::DesignedById(..) | Drawn::FromFile(..) | Drawn::Routed(..))
    }

    /// Whether a host can draw the sheet into a document that sews as the sheet does: a design whose
    /// elements' ids choose none of its stitches (`REQ-VC-015`), and which holds nothing only an Ink/Stitch
    /// file gives, as a sheet drawn from a file does. It is known from how the sheet is drawn, without drawing
    /// it, so a host can list the sheets it draws within the little work a plug-in may do.
    pub const fn drawable(&self) -> bool {
        matches!(self.drawn, Drawn::Designed(..))
    }

    /// Whether part of the sheet is routed, as TS-17C's lower letters are, which [`TestSheet::drawable`] leaves
    /// out: their pieces are cut at points a document holds only to within a unit in their last place.
    pub const fn routed(&self) -> bool {
        matches!(self.drawn, Drawn::Routed(..))
    }

    /// Whether the sheet's design holds what an Ink/Stitch file gives it, as TS-16's does: elements' start or
    /// end positions or target points ([`Element::hints`]), which a host would draw as markers grouped with
    /// their shapes; clips ([`Element::clips`]), which it would draw as clip groups; the design's origin or
    /// stop position ([`DesignSettings::origin`], [`DesignSettings::stop_position`]), which it would draw as
    /// markers; or its turn on export ([`DesignSettings::rotate_on_export`]), which the Design Settings filter
    /// would keep. VectorCraft's Test Sheet filter draws and keeps none of them. The sheets that do are those
    /// drawn from a file, which [`TestSheet::drawable`] leaves out, as the tests check.
    ///
    /// [`Element::hints`]: crate::design::Element::hints
    /// [`Element::clips`]: crate::design::Element::clips
    /// [`DesignSettings::origin`]: crate::design::DesignSettings::origin
    /// [`DesignSettings::stop_position`]: crate::design::DesignSettings::stop_position
    /// [`DesignSettings::rotate_on_export`]: crate::design::DesignSettings::rotate_on_export
    pub fn uses_file_features(&self) -> bool {
        self.design().and_then(Result::ok).is_some_and(|design| filed(&design))
    }

    /// Whether the sheet's design has shapes that StitchCraft digitizes automatically ([`Element::automatic`]),
    /// as the TS-13 sheets' do. A host draws such a sheet into a document only where an object can say so.
    ///
    /// [`Element::automatic`]: crate::design::Element::automatic
    pub fn digitized(&self) -> bool {
        self.design().and_then(Result::ok).is_some_and(|design| design.elements().iter().any(|element| element.automatic))
    }

    /// How far apart, in millimetres, the places lie that a host may move the sheet to and have it sew as it
    /// does where it is drawn: whole millimetres, or for TS-17A whole multiples of 6 mm, so that its crosses
    /// keep their cells on the canvas's grids of 2 and 3 mm.
    pub fn step(&self) -> f64 {
        if self.id == "TS-17A" { 6.0 } else { 1.0 }
    }

    /// The sheet as the design the engine plans, with its settings, as a host draws it into a document
    /// (`REQ-VC-015`); `None` for a sheet drawn stitch by stitch, which is no design.
    pub fn design(&self) -> Option<Result<Design, SheetError>> {
        match self.drawn {
            Drawn::Sketched(_) => None,
            Drawn::Designed(drawing, _) | Drawn::DesignedById(drawing, _) | Drawn::FromFile(drawing, _) | Drawn::Routed(drawing, _) => {
                Some(drawing().and_then(Drawing::design))
            }
        }
    }

    /// The elements of the sheet's design that its Ink/Stitch file draws as clones of others, as TS-16's file
    /// draws one (`REQ-SVG-012`); `None` for a sheet drawn stitch by stitch, which has no file.
    pub fn clones(&self) -> Option<Result<Vec<Cloned>, SheetError>> {
        match self.drawn {
            Drawn::Sketched(_) => None,
            Drawn::Designed(drawing, _) | Drawn::DesignedById(drawing, _) | Drawn::FromFile(drawing, _) | Drawn::Routed(drawing, _) => {
                Some(drawing().map(Drawing::clones))
            }
        }
    }
}

/// An element of a sheet's design that the sheet's Ink/Stitch file draws as a clone (`<use>`) of another,
/// which Ink/Stitch and the SVG reader both resolve into the element (`REQ-SVG-012`). The design holds the
/// element itself, as it holds any other, so `stitch testsheet` sews it: a clone is how a file draws an
/// element, not something the engine sews otherwise, and the element remembers nothing of it.
#[derive(Clone, Debug, PartialEq)]
pub struct Cloned {
    /// The element the file draws as a clone.
    pub element: ElementId,
    /// The element it is a clone of.
    pub of: ElementId,
    /// The clone's transform, SVG's `matrix(a b c d e f)`: the map from the source's millimetres to the
    /// element's.
    pub map: [f64; 6],
}

/// Whether `design` holds any of what an Ink/Stitch file gives it ([`TestSheet::uses_file_features`]).
fn filed(design: &Design) -> bool {
    let marked = |element: &crate::design::Element| element.hints != Hints::default() || !element.clips.is_empty();
    let settings = &design.settings;
    settings.origin.is_some() || settings.stop_position.is_some() || settings.rotate_on_export.turns() || design.elements().iter().any(marked)
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
    TestSheet {
        id: "TS-11A",
        title: "New strokes and satins: zigzag strokes, E-stitches, S-stitches and zigzag satins",
        checks: &[
            "4 bands, a thread to each, every sample trimmed after. Most samples are hooks, straight and then turning a quarter circle. Top band, in green: zigzag strokes 1, 2 and 4 mm wide down the rows, at spacings of 0.4, 1 and 2 mm across them, and beside the last row a straight line 4 mm wide at 1 mm whose stitches turn 45°.",
            "Second band, in red: E-stitches for appliqué, round a square 26 mm a side and 4 circles 18 mm across. The machine sews a line 1 mm inside each edge and stops: lay the appliqué fabric over the lines. It sews them again to tack the fabric down and stops: trim the fabric 1 mm outside the lines. Clockwise from the top, the square's sides are 3 mm wide at spacings of 2 and 3 mm, then 5 mm wide at 2 and 3 mm. The rings, left to right, are the same 4.",
            "Third band, in blue: S-stitches 6 mm wide, then 10 mm wide split at 3 mm by the default method and by the staggered one. The first row is at a spacing of 0.4 mm and the second at 0.6 mm.",
            "Bottom band, in gold: zigzag satins, each followed by a satin column of the same width and spacing, 4 mm wide in the first row and 8 mm in the second, at spacings of 0.4 and 1 mm. Under them, after a stop, a zigzag satin 8 mm wide at 1 mm over a contour underlay: go on with the same thread.",
            "Measure the zigzag strokes: how much narrower than drawn does each sew? Are their sides even round the curves? Note the narrowest width that sews cleanly, and the spacing that looks best. The line at 45° should sew as wide as the hooks beside it, its stitches slanting.",
            "Does each E-stitch's spine cover the fabric's cut edge, and do its spikes hold the fabric flat, without puckering? Compare the widths of 3 and 5 mm, and the spacings of 2 and 3 mm, on the straight sides and round the circles.",
            "Do the S-stitches cover their bands at 0.4 mm, with even steps along their edges? On the 10 mm bands, do the default method's splits leave a furrow along the band that the staggered ones don't? Round the curves, the inside of each S-stitch packs its steps closer than the machine's shortest stitch, and StitchCraft leaves those needle points out (SC-I0504): does the inner edge still look even?",
            "Does each zigzag satin cover its band at 0.4 mm as the satin column beside it does? Measure both: does the zigzag satin sew narrower? On the last one, does the contour underlay show past the zigzag at either end?",
        ],
        profile: REFERENCE,
        drawn: Drawn::Designed(ts11::strokes, &[Code::StitchesMerged]),
    },
    TestSheet {
        id: "TS-11B",
        title: "New fills: contour and circular fills",
        checks: &[
            "The top 3 rows are contour fills: rings that follow the outline inward, a row spacing apart, sewn as one line over the default underlay. Top row, in green, left to right: a disc 36 mm across sewn inner to outer round a hole, then a single spiral and a double spiral, the rings 0.25 mm apart. Second row, in blue: stars 40 mm across with round, mitred and bevelled corners, the rings 0.4 mm apart. Third row, in red: 2 squares joined by a neck 3 mm wide, a rectangle whose rings alternate direction, and a leaf smoothed by 0.5 mm.",
            "The bottom row, in gold, is circular fills: a spiral round the middle of each shape, cut by its outline, over the default underlay. Left to right: the disc round its hole, the star, and a rectangle whose circles fade from 0.25 mm apart at its middle to 1 mm at its corners. Where the spiral leaves the shape and comes back, the needle travels inside it under the spiral sewn later.",
            "Does fabric show between the rings, or does a fill pucker? Measure the discs: do they sew 36 mm across?",
            "Inner to outer steps in from ring to ring and sews each on its way out. Do the steps show as a line across the rings? On the rectangle, whose rings alternate, the steps don't cross: does it look calmer than the disc?",
            "Where do the spirals end in the middle, and the double spiral on its outline? Is there a knot or a bump there?",
            "On the stars, do the mitred rings reach into the points, and the round and bevelled ones leave a gap at each tip? Which star looks best?",
            "Does the neck between the 2 squares fill, where the rings part to each side? Does the smoothed leaf's line run evenly, without kinks?",
            "Do the circular fills' circles look round and even, and does fabric show between them? Does the travel between the pieces of a spiral show anywhere, on the star's points or round the disc's hole? Does the fading rectangle look denser at its middle and lighter at its corners, as it should?",
            "How long does the sheet take? A contour fill sews as one line, with no travel between rows.",
        ],
        profile: REFERENCE,
        drawn: Drawn::Designed(ts11::contours, &[]),
    },
    TestSheet {
        id: "TS-11C",
        title: "New fills: meander fills",
        checks: &[
            "Each sample is a meander fill: one line that wanders along the sides of a tiling without crossing itself, over the default underlay. Top row, in green, left to right: discs 36 mm across in hexagons, squares and triangles, each side 2 mm long. Middle row, in blue: discs in octagons and squares, hexagons and triangles, and hexagons, squares and triangles. Bottom row, in red: a star 40 mm across in squares and triangles, a rectangle in hexagons twice as large sewn as a zigzag 2.5 mm wide, and a leaf in hexagons half as large, turned by 30° and smoothed by 0.5 mm.",
            "Does each tiling show in the stitches? Does the line look even, or do bare patches show where it didn't reach? Which tile looks best, and at which size?",
            "Does the underlay show between the line's turns? A meander covers the area lightly: is it too light to hide the fabric, or the underlay?",
            "On the rectangle, does the zigzag stay inside its outline, and lie flat? Is 2.5 mm too wide?",
            "On the leaf, does the smoothed line run evenly round its turns, without kinks? Do the 1 mm sides sew cleanly, or do the stitches bunch?",
            "Does any sample pucker, or the line pull the fabric where it turns?",
        ],
        profile: REFERENCE,
        drawn: Drawn::DesignedById(ts11::meanders, &[]),
    },
    TestSheet {
        id: "TS-13A",
        title: "Pictures digitized automatically: neighbours and sewing order",
        checks: &[
            "The top half is digitized automatically, and the bottom half is the same shapes sewn as drawn, in paint order. The bottom half sews first, and the machine stops between them. Top row, left to right: a badge traced abutting and one traced stacked, each 40 mm across, and a blue disc with 7 red specks round it. Bottom row: 3 badges drawn badge by badge, and a gold square with a blue bar beside it.",
            "The bottom half changes thread 16 times and jumps 15 times, the top half 3 and 16 times. How long does each half take?",
            "Does fabric show between colours, on either half? On the top half, each area reaches 0.4 mm under the next: does that cover the gap, or show as a ridge?",
            "On the top half, the stacked badge sews only what shows of each colour, as the abutting badge beside it does: the red disc whole, cut away under the square, and the blue square's ring round the gold disc. Does it sew as flat as the abutting badge? On the bottom half, it sews its lower colours whole, under the upper ones.",
            "On the bottom half, each speck round the disc sews a red knot. On the top half, the 6 specks that touch the disc are blue, part of it, and the one apart is left out. Does the disc's edge show where they were?",
            "The blue bar meets the gold square along the top half of its side, and runs on 0.5 mm from it below. On the top half, the bar, sewn first, reaches under the square only where they meet. Does the fabric that shows below read as a gap drawn there?",
            "Does any shape cover one drawn over it? The order must never let that happen.",
            ts13::JUMP_THREADS,
        ],
        profile: REFERENCE,
        drawn: Drawn::Designed(ts13::neighbours, &ts13::NEIGHBOURS_SAY),
    },
    TestSheet {
        id: "TS-13B",
        title: "Pictures digitized automatically: letters and lines traced as areas",
        checks: &[
            "The top half is digitized automatically, and the bottom half is the same shapes sewn as drawn, in paint order. The bottom half sews first, and the machine stops between them. Left: the word MATCH in block letters 15, 10, 6 and 4 mm tall, top to bottom, each letter traced as an area. Right: 3 windows, top to bottom, whose black lines, traced as areas, are 0.3, 0.6 and 0.9 mm wide. One line runs down each window, and a branch runs across to its right side from it.",
            "The bottom half changes thread 12 times and jumps 16 times, the top half 4 and 41 times. A letter filled sews in one piece, and digitized it is columns and lines with jumps between them. How long does each half take?",
            "Which is the smallest word that reads, on each half?",
            "On the top half, the letters from 6 mm tall sew as satin columns between their own edges, and the 4 mm ones as triple runs along their middles. Do the columns' stitches lie square to their strokes, round the C's bend and the A's hole?",
            "Where a column ends under another, 0.4 mm longer, as the T's stem does under its bar, does fabric show there, or a ridge? Does a crossbar sewn as a column of its own, as the H's is, read as one stroke with its stems? The M's and the A's feet should end flat along the baseline, and the C's ends round.",
            "Do the underlays hold the columns up? With 0.4 mm of pull compensation, do they sew as wide as drawn? Measure the 15 mm T's stem, drawn 2.7 mm wide.",
            "Where the M's strokes meet at sharp angles, its stitches lean along the columns (SC-W0208). Do they show?",
            "On the top half, each window's lines sew along their middles after the areas beside them, which reach 0.4 mm under them: a single run at 0.3 mm, and a triple run at 0.6 and 0.9 mm. Does each line reach as far as it is drawn? Do the triple runs look as wide as their lines, and the single run not too thin? Does fabric show along a line, or a gap or a knot where the branch ends at the line it meets?",
            ts13::JUMP_THREADS,
        ],
        profile: REFERENCE,
        drawn: Drawn::Designed(ts13::letters, &ts13::LETTERS_SAY),
    },
    TestSheet {
        id: "TS-13C",
        title: "Pictures digitized automatically: lines, and shapes in parts",
        checks: &[
            "The top half is digitized automatically, and the bottom half is the same shapes sewn as drawn, in paint order. The bottom half sews first, and the machine stops between them. Left to right: a face drawn with black lines, a star, and a balloon on its string.",
            "The bottom half changes thread 4 times and jumps 14 times, the top half 4 and 11 times. How long does each half take?",
            "The face's outline is 4 mm wide, its eyes' 2 mm and its brows' 1 mm. On the top half, the outlines sew as satin columns and the brows as triple runs. Do the columns cover the edges of the pink and white areas beside them? Do the triple runs look as wide as the brows are drawn?",
            "The smile is 2 mm wide with round caps, and on the top half its column ends square, 1 mm past each end of its line, where the round cap ends. How far does its corner show past the round end?",
            "The pupils are small rings, and the 3 hairs hairpins, which turn more tightly than they are wide: the top half fills them. How solid do they sew beside the columns round them?",
            "On the top half, the star's points and the balloon's string sew as satin columns over the fill of the star's middle and of the balloon, cut square where they meet it. Does each column's flat end cover the fill's edge, or does fabric or a ridge show there? Do each point's stitches lie square to it from its tip to its cut?",
            "Does each line sewn after the area it borders cover the area's edge? Does any shape cover one drawn over it?",
            ts13::JUMP_THREADS,
        ],
        profile: REFERENCE,
        drawn: Drawn::Designed(ts13::lines_and_parts, &ts13::LINES_AND_PARTS_SAY),
    },
    TestSheet {
        id: "TS-13D",
        title: "Pictures digitized automatically: pictures traced in layers",
        checks: &[
            "The top half is digitized automatically, and the bottom half is the same shapes sewn as drawn, in paint order. The bottom half sews first, and the machine stops between them. Left to right: an emblem traced abutting and the same emblem traced stacked, each 42 mm wide, a navy square with a white disc, a red star on the disc and 3 gold bars, on white; and a button traced abutting above the same button traced stacked, a yellow disc 19 mm across with 7 orange dots, in a black rim.",
            "The bottom half changes thread 13 times and jumps 12 times, the top half 7 and 18 times. How long does each half take?",
            "On the bottom half, the stacked emblem sews each colour whole under the colours drawn over it, the white under everything. How stiff is it beside the abutting one? On the top half, it sews only what shows of each colour: does it sew as the abutting emblem beside it?",
            "On the top half, each navy square is one fill under the white disc and the bars, which sew after it, cut away 0.4 mm inside their edges. The white round each star, narrower than a satin column there, is a satin column round it. On the bottom half, the abutting emblem's navy sews round the disc and between the bars as drawn. Which sews better between the bars? Does fabric or a ridge show round the bars, the disc and the star?",
            "On the top half, the gold bars sew as satin columns, 2.1 mm tall as drawn. Do they keep their height, and do their ends stay square?",
            "On the top half, each button's yellow is one fill under the dots, which sew after it, and its black rim a satin column round it. On the bottom half, the abutting button's yellow sews round the dots as drawn. Which sews better between the dots? Does the rim cover the yellow's edge?",
            ts13::JUMP_THREADS,
        ],
        profile: REFERENCE,
        drawn: Drawn::Designed(ts13::layers, &ts13::LAYERS_SAY),
    },
    TestSheet {
        id: "TS-16",
        title: "An Ink/Stitch file: start and end commands, clip paths, origin, stop position, turn, a clone and drawn locks",
        checks: &[
            "Sew it beside Ink/Stitch's machine file of the same sheet, which the differential keeps (`target/conformance/differential/ts-16.inkstitch.pes`). Each element should start and end in the same place in both.",
            "The red fill, 40 by 30 mm, starts at its top-right corner, where its start command is, and ends at its bottom-left corner, at its end command.",
            "The blue satin column below it, 40 mm long and 5 mm wide, starts in its middle, where its start command is: the needle runs under the column to its left end, sews it to three quarters along, runs on under it to its right end and sews back. It ends three quarters along, at its end command. Is the needle's way under the column hidden?",
            "The green fill is a quarter of a disc, 25 mm round its bottom-right corner: what its clip path, a circle, leaves of a 30 mm square. Is its round edge smooth, with no row past it?",
            "The black zigzag at the bottom sews its first 15 mm and its last 15 mm, and nothing between, where its clip path leaves a gap: the needle jumps it. Does each piece end where the gap begins?",
            "The design's origin, on the blue column's top edge 10 mm in from its left end, goes to the hoop's centre, where the needle waits before it sews. Does the machine put the design there, or its middle?",
            "The machine stops after the blue column. Before it stops, the frame moves to the stop position, 20 mm right of the red fill's top-right corner and 10 mm above it. Does the frame wait there, and does the needle come back for the green fill?",
            "The file turns the sheet right about its origin, as Ink/Stitch's rotate on export does: its first 4 shapes sew 100 mm wide and 40 mm tall, the red fill at the right and the zigzag at the left. Does the machine sew it so, or turn it back?",
            "In red again, the red fill's clone sews below the fill, turned 45° right. Do its rows run along its long sides, as the fill's run along its own? It starts at its corner turned from the fill's top-right one and ends at the one turned from the fill's bottom-left.",
            "Last, in black after a colour change, 2 lines 40 mm long sew above the rest, each locked at both ends with a lock drawn as a path and trimmed after. The lower one, sewn from the right, starts with a zigzag across its first 1.6 mm and ends with a triangle over its last 1.4 mm, sewn backwards. The upper one, sewn from the left at 150 %, starts with a triangle along its first 1.8 mm and ends with one over its last 1.8 mm. Snip the thread between them, then pull each tail gently: does each lock hold? Does each look as it does in Ink/Stitch's sew-out?",
            "Any difference between the 2 sew-outs, and where.",
        ],
        profile: REFERENCE,
        drawn: Drawn::FromFile(ts16::drawing, &[]),
    },
    TestSheet {
        id: "TS-17A",
        title: "New stitch types: cross stitch",
        checks: &[
            "Each sample is a cross stitch fill, trimmed after. First row, in red, left to right: squares 24 mm wide on grids of 2 mm with 2 and 4 threads, then of 3 mm with 2 and 4 threads. Second row, in blue: the 2 mm square with 2 threads and the 3 mm one with 4, flipped, then a K 24 mm tall on a 2 mm grid with 2 threads, grown by 0.1 mm as Ink/Stitch's cross-stitch fonts grow their letters. Third row, in green, on the 3 mm grid: squares of upright, dense upright, double and Smyrna crosses, the double ones with 2 threads. Fourth row, in black: squares of half crosses with 3 threads, plain and flipped, and a square 18 mm wide turned 30°, its grid turned with it.",
            "Does every cross of the top row lie with its / diagonal on top, and every cross of the flipped squares with its \\ diagonal on top?",
            "Between its 2 diagonals the needle passes through each cross's middle, under the diagonal sewn last. Do those stitches show? The squares and the K first travel from where the needle comes in to where they end, through the middles of their crosses: does that travel show?",
            "Does fabric show between the crosses at 2 threads? At 4, do the holes at the cells' corners stay clean, or do they pack and tear? Which grid and thread count look best?",
            "The K's arms slant across the grid, and each cell their edges cut in half sews a cross, so the arms step along the grid. Does the K read at 24 mm on a 2 mm grid? A few of its crosses are reached by a corner of their bottom diagonal, and end with a stitch from their middle to it on top: do those show?",
            "Each upright cross sews its bar across and then its bar down over it, a + in each cell. The dense square adds a second grid of them half a cell over, on the corners of the first, and those along its sides reach half a cell past them. Does the dense square close the gaps of the plain one, or pack too much thread?",
            "Each double cross sews its \\ diagonal, an upright cross over it and its / diagonal over that, and each Smyrna cross both diagonals and an upright cross over them, its bar down on top. How do the layers look at 2 and 4 threads? Do the many stitches through each cell's middle pack or tear it?",
            "The half crosses sew one diagonal in each cell, \\ on the left square and / on the flipped one, 3 times, joined into rows along the square's diagonals. Between the rows the needle travels along the square's sides, sewn once. Do those travel stitches show on its edges?",
            "The square turned 30° sews its crosses square to its own sides, as a letter turned along a path sews its own. Do its rows of crosses run along its edges, with no part cell along them?",
            "How long does the sheet take?",
        ],
        profile: REFERENCE,
        drawn: Drawn::Designed(ts17::crosses, &[]),
    },
    TestSheet {
        id: "TS-17C",
        title: "Ripples and routing: satin letters as drawn and routed",
        checks: &[
            "Top: the word ROUTE in satin columns 3 mm wide, sewn as drawn, column by column. Bottom: the same letters as auto-satin routing joins them, cut into pieces, with running stitches under the satins and a trim between letters.",
            "Count the jump threads of each half, within each letter and between the letters. The routed half should have none within a letter. Its file asks for a trim between letters, which a machine that obeys trims cuts.",
            "On the routed half, do the running stitches under the satins show anywhere, at a column's edge, at its ends or where 2 columns meet?",
            "Where a routed column is cut into pieces, is the satin even across the cut, or does a gap, a ridge or a stitch out of line show?",
            "The time each half takes, from its first stitch to its last.",
        ],
        profile: REFERENCE,
        drawn: Drawn::Routed(ts17c::drawing, &[Code::StitchesMerged]),
    },
];

/// The sheet with id `id` (any case).
pub fn find(id: &str) -> Option<&'static TestSheet> {
    SHEETS.iter().find(|sheet| sheet.id.eq_ignore_ascii_case(id))
}

#[cfg(test)]
mod tests {
    use stitchcraft_plan::invariants::check;
    use stitchcraft_plan::{Rgb, Role, StitchKind};

    use super::*;
    use crate::design::Turn;

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
        assert_eq!(
            designed,
            [
                "TS-02B", "TS-03", "TS-04", "TS-05", "TS-06", "TS-07", "TS-08", "TS-09", "TS-11A", "TS-11B", "TS-11C", "TS-13A", "TS-13B", "TS-13C",
                "TS-13D", "TS-16", "TS-17A", "TS-17C"
            ]
        );
        // A host moves each by whole millimetres, but TS-17A by multiples of 6 mm, which keep its crosses' cells.
        assert_eq!(SHEETS.iter().filter(|sheet| sheet.step() != 1.0).map(|sheet| (sheet.id, sheet.step())).collect::<Vec<_>>(), [("TS-17A", 6.0)]);
        // The TS-13 sheets' shapes are digitized automatically, and no other sheet's are.
        let digitized: Vec<&str> = SHEETS.iter().filter(|sheet| sheet.digitized()).map(|sheet| sheet.id).collect();
        assert_eq!(digitized, ["TS-13A", "TS-13B", "TS-13C", "TS-13D"]);
        // TS-16 alone places and clips its elements, as an Ink/Stitch file does, and it alone is drawn from a
        // file.
        let filed: Vec<&str> = SHEETS.iter().filter(|sheet| sheet.uses_file_features()).map(|sheet| sheet.id).collect();
        assert_eq!(filed, ["TS-16"]);
        let from_file: Vec<&str> = SHEETS.iter().filter(|sheet| matches!(sheet.drawn, Drawn::FromFile(..))).map(|sheet| sheet.id).collect();
        assert_eq!(from_file, filed);
    }

    #[test]
    fn req_vc_015_a_host_draws_the_sheets_whose_stitches_their_elements_ids_dont_choose() {
        // Each element renamed, as a host's objects are: the same stitches, but for TS-11C's meander fills,
        // which wander as their ids draw. TS-16's start and end positions would be markers, and its clips clip
        // groups, which the filter doesn't draw.
        let stitches = |design: &crate::design::Design| {
            let plan = crate::plan(design, REFERENCE, &stitchcraft_core::Budget::DEFAULT).plan.unwrap();
            plan.stitches().map(|s| (s.at, s.kind)).collect::<Vec<_>>()
        };
        let mut drawable = Vec::new();
        for sheet in SHEETS.iter().filter(|sheet| sheet.designed()) {
            let design = sheet.design().unwrap().unwrap();
            let renamed: Vec<crate::design::Element> = design
                .elements()
                .iter()
                .map(|e| crate::design::Element { id: stitchcraft_core::ElementId::new(format!("host:{}", e.id.as_str())).unwrap(), ..e.clone() })
                .collect();
            let renamed = crate::design::Design::new(renamed, design.settings).unwrap();
            assert_eq!(stitches(&renamed) == stitches(&design), sheet.drawable() || sheet.uses_file_features() || sheet.routed(), "{}", sheet.id);
            if sheet.drawable() {
                drawable.push(sheet.id);
            }
        }
        assert_eq!(
            drawable,
            [
                "TS-02B", "TS-03", "TS-04", "TS-05", "TS-06", "TS-07", "TS-08", "TS-09", "TS-11A", "TS-11B", "TS-13A", "TS-13B", "TS-13C", "TS-13D",
                "TS-17A"
            ]
        );
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
        // TS-11A runs from x = -1, where the last hook of its 2 mm zigzags ends 1 mm either side of x = 0, to 126,
        // where its first S-stitches end, and from just below y = 3.5, the top of its 1 mm zigzags, which sew a
        // little narrower than drawn, to 148, where its last hook ends.
        assert_eq!(about("TS-11A"), [126.997970829, 144.458917156, 0.0, 0.0]);
        // The right-hand star's upper point on its right lies 20 mm from its middle, 18° above the level.
        assert_eq!(about("TS-11B"), [125.021130326, 165.993678599, 0.0, 0.0]);
        // TS-11C's star, in its bottom row, reaches 20 sin 54° mm below its middle, and the discs of its top row
        // 18 mm above theirs.
        assert_eq!(about("TS-11C"), [123.0, 120.176369967, 0.0, 0.0]);
        // TS-16's shapes, 40 × 100 mm as drawn, and the frame's move to its stop position, 20 mm right of them and
        // 10 mm above, round its origin, which lies 10 mm from their left side and 40 mm from their top, all
        // turned right. Its clone of the red fill reaches 35 / √2 mm from its middle, 62.5 mm right of the
        // origin, so 87.25 mm below it once turned. Its lines with drawn locks lie 20 and 30 mm left of the
        // origin, so above it once turned, their locks inside the 30 mm.
        assert_eq!(about("TS-16"), [110.0, 117.248737342, -5.0, 28.624368671]);
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

    /// Where TS-16's plan puts the point (x, y) of the sheet as drawn: moved so that its origin is at (0, 0),
    /// then turned as the sheet is written.
    fn ts16_planned(x: f64, y: f64) -> stitchcraft_core::Point {
        let [(ox, oy), _] = ts16::ORIGIN_AND_STOP_POSITION;
        ts16::TURN.apply(stitchcraft_core::Point::new(x - ox, y - oy).unwrap()).unwrap()
    }

    /// Where TS-16 draws its plan's point `q`: turned back, and moved from its origin.
    fn ts16_drawn(q: &stitchcraft_core::Point) -> stitchcraft_core::Point {
        let [(ox, oy), _] = ts16::ORIGIN_AND_STOP_POSITION;
        let back = Turn::new(-ts16::TURN.degrees()).and_then(|turn| turn.apply(*q)).unwrap();
        stitchcraft_core::Point::new(back.x() + ox, back.y() + oy).unwrap()
    }

    #[test]
    fn ts16_s_fill_and_satin_column_start_and_end_at_their_positions() {
        let at = ts16_planned;
        let fill = sewn("TS-16", "fill");
        assert!(fill[0].distance(at(40.0, 0.0)) < 1.5, "{:?}", fill[0]);
        assert!(fill.last().unwrap().distance(at(0.0, 30.0)) < 1.5, "{:?}", fill.last());
        let column = sewn("TS-16", "column");
        assert!(column[0].distance(at(20.0, 42.5)) < 1e-9 && column.last().unwrap().distance(at(30.0, 42.5)) < 1e-9, "{column:?}");
    }

    #[test]
    fn ts16_s_clone_of_the_red_fill_turns_its_rows_and_its_positions_with_it() {
        // The clone's map, from the fill's millimetres to its own, as the sheet draws it, and back.
        let (sin, cos) = stitchcraft_core::math::sin_cos(stitchcraft_core::math::to_radians(ts16::CLONE_TURN));
        let [left, top, right, bottom] = ts16::FILL;
        let ((cx, cy), (mx, my)) = (((left + right) / 2.0, (top + bottom) / 2.0), ts16::CLONE_MIDDLE);
        let turned =
            |(x, y): (f64, f64)| stitchcraft_core::Point::new(cos * (x - cx) - sin * (y - cy) + mx, sin * (x - cx) + cos * (y - cy) + my).unwrap();
        let back = |q: &stitchcraft_core::Point| {
            let (dx, dy) = (q.x() - mx, q.y() - my);
            (cos * dx + sin * dy + cx, -sin * dx + cos * dy + cy)
        };
        let clone: Vec<_> = sewn("TS-16", "clone").iter().map(ts16_drawn).collect();
        // It sews inside the fill turned, from the corner turned from the fill's start to the one turned from its end.
        assert!(clone.iter().map(back).all(|(x, y)| (left - 0.1..=right + 0.1).contains(&x) && (top - 0.1..=bottom + 0.1).contains(&y)), "{clone:?}");
        assert!(clone[0].distance(turned((right, top))) < 1.5, "{:?}", clone[0]);
        assert!(clone.last().unwrap().distance(turned((left, bottom))) < 1.5, "{:?}", clone.last());
        // Most of its stitches run along its rows, at -45° as the sheet is drawn: down to the right, or back.
        let long: Vec<(f64, f64)> =
            clone.windows(2).map(|w| (w[1].x() - w[0].x(), w[1].y() - w[0].y())).filter(|(dx, dy)| (dx * dx + dy * dy).sqrt() > 1.5).collect();
        let along = long.iter().filter(|(dx, dy)| dx * dy > 0.0 && (dx.abs() - dy.abs()).abs() < 0.05 * (dx * dx + dy * dy).sqrt()).count();
        assert!(2 * along > long.len(), "{along} of {}", long.len());
        let design = find("TS-16").unwrap().design().unwrap().unwrap();
        let element = design.elements().iter().find(|e| e.id.as_str() == "ts16:clone").unwrap();
        assert_eq!(element.params.get(crate::generators::tatami::ANGLE), Some(ts16::CLONE_ANGLE));
        // And the file draws it as a clone of the fill.
        let clones = find("TS-16").unwrap().clones().unwrap().unwrap();
        assert_eq!(clones.iter().map(|c| (c.element.as_str(), c.of.as_str())).collect::<Vec<_>>(), [("ts16:clone", "ts16:fill")]);
        assert_eq!(clones[0].map, [cos, sin, -sin, cos, mx - cos * cx + sin * cy, my - sin * cx - cos * cy]);
        // No other sheet's file draws a clone, and a sheet drawn stitch by stitch has no file.
        assert!(SHEETS.iter().filter(|sheet| sheet.id != "TS-16").all(|sheet| sheet.clones().is_none_or(|clones| clones.unwrap().is_empty())));
        assert!(find("TS-01").unwrap().clones().is_none());
    }

    #[test]
    fn ts16_s_lines_are_locked_with_locks_drawn_as_paths() {
        // The lock stitches each line sews, in order: its start lock's and its end lock's.
        let locked = |name: &str| {
            let plan = find("TS-16").unwrap().plan().unwrap();
            let element = plan.elements.iter().position(|e| e.as_str() == format!("ts16:{name}")).unwrap();
            let lock = |s: &&stitchcraft_plan::Stitch| {
                s.kind == StitchKind::Normal && s.origin.role == Role::Lock && s.origin.element.map(|e| e.index()) == Some(element)
            };
            plan.stitches().filter(lock).map(|s| s.at).collect::<Vec<_>>()
        };
        let [[(ax, ay), (bx, by)], [(cx, cy), (dx, dy)]] = ts16::LOCKED_LINES;
        // The first runs down as drawn: its start lock lies along it, a quarter turn on to the left, and its
        // end lock back up it, sewn backwards from its last point.
        let (start, end) = (|x: f64, y: f64| ts16_planned(ax - y, ay + x), |x: f64, y: f64| ts16_planned(bx + y, by - x));
        let zigzag = [start(0.0, 0.0), start(0.4, 0.4), start(0.8, -0.4), start(1.2, 0.4), start(1.6, -0.4)];
        let triangle = [end(0.7, 0.5), end(1.4, 0.0), end(0.0, 0.0)];
        let first = locked("locked-1");
        assert_eq!(first.len(), 8, "{first:?}");
        assert!(first.iter().zip(zigzag.iter().chain(&triangle)).all(|(a, b)| a.distance(*b) < 1e-9), "{first:?}");
        // The second runs up, at 150 %.
        let (start, end) = (|x: f64, y: f64| ts16_planned(cx + 1.5 * y, cy - 1.5 * x), |x: f64, y: f64| ts16_planned(dx - 1.5 * y, dy + 1.5 * x));
        let want = [start(0.0, 0.0), start(1.2, 0.0), start(1.2, 0.4), end(0.6, -0.4), end(1.2, 0.0), end(0.0, 0.0)];
        let second = locked("locked-2");
        assert_eq!(second.len(), 6, "{second:?}");
        assert!(second.iter().zip(&want).all(|(a, b)| a.distance(*b) < 1e-9), "{second:?}");
        // In black, after the clone, each trimmed after.
        assert_eq!(shape("TS-16").len(), 6);
        let params =
            |name: &str| find("TS-16").unwrap().design().unwrap().unwrap().elements().iter().find(|e| e.id.as_str() == name).unwrap().params.clone();
        assert_eq!(
            (params("ts16:locked-2").get("lock_custom_end"), params("ts16:locked-1").get("trim_after")),
            (Some(ts16::DRAWN_LOCKS[1].1), Some("true"))
        );
    }

    #[test]
    fn ts16_s_frame_moves_to_its_stop_position_before_its_stop() {
        let plan = find("TS-16").unwrap().plan().unwrap();
        let [_, (sx, sy)] = ts16::ORIGIN_AND_STOP_POSITION;
        let stitches: Vec<_> = plan.stitches().collect();
        let stops: Vec<_> = stitches.windows(2).filter(|w| w[1].kind == StitchKind::Stop).map(|w| (w[0].kind, w[0].at)).collect();
        assert_eq!(stops, [(StitchKind::Jump, ts16_planned(sx, sy))]);
        // It stops at the end of the blue column, the second element, whose thread's block it ends.
        assert!(shape("TS-16")[1].ends_with("llllJP"), "{:?}", shape("TS-16"));
    }

    #[test]
    fn a_design_holds_what_a_file_gives_it_with_any_one_of_the_file_s_features() {
        // Each alone, as a host would draw or keep it: start and end positions, a clip, an origin, a stop
        // position, a turn on export.
        let line = || {
            let mut d = Drawing::new("test");
            d.polyline("line", &[(0.0, 0.0), (10.0, 0.0)], &Thread::new(Rgb::new(0, 0, 0)), &[]).unwrap();
            d
        };
        assert!(!filed(&line().design().unwrap()));
        let mut positioned = line();
        positioned.positions((0.0, 0.0), (10.0, 0.0)).unwrap();
        let mut clipped = line();
        clipped.clip(vec![designed::polygon(&[(0.0, -1.0), (5.0, -1.0), (5.0, 1.0)]).unwrap()]);
        assert!(filed(&positioned.design().unwrap()) && filed(&clipped.design().unwrap()));
        let at = Some(stitchcraft_core::Point::new(5.0, 0.0).unwrap());
        let (mut placed, mut stopped, mut turned) = (line().design().unwrap(), line().design().unwrap(), line().design().unwrap());
        (placed.settings.origin, stopped.settings.stop_position, turned.settings.rotate_on_export) = (at, at, crate::design::Turn::RIGHT);
        assert!(filed(&placed) && filed(&stopped) && filed(&turned));
    }

    #[test]
    fn ts16_s_quarter_disc_and_zigzag_sew_what_their_clip_paths_leave() {
        let drawn = ts16_drawn;
        let ((cx, cy), radius) = ts16::CIRCLE;
        let corner = stitchcraft_core::Point::new(cx, cy).unwrap();
        let quarter: Vec<_> = sewn("TS-16", "quarter").iter().map(drawn).collect();
        assert!(quarter.iter().all(|q| q.distance(corner) <= radius + 0.1 && q.x() <= cx + 0.1 && q.y() <= cy + 0.1), "{quarter:?}");
        assert!(quarter.iter().any(|q| q.distance(corner) > radius - 1.0), "it reaches its round edge");
        let zigzag: Vec<_> = sewn("TS-16", "zigzag").iter().map(drawn).collect();
        assert!(zigzag.iter().all(|q| q.x() <= 15.05 || q.x() >= 24.95), "{zigzag:?}");
        // It starts and ends where it is drawn to, which the clip path's outer edges leave inside it.
        let at = |(x, y): (f64, f64)| stitchcraft_core::Point::new(x, y).unwrap();
        let (first, last) = (zigzag.first().unwrap(), zigzag.last().unwrap());
        let (start, end) = (at(*ts16::ZIGZAG.first().unwrap()), at(*ts16::ZIGZAG.last().unwrap()));
        assert!(first.distance(start) < 0.06 && last.distance(end) < 0.06, "{zigzag:?}");
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
    /// The sheets that sew pictures or letters as the engine plans any design, jump threads and all: which jump
    /// threads their shapes sew down is one of their checks, and TS-17C counts them on its letters as drawn
    /// and routed.
    const SEWN_AS_PLANNED: [&str; 5] = ["TS-13A", "TS-13B", "TS-13C", "TS-13D", "TS-17C"];

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
        for sheet in SHEETS.iter().filter(|sheet| !SEWN_AT_MC1.contains(&sheet.id) && !SEWN_AS_PLANNED.contains(&sheet.id)) {
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
