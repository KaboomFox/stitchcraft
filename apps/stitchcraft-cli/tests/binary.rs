//! The `stitch` binary as people run it: what `main` adds to the commands — printing what they say,
//! ending with their exit status, and the panic guard around them.

// Test code may unwrap (clippy.toml allows it inside #[test] functions; this helper is test code too).
#![allow(clippy::unwrap_used)]

use std::process::{Command, Output};

fn stitch(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_stitch")).args(args).output().unwrap()
}

#[test]
fn prints_what_a_command_says_and_ends_with_its_status() {
    let explained = stitch(&["explain", "SC-E0012"]);
    assert_eq!(explained.status.code(), Some(0));
    assert!(String::from_utf8_lossy(&explained.stdout).starts_with("SC-E0012: Bug-report bundle could not be replayed"));
    assert_eq!(stitch(&["sew-it"]).status.code(), Some(2));
    let missing = stitch(&["inspect", "no-such-file.pes"]);
    assert_eq!(missing.status.code(), Some(3));
    assert!(String::from_utf8_lossy(&missing.stderr).starts_with("stitch: cannot read no-such-file.pes"));
}
