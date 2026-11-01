//! Conformance level L3, the Ink/Stitch differential (`docs/src/design/conformance.md` › L3): a pinned
//! Ink/Stitch, run as a black box (ADR-0001, decision 6), and StitchCraft each sew the design of an
//! `inkstitch` data case, and the machine files they write are compared by measures, not stitch by stitch.
//! The `centre-lines` cases compare a step before the stitches, the lines of narrow shapes
//! ([`super::centre_lines`]), and run Ink/Stitch as these do ([`Inkstitch`]).
//!
//! **Running Ink/Stitch.** `STITCHCRAFT_INKSTITCH` names a checkout of Ink/Stitch at the commit
//! [`PIN`] gives, and `STITCHCRAFT_INKSTITCH_PYTHON` an interpreter with `conformance/inkstitch/
//! requirements.txt` installed (`python3` when unset). Ink/Stitch's own command line,
//! `inkstitch.py --extension=output --format=pes`, writes the PES file to its standard output, run in its
//! checkout with [`STUBS`] on the module path: a stand-in for wxPython, the toolkit of Ink/Stitch's
//! dialogs, which its extensions import as they load and writing a file never uses. Nothing of Ink/Stitch's
//! is kept in this repository: the `inkstitch` action fetches it where the job runs, as
//! `conformance/inkstitch/README.md` does by hand. Without `STITCHCRAFT_INKSTITCH`, a case is skipped.
//!
//! **StitchCraft** reads the design and plans it on the reference machine, as `stitch plan` does, and
//! writes PES. Both files are then read by StitchCraft's reader, so that both are measured on the 0.1 mm
//! grid PES keeps.
//!
//! **A test sheet** drawn as a design is a case's subject too (`sheet`). StitchCraft sews the sheet, as
//! `stitch testsheet` does, and Ink/Stitch the SVG file of the elements StitchCraft sews it with, automatic
//! digitizing's for its automatic shapes, written with their parameters (`stitchcraft_testkit::inkstitch`)
//! to [`WRITTEN`], where a reviewer can open it. Ink/Stitch then sews the stitch types StitchCraft chose,
//! with StitchCraft's settings, so the stitches StitchCraft makes of its own choices are checked against
//! Ink/Stitch's (`REQ-AUTO-021`). StitchCraft's SVG reader reads no element's parameters until M8, which
//! is why StitchCraft sews the sheet itself, not the file.
//!
//! **The measures** ([`Measure`]): each file's stitches, the needle-downs; its size, the bounds of its needle
//! points; its threads, in order; its trims; its jumps; and how nearly their thread's footprints overlap
//! once their bounds' centres are made one (`stitchcraft_testkit::metrics::footprint_overlap`). Each has the
//! tolerance the design gives: stitches within 10 %, each side within 0.5 mm, the same threads in the same
//! order, trims and jumps within 1, footprints overlapping by 0.9 at least. A case may excuse a measure
//! with an entry of the deviations ledger that explains the difference: it is reported as excused, not
//! failed.
//!
//! **Explained, not only excused.** Where the differential can undo a deviation in Ink/Stitch's file
//! ([`undone`]), a measure the deviation excuses is measured again on the file undone, and the excuse holds
//! only when the measure then agrees. So an excuse can't hide a difference of another kind: Ink/Stitch's
//! jumps that go nowhere (`DEV-FMT-001`) excuse as many trims and jumps as there are of them, and no more.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::process::Command;

use stitchcraft_cli::commands::plan::Source;
use stitchcraft_core::{Budget, Point};
use stitchcraft_engine::testsheets;
use stitchcraft_plan::{FormatId, Rgb, Stitch, StitchKind, StitchPlan, profiles};
use stitchcraft_testkit::metrics::footprint_overlap;

use super::cases::Subject;
use super::runner::Outcome;
use crate::util;

/// The Ink/Stitch the cases are measured against, relative to the repository root.
pub const PIN: &str = "conformance/inkstitch/pin.toml";
/// The stand-ins put first on Ink/Stitch's module path, relative to the repository root.
pub const STUBS: &str = "conformance/inkstitch/stubs";
/// Where the SVG files written for test sheets go, relative to the repository root.
pub const WRITTEN: &str = "target/conformance/differential";

/// What is measured, by the name a case's `excused` table gives it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Measure {
    /// The needle-downs: within 10 %.
    Stitches,
    /// The bounds of the needle points, each side: within 0.5 mm.
    Size,
    /// The threads, in order: the same.
    Threads,
    /// The trims: within 1.
    Trims,
    /// The jumps: within 1.
    Jumps,
    /// The overlap of the thread's footprints: 0.9 at least.
    Overlap,
}

impl Measure {
    /// Every measure, in the order the report gives them.
    pub const ALL: [Measure; 6] = [Measure::Stitches, Measure::Size, Measure::Threads, Measure::Trims, Measure::Jumps, Measure::Overlap];

    /// Its name in a case file and the report.
    pub fn name(self) -> &'static str {
        match self {
            Measure::Stitches => "stitches",
            Measure::Size => "size",
            Measure::Threads => "threads",
            Measure::Trims => "trims",
            Measure::Jumps => "jumps",
            Measure::Overlap => "overlap",
        }
    }

    /// The measure named `name`.
    pub fn named(name: &str) -> Option<Measure> {
        Measure::ALL.into_iter().find(|m| m.name() == name)
    }
}

/// A machine file, measured.
#[derive(Clone, Debug, PartialEq)]
pub struct Measured {
    stitches: usize,
    size: [f64; 2],
    threads: Vec<Rgb>,
    trims: usize,
    jumps: usize,
    /// Each stitch's thread, from a needle point to the next, its bounds' centre at the origin.
    sewn: Vec<[Point; 2]>,
}

impl Measured {
    /// `plan`, measured: `None` when it has no needle point.
    fn of(plan: &StitchPlan) -> Option<Measured> {
        let bounds = plan.bounds()?;
        let centre = bounds.center();
        let moved = |q: Point| Point::new(q.x() - centre.x(), q.y() - centre.y()).unwrap_or(q);
        let stats = plan.stats();
        Some(Measured {
            stitches: stats.stitches,
            size: [bounds.width(), bounds.height()],
            threads: plan.blocks.iter().map(|block| block.thread.color).collect(),
            trims: stats.trims,
            jumps: stats.jumps,
            sewn: plan.sewn_stitches().iter().map(|stitch| [moved(stitch.from), moved(stitch.to)]).collect(),
        })
    }
}

/// A measure compared: both values as the report shows them (the overlap, a measure of the pair, once), and
/// whether they lie within its tolerance.
pub type Compared = (Measure, String, String, bool);

/// How `ours` and `theirs` compare, measure by measure.
pub fn compare(ours: &Measured, theirs: &Measured) -> Vec<Compared> {
    let count = |n: usize| n.to_string();
    let size = |s: [f64; 2]| format!("{:.1} × {:.1} mm", s[0], s[1]);
    let threads = |t: &[Rgb]| t.iter().map(|c| format!("#{:02x}{:02x}{:02x}", c.r, c.g, c.b)).collect::<Vec<_>>().join(" ");
    let within = |a: usize, b: usize, by: usize| a.abs_diff(b) <= by;
    let overlap = footprint_overlap(&ours.sewn, &theirs.sewn);
    vec![
        (Measure::Stitches, count(ours.stitches), count(theirs.stitches), ours.stitches.abs_diff(theirs.stitches) * 10 <= theirs.stitches),
        (Measure::Size, size(ours.size), size(theirs.size), ours.size.iter().zip(theirs.size).all(|(a, b)| (a - b).abs() <= 0.5)),
        (Measure::Threads, threads(&ours.threads), threads(&theirs.threads), ours.threads == theirs.threads),
        (Measure::Trims, count(ours.trims), count(theirs.trims), within(ours.trims, theirs.trims, 1)),
        (Measure::Jumps, count(ours.jumps), count(theirs.jumps), within(ours.jumps, theirs.jumps, 1)),
        (Measure::Overlap, format!("{overlap:.3}"), String::new(), overlap >= 0.9),
    ]
}

/// Where Ink/Stitch is set up: its checkout and the interpreter that runs it.
pub(super) struct Inkstitch {
    checkout: PathBuf,
    python: String,
}

impl Inkstitch {
    /// Ink/Stitch as `STITCHCRAFT_INKSTITCH` sets it up, at the commit [`PIN`] gives; or `None`, with why it
    /// can't run in `outcome`: skipped when it isn't set up, failed when it is at another commit.
    pub(super) fn ready(root: &Path, outcome: &mut Outcome) -> Option<Inkstitch> {
        let Some(inkstitch) = Inkstitch::from_env() else {
            outcome.skipped = Some("Ink/Stitch is not set up: set STITCHCRAFT_INKSTITCH (conformance/inkstitch/README.md)".to_string());
            return None;
        };
        match (pinned(root), inkstitch.commit()) {
            (Ok(pin), Some(commit)) if pin == commit => Some(inkstitch),
            (Ok(pin), commit) => {
                let at = commit.as_deref().unwrap_or("no git commit");
                outcome.failures.push(format!("STITCHCRAFT_INKSTITCH is at {at}, and the cases are measured against {pin} ({PIN})"));
                None
            }
            (Err(e), _) => {
                outcome.failures.push(e);
                None
            }
        }
    }

    fn from_env() -> Option<Inkstitch> {
        let checkout = PathBuf::from(std::env::var_os("STITCHCRAFT_INKSTITCH")?);
        let python = std::env::var("STITCHCRAFT_INKSTITCH_PYTHON").unwrap_or_else(|_| "python3".to_string());
        Some(Inkstitch { checkout, python })
    }

    /// The commit checked out, when it is a git checkout.
    fn commit(&self) -> Option<String> {
        let output = Command::new("git").arg("-C").arg(&self.checkout).args(["rev-parse", "HEAD"]).output().ok()?;
        output.status.success().then(|| String::from_utf8_lossy(&output.stdout).trim().to_string())
    }

    /// The PES file Ink/Stitch writes for the SVG file `design`.
    fn pes(&self, root: &Path, design: &Path) -> Result<Vec<u8>, String> {
        self.output(root, &["--extension=output", "--format=pes"], design, "PES file")
    }

    /// What Ink/Stitch's command line writes to its standard output, `what`, run with `arguments` on the SVG
    /// file `design`: an extension's result.
    pub(super) fn output(&self, root: &Path, arguments: &[&str], design: &Path, what: &str) -> Result<Vec<u8>, String> {
        let output = Command::new(&self.python)
            .arg("inkstitch.py")
            .args(arguments)
            .arg(design)
            .current_dir(&self.checkout)
            .env("PYTHONPATH", root.join(STUBS))
            // Nothing written into the repository's stand-ins.
            .env("PYTHONDONTWRITEBYTECODE", "1")
            .output()
            .map_err(|e| format!("{}: {e}", self.python))?;
        if !output.status.success() || output.stdout.is_empty() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            let tail: Vec<&str> = stderr.lines().rev().take(8).collect();
            return Err(format!("Ink/Stitch wrote no {what} ({}): {}", output.status, tail.into_iter().rev().collect::<Vec<_>>().join(" / ")));
        }
        Ok(output.stdout)
    }
}

/// The commit [`PIN`] gives.
pub fn pinned(root: &Path) -> Result<String, String> {
    let pin: toml::Table = toml::from_str(&util::read(&root.join(PIN))?).map_err(|e| format!("{PIN}: {e}"))?;
    pin.get("commit").and_then(toml::Value::as_str).map(str::to_string).ok_or_else(|| format!("{PIN}: no `commit`"))
}

/// Runs the case that sews `subject`, with the measures `excused` excuses by a deviation's id: what differs
/// goes to `outcome`'s failures, or, excused, to its notes, and each measure to its details, a row of the
/// report's table. A deviation excusing a measure that agrees fails the case too, so that the ledger's
/// reasons stay true.
pub fn run(root: &Path, subject: &Subject, excused: &BTreeMap<String, String>, outcome: &mut Outcome) {
    let Some(inkstitch) = Inkstitch::ready(root, outcome) else { return };
    let (theirs, ours) = match subject {
        Subject::File(design) => {
            let path = root.join("conformance").join(design);
            (inkstitch.pes(root, &path), sewn(&path, design))
        }
        Subject::Sheet(id) => match sheet(root, id) {
            Ok((path, plan)) => (inkstitch.pes(root, &path), encoded(&plan, id)),
            Err(e) => return outcome.failures.push(e),
        },
        // `load_cases` draws letters for centre-lines cases alone.
        Subject::Letters(_) => return outcome.failures.push("an inkstitch case sews a design or a test sheet, not letters".to_string()),
    };
    let measured = |bytes: &[u8], who: &str| -> Result<(StitchPlan, Measured), String> {
        let plan = stitchcraft_formats::decode(bytes).map_err(|e| format!("{who}'s PES file doesn't read: {e}"))?.plan;
        let measured = Measured::of(&plan).ok_or_else(|| format!("{who}'s PES file has no needle point"))?;
        Ok((plan, measured))
    };
    let theirs = theirs.and_then(|bytes| measured(&bytes, "Ink/Stitch"));
    let ours = ours.and_then(|bytes| measured(&bytes, "StitchCraft"));
    let ((_, ours), (their_plan, theirs)) = match (ours, theirs) {
        (Ok(ours), Ok(theirs)) => (ours, theirs),
        (ours, theirs) => {
            outcome.failures.extend(ours.err().into_iter().chain(theirs.err()));
            return;
        }
    };
    // Each deviation an excuse names, once, undone in Ink/Stitch's file where the differential can undo it,
    // and the file measured again.
    let again: BTreeMap<String, Vec<Compared>> = excused
        .values()
        .collect::<BTreeSet<_>>()
        .into_iter()
        .filter_map(|deviation| Some((deviation.clone(), compare(&ours, &Measured::of(&undone(deviation, &their_plan)?)?))))
        .collect();
    judge(compare(&ours, &theirs), excused, &again, outcome);
}

/// The deviation by which pystitch, writing Ink/Stitch's PES files, writes jumps that go nowhere.
pub const NOWHERE: &str = "DEV-FMT-001";

/// Ink/Stitch's file `plan` with the deviation `deviation` undone, as the file would read had Ink/Stitch
/// done as StitchCraft does; `None` for a deviation the differential can't undo, which excuses its
/// measures as they are.
///
/// - [`NOWHERE`]: each jump that goes nowhere at the start of a colour block or right after a stop, where
///   Ink/Stitch's plan has no trim, and the trim it carries, as every jump of a PES file does. StitchCraft
///   writes no such record (`REQ-FMT-010`).
pub fn undone(deviation: &str, plan: &StitchPlan) -> Option<StitchPlan> {
    (deviation == NOWHERE).then(|| without_jumps_nowhere(plan))
}

/// `plan` without the jumps that go nowhere at the start of a colour block or right after a stop, nor the
/// trims they carry ([`undone`]).
fn without_jumps_nowhere(plan: &StitchPlan) -> StitchPlan {
    let mut plan = plan.clone();
    let mut needle = None;
    for block in &mut plan.blocks {
        let mut kept: Vec<Stitch> = Vec::with_capacity(block.stitches.len());
        for &stitch in &block.stitches {
            if stitch.kind == StitchKind::Jump && needle == Some(stitch.at) {
                // The trim a PES file's jump carries, and what comes before it.
                let carried = kept.last().is_some_and(|before| before.kind == StitchKind::Trim);
                let before = kept.len().saturating_sub(usize::from(carried));
                if kept.get(..before).and_then(<[_]>::last).is_none_or(|before| before.kind == StitchKind::Stop) {
                    kept.truncate(before);
                    continue;
                }
            }
            if !stitch.kind.is_command() {
                needle = Some(stitch.at);
            }
            kept.push(stitch);
        }
        block.stitches = kept;
    }
    plan
}

/// Judges the `compared` measures, with those `excused` excuses, and the measures compared `again` with
/// each deviation undone that the differential can undo: what differs goes to `outcome`'s failures, or,
/// excused, to its notes, an excuse for a measure that agrees to its failures, and so does one whose
/// deviation, undone, leaves the measure differing. Each measure goes to its details, a row of the report's
/// table.
fn judge(compared: Vec<Compared>, excused: &BTreeMap<String, String>, again: &BTreeMap<String, Vec<Compared>>, outcome: &mut Outcome) {
    for (measure, mine, other, within) in compared {
        let name = measure.name();
        let mark = match (within, excused.get(name)) {
            (true, None) => "✅".to_string(),
            (false, Some(deviation)) => match again.get(deviation).and_then(|rows| rows.iter().find(|row| row.0 == measure)) {
                None => {
                    outcome.notes.push(format!("{name} differ, as {deviation} says"));
                    format!("excused by `{deviation}`")
                }
                Some((_, mine, undone, true)) => {
                    outcome.notes.push(format!("{name} differ, as {deviation} says, and agree without it: StitchCraft {mine}, Ink/Stitch {undone}"));
                    format!("explained by `{deviation}`: Ink/Stitch {undone} without it")
                }
                Some((_, mine, undone, false)) => {
                    outcome
                        .failures
                        .push(format!("{name} differ by more than {deviation} explains: without it, StitchCraft {mine}, Ink/Stitch {undone}"));
                    format!("❌ Ink/Stitch {undone} without `{deviation}`")
                }
            },
            (false, None) => {
                outcome.failures.push(format!("{name} differ: StitchCraft {mine}, Ink/Stitch {other}"));
                "❌".to_string()
            }
            (true, Some(deviation)) => {
                outcome.failures.push(format!("{name} agree, and {deviation} excuses a difference no longer there: drop it from the case"));
                "❌ agree".to_string()
            }
        };
        outcome.details.push(format!("| {name} | {mine} | {other} | {mark} |"));
    }
}

/// The PES file StitchCraft writes for the design at `path`, planned on the reference machine as `stitch
/// plan` plans it, labelled by its name `file`.
fn sewn(path: &Path, file: &str) -> Result<Vec<u8>, String> {
    let bytes = std::fs::read(path).map_err(|e| format!("{file}: {e}"))?;
    let (design, _) = Source::of(path).read(&bytes, stitchcraft_engine::design::Reading::default()).map_err(|e| format!("{file}: {e}"))?;
    let planned = stitchcraft_engine::plan(&design, profiles::REFERENCE, &Budget::DEFAULT);
    let plan = planned.plan.ok_or_else(|| format!("{file}: StitchCraft planned nothing: {:?}", planned.diagnostics))?;
    encoded(&plan, path.file_stem().and_then(|s| s.to_str()).unwrap_or(file))
}

/// `plan` as a PES file, labelled `name`.
fn encoded(plan: &StitchPlan, name: &str) -> Result<Vec<u8>, String> {
    stitchcraft_formats::encode(plan, FormatId::PesV1, name).map(|encoded| encoded.bytes).map_err(|e| format!("{name}: {e}"))
}

/// The test sheet `id` as each program is given it: the SVG file for Ink/Stitch, written to [`WRITTEN`], of
/// the elements StitchCraft sews the sheet with, automatic digitizing's for its automatic shapes, with their
/// parameters; and StitchCraft's plan of the sheet, on the reference machine, as `stitch testsheet` writes it.
fn sheet(root: &Path, id: &str) -> Result<(PathBuf, StitchPlan), String> {
    let sheet = testsheets::find(id).ok_or_else(|| format!("there is no test sheet `{id}`"))?;
    let design =
        sheet.design().ok_or_else(|| format!("{id} is drawn stitch by stitch, and is no design to write"))?.map_err(|e| format!("{id}: {e}"))?;
    let (elements, _) = stitchcraft_engine::auto::digitized(&design, &Budget::DEFAULT);
    let svg = stitchcraft_testkit::inkstitch::svg(&elements, &design.settings).map_err(|e| format!("{id}: {e}"))?;
    let path = root.join(WRITTEN).join(format!("{}.svg", id.to_ascii_lowercase()));
    let written = path.parent().map_or(Ok(()), std::fs::create_dir_all).and_then(|()| std::fs::write(&path, svg));
    written.map_err(|e| format!("{}: {e}", util::rel(&path)))?;
    Ok((path, sheet.plan().map_err(|e| format!("{id}: {e}"))?))
}

/// The differential report, Markdown: each case's measures for both programs, failures first. `commit` is
/// StitchCraft's, and `pin` Ink/Stitch's.
pub fn report(outcomes: &[Outcome], commit: &str, pin: &str) -> String {
    let short = pin.get(..7).unwrap_or(pin);
    let mut out = format!("# Ink/Stitch differential\n\nStitchCraft at `{commit}`, against Ink/Stitch at `{short}` ({PIN}).\n\n");
    let mut order: Vec<&Outcome> = outcomes.iter().collect();
    order.sort_by_key(|o| (!o.failed(), o.skipped.is_some()));
    for o in order {
        let mark = if o.skipped.is_some() {
            "⏭"
        } else if o.passed() {
            "✅"
        } else {
            "❌"
        };
        out.push_str(&format!("## {mark} `{}` ({})\n\n", o.case, o.file));
        for line in o.failures.iter().map(|f| format!("- ❌ {f}")).chain(o.notes.iter().map(|n| format!("- excused: {n}"))) {
            out.push_str(&line);
            out.push('\n');
        }
        if let Some(why) = &o.skipped {
            out.push_str(&format!("- skipped: {why}\n"));
        }
        if !o.details.is_empty() {
            if !(o.failures.is_empty() && o.notes.is_empty() && o.skipped.is_none()) {
                out.push('\n');
            }
            out.push_str("| Measure | StitchCraft | Ink/Stitch | |\n|---|---|---|---|\n");
            for detail in &o.details {
                out.push_str(detail);
                out.push('\n');
            }
        }
        out.push('\n');
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn measured(stitches: usize, size: [f64; 2], threads: &[Rgb], trims: usize, jumps: usize, sewn: &[[Point; 2]]) -> Measured {
        Measured { stitches, size, threads: threads.to_vec(), trims, jumps, sewn: sewn.to_vec() }
    }

    fn p(x: f64, y: f64) -> Point {
        Point::new(x, y).unwrap()
    }

    #[test]
    fn each_measure_has_its_tolerance() {
        let (red, blue) = (Rgb::new(237, 23, 31), Rgb::new(11, 61, 145));
        let row = [[p(0.0, 0.0), p(10.0, 0.0)]];
        let theirs = measured(100, [50.0, 30.0], &[red, blue], 2, 3, &row);
        let within = |ours: &Measured| compare(ours, &theirs).into_iter().map(|(m, .., ok)| (m, ok)).collect::<Vec<_>>();
        let all = |ok: [bool; 6]| Measure::ALL.into_iter().zip(ok).collect::<Vec<_>>();
        // At the edge of each tolerance: 10 stitches of 100, 0.5 mm, a trim and a jump.
        assert_eq!(within(&measured(110, [50.5, 29.5], &[red, blue], 3, 2, &row)), all([true; 6]));
        assert_eq!(within(&measured(90, [49.5, 30.5], &[red, blue], 1, 4, &row)), all([true; 6]));
        // Just past each.
        let past = measured(111, [50.0, 30.6], &[blue, red], 4, 1, &[[p(0.0, 5.0), p(10.0, 5.0)]]);
        assert_eq!(within(&past), all([false; 6]));
        assert_eq!(within(&measured(89, [50.6, 30.0], &[red], 2, 5, &row)), all([false, false, false, true, false, true]));
    }

    #[test]
    fn a_plan_is_measured_by_its_needle_downs_bounds_threads_trims_and_jumps() {
        use stitchcraft_plan::{PlanBuilder, Provenance, Role, Thread};
        let (red, blue) = (Rgb::new(237, 23, 31), Rgb::new(11, 61, 145));
        let mut builder = PlanBuilder::new(Thread::new(red));
        let top = Provenance::plan(Role::Top);
        for (x, y) in [(0.0, 0.0), (10.0, 0.0), (10.0, 4.0)] {
            builder.stitch(p(x, y), top);
        }
        builder.trim(None);
        builder.change_thread(Thread::new(blue));
        builder.jump(p(20.0, 4.0), Provenance::plan(Role::Travel));
        builder.stitch(p(20.0, 4.0), top);
        builder.stitch(p(20.0, 8.0), top);
        let measured = Measured::of(&builder.finish()).unwrap();
        assert_eq!(
            (measured.stitches, measured.size, measured.threads.clone(), measured.trims, measured.jumps),
            (5, [20.0, 8.0], vec![red, blue], 1, 1)
        );
        // The thread runs from each needle point to the next, its bounds' centre, (10, 4), at the origin.
        assert_eq!(measured.sewn.first(), Some(&[p(-10.0, -4.0), p(0.0, -4.0)]));
    }

    fn outcome(case: &str) -> Outcome {
        Outcome {
            case: case.into(),
            kind: "data",
            file: format!("conformance/cases/inkstitch/{case}.toml"),
            requirements: vec!["REQ-SVG-008".into()],
            failures: Vec::new(),
            outputs: Vec::new(),
            skipped: None,
            notes: Vec::new(),
            details: Vec::new(),
        }
    }

    #[test]
    fn a_difference_fails_unless_a_deviation_excuses_it_and_an_excuse_needs_a_difference() {
        let row = |measure: Measure, within: bool| (measure, "1".to_string(), "2".to_string(), within);
        let excused = BTreeMap::from([("threads".to_string(), "DEV-THREAD-001".to_string()), ("trims".to_string(), "DEV-X-001".to_string())]);
        let mut judged = outcome("case");
        let compared = vec![row(Measure::Stitches, true), row(Measure::Size, false), row(Measure::Threads, false), row(Measure::Trims, true)];
        judge(compared, &excused, &BTreeMap::new(), &mut judged);
        assert_eq!(
            judged.failures,
            ["size differ: StitchCraft 1, Ink/Stitch 2", "trims agree, and DEV-X-001 excuses a difference no longer there: drop it from the case"]
        );
        assert_eq!(judged.notes, ["threads differ, as DEV-THREAD-001 says"]);
        assert_eq!(
            judged.details,
            [
                "| stitches | 1 | 2 | ✅ |",
                "| size | 1 | 2 | ❌ |",
                "| threads | 1 | 2 | excused by `DEV-THREAD-001` |",
                "| trims | 1 | 2 | ❌ agree |"
            ]
        );
    }

    #[test]
    fn an_excuse_whose_deviation_can_be_undone_holds_only_when_the_measure_then_agrees() {
        let row = |measure: Measure, mine: &str, other: &str, within: bool| (measure, mine.to_string(), other.to_string(), within);
        let excused = BTreeMap::from([("trims".to_string(), NOWHERE.to_string()), ("jumps".to_string(), NOWHERE.to_string())]);
        let again = BTreeMap::from([(NOWHERE.to_string(), vec![row(Measure::Trims, "30", "30", true), row(Measure::Jumps, "31", "35", false)])]);
        let mut judged = outcome("case");
        judge(vec![row(Measure::Trims, "30", "40", false), row(Measure::Jumps, "31", "41", false)], &excused, &again, &mut judged);
        assert_eq!(judged.notes, [format!("trims differ, as {NOWHERE} says, and agree without it: StitchCraft 30, Ink/Stitch 30")]);
        assert_eq!(judged.failures, [format!("jumps differ by more than {NOWHERE} explains: without it, StitchCraft 31, Ink/Stitch 35")]);
        assert_eq!(
            judged.details,
            [
                format!("| trims | 30 | 40 | explained by `{NOWHERE}`: Ink/Stitch 30 without it |"),
                format!("| jumps | 31 | 41 | ❌ Ink/Stitch 35 without `{NOWHERE}` |")
            ]
        );
    }

    #[test]
    fn jumps_that_go_nowhere_where_the_plan_has_no_trim_are_undone_with_the_trims_they_carry() {
        use stitchcraft_plan::{PlanBuilder, Provenance, Role, Thread};
        let (red, blue) = (Thread::new(Rgb::new(237, 23, 31)), Thread::new(Rgb::new(11, 61, 145)));
        let (top, travel) = (Provenance::plan(Role::Top), Provenance::plan(Role::Travel));
        // As StitchCraft reads Ink/Stitch's PES files back: every jump but the first carries a trim.
        let mut builder = PlanBuilder::new(red.clone());
        builder.jump(p(0.0, 0.0), travel);
        builder.stitch(p(0.0, 0.0), top);
        builder.stitch(p(10.0, 0.0), top);
        // A thread change where the needle is: undone.
        builder.change_thread(blue);
        builder.trim(None);
        builder.jump(p(10.0, 0.0), travel);
        builder.stitch(p(10.0, 5.0), top);
        // A stop where the needle is: undone.
        builder.stop(None);
        builder.trim(None);
        builder.jump(p(10.0, 5.0), travel);
        builder.stitch(p(10.0, 9.0), top);
        // A trim the plan asked for, and a jump that goes nowhere after it: StitchCraft writes it too.
        builder.trim(None);
        builder.jump(p(10.0, 9.0), travel);
        builder.stitch(p(14.0, 9.0), top);
        // A thread change that moves: kept.
        builder.change_thread(red);
        builder.trim(None);
        builder.jump(p(20.0, 9.0), travel);
        builder.stitch(p(20.0, 12.0), top);
        let plan = builder.finish();
        assert_eq!((plan.stats().trims, plan.stats().jumps), (4, 5));
        let without = undone(NOWHERE, &plan).unwrap();
        assert_eq!((without.stats().trims, without.stats().jumps), (2, 3));
        let kinds = |block: &stitchcraft_plan::ColorBlock| block.stitches.iter().map(|s| s.kind).collect::<Vec<_>>();
        let (n, j, t, s) = (StitchKind::Normal, StitchKind::Jump, StitchKind::Trim, StitchKind::Stop);
        assert_eq!(without.blocks.iter().map(kinds).collect::<Vec<_>>(), [vec![j, n, n], vec![n, s, n, t, j, n], vec![t, j, n]]);
        assert_eq!(without.stats().stitches, plan.stats().stitches);
        // A deviation the differential can't undo excuses as it is.
        assert_eq!(undone("DEV-THREAD-001", &plan), None);
    }

    #[test]
    fn every_deviation_the_differential_undoes_is_in_the_ledger() {
        let ledger = crate::deviations::parse(&util::read(&util::root().join(crate::deviations::LEDGER)).unwrap()).unwrap();
        assert!(ledger.iter().any(|d| d.id == NOWHERE && d.requirements.iter().any(|r| r == "REQ-FMT-010")));
    }

    #[test]
    fn the_report_puts_failed_cases_first_and_tabulates_each_case_s_measures() {
        let mut failed = outcome("failed");
        failed.failures.push("size differ".into());
        failed.details.push("| size | 1 | 2 | ❌ |".into());
        let mut passed = outcome("passed");
        passed.details.push("| size | 1 | 1 | ✅ |".into());
        let skipped = Outcome { skipped: Some("no Ink/Stitch".into()), ..outcome("skipped") };
        let report = report(&[skipped, passed, failed], "abc1234", "d59c9ab1e390285a6c67822436ffd9ba9843d8b4");
        assert!(report.contains("StitchCraft at `abc1234`, against Ink/Stitch at `d59c9ab`"), "{report}");
        let at = |what: &str| report.find(what).unwrap_or_else(|| panic!("no {what}: {report}"));
        assert!(at("## ❌ `failed`") < at("## ✅ `passed`") && at("## ✅ `passed`") < at("## ⏭ `skipped`"), "{report}");
        assert!(
            report.contains("- ❌ size differ\n\n| Measure | StitchCraft | Ink/Stitch | |\n|---|---|---|---|\n| size | 1 | 2 | ❌ |"),
            "{report}"
        );
        assert!(report.contains("## ✅ `passed` (conformance/cases/inkstitch/passed.toml)\n\n| Measure"), "{report}");
        assert!(report.contains("- skipped: no Ink/Stitch\n"), "{report}");
    }

    #[test]
    fn every_measure_has_a_name_a_case_can_give() {
        for measure in Measure::ALL {
            assert_eq!(Measure::named(measure.name()), Some(measure));
        }
        assert_eq!(Measure::named("colours"), None);
    }
}
