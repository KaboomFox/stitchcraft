//! The pictures of automatic digitizing that `cargo xtask perf` counts (`REQ-AUTO-083`,
//! `xtask/src/pictures.rs`): each is a command `stitch` reads, kept with its record in
//! `conformance/perf.toml`, so that a renamed option or a moved fixture fails here, in the tests, and not
//! only in the CI job that counts them.

// Test code may unwrap (clippy.toml allows it inside #[test] functions), and it reads the record and checks
// the files it names.
#![allow(clippy::unwrap_used, clippy::disallowed_methods)]

use std::path::Path;

use clap::Parser;
use stitchcraft_cli::cli::{Cli, Command};
use stitchcraft_cli::commands::plan::Source;

#[test]
fn req_auto_083_each_picture_the_budget_counts_is_a_command_stitch_reads() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let record: toml::Table = std::fs::read_to_string(root.join("conformance/perf.toml")).unwrap().parse().unwrap();
    let pictures = record["pictures"].as_table().unwrap();
    // The 4 traced dogs, the pear and the 6 TS-13 sheets.
    assert_eq!(pictures.len(), 11);
    for (name, picture) in pictures {
        let args: Vec<&str> = picture["args"].as_array().unwrap().iter().map(|arg| arg.as_str().unwrap()).collect();
        let cli = Cli::try_parse_from(["stitch"].into_iter().chain(args.iter().copied()).chain(["-o", "out.pes"]))
            .unwrap_or_else(|e| panic!("{name}: {e}"));
        match (&cli.command, picture.get("golden").and_then(toml::Value::as_str)) {
            // A traced picture is exported digitized automatically, as a VectorCraft document is by default.
            (Command::Plan(plan), None) => {
                assert!(plan.reading.reading(Source::of(&plan.design)).auto, "{name} is digitized automatically");
                assert!(root.join(&plan.design).is_file(), "{name}: {}", plan.design.display());
            }
            // A sheet is written by `stitch testsheet`, and is its golden file.
            (Command::Testsheet(sheet), Some(golden)) => {
                assert_eq!(sheet.sheet.as_deref(), Some(name.as_str()));
                assert_eq!(golden, format!("conformance/golden/testsheets/{name}.pes"));
                assert!(root.join(golden).is_file(), "{name}: {golden}");
            }
            (command, golden) => panic!("{name}: {command:?} against {golden:?}"),
        }
        assert!(picture["instructions"].as_integer().unwrap() > 0, "{name}");
    }
}
