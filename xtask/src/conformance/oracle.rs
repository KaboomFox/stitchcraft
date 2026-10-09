//! The independent reader (REQ-FMT-005): a pinned pyembroidery reads our files, and so does StitchCraft;
//! the machine must do the same thing either way.
//!
//! pyembroidery (MIT) runs as a black box through `conformance/oracle/decode.py`, which prints the
//! stitches it reads as JSON. Both readings become plans and are compared with
//! `stitchcraft_testkit::equivalence::events` — needle-downs, cuts and pauses — after moving both so their
//! first needle-down is at the origin: readers disagree about where a design starts (pyembroidery puts a
//! PES design's top-left corner at the origin; StitchCraft keeps the machine origin), not about where its
//! stitches are relative to each other.
//!
//! The Python interpreter is `STITCHCRAFT_PYTHON` (a virtual environment with
//! `conformance/oracle/requirements.txt` installed) or `python3`. Without pyembroidery the case is
//! skipped locally and fails in CI, like every optional tool.

use std::path::Path;
use std::process::Command;

use serde::Deserialize;
use stitchcraft_core::Point;
use stitchcraft_plan::{PlanBuilder, Provenance, Rgb, Role, StitchPlan, Thread};
use stitchcraft_testkit::equivalence::{Event, events};

use super::runner::Outcome;

const SCRIPT: &str = "conformance/oracle/decode.py";

#[derive(Deserialize)]
struct Reading {
    #[serde(default)]
    stitches: Vec<(String, i32, i32)>,
    #[serde(default)]
    error: Option<String>,
}

/// Runs the oracle on `files` (relative to `conformance/`).
pub fn run(root: &Path, files: &[String], outcome: &mut Outcome) {
    let python = std::env::var("STITCHCRAFT_PYTHON").unwrap_or_else(|_| "python3".to_string());
    let available = Command::new(&python).args(["-I", "-c", "import pyembroidery"]).output().is_ok_and(|o| o.status.success());
    if !available {
        outcome.skipped =
            Some(format!("pyembroidery is not installed for `{python}` (see conformance/oracle/requirements.txt; set STITCHCRAFT_PYTHON)"));
        return;
    }
    for file in files {
        let path = root.join("conformance").join(file);
        if let Err(problem) = compare(root, &python, &path) {
            outcome.failures.push(format!("{file}: {problem}"));
        }
    }
}

fn compare(root: &Path, python: &str, path: &Path) -> Result<(), String> {
    let bytes = std::fs::read(path).map_err(|e| e.to_string())?;
    let ours = stitchcraft_formats::decode(&bytes).map_err(|e| format!("StitchCraft cannot read it: {e}"))?.plan;
    let output = Command::new(python).arg("-I").arg(root.join(SCRIPT)).arg(path).output().map_err(|e| format!("{python}: {e}"))?;
    let reading: Reading = serde_json::from_slice(&output.stdout).map_err(|e| format!("unexpected oracle output ({e})"))?;
    if let Some(error) = reading.error {
        return Err(format!("pyembroidery: {error}"));
    }
    let theirs = plan_from(&reading.stitches)?;
    let (ours, theirs) = (anchored(events(&ours)), anchored(events(&theirs)));
    if ours == theirs {
        return Ok(());
    }
    let first = ours.iter().zip(&theirs).position(|(a, b)| a != b).unwrap_or(ours.len().min(theirs.len()));
    Err(format!(
        "readers disagree at event {first}: StitchCraft {:?}, pyembroidery {:?} ({} and {} events)",
        ours.get(first),
        theirs.get(first),
        ours.len(),
        theirs.len()
    ))
}

/// A plan from pyembroidery's stitch list (0.1 mm units, y down).
fn plan_from(stitches: &[(String, i32, i32)]) -> Result<StitchPlan, String> {
    let mut builder = PlanBuilder::new(Thread::new(Rgb::new(0, 0, 0)));
    let (top, travel) = (Provenance::plan(Role::Top), Provenance::plan(Role::Travel));
    for (command, x, y) in stitches {
        let at = Point::from_tenths(*x, *y);
        match command.as_str() {
            "stitch" => builder.stitch(at, top),
            "jump" => builder.jump(at, travel),
            "trim" => builder.trim(None),
            "stop" => builder.stop(None),
            "color_change" => builder.change_thread(Thread::new(Rgb::new(0, 0, 0))),
            "end" => break,
            other => return Err(format!("unknown command `{other}`")),
        }
    }
    Ok(builder.finish())
}

/// `events` moved so the first needle-down is at the origin.
fn anchored(events: Vec<Event>) -> Vec<Event> {
    let Some(&Event::Down(x0, y0)) = events.iter().find(|e| matches!(e, Event::Down(..))) else { return events };
    events
        .into_iter()
        .map(|e| match e {
            Event::Down(x, y) => Event::Down(x - x0, y - y0),
            Event::Cut(x, y) => Event::Cut(x - x0, y - y0),
            Event::Pause(x, y) => Event::Pause(x - x0, y - y0),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn anchoring_removes_only_the_offset() {
        let moved = anchored(vec![Event::Pause(5, 5), Event::Down(10, 20), Event::Cut(13, 20), Event::Down(30, 25)]);
        assert_eq!(moved, vec![Event::Pause(-5, -15), Event::Down(0, 0), Event::Cut(3, 0), Event::Down(20, 5)]);
    }

    #[test]
    fn pyembroidery_commands_become_plan_entries() {
        let reading = vec![("jump".to_string(), 10, 0), ("stitch".to_string(), 10, 0), ("trim".to_string(), 10, 0), ("end".to_string(), 10, 0)];
        let stats = plan_from(&reading).unwrap().stats();
        assert_eq!((stats.stitches, stats.jumps, stats.trims), (1, 1, 1));
        assert!(plan_from(&[("sequin".to_string(), 0, 0)]).is_err());
    }
}
