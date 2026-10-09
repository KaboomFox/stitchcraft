//! The deviations ledger (`conformance/deviations.toml`): every intended difference from Ink/Stitch's
//! behaviour, with its reason and the requirement that motivates it (`docs/src/design/conformance.md`).
//!
//! It is the one place a deviation is described. A parameter that deviates names its entry
//! (`Origin::InkStitchDeviates { deviation }`) and the reference pages show the entry's summary; the
//! Ink/Stitch differential (L3, from M8) reads the same file to report a documented difference as such.

use std::collections::BTreeSet;

use serde::Deserialize;
use stitchcraft_params::{Origin, ParamGroup};

/// The ledger, relative to the repository root.
pub const LEDGER: &str = "conformance/deviations.toml";

/// One intended difference.
#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct Deviation {
    /// `DEV-<AREA>-<NNN>`.
    pub id: String,
    /// The part of StitchCraft it concerns, such as `plan/locks`.
    pub area: String,
    /// What differs, in one sentence for people.
    pub summary: String,
    /// Why.
    pub reason: String,
    /// The requirements it serves.
    pub requirements: Vec<String>,
}

#[derive(Deserialize)]
struct Ledger {
    #[serde(default)]
    deviation: Vec<Deviation>,
}

/// The ledger's entries, from its text.
pub fn parse(text: &str) -> Result<Vec<Deviation>, String> {
    toml::from_str::<Ledger>(text).map(|ledger| ledger.deviation).map_err(|e| format!("{LEDGER}: {e}"))
}

/// What is wrong with the ledger and the registry's references to it: malformed or repeated ids,
/// requirements that do not exist, parameters naming entries that do not exist.
pub fn check(ledger: &[Deviation], requirements: &BTreeSet<String>, registry: &[&ParamGroup]) -> Vec<String> {
    let mut problems = Vec::new();
    let mut ids = BTreeSet::new();
    for d in ledger {
        if !well_formed(&d.id) {
            problems.push(format!("`{}` is not a deviation id (DEV-<AREA>-<NNN>)", d.id));
        }
        if !ids.insert(d.id.as_str()) {
            problems.push(format!("`{}` is listed twice", d.id));
        }
        if d.summary.trim().is_empty() || d.reason.trim().is_empty() || d.requirements.is_empty() {
            problems.push(format!("`{}` needs a summary, a reason and at least one requirement", d.id));
        }
        for r in d.requirements.iter().filter(|r| !requirements.contains(*r)) {
            problems.push(format!("`{}` names {r}, which is not in conformance/requirements.toml", d.id));
        }
    }
    for spec in registry.iter().flat_map(|g| g.specs) {
        if let Origin::InkStitchDeviates { deviation } = spec.origin
            && !ids.contains(deviation)
        {
            problems.push(format!("`{}` deviates as `{deviation}`, which is not in {LEDGER}", spec.key));
        }
    }
    problems
}

/// The summary of entry `id`.
pub fn summary<'a>(ledger: &'a [Deviation], id: &str) -> Option<&'a str> {
    ledger.iter().find(|d| d.id == id).map(|d| d.summary.as_str())
}

/// `DEV-LCK-001`, `DEV-FILL-TAT-006`: upper-case areas, a three-digit number.
fn well_formed(id: &str) -> bool {
    let parts: Vec<&str> = id.split('-').collect();
    let (Some(first), Some(number)) = (parts.first(), parts.last()) else { return false };
    let areas = parts.get(1..parts.len().saturating_sub(1)).unwrap_or_default();
    *first == "DEV"
        && number.len() == 3
        && number.bytes().all(|b| b.is_ascii_digit())
        && !areas.is_empty()
        && areas.iter().all(|a| !a.is_empty() && a.bytes().all(|b| b.is_ascii_uppercase()))
}

#[cfg(test)]
mod tests {
    use stitchcraft_params::{Kind, ParamSpec, Stability, StitchType};

    use super::*;

    const ENTRY: &str = "[[deviation]]\nid = \"DEV-LCK-001\"\narea = \"plan/locks\"\nsummary = \"Shapes differ.\"\nreason = \"Clean room.\"\nrequirements = [\"REQ-LCK-002\"]\n";

    const fn spec(deviation: &'static str) -> ParamSpec {
        ParamSpec {
            key: "lock_start",
            label: "Start lock",
            help: " Shape.\n",
            kind: Kind::Toggle,
            default: "false",
            group: "Locks",
            applies_to: StitchType::ALL,
            visible_when: None,
            stability: Stability::Stable,
            origin: Origin::InkStitchDeviates { deviation },
        }
    }

    #[test]
    fn the_committed_ledger_and_registry_agree() {
        let ledger = parse(&crate::util::read(&crate::util::root().join(LEDGER)).unwrap()).unwrap();
        let requirements = crate::conformance::requirement_ids().unwrap();
        assert_eq!(check(&ledger, &requirements, stitchcraft_engine::registry::PARAMETERS), Vec::<String>::new());
        assert!(summary(&ledger, "DEV-LCK-001").is_some_and(|s| s.contains("lock")));
    }

    #[test]
    fn every_problem_is_named() {
        let text = format!("{ENTRY}{}", ENTRY.replace("REQ-LCK-002", "REQ-NOPE-001").replace("Clean room.", ""));
        let ledger =
            parse(&format!("{text}[[deviation]]\nid = \"DEV-1\"\narea = \"x\"\nsummary = \"s\"\nreason = \"r\"\nrequirements = [\"REQ-LCK-002\"]\n"))
                .unwrap();
        let requirements: BTreeSet<String> = ["REQ-LCK-002".to_string()].into();
        const SPECS: &[ParamSpec] = &[spec("DEV-LCK-009")];
        let group = ParamGroup { name: "LockParams", help: " Locks.\n", applies_to: StitchType::ALL, specs: SPECS };
        assert_eq!(
            check(&ledger, &requirements, &[&group]),
            [
                "`DEV-LCK-001` is listed twice",
                "`DEV-LCK-001` needs a summary, a reason and at least one requirement",
                "`DEV-LCK-001` names REQ-NOPE-001, which is not in conformance/requirements.toml",
                "`DEV-1` is not a deviation id (DEV-<AREA>-<NNN>)",
                "`lock_start` deviates as `DEV-LCK-009`, which is not in conformance/deviations.toml",
            ]
        );
    }

    #[test]
    fn ids_have_areas_and_three_digits() {
        assert!(well_formed("DEV-LCK-001") && well_formed("DEV-FILL-TAT-006"));
        assert!(!well_formed("DEV-001") && !well_formed("DEV-lck-001") && !well_formed("DEV-LCK-01") && !well_formed("LCK-001"));
    }
}
