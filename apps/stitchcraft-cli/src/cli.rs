//! The command line's grammar. `clap` turns it into parsing, `--help` and the generated reference page.

use std::path::PathBuf;

use clap::{Args, Parser, Subcommand, ValueEnum};
use stitchcraft_plan::FormatId;

/// stitch — StitchCraft machine embroidery.
#[derive(Debug, Parser)]
#[command(name = "stitch", version, about, long_about = None, after_help = AFTER_HELP)]
pub struct Cli {
    /// What to do.
    #[command(subcommand)]
    pub command: Command,
}

const AFTER_HELP: &str = "\
Exit status: 0 done (warnings allowed) · 1 the design has errors, nothing written · 2 usage error ·
3 a file could not be read or written.

Coming with the roadmap (docs/src/plan/roadmap.md):
  stitch inspect FILE                                       (M2)
  stitch plan design.svg -o design.pes                      (M3)
  stitch export design.vectorcraft -o design.pes            (M6)";

/// The subcommands.
#[derive(Debug, Subcommand)]
pub enum Command {
    /// Write a machine-checkpoint test sheet (docs/src/plan/machine-testing.md).
    Testsheet(TestsheetArgs),
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
