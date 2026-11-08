//! Routing (`docs/src/design/routing.md`, ADR-0017): satin columns, and the running stitches among them,
//! joined into one route with as few jumps as it can, as Ink/Stitch's Auto-Route Satin Columns joins them,
//! and running stitches alone, as its Auto-Route Running Stitch joins them (read at `d59c9ab`).
//!
//! **Why one function.** VectorCraft's filters StitchCraft Auto-Route Satin Columns and Auto-Route Running
//! Stitch, lettering (M14) and a ported glyph's columns (M15.8) all route, and a route must be the same
//! wherever it is made, so they all call [`route`]. It takes elements in their order, the options of
//! Ink/Stitch's dialog ([`Tool::options`], declared once here, from which the filters' dialogs are made) and
//! a start and an end, and gives back the elements that replace those it routes, in sewing order. Each is an
//! ordinary element, which every host sews as it sews any, and says which element it comes from, so that a
//! host can place and name it.
//!
//! **How** (the steps of `routing.md` › Auto-satin and Auto-run):
//!
//! 1. The elements are read. For satin columns, the columns and the strokes among them are cut into pieces
//!    1 mm long at most along their centre lines (`read`). For running stitches, each stroke's lines are cut
//!    where they cross or pass near each other into straight pieces (`lines`). The rest are left as they
//!    are, with `SC-I1002`.
//! 2. The pieces' ends are places of a graph (`graph`). Jumps join the groups of pieces lying apart, as
//!    short in all as they can be, or each element to the next with Preserve order.
//! 3. The route starts at the place nearest its start, or else the leftmost, and ends at the place nearest
//!    its end, or else the rightmost: of the first and the last element's with Preserve order.
//! 4. The way with the fewest links from the start to the end is sewn, and at each place along it, the rest
//!    that can be reached from there, depth first, out and back.
//! 5. A piece passed twice is sewn the first time as a running stitch under the stitches of the last time.
//!    Pieces in a row are one element, and the elements are named and trimmed: as satin columns' pieces are
//!    (`sew`), or as running stitches' are (`runs`).
//!
//! A single column with a start and no end is routed otherwise, as in Ink/Stitch: a running stitch along its
//! centre line from the start back to its start goes before it, and it stays as it is.
//!
//! **Pure and bounded.** The route depends on its input alone, kept in ordered collections, and ties go to
//! the earlier element and to the earlier piece along it (`graph`). Every loop charges the budget's
//! meter, the searches of the graph included, and a route that needs more work than the budget allows gives
//! back no piece, with `SC-E0004`, so that a host keeps the elements as they were.

mod graph;
mod lines;
mod read;
mod runs;
mod sew;
mod trim;

pub use lines::{MOST_LINES, NEAR};
pub use read::PIECE;
pub use runs::RUN_TRIMMED;
pub use trim::TRIMMED;

use stitchcraft_core::{Budget, Code, Diagnostic, Exhausted, Fix, Meter, Point};

use crate::design::{DesignSettings, Element};
use crate::normalize::along::Along;
use crate::route::graph::{Graph, Step};
use crate::route::read::{Routable, routables};

/// How to route: with which tool, as its options say, from where to where.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Route {
    /// Which of Ink/Stitch's routing tools to route as.
    pub tool: Tool,
    /// The tool's options, as its dialog sets them.
    pub options: Options,
    /// Where the route starts, near enough: Ink/Stitch's `autoroute_start` command, or a VectorCraft
    /// document's `stitch:route-start` marker. Without one, it starts at the leftmost end of a piece.
    pub start: Option<Point>,
    /// Where it ends, near enough: `autoroute_end`, or `stitch:route-end`. Without one, at the rightmost.
    pub end: Option<Point>,
}

/// A routing tool.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum Tool {
    /// Ink/Stitch's Auto-Route Satin Columns: satin columns, and the strokes among them, which show where the
    /// needle may travel.
    Satins,
    /// Ink/Stitch's Auto-Route Running Stitch: strokes, cut where they cross and where they pass near each
    /// other.
    Runs,
}

impl Tool {
    /// Its name, as Ink/Stitch's menus give it.
    pub const fn name(self) -> &'static str {
        match self {
            Tool::Satins => "Auto-Route Satin Columns",
            Tool::Runs => "Auto-Route Running Stitch",
        }
    }

    /// Its options, in the order its dialog lists them.
    pub const fn options(self) -> &'static [RouteOption] {
        match self {
            Tool::Satins => &SATIN_OPTIONS,
            Tool::Runs => &RUN_OPTIONS,
        }
    }

    /// Its options, each at its default, and every option it doesn't have off.
    pub fn defaults(self) -> Options {
        let mut options = Options { trim: false, preserve_order: false, keep_originals: false, break_up: false };
        for option in self.options() {
            option.set(&mut options, option.default);
        }
        options
    }

    /// Its option called `name`, if it has one.
    pub fn option(self, name: &str) -> Option<&'static RouteOption> {
        self.options().iter().find(|option| option.name == name)
    }
}

/// A routing tool's options, as Ink/Stitch's dialog sets them ([`Tool::options`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Options {
    /// Trims the thread after the piece before each long jump: for satin columns, a jump longer than
    /// [`TRIMMED`] outside the elements it joins, and after the last piece; for running stitches, a jump
    /// longer than [`RUN_TRIMMED`].
    pub trim: bool,
    /// Keeps the elements' order: each is joined to the next by a jump the route takes only forwards.
    pub preserve_order: bool,
    /// Keeps the elements routed beside the route: satin columns. The function gives the same pieces either
    /// way, and a host keeps or deletes the elements as this says.
    pub keep_originals: bool,
    /// Cuts the strokes where they cross themselves and each other, and where 2 pass within [`NEAR`] of
    /// each other: running stitches, Ink/Stitch's Add nodes at intersections.
    pub break_up: bool,
}

/// One of a routing tool's options: its name, which Ink/Stitch's tool takes on its command line, what a
/// dialog calls it and says of it, and its default, all Ink/Stitch's.
#[derive(Clone, Copy, Debug)]
pub struct RouteOption {
    /// Its name, Ink/Stitch's.
    pub name: &'static str,
    /// What a dialog calls it, Ink/Stitch's label.
    pub label: &'static str,
    /// What it does, for people.
    pub help: &'static str,
    /// Its value unless the user sets it, as Ink/Stitch's dialog opens.
    pub default: bool,
    get: fn(&Options) -> bool,
    set: fn(&mut Options, bool),
}

impl RouteOption {
    /// Its value in `options`.
    pub fn get(&self, options: &Options) -> bool {
        (self.get)(options)
    }

    /// Sets it in `options` to `value`.
    pub fn set(&self, options: &mut Options, value: bool) {
        (self.set)(options, value);
    }
}

/// The options of Auto-Route Satin Columns, in the order its dialog lists them.
pub const SATIN_OPTIONS: [RouteOption; 3] = [
    RouteOption {
        name: "trim",
        label: "Trim jump stitches",
        help: "Trims the thread after a piece that a jump follows, when the jump runs more than 1 mm outside the shapes it joins, and \
               after the last piece. The running stitches right after a trim are left out, as the needle no longer needs them.",
        default: true,
        get: |options| options.trim,
        set: |options, value| options.trim = value,
    },
    RouteOption {
        name: "preserve_order",
        label: "Preserve order of satin columns",
        help: "Sews the paths in the order they are drawn, each joined to the next by the shortest jump between them, and only cuts them \
               and adds running stitches where the route needs them.",
        default: false,
        get: |options| options.preserve_order,
        set: |options, value| options.preserve_order = value,
    },
    RouteOption {
        name: "keep_originals",
        label: "Keep original paths",
        help: "Keeps the paths routed, and the markers of the route's start and end, beside the route.",
        default: false,
        get: |options| options.keep_originals,
        set: |options, value| options.keep_originals = value,
    },
];

/// The options of Auto-Route Running Stitch, in the order its dialog lists them.
pub const RUN_OPTIONS: [RouteOption; 3] = [
    RouteOption {
        name: "break_up",
        label: "Add nodes at intersections",
        help: "Cuts the lines where they cross or touch, and where 2 pass within 50 CSS pixels (13.2 mm) of each other, at their nearest \
               points, so that the route can turn there. Turn it off for lines that already have nodes where they meet.",
        default: true,
        get: |options| options.break_up,
        set: |options, value| options.break_up = value,
    },
    RouteOption {
        name: "preserve_order",
        label: "Preserve order of running stitches",
        help: "Sews the paths in the order they are drawn, each joined to the next by the shortest jump between them, and only adds \
               running stitches under them where the route needs them.",
        default: false,
        get: |options| options.preserve_order,
        set: |options, value| options.preserve_order = value,
    },
    RouteOption {
        name: "trim",
        label: "Trim jump stitches",
        help: "Trims the thread after a stretch that a jump longer than 0.75 mm follows. The running stitches under the others right \
               after a trim are left out, as the needle no longer needs them.",
        default: false,
        get: |options| options.trim,
        set: |options, value| options.trim = value,
    },
];

/// A route: the elements that replace those it routes, in sewing order, and what is said of them.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Routed {
    /// The elements that replace those routed, in sewing order; none when the route fails.
    pub pieces: Vec<Piece>,
    /// The elements routed, by their places among those given: the pieces replace them, and the others
    /// stay as they are.
    pub replaced: Vec<usize>,
    /// What is said of the route: why it failed, what it left as it was, and pieces that may sew otherwise.
    pub diagnostics: Vec<Diagnostic>,
}

/// An element of a route.
#[derive(Clone, Debug, PartialEq)]
pub struct Piece {
    /// The element: a column's piece, a stroke's stretch, or a run under the stitches sewn after it.
    pub element: Element,
    /// The element it was made from, by its place among those given: for a run under the others along
    /// several strokes, the last.
    pub from: usize,
    /// Whether it is a run that the stitches sewn after it cover: a column's satin, or a stroke's stretch.
    pub under: bool,
}

/// `elements`, in their order, routed as `how` says, in a design with `settings`, within `budget`'s work.
pub fn route(elements: &[Element], how: &Route, settings: &DesignSettings, budget: &Budget) -> Routed {
    let mut meter = budget.meter();
    let routed = match how.tool {
        Tool::Satins => satins(elements, how, settings, &mut meter),
        Tool::Runs => runs(elements, how, settings, &mut meter),
    };
    routed.unwrap_or_else(|_| Routed { diagnostics: vec![exhausted(budget)], ..Routed::default() })
}

/// [`route`] with Auto-Route Satin Columns, charging `meter`.
fn satins(elements: &[Element], how: &Route, settings: &DesignSettings, meter: &mut Meter) -> Result<Routed, Exhausted> {
    let mut said = Vec::new();
    let routables = routables(elements, settings, &mut said, meter)?;
    if !routables.iter().any(Routable::is_column) {
        said.push(nothing(Tool::Satins, elements.len()));
        return Ok(Routed { diagnostics: said, ..Routed::default() });
    }
    let replaced = routables.iter().map(|routable| routable.element).collect();
    if let ([only], Some(start), None) = (routables.as_slice(), how.start, how.end) {
        let pieces = single(only, elements, start, meter)?;
        return Ok(Routed { pieces, replaced, diagnostics: said });
    }
    let order = how.options.preserve_order;
    let mut graph = Graph::new(order);
    for (r, routable) in routables.iter().enumerate() {
        for (index, ends) in routable.ends(meter)?.into_iter().enumerate() {
            graph.piece(r, index, ends);
        }
    }
    if order {
        graph.join_in_order(&(0..routables.len()).collect::<Vec<_>>(), meter)?;
    } else {
        graph.join_apart(meter)?;
    }
    let Some(steps) = way(&graph, how, routables.len(), meter)? else { return Ok(lost(said)) };
    let pieces = sew::pieces((&steps, &graph), (&routables, elements), &how.options, &mut said, meter)?;
    Ok(Routed { pieces, replaced, diagnostics: said })
}

/// [`route`] with Auto-Route Running Stitch, charging `meter`.
fn runs(elements: &[Element], how: &Route, settings: &DesignSettings, meter: &mut Meter) -> Result<Routed, Exhausted> {
    let mut said = Vec::new();
    let strokes = lines::strokes(elements, settings, how.options.break_up, &mut said, meter)?;
    if strokes.is_empty() {
        said.push(nothing(Tool::Runs, elements.len()));
        return Ok(Routed { diagnostics: said, ..Routed::default() });
    }
    let replaced = strokes.iter().map(|stroke| stroke.element).collect();
    let order = how.options.preserve_order;
    let mut graph = Graph::new(order);
    for (r, stroke) in strokes.iter().enumerate() {
        for (index, &ends) in stroke.pieces.iter().enumerate() {
            meter.charge(1)?;
            graph.piece(r, index, ends);
        }
    }
    if order {
        graph.join_in_order(&(0..strokes.len()).collect::<Vec<_>>(), meter)?;
        for r in 0..strokes.len() {
            graph.join_parts(r, meter)?;
        }
    } else {
        graph.join_apart(meter)?;
    }
    let Some(steps) = way(&graph, how, strokes.len(), meter)? else { return Ok(lost(said)) };
    let pieces = runs::pieces((&steps, &graph), (&strokes, elements), &how.options, meter)?;
    Ok(Routed { pieces, replaced, diagnostics: said })
}

/// The steps of the route through `graph`, whose pieces are of `count` elements, as `how` says: from the place
/// nearest its start to the place nearest its end, along the way with the fewest links, and at each place of
/// that way the links still to sew, depth first. `None` when there is no way from the start to the end.
fn way(graph: &Graph, how: &Route, count: usize, meter: &mut Meter) -> Result<Option<Vec<Step>>, Exhausted> {
    let ends = if how.options.preserve_order {
        [Some(graph.places_on(0, meter)?), Some(graph.places_on(count.saturating_sub(1), meter)?)]
    } else {
        [None, None]
    };
    let start = graph.end(how.start, true, ends[0].as_deref());
    let end = graph.end(how.end, false, ends[1].as_deref());
    let (Some(start), Some(end)) = (start, end) else { return Ok(None) };
    let Some((places, links)) = graph.shortest(start, end, meter)? else { return Ok(None) };
    Ok(Some(graph.walk(&places, &links, meter)?))
}

/// The route of a single column, `only` of `elements`, from `start` with no end, as Ink/Stitch routes one: a
/// running stitch along its centre line from the point nearest the start back to the column's start, then
/// the column as it is. A start nearest the column's start leaves only the column.
fn single(only: &Routable, elements: &[Element], start: Point, meter: &mut Meter) -> Result<Vec<Piece>, Exhausted> {
    let Some(element) = elements.get(only.element) else { return Ok(Vec::new()) };
    let along = Along::new(&only.line, meter)?;
    let at = along.project(start, meter)?;
    let mut back = along.part(0.0, at);
    back.reverse();
    let run = if at > 0.0 { sew::run(only, elements, &back, true, 1) } else { None };
    Ok(run.into_iter().chain([Piece { element: element.clone(), from: only.element, under: false }]).collect())
}

/// `SC-E0004`: routing needed more work than `budget` allows.
fn exhausted(budget: &Budget) -> Diagnostic {
    let message = format!("Routing these elements needed more than the work budget of {} units, so they are left as they were.", budget.max_work);
    Diagnostic::new(Code::BudgetExhausted, message).with_fix(Fix::Hint("Route fewer elements at a time, or simplify them.".to_string()))
}

/// `SC-E1001`: none of the `count` elements given is one `tool` can take.
fn nothing(tool: Tool, count: usize) -> Diagnostic {
    let (name, what, hint) = match tool {
        Tool::Satins => (tool.name(), "satin columns", "Give the satin columns to route, with the running stitches among them."),
        Tool::Runs => (tool.name(), "running stitches", "Give the running stitches to route."),
    };
    let message = match count {
        0 => format!("There is nothing to route: {name} routes {what}, and was given no element."),
        1 => format!("There is nothing to route: {name} routes {what}, and the element given is none."),
        _ => format!("There is nothing to route: {name} routes {what}, and none of the {count} elements given is one."),
    };
    Diagnostic::new(Code::RouteNothing, message).with_fix(Fix::Hint(hint.to_string()))
}

/// No route, with what was `said` and `SC-E0009`: the graph has no way from the start to the end, which its
/// jumps always give.
fn lost(mut said: Vec<Diagnostic>) -> Routed {
    said.push(Diagnostic::new(Code::InternalCheckFailed, "The route found no way from its start to its end, though its jumps join every piece."));
    Routed { diagnostics: said, ..Routed::default() }
}

#[cfg(test)]
mod tests;
