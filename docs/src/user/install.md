# Install StitchCraft

StitchCraft has no packaged releases yet. Build the `stitch` command from its source on Linux, macOS or
Windows. It takes a few minutes.

## Build it

Install Rust with [rustup](https://rustup.rs/), then get the source and build it:

```sh
git clone https://github.com/KaboomFox/stitchcraft
cd stitchcraft
cargo install --locked --path apps/stitchcraft-cli
```

The repository names the Rust version it is built with, and rustup fetches that version the first time.
cargo puts `stitch` in its `bin` folder, which rustup adds to your `PATH`.

## Check it

```console
{{#include reference/generated/version.txt}}
```

To update StitchCraft, run `git pull` in the `stitchcraft` folder and the `cargo install` line again.

## Next

[Sew your first test sheet](tutorials/first-sew-out.md), or [turn an SVG into a PES
file](tutorials/svg-to-pes.md). The [command line](reference/cli.md) reference lists every command.
