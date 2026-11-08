//! The Auto-Route Satin Columns filter: Object › Plug-ins › StitchCraft Auto-Route Satin Columns replaces the
//! selected satin columns, and the strokes among them, with one route through them (`REQ-VC-018`), as
//! Ink/Stitch's tool of that name replaces a selection (`docs/src/design/routing.md`, ADR-0017).
//!
//! **What it routes.** Each selected path routes by its own StitchCraft effects, the only ones VectorCraft
//! gives a filter: as a satin column when the first of them that is on is the satin effect, and as a stroke
//! when it is the stroke effect. The others stay as they are: a path with the fill or the Auto effect first,
//! and one with no StitchCraft effect of its own, which may sew by its group's. The engine routes them
//! ([`route`]) with the dialog's options, Ink/Stitch's ([`Tool::options`]), and leaves out what it can't route,
//! a column whose rails can't be made among them: a filter can't warn, and the path stays as it was. Paths
//! named `stitch:route-start` and `stitch:route-end` mark the route's start and end at the middles of the
//! boxes round their anchors (`REQ-VC-019`).
//!
//! **What it writes.** Each piece of the route is an object, named as Ink/Stitch names it: a column's piece
//! a compound path of its rails and rungs with the satin effect, and a stroke's stretch, or a run under a
//! column, a path with the stroke effect. Its record holds the piece's settings, trims among them, in the
//! place of the first StitchCraft effect of the path it comes from, whose other effects it keeps. It is
//! painted with that path's stroke, 1 CSS pixel wide, as Stroke to Satin paints a column. The first piece
//! takes the place of the topmost path routed, and the others go above it in sewing order. With Preserve
//! order, the pieces of each path take its place instead, in sewing order, which then keeps the paths'
//! order. The paths routed and the markers used are deleted, all in one undo step, unless Keep original
//! paths keeps them, and the route then goes above the topmost.
//!
//! **What it refuses** (`SC-E0820`): a selection with nothing to route, and one with 2 markers of a kind,
//! which it can't choose between. It then changes nothing. Before VectorCraft 0.15, which keeps no effect
//! record a filter gives, it declines with `SC-E0813`.
//!
//! **Its budget.** A route reads the rails of every column it routes, and makes them for a column drawn as
//! one path, as Stroke to Satin does. So it is a heavy filter, which VectorCraft 0.17 runs with
//! [`HEAVY_FUEL`] instructions. The engine's work is priced at [`PER_UNIT`] instructions a unit, Stroke to
//! Satin's price: a tenth of what the run may spend reads the paths, and 8 tenths route them. The filter
//! stops with `SC-E0004` before VectorCraft would stop it.
//!
//! **Either tool.** The Auto-Route Running Stitch filter ([`crate::route_runs`]) reads, routes and writes as
//! this one does, with the engine's other tool: `routed_by` and `dialog` serve both.

use std::collections::{BTreeMap, BTreeSet};

use serde_json::Value;
use stitchcraft_core::{Code, Diagnostic, ElementId, Fix, Meter, Mm, Point, Severity};
use stitchcraft_engine::design::{Cap, DesignSettings, Element, Hints, Join, Shape};
use stitchcraft_engine::generators::satin::SATIN_COLUMN;
use stitchcraft_engine::route::{Options, Piece, Route, Routed, Tool, route};
use stitchcraft_params::Family;
use stitchcraft_plan::{Rgb, Thread};
use stitchcraft_vectorcraft::filter::{self, Object, Out, Record, solid, stroke};
use stitchcraft_vectorcraft::{RouteMarker, effect_error, route_marker};

use crate::Module;
use crate::dialog::own;
use crate::effect::{RunError, too_old};
use crate::manifest::{Param, Schema};
use crate::preview::{HEAVY_FUEL, spendable, work};
use crate::stroke_to_satin::in_place_of_ours;
use crate::testsheet::record_params;

/// The filter's name: its module's, and its plug-in id's last part.
pub const ROUTE_SATINS: &str = "route-satins";

/// The instructions VectorCraft's host runs for each unit of work the engine charges reading and routing
/// the paths: Stroke to Satin's price, as most of the work is reading and making rails (`req_vc_018_*` in
/// `compat/vc-contract`, which fails when TS-17C's letters run out).
pub const PER_UNIT: u64 = 450;

/// The page that says how to route satin columns in VectorCraft, which the options' help links to.
const PAGE: &str = "user/how-to/route-satin-columns.html";

/// The filter's dialog: a toggle for each of Auto-Route Satin Columns' options, with Ink/Stitch's labels and
/// defaults.
pub(crate) fn options() -> Vec<(&'static str, Param)> {
    dialog(Tool::Satins, PAGE)
}

/// The dialog of a routing filter that routes with `tool`: a toggle for each of its options, with Ink/Stitch's
/// labels and defaults, its help linking to `page`.
pub(crate) fn dialog(tool: Tool, page: &'static str) -> Vec<(&'static str, Param)> {
    tool.options().iter().map(|option| (option.name, own(Schema::Bool { default: option.default }, option.label, option.help, page))).collect()
}

/// The filter's run on `input`, the selected objects, with `params`, its dialog's: the route, or why there
/// is none. Parameters that aren't JSON are [`RunError::BadInput`]; an option left out is at its default.
pub fn run(input: &[u8], params: &[u8]) -> Result<Vec<u8>, RunError> {
    routed_by(Tool::Satins, input, params)
}

/// The run of a routing filter that routes with `tool` on `input`, the selected objects, with `params`, its
/// dialog's, as [`run`] runs.
pub(crate) fn routed_by(tool: Tool, input: &[u8], params: &[u8]) -> Result<Vec<u8>, RunError> {
    let named: Value = serde_json::from_slice(params).map_err(|_| RunError::BadInput)?;
    let mut options = tool.defaults();
    for option in tool.options() {
        if let Some(on) = named.get(option.name).and_then(Value::as_bool) {
            option.set(&mut options, on);
        }
    }
    let spend = spendable(HEAVY_FUEL, input.len(), params.len());
    let routed = filter::read(input, params).and_then(|given| replaced(&given.objects, tool, options, spend));
    match routed {
        Ok(objects) => filter::output(&objects).map_err(|_| RunError::Encode),
        Err(why) => Ok(effect_error(&why)),
    }
}

/// The selected objects, read: the elements the paths make, each with the object it is, and the markers.
#[derive(Default)]
struct Selection {
    /// The elements of the paths that may route, in order.
    elements: Vec<Element>,
    /// The object each element is, by its place in the selection.
    objects: Vec<usize>,
    /// The places the markers mark: the start's, then the end's.
    ends: [Option<Point>; 2],
    /// The markers that mark them, by their places in the selection.
    markers: Vec<usize>,
}

/// What the filter gives back for `objects`, the selection, routed by `tool` with `options` within `spend`
/// instructions: the route in the place of the paths it replaces; or why nothing changes.
fn replaced(objects: &[Object], tool: Tool, options: Options, spend: u64) -> Result<Vec<Out>, Diagnostic> {
    let selection = selected(objects, &mut work(spend, 1, PER_UNIT).meter())?;
    let how = Route { tool, options, start: selection.ends[0], end: selection.ends[1] };
    let routed = route(&selection.elements, &how, &DesignSettings::default(), &work(spend, 8, PER_UNIT));
    match routed.diagnostics.iter().find(|said| said.code.severity() == Severity::Error) {
        Some(said) if said.code == Code::RouteNothing => Err(nothing(tool, objects.len())),
        Some(said) => Err(said.clone()),
        None => written(objects, &selection, &routed, options),
    }
}

/// The paths of `objects` that may route as elements, and the markers among them, read charging `meter`.
/// `SC-E0820` for 2 markers of a kind, and `SC-E0813` for a VectorCraft that gives no effect records.
fn selected(objects: &[Object], meter: &mut Meter) -> Result<Selection, Diagnostic> {
    let mut selection = Selection::default();
    for (n, object) in objects.iter().enumerate() {
        if let Some(end) = object.name.as_deref().and_then(route_marker) {
            let Some(at) = object.middle(meter).map_err(|_| exhausted())? else { continue };
            let slot = if end == RouteMarker::Start { &mut selection.ends[0] } else { &mut selection.ends[1] };
            if slot.replace(at).is_some() {
                return Err(twice(end));
            }
            selection.markers.push(n);
            continue;
        }
        let Some(effects) = object.effects() else {
            let message = "This VectorCraft keeps no effect record a filter gives the objects it makes, as VectorCraft does from 0.15, so nothing was \
                           routed.";
            return Err(too_old(message, "0.17"));
        };
        let Some(record) = effects.iter().find(|record| record.visible && record.is_stitchcraft()) else { continue };
        let Some(family @ (Family::Satin | Family::Stroke)) = record.family() else { continue };
        let Ok(path) = object.path(meter).map_err(|_| exhausted())? else { continue };
        let (width, join, cap) = object.stroke().map_or((Mm::ZERO, Join::UNSET, Cap::Butt), |stroke| (stroke.width, stroke.join, stroke.cap));
        let mut params = record.settings();
        params.set(SATIN_COLUMN, if family == Family::Satin { "true" } else { "false" });
        let id = ElementId::new(format!("vc:{}", object.id)).map_err(bug)?;
        let shape = Shape::Stroke { path, width, join, cap };
        let thread = Thread::new(Rgb::new(0, 0, 0));
        selection.elements.push(Element {
            id,
            name: object.name.clone(),
            shape,
            thread,
            params,
            automatic: false,
            hints: Hints::default(),
            clips: Vec::new(),
        });
        selection.objects.push(n);
    }
    Ok(selection)
}

/// The objects given back for `objects`, `selection` of them `routed` with `options`: each kept as it is but
/// those routed and the markers used, which the pieces replace unless the originals are kept, and the pieces
/// where the module's docs say.
fn written(objects: &[Object], selection: &Selection, routed: &Routed, options: Options) -> Result<Vec<Out>, Diagnostic> {
    let replaced: BTreeSet<usize> = routed.replaced.iter().filter_map(|element| selection.objects.get(*element).copied()).collect();
    let topmost = replaced.last().copied().unwrap_or_default();
    let mut placed: BTreeMap<usize, Vec<&Piece>> = BTreeMap::new();
    for piece in &routed.pieces {
        let at = selection.objects.get(piece.from).copied().filter(|_| options.preserve_order).unwrap_or(topmost);
        placed.entry(at).or_default().push(piece);
    }
    let declared = declared();
    let mut out = Vec::with_capacity(objects.len() + routed.pieces.len());
    for (n, object) in objects.iter().enumerate() {
        let gone = !options.keep_originals && (replaced.contains(&n) || selection.markers.contains(&n));
        if !gone {
            out.push(Out::given(object));
        }
        for (k, piece) in placed.get(&n).into_iter().flatten().enumerate() {
            let from = selection.objects.get(piece.from).and_then(|from| objects.get(*from)).unwrap_or(object);
            let made = if gone && k == 0 { Out::given(object) } else { Out::default().with_opacity_of(from) };
            out.push(drawn(made, piece, from, &declared)?);
        }
    }
    Ok(out)
}

/// The settings the satin and the stroke effects declare, by their keys, as their manifests declare them.
fn declared() -> Vec<(Family, Vec<(&'static str, Schema)>)> {
    [Family::Satin, Family::Stroke]
        .into_iter()
        .map(|family| (family, Module::Family(family).manifest().params.into_iter().map(|(key, param)| (key, param.schema)).collect()))
        .collect()
}

/// `made` drawn as `piece`, which comes from the path `from`: its path, its name, `from`'s stroke 1 CSS pixel
/// wide, or black without one, and `from`'s effects with the piece's family's record, by the settings
/// `declared`, in the place of their first StitchCraft effect.
fn drawn(made: Out, piece: &Piece, from: &Object, declared: &[(Family, Vec<(&'static str, Schema)>)]) -> Result<Out, Diagnostic> {
    let family = if piece.element.params.get(SATIN_COLUMN) == Some("true") { Family::Satin } else { Family::Stroke };
    let schemas = declared.iter().find(|(each, _)| *each == family).map_or(&[][..], |(_, schemas)| schemas.as_slice());
    let params = record_params(Module::Family(family), &piece.element.params, schemas).map_err(bug)?;
    let record = Record::ours(family.id(), &params).map_err(bug)?;
    let paint = match from.stroke() {
        Some(followed) => followed.as_wide_as(Mm::SVG_PX),
        None => solid(Rgb::new(0, 0, 0)).and_then(|black| stroke(&black, Mm::SVG_PX, Join::UNSET, Cap::Butt)),
    }
    .map_err(bug)?;
    let path = piece.element.shape.path();
    let made = made.with_name(piece.element.name.clone().unwrap_or_default()).with_path(path, path.subpaths.len() > 1).map_err(bug)?;
    Ok(made.with_fills(Vec::new()).with_strokes(vec![paint]).with_effects(in_place_of_ours(from.effects().unwrap_or_default(), record)))
}

/// `SC-E0820`: none of the `count` objects selected is one `tool` routes.
fn nothing(tool: Tool, count: usize) -> Diagnostic {
    let (what, effect, hint) = match tool {
        Tool::Runs => (
            "a running stitch",
            "the stroke effect",
            "Select the running stitches to route, and put the stroke effect on the paths themselves rather than on their group.",
        ),
        _ => (
            "a satin column",
            "the satin effect",
            "Select the satin columns to route, with the strokes among them, and put the satin effect on the paths themselves rather than \
             on their group.",
        ),
    };
    let message = match count {
        0 => "Nothing is selected to route.".to_string(),
        _ => format!(
            "None of the {count} objects selected is {what} to route: a path routes as one when its own first StitchCraft effect is {effect}."
        ),
    };
    Diagnostic::new(Code::VcNotRouted, message).with_fix(Fix::Hint(hint.into()))
}

/// `SC-E0820`: 2 objects selected mark the route's `end`.
fn twice(end: RouteMarker) -> Diagnostic {
    let (name, what) = if end == RouteMarker::Start { ("stitch:route-start", "start") } else { ("stitch:route-end", "end") };
    let message = format!("2 objects selected are named {name}, and a route has one {what}, so nothing was routed.");
    Diagnostic::new(Code::VcNotRouted, message).with_fix(Fix::Hint(format!("Select one {name} marker with the paths.")))
}

/// `SC-E0004`: reading the selection needs more work than the filter may do.
fn exhausted() -> Diagnostic {
    Diagnostic::new(Code::BudgetExhausted, "Reading the selected paths needs more work than VectorCraft lets a filter do.")
        .with_fix(Fix::Hint("Select fewer paths at a time.".into()))
}

/// `SC-E0009` for a piece the filter can't write, because of `why`, which the route never makes.
fn bug(why: impl std::fmt::Display) -> Diagnostic {
    Diagnostic::new(Code::InternalCheckFailed, format!("The route could not be written: {why}."))
}

#[cfg(test)]
mod tests;
