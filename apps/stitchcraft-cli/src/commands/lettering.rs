//! `stitch lettering TEXT --font FOLDER -o FILE`: a text set in an embroidery font, planned for a machine
//! and written as a machine file (roadmap step M14.2, `REQ-FONT-014`, `docs/src/design/fonts.md` ›
//! Lettering).
//!
//! **Why a command of its own.** A font is a folder, never a design file, and lettering makes the design:
//! the command reads the folder's files, lets `stitchcraft_svg::lettering` set the text, and then plans and
//! writes the design with the code `stitch plan` plans and writes one with ([`sew_design`]), so that a text
//! sews as any design does, with the same checks. Nothing here lays out a glyph.
//!
//! A font's files are those directly in its folder and directly in the folders in it, as a direction's
//! glyph files lie. What reading the font and setting the text said is printed with what planning says, and
//! when anything said is an error nothing is written, as with `stitch plan`.

use std::fmt::Write as _;
use std::io;
use std::path::Path;

use stitchcraft_core::{Budget, Severity};
use stitchcraft_engine::design::Path as Outline;
use stitchcraft_plan::profiles;
use stitchcraft_svg::font::{self, Font};
use stitchcraft_svg::lettering::{self, Along, Lettering};

use super::plan::{Sewn, design_name, picture, sew_design};
use super::{Entries, Outcome, Status, asked_profile, describe_file, describe_plan, describe_threads, distinct, output_format};
use super::{render_diagnostics, write_files};
use crate::cli::LetteringArgs;
use crate::files;

/// Runs `stitch lettering`.
pub fn run(args: &LetteringArgs) -> Outcome {
    let (format, profile) = match output_format(args.format, &args.output).and_then(|format| Ok((format, asked_profile(args.profile.as_deref())?))) {
        Ok((format, profile)) => (format, profile.unwrap_or(profiles::REFERENCE)),
        Err(outcome) => return outcome,
    };
    let writes: Vec<&Path> = std::iter::once(args.output.as_path()).chain(args.preview.as_deref()).collect();
    if let Err(outcome) = distinct(&[], &writes) {
        return outcome;
    }
    let files = match folder(&args.font) {
        Ok(files) => files,
        Err(error) => return Outcome::unread(&args.font, &error),
    };
    let given: Vec<(&str, &[u8])> = files.iter().map(|(path, bytes)| (path.as_str(), bytes.as_slice())).collect();
    let name = args.font.file_name().map(|name| name.to_string_lossy().into_owned()).unwrap_or_default();
    let font = match font::read(&name, &given, &Budget::DEFAULT) {
        Ok(font) => font,
        Err(unreadable) => return Outcome::refuse(String::new(), &[unreadable]),
    };
    let scale = match (args.height, font.settings.size) {
        (Some(_), size) if size <= 0.0 => {
            return Outcome::usage(format!("the font `{name}` gives no size in its font.json, so --height can't be told: use --scale"));
        }
        (Some(height), size) => height / size * 100.0,
        (None, _) => args.scale.unwrap_or(100.0),
    };
    let how = Lettering {
        scale,
        letter_spacing: args.letter_spacing,
        word_spacing: args.word_spacing,
        line_height: args.line_height,
        align: args.align,
        back_and_forth: args.back_and_forth,
        trim: args.trim,
        color_sort: args.color_sort,
    };
    let mut said = font.warnings.clone();
    let along = match args.path.as_deref().map(|file| outline(file, &mut said)).transpose() {
        Ok(along) => along,
        Err(outcome) => return outcome,
    };
    let text = args.text.replace("\\n", "\n");
    let set = match &along {
        Some((_, path)) => {
            let along = Along { path: path.clone(), position: args.text_position, vertical: args.vertical_text_position };
            lettering::set_along(&font, &text, &how, &along, &Budget::DEFAULT)
        }
        None => lettering::set(&font, &text, &how, &Budget::DEFAULT),
    };
    let set = match set {
        Ok(set) => set,
        Err(unset) => {
            said.push(unset);
            return Outcome::refuse(String::new(), &said);
        }
    };
    said.extend(set.warnings.iter().cloned());
    let sewn = sew_design(&set.design, said, profile, format, design_name(&args.output));
    let Sewn { mut diagnostics, plan: Some(plan), file: Some(machine_file), back: Some(back) } = sewn else {
        return Outcome::refuse(String::new(), &sewn.diagnostics);
    };
    let image = match args.preview.as_ref().map(|path| (path, picture(&back, profile.trims))) {
        Some((path, Ok(image))) => Some((path, image)),
        Some((_, Err(undrawn))) => {
            diagnostics.push(undrawn);
            None
        }
        None => None,
    };
    if diagnostics.iter().any(|d| d.severity() == Severity::Error) {
        return Outcome::refuse(String::new(), &diagnostics);
    }
    let mut stdout = String::new();
    let _ = writeln!(stdout, "{} · {}", args.text, described(&font, &name));
    describe_file(&mut stdout, &args.output, format, &machine_file);
    let _ = writeln!(stdout, "  profile   {} ({})", profile.id, profile.name);
    let _ = writeln!(stdout, "  scale     {}", scale_line(&font, set.scale));
    if let (Some(file), Some((named, _))) = (&args.path, &along) {
        let _ = writeln!(stdout, "  path      {named} of {}", file.display());
    }
    describe_plan(&mut stdout, &back, Some(profile));
    describe_threads(&mut stdout, &plan, format.palette().map(|id| Entries::Written(id.palette())));
    if let Some((path, image)) = &image {
        let _ = writeln!(stdout, "  preview   {} ({} × {} pixels)", path.display(), image.width, image.height);
    }
    let mut written: Vec<(&Path, &[u8])> = vec![(&args.output, &machine_file)];
    written.extend(image.as_ref().map(|(path, image)| (path.as_path(), image.png.as_slice())));
    let said = render_diagnostics(&diagnostics);
    if let Err(unwritten) = write_files(&written, &said) {
        return unwritten;
    }
    Outcome { stdout, stderr: said, status: Status::Done }
}

/// The label of the path Ink/Stitch's batch lettering sets its text along.
const BATCH_LETTERING: &str = "batch lettering";

/// The path of the SVG file `file` that a text is set along, with what the summary calls it: the path
/// labelled `batch lettering`, or else the first that paints. What reading the file said goes to `said`.
fn outline(file: &Path, said: &mut Vec<stitchcraft_core::Diagnostic>) -> Result<(String, Outline), Outcome> {
    let bytes = files::read_capped(file).map_err(|error| Outcome::unread(file, &error))?;
    let svg = stitchcraft_svg::read(&bytes, &Budget::DEFAULT).map_err(|unread| Outcome::refuse(String::new(), &[unread]))?;
    said.extend(svg.warnings);
    let elements = svg.design.elements();
    let labelled = elements.iter().find(|element| element.name.as_deref() == Some(BATCH_LETTERING));
    let Some(element) = labelled.or_else(|| elements.first()) else {
        return Err(Outcome::usage(format!("{} has no path that paints to set the text along: give one a stroke", file.display())));
    };
    let named = if labelled.is_some() { format!("the path labelled `{BATCH_LETTERING}`") } else { format!("the first path, `{}`", element.id) };
    Ok((named, element.shape.path().clone()))
}

/// The font as the summary names it: its name, and its folder when that differs.
fn described(font: &Font, folder: &str) -> String {
    let name = &font.settings.name;
    if name.is_empty() || name == folder { folder.to_string() } else { format!("{name} ({folder})") }
}

/// The scale the text is set at, with the height that makes of the font's size when it gives one.
fn scale_line(font: &Font, scale: f64) -> String {
    let size = font.settings.size;
    if size > 0.0 { format!("{scale:.0} % ({:.1} mm tall)", size * scale / 100.0) } else { format!("{scale:.0} %") }
}

/// The files of the font in the folder `path`, each by its path in the folder with `/` between its parts:
/// those directly in it and directly in the folders in it, in the order of their paths.
pub(crate) fn folder(path: &Path) -> io::Result<Vec<(String, Vec<u8>)>> {
    let mut files = Vec::new();
    for entry in std::fs::read_dir(path)? {
        let entry = entry?;
        let name = entry.file_name().to_string_lossy().into_owned();
        if entry.file_type()?.is_dir() {
            for inner in std::fs::read_dir(entry.path())? {
                let inner = inner?;
                if inner.file_type()?.is_file() {
                    files.push((format!("{name}/{}", inner.file_name().to_string_lossy()), files::read_capped(&inner.path())?));
                }
            }
        } else {
            files.push((name, files::read_capped(&entry.path())?));
        }
    }
    files.sort();
    Ok(files)
}

#[cfg(test)]
mod tests;
