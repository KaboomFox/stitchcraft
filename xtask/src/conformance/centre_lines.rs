//! Conformance level L3, centre lines (`docs/src/design/conformance.md` › L3, `REQ-AUTO-022`): the lines
//! automatic digitizing sews a narrow shape along, compared with the lines a pinned Ink/Stitch's Fill to
//! Stroke draws for the same shape, run as a black box (ADR-0001, decision 6).
//!
//! **Why.** The differential's `inkstitch` cases give Ink/Stitch the stitch types StitchCraft chose, so a
//! choice that is wrong sews alike in both programs. TS-13B's small T sewed a Γ, the left arm of its bar
//! left out of its centre line, and every measure agreed. Fill to Stroke finds a shape's centre lines
//! another way, from the edges of a Voronoi diagram of its outline that lie inside it, so a branch either
//! program leaves out, or a line that strays from the middle, shows as a difference. Run on the Ts that
//! lost an arm before it was fixed, this comparison finds each missing branch.
//!
//! **What is compared.** Each shape of the case that automatic digitizing sews wholly as running stitches
//! ([`Stitches::Running`]), digitized alone: a test sheet's automatic shapes, the shapes of an SVG file
//! read as `--auto` reads it, or letters drawn by [`letter`]. StitchCraft's lines are the paths of the
//! running stitches it sews the shape with. Ink/Stitch's are the paths Fill to Stroke draws for it, with
//! its threshold at the shape's width, the widest of its lines, to the hundredth of a millimetre. Fill to
//! Stroke leaves out a branch to an end shorter than its threshold, and joins the 2 short spurs of a square
//! end into one line to the end's middle. A spur into a corner of a square end is about 0.7 of a width long,
//! and an arm of a letter reaches farther than a width past the line it leaves.
//!
//! **The measures** ([`compare`]), with `reach` the shape's width and [`SLACK`]:
//! - each end of Ink/Stitch's lines has an end of StitchCraft's within `reach`: no branch is left out;
//! - each end of StitchCraft's lies within `reach` of Ink/Stitch's lines: StitchCraft has no branch that
//!   Ink/Stitch doesn't. A stub shorter than the threshold, which Ink/Stitch leaves out, ends that near the
//!   line it leaves, as the line on into the top of an A does, which StitchCraft keeps to cover the top;
//! - farther than `reach` from their ends, each program's lines lie within [`NEAR`] widths of the
//!   other's. StitchCraft's line stops up to a width short of a round or square end (`REQ-AUTO-014`), and
//!   Ink/Stitch's runs on to the end's middle, so near the ends they differ that much.
//!
//! **The files**, in [`WRITTEN`]: the shapes compared, as the SVG file Ink/Stitch reads,
//! `centre-lines-<subject>.svg`, and each shape with both programs' lines drawn over it,
//! `centre-lines-<subject>-compared.svg`, Ink/Stitch's red and StitchCraft's blue.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use stitchcraft_cli::commands::plan::Source;
use stitchcraft_core::{Budget, Point, Rect};
use stitchcraft_engine::auto::{Stitches, digitize};
use stitchcraft_engine::design::{Design, DesignSettings, Element, Reading, Shape};
use stitchcraft_engine::normalize::stroke::{distance_to_segment, flatten};
use stitchcraft_engine::testsheets;
use stitchcraft_testkit::designs::{LETTERS, RED, automatic, fill, letter};
use stitchcraft_testkit::inkstitch;

use super::cases::{Letters, Subject};
use super::differential::{Inkstitch, WRITTEN};
use super::runner::Outcome;
use crate::util;

/// How far past a shape's width an end of one program's lines may lie from the other's, in millimetres.
pub const SLACK: f64 = 0.1;
/// How near, in widths of the shape, each program's lines lie to the other's away from their ends.
pub const NEAR: f64 = 0.25;
/// The step at which lines are measured, in millimetres.
const STEP: f64 = 0.05;
/// How closely a curve is followed when a path is measured as polylines, in millimetres.
const TOLERANCE: f64 = 0.01;
/// The room between letters drawn side by side, in millimetres.
const GAP: f64 = 2.0;

/// A shape that automatic digitizing sews wholly as running stitches.
struct Narrow {
    /// What the report calls it.
    name: String,
    /// The shape.
    element: Element,
    /// Its width: the widest of its lines.
    width: f64,
    /// StitchCraft's lines.
    lines: Vec<Vec<Point>>,
}

/// How the lines of a shape compare ([`compare`]).
#[derive(Clone, Debug, PartialEq)]
pub struct Compared {
    /// The ends of StitchCraft's lines, and of Ink/Stitch's.
    pub ends: [usize; 2],
    /// Ink/Stitch's ends with no end of StitchCraft's within reach: branches StitchCraft leaves out.
    pub missing: usize,
    /// StitchCraft's ends farther than reach from Ink/Stitch's lines: branches Ink/Stitch doesn't have.
    pub extra: usize,
    /// Farther than reach from their ends, how far StitchCraft's lines stray from Ink/Stitch's at most, and
    /// Ink/Stitch's from StitchCraft's, in widths of the shape.
    pub farthest: [f64; 2],
}

impl Compared {
    /// Whether the lines agree: no branch missing or extra, and each program's lines near the other's.
    pub fn agrees(&self) -> bool {
        self.missing == 0 && self.extra == 0 && self.farthest.iter().all(|far| *far <= NEAR)
    }

    /// What differs, for the case's failures; empty when the lines agree.
    fn differences(&self, width: f64) -> Vec<String> {
        let reach = width + SLACK;
        let mut out = Vec::new();
        if self.missing > 0 {
            out.push(format!("{} of Ink/Stitch's {} line ends have no end of StitchCraft's within {reach:.2} mm", self.missing, self.ends[1]));
        }
        if self.extra > 0 {
            out.push(format!("{} of StitchCraft's {} line ends lie farther than {reach:.2} mm from Ink/Stitch's lines", self.extra, self.ends[0]));
        }
        if self.farthest.iter().any(|far| *far > NEAR) {
            out.push(format!(
                "away from their ends, StitchCraft's lines stray {:.2} widths from Ink/Stitch's, and Ink/Stitch's {:.2} from StitchCraft's, more than {NEAR}",
                self.farthest[0], self.farthest[1]
            ));
        }
        out
    }
}

/// How StitchCraft's lines `ours` and Ink/Stitch's `theirs` of a shape `width` mm wide compare, by the
/// measures of the module's docs.
pub fn compare(ours: &[Vec<Point>], theirs: &[Vec<Point>], width: f64) -> Compared {
    let reach = width + SLACK;
    let (mine, other) = (ends(ours), ends(theirs));
    let missing = other.iter().filter(|end| !mine.iter().any(|q| q.distance(**end) <= reach)).count();
    let extra = mine.iter().filter(|end| distance(**end, theirs) > reach).count();
    let farthest = |lines: &[Vec<Point>], own: &[Point], from: &[Vec<Point>]| {
        samples(lines).into_iter().filter(|q| own.iter().all(|end| end.distance(*q) > reach)).map(|q| distance(q, from) / width).fold(0.0, f64::max)
    };
    Compared { ends: [mine.len(), other.len()], missing, extra, farthest: [farthest(ours, &mine, theirs), farthest(theirs, &other, ours)] }
}

/// The ends of `lines`: the points where one of their segments ends and no other does. A segment drawn
/// twice, as a line walked out along a branch and back retraces it, counts once. Points that round to the
/// same tenth of a micrometre are the same point.
fn ends(lines: &[Vec<Point>]) -> Vec<Point> {
    // Adding 0 makes a rounded -0 the same key as 0.
    let key = |q: Point| format!("{},{}", (q.x() * 1e4).round() + 0.0, (q.y() * 1e4).round() + 0.0);
    let mut segments = BTreeSet::new();
    let mut degrees: BTreeMap<String, (Point, usize)> = BTreeMap::new();
    for line in lines {
        for pair in line.windows(2) {
            let &[a, b] = pair else { continue };
            let (ka, kb) = (key(a), key(b));
            let segment = if ka < kb { (ka.clone(), kb.clone()) } else { (kb.clone(), ka.clone()) };
            if ka == kb || !segments.insert(segment) {
                continue;
            }
            for (k, q) in [(ka, a), (kb, b)] {
                degrees.entry(k).or_insert((q, 0)).1 += 1;
            }
        }
    }
    degrees.into_values().filter(|(_, degree)| *degree == 1).map(|(q, _)| q).collect()
}

/// How far `q` lies from the nearest of `lines`, a line of one point being that point; infinitely far
/// from none.
fn distance(q: Point, lines: &[Vec<Point>]) -> f64 {
    let mut nearest = f64::INFINITY;
    for line in lines {
        if let [only] = line.as_slice() {
            nearest = nearest.min(only.distance(q));
        }
        for pair in line.windows(2) {
            if let &[a, b] = pair {
                nearest = nearest.min(distance_to_segment(q, a, b));
            }
        }
    }
    nearest
}

/// Points along `lines`: their own, and points every [`STEP`] between them.
fn samples(lines: &[Vec<Point>]) -> Vec<Point> {
    let mut out = Vec::new();
    for line in lines {
        out.extend(line.first());
        for pair in line.windows(2) {
            let &[a, b] = pair else { continue };
            let length = a.distance(b);
            let mut along = STEP;
            while along < length {
                let t = along / length;
                out.extend(Point::new(a.x() + t * (b.x() - a.x()), a.y() + t * (b.y() - a.y())).ok());
                along += STEP;
            }
            out.push(b);
        }
    }
    out
}

/// The polylines of the stroke `element`'s path, none for a fill.
fn polylines(element: &Element) -> Result<Vec<Vec<Point>>, String> {
    let Shape::Stroke { path, .. } = &element.shape else { return Ok(Vec::new()) };
    let flat = flatten(path, TOLERANCE, &mut Budget::DEFAULT.meter())
        .map_err(|_| format!("{}: its path needs more work to measure than the budget allows", element.id))?;
    Ok(flat.pieces.into_iter().map(|piece| piece.points).collect())
}

/// Runs the centre-lines case of `subject`: each shape's lines compared goes to `outcome`'s details, a row
/// of the report's table, and a shape whose lines differ to its failures, as does a case that compares no
/// shape.
pub fn run(root: &Path, subject: &Subject, outcome: &mut Outcome) {
    let Some(inkstitch) = Inkstitch::ready(root, outcome) else { return };
    match compared(root, &inkstitch, subject) {
        Ok(rows) if rows.is_empty() => outcome.failures.push("no shape of the case sews wholly as running stitches".to_string()),
        Ok(rows) => {
            for (shape, compared) in rows {
                let mark = if compared.agrees() { "✅" } else { "❌" };
                let side = |who: usize| format!("{} ends; strays {:.2} widths", compared.ends[who], compared.farthest[who]);
                outcome.details.push(format!("| Lines of {} ({:.2} mm wide) | {} | {} | {mark} |", shape.name, shape.width, side(0), side(1)));
                let differences = compared.differences(shape.width);
                if !differences.is_empty() {
                    outcome.failures.push(format!("{}: {}", shape.name, differences.join("; ")));
                }
            }
        }
        Err(e) => outcome.failures.push(e),
    }
}

/// Each shape of `subject` that sews wholly as running stitches, with how its lines compare with the lines
/// Ink/Stitch's Fill to Stroke draws for it; and the files of the module's docs written.
fn compared(root: &Path, inkstitch: &Inkstitch, subject: &Subject) -> Result<Vec<(Narrow, Compared)>, String> {
    let Shapes { named, settings, name } = shapes(root, subject)?;
    let narrow = narrow(named, &settings)?;
    if narrow.is_empty() {
        return Ok(Vec::new());
    }
    let elements: Vec<Element> = narrow.iter().map(|shape| shape.element.clone()).collect();
    let file = root.join(WRITTEN).join(format!("centre-lines-{name}.svg"));
    write(&file, &inkstitch::svg(&elements, &settings)?)?;
    let theirs = theirs(root, inkstitch, &file, &narrow)?;
    write(&root.join(WRITTEN).join(format!("centre-lines-{name}-compared.svg")), &picture(&narrow, &theirs))?;
    Ok(narrow
        .into_iter()
        .zip(theirs)
        .map(|(shape, theirs)| {
            let compared = compare(&shape.lines, &theirs, shape.width);
            (shape, compared)
        })
        .collect())
}

/// `text` written to `path`, its folder made first.
fn write(path: &Path, text: &str) -> Result<(), String> {
    let written = path.parent().map_or(Ok(()), std::fs::create_dir_all).and_then(|()| std::fs::write(path, text));
    written.map_err(|e| format!("{}: {e}", util::rel(path)))
}

/// The automatic shapes of a case's subject ([`shapes`]).
struct Shapes {
    /// Each shape, with what the report calls it.
    named: Vec<(String, Element)>,
    /// The settings of the design they are digitized in.
    settings: DesignSettings,
    /// The subject's name in the files written.
    name: String,
}

/// The automatic shapes of `subject`.
fn shapes(root: &Path, subject: &Subject) -> Result<Shapes, String> {
    let automatic_of = |design: &Design| -> Vec<(String, Element)> {
        design.elements().iter().filter(|element| element.automatic).map(|element| (format!("`{}`", element.id), element.clone())).collect()
    };
    match subject {
        Subject::Sheet(id) => {
            let sheet = testsheets::find(id).ok_or_else(|| format!("there is no test sheet `{id}`"))?;
            let design =
                sheet.design().ok_or_else(|| format!("{id} is drawn stitch by stitch, and has no shapes"))?.map_err(|e| format!("{id}: {e}"))?;
            Ok(Shapes { named: automatic_of(&design), settings: design.settings, name: id.to_ascii_lowercase() })
        }
        Subject::File(file) => {
            let path = root.join("conformance").join(file);
            let bytes = std::fs::read(&path).map_err(|e| format!("{file}: {e}"))?;
            let reading = Reading { auto: true, sort_colours: false };
            let (design, _) = Source::of(&path).read(&bytes, reading).map_err(|e| format!("{file}: {e}"))?;
            let name = path.file_stem().and_then(|stem| stem.to_str()).unwrap_or("design").to_string();
            Ok(Shapes { named: automatic_of(&design), settings: design.settings, name })
        }
        Subject::Letters(letters) => Ok(Shapes { named: drawn(letters)?, settings: DesignSettings::default(), name: "letters".to_string() }),
    }
}

/// The letters `letters` asks for, drawn side by side as automatic shapes, each with what the report calls
/// it; or why they can't be: a letter [`letter`] doesn't draw, or a size that is no length.
pub(super) fn drawn(letters: &Letters) -> Result<Vec<(String, Element)>, String> {
    if let Some(unknown) = letters.names.chars().find(|name| !LETTERS.contains(name)) {
        let known: String = LETTERS.iter().collect();
        return Err(format!("`{unknown}` is no letter stitchcraft_testkit::designs::letter draws ({known})"));
    }
    if let Some(bad) = letters.heights_mm.iter().chain(&letters.widths_mm).find(|size| !(size.is_finite() && **size > 0.0)) {
        return Err(format!("{bad} mm is no letter's height or line width"));
    }
    let mut shapes = Vec::new();
    let mut left = 0.0;
    for name in letters.names.chars() {
        for &height in &letters.heights_mm {
            for &width in &letters.widths_mm {
                let outline: Vec<(f64, f64)> = letter(name, height, width).into_iter().map(|(x, y)| (x + left, y)).collect();
                left = outline.iter().map(|&(x, _)| x).fold(left, f64::max) + GAP;
                let element = automatic(fill(&format!("letter:{name}-{height}-{width}"), &[&outline], &RED, &[]));
                shapes.push((format!("{name} {height} mm tall in {width} mm lines"), element));
            }
        }
    }
    Ok(shapes)
}

/// The shapes of `shapes` that automatic digitizing, with the design's `settings`, sews wholly as running
/// stitches, each digitized alone, with its lines.
fn narrow(shapes: Vec<(String, Element)>, settings: &DesignSettings) -> Result<Vec<Narrow>, String> {
    let mut found = Vec::new();
    for (name, element) in shapes {
        if !matches!(element.shape, Shape::Fill { .. }) {
            continue;
        }
        let digitized = digitize(&element, settings, &mut Budget::DEFAULT.meter())
            .map_err(|_| format!("{name}: digitizing it needs more work than the budget allows"))?;
        if digitized.stitches != Some(Stitches::Running) {
            continue;
        }
        let (mut lines, mut width) = (Vec::new(), 0.0_f64);
        for sewn in &digitized.elements {
            if let Shape::Stroke { width: wide, .. } = &sewn.shape {
                width = width.max(wide.get());
            }
            lines.extend(polylines(sewn)?);
        }
        found.push(Narrow { name, element, width, lines });
    }
    Ok(found)
}

/// Ink/Stitch's lines of each of `shapes`, written in order as the SVG file `file`, its `e<index>`: the paths
/// Fill to Stroke draws for it, with its threshold at the shape's width to the hundredth of a millimetre.
/// The shapes of a width are drawn in one run.
fn theirs(root: &Path, inkstitch: &Inkstitch, file: &Path, shapes: &[Narrow]) -> Result<Vec<Vec<Vec<Point>>>, String> {
    let mut thresholds: BTreeMap<String, BTreeSet<usize>> = BTreeMap::new();
    for (index, shape) in shapes.iter().enumerate() {
        thresholds.entry(format!("{:.2}", shape.width)).or_default().insert(index);
    }
    let mut lines = vec![Vec::new(); shapes.len()];
    for (threshold, indices) in thresholds {
        let threshold = format!("--threshold_mm={threshold}");
        let ids: Vec<String> = indices.iter().map(|index| format!("--id=e{index}")).collect();
        let arguments: Vec<&str> = ["--extension=fill_to_stroke", threshold.as_str()].into_iter().chain(ids.iter().map(String::as_str)).collect();
        let bytes = inkstitch.output(root, &arguments, file, "SVG file from Fill to Stroke")?;
        let (design, _) =
            Source::Svg.read(&bytes, Reading::default()).map_err(|e| format!("the SVG file Ink/Stitch's Fill to Stroke wrote doesn't read: {e}"))?;
        for element in design.elements() {
            // Fill to Stroke draws a shape's lines as paths with the shape's id, then `_` and a number, or
            // with its id alone when there is one line; StitchCraft's reader names their strokes
            // `svg:<id>:stroke`.
            let Some(id) = element.id.as_str().strip_prefix("svg:").and_then(|id| id.strip_suffix(":stroke")) else { continue };
            let shape = id.split_once('_').map_or(id, |(shape, _)| shape);
            let Some(index) = shape.strip_prefix('e').and_then(|number| number.parse::<usize>().ok()).filter(|index| indices.contains(index)) else {
                continue;
            };
            if let Some(slot) = lines.get_mut(index) {
                slot.extend(polylines(element)?);
            }
        }
    }
    Ok(lines)
}

/// An SVG picture of `shapes` with their lines drawn over them, Ink/Stitch's `theirs` red under
/// StitchCraft's blue, in millimetres, each shape titled with its name for a viewer's tooltip.
fn picture(shapes: &[Narrow], theirs: &[Vec<Vec<Point>>]) -> String {
    let outlines = shapes.iter().flat_map(|shape| shape.element.shape.path().points());
    let bounds = Rect::around(outlines)
        .map_or([0.0, 0.0, 1.0, 1.0], |r| [r.min().x() - GAP, r.min().y() - GAP, r.width() + 2.0 * GAP, r.height() + 2.0 * GAP]);
    let [x, y, w, h] = bounds;
    let mut out = format!("<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{w}mm\" height=\"{h}mm\" viewBox=\"{x} {y} {w} {h}\">\n");
    let polyline = |line: &Vec<Point>, colour: &str, wide: f64| {
        let points: Vec<String> = line.iter().map(|q| format!("{},{}", q.x(), q.y())).collect();
        format!("    <polyline fill=\"none\" stroke=\"{colour}\" stroke-width=\"{wide}\" points=\"{}\"/>\n", points.join(" "))
    };
    for (shape, theirs) in shapes.iter().zip(theirs) {
        out.push_str(&format!("  <g>\n    <title>{}</title>\n", shape.name.replace('`', "")));
        out.push_str(&format!(
            "    <path fill=\"#e8e8e8\" stroke=\"#999\" stroke-width=\"0.02\" d=\"{}\"/>\n",
            inkstitch::data(shape.element.shape.path())
        ));
        out.extend(theirs.iter().map(|line| polyline(line, "#d00", 0.12)));
        out.extend(shape.lines.iter().map(|line| polyline(line, "#04c", 0.05)));
        out.push_str("  </g>\n");
    }
    out.push_str("</svg>\n");
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(x: f64, y: f64) -> Point {
        Point::new(x, y).unwrap()
    }

    fn lines(polylines: &[&[(f64, f64)]]) -> Vec<Vec<Point>> {
        polylines.iter().map(|line| line.iter().map(|&(x, y)| p(x, y)).collect()).collect()
    }

    /// A T's lines as StitchCraft draws them for a bar 4 mm long and a stem 4 mm tall, 0.8 mm wide: stopped
    /// short of the bar's ends and the stem's foot, walked out along the left arm and back.
    fn walked_t() -> Vec<Vec<Point>> {
        lines(&[&[(2.0, 4.0), (2.0, 0.4), (0.6, 0.4), (2.0, 0.4), (3.4, 0.4)]])
    }

    /// The same T as Fill to Stroke draws it: 3 lines from the junction, each to the middle of its end.
    fn voronoi_t() -> Vec<Vec<Point>> {
        lines(&[&[(2.0, 0.45), (0.05, 0.4)], &[(2.0, 0.45), (3.95, 0.4)], &[(2.0, 0.45), (2.0, 4.75)]])
    }

    #[test]
    fn a_line_walked_back_along_a_branch_ends_where_the_branch_does() {
        let mut found = ends(&walked_t());
        found.sort_by(|a, b| a.x().total_cmp(&b.x()));
        assert_eq!(found, [p(0.6, 0.4), p(2.0, 4.0), p(3.4, 0.4)]);
        // A loop has no end, and a line drawn twice has the ends of one.
        assert!(ends(&lines(&[&[(0.0, 0.0), (1.0, 0.0), (1.0, 1.0), (0.0, 0.0)]])).is_empty());
        assert_eq!(ends(&lines(&[&[(0.0, 0.0), (1.0, 0.0)], &[(1.0, 0.0), (0.0, 0.0)]])).len(), 2);
    }

    #[test]
    fn lines_that_differ_only_at_their_ends_agree() {
        let compared = compare(&walked_t(), &voronoi_t(), 0.8);
        assert_eq!(compared.ends, [3, 3]);
        assert!(compared.agrees(), "{compared:?}");
    }

    #[test]
    fn a_branch_left_out_is_missing() {
        // The T that sewed a Γ: no line along the left arm.
        let gamma = lines(&[&[(2.0, 4.0), (2.0, 0.4), (3.4, 0.4)]]);
        let compared = compare(&gamma, &voronoi_t(), 0.8);
        assert_eq!((compared.missing, compared.extra), (1, 0));
        assert!(compared.farthest[1] > NEAR, "{compared:?}");
        assert!(!compared.agrees());
        assert_eq!(compared.differences(0.8).len(), 2, "{:?}", compared.differences(0.8));
    }

    #[test]
    fn a_branch_ink_stitch_does_not_draw_is_extra_unless_it_is_a_stub() {
        // A bump of the outline kept as a branch 2 mm long, down from the middle of the bar.
        let mut bumped = walked_t();
        bumped.push(vec![p(1.0, 0.4), p(1.0, 2.4)]);
        let compared = compare(&bumped, &voronoi_t(), 0.8);
        assert_eq!((compared.missing, compared.extra), (0, 1));
        assert!(compared.farthest[0] > NEAR && !compared.agrees(), "{compared:?}");
        // A stub no longer than the width and the slack, as the line on into the top of an A.
        let mut stubbed = walked_t();
        stubbed.push(vec![p(1.0, 0.4), p(1.0, 1.2)]);
        assert!(compare(&stubbed, &voronoi_t(), 0.8).agrees());
    }

    #[test]
    fn a_line_off_the_middle_strays() {
        // The stem a quarter of a width and more to the side, and the line nearer the middle within it.
        let shifted = lines(&[&[(2.25, 4.0), (2.25, 0.4)], &[(0.6, 0.4), (3.4, 0.4)]]);
        let compared = compare(&shifted, &voronoi_t(), 0.8);
        assert_eq!((compared.missing, compared.extra), (0, 0));
        assert!(compared.farthest.iter().all(|far| *far > NEAR), "{compared:?}");
        let nearer = lines(&[&[(2.15, 4.0), (2.15, 0.4)], &[(0.6, 0.4), (3.4, 0.4)]]);
        assert!(compare(&nearer, &voronoi_t(), 0.8).agrees());
    }

    #[test]
    fn no_line_of_ink_stitch_is_infinitely_far() {
        let compared = compare(&walked_t(), &[], 0.8);
        assert_eq!((compared.missing, compared.extra), (0, 3));
        assert!(!compared.agrees());
    }

    #[test]
    fn letters_are_drawn_side_by_side_and_checked() {
        let letters = |names: &str, heights: &[f64]| Letters { names: names.to_string(), heights_mm: heights.to_vec(), widths_mm: vec![0.5] };
        let drawn = drawn(&letters("TH", &[3.0, 4.0])).unwrap();
        let names: Vec<&str> = drawn.iter().map(|(name, _)| name.as_str()).collect();
        assert_eq!(
            names,
            ["T 3 mm tall in 0.5 mm lines", "T 4 mm tall in 0.5 mm lines", "H 3 mm tall in 0.5 mm lines", "H 4 mm tall in 0.5 mm lines"]
        );
        let boxes: Vec<Rect> = drawn.iter().map(|(_, element)| Rect::around(element.shape.path().points()).unwrap()).collect();
        assert!(boxes.windows(2).all(|pair| pair[1].min().x() >= pair[0].max().x() + GAP - 1e-9), "{boxes:?}");
        assert!(drawn.iter().all(|(_, element)| element.automatic));
        assert!(super::drawn(&letters("A", &[3.0])).unwrap_err().contains("`A` is no letter"));
        assert!(super::drawn(&letters("T", &[0.0])).unwrap_err().contains("0 mm"));
    }

    #[test]
    fn the_narrow_letters_sew_as_running_stitches_and_the_wide_do_not() {
        let letters = Letters { names: "T".to_string(), heights_mm: vec![5.0], widths_mm: vec![0.5, 2.0] };
        let narrow = narrow(drawn(&letters).unwrap(), &DesignSettings::default()).unwrap();
        assert_eq!(narrow.len(), 1);
        let [shape] = narrow.as_slice() else { panic!() };
        assert!((shape.width - 0.5).abs() < 0.01, "{}", shape.width);
        assert_eq!(ends(&shape.lines).len(), 3, "{:?}", shape.lines);
    }
}
