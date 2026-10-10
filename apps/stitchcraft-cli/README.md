# stitchcraft-cli (`stitch`)

The command-line tool: the only StitchCraft component that touches the file system, the terminal and
(optionally) threads.

| Command | Since | What it does |
|---|---|---|
| `stitch plan design.svg -o design.pes --preview design.png --report design.json` | M3.9 | Plans an SVG design for the machine `--profile` names, `brother-pe800-5x7` by default, and writes the machine file (`--format pes\|dst` overrides the extension). `--preview` adds a picture of what it will sew, and `--report` a JSON report. Prints what was said about the design, element by element |
| `stitch testsheet TS-01 -o TS-01.pes` | M1.8 | Writes a machine-checkpoint test sheet, checked against the profile of the hoop it is for unless `--profile` names another (`--list` lists them, and `--format pes\|dst` overrides the extension). Prints its SHA-256, size and counts, its threads in the order they are sewn, and what to check after sewing |
| `stitch inspect design.pes --profile brother-pe800-5x7` | M2.1 | Reads any PES, PEC or DST file and describes it: size, counts, extent, stitch lengths, threads; with `--profile`, the hoop check and stitches outside the machine's limits |
| `stitch preview design.pes -o design.png` | M2.7 | Draws any PES, PEC or DST file as it will sew (`--style realistic\|simple`, `--scale` in pixels per millimetre) |
| `stitch convert design.dst -o design.pes` | M2.7 | Rewrites a machine file in another format (`--format pes\|dst` overrides the extension); says what the new format cannot store |
| `stitch profiles` | M1.8 | The built-in machine profiles and the evidence behind their values |
| `stitch explain SC-W0702` | M1.8 | A diagnostic's explanation, from the registry |
| `stitch bug-report design.svg --says "…"` | M3.10 | Writes a bug-report bundle: the design, profile, format and version, and digests of what came of them (`--replay BUNDLE` plans it again and compares) |

Coming with their milestones: `export` (M6.5), `import-inkstitch` (M8), `conformance run`.

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
  half a file.

## Invariants

- Contains no engine logic: it reads files, calls the engine, formats and renderer, and writes files.
- Writes only plans that have been checked (invariants, hoop and comfort zone). The command line checks
  its test sheets itself, and the engine's `plan` checks designs. When a check fails, the machine file is
  not written, and `stitch plan` still writes its report to say why.
- Never panics on bad arguments (arguments that are not valid Unicode included); exit codes are documented.
- A panic anyway is a bug: `main` catches it (`commands::bug_report::guarded`), says so, and for
  `stitch plan` writes a bug-report bundle.
- A closed stdout (`stitch … | head`) ends the program quietly instead of panicking.

## Dependencies

`clap` (restricted to this crate and `xtask` by `cargo xtask layers`), `sha2`, `serde` and `serde_json`
(the plan report and bug-report bundles), and the StitchCraft libraries.
