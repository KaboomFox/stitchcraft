//! Settings every stitch type shares: lock stitches, a trim or a stop after the element, and the
//! shortest stitch and jump.
//!
//! Plan assembly and finalizing honour them (roadmap M3.7–M3.9); they are declared once, here, with
//! Ink/Stitch's names and defaults, which are the interoperability contract
//! (`conformance/inkstitch-params.toml`; `cargo xtask docs --check` compares the two).

use stitchcraft_params::{ChoiceOption, Origin, StitchType, params};

/// The lock-stitch shapes, by their Ink/Stitch ids. The ids are Ink/Stitch's; the shapes are
/// StitchCraft's own designs with the same intent (deviation `DEV-LCK-001` in `conformance/deviations.toml`).
pub const LOCKS: &[ChoiceOption] = &[
    ChoiceOption { id: "half_stitch", label: "Half stitch" },
    ChoiceOption { id: "arrow", label: "Arrow" },
    ChoiceOption { id: "back_forth", label: "Back and forth" },
    ChoiceOption { id: "bowtie", label: "Bowtie" },
    ChoiceOption { id: "cross", label: "Cross" },
    ChoiceOption { id: "star", label: "Star" },
    ChoiceOption { id: "simple", label: "Simple" },
    ChoiceOption { id: "triangle", label: "Triangle" },
    ChoiceOption { id: "zigzag", label: "Zigzag" },
    ChoiceOption { id: "custom", label: "Custom" },
];

params! {
    /// Settings every stitch type shares: lock stitches where the element's stitching starts and
    /// ends, a trim or a stop after it, and the shortest stitch and jump.
    ///
    /// StitchCraft reads and checks them today; they change the stitches as plan assembly arrives
    /// (roadmap steps M3.7 to M3.9).
    pub struct CommonParams for StitchType::ALL;

    "Lock stitches" {
        /// Where the element gets lock stitches: a few tiny stitches that stop the thread from
        /// unravelling where its stitching starts and ends.
        ties: Choice = "0", label "Lock stitches",
            options ["0" => "At the start and the end", "1" => "At the start", "2" => "At the end", "3" => "None"];

        /// Sew the end lock even when the next element starts so close that the needle would otherwise
        /// go straight on to it without one.
        force_lock_stitches: Toggle = "false", label "Always lock";

        /// The shape of the lock stitches at the start. A half stitch (back and forth along the first
        /// stitch) is the least visible.
        lock_start: Choice = "half_stitch", label "Start lock", choices LOCKS, origin Origin::InkStitchDeviates { deviation: "DEV-LCK-001" };

        /// The shape of a custom start lock: an SVG path, or back-and-forth steps written as numbers
        /// separated by spaces.
        lock_custom_start: Text = "", label "Custom start lock", when lock_start == "custom";

        /// How large a start lock made of back-and-forth steps is, in millimetres.
        lock_start_scale_mm: Length = "0.7", label "Start lock size", range (0.1, 10.0);

        /// How large a start lock drawn as a path is, in percent of the shape's own size.
        lock_start_scale_percent: Percent = "100", label "Start lock scale", range (10.0, 500.0);

        /// The shape of the lock stitches at the end.
        lock_end: Choice = "half_stitch", label "End lock", choices LOCKS, origin Origin::InkStitchDeviates { deviation: "DEV-LCK-001" };

        /// The shape of a custom end lock: an SVG path, or back-and-forth steps written as numbers
        /// separated by spaces.
        lock_custom_end: Text = "", label "Custom end lock", when lock_end == "custom";

        /// How large an end lock made of back-and-forth steps is, in millimetres.
        lock_end_scale_mm: Length = "0.7", label "End lock size", range (0.1, 10.0);

        /// How large an end lock drawn as a path is, in percent of the shape's own size.
        lock_end_scale_percent: Percent = "100", label "End lock scale", range (10.0, 500.0);
    }

    "Trims and stops" {
        /// Cut the thread after this element, even if the next one starts close by.
        trim_after: Toggle = "false", label "Trim after";

        /// Pause the machine after this element: to add an appliqué fabric, or to check the work.
        stop_after: Toggle = "false", label "Stop after";
    }

    "Shortest stitch and jump" {
        /// Stitches shorter than this are merged into their neighbours, so the needle does not
        /// hammer one hole. Empty: the larger of the document's setting and the machine's minimum.
        min_stitch_length_mm: OptionalLength = "", label "Shortest stitch", range (0.0, 10.0);

        /// A move to the next element shorter than this is sewn straight on, without lock stitches or a
        /// jump. Empty: the document's setting.
        min_jump_stitch_length_mm: OptionalLength = "", label "Shortest jump", range (0.0, 20.0);
    }
}

#[cfg(test)]
mod tests {
    use stitchcraft_params::ParamSet;

    use super::*;

    #[test]
    fn a_design_that_says_nothing_gets_ink_stitch_defaults() {
        let read = CommonParams::from_set(&ParamSet::new()).unwrap();
        assert!(read.warnings.is_empty());
        let p = read.params;
        assert_eq!((p.ties, p.force_lock_stitches, p.lock_start, p.lock_end), ("0", false, "half_stitch", "half_stitch"));
        assert_eq!((p.lock_start_scale_mm.get(), p.lock_end_scale_percent), (0.7, 100.0));
        assert_eq!((p.trim_after, p.stop_after, p.min_stitch_length_mm, p.min_jump_stitch_length_mm), (false, false, None, None));
        assert_eq!(p.lock_custom_start, "");
    }

    #[test]
    fn every_lock_shape_is_accepted_and_others_are_errors() {
        for lock in LOCKS {
            let set: ParamSet = [("lock_start", lock.id), ("lock_end", lock.id)].into_iter().collect();
            let read = CommonParams::from_set(&set).unwrap();
            assert_eq!((read.params.lock_start, read.params.lock_end), (lock.id, lock.id));
        }
        let set: ParamSet = [("lock_start", "spiral")].into_iter().collect();
        let problems = CommonParams::from_set(&set).unwrap_err();
        assert_eq!(problems.len(), 1);
        assert!(problems[0].message.starts_with("`lock_start` is \"spiral\", but it must be one of half_stitch, arrow,"));
    }
}
