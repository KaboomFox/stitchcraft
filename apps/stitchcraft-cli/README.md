# stitchcraft-cli (`stitch`)

The command-line tool: the only StitchCraft component that touches the file system, the terminal and
(optionally) threads.

| Command | Since | What it does |
|---|---|---|
| `stitch testsheet TS-01 --profile brother-200x200 -o TS-01.pes` | M1.8 | Writes a machine-checkpoint test sheet (`--list` lists them; `--format pes\|dst` overrides the extension) and prints its SHA-256, size, counts, the threads the machine will ask for, and what to check after sewing |
| `stitch profiles` | M1.8 | The built-in machine profiles and the evidence behind their values |
| `stitch explain SC-W0702` | M1.8 | A diagnostic's explanation, from the registry |

Coming with their milestones: `inspect` (M2.1), `preview`, `convert` (M2.7), `plan`, `bug-report` (M3),
`export` (M6.5), `import-inkstitch` (M8), `conformance run`.

**Exit status:** 0 done (warnings allowed) · 1 the design has errors, nothing written · 2 usage error ·
3 a file could not be read or written.

## Layout

- `cli.rs` — the grammar (`clap` derive); `--help` and the reference page come from it.
- `commands/` — one module per subcommand. Each returns an `Outcome` (stdout, stderr, status) instead of
  printing, so commands are tested without a terminal.
- `files.rs` — files are written under a temporary name and renamed into place, so a machine never sees
  half a file.

## Invariants

- Contains no engine logic: it reads files, calls the engine, formats and renderer, and writes files.
- Checks every plan (invariants, then hoop and comfort zone) before writing it; errors write nothing.
- Never panics on bad arguments (arguments that are not valid Unicode included); exit codes are documented.
- A closed stdout (`stitch … | head`) ends the program quietly instead of panicking.

## Dependencies

`clap` (restricted to this crate and `xtask` by `cargo xtask layers`), `sha2`, and the StitchCraft
libraries.
