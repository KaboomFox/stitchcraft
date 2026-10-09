//! `cargo xtask mutants [--record] DIR…`: the mutation-testing ratchet (`docs/src/design/guardrails.md`).
//!
//! cargo-mutants changes the code in small ways — `<` for `<=`, a function that returns its default —
//! and runs the crate's tests on each change. A mutant no test notices is *missed*: a behaviour nobody
//! checks. Running every mutant takes a while, so `mutants.yml` runs them weekly in shards; this command
//! adds up the shards' results (each DIR holds a `mutants.out/`) per crate and compares the missed counts
//! with `conformance/mutation.toml`. More missed mutants than recorded fails; `--record` lowers the
//! recorded counts to today's and never raises one (raising is a hand edit, in a pull request that says
//! why). Mutants that time out are counted apart: an endless loop is noticed, just slowly.

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::Path;

use serde::Deserialize;

use crate::util::{self, Findings};

const BASELINE: &str = "conformance/mutation.toml";
/// cargo-mutants' outcome files, by what they hold.
const OUTCOMES: [(&str, Kind); 4] =
    [("caught.txt", Kind::Caught), ("missed.txt", Kind::Missed), ("timeout.txt", Kind::Timeout), ("unviable.txt", Kind::Unviable)];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Kind {
    Caught,
    Missed,
    Timeout,
    Unviable,
}

/// Outcomes of one crate's mutants.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct Tally {
    caught: u32,
    missed: u32,
    timeout: u32,
    unviable: u32,
    /// The missed mutants, as cargo-mutants names them.
    missed_names: Vec<String>,
}

#[derive(Deserialize, Default)]
struct BaselineFile {
    #[serde(default)]
    missed: BTreeMap<String, u32>,
}

/// `cargo xtask mutants`.
pub fn run(args: &[String]) -> Result<(), String> {
    let record = args.iter().any(|a| a == "--record");
    let dirs: Vec<&String> = args.iter().filter(|a| !a.starts_with("--")).collect();
    if dirs.is_empty() {
        return Err("name the cargo-mutants output directories to add up (each holds a mutants.out/)".to_string());
    }
    let root = util::root();
    let mut tallies: BTreeMap<String, Tally> = BTreeMap::new();
    for dir in dirs {
        let out = Path::new(dir).join("mutants.out");
        for (file, kind) in OUTCOMES {
            let text = std::fs::read_to_string(out.join(file)).map_err(|e| format!("{}: {e}", out.join(file).display()))?;
            add(&mut tallies, &text, kind);
        }
    }
    let path = root.join(BASELINE);
    let mut baseline: BaselineFile = toml::from_str(&util::read(&path)?).map_err(|e| format!("{BASELINE}: {e}"))?;

    let mut findings = Findings::default();
    let mut table = String::from("| Crate | Caught | Missed | Recorded | Timeouts | Unviable |\n|---|---:|---:|---:|---:|---:|\n");
    let mut numbers = Vec::new();
    for (crate_name, t) in &tallies {
        let recorded = baseline.missed.get(crate_name).copied();
        let shown = recorded.map_or_else(|| "none".to_string(), |r| r.to_string());
        let _ = writeln!(table, "| `{crate_name}` | {} | {} | {shown} | {} | {} |", t.caught, t.missed, t.timeout, t.unviable);
        numbers.push(format!("{crate_name} {} missed", t.missed));
        match recorded {
            None if !record => findings.error(format!("{crate_name} has no recorded count in {BASELINE}: run `cargo xtask mutants --record …`")),
            Some(r) if !record && t.missed > r => findings.error(format!(
                "{crate_name}: {} mutants no test notices, {r} recorded; test the behaviour the new ones change (see the list in the job summary)",
                t.missed
            )),
            _ => {}
        }
        if record {
            let entry = baseline.missed.entry(crate_name.clone()).or_insert(t.missed);
            *entry = (*entry).min(t.missed);
        }
    }
    let mut missed_list = String::new();
    for name in tallies.values().flat_map(|t| &t.missed_names) {
        let _ = writeln!(missed_list, "- `{name}`");
    }
    println!("{table}");
    if std::env::var_os("GITHUB_ACTIONS").is_some() {
        println!("{}", util::annotation("notice", "mutation testing", &numbers.join(", ")));
        if let Some(summary) = std::env::var_os("GITHUB_STEP_SUMMARY") {
            let mut text = std::fs::read_to_string(&summary).unwrap_or_default();
            let _ = write!(text, "## Mutation testing\n\n{table}\n### Missed mutants\n\n{missed_list}");
            let _ = std::fs::write(&summary, text);
        }
    }
    if record {
        std::fs::write(&path, render(&baseline.missed)).map_err(|e| format!("{BASELINE}: {e}"))?;
        println!("recorded the missed counts in {BASELINE}");
    }
    findings.finish("mutants", &format!("{} crates at or below their recorded missed mutants", tallies.len()))
}

/// Adds the outcomes listed in `text` (one mutant per line, `path:line:col: description`).
fn add(tallies: &mut BTreeMap<String, Tally>, text: &str, kind: Kind) {
    for line in text.lines().filter(|l| !l.trim().is_empty()) {
        let path = line.split(':').next().unwrap_or_default();
        let crate_name = path.split('/').skip_while(|p| *p != "crates" && *p != "apps").nth(1).unwrap_or("other").to_string();
        let tally = tallies.entry(crate_name).or_default();
        match kind {
            Kind::Caught => tally.caught += 1,
            Kind::Missed => {
                tally.missed += 1;
                tally.missed_names.push(line.to_string());
            }
            Kind::Timeout => tally.timeout += 1,
            Kind::Unviable => tally.unviable += 1,
        }
    }
}

fn render(missed: &BTreeMap<String, u32>) -> String {
    let mut out = String::from(
        "# Mutants no test notices, per crate (`cargo xtask mutants`, docs/src/design/guardrails.md). The weekly\n\
         # mutants.yml run fails when a crate has more. `cargo xtask mutants --record …` lowers the counts to\n\
         # today's and never raises one: raise a count only by hand, in a pull request that says why.\n\n[missed]\n",
    );
    for (name, count) in missed {
        let _ = writeln!(out, "{name} = {count}");
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn outcomes_add_up_per_crate() {
        let mut tallies = BTreeMap::new();
        add(&mut tallies, "crates/stitchcraft-core/src/budget.rs:98:9: replace Meter::stitches_left -> u32 with 0\n", Kind::Missed);
        add(&mut tallies, "crates/stitchcraft-core/src/a.rs:1:1: x\napps/stitchcraft-cli/src/main.rs:2:2: y\n\n", Kind::Caught);
        add(&mut tallies, "crates/stitchcraft-plan/src/plan.rs:3:3: z\n", Kind::Timeout);
        assert_eq!(tallies["stitchcraft-core"].missed, 1);
        assert_eq!(tallies["stitchcraft-core"].caught, 1);
        assert_eq!(tallies["stitchcraft-cli"].caught, 1);
        assert_eq!(tallies["stitchcraft-plan"].timeout, 1);
        assert_eq!(
            tallies["stitchcraft-core"].missed_names,
            ["crates/stitchcraft-core/src/budget.rs:98:9: replace Meter::stitches_left -> u32 with 0"]
        );
    }

    #[test]
    fn the_baseline_renders_and_parses_back() {
        let missed = BTreeMap::from([("stitchcraft-plan".to_string(), 4), ("stitchcraft-core".to_string(), 2)]);
        let text = render(&missed);
        assert!(text.ends_with("[missed]\nstitchcraft-core = 2\nstitchcraft-plan = 4\n"));
        assert_eq!(toml::from_str::<BaselineFile>(&text).unwrap().missed, missed);
    }
}
