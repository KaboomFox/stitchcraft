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

```sh
stitch --version
```

It prints `stitch` and its version number.

To update StitchCraft, run `git pull` in the `stitchcraft` folder and the `cargo install` line again.

## The VectorCraft plug-ins

The plug-ins say how each object of a [VectorCraft](https://github.com/storytold/vectorcraft) drawing
sews, and show its stitches. There are 3, one for each kind of stitch, built from the same source. In the
`stitchcraft` folder:

```sh
rustup target add wasm32-unknown-unknown
cargo build -p stitchcraft-vc-plugin --release --target wasm32-unknown-unknown --features fill
```

The build writes `target/wasm32-unknown-unknown/release/stitchcraft_vc_plugin.wasm`. Copy it into a
folder for plug-ins as `stitchcraft-fill.wasm`. Build again with `--features stroke` and with
`--features satin`, and copy the file each time, as `stitchcraft-stroke.wasm` and
`stitchcraft-satin.wasm`.

In VectorCraft's desktop app, choose that folder in **Edit › Preferences › Performance & Storage ›
Additional Plug-ins Folder**. VectorCraft installs the plug-ins in it whenever it starts, and their
effects are under **Effect › Plug-ins**. In VectorCraft on the web, open each file with **File › Open**
instead, once each time VectorCraft is opened. [Embroider a VectorCraft
design](how-to/embroider-a-vectorcraft-design.md) shows how to use them.

## Next

[Sew your first test sheet](tutorials/first-sew-out.md), or [turn an SVG into a PES
file](tutorials/svg-to-pes.md). The [command line](reference/cli.md) reference lists every command.
