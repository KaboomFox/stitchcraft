//! The glyph differential's report: `target/conformance/fonts.md` for people and `fonts.json` for tools.
//!
//! The Markdown report has a row for each font, the excuses and the known differences with the glyphs each
//! covered, the stitch types listed with their glyphs, the fonts the corpus leaves out with their licences,
//! and each glyph that differs, known or not, or didn't sew, with how. The case's own rows in the differential's report are the fonts', and its failures
//! a line for each font with a glyph that differs or didn't sew, so that a run of every glyph stays readable
//! there. The JSON file has every glyph's verdict, stitch types and measures.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::path::Path;

use super::excuses::Excuses;
use super::{Options, Sewn, Verdict};
use crate::conformance::runner::Outcome;
use crate::corpus::Font;
use crate::util;

/// Counts of the verdicts.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Counts {
    agrees: usize,
    excused: usize,
    known: usize,
    differs: usize,
    listed: usize,
    failed: usize,
}

impl Counts {
    /// Counts `verdict`.
    fn add(&mut self, verdict: &Verdict) {
        match verdict {
            Verdict::Agrees => self.agrees += 1,
            Verdict::Excused(_) => self.excused += 1,
            Verdict::Known(_) => self.known += 1,
            Verdict::Differs(_) => self.differs += 1,
            Verdict::Listed(_) => self.listed += 1,
            Verdict::Failed(_) => self.failed += 1,
        }
    }

    /// Every glyph counted.
    fn total(self) -> usize {
        self.agrees + self.excused + self.known + self.differs + self.listed + self.failed
    }
}

/// What a run sewed, in words.
fn which(options: Options) -> String {
    let part = options.shard.map(|shard| format!(", part {} of {}", shard.index, shard.count)).unwrap_or_default();
    if options.every { format!("every glyph{part}") } else { format!("a sample of {} glyphs of each font{part}", options.sample) }
}

/// Writes the report and the verdicts of `sewn`, the glyphs of `fonts`, under `root`, and puts each font's row
/// in `outcome`'s details and a line for each font with a glyph that differs or didn't sew in its failures.
pub fn write(root: &Path, fonts: &[Font], sewn: &[Sewn], excuses: &Excuses, options: Options, outcome: &mut Outcome) {
    let (markdown, failures, details) = markdown(fonts, sewn, excuses, options);
    let json: Vec<serde_json::Value> = sewn.iter().map(json).collect();
    let dir = root.join("target/conformance");
    let written = std::fs::create_dir_all(&dir)
        .and_then(|()| std::fs::write(dir.join("fonts.md"), &markdown))
        .and_then(|()| std::fs::write(dir.join("fonts.json"), serde_json::to_string_pretty(&json).unwrap_or_default()));
    if let Err(e) = written {
        outcome.failures.push(format!("{}: {e}", util::rel(&dir)));
    }
    outcome.failures.extend(failures);
    outcome.details.extend(details);
    if let Some(summary) = markdown.lines().nth(2) {
        outcome.notes.push(format!("{summary} (target/conformance/fonts.md)"));
    }
}

/// The report of `sewn`, the glyphs of `fonts`; the case's failures, a line for each font with a glyph that
/// differs or didn't sew; and its details, a table of the fonts, its head first.
fn markdown(fonts: &[Font], sewn: &[Sewn], excuses: &Excuses, options: Options) -> (String, Vec<String>, Vec<String>) {
    let mut per_font: BTreeMap<&str, Counts> = BTreeMap::new();
    let mut total = Counts::default();
    let mut listed: BTreeMap<String, (usize, BTreeSet<&str>)> = BTreeMap::new();
    let mut used: BTreeMap<usize, usize> = BTreeMap::new();
    for glyph in sewn {
        per_font.entry(glyph.job.font.as_str()).or_default().add(&glyph.verdict);
        total.add(&glyph.verdict);
        if let Verdict::Listed(types) = &glyph.verdict {
            let entry = listed.entry(types.join(", ")).or_default();
            entry.0 += 1;
            entry.1.insert(glyph.job.font.as_str());
        }
        for &place in &glyph.used {
            *used.entry(place).or_default() += 1;
        }
    }
    let mut md = String::from("# Ink/Stitch's fonts in the differential\n\n");
    let _ = writeln!(
        md,
        "{}: {} glyphs of {} fonts. {} agree, {} differ as the ledger explains, {} differ as known follow-up work, {} differ otherwise, {} are listed for stitch types StitchCraft doesn't sew yet, and {} didn't sew.\n",
        which(options),
        total.total(),
        per_font.len(),
        total.agrees,
        total.excused,
        total.known,
        total.differs,
        total.listed,
        total.failed
    );
    md.push_str("| Font | Licence | Glyphs | Agree | Excused | Known | Differ | Listed | Didn't sew |\n|---|---|---|---|---|---|---|---|---|\n");
    let mut details = vec!["| Font | Glyphs | Agree, excused, known | Differ, listed, didn't sew |".to_string(), "|---|---|---|---|".to_string()];
    let mut failures = Vec::new();
    for font in fonts.iter().filter(|font| per_font.contains_key(font.name.as_str())) {
        let c = per_font.get(font.name.as_str()).copied().unwrap_or_default();
        let row = [c.total(), c.agrees, c.excused, c.known, c.differs, c.listed, c.failed].map(|n| n.to_string()).join(" | ");
        let _ = writeln!(md, "| {} | {} | {row} |", font.name, font.licence.name());
        let mark = if c.differs + c.failed == 0 { "✅" } else { "❌" };
        details.push(format!(
            "| {} | {} | {}, {}, {} | {mark} {}, {}, {} |",
            font.name,
            c.total(),
            c.agrees,
            c.excused,
            c.known,
            c.differs,
            c.listed,
            c.failed
        ));
        if c.differs + c.failed > 0 {
            failures.push(format!(
                "{}: {} of {} glyphs differ and {} didn't sew (target/conformance/fonts.md)",
                font.name,
                c.differs,
                c.total(),
                c.failed
            ));
        }
    }
    md.push_str("\n## Excuses\n\n| Deviation | Measures | For glyphs where | Glyphs excused |\n|---|---|---|---|\n");
    let row = |excuse: &super::excuses::Excuse, place: usize| {
        let measures: Vec<&str> = excuse.measures.iter().map(|m| m.name()).collect();
        format!("| {} | {} | {} | {} |", excuse.reason, measures.join(", "), excuse.conditions(), used.get(&place).copied().unwrap_or_default())
    };
    for (place, excuse) in excuses.excuses.iter().enumerate() {
        let _ = writeln!(md, "{}", row(excuse, place));
    }
    if !excuses.known.is_empty() {
        md.push_str("\n## Known differences: follow-up work\n\n| Follow-up | Measures | For glyphs where | Glyphs |\n|---|---|---|---|\n");
        for (place, known) in excuses.known.iter().enumerate() {
            let _ = writeln!(md, "{}", row(known, excuses.excuses.len() + place));
        }
    }
    if !listed.is_empty() {
        md.push_str("\n## Listed: stitch types StitchCraft doesn't sew yet\n\n| Stitch types | Glyphs | Fonts |\n|---|---|---|\n");
        for (types, (count, fonts)) in &listed {
            let _ = writeln!(md, "| {types} | {count} | {} |", fonts.iter().copied().collect::<Vec<_>>().join(", "));
        }
    }
    let left: Vec<String> =
        fonts.iter().filter(|font| !font.licence.taken()).map(|font| format!("{} ({})", font.name, font.licence.name())).collect();
    let _ = writeln!(md, "\n## Left out by their licences\n\n{}", left.join(", "));
    md.push_str("\n## Glyphs that differ or didn't sew\n\n");
    for glyph in sewn {
        let how = match &glyph.verdict {
            Verdict::Differs(how) => how.join("; "),
            Verdict::Known(how) => format!("known: {}", how.join("; ")),
            Verdict::Failed(why) => why.clone(),
            Verdict::Agrees | Verdict::Excused(_) | Verdict::Listed(_) => continue,
        };
        let job = &glyph.job;
        let _ = writeln!(md, "- {}/{} glyph {} `{}` ({}): {how}", job.font, job.file, job.index, job.name, glyph.traits.types.join(", "));
    }
    (md, failures, details)
}

/// A glyph's verdict, for tools.
fn json(glyph: &Sewn) -> serde_json::Value {
    let how = match &glyph.verdict {
        Verdict::Excused(how) | Verdict::Known(how) | Verdict::Differs(how) | Verdict::Listed(how) => how.clone(),
        Verdict::Failed(why) => vec![why.clone()],
        Verdict::Agrees => Vec::new(),
    };
    let measures: Vec<serde_json::Value> =
        glyph.rows.iter().map(|(m, mine, other, within)| serde_json::json!([m.name(), mine, other, within])).collect();
    let said: BTreeSet<&str> = glyph.traits.said.iter().map(|(code, _)| *code).collect();
    serde_json::json!({
        "font": glyph.job.font, "file": glyph.job.file, "index": glyph.job.index, "name": glyph.job.name,
        "types": glyph.traits.types, "said": said, "verdict": glyph.verdict.name(), "how": how, "measures": measures,
    })
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::super::excuses::Traits;
    use super::super::{Job, Shard};
    use super::*;
    use crate::conformance::differential::Measure;
    use crate::corpus::licence::Licence;

    fn font(name: &str, licence: Licence) -> Font {
        Font { name: name.into(), licence, files: vec!["ltr.svg".into()], dir: PathBuf::new() }
    }

    fn glyph(font: &str, index: usize, verdict: Verdict, used: &[usize]) -> Sewn {
        let job = Job { font: font.into(), file: "ltr.svg".into(), index, name: format!("g{index}") };
        let traits = Traits { types: vec!["satin_column"], ..Traits::default() };
        let rows = vec![(Measure::Stitches, "10".to_string(), "20".to_string(), false)];
        Sewn { job, traits, verdict, rows, used: used.iter().copied().collect() }
    }

    #[test]
    fn the_report_counts_each_font_s_verdicts_and_lists_what_differs() {
        let fonts = [font("a", Licence::Ofl), font("b", Licence::CcBySa), font("c", Licence::Gpl)];
        let excuses = Excuses::parse("[[excuse]]\nmeasures = [\"threads\"]\ndeviation = \"DEV-THREAD-001\"\n\n[[excuse]]\nmeasures = [\"size\"]\ndeviation = \"DEV-X-001\"\ntypes = [\"meander_fill\"]\n\n[[known]]\nmeasures = [\"size\"]\nfollow_up = \"the sizes of b\"\nfonts = [\"b\"]\n").unwrap();
        let sewn = [
            glyph("a", 0, Verdict::Agrees, &[]),
            glyph("a", 1, Verdict::Excused(vec!["threads differ".into()]), &[0]),
            glyph("a", 2, Verdict::Listed(vec!["cross_stitch".into()]), &[]),
            glyph("b", 0, Verdict::Differs(vec!["stitches differ: StitchCraft 10, Ink/Stitch 20".into()]), &[]),
            glyph("b", 1, Verdict::Failed("Ink/Stitch: it failed".into()), &[]),
            glyph("b", 2, Verdict::Known(vec!["size differ (known: the sizes of b)".into()]), &[2]),
        ];
        let options = Options { sample: 4, every: false, shard: None };
        let (md, failures, details) = markdown(&fonts, &sewn, &excuses, options);
        assert!(md.contains("a sample of 4 glyphs of each font: 6 glyphs of 2 fonts. 1 agree, 1 differ as the ledger explains, 1 differ as known follow-up work, 1 differ otherwise, 1 are listed for stitch types StitchCraft doesn't sew yet, and 1 didn't sew."), "{md}");
        assert!(md.contains("| a | OFL | 3 | 1 | 1 | 0 | 0 | 1 | 0 |\n| b | CC BY-SA | 3 | 0 | 0 | 1 | 1 | 0 | 1 |\n"), "{md}");
        assert!(md.contains("| DEV-THREAD-001 | threads | every glyph | 1 |\n| DEV-X-001 | size | an element is a `meander_fill` | 0 |"), "{md}");
        assert!(md.contains("## Known differences: follow-up work\n\n| Follow-up | Measures | For glyphs where | Glyphs |\n|---|---|---|---|\n| the sizes of b | size | the font is `b` | 1 |"), "{md}");
        assert!(md.contains("- b/ltr.svg glyph 2 `g2` (satin_column): known: size differ (known: the sizes of b)\n"), "{md}");
        assert!(md.contains("| cross_stitch | 1 | a |"), "{md}");
        assert!(md.contains("## Left out by their licences\n\nc (GPL)\n"), "{md}");
        assert!(md.contains("- b/ltr.svg glyph 0 `g0` (satin_column): stitches differ: StitchCraft 10, Ink/Stitch 20\n- b/ltr.svg glyph 1 `g1` (satin_column): Ink/Stitch: it failed\n"), "{md}");
        assert_eq!(failures, ["b: 1 of 3 glyphs differ and 1 didn't sew (target/conformance/fonts.md)"]);
        assert_eq!(
            details,
            [
                "| Font | Glyphs | Agree, excused, known | Differ, listed, didn't sew |",
                "|---|---|---|---|",
                "| a | 3 | 1, 1, 0 | ✅ 0, 1, 0 |",
                "| b | 3 | 0, 0, 1 | ❌ 1, 0, 1 |"
            ]
        );
        let json = json(&sewn[3]);
        assert_eq!(json["verdict"], "differs");
        assert_eq!(json["measures"][0], serde_json::json!(["stitches", "10", "20", false]));
    }

    #[test]
    fn a_run_says_which_glyphs_it_sewed() {
        let shard = Some(Shard { index: 2, count: 6 });
        assert_eq!(which(Options { sample: 4, every: false, shard: None }), "a sample of 4 glyphs of each font");
        assert_eq!(which(Options { sample: 4, every: true, shard }), "every glyph, part 2 of 6");
        assert_eq!(which(Options { sample: 3, every: false, shard }), "a sample of 3 glyphs of each font, part 2 of 6");
    }

    #[test]
    fn the_report_is_written_under_the_root_and_summed_up_in_the_outcome() {
        let root = util::root().join("target/xtask-tests/fonts-report");
        let mut outcome = crate::conformance::runner::Outcome {
            case: "inkstitch-fonts".into(),
            kind: "data",
            file: "f".into(),
            requirements: Vec::new(),
            failures: Vec::new(),
            outputs: Vec::new(),
            skipped: None,
            notes: Vec::new(),
            details: Vec::new(),
        };
        write(
            &root,
            &[font("a", Licence::Ofl)],
            &[glyph("a", 0, Verdict::Agrees, &[])],
            &Excuses::default(),
            Options { sample: 4, every: true, shard: None },
            &mut outcome,
        );
        assert!(
            util::read(&root.join("target/conformance/fonts.md"))
                .unwrap()
                .starts_with("# Ink/Stitch's fonts in the differential\n\nevery glyph: 1 glyphs")
        );
        assert!(util::read(&root.join("target/conformance/fonts.json")).unwrap().contains("\"verdict\": \"agrees\""));
        assert_eq!(
            outcome.notes,
            [
                "every glyph: 1 glyphs of 1 fonts. 1 agree, 0 differ as the ledger explains, 0 differ as known follow-up work, 0 differ otherwise, 0 are listed for stitch types StitchCraft doesn't sew yet, and 0 didn't sew. (target/conformance/fonts.md)"
            ]
        );
        assert!(outcome.failures.is_empty() && outcome.details.len() == 3);
        // A root it can't write in is said.
        std::fs::write(root.join("file"), "").unwrap();
        let mut failed = outcome.clone();
        write(&root.join("file"), &[], &[], &Excuses::default(), Options { sample: 4, every: true, shard: None }, &mut failed);
        assert!(failed.failures.iter().any(|f| f.contains("target/conformance")), "{:?}", failed.failures);
    }
}
