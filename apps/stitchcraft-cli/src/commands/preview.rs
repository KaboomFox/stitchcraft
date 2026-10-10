//! `stitch preview FILE -o FILE.png`: a picture of what a machine file will sew.
//!
//! Reads any PES, PEC or DST file the way `stitch inspect` does and draws it with `stitchcraft-render`
//! (`docs/src/design/rendering.md`). The realistic style shows the finished embroidery, including jump
//! threads the machine leaves for you to cut; the simple style shows every stitch, needle hole, trim and
//! stop, for checking a design. Either way the picture is drawn from the file's own 0.1 mm positions, so
//! it shows what the machine will sew.

use std::fmt::Write as _;

use stitchcraft_core::Budget;
use stitchcraft_render::Settings;

use super::{Outcome, Status, read_machine_file, reader_warnings, render_diagnostics, unknown_colors, write_file};
use crate::cli::PreviewArgs;

/// Runs `stitch preview`.
pub fn run(args: &PreviewArgs) -> Outcome {
    let Some(settings) = Settings::new(args.style.into(), args.scale) else {
        return Outcome::usage(format!("the scale must be from {} to {} pixels per millimetre", Settings::MIN_SCALE, Settings::MAX_SCALE));
    };
    let (_, decoded) = match read_machine_file(&args.file) {
        Ok(read) => read,
        Err(outcome) => return outcome,
    };
    let warnings = reader_warnings(&decoded);
    let diagnostics: Vec<_> = unknown_colors(&decoded).into_iter().collect();
    let image = match stitchcraft_render::preview(&decoded.plan, settings, &mut Budget::DEFAULT.meter()) {
        Ok(image) => image,
        Err(e) => return Outcome::refuse(warnings + &render_diagnostics(&diagnostics), &[e.diagnostic()]),
    };
    if let Err(outcome) = write_file(&args.output, &image.png) {
        return outcome;
    }
    let mut out = String::new();
    let _ = writeln!(out, "{} · {} × {} pixels", args.output.display(), image.width, image.height);
    let name = if decoded.name.is_empty() { String::new() } else { format!(" · \"{}\"", decoded.name) };
    let _ = writeln!(out, "  from      {} · {}{name}", args.file.display(), decoded.format);
    let _ = writeln!(out, "  style     {}, {} pixels per millimetre", settings.style().name(), settings.scale());
    Outcome { stdout: out, stderr: warnings + &render_diagnostics(&diagnostics), status: Status::Done }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;
    use crate::cli::PreviewStyle;

    fn golden(name: &str) -> PathBuf {
        PathBuf::from(format!("{}/../../conformance/golden/{name}", env!("CARGO_MANIFEST_DIR")))
    }

    fn temp(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("stitchcraft-preview-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        dir.join(name)
    }

    fn preview(file: PathBuf, output: PathBuf, style: PreviewStyle, scale: f32) -> Outcome {
        run(&PreviewArgs { file, output, style, scale })
    }

    #[test]
    fn draws_a_pes_file() {
        let output = temp("TS-02.png");
        let out = preview(golden("testsheets/TS-02.pes"), output.clone(), PreviewStyle::Simple, 4.0);
        assert_eq!(out.status, Status::Done, "{}", out.stderr);
        // 120 × 70 mm plus 2 mm margins, at 4 pixels per millimetre.
        assert!(out.stdout.starts_with(&format!("{} · 496 × 296 pixels\n", output.display())), "{}", out.stdout);
        assert!(out.stdout.contains("  style     simple, 4 pixels per millimetre\n"));
        assert!(out.stderr.is_empty(), "{}", out.stderr);
        assert!(std::fs::read(&output).unwrap().starts_with(b"\x89PNG"));
    }

    #[test]
    fn dst_files_get_placeholder_colours_and_say_so() {
        let out = preview(golden("testsheets/TS-01.dst"), temp("TS-01.png"), PreviewStyle::Realistic, 2.0);
        assert_eq!(out.status, Status::Done);
        assert!(out.stderr.starts_with("warning SC-W0604: DST stores no thread colours"), "{}", out.stderr);
    }

    #[test]
    fn bad_scales_files_and_sizes_are_reported() {
        let out = preview(golden("testsheets/TS-01.pes"), temp("x.png"), PreviewStyle::Simple, 0.0);
        assert_eq!(out.status, Status::Usage);
        assert!(out.stderr.contains("from 0.1 to 50 pixels per millimetre"), "{}", out.stderr);
        assert_eq!(preview(temp("missing.pes"), temp("x.png"), PreviewStyle::Simple, 8.0).status, Status::Io);
        // 134 × 184 mm at 50 pixels per millimetre is 9200 pixels tall: too large, and nothing written.
        let output = temp("TS-10B.png");
        let out = preview(golden("testsheets/TS-10B.pes"), output.clone(), PreviewStyle::Simple, 50.0);
        assert_eq!(out.status, Status::DesignErrors);
        assert!(out.stderr.starts_with("error SC-E0005: The preview would be 6700 × 9200 pixels"), "{}", out.stderr);
        assert!(out.stderr.contains("hint: Use a scale of at most 22.2 pixels per millimetre."), "{}", out.stderr);
        assert!(!output.exists());
    }
}
