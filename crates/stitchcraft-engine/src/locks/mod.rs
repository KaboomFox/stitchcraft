//! Lock stitches: a few small stitches where an element's stitching starts (the tie-in) and where it ends
//! (the tie-off), so the thread holds when it is trimmed or jumps on.
//!
//! Design: `docs/src/design/algorithms/locks.md`. Plan assembly (roadmap M3.8) decides which ends of which
//! groups get a lock (`ties`, `force_lock_stitches`, jumps and trims); this module sews one lock at one
//! end of a group's needle points:
//!
//! 1. **Where it lies.** The anchor is the point where the stitching starts or ends. Steps are distances
//!    along the stitching from the anchor, positive into it, so a lock of steps follows the stitching round
//!    its corners and the stitching covers it. A drawn lock lies in a frame at the anchor: x along the
//!    stitch there, pointing into the stitching, and y across it.
//! 2. **The shape** ([`LOCKS`]): the half stitch, sized from that stitch; steps, sized by
//!    `lock_*_scale_mm`; a drawn loop, sized by `lock_*_scale_percent`; or the element's custom steps.
//! 3. **No lock stitch shorter than 0.2 mm** ([`LOCK_MIN_STITCH`]), so the needle never goes back into the
//!    hole it just left: a shorter step is lengthened to it, a drawn lock enlarged until its shortest
//!    stitch is that long, and a lock of steps that a sharp turn would fold onto itself sewn straight
//!    along the first (or last) stitch (`SC-W0502`).
//! 4. **The ends.** A lock is sewn in the same order at both ends. A tie-in leads into the anchor: its
//!    steps end there, its loop starts there. A tie-off leaves from it: its steps start there, its loop
//!    comes back to it, where the thread is trimmed. That is how Ink/Stitch reads custom steps, so a file
//!    sews the same in both.

mod custom;
mod shapes;

pub use shapes::{LOCKS, SIZED_IN_MM, SIZED_IN_PERCENT};
use stitchcraft_core::math::hypot;
use stitchcraft_core::units::at_least;
use stitchcraft_core::{Code, Diagnostic, Exhausted, Meter, Point};
use stitchcraft_plan::invariants::LOCK_MIN_STITCH;

use crate::common::CommonParams;
use crate::generators::mm;
use shapes::Shape;

/// The longest stitch of the half stitch, in millimetres. Its stitches are half the first stitch, so the
/// first stitch covers them, but no longer than this, so they grip like the other locks' stitches, and
/// no shorter than [`LOCK_MIN_STITCH`].
pub const HALF_STITCH_LONGEST: f64 = 1.0;

/// The lock stitches at one end of a group.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Lock {
    /// The needle points. A tie-in's come before the group's first point, which ends the lock; a
    /// tie-off's come after its last point, which starts it. Empty when the group has no stitch to lock.
    pub points: Vec<Point>,
    /// What was changed (`SC-W0502`) or could not be sewn as set (`SC-W0503`), located at the anchor.
    /// They name no element: the caller adds it.
    pub warnings: Vec<Diagnostic>,
}

/// The tie-in for a group whose needle points are `group`, as the element's `params` set it
/// (`lock_start` and the settings that go with it). Every point costs a unit of `meter`.
pub fn tie_in(group: &[Point], params: &CommonParams, meter: &mut Meter) -> Result<Lock, Exhausted> {
    sew(group.iter().copied(), End::Start, params, meter)
}

/// The tie-off for a group whose needle points are `group`, as the element's `params` set it
/// (`lock_end` and the settings that go with it). Every point costs a unit of `meter`.
pub fn tie_off(group: &[Point], params: &CommonParams, meter: &mut Meter) -> Result<Lock, Exhausted> {
    sew(group.iter().rev().copied(), End::End, params, meter)
}

/// Which end of a group a lock secures.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum End {
    Start,
    End,
}

impl End {
    /// The end, as messages name it.
    const fn name(self) -> &'static str {
        match self {
            End::Start => "start",
            End::End => "end",
        }
    }
}

/// One end's lock settings, from the element's.
struct Settings<'p> {
    id: &'p str,
    custom: &'p str,
    scale_mm: f64,
    scale_percent: f64,
}

impl<'p> Settings<'p> {
    fn of(params: &'p CommonParams, end: End) -> Self {
        match end {
            End::Start => Settings {
                id: params.lock_start,
                custom: &params.lock_custom_start,
                scale_mm: params.lock_start_scale_mm.get(),
                scale_percent: params.lock_start_scale_percent,
            },
            End::End => Settings {
                id: params.lock_end,
                custom: &params.lock_custom_end,
                scale_mm: params.lock_end_scale_mm.get(),
                scale_percent: params.lock_end_scale_percent,
            },
        }
    }
}

/// A lock before it is placed on the stitching.
enum Planned {
    /// Distances along the stitching from the anchor, positive into it: the half stitch and steps.
    Along(Vec<f64>),
    /// Points in the frame: the drawn locks.
    Drawn(Vec<(f64, f64)>),
}

/// The lock at the start of `track`: the group's needle points from the end being locked on.
fn sew(mut track: impl Iterator<Item = Point>, end: End, params: &CommonParams, meter: &mut Meter) -> Result<Lock, Exhausted> {
    let Some(anchor) = track.next() else { return Ok(Lock::default()) };
    let mut toward = None;
    for point in track.by_ref() {
        meter.charge(1)?;
        if point != anchor {
            toward = Some(point);
            break;
        }
    }
    let Some(toward) = toward else { return Ok(Lock::default()) };
    let settings = Settings::of(params, end);
    let mut warnings = Vec::new();
    let fallback = || Planned::Along(along(&half_stitch(anchor.distance(toward)), end));
    let planned = match shapes::shape(settings.id) {
        Shape::HalfStitch => fallback(),
        Shape::Steps(units) => Planned::Along(along(&lengthened(units.iter().map(|u| u * settings.scale_mm).collect(), end, &mut warnings), end)),
        Shape::Drawn(points) => Planned::Drawn(drawn(points, settings.scale_percent, end, &mut warnings)),
        Shape::Custom => match custom::read(settings.custom, settings.scale_mm, meter)? {
            custom::Read::Steps { steps, skipped } => {
                if !skipped.is_empty() {
                    warnings.push(not_steps(end, &skipped));
                }
                if steps.is_empty() {
                    warnings.push(unusable(end, "has no steps to sew"));
                    fallback()
                } else {
                    Planned::Along(along(&lengthened(steps, end, &mut warnings), end))
                }
            }
            custom::Read::Drawn => {
                warnings.push(unusable(end, "is not written as numbers, and StitchCraft cannot sew a lock drawn as an SVG path yet"));
                fallback()
            }
        },
    };
    let frame = Frame::new(anchor, toward);
    let points = match planned {
        Planned::Drawn(planned) => {
            let mut points = Vec::with_capacity(planned.len());
            for at in planned {
                meter.charge(1)?;
                points.push(frame.at(at));
            }
            points
        }
        Planned::Along(planned) => {
            let reach = planned.iter().copied().fold(0.0, f64::max);
            let stitching = Stitching::follow(anchor, toward, track, reach, meter)?;
            let mut points = Vec::with_capacity(planned.len());
            for &x in &planned {
                meter.charge(1)?;
                points.push(stitching.at(x));
            }
            let short = shortest(&points, anchor, end);
            if at_least(short, LOCK_MIN_STITCH.get()) {
                points
            } else {
                warnings.push(straightened(end, short));
                planned.iter().map(|&x| frame.at((x, 0.0))).collect()
            }
        }
    };
    Ok(Lock { points, warnings: warnings.into_iter().map(|w| w.located(anchor)).collect() })
}

/// The stitching a lock of steps follows: the group's needle points from the anchor, without repeats, as
/// far as the lock reaches into it.
struct Stitching {
    points: Vec<Point>,
    /// How far along the stitching each point is from the anchor, in millimetres: rising, from 0.
    distances: Vec<f64>,
    /// How far the last point is.
    length: f64,
    /// The frame at the last point, along the last stitch back into the stitching: a lock that reaches
    /// past the end goes straight on.
    past_end: Frame,
}

impl Stitching {
    /// The needle points `anchor`, `toward` (another point) and those of `rest` that the lock needs to
    /// reach `reach` mm into the stitching. Every point taken from `rest` costs a unit of `meter`.
    fn follow(anchor: Point, toward: Point, mut rest: impl Iterator<Item = Point>, reach: f64, meter: &mut Meter) -> Result<Stitching, Exhausted> {
        let (mut before, mut last) = (anchor, toward);
        let (mut points, mut distances) = (vec![anchor, toward], vec![0.0, anchor.distance(toward)]);
        let mut length = anchor.distance(toward);
        while length < reach {
            let Some(point) = rest.next() else { break };
            meter.charge(1)?;
            if point != last {
                length += last.distance(point);
                points.push(point);
                distances.push(length);
                (before, last) = (last, point);
            }
        }
        Ok(Stitching { points, distances, length, past_end: Frame::new(last, before) })
    }

    /// The point `distance` mm along the stitching from the anchor. Before the anchor (a negative
    /// distance) it is straight back along the first stitch, and past the end straight on from the last.
    fn at(&self, distance: f64) -> Point {
        // The stitch the distance falls on: the last one that starts at or before it, or the first.
        let next = self.distances.partition_point(|&d| d <= distance).max(1);
        match (self.points.get(next - 1), self.points.get(next), self.distances.get(next - 1)) {
            (Some(&from), Some(&to), Some(&start)) => Frame::new(from, to).at((distance - start, 0.0)),
            _ => self.past_end.at((self.length - distance, 0.0)),
        }
    }
}

/// The shortest stitch that the lock `points` at `end` sews, joins included: a tie-in's last stitch into
/// the anchor, a tie-off's first stitch out of it.
fn shortest(points: &[Point], anchor: Point, end: End) -> f64 {
    let sewn: Vec<Point> = match end {
        End::Start => points.iter().copied().chain([anchor]).collect(),
        End::End => [anchor].into_iter().chain(points.iter().copied()).collect(),
    };
    sewn.windows(2).map(|pair| if let [a, b] = pair { a.distance(*b) } else { f64::INFINITY }).fold(f64::INFINITY, f64::min)
}

/// `SC-W0502` for a lock of steps that would sew a stitch `short` mm long where it follows a turn of the
/// stitching, and is sewn straight instead.
fn straightened(end: End, short: f64) -> Diagnostic {
    let stitch = match end {
        End::Start => "first",
        End::End => "last",
    };
    Diagnostic::new(
        Code::LockStitchLengthened,
        format!(
            "The {} lock would sew a stitch of {} mm where it follows a turn of the stitching, shorter than {} mm, so it is sewn straight along the {stitch} stitch.",
            end.name(),
            mm(short),
            mm(LOCK_MIN_STITCH.get())
        ),
    )
}

/// The half stitch's steps for a first stitch `first` mm long: forth and back over half of it, twice,
/// each step from [`LOCK_MIN_STITCH`] to [`HALF_STITCH_LONGEST`] long.
fn half_stitch(first: f64) -> [f64; 4] {
    let half = (first / 2.0).clamp(LOCK_MIN_STITCH.get(), HALF_STITCH_LONGEST);
    [half, -half, half, -half]
}

/// The needle positions of `steps` (millimetres, in sewing order, positive into the stitching), as
/// distances along the stitching from the anchor: for a tie-in, those leading into the anchor; for a
/// tie-off, those leaving it.
fn along(steps: &[f64], end: End) -> Vec<f64> {
    let mut at = 0.0;
    match end {
        End::Start => {
            // Back from the anchor: each position is the anchor less the steps still to come.
            let mut positions: Vec<f64> = steps
                .iter()
                .rev()
                .map(|step| {
                    at -= step;
                    at
                })
                .collect();
            positions.reverse();
            positions
        }
        End::End => steps
            .iter()
            .map(|step| {
                at += step;
                at
            })
            .collect(),
    }
}

/// `steps` with each one shorter than [`LOCK_MIN_STITCH`] lengthened to it, which `SC-W0502` reports.
fn lengthened(mut steps: Vec<f64>, end: End, warnings: &mut Vec<Diagnostic>) -> Vec<f64> {
    let min = LOCK_MIN_STITCH.get();
    let short: Vec<f64> = steps.iter().map(|step| step.abs()).filter(|length| !at_least(*length, min)).collect();
    let Some(shortest) = short.iter().copied().reduce(f64::min) else { return steps };
    for step in &mut steps {
        if !at_least(step.abs(), min) {
            *step = min.copysign(*step);
        }
    }
    let (end, min) = (end.name(), mm(min));
    let message = match short.len() {
        1 => format!("A step of the {end} lock would be {} mm, shorter than {min} mm, so it is lengthened to {min} mm.", mm(shortest)),
        n => format!(
            "{n} steps of the {end} lock would be shorter than {min} mm, the shortest {} mm, so they are lengthened to {min} mm.",
            mm(shortest)
        ),
    };
    warnings.push(Diagnostic::new(Code::LockStitchLengthened, message));
    steps
}

/// The needle positions in the frame of the drawn loop `points` at `percent`: for a tie-in, all but the
/// loop's last point (the anchor, where the stitching starts); for a tie-off, all but its first. A loop
/// whose shortest stitch would be shorter than [`LOCK_MIN_STITCH`] is enlarged until it is that long,
/// which `SC-W0502` reports.
fn drawn(points: &[(f64, f64)], percent: f64, end: End, warnings: &mut Vec<Diagnostic>) -> Vec<(f64, f64)> {
    let min = LOCK_MIN_STITCH.get();
    let mut scale = percent / 100.0;
    // The shapes are tested to have no stitch shorter than 0.46 mm, so the shortest is never 0.
    let shortest =
        points.windows(2).map(|pair| if let [a, b] = pair { hypot(b.0 - a.0, b.1 - a.1) } else { 0.0 }).fold(f64::INFINITY, f64::min) * scale;
    if !at_least(shortest, min) {
        let factor = min / shortest;
        scale *= factor;
        warnings.push(Diagnostic::new(
            Code::LockStitchLengthened,
            format!(
                "The {} lock's shortest stitch would be {} mm, shorter than {} mm, so the lock is sewn {} times as large.",
                end.name(),
                mm(shortest),
                mm(min),
                mm(factor)
            ),
        ));
    }
    let scaled = points.iter().map(|(x, y)| (x * scale, y * scale));
    match end {
        End::Start => scaled.take(points.len().saturating_sub(1)).collect(),
        End::End => scaled.skip(1).collect(),
    }
}

/// `SC-W0503` for a custom lock that cannot be sewn, with the reason: the half stitch is sewn instead.
fn unusable(end: End, why: &str) -> Diagnostic {
    Diagnostic::new(Code::CustomLockUnusable, format!("The custom {} lock {why}, so the half stitch is sewn instead.", end.name()))
}

/// `SC-W0503` for the pieces of a custom lock that are left out, quoting up to three.
fn not_steps(end: End, skipped: &[String]) -> Diagnostic {
    let mut quoted: Vec<String> = skipped.iter().take(3).map(|piece| format!("\"{piece}\"")).collect();
    if skipped.len() > quoted.len() {
        quoted.push("…".to_string());
    }
    let (end, quoted) = (end.name(), quoted.join(", "));
    let message = match skipped.len() {
        1 => format!("A part of the custom {end} lock is not a step it can sew ({quoted}), so it is left out."),
        n => format!("{n} parts of the custom {end} lock are not steps it can sew ({quoted}), so they are left out."),
    };
    Diagnostic::new(Code::CustomLockUnusable, message)
}

/// The frame a lock is drawn in: at `anchor`, x along the stitch towards the next needle point, y a
/// quarter turn from it.
struct Frame {
    anchor: Point,
    along: (f64, f64),
    across: (f64, f64),
}

impl Frame {
    /// The frame at `anchor`, its x axis towards `toward`, which is another point.
    fn new(anchor: Point, toward: Point) -> Frame {
        let length = anchor.distance(toward);
        let along = ((toward.x() - anchor.x()) / length, (toward.y() - anchor.y()) / length);
        Frame { anchor, along, across: (-along.1, along.0) }
    }

    /// The point `x` along and `y` across from the anchor. Locks reach at most a few metres from a point
    /// within the data model's 10 m, so it is finite; if it ever were not, the anchor is returned.
    fn at(&self, (x, y): (f64, f64)) -> Point {
        let (anchor, along, across) = (self.anchor, self.along, self.across);
        Point::new(anchor.x() + x * along.0 + y * across.0, anchor.y() + x * along.1 + y * across.1).unwrap_or(anchor)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(x: f64, y: f64) -> Point {
        Point::new(x, y).unwrap()
    }

    #[test]
    fn steps_lead_into_the_anchor_or_leave_it() {
        assert_eq!(along(&[1.0, -1.0, 1.0, -1.0], End::Start), [0.0, 1.0, 0.0, 1.0]);
        assert_eq!(along(&[1.0, -1.0, 1.0, -1.0], End::End), [1.0, 0.0, 1.0, 0.0]);
        // Steps that do not come back: a tie-in starts behind the anchor, a tie-off ends ahead of it.
        assert_eq!(along(&[2.0, -1.0], End::Start), [-1.0, 1.0]);
        assert_eq!(along(&[2.0, -1.0], End::End), [2.0, 1.0]);
    }

    #[test]
    fn the_half_stitch_is_half_the_first_stitch_within_limits() {
        assert_eq!(half_stitch(1.0), [0.5, -0.5, 0.5, -0.5]);
        assert_eq!(half_stitch(3.0), [1.0, -1.0, 1.0, -1.0]);
        assert_eq!(half_stitch(0.3), [0.2, -0.2, 0.2, -0.2]);
    }

    #[test]
    fn short_steps_are_lengthened_keeping_their_direction() {
        let mut warnings = Vec::new();
        assert_eq!(lengthened(vec![0.5, -0.1, 0.2, -0.05], End::Start, &mut warnings), [0.5, -0.2, 0.2, -0.2]);
        assert_eq!(warnings.len(), 1);
        assert_eq!(lengthened(vec![0.5, -0.5], End::Start, &mut warnings), [0.5, -0.5]);
        assert_eq!(warnings.len(), 1, "nothing to lengthen, nothing to say");
    }

    #[test]
    fn a_tie_in_loop_leaves_out_its_last_point_and_a_tie_off_its_first() {
        let triangle = [(0.0, 0.0), (1.0, 0.5), (1.0, -0.5), (0.0, 0.0)];
        let mut warnings = Vec::new();
        assert_eq!(drawn(&triangle, 100.0, End::Start, &mut warnings), [(0.0, 0.0), (1.0, 0.5), (1.0, -0.5)]);
        assert_eq!(drawn(&triangle, 200.0, End::End, &mut warnings), [(2.0, 1.0), (2.0, -1.0), (0.0, 0.0)]);
        assert!(warnings.is_empty());
    }

    #[test]
    fn the_frame_turns_with_the_stitch() {
        let frame = Frame::new(p(1.0, 1.0), p(1.0, 3.0));
        assert_eq!(frame.at((2.0, 0.0)), p(1.0, 3.0));
        assert_eq!(frame.at((0.0, 1.0)), p(0.0, 1.0), "a quarter turn from the stitch");
        let slant = Frame::new(p(0.0, 0.0), p(3.0, 4.0));
        assert_eq!(slant.at((f64::MAX, -f64::MAX)), p(0.0, 0.0), "never off the map");
    }
}
