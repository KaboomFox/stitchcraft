//! The lock shapes: StitchCraft's own design behind each of Ink/Stitch's lock ids.
//!
//! The ids are the interoperability contract, so a file that asks for a bowtie gets one; the shapes are
//! designed here, never taken from Ink/Stitch (ADR-0001; deviation `DEV-LCK-001` in
//! `conformance/deviations.toml`). Design: `docs/src/design/algorithms/locks.md`.
//!
//! A shape is drawn in a frame at the end of the stitching it secures: x runs along the stitching, into
//! it, and y across it, in millimetres. Every shape follows the same rules, so each one holds the same
//! way and hides under the stitching it secures:
//!
//! - **A loop from the stitching's end back to it.** The needle arrives at the start of the stitching, sews
//!   the lock and carries on from where it arrived; at the end, it sews the lock and is back where the
//!   stitching ended, for the trim.
//! - **Mostly ahead of that point,** at most 1.4 mm along the stitching and 0.6 mm across it at 100 %,
//!   so the stitching covers the lock.
//! - **Stitches 0.46 to 1.4 mm long at 100 %,** well clear of the 0.2 mm a lock stitch needs, and long
//!   enough to grip.
//!
//! Which size parameter a shape takes follows from its kind: steps take `lock_*_scale_mm`, drawn loops
//! `lock_*_scale_percent`, and a custom lock either. [`LOCKS`], [`SIZED_IN_MM`] and [`SIZED_IN_PERCENT`]
//! are computed from the one table below, so they cannot disagree with it.

use stitchcraft_params::ChoiceOption;

/// How a lock is sewn.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Shape {
    /// Back and forth over the first half of the first stitch, sized from that stitch.
    HalfStitch,
    /// Steps along the stitching in sewing order, positive into it, in sizes of `lock_*_scale_mm`.
    Steps(&'static [f64]),
    /// A loop from (0, 0) round and back to it, in millimetres at 100 % of `lock_*_scale_percent`.
    Drawn(&'static [(f64, f64)]),
    /// The element's own steps (`lock_custom_start`, `lock_custom_end`).
    Custom,
}

impl Shape {
    /// Whether `lock_*_scale_mm` (`by_mm`) or `lock_*_scale_percent` (otherwise) sizes it.
    const fn sized_by(self, by_mm: bool) -> bool {
        match self {
            Shape::HalfStitch => false,
            Shape::Steps(_) => by_mm,
            Shape::Drawn(_) => !by_mm,
            Shape::Custom => true,
        }
    }
}

/// One lock: its id and label, as a settings window offers it, and its shape.
#[derive(Clone, Copy, Debug)]
struct Design {
    option: ChoiceOption,
    shape: Shape,
}

/// Every lock, in the order Ink/Stitch offers them.
const TABLE: &[Design] = &[
    Design { option: ChoiceOption { id: "half_stitch", label: "Half stitch" }, shape: Shape::HalfStitch },
    // A shaft along the stitching to the tip of a small arrowhead, round the head and back down the shaft.
    Design {
        option: ChoiceOption { id: "arrow", label: "Arrow" },
        shape: Shape::Drawn(&[(0.0, 0.0), (1.4, 0.0), (0.9, 0.4), (0.9, -0.4), (1.4, 0.0), (0.0, 0.0)]),
    },
    // Forth and back over one step, twice: the same rhythm as the half stitch, at a size the user sets.
    Design { option: ChoiceOption { id: "back_forth", label: "Back and forth" }, shape: Shape::Steps(&[1.0, -1.0, 1.0, -1.0]) },
    // Two triangles meeting tip to tip across the stitching: the long stitches cross in the middle.
    Design {
        option: ChoiceOption { id: "bowtie", label: "Bowtie" },
        shape: Shape::Drawn(&[(0.0, 0.0), (0.2, 0.45), (1.2, -0.45), (1.2, 0.45), (0.2, -0.45), (0.0, 0.0)]),
    },
    // An X: one diagonal, back to its middle, then the whole other diagonal through it.
    Design {
        option: ChoiceOption { id: "cross", label: "Cross" },
        shape: Shape::Drawn(&[(0.0, 0.0), (0.3, 0.4), (1.1, -0.4), (0.7, 0.0), (1.1, 0.4), (0.3, -0.4), (0.0, 0.0)]),
    },
    // A five-pointed star (a pentagram 1.2 mm across), one of its points where the stitching ends.
    Design {
        option: ChoiceOption { id: "star", label: "Star" },
        shape: Shape::Drawn(&[(0.0, 0.0), (1.0854, -0.3527), (0.4146, 0.5706), (0.4146, -0.5706), (1.0854, 0.3527), (0.0, 0.0)]),
    },
    // The plainest loop round the stitching: a small diamond of four equal stitches.
    Design {
        option: ChoiceOption { id: "simple", label: "Simple" },
        shape: Shape::Drawn(&[(0.0, 0.0), (0.5, 0.3), (1.0, 0.0), (0.5, -0.3), (0.0, 0.0)]),
    },
    // A triangle opening ahead, its tip where the stitching ends.
    Design { option: ChoiceOption { id: "triangle", label: "Triangle" }, shape: Shape::Drawn(&[(0.0, 0.0), (1.2, 0.5), (1.2, -0.5), (0.0, 0.0)]) },
    // Zigzag across the stitching on the way out, then straight back along it.
    Design {
        option: ChoiceOption { id: "zigzag", label: "Zigzag" },
        shape: Shape::Drawn(&[(0.0, 0.0), (0.3, 0.35), (0.6, -0.35), (0.9, 0.35), (1.2, 0.0), (0.0, 0.0)]),
    },
    Design { option: ChoiceOption { id: "custom", label: "Custom" }, shape: Shape::Custom },
];

/// The lock ids and labels `lock_start` and `lock_end` offer, in Ink/Stitch's order.
pub const LOCKS: &[ChoiceOption] = &options::<{ TABLE.len() }>();

/// The locks `lock_*_scale_mm` sizes: those made of steps, and custom ones.
pub const SIZED_IN_MM: &[&str] = &sized::<{ count(true) }>(true);

/// The locks `lock_*_scale_percent` sizes: the drawn ones, and custom ones.
pub const SIZED_IN_PERCENT: &[&str] = &sized::<{ count(false) }>(false);

/// The shape of the lock `id`; the half stitch, the default, for an id that is not one (the parameter
/// accepts none, so this does not happen).
pub(crate) fn shape(id: &str) -> Shape {
    TABLE.iter().find(|d| d.option.id == id).map_or(Shape::HalfStitch, |d| d.shape)
}

/// The table's options. (`const` functions cannot use iterators yet, hence the loops below.)
const fn options<const N: usize>() -> [ChoiceOption; N] {
    let mut out = [ChoiceOption { id: "", label: "" }; N];
    let mut i = 0;
    while i < N {
        out[i] = TABLE[i].option;
        i += 1;
    }
    out
}

/// How many locks the parameter `by_mm` says sizes.
const fn count(by_mm: bool) -> usize {
    let (mut n, mut i) = (0, 0);
    while i < TABLE.len() {
        if TABLE[i].shape.sized_by(by_mm) {
            n += 1;
        }
        i += 1;
    }
    n
}

/// The ids of those locks, in the table's order.
const fn sized<const N: usize>(by_mm: bool) -> [&'static str; N] {
    let mut out = [""; N];
    let (mut n, mut i) = (0, 0);
    while i < TABLE.len() {
        if TABLE[i].shape.sized_by(by_mm) {
            out[n] = TABLE[i].option.id;
            n += 1;
        }
        i += 1;
    }
    out
}

#[cfg(test)]
mod tests {
    use stitchcraft_core::math::hypot;
    use stitchcraft_core::units::at_least;

    use super::*;

    #[test]
    fn the_size_parameters_follow_the_shapes() {
        assert_eq!(SIZED_IN_MM, ["back_forth", "custom"]);
        assert_eq!(SIZED_IN_PERCENT, ["arrow", "bowtie", "cross", "star", "simple", "triangle", "zigzag", "custom"]);
        let ids: Vec<&str> = LOCKS.iter().map(|o| o.id).collect();
        assert_eq!(ids, ["half_stitch", "arrow", "back_forth", "bowtie", "cross", "star", "simple", "triangle", "zigzag", "custom"]);
        assert_eq!(LOCKS[2].label, "Back and forth");
    }

    #[test]
    fn every_id_has_its_shape() {
        assert_eq!(shape("half_stitch"), Shape::HalfStitch);
        assert_eq!(shape("back_forth"), Shape::Steps(&[1.0, -1.0, 1.0, -1.0]));
        assert_eq!(shape("custom"), Shape::Custom);
        assert!(matches!(shape("zigzag"), Shape::Drawn(_)));
        assert_eq!(shape("spiral"), Shape::HalfStitch);
    }

    /// The drawn shapes are frozen: test sheet TS-04 sews them at machine checkpoint MC-2, so a change is
    /// deliberate and comes with the sew-out report that asks for it. Each number sums n·x + n²·y over
    /// the loop's points, n counting from 1, so any change to a coordinate moves it.
    #[test]
    fn drawn_shapes_are_frozen() {
        let fingerprint = |points: &[(f64, f64)]| -> f64 {
            points
                .iter()
                .zip(1_u32..)
                .map(|(&(x, y), n)| {
                    let n = f64::from(n);
                    n * x + n * n * y
                })
                .sum()
        };
        let drawn: Vec<(&str, f64)> =
            TABLE.iter().filter_map(|d| if let Shape::Drawn(points) = d.shape { Some((d.option.id, fingerprint(points))) } else { None }).collect();
        let frozen = [("arrow", 13.3), ("bowtie", 3.5), ("cross", 7.6), ("star", 13.9125), ("simple", 2.4), ("triangle", 3.5), ("zigzag", 15.85)];
        assert_eq!(drawn.len(), frozen.len());
        for ((id, got), (want_id, want)) in drawn.into_iter().zip(frozen) {
            assert!(id == want_id && (got - want).abs() < 1e-9, "{id}: {got}");
        }
    }

    #[test]
    fn drawn_shapes_keep_the_design_rules() {
        for design in TABLE {
            let Shape::Drawn(points) = design.shape else { continue };
            let id = design.option.id;
            assert_eq!((points.first(), points.last()), (Some(&(0.0, 0.0)), Some(&(0.0, 0.0))), "{id} is a loop from the anchor");
            for pair in points.windows(2) {
                let length = hypot(pair[1].0 - pair[0].0, pair[1].1 - pair[0].1);
                assert!(at_least(length, 0.46) && at_least(1.4, length), "{id}: a stitch of {length} mm");
            }
            assert!(points.iter().all(|&(x, y)| (0.0..=1.4).contains(&x) && y.abs() <= 0.6), "{id} stays ahead, close to the stitching");
        }
    }
}
