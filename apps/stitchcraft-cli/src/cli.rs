//! The command line's grammar. `clap` turns it into parsing, `--help` and the generated reference page.

use std::path::PathBuf;

use clap::{Args, Parser, Subcommand, ValueEnum};
use stitchcraft_core::Mm;
use stitchcraft_engine::auto::AutoParams;
use stitchcraft_engine::auto::rules::{PRESETS, Preset};
use stitchcraft_engine::design::Reading;
use stitchcraft_plan::FormatId;
use stitchcraft_render::{Settings, Style};
use stitchcraft_svg::lettering::{self, Align, ColourSort, TextPosition, Trim, VerticalPosition};

/// stitch — StitchCraft machine embroidery.
#[derive(Debug, Parser)]
#[command(name = "stitch", version = VERSION, about, long_about = None, after_help = AFTER_HELP)]
pub struct Cli {
    /// What to do.
    #[command(subcommand)]
    pub command: Command,
}

/// The version `stitch --version` shows: the workspace's, and the commit it was built from when it was built
/// from a git checkout (`build.rs`).
const VERSION: &str = if env!("STITCHCRAFT_COMMIT").is_empty() {
    env!("CARGO_PKG_VERSION")
} else {
    concat!(env!("CARGO_PKG_VERSION"), " (commit ", env!("STITCHCRAFT_COMMIT"), ")")
};

const AFTER_HELP: &str = "\
Exit status: 0 done (warnings allowed) · 1 the design or file has errors (nothing written) · 2 usage
error · 3 a file could not be read or written · 4 a bug in StitchCraft (a bug-report bundle is written
where it can be: see `stitch bug-report`).";

/// The subcommands.
#[derive(Debug, Subcommand)]
pub enum Command {
    /// Plan a design, an SVG file or a VectorCraft document, for a machine and write the machine file,
    /// with a picture and a report if asked. A picture, a PNG or JPEG file, is traced with VectorCraft's
    /// command line first.
    #[command(visible_alias = "export")]
    Plan(PlanArgs),
    /// Expand an automatic design: write it as the ordinary shapes automatic digitizing makes of it, each
    /// with its own StitchCraft effect, to edit one by one. A VectorCraft document keeps its traced shapes,
    /// hidden and locked, under a new group of the parts. An SVG file has the parts alone. The expanded
    /// design sews as the automatic one did, and no longer changes with the picture or the Auto effect.
    Expand(ExpandArgs),
    /// Set a text in an embroidery font, a folder in Ink/Stitch's font format, plan it for a machine and
    /// write the machine file, with a picture if asked.
    Lettering(LetteringArgs),
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
    /// Write a machine's threads as a GIMP palette (.gpl), which VectorCraft loads as a swatch library, so
    /// that Image Trace traces a picture in them.
    Palette(PaletteArgs),
    /// Cut a picture's subject out, a PNG or JPEG file, as `stitch plan --subject` does before VectorCraft
    /// traces it, and write the cut-out alone (PNG): to place in VectorCraft instead of the picture.
    Cutout(CutoutArgs),
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

/// `stitch expand`.
#[derive(Debug, Args)]
pub struct ExpandArgs {
    /// The design: a VectorCraft document (.vectorcraft), an SVG file, or a picture (.png, .jpg or .jpeg),
    /// which VectorCraft's command line traces first, as `stitch plan` traces it. Every shape without
    /// settings is digitized automatically, as `stitch plan --auto` digitizes it.
    pub design: PathBuf,
    /// The expanded design to write: a VectorCraft document (.vectorcraft), from a VectorCraft document or a
    /// picture, or an SVG file with Ink/Stitch's attributes (.svg).
    #[arg(long, short)]
    pub output: PathBuf,
    /// The machine profile whose threads name the parts (see `stitch profiles`); by default the one a
    /// VectorCraft document keeps, or else the reference machine. The document written keeps it.
    #[arg(long, short)]
    pub profile: Option<String>,
    /// How the design is read.
    #[command(flatten)]
    pub reading: ReadingArgs,
    /// How a picture is traced.
    #[command(flatten)]
    pub tracing: TracingArgs,
}

/// `stitch plan`.
#[derive(Debug, Args)]
pub struct PlanArgs {
    /// The design: an SVG file, a VectorCraft document (.vectorcraft), or a picture (.png, .jpg or .jpeg),
    /// which VectorCraft's command line traces in the machine's threads and StitchCraft digitizes
    /// automatically.
    pub design: PathBuf,
    /// The machine file to write.
    #[arg(long, short)]
    pub output: PathBuf,
    /// The machine profile (see `stitch profiles`); by default the one a VectorCraft document keeps, which
    /// StitchCraft's Design Settings filter sets, or else the reference machine, which `stitch profiles`
    /// lists first.
    #[arg(long, short)]
    pub profile: Option<String>,
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
    /// How the design is read.
    #[command(flatten)]
    pub reading: ReadingArgs,
    /// How a picture is traced.
    #[command(flatten)]
    pub tracing: TracingArgs,
    /// How a picture's subject is cut out first.
    #[command(flatten)]
    pub subject: SubjectArgs,
}

/// `stitch lettering`. The help of the options that set the text is the settings' own
/// ([`stitchcraft_svg::lettering::SETTINGS`]).
#[derive(Debug, Args)]
pub struct LetteringArgs {
    /// The text. A backslash and n (`\n`) ends a line.
    pub text: String,
    /// The font: a folder in Ink/Stitch's font format, with its font.json and its glyph files.
    #[arg(long, value_name = "FOLDER")]
    pub font: PathBuf,
    /// The machine file to write.
    #[arg(long, short)]
    pub output: PathBuf,
    /// The machine profile (see `stitch profiles`); by default the reference machine, which `stitch profiles`
    /// lists first.
    #[arg(long, short)]
    pub profile: Option<String>,
    /// The file format; by default the output file's extension decides.
    #[arg(long, value_enum)]
    pub format: Option<Format>,
    /// Also draw a picture of what the file will sew (PNG).
    #[arg(long)]
    pub preview: Option<PathBuf>,
    #[arg(long, value_name = lettering::SCALE.takes, value_parser = number, conflicts_with = "height", help = lettering::SCALE.does)]
    #[allow(missing_docs)] // Its help is the setting's own, above.
    pub scale: Option<f64>,
    /// The height of the text in millimetres, in place of --scale: the scale is the height over the font's
    /// size, the height of its reference capital.
    #[arg(long, value_name = "MM", value_parser = number)]
    pub height: Option<f64>,
    #[arg(long, value_name = lettering::TEXT_ALIGN.takes, value_parser = align, default_value = lettering::TEXT_ALIGN.default, help = lettering::TEXT_ALIGN.does)]
    #[allow(missing_docs)] // Its help is the setting's own, above.
    pub align: Align,
    #[arg(long, value_name = lettering::LETTER_SPACING.takes, value_parser = number, default_value = lettering::LETTER_SPACING.default, allow_hyphen_values = true, help = lettering::LETTER_SPACING.does)]
    #[allow(missing_docs)] // Its help is the setting's own, above.
    pub letter_spacing: f64,
    #[arg(long, value_name = lettering::WORD_SPACING.takes, value_parser = number, default_value = lettering::WORD_SPACING.default, allow_hyphen_values = true, help = lettering::WORD_SPACING.does)]
    #[allow(missing_docs)] // Its help is the setting's own, above.
    pub word_spacing: f64,
    #[arg(long, value_name = lettering::LINE_HEIGHT.takes, value_parser = number, default_value = lettering::LINE_HEIGHT.default, allow_hyphen_values = true, help = lettering::LINE_HEIGHT.does)]
    #[allow(missing_docs)] // Its help is the setting's own, above.
    pub line_height: f64,
    #[arg(long, value_name = lettering::TRIM.takes, value_parser = trim, default_value = lettering::TRIM.default, help = lettering::TRIM.does)]
    #[allow(missing_docs)] // Its help is the setting's own, above.
    pub trim: Trim,
    #[arg(long, value_name = lettering::COLOR_SORT.takes, value_parser = colour_sort, default_value = lettering::COLOR_SORT.default, help = lettering::COLOR_SORT.does)]
    #[allow(missing_docs)] // Its help is the setting's own, above.
    pub color_sort: ColourSort,
    #[arg(long, value_name = lettering::BACK_AND_FORTH.takes, action = clap::ArgAction::Set, default_value = lettering::BACK_AND_FORTH.default, help = lettering::BACK_AND_FORTH.does)]
    #[allow(missing_docs)] // Its help is the setting's own, above.
    pub back_and_forth: bool,
    #[arg(long, value_name = lettering::WIDEN_PERCENT.takes, value_parser = number, default_value = lettering::WIDEN_PERCENT.default, allow_hyphen_values = true, help = lettering::WIDEN_PERCENT.does)]
    #[allow(missing_docs)] // Its help is the setting's own, above.
    pub widen_percent: f64,
    /// Set the text along a path of this SVG file, in the file's place on its page: the path labelled
    /// `batch lettering`, as Ink/Stitch's batch lettering takes it, or else the file's first path that
    /// paints. The text follows the path's first subpath.
    #[arg(long, value_name = "FILE")]
    pub path: Option<PathBuf>,
    #[arg(long, value_name = lettering::TEXT_POSITION.takes, value_parser = text_position, default_value = lettering::TEXT_POSITION.default, requires = "path", help = lettering::TEXT_POSITION.does)]
    #[allow(missing_docs)] // Its help is the setting's own, above.
    pub text_position: TextPosition,
    #[arg(long, value_name = lettering::VERTICAL_TEXT_POSITION.takes, value_parser = vertical_text_position, default_value = lettering::VERTICAL_TEXT_POSITION.default, requires = "path", help = lettering::VERTICAL_TEXT_POSITION.does)]
    #[allow(missing_docs)] // Its help is the setting's own, above.
    pub vertical_text_position: VerticalPosition,
}

/// The value `name` of the option `what`, one of `all` by its id.
fn choice<T: Copy>(what: &str, name: &str, all: &[T], id: fn(T) -> &'static str) -> Result<T, String> {
    let ids: Vec<&str> = all.iter().map(|one| id(*one)).collect();
    all.iter().copied().find(|one| id(*one) == name).ok_or_else(|| format!("`{name}` isn't {what}: they are {}", ids.join(", ")))
}

/// Where trims go, as `--trim` takes it.
fn trim(name: &str) -> Result<Trim, String> {
    choice("a place to trim", name, &Trim::ALL, Trim::id)
}

/// What is sorted by colour, as `--color-sort` takes it.
fn colour_sort(name: &str) -> Result<ColourSort, String> {
    choice("a colour sort", name, &ColourSort::ALL, ColourSort::id)
}

/// Where the text lies along its path, as `--text-position` takes it.
fn text_position(name: &str) -> Result<TextPosition, String> {
    choice("a text position", name, &TextPosition::ALL, TextPosition::id)
}

/// Which part of the text lies on its path, as `--vertical-text-position` takes it.
fn vertical_text_position(name: &str) -> Result<VerticalPosition, String> {
    choice("a vertical text position", name, &VerticalPosition::ALL, VerticalPosition::id)
}

/// A number, as lettering's options take it: one that is finite.
fn number(text: &str) -> Result<f64, String> {
    text.parse::<f64>().ok().filter(|number| number.is_finite()).ok_or_else(|| format!("`{text}` isn't a number"))
}

/// The alignment called `name`, as `--align` takes it.
fn align(name: &str) -> Result<Align, String> {
    Align::named(name).ok_or_else(|| format!("`{name}` isn't an alignment: they are {}", Align::ALL.map(Align::id).join(", ")))
}

/// `--subject` and the options that go with it: a picture's subject cut out before VectorCraft traces it
/// (ADR-0019).
#[derive(Debug, Default, Args)]
pub struct SubjectArgs {
    /// Cut the picture's subject out before VectorCraft traces it, so that its background isn't sewn: the
    /// rest is made transparent, and the picture cropped to what is kept.
    #[arg(long)]
    pub subject: bool,
    /// Where the subject is, and isn't.
    #[command(flatten)]
    pub marks: MarkArgs,
    /// Also write the cut-out (PNG), with the other files, to see what was kept.
    #[arg(long, value_name = "FILE")]
    pub cutout: Option<PathBuf>,
}

impl SubjectArgs {
    /// Whether any option is given, which only a picture takes.
    pub fn given(&self) -> bool {
        self.subject || self.marks.given() || self.cutout.is_some()
    }
}

/// The marks that correct a cut-out, in the picture's pixels as its file stores them, from its left and top
/// edges.
#[derive(Debug, Default, Args)]
pub struct MarkArgs {
    /// The subject's box, from its left and top edges to its right and bottom ones; by default the box round
    /// the main subject that U²-Net-p finds.
    #[arg(long, value_name = "X0,Y0,X1,Y1", value_parser = stitchcraft_cutout::PixelBox::parse, allow_hyphen_values = true)]
    pub subject_box: Option<stitchcraft_cutout::PixelBox>,
    /// A point of the subject, as often as needed.
    #[arg(long, value_name = "X,Y", value_parser = stitchcraft_cutout::Point::parse, allow_hyphen_values = true)]
    pub subject_point: Vec<stitchcraft_cutout::Point>,
    /// A point of what isn't the subject, as often as needed.
    #[arg(long, value_name = "X,Y", value_parser = stitchcraft_cutout::Point::parse, allow_hyphen_values = true)]
    pub not_subject: Vec<stitchcraft_cutout::Point>,
}

impl MarkArgs {
    /// Whether any mark is given.
    pub fn given(&self) -> bool {
        self.marks().given()
    }

    /// The marks, as the cut-out takes them.
    pub fn marks(&self) -> stitchcraft_cutout::Marks {
        stitchcraft_cutout::Marks { subject_box: self.subject_box, subject: self.subject_point.clone(), not_subject: self.not_subject.clone() }
    }
}

/// `stitch cutout`.
#[derive(Debug, Args)]
pub struct CutoutArgs {
    /// The picture: a PNG or JPEG file.
    pub picture: PathBuf,
    /// The cut-out to write (PNG).
    #[arg(long, short)]
    pub output: PathBuf,
    /// Where the subject is, and isn't.
    #[command(flatten)]
    pub marks: MarkArgs,
}

/// How `stitch plan` traces a picture with VectorCraft's command line (ADR-0015): options for a picture
/// only, which another design refuses.
#[derive(Debug, Default, Args)]
pub struct TracingArgs {
    /// VectorCraft's command line, which traces a picture; by default `vectorcraft-cli` on the PATH.
    #[arg(long, value_name = "PROGRAM")]
    pub vectorcraft_cli: Option<PathBuf>,
    /// How large a picture is sewn: its longer side, in millimetres, its background included; by default
    /// as large as the machine's hoop holds.
    #[arg(long, value_name = "MM", value_parser = positive_mm)]
    pub size: Option<Mm>,
    /// How many colours a picture has, its background included: Image Trace traces it in at most that many
    /// of the machine's threads, 2 to 256; by default 6.
    #[arg(long, value_name = "N", value_parser = clap::value_parser!(u32).range(2..=256))]
    pub colours: Option<u32>,
    /// Trace a picture with fewer points: Image Trace's Paths and Corners at their lowest, which draw its
    /// outlines with about a third fewer anchors and fewer jags. Off by default, since it changes how many
    /// shapes of a traced picture sew.
    #[arg(long)]
    pub fewer_points: bool,
    /// Give a colour of a picture a thread at any --colours: a thread of the machine by its name, in any
    /// case; a colour as #rrggbb, for the thread nearest it; or a pixel of the picture as X,Y, from its left
    /// and top edges, for the thread nearest its colour. May be given several times. The picture is then
    /// traced twice, the second time in the threads the first chose and the kept ones.
    #[arg(long, value_name = "COLOUR", value_parser = crate::colours::colour)]
    pub keep_colour: Vec<crate::colours::Colour>,
    /// Trace a picture without a thread of the machine, so that its colours take the nearest of the others:
    /// a colour read as --keep-colour reads it. May be given several times.
    #[arg(long, value_name = "COLOUR", value_parser = crate::colours::colour)]
    pub drop_colour: Vec<crate::colours::Colour>,
    /// Also write the document VectorCraft traced a picture into (.vectorcraft), to correct the trace in
    /// VectorCraft.
    #[arg(long, value_name = "FILE")]
    pub traced: Option<PathBuf>,
}

impl TracingArgs {
    /// Whether any option is given, which only a picture takes.
    pub fn given(&self) -> bool {
        let TracingArgs { vectorcraft_cli, size, colours, fewer_points, keep_colour, drop_colour, traced } = self;
        let colours_named = !keep_colour.is_empty() || !drop_colour.is_empty();
        vectorcraft_cli.is_some() || size.is_some() || colours.is_some() || *fewer_points || colours_named || traced.is_some()
    }
}

/// A length of more than 0 millimetres, as `--size` takes it.
fn positive_mm(text: &str) -> Result<Mm, String> {
    let mm = text.trim().parse::<f64>().ok().and_then(|value| Mm::new(value).ok()).filter(|mm| mm.get() > 0.0);
    mm.ok_or_else(|| format!("`{text}` isn't a positive number of millimetres"))
}

/// How `stitch plan` and `stitch bug-report` read a design: the switches both take, which make its
/// [`Reading`].
#[derive(Debug, Default, Args)]
pub struct ReadingArgs {
    /// Digitize automatically the shapes that have no settings: StitchCraft chooses the stitches of each by
    /// its width (docs/src/design/image-to-embroidery.md). A shape with settings is left to them.
    #[arg(long)]
    pub auto: bool,
    /// Sew the shapes with settings in the order StitchCraft chooses too, as --auto sews the shapes it
    /// digitizes: shapes that overlap keep their paint order, and each thread then sews as long as it can,
    /// the nearest shape next (docs/src/design/image-to-embroidery.md).
    #[arg(long)]
    pub sort_colours: bool,
    /// The style of automatic digitizing: a named set of its rules' settings, set on every shape digitized
    /// automatically that doesn't set them itself. `logo` suits flat art drawn in strokes, and `photo` a
    /// traced photo (docs/src/user/reference/auto-rules.md).
    #[arg(long, value_parser = style)]
    pub style: Option<&'static Preset>,
    /// Set a setting of automatic digitizing's rules, as KEY=VALUE, on every shape digitized automatically
    /// that doesn't set it itself, such as auto_min_slenderness=4. A setting given here goes before the
    /// style's (docs/src/user/reference/params/auto.md).
    #[arg(long = "set", value_name = "KEY=VALUE", value_parser = rule)]
    pub set: Vec<(String, String)>,
    /// Sew the traced letters inside an area as lettering in an embroidery font, as tall as they are: the
    /// area's LEFT,TOP,WIDTH,HEIGHT in percent of the box round the design's shapes, which is a traced
    /// picture's own box, then a colon and the text, such as 10,40,80,20:MATCH. A font is named after the
    /// text in double quotes, as "MATCH" font=NAME, and is found in the folder STITCHCRAFT_FONTS names
    /// (docs/src/user/how-to/letter-a-picture.md). May be given several times.
    #[arg(long = "text", value_name = "AREA:TEXT", value_parser = crate::text::mark)]
    pub text: Vec<(String, String)>,
    /// Keep the parts of the design inside an area from merging into their neighbours: each part of a shape
    /// digitized automatically that lies wholly inside it sews by its width, where it would merge as a speck
    /// or a thin line. The area is LEFT,TOP,WIDTH,HEIGHT in percent of the box round the design's shapes,
    /// such as 62,18,6,5 round an eye (docs/src/user/how-to/keep-small-parts.md). May be given several
    /// times.
    #[arg(long = "keep", value_name = "AREA", value_parser = crate::keep::mark)]
    pub keep: Vec<(String, String)>,
    /// Draw a guide over the design: each filled part of a shape digitized automatically that it crosses
    /// sews as a guided fill whose rows follow it. The guide is 2 points or more, X,Y,X,Y… in percent of the
    /// box round the design's shapes, joined by straight lines, such as 20,80,45,50,60,20 along a leaf
    /// (docs/src/user/how-to/guide-the-rows.md). May be given several times.
    #[arg(long = "guide", value_name = "POINTS", value_parser = crate::guide::guide, allow_hyphen_values = true)]
    pub guide: Vec<(String, String)>,
}

impl ReadingArgs {
    /// The settings of automatic digitizing's rules that the options set on every automatic shape, each
    /// key once: those of `--set`, the last of a key given twice, then the style's for the keys not set.
    pub fn rules(&self) -> Vec<(String, String)> {
        let mut rules: Vec<(String, String)> = Vec::new();
        let styled = self.style.iter().flat_map(|preset| preset.values).map(|(key, value)| (key.to_string(), value.to_string()));
        for (key, value) in self.set.iter().rev().cloned().chain(styled) {
            if rules.iter().all(|(set, _)| *set != key) {
                rules.push((key, value));
            }
        }
        // The marks of `--text` travel with the rules, every one of them, under the marker's name.
        rules.extend(self.text.iter().cloned());
        // So do the marks of `--keep`, and the guides of `--guide`.
        rules.extend(self.keep.iter().cloned());
        rules.extend(self.guide.iter().cloned());
        rules
    }
}

/// The style called `name`, as `--style` takes it.
fn style(name: &str) -> Result<&'static Preset, String> {
    let known = || PRESETS.iter().map(|preset| preset.name).collect::<Vec<_>>().join(", ");
    PRESETS.iter().find(|preset| preset.name == name).ok_or_else(|| format!("`{name}` isn't a style: the styles are {}", known()))
}

/// A setting of automatic digitizing's rules and its value, as `--set` takes them: KEY=VALUE, the key one
/// of the rules' settings. The value is read when a shape is digitized, as a shape's own is.
fn rule(text: &str) -> Result<(String, String), String> {
    let (key, value) = text.split_once('=').ok_or_else(|| format!("`{text}` isn't KEY=VALUE"))?;
    let (key, value) = (key.trim(), value.trim());
    if AutoParams::SPECS.iter().all(|spec| spec.key != key) {
        return Err(format!("`{key}` isn't a setting of automatic digitizing's rules (see docs/src/user/reference/params/auto.md)"));
    }
    Ok((key.to_string(), value.to_string()))
}

impl From<&ReadingArgs> for Reading {
    fn from(args: &ReadingArgs) -> Reading {
        Reading { auto: args.auto, sort_colours: args.sort_colours }
    }
}

/// `stitch bug-report`.
#[derive(Debug, Args)]
pub struct BugReportArgs {
    /// The design StitchCraft gets wrong: an SVG file, or a VectorCraft document (.vectorcraft).
    #[arg(required_unless_present = "replay")]
    pub design: Option<PathBuf>,
    /// Replay this bundle instead: plan its design again, with the profile and format it names, and say
    /// whether everything comes out the same.
    #[arg(long, conflicts_with_all = ["design", "output", "profile", "format", "says", "auto", "sort_colours", "style", "set"])]
    pub replay: Option<PathBuf>,
    /// The bundle to write; by default next to the design, as DESIGN.bug-report.json.
    #[arg(long, short)]
    pub output: Option<PathBuf>,
    /// The machine profile (see `stitch profiles`); by default the one a VectorCraft document keeps, which
    /// StitchCraft's Design Settings filter sets, or else the reference machine, which `stitch profiles`
    /// lists first.
    #[arg(long, short)]
    pub profile: Option<String>,
    /// The file format; by default the profile's.
    #[arg(long, value_enum)]
    pub format: Option<Format>,
    /// What goes wrong, in your words; it goes into the bundle.
    #[arg(long)]
    pub says: Option<String>,
    /// How the design is read.
    #[command(flatten)]
    pub reading: ReadingArgs,
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

/// `stitch palette`.
#[derive(Debug, Args)]
pub struct PaletteArgs {
    /// The machine profile whose threads to write (see `stitch profiles`); by default the reference
    /// machine's.
    #[arg(long, short)]
    pub profile: Option<String>,
    /// The file to write; by default the palette goes to standard output.
    #[arg(long, short)]
    pub output: Option<PathBuf>,
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
    /// The machine the picture is for, which decides where the thread is cut (see `stitch profiles`); by
    /// default the reference machine.
    #[arg(long, short)]
    pub profile: Option<String>,
}

/// Preview styles on the command line.
#[derive(Clone, Copy, Debug, PartialEq, Eq, ValueEnum)]
pub enum PreviewStyle {
    /// The finished embroidery on fabric, with the jump threads the machine leaves for you to cut.
    Realistic,
    /// Every stitch, needle hole, move, trim and stop, for checking a design.
    Simple,
    /// How thick the design sews: grey thread, yellow from 2 fills of it over one spot, orange from 4 and
    /// red from 6.
    Thickness,
}

impl From<PreviewStyle> for Style {
    fn from(style: PreviewStyle) -> Style {
        match style {
            PreviewStyle::Realistic => Style::Realistic,
            PreviewStyle::Simple => Style::Simple,
            PreviewStyle::Thickness => Style::Thickness,
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

#[cfg(test)]
mod tests {
    use clap::ValueEnum;
    use stitchcraft_plan::thickness::{Limits, ORDINARY};

    use super::PreviewStyle;

    #[test]
    fn req_rnd_004_the_thickness_styles_help_names_the_steps_the_renderer_draws() {
        let value = PreviewStyle::Thickness.to_possible_value().unwrap();
        let help = value.get_help().unwrap().to_string().replace('\n', " ");
        let steps = format!(
            "yellow from {ORDINARY} fills of it over one spot, orange from {} and red from {}",
            Limits::DEFAULT.warn_fills,
            Limits::DEFAULT.fail_fills
        );
        assert!(help.contains(&steps), "{steps:?} is not in {help:?}");
    }
}
