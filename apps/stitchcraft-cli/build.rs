//! Names the commit `stitch` is built from, when it is built from a git checkout, as `STITCHCRAFT_COMMIT`
//! (empty otherwise): `stitch --version` shows it, and a bug-report bundle records it, since every build of
//! the workspace says the version it will be released as (`REQ-CLI-001`).

// A build script speaks to Cargo on standard output, and runs git.
#![allow(clippy::print_stdout, clippy::disallowed_methods)]

use std::process::Command;

/// What `git` prints for `args`, trimmed, when it runs and succeeds.
fn git(args: &[&str]) -> Option<String> {
    let output = Command::new("git").args(args).output().ok().filter(|output| output.status.success())?;
    String::from_utf8(output.stdout).ok().map(|text| text.trim().to_string())
}

fn main() {
    // A checkout is one whose git tracks this file. A repository that only holds a copy of the sources, as
    // VectorCraft's checkout does in the rehearsal of the move (ADR-0011), or a home folder kept in git that
    // they were unpacked in, did not build them: its commit holds none of them.
    let tracked = git(&["ls-files", "--error-unmatch", "build.rs"]).is_some();
    let commit = if tracked { git(&["rev-parse", "--short=12", "HEAD"]) } else { None };
    println!("cargo:rustc-env=STITCHCRAFT_COMMIT={}", commit.unwrap_or_default());
    // Built again when the checkout moves to another commit: HEAD moves, or the branch it names does.
    for path in [git(&["rev-parse", "--git-path", "HEAD"]), git(&["symbolic-ref", "-q", "HEAD"]).and_then(|r| git(&["rev-parse", "--git-path", &r]))]
        .into_iter()
        .flatten()
    {
        println!("cargo:rerun-if-changed={path}");
    }
    println!("cargo:rerun-if-changed=build.rs");
}
