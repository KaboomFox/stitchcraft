# stitchcraft-cli (`stitch`)

The command-line tool: the only StitchCraft component that touches the file system, the terminal and
(optionally) threads.

**Status:** M0 answers `--help` and `--version`. Subcommands arrive with their milestones:
`testsheet`, `inspect` (M1.8), `preview`, `convert` (M2.7), `plan`, `explain`, `bug-report` (M3),
`export` (M6.5), `import-inkstitch` (M8), `conformance run`. With M1.8 the command line moves to `clap`, and
its reference page is generated from the definition.

## Invariants

- Contains no engine logic: it reads files, calls the engine, formats and renderer, and writes files.
- Never panics on bad arguments (arguments that are not valid Unicode included); exit codes are documented.
- A closed stdout (`stitch … | head`) ends the program quietly instead of panicking.
