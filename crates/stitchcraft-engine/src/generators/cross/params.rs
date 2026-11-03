//! Cross stitch's settings, under Ink/Stitch's names and with the defaults it sews for a file that sets
//! none, which its files and its cross-stitch fonts count on (`docs/src/design/algorithms/cross-stitch.md`).
//!
//! **Why here.** A setting is declared once, beside the generator that reads it, and the registry, the
//! reference pages, the JSON Schema, VectorCraft's dialogs and the compatibility contract are made from the
//! declaration. Reading the settings into what the generator needs ([`CrossParams::cross`]) is here too, so
//! that what a setting means, and what is said when its value can't be sewn as set, lies beside it.
//!
//! **What changes as it is read.** A full cross takes an even number of passes on each half of a bar, so
//! an odd `cross_thread_count` sews the even count below it, 2 at least (`SC-W0320`). A half cross sews each
//! stitch an odd number of times, so an even count sews the odd count above it (`SC-W0322`). A longest
//! stitch shorter than twice the shortest stitch is raised to it (`SC-W0402`), as a fill's is. A grid whose
//! crosses' shortest stitches are shorter than the shortest stitch is said (`SC-W0321`).

use stitchcraft_core::math::hypot;
use stitchcraft_core::units::at_least;
use stitchcraft_core::{Code, Diagnostic, Mm};
use stitchcraft_params::{ChoiceOption, StitchType, params};

use super::{Cross, Method};
use crate::generators::mm;
use crate::generators::running::raised;

/// The stitch types cross stitch's own settings apply to.
const CROSS: &[StitchType] = &[StitchType::CrossStitch];

/// The cross stitch methods `cross_stitch_method` reads, in Ink/Stitch's order, which its files count on:
/// Ink/Stitch gives the parameter's default as the first one's place in this list. Each method comes with
/// its flipped variant after it, the methods in the order of [`ORDER`].
pub const METHODS: &[ChoiceOption] = &[
    ChoiceOption::new("simple_cross", "Simple cross"),
    ChoiceOption::new("simple_cross_flipped", "Simple cross, flipped"),
    ChoiceOption::new("half_cross", "Half cross"),
    ChoiceOption::new("half_cross_flipped", "Half cross, flipped"),
    ChoiceOption::new("upright_cross", "Upright cross"),
    ChoiceOption::new("upright_cross_flipped", "Upright cross, flipped"),
    ChoiceOption::new("dense_upright_cross", "Dense upright cross"),
    ChoiceOption::new("dense_upright_cross_flipped", "Dense upright cross, flipped"),
    ChoiceOption::new("double_cross", "Double cross"),
    ChoiceOption::new("double_cross_flipped", "Double cross, flipped"),
    ChoiceOption::new("smyrna_cross", "Smyrna cross"),
    ChoiceOption::new("smyrna_cross_flipped", "Smyrna cross, flipped"),
];

/// What each pair of [`METHODS`] sews, plain and then flipped.
const ORDER: [Method; 6] = [Method::Simple, Method::Half, Method::Upright, Method::DenseUpright, Method::Double, Method::Smyrna];

params! {
    /// Cross stitch: a cross in each cell of a grid laid over the fill, where the fill covers enough of the
    /// cell, as a hand embroiderer counts crosses on even-weave fabric. Each cross sews its bars one over
    /// another, and the crosses that meet sew in one run. Half crosses sew one diagonal in each cell, in rows.
    pub struct CrossParams for CROSS;

    "Crosses" {
        /// The kind of cross. A simple cross sews its `\` diagonal and then its `/` diagonal over it, as the
        /// drawing shows it. An upright cross sews a `+`, its bar across and then its bar down, and a dense
        /// upright cross adds a second grid of them half a cell over. A double cross sews an upright cross
        /// between its 2 diagonals, and a Smyrna cross one over them. A half cross sews the `\` diagonal
        /// alone, joined into rows from cell to cell. Flipped, each cross is turned a quarter turn on its
        /// cell, so that a simple cross's `/` diagonal goes under, and a half cross sews the `/` diagonal.
        cross_stitch_method: Choice = "simple_cross", label "Cross", choices METHODS;

        /// How many times the thread passes along each half of each bar of a cross. A full cross takes an
        /// even number, since the needle comes back to where it started: an odd number sews the even number
        /// below it, and 1 sews 2. A half cross sews each stitch an odd number of times: an even number sews
        /// the odd number above it, 4 sewing 5. More threads make bolder crosses, and pack the holes at the
        /// cells' corners.
        cross_thread_count: Count = "4", label "Threads", range (1, 20);

        /// The longest stitch a cross sews. A diagonal longer than this, on a large grid, is sewn in equal
        /// stitches, each no longer. Half crosses travel between their rows in stitches this long.
        max_cross_stitch_length_mm: Length = "11", label "Longest cross stitch", range (0.1, 100.0);
    }

    "Grid" {
        /// How wide and how tall the grid's cells are, each the size of a cross. One length gives square
        /// cells, and 2 separated by a space give the width, then the height.
        pattern_size_mm: LengthPair = "3", label "Cell size", range (0.1, 100.0);

        /// Lay the grid from the canvas's origin, so that cross stitch fills side by side share one grid and
        /// their crosses line up. Moved, the fill may then sew other crosses. Off, the grid starts at the top
        /// left corner of each part of the fill, which then sews the same wherever it lies.
        canvas_grid_origin: Toggle = "true", label "Grid on the canvas";

        /// How far the grid is moved: left by the first length, and down by the second. One length moves it
        /// as far each way. Moved by a whole cell, the grid lies where it was.
        cross_offset_mm: LengthPair = "0", label "Grid offset", range (0.0, 100.0);

        /// How far the grid is turned, clockwise for a positive angle. A letter turned along a path keeps its
        /// crosses square to it when its grid turns with it, from the letter's own corner
        /// (`canvas_grid_origin` off). On the canvas's grid, the grid turns about the point the offset names.
        cross_rotation: Angle = "0", label "Grid rotation";

        /// How much of a cell, in percent, the fill must cover for the cell to sew a cross. Lower, crosses
        /// are sewn further past the fill's edges. At 100, only cells wholly inside it sew.
        fill_coverage: Percent = "50", label "Coverage", range (1.0, 100.0);
    }
}

impl CrossParams {
    /// The cross stitch these settings sew, grown by `expand` mm, with `min_stitch` the element's shortest
    /// stitch: what is said of them goes to `diagnostics`.
    pub fn cross(&self, expand: f64, min_stitch: Mm, diagnostics: &mut Vec<Diagnostic>) -> Cross {
        let (method, flipped) = self.method();
        let threads = if method == Method::Half { self.odd_threads(diagnostics) } else { self.even_threads(diagnostics) };
        let min = min_stitch.get();
        let longest = raised("max_cross_stitch_length_mm", self.max_cross_stitch_length_mm, min, diagnostics);
        let size = self.pattern_size_mm.map(Mm::get);
        diagnostics.extend(too_fine(method, size, min));
        Cross {
            size,
            on_canvas: self.canvas_grid_origin,
            offset: self.cross_offset_mm.map(Mm::get),
            rotation: self.cross_rotation,
            coverage: self.fill_coverage,
            method,
            flipped,
            threads,
            longest,
            expand,
            min_stitch: min,
        }
    }

    /// The method `cross_stitch_method` names, and whether it is the flipped variant: by its place in
    /// [`METHODS`], each method followed by its flipped variant. The registry reads only those values, and
    /// one it didn't would sew simple crosses.
    fn method(&self) -> (Method, bool) {
        let at = METHODS.iter().position(|option| option.id == self.cross_stitch_method).unwrap_or(0);
        (ORDER.get(at / 2).copied().unwrap_or(Method::Simple), at % 2 == 1)
    }

    /// The passes on each half of a full cross's bar: `cross_thread_count`, or the even count below it when
    /// it is odd, 2 at least, which `SC-W0320` says.
    fn even_threads(&self, diagnostics: &mut Vec<Diagnostic>) -> u32 {
        let set = self.cross_thread_count;
        let sewn = (set - set % 2).max(2);
        if sewn != set {
            let message =
                format!("`cross_thread_count` is {set}, and a full cross takes an even number of passes on each diagonal, so it sews {sewn}.");
            diagnostics.push(Diagnostic::new(Code::FillCrossThreadsOdd, message));
        }
        sewn
    }

    /// The passes along each stitch of a half cross: `cross_thread_count`, or the odd count above it when it
    /// is even, which `SC-W0322` says.
    fn odd_threads(&self, diagnostics: &mut Vec<Diagnostic>) -> u32 {
        let set = self.cross_thread_count;
        let sewn = set | 1;
        if sewn != set {
            let message = format!("`cross_thread_count` is {set}, and a half cross sews each stitch an odd number of times, so it sews {sewn}.");
            diagnostics.push(Diagnostic::new(Code::FillCrossThreadsEven, message));
        }
        sewn
    }
}

/// `SC-W0321` when the shortest stitch `method` sews on cells `size` mm wide and tall is shorter than
/// `min` mm: from a corner of a simple cross to its middle, from the middle of a side of a cross with
/// upright bars to its middle, or along a side of a cell, as a half cross's travel runs.
fn too_fine(method: Method, [w, h]: [f64; 2], min: f64) -> Option<Diagnostic> {
    let (what, shortest) = match method {
        Method::Simple => ("the stitch from a corner of a cross to its middle", hypot(w, h) / 2.0),
        Method::Half => ("a stitch along a side of a cell", w.min(h)),
        Method::Upright | Method::DenseUpright | Method::Double | Method::Smyrna => {
            ("the stitch from the middle of a side of a cross to its middle", w.min(h) / 2.0)
        }
    };
    if at_least(shortest, min) {
        return None;
    }
    let message = format!(
        "This cross stitch fill's cells are {} by {} mm, so {what} is {} mm long, shorter than the shortest stitch ({} mm), and needle \
         points of the crosses are left out.",
        mm(w),
        mm(h),
        mm(shortest),
        mm(min)
    );
    Some(Diagnostic::new(Code::FillCrossTooFine, message))
}
