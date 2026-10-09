//! `stitch convert FILE -o FILE`: a machine file written in another format (DST to PES, PES to DST).
//!
//! The file is read the way `stitch inspect` reads it and written with the same writers as
//! `stitch testsheet`, so the needle goes down at exactly the same 0.1 mm positions (both formats use
//! that unit, so nothing is rounded twice). What the new format cannot say is reported, never guessed:
//! a DST file stores no thread colours, so a PES file made from one names a placeholder colour for every
//! thread (`SC-W0604`). The other way round, DST records a stop as a pause for the next thread, which is
//! what a stop is, but its machines cut the thread before every jump longer than 24.2 mm, where a PES
//! machine leaves a jump thread: `SC-I0605` says how many.

use std::fmt::Write as _;

use stitchcraft_formats::Encoded;

use super::{Outcome, Status, describe_file, describe_plan, describe_threads, output_format, read_machine_file, reader_warnings};
use super::{render_diagnostics, unknown_colors, write_file};
use crate::cli::ConvertArgs;

/// Runs `stitch convert`.
pub fn run(args: &ConvertArgs) -> Outcome {
    let format = match output_format(args.format, &args.output) {
        Ok(format) => format,
        Err(outcome) => return outcome,
    };
    let (_, decoded) = match read_machine_file(&args.file) {
        Ok(read) => read,
        Err(outcome) => return outcome,
    };
    let warnings = reader_warnings(&decoded);
    let mut diagnostics = Vec::new();
    if format.palette().is_some() {
        diagnostics.extend(unknown_colors(&decoded));
    }
    // The design's name goes into the file's label; a file without one is named after the output.
    let stem = args.output.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
    let name = if decoded.name.is_empty() { stem } else { decoded.name.clone() };
    let Encoded { bytes, notes } = match stitchcraft_formats::encode(&decoded.plan, format, &name) {
        Ok(encoded) => encoded,
        Err(e) => {
            diagnostics.push(e.diagnostic());
            return Outcome::refuse(warnings, &diagnostics);
        }
    };
    diagnostics.extend(notes);
    if let Err(outcome) = write_file(&args.output, &bytes) {
        return outcome;
    }
    let mut out = String::new();
    let _ = writeln!(out, "{} · {} → {}", args.file.display(), decoded.format, format.name());
    describe_file(&mut out, &args.output, format, &bytes);
    describe_plan(&mut out, &decoded.plan);
    describe_threads(&mut out, &decoded.plan, format.palette().map(|id| id.palette()));
    Outcome { stdout: out, stderr: warnings + &render_diagnostics(&diagnostics), status: Status::Done }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use stitchcraft_testkit::equivalence::{dst_events, events};

    use super::*;
    use crate::cli::Format;

    fn golden(name: &str) -> PathBuf {
        PathBuf::from(format!("{}/../../conformance/golden/{name}", env!("CARGO_MANIFEST_DIR")))
    }

    fn temp(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("stitchcraft-convert-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        dir.join(name)
    }

    fn convert(file: PathBuf, output: PathBuf, format: Option<Format>) -> Outcome {
        run(&ConvertArgs { file, output, format })
    }

    fn read(path: &PathBuf) -> stitchcraft_formats::Decoded {
        stitchcraft_formats::decode(&std::fs::read(path).unwrap()).unwrap()
    }

    #[test]
    fn diag_sc_i0605_pes_to_dst_sews_as_dst_machines_do() {
        // TS-02's right half has no trims, and DST machines cut the thread before its long jumps.
        let (source, output) = (golden("testsheets/TS-02.pes"), temp("TS-02.dst"));
        let out = convert(source.clone(), output.clone(), None);
        assert_eq!(out.status, Status::Done, "{}", out.stderr);
        assert!(out.stdout.starts_with(&format!("{} · PES (#PES0001) → DST\n", source.display())), "{}", out.stdout);
        assert_eq!(
            out.stderr,
            "info SC-I0605: 3 jumps longer than 24.2 mm take 3 or more DST jump records each, which machines read as a trim, so the thread will be cut before them.\n"
        );
        let (before, after) = (read(&source), read(&output));
        assert_eq!(after.name, "TS-02");
        assert_eq!(events(&after.plan), dst_events(&before.plan));
        assert_eq!(after.plan.stats().trims, before.plan.stats().trims + 3);
    }

    #[test]
    fn diag_sc_w0604_converting_dst_says_its_colours_are_placeholders() {
        let (source, output) = (golden("testsheets/TS-01.dst"), temp("TS-01-from-dst.bin"));
        let out = convert(source.clone(), output.clone(), Some(Format::Pes));
        assert_eq!(out.status, Status::Done, "{}", out.stderr);
        assert!(out.stderr.starts_with("warning SC-W0604: DST stores no thread colours; every thread is a black placeholder."), "{}", out.stderr);
        assert_eq!(events(&read(&output).plan), events(&read(&source).plan));
    }

    #[test]
    fn unreadable_or_unnamed_inputs_are_reported() {
        assert_eq!(convert(golden("testsheets/TS-01.pes"), temp("TS-01.png"), None).status, Status::Usage);
        assert_eq!(convert(temp("missing.dst"), temp("x.pes"), None).status, Status::Io);
        let junk = temp("junk.pes");
        std::fs::write(&junk, b"not embroidery").unwrap();
        let out = convert(junk, temp("y.dst"), None);
        assert_eq!(out.status, Status::DesignErrors);
        assert!(out.stderr.starts_with("error SC-E0603: "), "{}", out.stderr);
    }
}
