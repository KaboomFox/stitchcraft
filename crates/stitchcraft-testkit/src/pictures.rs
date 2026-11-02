//! The test pictures of automatic digitizing where the engine's cases measure them. The pictures are drawn
//! by `stitchcraft_engine::testsheets::pictures`, which the TS-13 sheets sew, so what the cases measure is
//! what the machine sews (`docs/src/design/image-to-embroidery.md` › Testing). Each is placed here where its
//! numbers were measured and recorded, and 3 stripes, which only the cases sew, are drawn here.
//!
//! A picture is traced one of 2 ways ([`Trace`]), and each sews the same areas once digitized: abutting,
//! as VectorCraft's Abutting method traces it, each area with a hole where the next one is; or stacked, as
//! its Overlapping method and Inkscape's Stack option trace it, each colour a whole shape over the ones
//! under it. Every shape is automatic: a shape without settings, as `--auto` reads it.
//!
//! The TS-13 sheets themselves are here too, with their stitches where the sheet draws its shapes
//! ([`sheet`]), so that a case can measure what a sheet sews against what it draws. A sheet's machine file
//! has its origin at the middle of its stitches, which moves every stitch by the same amount.

use stitchcraft_engine::design::{Design, DesignSettings};
pub use stitchcraft_engine::testsheets::pictures::{Picture, Trace};
use stitchcraft_engine::testsheets::{find, pictures};
use stitchcraft_plan::{Rgb, StitchPlan, Thread};

use crate::designs::{BLUE, RED, automatic, fill, p, planned_with};

/// A gold thread.
pub const GOLD: Thread = Thread::new(Rgb::new(230, 180, 30));

/// Every test picture the cases measure.
pub fn all() -> Vec<Picture> {
    vec![stripes(), badge(Trace::Abutting), badge(Trace::Stacked), badges(Trace::Abutting), badges(Trace::Stacked)]
}

/// 3 stripes side by side, 15 mm wide and 40 mm tall, red, blue and gold: 2 straight edges, upright, which
/// the rows of a tatami fill, level by default, end on.
pub fn stripes() -> Picture {
    let stripe = |id: &str, x: f64, thread: &Thread| {
        let ring = [(x, 0.0), (x + 15.0, 0.0), (x + 15.0, 40.0), (x, 40.0)];
        automatic(fill(id, &[&ring], thread, &[]))
    };
    Picture {
        name: "stripes".into(),
        elements: vec![stripe("red", 0.0, &RED), stripe("blue", 15.0, &BLUE), stripe("gold", 30.0, &GOLD)],
        edges: vec![vec![p(15.0, 0.0), p(15.0, 40.0)], vec![p(30.0, 0.0), p(30.0, 40.0)]],
    }
}

/// The badge, 50 mm across round (25, 25), traced as `trace` says ([`pictures::badge`]).
pub fn badge(trace: Trace) -> Picture {
    pictures::badge((25.0, 25.0), 1.0, trace).unwrap()
}

/// 3 badges in a row round the origin, each 30 mm across, the badge at three fifths of its size, traced as
/// `trace` says, badge by badge ([`pictures::badges`]).
pub fn badges(trace: Trace) -> Picture {
    pictures::badges((0.0, 0.0), 0.6, trace).unwrap()
}

/// The emblem, 60 mm wide round (30, 30), traced as `trace` says ([`pictures::emblem`]): its white is the
/// lowest shape and covers the design, the picture's background.
pub fn emblem(trace: Trace) -> Picture {
    pictures::emblem((30.0, 30.0), 1.0, trace).unwrap()
}

/// The button, 29 mm across round (15, 15), traced as `trace` says ([`pictures::button`]).
pub fn button(trace: Trace) -> Picture {
    pictures::button((15.0, 15.0), 1.0, trace).unwrap()
}

/// The test sheet `id`'s design, and its plan for the reference machine with its stitches where the design
/// draws them: planned with its origin at the middle of its shapes, which the hoop holds, and moved back by
/// it.
pub fn sheet(id: &str) -> (Design, StitchPlan) {
    let design = find(id).and_then(|sheet| sheet.design()).unwrap_or_else(|| panic!("{id} is no designed sheet")).unwrap();
    let bounds = design.bounds().unwrap();
    let middle = p((bounds.min().x() + bounds.max().x()) / 2.0, (bounds.min().y() + bounds.max().y()) / 2.0);
    let outcome = planned_with(design.elements().to_vec(), DesignSettings { origin: Some(middle), ..design.settings });
    let mut plan = outcome.plan.unwrap_or_else(|| panic!("{id}: {:?}", outcome.diagnostics));
    for stitch in plan.blocks.iter_mut().flat_map(|block| &mut block.stitches) {
        stitch.at = p(stitch.at.x() + middle.x(), stitch.at.y() + middle.y());
    }
    (design, plan)
}
