//! The `stitch` command line as a library: its grammar ([`cli`]), its subcommands ([`commands`]) and
//! safe file writing ([`files`]).
//!
//! The binary (`src/main.rs`) only parses arguments and prints; everything else lives here so the
//! subcommands are tested without a terminal, and so `cargo xtask docs` can generate the command-line
//! reference from the same definition users run.
#![forbid(unsafe_code)]

pub mod cli;
pub mod commands;
pub mod files;
