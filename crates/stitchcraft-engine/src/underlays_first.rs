//! Underlays first (M10.15, ADR-0018, `docs/src/design/engine-pipeline.md` › Underlays first): the runs of
//! elements that sew all their underlays before any of their top stitches, found before the elements are
//! generated, and the order of the layers they are sewn as.
//!
//! **Runs.** In the order stage 0 gives, a run is a stretch of consecutive elements that set
//! `underlays_first` and sew in one thread, compared by colour as assembly compares threads, so 2 names for
//! one colour are one thread. An element with a trim or a stop after it ends its run there. An element sewn
//! in several threads, or whose settings don't read, is in no run, and parts the elements on either side
//! of it. A run of one element sews as it does without the setting, and `SC-I0506` names it, as it names an
//! element sewn in several threads that sets it. An element whose settings don't read is skipped with
//! `SC-E0101`, which says enough.
//!
//! **Steps.** A run of 2 or more sews in 2 passes: the underlays of its elements that have one, in their
//! order, then the top of every element in the same order. Each element with an underlay is generated as 2
//! layers ([`Layer::Underlay`], [`Layer::Top`]), and every other element of the run whole, in the second pass
//! ([`Layer::Whole`]). Each step is generated as an element is, from the last needle point before it to near
//! what the next step's element offers (`crate::generate::approach`). An element outside every run is one
//! step, whole, in its place.
//!
//! **Why here.** The steps are known before any element is generated, from each element's settings alone
//! ([`Candidate`]), so a step's neighbours are known when it is generated, as ADR-0014 asks. Nothing of a
//! design without the setting changes: each of its elements is one step, whole, in its place.

use stitchcraft_core::{Code, Diagnostic, Fix};
use stitchcraft_plan::Rgb;

use crate::design::{DesignSettings, Element};
use crate::generate::colours;
use crate::generate::layered::{ends, has_underlay, reads, sets};
use crate::generators::Layer;

/// What the order of a design's elements reads of one before it is generated.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Candidate {
    /// Whether it sets `underlays_first`.
    pub(crate) sets: bool,
    /// Whether its settings read: one whose settings don't is skipped, and is in no run.
    pub(crate) reads: bool,
    /// The colours of the threads it sews in, in their order.
    pub(crate) threads: Vec<Rgb>,
    /// Whether a trim or a stop follows it, which ends its run.
    pub(crate) ends: bool,
    /// Whether it has an underlay, which its run sews in the first pass.
    pub(crate) underlay: bool,
}

impl Candidate {
    /// `element`, in a design with `settings`, as the order reads it. What only an element that sets
    /// `underlays_first` needs is read only for one.
    pub(crate) fn of(element: &Element, settings: &DesignSettings) -> Candidate {
        let sets = sets(element);
        Candidate {
            sets,
            reads: sets && reads(element, settings),
            threads: colours(element),
            ends: ends(element),
            underlay: sets && has_underlay(element, settings),
        }
    }

    /// The colour of its one thread, when it can join a run: it sets `underlays_first`, its settings read,
    /// and it sews in one thread.
    fn joins(&self) -> Option<Rgb> {
        match self.threads.as_slice() {
            [colour] if self.sets && self.reads => Some(*colour),
            _ => None,
        }
    }
}

/// One step of sewing a design: an element, whole or one of its 2 layers.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Step {
    /// The element's place among the design's elements.
    pub(crate) element: usize,
    /// What of it the step sews.
    pub(crate) layer: Layer,
}

/// Why an element that sets `underlays_first` joins no run.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Alone {
    /// No element next to it joins it.
    Lone,
    /// It is sewn in several threads.
    Threads,
}

/// The steps that sew the elements `candidates` describes, in order, and the elements that set
/// `underlays_first` but join no run, by their places, each with why.
pub(crate) fn steps(candidates: &[Candidate]) -> (Vec<Step>, Vec<(usize, Alone)>) {
    let (mut steps, mut alone) = (Vec::with_capacity(candidates.len()), Vec::new());
    let mut first = 0;
    while let Some(candidate) = candidates.get(first) {
        let Some(colour) = candidate.joins() else {
            if candidate.sets && candidate.reads {
                alone.push((first, Alone::Threads));
            }
            steps.push(Step { element: first, layer: Layer::Whole });
            first += 1;
            continue;
        };
        // The run goes on while the element before it has no trim or stop, and the next joins in its colour.
        let mut end = first + 1;
        while candidates.get(end - 1).is_some_and(|before| !before.ends) && candidates.get(end).and_then(Candidate::joins) == Some(colour) {
            end += 1;
        }
        let run = first..end;
        if run.len() == 1 {
            alone.push((first, Alone::Lone));
            steps.push(Step { element: first, layer: Layer::Whole });
        } else {
            let with_underlay = |k: &usize| candidates.get(*k).is_some_and(|c| c.underlay);
            steps.extend(run.clone().filter(with_underlay).map(|element| Step { element, layer: Layer::Underlay }));
            let second = |element: usize| Step { element, layer: if with_underlay(&element) { Layer::Top } else { Layer::Whole } };
            steps.extend(run.map(second));
        }
        first = end;
    }
    (steps, alone)
}

/// `SC-I0506` naming `element`, which sets `underlays_first` but joins no run, as `why` says.
pub(crate) fn alone(element: &Element, why: Alone) -> Diagnostic {
    let (message, fix) = match why {
        Alone::Lone => (
            "This element sets `underlays_first`, but neither element next to it in sewing order joins it: they don't set it, or sew in \
             another thread, or a trim or a stop parts them. So it sews its underlay just before its top stitches, as it does without \
             the setting.",
            Some("Set `underlays_first` on the elements that cross or overlap it, in its thread, or move the trim or the stop."),
        ),
        Alone::Threads => (
            "This element sets `underlays_first`, but it is sewn in several threads, so it joins no run, and sews its underlay just \
             before its top stitches, as it does without the setting.",
            None,
        ),
    };
    let said = Diagnostic::new(Code::UnderlaysFirstAlone, message).with_element(element.id.clone());
    match fix {
        Some(fix) => said.with_fix(Fix::Hint(fix.to_string())),
        None => said,
    }
}

#[cfg(test)]
mod tests;
