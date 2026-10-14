//! `cargo xtask` as people and workflows run it: an argument it does not know is refused before anything
//! runs, so a misspelt `--check` cannot run the command's other mode and pass.

// Test code may expect (clippy.toml allows it inside #[test] functions; this helper is test code too). It
// runs the xtask binary.
#![allow(clippy::expect_used, clippy::disallowed_methods)]

use std::process::{Command, Output};

fn xtask(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_stitchcraft-xtask")).args(args).output().expect("the xtask binary runs")
}

#[test]
fn an_argument_a_command_does_not_take_is_refused_with_status_2() {
    // Read-only gates, so that the old parser, which ran them anyway, changed nothing.
    for args in [&["layers", "--checks"][..], &["filesize", "--record"], &["cleanroom", "--no-such-flag"], &["unsafe-audit", "now"]] {
        let out = xtask(args);
        let said = String::from_utf8_lossy(&out.stderr);
        assert_eq!(out.status.code(), Some(2), "{args:?}: {said}");
        assert!(said.contains("unexpected argument"), "{args:?}: {said}");
    }
}

#[test]
fn an_option_without_its_value_is_refused_with_status_2() {
    for args in [&["conformance", "--filter"][..], &["compat", "contract", "--ref", "v0.7.0"], &["prose", "--base"]] {
        let out = xtask(args);
        assert_eq!(out.status.code(), Some(2), "{args:?}: {}", String::from_utf8_lossy(&out.stderr));
    }
}
