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
sews, and show its stitches. There are 3, one for each kind of stitch, and 2 more that write PES and DST
files from VectorCraft's File › Export, from VectorCraft 0.14. One more keeps a design's settings and
machine with the document, from VectorCraft 0.16. All are built from the same source. In the `stitchcraft`
folder:

```sh
rustup target add wasm32-unknown-unknown
cargo build -p stitchcraft-vc-plugin --release --target wasm32-unknown-unknown --features fill
```

The build writes `target/wasm32-unknown-unknown/release/stitchcraft_vc_plugin.wasm`. Copy it into a
folder for plug-ins as `stitchcraft-fill.wasm`. Build again with `--features stroke`, `--features satin`,
`--features pes`, `--features dst` and `--features settings`, and copy the file each time, as
`stitchcraft-stroke.wasm`, `stitchcraft-satin.wasm`, `stitchcraft-pes.wasm`, `stitchcraft-dst.wasm` and
`stitchcraft-settings.wasm`. VectorCraft 0.13 and earlier refuse the exporters, and the settings need
VectorCraft 0.16.

In VectorCraft's desktop app, choose that folder in **Edit › Preferences › Plug-ins & Scratch Disks ›
Additional Plug-ins Folder**, which is under **Performance & Storage** up to VectorCraft 0.14. VectorCraft
installs the plug-ins in it whenever it starts. Their effects are under **Effect › Plug-ins**, their
formats in **File › Export › Export As**, and the design settings under **Object › Plug-ins**. In VectorCraft on the web, open each file with **File › Open**
instead, once each time VectorCraft is opened. [Embroider a VectorCraft
design](how-to/embroider-a-vectorcraft-design.md) shows how to use them.

## Next

[Sew your first test sheet](tutorials/first-sew-out.md), or [turn an SVG into a PES
file](tutorials/svg-to-pes.md). The [command line](reference/cli.md) reference lists every command.
