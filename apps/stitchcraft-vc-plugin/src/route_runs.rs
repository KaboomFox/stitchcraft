//! The Auto-Route Running Stitch filter: Object › Plug-ins › StitchCraft Auto-Route Running Stitch replaces the
//! selected running stitches with one route through them (`REQ-VC-020`), as Ink/Stitch's tool of that name
//! replaces a selection (`docs/src/design/routing.md`, ADR-0017).
//!
//! **What it routes.** Each selected path routes by its own StitchCraft effects, the only ones VectorCraft
//! gives a filter: as a stroke when the first of them that is on is the stroke effect, whatever its stroke
//! method. The others stay as they are: a path with the satin, the fill or the Auto effect first, as
//! Ink/Stitch's tool leaves a satin column and a fill, and one with no StitchCraft effect of its own, which
//! may sew by its group's. The engine routes the strokes ([`route`], [`Tool::Runs`]) with the dialog's
//! options, Ink/Stitch's, and leaves out what it can't route: a filter can't warn, and the path stays as it
//! was. Paths named `stitch:route-start` and `stitch:route-end` mark the route's start and end at the middles
//! of the boxes round their anchors, as for Auto-Route Satin Columns (`REQ-VC-019`).
//!
//! **What it writes.** Each element of the route is a path with the stroke effect, named as Ink/Stitch names
//! it, AutoRun or AutoRun Underpath and a number. Its record holds the element's settings, trims among them, in
//! the place of the first StitchCraft effect of the path it comes from, whose other effects it keeps, and it
//! is painted with that path's stroke, 1 CSS pixel wide. The elements go where the topmost path routed was, in
//! sewing order, or with Preserve order each where the path it comes from was, and the paths routed and the
//! markers used are deleted, all in one undo step. Ink/Stitch's tool has no Keep original paths.
//!
//! **What it refuses** (`SC-E0820`): a selection with no stroke to route, and one with 2 markers of a kind,
//! which it can't choose between. It then changes nothing. Before VectorCraft 0.15 it declines with
//! `SC-E0813`.
//!
//! **Its budget.** It reads the selection and writes the route as Auto-Route Satin Columns does
//! ([`crate::route_satins`], whose run it shares), and is a heavy filter as that one is. Cutting the lines
//! where they cross compares every 2 of them, which for many lines is most of its work.
//!
//! [`route`]: stitchcraft_engine::route::route

use stitchcraft_engine::route::Tool;

use crate::effect::RunError;
use crate::manifest::Param;
use crate::route_satins::{dialog, routed_by};

/// The filter's name: its module's, and its plug-in id's last part.
pub const ROUTE_RUNS: &str = "route-runs";

/// The page that says how to route running stitches in VectorCraft, which the options' help links to.
const PAGE: &str = "user/how-to/route-running-stitches.html";

/// The filter's dialog: a toggle for each of Auto-Route Running Stitch's options, with Ink/Stitch's labels and
/// defaults.
pub(crate) fn options() -> Vec<(&'static str, Param)> {
    dialog(Tool::Runs, PAGE)
}

/// The filter's run on `input`, the selected objects, with `params`, its dialog's: the route, or why there
/// is none. Parameters that aren't JSON are [`RunError::BadInput`]; an option left out is at its default.
pub fn run(input: &[u8], params: &[u8]) -> Result<Vec<u8>, RunError> {
    routed_by(Tool::Runs, input, params)
}

#[cfg(test)]
mod tests;
