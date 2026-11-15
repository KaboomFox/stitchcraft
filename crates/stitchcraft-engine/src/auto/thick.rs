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

use stitchcraft_core::{Budget, Code, Diagnostic, Fix};
use stitchcraft_plan::StitchPlan;
use stitchcraft_plan::thickness::{Layer, Limits, Over, Report, Spot, Thickness};

use super::AutoParams;

/// How many of the shapes stacked at the thickest spot the warning names: those with the most thread.
const NAMED: usize = 3;

/// The limits that `rules` set.
pub(crate) fn limits(rules: &AutoParams) -> Limits {
    Limits {
        warn_fills: rules.auto_thick_warn_fills,
        fail_fills: rules.auto_thick_fail_fills,
        warn_points: rules.auto_thick_warn_points,
        fail_points: rules.auto_thick_fail_points,
    }
}

/// What is said of how thick `plan` sews against `limits`, measured with `budget`'s work: the warning when
/// a place is at a warning's limit or over it, or when the plan could not be measured, and nothing of a
/// plan within its limits.
pub(crate) fn said(plan: &StitchPlan, limits: &Limits, budget: &Budget) -> Option<Diagnostic> {
    let mut meter = budget.meter();
    let measured =
        Thickness::of(plan, &mut meter).and_then(|thickness| thickness.map(|thickness| thickness.report(plan, limits, &mut meter)).transpose());
    let message = match measured {
        Ok(Some(report)) if report.warns() => message(plan, &report, limits),
        Ok(_) => return None,
        Err(_) => "How thick the design sews was not measured: it is larger than 500 mm, or measuring it needs more work than the budget allows."
            .to_string(),
    };
    let hint = "The limits are starting values that no sew-out has confirmed: sew a test piece of the thickest part first. `stitch preview --style thickness` draws where the thread is thick.";
    Some(Diagnostic::new(Code::AutoThick, message).with_fix(Fix::Hint(hint.to_string())))
}

/// The warning's message for `report` of `plan`. A place is given from the design's centre, x to the right
/// and y down, as the plan has it.
fn message(plan: &StitchPlan, report: &Report, limits: &Limits) -> String {
    let place = |spot: &Spot| format!("({:.1}, {:.1}) mm from the design's centre", spot.at.x(), spot.at.y());
    let areas =
        |over: &Over, failing: &Over| format!("{:.1} mm² in {}", over.area, places(over.pieces)) + &format!(", {:.1} mm² of it", failing.area);
    format!(
        "The design sews thick. Thread: up to {:.1} fills over one spot, at {}, where {} are stacked. At {} fills or more: {} at {} or more. Needle points: up to {:.0} in a square millimetre, at {}. At {} or more: {} at {} or more.",
        report.fills.value,
        place(&report.fills),
        stacked(plan, &report.stack),
        limits.warn_fills,
        areas(&report.thick, &report.failing),
        limits.fail_fills.max(limits.warn_fills),
        report.points.value,
        place(&report.points),
        limits.warn_points,
        areas(&report.dense, &report.pierced),
        limits.fail_points.max(limits.warn_points),
    )
}

/// `count` places, in words.
fn places(count: usize) -> String {
    if count == 1 { "1 place".to_string() } else { format!("{count} places") }
}

/// The [`NAMED`] layers of `stack` with the most thread, the most first: each element's id, its thread and
/// its fills, and how many others lie there.
fn stacked(plan: &StitchPlan, stack: &[Layer]) -> String {
    let mut layers: Vec<&Layer> = stack.iter().collect();
    layers.sort_by(|a, b| (b.top + b.under).total_cmp(&(a.top + a.under)));
    let name = |layer: &&Layer| {
        let id = layer
            .element
            .and_then(|element| plan.elements.get(element.index()))
            .map_or_else(|| "stitches of no shape".to_string(), |id| format!("`{id}`"));
        let thread = plan.blocks.get(layer.block).map(|block| block.thread.color.to_string()).unwrap_or_default();
        format!("{id} in {thread} with {:.1}", layer.top + layer.under)
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
    }

    /// The message of 4 squares on a layer of no shape: the turns along 2 sides are thickest.
    const EXPECTED: &str = "The design sews thick. Thread: up to 5.6 fills over one spot, at (9.5, 4.1) mm from the design's centre, where stitches of no shape in #000000 with 1.1, `s1` in #010000 with 1.1, `s2` in #020000 with 1.1 and 2 more are stacked. At 3 fills or more: 99.8 mm² in 1 place, 90.0 mm² of it at 4.5 or more. Needle points: up to 25 in a square millimetre, at (10.5, 1.5) mm from the design's centre. At 12 or more: 19.8 mm² in 2 places, 18.4 mm² of it at 16 or more.";

    #[test]
    fn diag_sc_w0913_says_when_the_design_was_not_measured() {
        // Without the work to measure it, and when it is too large.
        for (plan, budget) in [(plan(4, 0), Budget { max_work: 1_000, ..Budget::DEFAULT }), (wide(), Budget::DEFAULT)] {
            let warned = said(&plan, &Limits::DEFAULT, &budget).unwrap();
            assert!(warned.code == Code::AutoThick && warned.message.starts_with("How thick the design sews was not measured"), "{warned:?}");
        }
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
        assert_eq!(stacked(&plan, &stack[..1]), "`s0` in #000000 with 0.8");
        assert_eq!(stacked(&plan, &stack[..3]), "`s1` in #010000 with 1.5, `s0` in #010000 with 1.0, `s0` in #000000 with 0.8");
        assert_eq!(stacked(&plan, &stack), "`s1` in #010000 with 1.5, `s0` in #010000 with 1.0, `s0` in #000000 with 0.8 and 1 more");
        assert_eq!(stacked(&plan, &stack[3..]), "stitches of no shape in  with 0.2");
        assert_eq!((places(0), places(1), places(2)), ("0 places".to_string(), "1 place".to_string(), "2 places".to_string()));
    }
}
