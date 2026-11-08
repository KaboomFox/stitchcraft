//! The glyph differential (M8.6, `docs/src/design/conformance.md` › L3): each glyph of Ink/Stitch's fonts,
//! sewn as a design of its own in Ink/Stitch and in StitchCraft, compared by the differential's measures.
//!
//! **The glyphs.** The corpus is Ink/Stitch's fonts at the commit `conformance/inkstitch/fonts.toml` pins,
//! which `cargo xtask corpus` fetches ([`crate::corpus`]), and of them the fonts whose licence file is the
//! OFL, CC0, CC BY or CC BY-SA. Each glyph is cut out of its glyph file as a design of its own, its group
//! shown and the other glyphs left out ([`crate::corpus::glyphs`]). The design is written to [`WRITTEN`],
//! read and planned by StitchCraft for the [`machine`], as `stitch plan` plans it, and sewn by Ink/Stitch
//! ([`super::differential`]). Both PES files are read back by StitchCraft's reader and measured as the
//! differential's cases are. Glyphs run side by side, as many at a time as the computer has cores.
//!
//! **The sample.** Every night the differential sews a sample of [`Options::sample`] glyphs of each font
//! (the case's `sample`), and every week every glyph, split over several jobs ([`Shard`]). The sample is
//! fixed by the pinned corpus: the font's glyphs are taken in the order of its files, sorted by path, and of
//! the glyphs in each file, and the sample takes those at the places `i·n/k`, rounded down, of `n` glyphs,
//! for `i` from 0 to `k` − 1. So it spreads over the font, its first glyph among them, and moves only when
//! the pin does. A run may sew the glyphs of some fonts alone ([`Options::fonts`]), so that the fonts a
//! weekly run found differing can be sewn again in minutes, where a part of every glyph takes hours.
//!
//! **What a glyph gives** ([`Verdict`]). A glyph agrees when every measure lies within its tolerance. A
//! measure that differs may be excused by the ledger, or be a known difference, follow-up work, as
//! [`excuses`] says, and so may a glyph that one program sews and the other doesn't. A glyph whose stitch
//! types StitchCraft doesn't sew yet (`SC-W0011`) is listed with them and not sewn in Ink/Stitch: it joins
//! the comparison with M10's steps. A glyph with an element whose setting StitchCraft can't read, which it
//! refuses (`SC-E0101`) and Ink/Stitch sews at the setting's default, is excused by [`REFUSED`] and not
//! compared. A glyph neither program writes a file of agrees: Ink/Stitch writes none when it finds nothing
//! to sew, an element it can't sew or elements that sew no stitch, such as a glyph of one empty path, and
//! StitchCraft none when it has nothing to stitch (`SC-E0010`) or an element is left out with an error, as
//! `stitch plan` does. A glyph that a program fails on is said, with why. The case fails when a glyph
//! differs by a measure no excuse explains, or a program fails on it, as a case of the differential does.
//!
//! **The report** ([`report`]): `target/conformance/fonts.md` and `target/conformance/fonts.json`. The design
//! of each glyph that differs, or didn't sew, stays in [`WRITTEN`] with Ink/Stitch's PES file of it, where a
//! reviewer can open them.

pub mod excuses;
mod report;

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, mpsc};

use stitchcraft_cli::commands::plan::Source;
use stitchcraft_core::{Budget, Code, Mm, Severity, Size};
use stitchcraft_engine::generate::stitch_type_of;
use stitchcraft_params::StitchType;
use stitchcraft_plan::{MachineProfile, StitchPlan, profiles};

use super::differential::{self, Compared, Inkstitch, Measured, Ran};
use super::runner::Outcome;
use crate::corpus::{self, Font, glyphs};
use crate::util;
use excuses::{Excuses, Traits};

/// Where each glyph's design is written, relative to the repository root.
pub const WRITTEN: &str = "target/conformance/fonts";
/// The deviation by which StitchCraft refuses an element whose setting it can't read, where Ink/Stitch sews
/// the setting's default.
pub const REFUSED: &str = "DEV-PRM-001";
/// The side of the hoop the glyphs are planned in, in millimetres ([`machine`]).
pub const HOOP: f64 = 400.0;

/// The machine each glyph is planned for: the reference machine, with a hoop [`HOOP`] mm a side. A glyph
/// file is drawn at its font's height, up to 144 mm, and a glyph may be wider than the reference hoop,
/// 130 mm. StitchCraft plans no design larger than its hoop (`SC-E0701`), and Ink/Stitch, which knows no
/// hoop, sews it. The machine's stitches, trims and threads are the reference machine's.
pub fn machine() -> MachineProfile {
    let side = Mm::new(HOOP).unwrap_or(profiles::REFERENCE.hoop.width);
    MachineProfile { hoop: Size::new(side, side), comfort: None, ..profiles::REFERENCE.clone() }
}

/// Which glyphs a run sews.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Options {
    /// The glyphs of each font in the sample.
    pub sample: usize,
    /// Every glyph, not the sample.
    pub every: bool,
    /// Only the glyphs of these fonts, by their folders' names, or of every font when there are none.
    pub fonts: Vec<String>,
    /// Only this part of them.
    pub shard: Option<Shard>,
}

/// A part of the glyphs, for a run split over several jobs: the `index`-th of `count`, from 1. The glyphs
/// are dealt out in turn, so each part holds some of every large font.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Shard {
    /// Which part, from 1.
    pub index: usize,
    /// Of how many.
    pub count: usize,
}

impl Shard {
    /// The shard `text` names, `I/N`.
    pub fn parse(text: &str) -> Result<Shard, String> {
        let parsed =
            text.split_once('/').and_then(|(index, count)| Some(Shard { index: index.trim().parse().ok()?, count: count.trim().parse().ok()? }));
        parsed.filter(|s| (1..=s.count).contains(&s.index)).ok_or(format!("`{text}` is no part I/N, with I from 1 to N"))
    }

    /// Whether the `job`-th glyph, from 0, is in this part.
    fn holds(self, job: usize) -> bool {
        job % self.count == self.index - 1
    }
}

/// The places of the glyphs a sample of `k` takes of `n`: `i·n/k`, rounded down, for `i` from 0 to `k` − 1;
/// all of them when `k` is `n` or more.
pub fn sampled(n: usize, k: usize) -> Vec<usize> {
    if k >= n {
        return (0..n).collect();
    }
    (0..k).map(|i| i * n / k).collect()
}

/// What sewing a glyph in both programs gave.
#[derive(Clone, Debug, PartialEq)]
pub enum Verdict {
    /// Every measure agrees.
    Agrees,
    /// Measures differ, each excused: how, as the report says it.
    Excused(Vec<String>),
    /// Measures differ that no excuse explains, each a known difference: how.
    Known(Vec<String>),
    /// Measures differ that no excuse explains: how.
    Differs(Vec<String>),
    /// StitchCraft doesn't sew these stitch types yet (`SC-W0011`), so the glyph isn't compared.
    Listed(Vec<String>),
    /// A program couldn't sew it: why.
    Failed(String),
}

impl Verdict {
    /// Its name in the reports.
    fn name(&self) -> &'static str {
        match self {
            Verdict::Agrees => "agrees",
            Verdict::Excused(_) => "excused",
            Verdict::Known(_) => "known",
            Verdict::Differs(_) => "differs",
            Verdict::Listed(_) => "listed",
            Verdict::Failed(_) => "failed",
        }
    }
}

/// A glyph to sew: its font, its file and its place there, and its name.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Job {
    font: String,
    file: String,
    index: usize,
    name: String,
}

impl Job {
    /// Where its design is written, under [`WRITTEN`]: its font, its file without `.svg`, and its place there.
    fn path(&self, root: &Path) -> PathBuf {
        let file = self.file.strip_suffix(".svg").unwrap_or(&self.file);
        root.join(WRITTEN).join(&self.font).join(format!("{file}-{:04}.svg", self.index))
    }
}

/// A glyph sewn.
#[derive(Clone, Debug)]
struct Sewn {
    job: Job,
    /// What StitchCraft said and read of it.
    traits: Traits,
    verdict: Verdict,
    /// Each measure of both machine files.
    rows: Vec<Compared>,
    /// The excuses that excused its measures, by their place in their file.
    used: BTreeSet<usize>,
}

/// Runs the glyph differential with `options`: the fonts' verdicts go to `outcome`.
pub fn run(root: &Path, options: &Options, outcome: &mut Outcome) {
    let Some(inkstitch) = Inkstitch::ready(root, outcome) else { return };
    let corpus = match corpus::unpacked(root) {
        Ok(Some(corpus)) => corpus,
        Ok(None) => {
            outcome.skipped = Some("Ink/Stitch's fonts are not fetched: run cargo xtask corpus".to_string());
            return;
        }
        Err(e) => return outcome.failures.push(e),
    };
    let (fonts, excuses) = match (corpus::fonts(&corpus), Excuses::load(root)) {
        (Ok(fonts), Ok(excuses)) => (fonts, excuses),
        (fonts, excuses) => return outcome.failures.extend(fonts.err().into_iter().chain(excuses.err())),
    };
    let (jobs, problems) = jobs(&fonts, options);
    outcome.failures.extend(problems);
    let sewn = sew_all(root, &inkstitch, &fonts, &jobs, &excuses);
    report::write(root, &fonts, &sewn, &excuses, options, outcome);
}

/// The glyphs `options` asks for, of the fonts the corpus takes, each with its place in its file, and what
/// kept a glyph file from being read or a font named from being sewn.
fn jobs(fonts: &[Font], options: &Options) -> (Vec<Job>, Vec<String>) {
    let mut jobs = Vec::new();
    let taken: Vec<&Font> = fonts.iter().filter(|font| font.licence.taken()).collect();
    let mut problems: Vec<String> = options
        .fonts
        .iter()
        .filter(|name| !taken.iter().any(|font| font.name == **name))
        .map(|name| format!("there is no font `{name}` among the fonts the corpus takes"))
        .collect();
    for font in taken.into_iter().filter(|font| options.fonts.is_empty() || options.fonts.contains(&font.name)) {
        let mut of_font = Vec::new();
        for file in &font.files {
            match util::read(&font.dir.join(file)).and_then(|text| glyphs::glyphs(&text)) {
                Ok(names) => of_font.extend(names.into_iter().enumerate().map(|(index, name)| Job {
                    font: font.name.clone(),
                    file: file.clone(),
                    index,
                    name,
                })),
                Err(e) => problems.push(format!("{}/{file}: {e}", font.name)),
            }
        }
        if options.every {
            jobs.extend(of_font);
        } else {
            let places = sampled(of_font.len(), options.sample);
            jobs.extend(of_font.into_iter().enumerate().filter(|(place, _)| places.contains(place)).map(|(_, job)| job));
        }
    }
    let jobs = jobs.into_iter().enumerate().filter(|(n, _)| options.shard.is_none_or(|shard| shard.holds(*n))).map(|(_, job)| job).collect();
    (jobs, problems)
}

/// Sews every glyph of `jobs`, as many at a time as there are cores: the glyph files are read one after
/// another, and each glyph's design handed to the next free worker.
fn sew_all(root: &Path, inkstitch: &Inkstitch, fonts: &[Font], jobs: &[Job], excuses: &Excuses) -> Vec<Sewn> {
    let workers = std::thread::available_parallelism().map_or(1, usize::from);
    let (send, receive) = mpsc::sync_channel::<(Job, Result<String, String>)>(workers * 2);
    let receive = Mutex::new(receive);
    let sewn = Mutex::new(Vec::with_capacity(jobs.len()));
    std::thread::scope(|scope| {
        for _ in 0..workers {
            scope.spawn(|| {
                loop {
                    let next = receive.lock().ok().and_then(|receive| receive.recv().ok());
                    let Some((job, design)) = next else { break };
                    let glyph = match design {
                        Ok(design) => sew(root, inkstitch, job, &design, excuses),
                        Err(e) => Sewn { job, traits: Traits::default(), verdict: Verdict::Failed(e), rows: Vec::new(), used: BTreeSet::new() },
                    };
                    if let Ok(mut sewn) = sewn.lock() {
                        sewn.push(glyph);
                    }
                }
            });
        }
        let mut at = 0;
        while let Some(first) = jobs.get(at) {
            let of_file: Vec<&Job> = jobs.iter().skip(at).take_while(|job| job.font == first.font && job.file == first.file).collect();
            at += of_file.len();
            let font = fonts.iter().find(|font| font.name == first.font);
            let text = font.map_or(Err(format!("no font {}", first.font)), |font| util::read(&font.dir.join(&first.file)));
            // Each design goes to a worker as soon as it is made, and waits while every worker is busy.
            let mut sent = BTreeSet::new();
            let made = text.and_then(|text| {
                glyphs::each_design(&text, &|index| of_file.iter().any(|job| job.index == index), &mut |index, glyph| {
                    if let (Some(job), Some(design)) = (of_file.iter().find(|job| job.index == index), glyph.design) {
                        sent.insert(index);
                        let _ = send.send(((*job).clone(), Ok(design)));
                    }
                })
            });
            for job in of_file.into_iter().filter(|job| !sent.contains(&job.index)) {
                let why = made.clone().err().unwrap_or(format!("no glyph {} in the file", job.index));
                let _ = send.send((job.clone(), Err(why)));
            }
        }
        drop(send);
    });
    let mut sewn = sewn.into_inner().unwrap_or_default();
    sewn.sort_by(|a, b| (&a.job.font, &a.job.file, a.job.index).cmp(&(&b.job.font, &b.job.file, b.job.index)));
    sewn
}

/// Sews the glyph of `job`, whose design is `design`, in both programs, and judges it.
fn sew(root: &Path, inkstitch: &Inkstitch, job: Job, design: &str, excuses: &Excuses) -> Sewn {
    let path = job.path(root);
    let mut sewn = Sewn { job, traits: Traits::default(), verdict: Verdict::Agrees, rows: Vec::new(), used: BTreeSet::new() };
    let written = path.parent().map_or(Ok(()), std::fs::create_dir_all).and_then(|()| std::fs::write(&path, design));
    if let Err(e) = written {
        sewn.verdict = Verdict::Failed(format!("{}: {e}", util::rel(&path)));
        return sewn;
    }
    sewn.verdict = match ours(design.as_bytes(), &sewn.job.name, &mut sewn.traits) {
        Err(e) => Verdict::Failed(format!("StitchCraft: {e}")),
        Ok(Ours { listed, .. }) if !listed.is_empty() => Verdict::Listed(listed),
        Ok(Ours { refused, .. }) if !refused.is_empty() => {
            Verdict::Excused(refused.into_iter().map(|why| format!("StitchCraft refuses an element: {why}, as {REFUSED} says")).collect())
        }
        Ok(Ours { plan, nothing, .. }) => {
            let theirs = inkstitch.pes_or_nothing(root, &path).and_then(|ran| match ran {
                Ran::Nothing(said) => Ok(Err(said)),
                Ran::Wrote(bytes) => {
                    let kept = path.with_extension("inkstitch.pes");
                    std::fs::write(&kept, &bytes).map_err(|e| format!("{}: {e}", util::rel(&kept)))?;
                    Ok(Ok(stitchcraft_formats::decode(&bytes).map_err(|e| format!("Ink/Stitch's PES file doesn't read: {e}"))?.plan))
                }
            });
            match theirs {
                Err(e) => Verdict::Failed(format!("Ink/Stitch: {e}")),
                Ok(written) => {
                    let ours = plan.as_ref().and_then(|plan| Some((plan, Measured::of(plan)?)));
                    let theirs = written.as_ref().ok().and_then(|plan| Some((plan, Measured::of(plan)?)));
                    match (ours, theirs) {
                        (Some((our_plan, ours)), Some((their_plan, theirs))) => {
                            sewn.rows = differential::compare(&ours, &theirs);
                            let (verdict, used) =
                                excuses.judge(&sewn.job.font, &sewn.job.name, &sewn.traits, &sewn.rows, (our_plan, &ours), (their_plan, &theirs));
                            sewn.used = used;
                            verdict
                        }
                        // Neither program writes a file of the glyph, or a file that sews.
                        (None, None) => Verdict::Agrees,
                        (ours, _) => {
                            let how = if ours.is_none() {
                                format!("StitchCraft sews nothing ({}), and Ink/Stitch sews the glyph", nothing.join("; "))
                            } else {
                                let said = written.err().unwrap_or_else(|| "its file sews nothing".to_string());
                                format!("Ink/Stitch sews nothing ({said}), and StitchCraft sews the glyph")
                            };
                            let (verdict, used) = excuses.judge_alone(&sewn.job.font, &sewn.job.name, &sewn.traits, how);
                            sewn.used = used;
                            verdict
                        }
                    }
                }
            }
        }
    };
    if !matches!(sewn.verdict, Verdict::Differs(_) | Verdict::Known(_) | Verdict::Failed(_)) {
        // Only what differs, or didn't sew, is kept for a reviewer.
        let _ = std::fs::remove_file(path.with_extension("inkstitch.pes"));
        let _ = std::fs::remove_file(&path);
    }
    sewn
}

/// What StitchCraft makes of a glyph.
#[derive(Debug, Default)]
struct Ours {
    /// Its plan, read back from its PES file, or `None` when it writes no file.
    plan: Option<StitchPlan>,
    /// The stitch types of its elements it doesn't sew yet, named by `SC-W0011`.
    listed: Vec<String>,
    /// Why it refuses elements whose settings it can't read, by `SC-E0101`.
    refused: Vec<String>,
    /// Why it writes no file, when it writes none: its errors.
    nothing: Vec<String>,
}

/// What StitchCraft makes of the glyph design `bytes`, planned for the [`machine`]. What it says and reads
/// of the glyph goes to `traits`.
fn ours(bytes: &[u8], name: &str, traits: &mut Traits) -> Result<Ours, String> {
    let (design, warnings) = Source::Svg.read(bytes, stitchcraft_engine::design::Reading::default()).map_err(|e| e.to_string())?;
    let mut found: Vec<StitchType> = design.elements().iter().filter_map(|element| stitch_type_of(element, &design.settings)).collect();
    found.sort_by_key(|t| StitchType::ALL.iter().position(|all| all == t));
    found.dedup();
    traits.types = found.iter().map(|t| t.id()).collect();
    traits.settings = design
        .elements()
        .iter()
        .flat_map(|element| element.params.iter())
        .filter(|(_, value)| Traits::sets(value))
        .map(|(key, _)| key.to_string())
        .collect();
    let planned = stitchcraft_engine::plan(&design, &machine(), &Budget::DEFAULT);
    traits.said = warnings.iter().chain(&planned.diagnostics).map(|d| (d.code.id(), d.message.clone())).collect();
    let said = |code: Code| warnings.iter().chain(&planned.diagnostics).filter(|d| d.code == code).map(|d| d.message.clone()).collect::<Vec<_>>();
    let listed = said(Code::StitchTypeNotYet);
    if !listed.is_empty() {
        // `SC-W0011` names the method in code quotes.
        let named: Vec<String> =
            found.iter().filter(|t| listed.iter().any(|m| m.contains(&format!("`{}`", t.id())))).map(|t| t.id().to_string()).collect();
        return Ok(Ours { listed: if named.is_empty() { listed } else { named }, ..Ours::default() });
    }
    let refused = said(Code::ParamInvalid);
    let errors: Vec<String> = warnings
        .iter()
        .chain(&planned.diagnostics)
        .filter(|d| d.severity() == Severity::Error)
        .map(|d| format!("{}: {}", d.code.id(), d.message))
        .collect();
    // An element left out with an error leaves the rest of the design planned, but `stitch plan` writes no
    // file then, as Ink/Stitch writes none when an element fails.
    let (Some(plan), true) = (planned.plan, errors.is_empty()) else {
        return Ok(Ours { refused, nothing: errors, ..Ours::default() });
    };
    let pes = stitchcraft_formats::encode(&plan, stitchcraft_plan::FormatId::PesV1, "glyph").map_err(|e| format!("{name}: {e}"))?;
    let read = stitchcraft_formats::decode(&pes.bytes).map_err(|e| format!("its PES file doesn't read: {e}"))?.plan;
    Ok(Ours { plan: Some(read), refused, ..Ours::default() })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_shard_is_a_part_i_of_n_and_holds_every_n_th_glyph() {
        assert_eq!(Shard::parse("2/6").unwrap(), Shard { index: 2, count: 6 });
        assert_eq!(Shard::parse(" 6 / 6 ").unwrap(), Shard { index: 6, count: 6 });
        for wrong in ["0/6", "7/6", "a/6", "2", "2/0", "-1/6", ""] {
            assert_eq!(Shard::parse(wrong).unwrap_err(), format!("`{wrong}` is no part I/N, with I from 1 to N"));
        }
        let held = |shard: &str| (0..9).filter(|&job| Shard::parse(shard).unwrap().holds(job)).collect::<Vec<_>>();
        assert_eq!((held("1/3"), held("2/3"), held("3/3")), (vec![0, 3, 6], vec![1, 4, 7], vec![2, 5, 8]));
        assert_eq!(held("1/1"), (0..9).collect::<Vec<_>>());
    }

    #[test]
    fn the_sample_spreads_over_the_font_from_its_first_glyph() {
        assert_eq!(sampled(100, 4), [0, 25, 50, 75]);
        assert_eq!(sampled(7, 4), [0, 1, 3, 5]);
        assert_eq!(sampled(5, 4), [0, 1, 2, 3]);
        // A font of as many glyphs as the sample, or fewer, is sewn whole.
        assert_eq!(sampled(4, 4), [0, 1, 2, 3]);
        assert_eq!(sampled(3, 4), [0, 1, 2]);
        assert_eq!(sampled(0, 4), Vec::<usize>::new());
    }

    #[test]
    fn a_run_may_sew_the_glyphs_of_the_fonts_it_names_alone() {
        use crate::corpus::licence::Licence;
        let dir = util::root().join("target/xtask-tests/fonts-jobs");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("ltr.svg"), GLYPHS).unwrap();
        let font = |name: &str, licence: Licence| Font { name: name.into(), licence, files: vec!["ltr.svg".into()], dir: dir.clone() };
        let fonts = [font("a", Licence::Ofl), font("b", Licence::CcBySa), font("c", Licence::Gpl)];
        let options = |every: bool, named: &[&str]| Options { sample: 1, every, fonts: named.iter().map(|n| n.to_string()).collect(), shard: None };
        let sewn = |options: &Options| {
            let (jobs, problems) = jobs(&fonts, options);
            (jobs.into_iter().map(|job| format!("{} {}", job.font, job.name)).collect::<Vec<_>>(), problems)
        };
        // Every font the corpus takes, or those named alone; a font it doesn't take, or none of that name, is said.
        assert_eq!(sewn(&options(true, &[])), (vec!["a l".to_string(), "a x".into(), "b l".into(), "b x".into()], Vec::new()));
        assert_eq!(sewn(&options(true, &["b"])), (vec!["b l".to_string(), "b x".into()], Vec::new()));
        assert_eq!(sewn(&options(false, &["a", "b"])), (vec!["a l".to_string(), "b l".into()], Vec::new()));
        assert_eq!(
            sewn(&options(true, &["c", "d", "a"])),
            (
                vec!["a l".to_string(), "a x".into()],
                vec![
                    "there is no font `c` among the fonts the corpus takes".to_string(),
                    "there is no font `d` among the fonts the corpus takes".into()
                ]
            )
        );
    }

    #[test]
    fn a_glyph_s_design_is_written_under_its_font_and_file() {
        let job = Job { font: "f".into(), file: "ltr/latin.svg".into(), index: 7, name: "A".into() };
        assert_eq!(job.path(Path::new("/r")), Path::new("/r").join(WRITTEN).join("f").join("ltr/latin-0007.svg"));
    }

    #[test]
    fn verdicts_are_named() {
        let verdicts = [
            Verdict::Agrees,
            Verdict::Excused(Vec::new()),
            Verdict::Known(Vec::new()),
            Verdict::Differs(Vec::new()),
            Verdict::Listed(Vec::new()),
            Verdict::Failed(String::new()),
        ];
        assert_eq!(verdicts.iter().map(Verdict::name).collect::<Vec<_>>(), ["agrees", "excused", "known", "differs", "listed", "failed"]);
    }

    /// A glyph file of 2 glyphs: a running stitch and a guided fill.
    pub(super) const GLYPHS: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" xmlns:inkscape="http://www.inkscape.org/namespaces/inkscape" xmlns:inkstitch="http://inkstitch.org/namespace" width="20mm" height="20mm" viewBox="0 0 20 20">
  <g inkscape:groupmode="layer" inkscape:label="GlyphLayer-l" style="display:none"><path d="M2 2 L18 2" style="fill:none;stroke:#000000;stroke-width:0.26" inkstitch:running_stitch_length_mm="2" inkstitch:bean_stitch_repeats="0"/></g>
  <g inkscape:groupmode="layer" inkscape:label="GlyphLayer-x" style="display:none"><path d="M2 2 H18 V18 H2 Z" style="fill:#ed171f" inkstitch:fill_method="guided_fill"/></g>
</svg>"#;

    /// The design of the `index`-th glyph of the glyph file `text`.
    fn design(text: &str, index: usize) -> String {
        glyphs::designs(text, &|i| i == index).unwrap().into_iter().find_map(|glyph| glyph.design).unwrap()
    }

    #[test]
    fn stitchcraft_sews_a_glyph_and_reads_what_an_excuse_asks_about() {
        let mut traits = Traits::default();
        let made = ours(design(GLYPHS, 0).as_bytes(), "l", &mut traits).unwrap();
        assert_eq!((made.listed, made.refused, made.nothing), (Vec::<String>::new(), Vec::<String>::new(), Vec::<String>::new()));
        assert_eq!(traits.types, ["running_stitch"]);
        // A setting of 0 sets nothing.
        assert_eq!(traits.settings, BTreeSet::from(["running_stitch_length_mm".to_string()]));
        let measured = Measured::of(&made.plan.unwrap()).unwrap();
        assert_eq!(differential::compare(&measured, &measured).iter().filter(|row| !row.3).count(), 0);
        // What it says is kept with its code.
        let mut traits = Traits::default();
        let short = GLYPHS.replace(r#"inkstitch:running_stitch_length_mm="2""#, r#"inkstitch:repeats="0""#);
        ours(design(&short, 0).as_bytes(), "l", &mut traits).unwrap();
        assert!(traits.said.iter().any(|(code, message)| *code == "SC-W0102" && message.contains("`repeats` is 0")), "{:?}", traits.said);
    }

    #[test]
    fn stitchcraft_lists_the_stitch_types_it_doesn_t_sew_yet() {
        let mut traits = Traits::default();
        let made = ours(design(GLYPHS, 1).as_bytes(), "x", &mut traits).unwrap();
        assert_eq!((made.plan.is_none(), traits.types, made.listed), (true, vec!["guided_fill"], vec!["guided_fill".to_string()]));
        // A design StitchCraft can't read is said.
        assert!(ours(b"<svg", "y", &mut Traits::default()).is_err());
    }

    #[test]
    fn an_element_whose_setting_stitchcraft_cant_read_is_refused_and_said() {
        let refused = GLYPHS.replace(r#"stroke-width:0.26""#, r#"stroke-width:0.26" inkstitch:repeats="0 2""#);
        let made = ours(design(&refused, 0).as_bytes(), "l", &mut Traits::default()).unwrap();
        assert_eq!(made.refused.len(), 1, "{:?}", made.refused);
        assert!(made.refused.first().unwrap().contains("`repeats` is \"0 2\""), "{:?}", made.refused);
        // It sews nothing else, and its error is said.
        assert!(made.plan.is_none() && made.nothing.iter().any(|e| e.starts_with("SC-E0101: ")), "{:?}", made.nothing);
    }

    #[test]
    fn stitchcraft_writes_no_file_of_a_glyph_with_an_element_left_out_with_an_error() {
        // A satin column whose rung joins its first rail's start to its second's end has no section to sew
        // (SC-E0216). The line beside it is planned, but `stitch plan` writes no file, as Ink/Stitch writes none.
        let satin =
            r#"<path d="M 0,0 H 10 M 0,4 H 10 M -5,-2 L 15,6" style="fill:none;stroke:#000000;stroke-width:0.26" inkstitch:satin_column="true"/>"#;
        let text = GLYPHS.replace(r#"<path d="M2 2 L18 2""#, &format!(r#"{satin}<path d="M2 2 L18 2""#));
        let mut traits = Traits::default();
        let made = ours(design(&text, 0).as_bytes(), "l", &mut traits).unwrap();
        assert!(made.plan.is_none() && made.refused.is_empty(), "{made:?}");
        assert_eq!(made.nothing.len(), 1, "{:?}", made.nothing);
        assert!(made.nothing.first().unwrap().starts_with("SC-E0216: "), "{:?}", made.nothing);
        assert_eq!(traits.types, ["running_stitch", "satin_column"]);
    }

    #[test]
    fn stitchcraft_writes_no_file_of_a_glyph_of_one_empty_path() {
        // As Egyptian's spaces and Noble's tilde: a glyph whose one path draws nothing. Ink/Stitch writes no
        // file of it either, and the 2 agree.
        let empty = GLYPHS.replace(r#"<path d="M2 2 L18 2""#, r#"<path d="""#);
        let mut traits = Traits::default();
        let made = ours(design(&empty, 0).as_bytes(), " ", &mut traits).unwrap();
        assert!(made.plan.is_none() && made.refused.is_empty() && made.listed.is_empty(), "{made:?}");
        assert_eq!(made.nothing.len(), 1, "{:?}", made.nothing);
        assert!(made.nothing.first().unwrap().starts_with("SC-E0010: "), "{:?}", made.nothing);
    }

    #[test]
    fn a_glyph_larger_than_the_reference_hoop_is_planned_in_a_larger_one() {
        let large = GLYPHS.replace(r#"width="20mm" height="20mm" viewBox="0 0 20 20""#, r#"width="200mm" height="200mm" viewBox="0 0 20 20""#);
        let made = ours(design(&large, 0).as_bytes(), "l", &mut Traits::default()).unwrap();
        let bounds = made.plan.unwrap().bounds().unwrap();
        assert!(bounds.width() > profiles::REFERENCE.hoop.width.get(), "{}", bounds.width());
        let machine = machine();
        assert_eq!((machine.hoop.width.get(), machine.hoop.height.get(), machine.comfort), (HOOP, HOOP, None));
        assert_eq!(MachineProfile { hoop: profiles::REFERENCE.hoop, ..machine }, profiles::REFERENCE.clone());
    }
}
