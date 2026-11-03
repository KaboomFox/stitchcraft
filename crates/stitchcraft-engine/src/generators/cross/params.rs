//! Cross stitch's settings, under Ink/Stitch's names and with the defaults it sews for a file that sets
//! none, which its files and its cross-stitch fonts count on (`docs/src/design/algorithms/cross-stitch.md`).
//!
//! **Why here.** A setting is declared once, beside the generator that reads it, and the registry, the
//! reference pages, the JSON Schema, VectorCraft's dialogs and the compatibility contract are made from the
//! declaration. Reading the settings into what the generator needs ([`CrossParams::cross`]) is here too, so
//! that what a setting means, and what is said when its value can't be sewn as set, lies beside it.
//!
//! **What changes as it is read.** A full cross takes an even number of passes on each half of a
//! diagonal, so an odd `cross_thread_count` sews the even count below it, 2 at least (`SC-W0320`). A longest
//! stitch shorter than twice the shortest stitch is raised to it (`SC-W0402`), as a fill's is. A grid whose
//! half diagonal is shorter than the shortest stitch is said (`SC-W0321`). The methods of M10.2 are kept for
//! the files that name them, and an element set to one is skipped (`SC-W0011`).

use stitchcraft_core::math::hypot;
use stitchcraft_core::units::at_least;
use stitchcraft_core::{Code, Diagnostic, Mm};
use stitchcraft_params::{ChoiceOption, StitchType, params};

use super::Cross;
use crate::generators::mm;
use crate::generators::running::raised;

/// The stitch types cross stitch's own settings apply to.
const CROSS: &[StitchType] = &[StitchType::CrossStitch];

/// The method of plain crosses, the default: each `\` diagonal under its `/` one.
pub const SIMPLE: &str = "simple_cross";
/// The method of flipped crosses: each `/` diagonal under its `\` one.
pub const FLIPPED: &str = "simple_cross_flipped";

/// The cross stitch methods `cross_stitch_method` reads, in Ink/Stitch's order, which its files count on:
/// Ink/Stitch gives the parameter's default as the first one's place in this list. The methods M10.2 sews
/// are kept for the files that name them, and dialogs don't offer them yet.
pub const METHODS: &[ChoiceOption] = &[
    ChoiceOption::new(SIMPLE, "Simple cross"),
    ChoiceOption::new(FLIPPED, "Simple cross, flipped"),
    ChoiceOption::kept("half_cross", "Half cross"),
    ChoiceOption::kept("half_cross_flipped", "Half cross, flipped"),
    ChoiceOption::kept("upright_cross", "Upright cross"),
    ChoiceOption::kept("upright_cross_flipped", "Upright cross, flipped"),
    ChoiceOption::kept("dense_upright_cross", "Dense upright cross"),
    ChoiceOption::kept("dense_upright_cross_flipped", "Dense upright cross, flipped"),
    ChoiceOption::kept("double_cross", "Double cross"),
    ChoiceOption::kept("double_cross_flipped", "Double cross, flipped"),
    ChoiceOption::kept("smyrna_cross", "Smyrna cross"),
    ChoiceOption::kept("smyrna_cross_flipped", "Smyrna cross, flipped"),
];

params! {
    /// Cross stitch: a cross in each cell of a grid laid over the fill, where the fill covers enough of the
    /// cell, as a hand embroiderer counts crosses on even-weave fabric. Each cross sews one diagonal, then
    /// the other over it, and the crosses that meet at a corner sew in one run.
    pub struct CrossParams for CROSS;

    "Crosses" {
        /// The kind of cross. A simple cross sews its `\` diagonal and then its `/` diagonal over it, as the
        /// drawing shows it; flipped, the `/` diagonal goes under. The other kinds arrive in a later version,
        /// and until then an element set to one is skipped (`SC-W0011`).
        cross_stitch_method: Choice = "simple_cross", label "Cross", choices METHODS;

        /// How many times the thread passes along each diagonal of a cross. A full cross takes an even
        /// number, since the needle comes back to where it started: an odd number sews the even number below
        /// it, and 1 sews 2. More threads make bolder crosses, and pack the holes at the cells' corners.
        cross_thread_count: Count = "4", label "Threads", range (1, 20);

        /// The longest stitch a cross sews. A diagonal longer than this, on a large grid, is sewn in equal
        /// stitches, each no longer.
        max_cross_stitch_length_mm: Length = "11", label "Longest cross stitch", range (0.1, 100.0);
    }

    "Grid" {
        /// How wide and how tall the grid's cells are, each the size of a cross. One length gives square
        /// cells; 2 separated by a space give the width, then the height.
        pattern_size_mm: LengthPair = "3", label "Cell size", range (0.1, 100.0);

        /// Lay the grid from the canvas's origin, so that cross stitch fills side by side share one grid and
        /// their crosses line up. Moved, the fill may then sew other crosses. Off, the grid starts at the top
        /// left corner of each part of the fill, which then sews the same wherever it lies.
        canvas_grid_origin: Toggle = "true", label "Grid on the canvas";

        /// How far the grid is moved: left by the first length, and down by the second. One length moves it
        /// as far each way. Moved by a whole cell, the grid lies where it was.
        cross_offset_mm: LengthPair = "0", label "Grid offset", range (0.0, 100.0);

        /// How much of a cell, in percent, the fill must cover for the cell to sew a cross. Lower, the
        /// crosses reach further past the fill's edges; at 100, only cells wholly inside it sew.
        fill_coverage: Percent = "50", label "Coverage", range (1.0, 100.0);
    }
}

impl CrossParams {
    /// The cross stitch these settings sew, grown by `expand` mm, with `min_stitch` the element's shortest
    /// stitch: what is said of them goes to `diagnostics`. `None`, saying so with `SC-W0011`, when its method
    /// is one this version doesn't sew.
    pub fn cross(&self, expand: f64, min_stitch: Mm, diagnostics: &mut Vec<Diagnostic>) -> Option<Cross> {
        let flipped = match self.cross_stitch_method {
            SIMPLE => false,
            FLIPPED => true,
            other => {
                let message =
                    format!("This element's cross stitch method, `{other}`, is not sewn by this version of StitchCraft yet, so it is skipped.");
                diagnostics.push(Diagnostic::new(Code::StitchTypeNotYet, message));
                return None;
            }
        };
        let threads = self.threads(diagnostics);
        let min = min_stitch.get();
        let longest = raised("max_cross_stitch_length_mm", self.max_cross_stitch_length_mm, min, diagnostics);
        let size = self.pattern_size_mm.map(Mm::get);
        let half = hypot(size[0], size[1]) / 2.0;
        if !at_least(half, min) {
            let message = format!(
                "This cross stitch fill's cells are {} by {} mm, so the stitch from a corner of a cross to its middle is {} mm long, shorter \
                 than the shortest stitch ({} mm), and needle points of the crosses are left out.",
                mm(size[0]),
                mm(size[1]),
                mm(half),
                mm(min)
            );
            diagnostics.push(Diagnostic::new(Code::FillCrossTooFine, message));
        }
        Some(Cross {
            size,
            on_canvas: self.canvas_grid_origin,
            offset: self.cross_offset_mm.map(Mm::get),
            coverage: self.fill_coverage,
            flipped,
            threads,
            longest,
            expand,
            min_stitch: min,
        })
    }

    /// The passes on each half of a diagonal: `cross_thread_count`, or the even count below it when it is
    /// odd, 2 at least, which `SC-W0320` says.
    fn threads(&self, diagnostics: &mut Vec<Diagnostic>) -> u32 {
        let set = self.cross_thread_count;
        let sewn = (set - set % 2).max(2);
        if sewn != set {
            let message =
                format!("`cross_thread_count` is {set}, and a full cross takes an even number of passes on each diagonal, so it sews {sewn}.");
            diagnostics.push(Diagnostic::new(Code::FillCrossThreadsOdd, message));
        }
        sewn
    }
}
