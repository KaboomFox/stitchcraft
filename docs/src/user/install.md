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
sews, and show its stitches. There are 13 modules, each built from the `stitchcraft-vc-plugin` package
with a feature of its own:

| Module | VectorCraft shows it as | Where |
|---|---|---|
| `stitchcraft-fill.wasm`, `stitchcraft-stroke.wasm`, `stitchcraft-satin.wasm` | StitchCraft Fill, Stroke and Satin, one effect for each kind of stitch | Effect › Plug-ins |
| `stitchcraft-auto.wasm` | StitchCraft Auto, which chooses each shape's stitches by its width | Effect › Plug-ins |
| `stitchcraft-lettering.wasm` | StitchCraft Lettering, which sews a text in an embroidery font | Effect › Plug-ins |
| `stitchcraft-pes.wasm`, `stitchcraft-dst.wasm` | PES embroidery and DST embroidery (StitchCraft) | File › Export › Export As |
| `stitchcraft-settings.wasm` | StitchCraft Design Settings, which keeps a design's settings and machine with the document | Object › Plug-ins |
| `stitchcraft-testsheet.wasm` | StitchCraft Test Sheet, which draws a test sheet | Object › Plug-ins |
| `stitchcraft-tartan.wasm` | StitchCraft Tartan, which sets a tartan's sett | Object › Plug-ins |
| `stitchcraft-stroke-to-satin.wasm` | StitchCraft Stroke to Satin, which makes lines into satin columns | Object › Plug-ins |
| `stitchcraft-route-satins.wasm`, `stitchcraft-route-runs.wasm` | StitchCraft Auto-Route Satin Columns and Auto-Route Running Stitch | Object › Plug-ins |

Build each one in the `stitchcraft` folder, and copy it into a folder for plug-ins, here `plugins`:

```sh
rustup target add wasm32-unknown-unknown
mkdir -p plugins
for m in fill stroke satin auto lettering pes dst settings testsheet tartan stroke-to-satin route-satins route-runs; do
  cargo build --locked -p stitchcraft-vc-plugin --release --target wasm32-unknown-unknown --features "$m"
  cp target/wasm32-unknown-unknown/release/stitchcraft_vc_plugin.wasm "plugins/stitchcraft-$m.wasm"
done
```

Each build writes the same file, `stitchcraft_vc_plugin.wasm`, so copy it before the next. The exporters
and the Fill and Lettering effects need VectorCraft 0.14 or later, Tartan 0.15, Design Settings and Test
Sheet 0.16, and Stroke to Satin and the 2 Auto-Route filters 0.17. [VectorCraft
compatibility](reference/compatibility.md) lists the releases they are tested with.

In VectorCraft's desktop app, choose the `plugins` folder in **Edit › Preferences › Plug-ins & Scratch
Disks › Additional Plug-ins Folder**, which is under **Performance & Storage** up to VectorCraft 0.14.
VectorCraft installs the plug-ins in it whenever it starts, all but Stroke to Satin and the 2 Auto-Route
filters. They ask for more work than a filter usually may do, which VectorCraft lets you allow when you
install each on its own. Choose **Object › Plug-ins › Install Plug-in…** and pick
`stitchcraft-stroke-to-satin.wasm`, and install `stitchcraft-route-satins.wasm` and
`stitchcraft-route-runs.wasm` the same way. VectorCraft asks you first, and once you allow one, keeps it
and installs it again when it starts. In VectorCraft on the web, open each file with **File › Open**
instead, once each time VectorCraft is opened. [Embroider a VectorCraft
design](how-to/embroider-a-vectorcraft-design.md) shows how to use them. Keep the `stitch` program at hand
too: VectorCraft stops the export of a design as large as a traced photo, and `stitch export` writes it
([when VectorCraft stops the export](how-to/embroider-a-vectorcraft-design.md#when-vectorcraft-stops-the-export)).

When VectorCraft asks about installing a filter "which asks for a heavy budget", choose **OK**. From
VectorCraft 0.24.1 the **OK** button is grey for half a second after the question appears, and a
script that controls the app can't choose it for you. To install such a filter without the app, give
VectorCraft's command line `--allow-plugin-grants`:

```sh
vectorcraft-cli run --allow-plugin-grants --cmd plugin.install \
  --params '{"path": "/path/to/plugins/stitchcraft-stroke-to-satin.wasm", "allowHeavy": true}'
```

VectorCraft keeps the filter, and the app installs it again when it starts. To install a filter you
built again in place of the one VectorCraft has, add `"replace": true` to the parameters: the app asks
first. [VectorCraft compatibility](reference/compatibility.md) says which releases ask.

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
