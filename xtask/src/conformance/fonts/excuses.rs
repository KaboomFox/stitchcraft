//! The excuses of the glyph differential ([`EXCUSES`]): the measures the deviations ledger explains, and for
//! which glyphs.
//!
//! **Why conditions.** A case of the differential excuses a measure of one design. The corpus has 21,616
//! glyphs, and the same deviation shows in many of them, but only where its cause is: stitches split where
//! StitchCraft splits a stitch longer than the machine's longest, the size where it clamps a setting. So an
//! excuse names the measures and the ledger's entry that explains them, and when it holds: for a glyph that
//! StitchCraft says a diagnostic of (`codes`, and `saying`, words of the message), whose elements are of a
//! stitch type (`types`), all of them of these stitch types (`only_types`), or set an Ink/Stitch setting to
//! something other than nothing or 0 (`settings`), or of a font (`fonts`). An excuse without conditions holds
//! for every glyph.
//!
//! **Explained where it can be.** A measure that differs is excused by the first excuse that holds for the
//! glyph and names the measure. Where the differential can undo the deviation in Ink/Stitch's file
//! ([`differential::undone`]), it measures the file again, and the excuse holds only when the measure then
//! agrees, as a case's does. An excuse with `within_mm` holds only where each program's thread lies within
//! that many millimetres of the other's, for 0.9 of it at least: the stitches are placed otherwise, along the
//! same lines. Where an excuse doesn't hold so, the next is tried, and with none left the glyph differs by
//! more than the first explains. The report counts the glyphs each excuse excused, so that one that excuses
//! nothing in the weekly run can be dropped.
//!
//! **Known differences.** A difference found and not yet explained or fixed is follow-up work. The file
//! lists it as `[[known]]`, with the same conditions and a `follow_up` that says what is known of it in place
//! of a deviation. A glyph whose differences are all excused or known counts as known: the report shows it
//! apart, and the case doesn't fail for it, so that the nightly run fails for a difference no one has seen.
//! M8.6 is done when no known difference is left.

use std::collections::BTreeSet;
use std::path::Path;

use serde::Deserialize;
use stitchcraft_plan::StitchPlan;

use super::super::differential::{self, Compared, Measure, Measured};
use super::Verdict;
use crate::util;

/// The excuses, relative to the repository root.
pub const EXCUSES: &str = "conformance/inkstitch/glyphs.toml";

/// What StitchCraft says and reads of a glyph, which an excuse's conditions ask about.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Traits {
    /// The stitch types of its elements, by id, in the order of `StitchType::ALL`.
    pub types: Vec<&'static str>,
    /// What StitchCraft said reading and planning it: each diagnostic's code and message.
    pub said: Vec<(&'static str, String)>,
    /// The Ink/Stitch settings its elements set to something other than nothing or 0.
    pub settings: BTreeSet<String>,
}

impl Traits {
    /// Whether a setting's text `value` sets something: neither nothing nor only zeros.
    pub fn sets(value: &str) -> bool {
        value.split_whitespace().any(|word| word.parse::<f64>() != Ok(0.0))
    }
}

/// An excuse or a known difference, as the file gives it.
#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
struct Entry {
    measures: Vec<String>,
    #[serde(default)]
    deviation: Option<String>,
    #[serde(default)]
    follow_up: Option<String>,
    #[serde(default)]
    codes: Vec<String>,
    #[serde(default)]
    saying: Option<String>,
    #[serde(default)]
    types: Vec<String>,
    #[serde(default)]
    only_types: Vec<String>,
    #[serde(default)]
    settings: Vec<String>,
    #[serde(default)]
    fonts: Vec<String>,
    #[serde(default)]
    within_mm: Option<f64>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct File {
    #[serde(default)]
    excuse: Vec<Entry>,
    #[serde(default)]
    known: Vec<Entry>,
}

/// An excuse, the measures a deviation explains, or a known difference, for the glyphs its conditions name.
#[derive(Clone, Debug, PartialEq)]
pub struct Excuse {
    /// The measures it excuses.
    pub measures: Vec<Measure>,
    /// The entry of the deviations ledger that explains them, or for a known difference what is known of it.
    pub reason: String,
    /// The glyph's conditions; an empty one asks nothing.
    codes: Vec<String>,
    saying: Option<String>,
    types: Vec<String>,
    only_types: Vec<String>,
    settings: Vec<String>,
    fonts: Vec<String>,
    /// How near each program's thread must lie to the other's, in millimetres, for it to hold.
    within_mm: Option<f64>,
}

impl Excuse {
    /// The excuse or known difference `entry` gives, for `reason`.
    fn of(reason: String, entry: Entry) -> Result<Excuse, String> {
        let measures =
            entry.measures.iter().map(|m| Measure::named(m).ok_or(format!("{EXCUSES}: `{m}` is not a measure"))).collect::<Result<Vec<_>, _>>()?;
        if measures.is_empty() {
            return Err(format!("{EXCUSES}: `{reason}` names no measure"));
        }
        if entry.within_mm.is_some_and(|mm| !(mm > 0.0 && mm.is_finite())) {
            return Err(format!("{EXCUSES}: `{reason}` asks for threads within a distance that is no length above 0"));
        }
        let Entry { codes, saying, types, only_types, settings, fonts, within_mm, .. } = entry;
        Ok(Excuse { measures, reason, codes, saying, types, only_types, settings, fonts, within_mm })
    }

    /// Whether it holds for a glyph of `font` with `traits`.
    fn holds(&self, font: &str, traits: &Traits) -> bool {
        let any = |wanted: &[String], has: &[&str]| wanted.is_empty() || has.iter().any(|h| wanted.iter().any(|w| w == h));
        let said = (self.codes.is_empty() && self.saying.is_none())
            || traits.said.iter().any(|(code, message)| {
                (self.codes.is_empty() || self.codes.iter().any(|c| c == code)) && self.saying.as_deref().is_none_or(|words| message.contains(words))
            });
        let settings: Vec<&str> = traits.settings.iter().map(String::as_str).collect();
        let only = self.only_types.is_empty() || (!traits.types.is_empty() && traits.types.iter().all(|t| self.only_types.iter().any(|o| o == t)));
        said && only && any(&self.types, &traits.types) && any(&self.settings, &settings) && any(&self.fonts, &[font])
    }

    /// Its conditions, as the report gives them.
    pub fn conditions(&self) -> String {
        let mut parts = Vec::new();
        let list = |words: &[String]| words.iter().map(|w| format!("`{w}`")).collect::<Vec<_>>().join(", ");
        match (&self.codes[..], &self.saying) {
            ([], None) => {}
            (codes, saying) => {
                let words = saying.as_deref().map(|w| format!(" saying \"{w}\"")).unwrap_or_default();
                let codes = if codes.is_empty() { "a diagnostic".to_string() } else { list(codes) };
                parts.push(format!("StitchCraft says {codes}{words}"));
            }
        }
        if !self.types.is_empty() {
            parts.push(format!("an element is a {}", list(&self.types)));
        }
        if !self.only_types.is_empty() {
            parts.push(format!("every element is a {}", list(&self.only_types)));
        }
        if !self.settings.is_empty() {
            parts.push(format!("an element sets {}", list(&self.settings)));
        }
        if !self.fonts.is_empty() {
            parts.push(format!("the font is {}", list(&self.fonts)));
        }
        if let Some(mm) = self.within_mm {
            parts.push(format!("each program's thread lies within {mm} mm of the other's"));
        }
        if parts.is_empty() { "every glyph".to_string() } else { parts.join("; ") }
    }
}

/// The excuses of the glyph differential, and its known differences.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Excuses {
    /// In the file's order.
    pub excuses: Vec<Excuse>,
    /// In the file's order, each `reason` its follow-up.
    pub known: Vec<Excuse>,
}

impl Excuses {
    /// The excuses [`EXCUSES`] gives under `root`.
    pub fn load(root: &Path) -> Result<Excuses, String> {
        Excuses::parse(&util::read(&root.join(EXCUSES))?)
    }

    /// The excuses in `text`, [`EXCUSES`]'s contents.
    pub fn parse(text: &str) -> Result<Excuses, String> {
        let file: File = toml::from_str(text).map_err(|e| format!("{EXCUSES}: {e}"))?;
        let excuses = file.excuse.into_iter().map(|entry| match (&entry.deviation, &entry.follow_up) {
            (Some(deviation), None) => Excuse::of(deviation.clone(), entry),
            _ => Err(format!("{EXCUSES}: an excuse names a deviation, and no follow_up")),
        });
        let known = file.known.into_iter().map(|entry| match (&entry.deviation, &entry.follow_up) {
            (None, Some(follow_up)) => Excuse::of(follow_up.clone(), entry),
            _ => Err(format!("{EXCUSES}: a known difference names its follow_up, and no deviation")),
        });
        Ok(Excuses { excuses: excuses.collect::<Result<_, _>>()?, known: known.collect::<Result<_, _>>()? })
    }

    /// The verdict on a glyph of `font` with `traits` whose measures are `rows`: StitchCraft's plan and its
    /// measures `ours`, against Ink/Stitch's plan and measures `theirs`. With it, the excuses and then the
    /// known differences used, by their place in the file, the known differences' places after the excuses'.
    pub fn judge(
        &self,
        font: &str,
        traits: &Traits,
        rows: &[Compared],
        ours: (&StitchPlan, &Measured),
        theirs: (&StitchPlan, &Measured),
    ) -> (Verdict, BTreeSet<usize>) {
        let ((our_plan, ours), (theirs, their_measures)) = (ours, theirs);
        let (mut excused, mut known, mut differs, mut used) = (Vec::new(), Vec::new(), Vec::new(), BTreeSet::new());
        let near = |excuse: &Excuse| excuse.within_mm.is_none_or(|mm| ours.within(their_measures, mm) >= differential::LEAST_OVERLAP);
        for (measure, mine, other, _) in rows.iter().filter(|row| !row.3) {
            let said = differential::differs(*measure, mine, other);
            let mut failed = None;
            let holding = self.excuses.iter().enumerate().filter(|(_, e)| e.measures.contains(measure) && e.holds(font, traits));
            let found = holding.into_iter().find_map(|(place, excuse)| {
                let deviation = &excuse.reason;
                if let Some(mm) = excuse.within_mm.filter(|_| !near(excuse)) {
                    let within = ours.within(their_measures, mm);
                    failed.get_or_insert(format!(
                        "{} differ by more than {deviation} explains: the threads lie within {mm} mm of each other for {within:.3} of them",
                        measure.name()
                    ));
                    return None;
                }
                let undone = differential::undone(deviation, theirs, our_plan)
                    .map(|undone| Measured::of(&undone).map(|undone| differential::compare(ours, &undone)));
                match undone.map(|rows| rows.and_then(|rows| rows.into_iter().find(|row| row.0 == *measure))) {
                    None => Some((place, format!("{said}, as {deviation} says"))),
                    Some(Some((_, mine, undone, true))) => {
                        // The overlap is one number of both files, which the row gives as StitchCraft's.
                        let value = if *measure == Measure::Overlap { mine } else { undone };
                        Some((place, format!("{said}, as {deviation} says, and agree without it ({value})")))
                    }
                    Some(again) => {
                        let without =
                            again.map_or("no needle point".to_string(), |(_, mine, undone, _)| differential::said_undone(*measure, &mine, &undone));
                        failed.get_or_insert(format!("{} differ by more than {deviation} explains: without it, {without}", measure.name()));
                        None
                    }
                }
            });
            let how = failed.unwrap_or(said);
            let follow_up = || {
                let mut holding = self.known.iter().enumerate().filter(|(_, k)| k.measures.contains(measure) && k.holds(font, traits) && near(k));
                holding.next().map(|(place, k)| (self.excuses.len() + place, format!("{how} (known: {})", k.reason)))
            };
            match found.map(|found| (found, true)).or_else(|| follow_up().map(|known| (known, false))) {
                Some(((place, how), true)) => {
                    used.insert(place);
                    excused.push(how);
                }
                Some(((place, how), false)) => {
                    used.insert(place);
                    known.push(how);
                }
                None => differs.push(how),
            }
        }
        let verdict = if !differs.is_empty() {
            Verdict::Differs(differs.into_iter().chain(known).chain(excused).collect())
        } else if !known.is_empty() {
            Verdict::Known(known.into_iter().chain(excused).collect())
        } else if !excused.is_empty() {
            Verdict::Excused(excused)
        } else {
            Verdict::Agrees
        };
        (verdict, used)
    }
}

#[cfg(test)]
mod tests {
    use stitchcraft_plan::{PlanBuilder, Provenance, Rgb, Role, Thread};

    use super::*;

    const FILE: &str = r#"
[[excuse]]
measures = ["threads"]
deviation = "DEV-THREAD-001"

[[excuse]]
measures = ["stitches", "size"]
deviation = "DEV-PRM-002"
codes = ["SC-W0102"]

[[excuse]]
measures = ["stitches"]
deviation = "DEV-FILL-018"
types = ["meander_fill"]

[[excuse]]
measures = ["overlap"]
deviation = "DEV-SAT-002"
settings = ["random_width_increase_percent"]
fonts = ["magnolia"]

[[excuse]]
measures = ["overlap"]
deviation = "DEV-SVG-026"
codes = ["SC-W0802"]
saying = "stitch pattern"
"#;

    fn traits(types: &[&'static str], said: &[(&'static str, &str)], settings: &[&str]) -> Traits {
        Traits {
            types: types.to_vec(),
            said: said.iter().map(|&(code, message)| (code, message.to_string())).collect(),
            settings: settings.iter().map(|s| s.to_string()).collect(),
        }
    }

    #[test]
    fn an_excuse_holds_where_its_conditions_all_do() {
        let [threads, clamped, meander, random, pattern] = <[Excuse; 5]>::try_from(Excuses::parse(FILE).unwrap().excuses).unwrap();
        let none = traits(&[], &[], &[]);
        assert!(threads.holds("any", &none));
        assert!(!clamped.holds("any", &none) && clamped.holds("any", &traits(&[], &[("SC-W0102", "clamped")], &[])));
        assert!(
            meander.holds("x", &traits(&["running_stitch", "meander_fill"], &[], &[])) && !meander.holds("x", &traits(&["tatami_fill"], &[], &[]))
        );
        // Every condition must hold: the setting and the font.
        let set = traits(&[], &[], &["random_width_increase_percent"]);
        assert!(random.holds("magnolia", &set) && !random.holds("other", &set) && !random.holds("magnolia", &none));
        // The code, with the words.
        assert!(pattern.holds("x", &traits(&[], &[("SC-W0802", "`p1` is an Ink/Stitch stitch pattern")], &[])));
        assert!(!pattern.holds("x", &traits(&[], &[("SC-W0802", "a clip path")], &[])));
        assert!(!pattern.holds("x", &traits(&[], &[("SC-W0804", "an Ink/Stitch stitch pattern")], &[])));
        let words = Excuses::parse("[[excuse]]\nmeasures = [\"size\"]\ndeviation = \"D\"\nsaying = \"far\"").unwrap().excuses;
        assert!(words[0].holds("x", &traits(&[], &[("SC-W0013", "too far")], &[])) && !words[0].holds("x", &none));
    }

    #[test]
    fn an_excuse_s_conditions_are_said() {
        let said: Vec<String> = Excuses::parse(FILE).unwrap().excuses.iter().map(Excuse::conditions).collect();
        assert_eq!(
            said,
            [
                "every glyph",
                "StitchCraft says `SC-W0102`",
                "an element is a `meander_fill`",
                "an element sets `random_width_increase_percent`; the font is `magnolia`",
                "StitchCraft says `SC-W0802` saying \"stitch pattern\"",
            ]
        );
        let words = Excuses::parse("[[excuse]]\nmeasures = [\"size\"]\ndeviation = \"D\"\nsaying = \"far\"").unwrap().excuses;
        assert_eq!(words[0].conditions(), "StitchCraft says a diagnostic saying \"far\"");
    }

    #[test]
    fn a_file_of_excuses_names_measures_and_nothing_else() {
        assert_eq!(Excuses::parse("").unwrap(), Excuses::default());
        assert_eq!(
            Excuses::parse("[[excuse]]\nmeasures = [\"colours\"]\ndeviation = \"D\"").unwrap_err(),
            format!("{EXCUSES}: `colours` is not a measure")
        );
        assert_eq!(Excuses::parse("[[excuse]]\nmeasures = []\ndeviation = \"D\"").unwrap_err(), format!("{EXCUSES}: `D` names no measure"));
        for wrong in ["[[excuse]]\nmeasures = [\"size\"]\ndeviation = \"D\"\nfont = [\"x\"]", "[[excuses]]"] {
            assert!(Excuses::parse(wrong).unwrap_err().starts_with(EXCUSES), "{wrong}");
        }
        // An excuse names a deviation, and a known difference its follow-up.
        let excuse = format!("{EXCUSES}: an excuse names a deviation, and no follow_up");
        assert_eq!(Excuses::parse("[[excuse]]\nmeasures = [\"size\"]").unwrap_err(), excuse);
        assert_eq!(Excuses::parse("[[excuse]]\nmeasures = [\"size\"]\ndeviation = \"D\"\nfollow_up = \"F\"").unwrap_err(), excuse);
        let known = format!("{EXCUSES}: a known difference names its follow_up, and no deviation");
        assert_eq!(Excuses::parse("[[known]]\nmeasures = [\"size\"]\ndeviation = \"D\"").unwrap_err(), known);
        assert_eq!(Excuses::parse("[[known]]\nmeasures = [\"size\"]").unwrap_err(), known);
        assert_eq!(Excuses::parse("[[known]]\nmeasures = [\"size\"]\nfollow_up = \"F\"").unwrap().known.first().unwrap().reason, "F");
        // The repository's own file reads, and names only deviations of the ledger.
        let ledger: Vec<String> = crate::deviations::parse(&util::read(&util::root().join(crate::deviations::LEDGER)).unwrap())
            .unwrap()
            .into_iter()
            .map(|d| d.id)
            .collect();
        let excuses = Excuses::load(&util::root()).unwrap().excuses;
        assert!(!excuses.is_empty());
        for excuse in excuses {
            assert!(ledger.contains(&excuse.reason), "{}", excuse.reason);
        }
    }

    #[test]
    fn an_excuse_may_hold_only_for_glyphs_of_some_stitch_types_alone() {
        let file = "[[excuse]]\nmeasures = [\"stitches\"]\ndeviation = \"DEV-RUN-002\"\nonly_types = [\"running_stitch\", \"manual_stitch\"]\nwithin_mm = 0.3\n";
        let excuses = Excuses::parse(file).unwrap().excuses;
        let excuse = excuses.first().unwrap();
        assert!(
            excuse.holds("x", &traits(&["running_stitch"], &[], &[])) && excuse.holds("x", &traits(&["running_stitch", "manual_stitch"], &[], &[]))
        );
        assert!(!excuse.holds("x", &traits(&["running_stitch", "satin_column"], &[], &[])) && !excuse.holds("x", &traits(&[], &[], &[])));
        assert_eq!(
            excuse.conditions(),
            "every element is a `running_stitch`, `manual_stitch`; each program's thread lies within 0.3 mm of the other's"
        );
        for wrong in ["0", "-1", "nan"] {
            let text = file.replace("0.3", wrong);
            assert_eq!(
                Excuses::parse(&text).unwrap_err(),
                format!("{EXCUSES}: `DEV-RUN-002` asks for threads within a distance that is no length above 0"),
                "{wrong}"
            );
        }
    }

    #[test]
    fn an_excuse_within_a_distance_holds_only_where_the_threads_lie_that_near() {
        let excuses = Excuses::parse("[[excuse]]\nmeasures = [\"stitches\"]\ndeviation = \"DEV-RUN-002\"\nwithin_mm = 0.3\n").unwrap();
        let blue = Rgb::new(11, 61, 145);
        // A line of 20 stitches and one of 10 along it, and 10 along a line 2 mm off it at its middle.
        let line = |n: u32, bend: f64| {
            let mut builder = PlanBuilder::new(Thread::new(blue));
            for i in 0..=n {
                let x = 10.0 * f64::from(i) / f64::from(n);
                let y = if (x - 5.0).abs() < 1e-9 { bend } else { 0.0 };
                builder.stitch(stitchcraft_core::Point::new(x, y).unwrap(), Provenance::plan(Role::Top));
            }
            builder.finish()
        };
        let judged = |theirs: &StitchPlan| {
            let ours = line(20, 0.0);
            let (measured, their_measures) = (Measured::of(&ours).unwrap(), Measured::of(theirs).unwrap());
            let rows = differential::compare(&measured, &their_measures);
            excuses.judge("f", &Traits::default(), &rows, (&ours, &measured), (theirs, &their_measures))
        };
        let (verdict, used) = judged(&line(10, 0.0));
        assert_eq!(verdict, Verdict::Excused(vec!["stitches differ: StitchCraft 21, Ink/Stitch 11, as DEV-RUN-002 says".to_string()]));
        assert_eq!(used, BTreeSet::from([0]));
        let (verdict, used) = judged(&line(10, 2.0));
        let how = match verdict {
            Verdict::Differs(how) => how,
            other => vec![format!("{other:?}")],
        };
        assert!(
            how.first().unwrap().starts_with("stitches differ by more than DEV-RUN-002 explains: the threads lie within 0.3 mm of each other for 0."),
            "{how:?}"
        );
        assert!(used.is_empty());
    }

    #[test]
    fn a_difference_no_excuse_explains_may_be_known_and_then_doesn_t_fail() {
        let file = "[[excuse]]\nmeasures = [\"threads\"]\ndeviation = \"DEV-THREAD-001\"\n\n[[known]]\nmeasures = [\"threads\", \"stitches\"]\nfollow_up = \"the threads of f\"\nfonts = [\"f\"]\n";
        let excuses = Excuses::parse(file).unwrap();
        let (red, blue) = (Rgb::new(237, 23, 31), Rgb::new(11, 61, 145));
        let judged = |font: &str, ours: &StitchPlan, theirs: &StitchPlan| {
            let (measured, their_measures) = (Measured::of(ours).unwrap(), Measured::of(theirs).unwrap());
            let rows = differential::compare(&measured, &their_measures);
            excuses.judge(font, &Traits::default(), &rows, (ours, &measured), (theirs, &their_measures))
        };
        // The palette can't explain threads that change at other blocks; in font f that is known.
        let (ours, theirs) = (plan_of(&[red, blue], 5.0), plan_of(&[red, red], 5.0));
        let (verdict, used) = judged("f", &ours, &theirs);
        assert_eq!(verdict, Verdict::Known(vec!["threads differ by more than DEV-THREAD-001 explains: without it, StitchCraft #ed171f #0b3d91, Ink/Stitch #ed171f #ed171f (known: the threads of f)".to_string()]));
        assert_eq!(used, BTreeSet::from([1]));
        let (verdict, used) = judged("g", &ours, &theirs);
        assert!(matches!(verdict, Verdict::Differs(_)) && used.is_empty(), "{verdict:?}");
        // An excused measure stays excused beside a known one.
        let (verdict, used) = judged("f", &plan_of(&[red, blue], 5.0), &plan_of(&[blue, red], 5.0));
        assert_eq!(verdict, Verdict::Excused(vec!["threads differ: StitchCraft #ed171f #0b3d91, Ink/Stitch #0b3d91 #ed171f, as DEV-THREAD-001 says, and agree without it (#ed171f #0b3d91)".to_string()]));
        assert_eq!(used, BTreeSet::from([0]));
    }

    #[test]
    fn a_needle_point_ink_stitch_sews_alone_is_explained_where_the_glyph_agrees_without_it() {
        let file = "[[excuse]]\nmeasures = [\"size\", \"place\", \"trims\", \"jumps\", \"overlap\"]\ndeviation = \"DEV-SVG-011\"\n";
        let excuses = Excuses::parse(file).unwrap();
        let point = |x: f64, y: f64| stitchcraft_core::Point::new(x, y).unwrap();
        // Needle points, and one sewn alone after a trim, where a move-to draws nothing.
        let plan = |points: &[(f64, f64)], lone: Option<(f64, f64)>| {
            let mut builder = PlanBuilder::new(Thread::new(Rgb::new(11, 61, 145)));
            for &(x, y) in points {
                builder.stitch(point(x, y), Provenance::plan(Role::Top));
            }
            if let Some((x, y)) = lone {
                builder.trim(None);
                builder.jump(point(x, y), Provenance::plan(Role::Travel));
                builder.stitch(point(x, y), Provenance::plan(Role::Top));
            }
            builder.finish()
        };
        let judged = |ours: &StitchPlan, theirs: &StitchPlan| {
            let (measured, their_measures) = (Measured::of(ours).unwrap(), Measured::of(theirs).unwrap());
            let rows = differential::compare(&measured, &their_measures);
            excuses.judge("f", &Traits::default(), &rows, (ours, &measured), (theirs, &their_measures))
        };
        // A line 10 mm long, centred as a PES file is. Ink/Stitch's has a needle point 20 mm off, and the
        // design is centred with it.
        let along = |y: f64| (0..=10).map(|i| (f64::from(i) - 5.0, y)).collect::<Vec<_>>();
        let ours = plan(&along(0.0), None);
        let (verdict, used) = judged(&ours, &plan(&along(-10.0), Some((-5.0, 10.0))));
        let size = "size differ: StitchCraft 10.0 × 0.0 mm, Ink/Stitch 10.0 × 20.0 mm, as DEV-SVG-011 says, and agree without it (10.0 × 0.0 mm)";
        // The overlap, one number of both files, is said as it is without the needle point.
        let overlap = "overlap 0.000, below 0.9, as DEV-SVG-011 says, and agree without it (1.000)";
        assert_eq!(verdict, Verdict::Excused(vec![size.to_string(), overlap.to_string()]));
        assert_eq!(used, BTreeSet::from([0]));
        // Where the glyph still differs without the needle point, the deviation doesn't explain it: Ink/Stitch's
        // line runs across StitchCraft's.
        let across: Vec<(f64, f64)> = (0..=10).map(|i| (-10.0, f64::from(i) - 5.0)).collect();
        let (verdict, used) = judged(&ours, &plan(&across, Some((10.0, 0.0))));
        let how = match verdict {
            Verdict::Differs(how) => how,
            other => vec![format!("{other:?}")],
        };
        let without = "size differ by more than DEV-SVG-011 explains: without it, StitchCraft 10.0 × 0.0 mm, Ink/Stitch 0.0 × 10.0 mm";
        assert_eq!(how.first().unwrap(), without);
        assert!(how.last().unwrap().starts_with("overlap differ by more than DEV-SVG-011 explains: without it, overlap 0."), "{how:?}");
        assert!(used.is_empty());
    }

    #[test]
    fn a_setting_sets_something_unless_it_is_nothing_or_zeros() {
        for (value, sets) in
            [("", false), ("0", false), ("0 0.0", false), (" -0 ", false), ("0 20.0", true), ("-34.0 0", true), ("True", true), ("x", true)]
        {
            assert_eq!(Traits::sets(value), sets, "{value:?}");
        }
    }

    fn plan_of(threads: &[Rgb], long: f64) -> StitchPlan {
        let mut builder = PlanBuilder::new(Thread::new(threads[0]));
        for (i, &thread) in threads.iter().enumerate() {
            if i > 0 {
                builder.change_thread(Thread::new(thread));
            }
            builder.stitch(stitchcraft_core::Point::new(0.0, 0.0).unwrap(), Provenance::plan(Role::Top));
            builder.stitch(stitchcraft_core::Point::new(long, 0.0).unwrap(), Provenance::plan(Role::Top));
        }
        builder.finish()
    }

    #[test]
    fn a_measure_is_excused_by_the_first_excuse_that_holds_and_explains_it() {
        let (red, vermilion, blue) = (Rgb::new(237, 23, 31), Rgb::new(254, 55, 15), Rgb::new(11, 61, 145));
        let file = "[[excuse]]\nmeasures = [\"threads\"]\ndeviation = \"DEV-THREAD-001\"\n\n[[excuse]]\nmeasures = [\"threads\"]\ndeviation = \"DEV-X-001\"\ncodes = [\"SC-W0102\"]\n\n[[excuse]]\nmeasures = [\"stitches\"]\ndeviation = \"DEV-FIN-001\"\n";
        let excuses = Excuses::parse(file).unwrap();
        let judged = |ours: &StitchPlan, theirs: &StitchPlan, traits: &Traits| {
            let (measured, their_measures) = (Measured::of(ours).unwrap(), Measured::of(theirs).unwrap());
            let rows = differential::compare(&measured, &their_measures);
            excuses.judge("f", traits, &rows, (ours, &measured), (theirs, &their_measures))
        };
        let none = traits(&[], &[], &[]);
        let ours = plan_of(&[vermilion, blue], 5.0);
        // The threads change at the same blocks: the palette explains them.
        let (verdict, used) = judged(&ours, &plan_of(&[red, blue], 5.0), &none);
        assert_eq!(verdict, Verdict::Excused(vec!["threads differ: StitchCraft #fe370f #0b3d91, Ink/Stitch #ed171f #0b3d91, as DEV-THREAD-001 says, and agree without it (#fe370f #0b3d91)".to_string()]));
        assert_eq!(used, BTreeSet::from([0]));
        // At other blocks it doesn't, and the next excuse that holds is tried, which needs a clamped value.
        let other = plan_of(&[red, red], 5.0);
        let (verdict, _) = judged(&ours, &other, &none);
        assert_eq!(
            verdict,
            Verdict::Differs(vec![
                "threads differ by more than DEV-THREAD-001 explains: without it, StitchCraft #fe370f #0b3d91, Ink/Stitch #ed171f #ed171f"
                    .to_string()
            ])
        );
        let (verdict, used) = judged(&ours, &other, &traits(&[], &[("SC-W0102", "clamped")], &[]));
        assert_eq!(
            verdict,
            Verdict::Excused(vec!["threads differ: StitchCraft #fe370f #0b3d91, Ink/Stitch #ed171f #ed171f, as DEV-X-001 says".to_string()])
        );
        assert_eq!(used, BTreeSet::from([1]));
        // A measure an excuse can't explain differs by more than it explains, and one no excuse names differs
        // as it is: 20 stitches of 1 mm against 10, which no split explains, and so 19 mm against 9.
        let line = |n: u32| {
            let mut builder = PlanBuilder::new(Thread::new(blue));
            for i in 0..n {
                builder.stitch(stitchcraft_core::Point::new(f64::from(i), 0.0).unwrap(), Provenance::plan(Role::Top));
            }
            builder.finish()
        };
        let (verdict, used) = judged(&line(20), &line(10), &none);
        let how = match verdict {
            Verdict::Differs(how) => how,
            other => vec![format!("{other:?}")],
        };
        assert_eq!(how.first().unwrap(), "stitches differ by more than DEV-FIN-001 explains: without it, StitchCraft 20, Ink/Stitch 10");
        assert_eq!(how.get(1).unwrap(), "size differ: StitchCraft 19.0 × 0.0 mm, Ink/Stitch 9.0 × 0.0 mm");
        assert!(used.is_empty());
        // What agrees is excused by nothing.
        assert_eq!(judged(&ours, &ours, &none), (Verdict::Agrees, BTreeSet::new()));
    }
}
