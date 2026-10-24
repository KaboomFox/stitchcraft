# stitchcraft-cli (`stitch`)

The command-line tool: the only StitchCraft component that touches the file system, the terminal and
(optionally) threads.

| Command | Since | What it does |
|---|---|---|
| `stitch plan design.svg -o design.pes --preview design.png --report design.json` | M3.9 | Plans a design for the machine `--profile` names, `brother-pe800-5x7` by default, and writes the machine file (`--format pes\|dst` overrides the extension). `--preview` adds a picture of what it will sew, and `--report` a JSON report. Prints what was said about the design, element by element. The design is an SVG file, or from M6.5 a VectorCraft document (`.vectorcraft`, `.drawcraft` from before VectorCraft's rename, or a `.vctemplate` template), and `stitch export` is another name for the command. From M13.1, `--auto` digitizes automatically the shapes that have no settings |
| `stitch testsheet TS-01 -o TS-01.pes` | M1.8 | Writes a machine-checkpoint test sheet, checked against the profile of the hoop it is for unless `--profile` names another (`--list` lists them, and `--format pes\|dst` overrides the extension). Prints its SHA-256, size and counts, its threads in the order they are sewn, and what to check after sewing |
| `stitch inspect design.pes --profile brother-pe800-5x7` | M2.1 | Reads any PES, PEC or DST file and describes it: size, counts, extent, stitch lengths, threads; with `--profile`, the hoop check and stitches outside the machine's limits |
| `stitch preview design.pes -o design.png` | M2.7 | Draws any PES, PEC or DST file as it will sew (`--style realistic\|simple`, `--scale` in pixels per millimetre) |
| `stitch convert design.dst -o design.pes` | M2.7 | Rewrites a machine file in another format (`--format pes\|dst` overrides the extension); says what the new format cannot store |
| `stitch profiles` | M1.8 | The built-in machine profiles and the evidence behind their values |
| `stitch explain SC-W0702` | M1.8 | A diagnostic's explanation, from the registry |
| `stitch bug-report design.svg --says "…"` | M3.10 | Writes a bug-report bundle, which keeps the design, an SVG file or the part of a VectorCraft document its reader reads, with the profile, format and version it was planned with, whether it was digitized automatically (`--auto`), and digests of what came of them (`--replay BUNDLE` plans it again and compares) |

Coming with their milestones: `import-inkstitch` (M8), `conformance run`.

| Exit status | Meaning |
|---|---|
| 0 | Done, perhaps with warnings |
| 1 | The design or file has errors and nothing was written, or a replayed bundle does not reproduce |
| 2 | Usage error |
| 3 | A file could not be read or written |
| 4 | A bug in StitchCraft, a failed plan check or a panic. A bug-report bundle is written where one can be |

## Layout

- `cli.rs` — the grammar (`clap` derive); `--help` and the reference page come from it.
- `commands/` — one module per subcommand. Each returns an `Outcome` (stdout, stderr, status) instead of
  printing, so commands are tested without a terminal. `commands/mod.rs` holds what several share:
  reading a machine file, writing a file, the report lines and the diagnostics' rendering.
- `files.rs` — files are written under a temporary name and renamed into place, so a machine never sees
  half a file, and reads stop past a cap.

## Invariants

- Contains no engine logic: it reads files, calls the engine, formats and renderer, and writes files.
- `stitch plan` and `stitch testsheet` write only plans that have been checked (invariants, hoop and
  comfort zone), and none while anything said about the design is an error, an element left out with one
  included (`REQ-CLI-003`). The command line checks its test sheets itself, and the engine's `plan` checks
  designs. `stitch convert` writes the file it read in another format, unchecked, as `stitch inspect`
  reads it. When a check fails or an error is said, the machine file is not written, and `stitch plan`
  still writes its report to say why.
- Never panics on bad arguments (arguments that are not valid Unicode included); exit codes are documented.
- Writes the files a command was asked for all or none, each under a temporary name of its own, created
  new and synced before it is renamed into place, and prints what was said about the design even when one
  cannot be written, with the file and why (`REQ-CLI-006`). Refuses outputs that name the same file as
  each other or as a file it reads (`REQ-CLI-007`).
- Says what is in a file it wrote by reading the file back: the summary's size and counts, the report's
  and the picture (`REQ-CLI-008`).
- Reads no more than 64 MiB from any path, a pipe or a device included, and 192 MiB from a bug-report
  bundle it replays, which can hold a design that large in hexadecimal (`REQ-CLI-005`).
- A panic anyway is a bug: `stitch plan` catches it once it has read the design, and `main` before then
  and in the other commands (`commands::bug_report::guarded`). Each says so, and `stitch plan` writes a
  bug-report bundle that says where the panic came, which a replay reruns.
- A bug-report bundle is untrusted input: a replay prints what it takes from one with each character a
  terminal would act on written as an escape (`REQ-CLI-004`).
- A closed stdout (`stitch … | head`) ends the writing quietly and keeps the command's exit status, and any
  other error writing stdout or stderr is said and ends with 3 (`REQ-CLI-009`). Help and errors are
  coloured only on a terminal, and not with `NO_COLOR` set or a `dumb` terminal (`REQ-CLI-010`).

## Dependencies

`clap` (restricted to this crate and `xtask` by `cargo xtask layers`), `sha2`, `serde` and `serde_json`
(the plan report and bug-report bundles), and the StitchCraft libraries.
