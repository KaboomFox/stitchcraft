//! The notice that the style changes how a design sews (`REQ-AUTO-151`, `SC-I0923`). Design:
//! `docs/src/design/image-to-embroidery.md` › Choosing a style.
//!
//! No measure of a picture chooses between the logo and the photo style reliably (`REQ-AUTO-150`), so the
//! person chooses. What planning can tell them is whether the choice mattered for this design, and which
//! way the other style goes. It goes by what the rules already decided, and changes nothing that sews:
//!
//! - **Lines.** An area with `auto_fill_uneven` on merges its ragged thin lines into their neighbours
//!   (`REQ-AUTO-090`), and the other style sews each as a line. An area with it off keeps them, and the lines
//!   that the other style would merge are found as merging finds them (`auto::neighbours::lines`), without
//!   merging them. That costs each such area the work of measuring its thin parts once more.
//! - **Parts.** Each part of an automatic shape is decided again from the measures it was decided by, under
//!   the bounds that the other style's settings give: a part filled here that the other style sews as satin
//!   columns, or the reverse, is counted. That costs a unit of work for each part, and no geometry.
//!
//! The other style is the preset whose `auto_fill_uneven` differs from the shape's, with its values set over
//! the shape's settings as `stitch plan --style` sets them. A shape whose settings have no other style, or
//! don't read, counts nothing. The count of the parts doesn't plan the design again: the way a fill splits
//! off its narrow pieces (`auto_min_fill_width_mm`, which `auto_prefer` leans too) is not counted, nor the
//! lines a merge keeps for the picture's likeness. A design whose counts are all 0 is said nothing.

use std::collections::BTreeMap;

use stitchcraft_core::{Budget, Code, Diagnostic, ElementId, Exhausted, Fix};

use super::params::{AutoParams, FILL_UNEVEN};
use super::rules::{Bounds, Fired, PRESETS, Preset, RuleId, choose, fills};
use crate::design::Element;

/// The style that sews otherwise than a shape whose `auto_fill_uneven` is `fill_uneven`: the preset that
/// sets it the other way.
pub(super) fn other(fill_uneven: bool) -> Option<&'static Preset> {
    let sets = |preset: &Preset, value: &str| preset.values.iter().any(|(key, set)| *key == FILL_UNEVEN && *set == value);
    PRESETS.iter().find(|preset| sets(preset, if fill_uneven { "false" } else { "true" }))
}

/// For each of `elements`, by its place, the settings of the rules that the other style gives it: its own
/// settings, `rules`, with the other style's values set over them. `None` for an element without settings
/// of the rules, and for one without another style.
pub(super) fn others(elements: &[Element], rules: &[Option<AutoParams>]) -> Vec<Option<AutoParams>> {
    let of = |(element, rules): (&Element, &Option<AutoParams>)| {
        let preset = other(rules.as_ref()?.auto_fill_uneven)?;
        let mut params = element.params.clone();
        for (key, value) in preset.values {
            params.set(*key, *value);
        }
        AutoParams::from_set(&params).ok().map(|read| read.params)
    };
    elements.iter().zip(rules).map(of).collect()
}

/// What the other style would sew otherwise, counted for one direction: from the style a shape is in, to
/// the other.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct Count {
    /// Thin lines merged into their neighbours, which the other style sews as lines.
    merged: usize,
    /// Thin lines sewn as lines, which the other style merges.
    kept: usize,
    /// Parts filled, by the rule that filled them, which the other style sews as satin columns.
    filled: BTreeMap<RuleId, usize>,
    /// Parts sewn as satin columns, which the other style fills.
    along: usize,
}

impl Count {
    /// Whether nothing is counted.
    fn is_empty(&self) -> bool {
        self.merged == 0 && self.kept == 0 && self.filled.is_empty() && self.along == 0
    }
}

/// What the other style would sew otherwise in a design: a [`Count`] by the name of the other style.
#[derive(Clone, Debug, Default)]
pub(super) struct Tally {
    /// The counts, by the other style's name.
    counts: BTreeMap<&'static str, Count>,
    /// What is said of a shape whose parts couldn't be counted.
    said: Vec<Diagnostic>,
}

impl Tally {
    /// Counts `merged` thin lines merged by areas with `auto_fill_uneven` on, and `kept` lines that areas with
    /// it off sew as lines and the other style merges.
    pub(super) fn lines(&mut self, merged: usize, kept: usize) {
        if let Some(preset) = other(true) {
            self.counts.entry(preset.name).or_default().merged += merged;
        }
        if let Some(preset) = other(false) {
            self.counts.entry(preset.name).or_default().kept += kept;
        }
    }

    /// Counts the parts of the automatic shape `id` that sewed by `fired` under `here`, its bounds, and would
    /// sew otherwise under `there`, the other style's, a part filled here and along its middle there or the
    /// reverse. A unit of `budget`'s work for each part. A shape whose parts need more is said, uncounted.
    pub(super) fn parts(&mut self, id: &ElementId, fired: &[Fired], (here, there): (&Bounds, &Bounds), budget: &Budget) {
        let Some(preset) = other(here.fill_uneven) else { return };
        let mut meter = budget.meter();
        let mut count = Count::default();
        let counted: Result<(), Exhausted> = fired.iter().try_for_each(|fired| {
            meter.charge(1)?;
            match changed(fired, here, there) {
                Some(Changed::Filled(rule)) => *count.filled.entry(rule).or_default() += 1,
                Some(Changed::Along) => count.along += 1,
                None => {}
            }
            Ok(())
        });
        if counted.is_err() {
            let message = format!(
                "Finding whether the {} style would sew this shape otherwise needed more than the work budget of {} units, so its parts aren't counted.",
                preset.name, budget.max_work
            );
            self.said.push(Diagnostic::new(Code::AutoStyle, message).with_element(id.clone()));
            return;
        }
        let total = self.counts.entry(preset.name).or_default();
        total.along += count.along;
        for (rule, n) in count.filled {
            *total.filled.entry(rule).or_default() += n;
        }
    }

    /// The notices: one for each other style that would sew the design otherwise, by the style's name, and
    /// what was said of what couldn't be counted. None when nothing is counted.
    pub(super) fn said(self) -> Vec<Diagnostic> {
        let mut said: Vec<Diagnostic> = self
            .counts
            .iter()
            .filter(|(_, count)| !count.is_empty())
            .filter_map(|(name, count)| PRESETS.iter().find(|preset| preset.name == *name).map(|preset| notice(preset, count)))
            .collect();
        said.extend(self.said);
        said
    }
}

/// How a part would sew otherwise.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Changed {
    /// Filled here by the rule, and along its middle under the other bounds.
    Filled(RuleId),
    /// Along its middle here, and filled under the other bounds.
    Along,
}

/// How the part that sewed by `fired` under `here` would sew otherwise under `there`, decided again from its
/// measures; `None` when it would sew alike, or has no measures. A part whose column was refused is filled
/// here, and would be under `there` too when the rules give it a column again.
fn changed(fired: &Fired, here: &Bounds, there: &Bounds) -> Option<Changed> {
    let measures = fired.measures?;
    let rule = choose(&measures, there);
    let refused = fired.rule == RuleId::Refused;
    let filled_here = refused || fills(fired.rule, here);
    let filled_there = fills(rule, there) || (refused && rule == RuleId::Column);
    match (filled_here, filled_there) {
        (true, false) => Some(Changed::Filled(fired.rule)),
        (false, true) => Some(Changed::Along),
        _ => None,
    }
}

/// `n` of a thing, in the singular `one` or the plural `many`.
fn counted(n: usize, one: &str, many: &str) -> String {
    if n == 1 { format!("1 {one}") } else { format!("{n} {many}") }
}

/// The notice that `preset`, the other style, would sew the design otherwise by `count`.
fn notice(preset: &Preset, count: &Count) -> Diagnostic {
    let mut changes = Vec::new();
    if count.merged > 0 {
        changes.push(format!(
            "{} would sew as {}",
            counted(count.merged, "thin line merged into its neighbour", "thin lines merged into their neighbours"),
            if count.merged == 1 { "a line" } else { "lines" }
        ));
    }
    if count.kept > 0 {
        changes.push(format!(
            "{} may merge into {}",
            counted(count.kept, "thin line sewn as a line", "thin lines sewn as lines"),
            if count.kept == 1 { "its neighbour" } else { "their neighbours" }
        ));
    }
    if !count.filled.is_empty() {
        let by: Vec<String> = count
            .filled
            .iter()
            .enumerate()
            .map(|(n, (rule, parts))| match n {
                0 => format!("{} by the rule {}", counted(*parts, "part filled", "parts filled"), rule.key()),
                _ => format!("{parts} by the rule {}", rule.key()),
            })
            .collect();
        changes.push(format!("{} would sew as satin columns", listed(&by)));
    }
    if count.along > 0 {
        changes.push(format!("{} would be filled", counted(count.along, "part sewn as satin columns", "parts sewn as satin columns")));
    }
    let message = format!("The {} style (`--style {}`) would sew this design otherwise: {}.", preset.name, preset.name, listed(&changes));
    let values: Vec<String> = preset.values.iter().map(|(key, value)| format!("`{key}` to `{value}`")).collect();
    let hint = format!(
        "`--style {}` sets {} on every automatic shape. In VectorCraft, set them in the dialog of StitchCraft's Auto effect.",
        preset.name,
        listed(&values)
    );
    Diagnostic::new(Code::AutoStyle, message).with_fix(Fix::Hint(hint))
}

/// `items` joined: with commas, and "and" before the last.
fn listed(items: &[String]) -> String {
    match items.split_last() {
        None => String::new(),
        Some((last, [])) => last.clone(),
        Some((last, rest)) => format!("{} and {last}", rest.join(", ")),
    }
}

#[cfg(test)]
mod tests;
