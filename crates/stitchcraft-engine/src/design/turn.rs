//! The turn a design is written at: Ink/Stitch's design setting `rotate_on_export`, which turns the whole
//! design about the hoop's centre as the machine file is written, for machines that don't turn a design to
//! fit their hoop (`REQ-ASM-005`, roadmap step M8.9).
//!
//! **Which way.** Ink/Stitch passes the setting's number to its machine-file writer, pystitch (MIT), which
//! turns each point by x' = x cos a − y sin a, y' = x sin a + y cos a once the design's origin is at (0, 0)
//! (`lib/extensions/output.py` at Ink/Stitch `d59c9ab`, and pystitch 1.0.1's `EmbMatrix`). With y down, as
//! here, 90° takes a point right of the centre to below it: the design turns right, clockwise as it is
//! seen, and −90° turns it left. Ink/Stitch's preferences offer those 2 and no turn ([`Turn::CHOICES`]). Any
//! other number turns the design that many degrees, as Ink/Stitch's writer turns it: a file turned by 45°
//! and by 180° was checked so.
//!
//! **Exactly.** A whole number of quarter turns moves each coordinate to the other axis, its sign changed or
//! not, and rounds nothing, so every stitch keeps its length to the last bit. Any other angle rounds each
//! point to the nearest double, as moving the plan to the hoop's centre does, which finalize allows for
//! ([`crate::finalize`]).

use stitchcraft_core::{Point, math};

/// How far a design turns about the hoop's centre as it is written into a machine file, in degrees: right,
/// clockwise as the design is seen, for a positive angle. Ink/Stitch's design setting `rotate_on_export`.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Turn(f64);

impl Turn {
    /// The setting's name, Ink/Stitch's: the SVG adapter reads it in a file's metadata, and the Design
    /// Settings filter keeps it with a VectorCraft document.
    pub const NAME: &'static str = "rotate_on_export";
    /// What a dialog calls it.
    pub const LABEL: &'static str = "Turn on export";
    /// What it does, for people.
    pub const HELP: &'static str = "Turns the whole design about the hoop's centre as it is written into a machine file: 90 turns it \
                                    right, clockwise, and -90 left. For a machine that doesn't turn a design to fit its hoop.";
    /// No turn: the design is written as it is drawn.
    pub const NONE: Turn = Turn(0.0);
    /// A quarter turn right, clockwise as the design is seen: 90°.
    pub const RIGHT: Turn = Turn(90.0);
    /// A quarter turn left: −90°.
    pub const LEFT: Turn = Turn(-90.0);
    /// The turns a dialog offers, in the order of Ink/Stitch's preferences, each with its id, its degrees as
    /// text: none, left and right.
    pub const CHOICES: [(&'static str, Turn); 3] = [("0", Turn::NONE), ("-90", Turn::LEFT), ("90", Turn::RIGHT)];

    /// A turn of `degrees`; `None` when it is not a finite number.
    pub fn new(degrees: f64) -> Option<Turn> {
        // Adding 0 makes -0 a plain 0.
        degrees.is_finite().then_some(Turn(degrees + 0.0))
    }

    /// Its angle, in degrees.
    pub const fn degrees(self) -> f64 {
        self.0
    }

    /// Whether it turns the design at all: by anything but a whole number of full turns.
    pub fn turns(self) -> bool {
        self.0.rem_euclid(360.0) != 0.0
    }

    /// `point` turned about (0, 0): exactly, for a whole number of quarter turns. `None` when the turned
    /// point isn't finite, which a turn of a finite point never makes.
    pub fn apply(self, point: Point) -> Option<Point> {
        let (x, y) = (point.x(), point.y());
        // In [0, 360), exactly: the remainder of a division by 360 is a double too.
        let degrees = self.0.rem_euclid(360.0);
        let (x, y) = if degrees == 0.0 {
            (x, y)
        } else if degrees == 90.0 {
            (-y, x)
        } else if degrees == 180.0 {
            (-x, -y)
        } else if degrees == 270.0 {
            (y, -x)
        } else {
            let (sin, cos) = math::sin_cos(math::to_radians(degrees));
            (x * cos - y * sin, x * sin + y * cos)
        };
        Point::new(x + 0.0, y + 0.0).ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(x: f64, y: f64) -> Point {
        Point::new(x, y).unwrap()
    }

    #[test]
    fn req_asm_005_a_quarter_turn_right_takes_a_point_right_of_the_centre_below_it() {
        // y is down: right of the centre, then below it, then left, then above, each exactly.
        let q = p(3.0, 1.0);
        assert_eq!(Turn::RIGHT.apply(q), Some(p(-1.0, 3.0)));
        assert_eq!(Turn::new(180.0).unwrap().apply(q), Some(p(-3.0, -1.0)));
        assert_eq!(Turn::LEFT.apply(q), Some(p(1.0, -3.0)));
        assert_eq!(Turn::new(270.0).unwrap().apply(q), Turn::LEFT.apply(q));
        // Whole turns, either way, turn nothing.
        for degrees in [0.0, -0.0, 360.0, -720.0] {
            let turn = Turn::new(degrees).unwrap();
            assert_eq!((turn.apply(q), turn.turns()), (Some(q), false), "{degrees}");
        }
        assert!(Turn::RIGHT.turns() && Turn::new(-450.0).unwrap().turns());
        assert_eq!(Turn::new(-450.0).unwrap().apply(q), Turn::LEFT.apply(q));
    }

    #[test]
    fn req_asm_005_any_other_angle_turns_by_its_sine_and_cosine() {
        // 45° right: (1, 0) to (√½, √½), below and right of the centre; (0, 1) to (-√½, √½).
        let half = std::f64::consts::FRAC_1_SQRT_2;
        let turned = Turn::new(45.0).unwrap();
        for (from, to) in [(p(1.0, 0.0), p(half, half)), (p(0.0, 1.0), p(-half, half)), (p(2.0, 2.0), p(0.0, 2.0 * 2.0 * half))] {
            let at = turned.apply(from).unwrap();
            assert!(at.distance(to) < 1e-12, "{from:?} to {at:?}");
            assert!((at.distance(Point::ORIGIN) - from.distance(Point::ORIGIN)).abs() < 1e-12);
        }
        // 30° right: (2, 0) to (√3, 1).
        let at = Turn::new(30.0).unwrap().apply(p(2.0, 0.0)).unwrap();
        assert!(at.distance(p(3.0_f64.sqrt(), 1.0)) < 1e-12, "{at:?}");
    }

    #[test]
    fn a_turn_is_a_finite_number_of_degrees_and_minus_0_is_0() {
        assert_eq!(Turn::new(f64::NAN), None);
        assert_eq!(Turn::new(f64::INFINITY), None);
        assert!(Turn::new(-0.0).unwrap().degrees().is_sign_positive());
        assert_eq!(Turn::new(-90.0), Some(Turn::LEFT));
        assert_eq!(Turn::default(), Turn::NONE);
        // A dialog's choices are their degrees as text.
        for (id, turn) in Turn::CHOICES {
            assert_eq!(id.parse::<f64>().ok().and_then(Turn::new), Some(turn), "{id}");
        }
    }
}
