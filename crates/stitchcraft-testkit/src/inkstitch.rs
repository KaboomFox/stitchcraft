//! A design's elements written as an SVG file that Ink/Stitch reads, with their parameters, for the Ink/Stitch
//! differential's cases of test sheets (`docs/src/design/conformance.md` › L3).
//!
//! **Why.** Automatic digitizing ends in ordinary elements whose settings are Ink/Stitch's parameters.
//! Written with them, the elements are a design Ink/Stitch sews with the same stitch types, so the stitches
//! StitchCraft makes of its own choices can be checked against another program's (`REQ-AUTO-021`), and both
//! programs sew the same file. The writer is `stitch expand`'s ([`stitchcraft_svg::written`], whose docs
//! describe the file), with Ink/Stitch's settings alone ([`Params::InkStitch`]): an element that sets one
//! of StitchCraft's own that a stitch type reads, such as `underlays_first` (`REQ-ASM-009`), is refused, and
//! the differential writes a sheet's file of the elements both programs sew alike, [`sewn_alike`]. It digitizes
//! a sheet's automatic shapes with `auto_short_stitches` `crowded` ([`digitized`]): by default an automatic
//! satin column opens its bends (`bend_open_percent`, `REQ-SAT-030`), which is StitchCraft's own, and with
//! `crowded` it sets Ink/Stitch's short-stitch settings instead (`REQ-AUTO-173`). An automatic satin column
//! also holds its pull compensation (`pull_compensation_max_percent`, `REQ-AUTO-176`), StitchCraft's own,
//! which the differential takes off the elements made, so that both programs sew them with Ink/Stitch's
//! compensation. The tests
//! read the file back with StitchCraft's SVG reader, which reads each element's shape, colour and parameters
//! as they were written (`REQ-PRM-003`, `REQ-SVG-009`).

use stitchcraft_core::{Budget, Diagnostic};
use stitchcraft_engine::auto::SHORT_STITCHES;
use stitchcraft_engine::design::{Design, DesignSettings, Element};
use stitchcraft_engine::generators::satin::PULL_COMPENSATION_MAX;
use stitchcraft_engine::testsheets::Cloned;
use stitchcraft_svg::written::{self, Options, Params};
pub use stitchcraft_svg::written::{MARGIN, data, sewn_alike};

/// The ordinary elements `design` sews as, as `stitchcraft_engine::auto::digitized` makes them with `budget`,
/// its automatic shapes digitized with `auto_short_stitches` `crowded`, so that Ink/Stitch sews their satin
/// columns alike, the setting itself left out of them; with what digitizing said. A design that can't be made again is digitized as it is.
pub fn digitized(design: &Design, budget: &Budget) -> (Vec<Element>, Vec<Diagnostic>) {
    let mut elements = design.elements().to_vec();
    for element in elements.iter_mut().filter(|element| element.automatic) {
        element.params.set(SHORT_STITCHES, "crowded");
    }
    let (made, said) = match Design::new(elements, design.settings) {
        Ok(crowded) => {
            let (made, said) = stitchcraft_engine::auto::digitized(&crowded, budget);
            (made.into_owned(), said)
        }
        Err(_) => {
            let (made, said) = stitchcraft_engine::auto::digitized(design, budget);
            (made.into_owned(), said)
        }
    };
    // The setting is automatic digitizing's: the elements it made sew as any other. Their columns' hold is
    // StitchCraft's own, which Ink/Stitch doesn't read.
    let made = made
        .into_iter()
        .map(|mut element| {
            element.params.remove(SHORT_STITCHES);
            element.params.remove(PULL_COMPENSATION_MAX);
            element
        })
        .collect();
    (made, said)
}

/// `elements`, in order, as an SVG file that Ink/Stitch reads, with the design's `settings`; or why they
/// can't be: no element has a point, or an element has a parameter of StitchCraft's own that a stitch type
/// reads.
pub fn svg(elements: &[Element], settings: &DesignSettings) -> Result<String, String> {
    svg_with(elements, settings, &[])
}

/// [`svg`], with the elements `clones` names drawn as clones of others; or why they can't be, as for `svg`,
/// or a clone of an element that isn't there.
pub fn svg_with(elements: &[Element], settings: &DesignSettings, clones: &[Cloned]) -> Result<String, String> {
    written::svg(elements, settings, &Options { params: Params::InkStitch, clones, styled: false })
}

/// [`svg`], each element painted by a `style` attribute, as Inkscape paints a path, rather than by
/// attributes of their own. Ink/Stitch's Auto-Route Running Stitch gives the elements it makes the style of
/// the stroke each comes from, and nothing else of its paint, so the strokes it routes are written so.
pub fn svg_styled(elements: &[Element], settings: &DesignSettings) -> Result<String, String> {
    written::svg(elements, settings, &Options { params: Params::InkStitch, clones: &[], styled: true })
}
