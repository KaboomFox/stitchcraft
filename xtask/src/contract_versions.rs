//! The compatibility contract's file versions: the commands Ink/Stitch renamed, and the changes its
//! updater makes to a file an older Ink/Stitch saved (`REQ-SVG-007`).
//!
//! Ink/Stitch numbers the files it works on, and updates a file of an older version so that it sews as it
//! did. The data file lists each change, in StitchCraft's words, with the version that brings it and the
//! milestone that applies it; the SVG adapter says which it applies already
//! ([`stitchcraft_svg::version::APPLIED`]) and which old command names it reads
//! ([`stitchcraft_svg::inkstitch::RENAMED`]). [`check`] keeps the three in step: a change the adapter
//! applies must be listed, and the renamed commands must be the same.

use std::collections::BTreeSet;

use serde::Deserialize;
use stitchcraft_svg::inkstitch::RENAMED;
use stitchcraft_svg::version::{APPLIED, CURRENT};

use crate::contract_page::DATA;

/// A command's name from before Ink/Stitch's file version 3, and the command it became.
#[derive(Deserialize)]
pub struct Renamed {
    old: String,
    new: String,
}

/// A change Ink/Stitch's updater makes to a file of an older version: the version it brings, its id, what
/// it changes, and the milestone in which StitchCraft applies it (`—` for never).
#[derive(Deserialize)]
pub struct Update {
    version: i64,
    id: String,
    change: String,
    milestone: String,
}

/// The table of the renamed commands, which follows the commands' table.
pub fn render_renamed(renamed: &[Renamed]) -> String {
    let mut md = String::from(
        "\nBefore its file version 3, Ink/Stitch named 7 commands otherwise. Its updater renames them in the files it updates, and\n\
         StitchCraft reads them as the commands they became in any file (`DEV-SVG-014`).\n\n| Old name | Command |\n|---|---|\n",
    );
    for r in renamed {
        md.push_str(&format!("| `{}` | `{}` |\n", r.old, r.new));
    }
    md
}

/// The section of the file versions.
pub fn render_updates(updates: &[Update]) -> String {
    let mut md = format!(
        "\n## File versions\n\nInk/Stitch numbers the files it works on (`inkstitch_svg_version` in the metadata, {CURRENT} at the commit above).\n\
         When it opens a file of an older version that has content of its own (an element or an attribute in its namespace, or\n\
         an `embroider_` attribute), it changes the file so that it sews as it did, version by version: a file of version 2\n\
         gets the changes of versions 3 and 4. A file with its content and no version gets them all if its user says it is an\n\
         old design. StitchCraft applies the changes that alter what it sews today as parameters of the shapes\n\
         (`REQ-SVG-007`). A file without a version it reads as it is: version 1's changes don't apply. Each row is a\n\
         change Ink/Stitch's updater makes (`lib/update.py`), in StitchCraft's words.\n\n\
         | Version | Change | StitchCraft |\n|---|---|---|\n"
    );
    for u in updates {
        let ours = APPLIED.iter().find(|(id, _)| *id == u.id).map_or_else(
            || match u.milestone.as_str() {
                "—" => "not planned".to_string(),
                milestone => format!("planned ({milestone})"),
            },
            |(_, how)| (*how).to_string(),
        );
        md.push_str(&format!("| {} | {} | {ours} |\n", u.version, u.change.replace('|', "\\|")));
    }
    md
}

/// Where the data file and the SVG adapter disagree on the file versions, one line each.
pub fn check(renamed: &[Renamed], updates: &[Update]) -> Vec<String> {
    let mut problems = Vec::new();
    let ours: Vec<(&str, &str)> = RENAMED.to_vec();
    let theirs: Vec<(&str, &str)> = renamed.iter().map(|r| (r.old.as_str(), r.new.as_str())).collect();
    if ours != theirs {
        problems.push(format!("the SVG adapter renames the commands {ours:?}, but {DATA} lists {theirs:?}"));
    }
    let mut ids = BTreeSet::new();
    for u in updates {
        if !ids.insert(u.id.as_str()) {
            problems.push(format!("{DATA} lists the update `{}` twice", u.id));
        }
        if !(1..=CURRENT).contains(&u.version) {
            problems.push(format!("the update `{}` brings version {}, which is not one from 1 to {CURRENT}", u.id, u.version));
        }
    }
    for (id, _) in APPLIED {
        if !ids.contains(id) {
            problems.push(format!("the SVG adapter applies the update `{id}`, which {DATA} does not list"));
        }
    }
    problems
}

#[cfg(test)]
mod tests {
    use super::*;

    fn renamed() -> Vec<Renamed> {
        RENAMED.iter().map(|(old, new)| Renamed { old: (*old).to_string(), new: (*new).to_string() }).collect()
    }

    fn update(version: i64, id: &str, milestone: &str) -> Update {
        Update { version, id: id.to_string(), change: format!("{id} | changes"), milestone: milestone.to_string() }
    }

    fn listed() -> Vec<Update> {
        APPLIED.iter().map(|(id, _)| update(4, id, "M3")).collect()
    }

    #[test]
    fn agreeing_lists_have_no_problems_and_render_what_the_adapter_does() {
        let mut updates = listed();
        updates.extend([update(3, "later", "M8"), update(1, "never", "—")]);
        assert_eq!(check(&renamed(), &updates), Vec::<String>::new());
        let page = render_updates(&updates);
        assert!(page.contains("| 4 | longest-fill-stitch \\| changes | applied to the shapes whose fill is sewn"), "{page}");
        assert!(page.contains("| 3 | later \\| changes | planned (M8) |"), "{page}");
        assert!(page.contains("| 1 | never \\| changes | not planned |"), "{page}");
        assert!(render_renamed(&renamed()).contains("| `fill_start` | `starting_point` |"));
    }

    #[test]
    fn every_disagreement_is_reported() {
        let mut updates = listed();
        let applied = updates.remove(0);
        updates.extend([update(0, "zero", "M8"), update(CURRENT + 1, "next", "M8"), update(2, "zero", "M8")]);
        let problems = check(&renamed()[1..], &updates);
        assert_eq!(problems.len(), 5, "{problems:?}");
        assert!(problems[0].starts_with("the SVG adapter renames the commands [(\"fill_start\""), "{problems:?}");
        assert_eq!(problems[1], "the update `zero` brings version 0, which is not one from 1 to 4");
        assert_eq!(problems[2], "the update `next` brings version 5, which is not one from 1 to 4");
        assert_eq!(problems[3], "conformance/inkstitch-params.toml lists the update `zero` twice");
        assert_eq!(
            problems[4],
            format!("the SVG adapter applies the update `{}`, which conformance/inkstitch-params.toml does not list", applied.id)
        );
    }
}
