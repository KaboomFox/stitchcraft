//! The pictures of automatic digitizing in `cargo xtask perf` (`REQ-AUTO-083`, M13.32): where digitizing
//! spends its work, and a record that holds each picture's count to account.
//!
//! **Why pictures.** VectorCraft ends an export after about 11 billion instructions, and a traced dog's
//! export takes several times that. The benchmarks of [`crate::perf`] time one fill and one offset, and
//! nothing counted the work of a whole picture until the compatibility gate failed a test sheet's export. So
//! each picture here is `stitch` writing a machine file as VectorCraft's exporter writes it ([`Picture`]):
//! the 4 traced dogs and the pear with the Auto effect (`--auto`, since the fixtures carry no effect), and
//! the 6 TS-13 sheets, whose shapes are digitized automatically. TS-13B's word stands for lettering. Each
//! picture's arguments are kept with its record in `conformance/perf.toml`, where `stitch`'s own tests
//! check that it reads them (`apps/stitchcraft-cli/tests/pictures.rs`).
//!
//! **The build.** The `profiling` profile of the root `Cargo.toml`: the release build without link-time
//! optimization and with line tables. Link-time optimization inlines the stages into each other, and then
//! callgrind can't tell them apart. The count is the whole run of `stitch`, reading and writing included,
//! so it can't be lost the way a benchmark's counted function can.
//!
//! **Stages** ([`STAGES`]). Callgrind counts the instructions of each function with everything it calls,
//! on the lines of each source file apart, and each stage is the functions that do it ([`Part`]). A
//! function the compiler inlines into its caller has no count of its own, but its lines do: the specks,
//! the base layer and the outline are the lines of their files inlined into the neighbours' work. The
//! stages don't call each other, but for those a stage names as within it, so they add up, and what is
//! left is one more row. A stage at 0 in every picture means a function was renamed or inlined elsewhere:
//! the report says so, and the table here is changed with it. The table is a report: only each picture's
//! whole count is guarded.
//!
//! **What sews doesn't change.** Each sheet's file is compared with its golden file, byte for byte, so the
//! counted build is checked to sew as the tests' build does.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;

use serde::Deserialize;

/// A picture `cargo xtask perf` counts, its table under `[pictures]` in `conformance/perf.toml`: what
/// `stitch` is given to write its machine file, and its record.
#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Picture {
    /// `stitch`'s arguments, before `-o FILE`; paths from the repository root.
    pub args: Vec<String>,
    /// The golden file the machine file must equal, when the picture has one.
    #[serde(default)]
    pub golden: Option<String>,
    /// The instructions the run takes, as recorded.
    pub instructions: u64,
}

/// Part of a stage: a function as callgrind names it, and, for the code of another file inlined into it,
/// the end of that file's path. Callgrind counts a function's instructions on the lines of each file apart,
/// each with all that its code there calls, so code inlined from `specks.rs` into the neighbours' work is
/// counted on its own.
#[derive(Clone, Copy)]
pub struct Part {
    /// The function; a generic function's instances all count.
    pub function: &'static str,
    /// The file whose lines count, or every file.
    pub file: Option<&'static str>,
}

/// A function's whole count.
const fn whole(function: &'static str) -> Part {
    Part { function, file: None }
}

/// The lines of `file` inlined into `function`.
const fn lines(function: &'static str, file: &'static str) -> Part {
    Part { function, file: Some(file) }
}

/// A stage of automatic digitizing: its name in the report, its parts, and the stages within it, whose
/// counts are taken from its own.
pub struct Stage {
    /// The stage, as the design page's table names it.
    pub name: &'static str,
    /// What counts as the stage.
    pub parts: &'static [Part],
    /// The stages its parts call, whose work is theirs.
    pub within: &'static [&'static str],
}

const BESIDE: &str = "stitchcraft_engine::auto::neighbours::beside";
const PLAN: &str = "stitchcraft_engine::pipeline::plan";
const SPECKS: &str = "the specks";
const BASE: &str = "the base layer";
const ORDER: &str = "the search for the sewing order";
const OUTLINE: &str = "the outline";
const OVERLAPS: &str = "the neighbours' overlaps";

/// A stage that calls no other.
const fn stage(name: &'static str, parts: &'static [Part]) -> Stage {
    Stage { name, parts, within: &[] }
}

/// The stages, in the order a picture goes through them (`docs/src/design/guardrails.md`). None of their
/// parts calls another's, but for those in a stage's `within`.
pub const STAGES: [Stage; 16] = [
    stage("reading the shapes", &[whole("<stitchcraft_cli::commands::plan::Source>::read")]),
    stage("flattening the layers", &[whole("stitchcraft_engine::auto::neighbours::layers::flattened")]),
    stage("the split into parts by the rules", &[whole("stitchcraft_engine::auto::made_by")]),
    stage(SPECKS, &[lines(BESIDE, "auto/neighbours/specks.rs")]),
    stage(BASE, &[lines(BESIDE, "auto/neighbours/base.rs")]),
    stage(ORDER, &[whole("stitchcraft_engine::auto::neighbours::order::ordered")]),
    stage(OUTLINE, &[lines(BESIDE, "auto/neighbours/outline.rs")]),
    stage(OVERLAPS, &[whole("stitchcraft_engine::auto::neighbours::under::reached")]),
    Stage { name: "the rest of the neighbours' work", parts: &[whole(BESIDE)], within: &[SPECKS, BASE, ORDER, OUTLINE, OVERLAPS] },
    stage("the stitches of each fill", &[whole("<stitchcraft_engine::generate::FillMethod>::sew")]),
    stage("the stitches of each column", &[whole("stitchcraft_engine::generators::satin::satin_stitch")]),
    stage("the stitches of each line", &[whole("stitchcraft_engine::generate::stroke")]),
    stage("the travel to and from each shape", &[whole("stitchcraft_engine::generate::prepare"), whole("stitchcraft_engine::generate::approach")]),
    stage("the runs in place of jumps", &[lines(PLAN, "/moves.rs"), lines("stitchcraft_engine::assemble::assemble_routed", "/moves.rs")]),
    stage("the thickness measure", &[lines(PLAN, "auto/thick.rs")]),
    stage("the machine file", &[whole("stitchcraft_formats::pes::encode"), whole("stitchcraft_formats::read_back")]),
];

/// Where the profiling build puts `stitch` in `target`.
pub fn binary(target: &Path) -> PathBuf {
    target.join("profiling").join(format!("stitch{}", std::env::consts::EXE_SUFFIX))
}

/// The profiling build of `stitch`.
pub fn build() -> Command {
    let mut build = crate::util::cargo();
    build.args(["build", "--profile", "profiling", "--locked", "--package", "stitchcraft-cli", "--bin", "stitch"]);
    build
}

/// A picture's count by stage: each stage's instructions, in the order of [`STAGES`], and the rest.
#[derive(Debug, PartialEq)]
pub struct Staged {
    /// Each stage's instructions, those of the stages within it taken away.
    pub stages: Vec<(&'static str, u64)>,
    /// The instructions of the run that no stage has.
    pub rest: u64,
}

/// `total`'s instructions by stage, from what `callgrind_annotate --inclusive=yes` prints of the run.
pub fn staged(annotated: &str, total: u64) -> Staged {
    let rows = inclusive(annotated);
    // A function's rows overlap: the row of its own file has all its work, the code inlined from other
    // files included, and each other file's row has that file's part. So each function counts its largest
    // row that a part takes, and the functions of a stage add up.
    let counted = |stage: &Stage| {
        let mut largest: BTreeMap<&str, u64> = BTreeMap::new();
        for (file, function, n) in &rows {
            if stage.parts.iter().any(|part| part.counts(file, function)) {
                let kept = largest.entry(function.as_str()).or_insert(0);
                *kept = (*kept).max(*n);
            }
        }
        largest.values().copied().fold(0, u64::saturating_add)
    };
    let own: Vec<u64> = STAGES.iter().map(counted).collect();
    let stages: Vec<(&'static str, u64)> = STAGES
        .iter()
        .zip(&own)
        .map(|(stage, n)| {
            let within = STAGES.iter().zip(&own).filter(|(inner, _)| stage.within.contains(&inner.name)).map(|(_, n)| *n);
            (stage.name, n.saturating_sub(within.fold(0, u64::saturating_add)))
        })
        .collect();
    let counted = stages.iter().map(|(_, n)| *n).fold(0, u64::saturating_add);
    Staged { rest: total.saturating_sub(counted), stages }
}

impl Part {
    /// Whether callgrind's row of `function`, for the lines of `file`, counts as this part.
    fn counts(&self, file: &str, function: &str) -> bool {
        is(function, self.function) && self.file.is_none_or(|end| file.replace('\\', "/").ends_with(end))
    }
}

/// Whether callgrind's `name` is the function `function`, or an instance of it when it is generic. A
/// function's recursive calls (`name'2`) are inside its first call, and don't count again.
fn is(name: &str, function: &str) -> bool {
    if name.contains('\'') {
        return false;
    }
    name == function || name.strip_prefix(function).is_some_and(|rest| rest.starts_with("::<") || rest.starts_with("::{"))
}

/// Each row `callgrind_annotate --inclusive=yes --auto=no` lists: the file, the function and the
/// instructions of the function's code on that file's lines, with all it calls. A line of it is the count,
/// its share, then `file:function`, and the program in brackets. A line of a call, `=> file:function`,
/// is a call's count, which the function's own row has.
fn inclusive(annotated: &str) -> Vec<(String, String, u64)> {
    annotated
        .lines()
        .filter_map(|line| {
            let line = line.trim_start();
            let (count, rest) = line.split_once(' ')?;
            let count: u64 = count.replace(',', "").parse().ok()?;
            let rest = rest.trim_start();
            let rest = rest.strip_prefix('(').and_then(|r| r.split_once(')')).map_or(rest, |(_, after)| after).trim_start();
            if rest.starts_with("=>") {
                return None;
            }
            let rest = rest.split_once(" [").map_or(rest, |(name, _)| name);
            let (file, function) = located(rest);
            Some((file.to_string(), function.to_string(), count))
        })
        .collect()
}

/// The file and the function of callgrind's `file:function`, split at the first colon that isn't part of a
/// `::`; no file when there is no such colon.
fn located(row: &str) -> (&str, &str) {
    let bytes = row.as_bytes();
    let colon =
        (0..bytes.len()).find(|&i| bytes.get(i) == Some(&b':') && bytes.get(i + 1) != Some(&b':') && (i == 0 || bytes.get(i - 1) != Some(&b':')));
    colon.and_then(|i| Some((row.get(..i)?, row.get(i + 1..)?))).unwrap_or(("", row))
}

/// The stage table of the report: a row for each stage and one for the rest, with its share of `total`.
pub fn table(name: &str, staged: &Staged, total: u64) -> String {
    let mut out = format!("| {name} | instructions | share |\n|---|---:|---:|\n");
    let share = |n: u64| if total == 0 { 0.0 } else { to_f64(n) * 100.0 / to_f64(total) };
    for (stage, n) in staged.stages.iter().copied().chain([("the rest of the run", staged.rest)]) {
        out.push_str(&format!("| {stage} | {} | {:.1} % |\n", millions(n), share(n)));
    }
    out.push_str(&format!("| the whole run | {} | 100 % |\n", millions(total)));
    out
}

/// `n` instructions in millions, as the report gives them.
fn millions(n: u64) -> String {
    format!("{:.1} M", to_f64(n) / 1e6)
}

/// An instruction count as a float, exact below 2^53, which every count is.
fn to_f64(n: u64) -> f64 {
    #[allow(clippy::cast_precision_loss)]
    let float = n as f64;
    float
}

#[cfg(test)]
mod tests {
    use super::*;

    /// What callgrind_annotate prints of a run, cut down: the neighbours' work with the overlaps and an
    /// outline's inlined lines inside it, a generic instance, a recursive call, a call's line and a
    /// function of no stage.
    const PRINTED: &str = "\
--------------------------------------------------------------------------------
Ir                      file:function
--------------------------------------------------------------------------------
1,000,000 (100.0%)  ???:main [/target/profiling/stitch]
  900,000 (90.0%)  /src/crates/stitchcraft-engine/src/auto/neighbours/mod.rs:stitchcraft_engine::auto::neighbours::beside [/t/stitch]
  100,000 (10.0%)  /src/crates/stitchcraft-engine/src/auto/neighbours/outline.rs:stitchcraft_engine::auto::neighbours::beside [/t/stitch]
  700,000 (70.0%)  /src/x.rs:stitchcraft_engine::auto::neighbours::under::reached [/t/stitch]
  500,000 (50.0%)  /src/x.rs:stitchcraft_engine::auto::neighbours::under::reached'2 [/t/stitch]
   20,000 ( 2.0%)  /src/crates/stitchcraft-engine/src/auto/neighbours/outline.rs:stitchcraft_engine::auto::sewing [/t/stitch]
   60,000 ( 6.0%)  ???:<stitchcraft_engine::generate::FillMethod>::sew [/t/stitch]
   60,000 ( 6.0%)  => ???:<stitchcraft_engine::generate::FillMethod>::sew (1x)
   40,000 ( 4.0%)  ???:stitchcraft_formats::pes::encode::<alloc::vec::Vec<u8>> [/t/stitch]
   30,000 ( 3.0%)  ???:stitchcraft_formats::pes::encoded [/t/stitch]
";

    #[test]
    fn req_auto_083_a_picture_s_count_is_split_into_its_stages_and_the_rest() {
        let staged = staged(PRINTED, 1_000_000);
        let of = |stage: &str| staged.stages.iter().find(|(name, _)| *name == stage).unwrap().1;
        // The overlaps once, though callgrind lists their recursive call, and the outline's lines inlined into
        // the neighbours' work, and not those inlined elsewhere.
        assert_eq!(of(OVERLAPS), 700_000);
        assert_eq!(of(OUTLINE), 100_000);
        // The neighbours' own work is the row of their own file, which has the outline's lines too, less the
        // stages within it.
        assert_eq!(of("the rest of the neighbours' work"), 900_000 - 700_000 - 100_000);
        // A call's line doesn't count again.
        assert_eq!(of("the stitches of each fill"), 60_000);
        // A generic instance counts, and a function whose name only starts like it doesn't.
        assert_eq!(of("the machine file"), 40_000);
        assert_eq!(of("reading the shapes"), 0);
        assert_eq!(staged.stages.len(), STAGES.len());
        assert_eq!(staged.rest, 1_000_000 - 700_000 - 100_000 - 100_000 - 60_000 - 40_000);
        // The stages never add up to more than the run.
        assert_eq!(super::staged(PRINTED, 10).rest, 0);
    }

    #[test]
    fn a_function_is_found_by_its_name_its_instances_and_its_file() {
        assert!(is("stitchcraft_formats::read_back", "stitchcraft_formats::read_back"));
        assert!(is("stitchcraft_formats::read_back::<u8>", "stitchcraft_formats::read_back"));
        assert!(is("stitchcraft_formats::read_back::{closure#0}", "stitchcraft_formats::read_back"));
        assert!(!is("stitchcraft_formats::read_backs", "stitchcraft_formats::read_back"));
        assert!(!is("stitchcraft_formats::read_back'2", "stitchcraft_formats::read_back"));
        assert_eq!(located("???:std::rt::lang_start"), ("???", "std::rt::lang_start"));
        assert_eq!(located("src/a.rs:<a::B>::c"), ("src/a.rs", "<a::B>::c"));
        assert_eq!(located("a::b"), ("", "a::b"));
        let part = lines(BESIDE, "auto/neighbours/base.rs");
        assert!(part.counts("/w/crates/stitchcraft-engine/src/auto/neighbours/base.rs", BESIDE));
        assert!(part.counts("C:\\w\\src\\auto\\neighbours\\base.rs", BESIDE));
        assert!(!part.counts("/w/src/auto/neighbours/mod.rs", BESIDE));
        assert!(!part.counts("/w/src/auto/neighbours/base.rs", PLAN));
        assert!(whole(PLAN).counts("???", PLAN));
    }

    #[test]
    fn each_stage_within_another_is_a_stage() {
        for stage in &STAGES {
            for inner in stage.within {
                assert!(STAGES.iter().any(|each| each.name == *inner && each.within.is_empty()), "{}: {inner}", stage.name);
            }
            assert!(!stage.parts.is_empty(), "{}", stage.name);
        }
    }

    #[test]
    fn the_table_gives_each_stage_the_rest_and_the_whole_run() {
        let staged = Staged { stages: vec![("the neighbours' overlaps", 750_000)], rest: 250_000 };
        let table = table("dog", &staged, 1_000_000);
        assert!(table.starts_with("| dog | instructions | share |\n|---|---:|---:|\n"), "{table}");
        assert!(table.contains("| the neighbours' overlaps | 0.8 M | 75.0 % |\n"), "{table}");
        assert!(table.contains("| the rest of the run | 0.2 M | 25.0 % |\n"), "{table}");
        assert!(table.ends_with("| the whole run | 1.0 M | 100 % |\n"), "{table}");
        assert!(super::table("none", &Staged { stages: Vec::new(), rest: 0 }, 0).contains("| 0.0 M | 0.0 % |"));
    }

    #[test]
    fn the_profiling_build_is_where_cargo_puts_it() {
        assert_eq!(binary(Path::new("/t")), Path::new(&format!("/t/profiling/stitch{}", std::env::consts::EXE_SUFFIX)));
    }
}
