//! Routing (`docs/src/design/routing.md`, ADR-0017): satin columns, and the running stitches among them,
//! joined into one route with as few jumps as it can, as Ink/Stitch's Auto-Route Satin Columns joins them
//! (read at `d59c9ab`).
//!
//! **Why one function.** VectorCraft's filter StitchCraft Auto-Route Satin Columns, lettering (M14) and a
//! ported glyph's columns (M15.8) all route, and a route must be the same wherever it is made, so they all
//! call [`route`]. It takes elements in their order, the options of Ink/Stitch's dialog ([`OPTIONS`],
//! declared once here, from which the filter's dialog is made) and a start and an end, and gives back the
//! elements that replace those it routes, in sewing order. Each is an ordinary element, which every host
//! sews as it sews any, and says which element it comes from, so that a host can place and name it.
//!
//! **How** (the steps of `routing.md` › Auto-satin):
//!
//! 1. The satin columns, and the strokes among them, are read and cut into pieces 1 mm long at most along
//!    their centre lines (`read`). The rest are left as they are, with `SC-I1002`.
//! 2. The pieces' ends are places of a graph (`graph`). Jumps join the groups of pieces lying apart, as
//!    short in all as they can be, or each element to the next with Preserve order.
//! 3. The route starts at the place nearest its start, or else the leftmost, and ends at the place nearest
//!    its end, or else the rightmost: of the first and the last element's with Preserve order.
//! 4. The way with the fewest links from the start to the end is sewn, and at each place along it, the rest
//!    that can be reached from there, depth first, out and back.
//! 5. A column's piece passed twice is sewn under its satin as a running stitch the first time. Pieces in a
//!    row are one element, and the pieces are cut from their columns, named, and trimmed (`sew`).
//!
//! A single column with a start and no end is routed otherwise, as in Ink/Stitch: a running stitch along its
//! centre line from the start back to its start goes before it, and it stays as it is.
//!
//! **Pure and bounded.** The route depends on its input alone, kept in ordered collections, and ties go to
//! the earlier element and to the earlier piece along it (`graph`). Every loop charges the budget's
//! meter, the searches of the graph included, and a route that needs more work than the budget allows gives
//! back no piece, with `SC-E0004`, so that a host keeps the elements as they were.

mod graph;
mod read;
mod sew;
pub(crate) mod trim;

pub use read::PIECE;
pub use trim::TRIMMED;

use stitchcraft_core::{Budget, Code, Diagnostic, Exhausted, Fix, Meter, Point};

use crate::design::{DesignSettings, Element};
use crate::normalize::along::Along;
use crate::route::graph::Graph;
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
}

/// A routing tool's options, as Ink/Stitch's dialog sets them ([`OPTIONS`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Options {
    /// Trims the thread after the piece before each jump longer than [`TRIMMED`] outside the elements it
    /// joins, and after the last.
    pub trim: bool,
    /// Keeps the elements' order: each is joined to the next by a jump the route takes only forwards.
    pub preserve_order: bool,
    /// Keeps the elements routed beside the route. The function gives the same pieces either way, and a
    /// host keeps or deletes the elements as this says.
    pub keep_originals: bool,
}

impl Default for Options {
    /// Each option at its default ([`OPTIONS`]).
    fn default() -> Options {
        let mut options = Options { trim: false, preserve_order: false, keep_originals: false };
        for option in &OPTIONS {
            option.set(&mut options, option.default);
        }
        options
    }
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
    /// The option called `name`, if there is one.
    pub fn named(name: &str) -> Option<&'static RouteOption> {
        OPTIONS.iter().find(|option| option.name == name)
    }

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
pub const OPTIONS: [RouteOption; 3] = [
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
    /// The element: a column's piece, a stroke's stretch, or a run under the satin sewn after it.
    pub element: Element,
    /// The element it was made from, by its place among those given.
    pub from: usize,
    /// Whether it is a run that a column's satin, sewn after it, covers.
    pub under: bool,
}

/// `elements`, in their order, routed as `how` says, in a design with `settings`, within `budget`'s work.
pub fn route(elements: &[Element], how: &Route, settings: &DesignSettings, budget: &Budget) -> Routed {
    let mut meter = budget.meter();
    routed(elements, how, settings, &mut meter).unwrap_or_else(|_| Routed { diagnostics: vec![exhausted(budget)], ..Routed::default() })
}

/// [`route`], charging `meter`.
fn routed(elements: &[Element], how: &Route, settings: &DesignSettings, meter: &mut Meter) -> Result<Routed, Exhausted> {
    let mut said = Vec::new();
    let routables = routables(elements, settings, &mut said, meter)?;
    if !routables.iter().any(Routable::is_column) {
        said.push(nothing(elements.len()));
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
    let ends = if order { [Some(graph.places_on(0, meter)?), Some(graph.places_on(routables.len() - 1, meter)?)] } else { [None, None] };
    let start = graph.end(how.start, true, ends[0].as_deref());
    let end = graph.end(how.end, false, ends[1].as_deref());
    let (Some(start), Some(end)) = (start, end) else { return Ok(lost(said)) };
    let Some((places, links)) = graph.shortest(start, end, meter)? else { return Ok(lost(said)) };
    let steps = graph.walk(&places, &links, meter)?;
    let pieces = sew::pieces((&steps, &graph), (&routables, elements), &how.options, &mut said, meter)?;
    Ok(Routed { pieces, replaced, diagnostics: said })
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

/// `SC-E1001`: none of the `count` elements given is a satin column the route can take.
fn nothing(count: usize) -> Diagnostic {
    let message = match count {
        0 => "There is nothing to route: Auto-Route Satin Columns routes satin columns, and was given no element.".to_string(),
        1 => "There is nothing to route: Auto-Route Satin Columns routes satin columns, and the element given is none.".to_string(),
        _ => format!("There is nothing to route: Auto-Route Satin Columns routes satin columns, and none of the {count} elements given is one."),
    };
    Diagnostic::new(Code::RouteNothing, message)
        .with_fix(Fix::Hint("Give the satin columns to route, with the running stitches among them.".to_string()))
}

/// No route, with what was `said` and `SC-E0009`: the graph has no way from the start to the end, which its
/// jumps always give.
fn lost(mut said: Vec<Diagnostic>) -> Routed {
    said.push(Diagnostic::new(Code::InternalCheckFailed, "The route found no way from its start to its end, though its jumps join every piece."));
    Routed { diagnostics: said, ..Routed::default() }
}

#[cfg(test)]
mod tests;
