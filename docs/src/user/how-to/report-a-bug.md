# Report a bug

When StitchCraft gets a design wrong, a maintainer needs one file to see what you saw: a **bug-report
bundle**. Write one when:

- the stitches differ from the design
- a message is unclear
- StitchCraft crashes

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
- a digest of the plan's stitches, and the machine file's size and digest;
- the messages, or the crash's message;
- what you wrote with `--says`.

**It holds your design.** Share it only where you are happy for the design to be seen.

## Report it

Open an issue at <https://github.com/KaboomFox/stitchcraft/issues>. Say what you expected and what you
got, and attach the bundle. GitHub takes `.json` files up to 25 MB. Zip a larger one first.

## When StitchCraft finds the bug itself

StitchCraft checks each plan before writing a machine file. If a check fails (`SC-E0009`) or StitchCraft
crashes, `stitch plan` writes the bundle for you in place of the machine file. The bundle goes next to
the file you named, as `design.bug-report.json` for `design.pes`. The command says where it is and ends
with exit status 4. Attach that bundle to an issue.

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
