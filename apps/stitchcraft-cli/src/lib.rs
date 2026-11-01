//! The `stitch` command line as a library: its grammar ([`cli`]), its subcommands ([`commands`]), safe file
//! writing ([`files`]), and pictures traced by VectorCraft's command line ([`trace`]).
//!
//! The binary (`src/main.rs`) only parses arguments and prints; everything else lives here so the
//! subcommands are tested without a terminal, and so `cargo xtask docs` can generate the command-line
//! reference from the same definition users run.
#![forbid(unsafe_code)]
// An app, not a library crate: the command line reads and writes files and its environment, which
// `clippy.toml` keeps out of library crates. Its geometry is the engine's.
#![allow(clippy::disallowed_methods)]

pub mod cli;
pub mod commands;
pub mod files;
pub mod trace;
