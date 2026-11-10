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
files from VectorCraft's File › Export, from VectorCraft 0.14. Filters change the drawing: one sets a
tartan's sett, from VectorCraft 0.15, one keeps a design's settings and machine with the document and one
draws a test sheet, from VectorCraft 0.16, and others make lines into satin columns and route stitches,
from VectorCraft 0.17. All are built from the same source. In the
`stitchcraft` folder:

```sh
rustup target add wasm32-unknown-unknown
cargo build -p stitchcraft-vc-plugin --release --target wasm32-unknown-unknown --features fill
```

The build writes `target/wasm32-unknown-unknown/release/stitchcraft_vc_plugin.wasm`. Copy it into a
folder for plug-ins as `stitchcraft-fill.wasm`. Build again with `--features stroke`, `--features satin`,
`--features auto`, `--features pes`, `--features dst`, `--features settings`, `--features testsheet`,
`--features stroke-to-satin`, `--features route-satins`, `--features route-runs` and `--features tartan`,
and copy the file each time, as `stitchcraft-stroke.wasm`, `stitchcraft-satin.wasm`, `stitchcraft-auto.wasm`,
`stitchcraft-pes.wasm`, `stitchcraft-dst.wasm`, `stitchcraft-settings.wasm`, `stitchcraft-testsheet.wasm`,
`stitchcraft-stroke-to-satin.wasm`, `stitchcraft-route-satins.wasm`, `stitchcraft-route-runs.wasm` and
`stitchcraft-tartan.wasm`. VectorCraft 0.13 and earlier refuse the exporters, and from StitchCraft's M10.7
the fill effect too, whose dialog holds a tartan's sett as text.

In VectorCraft's desktop app, choose that folder in **Edit › Preferences › Plug-ins & Scratch Disks ›
Additional Plug-ins Folder**, which is under **Performance & Storage** up to VectorCraft 0.14. VectorCraft
installs the plug-ins in it whenever it starts, all but Stroke to Satin and the 2 Auto-Route filters. They
ask for more work than a filter usually may do, which VectorCraft lets you allow when you install each on
its own. Choose **Object › Plug-ins › Install Plug-in…** and pick `stitchcraft-stroke-to-satin.wasm`, and
install `stitchcraft-route-satins.wasm` and `stitchcraft-route-runs.wasm` the same way. VectorCraft asks you first, and once you allow one,
keeps it and installs it again when it starts. The effects are under **Effect › Plug-ins**, the formats in **File ›
Export › Export As**, and the filters under **Object › Plug-ins**. In VectorCraft on the web, open each
file with **File › Open** instead, once each time VectorCraft is opened. [Embroider a VectorCraft
design](how-to/embroider-a-vectorcraft-design.md) shows how to use them.

## VectorCraft's command line

`stitch plan` traces a picture, a PNG or JPEG file, with VectorCraft's command line, `vectorcraft-cli`
([from a picture to a machine file](tutorials/picture-to-machine-file.md#from-the-command-line)). VectorCraft
publishes it with each [release](https://github.com/storytold/vectorcraft/releases): beside the app in the
Windows and Linux downloads, and as a zip of its own for macOS. Put it in a folder on your `PATH`, or name it
with `stitch plan --vectorcraft-cli`. Check it with `vectorcraft-cli --version`. Only `stitch plan` uses it,
to trace a picture.

## The subject cut-out's models

`stitch plan --subject` and `stitch cutout` run 3 models, 49 MB together, which aren't part of the program
([cut a photo's subject out](how-to/cut-out-a-photos-subject.md)). `stitch` fetches them on first use with
`curl`, which Windows, macOS and most Linux systems have, from StitchCraft's release
[`cutout-models-1`](https://github.com/KaboomFox/stitchcraft/releases/tag/cutout-models-1). GitHub builds
that release from the models' authors' own files, and its notice names the authors and their licences.
`stitch` checks each file's SHA-256 before each use.

The models are kept in the folder the environment variable `STITCHCRAFT_MODELS` names. Without it, they go
to `stitchcraft/models` in your cache folder: `$XDG_CACHE_HOME` or `~/.cache`, or `%LOCALAPPDATA%` on
Windows. On a computer without a network, copy the 3 files there from another one. When a model isn't
ready, `SC-E1201` names each file's address, size and SHA-256, and the folder.

## Next

[Sew your first test sheet](tutorials/first-sew-out.md), or [turn an SVG into a PES
file](tutorials/svg-to-pes.md). The [command line](reference/cli.md) reference lists every command.
