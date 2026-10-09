//! The command line's grammar. `clap` turns it into parsing, `--help` and the generated reference page.

use std::path::PathBuf;

use clap::{Args, Parser, Subcommand, ValueEnum};
use stitchcraft_plan::FormatId;
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
error · 3 a file could not be read or written.

Coming with the roadmap (docs/src/plan/roadmap.md):
  stitch plan design.svg -o design.pes                      (M3)
  stitch export design.vectorcraft -o design.pes            (M6)";

/// The subcommands.
#[derive(Debug, Subcommand)]
pub enum Command {
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
    /// The machine profile, such as brother-200x200 (see `stitch profiles`).
    #[arg(long, short, required_unless_present = "list")]
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
    /// Also check the design against a machine profile, such as brother-200x200 (see `stitch profiles`).
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
