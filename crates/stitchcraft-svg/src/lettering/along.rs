//! Lettering along a path (roadmap step M14.3, `docs/src/design/fonts.md` › Along a path, `REQ-FONT-015`
//! and `REQ-FONT-016`): where each glyph of a line sits on the path, and how far it is turned.
//!
//! **Why apart from the placing.** A seat is numbers alone: the path's length, the glyphs' boxes and the
//! position asked for. Its cases check places and angles and no stitches, as the layout's do.
//!
//! - **The path** is the first subpath of the path given, as a polyline within [`FLATNESS`] of it. A closed
//!   one goes round again after its end. An open one goes on straight past its ends, the way it ends.
//! - **A glyph** is never bent. Its box's left edge on the baseline sits on the path, and the glyph is
//!   turned the way the path runs from there to its box's right edge, so its baseline is the chord under
//!   it.
//! - **Positions.** A line starts at the path's start, or is centred on the path, or ends at its end.
//!   Stretched, it starts at the start, and what the path is longer than the line is shared out equally: a
//!   share between 2 glyphs, and one more between 2 words.

use stitchcraft_core::units::Point;
use stitchcraft_core::{Exhausted, Meter, math};
use stitchcraft_engine::design::Path;
use stitchcraft_engine::normalize::stroke::flatten;

/// How close the polyline keeps to the path, in millimetres.
pub(super) const FLATNESS: f64 = 0.01;

/// How far a line may reach past an end of its path before it is said to be longer, in millimetres: more
/// than the polyline's length can differ from the path's.
const SLACK: f64 = 1.0 / 64.0;

/// Where a text lies along its path: Ink/Stitch's `text-position`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TextPosition {
    /// Each line starts at the path's start.
    #[default]
    Left,
    /// Each line is centred on the path.
    Centre,
    /// Each line ends at the path's end.
    Right,
    /// Each line's glyphs are spread from the path's start to its end.
    Stretch,
}

impl TextPosition {
    /// Every position, in the order Ink/Stitch lists them.
    pub const ALL: [TextPosition; 4] = [TextPosition::Left, TextPosition::Centre, TextPosition::Right, TextPosition::Stretch];

    /// Its name, as Ink/Stitch's lettering along a path names it.
    pub const fn id(self) -> &'static str {
        match self {
            TextPosition::Left => "left",
            TextPosition::Centre => "center",
            TextPosition::Right => "right",
            TextPosition::Stretch => "stretch",
        }
    }

    /// The position `text` names.
    pub fn named(text: &str) -> Option<TextPosition> {
        TextPosition::ALL.into_iter().find(|position| position.id() == text)
    }
}

/// Which part of a text lies on its path: Ink/Stitch's `vertical-text-position`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum VerticalPosition {
    /// The text stands on the path: the lowest point of its last line is on it.
    #[default]
    Top,
    /// The first line's baseline is on the path.
    Baseline,
    /// The text hangs from the path: the highest point of its first line is on it.
    Bottom,
}

impl VerticalPosition {
    /// Every position, in the order Ink/Stitch lists them.
    pub const ALL: [VerticalPosition; 3] = [VerticalPosition::Top, VerticalPosition::Baseline, VerticalPosition::Bottom];

    /// Its name, as Ink/Stitch's lettering along a path names it.
    pub const fn id(self) -> &'static str {
        match self {
            VerticalPosition::Top => "top",
            VerticalPosition::Baseline => "baseline",
            VerticalPosition::Bottom => "bottom",
        }
    }

    /// The position `text` names.
    pub fn named(text: &str) -> Option<VerticalPosition> {
        VerticalPosition::ALL.into_iter().find(|position| position.id() == text)
    }
}

/// A path to set a text along, and where on it.
#[derive(Clone, Debug, PartialEq)]
pub struct Along {
    /// The path, in millimetres. Its first subpath is the one the text follows.
    pub path: Path,
    /// `text-position`: where each line lies along the path.
    pub position: TextPosition,
    /// `vertical-text-position`: which part of the text lies on the path.
    pub vertical: VerticalPosition,
}

/// A path as a polyline, with how far along it each point lies.
#[derive(Clone, Debug)]
pub(super) struct Track {
    /// Its points, no 2 in a row the same: 2 or more.
    points: Vec<Point>,
    /// How far along the polyline each point lies, from 0.
    far: Vec<f64>,
    /// Whether it closes, and goes round again after its end.
    closed: bool,
}

impl Track {
    /// The first subpath of `path`, or `None` when it has none or its first has no length. Flattening charges
    /// `meter` as [`flatten`] does, and a unit for each point.
    pub(super) fn of(path: &Path, meter: &mut Meter) -> Result<Option<Track>, Exhausted> {
        let Some(first) = path.subpaths.first() else { return Ok(None) };
        let only = Path { subpaths: vec![first.clone()] };
        let Some(points) = flatten(&only, FLATNESS, meter)?.pieces.into_iter().next().map(|piece| piece.points) else { return Ok(None) };
        meter.charge(u64::try_from(points.len()).unwrap_or(u64::MAX))?;
        let mut far = Vec::with_capacity(points.len());
        let mut length = 0.0;
        let mut before: Option<Point> = None;
        for point in &points {
            if let Some(before) = before {
                length += math::hypot(point.x() - before.x(), point.y() - before.y());
            }
            far.push(length);
            before = Some(*point);
        }
        Ok((length > 0.0).then_some(Track { points, far, closed: first.closed }))
    }

    /// Its length.
    pub(super) fn length(&self) -> f64 {
        self.far.last().copied().unwrap_or_default()
    }

    /// The segment that `far` along the path lies in or past: its start, how far along the path that lies, and
    /// the unit vector it runs in.
    fn segment(&self, far: f64) -> ((f64, f64), f64, (f64, f64)) {
        // The first point farther than `far` ends the segment, and the last segment serves past the end.
        let end = self.far.partition_point(|at| *at <= far).clamp(1, self.points.len().saturating_sub(1));
        let at = |index: usize| self.points.get(index).map_or((0.0, 0.0), |point| (point.x(), point.y()));
        let (a, b) = (at(end - 1), at(end));
        let from = self.far.get(end - 1).copied().unwrap_or_default();
        let span = self.far.get(end).copied().unwrap_or_default() - from;
        (a, from, ((b.0 - a.0) / span, (b.1 - a.1) / span))
    }

    /// `far` as the path counts it: on a closed path, within its length.
    fn wrapped(&self, far: f64) -> f64 {
        if self.closed { far.rem_euclid(self.length()) } else { far }
    }

    /// The point `far` millimetres along the path. Past an end of an open path, the point that far along the
    /// line its last segment there runs in.
    pub(super) fn point(&self, far: f64) -> (f64, f64) {
        let far = self.wrapped(far);
        let ((x, y), from, (dx, dy)) = self.segment(far);
        (x + dx * (far - from), y + dy * (far - from))
    }

    /// The unit vector the path runs in `far` millimetres along it.
    pub(super) fn direction(&self, far: f64) -> (f64, f64) {
        self.segment(self.wrapped(far)).2
    }
}

/// A glyph of a line, as its seat is found: its box, in millimetres at the scale set.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct Boxed {
    /// Its box's left edge, from the line's leftmost.
    pub left: f64,
    /// Its box's width.
    pub width: f64,
    /// Its word in the line.
    pub word: usize,
}

/// Where a glyph sits on a path.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct Seat {
    /// Where its box's left edge on the baseline sits.
    pub at: (f64, f64),
    /// The unit vector its baseline runs in.
    pub runs: (f64, f64),
}

impl Seat {
    /// How far the glyph is turned, in degrees, clockwise as a picture shows it.
    pub(super) fn angle(&self) -> f64 {
        math::to_degrees(math::atan2(self.runs.1, self.runs.0))
    }

    /// Where the point `across` millimetres along the glyph's baseline from its box's left edge and `down`
    /// below it lies.
    pub(super) fn place(&self, across: f64, down: f64) -> (f64, f64) {
        let (cos, sin) = self.runs;
        (self.at.0 + across * cos - down * sin, self.at.1 + across * sin + down * cos)
    }
}

/// The seats of a line's `glyphs` on `track` at `position`, in the order given, which is from the line's
/// left, and whether the line is longer than the path: a glyph lies past an end of it, or the line was
/// squeezed to fit.
pub(super) fn seats(track: &Track, position: TextPosition, glyphs: &[Boxed]) -> (Vec<Seat>, bool) {
    let length = track.length();
    let width = glyphs.iter().map(|glyph| glyph.left + glyph.width).fold(0.0, f64::max);
    let gaps = glyphs.windows(2).map(|pair| if matches!(pair, [a, b] if a.word != b.word) { 2 } else { 1 }).sum::<usize>();
    let share = if position == TextPosition::Stretch && gaps > 0 { (length - width) / gaps as f64 } else { 0.0 };
    let start = match position {
        TextPosition::Left | TextPosition::Stretch => 0.0,
        TextPosition::Centre => (length - width) / 2.0,
        TextPosition::Right => length - width,
    };
    let mut past = share < 0.0;
    let mut shares = 0.0;
    let mut before: Option<usize> = None;
    let mut seats = Vec::with_capacity(glyphs.len());
    for glyph in glyphs {
        shares += match before {
            None => 0.0,
            Some(word) if word == glyph.word => 1.0,
            Some(_) => 2.0,
        };
        before = Some(glyph.word);
        let from = start + glyph.left + share * shares;
        let to = from + glyph.width;
        past |= from < -SLACK || to > length + SLACK;
        let (a, b) = (track.point(from), track.point(to));
        let chord = math::hypot(b.0 - a.0, b.1 - a.1);
        let runs = if chord > 0.0 { ((b.0 - a.0) / chord, (b.1 - a.1) / chord) } else { track.direction(from) };
        seats.push(Seat { at: a, runs });
    }
    (seats, past)
}
