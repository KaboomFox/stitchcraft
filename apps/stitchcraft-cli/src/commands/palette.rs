//! `stitch palette`: a machine's threads as a GIMP palette (`REQ-CLI-011`). VectorCraft loads the file as a
//! swatch library (Window › Swatch Libraries › Other Library…), and Image Trace's Document Library palette
//! then traces a picture in the threads' own colours, which the machine file names as those threads
//! (`docs/src/design/image-to-embroidery.md` › What VectorCraft does and what StitchCraft does). The
//! machine is the reference machine unless `--profile` names another, and the palette goes to standard
//! output unless `-o` names a file.

use stitchcraft_plan::MachineProfile;
use stitchcraft_plan::palette::PaletteEntry;
use stitchcraft_plan::profiles::REFERENCE;

use super::{Outcome, asked_profile, write_files};
use crate::cli::PaletteArgs;

/// Runs `stitch palette`.
pub fn run(args: &PaletteArgs) -> Outcome {
    let profile = match asked_profile(args.profile.as_deref()) {
        Ok(asked) => asked.unwrap_or(REFERENCE),
        Err(outcome) => return outcome,
    };
    let text = gpl(profile);
    let Some(output) = &args.output else { return Outcome::done(text) };
    if let Err(unwritten) = write_files(&[(output, text.as_bytes())], "") {
        return unwritten;
    }
    let palette = profile.palette.palette();
    let threads = palette.entries.iter().filter(|entry| entry.matchable).count();
    Outcome::done(format!("{threads} threads of the {} ({}) written to {}\n", profile.name, palette.name, output.display()))
}

/// `profile`'s threads as a GIMP palette, which says whose they are.
pub fn gpl(profile: &MachineProfile) -> String {
    profile.palette.palette().gpl(&format!("The threads of the {}, profile {}, from StitchCraft's stitch palette.", profile.name, profile.id))
}

/// [`gpl`] with only the threads `kept` (`REQ-AUTO-160`): the swatch library a picture is traced in when a
/// person keeps a colour. Each line of a thread is the palette's own, as [`gpl`] writes it, and the threads
/// keep the palette's order.
pub fn gpl_of(profile: &MachineProfile, kept: &[&PaletteEntry]) -> String {
    let text = gpl(profile);
    let mut lines = text.lines();
    let mut out: String = lines.by_ref().take(GPL_HEADER).flat_map(|line| [line, "\n"]).collect();
    for (line, entry) in lines.zip(crate::colours::threads(profile.palette.palette())) {
        if kept.iter().any(|kept| kept.index == entry.index) {
            out.push_str(line);
            out.push('\n');
        }
    }
    out
}

/// The lines a GIMP palette starts with, before its colours: its header, name, columns and the comment.
const GPL_HEADER: usize = 4;

#[cfg(test)]
mod tests {
    use stitchcraft_plan::profiles::BROTHER_PE800_4X4;
    use stitchcraft_testkit::temp::TempDir;

    use super::*;
    use crate::commands::Status;

    #[test]
    fn req_auto_160_a_library_of_the_threads_kept_is_the_palette_s_own_lines_in_its_order() {
        let palette = REFERENCE.palette.palette();
        let named = |name: &str| palette.entries.iter().find(|entry| entry.name == name).unwrap();
        let text = gpl_of(REFERENCE, &[named("White"), named("Prussian Blue")]);
        let all = gpl(REFERENCE);
        let header: Vec<&str> = all.lines().take(GPL_HEADER).collect();
        assert_eq!(text.lines().take(GPL_HEADER).collect::<Vec<_>>(), header);
        assert_eq!(text.lines().skip(GPL_HEADER).collect::<Vec<_>>(), [" 14  31 124\tPrussian Blue", "240 240 240\tWhite"]);
        assert!(text.ends_with("White\n"));
        assert_eq!(gpl_of(REFERENCE, &[]).lines().count(), GPL_HEADER);
    }

    fn args(profile: Option<&str>, output: Option<std::path::PathBuf>) -> PaletteArgs {
        PaletteArgs { profile: profile.map(str::to_string), output }
    }

    #[test]
    fn req_cli_011_the_reference_machines_threads_go_to_standard_output_as_a_gimp_palette() {
        let out = run(&args(None, None));
        assert_eq!(out.status, Status::Done);
        assert!(
            out.stdout.starts_with(
                "GIMP Palette\nName: Brother PEC\nColumns: 8\n# The threads of the Brother PE800 with its 5 × 7 in hoop, profile \
                 brother-pe800-5x7, from StitchCraft's stitch palette.\n 14  31 124\tPrussian Blue\n"
            ),
            "{}",
            out.stdout
        );
        assert_eq!(out.stdout, gpl(REFERENCE));
    }

    #[test]
    fn req_cli_011_a_profile_named_writes_its_threads_to_the_file_named() {
        let dir = TempDir::new("palette");
        let path = dir.path().join("threads.gpl");
        let out = run(&args(Some("brother-pe800-4x4"), Some(path.clone())));
        assert_eq!(out.status, Status::Done, "{}", out.stderr);
        assert_eq!(std::fs::read_to_string(&path).unwrap(), gpl(&BROTHER_PE800_4X4));
        assert_eq!(out.stdout, format!("61 threads of the Brother PE800 with its 4 × 4 in hoop (Brother PEC) written to {}\n", path.display()));
        // A profile StitchCraft hasn't is a usage error, and nothing is written.
        let none = dir.path().join("none.gpl");
        let out = run(&args(Some("nope"), Some(none.clone())));
        assert_eq!(out.status, Status::Usage);
        assert!(!none.exists());
    }
}
