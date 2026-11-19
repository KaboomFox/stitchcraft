//! What planning says of a design that sews thick (`SC-W0913`, `REQ-AUTO-062`;
//! `docs/src/design/image-to-embroidery.md` › How thick a design sews).
//!
//! Automatic digitizing stacks thread on purpose: areas reach under their neighbours, a base layer lies
//! under them and an outline on their edges. The owner of a one-needle machine can't see from a picture
//! where that is too much for the needle, so the plan is measured once it is finished, as the machine
//! will sew it (`stitchcraft_plan::thickness`), and the warning names the thickest spot and what is
//! stacked there. The design is written all the same: the limits are starting values, and a design over
//! them may sew well. Only a design with an automatic shape is measured, by the limits of its lowest
//! one, as the base layer goes by that shape's settings.

use std::collections::BTreeSet;

use stitchcraft_core::diag::called;
use stitchcraft_core::{Budget, Code, Diagnostic, ElementId, Exhausted, Fix, Meter};
use stitchcraft_plan::thickness::{Layer, Limits, Measure, Over, Report, Spot, Thickness};
use stitchcraft_plan::{ElementRef, MachineProfile, StitchPlan};

use super::AutoParams;
use super::cap::{self, Capped};
use super::thin::{self, Thinned};
use crate::names::Names;

/// How many of the shapes stacked at the thickest spot the warning names: those with the most thread.
const NAMED: usize = 3;

/// What to do about the warning. VectorCraft shows 500 characters of an export's warning, so the message
/// and this are kept short enough to be shown whole, the advice first.
const HINT: &str = "Sew a test piece of it first. `stitch preview --style thickness` draws the thick places.";

/// The limits that `rules` set.
pub(crate) fn limits(rules: &AutoParams) -> Limits {
    Limits {
        warn_fills: rules.auto_thick_warn_fills,
        fail_fills: rules.auto_thick_fail_fills,
        warn_points: rules.auto_thick_warn_points,
        fail_points: rules.auto_thick_fail_points,
    }
}

/// What a design digitized automatically is held to: the limits of its thickness and the overlap of its
/// areas, by its lowest automatic shape's settings.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Held {
    /// The limits of its thickness.
    pub(crate) limits: Limits,
    /// How far an area reaches under its neighbours, in millimetres (`auto_overlap_mm`).
    pub(crate) overlap: f64,
    /// The most fills of thread a spot of a millimetre sews (`auto_thick_cap_fills`, `REQ-AUTO-099`).
    pub(crate) cap: f64,
}

/// What `rules` hold a design to.
pub(crate) fn held(rules: &AutoParams) -> Held {
    Held { limits: limits(rules), overlap: rules.auto_overlap_mm.get(), cap: rules.auto_thick_cap_fills }
}

/// The finished `plan` of a design digitized automatically, thinned where a spot sews above the cap
/// (`REQ-AUTO-074`, [`thin`]) and then capped there (`REQ-AUTO-100`, [`cap`]), and what is said of it: what
/// the pass and the cap took (`SC-I1119`) and how thick the plan sews (`SC-W0913`). It is measured once, with
/// `budget`'s work, and the pass and the cap have the work once more each. `base` names the base layer's
/// pieces and `made` the elements that digitizing made, whose needle points alone the cap takes, and
/// `profile` is the machine's, whose stitches the pass keeps to. The warning names the shapes by `names`.
pub(crate) fn finished(
    plan: &mut StitchPlan,
    held: &Held,
    (base, made): (&[ElementId], &BTreeSet<ElementId>),
    profile: &MachineProfile,
    names: &Names,
    budget: &Budget,
) -> Vec<Diagnostic> {
    let mut meter = budget.meter();
    let mut said = Vec::new();
    let measured = Thickness::of(plan, &mut meter).map(|thickness| {
        thickness.map(|thickness| {
            if thickness.most(Measure::Fills).value <= held.cap {
                return thickness;
            }
            let refs = |of: &dyn Fn(&ElementId) -> bool| -> BTreeSet<ElementRef> {
                plan.elements.iter().enumerate().filter(|(_, id)| of(id)).filter_map(|(at, _)| ElementRef::from_index(at)).collect()
            };
            let (base, made) = (refs(&|id| base.contains(id)), refs(&|id| made.contains(id)));
            let rules = thin::Rules {
                cap: held.cap,
                base: &base,
                overlap: held.overlap,
                shortest: profile.min_stitch.get(),
                longest: profile.max_stitch.get(),
            };
            let (mut thinner, mut measure) = (plan.clone(), thickness.clone());
            let thinned = match thin::thinned(&mut thinner, &mut measure, &rules, &mut budget.meter()) {
                Ok(thinned) => thinned,
                Err(_) => {
                    said.push(Diagnostic::new(Code::AutoThinned, not_thinned(held.cap)));
                    return thickness;
                }
            };
            // The pass looks at a plan above the cap only, which this one is.
            let Some(thinned) = thinned else { return thickness };
            // The cap works on what the pass left, and keeps it when it runs out of work (`REQ-AUTO-101`).
            let (mut capped, mut kept) = (thinner.clone(), measure.clone());
            let outcome = cap::capped(&mut capped, &mut kept, &rules, &made, &mut budget.meter());
            let took = matches!(&outcome, Ok(Some(capped)) if capped.took());
            if thinned.took() || took || outcome.is_err() {
                said.push(note(&thinned, &outcome, held.cap));
            }
            if took {
                (*plan, measure) = (capped, kept);
            } else if thinned.took() {
                *plan = thinner;
            } else {
                return thickness;
            }
            measure
        })
    });
    said.extend(told(plan, measured, (&held.limits, held.cap), names, &mut meter));
    said
}

/// What the note says when the pass ran out of work, for a cap of `cap` fills.
fn not_thinned(cap: f64) -> String {
    format!("No thread was taken from the spots above {cap} fills: the last pass needs more work than the budget allows.")
}

/// The note of what the pass took and what the cap took after it, or that the cap ran out of work, for a
/// cap of `cap` fills (`SC-I1119`).
fn note(thinned: &Thinned, capped: &Result<Option<Capped>, Exhausted>, cap: f64) -> Diagnostic {
    let place = |spot: &Spot| format!("({:.1}, {:.1})", spot.at.x(), spot.at.y());
    let pass = if thinned.took() { thin::kinds(thinned) } else { "nothing".to_string() };
    let (then, left, thickest) = match capped {
        Ok(Some(capped)) if capped.took() => (format!(", and the cap took {}", cap::kinds(capped)), &capped.left, &capped.thickest),
        Err(_) => (". The cap needs more work than the budget allows, and took nothing".to_string(), &thinned.left, &thinned.thickest),
        Ok(_) => (String::new(), &thinned.left, &thinned.thickest),
    };
    let message = format!(
        "{:.1} mm² in {} sewed {cap} fills or more. The last pass took {pass} there{then}. Left: {:.1} mm² at {cap} fills or more, and the thickest spot, {:.1} fills at {} mm from the design's centre.",
        thinned.above.area,
        places(thinned.above.pieces),
        left.area,
        thickest.value,
        place(thickest),
    );
    Diagnostic::new(Code::AutoThinned, message)
}

/// What is said of how thick `plan` sews against `limits`, measured with `budget`'s work: the warning when
/// a place is at a warning's limit or over it, or when the plan could not be measured, and nothing of a
/// plan within its limits.
#[cfg(test)]
pub(crate) fn said(plan: &StitchPlan, limits: &Limits, budget: &Budget) -> Option<Diagnostic> {
    let mut meter = budget.meter();
    let measured = Thickness::of(plan, &mut meter);
    told(plan, measured, (limits, held(&AutoParams::defaults()).cap), &Names::default(), &mut meter)
}

/// What is said of `plan`, `measured`, against `limits` and a cap of `cap` fills, the report made with
/// `meter`'s work and its shapes named by `names`.
fn told(
    plan: &StitchPlan,
    measured: Result<Option<Thickness>, Exhausted>,
    (limits, cap): (&Limits, f64),
    names: &Names,
    meter: &mut Meter,
) -> Option<Diagnostic> {
    let measured = measured.and_then(|thickness| thickness.map(|thickness| thickness.report(plan, limits, meter)).transpose());
    let message = match measured {
        Ok(Some(report)) if report.warns() => message(plan, &report, (limits, cap), names),
        Ok(_) => return None,
        Err(_) => "How thick the design sews was not measured: it is larger than 500 mm, or measuring it needs more work than the budget allows."
            .to_string(),
    };
    let hint = HINT;
    Some(Diagnostic::new(Code::AutoThick, message).with_fix(Fix::Hint(hint.to_string())))
}

/// The warning's message for `report` of `plan`, naming the cap of `cap` fills when the thickest spot is
/// above it (`REQ-AUTO-101`). A place is given from the design's centre, x to the right and y down, as the
/// plan has it, and the shapes stacked there by `names`.
fn message(plan: &StitchPlan, report: &Report, (limits, cap): (&Limits, f64), names: &Names) -> String {
    let place = |spot: &Spot| format!("({:.1}, {:.1})", spot.at.x(), spot.at.y());
    let areas = |over: &Over, failing: &Over| (format!("{:.1} mm² in {}", over.area, places(over.pieces)), format!("{:.1} mm² of it", failing.area));
    let (thick, failing) = areas(&report.thick, &report.failing);
    let (dense, pierced) = areas(&report.dense, &report.pierced);
    let capped = if report.fills.value > cap { " Above the cap." } else { "" };
    format!(
        "Thread is up to {:.1} fills thick at {} mm from the design's centre: {}.{capped} {thick} has {} fills or more, {failing} {} or more. Needle points: up to {:.0} per mm² at {}, {dense} at {} or more, {pierced} at {} or more.",
        report.fills.value,
        place(&report.fills),
        stacked(plan, &report.stack, names),
        limits.warn_fills,
        limits.fail_fills.max(limits.warn_fills),
        report.points.value,
        place(&report.points),
        limits.warn_points,
        limits.fail_points.max(limits.warn_points),
    )
}

/// `count` places, in words.
pub(super) fn places(count: usize) -> String {
    if count == 1 { "1 place".to_string() } else { format!("{count} places") }
}

/// The [`NAMED`] layers of `stack` with the most thread, the most first: each element's shape's name by
/// `names` and its id in brackets ([`called`]), its thread and its fills, and how many others lie there.
fn stacked(plan: &StitchPlan, stack: &[Layer], names: &Names) -> String {
    let mut layers: Vec<&Layer> = stack.iter().collect();
    layers.sort_by(|a, b| (b.top + b.under).total_cmp(&(a.top + a.under)));
    let name = |layer: &&Layer| {
        let id = layer
            .element
            .and_then(|element| plan.elements.get(element.index()))
            .map_or_else(|| "stitches of no shape".to_string(), |id| format!("({})", called(names.name_of(id.as_str()), id)));
        let thread = plan.blocks.get(layer.block).map(|block| block.thread.color.to_string()).unwrap_or_default();
        format!("{id} {thread} {:.1}", layer.top + layer.under)
    };
    let named: Vec<String> = layers.iter().take(NAMED).map(name).collect();
    let others = layers.len().saturating_sub(NAMED);
    if others == 0 { named.join(", ") } else { format!("{} and {others} more", named.join(", ")) }
}

#[cfg(test)]
mod tests {
    use stitchcraft_core::{ElementId, Point};
    use stitchcraft_plan::{PlanBuilder, Provenance, Rgb, Role, Thread};

    use super::*;

    /// A plan of the square from (0, 0) to (10, 10) sewn `layers` times in a fill's rows, 0.25 mm apart, each
    /// time by an element of its own in a block of its own, after `under` such layers of no element.
    fn plan(layers: u8, under: u8) -> StitchPlan {
        let mut builder = PlanBuilder::new(Thread::new(Rgb::new(0, 0, 0)));
        for layer in 0..under + layers {
            let element = (layer >= under).then(|| builder.element(ElementId::new(format!("s{layer}")).unwrap()).unwrap());
            for row in 0..41 {
                let ends = if row % 2 == 0 { [0.0, 10.0] } else { [10.0, 0.0] };
                for x in ends {
                    let origin = element.map_or(Provenance::plan(Role::Travel), |element| Provenance::new(element, Role::Top));
                    builder.stitch(Point::new(x, f64::from(row) * stitchcraft_plan::thickness::ROWS).unwrap(), origin);
                }
            }
            builder.change_thread(Thread::new(Rgb::new(layer + 1, 0, 0)));
        }
        builder.finish()
    }

    #[test]
    fn req_auto_062_the_limits_are_the_settings_of_the_lowest_automatic_shape() {
        assert_eq!(limits(&AutoParams::defaults()), Limits::DEFAULT);
        // A fill is one whose rows lie as far apart as a fill's by default.
        let spacing = crate::generators::tatami::TatamiParams::SPECS.iter().find(|spec| spec.key == "row_spacing_mm").unwrap();
        assert_eq!(spacing.default.parse(), Ok(stitchcraft_plan::thickness::ROWS));
        let set =
            [("auto_thick_warn_fills", "2.5"), ("auto_thick_fail_fills", "4"), ("auto_thick_warn_points", "7"), ("auto_thick_fail_points", "9")];
        let rules = AutoParams::from_set(&set.into_iter().collect()).unwrap().params;
        assert_eq!(limits(&rules), Limits { warn_fills: 2.5, fail_fills: 4.0, warn_points: 7.0, fail_points: 9.0 });
    }

    #[test]
    fn req_auto_062_2_fills_never_warn_and_3_do() {
        // The rows' ends of these squares lie 2 to a row along 2 sides, which no fill sews: the cases of
        // the needle points are those of the plan's measure.
        let limits = Limits { warn_points: 99.0, fail_points: 99.0, ..Limits::DEFAULT };
        assert_eq!(said(&plan(2, 0), &limits, &Budget::DEFAULT), None);
        assert_eq!(said(&plan(1, 0), &Limits::DEFAULT, &Budget { max_work: 100_000, ..Budget::DEFAULT }), None);
        // 3 fills measure 3 but for a rounding, so the case sews 4.
        let warned = said(&plan(4, 0), &limits, &Budget::DEFAULT).unwrap();
        assert_eq!((warned.code, warned.element, warned.at), (Code::AutoThick, None, None));
        assert!(matches!(&warned.fix, Some(Fix::Hint(hint)) if hint.contains("stitch preview --style thickness")));
        // A plan without a stitch has nothing to say.
        assert_eq!(said(&StitchPlan::default(), &limits, &Budget::DEFAULT), None);
    }

    #[test]
    fn diag_sc_w0913_gives_the_most_of_each_measure_and_what_is_stacked() {
        let limits = Limits { fail_fills: 4.5, warn_points: 12.0, fail_points: 16.0, ..Limits::DEFAULT };
        let warned = said(&plan(4, 1), &limits, &Budget::DEFAULT).unwrap();
        assert_eq!(warned.message, EXPECTED, "{}", warned.message);
        // VectorCraft shows 500 characters of a warning. With ids as long as a traced picture's shapes have,
        // names as long as the machine's thread names make them, coordinates and areas of 5 figures and the
        // hint, the line has fewer.
        let named = |n: u8| format!("(\"Wisteria Violet column (12)\", vc:1234:fill#2~{n})");
        let long = EXPECTED.replace("(s1)", &named(2)).replace("(s2)", &named(3)).replace("stitches of no shape", &named(4));
        let long = long.replace("(9.5, 9.5)", "(-123.4, -123.4)").replace("(10.5, 9.5)", "(-123.4, -123.4)").replace("2 more", "12 more");
        let line = Diagnostic { message: long, ..warned }.one_line();
        assert!(line.ends_with(HINT) && line.chars().count() <= 500, "{}: {line}", line.chars().count());
    }

    #[test]
    fn req_auto_101_the_warning_names_the_cap_only_where_a_spot_is_above_it() {
        let sewn = plan(4, 1);
        let mut meter = Budget::DEFAULT.meter();
        let report = Thickness::of(&sewn, &mut meter).unwrap().unwrap().report(&sewn, &Limits::DEFAULT, &mut meter).unwrap();
        let at = report.fills.value;
        let names = Names::default();
        assert!(!message(&sewn, &report, (&Limits::DEFAULT, at), &names).contains("cap"));
        assert!(message(&sewn, &report, (&Limits::DEFAULT, at - 0.01), &names).contains(" and 2 more. Above the cap. "));
    }

    /// The message of 4 squares on a layer of no shape: a corner where the rows turn is thickest, above the cap
    /// (`REQ-AUTO-101`).
    const EXPECTED: &str = "Thread is up to 5.3 fills thick at (9.5, 9.5) mm from the design's centre: stitches of no shape #000000 1.1, (s1) #010000 1.1, (s2) #020000 1.1 and 2 more. Above the cap. 109.2 mm² in 1 place has 3 fills or more, 90.2 mm² of it 4.5 or more. Needle points: up to 20 per mm² at (10.5, 9.5), 20.0 mm² in 2 places at 12 or more, 18.5 mm² of it at 16 or more.";

    #[test]
    fn diag_sc_w0913_says_when_the_design_was_not_measured() {
        // Without the work to measure it, and when it is too large.
        for (plan, budget) in [(plan(4, 0), Budget { max_work: 1_000, ..Budget::DEFAULT }), (wide(), Budget::DEFAULT)] {
            let warned = said(&plan, &Limits::DEFAULT, &budget).unwrap();
            assert!(warned.code == Code::AutoThick && warned.message.starts_with("How thick the design sews was not measured"), "{warned:?}");
        }
    }

    #[test]
    fn req_auto_074_a_pass_that_takes_nothing_says_nothing_and_one_out_of_work_says_so() {
        let held = Held { limits: Limits::DEFAULT, overlap: 1.0, cap: 4.0 };
        let profile = stitchcraft_plan::profiles::REFERENCE;
        // 5 squares of rows of no shape, which no shape hides: the pass looks and takes nothing, and only
        // SC-W0913 is said.
        let sewn = plan(0, 5);
        let mut kept = sewn.clone();
        let codes = |said: &[Diagnostic]| said.iter().map(|said| said.code).collect::<Vec<_>>();
        assert_eq!(codes(&finished(&mut kept, &held, (&[], &BTreeSet::new()), profile, &Names::default(), &Budget::DEFAULT)), [Code::AutoThick]);
        assert_eq!(kept, sewn);
        // A plan within the cap is not gone through, and 2 fills say nothing.
        let (within, mut kept) = (plan(0, 2), plan(0, 2));
        assert!(finished(&mut kept, &held, (&[], &BTreeSet::new()), profile, &Names::default(), &Budget::DEFAULT).is_empty());
        assert_eq!(kept, within);
        // With the work to measure it and not to thin it, the plan stays as it is, and SC-I1119 says so.
        let used = |work: &dyn Fn(&mut Meter) -> bool| {
            let mut meter = Budget::DEFAULT.meter();
            assert!(work(&mut meter));
            Budget::DEFAULT.max_work - meter.work_left()
        };
        let measure = used(&|meter| Thickness::of(&sewn, meter).is_ok());
        let thickness = Thickness::of(&sewn, &mut Budget::DEFAULT.meter()).unwrap().unwrap();
        let base = BTreeSet::new();
        let rules = thin::Rules { cap: 4.0, base: &base, overlap: 1.0, shortest: 0.3, longest: 12.1 };
        let pass = used(&|meter| thin::thinned(&mut sewn.clone(), &mut thickness.clone(), &rules, meter).is_ok());
        assert!(pass > measure, "{pass} {measure}");
        let budget = Budget { max_work: measure, ..Budget::DEFAULT };
        let mut kept = sewn.clone();
        let said = finished(&mut kept, &held, (&[], &BTreeSet::new()), profile, &Names::default(), &budget);
        assert_eq!(
            said.first().map(|said| (said.code, said.message.as_str())),
            Some((Code::AutoThinned, "No thread was taken from the spots above 4 fills: the last pass needs more work than the budget allows.")),
            "{said:?}"
        );
        assert_eq!(kept, sewn);
    }

    /// A plan of one stitch 600 mm long and another across it.
    fn wide() -> StitchPlan {
        let mut builder = PlanBuilder::new(Thread::new(Rgb::new(0, 0, 0)));
        for (x, y) in [(0.0, 0.0), (600.0, 0.0), (600.0, 600.0)] {
            builder.stitch(Point::new(x, y).unwrap(), Provenance::plan(Role::Top));
        }
        builder.finish()
    }

    #[test]
    fn diag_sc_w0913_names_the_3_shapes_with_the_most_thread() {
        let plan = plan(2, 0);
        let layer = |block, element, top, under| Layer { block, element: stitchcraft_plan::ElementRef::from_index(element), top, under };
        let stack = [layer(0, 0, 0.5, 0.25), layer(1, 1, 1.5, 0.0), layer(1, 0, 0.0, 1.0), layer(7, 9, 0.25, 0.0)];
        let names = Names::default();
        assert_eq!(stacked(&plan, &stack[..1], &names), "(s0) #000000 0.8");
        assert_eq!(stacked(&plan, &stack[..3], &names), "(s1) #010000 1.5, (s0) #010000 1.0, (s0) #000000 0.8");
        assert_eq!(stacked(&plan, &stack, &names), "(s1) #010000 1.5, (s0) #010000 1.0, (s0) #000000 0.8 and 1 more");
        assert_eq!(stacked(&plan, &stack[3..], &names), "stitches of no shape  0.2");
        assert_eq!((places(0), places(1), places(2)), ("0 places".to_string(), "1 place".to_string(), "2 places".to_string()));
    }
}
