//! The test pictures of automatic digitizing where the engine's cases measure them. The pictures are drawn
//! by `stitchcraft_engine::testsheets::pictures`, which the TS-13 sheets sew, so what the cases measure is
//! what the machine sews (`docs/src/design/image-to-embroidery.md` › Testing). Each is placed here where its
//! numbers were measured and recorded, and 3 stripes, which only the cases sew, are drawn here.
//!
//! A picture is traced one of 2 ways ([`Trace`]), and each sews the same areas once digitized: abutting,
//! as VectorCraft's Abutting method traces it, each area with a hole where the next one is; or stacked, as
//! its Overlapping method and Inkscape's Stack option trace it, each colour a whole shape over the ones
//! under it. Every shape is automatic: a shape without settings, as `--auto` reads it.

use stitchcraft_engine::testsheets::pictures;
pub use stitchcraft_engine::testsheets::pictures::{Picture, Trace};
use stitchcraft_plan::{Rgb, Thread};

use crate::designs::{BLUE, RED, automatic, fill, p};

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
