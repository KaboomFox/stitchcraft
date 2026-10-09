# Report a bug

When StitchCraft gets a design wrong — stitches that are not what the design asks for, a message that
makes no sense, or a crash — one file is all a maintainer needs to see exactly what you saw: a
**bug-report bundle**.

## Write the bundle

Run `stitch bug-report` with the design, and the profile and format you sew with:

```sh
stitch bug-report design.svg --profile brother-200x200 --says "the zigzag's corners are rounded"
```

It plans the design exactly as `stitch plan` does and writes `design.bug-report.json` next to it (`-o`
names another file). The bundle holds:

- your design file, whole;
- the profile, the format (`--format pes|dst`, the profile's own by default) and the version of
  StitchCraft;
- what came of it: a digest of every stitch of the plan, the machine file's size and digest, and every
  message, or the crash's message;
- what you wrote with `--says`.

**It holds your design.** Share it only where you are happy for the design to be seen.

## Report it

Open an issue at <https://github.com/KaboomFox/stitchcraft/issues>, say what goes wrong — what you expected
and what you got — and attach the bundle. GitHub takes `.json` files up to 25 MB; zip a larger one.

## When StitchCraft finds the bug itself

StitchCraft checks every plan before writing a machine file. If a check fails (`SC-E0009`) or StitchCraft
crashes, `stitch plan` writes no machine file. It writes the bundle for you instead, next to the file you
asked for (`design.pes` → `design.bug-report.json`), says where, and ends with exit status 4. Attach that
bundle to an issue.

## For maintainers: replay a bundle

```sh
stitch bug-report --replay design.bug-report.json
```

This plans the bundled design again with the bundled profile and format, and compares everything the
bundle recorded:

```text
design.bug-report.json: written by StitchCraft 0.1.0 on windows x86_64, replayed by 0.1.0 on linux x86_64
  design        same: design.svg (sha256 …)
  plan          same: 118 stitches, 4 jumps, 0 trims, 2 colour changes, 0 stops (sha256 …)
  machine file  same: 1725 bytes (sha256 …)
  diagnostics   same: 2
reproduced
```

StitchCraft gives the same plan, bit for bit, for the same design, profile, format and version on every
platform, so with the version that wrote the bundle, the replay reproduces it. With another version, what
differs is listed under `then:` and `now:`: that is how to tell whether a bug is fixed. The exit status is
1 when the bundle does not reproduce. The design is in the bundle's `design.svg` field, ready to save and
open.
