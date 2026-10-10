//! The command line's grammar. `clap` turns it into parsing, `--help` and the generated reference page.

use std::path::PathBuf;

use clap::{Args, Parser, Subcommand, ValueEnum};
use stitchcraft_plan::FormatId;
use stitchcraft_plan::profiles::REFERENCE;
use stitchcraft_render::{Settings, Style};

/// stitch — StitchCraft machine embroidery.
#[derive(Debug, Parser)]
#[command(name = "stitch", version, about, long_about = None, after_help = AFTER_HELP)]
pub struct Cli {
    /// What to do.
    #[command(subcommand)]
    pub command: Command,
}

const AFTER_HELP: &str = "\
Exit status: 0 done (warnings allowed) · 1 the design or file has errors (nothing written) · 2 usage
error · 3 a file could not be read or written · 4 a bug in StitchCraft (a bug-report bundle is written
where it can be: see `stitch bug-report`).

Coming with the roadmap (docs/src/plan/roadmap.md):
  stitch export design.vectorcraft -o design.pes            (M6)";

/// The subcommands.
#[derive(Debug, Subcommand)]
pub enum Command {
    /// Plan a design (SVG) for a machine and write the machine file, with a picture and a report if
    /// asked.
    Plan(PlanArgs),
    /// Write a machine-checkpoint test sheet (docs/src/plan/machine-testing.md).
    Testsheet(TestsheetArgs),
    /// Read a machine file (PES, PEC or DST) and describe it: size, stitches, threads, stitch lengths.
    Inspect(InspectArgs),
    /// Draw a machine file (PES, PEC or DST) as a picture of what it will sew (PNG).
    Preview(PreviewArgs),
    /// Write a machine file (PES, PEC or DST) in another format.
    Convert(ConvertArgs),
    /// List the built-in machine profiles.
    Profiles,
    /// Explain a diagnostic code, such as SC-W0702.
    Explain {
        /// The code.
        code: String,
    },
    /// Write a bug-report bundle that reproduces what StitchCraft does with a design, or replay one.
    ///
    /// The bundle is one file to attach to an issue, and it holds the design. With --replay, plan a
    /// bundle's design again and compare; the exit status is 1 when it does not reproduce.
    BugReport(BugReportArgs),
}

/// `stitch plan`.
#[derive(Debug, Args)]
pub struct PlanArgs {
    /// The design (SVG).
    pub design: PathBuf,
    /// The machine file to write.
    #[arg(long, short)]
    pub output: PathBuf,
    /// The machine profile (see `stitch profiles`).
    #[arg(long, short, default_value = REFERENCE.id)]
    pub profile: String,
    /// The file format; by default the output file's extension decides.
    #[arg(long, value_enum)]
    pub format: Option<Format>,
    /// Also draw a picture of what the file will sew (PNG).
    #[arg(long)]
    pub preview: Option<PathBuf>,
    /// Also write a report of the plan and what was said about it (JSON), even when nothing else is
    /// written.
    #[arg(long)]
    pub report: Option<PathBuf>,
}

/// `stitch bug-report`.
#[derive(Debug, Args)]
pub struct BugReportArgs {
    /// The design (SVG) StitchCraft gets wrong.
    #[arg(required_unless_present = "replay")]
    pub design: Option<PathBuf>,
    /// Replay this bundle instead: plan its design again and say whether everything comes out the same.
    #[arg(long, conflicts_with_all = ["design", "output", "format", "says"])]
    pub replay: Option<PathBuf>,
    /// The bundle to write; by default next to the design, as DESIGN.bug-report.json.
    #[arg(long, short)]
    pub output: Option<PathBuf>,
    /// The machine profile (see `stitch profiles`).
    #[arg(long, short, default_value = REFERENCE.id)]
    pub profile: String,
    /// The file format; by default the profile's.
    #[arg(long, value_enum)]
    pub format: Option<Format>,
    /// What goes wrong, in your words; it goes into the bundle.
    #[arg(long)]
    pub says: Option<String>,
}

/// `stitch testsheet`.
#[derive(Debug, Args)]
pub struct TestsheetArgs {
    /// The sheet, such as TS-01 (see --list).
    #[arg(required_unless_present = "list")]
    pub sheet: Option<String>,
    /// List the test sheets and exit.
    #[arg(long)]
    pub list: bool,
    /// The machine profile to check the sheet against (see `stitch profiles`); by default the one for
    /// the hoop the sheet is sewn in.
    #[arg(long, short)]
    pub profile: Option<String>,
    /// The file to write.
    #[arg(long, short, required_unless_present = "list")]
    pub output: Option<PathBuf>,
    /// The file format; by default the output file's extension decides.
    #[arg(long, value_enum)]
    pub format: Option<Format>,
}

/// `stitch inspect`.
#[derive(Debug, Args)]
pub struct InspectArgs {
    /// The machine file.
    pub file: PathBuf,
    /// Also check the design against a machine profile (see `stitch profiles`).
    #[arg(long, short)]
    pub profile: Option<String>,
}

/// `stitch preview`.
#[derive(Debug, Args)]
pub struct PreviewArgs {
    /// The machine file.
    pub file: PathBuf,
    /// The picture to write (PNG).
    #[arg(long, short)]
    pub output: PathBuf,
    /// How the picture looks.
    #[arg(long, value_enum, default_value_t = PreviewStyle::Realistic)]
    pub style: PreviewStyle,
    /// Pixels per millimetre.
    #[arg(long, default_value_t = Settings::DEFAULT_SCALE)]
    pub scale: f32,
}

/// Preview styles on the command line.
#[derive(Clone, Copy, Debug, PartialEq, Eq, ValueEnum)]
pub enum PreviewStyle {
    /// The finished embroidery on fabric, with the jump threads the machine leaves for you to cut.
    Realistic,
    /// Every stitch, needle hole, move, trim and stop, for checking a design.
    Simple,
}

impl From<PreviewStyle> for Style {
    fn from(style: PreviewStyle) -> Style {
        match style {
            PreviewStyle::Realistic => Style::Realistic,
            PreviewStyle::Simple => Style::Simple,
        }
    }
}

/// `stitch convert`.
#[derive(Debug, Args)]
pub struct ConvertArgs {
    /// The machine file to read.
    pub file: PathBuf,
    /// The file to write.
    #[arg(long, short)]
    pub output: PathBuf,
    /// The file format; by default the output file's extension decides.
    #[arg(long, value_enum)]
    pub format: Option<Format>,
}

/// File formats on the command line.
#[derive(Clone, Copy, Debug, PartialEq, Eq, ValueEnum)]
pub enum Format {
    /// Brother PES, version 1.
    Pes,
    /// Tajima DST.
    Dst,
}

impl From<Format> for FormatId {
    fn from(format: Format) -> FormatId {
        match format {
            Format::Pes => FormatId::PesV1,
            Format::Dst => FormatId::Dst,
        }
    }
}
