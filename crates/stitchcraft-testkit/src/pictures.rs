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
use stitchcraft_engine::design::{Design, DesignSettings, Element, Reading};
pub use stitchcraft_engine::testsheets::pictures::{Picture, Trace};
use stitchcraft_engine::testsheets::{find, pictures};
use stitchcraft_plan::{Rgb, StitchPlan, Thread};

use crate::designs::{BLUE, RED, automatic, fill, in_place, outlined, p};

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
        outlined(fill(id, &[&ring], thread, &[]))
    };
    Picture {
        name: "stripes".into(),
        elements: vec![stripe("red", 0.0, &RED), stripe("blue", 15.0, &BLUE), stripe("gold", 30.0, &GOLD)],
        edges: vec![vec![p(15.0, 0.0), p(15.0, 40.0)], vec![p(30.0, 0.0), p(30.0, 40.0)]],
    }
}

/// A black thread, of the details on [`marked`].
pub const BLACK: Thread = Thread::new(Rgb::new(20, 20, 20));

/// A green thread, of the right half of [`marked`].
pub const GREEN: Thread = Thread::new(Rgb::new(0, 140, 60));

/// A patch 60 mm wide and 40 mm tall with 2 details on it, traced as `trace` says, which only the cases
/// sew (`REQ-AUTO-096`, `REQ-AUTO-097`): its left half blue and its right half green, a black diamond 12 mm
/// across where the halves meet, and in the left half a gold square ring 3 mm wide round a black dot 6 mm
/// square. Abutting, the details are drawn first, the dot, the mark and the ring, and each half has a notch
/// where the mark lies, so the mark lies in no area's hole. Stacked, the halves are drawn first and the
/// details on them. The edges are where the halves meet each other and the mark.
pub fn marked(trace: Trace) -> Picture {
    let square = |(x, y): (f64, f64), size: f64| vec![(x, y), (x + size, y), (x + size, y + size), (x, y + size)];
    let reversed = |ring: Vec<(f64, f64)>| ring.into_iter().rev().collect::<Vec<_>>();
    let (outer, inner) = (square((8.0, 14.0), 12.0), square((11.0, 17.0), 6.0));
    let mark = vec![(24.0, 20.0), (30.0, 14.0), (36.0, 20.0), (30.0, 26.0)];
    let area = |id: &str, rings: &[&[(f64, f64)]], thread: &Thread| automatic(fill(id, rings, thread, &[]));
    let elements = match trace {
        Trace::Abutting => {
            let left = [(0.0, 0.0), (30.0, 0.0), (30.0, 14.0), (24.0, 20.0), (30.0, 26.0), (30.0, 40.0), (0.0, 40.0)];
            let right = [(30.0, 0.0), (60.0, 0.0), (60.0, 40.0), (30.0, 40.0), (30.0, 26.0), (36.0, 20.0), (30.0, 14.0)];
            vec![
                area("dot", &[&inner], &BLACK),
                area("mark", &[&mark], &BLACK),
                area("ring", &[&outer, &reversed(inner.clone())], &GOLD),
                area("left", &[&left, &reversed(outer.clone())], &BLUE),
                area("right", &[&right], &GREEN),
            ]
        }
        Trace::Stacked => vec![
            area("left", &[&[(0.0, 0.0), (30.0, 0.0), (30.0, 40.0), (0.0, 40.0)]], &BLUE),
            area("right", &[&[(30.0, 0.0), (60.0, 0.0), (60.0, 40.0), (30.0, 40.0)]], &GREEN),
            area("ring", &[&outer], &GOLD),
            area("dot", &[&inner], &BLACK),
            area("mark", &[&mark], &BLACK),
        ],
    };
    let meeting = [vec![p(30.0, 0.0), p(30.0, 14.0)], vec![p(30.0, 26.0), p(30.0, 40.0)], mark.iter().map(|&(x, y)| p(x, y)).collect()];
    let name = match trace {
        Trace::Abutting => "marked-abutting",
        Trace::Stacked => "marked-stacked",
    };
    Picture { name: name.into(), elements, edges: meeting.to_vec() }
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

/// A lettered patch drawn in code: the word MATCH in block letters 10 mm tall, each a shape of its own
/// (`testsheets::pictures::word`), on a cream field with a red border, both with cut corners. Every shape is
/// automatic, and the field and the border lie whole under what is drawn on them, as a patch drawn in an
/// editor has them.
pub fn lettered_patch() -> Design {
    let word = pictures::word((0.0, 0.0), 10.0).unwrap();
    let letters = Design::new(word.elements.clone(), DesignSettings::default()).unwrap();
    let bounds = letters.bounds().unwrap();
    let plate = |id: &str, margin: f64, thread: &Thread| {
        let (left, top) = (bounds.min().x() - margin, bounds.min().y() - margin);
        let (right, bottom) = (bounds.max().x() + margin, bounds.max().y() + margin);
        let cut = margin / 2.0;
        let ring = [
            (left + cut, top),
            (right - cut, top),
            (right, top + cut),
            (right, bottom - cut),
            (right - cut, bottom),
            (left + cut, bottom),
            (left, bottom - cut),
            (left, top + cut),
        ];
        automatic(fill(id, &[&ring], thread, &[]))
    };
    let border = plate("patch-border", 9.0, &Thread::new(Rgb::new(200, 0, 0)));
    let field = plate("patch-field", 6.0, &Thread::new(Rgb::new(236, 226, 198)));
    let shapes: Vec<Element> = [border, field].into_iter().chain(word.elements).collect();
    Design::new(shapes, DesignSettings::default()).unwrap()
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
