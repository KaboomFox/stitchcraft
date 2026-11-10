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
//!
//! So are the pear the docs show ([`pear`]) and the owner's dog ([`dogs`]), photos that VectorCraft traced
//! into shapes, whose ragged edges no picture drawn in code has.

use stitchcraft_core::Budget;
use stitchcraft_engine::design::{Design, Reading};
pub use stitchcraft_engine::testsheets::pictures::{Picture, Trace};
use stitchcraft_engine::testsheets::{find, pictures};
use stitchcraft_plan::{Rgb, StitchPlan, Thread};

use crate::designs::{BLUE, RED, automatic, fill, in_place, p};

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
/// draws them ([`in_place`]).
pub fn sheet(id: &str) -> (Design, StitchPlan) {
    let design = find(id).and_then(|sheet| sheet.design()).unwrap_or_else(|| panic!("{id} is no designed sheet")).unwrap();
    let outcome = in_place(&design);
    let plan = outcome.plan.unwrap_or_else(|| panic!("{id}: {:?}", outcome.diagnostics));
    (design, plan)
}

/// The photo of a pear that the docs show, traced 90 mm across into 4 of the reference machine's threads by
/// VectorCraft 0.19.0 (`docs/fixtures/traced-pear.vectorcraft`): a light grey square, the picture's
/// background, and on it the pear in 3 greens, read as `--auto` reads it, every shape automatic.
pub fn pear() -> Design {
    traced("traced-pear.vectorcraft")
}

/// The owner's 2 photos of her dog, each cut out by a mask checked by hand and traced 100 mm tall by
/// VectorCraft 0.22.0 in 6 and in 9 of the reference machine's threads (`docs/fixtures/README.md` › The
/// traced dogs), by the names the cases give them. The background is transparent and traces into no shape,
/// so every shape is the dog's.
pub fn dogs() -> Vec<(String, Design)> {
    ["dog-on-turf-6", "dog-on-turf-9", "dog-running-6", "dog-running-9"]
        .into_iter()
        .map(|name| (name.to_string(), traced(&format!("traced-{name}.vectorcraft"))))
        .collect()
}

/// The shapes `ids` of the traced dog `name` ([`dogs`]) alone, in their paint order: a piece of the picture
/// that a case digitizes in the tests' debug build, where a whole dog takes too long.
pub fn dog_shapes(name: &str, ids: &[&str]) -> Design {
    let dog = traced(&format!("traced-{name}.vectorcraft"));
    let shapes = dog.elements().iter().filter(|element| ids.contains(&element.id.to_string().as_str())).cloned().collect();
    Design::new(shapes, dog.settings).unwrap()
}

/// The VectorCraft document `fixture` of the docs' fixtures, read as `--auto` reads it, every shape
/// automatic.
#[allow(clippy::disallowed_methods)] // a test reads the docs' fixture from the repository
fn traced(fixture: &str) -> Design {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/fixtures").join(fixture);
    let bytes = std::fs::read(path).unwrap();
    stitchcraft_vectorcraft::read_with(&bytes, &Budget::DEFAULT, Reading { auto: true, ..Reading::default() }).unwrap().design
}
